import { useState, useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
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
import { Checkbox } from "@/components/ui/checkbox";
import { Spinner } from "@/components/ui/spinner";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Trash2, FileInput, FileOutput, Pencil, Tag, Shuffle, X, XCircle, Download, StopCircle, Globe, Lock, Bookmark, Heart, Image, Images, Video, Film, FileText, Filter, AlertCircle, MoreVertical, FileBraces, CloudBackup, Search, LayoutGrid, Grid3X3, List, ArrowUpDown, ArrowUp, FolderOpen } from "lucide-react";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { getSettings } from "@/lib/settings";
import { openExternal } from "@/lib/utils";
import {
  GetAllAccountsFromDB,
  GetAccountFromDB,
  DeleteAccountFromDB,
  SaveAccountToDB,
  ExportAccountJSON,
  ExportAccountsTXT,
  UpdateAccountGroup,
  GetAllGroups,
  DownloadMediaWithMetadata,
  StopDownload,
  CheckFolderExists,
  OpenFolder,
  GetFolderPath,
} from "../../wailsjs/go/main/App";
import { EventsOn, EventsOff } from "../../wailsjs/runtime/runtime";
import { main } from "../../wailsjs/go/models";

interface DownloadProgress {
  current: number;
  total: number;
  percent: number;
}



function formatNumberWithComma(num: number): string {
  return num.toLocaleString();
}

function getRelativeTime(dateStr: string): string {
  try {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    
    const diffMinutes = Math.floor(diffMs / (1000 * 60));
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    
    if (diffDays > 0) {
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

// Using backend.AccountListItem from wailsjs/go/models
import { backend } from "../../wailsjs/go/models";
type AccountListItem = backend.AccountListItem;

interface GroupInfo {
  name: string;
  color: string;
}

interface DatabaseViewProps {
  onBack: () => void;
  onLoadAccount: (responseJSON: string, username: string) => void;
  onUpdateSelected?: (usernames: string[]) => void;
}

const INITIAL_LOAD_COUNT = 12;
const LOAD_MORE_COUNT = 12;

export function DatabaseView({ onLoadAccount, onUpdateSelected }: DatabaseViewProps) {
  const [accounts, setAccounts] = useState<AccountListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [visibleCount, setVisibleCount] = useState(INITIAL_LOAD_COUNT);
  const [showScrollTop, setShowScrollTop] = useState(false);
  const [groups, setGroups] = useState<GroupInfo[]>([]);
  const [filterGroup, setFilterGroup] = useState<string>("all");
  const [filterMediaType, setFilterMediaType] = useState<string>("all");
  const [accountViewMode, setAccountViewMode] = useState<"public" | "private">("public");
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [sortOrder, setSortOrder] = useState<"newest" | "oldest">("newest");
  const [gridView, setGridView] = useState<"large" | "small" | "list">("list");
  const [editingAccount, setEditingAccount] = useState<AccountListItem | null>(null);
  const [editGroupName, setEditGroupName] = useState("");
  const [editGroupColor, setEditGroupColor] = useState("");
  const [clearAllDialogOpen, setClearAllDialogOpen] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadingAccountId, setDownloadingAccountId] = useState<number | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);
  const [isBulkDownloading, setIsBulkDownloading] = useState(false);
  const [bulkDownloadCurrent, setBulkDownloadCurrent] = useState(0);
  const [bulkDownloadTotal, setBulkDownloadTotal] = useState(0);
  const stopBulkDownloadRef = useRef(false);
  const loadMoreRef = useRef<HTMLDivElement>(null);
  const [folderExistence, setFolderExistence] = useState<Map<number, boolean>>(new Map());

  // Check folder existence for all accounts
  const checkFolderExistence = async (accountsList: AccountListItem[]) => {
    const settings = getSettings();
    const basePath = settings.downloadPath;
    if (!basePath) return;

    const folderMap = new Map<number, boolean>();

    for (const account of accountsList) {
      // For bookmarks/likes, check special folder names
      let folderName = account.username;
      if (account.username === "bookmarks") {
        folderName = "My Bookmarks";
      } else if (account.username === "likes") {
        folderName = "My Likes";
      }

      const exists = await CheckFolderExists(basePath, folderName);
      folderMap.set(account.id, exists);
    }

    setFolderExistence(folderMap);
  };

  const loadAccounts = async () => {
    setLoading(true);
    try {
      const data = await GetAllAccountsFromDB();
      setAccounts(data || []);
      const groupsData = await GetAllGroups();
      if (groupsData) {
        setGroups(groupsData.map((g) => ({ name: g.name || "", color: g.color || "" })));
      }
      // Check folder existence for all accounts
      if (data && data.length > 0) {
        checkFolderExistence(data);
      }
    } catch (error) {
      console.error("Failed to load accounts:", error);
      toast.error("Failed to load accounts");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadAccounts();
  }, []);

  // Reset visible count when grid view or filters change
  useEffect(() => {
    setVisibleCount(INITIAL_LOAD_COUNT);
  }, [gridView, searchQuery, filterGroup, filterMediaType, accountViewMode, sortOrder]);

  // Listen for scroll to show/hide scroll-to-top button
  // Listen for scroll to show/hide scroll-to-top button
  useEffect(() => {
    const handleScroll = () => {
      setShowScrollTop(window.scrollY > 300);
    };
    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  // Intersection Observer for lazy loading
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

  const scrollToTop = () => {
    window.scrollTo({ top: 0, behavior: "smooth" });
  };

  // Separate accounts into public and private
  const isPrivateAccount = (username: string) => 
    username === "bookmarks" || username === "likes";

  const publicAccounts = accounts.filter((acc) => !isPrivateAccount(acc.username));
  const privateAccounts = accounts.filter((acc) => isPrivateAccount(acc.username));

  const baseAccounts = accountViewMode === "public" ? publicAccounts : privateAccounts;

  const filteredAccounts = baseAccounts
    .filter((acc) => {
      // Filter by search query
      if (searchQuery) {
        const query = searchQuery.toLowerCase();
        const matchesUsername = acc.username.toLowerCase().includes(query);
        const matchesName = acc.name.toLowerCase().includes(query);
        if (!matchesUsername && !matchesName) return false;
      }
      // Filter by group
      if (filterGroup !== "all") {
        if (filterGroup === "ungrouped" && acc.group_name) return false;
        if (filterGroup !== "ungrouped" && acc.group_name !== filterGroup) return false;
      }
      // Filter by media type
      if (filterMediaType !== "all") {
        const accMediaType = acc.media_type || "all";
        // "all-media" in filter matches "all" in database (fetched with All Media option)
        if (filterMediaType === "all-media") {
          if (accMediaType !== "all") return false;
        } else {
          if (accMediaType !== filterMediaType) return false;
        }
      }
      return true;
    })
    .sort((a, b) => {
      // Sort by last_fetched date
      const dateA = new Date(a.last_fetched).getTime();
      const dateB = new Date(b.last_fetched).getTime();
      return sortOrder === "newest" ? dateB - dateA : dateA - dateB;
    });

  // Intersection Observer for lazy loading
  useEffect(() => {
    const currentRef = loadMoreRef.current;
    if (!currentRef) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          setVisibleCount((prev) => prev + LOAD_MORE_COUNT);
        }
      },
      { threshold: 0.1, rootMargin: "100px" }
    );

    observer.observe(currentRef);

    return () => {
      observer.unobserve(currentRef);
    };
  }, [filteredAccounts.length, visibleCount, loading]);

  const handleEditGroup = (account: AccountListItem) => {
    setEditingAccount(account);
    setEditGroupName(account.group_name || "");
    setEditGroupColor(account.group_color || "#3b82f6");
  };

  const handleSaveGroup = async () => {
    if (!editingAccount) return;
    try {
      await UpdateAccountGroup(editingAccount.id, editGroupName, editGroupColor);
      toast.success(`Updated group for @${editingAccount.username}`);
      setEditingAccount(null);
      loadAccounts();
    } catch (error) {
      toast.error("Failed to update group");
    }
  };

  const handleDelete = async (id: number, username: string) => {
    try {
      await DeleteAccountFromDB(id);
      toast.success(`Deleted @${username}`);
      loadAccounts();
    } catch (error) {
      toast.error("Failed to delete account");
    }
  };

  const handleView = async (id: number, username: string) => {
    try {
      const responseJSON = await GetAccountFromDB(id);
      onLoadAccount(responseJSON, username);
    } catch (error) {
      toast.error("Failed to load account data");
    }
  };

  const handleOpenFolder = async (username: string) => {
    const settings = getSettings();
    let folderName = username;
    if (username === "bookmarks") {
      folderName = "My Bookmarks";
    } else if (username === "likes") {
      folderName = "My Likes";
    }
    const folderPath = await GetFolderPath(settings.downloadPath, folderName);
    try {
      await OpenFolder(folderPath);
    } catch (error) {
      toast.error("Failed to open folder");
    }
  };

  const handleDownload = async (id: number, username: string) => {
    try {
      const responseJSON = await GetAccountFromDB(id);
      const data = JSON.parse(responseJSON);
      const timeline = data.timeline || [];
      
      if (timeline.length === 0) {
        toast.error("No media to download");
        return;
      }

      const settings = getSettings();
      // Check if it's bookmarks or likes by checking account_info.nick or username (for backward compatibility)
      const isBookmarks = data.account_info?.nick === "My Bookmarks" || username === "bookmarks";
      const isLikes = data.account_info?.nick === "My Likes" || username === "likes";
      let outputDir = settings.downloadPath;
      if (isBookmarks) {
        const separator = settings.downloadPath.includes("/") ? "/" : "\\";
        outputDir = `${settings.downloadPath}${separator}My Bookmarks`;
      } else if (isLikes) {
        const separator = settings.downloadPath.includes("/") ? "/" : "\\";
        outputDir = `${settings.downloadPath}${separator}My Likes`;
      }
      // Use actual username from account_info if available, otherwise use stored username
      const actualUsername = data.account_info?.name || username;

      setIsDownloading(true);
      setDownloadingAccountId(id);
      setDownloadProgress({ current: 0, total: timeline.length, percent: 0 });

      const request = new main.DownloadMediaWithMetadataRequest({
        items: timeline.map((item: { url: string; date: string; tweet_id: string; type: string; original_filename?: string; author_username?: string }) => new main.MediaItemRequest({
          url: item.url,
          date: item.date,
          tweet_id: item.tweet_id,
          type: item.type,
          original_filename: item.original_filename || "",
          author_username: item.author_username || "",
        })),
        output_dir: outputDir,
        username: actualUsername,
        proxy: settings.proxy || "",
      });

      const response = await DownloadMediaWithMetadata(request);

      if (response.success) {
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
        const message = parts.length > 0 ? `${parts.join(', ')} for @${username}` : `Download completed for @${username}`;
        
        // Use info toast if only skipped files (no downloaded, no failed)
        if (response.downloaded === 0 && response.failed === 0 && response.skipped > 0) {
          toast.info(message);
        } else {
          toast.success(message);
        }
      } else {
        toast.error(response.message || "Download failed");
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      toast.error(`Download failed: ${errorMsg}`);
    } finally {
      setIsDownloading(false);
      setDownloadingAccountId(null);
      setDownloadProgress(null);
    }
  };

  const handleStopDownload = async () => {
    try {
      const stopped = await StopDownload();
      if (stopped) {
        toast.info("Download stopped");
      }
    } catch (error) {
      console.error("Failed to stop download:", error);
    }
  };

  const handleBulkDownload = async () => {
    const idsToDownload = Array.from(selectedIds);
    if (idsToDownload.length === 0) {
      toast.error("No accounts selected");
      return;
    }

    setIsBulkDownloading(true);
    setBulkDownloadTotal(idsToDownload.length);
    setBulkDownloadCurrent(0);
    stopBulkDownloadRef.current = false;

    const settings = getSettings();
    let totalDownloaded = 0;
    let totalSkipped = 0;
    let downloadedAccounts = 0;

    for (let i = 0; i < idsToDownload.length; i++) {
      // Check stop flag using ref (works inside async loop)
      if (stopBulkDownloadRef.current) {
        toast.info("Bulk download stopped");
        break;
      }

      const id = idsToDownload[i];
      const account = accounts.find((a) => a.id === id);
      if (!account) continue;

      setBulkDownloadCurrent(i + 1);
      setDownloadingAccountId(id);

      try {
        const responseJSON = await GetAccountFromDB(id);
        const data = JSON.parse(responseJSON);
        const timeline = data.timeline || [];

        if (timeline.length === 0) continue;

        // Check again before starting download
        if (stopBulkDownloadRef.current) {
          toast.info("Bulk download stopped");
          break;
        }

        setDownloadProgress({ current: 0, total: timeline.length, percent: 0 });

        // Check if it's bookmarks or likes by checking account_info.nick or username (for backward compatibility)
        const isBookmarks = data.account_info?.nick === "My Bookmarks" || account.username === "bookmarks";
        const isLikes = data.account_info?.nick === "My Likes" || account.username === "likes";
        let outputDir = settings.downloadPath;
        if (isBookmarks) {
          const separator = settings.downloadPath.includes("/") ? "/" : "\\";
          outputDir = `${settings.downloadPath}${separator}My Bookmarks`;
        } else if (isLikes) {
          const separator = settings.downloadPath.includes("/") ? "/" : "\\";
          outputDir = `${settings.downloadPath}${separator}My Likes`;
        }
        // Use actual username from account_info if available, otherwise use stored username
        const actualUsername = data.account_info?.name || account.username;

        const request = new main.DownloadMediaWithMetadataRequest({
          items: timeline.map((item: { url: string; date: string; tweet_id: string; type: string; author_username?: string; original_filename?: string }) => new main.MediaItemRequest({
            url: item.url,
            date: item.date,
            tweet_id: item.tweet_id,
            type: item.type,
            author_username: item.author_username || "",
            original_filename: item.original_filename || "",
          })),
          output_dir: outputDir,
          username: actualUsername,
          proxy: settings.proxy || "",
        });

        const response = await DownloadMediaWithMetadata(request);
        if (response.success) {
          totalDownloaded += response.downloaded;
          totalSkipped += response.skipped || 0;
          downloadedAccounts++;
        }
      } catch (error) {
        console.error(`Failed to download @${account.username}:`, error);
      }
    }

    setIsBulkDownloading(false);
    setDownloadingAccountId(null);
    setDownloadProgress(null);
    setBulkDownloadCurrent(0);
    setBulkDownloadTotal(0);

    if ((totalDownloaded > 0 || totalSkipped > 0) && !stopBulkDownloadRef.current) {
      const parts: string[] = [];
      if (totalDownloaded > 0) {
        parts.push(`${totalDownloaded} file${totalDownloaded !== 1 ? 's' : ''} downloaded`);
      }
      if (totalSkipped > 0) {
        parts.push(`${totalSkipped} file${totalSkipped !== 1 ? 's' : ''} already exist${totalSkipped !== 1 ? '' : 's'}`);
      }
      const message = parts.length > 0 ? `${parts.join(', ')} from ${downloadedAccounts} account${downloadedAccounts !== 1 ? 's' : ''}` : `Download completed from ${downloadedAccounts} account${downloadedAccounts !== 1 ? 's' : ''}`;
      
      // Use info toast if only skipped files (no downloaded)
      if (totalDownloaded === 0 && totalSkipped > 0) {
        toast.info(message);
      } else {
        toast.success(message);
      }
    }
  };

  const handleStopBulkDownload = async () => {
    stopBulkDownloadRef.current = true;
    await handleStopDownload();
  };

  const toggleSelect = (id: number) => {
    setSelectedIds((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(id)) {
        newSet.delete(id);
      } else {
        newSet.add(id);
      }
      return newSet;
    });
  };

  const toggleSelectAll = () => {
    if (selectedIds.size === filteredAccounts.length && filteredAccounts.length > 0) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(filteredAccounts.map((a) => a.id)));
    }
  };

  const handleExportJSON = async () => {
    const idsToExport = selectedIds.size > 0 ? Array.from(selectedIds) : accounts.map((a) => a.id);

    if (idsToExport.length === 0) {
      toast.error("No accounts to export");
      return;
    }

    const settings = getSettings();
    const outputDir = settings.downloadPath || "";

    try {
      let exported = 0;
      for (const id of idsToExport) {
        await ExportAccountJSON(id, outputDir);
        exported++;
      }
      toast.success(`Exported ${exported} account(s) to ${outputDir}\\twitterxmediabatchdownloader_backups`);
    } catch (error) {
      toast.error("Failed to export");
    }
  };

  const handleExportTXT = async () => {
    const idsToExport = selectedIds.size > 0 ? Array.from(selectedIds) : accounts.map((a) => a.id);

    if (idsToExport.length === 0) {
      toast.error("No accounts to export");
      return;
    }

    const settings = getSettings();
    const outputDir = settings.downloadPath || "";

    try {
      await ExportAccountsTXT(idsToExport, outputDir);
      toast.success(`Exported ${formatNumberWithComma(idsToExport.length)} account(s) to ${outputDir}\\twitterxmediabatchdownloader_backups\\twitterxmediabatchdownloader_multiple.txt`);
    } catch (error) {
      toast.error("Failed to export");
    }
  };

  // Check if any selected accounts are private (bookmarks/likes)
  const hasPrivateAccountSelected = () => {
    if (selectedIds.size === 0) return false;
    return Array.from(selectedIds).some((id) => {
      const account = accounts.find((a) => a.id === id);
      return account && isPrivateAccount(account.username);
    });
  };

  const handleImport = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.multiple = true;
    input.onchange = async (e) => {
      const files = (e.target as HTMLInputElement).files;
      if (!files || files.length === 0) return;

      let imported = 0;
      for (const file of Array.from(files)) {
        try {
          const text = await file.text();
          const data = JSON.parse(text);
          
          // Detect media_type from imported file
          let detectedMediaType = "all";
          
          // Check if media_type is explicitly set in the file
          if (data.media_type) {
            detectedMediaType = data.media_type;
          }
          // If not, try to detect from media_list content
          else if (data.media_list && Array.isArray(data.media_list)) {
            const types = new Set(data.media_list.map((item: { media_type?: string; type?: string }) => 
              item.media_type || item.type
            ).filter(Boolean));
            
            // If all items are same type, use that type
            if (types.size === 1) {
              const singleType = Array.from(types)[0] as string;
              if (singleType === "photo") detectedMediaType = "image";
              else if (singleType === "video") detectedMediaType = "video";
              else if (singleType === "animated_gif") detectedMediaType = "gif";
              else detectedMediaType = singleType;
            }
          }
          // Or detect from timeline content
          else if (data.timeline && Array.isArray(data.timeline)) {
            const types = new Set(data.timeline.map((item: { type?: string }) => item.type).filter(Boolean));
            
            // If all items are same type, use that type
            if (types.size === 1) {
              const singleType = Array.from(types)[0] as string;
              if (singleType === "photo") detectedMediaType = "image";
              else if (singleType === "video") detectedMediaType = "video";
              else if (singleType === "animated_gif") detectedMediaType = "gif";
              else if (singleType === "text") detectedMediaType = "text";
              else detectedMediaType = singleType;
            }
          }
          
          // Support new format (account_info + timeline)
          if (data.account_info && data.timeline) {
            await SaveAccountToDB(
              data.account_info.name,  // username/handle
              data.account_info.nick,  // display name
              data.account_info.profile_image,
              data.total_urls || data.timeline.length,
              text,
              detectedMediaType
            );
            imported++;
          }
          // Support legacy format (username + media_list)
          else if (data.username && data.media_list) {
            // Convert legacy format to new format
            const convertedData = {
              account_info: {
                name: data.username,
                nick: data.nick || data.username,
                date: "",
                followers_count: data.followers || 0,
                friends_count: data.following || 0,
                profile_image: data.profile_image || "",
                statuses_count: data.posts || 0,
              },
              total_urls: data.media_list.length,
              timeline: data.media_list.map((item: { url: string; date: string; tweet_id: string; type: string; media_type?: string }) => ({
                url: item.url,
                date: item.date,
                tweet_id: item.tweet_id,
                type: item.media_type || item.type,
                is_retweet: false,
              })),
              metadata: {
                new_entries: data.media_list.length,
                page: 0,
                batch_size: 0,
                has_more: false,
              },
            };
            
            await SaveAccountToDB(
              convertedData.account_info.name,
              convertedData.account_info.nick,
              convertedData.account_info.profile_image,
              convertedData.total_urls,
              JSON.stringify(convertedData),
              detectedMediaType
            );
            imported++;
          }
        } catch (err) {
          console.error(`Failed to import ${file.name}:`, err);
        }
      }
      
      if (imported > 0) {
        toast.success(`Imported ${imported} account(s)`);
        loadAccounts();
      } else {
        toast.error("No valid files imported");
      }
    };
    input.click();
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-2xl font-bold">Saved Accounts</h2>
          {/* Public/Private Toggle */}
          <div className="flex gap-0.5 p-0.5 bg-muted rounded-lg">
            <button
              type="button"
              onClick={() => setAccountViewMode("public")}
              className={`flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-all ${
                accountViewMode === "public"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <Globe className="h-3 w-3" />
              Public ({formatNumberWithComma(publicAccounts.length)})
            </button>
            <button
              type="button"
              onClick={() => setAccountViewMode("private")}
              className={`flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-all ${
                accountViewMode === "private"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <Lock className="h-3 w-3" />
              Private ({formatNumberWithComma(privateAccounts.length)})
            </button>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="outline" size="icon" onClick={handleImport}>
                <FileInput className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Import JSON</TooltipContent>
          </Tooltip>
          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" size="icon" disabled={selectedIds.size === 0}>
                    <FileOutput className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>Export Selected ({formatNumberWithComma(selectedIds.size)})</TooltipContent>
            </Tooltip>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onClick={handleExportJSON}>
                <FileBraces className="h-4 w-4 mr-2" />
                Export JSON
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleExportTXT} disabled={hasPrivateAccountSelected()}>
                <FileText className="h-4 w-4 mr-2" />
                Export TXT
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button 
                variant="outline" 
                size="icon" 
                onClick={() => {
                  const idsToUpdate = selectedIds.size > 0 ? Array.from(selectedIds) : accounts.map((a) => a.id);
                  if (idsToUpdate.length === 0) {
                    toast.error("No accounts selected");
                    return;
                  }
                  const usernames = idsToUpdate
                    .map((id) => {
                      const account = accounts.find((a) => a.id === id);
                      return account?.username;
                    })
                    .filter((username): username is string => !!username);
                  
                  if (usernames.length === 0) {
                    toast.error("No valid usernames found");
                    return;
                  }
                  
                  if (onUpdateSelected) {
                    onUpdateSelected(usernames);
                    toast.success(`Added ${formatNumberWithComma(usernames.length)} account(s) to multiple fetch`);
                  }
                }}
                disabled={selectedIds.size === 0 || hasPrivateAccountSelected()}
              >
                <CloudBackup className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Update Selected ({formatNumberWithComma(selectedIds.size)})</TooltipContent>
          </Tooltip>
          {isBulkDownloading ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="outline" size="icon" onClick={handleStopBulkDownload}>
                  <StopCircle className="h-4 w-4 text-destructive" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Stop Bulk Download</TooltipContent>
            </Tooltip>
          ) : (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="default" size="icon" onClick={handleBulkDownload} disabled={selectedIds.size === 0 || isDownloading}>
                  <Download className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Download Selected ({formatNumberWithComma(selectedIds.size)})</TooltipContent>
            </Tooltip>
          )}
          <Dialog open={clearAllDialogOpen} onOpenChange={setClearAllDialogOpen}>
            <Tooltip>
              <TooltipTrigger asChild>
                <DialogTrigger asChild>
                  <Button variant="destructive" size="icon" disabled={selectedIds.size === 0}>
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </DialogTrigger>
              </TooltipTrigger>
              <TooltipContent>Delete Selected ({formatNumberWithComma(selectedIds.size)})</TooltipContent>
            </Tooltip>
            <DialogContent className="[&>button]:hidden">
              <div className="absolute right-4 top-4">
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-6 w-6 opacity-70 hover:opacity-100"
                  onClick={() => setClearAllDialogOpen(false)}
                >
                  <X className="h-4 w-4" />
                </Button>
              </div>
              <DialogHeader>
                <DialogTitle>Delete {formatNumberWithComma(selectedIds.size)} Selected Account{selectedIds.size !== 1 ? 's' : ''}?</DialogTitle>
                <DialogDescription>
                  This will permanently delete the selected account{selectedIds.size !== 1 ? 's' : ''}. This action cannot be undone.
                </DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button
                  variant="destructive"
                  onClick={async () => {
                    try {
                      const idsToDelete = Array.from(selectedIds);
                      for (const id of idsToDelete) {
                        await DeleteAccountFromDB(id);
                      }
                      toast.success(`Deleted ${formatNumberWithComma(idsToDelete.length)} account${idsToDelete.length !== 1 ? 's' : ''}`);
                      setClearAllDialogOpen(false);
                      setSelectedIds(new Set());
                      loadAccounts();
                    } catch (error) {
                      toast.error("Failed to delete accounts");
                    }
                  }}
                >
                  Delete {formatNumberWithComma(selectedIds.size)} Account{selectedIds.size !== 1 ? 's' : ''}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>
      </div>

      {loading ? (
        <div className="text-center py-12 text-muted-foreground">Loading...</div>
      ) : accounts.length === 0 ? (
        <div className="text-center py-12 text-muted-foreground">
          No saved accounts yet. Fetch a user's media to save it here.
        </div>
      ) : (
        <div className="space-y-2">
          {/* Row 1: Select All + Search Bar (Search only for public) */}
          <div className="flex items-center gap-4 py-2">
            <div className="flex items-center gap-2">
              <Checkbox
                checked={selectedIds.size === filteredAccounts.length && filteredAccounts.length > 0}
                onCheckedChange={toggleSelectAll}
              />
              <span className="text-sm text-muted-foreground">
                Select all {selectedIds.size > 0 && `(${formatNumberWithComma(selectedIds.size)} selected)`}
              </span>
            </div>
            
            {/* Search Bar - Only show for public accounts */}
            {accountViewMode === "public" && (
              <div className="relative flex-1">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                <Input
                  placeholder="Search..."
                  value={searchQuery}
                  onChange={(e) => {
                    setSearchQuery(e.target.value);
                  }}
                  className="pl-9 h-9"
                />
                {searchQuery && (
                  <button
                    type="button"
                    onClick={() => {
                      setSearchQuery("");
                    }}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  >
                    <XCircle className="h-4 w-4" />
                  </button>
                )}
              </div>
            )}
          </div>

          {/* Row 2: Sort, Filters, View Toggle */}
          <div className="flex items-center gap-2 pb-2">
            {/* Sort Order */}
            <Select value={sortOrder} onValueChange={(v) => setSortOrder(v as "newest" | "oldest")}>
              <SelectTrigger className="w-auto">
                <ArrowUpDown className="h-4 w-4 mr-2" />
                <SelectValue placeholder="Sort" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="newest">Newest</SelectItem>
                <SelectItem value="oldest">Oldest</SelectItem>
              </SelectContent>
            </Select>
            
            {/* Media Type Filter */}
            <Select value={filterMediaType} onValueChange={setFilterMediaType}>
              <SelectTrigger className="w-auto">
                <Filter className="h-4 w-4 mr-2" />
                <SelectValue placeholder="Filter by type" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">
                  <span className="flex items-center gap-2">All Types</span>
                </SelectItem>
                <SelectItem value="all-media">
                  <span className="flex items-center gap-2">
                    <Images className="h-4 w-4 text-indigo-500" />
                    All Media
                  </span>
                </SelectItem>
                <SelectItem value="image">
                  <span className="flex items-center gap-2">
                    <Image className="h-4 w-4 text-blue-500" />
                    Images
                  </span>
                </SelectItem>
                <SelectItem value="video">
                  <span className="flex items-center gap-2">
                    <Video className="h-4 w-4 text-purple-500" />
                    Videos
                  </span>
                </SelectItem>
                <SelectItem value="gif">
                  <span className="flex items-center gap-2">
                    <Film className="h-4 w-4 text-green-500" />
                    GIFs
                  </span>
                </SelectItem>
                <SelectItem value="text">
                  <span className="flex items-center gap-2">
                    <FileText className="h-4 w-4 text-orange-500" />
                    Text
                  </span>
                </SelectItem>
              </SelectContent>
            </Select>

            {/* Group Filter */}
            <Select value={filterGroup} onValueChange={setFilterGroup} disabled={groups.length === 0}>
              <SelectTrigger className="w-auto">
                <Tag className="h-4 w-4 mr-2" />
                <SelectValue placeholder="Filter by group" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Groups</SelectItem>
                <SelectItem value="ungrouped">Ungrouped</SelectItem>
                {groups.map((group) => (
                  <SelectItem key={group.name} value={group.name}>
                    <span className="flex items-center gap-2">
                      <span
                        className="w-3 h-3 rounded-full"
                        style={{ backgroundColor: group.color }}
                      />
                      {group.name}
                    </span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <div className="flex-1" />

            {/* Grid View Toggle */}
            <div className="flex items-center border rounded-md">
              <Button
                variant={gridView === "large" ? "secondary" : "ghost"}
                size="icon"
                className="h-9 w-9 rounded-r-none"
                onClick={() => setGridView("large")}
              >
                <LayoutGrid className="h-4 w-4" />
              </Button>
              <Button
                variant={gridView === "small" ? "secondary" : "ghost"}
                size="icon"
                className="h-9 w-9 rounded-none border-x"
                onClick={() => setGridView("small")}
              >
                <Grid3X3 className="h-4 w-4" />
              </Button>
              <Button
                variant={gridView === "list" ? "secondary" : "ghost"}
                size="icon"
                className="h-9 w-9 rounded-l-none"
                onClick={() => setGridView("list")}
              >
                <List className="h-4 w-4" />
              </Button>
            </div>
          </div>

          {/* Bulk Download Progress */}
          {isBulkDownloading && (
            <div className="px-4 py-3 bg-muted/50 rounded-lg space-y-2">
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">
                  Downloading account {bulkDownloadCurrent} of {bulkDownloadTotal}
                </span>
                <span className="font-medium">{Math.round((bulkDownloadCurrent / bulkDownloadTotal) * 100)}%</span>
              </div>
              <Progress value={(bulkDownloadCurrent / bulkDownloadTotal) * 100} className="h-2" />
            </div>
          )}

          {/* Grid View: Large */}
          {gridView === "large" && (
            <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
              {filteredAccounts
                .slice(0, visibleCount)
                .map((account) => (
                <div
                  key={account.id}
                  className={`relative rounded-lg border transition-colors p-4 ${
                    selectedIds.has(account.id) ? "border-primary bg-primary/5" : "bg-card hover:bg-muted/50"
                  }`}
                >
                  {/* Checkbox - Top Left */}
                  <Checkbox
                    checked={selectedIds.has(account.id)}
                    onCheckedChange={() => toggleSelect(account.id)}
                    className="absolute top-2 left-2 z-10"
                  />
                  {/* Media Type Badge - Top Right */}
                  <div className="absolute top-2 right-2 z-10 flex gap-1">
                    <Badge
                      variant="secondary"
                      className={cn(
                        "text-xs flex items-center gap-1",
                        account.media_type === "text" && "bg-orange-500/20 text-orange-600 dark:text-orange-400",
                        account.media_type === "image" && "bg-blue-500/20 text-blue-600 dark:text-blue-400",
                        account.media_type === "video" && "bg-purple-500/20 text-purple-600 dark:text-purple-400",
                        account.media_type === "gif" && "bg-green-500/20 text-green-600 dark:text-green-400",
                        (!account.media_type || account.media_type === "all") && "bg-indigo-500/20 text-indigo-600 dark:text-indigo-400"
                      )}
                    >
                      {account.media_type === "text" ? <FileText className="h-3 w-3" /> :
                       account.media_type === "image" ? <Image className="h-3 w-3" /> :
                       account.media_type === "video" ? <Video className="h-3 w-3" /> :
                       account.media_type === "gif" ? <Film className="h-3 w-3" /> :
                       <Images className="h-3 w-3" />}
                    </Badge>
                    {!account.completed && (
                      <Badge variant="secondary" className="text-xs bg-yellow-500/20 text-yellow-600 dark:text-yellow-400">
                        <AlertCircle className="h-3 w-3" />
                      </Badge>
                    )}
                  </div>
                  <div className="flex flex-col items-center text-center gap-3 pt-4">
                    {isPrivateAccount(account.username) ? (
                      <div 
                        className="w-20 h-20 rounded-full bg-primary/10 flex items-center justify-center cursor-pointer hover:bg-primary/20 transition-colors"
                        onClick={() => handleView(account.id, account.username)}
                      >
                        {account.username === "bookmarks" ? (
                          <Bookmark className="h-10 w-10 text-primary" />
                        ) : (
                          <Heart className="h-10 w-10 text-primary" />
                        )}
                      </div>
                    ) : (
                      <img
                        src={account.profile_image}
                        alt={account.name}
                        className="w-20 h-20 rounded-full cursor-pointer hover:opacity-80 transition-opacity"
                        onClick={() => handleView(account.id, account.username)}
                      />
                    )}
                    <div className="w-full min-w-0">
                      <div className="font-medium truncate">{account.name}</div>
                      {!isPrivateAccount(account.username) && (
                        <button
                          type="button"
                          onClick={() => openExternal(`https://x.com/${account.username}`)}
                          className="text-sm text-muted-foreground hover:text-primary hover:underline"
                        >
                          @{account.username}
                        </button>
                      )}
                      <div className="text-sm text-muted-foreground mt-1">
                        {formatNumberWithComma(account.total_media)}
                      </div>
                    </div>
                    <div className="flex gap-1">
                      {downloadingAccountId === account.id ? (
                        <Button
                          variant="outline"
                          size="icon"
                          className="h-8 w-8"
                          onClick={handleStopDownload}
                        >
                          <StopCircle className="h-4 w-4 text-destructive" />
                        </Button>
                      ) : (
                        <Button
                          variant="default"
                          size="icon"
                          className="h-8 w-8"
                          onClick={() => handleDownload(account.id, account.username)}
                          disabled={isDownloading}
                        >
                          <Download className="h-4 w-4" />
                        </Button>
                      )}
                      <Button
                        variant="outline"
                        size="icon"
                        className="h-8 w-8"
                        onClick={() => handleOpenFolder(account.username)}
                        disabled={!folderExistence.get(account.id)}
                      >
                        <FolderOpen className="h-4 w-4" />
                      </Button>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button variant="outline" size="icon" className="h-8 w-8">
                            <MoreVertical className="h-4 w-4" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem onClick={() => handleEditGroup(account)}>
                            <Pencil className="h-4 w-4 mr-2" />
                            Edit Group
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            onClick={async () => {
                              const settings = getSettings();
                              const outputDir = settings.downloadPath || "";
                              try {
                                await ExportAccountJSON(account.id, outputDir);
                                toast.success(`Exported @${account.username}`);
                              } catch (error) {
                                toast.error("Failed to export");
                              }
                            }}
                          >
                            <FileOutput className="h-4 w-4 mr-2" />
                            Export JSON
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            onClick={() => handleDelete(account.id, account.username)}
                            className="text-destructive focus:text-destructive"
                          >
                            <Trash2 className="h-4 w-4 mr-2" />
                            Delete
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                  </div>
                  {/* Progress bar */}
                  {downloadingAccountId === account.id && downloadProgress && (
                    <div className="mt-3 space-y-1">
                      <div className="flex items-center justify-between text-xs">
                        <span className="text-muted-foreground">
                          {downloadProgress.current}/{downloadProgress.total}
                        </span>
                        <span className="font-medium">{downloadProgress.percent}%</span>
                      </div>
                      <Progress value={downloadProgress.percent} className="h-1.5" />
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}

          {/* Grid View: Small */}
          {gridView === "small" && (
            <div className="grid grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-3">
              {filteredAccounts
                .slice(0, visibleCount)
                .map((account) => (
                <div
                  key={account.id}
                  className={`relative rounded-lg border transition-colors p-3 ${
                    selectedIds.has(account.id) ? "border-primary bg-primary/5" : "bg-card hover:bg-muted/50"
                  }`}
                >
                  {/* Checkbox - Top Left */}
                  <Checkbox
                    checked={selectedIds.has(account.id)}
                    onCheckedChange={() => toggleSelect(account.id)}
                    className="absolute top-1.5 left-1.5 z-10"
                  />
                  {/* Media Type Badge & Incomplete - Top Right */}
                  <div className="absolute top-1.5 right-1.5 z-10 flex gap-1">
                    <Badge
                      variant="secondary"
                      className={cn(
                        "text-xs p-1",
                        account.media_type === "text" && "bg-orange-500/20 text-orange-600 dark:text-orange-400",
                        account.media_type === "image" && "bg-blue-500/20 text-blue-600 dark:text-blue-400",
                        account.media_type === "video" && "bg-purple-500/20 text-purple-600 dark:text-purple-400",
                        account.media_type === "gif" && "bg-green-500/20 text-green-600 dark:text-green-400",
                        (!account.media_type || account.media_type === "all") && "bg-indigo-500/20 text-indigo-600 dark:text-indigo-400"
                      )}
                    >
                      {account.media_type === "text" ? <FileText className="h-3 w-3" /> :
                       account.media_type === "image" ? <Image className="h-3 w-3" /> :
                       account.media_type === "video" ? <Video className="h-3 w-3" /> :
                       account.media_type === "gif" ? <Film className="h-3 w-3" /> :
                       <Images className="h-3 w-3" />}
                    </Badge>
                    {!account.completed && (
                      <Badge variant="secondary" className="text-xs p-1 bg-yellow-500/20 text-yellow-600 dark:text-yellow-400">
                        <AlertCircle className="h-3 w-3" />
                      </Badge>
                    )}
                  </div>
                  <div className="flex flex-col items-center text-center gap-2 pt-4">
                    {isPrivateAccount(account.username) ? (
                      <div 
                        className="w-14 h-14 rounded-full bg-primary/10 flex items-center justify-center cursor-pointer hover:bg-primary/20 transition-colors"
                        onClick={() => handleView(account.id, account.username)}
                      >
                        {account.username === "bookmarks" ? (
                          <Bookmark className="h-7 w-7 text-primary" />
                        ) : (
                          <Heart className="h-7 w-7 text-primary" />
                        )}
                      </div>
                    ) : (
                      <img
                        src={account.profile_image}
                        alt={account.name}
                        className="w-14 h-14 rounded-full cursor-pointer hover:opacity-80 transition-opacity"
                        onClick={() => handleView(account.id, account.username)}
                      />
                    )}
                    <div className="w-full min-w-0">
                      <div className="text-xs font-medium truncate">{account.name}</div>
                      {!isPrivateAccount(account.username) && (
                        <button
                          type="button"
                          onClick={() => openExternal(`https://x.com/${account.username}`)}
                          className="text-xs text-muted-foreground hover:text-primary hover:underline truncate block w-full"
                        >
                          @{account.username}
                        </button>
                      )}
                      <div className="text-xs text-muted-foreground">
                        {formatNumberWithComma(account.total_media)}
                      </div>
                    </div>
                    <div className="flex gap-1">
                      {downloadingAccountId === account.id ? (
                        <Button
                          variant="outline"
                          size="icon"
                          className="h-7 w-7"
                          onClick={handleStopDownload}
                        >
                          <StopCircle className="h-3 w-3 text-destructive" />
                        </Button>
                      ) : (
                        <Button
                          variant="default"
                          size="icon"
                          className="h-7 w-7"
                          onClick={() => handleDownload(account.id, account.username)}
                          disabled={isDownloading}
                        >
                          <Download className="h-3 w-3" />
                        </Button>
                      )}
                      <Button
                        variant="outline"
                        size="icon"
                        className="h-7 w-7"
                        onClick={() => handleOpenFolder(account.username)}
                        disabled={!folderExistence.get(account.id)}
                      >
                        <FolderOpen className="h-3 w-3" />
                      </Button>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button variant="outline" size="icon" className="h-7 w-7">
                            <MoreVertical className="h-3 w-3" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem onClick={() => handleEditGroup(account)}>
                            <Pencil className="h-4 w-4 mr-2" />
                            Edit Group
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            onClick={async () => {
                              const settings = getSettings();
                              const outputDir = settings.downloadPath || "";
                              try {
                                await ExportAccountJSON(account.id, outputDir);
                                toast.success(`Exported @${account.username}`);
                              } catch (error) {
                                toast.error("Failed to export");
                              }
                            }}
                          >
                            <FileOutput className="h-4 w-4 mr-2" />
                            Export JSON
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            onClick={() => handleDelete(account.id, account.username)}
                            className="text-destructive focus:text-destructive"
                          >
                            <Trash2 className="h-4 w-4 mr-2" />
                            Delete
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                  </div>
                  {/* Progress bar */}
                  {downloadingAccountId === account.id && downloadProgress && (
                    <div className="mt-2 space-y-1">
                      <div className="flex items-center justify-between text-xs">
                        <span className="text-muted-foreground">
                          {downloadProgress.current}/{downloadProgress.total}
                        </span>
                        <span className="font-medium">{downloadProgress.percent}%</span>
                      </div>
                      <Progress value={downloadProgress.percent} className="h-1" />
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}

          {/* List View */}
          {gridView === "list" && filteredAccounts
            .slice(0, visibleCount)
            .map((account, index) => (
            <div
              key={account.id}
              className={`rounded-lg border transition-colors ${
                selectedIds.has(account.id) ? "border-primary bg-primary/5" : "bg-card hover:bg-muted/50"
              }`}
            >
              <div className="flex items-center gap-4 p-4">
                <Checkbox
                  checked={selectedIds.has(account.id)}
                  onCheckedChange={() => toggleSelect(account.id)}
                />
                <span className="text-sm text-muted-foreground w-8 text-center shrink-0">
                  {index + 1}
                </span>
                {isPrivateAccount(account.username) ? (
                  <div 
                    className="w-12 h-12 rounded-full bg-primary/10 flex items-center justify-center cursor-pointer hover:bg-primary/20 transition-colors"
                    onClick={() => handleView(account.id, account.username)}
                  >
                    {account.username === "bookmarks" ? (
                      <Bookmark className="h-6 w-6 text-primary" />
                    ) : (
                      <Heart className="h-6 w-6 text-primary" />
                    )}
                  </div>
                ) : (
                  <img
                    src={account.profile_image}
                    alt={account.name}
                    className="w-12 h-12 rounded-full cursor-pointer hover:opacity-80 transition-opacity"
                    onClick={() => handleView(account.id, account.username)}
                  />
                )}
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="truncate">{account.name}</span>
                    <span className="text-muted-foreground">({formatNumberWithComma(account.total_media)})</span>
                    {/* Media Type Badge */}
                    <Badge
                      variant="secondary"
                      className={cn(
                        "text-xs flex items-center gap-1",
                        account.media_type === "text" && "bg-orange-500/20 text-orange-600 dark:text-orange-400",
                        account.media_type === "image" && "bg-blue-500/20 text-blue-600 dark:text-blue-400",
                        account.media_type === "video" && "bg-purple-500/20 text-purple-600 dark:text-purple-400",
                        account.media_type === "gif" && "bg-green-500/20 text-green-600 dark:text-green-400",
                        (!account.media_type || account.media_type === "all") && "bg-indigo-500/20 text-indigo-600 dark:text-indigo-400"
                      )}
                    >
                      {account.media_type === "text" ? <><FileText className="h-3 w-3" /> Text</> :
                       account.media_type === "image" ? <><Image className="h-3 w-3" /> Images</> :
                       account.media_type === "video" ? <><Video className="h-3 w-3" /> Videos</> :
                       account.media_type === "gif" ? <><Film className="h-3 w-3" /> GIFs</> :
                       <><Images className="h-3 w-3" /> All Media</>}
                    </Badge>
                    {!account.completed && (
                      <Badge
                        variant="secondary"
                        className="text-xs flex items-center gap-1 bg-yellow-500/20 text-yellow-600 dark:text-yellow-400"
                      >
                        <AlertCircle className="h-3 w-3" />
                        Incomplete
                      </Badge>
                    )}
                    {account.group_name && (
                      <Badge
                        variant="outline"
                        className="text-xs"
                        style={{ borderColor: account.group_color, color: account.group_color }}
                      >
                        {account.group_name}
                      </Badge>
                    )}
                  </div>
                  {!isPrivateAccount(account.username) && (
                    <button
                      type="button"
                      onClick={() => openExternal(`https://x.com/${account.username}`)}
                      className="text-sm text-muted-foreground hover:text-primary hover:underline"
                    >
                      @{account.username}
                    </button>
                  )}
                  <div className="text-sm text-muted-foreground">
                    {account.last_fetched} {getRelativeTime(account.last_fetched)}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {downloadingAccountId === account.id ? (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          variant="outline"
                          size="icon"
                          onClick={handleStopDownload}
                        >
                          <StopCircle className="h-4 w-4 text-destructive" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>Stop Download</TooltipContent>
                    </Tooltip>
                  ) : (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          variant="default"
                          size="icon"
                          onClick={() => handleDownload(account.id, account.username)}
                          disabled={isDownloading}
                        >
                          {isDownloading && downloadingAccountId === account.id ? (
                            <Spinner className="h-4 w-4" />
                          ) : (
                            <Download className="h-4 w-4" />
                          )}
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>Download All Media</TooltipContent>
                    </Tooltip>
                  )}
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="outline"
                        size="icon"
                        onClick={() => handleOpenFolder(account.username)}
                        disabled={!folderExistence.get(account.id)}
                      >
                        <FolderOpen className="h-4 w-4" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Open Folder</TooltipContent>
                  </Tooltip>
                  <DropdownMenu>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <DropdownMenuTrigger asChild>
                          <Button variant="outline" size="icon">
                            <MoreVertical className="h-4 w-4" />
                          </Button>
                        </DropdownMenuTrigger>
                      </TooltipTrigger>
                      <TooltipContent>More Options</TooltipContent>
                    </Tooltip>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem onClick={() => handleEditGroup(account)}>
                        <Pencil className="h-4 w-4 mr-2" />
                        Edit Group
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={async () => {
                          const settings = getSettings();
                          const outputDir = settings.downloadPath || "";
                          try {
                            await ExportAccountJSON(account.id, outputDir);
                            toast.success(`Exported @${account.username} to ${outputDir}\\twitterxmediabatchdownloader_backups`);
                          } catch (error) {
                            toast.error("Failed to export");
                          }
                        }}
                      >
                        <FileOutput className="h-4 w-4 mr-2" />
                        Export JSON
                      </DropdownMenuItem>
                      <Dialog>
                        <DialogTrigger asChild>
                          <DropdownMenuItem
                            onSelect={(e: Event) => {
                              e.preventDefault();
                            }}
                            className="text-destructive focus:text-destructive"
                          >
                            <Trash2 className="h-4 w-4 mr-2" />
                            Delete
                          </DropdownMenuItem>
                        </DialogTrigger>
                        <DialogContent className="[&>button]:hidden">
                          <div className="absolute right-4 top-4">
                            <DialogTrigger asChild>
                              <Button variant="ghost" size="icon" className="h-6 w-6 opacity-70 hover:opacity-100">
                                <X className="h-4 w-4" />
                              </Button>
                            </DialogTrigger>
                          </div>
                          <DialogHeader>
                            <DialogTitle>Delete @{account.username}?</DialogTitle>
                            <DialogDescription>
                              This will permanently delete the saved data for this account.
                            </DialogDescription>
                          </DialogHeader>
                          <DialogFooter>
                            <Button
                              variant="destructive"
                              onClick={() => handleDelete(account.id, account.username)}
                            >
                              Delete
                            </Button>
                          </DialogFooter>
                        </DialogContent>
                      </Dialog>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>
              {/* Progress bar for this account */}
              {downloadingAccountId === account.id && downloadProgress && (
                <div className="px-4 pb-3 space-y-1">
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-muted-foreground">
                      Downloading {downloadProgress.current} of {downloadProgress.total}
                    </span>
                    <span className="font-medium">{downloadProgress.percent}%</span>
                  </div>
                  <Progress value={downloadProgress.percent} className="h-1.5" />
                </div>
              )}
            </div>
          ))}

          {/* Load More Trigger (invisible) */}
          <div 
            ref={loadMoreRef} 
            className={`h-4 ${visibleCount >= filteredAccounts.length ? 'hidden' : ''}`} 
          />


        </div>
      )}

      {/* Scroll to Top Button */}
      {showScrollTop && (
        <Button
          variant="default"
          size="icon"
          className="fixed bottom-6 left-1/2 -translate-x-1/2 h-9 w-9 rounded-full shadow-lg z-30"
          onClick={scrollToTop}
        >
          <ArrowUp className="h-4 w-4" />
        </Button>
      )}

      {/* Edit Group Dialog */}
      <Dialog open={!!editingAccount} onOpenChange={(open) => !open && setEditingAccount(null)}>
        <DialogContent className="[&>button]:hidden">
          <div className="absolute right-4 top-4">
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6 opacity-70 hover:opacity-100"
              onClick={() => setEditingAccount(null)}
            >
              <X className="h-4 w-4" />
            </Button>
          </div>
          <DialogHeader>
            <DialogTitle>Edit Group for @{editingAccount?.username}</DialogTitle>
            <DialogDescription>
              Assign this account to a group for better organization.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="groupName">Group Name</Label>
              <div className="flex items-center gap-2">
                <Input
                  id="groupName"
                  placeholder="e.g., Artists, Photographers, Friends"
                  value={editGroupName}
                  onChange={(e) => setEditGroupName(e.target.value)}
                  className="flex-1"
                />
                {groups.length > 0 && (
                  <Select
                    value=""
                    onValueChange={(value) => {
                      setEditGroupName(value);
                      const group = groups.find((g) => g.name === value);
                      if (group) setEditGroupColor(group.color);
                    }}
                  >
                    <SelectTrigger className="w-auto">
                      <Tag className="h-4 w-4" />
                    </SelectTrigger>
                    <SelectContent>
                      {groups.map((g) => (
                        <SelectItem key={g.name} value={g.name}>
                          <span className="flex items-center gap-2">
                            <span
                              className="w-3 h-3 rounded-full"
                              style={{ backgroundColor: g.color }}
                            />
                            {g.name}
                          </span>
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
                {editGroupName && (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="outline"
                        size="icon"
                        onClick={() => {
                          setEditGroupName("");
                          setEditGroupColor("#3b82f6");
                        }}
                      >
                        <X className="h-4 w-4" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Remove from Group</TooltipContent>
                  </Tooltip>
                )}
              </div>
            </div>
            <div className="space-y-2">
              <Label htmlFor="groupColor">Group Color</Label>
              <div className="flex items-center gap-3">
                <div className="relative w-10 h-10">
                  <input
                    id="groupColor"
                    type="color"
                    value={editGroupColor}
                    onChange={(e) => setEditGroupColor(e.target.value)}
                    className="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
                  />
                  <div
                    className="w-10 h-10 rounded-full border-2 border-border cursor-pointer"
                    style={{ backgroundColor: editGroupColor }}
                  />
                </div>
                <Input
                  value={editGroupColor}
                  onChange={(e) => setEditGroupColor(e.target.value)}
                  placeholder="#3b82f6"
                  className="w-28 font-mono text-sm"
                />
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="outline"
                      size="icon"
                      onClick={() => {
                        const randomColor = "#" + Math.floor(Math.random() * 16777215).toString(16).padStart(6, "0");
                        setEditGroupColor(randomColor);
                      }}
                    >
                      <Shuffle className="h-4 w-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Random Color</TooltipContent>
                </Tooltip>
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setEditingAccount(null)}>
              Cancel
            </Button>
            <Button onClick={handleSaveGroup}>
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
