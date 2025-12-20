import { useState, useEffect, useRef, useCallback } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { getSettings, applyThemeMode, applyFont } from "@/lib/settings";
import { applyTheme } from "@/lib/themes";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { logger } from "@/lib/logger";
import {
  saveFetchState,
  getFetchState,
  clearFetchState,
  getResumableInfo,
  stateToResponse,
  mergeTimelines,
  saveCursor,
  getCursor,
  clearCursor,
  type FetchState,
} from "@/lib/fetch-state";

// Components
import { TitleBar } from "@/components/TitleBar";
import { Sidebar, type PageType } from "@/components/Sidebar";
import { Header } from "@/components/Header";
import { SearchBar, type FetchMode, type PrivateType, type FetchType, type MultipleAccount } from "@/components/SearchBar";
import { MediaList } from "@/components/MediaList";
import { DatabaseView } from "@/components/DatabaseView";
import { SettingsPage } from "@/components/SettingsPage";
import { DebugLoggerPage } from "@/components/DebugLoggerPage";
import type { HistoryItem } from "@/components/FetchHistory";
import type { TwitterResponse } from "@/types/api";

// Wails bindings
import { ExtractTimeline, ExtractDateRange, SaveAccountToDBWithStatus, CleanupExtractorProcesses, GetAllAccountsFromDB } from "../wailsjs/go/main/App";

const HISTORY_KEY = "twitter_media_fetch_history";
const MAX_HISTORY = 10;
const CURRENT_VERSION = "4.3";
const BATCH_SIZE = 200; // Fetch in batches for progressive display and resume

function formatNumberWithComma(num: number): string {
  return num.toLocaleString();
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
  const [resumeInfo, setResumeInfo] = useState<{ canResume: boolean; mediaCount: number } | null>(null);
  const [elapsedTime, setElapsedTime] = useState(0);
  const [remainingTime, setRemainingTime] = useState<number | null>(null);
  const [newMediaCount, setNewMediaCount] = useState<number | null>(null);
  const stopFetchRef = useRef(false);
  const fetchStartTimeRef = useRef<number | null>(null);
  const timeoutIntervalRef = useRef<number | null>(null);
  
  // Multiple mode state
  const [fetchType, setFetchType] = useState<FetchType>("single");
  const [multipleAccounts, setMultipleAccounts] = useState<MultipleAccount[]>([]);
  const [isFetchingAll, setIsFetchingAll] = useState(false);
  
  // Mode state for SearchBar
  const [searchMode, setSearchMode] = useState<FetchMode>("public");
  const [searchPrivateType, setSearchPrivateType] = useState<PrivateType>("bookmarks");
  const stopAllRef = useRef(false);
  const accountTimersRef = useRef<Map<string, number>>(new Map());
  const accountStartTimesRef = useRef<Map<string, number>>(new Map());
  const accountStopFlagsRef = useRef<Map<string, boolean>>(new Map());
  const accountMediaCountRef = useRef<Map<string, number>>(new Map());
  const accountTimeoutSecondsRef = useRef<Map<string, number>>(new Map());

  // Handle fetch type change
  const handleFetchTypeChange = (type: FetchType) => {
    setFetchType(type);
    if (type === "single") {
      // Clear multiple accounts when switching to single
      setMultipleAccounts([]);
      setIsFetchingAll(false);
      stopAllRef.current = true;
      // Clear all timers
      accountTimersRef.current.forEach((interval) => clearInterval(interval));
      accountTimersRef.current.clear();
      accountStartTimesRef.current.clear();
      accountTimeoutSecondsRef.current.clear();
      accountStopFlagsRef.current.clear();
      accountMediaCountRef.current.clear();
    } else {
      // Clear result when switching to multiple
      setResult(null);
    }
  };

  useEffect(() => {
    const settings = getSettings();
    applyThemeMode(settings.themeMode);
    applyTheme(settings.theme);
    applyFont(settings.fontFamily);

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      const currentSettings = getSettings();
      if (currentSettings.themeMode === "auto") {
        applyThemeMode("auto");
        applyTheme(currentSettings.theme);
      }
    };

    mediaQuery.addEventListener("change", handleChange);
    checkForUpdates();
    loadHistory();

    return () => {
      mediaQuery.removeEventListener("change", handleChange);
    };
  }, []);

  const checkForUpdates = async () => {
    try {
      const response = await fetch(
        "https://api.github.com/repos/afkarxyz/Twitter-X-Media-Batch-Downloader/releases/latest"
      );
      const data = await response.json();
      const latestVersion = data.tag_name?.replace(/^v/, "") || "";

      if (data.published_at) {
        setReleaseDate(data.published_at);
      }

      if (latestVersion && latestVersion > CURRENT_VERSION) {
        setHasUpdate(true);
      }
    } catch (err) {
      console.error("Failed to check for updates:", err);
    }
  };

  const loadHistory = () => {
    try {
      const saved = localStorage.getItem(HISTORY_KEY);
      if (saved) {
        setFetchHistory(JSON.parse(saved));
      }
    } catch (err) {
      console.error("Failed to load history:", err);
    }
  };

  const saveHistory = (history: HistoryItem[]) => {
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
    } catch (err) {
      console.error("Failed to save history:", err);
    }
  };

  const addToHistory = (data: TwitterResponse, inputUsername: string) => {
    // Clean username (remove @ and extract from URL if needed)
    let cleanUsername = inputUsername.trim();
    if (cleanUsername.startsWith("@")) {
      cleanUsername = cleanUsername.slice(1);
    }
    if (cleanUsername.includes("x.com/") || cleanUsername.includes("twitter.com/")) {
      const match = cleanUsername.match(/(?:x\.com|twitter\.com)\/([^/?]+)/);
      if (match) cleanUsername = match[1];
    }

    setFetchHistory((prev) => {
      // Use username from API response (account_info.name) for consistency
      const apiUsername = data.account_info.name;
      const filtered = prev.filter((h) => h.username.toLowerCase() !== apiUsername.toLowerCase());
      const newItem: HistoryItem = {
        id: crypto.randomUUID(),
        username: apiUsername,           // username/handle from API
        name: data.account_info.nick,    // display name from API
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

  // Check for resumable fetch when username changes
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
    // Kill any running extractor processes
    try {
      await CleanupExtractorProcesses();
    } catch {
      // Ignore cleanup errors
    }
  }, []);

  // Timer effect for fetch timeout
  useEffect(() => {
    if (loading && fetchStartTimeRef.current !== null) {
      const settings = getSettings();
      const timeoutSeconds = settings.fetchTimeout || 60;
      
      // Update timer every second
      timeoutIntervalRef.current = window.setInterval(() => {
        if (fetchStartTimeRef.current !== null) {
          const elapsed = Math.floor((Date.now() - fetchStartTimeRef.current) / 1000);
          setElapsedTime(elapsed);
          const remaining = Math.max(0, timeoutSeconds - elapsed);
          setRemainingTime(remaining);
          
          // Auto-stop if timeout reached
          if (remaining <= 0) {
            stopFetchRef.current = true;
            logger.warning("Fetch timeout reached. Stopping...");
            toast.warning("Fetch timeout reached. Stopping...");
            handleStopFetch();
          }
        }
      }, 1000);
    } else {
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

  const handleFetch = async (
    useDateRange: boolean,
    startDate?: string,
    endDate?: string,
    mediaType?: string,
    retweets?: boolean,
    mode: FetchMode = "public",
    privateType?: PrivateType,
    authToken?: string,
    isResume?: boolean
  ) => {
    // Prevent multiple concurrent fetches
    if (loading) {
      logger.warning("Fetch already in progress, please wait or stop first");
      toast.warning("Fetch already in progress");
      return;
    }

    // Validate based on mode
    const isBookmarks = mode === "private" && privateType === "bookmarks";
    const isLikes = mode === "private" && privateType === "likes";

    // Username required for public mode and likes mode
    if (!isBookmarks && !username.trim()) {
      toast.error("Please enter a username");
      return;
    }

    // Use auth token from SearchBar
    if (!authToken?.trim()) {
      toast.error("Please enter your auth token");
      return;
    }

    // Reset state for new fetch
    setLoading(true);
    setFetchedMediaType(mediaType || "all");
    stopFetchRef.current = false;
    fetchStartTimeRef.current = Date.now();
    setElapsedTime(0);
    setNewMediaCount(null);
    
    // Get timeout from settings
    const settings = getSettings();
    const timeoutSeconds = settings.fetchTimeout || 60;
    setRemainingTime(timeoutSeconds);

    // Cleanup any leftover processes from previous fetch
    try {
      await CleanupExtractorProcesses();
    } catch {
      // Ignore cleanup errors
    }

    // Determine what we're fetching for logging
    const fetchTarget = isBookmarks
      ? "your bookmarks"
      : isLikes
        ? "your likes"
        : `@${username}`;

    // Check for resume state
    const cleanUsername = isBookmarks ? "bookmarks" : isLikes ? "likes" : username.trim();
    let existingState: FetchState | null = null;
    let cursor: string | undefined;
    let allTimeline: TwitterResponse["timeline"] = [];
    let accountInfo: TwitterResponse["account_info"] | null = null;

    if (isResume) {
      existingState = getFetchState(cleanUsername);
      if (existingState && existingState.cursor && !existingState.completed) {
        cursor = existingState.cursor;
        allTimeline = existingState.timeline;
        accountInfo = existingState.accountInfo;
        logger.info(`Resuming ${fetchTarget} from ${allTimeline.length} items...`);
        
        // Show existing data immediately
        if (accountInfo) {
          setResult({
            account_info: accountInfo,
            timeline: allTimeline,
            total_urls: allTimeline.length,
            metadata: { new_entries: 0, page: 0, batch_size: 0, has_more: true },
          });
        }
      } else {
        toast.error("No resumable fetch found");
        setLoading(false);
        return;
      }
    } else {
      // Fresh fetch - clear any existing state
      clearFetchState(cleanUsername);
      setResult(null);
      logger.info(`Fetching ${fetchTarget}...`);
    }

    try {
      let finalData: TwitterResponse | null = null;

      if (useDateRange && startDate && endDate && mode === "public") {
        // Date range mode - single fetch (only for public mode)
        logger.info(`Using date range: ${startDate} to ${endDate}`);
        
        // Check timeout before date range fetch
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
          media_filter: mediaType || "all",
          retweets: retweets || false,
        });
        finalData = JSON.parse(response);
        
        // Save to database immediately for date range mode
        if (finalData && finalData.account_info) {
          try {
            await SaveAccountToDBWithStatus(
              finalData.account_info.name,
              finalData.account_info.nick,
              finalData.account_info.profile_image,
              finalData.total_urls,
              JSON.stringify(finalData),
              mediaType || "all",
              finalData.cursor || "",
              finalData.completed ?? true
            );
          } catch (err) {
            console.error("Failed to save date range data to database:", err);
          }
        }
        
        // Save to database immediately for date range mode
        if (finalData && finalData.account_info) {
          try {
            await SaveAccountToDBWithStatus(
              finalData.account_info.name,
              finalData.account_info.nick,
              finalData.account_info.profile_image,
              finalData.total_urls,
              JSON.stringify(finalData),
              mediaType || "all",
              finalData.cursor || "",
              finalData.completed ?? true
            );
          } catch (err) {
            console.error("Failed to save date range data to database:", err);
          }
        }
      } else {
        // Timeline mode - check fetch mode from settings
        const settings = getSettings();
        const isSingleMode = settings.fetchMode === "single";
        const batchSize = isSingleMode ? 0 : BATCH_SIZE;
        
        let timelineType = "timeline";
        if (isBookmarks) {
          timelineType = "bookmarks";
        } else if (isLikes) {
          timelineType = "likes";
        }

        let hasMore = true;
        let page = 0;

        // Single mode: one fetch, no loop
        // Batch mode: loop until done or stopped
        while (hasMore && !stopFetchRef.current) {
          // Check timeout before each batch
          if (fetchStartTimeRef.current !== null) {
            const settings = getSettings();
            const timeoutSeconds = settings.fetchTimeout || 60;
            const elapsed = Math.floor((Date.now() - fetchStartTimeRef.current) / 1000);
            if (elapsed >= timeoutSeconds) {
              logger.warning(`Timeout reached (${timeoutSeconds}s). Stopping fetch...`);
              stopFetchRef.current = true;
              toast.warning("Fetch timeout reached. Stopping...");
              
              // IMPORTANT: Save state immediately before breaking out of loop
              // This ensures no data is lost when timeout occurs
              if (accountInfo && allTimeline.length > 0) {
                saveFetchState({
                  username: cleanUsername,
                  cursor: cursor || "",
                  timeline: allTimeline,
                  accountInfo: accountInfo,
                  totalFetched: allTimeline.length,
                  completed: false,
                  authToken: authToken.trim(),
                  mediaType: mediaType || "all",
                  retweets: retweets || false,
                  timelineType: timelineType,
                });
                
                // Also save to database immediately
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
                  await SaveAccountToDBWithStatus(
                    accountInfo.name,
                    accountInfo.nick,
                    accountInfo.profile_image,
                    allTimeline.length,
                    JSON.stringify(timeoutResponse),
                    mediaType || "all",
                    cursor || "",
                    false
                  );
                  logger.info(`Saved ${allTimeline.length} items before timeout`);
                } catch (err) {
                  console.error("Failed to save timeout state to database:", err);
                }
              }
              break;
            }
          }

          const batchNum = page + 1;
          logger.info(isSingleMode ? "Fetching all media..." : `Fetching batch ${batchNum}${cursor ? " (resuming)" : ""}...`);

          const response = await ExtractTimeline({
            username: isBookmarks ? "" : username.trim(),
            auth_token: authToken.trim(),
            timeline_type: timelineType,
            batch_size: batchSize,
            page: page,
            media_type: mediaType || "all",
            retweets: retweets || false,
            cursor: cursor,
          });

          const data: TwitterResponse = JSON.parse(response);

          // Set account info from first response
          if (!accountInfo && data.account_info) {
            accountInfo = data.account_info;
            if (isBookmarks) {
              // For bookmarks, ensure name is "bookmarks" and nick is "My Bookmarks"
              accountInfo.name = "bookmarks";
              accountInfo.nick = "My Bookmarks";
            } else if (isLikes) {
              // For likes, ensure name is "likes" and nick is "My Likes"
              accountInfo.name = "likes";
              accountInfo.nick = "My Likes";
            }
          }

          // Merge new entries (deduplicate)
          const previousCount = allTimeline.length;
          allTimeline = mergeTimelines(allTimeline, data.timeline);
          const newCount = allTimeline.length - previousCount;
          
          // Update new media count for display
          if (newCount > 0) {
            setNewMediaCount(newCount);
            // Clear after 1 second
            setTimeout(() => setNewMediaCount(null), 1000);
          }

          // Update cursor for next batch
          cursor = data.cursor;
          hasMore = !!data.cursor && !data.completed;
          page++;

          // Save cursor every batch (lightweight, just a string)
          if (cursor) {
            saveCursor(cleanUsername, cursor);
          }

          // Update UI progressively - show results immediately
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

          // Save state EVERY batch to ensure no data loss on timeout/error
          // This is critical for large accounts where fetch can take a long time
          saveFetchState({
            username: cleanUsername,
            cursor: cursor || "",
            timeline: allTimeline,
            accountInfo: accountInfo,
            totalFetched: allTimeline.length,
            completed: !hasMore,
            authToken: authToken.trim(),
            mediaType: mediaType || "all",
            retweets: retweets || false,
            timelineType: timelineType,
          });

          // Save to database immediately after each batch for realtime persistence
          // This ensures data is saved even if fetch fails mid-way
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
              await SaveAccountToDBWithStatus(
                accountInfo.name,
                accountInfo.nick,
                accountInfo.profile_image,
                allTimeline.length,
                JSON.stringify(currentResponse),
                mediaType || "all",
                cursor || "",
                !hasMore
              );
            } catch (err) {
              console.error("Failed to save progress to database:", err);
            }
          }
        }

        // If stopped by user, keep state for resume
        if (stopFetchRef.current) {
          const elapsedSecs = fetchStartTimeRef.current ? Math.floor((Date.now() - fetchStartTimeRef.current) / 1000) : 0;
          logger.info(`Stopped at ${allTimeline.length} items - can resume later (${elapsedSecs}s)`);
          toast.info(`Stopped at ${formatNumberWithComma(allTimeline.length)} items`);
          setResumeInfo({ canResume: true, mediaCount: allTimeline.length });
        } else {
          // Completed - clear state
          clearFetchState(cleanUsername);
          clearCursor(cleanUsername);
          setResumeInfo(null);
        }

        // Build final response
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

        // Only add to history for public mode
        if (mode === "public") {
          addToHistory(finalData, username);
        }

        // Note: Data is already saved to database after each batch for timeline mode
        // and immediately after fetch for date range mode, so we don't need to save again here
        // unless it's the final completion status update for timeline mode
        if (!useDateRange) {
          // For timeline mode, final save is already done in the loop after each batch
          // This is just for ensuring final status is correct
          try {
            await SaveAccountToDBWithStatus(
              finalData.account_info.name,
              finalData.account_info.nick,
              finalData.account_info.profile_image,
              finalData.total_urls,
              JSON.stringify(finalData),
              mediaType || "all",
              finalData.cursor || "",
              finalData.completed ?? true
            );
          } catch (err) {
            console.error("Failed to save final status to database:", err);
          }
        }

        if (!stopFetchRef.current) {
          const elapsedSecs = fetchStartTimeRef.current ? Math.floor((Date.now() - fetchStartTimeRef.current) / 1000) : 0;
          logger.success(`Found ${finalData.total_urls} media items (${elapsedSecs}s)`);
          toast.success(`${finalData.total_urls} media items found`);
        }
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      const elapsedSecs = fetchStartTimeRef.current ? Math.floor((Date.now() - fetchStartTimeRef.current) / 1000) : 0;
      logger.error(`Failed to fetch: ${errorMsg} (${elapsedSecs}s)`);
      toast.error("Failed to fetch media");

      // On error, check if we have partial data saved
      const savedState = getFetchState(cleanUsername);
      if (savedState && savedState.timeline.length > 0) {
        const partialResponse = stateToResponse(savedState);
        if (partialResponse) {
          setResult(partialResponse);
          setResumeInfo({ canResume: true, mediaCount: savedState.timeline.length });
          toast.info(`Saved ${formatNumberWithComma(savedState.timeline.length)} items - can resume`);
          
          // Also save partial data to database for persistence
          try {
            await SaveAccountToDBWithStatus(
              partialResponse.account_info.name,
              partialResponse.account_info.nick,
              partialResponse.account_info.profile_image,
              partialResponse.total_urls,
              JSON.stringify(partialResponse),
              mediaType || "all",
              savedState.cursor || "",
              false // not completed
            );
          } catch (dbErr) {
            console.error("Failed to save partial data to database:", dbErr);
          }
        }
      }
    } finally {
      setLoading(false);
      // Reset timer when fetch completes or stops
      fetchStartTimeRef.current = null;
      setElapsedTime(0);
      setRemainingTime(null);
    }
  };

  // Handle resume fetch
  const handleResume = (authToken: string, mediaType?: string, retweets?: boolean) => {
    handleFetch(false, undefined, undefined, mediaType, retweets, "public", undefined, authToken, true);
  };

  // Clear resume state
  const handleClearResume = () => {
    if (username.trim()) {
      clearFetchState(username.trim());
      setResumeInfo(null);
      toast.info("Resume data cleared");
    }
  };

  const handleLoadFromDB = (responseJSON: string, loadedUsername: string) => {
    try {
      const data: TwitterResponse = JSON.parse(responseJSON);
      setResult(data);
      setUsername(loadedUsername);
      setCurrentPage("main");
      
      // Set fetch type to single when loading from database
      setFetchType("single");
      
      // Determine if account is private (bookmarks or likes)
      const isPrivate = loadedUsername === "bookmarks" || loadedUsername === "likes";
      
      // Set mode based on account type
      if (isPrivate) {
        if (loadedUsername === "bookmarks") {
          setSearchMode("private");
          setSearchPrivateType("bookmarks");
        } else if (loadedUsername === "likes") {
          setSearchMode("private");
          setSearchPrivateType("likes");
        }
      } else {
        setSearchMode("public");
      }
      
      toast.success(`Loaded @${loadedUsername} from database`);
    } catch (error) {
      toast.error("Failed to parse saved data");
    }
  };

  // Parse username from various formats
  const parseUsername = (input: string): string => {
    let clean = input.trim();
    if (clean.startsWith("@")) {
      clean = clean.slice(1);
    }
    if (clean.includes("x.com/") || clean.includes("twitter.com/")) {
      const match = clean.match(/(?:x\.com|twitter\.com)\/([^/?]+)/);
      if (match) clean = match[1];
    }
    return clean;
  };

  // Handle update selected accounts from database view
  const handleUpdateSelected = (usernames: string[]) => {
    if (usernames.length === 0) {
      toast.error("No accounts selected");
      return;
    }

    // Parse usernames (same as handleImportFile)
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

    // Merge with existing accounts (avoid duplicates by username)
    setMultipleAccounts((prev) => {
      const existingUsernames = new Set(prev.map((acc) => acc.username.toLowerCase()));
      const newAccounts = accounts.filter(
        (acc) => !existingUsernames.has(acc.username.toLowerCase())
      );
      return [...prev, ...newAccounts];
    });

    // Set fetch type to multiple and navigate to home
    setFetchType("multiple");
    setCurrentPage("main");
    toast.success(`Added ${formatNumberWithComma(accounts.length)} account(s) to multiple fetch`);
  };

  // Handle import txt file
  const handleImportFile = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".txt";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      try {
        const text = await file.text();
        const lines = text.split("\n").map((line) => line.trim()).filter((line) => line.length > 0);
        
        if (lines.length === 0) {
          toast.error("File is empty");
          return;
        }

        const accounts: MultipleAccount[] = lines.map((line) => {
          const username = parseUsername(line);
          return {
            id: crypto.randomUUID(),
            username,
            status: "pending",
            mediaCount: 0,
            previousMediaCount: 0,
            elapsedTime: 0,
            remainingTime: null,
            showDiff: false,
          };
        });

        setMultipleAccounts(accounts);
        toast.success(`Imported ${formatNumberWithComma(accounts.length)} account(s)`);
      } catch (error) {
        toast.error("Failed to read file");
      }
    };
    input.click();
  };

  // Handle fetch all accounts
  const handleFetchAll = async () => {
    if (multipleAccounts.length === 0) {
      toast.error("No accounts to fetch");
      return;
    }

    setIsFetchingAll(true);
    stopAllRef.current = false;

    // Get auth token from localStorage (same as SearchBar)
    const PUBLIC_AUTH_TOKEN_KEY = "twitter_public_auth_token";
    const authToken = localStorage.getItem(PUBLIC_AUTH_TOKEN_KEY) || "";
    if (!authToken.trim()) {
      toast.error("Please enter your auth token");
      setIsFetchingAll(false);
      return;
    }

      // Get timeout from settings
      const settings = getSettings();
      const timeoutSeconds = settings.fetchTimeout || 60;

      // Reset all accounts to pending
      setMultipleAccounts((prev) =>
        prev.map((acc) => ({
          ...acc,
          status: "pending" as const,
          mediaCount: 0,
          previousMediaCount: 0,
          elapsedTime: 0,
          remainingTime: timeoutSeconds,
          error: undefined,
          showDiff: false,
        }))
      );

    // Get current accounts list (use state getter function)
    let currentAccounts = [...multipleAccounts];

    // Fetch each account sequentially
    for (let i = 0; i < currentAccounts.length; i++) {
      if (stopAllRef.current) {
        break;
      }

      const account = currentAccounts[i];
      const accountId = account.id;
      accountStopFlagsRef.current.set(accountId, false);

      // Update status to fetching
      setMultipleAccounts((prev) =>
        prev.map((acc) =>
          acc.id === accountId
            ? { ...acc, status: "fetching" as const }
            : acc
        )
      );

      // Start timer for this account (countdown)
      const settings = getSettings();
      const timeoutSeconds = settings.fetchTimeout || 60;
      accountStartTimesRef.current.set(accountId, Date.now());
      accountTimeoutSecondsRef.current.set(accountId, timeoutSeconds);
      
      // Initialize with timeout
      setMultipleAccounts((prev) =>
        prev.map((acc) =>
          acc.id === accountId
            ? { ...acc, elapsedTime: 0, remainingTime: timeoutSeconds }
            : acc
        )
      );

      const timerInterval = window.setInterval(() => {
        const startTime = accountStartTimesRef.current.get(accountId);
        const timeoutSecs = accountTimeoutSecondsRef.current.get(accountId) || timeoutSeconds;
        if (startTime) {
          const elapsed = Math.floor((Date.now() - startTime) / 1000);
          const remaining = Math.max(0, timeoutSecs - elapsed);
          setMultipleAccounts((prev) =>
            prev.map((acc) =>
              acc.id === accountId
                ? { ...acc, elapsedTime: elapsed, remainingTime: remaining }
                : acc
            )
          );
          
          // Auto-stop if timeout reached
          if (remaining <= 0) {
            accountStopFlagsRef.current.set(accountId, true);
            clearInterval(timerInterval);
            accountTimersRef.current.delete(accountId);
            accountStartTimesRef.current.delete(accountId);
            accountTimeoutSecondsRef.current.delete(accountId);
            
            // Check if any media was fetched using ref (most up-to-date value)
            const mediaCount = accountMediaCountRef.current.get(accountId) || 0;
            setMultipleAccounts((prev) =>
              prev.map((acc) => {
                if (acc.id === accountId) {
                  // If no media was fetched at all, mark as failed
                  // Otherwise mark as incomplete
                  const status = mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
                  return { ...acc, status, remainingTime: 0, mediaCount };
                }
                return acc;
              })
            );
            CleanupExtractorProcesses().catch(() => {});
          }
        }
      }, 1000);
      accountTimersRef.current.set(accountId, timerInterval);

      try {
        // Check if account should be stopped
        if (stopAllRef.current) {
          clearInterval(timerInterval);
          accountTimersRef.current.delete(accountId);
          accountStartTimesRef.current.delete(accountId);
          accountTimeoutSecondsRef.current.delete(accountId);
          // Check if any media was fetched
          setMultipleAccounts((prev) =>
            prev.map((acc) => {
              if (acc.id === accountId) {
                const status = acc.mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
                return { ...acc, status };
              }
              return acc;
            })
          );
          continue;
        }

        // Fetch account - check fetch mode from settings
        const fetchSettings = getSettings();
        const isSingleModeMultiple = fetchSettings.fetchMode === "single";
        const batchSizeMultiple = isSingleModeMultiple ? 0 : BATCH_SIZE;
        
        const cleanUsername = account.username.trim();
        let allTimeline: TwitterResponse["timeline"] = [];
        let accountInfo: TwitterResponse["account_info"] | null = null;
        let cursor: string | undefined;
        let hasMore = true;
        let page = 0;
        let previousMediaCount = account.mediaCount || 0;

        while (hasMore && !stopAllRef.current) {
          // Check if this specific account should be stopped (timeout)
          if (accountStopFlagsRef.current.get(accountId)) {
            // IMPORTANT: Save state before breaking to prevent data loss
            if (accountInfo && allTimeline.length > 0) {
              saveFetchState({
                username: cleanUsername,
                cursor: cursor || "",
                timeline: allTimeline,
                accountInfo: accountInfo,
                totalFetched: allTimeline.length,
                completed: false,
                authToken: authToken.trim(),
                mediaType: fetchedMediaType || "all",
                retweets: false,
                timelineType: "timeline",
              });
              
              // Also save to database
              try {
                await SaveAccountToDBWithStatus(
                  accountInfo.name,
                  accountInfo.nick,
                  accountInfo.profile_image,
                  allTimeline.length,
                  JSON.stringify({
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
                  }),
                  fetchedMediaType || "all",
                  cursor || "",
                  false
                );
              } catch (err) {
                console.error("Failed to save timeout state to database:", err);
              }
            }
            break;
          }

          const response = await ExtractTimeline({
            username: cleanUsername,
            auth_token: authToken.trim(),
            timeline_type: "timeline",
            batch_size: batchSizeMultiple,
            page: page,
            media_type: fetchedMediaType || "all",
            retweets: false,
            cursor: cursor,
          });

          const data: TwitterResponse = JSON.parse(response);

          if (!accountInfo && data.account_info) {
            accountInfo = data.account_info;
          }

          allTimeline = mergeTimelines(allTimeline, data.timeline);

          cursor = data.cursor;
          hasMore = !!data.cursor && !data.completed;
          page++;

          // Save cursor every batch (lightweight, just a string)
          if (cursor) {
            saveCursor(cleanUsername, cursor);
          }

          // Update account with progress
          const currentMediaCount = allTimeline.length;
          accountMediaCountRef.current.set(accountId, currentMediaCount);
          const hasNewItems = currentMediaCount > previousMediaCount;
          
          setMultipleAccounts((prev) =>
            prev.map((acc) => {
              if (acc.id === accountId) {
                return {
                  ...acc,
                  accountInfo: accountInfo || undefined,
                  previousMediaCount: previousMediaCount,
                  mediaCount: currentMediaCount,
                  showDiff: hasNewItems,
                  cursor: cursor, // Save cursor for retry
                };
              }
              return acc;
            })
          );
          
          // Clear diff after 1 second
          if (hasNewItems) {
            setTimeout(() => {
              setMultipleAccounts((prev) =>
                prev.map((acc) =>
                  acc.id === accountId ? { ...acc, showDiff: false } : acc
                )
              );
            }, 1000);
          }
          
          // Update previous count for next iteration
          previousMediaCount = currentMediaCount;

          // Save fetch state EVERY batch to ensure no data loss on timeout/error
          if (accountInfo) {
            saveFetchState({
              username: cleanUsername,
              cursor: cursor || "",
              timeline: allTimeline,
              accountInfo: accountInfo,
              totalFetched: allTimeline.length,
              completed: !hasMore,
              authToken: authToken.trim(),
              mediaType: fetchedMediaType || "all",
              retweets: false,
              timelineType: "timeline",
            });
          }

          // Save to database after each batch
          if (accountInfo) {
            try {
              await SaveAccountToDBWithStatus(
                accountInfo.name,
                accountInfo.nick,
                accountInfo.profile_image,
                allTimeline.length,
                JSON.stringify({
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
                }),
                fetchedMediaType || "all",
                cursor || "",
                !hasMore
              );
            } catch (err) {
              console.error("Failed to save progress to database:", err);
            }
          }
        }

        // Calculate elapsed time before clearing
        const startTime = accountStartTimesRef.current.get(accountId);
        const elapsedSecs = startTime ? Math.floor((Date.now() - startTime) / 1000) : 0;

        // Clear timer
        clearInterval(timerInterval);
        accountTimersRef.current.delete(accountId);
        accountStartTimesRef.current.delete(accountId);
        accountTimeoutSecondsRef.current.delete(accountId);

        // Update final status - check if media was fetched
        const finalMediaCount = allTimeline.length;
        accountMediaCountRef.current.set(accountId, finalMediaCount);
        
        // Check if stopped due to timeout (accountStopFlags was set by timeout handler)
        const wasTimeout = accountStopFlagsRef.current.get(accountId);
        
        if (wasTimeout) {
          // Timeout: incomplete if media was fetched, failed if no media
          logger.warning(`@${account.username}: timeout - ${finalMediaCount} items (${elapsedSecs}s)`);
          setMultipleAccounts((prev) =>
            prev.map((acc) =>
              acc.id === accountId
                ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
                : acc
            )
          );
        } else if (stopAllRef.current) {
          // Stopped by user: incomplete if media was fetched, failed if no media
          logger.info(`@${account.username}: stopped - ${finalMediaCount} items (${elapsedSecs}s)`);
          setMultipleAccounts((prev) =>
            prev.map((acc) =>
              acc.id === accountId
                ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
                : acc
            )
          );
        } else if (hasMore) {
          // Has more but stopped (e.g., API error): incomplete if media was fetched, failed if no media
          logger.warning(`@${account.username}: incomplete - ${finalMediaCount} items (${elapsedSecs}s)`);
          setMultipleAccounts((prev) =>
            prev.map((acc) =>
              acc.id === accountId
                ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
                : acc
            )
          );
        } else {
          // Completed successfully
          logger.success(`@${account.username}: completed - ${finalMediaCount} items (${elapsedSecs}s)`);
          setMultipleAccounts((prev) =>
            prev.map((acc) =>
              acc.id === accountId
                ? { ...acc, status: "completed" as const }
                : acc
            )
          );
        }
      } catch (error) {
        // Calculate elapsed time before clearing
        const startTime = accountStartTimesRef.current.get(accountId);
        const elapsedSecs = startTime ? Math.floor((Date.now() - startTime) / 1000) : 0;
        
        // Clear timer on error
        const timerInterval = accountTimersRef.current.get(accountId);
        if (timerInterval) {
          clearInterval(timerInterval);
          accountTimersRef.current.delete(accountId);
        }
        accountStartTimesRef.current.delete(accountId);
        accountTimeoutSecondsRef.current.delete(accountId);
        
        const errorMsg = error instanceof Error ? error.message : String(error);
        
        // Check if any media was fetched before the error
        const mediaCount = accountMediaCountRef.current.get(accountId) || 0;
        
        // Check if error is auth token related (401, unauthorized, invalid, expired)
        const isAuthError = errorMsg.toLowerCase().includes("401") || 
                           errorMsg.toLowerCase().includes("unauthorized") ||
                           errorMsg.toLowerCase().includes("auth token may be invalid") ||
                           errorMsg.toLowerCase().includes("auth token may be expired") ||
                           errorMsg.toLowerCase().includes("invalid or expired");
        
        // Determine status:
        // - If auth error: always "failed" (even if media was fetched, it's likely incomplete data)
        // - If not auth error and media was fetched: "incomplete"
        // - If not auth error and no media: "failed"
        const status = isAuthError || mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
        
        accountMediaCountRef.current.delete(accountId);
        
        setMultipleAccounts((prev) =>
          prev.map((acc) =>
            acc.id === accountId
              ? {
                  ...acc,
                  status,
                  error: errorMsg,
                  mediaCount,
                }
              : acc
          )
        );
        logger.error(`@${account.username}: failed - ${errorMsg} (${elapsedSecs}s)`);
      }
    }

    setIsFetchingAll(false);
    if (!stopAllRef.current) {
      toast.success("All accounts fetched");
    }
  };

  // Handle stop all
  const handleStopAll = () => {
    stopAllRef.current = true;
    setIsFetchingAll(false);
    
    // Stop all timers
    accountTimersRef.current.forEach((interval) => clearInterval(interval));
    accountTimersRef.current.clear();
    accountStartTimesRef.current.clear();
    accountTimeoutSecondsRef.current.clear();
    // Keep accountMediaCountRef for status checking

    // Update all fetching accounts - check if they have media using ref (most up-to-date value)
    setMultipleAccounts((prev) =>
      prev.map((acc) => {
        if (acc.status === "fetching") {
          // If no media was fetched, mark as failed; otherwise incomplete
          const mediaCount = accountMediaCountRef.current.get(acc.id) || acc.mediaCount || 0;
          const status = mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
          return { ...acc, status, mediaCount };
        }
        return acc;
      })
    );

    // Cleanup processes
    CleanupExtractorProcesses().catch(() => {});
    toast.info("Stopped all fetches");
  };

  // Handle stop specific account
  const handleStopAccount = (accountId: string) => {
    // Set stop flag for this account
    accountStopFlagsRef.current.set(accountId, true);

    // Check if any media was fetched before marking status using ref (most up-to-date value)
    setMultipleAccounts((prev) =>
      prev.map((acc) => {
        if (acc.id === accountId && acc.status === "fetching") {
          // If no media was fetched, mark as failed; otherwise incomplete
          const mediaCount = accountMediaCountRef.current.get(accountId) || acc.mediaCount || 0;
          const status = mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
          return { ...acc, status, mediaCount };
        }
        return acc;
      })
    );

    // Clear timer for this account
    const timerInterval = accountTimersRef.current.get(accountId);
    if (timerInterval) {
      clearInterval(timerInterval);
      accountTimersRef.current.delete(accountId);
    }
    accountStartTimesRef.current.delete(accountId);
    accountTimeoutSecondsRef.current.delete(accountId);

    // Cleanup processes
    CleanupExtractorProcesses().catch(() => {});
    toast.info("Stopped account fetch");
  };

  // Handle retry specific account
  const handleRetryAccount = async (accountId: string) => {
    const account = multipleAccounts.find((acc) => acc.id === accountId);
    if (!account) return;

    // Get auth token from localStorage
    const PUBLIC_AUTH_TOKEN_KEY = "twitter_public_auth_token";
    const authToken = localStorage.getItem(PUBLIC_AUTH_TOKEN_KEY) || "";
    if (!authToken.trim()) {
      toast.error("Please enter your auth token");
      return;
    }

    // Get fetch mode from settings
    const retrySettings = getSettings();
    const isSingleModeRetry = retrySettings.fetchMode === "single";
    const batchSizeRetry = isSingleModeRetry ? 0 : BATCH_SIZE;

    // Check if we have a saved cursor (like resume in single fetch)
    const cleanUsername = account.username.trim();
    let existingState: FetchState | null = null;
    let cursor: string | undefined = account.cursor; // Use saved cursor from account state
    let allTimeline: TwitterResponse["timeline"] = [];
    let accountInfo: TwitterResponse["account_info"] | null = null;
    let previousMediaCount = account.mediaCount || 0;

    // Try to load from fetch state (like single fetch resume)
    existingState = getFetchState(cleanUsername);
    if (existingState && existingState.cursor && !existingState.completed) {
      cursor = existingState.cursor;
      allTimeline = existingState.timeline;
      if (existingState.accountInfo) {
        accountInfo = existingState.accountInfo;
      }
      previousMediaCount = existingState.timeline.length;
      logger.info(`Resuming @${cleanUsername} from ${allTimeline.length} items...`);
    } else {
      // Try to get cursor from lightweight storage (localStorage)
      if (!cursor) {
        const savedCursor = getCursor(cleanUsername);
        if (savedCursor) {
          cursor = savedCursor;
          previousMediaCount = account.mediaCount || 0;
          logger.info(`Retrying @${cleanUsername} from cursor (${previousMediaCount} items)...`);
        } else {
          // Fallback: try to get cursor from database
          try {
            const accounts = await GetAllAccountsFromDB();
            const dbAccount = accounts.find(
              (acc) => acc.username.toLowerCase() === cleanUsername.toLowerCase()
            );
            if (dbAccount?.cursor) {
              cursor = dbAccount.cursor;
              previousMediaCount = account.mediaCount || 0;
              logger.info(`Retrying @${cleanUsername} from database cursor (${previousMediaCount} items)...`);
            } else {
              // Fresh retry - clear any existing state
              clearFetchState(cleanUsername);
              clearCursor(cleanUsername);
              logger.info(`Retrying @${cleanUsername} from beginning...`);
            }
          } catch (dbError) {
            console.error("Failed to get cursor from database:", dbError);
            clearFetchState(cleanUsername);
            clearCursor(cleanUsername);
            logger.info(`Retrying @${cleanUsername} from beginning...`);
          }
        }
      } else {
        previousMediaCount = account.mediaCount || 0;
        logger.info(`Retrying @${cleanUsername} from cursor (${previousMediaCount} items)...`);
      }
    }

    // Update account status to fetching (preserve existing data)
    setMultipleAccounts((prev) =>
      prev.map((acc) =>
        acc.id === accountId
          ? {
              ...acc,
              status: "fetching" as const,
              elapsedTime: 0,
              remainingTime: null,
              error: undefined,
              showDiff: false,
              mediaCount: previousMediaCount, // Preserve existing count
              previousMediaCount: previousMediaCount,
            }
          : acc
      )
    );

    accountStopFlagsRef.current.set(accountId, false);

    // Start timer for this account
    const settings = getSettings();
    const timeoutSeconds = settings.fetchTimeout || 60;
    accountStartTimesRef.current.set(accountId, Date.now());
    accountTimeoutSecondsRef.current.set(accountId, timeoutSeconds);

    setMultipleAccounts((prev) =>
      prev.map((acc) =>
        acc.id === accountId
          ? { ...acc, elapsedTime: 0, remainingTime: timeoutSeconds }
          : acc
      )
    );

    const timerInterval = window.setInterval(() => {
      const startTime = accountStartTimesRef.current.get(accountId);
      const timeoutSecs = accountTimeoutSecondsRef.current.get(accountId) || timeoutSeconds;
      if (startTime) {
        const elapsed = Math.floor((Date.now() - startTime) / 1000);
        const remaining = Math.max(0, timeoutSecs - elapsed);
        setMultipleAccounts((prev) =>
          prev.map((acc) =>
            acc.id === accountId
              ? { ...acc, elapsedTime: elapsed, remainingTime: remaining }
              : acc
          )
        );

        // Auto-stop if timeout reached
        if (remaining <= 0) {
          accountStopFlagsRef.current.set(accountId, true);
          clearInterval(timerInterval);
          accountTimersRef.current.delete(accountId);
          accountStartTimesRef.current.delete(accountId);
          accountTimeoutSecondsRef.current.delete(accountId);

          // Check if any media was fetched using ref (most up-to-date value)
          const mediaCount = accountMediaCountRef.current.get(accountId) || 0;
          setMultipleAccounts((prev) =>
            prev.map((acc) => {
              if (acc.id === accountId) {
                const status = mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);
                return { ...acc, status, remainingTime: 0, mediaCount };
              }
              return acc;
            })
          );
          CleanupExtractorProcesses().catch(() => {});
        }
      }
    }, 1000);
    accountTimersRef.current.set(accountId, timerInterval);

    try {
      let hasMore = true;
      let page = 0;

      while (hasMore && !stopAllRef.current) {
        // Check if this specific account should be stopped
        if (accountStopFlagsRef.current.get(accountId)) {
          break;
        }

        const response = await ExtractTimeline({
          username: cleanUsername,
          auth_token: authToken.trim(),
          timeline_type: "timeline",
          batch_size: batchSizeRetry,
          page: page,
          media_type: fetchedMediaType || "all",
          retweets: false,
          cursor: cursor,
        });

        const data: TwitterResponse = JSON.parse(response);

        if (!accountInfo && data.account_info) {
          accountInfo = data.account_info;
        }

        allTimeline = mergeTimelines(allTimeline, data.timeline);

        cursor = data.cursor;
        hasMore = !!data.cursor && !data.completed;
        page++;

        // Save cursor every batch (lightweight, just a string)
        if (cursor) {
          saveCursor(cleanUsername, cursor);
        }

        // Update account with progress
        const currentMediaCount = allTimeline.length;
        accountMediaCountRef.current.set(accountId, currentMediaCount);
        const hasNewItems = currentMediaCount > previousMediaCount;

        setMultipleAccounts((prev) =>
          prev.map((acc) => {
            if (acc.id === accountId) {
              return {
                ...acc,
                accountInfo: accountInfo || undefined,
                previousMediaCount: previousMediaCount,
                mediaCount: currentMediaCount,
                showDiff: hasNewItems,
                cursor: cursor, // Save cursor for next retry
              };
            }
            return acc;
          })
        );

        // Save state periodically (like single fetch) for resume capability
        if (page % 3 === 0 || !hasMore || accountStopFlagsRef.current.get(accountId)) {
          saveFetchState({
            username: cleanUsername,
            cursor: cursor || "",
            timeline: allTimeline,
            accountInfo: accountInfo,
            totalFetched: allTimeline.length,
            completed: !hasMore,
            authToken: authToken.trim(),
            mediaType: fetchedMediaType || "all",
            retweets: false,
            timelineType: "timeline",
          });
        }

        // Clear diff after 1 second
        if (hasNewItems) {
          setTimeout(() => {
            setMultipleAccounts((prev) =>
              prev.map((acc) =>
                acc.id === accountId ? { ...acc, showDiff: false } : acc
              )
            );
          }, 1000);
        }

        // Update previous count for next iteration
        previousMediaCount = currentMediaCount;

        // Save to database after each batch
        if (accountInfo) {
          try {
            await SaveAccountToDBWithStatus(
              accountInfo.name,
              accountInfo.nick,
              accountInfo.profile_image,
              allTimeline.length,
              JSON.stringify({
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
              }),
              fetchedMediaType || "all",
              cursor || "",
              !hasMore
            );
          } catch (err) {
            console.error("Failed to save progress to database:", err);
          }
        }
      }

      // Calculate elapsed time before clearing
      const startTime = accountStartTimesRef.current.get(accountId);
      const elapsedSecs = startTime ? Math.floor((Date.now() - startTime) / 1000) : 0;

      // Clear timer
      clearInterval(timerInterval);
      accountTimersRef.current.delete(accountId);
      accountStartTimesRef.current.delete(accountId);
      accountTimeoutSecondsRef.current.delete(accountId);

      // Update final status
      const finalMediaCount = allTimeline.length;
      accountMediaCountRef.current.set(accountId, finalMediaCount);

      const wasTimeout = accountStopFlagsRef.current.get(accountId);

      if (wasTimeout) {
        logger.warning(`@${account.username}: timeout - ${finalMediaCount} items (${elapsedSecs}s)`);
        setMultipleAccounts((prev) =>
          prev.map((acc) =>
            acc.id === accountId
              ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
              : acc
          )
        );
      } else if (stopAllRef.current) {
        logger.info(`@${account.username}: stopped - ${finalMediaCount} items (${elapsedSecs}s)`);
        setMultipleAccounts((prev) =>
          prev.map((acc) =>
            acc.id === accountId
              ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
              : acc
          )
        );
      } else if (hasMore) {
        logger.warning(`@${account.username}: incomplete - ${finalMediaCount} items (${elapsedSecs}s)`);
        setMultipleAccounts((prev) =>
          prev.map((acc) =>
            acc.id === accountId
              ? { ...acc, status: finalMediaCount === 0 ? ("failed" as const) : ("incomplete" as const) }
              : acc
          )
        );
      } else {
        // Completed successfully - clear fetch state and cursor
        logger.success(`@${account.username}: completed - ${finalMediaCount} items (${elapsedSecs}s)`);
        clearFetchState(cleanUsername);
        clearCursor(cleanUsername);
        setMultipleAccounts((prev) =>
          prev.map((acc) =>
            acc.id === accountId ? { ...acc, status: "completed" as const, cursor: undefined } : acc
          )
        );
      }
    } catch (error) {
      // Calculate elapsed time before clearing
      const startTime = accountStartTimesRef.current.get(accountId);
      const elapsedSecs = startTime ? Math.floor((Date.now() - startTime) / 1000) : 0;
      
      // Clear timer on error
      const timerInterval = accountTimersRef.current.get(accountId);
      if (timerInterval) {
        clearInterval(timerInterval);
        accountTimersRef.current.delete(accountId);
      }
      accountStartTimesRef.current.delete(accountId);
      accountTimeoutSecondsRef.current.delete(accountId);

      const errorMsg = error instanceof Error ? error.message : String(error);

      // Check if any media was fetched before the error
      const mediaCount = accountMediaCountRef.current.get(accountId) || 0;

      // Check if error is auth token related
      const isAuthError = errorMsg.toLowerCase().includes("401") ||
                         errorMsg.toLowerCase().includes("unauthorized") ||
                         errorMsg.toLowerCase().includes("auth token may be invalid") ||
                         errorMsg.toLowerCase().includes("auth token may be expired") ||
                         errorMsg.toLowerCase().includes("invalid or expired");

      const status = isAuthError || mediaCount === 0 ? ("failed" as const) : ("incomplete" as const);

      accountMediaCountRef.current.delete(accountId);

      setMultipleAccounts((prev) =>
        prev.map((acc) =>
          acc.id === accountId
            ? {
                ...acc,
                status,
                error: errorMsg,
                mediaCount,
              }
            : acc
        )
      );
      logger.error(`@${account.username}: failed - ${errorMsg} (${elapsedSecs}s)`);
    }
  };

  const renderPage = () => {
    switch (currentPage) {
      case "settings":
        return <SettingsPage />;
      case "debug":
        return <DebugLoggerPage />;
      case "database":
        return (
          <DatabaseView
            onBack={() => setCurrentPage("main")}
            onLoadAccount={handleLoadFromDB}
            onUpdateSelected={handleUpdateSelected}
          />
        );
      default:
        return (
          <>
            <Header
              version={CURRENT_VERSION}
              hasUpdate={hasUpdate}
              releaseDate={releaseDate}
            />

            <SearchBar
              username={username}
              loading={loading}
              onUsernameChange={setUsername}
              onFetch={handleFetch}
              onStopFetch={handleStopFetch}
              onResume={handleResume}
              onClearResume={handleClearResume}
              resumeInfo={resumeInfo}
              history={fetchHistory}
              onHistorySelect={handleHistorySelect}
              onHistoryRemove={removeFromHistory}
              hasResult={!!result}
              elapsedTime={elapsedTime}
              remainingTime={remainingTime}
              fetchType={fetchType}
              onFetchTypeChange={handleFetchTypeChange}
              multipleAccounts={multipleAccounts}
              onImportFile={handleImportFile}
              onFetchAll={handleFetchAll}
              onStopAll={handleStopAll}
              onStopAccount={handleStopAccount}
              onRetryAccount={handleRetryAccount}
              isFetchingAll={isFetchingAll}
              mode={searchMode}
              privateType={searchPrivateType}
              onModeChange={(mode, privateType) => {
                setSearchMode(mode);
                if (privateType) {
                  setSearchPrivateType(privateType);
                }
              }}
            />

            {result && fetchType === "single" && (
              <div className="mt-4">
                <MediaList
                  accountInfo={result.account_info}
                  timeline={result.timeline}
                  totalUrls={result.total_urls}
                  fetchedMediaType={fetchedMediaType}
                  newMediaCount={newMediaCount}
                />
              </div>
            )}
          </>
        );
    }
  };

  return (
    <TooltipProvider>
      <div className="min-h-screen bg-background flex flex-col">
        <TitleBar />
        <Sidebar currentPage={currentPage} onPageChange={setCurrentPage} />
        
        {/* Main content area with sidebar offset */}
        <div className="flex-1 ml-14 mt-10 p-4 md:p-8">
          <div className="max-w-5xl mx-auto">
            {renderPage()}
          </div>
        </div>
      </div>
    </TooltipProvider>
  );
}

export default App;
