import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { InputWithContext } from "@/components/ui/input-with-context";
import { Label } from "@/components/ui/label";
import {
  Search,
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

// Local storage keys for auth tokens
const PUBLIC_AUTH_TOKEN_KEY = "twitter_public_auth_token";
const PRIVATE_AUTH_TOKEN_KEY = "twitter_private_auth_token";

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
}: SearchBarProps) {
  const [useDateRange, setUseDateRange] = useState(false);
  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [mediaType, setMediaType] = useState("all");
  const [retweets, setRetweets] = useState(false);
  const [mode, setMode] = useState<FetchMode>("public");
  const [privateType, setPrivateType] = useState<PrivateType>("bookmarks");
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

  return (
    <div className="space-y-3">
      {/* Mode Toggle */}
      <div className="flex justify-center">
        <div className="flex gap-1 p-1 bg-muted rounded-lg w-fit">
          <button
            type="button"
            onClick={() => setMode("public")}
            className={cn(
              "flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all cursor-pointer",
              mode === "public"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground"
            )}
          >
            <Globe className="h-4 w-4" />
            Public
          </button>
          <button
            type="button"
            onClick={() => setMode("private")}
            className={cn(
              "flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all cursor-pointer",
              mode === "private"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground"
            )}
          >
            <Lock className="h-4 w-4" />
            Private
          </button>
        </div>
      </div>

      {/* Username Input - for public mode and likes mode */}
      {(mode === "public" || isLikesMode) && (
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
                <Button variant="destructive" onClick={onStopFetch}>
                  <StopCircle className="h-4 w-4" />
                  Stop
                </Button>
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
                    {isLikesMode ? <Heart className="h-4 w-4" /> : <Search className="h-4 w-4" />}
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
        {mode === "private" && (
          <>
            <button
              type="button"
              onClick={() => setPrivateType("bookmarks")}
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
              onClick={() => setPrivateType("likes")}
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
              <Button variant="destructive" onClick={onStopFetch}>
                <StopCircle className="h-4 w-4" />
                Stop
              </Button>
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

      {!hasResult && mode === "public" && (
        <FetchHistory
          history={history}
          onSelect={onHistorySelect}
          onRemove={onHistoryRemove}
        />
      )}
    </div>
  );
}
