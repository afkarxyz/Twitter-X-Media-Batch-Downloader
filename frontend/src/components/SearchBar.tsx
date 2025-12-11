import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { InputWithContext } from "@/components/ui/input-with-context";
import { Label } from "@/components/ui/label";
import {
  XCircle,
  Calendar,
  StopCircle,
  Globe,
  Lock,
  Bookmark,
  Heart,
  Key,
  Eye,
  EyeOff,
  ChevronDown,
  ChevronUp,
  Info,
  RotateCcw,
  Trash2,
  Clock,
  CloudDownload,
  User,
  Users,
  FileText,
  Hourglass,
  CheckCircle,
  AlertCircle,
} from "lucide-react";
import { Spinner } from "@/components/ui/spinner";
import { Checkbox } from "@/components/ui/checkbox";
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
import { Settings as SettingsIcon } from "lucide-react";
import { FetchHistory } from "@/components/FetchHistory";
import type { HistoryItem } from "@/components/FetchHistory";
import { cn } from "@/lib/utils";

export type FetchMode = "public" | "private";
export type PrivateType = "bookmarks" | "likes";
export type FetchType = "single" | "multiple";

// Local storage keys for auth tokens
const PUBLIC_AUTH_TOKEN_KEY = "twitter_public_auth_token";
const PRIVATE_AUTH_TOKEN_KEY = "twitter_private_auth_token";

export interface MultipleAccount {
  id: string;
  username: string;
  status: "pending" | "fetching" | "completed" | "incomplete" | "failed";
  accountInfo?: {
    name: string;
    nick: string;
    profile_image: string;
  };
  mediaCount: number;
  previousMediaCount: number;
  elapsedTime: number;
  remainingTime: number | null;
  error?: string;
  showDiff?: boolean;
  cursor?: string;
}

interface SearchBarProps {
  username: string;
  loading: boolean;
  onUsernameChange: (username: string) => void;
  onFetch: (
    useDateRange: boolean,
    startDate?: string,
    endDate?: string,
    mediaType?: string,
    retweets?: boolean,
    mode?: FetchMode,
    privateType?: PrivateType,
    authToken?: string,
    isResume?: boolean
  ) => void;
  onStopFetch: () => void;
  onResume?: (authToken: string, mediaType?: string, retweets?: boolean) => void;
  onClearResume?: () => void;
  resumeInfo?: { canResume: boolean; mediaCount: number } | null;
  history: HistoryItem[];
  onHistorySelect: (item: HistoryItem) => void;
  onHistoryRemove: (id: string) => void;
  hasResult: boolean;
  elapsedTime?: number;
  remainingTime?: number | null;
  // Multiple mode props
  fetchType?: FetchType;
  onFetchTypeChange?: (type: FetchType) => void;
  multipleAccounts?: MultipleAccount[];
  onImportFile?: () => void;
  onFetchAll?: () => void;
  onStopAll?: () => void;
  onStopAccount?: (accountId: string) => void;
  onRetryAccount?: (accountId: string) => void;
  isFetchingAll?: boolean;
  // Mode control
  mode?: FetchMode;
  privateType?: PrivateType;
  onModeChange?: (mode: FetchMode, privateType?: PrivateType) => void;
}

export function SearchBar({
  username,
  loading,
  onUsernameChange,
  onFetch,
  onStopFetch,
  onResume,
  onClearResume,
  resumeInfo,
  history,
  onHistorySelect,
  onHistoryRemove,
  hasResult,
  elapsedTime = 0,
  remainingTime = null,
  fetchType = "single",
  onFetchTypeChange,
  multipleAccounts = [],
  onImportFile,
  onFetchAll,
  onStopAll,
  onStopAccount,
  onRetryAccount,
  isFetchingAll = false,
  mode: externalMode,
  privateType: externalPrivateType,
  onModeChange,
}: SearchBarProps) {
  const [useDateRange, setUseDateRange] = useState(false);
  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [mediaType, setMediaType] = useState("all");
  const [retweets, setRetweets] = useState(false);
  const [mode, setMode] = useState<FetchMode>(externalMode || "public");
  const [privateType, setPrivateType] = useState<PrivateType>(externalPrivateType || "bookmarks");
  
  // Sync with external mode if provided
  useEffect(() => {
    if (externalMode !== undefined) {
      setMode(externalMode);
    }
  }, [externalMode]);
  
  useEffect(() => {
    if (externalPrivateType !== undefined) {
      setPrivateType(externalPrivateType);
    }
  }, [externalPrivateType]);
  const [showAuthInput, setShowAuthInput] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);

  // Separate auth tokens for public and private modes
  const [publicAuthToken, setPublicAuthToken] = useState("");
  const [privateAuthToken, setPrivateAuthToken] = useState("");
  const [showPublicToken, setShowPublicToken] = useState(false);
  const [showPrivateToken, setShowPrivateToken] = useState(false);

  // Load saved auth tokens on mount
  useEffect(() => {
    const savedPublicToken = localStorage.getItem(PUBLIC_AUTH_TOKEN_KEY) || "";
    const savedPrivateToken = localStorage.getItem(PRIVATE_AUTH_TOKEN_KEY) || "";
    setPublicAuthToken(savedPublicToken);
    setPrivateAuthToken(savedPrivateToken);
  }, []);

  // Save auth tokens when they change
  const handlePublicTokenChange = (value: string) => {
    setPublicAuthToken(value);
    localStorage.setItem(PUBLIC_AUTH_TOKEN_KEY, value);
  };

  const handlePrivateTokenChange = (value: string) => {
    setPrivateAuthToken(value);
    localStorage.setItem(PRIVATE_AUTH_TOKEN_KEY, value);
  };

  const handleFetch = () => {
    const authToken = mode === "public" ? publicAuthToken : privateAuthToken;
    onFetch(useDateRange, startDate, endDate, mediaType, retweets, mode, privateType, authToken, false);
  };

  const handleResume = () => {
    const authToken = mode === "public" ? publicAuthToken : privateAuthToken;
    if (onResume) {
      onResume(authToken, mediaType, retweets);
    }
  };

  const currentAuthToken = mode === "public" ? publicAuthToken : privateAuthToken;
  const hasAuthToken = currentAuthToken.trim().length > 0;
  // Likes needs username (URL is /username/likes), bookmarks doesn't
  const isLikesMode = mode === "private" && privateType === "likes";
  const isBookmarksMode = mode === "private" && privateType === "bookmarks";

  const formatTime = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${String(secs).padStart(2, "0")}`;
  };

  return (
    <div className="space-y-3">
      {/* Fetch Type and Mode Toggle (Single/Multiple and Public/Private in one line) */}
      <div className="flex justify-center gap-2">
        <div className="flex gap-0.5 p-0.5 bg-muted rounded-lg w-fit">
          <button
            type="button"
            onClick={() => onFetchTypeChange?.("single")}
            className={cn(
              "flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-all cursor-pointer",
              fetchType === "single"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground"
            )}
          >
            <User className="h-3 w-3" />
            Single
          </button>
          <button
            type="button"
            onClick={() => onFetchTypeChange?.("multiple")}
            className={cn(
              "flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-all cursor-pointer",
              fetchType === "multiple"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground"
            )}
          >
            <Users className="h-3 w-3" />
            Multiple
          </button>
        </div>
        {fetchType === "single" && (
          <div className="flex gap-0.5 p-0.5 bg-muted rounded-lg w-fit">
            <button
              type="button"
              onClick={() => {
                setMode("public");
                onModeChange?.("public");
              }}
              className={cn(
                "flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-all cursor-pointer",
                mode === "public"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              <Globe className="h-3 w-3" />
              Public
            </button>
            <button
              type="button"
              onClick={() => {
                setMode("private");
                onModeChange?.("private");
              }}
              className={cn(
                "flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-all cursor-pointer",
                mode === "private"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              <Lock className="h-3 w-3" />
              Private
            </button>
          </div>
        )}
      </div>

      {/* Multiple Mode UI - Import and Fetch All Buttons */}
      {fetchType === "multiple" && (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Label htmlFor="import-txt" className="text-sm">
              Import Accounts
            </Label>
            <Tooltip>
              <TooltipTrigger asChild>
                <Info className="h-4 w-4 text-muted-foreground cursor-help" />
              </TooltipTrigger>
              <TooltipContent className="max-w-xs">
                <p className="text-sm">
                  One username per line, example:
                  <br />
                  <span className="font-mono text-xs">masteraoko</span>
                  <br />
                  <span className="font-mono text-xs">xbatchdemo</span>
                  <br />
                  <span className="font-mono text-xs">takomayuyi</span>
                </p>
              </TooltipContent>
            </Tooltip>
          </div>
          <div className="flex items-center gap-2">
            <Button
              id="import-txt"
              variant="outline"
              onClick={onImportFile}
              className="flex items-center gap-2 flex-1"
            >
              <FileText className="h-4 w-4" />
              Import TXT File
            </Button>
            <Button
              variant="default"
              onClick={onFetchAll}
              disabled={multipleAccounts.length === 0 || isFetchingAll}
              className="flex items-center gap-2 flex-1"
            >
              {isFetchingAll ? (
                <>
                  <Spinner />
                  Fetching All...
                </>
              ) : (
                <>
                  <CloudDownload className="h-4 w-4" />
                  Fetch All
                </>
              )}
            </Button>
            {isFetchingAll && (
              <Button
                variant="destructive"
                onClick={onStopAll}
                className="flex items-center gap-2"
              >
                <StopCircle className="h-4 w-4" />
                Stop All
              </Button>
            )}
          </div>
        </div>
      )}

      {/* Username Input - for public mode and likes mode, only in single mode */}
      {fetchType === "single" && (mode === "public" || isLikesMode) && (
        <div className="space-y-2">
          <Label htmlFor="username">
            {isLikesMode ? "Your Username" : "Username"}
          </Label>

          <div className="flex gap-2">
            <div className="relative flex-1">
              <InputWithContext
                id="username"
                placeholder={
                  isLikesMode
                    ? "your_username or @your_username or https://x.com/your_username"
                    : "masteraoko or @masteraoko or https://x.com/masteraoko"
                }
                value={username}
                onChange={(e) => onUsernameChange(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleFetch()}
                className="pr-8"
              />
              {username && (
                <button
                  type="button"
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
                  onClick={() => onUsernameChange("")}
                >
                  <XCircle className="h-4 w-4" />
                </button>
              )}
            </div>
            <div className="flex items-center gap-2">
              {loading && (
                <>
                  {/* Timer Display */}
                  {(remainingTime !== null || elapsedTime > 0) && (
                    <div className="flex items-center gap-1.5 px-3 py-1.5 border rounded-md bg-muted/50 text-sm w-[85px]">
                      <Clock className="h-3.5 w-3.5 text-muted-foreground flex-shrink-0" />
                      <span className="font-mono">
                        {remainingTime !== null && remainingTime >= 0
                          ? `${Math.floor(remainingTime / 60)}:${String(remainingTime % 60).padStart(2, "0")}`
                          : `${Math.floor(elapsedTime / 60)}:${String(elapsedTime % 60).padStart(2, "0")}`}
                      </span>
                    </div>
                  )}
                  <Button variant="destructive" onClick={onStopFetch}>
                    <StopCircle className="h-4 w-4" />
                    Stop
                  </Button>
                </>
              )}
              {/* Resume button - show when there's resumable data */}
              {!loading && resumeInfo?.canResume && mode === "public" && (
                <>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button variant="outline" size="icon" onClick={onClearResume}>
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Clear resume data</TooltipContent>
                  </Tooltip>
                  <Button variant="secondary" onClick={handleResume} disabled={!hasAuthToken}>
                    <RotateCcw className="h-4 w-4" />
                    Resume ({resumeInfo.mediaCount})
                  </Button>
                </>
              )}
              <Button onClick={handleFetch} disabled={loading || !hasAuthToken}>
                {loading ? (
                  <>
                    <Spinner />
                    Fetching...
                  </>
                ) : (
                  <>
                    {isLikesMode ? <Heart className="h-4 w-4" /> : <CloudDownload className="h-4 w-4" />}
                    {isLikesMode ? "Fetch Likes" : "Fetch"}
                  </>
                )}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Auth Token & Advanced Settings Row */}
      <div className="flex items-center gap-2">
        {/* Auth Token Button */}
        <button
          type="button"
          onClick={() => setShowAuthInput(!showAuthInput)}
          className="flex items-center gap-2 px-4 py-2 border rounded-lg text-sm font-medium hover:bg-muted/50 transition-colors cursor-pointer"
        >
          <Key className={cn("h-4 w-4", hasAuthToken ? "text-green-500" : "text-destructive")} />
          Auth Token
          {showAuthInput ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
        </button>

        {/* Advanced Settings Button */}
        <button
          type="button"
          onClick={() => setShowAdvanced(!showAdvanced)}
          className="flex items-center gap-2 px-4 py-2 border rounded-lg text-sm font-medium hover:bg-muted/50 transition-colors cursor-pointer"
        >
          <SettingsIcon className="h-4 w-4" />
          Advanced Settings
          {showAdvanced ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
        </button>

        {/* Private Mode Options - My Bookmarks / My Likes */}
        {mode === "private" && fetchType === "single" && (
          <>
            <button
              type="button"
              onClick={() => {
                setPrivateType("bookmarks");
                onModeChange?.("private", "bookmarks");
              }}
              className={cn(
                "flex items-center gap-2 px-4 py-2 rounded-lg border text-sm font-medium transition-all cursor-pointer",
                privateType === "bookmarks"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border hover:border-primary/50"
              )}
            >
              <Bookmark className="h-4 w-4" />
              My Bookmarks
            </button>
            <button
              type="button"
              onClick={() => {
                setPrivateType("likes");
                onModeChange?.("private", "likes");
              }}
              className={cn(
                "flex items-center gap-2 px-4 py-2 rounded-lg border text-sm font-medium transition-all cursor-pointer",
                privateType === "likes"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border hover:border-primary/50"
              )}
            >
              <Heart className="h-4 w-4" />
              My Likes
            </button>
          </>
        )}

        {/* Bookmarks Mode - Fetch button inline */}
        {isBookmarksMode && (
          <div className="flex items-center gap-2 ml-auto">
            {loading && (
              <>
                {/* Timer Display */}
                {(remainingTime !== null || elapsedTime > 0) && (
                  <div className="flex items-center gap-1.5 px-3 py-1.5 border rounded-md bg-muted/50 text-sm w-[85px]">
                    <Clock className="h-3.5 w-3.5 text-muted-foreground flex-shrink-0" />
                    <span className="font-mono">
                      {remainingTime !== null && remainingTime >= 0
                        ? `${Math.floor(remainingTime / 60)}:${String(remainingTime % 60).padStart(2, "0")}`
                        : `${Math.floor(elapsedTime / 60)}:${String(elapsedTime % 60).padStart(2, "0")}`}
                    </span>
                  </div>
                )}
                <Button variant="destructive" onClick={onStopFetch}>
                  <StopCircle className="h-4 w-4" />
                  Stop
                </Button>
              </>
            )}
            <Button onClick={handleFetch} disabled={loading || !hasAuthToken}>
              {loading ? (
                <>
                  <Spinner />
                  Fetching...
                </>
              ) : (
                <>
                  <Bookmark className="h-4 w-4" />
                  Fetch Bookmarks
                </>
              )}
            </Button>
          </div>
        )}
      </div>

      {/* Auth Token Input (collapsible) */}
      {showAuthInput && (
        <div className="p-3 border rounded-lg bg-muted/30">
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5">
              <Label htmlFor="auth-token" className="text-sm whitespace-nowrap">
                Auth Token
              </Label>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Info className="h-4 w-4 text-muted-foreground cursor-help" />
                </TooltipTrigger>
                <TooltipContent className="text-center">
                  {mode === "private" ? (
                    <p>Use auth token from the account whose bookmarks/likes you want to fetch</p>
                  ) : (
                    <p>Recommended to use a dummy account, not your main account.<br />Excessive usage may cause suspension</p>
                  )}
                </TooltipContent>
              </Tooltip>
            </div>
            <div className="relative flex-1">
              <InputWithContext
                id="auth-token"
                type={
                  mode === "public"
                    ? showPublicToken
                      ? "text"
                      : "password"
                    : showPrivateToken
                      ? "text"
                      : "password"
                }
                placeholder="Enter your auth_token cookie value"
                value={currentAuthToken}
                onChange={(e) =>
                  mode === "public"
                    ? handlePublicTokenChange(e.target.value)
                    : handlePrivateTokenChange(e.target.value)
                }
                className="pr-10 bg-background"
              />
              <button
                type="button"
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                onClick={() =>
                  mode === "public"
                    ? setShowPublicToken(!showPublicToken)
                    : setShowPrivateToken(!showPrivateToken)
                }
              >
                {(mode === "public" ? showPublicToken : showPrivateToken) ? (
                  <EyeOff className="h-4 w-4" />
                ) : (
                  <Eye className="h-4 w-4" />
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Advanced Settings (collapsible) */}
      {showAdvanced && (
        <div className="p-3 border rounded-lg bg-muted/30 space-y-3">
          {/* Options Row */}
          <div className="flex items-center gap-4">
            {/* Media Type */}
            <div className="flex items-center gap-2">
              <Label htmlFor="media-type" className="text-sm">
                Media Type
              </Label>
              <Select value={mediaType} onValueChange={setMediaType}>
                <SelectTrigger id="media-type" className="w-auto h-8 bg-background">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All Media</SelectItem>
                  <SelectItem value="image">Images Only</SelectItem>
                  <SelectItem value="video">Videos Only</SelectItem>
                  <SelectItem value="gif">GIFs Only</SelectItem>
                  <SelectItem value="text">Text Only (No Media)</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {/* Include Retweets */}
            <div className="flex items-center gap-2">
              <Checkbox
                id="retweets"
                checked={retweets}
                onCheckedChange={(checked) => setRetweets(checked as boolean)}
                className="bg-background"
              />
              <Label htmlFor="retweets" className="text-sm cursor-pointer">
                Include Retweets
              </Label>
            </div>

            {/* Date Range Toggle - only for public mode */}
            {mode === "public" && (
              <>
                <div className="flex items-center gap-2">
                  <Checkbox
                    id="date-range"
                    checked={useDateRange}
                    onCheckedChange={(checked) => setUseDateRange(checked as boolean)}
                    className="bg-background"
                  />
                  <Label
                    htmlFor="date-range"
                    className="text-sm cursor-pointer flex items-center gap-1"
                  >
                    <Calendar className="h-4 w-4" />
                    Date Range
                  </Label>
                </div>

                {/* Date Range Inputs - inline */}
                {useDateRange && (
                  <>
                    <InputWithContext
                      id="start-date"
                      type="date"
                      value={startDate}
                      onChange={(e) => setStartDate(e.target.value)}
                      className="w-[140px] h-8 bg-background"
                    />
                    <span className="text-sm text-muted-foreground">-</span>
                    <InputWithContext
                      id="end-date"
                      type="date"
                      value={endDate}
                      onChange={(e) => setEndDate(e.target.value)}
                      className="w-[140px] h-8 bg-background"
                    />
                  </>
                )}
              </>
            )}
          </div>
        </div>
      )}

      {/* Statistics for Multiple Fetch - below auth token and advanced settings */}
      {fetchType === "multiple" && multipleAccounts.length > 0 && (
        <div className="p-3 border rounded-lg bg-muted/30">
          <div className="flex items-center gap-6 text-sm">
            <div className="flex items-center gap-2">
              <CheckCircle className="h-4 w-4 text-green-600 dark:text-green-400" />
              <span className="text-muted-foreground">Completed:</span>
              <span className="font-medium">
                {multipleAccounts.filter((acc) => acc.status === "completed").length}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <AlertCircle className="h-4 w-4 text-yellow-600 dark:text-yellow-400" />
              <span className="text-muted-foreground">Incomplete:</span>
              <span className="font-medium">
                {multipleAccounts.filter((acc) => acc.status === "incomplete").length}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <XCircle className="h-4 w-4 text-red-600 dark:text-red-400" />
              <span className="text-muted-foreground">Failed:</span>
              <span className="font-medium">
                {multipleAccounts.filter((acc) => acc.status === "failed").length}
              </span>
            </div>
            <div className="flex items-center gap-2 ml-auto">
              <span className="text-muted-foreground">Total:</span>
              <span className="font-medium">
                {multipleAccounts.filter((acc) => 
                  acc.status === "completed" || acc.status === "incomplete" || acc.status === "failed"
                ).length}/{multipleAccounts.length}
              </span>
            </div>
          </div>
        </div>
      )}

      {/* Account List - below auth token and advanced settings */}
      {fetchType === "multiple" && multipleAccounts.length > 0 && (
        <div className="space-y-2">
          <p className="text-sm text-muted-foreground">Accounts to Fetch ({multipleAccounts.length})</p>
          <div className="space-y-2 max-h-[400px] overflow-y-auto">
            {multipleAccounts.map((account) => (
              <div
                key={account.id}
                className="p-3 border rounded-lg bg-card"
              >
                <div className="flex items-center gap-3">
                  {account.accountInfo ? (
                    <img
                      src={account.accountInfo.profile_image}
                      alt={account.accountInfo.nick}
                      className="w-10 h-10 rounded-full"
                    />
                  ) : (
                    <div className="w-10 h-10 rounded-full bg-muted flex items-center justify-center">
                      <User className="h-5 w-5 text-muted-foreground" />
                    </div>
                  )}
                  <div className="flex-1 min-w-0">
                    {account.accountInfo ? (
                      <>
                        <div className="flex items-center gap-2">
                          <span className="font-medium">{account.accountInfo.nick}</span>
                          <span className="text-sm text-muted-foreground">@{account.accountInfo.name}</span>
                        </div>
                        <div className="flex items-center gap-2 mt-1">
                          <span className="text-sm text-muted-foreground">
                            <span className="text-primary">{account.mediaCount.toLocaleString()}</span> items found
                          </span>
                          {account.showDiff && account.previousMediaCount > 0 && account.mediaCount > account.previousMediaCount && (
                            <span className="text-sm text-green-600 dark:text-green-400 font-medium">
                              +{account.mediaCount - account.previousMediaCount}
                            </span>
                          )}
                        </div>
                      </>
                    ) : (
                      <div className="flex items-center gap-2">
                        <span className="font-medium">@{account.username}</span>
                      </div>
                    )}
                  </div>
                  <div className="flex items-center gap-2">
                    {account.status === "fetching" && (
                      <>
                        <div className="flex items-center gap-1.5 px-3 py-1.5 border rounded-md bg-muted/50 text-sm w-[85px]">
                          <Clock className="h-3.5 w-3.5 text-muted-foreground flex-shrink-0" />
                          <span className="font-mono">
                            {account.remainingTime !== null && account.remainingTime >= 0
                              ? formatTime(account.remainingTime)
                              : formatTime(account.elapsedTime)}
                          </span>
                        </div>
                        <Button
                          variant="destructive"
                          onClick={() => onStopAccount?.(account.id)}
                          className="flex items-center gap-2"
                        >
                          <StopCircle className="h-4 w-4" />
                          Stop
                        </Button>
                      </>
                    )}
                    {account.status === "pending" && (
                      <span className="text-xs px-2 py-1 bg-gray-500/20 text-gray-600 dark:text-gray-400 rounded flex items-center gap-1.5">
                        <Hourglass className="h-3 w-3" />
                        Pending
                      </span>
                    )}
                    {account.status === "completed" && (
                      <span className="text-xs px-2 py-1 bg-green-500/20 text-green-600 dark:text-green-400 rounded flex items-center gap-1.5">
                        <CheckCircle className="h-3 w-3" />
                        Completed
                      </span>
                    )}
                    {account.status === "incomplete" && (
                      <>
                        <span className="text-xs px-2 py-1 bg-yellow-500/20 text-yellow-600 dark:text-yellow-400 rounded flex items-center gap-1.5">
                          <AlertCircle className="h-3 w-3" />
                          Incomplete
                        </span>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => onRetryAccount?.(account.id)}
                          disabled={isFetchingAll}
                          className="flex items-center gap-2"
                        >
                          <RotateCcw className="h-3.5 w-3.5" />
                          Retry
                        </Button>
                      </>
                    )}
                    {account.status === "failed" && (
                      <>
                        <span className="text-xs px-2 py-1 bg-red-500/20 text-red-600 dark:text-red-400 rounded flex items-center gap-1.5">
                          <XCircle className="h-3 w-3" />
                          Failed
                        </span>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => onRetryAccount?.(account.id)}
                          disabled={isFetchingAll}
                          className="flex items-center gap-2"
                        >
                          <RotateCcw className="h-3.5 w-3.5" />
                          Retry
                        </Button>
                      </>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {!hasResult && mode === "public" && fetchType === "single" && (
        <FetchHistory
          history={history}
          onSelect={onHistorySelect}
          onRemove={onHistoryRemove}
        />
      )}
    </div>
  );
}
