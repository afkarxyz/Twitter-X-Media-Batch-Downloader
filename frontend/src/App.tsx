import { useState, useEffect, useRef, useCallback } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { ArrowLeft } from "lucide-react";
import { getSettings, applyThemeMode, applyFont } from "@/lib/settings";
import { applyTheme } from "@/lib/themes";
import { openExternal } from "@/lib/utils";
import { compareVersionNumbers } from "@/lib/version";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { logger } from "@/lib/logger";
import { initDownloadProgressEvents } from "@/lib/download-state";
import { saveFetchState, getFetchState, clearFetchState, getResumableInfo, mergeTimelines, saveCursor, getCursor, clearCursor, type FetchState, } from "@/lib/fetch-state";
import { TitleBar } from "@/components/TitleBar";
import { Sidebar, type PageType } from "@/components/Sidebar";
import { Header } from "@/components/Header";
import { SearchBar, type FetchMode, type PrivateType, type FetchType, type MultipleAccount } from "@/components/SearchBar";
import { MediaList } from "@/components/MediaList";
import { DatabaseView } from "@/components/DatabaseView";
import { SettingsPage } from "@/components/SettingsPage";
import { DebugLoggerPage } from "@/components/DebugLoggerPage";
import { SupportPage } from "@/components/SupportPage";
import { DependencySetupDialog } from "@/components/DependencySetupDialog";
import type { HistoryItem } from "@/components/FetchHistory";
import type { TwitterResponse } from "@/types/api";
import { ExtractTimeline, ExtractDateRange, SaveAccountToDBWithStatus, CleanupExtractorProcesses, GetAllAccountsFromDB, GetSavedAccountFromDB } from "../wailsjs/go/main/App";
const HISTORY_KEY = "twitter_media_fetch_history";
const MAX_HISTORY = 10;
const CURRENT_VERSION = __APP_VERSION__;
const BATCH_SIZE = 100;
function formatNumberWithComma(num: number): string {
    return num.toLocaleString();
}
function normalizeTimeline(timeline: TwitterResponse["timeline"] | null | undefined): TwitterResponse["timeline"] {
    return Array.isArray(timeline) ? timeline : [];
}
function normalizeTwitterResponse(data: TwitterResponse): TwitterResponse {
    return {
        ...data,
        timeline: normalizeTimeline(data.timeline),
    };
}
async function loadSavedAccountResponse(username: string, mediaType: string): Promise<TwitterResponse | null> {
    try {
        const savedResponseJSON = await GetSavedAccountFromDB(username, mediaType);
        return normalizeTwitterResponse(JSON.parse(savedResponseJSON) as TwitterResponse);
    }
    catch {
        return null;
    }
}
function App() {
    const [currentPage, setCurrentPage] = useState<PageType>("main");
    const [username, setUsername] = useState("");
    const [loading, setLoading] = useState(false);
    const [result, setResult] = useState<TwitterResponse | null>(null);
    const [fetchedMediaType, setFetchedMediaType] = useState<string>("all");
    const [hasUpdate, setHasUpdate] = useState(false);
    const [releaseDate, setReleaseDate] = useState<string | null>(null);
    const [fetchHistory, setFetchHistory] = useState<HistoryItem[]>([]);
    const [resumeInfo, setResumeInfo] = useState<{
        canResume: boolean;
        mediaCount: number;
    } | null>(null);
    const [elapsedTime, setElapsedTime] = useState(0);
    const [remainingTime, setRemainingTime] = useState<number | null>(null);
    const [newMediaCount, setNewMediaCount] = useState<number | null>(null);
    const stopFetchRef = useRef(false);
    const fetchStartTimeRef = useRef<number | null>(null);
    const timeoutIntervalRef = useRef<number | null>(null);
    const [fetchType, setFetchType] = useState<FetchType>("single");
    const [multipleAccounts, setMultipleAccounts] = useState<MultipleAccount[]>([]);
    const [isFetchingAll, setIsFetchingAll] = useState(false);
    const [searchMode, setSearchMode] = useState<FetchMode>("public");
    const [searchPrivateType, setSearchPrivateType] = useState<PrivateType>("bookmarks");
    const [hasUnsavedSettings, setHasUnsavedSettings] = useState(false);
    const [pendingPageChange, setPendingPageChange] = useState<PageType | null>(null);
    const [showUnsavedChangesDialog, setShowUnsavedChangesDialog] = useState(false);
    const [resetSettingsFn, setResetSettingsFn] = useState<(() => void) | null>(null);
    const stopAllRef = useRef(false);
    const accountTimersRef = useRef<Map<string, number>>(new Map());
    const accountStartTimesRef = useRef<Map<string, number>>(new Map());
    const accountStopFlagsRef = useRef<Map<string, boolean>>(new Map());
    const accountMediaCountRef = useRef<Map<string, number>>(new Map());
    const accountTimeoutSecondsRef = useRef<Map<string, number>>(new Map());
    const dbSaveWarningShownRef = useRef(false);
    const reportDatabaseSaveError = (scope: string, error: unknown) => {
        console.error(`Failed to save ${scope} to database:`, error);
        if (!dbSaveWarningShownRef.current) {
            dbSaveWarningShownRef.current = true;
            toast.warning("Fetch completed, but saving to Saved Accounts failed");
        }
    };
    const restorePartialFetchState = async (cleanUsername: string, mediaType: string, showStoppedToast: boolean) => {
        const savedState = getFetchState(cleanUsername);
        if (!savedState || savedState.totalFetched <= 0) {
            if (showStoppedToast) {
                toast.info("Fetch stopped");
            }
            return;
        }
        const canResume = !!savedState.cursor && !savedState.completed;
        if (!canResume) {
            clearFetchState(cleanUsername);
            clearCursor(cleanUsername);
            setResumeInfo(null);
        }
        const partialResponse = await loadSavedAccountResponse(cleanUsername, savedState.mediaType || mediaType);
        if (canResume) {
            setResumeInfo({ canResume: true, mediaCount: savedState.totalFetched });
        }
        if (partialResponse) {
            setResult(partialResponse);
            try {
                await SaveAccountToDBWithStatus(partialResponse.account_info.name, partialResponse.account_info.nick, partialResponse.account_info.profile_image, partialResponse.total_urls, JSON.stringify(partialResponse), savedState.mediaType || mediaType, savedState.cursor || "", savedState.completed);
            }
            catch (dbErr) {
                reportDatabaseSaveError("partial fetch data", dbErr);
            }
        }
        if (showStoppedToast) {
            toast.info(`Stopped at ${formatNumberWithComma(savedState.totalFetched)} items`);
        }
    };
    const stopMultipleFetches = (showToast: boolean) => {
        stopAllRef.current = true;
        setIsFetchingAll(false);
        accountTimersRef.current.forEach((interval) => clearInterval(interval));
        accountTimersRef.current.clear();
        accountStartTimesRef.current.clear();
        accountTimeoutSecondsRef.current.clear();
        setMultipleAccounts((prev) => prev.map((acc) => {
            if (acc.status === "fetching") {
                const mediaCount = accountMediaCountRef.current.get(acc.id) || acc.mediaCount || 0;
                return { ...acc, status: "incomplete" as const, mediaCount, remainingTime: 0 };
            }
            return acc;
        }));
        CleanupExtractorProcesses().catch(() => { });
        if (showToast) {
            toast.info("Stopped all fetches");
        }
    };
    const handleFetchTypeChange = (type: FetchType) => {
        setFetchType(type);
        if (type === "single") {
            if (isFetchingAll || multipleAccounts.some((acc) => acc.status === "fetching")) {
                stopMultipleFetches(false);
            }
        }
        else {
            setResult(null);
        }
    };
    useEffect(() => {
        const settings = getSettings();
        applyThemeMode(settings.themeMode);
        applyTheme(settings.theme);
        applyFont(settings.fontFamily, settings.customFonts);
        const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
        const handleChange = () => {
            const currentSettings = getSettings();
            if (currentSettings.themeMode === "auto") {
                applyThemeMode("auto");
                applyTheme(currentSettings.theme);
            }
        };
        mediaQuery.addEventListener("change", handleChange);
        const cleanupDownloadProgress = initDownloadProgressEvents();
        checkForUpdates();
        loadHistory();
        return () => {
            mediaQuery.removeEventListener("change", handleChange);
            cleanupDownloadProgress();
        };
    }, []);
    useEffect(() => {
        const handleDocumentClick = (event: MouseEvent) => {
            if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
                return;
            }
            const target = event.target;
            if (!(target instanceof Element)) {
                return;
            }
            const anchor = target.closest("a[href]");
            if (!(anchor instanceof HTMLAnchorElement)) {
                return;
            }
            const href = anchor.getAttribute("href");
            if (!href || href.startsWith("#") || href.startsWith("javascript:")) {
                return;
            }
            try {
                const url = new URL(href, window.location.href);
                if (url.protocol !== "http:" && url.protocol !== "https:") {
                    return;
                }
                event.preventDefault();
                openExternal(url.toString());
            }
            catch {
            }
        };
        document.addEventListener("click", handleDocumentClick);
        return () => {
            document.removeEventListener("click", handleDocumentClick);
        };
    }, []);
    const checkForUpdates = async () => {
        try {
            const response = await fetch("https://api.github.com/repos/afkarxyz/Twitter-X-Media-Batch-Downloader/releases/latest");
            const data = await response.json();
            const latestVersion = data.tag_name?.replace(/^v/, "") || "";
            if (data.published_at) {
                setReleaseDate(data.published_at);
            }
            if (latestVersion && compareVersionNumbers(latestVersion, CURRENT_VERSION) > 0) {
                setHasUpdate(true);
            }
        }
        catch (err) {
            console.error("Failed to check for updates:", err);
        }
    };
    const loadHistory = () => {
        try {
            const saved = localStorage.getItem(HISTORY_KEY);
            if (saved) {
                setFetchHistory(JSON.parse(saved));
            }
        }
        catch (err) {
            console.error("Failed to load history:", err);
        }
    };
    const saveHistory = (history: HistoryItem[]) => {
        try {
            localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
        }
        catch (err) {
            console.error("Failed to save history:", err);
        }
    };
    const addToHistory = (data: TwitterResponse, inputUsername: string) => {
        let cleanUsername = inputUsername.trim();
        if (cleanUsername.startsWith("@")) {
            cleanUsername = cleanUsername.slice(1);
        }
        if (cleanUsername.includes("x.com/") || cleanUsername.includes("twitter.com/")) {
            const match = cleanUsername.match(/(?:x\.com|twitter\.com)\/([^/?]+)/);
            if (match)
                cleanUsername = match[1];
        }
        setFetchHistory((prev) => {
            const apiUsername = data.account_info.name;
            const filtered = prev.filter((h) => h.username.toLowerCase() !== apiUsername.toLowerCase());
            const newItem: HistoryItem = {
                id: crypto.randomUUID(),
                username: apiUsername,
                name: data.account_info.nick,
                image: data.account_info.profile_image,
                mediaCount: data.total_urls,
                timestamp: Date.now(),
            };
            const updated = [newItem, ...filtered].slice(0, MAX_HISTORY);
            saveHistory(updated);
            return updated;
        });
    };
    const removeFromHistory = (id: string) => {
        setFetchHistory((prev) => {
            const updated = prev.filter((h) => h.id !== id);
            saveHistory(updated);
            return updated;
        });
    };
    const handleHistorySelect = (item: HistoryItem) => {
        setUsername(item.username);
    };
    const checkResumable = useCallback((user: string) => {
        if (!user.trim()) {
            setResumeInfo(null);
            return;
        }
        const info = getResumableInfo(user.trim());
        setResumeInfo(info.canResume ? { canResume: true, mediaCount: info.mediaCount } : null);
    }, []);
    useEffect(() => {
        checkResumable(username);
    }, [username, checkResumable]);
    const handleStopFetch = useCallback(async () => {
        stopFetchRef.current = true;
        logger.info("Stopping fetch...");
        toast.info("Stopping...");
        try {
            await CleanupExtractorProcesses();
        }
        catch (cleanupError) {
            console.error("Failed to stop extractor processes:", cleanupError);
        }
    }, []);
    useEffect(() => {
        if (loading && fetchStartTimeRef.current !== null) {
            const settings = getSettings();
            const timeoutSeconds = settings.fetchTimeout || 60;
            timeoutIntervalRef.current = window.setInterval(() => {
                if (fetchStartTimeRef.current !== null) {
                    const elapsed = Math.floor((Date.now() - fetchStartTimeRef.current) / 1000);
                    setElapsedTime(elapsed);
                    const remaining = Math.max(0, timeoutSeconds - elapsed);
                    setRemainingTime(remaining);
                    if (remaining <= 0) {
                        stopFetchRef.current = true;
                        logger.warning("Fetch timeout reached. Stopping...");
                        toast.warning("Fetch timeout reached. Stopping...");
                        handleStopFetch();
                    }
                }
            }, 1000);
        }
        else {
            if (timeoutIntervalRef.current !== null) {
                clearInterval(timeoutIntervalRef.current);
                timeoutIntervalRef.current = null;
            }
            setElapsedTime(0);
            setRemainingTime(null);
            fetchStartTimeRef.current = null;
        }
        return () => {
            if (timeoutIntervalRef.current !== null) {
                clearInterval(timeoutIntervalRef.current);
                timeoutIntervalRef.current = null;
            }
        };
    }, [loading, handleStopFetch]);
    const handleFetch = async (useDateRange: boolean, startDate?: string, endDate?: string, mediaType?: string, retweets?: boolean, mode: FetchMode = "public", privateType?: PrivateType, authToken?: string, isResume?: boolean) => {
        if (loading) {
            logger.warning("Fetch already in progress, please wait or stop first");
            toast.warning("Fetch already in progress");
            return;
        }
        const isBookmarks = mode === "private" && privateType === "bookmarks";
        const isLikes = mode === "private" && privateType === "likes";
        if (!isBookmarks && !username.trim()) {
            toast.error("Please enter a username");
            return;
        }
        if (!authToken?.trim()) {
            toast.error("Please enter your auth token");
            return;
        }
        setLoading(true);
        setFetchedMediaType(mediaType || "all");
        stopFetchRef.current = false;
        fetchStartTimeRef.current = Date.now();
        setElapsedTime(0);
        setNewMediaCount(null);
        const settings = getSettings();
        const timeoutSeconds = settings.fetchTimeout || 60;
        setRemainingTime(timeoutSeconds);
        try {
            await CleanupExtractorProcesses();
        }
        catch (cleanupError) {
            console.error("Failed to clean extractor processes before fetch:", cleanupError);
        }
        const fetchTarget = isBookmarks
            ? "your bookmarks"
            : isLikes
                ? "your likes"
                : `@${username}`;
        const cleanUsername = isBookmarks ? "bookmarks" : isLikes ? "likes" : username.trim();
        const requestedMediaType = mediaType || "all";
        let existingState: FetchState | null = null;
        let cursor: string | undefined;
        let allTimeline: TwitterResponse["timeline"] = [];
        let accountInfo: TwitterResponse["account_info"] | null = null;
        if (isResume) {
            existingState = getFetchState(cleanUsername);
            if (existingState && existingState.cursor && !existingState.completed) {
                cursor = existingState.cursor;
                const savedResponse = await loadSavedAccountResponse(cleanUsername, existingState.mediaType || requestedMediaType);
                if (!savedResponse) {
                    clearFetchState(cleanUsername);
                    clearCursor(cleanUsername);
                    toast.error("Resume data is no longer available");
                    setLoading(false);
                    return;
                }
                allTimeline = savedResponse.timeline;
                accountInfo = savedResponse.account_info;
                logger.info(`Resuming ${fetchTarget} from ${existingState.totalFetched} items...`);
                setResult(savedResponse);
            }
            else {
                toast.error("No resumable fetch found");
                setLoading(false);
                return;
            }
        }
        else {
            clearFetchState(cleanUsername);
            setResult(null);
            logger.info(`Fetching ${fetchTarget}...`);
        }
        try {
            let finalData: TwitterResponse | null = null;
            if (useDateRange && startDate && endDate && mode === "public") {
                logger.info(`Using date range: ${startDate} to ${endDate}`);
                if (fetchStartTimeRef.current !== null) {
                    const settings = getSettings();
                    const timeoutSeconds = settings.fetchTimeout || 60;
                    const elapsed = Math.floor((Date.now() - fetchStartTimeRef.current) / 1000);
                    if (elapsed >= timeoutSeconds) {
                        logger.warning(`Timeout reached (${timeoutSeconds}s). Stopping fetch...`);
                        stopFetchRef.current = true;
                        toast.warning("Fetch timeout reached. Stopping...");
                        setLoading(false);
                        return;
                    }
                }
                const response = await ExtractDateRange({
                    username: username.trim(),
                    auth_token: authToken.trim(),
                    start_date: startDate,
                    end_date: endDate,
                    media_filter: requestedMediaType,
                    retweets: retweets || false,
                });
                finalData = normalizeTwitterResponse(JSON.parse(response) as TwitterResponse);
                if (finalData && finalData.account_info) {
                    try {
                        await SaveAccountToDBWithStatus(finalData.account_info.name, finalData.account_info.nick, finalData.account_info.profile_image, finalData.total_urls, JSON.stringify(finalData), requestedMediaType, finalData.cursor || "", finalData.completed ?? true);
                    }
                    catch (err) {
                        reportDatabaseSaveError("date range data", err);
                    }
                }
            }
            else {
                const batchSize = BATCH_SIZE;
                let timelineType = (settings.includeText || retweets) ? "tweets" : "media";
                if (isBookmarks) {
                    timelineType = "bookmarks";
                }
                else if (isLikes) {
                    timelineType = "likes";
                }
                let hasMore = true;
                let page = 0;
                const seenCursors = new Set<string>();
                while (hasMore && !stopFetchRef.current) {
                    if (fetchStartTimeRef.current !== null) {
                        const settings = getSettings();
                        const timeoutSeconds = settings.fetchTimeout || 60;
                        const elapsed = Math.floor((Date.now() - fetchStartTimeRef.current) / 1000);
                        if (elapsed >= timeoutSeconds) {
                            logger.warning(`Timeout reached (${timeoutSeconds}s). Stopping fetch...`);
                            stopFetchRef.current = true;
                            toast.warning("Fetch timeout reached. Stopping...");
                            if (accountInfo && allTimeline.length > 0) {
                                saveFetchState({
                                    username: cleanUsername,
                                    cursor: cursor || "",
                                    totalFetched: allTimeline.length,
                                    completed: false,
                                    mediaType: requestedMediaType,
                                    retweets: retweets || false,
                                    timelineType: timelineType,
                                });
                                const timeoutResponse: TwitterResponse = {
                                    account_info: accountInfo,
                                    timeline: allTimeline,
                                    total_urls: allTimeline.length,
                                    metadata: {
                                        new_entries: 0,
                                        page: page,
                                        batch_size: batchSize,
                                        has_more: hasMore,
                                        cursor: cursor,
                                        completed: false,
                                    },
                                    cursor: cursor,
                                    completed: false,
                                };
                                try {
                                    await SaveAccountToDBWithStatus(accountInfo.name, accountInfo.nick, accountInfo.profile_image, allTimeline.length, JSON.stringify(timeoutResponse), requestedMediaType, cursor || "", false);
                                    logger.info(`Saved ${allTimeline.length} items before timeout`);
                                }
                                catch (err) {
                                    reportDatabaseSaveError("timeout state", err);
                                }
                            }
                            break;
                        }
                    }
                    const batchNum = page + 1;
                    logger.info(`Fetching batch ${batchNum}${cursor ? " (resuming)" : ""}...`);
                    const response = await ExtractTimeline({
                        username: isBookmarks ? "" : username.trim(),
                        auth_token: authToken.trim(),
                        timeline_type: timelineType,
                        batch_size: batchSize,
                        page: page,
                        media_type: requestedMediaType,
                        retweets: retweets || false,
                        cursor: cursor,
                        include_photos: settings.includePhotos,
                        include_videos: settings.includeVideos,
                        include_gifs: settings.includeGifs,
                        include_text: settings.includeText,
                        video_quality: settings.videoQuality,
                        image_size: settings.imageSize,
                    });
                    const data = normalizeTwitterResponse(JSON.parse(response) as TwitterResponse);
                    if (!accountInfo && data.account_info) {
                        accountInfo = data.account_info;
                        if (isBookmarks) {
                            accountInfo.name = "bookmarks";
                            accountInfo.nick = "My Bookmarks";
                        }
                        else if (isLikes) {
                            accountInfo.name = "likes";
                            accountInfo.nick = "My Likes";
                        }
                    }
                    const previousCount = allTimeline.length;
                    allTimeline = mergeTimelines(allTimeline, data.timeline);
                    const newCount = allTimeline.length - previousCount;
                    if (newCount > 0) {
                        setNewMediaCount(newCount);
                        setTimeout(() => setNewMediaCount(null), 1000);
                    }
                    cursor = data.cursor;
                    hasMore = !!data.cursor && !data.completed;
                    if (hasMore && cursor) {
                        if (seenCursors.has(cursor) || (data.timeline.length > 0 && newCount === 0)) {
                            hasMore = false;
                        }
                        else {
                            seenCursors.add(cursor);
                        }
                    }
                    page++;
                    if (cursor) {
                        saveCursor(cleanUsername, cursor);
                    }
                    if (accountInfo) {
                        setResult({
                            account_info: accountInfo,
                            timeline: allTimeline,
                            total_urls: allTimeline.length,
                            metadata: {
                                new_entries: data.timeline.length,
                                page: page,
                                batch_size: batchSize,
                                has_more: hasMore,
                                cursor: cursor,
                                completed: !hasMore,
                            },
                            cursor: cursor,
                            completed: !hasMore,
                        });
                    }
                    logger.info(`Fetched ${allTimeline.length} items total`);
                    saveFetchState({
                        username: cleanUsername,
                        cursor: cursor || "",
                        totalFetched: allTimeline.length,
                        completed: !hasMore,
                        mediaType: requestedMediaType,
                        retweets: retweets || false,
                        timelineType: timelineType,
                    });
                    if (accountInfo) {
                        const currentResponse: TwitterResponse = {
                            account_info: accountInfo,
                            timeline: allTimeline,
                            total_urls: allTimeline.length,
                            metadata: {
                                new_entries: data.timeline.length,
                                page: page,
                                batch_size: batchSize,
                                has_more: hasMore,
                                cursor: cursor,
                                completed: !hasMore,
                            },
                            cursor: cursor,
                            completed: !hasMore,
                        };
                        try {
                            await SaveAccountToDBWithStatus(accountInfo.name, accountInfo.nick, accountInfo.profile_image, allTimeline.length, JSON.stringify(currentResponse), requestedMediaType, cursor || "", !hasMore);
                        }
                        catch (err) {
                            reportDatabaseSaveError("fetch progress", err);
                        }
                    }
                }
                if (stopFetchRef.current && hasMore && cursor) {
                    const elapsedSecs = fetchStartTimeRef.current ? Math.floor((Date.now() - fetchStartTimeRef.current) / 1000) : 0;
                    logger.info(`Stopped at ${allTimeline.length} items - can resume later (${elapsedSecs}s)`);
                    toast.info(`Stopped at ${formatNumberWithComma(allTimeline.length)} items`);
                    setResumeInfo({ canResume: true, mediaCount: allTimeline.length });
                }
                else {
                    clearFetchState(cleanUsername);
                    clearCursor(cleanUsername);
                    setResumeInfo(null);
                }
                finalData = accountInfo
                    ? {
                        account_info: accountInfo,
                        timeline: allTimeline,
                        total_urls: allTimeline.length,
                        metadata: {
                            new_entries: allTimeline.length,
                            page: page,
                            batch_size: batchSize,
                            has_more: false,
                            cursor: cursor,
                            completed: !stopFetchRef.current,
                        },
                        cursor: cursor,
                        completed: !stopFetchRef.current,
                    }
                    : null;
            }
            if (finalData) {
                setResult(finalData);
                if (mode === "public") {
                    addToHistory(finalData, username);
                }
                if (!useDateRange) {
                    try {
                        await SaveAccountToDBWithStatus(finalData.account_info.name, finalData.account_info.nick, finalData.account_info.profile_image, finalData.total_urls, JSON.stringify(finalData), requestedMediaType, finalData.cursor || "", finalData.completed ?? true);
                    }
                    catch (err) {
                        reportDatabaseSaveError("final fetch status", err);
                    }
                }
                if (!stopFetchRef.current) {
                    const elapsedSecs = fetchStartTimeRef.current ? Math.floor((Date.now() - fetchStartTimeRef.current) / 1000) : 0;
                    logger.success(`Found ${finalData.total_urls} media items (${elapsedSecs}s)`);
                    toast.success(`${finalData.total_urls} media items found`);
                }
            }
        }
        catch (error) {
            const errorMsg = error instanceof Error ? error.message : String(error);
            const elapsedSecs = fetchStartTimeRef.current ? Math.floor((Date.now() - fetchStartTimeRef.current) / 1000) : 0;
            if (stopFetchRef.current) {
                logger.info(`Stopped fetch after ${elapsedSecs}s`);
                await restorePartialFetchState(cleanUsername, requestedMediaType, true);
                return;
            }
            logger.error(`Failed to fetch: ${errorMsg} (${elapsedSecs}s)`);
            if (errorMsg.toLowerCase().includes("xtractor dependency") || errorMsg.toLowerCase().includes("extractor dependency")) {
                toast.error(errorMsg);
            }
            else {
                toast.error("Failed to fetch media");
            }
            const savedState = getFetchState(cleanUsername);
            if (savedState && savedState.totalFetched > 0) {
                await restorePartialFetchState(cleanUsername, requestedMediaType, false);
                toast.info(`Saved ${formatNumberWithComma(savedState.totalFetched)} items - can resume`);
            }
        }
        finally {
            setLoading(false);
            fetchStartTimeRef.current = null;
            setElapsedTime(0);
            setRemainingTime(null);
        }
    };
    const handleResume = (authToken: string, mediaType?: string, retweets?: boolean) => {
        handleFetch(false, undefined, undefined, mediaType, retweets, "public", undefined, authToken, true);
    };
    const handleClearResume = () => {
        if (username.trim()) {
            clearFetchState(username.trim());
            clearCursor(username.trim());
            setResumeInfo(null);
            toast.info("Resume data cleared");
        }
    };
    const handleLoadFromDB = (responseJSON: string, loadedUsername: string) => {
        try {
            const data = normalizeTwitterResponse(JSON.parse(responseJSON) as TwitterResponse);
            setResult(data);
            setUsername(loadedUsername);
            setCurrentPage("main");
            setFetchType("single");
            const isPrivate = loadedUsername === "bookmarks" || loadedUsername === "likes";
            if (isPrivate) {
                if (loadedUsername === "bookmarks") {
                    setSearchMode("private");
                    setSearchPrivateType("bookmarks");
                }
                else if (loadedUsername === "likes") {
                    setSearchMode("private");
                    setSearchPrivateType("likes");
                }
            }
            else {
                setSearchMode("public");
            }
            toast.success(`Loaded @${loadedUsername} from database`);
        }
        catch (error) {
            console.error("Failed to parse saved data:", error);
            toast.error("Failed to parse saved data");
        }
    };
    const handleBackToHomeResult = () => {
        setCurrentPage("main");
        setResult(null);
        setNewMediaCount(null);
        const scrollElement = document.getElementById("app-content-scroll");
        scrollElement?.scrollTo({ top: 0, behavior: "smooth" });
    };
    const parseUsername = (input: string): string => {
        let clean = input.trim();
        if (clean.startsWith("@")) {
            clean = clean.slice(1);
        }
        if (clean.includes("x.com/") || clean.includes("twitter.com/")) {
            const match = clean.match(/(?:x\.com|twitter\.com)\/([^/?]+)/);
            if (match)
                clean = match[1];
        }
        return clean;
    };
    const handleUpdateSelected = (usernames: string[]) => {
        if (usernames.length === 0) {
            toast.error("No accounts selected");
            return;
        }
        const accounts: MultipleAccount[] = usernames.map((username) => {
            const cleanUsername = parseUsername(username);
            return {
                id: crypto.randomUUID(),
                username: cleanUsername,
                status: "pending",
                mediaCount: 0,
                previousMediaCount: 0,
                elapsedTime: 0,
                remainingTime: null,
                showDiff: false,
            };
        });
        setMultipleAccounts((prev) => {
            const existingUsernames = new Set(prev.map((acc) => acc.username.toLowerCase()));
            const newAccounts = accounts.filter((acc) => !existingUsernames.has(acc.username.toLowerCase()));
            return [...prev, ...newAccounts];
        });
        setFetchType("multiple");
        setCurrentPage("main");
        toast.success(`Added ${formatNumberWithComma(accounts.length)} account(s) to multiple fetch`);
    };
    const handleImportAccounts = (entries: string[]) => {
        const seenUsernames = new Set<string>();
        const uniqueUsernames = entries
            .map((entry) => parseUsername(entry))
            .filter((entry) => entry.length > 0)
            .filter((entry) => {
            const normalized = entry.toLowerCase();
            if (seenUsernames.has(normalized)) {
                return false;
            }
            seenUsernames.add(normalized);
            return true;
        });
        if (uniqueUsernames.length === 0) {
            toast.error("No valid accounts found");
            return;
        }
        const accounts: MultipleAccount[] = uniqueUsernames.map((entry) => ({
            id: crypto.randomUUID(),
            username: entry,
            status: "pending",
            mediaCount: 0,
            previousMediaCount: 0,
            elapsedTime: 0,
            remainingTime: null,
            showDiff: false,
        }));
        setMultipleAccounts(accounts);
        toast.success(`Imported ${formatNumberWithComma(accounts.length)} account(s)`);
    };
    const handleClearMultipleAccounts = () => {
        accountTimersRef.current.forEach((interval) => clearInterval(interval));
        accountTimersRef.current.clear();
        accountStartTimesRef.current.clear();
        accountTimeoutSecondsRef.current.clear();
        accountStopFlagsRef.current.clear();
        accountMediaCountRef.current.clear();
        stopAllRef.current = false;
        setIsFetchingAll(false);
        setMultipleAccounts([]);
        toast.info("Multiple fetch list cleared");
    };
    const handleRemoveMultipleAccount = (accountId: string) => {
        const account = multipleAccounts.find((entry) => entry.id === accountId);
        accountTimersRef.current.delete(accountId);
        accountStartTimesRef.current.delete(accountId);
        accountTimeoutSecondsRef.current.delete(accountId);
        accountStopFlagsRef.current.delete(accountId);
        accountMediaCountRef.current.delete(accountId);
        setMultipleAccounts((prev) => prev.filter((entry) => entry.id !== accountId));
        if (account) {
            toast.info(`Removed @${account.username} from list`);
        }
    };
    const handleFetchAll = async (authToken: string) => {
        if (multipleAccounts.length === 0) {
            toast.error("No accounts to fetch");
            return;
        }
        setIsFetchingAll(true);
        stopAllRef.current = false;
        if (!authToken.trim()) {
            toast.error("Please enter your auth token");
            setIsFetchingAll(false);
            return;
        }
        const settings = getSettings();
        const selectedMediaType = "all";
        setFetchedMediaType(selectedMediaType);
        const timeoutSeconds = settings.fetchTimeout || 60;
        setMultipleAccounts((prev) => prev.map((acc) => ({
            ...acc,
            status: "pending" as const,
            mediaCount: 0,
            previousMediaCount: 0,
            elapsedTime: 0,
            remainingTime: timeoutSeconds,
            error: undefined,
            showDiff: false,
        })));
        const currentAccounts = [...multipleAccounts];
        for (let i = 0; i < currentAccounts.length; i++) {
            if (stopAllRef.current) {
                break;
            }
            const account = currentAccounts[i];
            const accountId = account.id;
            accountStopFlagsRef.current.set(accountId, false);
            setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                ? { ...acc, status: "fetching" as const }
                : acc));
            const settings = getSettings();
            const timeoutSeconds = settings.fetchTimeout || 60;
            accountStartTimesRef.current.set(accountId, Date.now());
            accountTimeoutSecondsRef.current.set(accountId, timeoutSeconds);
            setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                ? { ...acc, elapsedTime: 0, remainingTime: timeoutSeconds }
                : acc));
            const timerInterval = window.setInterval(() => {
                const startTime = accountStartTimesRef.current.get(accountId);
                const timeoutSecs = accountTimeoutSecondsRef.current.get(accountId) || timeoutSeconds;
                if (startTime) {
                    const elapsed = Math.floor((Date.now() - startTime) / 1000);
                    const remaining = Math.max(0, timeoutSecs - elapsed);
                    setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                        ? { ...acc, elapsedTime: elapsed, remainingTime: remaining }
                        : acc));
                    if (remaining <= 0) {
                        accountStopFlagsRef.current.set(accountId, true);
                        clearInterval(timerInterval);
                        accountTimersRef.current.delete(accountId);
                        accountStartTimesRef.current.delete(accountId);
                        accountTimeoutSecondsRef.current.delete(accountId);
                        const mediaCount = accountMediaCountRef.current.get(accountId) || 0;
                        setMultipleAccounts((prev) => prev.map((acc) => {
                            if (acc.id === accountId) {
                                const status = mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
                                return { ...acc, status, remainingTime: 0, mediaCount };
                            }
                            return acc;
                        }));
                        CleanupExtractorProcesses().catch(() => { });
                    }
                }
            }, 1000);
            accountTimersRef.current.set(accountId, timerInterval);
            try {
                if (stopAllRef.current) {
                    clearInterval(timerInterval);
                    accountTimersRef.current.delete(accountId);
                    accountStartTimesRef.current.delete(accountId);
                    accountTimeoutSecondsRef.current.delete(accountId);
                    setMultipleAccounts((prev) => prev.map((acc) => {
                        if (acc.id === accountId) {
                            const status = acc.mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
                            return { ...acc, status };
                        }
                        return acc;
                    }));
                    continue;
                }
                const batchSizeMultiple = BATCH_SIZE;
                const cleanUsername = account.username.trim();
                let allTimeline: TwitterResponse["timeline"] = [];
                let accountInfo: TwitterResponse["account_info"] | null = null;
                let cursor: string | undefined;
                let hasMore = true;
                let page = 0;
                const seenCursors = new Set<string>();
                let previousMediaCount = account.mediaCount || 0;
                while (hasMore && !stopAllRef.current) {
                    if (accountStopFlagsRef.current.get(accountId)) {
                        if (accountInfo && allTimeline.length > 0) {
                            saveFetchState({
                                username: cleanUsername,
                                cursor: cursor || "",
                                totalFetched: allTimeline.length,
                                completed: false,
                                mediaType: selectedMediaType,
                                retweets: false,
                                timelineType: "timeline",
                            });
                            try {
                                await SaveAccountToDBWithStatus(accountInfo.name, accountInfo.nick, accountInfo.profile_image, allTimeline.length, JSON.stringify({
                                    account_info: accountInfo,
                                    timeline: allTimeline,
                                    total_urls: allTimeline.length,
                                    metadata: {
                                        new_entries: 0,
                                        page: page,
                                        batch_size: batchSizeMultiple,
                                        has_more: hasMore,
                                        cursor: cursor,
                                        completed: false,
                                    },
                                    cursor: cursor,
                                    completed: false,
                                }), selectedMediaType, cursor || "", false);
                            }
                            catch (err) {
                                reportDatabaseSaveError("multi-account timeout state", err);
                            }
                        }
                        break;
                    }
                    const response = await ExtractTimeline({
                        username: cleanUsername,
                        auth_token: authToken.trim(),
                        timeline_type: (settings.includeText || settings.includeRetweets) ? "tweets" : "media",
                        batch_size: batchSizeMultiple,
                        page: page,
                        media_type: selectedMediaType,
                        retweets: settings.includeRetweets,
                        cursor: cursor,
                        include_photos: settings.includePhotos,
                        include_videos: settings.includeVideos,
                        include_gifs: settings.includeGifs,
                        include_text: settings.includeText,
                        video_quality: settings.videoQuality,
                        image_size: settings.imageSize,
                    });
                    const data = normalizeTwitterResponse(JSON.parse(response) as TwitterResponse);
                    if (!accountInfo && data.account_info) {
                        accountInfo = data.account_info;
                    }
                    const previousMergeCount = allTimeline.length;
                    allTimeline = mergeTimelines(allTimeline, data.timeline);
                    cursor = data.cursor;
                    hasMore = !!data.cursor && !data.completed;
                    if (hasMore && cursor) {
                        if (seenCursors.has(cursor) || (data.timeline.length > 0 && allTimeline.length === previousMergeCount)) {
                            hasMore = false;
                        }
                        else {
                            seenCursors.add(cursor);
                        }
                    }
                    page++;
                    if (cursor) {
                        saveCursor(cleanUsername, cursor);
                    }
                    const currentMediaCount = allTimeline.length;
                    accountMediaCountRef.current.set(accountId, currentMediaCount);
                    const hasNewItems = currentMediaCount > previousMediaCount;
                    setMultipleAccounts((prev) => prev.map((acc) => {
                        if (acc.id === accountId) {
                            return {
                                ...acc,
                                accountInfo: accountInfo || undefined,
                                previousMediaCount: previousMediaCount,
                                mediaCount: currentMediaCount,
                                showDiff: hasNewItems,
                                cursor: cursor,
                            };
                        }
                        return acc;
                    }));
                    if (hasNewItems) {
                        setTimeout(() => {
                            setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId ? { ...acc, showDiff: false } : acc));
                        }, 1000);
                    }
                    previousMediaCount = currentMediaCount;
                    if (accountInfo) {
                        saveFetchState({
                            username: cleanUsername,
                            cursor: cursor || "",
                            totalFetched: allTimeline.length,
                            completed: !hasMore,
                            mediaType: selectedMediaType,
                            retweets: false,
                            timelineType: "timeline",
                        });
                    }
                    if (accountInfo) {
                        try {
                            await SaveAccountToDBWithStatus(accountInfo.name, accountInfo.nick, accountInfo.profile_image, allTimeline.length, JSON.stringify({
                                account_info: accountInfo,
                                timeline: allTimeline,
                                total_urls: allTimeline.length,
                                metadata: {
                                    new_entries: data.timeline.length,
                                    page: page,
                                    batch_size: batchSizeMultiple,
                                    has_more: hasMore,
                                    cursor: cursor,
                                    completed: !hasMore,
                                },
                                cursor: cursor,
                                completed: !hasMore,
                            }), selectedMediaType, cursor || "", !hasMore);
                        }
                        catch (err) {
                            reportDatabaseSaveError("multi-account progress", err);
                        }
                    }
                }
                const startTime = accountStartTimesRef.current.get(accountId);
                const elapsedSecs = startTime ? Math.floor((Date.now() - startTime) / 1000) : 0;
                clearInterval(timerInterval);
                accountTimersRef.current.delete(accountId);
                accountStartTimesRef.current.delete(accountId);
                accountTimeoutSecondsRef.current.delete(accountId);
                const finalMediaCount = allTimeline.length;
                accountMediaCountRef.current.set(accountId, finalMediaCount);
                const wasTimeout = accountStopFlagsRef.current.get(accountId);
                if (wasTimeout) {
                    logger.warning(`@${account.username}: timeout - ${finalMediaCount} items (${elapsedSecs}s)`);
                    setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                        ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
                        : acc));
                }
                else if (stopAllRef.current) {
                    logger.info(`@${account.username}: stopped - ${finalMediaCount} items (${elapsedSecs}s)`);
                    setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                        ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
                        : acc));
                }
                else if (hasMore) {
                    logger.warning(`@${account.username}: incomplete - ${finalMediaCount} items (${elapsedSecs}s)`);
                    setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                        ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
                        : acc));
                }
                else {
                    logger.success(`@${account.username}: completed - ${finalMediaCount} items (${elapsedSecs}s)`);
                    setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                        ? { ...acc, status: "completed" as const }
                        : acc));
                }
            }
            catch (error) {
                const startTime = accountStartTimesRef.current.get(accountId);
                const elapsedSecs = startTime ? Math.floor((Date.now() - startTime) / 1000) : 0;
                const timerInterval = accountTimersRef.current.get(accountId);
                if (timerInterval) {
                    clearInterval(timerInterval);
                    accountTimersRef.current.delete(accountId);
                }
                accountStartTimesRef.current.delete(accountId);
                accountTimeoutSecondsRef.current.delete(accountId);
                const errorMsg = error instanceof Error ? error.message : String(error);
                const mediaCount = accountMediaCountRef.current.get(accountId) || 0;
                if (stopAllRef.current || accountStopFlagsRef.current.get(accountId)) {
                    accountMediaCountRef.current.delete(accountId);
                    setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                        ? {
                            ...acc,
                            status: "incomplete" as const,
                            error: undefined,
                            mediaCount,
                        }
                        : acc));
                    logger.info(`@${account.username}: stopped - ${mediaCount} items (${elapsedSecs}s)`);
                    continue;
                }
                const isAuthError = errorMsg.toLowerCase().includes("401") ||
                    errorMsg.toLowerCase().includes("unauthorized") ||
                    errorMsg.toLowerCase().includes("auth token may be invalid") ||
                    errorMsg.toLowerCase().includes("auth token may be expired") ||
                    errorMsg.toLowerCase().includes("invalid or expired");
                const status = isAuthError || mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
                accountMediaCountRef.current.delete(accountId);
                setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                    ? {
                        ...acc,
                        status,
                        error: errorMsg,
                        mediaCount,
                    }
                    : acc));
                logger.error(`@${account.username}: failed - ${errorMsg} (${elapsedSecs}s)`);
            }
        }
        setIsFetchingAll(false);
        if (!stopAllRef.current) {
            toast.success("All accounts fetched");
        }
    };
    const handleStopAll = () => {
        stopMultipleFetches(true);
    };
    const handleStopAccount = (accountId: string) => {
        accountStopFlagsRef.current.set(accountId, true);
        setMultipleAccounts((prev) => prev.map((acc) => {
            if (acc.id === accountId && acc.status === "fetching") {
                const mediaCount = accountMediaCountRef.current.get(accountId) || acc.mediaCount || 0;
                return { ...acc, status: "incomplete" as const, mediaCount };
            }
            return acc;
        }));
        const timerInterval = accountTimersRef.current.get(accountId);
        if (timerInterval) {
            clearInterval(timerInterval);
            accountTimersRef.current.delete(accountId);
        }
        accountStartTimesRef.current.delete(accountId);
        accountTimeoutSecondsRef.current.delete(accountId);
        CleanupExtractorProcesses().catch(() => { });
        toast.info("Stopped account fetch");
    };
    const handlePageChange = (page: PageType) => {
        if (currentPage === "settings" && hasUnsavedSettings && page !== "settings") {
            setPendingPageChange(page);
            setShowUnsavedChangesDialog(true);
            return;
        }
        setCurrentPage(page);
    };
    const handleDiscardChanges = () => {
        setShowUnsavedChangesDialog(false);
        if (resetSettingsFn) {
            resetSettingsFn();
        }
        const savedSettings = getSettings();
        applyThemeMode(savedSettings.themeMode);
        applyTheme(savedSettings.theme);
        applyFont(savedSettings.fontFamily, savedSettings.customFonts);
        setHasUnsavedSettings(false);
        if (pendingPageChange) {
            setCurrentPage(pendingPageChange);
            setPendingPageChange(null);
        }
    };
    const handleCancelNavigation = () => {
        setShowUnsavedChangesDialog(false);
        setPendingPageChange(null);
    };
    const handleRetryAccount = async (accountId: string, authToken: string) => {
        const account = multipleAccounts.find((acc) => acc.id === accountId);
        if (!account)
            return;
        if (!authToken.trim()) {
            toast.error("Please enter your auth token");
            return;
        }
        const retrySettings = getSettings();
        const selectedMediaType = fetchedMediaType || "all";
        setFetchedMediaType(selectedMediaType);
        const batchSizeRetry = BATCH_SIZE;
        const cleanUsername = account.username.trim();
        let existingState: FetchState | null = null;
        let cursor: string | undefined = account.cursor;
        let allTimeline: TwitterResponse["timeline"] = [];
        let accountInfo: TwitterResponse["account_info"] | null = null;
        let previousMediaCount = account.mediaCount || 0;
        existingState = getFetchState(cleanUsername);
        if (existingState && existingState.cursor && !existingState.completed) {
            cursor = existingState.cursor;
            const resumeMediaType = existingState.mediaType || selectedMediaType;
            const canonicalHandle = account.accountInfo?.name?.trim();
            let savedResponse = await loadSavedAccountResponse(cleanUsername, resumeMediaType);
            if (!savedResponse && canonicalHandle && canonicalHandle.toLowerCase() !== cleanUsername.toLowerCase()) {
                savedResponse = await loadSavedAccountResponse(canonicalHandle, resumeMediaType);
            }
            if (savedResponse) {
                allTimeline = savedResponse.timeline;
                accountInfo = savedResponse.account_info;
                previousMediaCount = existingState.totalFetched;
                logger.info(`Resuming @${cleanUsername} from ${existingState.totalFetched} items...`);
            }
            else {
                clearFetchState(cleanUsername);
                clearCursor(cleanUsername);
                logger.warning(`Saved data for @${cleanUsername} is missing (Saved Accounts entry was cleared). Re-fetching from the beginning...`);
                toast.warning(`@${cleanUsername}: saved progress was lost, re-fetching from the start`);
                cursor = undefined;
                allTimeline = [];
                accountInfo = null;
                previousMediaCount = 0;
            }
        }
        else {
            if (!cursor) {
                const savedCursor = getCursor(cleanUsername);
                if (savedCursor) {
                    cursor = savedCursor;
                    previousMediaCount = account.mediaCount || 0;
                    logger.info(`Retrying @${cleanUsername} from cursor (${previousMediaCount} items)...`);
                }
                else {
                    try {
                        const accounts = await GetAllAccountsFromDB();
                        const dbAccount = accounts.find((acc) => acc.username.toLowerCase() === cleanUsername.toLowerCase() && (acc.media_type || "all") === selectedMediaType);
                        if (dbAccount?.cursor) {
                            cursor = dbAccount.cursor;
                            previousMediaCount = account.mediaCount || 0;
                            logger.info(`Retrying @${cleanUsername} from database cursor (${previousMediaCount} items)...`);
                        }
                        else {
                            clearFetchState(cleanUsername);
                            clearCursor(cleanUsername);
                            logger.info(`Retrying @${cleanUsername} from beginning...`);
                        }
                    }
                    catch (dbError) {
                        console.error("Failed to get cursor from database:", dbError);
                        clearFetchState(cleanUsername);
                        clearCursor(cleanUsername);
                        logger.info(`Retrying @${cleanUsername} from beginning...`);
                    }
                }
            }
            else {
                previousMediaCount = account.mediaCount || 0;
                logger.info(`Retrying @${cleanUsername} from cursor (${previousMediaCount} items)...`);
            }
        }
        setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
            ? {
                ...acc,
                status: "fetching" as const,
                elapsedTime: 0,
                remainingTime: null,
                error: undefined,
                showDiff: false,
                mediaCount: previousMediaCount,
                previousMediaCount: previousMediaCount,
            }
            : acc));
        accountStopFlagsRef.current.set(accountId, false);
        const settings = getSettings();
        const timeoutSeconds = settings.fetchTimeout || 60;
        accountStartTimesRef.current.set(accountId, Date.now());
        accountTimeoutSecondsRef.current.set(accountId, timeoutSeconds);
        setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
            ? { ...acc, elapsedTime: 0, remainingTime: timeoutSeconds }
            : acc));
        const timerInterval = window.setInterval(() => {
            const startTime = accountStartTimesRef.current.get(accountId);
            const timeoutSecs = accountTimeoutSecondsRef.current.get(accountId) || timeoutSeconds;
            if (startTime) {
                const elapsed = Math.floor((Date.now() - startTime) / 1000);
                const remaining = Math.max(0, timeoutSecs - elapsed);
                setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                    ? { ...acc, elapsedTime: elapsed, remainingTime: remaining }
                    : acc));
                if (remaining <= 0) {
                    accountStopFlagsRef.current.set(accountId, true);
                    clearInterval(timerInterval);
                    accountTimersRef.current.delete(accountId);
                    accountStartTimesRef.current.delete(accountId);
                    accountTimeoutSecondsRef.current.delete(accountId);
                    const mediaCount = accountMediaCountRef.current.get(accountId) || 0;
                    setMultipleAccounts((prev) => prev.map((acc) => {
                        if (acc.id === accountId) {
                            const status = mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
                            return { ...acc, status, remainingTime: 0, mediaCount };
                        }
                        return acc;
                    }));
                    CleanupExtractorProcesses().catch(() => { });
                }
            }
        }, 1000);
        accountTimersRef.current.set(accountId, timerInterval);
        try {
            let hasMore = true;
            let page = 0;
            const seenCursors = new Set<string>();
            while (hasMore && !stopAllRef.current) {
                if (accountStopFlagsRef.current.get(accountId)) {
                    break;
                }
                const response = await ExtractTimeline({
                    username: cleanUsername,
                    auth_token: authToken.trim(),
                    timeline_type: (retrySettings.includeText || retrySettings.includeRetweets) ? "tweets" : "media",
                    batch_size: batchSizeRetry,
                    page: page,
                    media_type: selectedMediaType,
                    retweets: retrySettings.includeRetweets,
                    cursor: cursor,
                    include_photos: retrySettings.includePhotos,
                    include_videos: retrySettings.includeVideos,
                    include_gifs: retrySettings.includeGifs,
                    include_text: retrySettings.includeText,
                    video_quality: retrySettings.videoQuality,
                    image_size: retrySettings.imageSize,
                });
                const data = normalizeTwitterResponse(JSON.parse(response) as TwitterResponse);
                if (!accountInfo && data.account_info) {
                    accountInfo = data.account_info;
                }
                const previousRetryCount = allTimeline.length;
                allTimeline = mergeTimelines(allTimeline, data.timeline);
                cursor = data.cursor;
                hasMore = !!data.cursor && !data.completed;
                if (hasMore && cursor) {
                    if (seenCursors.has(cursor) || (data.timeline.length > 0 && allTimeline.length === previousRetryCount)) {
                        hasMore = false;
                    }
                    else {
                        seenCursors.add(cursor);
                    }
                }
                page++;
                if (cursor) {
                    saveCursor(cleanUsername, cursor);
                }
                const currentMediaCount = allTimeline.length;
                accountMediaCountRef.current.set(accountId, currentMediaCount);
                const hasNewItems = currentMediaCount > previousMediaCount;
                setMultipleAccounts((prev) => prev.map((acc) => {
                    if (acc.id === accountId) {
                        return {
                            ...acc,
                            accountInfo: accountInfo || undefined,
                            previousMediaCount: previousMediaCount,
                            mediaCount: currentMediaCount,
                            showDiff: hasNewItems,
                            cursor: cursor,
                        };
                    }
                    return acc;
                }));
                if (page % 3 === 0 || !hasMore || accountStopFlagsRef.current.get(accountId)) {
                    saveFetchState({
                        username: cleanUsername,
                        cursor: cursor || "",
                        totalFetched: allTimeline.length,
                        completed: !hasMore,
                        mediaType: selectedMediaType,
                        retweets: false,
                        timelineType: "timeline",
                    });
                }
                if (hasNewItems) {
                    setTimeout(() => {
                        setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId ? { ...acc, showDiff: false } : acc));
                    }, 1000);
                }
                previousMediaCount = currentMediaCount;
                if (accountInfo) {
                    try {
                        await SaveAccountToDBWithStatus(accountInfo.name, accountInfo.nick, accountInfo.profile_image, allTimeline.length, JSON.stringify({
                            account_info: accountInfo,
                            timeline: allTimeline,
                            total_urls: allTimeline.length,
                            metadata: {
                                new_entries: data.timeline.length,
                                page: page,
                                batch_size: batchSizeRetry,
                                has_more: hasMore,
                                cursor: cursor,
                                completed: !hasMore,
                            },
                            cursor: cursor,
                            completed: !hasMore,
                        }), selectedMediaType, cursor || "", !hasMore);
                    }
                    catch (err) {
                        reportDatabaseSaveError("retry progress", err);
                    }
                }
            }
            const startTime = accountStartTimesRef.current.get(accountId);
            const elapsedSecs = startTime ? Math.floor((Date.now() - startTime) / 1000) : 0;
            clearInterval(timerInterval);
            accountTimersRef.current.delete(accountId);
            accountStartTimesRef.current.delete(accountId);
            accountTimeoutSecondsRef.current.delete(accountId);
            const finalMediaCount = allTimeline.length;
            accountMediaCountRef.current.set(accountId, finalMediaCount);
            const wasTimeout = accountStopFlagsRef.current.get(accountId);
            if (wasTimeout) {
                logger.warning(`@${account.username}: timeout - ${finalMediaCount} items (${elapsedSecs}s)`);
                setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                    ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
                    : acc));
            }
            else if (stopAllRef.current) {
                logger.info(`@${account.username}: stopped - ${finalMediaCount} items (${elapsedSecs}s)`);
                setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                    ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
                    : acc));
            }
            else if (hasMore) {
                logger.warning(`@${account.username}: incomplete - ${finalMediaCount} items (${elapsedSecs}s)`);
                setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                    ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
                    : acc));
            }
            else {
                logger.success(`@${account.username}: completed - ${finalMediaCount} items (${elapsedSecs}s)`);
                clearFetchState(cleanUsername);
                clearCursor(cleanUsername);
                setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId ? { ...acc, status: "completed" as const, cursor: undefined } : acc));
            }
        }
        catch (error) {
            const startTime = accountStartTimesRef.current.get(accountId);
            const elapsedSecs = startTime ? Math.floor((Date.now() - startTime) / 1000) : 0;
            const timerInterval = accountTimersRef.current.get(accountId);
            if (timerInterval) {
                clearInterval(timerInterval);
                accountTimersRef.current.delete(accountId);
            }
            accountStartTimesRef.current.delete(accountId);
            accountTimeoutSecondsRef.current.delete(accountId);
            const errorMsg = error instanceof Error ? error.message : String(error);
            const mediaCount = accountMediaCountRef.current.get(accountId) || 0;
            if (stopAllRef.current || accountStopFlagsRef.current.get(accountId)) {
                accountMediaCountRef.current.delete(accountId);
                setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                    ? {
                        ...acc,
                        status: "incomplete" as const,
                        error: undefined,
                        mediaCount,
                    }
                    : acc));
                logger.info(`@${account.username}: stopped - ${mediaCount} items (${elapsedSecs}s)`);
                return;
            }
            const isAuthError = errorMsg.toLowerCase().includes("401") ||
                errorMsg.toLowerCase().includes("unauthorized") ||
                errorMsg.toLowerCase().includes("auth token may be invalid") ||
                errorMsg.toLowerCase().includes("auth token may be expired") ||
                errorMsg.toLowerCase().includes("invalid or expired");
            const status = isAuthError || mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
            accountMediaCountRef.current.delete(accountId);
            setMultipleAccounts((prev) => prev.map((acc) => acc.id === accountId
                ? {
                    ...acc,
                    status,
                    error: errorMsg,
                    mediaCount,
                }
                : acc));
            logger.error(`@${account.username}: failed - ${errorMsg} (${elapsedSecs}s)`);
        }
    };
    const renderPage = () => {
        switch (currentPage) {
            case "settings":
                return <SettingsPage onUnsavedChangesChange={setHasUnsavedSettings} onResetRequest={setResetSettingsFn}/>;
            case "debug":
                return <DebugLoggerPage />;
            case "database":
                return (<DatabaseView onBack={() => setCurrentPage("main")} onLoadAccount={handleLoadFromDB} onUpdateSelected={handleUpdateSelected}/>);
            case "support":
                return <SupportPage />;
            default:
                return (<>
            <Header version={CURRENT_VERSION} hasUpdate={hasUpdate} releaseDate={releaseDate}/>

            <SearchBar username={username} loading={loading} onUsernameChange={setUsername} onFetch={handleFetch} onStopFetch={handleStopFetch} onResume={handleResume} onClearResume={handleClearResume} resumeInfo={resumeInfo} history={fetchHistory} onHistorySelect={handleHistorySelect} onHistoryRemove={removeFromHistory} hasResult={!!result} elapsedTime={elapsedTime} remainingTime={remainingTime} fetchType={fetchType} onFetchTypeChange={handleFetchTypeChange} multipleAccounts={multipleAccounts} onImportAccounts={handleImportAccounts} onFetchAll={handleFetchAll} onStopAll={handleStopAll} onStopAccount={handleStopAccount} onRetryAccount={handleRetryAccount} onClearMultipleAccounts={handleClearMultipleAccounts} onRemoveMultipleAccount={handleRemoveMultipleAccount} onOpenSavedAccounts={() => setCurrentPage("database")} isFetchingAll={isFetchingAll} mode={searchMode} privateType={searchPrivateType} onModeChange={(mode, privateType) => {
                        setSearchMode(mode);
                        if (privateType) {
                            setSearchPrivateType(privateType);
                        }
                    }}/>

            {result && fetchType === "single" && (<div className="mt-4 space-y-3">
                <div className="flex justify-start">
                  <Button variant="outline" onClick={handleBackToHomeResult} className="flex items-center gap-2">
                    <ArrowLeft className="h-4 w-4"/>
                    Back to Home
                  </Button>
                </div>
                <MediaList accountInfo={result.account_info} timeline={result.timeline} totalUrls={result.total_urls} fetchedMediaType={fetchedMediaType} newMediaCount={newMediaCount}/>
              </div>)}
          </>);
        }
    };
    return (<TooltipProvider>
      <div className="h-screen overflow-hidden bg-background">
        <DependencySetupDialog />
        <TitleBar />
        <Sidebar currentPage={currentPage} onPageChange={handlePageChange}/>

        <div id="app-content-scroll" className="fixed top-10 right-0 bottom-0 left-14 overflow-y-auto overflow-x-hidden">
          <div className="p-4 md:p-8">
            <div className="max-w-5xl mx-auto">
              {renderPage()}
            </div>
          </div>
        </div>

        <Dialog open={showUnsavedChangesDialog} onOpenChange={(open) => {
            if (open) {
                setShowUnsavedChangesDialog(true);
                return;
            }
            handleCancelNavigation();
        }}>
          <DialogContent className="sm:max-w-106.25 [&>button]:hidden">
            <DialogHeader>
              <DialogTitle>Unsaved Changes</DialogTitle>
              <DialogDescription>
                You have unsaved changes in Settings. If you leave now, your changes will be lost.
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button variant="outline" onClick={handleCancelNavigation}>
                Cancel
              </Button>
              <Button variant="destructive" onClick={handleDiscardChanges}>
                Discard Changes
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>
    </TooltipProvider>);
}
export default App;
