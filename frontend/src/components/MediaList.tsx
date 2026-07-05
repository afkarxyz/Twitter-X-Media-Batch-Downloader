import { useState, useMemo, useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue, } from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger, } from "@/components/ui/tooltip";
import { Image, Video, Film, FileText, ExternalLink, Repeat2, Download, FolderOpen, LayoutGrid, Grid3X3, List, StopCircle, MapPin, Link2, Lock, Calendar, ArrowUp, ChevronLeft, ChevronRight, Eye, Heart, Bookmark, BadgeCheck, Maximize2, CheckCircle, XCircle, FileCheck, } from "lucide-react";
import { Spinner } from "@/components/ui/spinner";
import type { TimelineEntry, AccountInfo } from "@/types/api";
import { logger } from "@/lib/logger";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { getSettings, renderFolderTemplate } from "@/lib/settings";
import { beginDownload, finishDownload, setDownloadItemStatus, useDownloadState } from "@/lib/download-state";
import { getCachedMediaFolderStatus, setCachedMediaFolderStatus } from "@/lib/runtime-cache";
import { openExternal } from "@/lib/utils";
import { DownloadMediaWithMetadata, DownloadProfileImage, OpenFolder, StopDownload, CheckFolderExists } from "../../wailsjs/go/main/App";
import { main } from "../../wailsjs/go/models";
interface MediaListProps {
    accountInfo: AccountInfo;
    timeline: TimelineEntry[];
    totalUrls: number;
    fetchedMediaType?: string;
    newMediaCount?: number | null;
}
const MEDIA_LIST_PAGE_SIZE = 30;
function getContentScrollElement(): HTMLElement | null {
    return document.getElementById("app-content-scroll");
}
function getThumbnailUrl(url: string): string {
    if (url.includes("video.twimg.com/tweet_video/")) {
        const match = url.match(/tweet_video\/([^/]+)\.mp4/);
        if (match && match[1]) {
            const filename = match[1];
            return `https://pbs.twimg.com/tweet_video_thumb/${filename}?format=jpg&name=360x360`;
        }
    }
    if (url.includes("pbs.twimg.com/media/")) {
        if (url.includes("?format=")) {
            if (url.includes("&name=")) {
                const parts = url.split("&name=");
                return parts[0] + "&name=360x360";
            }
            return url + "&name=360x360";
        }
        if (url.includes("?")) {
            return url + "&name=360x360";
        }
        return url + "?format=jpg&name=360x360";
    }
    return url;
}
function getPreviewUrl(url: string): string {
    if (url.includes("pbs.twimg.com/media/")) {
        if (url.includes("?format=")) {
            if (url.includes("&name=")) {
                const parts = url.split("&name=");
                return parts[0] + "&name=large";
            }
            return url + "&name=large";
        }
        if (url.includes("?")) {
            return url + "&name=large";
        }
        return url + "?format=jpg&name=large";
    }
    return url;
}
function getEntryThumbnail(item: TimelineEntry): string {
    if (item.thumbnail) {
        return item.thumbnail;
    }
    return getThumbnailUrl(item.url);
}
function getMediaIcon(type: string) {
    switch (type) {
        case "photo":
            return <Image className="h-4 w-4"/>;
        case "video":
            return <Video className="h-4 w-4"/>;
        case "gif":
        case "animated_gif":
            return <Film className="h-4 w-4"/>;
        case "text":
            return <FileText className="h-4 w-4"/>;
        default:
            return <Image className="h-4 w-4"/>;
    }
}
function formatDate(dateStr: string): string {
    try {
        const d = new Date(dateStr);
        if (!Number.isNaN(d.getTime())) {
            const year = d.getFullYear();
            const month = String(d.getMonth() + 1).padStart(2, "0");
            const day = String(d.getDate()).padStart(2, "0");
            const hours = String(d.getHours()).padStart(2, "0");
            const minutes = String(d.getMinutes()).padStart(2, "0");
            return `${year}-${month}-${day} ${hours}:${minutes}`;
        }
        return dateStr;
    }
    catch {
        return dateStr;
    }
}
function getRelativeTime(dateStr: string): string {
    try {
        const date = new Date(dateStr);
        const now = new Date();
        const diffMs = now.getTime() - date.getTime();
        const diffMinutes = Math.floor(diffMs / (1000 * 60));
        const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
        const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
        const diffMonths = Math.floor(diffDays / 30);
        const diffYears = Math.floor(diffDays / 365);
        if (diffYears > 0) {
            const remainingMonths = Math.floor((diffDays % 365) / 30);
            return `(${diffYears}y ${remainingMonths}m ago)`;
        }
        else if (diffMonths > 0) {
            const remainingDays = diffDays % 30;
            return `(${diffMonths}m ${remainingDays}d ago)`;
        }
        else if (diffDays > 0) {
            const remainingHours = diffHours % 24;
            return `(${diffDays}d ${remainingHours}h ago)`;
        }
        else if (diffHours > 0) {
            const remainingMinutes = diffMinutes % 60;
            return `(${diffHours}h ${remainingMinutes}m ago)`;
        }
        else if (diffMinutes > 0) {
            return `(${diffMinutes}m ago)`;
        }
        else {
            return "(just now)";
        }
    }
    catch {
        return "";
    }
}
function formatNumber(num: number): string {
    if (num >= 1000000) {
        return (num / 1000000).toFixed(1) + "M";
    }
    if (num >= 1000) {
        return (num / 1000).toFixed(1) + "K";
    }
    return num.toString();
}
function formatNumberWithComma(num: number): string {
    return num.toLocaleString();
}
function getItemKey(item: TimelineEntry): string {
    return `${item.tweet_id}-${item.url}`;
}
function formatJoinDate(dateStr: string): string {
    try {
        const date = new Date(dateStr);
        const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        return `${months[date.getMonth()]} ${date.getFullYear()}`;
    }
    catch {
        return dateStr;
    }
}
function getAccountFolderName(accountInfo: AccountInfo): string {
    if (accountInfo.nick === "My Bookmarks" || accountInfo.name === "bookmarks") {
        return "My Bookmarks";
    }
    if (accountInfo.nick === "My Likes" || accountInfo.name === "likes") {
        return "My Likes";
    }
    const template = getSettings().folderTemplate;
    const pad = (n: number) => String(n).padStart(2, "0");
    const now = new Date();
    const rendered = renderFolderTemplate(template, {
        username: accountInfo.name,
        accountName: accountInfo.nick,
        date: `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`,
    });
    return rendered || accountInfo.name;
}
function pickVariantUrl(variants: {
    name: string;
    url: string;
}[] | undefined, size: string, fallback: string): string {
    if (variants && variants.length > 0) {
        const match = variants.find((v) => v.name === size);
        if (match?.url)
            return match.url;
    }
    return fallback;
}
export function MediaList({ accountInfo, timeline, totalUrls, fetchedMediaType = "all", newMediaCount = null, }: MediaListProps) {
    const settings = getSettings();
    const folderName = getAccountFolderName(accountInfo);
    const downloadState = useDownloadState();
    const anyDownloadActive = downloadState.active;
    const isDownloading = downloadState.active && downloadState.scope === "media";
    const downloadProgress = isDownloading ? downloadState.progress : null;
    const downloadingItem = isDownloading ? downloadState.currentItemKey : null;
    const itemStatusByKey = downloadState.itemStatusByKey;
    const cachedMediaFolderStatus = settings.downloadPath
        ? getCachedMediaFolderStatus(settings.downloadPath, folderName)
        : null;
    const [selectedItems, setSelectedItems] = useState<Set<string>>(new Set());
    const [sortBy, setSortBy] = useState<string>("date-desc");
    const [filterType, setFilterType] = useState<string>("all");
    const [viewMode, setViewMode] = useState<"large" | "small" | "list">(() => {
        const stored = localStorage.getItem("resultViewMode");
        return stored === "large" || stored === "small" || stored === "list" ? stored : "list";
    });
    useEffect(() => {
        localStorage.setItem("resultViewMode", viewMode);
    }, [viewMode]);
    const [hasDownloaded, setHasDownloaded] = useState(false);
    const [showScrollTop, setShowScrollTop] = useState(false);
    const [folderExists, setFolderExists] = useState(cachedMediaFolderStatus?.folderExists ?? false);
    const [previewIndex, setPreviewIndex] = useState<number | null>(null);
    const [visibleCount, setVisibleCount] = useState<number>(MEDIA_LIST_PAGE_SIZE);
    const loadMoreRef = useRef<HTMLDivElement>(null);
    useEffect(() => {
        const scrollElement = getContentScrollElement();
        if (!scrollElement) {
            return;
        }
        const handleScroll = () => {
            setShowScrollTop(scrollElement.scrollTop > 300);
        };
        handleScroll();
        scrollElement.addEventListener("scroll", handleScroll);
        return () => scrollElement.removeEventListener("scroll", handleScroll);
    }, []);
    const filteredTimeline = useMemo(() => {
        let filtered = [...timeline];
        if (filterType !== "all") {
            filtered = filtered.filter((item) => {
                if (filterType === "photo")
                    return item.type === "photo";
                if (filterType === "video")
                    return item.type === "video";
                if (filterType === "gif")
                    return item.type === "animated_gif" || item.type === "gif";
                if (filterType === "text")
                    return item.type === "text";
                return true;
            });
        }
        filtered.sort((a, b) => {
            if (sortBy === "date-desc") {
                return new Date(b.date).getTime() - new Date(a.date).getTime();
            }
            else if (sortBy === "date-asc") {
                return new Date(a.date).getTime() - new Date(b.date).getTime();
            }
            else if (sortBy === "tweet-id-desc") {
                return Number(b.tweet_id) - Number(a.tweet_id);
            }
            else if (sortBy === "tweet-id-asc") {
                return Number(a.tweet_id) - Number(b.tweet_id);
            }
            return 0;
        });
        return filtered;
    }, [timeline, filterType, sortBy]);
    useEffect(() => {
        setVisibleCount(MEDIA_LIST_PAGE_SIZE);
    }, [filteredTimeline.length]);
    useEffect(() => {
        const checkFolders = async () => {
            const settings = getSettings();
            const basePath = settings.downloadPath;
            if (!basePath || !accountInfo.name)
                return;
            const exists = await CheckFolderExists(basePath, folderName);
            setCachedMediaFolderStatus(basePath, folderName, {
                folderExists: exists,
                gifsFolderHasMP4: false,
            });
            setFolderExists(exists);
        };
        checkFolders();
    }, [accountInfo.name, accountInfo.nick, folderName, hasDownloaded]);
    useEffect(() => {
        const sentinel = loadMoreRef.current;
        if (!sentinel || visibleCount >= filteredTimeline.length)
            return;
        const observer = new IntersectionObserver((entries) => {
            if (entries.some((entry) => entry.isIntersecting)) {
                setVisibleCount((prev) => {
                    const filteredLength = filteredTimeline.length;
                    return Math.min(prev + MEDIA_LIST_PAGE_SIZE, filteredLength);
                });
            }
        }, {
            rootMargin: "400px 0px",
            threshold: 0,
        });
        observer.observe(sentinel);
        return () => {
            observer.disconnect();
        };
    }, [visibleCount, filteredTimeline.length]);
    const scrollToTop = () => {
        getContentScrollElement()?.scrollTo({ top: 0, behavior: "smooth" });
    };
    const openPreview = (index: number) => {
        setPreviewIndex(index);
    };
    const closePreview = () => {
        setPreviewIndex(null);
    };
    useEffect(() => {
        if (previewIndex === null) {
            return;
        }
        const previousOverflow = document.body.style.overflow;
        document.body.style.overflow = "hidden";
        return () => {
            document.body.style.overflow = previousOverflow;
        };
    }, [previewIndex]);
    const goToPrevious = () => {
        if (previewIndex !== null && previewIndex > 0) {
            setPreviewIndex(previewIndex - 1);
        }
    };
    const goToNext = () => {
        if (previewIndex !== null && previewIndex < filteredTimeline.length - 1) {
            setPreviewIndex(previewIndex + 1);
        }
    };
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (previewIndex === null)
                return;
            if (e.key === "ArrowLeft")
                goToPrevious();
            if (e.key === "ArrowRight")
                goToNext();
            if (e.key === "Escape")
                closePreview();
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [previewIndex, filteredTimeline.length]);
    const mediaCounts = useMemo(() => {
        const counts = { photo: 0, video: 0, gif: 0, text: 0 };
        timeline.forEach((item) => {
            if (item.type === "photo")
                counts.photo++;
            else if (item.type === "video")
                counts.video++;
            else if (item.type === "gif" || item.type === "animated_gif")
                counts.gif++;
            else if (item.type === "text")
                counts.text++;
        });
        return counts;
    }, [timeline]);
    const toggleSelectAll = () => {
        if (selectedItems.size === filteredTimeline.length) {
            setSelectedItems(new Set());
        }
        else {
            setSelectedItems(new Set(filteredTimeline.map((item) => getItemKey(item))));
        }
    };
    const toggleItem = (key: string) => {
        const newSelected = new Set(selectedItems);
        if (newSelected.has(key)) {
            newSelected.delete(key);
        }
        else {
            newSelected.add(key);
        }
        setSelectedItems(newSelected);
    };
    const getOutputDir = (): string => {
        const settings = getSettings();
        const isBookmarks = accountInfo.nick === "My Bookmarks";
        const isLikes = accountInfo.nick === "My Likes";
        if (isBookmarks || isLikes) {
            const separator = settings.downloadPath.includes("/") ? "/" : "\\";
            const folderName = isBookmarks ? "My Bookmarks" : "My Likes";
            return `${settings.downloadPath}${separator}${folderName}`;
        }
        return settings.downloadPath;
    };
    const createMediaItemRequest = (item: TimelineEntry) => new main.MediaItemRequest({
        url: item.url,
        date: item.date,
        tweet_id: item.tweet_id,
        type: item.type,
        content: item.content || "",
        original_filename: item.original_filename || "",
        author_username: item.author_username || "",
        account_name: item.author_name || accountInfo.nick || accountInfo.name,
    });
    const getItemStatusFlags = (itemKey: string) => {
        const status = itemStatusByKey[itemKey];
        return {
            isItemDownloaded: status === "success",
            isItemFailed: status === "failed",
            isItemSkipped: status === "skipped",
            isItemDownloading: downloadingItem === itemKey,
        };
    };
    const handleDownloadItem = async (item: TimelineEntry, itemKey: string) => {
        beginDownload({ current: 0, total: 1, percent: 0 }, { scope: "media", currentItemKey: itemKey, itemKeyByIndex: { 0: itemKey } });
        try {
            const settings = getSettings();
            const request = new main.DownloadMediaWithMetadataRequest({
                items: [createMediaItemRequest(item)],
                output_dir: getOutputDir(),
                username: accountInfo.name,
                concurrent_downloads: settings.concurrentDownloads || 10,
                skip_existing: settings.skipExistingFiles,
                delete_incomplete_files: settings.deleteIncompleteFiles,
                retry_attempts: settings.retryAttempts,
                proxy: settings.proxy || "",
                filename_template: settings.filenameTemplate || "",
                folder_template: settings.folderTemplate || "",
                auto_convert_gifs: settings.autoConvertGifs,
                gif_quality: settings.gifQuality || "fast",
                gif_resolution: settings.gifResolution || "original",
            });
            const response = await DownloadMediaWithMetadata(request);
            if (response.cancelled) {
                toast.info(response.message || "Download stopped");
            }
            else if (response.success) {
                if (response.skipped > 0) {
                    setDownloadItemStatus(itemKey, "skipped");
                    toast.info("Already exists");
                }
                else if (response.downloaded > 0) {
                    setDownloadItemStatus(itemKey, "success");
                    toast.success("Downloaded");
                }
                setHasDownloaded(true);
            }
            else {
                setDownloadItemStatus(itemKey, "failed");
                toast.error(response.message || "Download failed");
            }
        }
        catch (error) {
            setDownloadItemStatus(itemKey, "failed");
            const errorMsg = error instanceof Error ? error.message : String(error);
            toast.error(`Download failed: ${errorMsg}`);
        }
        finally {
            finishDownload();
        }
    };
    const handleDownload = async () => {
        const itemsWithIndices = selectedItems.size > 0
            ? filteredTimeline
                .map((item, i) => ({ item, originalIndex: i }))
                .filter((entry) => selectedItems.has(getItemKey(entry.item)))
            : filteredTimeline.map((item, i) => ({ item, originalIndex: i }));
        if (itemsWithIndices.length === 0) {
            toast.error("No media to download");
            return;
        }
        const keyMap: Record<number, string> = {};
        itemsWithIndices.forEach((entry, backendIndex) => {
            keyMap[backendIndex] = getItemKey(entry.item);
        });
        beginDownload({ current: 0, total: itemsWithIndices.length, percent: 0 }, { scope: "media", itemKeyByIndex: keyMap });
        logger.info(`Starting download of ${itemsWithIndices.length} files...`);
        try {
            const settings = getSettings();
            const request = new main.DownloadMediaWithMetadataRequest({
                items: itemsWithIndices.map(({ item }) => createMediaItemRequest(item)),
                output_dir: getOutputDir(),
                username: accountInfo.name,
                concurrent_downloads: settings.concurrentDownloads || 10,
                skip_existing: settings.skipExistingFiles,
                delete_incomplete_files: settings.deleteIncompleteFiles,
                retry_attempts: settings.retryAttempts,
                proxy: settings.proxy || "",
                filename_template: settings.filenameTemplate || "",
                folder_template: settings.folderTemplate || "",
                auto_convert_gifs: settings.autoConvertGifs,
                gif_quality: settings.gifQuality || "fast",
                gif_resolution: settings.gifResolution || "original",
            });
            const response = await DownloadMediaWithMetadata(request);
            if (response.cancelled) {
                logger.info(response.message || "Download stopped");
                toast.info(response.message || "Download stopped");
                if (response.downloaded > 0 || response.skipped > 0) {
                    setHasDownloaded(true);
                }
            }
            else if (response.success) {
                const parts: string[] = [];
                if (response.downloaded > 0) {
                    parts.push(`${response.downloaded} file${response.downloaded !== 1 ? 's' : ''} downloaded`);
                }
                if (response.skipped > 0) {
                    parts.push(`${response.skipped} file${response.skipped !== 1 ? 's' : ''} already exist${response.skipped !== 1 ? '' : 's'}`);
                }
                if (response.failed > 0) {
                    parts.push(`${response.failed} failed`);
                }
                const message = parts.length > 0 ? parts.join(', ') : 'Download completed';
                if (response.downloaded === 0 && response.failed === 0 && response.skipped > 0) {
                    logger.info(message);
                    toast.info(message);
                }
                else {
                    logger.success(message);
                    toast.success(message);
                }
                setHasDownloaded(true);
                if (settings.downloadPath) {
                    setCachedMediaFolderStatus(settings.downloadPath, folderName, {
                        folderExists: true,
                        gifsFolderHasMP4: false,
                    });
                    setFolderExists(true);
                }
            }
            else {
                logger.error(response.message);
                toast.error("Download failed");
            }
        }
        catch (error) {
            const errorMsg = error instanceof Error ? error.message : String(error);
            logger.error(`Download failed: ${errorMsg}`);
            toast.error("Download failed");
        }
        finally {
            finishDownload();
        }
    };
    const handleStopDownload = async () => {
        try {
            const stopped = await StopDownload();
            if (stopped) {
                finishDownload();
                logger.info("Download stopped by user");
                toast.info("Stopped");
            }
        }
        catch (error) {
            console.error("Failed to stop download:", error);
        }
    };
    const handleDownloadProfileImage = async (kind: "avatar" | "banner") => {
        const settings = getSettings();
        const url = kind === "avatar"
            ? pickVariantUrl(accountInfo.avatar_variants, settings.avatarSize, accountInfo.profile_image)
            : pickVariantUrl(accountInfo.banner_variants, settings.bannerSize, accountInfo.banner || "");
        if (!url) {
            toast.error(`No ${kind} available`);
            return;
        }
        try {
            await DownloadProfileImage(new main.DownloadProfileImageRequest({
                url,
                output_dir: settings.downloadPath,
                username: folderName,
                kind,
                proxy: settings.proxy || "",
            }));
            toast.success(`${kind === "avatar" ? "Avatar" : "Banner"} downloaded`);
        }
        catch (error) {
            logger.error(`Failed to download ${kind}: ${error}`);
            toast.error(`Failed to download ${kind}`);
        }
    };
    const handleOpenFolder = async () => {
        const settings = getSettings();
        const isBookmarks = accountInfo.nick === "My Bookmarks";
        const isLikes = accountInfo.nick === "My Likes";
        let folderPath: string;
        if (isBookmarks || isLikes) {
            const separator = settings.downloadPath.includes("/") ? "/" : "\\";
            const folderName = isBookmarks ? "My Bookmarks" : "My Likes";
            folderPath = settings.downloadPath
                ? `${settings.downloadPath}${separator}${folderName}`
                : folderName;
        }
        else {
            folderPath = settings.downloadPath
                ? `${settings.downloadPath}/${folderName}`
                : folderName;
        }
        try {
            await OpenFolder(folderPath);
        }
        catch {
            try {
                await OpenFolder(settings.downloadPath);
            }
            catch {
                toast.error("Could not open folder");
            }
        }
    };
    const handleOpenTweet = (tweetId: string) => {
        openExternal(`https://x.com/${accountInfo.name}/status/${tweetId}`);
    };
    return (<div className="space-y-4">
      
      {accountInfo.name === "bookmarks" || accountInfo.name === "likes" ? (<div className="flex items-center gap-4 p-4 bg-muted/50 rounded-lg">
          <div className="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center">
            {accountInfo.name === "bookmarks" ? (<Bookmark className="h-8 w-8 text-primary"/>) : (<Heart className="h-8 w-8 text-primary"/>)}
          </div>
          <div className="flex-1">
            <h2 className="text-xl font-semibold">{accountInfo.nick}</h2>
          </div>
          <div className="text-right">
            <div className="flex items-center justify-end gap-2">
              {newMediaCount !== null && newMediaCount > 0 && (<div className="text-lg font-semibold text-green-600 dark:text-green-400 animate-in fade-in slide-in-from-left-2 duration-300">
                  {formatNumberWithComma(newMediaCount)}+
                </div>)}
              <div className="text-2xl font-bold text-primary">{formatNumberWithComma(totalUrls)}</div>
            </div>
            <div className="text-sm text-muted-foreground">items found</div>
          </div>
        </div>) : (<div className="rounded-lg overflow-hidden bg-muted/50 border">
          {accountInfo.banner && (<div className="group/banner relative h-24 w-full bg-cover bg-center" style={{ backgroundImage: `url("${accountInfo.banner}")` }}>
            <button type="button" aria-label="Download banner" onClick={() => void handleDownloadProfileImage("banner")} className="absolute top-2 right-2 flex h-9 w-9 items-center justify-center rounded-full bg-black/60 text-white opacity-0 transition-all hover:scale-105 hover:bg-primary hover:text-primary-foreground group-hover/banner:opacity-100">
              <Download className="h-4 w-4"/>
            </button>
          </div>)}
          <div className="p-4">
            <div className="flex items-end justify-between gap-4">
              <div className="flex items-end gap-3 min-w-0">
                <div className={`group/avatar relative shrink-0 ${accountInfo.banner ? "-mt-14" : ""}`}>
                  <img src={accountInfo.profile_image} alt={accountInfo.nick} className={`w-20 h-20 rounded-full object-cover bg-muted ${accountInfo.banner ? "ring-4 ring-background" : ""}`}/>
                  <button type="button" aria-label="Download avatar" onClick={() => void handleDownloadProfileImage("avatar")} className="absolute inset-0 flex items-center justify-center rounded-full bg-black/50 text-white opacity-0 transition-opacity hover:bg-black/62 group-hover/avatar:opacity-100">
                    <Download className="h-5 w-5"/>
                  </button>
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-1.5">
                    <h2 className="text-xl font-bold truncate">{accountInfo.nick}</h2>
                    {(accountInfo.verified || accountInfo.blue_verified) && (<BadgeCheck className="h-4 w-4 text-blue-500 shrink-0"/>)}
                    {accountInfo.protected && (<Lock className="h-3.5 w-3.5 text-muted-foreground shrink-0"/>)}
                  </div>
                  <div className="text-sm text-muted-foreground truncate">@{accountInfo.name}</div>
                </div>
              </div>
              <div className="text-right shrink-0">
                <div className="flex items-center justify-end gap-2">
                  {newMediaCount !== null && newMediaCount > 0 && (<div className="text-lg font-semibold text-green-600 dark:text-green-400 animate-in fade-in slide-in-from-left-2 duration-300">
                      {formatNumberWithComma(newMediaCount)}+
                    </div>)}
                  <div className="text-2xl font-bold text-primary">{formatNumberWithComma(totalUrls)}</div>
                </div>
                <div className="text-sm text-muted-foreground">items found</div>
              </div>
            </div>

            {accountInfo.bio && (<p className="text-sm mt-3 whitespace-pre-wrap wrap-break-word">{accountInfo.bio}</p>)}

            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-muted-foreground mt-3">
              {accountInfo.location && (<span className="flex items-center gap-1">
                  <MapPin className="h-3.5 w-3.5"/>
                  {accountInfo.location}
                </span>)}
              {accountInfo.url && (<button type="button" onClick={() => openExternal(accountInfo.url!)} className="flex items-center gap-1 hover:underline">
                  <Link2 className="h-3.5 w-3.5"/>
                  {accountInfo.url.replace(/^https?:\/\//, "").replace(/\/+$/, "")}
                </button>)}
              {accountInfo.date && (<span className="flex items-center gap-1">
                  <Calendar className="h-3.5 w-3.5"/>
                  Joined {formatJoinDate(accountInfo.date)}
                </span>)}
            </div>

            <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm mt-3">
              <span><strong className="text-foreground">{formatNumber(accountInfo.statuses_count)}</strong> <span className="text-muted-foreground">posts</span></span>
              <span className="text-muted-foreground">·</span>
              <span><strong className="text-foreground">{formatNumber(accountInfo.friends_count)}</strong> <span className="text-muted-foreground">following</span></span>
              <span className="text-muted-foreground">·</span>
              <span><strong className="text-foreground">{formatNumber(accountInfo.followers_count)}</strong> <span className="text-muted-foreground">followers</span></span>
              {accountInfo.media_count ? (<>
                  <span className="text-muted-foreground">·</span>
                  <span><strong className="text-foreground">{formatNumber(accountInfo.media_count)}</strong> <span className="text-muted-foreground">media</span></span>
                </>) : null}
            </div>
          </div>
        </div>)}

      
      <div className="flex items-center gap-4">
        <Select value={sortBy} onValueChange={setSortBy}>
          <SelectTrigger className="w-auto">
            <SelectValue placeholder="Sort by"/>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="date-desc">Newest</SelectItem>
            <SelectItem value="date-asc">Oldest</SelectItem>
          </SelectContent>
        </Select>

        
        {fetchedMediaType === "all" && (<Select value={filterType} onValueChange={setFilterType}>
            <SelectTrigger className="w-auto">
              <SelectValue placeholder="Filter"/>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All ({formatNumberWithComma(totalUrls)})</SelectItem>
              {mediaCounts.photo > 0 && (<SelectItem value="photo">
                  <span className="flex items-center gap-2">
                    <Image className="h-4 w-4 text-blue-500"/>
                    Images ({formatNumberWithComma(mediaCounts.photo)})
                  </span>
                </SelectItem>)}
              {mediaCounts.video > 0 && (<SelectItem value="video">
                  <span className="flex items-center gap-2">
                    <Video className="h-4 w-4 text-purple-500"/>
                    Videos ({formatNumberWithComma(mediaCounts.video)})
                  </span>
                </SelectItem>)}
              {mediaCounts.gif > 0 && (<SelectItem value="gif">
                  <span className="flex items-center gap-2">
                    <Film className="h-4 w-4 text-green-500"/>
                    GIFs ({formatNumberWithComma(mediaCounts.gif)})
                  </span>
                </SelectItem>)}
              {mediaCounts.text > 0 && (<SelectItem value="text">
                  <span className="flex items-center gap-2">
                    <FileText className="h-4 w-4 text-orange-500"/>
                    Text ({formatNumberWithComma(mediaCounts.text)})
                  </span>
                </SelectItem>)}
            </SelectContent>
          </Select>)}

        
        <div className="flex items-center border rounded-md">
          <Button variant={viewMode === "large" ? "secondary" : "ghost"} size="icon" className="h-9 w-9 rounded-r-none" onClick={() => setViewMode("large")}>
            <LayoutGrid className="h-4 w-4"/>
          </Button>
          <Button variant={viewMode === "small" ? "secondary" : "ghost"} size="icon" className="h-9 w-9 rounded-none border-x" onClick={() => setViewMode("small")}>
            <Grid3X3 className="h-4 w-4"/>
          </Button>
          <Button variant={viewMode === "list" ? "secondary" : "ghost"} size="icon" className="h-9 w-9 rounded-l-none" onClick={() => setViewMode("list")}>
            <List className="h-4 w-4"/>
          </Button>
        </div>

        <div className="flex-1"/>
        <Button variant="outline" onClick={handleOpenFolder} disabled={!folderExists}>
          <FolderOpen className="h-4 w-4"/>
          Open Folder
        </Button>
        <div className="flex items-center gap-2">
          {isDownloading && (<Button variant="destructive" onClick={handleStopDownload}>
              <StopCircle className="h-4 w-4"/>
              Stop
            </Button>)}
          <Button onClick={handleDownload} disabled={anyDownloadActive}>
          {isDownloading ? (<>
              <Spinner />
              Downloading...
            </>) : (<>
              <Download className="h-4 w-4"/>
              Download {selectedItems.size > 0 ? `${selectedItems.size}` : "All"}
            </>)}
          </Button>
        </div>
      </div>

      
      {isDownloading && downloadProgress && (<div className="space-y-2">
          <div className="flex items-center justify-between text-sm">
            <span className="text-muted-foreground">
              Downloading {downloadProgress.current} of {downloadProgress.total}
            </span>
            <span className="font-medium">{downloadProgress.percent}%</span>
          </div>
          <Progress value={downloadProgress.percent} className="h-2"/>
        </div>)}

      
      <div className="flex items-center gap-2">
        <Checkbox checked={selectedItems.size === filteredTimeline.length && filteredTimeline.length > 0} onCheckedChange={toggleSelectAll}/>
        <span className="text-sm text-muted-foreground">
          Select all ({formatNumberWithComma(filteredTimeline.length)} items)
        </span>
        {selectedItems.size > 0 && (<Badge variant="secondary">{formatNumberWithComma(selectedItems.size)} selected</Badge>)}
      </div>

      
      {viewMode === "list" ? (<div className="space-y-2">
          {filteredTimeline.slice(0, visibleCount).map((item, index) => {
                const itemKey = getItemKey(item);
                const isSelected = selectedItems.has(itemKey);
                const { isItemDownloaded, isItemFailed, isItemSkipped, isItemDownloading } = getItemStatusFlags(itemKey);
                return (<div key={itemKey} onClick={() => openPreview(index)} className={`flex items-center gap-4 p-3 rounded-lg border-2 transition-all cursor-pointer ${isSelected ? "border-primary bg-primary/5" : "border-transparent hover:bg-muted/50"}`}>
                <Checkbox checked={isSelected} onCheckedChange={() => toggleItem(itemKey)} onClick={(e) => e.stopPropagation()}/>
                <div className="relative shrink-0">
                  <span className="absolute -top-1.5 -left-1.5 z-10 flex h-5 min-w-5 items-center justify-center rounded-full bg-primary px-1 text-xs font-medium text-primary-foreground">
                    {index + 1}
                  </span>
                  <div className="w-16 h-16 rounded overflow-hidden bg-muted">
                    {item.type !== "text" && getEntryThumbnail(item) ? (<img src={getEntryThumbnail(item)} alt="" className="w-full h-full object-cover" loading="lazy"/>) : (<div className="w-full h-full flex items-center justify-center">
                        {getMediaIcon(item.type)}
                      </div>)}
                  </div>
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <p className="text-sm font-medium">{item.tweet_id}</p>
                    <Badge variant="secondary" className={`text-xs ${item.type === "photo"
                        ? "bg-blue-500/20 text-blue-700 dark:text-blue-300"
                        : item.type === "video"
                            ? "bg-purple-500/20 text-purple-700 dark:text-purple-300"
                            : item.type === "text"
                                ? "bg-orange-500/20 text-orange-700 dark:text-orange-300"
                                : "bg-green-500/20 text-green-700 dark:text-green-300"}`}>
                      {getMediaIcon(item.type)}
                    </Badge>
                    {item.is_retweet && (<Badge variant="outline" className="text-xs px-1.5">
                        <Repeat2 className="h-3 w-3"/>
                      </Badge>)}
                    
                    {isItemSkipped ? (<FileCheck className="h-4 w-4 text-yellow-500 shrink-0"/>) : isItemDownloaded ? (<CheckCircle className="h-4 w-4 text-green-500 shrink-0"/>) : isItemFailed ? (<XCircle className="h-4 w-4 text-red-500 shrink-0"/>) : null}
                  </div>
                  {item.type === "text" && item.content && (<p className="text-sm mt-1 line-clamp-2">{item.content}</p>)}
                  <p className="text-sm text-muted-foreground mt-1">
                    {formatDate(item.date)} {getRelativeTime(item.date)}
                  </p>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button size="icon" variant="default" onClick={() => void handleDownloadItem(item, itemKey)} disabled={anyDownloadActive}>
                        {isItemDownloading ? (<Spinner />) : isItemSkipped ? (<FileCheck className="h-4 w-4"/>) : isItemDownloaded ? (<CheckCircle className="h-4 w-4"/>) : isItemFailed ? (<XCircle className="h-4 w-4"/>) : (<Download className="h-4 w-4"/>)}
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {isItemDownloading ? (<p>Downloading...</p>) : isItemSkipped ? (<p>Already exists</p>) : isItemDownloaded ? (<p>Downloaded</p>) : isItemFailed ? (<p>Failed</p>) : (<p>Download</p>)}
                    </TooltipContent>
                  </Tooltip>
                  <Button size="icon" variant="outline" onClick={() => handleOpenTweet(item.tweet_id)}>
                    <ExternalLink className="h-4 w-4"/>
                  </Button>
                </div>
              </div>);
            })}
        </div>) : (<div className={`grid gap-3 ${viewMode === "large" ? "grid-cols-4" : "grid-cols-6"}`}>
          {filteredTimeline.slice(0, visibleCount).map((item, index) => {
                const itemKey = getItemKey(item);
                const isSelected = selectedItems.has(itemKey);
                const { isItemDownloaded, isItemFailed, isItemSkipped, isItemDownloading } = getItemStatusFlags(itemKey);
                return (<div key={itemKey} className={`relative group rounded-lg overflow-hidden border-2 transition-all ${isSelected ? "border-primary" : "border-transparent hover:border-muted-foreground/30"}`}>
                
                <div className="aspect-square bg-muted relative cursor-pointer" onClick={() => openPreview(index)}>
                  <span className="absolute top-1.5 left-1.5 z-20 flex h-5 min-w-5 items-center justify-center rounded-full bg-primary px-1 text-xs font-medium text-primary-foreground">
                    {index + 1}
                  </span>
                  {item.type !== "text" && getEntryThumbnail(item) ? (<img src={getEntryThumbnail(item)} alt="" className="w-full h-full object-cover" loading="lazy"/>) : (<div className="w-full h-full flex items-center justify-center bg-muted">
                      {getMediaIcon(item.type)}
                    </div>)}

                  
                  <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-2">
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button size="icon" variant="default" className="h-8 w-8" onClick={async (e) => {
                        e.stopPropagation();
                        await handleDownloadItem(item, itemKey);
                    }} disabled={anyDownloadActive}>
                          {isItemDownloading ? (<Spinner />) : isItemSkipped ? (<FileCheck className="h-4 w-4"/>) : isItemDownloaded ? (<CheckCircle className="h-4 w-4"/>) : isItemFailed ? (<XCircle className="h-4 w-4"/>) : (<Download className="h-4 w-4"/>)}
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>
                        {isItemDownloading ? (<p>Downloading...</p>) : isItemSkipped ? (<p>Already exists</p>) : isItemDownloaded ? (<p>Downloaded</p>) : isItemFailed ? (<p>Failed</p>) : (<p>Download</p>)}
                      </TooltipContent>
                    </Tooltip>
                    <Button size="icon" variant="outline" className="h-8 w-8" onClick={(e) => {
                        e.stopPropagation();
                        handleOpenTweet(item.tweet_id);
                    }}>
                      <ExternalLink className="h-4 w-4"/>
                    </Button>
                  </div>

                  
                  <div className="absolute top-2 left-2" onClick={(e) => e.stopPropagation()}>
                    <Checkbox checked={isSelected} onCheckedChange={() => toggleItem(itemKey)} className="bg-background/80"/>
                  </div>

                  
                  <div className="absolute top-2 right-2">
                    <Badge variant="secondary" className={`text-xs px-1.5 py-0.5 ${item.type === "photo"
                        ? "bg-blue-500/20 text-blue-700 dark:text-blue-300"
                        : item.type === "video"
                            ? "bg-purple-500/20 text-purple-700 dark:text-purple-300"
                            : item.type === "text"
                                ? "bg-orange-500/20 text-orange-700 dark:text-orange-300"
                                : "bg-green-500/20 text-green-700 dark:text-green-300"}`}>
                      {getMediaIcon(item.type)}
                    </Badge>
                  </div>

                  
                  {item.is_retweet && (<div className="absolute bottom-2 right-2">
                      <Badge variant="outline" className="text-xs px-1.5 py-0.5 bg-background/80">
                        <Repeat2 className="h-3 w-3"/>
                      </Badge>
                    </div>)}

                  
                  <div className="absolute bottom-2 left-2">
                    <span className="text-xs px-1.5 py-0.5 bg-black/60 text-white rounded">
                      {index + 1}
                    </span>
                  </div>
                </div>

                
                <div className="p-2 text-xs text-muted-foreground">
                  <div className="truncate">{formatDate(item.date)}</div>
                  <div className="text-[10px] mt-0.5">{getRelativeTime(item.date)}</div>
                </div>
              </div>);
            })}
        </div>)}

      
      {visibleCount < filteredTimeline.length && (<div ref={loadMoreRef} className="flex w-full flex-col items-center justify-center gap-3 py-6">
          <Spinner />
          <Button variant="outline" size="sm" onClick={() => {
                setVisibleCount((prev) => Math.min(prev + MEDIA_LIST_PAGE_SIZE, filteredTimeline.length));
            }}>
            Load More ({formatNumberWithComma(Math.max(filteredTimeline.length - visibleCount, 0))} remaining)
          </Button>
        </div>)}

      
      {previewIndex !== null && filteredTimeline[previewIndex] && (<div className="fixed inset-0 z-40 bg-black/80 flex flex-col items-center justify-center pt-8" onClick={(e) => {
                if (e.target === e.currentTarget)
                    closePreview();
            }}>
          
          {previewIndex > 0 && (<Button variant="ghost" size="icon" className="absolute left-4 top-1/2 -translate-y-1/2 text-white hover:bg-white/20 h-12 w-12 z-10" onClick={goToPrevious}>
              <ChevronLeft className="h-8 w-8"/>
            </Button>)}

          
          <div className="text-white text-sm bg-black/50 px-4 py-1.5 rounded-full mb-4">
            {formatNumberWithComma(previewIndex + 1)} / {formatNumberWithComma(filteredTimeline.length)}
          </div>

          
          <div className="max-w-[90%] max-h-[70%] flex items-center justify-center">
            {filteredTimeline[previewIndex].type === "photo" ? (<img src={getPreviewUrl(filteredTimeline[previewIndex].url)} alt="" className="max-w-full max-h-[65vh] object-contain rounded-lg"/>) : filteredTimeline[previewIndex].type === "video" ? (<video src={filteredTimeline[previewIndex].url} controls autoPlay className="max-w-full max-h-[65vh] rounded-lg"/>) : filteredTimeline[previewIndex].type === "text" ? (<div className="bg-white dark:bg-gray-800 p-6 rounded-lg max-w-2xl">
                <p className="text-lg whitespace-pre-wrap">{filteredTimeline[previewIndex].content || "No content"}</p>
                <p className="text-sm text-muted-foreground mt-4">
                  {formatDate(filteredTimeline[previewIndex].date)} {getRelativeTime(filteredTimeline[previewIndex].date)}
                </p>
              </div>) : (<video src={filteredTimeline[previewIndex].url} autoPlay loop muted className="max-w-full max-h-[65vh] rounded-lg"/>)}
          </div>

          
          <div className="flex items-center gap-4 mt-3 text-white/80 text-sm">
            {filteredTimeline[previewIndex].verified && (<span className="flex items-center gap-1 text-blue-400">
                <BadgeCheck className="h-4 w-4"/>
                Verified
              </span>)}
            {filteredTimeline[previewIndex].width > 0 && filteredTimeline[previewIndex].height > 0 && (<span className="flex items-center gap-1">
                <Maximize2 className="h-4 w-4"/>
                {filteredTimeline[previewIndex].width} × {filteredTimeline[previewIndex].height}
              </span>)}
            {filteredTimeline[previewIndex].view_count !== undefined && filteredTimeline[previewIndex].view_count > 0 && (<span className="flex items-center gap-1">
                <Eye className="h-4 w-4"/>
                {formatNumber(filteredTimeline[previewIndex].view_count)}
              </span>)}
            {filteredTimeline[previewIndex].favorite_count !== undefined && filteredTimeline[previewIndex].favorite_count > 0 && (<span className="flex items-center gap-1">
                <Heart className="h-4 w-4"/>
                {formatNumber(filteredTimeline[previewIndex].favorite_count)}
              </span>)}
            {filteredTimeline[previewIndex].retweet_count !== undefined && filteredTimeline[previewIndex].retweet_count > 0 && (<span className="flex items-center gap-1">
                <Repeat2 className="h-4 w-4"/>
                {formatNumber(filteredTimeline[previewIndex].retweet_count)}
              </span>)}
            {filteredTimeline[previewIndex].bookmark_count !== undefined && filteredTimeline[previewIndex].bookmark_count > 0 && (<span className="flex items-center gap-1">
                <Bookmark className="h-4 w-4"/>
                {formatNumber(filteredTimeline[previewIndex].bookmark_count)}
              </span>)}
            {filteredTimeline[previewIndex].source && (<span className="text-white/60">
                via {filteredTimeline[previewIndex].source}
              </span>)}
          </div>

          
          <div className="flex items-center gap-3 mt-4 z-10">
            {(() => {
                const item = filteredTimeline[previewIndex];
                const itemKey = getItemKey(item);
                const { isItemDownloaded, isItemFailed, isItemSkipped, isItemDownloading } = getItemStatusFlags(itemKey);
                return (<Button variant="default" size="sm" className="h-9" onClick={async () => {
                        await handleDownloadItem(item, itemKey);
                    }} disabled={anyDownloadActive}>
                  {isItemDownloading ? (<Spinner className="mr-1"/>) : isItemSkipped ? (<FileCheck className="h-4 w-4 mr-1"/>) : isItemDownloaded ? (<CheckCircle className="h-4 w-4 mr-1"/>) : isItemFailed ? (<XCircle className="h-4 w-4 mr-1"/>) : (<Download className="h-4 w-4 mr-1"/>)}
                  {isItemDownloading ? "Downloading..." : isItemSkipped ? "Already exists" : isItemDownloaded ? "Downloaded" : isItemFailed ? "Failed" : "Download"}
                </Button>);
            })()}
            <Button variant="secondary" size="sm" className="h-9" onClick={() => handleOpenTweet(filteredTimeline[previewIndex].tweet_id)}>
              <ExternalLink className="h-4 w-4 mr-1"/>
              Open Tweet
            </Button>
          </div>

          
          {previewIndex < filteredTimeline.length - 1 && (<Button variant="ghost" size="icon" className="absolute right-4 top-1/2 -translate-y-1/2 text-white hover:bg-white/20 h-12 w-12 z-10" onClick={goToNext}>
              <ChevronRight className="h-8 w-8"/>
            </Button>)}
        </div>)}

      
      {showScrollTop && previewIndex === null && (<Button variant="default" size="icon" className="fixed bottom-6 left-1/2 -translate-x-1/2 h-9 w-9 rounded-full shadow-lg z-30" onClick={scrollToTop}>
          <ArrowUp className="h-4 w-4"/>
        </Button>)}
    </div>);
}
