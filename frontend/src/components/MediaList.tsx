import { useState, useMemo, useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

import {
  Image,
  Video,
  Film,
  FileText,
  ExternalLink,
  Repeat2,
  Download,
  FolderOpen,
  LayoutGrid,
  Grid3X3,
  List,
  StopCircle,
  Users,
  UserPlus,
  MessageSquare,
  Calendar,
  ArrowUp,
  ChevronLeft,
  ChevronRight,
  Eye,
  Heart,
  Bookmark,
  BadgeCheck,
  Maximize2,
  CheckCircle,
  XCircle,
  FileCheck,
} from "lucide-react";
import { Spinner } from "@/components/ui/spinner";
import type { TimelineEntry, AccountInfo } from "@/types/api";
import { logger } from "@/lib/logger";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { getSettings } from "@/lib/settings";
import { openExternal } from "@/lib/utils";
import { DownloadMediaWithMetadata, OpenFolder, IsFFmpegInstalled, ConvertGIFs, StopDownload } from "../../wailsjs/go/main/App";
import { EventsOn, EventsOff } from "../../wailsjs/runtime/runtime";
import { main } from "../../wailsjs/go/models";

interface DownloadProgress {
  current: number;
  total: number;
  percent: number;
}

interface DownloadItemStatus {
  tweet_id: number;
  index: number;
  status: "success" | "failed" | "skipped";
}

interface MediaListProps {
  accountInfo: AccountInfo;
  timeline: TimelineEntry[];
  totalUrls: number;
  fetchedMediaType?: string;
}

function getThumbnailUrl(url: string): string {
  if (url.includes("pbs.twimg.com/media/")) {
    if (url.includes("?format=")) {
      if (url.includes("&name=")) {
        const parts = url.split("&name=");
        return parts[0] + "&name=thumb";
      }
      return url + "&name=thumb";
    }
    if (url.includes("?")) {
      return url + "&name=thumb";
    }
    return url + "?format=jpg&name=thumb";
  }
  return url;
}

function getPreviewUrl(url: string): string {
  // For images, use large size for preview
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

function getMediaIcon(type: string) {
  switch (type) {
    case "photo":
      return <Image className="h-4 w-4" />;
    case "video":
      return <Video className="h-4 w-4" />;
    case "gif":
    case "animated_gif":
      return <Film className="h-4 w-4" />;
    case "text":
      return <FileText className="h-4 w-4" />;
    default:
      return <Image className="h-4 w-4" />;
  }
}

function formatDate(dateStr: string): string {
  try {
    // Handle ISO format: 2022-12-30T03:21:36 -> 2022-12-30 • 03:21:36
    if (dateStr.includes("T")) {
      const [datePart, timePart] = dateStr.split("T");
      // Remove timezone if present
      const timeClean = timePart.split("+")[0].split("Z")[0];
      return `${datePart} • ${timeClean}`;
    }
    return dateStr;
  } catch {
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
    } else if (diffMonths > 0) {
      const remainingDays = diffDays % 30;
      return `(${diffMonths}m ${remainingDays}d ago)`;
    } else if (diffDays > 0) {
      const remainingHours = diffHours % 24;
      return `(${diffDays}d ${remainingHours}h ago)`;
    } else if (diffHours > 0) {
      const remainingMinutes = diffMinutes % 60;
      return `(${diffHours}h ${remainingMinutes}m ago)`;
    } else if (diffMinutes > 0) {
      return `(${diffMinutes}m ago)`;
    } else {
      return "(just now)";
    }
  } catch {
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
  // Use tweet_id + url hash for unique identification
  // This ensures each media item is uniquely identified even for same tweet_id
  return `${item.tweet_id}-${item.url}`;
}

function formatJoinDate(dateStr: string): string {
  try {
    const date = new Date(dateStr);
    const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    return `${months[date.getMonth()]} ${date.getFullYear()}`;
  } catch {
    return dateStr;
  }
}

export function MediaList({
  accountInfo,
  timeline,
  totalUrls,
  fetchedMediaType = "all",
}: MediaListProps) {
  const [selectedItems, setSelectedItems] = useState<Set<number>>(new Set());
  const [sortBy, setSortBy] = useState<string>("date-desc");
  const [filterType, setFilterType] = useState<string>("all");
  const [viewMode, setViewMode] = useState<"large" | "small" | "list">("list");
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);
  const [hasDownloaded, setHasDownloaded] = useState(false);
  const [isConverting, setIsConverting] = useState(false);
  const [hasGifs, setHasGifs] = useState(false);
  const [ffmpegInstalled, setFfmpegInstalled] = useState(false);
  const [showScrollTop, setShowScrollTop] = useState(false);
  const [previewIndex, setPreviewIndex] = useState<number | null>(null);
  // Download status tracking per item (by tweet_id + index for uniqueness)
  const [downloadedItems, setDownloadedItems] = useState<Set<string>>(new Set());
  const [failedItems, setFailedItems] = useState<Set<string>>(new Set());
  const [skippedItems, setSkippedItems] = useState<Set<string>>(new Set());
  const [downloadingItem, setDownloadingItem] = useState<string | null>(null);

  // Listen for scroll to show/hide scroll-to-top button
  useEffect(() => {
    const handleScroll = () => {
      setShowScrollTop(window.scrollY > 300);
    };
    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  const scrollToTop = () => {
    window.scrollTo({ top: 0, behavior: "smooth" });
  };

  const openPreview = (index: number) => {
    setPreviewIndex(index);
  };

  const closePreview = () => {
    setPreviewIndex(null);
  };

  // Lock body scroll when preview is open
  useEffect(() => {
    if (previewIndex !== null) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => {
      document.body.style.overflow = "";
    };
  }, [previewIndex]);

  // Listen for download progress events
  useEffect(() => {
    const unsubscribe = EventsOn("download-progress", (progress: DownloadProgress) => {
      setDownloadProgress(progress);
    });
    return () => {
      EventsOff("download-progress");
      unsubscribe();
    };
  }, []);

  // Listen for per-item download status events
  // Store current downloading item key for event listener (single download)
  const currentDownloadingItemKeyRef = useRef<string | null>(null);
  // Store mapping from backend index to itemKey for bulk download
  const bulkDownloadKeyMapRef = useRef<Map<number, string>>(new Map());

  useEffect(() => {
    const unsubscribe = EventsOn("download-item-status", (status: DownloadItemStatus) => {
      // For single download, use the current downloading item key directly
      const currentKey = currentDownloadingItemKeyRef.current;
      // For bulk download, use key mapping
      const keyMap = bulkDownloadKeyMapRef.current;
      
      let itemKey: string;
      if (currentKey) {
        // Single download - use the stored key
        itemKey = currentKey;
      } else if (keyMap.size > 0) {
        // Bulk download - map backend index to itemKey
        itemKey = keyMap.get(status.index) ?? `${String(status.tweet_id)}-${status.index}`;
      } else {
        // Fallback
        itemKey = `${String(status.tweet_id)}-${status.index}`;
      }
      
      if (status.status === "success") {
        setDownloadedItems((prev) => new Set(prev).add(itemKey));
        // Show toast for single download
        if (currentKey) {
          toast.success("Downloaded");
        }
      } else if (status.status === "failed") {
        setFailedItems((prev) => new Set(prev).add(itemKey));
        if (currentKey) {
          toast.error("Download failed");
        }
      } else if (status.status === "skipped") {
        // Remove from downloaded if it was added, then add to skipped
        setDownloadedItems((prev) => {
          const newSet = new Set(prev);
          newSet.delete(itemKey);
          return newSet;
        });
        setSkippedItems((prev) => new Set(prev).add(itemKey));
        // Show toast for single download
        if (currentKey) {
          toast.info("Already exists");
        }
      }
    });
    return () => {
      EventsOff("download-item-status");
      unsubscribe();
    };
  }, []);

  // Filter and sort timeline
  const filteredTimeline = useMemo(() => {
    let filtered = [...timeline];

    // Filter by media type
    if (filterType !== "all") {
      filtered = filtered.filter((item) => {
        if (filterType === "photo") return item.type === "photo";
        if (filterType === "video") return item.type === "video";
        if (filterType === "gif") return item.type === "gif" || item.type === "animated_gif";
        if (filterType === "text") return item.type === "text";
        return true;
      });
    }

    // Sort
    if (sortBy === "date-asc") {
      filtered.sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime());
    } else {
      filtered.sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
    }

    return filtered;
  }, [timeline, sortBy, filterType]);

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

  // Handle keyboard navigation for preview
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (previewIndex === null) return;
      if (e.key === "ArrowLeft") goToPrevious();
      if (e.key === "ArrowRight") goToNext();
      if (e.key === "Escape") closePreview();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [previewIndex, filteredTimeline.length]);

  // Count media types
  const mediaCounts = useMemo(() => {
    const counts = { photo: 0, video: 0, gif: 0, text: 0 };
    timeline.forEach((item) => {
      if (item.type === "photo") counts.photo++;
      else if (item.type === "video") counts.video++;
      else if (item.type === "gif" || item.type === "animated_gif") counts.gif++;
      else if (item.type === "text") counts.text++;
    });
    return counts;
  }, [timeline]);

  const toggleSelectAll = () => {
    if (selectedItems.size === filteredTimeline.length) {
      setSelectedItems(new Set());
    } else {
      setSelectedItems(new Set(filteredTimeline.map((_, i) => i)));
    }
  };

  const toggleItem = (index: number) => {
    const newSelected = new Set(selectedItems);
    if (newSelected.has(index)) {
      newSelected.delete(index);
    } else {
      newSelected.add(index);
    }
    setSelectedItems(newSelected);
  };

  const handleDownload = async () => {
    const settings = getSettings();
    
    // Get items with their original indices in filteredTimeline
    const itemsWithIndices = selectedItems.size > 0
      ? Array.from(selectedItems).map((i) => ({ item: filteredTimeline[i], originalIndex: i }))
      : filteredTimeline.map((item, i) => ({ item, originalIndex: i }));

    if (itemsWithIndices.length === 0) {
      toast.error("No media to download");
      return;
    }

    // Create mapping from backend index to itemKey for bulk download
    const keyMap = new Map<number, string>();
    itemsWithIndices.forEach((entry, backendIndex) => {
      keyMap.set(backendIndex, getItemKey(entry.item));
    });
    bulkDownloadKeyMapRef.current = keyMap;

    setIsDownloading(true);
    setDownloadProgress({ current: 0, total: itemsWithIndices.length, percent: 0 });
    logger.info(`Starting download of ${itemsWithIndices.length} files...`);

    try {
      const request = new main.DownloadMediaWithMetadataRequest({
        items: itemsWithIndices.map(({ item }) => new main.MediaItemRequest({
          url: item.url,
          date: item.date,
          tweet_id: item.tweet_id,
          type: item.type,
          content: item.content || "",
        })),
        output_dir: settings.downloadPath,
        username: accountInfo.name,
      });
      const response = await DownloadMediaWithMetadata(request);

      if (response.success) {
        logger.success(`Downloaded ${response.downloaded} files`);
        toast.success(`${response.downloaded} files downloaded`);
        setHasDownloaded(true);
        
        // Check if there are GIFs and FFmpeg is installed
        const hasGifItems = itemsWithIndices.some(({ item }) => item.type === "gif" || item.type === "animated_gif");
        if (hasGifItems) {
          setHasGifs(true);
          const installed = await IsFFmpegInstalled();
          setFfmpegInstalled(installed);
        }
      } else {
        logger.error(response.message);
        toast.error("Download failed");
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      logger.error(`Download failed: ${errorMsg}`);
      toast.error("Download failed");
    } finally {
      setIsDownloading(false);
      setDownloadProgress(null);
      bulkDownloadKeyMapRef.current = new Map();
    }
  };

  const handleStopDownload = async () => {
    try {
      const stopped = await StopDownload();
      if (stopped) {
        logger.info("Download stopped by user");
        toast.info("Stopped");
      }
    } catch (error) {
      console.error("Failed to stop download:", error);
    }
  };

  const handleOpenFolder = async () => {
    const settings = getSettings();
    // Build path - backend will handle path separators with filepath.Clean
    const folderPath = settings.downloadPath 
      ? `${settings.downloadPath}/${accountInfo.name}`
      : accountInfo.name;
    
    try {
      await OpenFolder(folderPath);
    } catch {
      try {
        await OpenFolder(settings.downloadPath);
      } catch {
        toast.error("Could not open folder");
      }
    }
  };

  const handleOpenTweet = (tweetId: string) => {
    openExternal(`https://x.com/${accountInfo.name}/status/${tweetId}`);
  };

  const handleConvertGifs = async () => {
    const settings = getSettings();
    // Use forward slash for cross-platform compatibility (Go's filepath.Join handles it)
    const folderPath = `${settings.downloadPath}/${accountInfo.name}`;

    setIsConverting(true);
    logger.info("Converting GIFs...");

    try {
      const response = await ConvertGIFs({
        folder_path: folderPath,
        quality: settings.gifQuality || "fast",
        resolution: settings.gifResolution || "high",
        delete_original: false, // Keep MP4 original
      });

      if (response.success) {
        logger.success(`Converted ${response.converted} GIFs`);
        toast.success(`${response.converted} GIFs converted`);
        setHasGifs(false);
      } else {
        logger.error(response.message);
        toast.error("Convert failed");
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      logger.error(`Convert failed: ${errorMsg}`);
      toast.error("Convert failed");
    } finally {
      setIsConverting(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Account Info Card */}
      {accountInfo.name === "bookmarks" ? (
        // Bookmarks mode - simple card without account info
        <div className="flex items-center gap-4 p-4 bg-muted/50 rounded-lg">
          <div className="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center">
            <Bookmark className="h-8 w-8 text-primary" />
          </div>
          <div className="flex-1">
            <h2 className="text-xl font-semibold">My Bookmarks</h2>
            <p className="text-sm text-muted-foreground mt-1">
              Your saved tweets from various accounts
            </p>
          </div>
          <div className="text-right">
            <div className="text-2xl font-bold text-primary">{formatNumberWithComma(totalUrls)}</div>
            <div className="text-sm text-muted-foreground">items found</div>
          </div>
        </div>
      ) : (
        // Normal account info card
        <div className="flex items-center gap-4 p-4 bg-muted/50 rounded-lg">
          <img
            src={accountInfo.profile_image}
            alt={accountInfo.nick}
            className="w-16 h-16 rounded-full"
          />
          <div className="flex-1">
            <div className="flex items-center gap-2">
              <h2 className="text-xl">{accountInfo.nick}</h2>
              <span className="text-muted-foreground">@{accountInfo.name}</span>
            </div>
            <div className="flex items-center gap-4 text-sm text-muted-foreground mt-1">
              <span className="flex items-center gap-1">
                <Users className="h-3.5 w-3.5" />
                {formatNumber(accountInfo.followers_count)} followers
              </span>
              <span className="flex items-center gap-1">
                <UserPlus className="h-3.5 w-3.5" />
                {formatNumber(accountInfo.friends_count)} following
              </span>
              <span className="flex items-center gap-1">
                <MessageSquare className="h-3.5 w-3.5" />
                {formatNumber(accountInfo.statuses_count)} tweets
              </span>
              {accountInfo.date && (
                <span className="flex items-center gap-1">
                  <Calendar className="h-3.5 w-3.5" />
                  Joined {formatJoinDate(accountInfo.date)}
                </span>
              )}
            </div>
          </div>
          <div className="text-right">
            <div className="text-2xl font-bold text-primary">{formatNumberWithComma(totalUrls)}</div>
            <div className="text-sm text-muted-foreground">items found</div>
          </div>
        </div>
      )}

      {/* Controls */}
      <div className="flex items-center gap-4">
        <Select value={sortBy} onValueChange={setSortBy}>
          <SelectTrigger className="w-auto">
            <SelectValue placeholder="Sort by" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="date-desc">Newest</SelectItem>
            <SelectItem value="date-asc">Oldest</SelectItem>
          </SelectContent>
        </Select>

        {/* Filter - only show when fetched "all" media types */}
        {fetchedMediaType === "all" && (
          <Select value={filterType} onValueChange={setFilterType}>
            <SelectTrigger className="w-auto">
              <SelectValue placeholder="Filter" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All ({formatNumberWithComma(totalUrls)})</SelectItem>
              <SelectItem value="photo">
                <span className="flex items-center gap-2">
                  <Image className="h-4 w-4 text-blue-500" />
                  Images ({formatNumberWithComma(mediaCounts.photo)})
                </span>
              </SelectItem>
              <SelectItem value="video">
                <span className="flex items-center gap-2">
                  <Video className="h-4 w-4 text-purple-500" />
                  Videos ({formatNumberWithComma(mediaCounts.video)})
                </span>
              </SelectItem>
              <SelectItem value="gif">
                <span className="flex items-center gap-2">
                  <Film className="h-4 w-4 text-green-500" />
                  GIFs ({formatNumberWithComma(mediaCounts.gif)})
                </span>
              </SelectItem>
            </SelectContent>
          </Select>
        )}

        {/* View Mode Toggle */}
        <div className="flex items-center border rounded-md">
          <Button
            variant={viewMode === "large" ? "secondary" : "ghost"}
            size="icon"
            className="h-9 w-9 rounded-r-none"
            onClick={() => setViewMode("large")}
          >
            <LayoutGrid className="h-4 w-4" />
          </Button>
          <Button
            variant={viewMode === "small" ? "secondary" : "ghost"}
            size="icon"
            className="h-9 w-9 rounded-none border-x"
            onClick={() => setViewMode("small")}
          >
            <Grid3X3 className="h-4 w-4" />
          </Button>
          <Button
            variant={viewMode === "list" ? "secondary" : "ghost"}
            size="icon"
            className="h-9 w-9 rounded-l-none"
            onClick={() => setViewMode("list")}
          >
            <List className="h-4 w-4" />
          </Button>
        </div>

        <div className="flex-1" />
        {hasDownloaded && (
          <Button variant="outline" onClick={handleOpenFolder}>
            <FolderOpen className="h-4 w-4" />
            Open Folder
          </Button>
        )}
        {hasGifs && ffmpegInstalled && (
          <Button variant="outline" onClick={handleConvertGifs} disabled={isConverting}>
            {isConverting ? (
              <>
                <Spinner />
                Converting...
              </>
            ) : (
              <>
                <Film className="h-4 w-4" />
                Convert GIFs
              </>
            )}
          </Button>
        )}
        <div className="flex items-center gap-2">
          {isDownloading && (
            <Button variant="destructive" onClick={handleStopDownload}>
              <StopCircle className="h-4 w-4" />
              Stop
            </Button>
          )}
          <Button onClick={handleDownload} disabled={isDownloading}>
          {isDownloading ? (
            <>
              <Spinner />
              Downloading...
            </>
          ) : (
            <>
              <Download className="h-4 w-4" />
              Download {selectedItems.size > 0 ? `${selectedItems.size}` : "All"}
            </>
          )}
          </Button>
        </div>
      </div>

      {/* Download Progress Bar */}
      {isDownloading && downloadProgress && (
        <div className="space-y-2">
          <div className="flex items-center justify-between text-sm">
            <span className="text-muted-foreground">
              Downloading {downloadProgress.current} of {downloadProgress.total}
            </span>
            <span className="font-medium">{downloadProgress.percent}%</span>
          </div>
          <Progress value={downloadProgress.percent} className="h-2" />
        </div>
      )}

      {/* Select All */}
      <div className="flex items-center gap-2">
        <Checkbox
          checked={selectedItems.size === filteredTimeline.length && filteredTimeline.length > 0}
          onCheckedChange={toggleSelectAll}
        />
        <span className="text-sm text-muted-foreground">
          Select all ({formatNumberWithComma(filteredTimeline.length)} items)
        </span>
        {selectedItems.size > 0 && (
          <Badge variant="secondary">{formatNumberWithComma(selectedItems.size)} selected</Badge>
        )}
      </div>

      {/* Media Grid/List */}
      {viewMode === "list" ? (
        <div className="space-y-2">
          {filteredTimeline.map((item, index) => {
            const isSelected = selectedItems.has(index);
            const itemKey = getItemKey(item);
            const isItemDownloaded = downloadedItems.has(itemKey);
            const isItemFailed = failedItems.has(itemKey);
            const isItemSkipped = skippedItems.has(itemKey);
            const isItemDownloading = downloadingItem === itemKey;
            return (
              <div
                key={itemKey}
                className={`flex items-center gap-4 p-3 rounded-lg border-2 transition-all ${
                  isSelected ? "border-primary bg-primary/5" : "border-transparent hover:bg-muted/50"
                }`}
              >
                <Checkbox
                  checked={isSelected}
                  onCheckedChange={() => toggleItem(index)}
                />
                <span className="text-sm text-muted-foreground w-8 text-center shrink-0">
                  {index + 1}
                </span>
                <div
                  className="w-16 h-16 rounded overflow-hidden bg-muted shrink-0 cursor-pointer hover:opacity-80 transition-opacity"
                  onClick={() => openPreview(index)}
                >
                  {item.type === "photo" ? (
                    <img
                      src={getThumbnailUrl(item.url)}
                      alt=""
                      className="w-full h-full object-cover"
                      loading="lazy"
                    />
                  ) : (
                    <div className="w-full h-full flex items-center justify-center">
                      {getMediaIcon(item.type)}
                    </div>
                  )}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <p className="text-sm font-medium">{item.tweet_id}</p>
                    <Badge 
                      variant="secondary" 
                      className={`text-xs ${
                        item.type === "photo" 
                          ? "bg-blue-500/20 text-blue-700 dark:text-blue-300" 
                          : item.type === "video" 
                          ? "bg-purple-500/20 text-purple-700 dark:text-purple-300"
                          : item.type === "text"
                          ? "bg-orange-500/20 text-orange-700 dark:text-orange-300"
                          : "bg-green-500/20 text-green-700 dark:text-green-300"
                      }`}
                    >
                      {getMediaIcon(item.type)}
                    </Badge>
                    {item.is_retweet && (
                      <Badge variant="outline" className="text-xs px-1.5">
                        <Repeat2 className="h-3 w-3" />
                      </Badge>
                    )}
                    {/* Download status icon - after badges */}
                    {isItemSkipped ? (
                      <FileCheck className="h-4 w-4 text-yellow-500 shrink-0" />
                    ) : isItemDownloaded ? (
                      <CheckCircle className="h-4 w-4 text-green-500 shrink-0" />
                    ) : isItemFailed ? (
                      <XCircle className="h-4 w-4 text-red-500 shrink-0" />
                    ) : null}
                  </div>
                  {item.type === "text" && item.content && (
                    <p className="text-sm mt-1 line-clamp-2">{item.content}</p>
                  )}
                  <p className="text-sm text-muted-foreground mt-1">
                    {formatDate(item.date)} {getRelativeTime(item.date)}
                  </p>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        size="icon"
                        variant="default"
                        onClick={async () => {
                          const settings = getSettings();
                          setDownloadingItem(itemKey);
                          currentDownloadingItemKeyRef.current = itemKey;
                          try {
                            const request = new main.DownloadMediaWithMetadataRequest({
                              items: [new main.MediaItemRequest({
                                url: item.url,
                                date: item.date,
                                tweet_id: item.tweet_id,
                                type: item.type,
                                content: item.content || "",
                              })],
                              output_dir: settings.downloadPath,
                              username: accountInfo.name,
                            });
                            const response = await DownloadMediaWithMetadata(request);
                            if (response.success) {
                              setHasDownloaded(true);
                              // Event listener will handle setting downloaded/skipped status
                            } else {
                              setFailedItems((prev) => new Set(prev).add(itemKey));
                              toast.error(response.message || "Download failed");
                            }
                          } catch (error) {
                            setFailedItems((prev) => new Set(prev).add(itemKey));
                            const errorMsg = error instanceof Error ? error.message : String(error);
                            toast.error(`Download failed: ${errorMsg}`);
                          } finally {
                            setDownloadingItem(null);
                            setTimeout(() => { currentDownloadingItemKeyRef.current = null; }, 1000);
                          }
                        }}
                        disabled={downloadingItem !== null}
                      >
                        {isItemDownloading ? (
                          <Spinner />
                        ) : isItemSkipped ? (
                          <FileCheck className="h-4 w-4" />
                        ) : isItemDownloaded ? (
                          <CheckCircle className="h-4 w-4" />
                        ) : isItemFailed ? (
                          <XCircle className="h-4 w-4" />
                        ) : (
                          <Download className="h-4 w-4" />
                        )}
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {isItemDownloading ? (
                        <p>Downloading...</p>
                      ) : isItemSkipped ? (
                        <p>Already exists</p>
                      ) : isItemDownloaded ? (
                        <p>Downloaded</p>
                      ) : isItemFailed ? (
                        <p>Failed</p>
                      ) : (
                        <p>Download</p>
                      )}
                    </TooltipContent>
                  </Tooltip>
                  <Button
                    size="icon"
                    variant="outline"
                    onClick={() => handleOpenTweet(item.tweet_id)}
                  >
                    <ExternalLink className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <div className={`grid gap-3 ${viewMode === "large" ? "grid-cols-4" : "grid-cols-6"}`}>
          {filteredTimeline.map((item, index) => {
            const isSelected = selectedItems.has(index);
            const itemKey = getItemKey(item);
            const isItemDownloaded = downloadedItems.has(itemKey);
            const isItemFailed = failedItems.has(itemKey);
            const isItemSkipped = skippedItems.has(itemKey);
            const isItemDownloading = downloadingItem === itemKey;

            return (
              <div
                key={itemKey}
                className={`relative group rounded-lg overflow-hidden border-2 transition-all ${
                  isSelected ? "border-primary" : "border-transparent hover:border-muted-foreground/30"
                }`}
              >
                {/* Thumbnail */}
                <div
                  className="aspect-square bg-muted relative cursor-pointer"
                  onClick={() => openPreview(index)}
                >
                  {item.type === "photo" ? (
                    <img
                      src={getThumbnailUrl(item.url)}
                      alt=""
                      className="w-full h-full object-cover"
                      loading="lazy"
                    />
                  ) : (
                    <div className="w-full h-full flex items-center justify-center bg-muted">
                      {getMediaIcon(item.type)}
                    </div>
                  )}

                  {/* Overlay */}
                  <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-2">
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          size="icon"
                          variant="default"
                          className="h-8 w-8"
                          onClick={async (e) => {
                            e.stopPropagation();
                            const settings = getSettings();
                            setDownloadingItem(itemKey);
                            currentDownloadingItemKeyRef.current = itemKey;
                            try {
                              const request = new main.DownloadMediaWithMetadataRequest({
                                items: [new main.MediaItemRequest({
                                  url: item.url,
                                  date: item.date,
                                  tweet_id: item.tweet_id,
                                  type: item.type,
                                  content: item.content || "",
                                })],
                                output_dir: settings.downloadPath,
                                username: accountInfo.name,
                              });
                              const response = await DownloadMediaWithMetadata(request);
                              if (response.success) {
                                setHasDownloaded(true);
                                // Event listener will handle setting downloaded/skipped status
                              } else {
                                setFailedItems((prev) => new Set(prev).add(itemKey));
                                toast.error(response.message || "Download failed");
                              }
                            } catch (error) {
                              setFailedItems((prev) => new Set(prev).add(itemKey));
                              const errorMsg = error instanceof Error ? error.message : String(error);
                              toast.error(`Download failed: ${errorMsg}`);
                            } finally {
                              setDownloadingItem(null);
                              setTimeout(() => { currentDownloadingItemKeyRef.current = null; }, 1000);
                            }
                          }}
                          disabled={downloadingItem !== null}
                        >
                          {isItemDownloading ? (
                            <Spinner />
                          ) : isItemSkipped ? (
                            <FileCheck className="h-4 w-4" />
                          ) : isItemDownloaded ? (
                            <CheckCircle className="h-4 w-4" />
                          ) : isItemFailed ? (
                            <XCircle className="h-4 w-4" />
                          ) : (
                            <Download className="h-4 w-4" />
                          )}
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>
                        {isItemDownloading ? (
                          <p>Downloading...</p>
                        ) : isItemSkipped ? (
                          <p>Already exists</p>
                        ) : isItemDownloaded ? (
                          <p>Downloaded</p>
                        ) : isItemFailed ? (
                          <p>Failed</p>
                        ) : (
                          <p>Download</p>
                        )}
                      </TooltipContent>
                    </Tooltip>
                    <Button
                      size="icon"
                      variant="outline"
                      className="h-8 w-8"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleOpenTweet(item.tweet_id);
                      }}
                    >
                      <ExternalLink className="h-4 w-4" />
                    </Button>
                  </div>

                  {/* Checkbox */}
                  <div className="absolute top-2 left-2" onClick={(e) => e.stopPropagation()}>
                    <Checkbox
                      checked={isSelected}
                      onCheckedChange={() => toggleItem(index)}
                      className="bg-background/80"
                    />
                  </div>

                  {/* Type Badge */}
                  <div className="absolute top-2 right-2">
                    <Badge 
                      variant="secondary" 
                      className={`text-xs px-1.5 py-0.5 ${
                        item.type === "photo" 
                          ? "bg-blue-500/20 text-blue-700 dark:text-blue-300" 
                          : item.type === "video" 
                          ? "bg-purple-500/20 text-purple-700 dark:text-purple-300"
                          : item.type === "text"
                          ? "bg-orange-500/20 text-orange-700 dark:text-orange-300"
                          : "bg-green-500/20 text-green-700 dark:text-green-300"
                      }`}
                    >
                      {getMediaIcon(item.type)}
                    </Badge>
                  </div>

                  {/* Retweet indicator */}
                  {item.is_retweet && (
                    <div className="absolute bottom-2 right-2">
                      <Badge variant="outline" className="text-xs px-1.5 py-0.5 bg-background/80">
                        <Repeat2 className="h-3 w-3" />
                      </Badge>
                    </div>
                  )}

                  {/* Number badge - bottom left inside thumbnail */}
                  <div className="absolute bottom-2 left-2">
                    <span className="text-xs px-1.5 py-0.5 bg-black/60 text-white rounded">
                      {index + 1}
                    </span>
                  </div>
                </div>

                {/* Info */}
                <div className="p-2 text-xs text-muted-foreground">
                  <div className="truncate">{formatDate(item.date)}</div>
                  <div className="text-[10px] mt-0.5">{getRelativeTime(item.date)}</div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Media Preview Overlay */}
      {previewIndex !== null && filteredTimeline[previewIndex] && (
        <div 
          className="fixed inset-0 z-40 bg-black/80 flex flex-col items-center justify-center pt-8"
          onClick={(e) => {
            if (e.target === e.currentTarget) closePreview();
          }}
        >
          {/* Previous button - left side */}
          {previewIndex > 0 && (
            <Button
              variant="ghost"
              size="icon"
              className="absolute left-4 top-1/2 -translate-y-1/2 text-white hover:bg-white/20 h-12 w-12 z-10"
              onClick={goToPrevious}
            >
              <ChevronLeft className="h-8 w-8" />
            </Button>
          )}

          {/* Counter - above media */}
          <div className="text-white text-sm bg-black/50 px-4 py-1.5 rounded-full mb-4">
            {previewIndex + 1} / {filteredTimeline.length}
          </div>

          {/* Media content - center */}
          <div className="max-w-[90%] max-h-[70%] flex items-center justify-center">
            {filteredTimeline[previewIndex].type === "photo" ? (
              <img
                src={getPreviewUrl(filteredTimeline[previewIndex].url)}
                alt=""
                className="max-w-full max-h-[65vh] object-contain rounded-lg"
              />
            ) : filteredTimeline[previewIndex].type === "video" ? (
              <video
                src={filteredTimeline[previewIndex].url}
                controls
                autoPlay
                className="max-w-full max-h-[65vh] rounded-lg"
              />
            ) : filteredTimeline[previewIndex].type === "text" ? (
              <div className="bg-white dark:bg-gray-800 p-6 rounded-lg max-w-2xl">
                <p className="text-lg whitespace-pre-wrap">{filteredTimeline[previewIndex].content || "No content"}</p>
                <p className="text-sm text-muted-foreground mt-4">
                  {formatDate(filteredTimeline[previewIndex].date)} {getRelativeTime(filteredTimeline[previewIndex].date)}
                </p>
              </div>
            ) : (
              <video
                src={filteredTimeline[previewIndex].url}
                autoPlay
                loop
                muted
                className="max-w-full max-h-[65vh] rounded-lg"
              />
            )}
          </div>

          {/* Tweet Stats */}
          <div className="flex items-center gap-4 mt-3 text-white/80 text-sm">
            {filteredTimeline[previewIndex].verified && (
              <span className="flex items-center gap-1 text-blue-400">
                <BadgeCheck className="h-4 w-4" />
                Verified
              </span>
            )}
            {filteredTimeline[previewIndex].width > 0 && filteredTimeline[previewIndex].height > 0 && (
              <span className="flex items-center gap-1">
                <Maximize2 className="h-4 w-4" />
                {filteredTimeline[previewIndex].width} × {filteredTimeline[previewIndex].height}
              </span>
            )}
            {filteredTimeline[previewIndex].view_count !== undefined && filteredTimeline[previewIndex].view_count > 0 && (
              <span className="flex items-center gap-1">
                <Eye className="h-4 w-4" />
                {formatNumber(filteredTimeline[previewIndex].view_count)}
              </span>
            )}
            {filteredTimeline[previewIndex].favorite_count !== undefined && filteredTimeline[previewIndex].favorite_count > 0 && (
              <span className="flex items-center gap-1">
                <Heart className="h-4 w-4" />
                {formatNumber(filteredTimeline[previewIndex].favorite_count)}
              </span>
            )}
            {filteredTimeline[previewIndex].retweet_count !== undefined && filteredTimeline[previewIndex].retweet_count > 0 && (
              <span className="flex items-center gap-1">
                <Repeat2 className="h-4 w-4" />
                {formatNumber(filteredTimeline[previewIndex].retweet_count)}
              </span>
            )}
            {filteredTimeline[previewIndex].bookmark_count !== undefined && filteredTimeline[previewIndex].bookmark_count > 0 && (
              <span className="flex items-center gap-1">
                <Bookmark className="h-4 w-4" />
                {formatNumber(filteredTimeline[previewIndex].bookmark_count)}
              </span>
            )}
            {filteredTimeline[previewIndex].source && (
              <span className="text-white/60">
                via {filteredTimeline[previewIndex].source}
              </span>
            )}
          </div>

          {/* Action buttons - bottom center */}
          <div className="flex items-center gap-3 mt-4 z-10">
            {(() => {
              const item = filteredTimeline[previewIndex];
              const itemKey = getItemKey(item);
              const isItemDownloaded = downloadedItems.has(itemKey);
              const isItemFailed = failedItems.has(itemKey);
              const isItemSkipped = skippedItems.has(itemKey);
              const isItemDownloading = downloadingItem === itemKey;
              return (
                <Button
                  variant="default"
                  size="sm"
                  className="h-9"
                  onClick={async () => {
                    const settings = getSettings();
                    setDownloadingItem(itemKey);
                    currentDownloadingItemKeyRef.current = itemKey;
                    try {
                      const request = new main.DownloadMediaWithMetadataRequest({
                        items: [new main.MediaItemRequest({
                          url: item.url,
                          date: item.date,
                          tweet_id: item.tweet_id,
                          type: item.type,
                          content: item.content || "",
                        })],
                        output_dir: settings.downloadPath,
                        username: accountInfo.name,
                      });
                      const response = await DownloadMediaWithMetadata(request);
                      if (response.success) {
                        setHasDownloaded(true);
                        // Event listener will handle setting downloaded/skipped status
                      } else {
                        setFailedItems((prev) => new Set(prev).add(itemKey));
                        toast.error("Download failed");
                      }
                    } catch {
                      setFailedItems((prev) => new Set(prev).add(itemKey));
                      toast.error("Download failed");
                    } finally {
                      setDownloadingItem(null);
                      setTimeout(() => { currentDownloadingItemKeyRef.current = null; }, 1000);
                    }
                  }}
                  disabled={downloadingItem !== null}
                >
                  {isItemDownloading ? (
                    <Spinner className="mr-1" />
                  ) : isItemSkipped ? (
                    <FileCheck className="h-4 w-4 mr-1" />
                  ) : isItemDownloaded ? (
                    <CheckCircle className="h-4 w-4 mr-1" />
                  ) : isItemFailed ? (
                    <XCircle className="h-4 w-4 mr-1" />
                  ) : (
                    <Download className="h-4 w-4 mr-1" />
                  )}
                  {isItemDownloading ? "Downloading..." : isItemSkipped ? "Already exists" : isItemDownloaded ? "Downloaded" : isItemFailed ? "Failed" : "Download"}
                </Button>
              );
            })()}
            <Button
              variant="secondary"
              size="sm"
              className="h-9"
              onClick={() => handleOpenTweet(filteredTimeline[previewIndex].tweet_id)}
            >
              <ExternalLink className="h-4 w-4 mr-1" />
              Open Tweet
            </Button>
          </div>

          {/* Next button - right side */}
          {previewIndex < filteredTimeline.length - 1 && (
            <Button
              variant="ghost"
              size="icon"
              className="absolute right-4 top-1/2 -translate-y-1/2 text-white hover:bg-white/20 h-12 w-12 z-10"
              onClick={goToNext}
            >
              <ChevronRight className="h-8 w-8" />
            </Button>
          )}
        </div>
      )}

      {/* Scroll to Top Button - hide when preview is open */}
      {showScrollTop && previewIndex === null && (
        <Button
          variant="default"
          size="icon"
          className="fixed bottom-6 left-1/2 -translate-x-1/2 h-9 w-9 rounded-full shadow-lg z-30"
          onClick={scrollToTop}
        >
          <ArrowUp className="h-4 w-4" />
        </Button>
      )}
    </div>
  );
}
