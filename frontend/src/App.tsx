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
  type FetchState,
} from "@/lib/fetch-state";

// Components
import { TitleBar } from "@/components/TitleBar";
import { Sidebar, type PageType } from "@/components/Sidebar";
import { Header } from "@/components/Header";
import { SearchBar, type FetchMode, type PrivateType } from "@/components/SearchBar";
import { MediaList } from "@/components/MediaList";
import { DatabaseView } from "@/components/DatabaseView";
import { SettingsPage } from "@/components/SettingsPage";
import { DebugLoggerPage } from "@/components/DebugLoggerPage";
import type { HistoryItem } from "@/components/FetchHistory";
import type { TwitterResponse } from "@/types/api";

// Wails bindings
import { ExtractTimeline, ExtractDateRange, SaveAccountToDBWithStatus, CleanupExtractorProcesses } from "../wailsjs/go/main/App";

const HISTORY_KEY = "twitter_media_fetch_history";
const MAX_HISTORY = 10;
const CURRENT_VERSION = "4.0";
const BATCH_SIZE = 200; // Fetch in batches for progressive display and resume

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
  const stopFetchRef = useRef(false);

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

  const handleStopFetch = async () => {
    stopFetchRef.current = true;
    logger.info("Stopping fetch...");
    toast.info("Stopping...");
    // Kill any running extractor processes
    try {
      await CleanupExtractorProcesses();
    } catch {
      // Ignore cleanup errors
    }
  };

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
    const cleanUsername = isBookmarks ? "bookmarks" : username.trim();
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
        const response = await ExtractDateRange({
          username: username.trim(),
          auth_token: authToken.trim(),
          start_date: startDate,
          end_date: endDate,
          media_filter: mediaType || "all",
          retweets: retweets || false,
        });
        finalData = JSON.parse(response);
      } else {
        // Timeline mode with batching for progressive display and resume
        let timelineType = "timeline";
        if (isBookmarks) {
          timelineType = "bookmarks";
        } else if (isLikes) {
          timelineType = "likes";
        }

        let hasMore = true;
        let page = 0;

        while (hasMore && !stopFetchRef.current) {
          const batchNum = page + 1;
          logger.info(`Fetching batch ${batchNum}${cursor ? " (resuming)" : ""}...`);

          const response = await ExtractTimeline({
            username: isBookmarks ? "" : username.trim(),
            auth_token: authToken.trim(),
            timeline_type: timelineType,
            batch_size: BATCH_SIZE,
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
              accountInfo.name = "bookmarks";
              accountInfo.nick = "My Bookmarks";
            }
          }

          // Merge new entries (deduplicate)
          allTimeline = mergeTimelines(allTimeline, data.timeline);

          // Update cursor for next batch
          cursor = data.cursor;
          hasMore = !!data.cursor && !data.completed;
          page++;

          // Update UI progressively - show results immediately
          if (accountInfo) {
            setResult({
              account_info: accountInfo,
              timeline: allTimeline,
              total_urls: allTimeline.length,
              metadata: {
                new_entries: data.timeline.length,
                page: page,
                batch_size: BATCH_SIZE,
                has_more: hasMore,
                cursor: cursor,
                completed: !hasMore,
              },
              cursor: cursor,
              completed: !hasMore,
            });
          }

          logger.info(`Fetched ${allTimeline.length} items total`);

          // Save state periodically (every 3 batches) or when stopping
          // This reduces localStorage writes for better performance
          if (page % 3 === 0 || !hasMore || stopFetchRef.current) {
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

            // Also save to database for persistence (can resume even after app restart)
            if (accountInfo) {
              const currentResponse: TwitterResponse = {
                account_info: accountInfo,
                timeline: allTimeline,
                total_urls: allTimeline.length,
                metadata: {
                  new_entries: data.timeline.length,
                  page: page,
                  batch_size: BATCH_SIZE,
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
        }

        // If stopped by user, keep state for resume
        if (stopFetchRef.current) {
          logger.info(`Stopped at ${allTimeline.length} items - can resume later`);
          toast.info(`Stopped at ${allTimeline.length} items`);
          setResumeInfo({ canResume: true, mediaCount: allTimeline.length });
        } else {
          // Completed - clear state
          clearFetchState(cleanUsername);
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
                batch_size: BATCH_SIZE,
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

        // Save to database with media type and completion status
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
          console.error("Failed to save to database:", err);
        }

        if (!stopFetchRef.current) {
          logger.success(`Found ${finalData.total_urls} media items`);
          toast.success(`${finalData.total_urls} media items found`);
        }
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      logger.error(`Failed to fetch: ${errorMsg}`);
      toast.error("Failed to fetch media");

      // On error, check if we have partial data saved
      const savedState = getFetchState(cleanUsername);
      if (savedState && savedState.timeline.length > 0) {
        const partialResponse = stateToResponse(savedState);
        if (partialResponse) {
          setResult(partialResponse);
          setResumeInfo({ canResume: true, mediaCount: savedState.timeline.length });
          toast.info(`Saved ${savedState.timeline.length} items - can resume`);
          
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
      toast.success(`Loaded @${loadedUsername} from database`);
    } catch (error) {
      toast.error("Failed to parse saved data");
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
            />

            {result && (
              <div className="mt-4">
                <MediaList
                  accountInfo={result.account_info}
                  timeline={result.timeline}
                  totalUrls={result.total_urls}
                  fetchedMediaType={fetchedMediaType}
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
