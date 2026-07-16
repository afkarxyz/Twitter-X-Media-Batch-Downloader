import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { InputWithContext } from "@/components/ui/input-with-context";
import { Label } from "@/components/ui/label";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { FetchHistory } from "@/components/FetchHistory";
import type { HistoryItem } from "@/components/FetchHistory";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { cn } from "@/lib/utils";
import { getSettings, updateSettings, VIDEO_QUALITIES, IMAGE_SIZES, AVATAR_SIZES, BANNER_SIZES, type VideoQuality, type ImageSize, type AvatarSize, type BannerSize } from "@/lib/settings";
import { GetStoredAuthToken, SetStoredAuthToken } from "../../wailsjs/go/main/App";
import { AlertCircle, Bookmark, CheckCircle, Clipboard, CloudDownload, Clock, Database, Eye, EyeOff, Globe, Heart, Hourglass, Info, Lock, RotateCcw, SlidersHorizontal, StopCircle, Trash2, User, Users, XCircle } from "lucide-react";
import { Spinner } from "@/components/ui/spinner";
function formatNumberWithComma(num: number): string {
    return num.toLocaleString();
}
function pillClass(active: boolean): string {
    return cn("rounded-md px-3 py-1.5 text-sm font-medium transition-colors cursor-pointer", active
        ? "bg-primary text-primary-foreground"
        : "bg-muted text-muted-foreground hover:text-foreground");
}
export type FetchMode = "public" | "private";
export type PrivateType = "bookmarks" | "likes";
export type FetchType = "single" | "multiple";
const LEGACY_PUBLIC_AUTH_TOKEN_KEY = "twitter_public_auth_token";
const LEGACY_PRIVATE_AUTH_TOKEN_KEY = "twitter_private_auth_token";
let sessionAuthToken = "";
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
    onFetch: (useDateRange: boolean, startDate?: string, endDate?: string, mediaType?: string, retweets?: boolean, mode?: FetchMode, privateType?: PrivateType, authToken?: string, isResume?: boolean) => void;
    onStopFetch: () => void;
    onResume?: (authToken: string, mediaType?: string, retweets?: boolean) => void;
    onClearResume?: () => void;
    resumeInfo?: {
        canResume: boolean;
        mediaCount: number;
    } | null;
    history: HistoryItem[];
    onHistorySelect: (item: HistoryItem) => void;
    onHistoryRemove: (id: string) => void;
    hasResult: boolean;
    elapsedTime?: number;
    remainingTime?: number | null;
    fetchType?: FetchType;
    onFetchTypeChange?: (type: FetchType) => void;
    multipleAccounts?: MultipleAccount[];
    onImportAccounts?: (accounts: string[]) => void;
    onFetchAll?: (authToken: string) => void;
    onStopAll?: () => void;
    onStopAccount?: (accountId: string) => void;
    onRetryAccount?: (accountId: string, authToken: string) => void;
    onClearMultipleAccounts?: () => void;
    onRemoveMultipleAccount?: (accountId: string) => void;
    onOpenSavedAccounts?: () => void;
    isFetchingAll?: boolean;
    mode?: FetchMode;
    privateType?: PrivateType;
    onModeChange?: (mode: FetchMode, privateType?: PrivateType) => void;
}
export function SearchBar({ username, loading, onUsernameChange, onFetch, onStopFetch, onResume, onClearResume, resumeInfo, history, onHistorySelect, onHistoryRemove, hasResult, elapsedTime = 0, remainingTime = null, fetchType = "single", onFetchTypeChange, multipleAccounts = [], onImportAccounts, onFetchAll, onStopAll, onStopAccount, onRetryAccount, onClearMultipleAccounts, onRemoveMultipleAccount, onOpenSavedAccounts, isFetchingAll = false, mode: externalMode, privateType: externalPrivateType, onModeChange, }: SearchBarProps) {
    const [includePhotos, setIncludePhotos] = useState(getSettings().includePhotos);
    const [includeVideos, setIncludeVideos] = useState(getSettings().includeVideos);
    const [includeGifs, setIncludeGifs] = useState(getSettings().includeGifs);
    const [includeText, setIncludeText] = useState(getSettings().includeText);
    const [retweets, setRetweets] = useState(getSettings().includeRetweets);
    const [videoQuality, setVideoQuality] = useState<VideoQuality>(getSettings().videoQuality);
    const [imageSize, setImageSize] = useState<ImageSize>(getSettings().imageSize);
    const [avatarSize, setAvatarSize] = useState<AvatarSize>(getSettings().avatarSize);
    const [bannerSize, setBannerSize] = useState<BannerSize>(getSettings().bannerSize);
    const [sharedAuthToken, setSharedAuthToken] = useState(() => sessionAuthToken || localStorage.getItem(LEGACY_PUBLIC_AUTH_TOKEN_KEY) || localStorage.getItem(LEGACY_PRIVATE_AUTH_TOKEN_KEY) || "");
    const [showAuthToken, setShowAuthToken] = useState(false);
    const [showAuthRequiredDialog, setShowAuthRequiredDialog] = useState(false);
    const [showSettingsDialog, setShowSettingsDialog] = useState(false);
    useEffect(() => {
        if (showSettingsDialog) {
            const fresh = getSettings();
            setIncludePhotos(fresh.includePhotos);
            setIncludeVideos(fresh.includeVideos);
            setIncludeGifs(fresh.includeGifs);
            setIncludeText(fresh.includeText);
            setRetweets(fresh.includeRetweets);
            setVideoQuality(fresh.videoQuality);
            setImageSize(fresh.imageSize);
            setAvatarSize(fresh.avatarSize);
            setBannerSize(fresh.bannerSize);
        }
    }, [showSettingsDialog]);
    const [showImportDialog, setShowImportDialog] = useState(false);
    const [importAccountsText, setImportAccountsText] = useState("");
    const [authPromptToken, setAuthPromptToken] = useState("");
    const mode = externalMode || "public";
    const privateType = externalPrivateType || "bookmarks";
    const isLikesMode = mode === "private" && privateType === "likes";
    const isBookmarksMode = mode === "private" && privateType === "bookmarks";
    const isAnyMultipleAccountFetching = multipleAccounts.some((account) => account.status === "fetching");
    useEffect(() => {
        let active = true;
        const syncStoredTokens = async () => {
            const legacyPublicToken = localStorage.getItem(LEGACY_PUBLIC_AUTH_TOKEN_KEY) || "";
            const legacyPrivateToken = localStorage.getItem(LEGACY_PRIVATE_AUTH_TOKEN_KEY) || "";
            const legacyUnifiedToken = legacyPublicToken || legacyPrivateToken;
            try {
                if (legacyUnifiedToken) {
                    await Promise.all([
                        SetStoredAuthToken("public", legacyUnifiedToken),
                        SetStoredAuthToken("private", legacyUnifiedToken),
                    ]);
                }
            }
            catch (error) {
                console.error("Failed to migrate legacy auth token:", error);
            }
            finally {
                localStorage.removeItem(LEGACY_PUBLIC_AUTH_TOKEN_KEY);
                localStorage.removeItem(LEGACY_PRIVATE_AUTH_TOKEN_KEY);
            }
            try {
                const [storedPublicToken, storedPrivateToken] = await Promise.all([
                    GetStoredAuthToken("public"),
                    GetStoredAuthToken("private"),
                ]);
                if (!active) {
                    return;
                }
                const unifiedToken = storedPublicToken || storedPrivateToken || "";
                sessionAuthToken = unifiedToken;
                setSharedAuthToken(unifiedToken);
                if (unifiedToken && (storedPublicToken !== unifiedToken || storedPrivateToken !== unifiedToken)) {
                    await Promise.all([
                        SetStoredAuthToken("public", unifiedToken),
                        SetStoredAuthToken("private", unifiedToken),
                    ]);
                }
            }
            catch (error) {
                console.error("Failed to load auth token from secure storage:", error);
            }
        };
        void syncStoredTokens();
        return () => {
            active = false;
        };
    }, []);
    const tokenWriteTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const pendingAuthActionRef = useRef<((authToken: string) => void) | null>(null);
    useEffect(() => {
        return () => {
            if (tokenWriteTimerRef.current) {
                clearTimeout(tokenWriteTimerRef.current);
            }
        };
    }, []);
    const handleSharedTokenChange = (value: string) => {
        sessionAuthToken = value;
        setSharedAuthToken(value);
        if (tokenWriteTimerRef.current) {
            clearTimeout(tokenWriteTimerRef.current);
        }
        tokenWriteTimerRef.current = setTimeout(() => {
            void Promise.all([
                SetStoredAuthToken("public", value),
                SetStoredAuthToken("private", value),
            ]).catch((error) => {
                console.error("Failed to store auth token securely:", error);
            });
        }, 400);
    };
    const closeAuthRequiredDialog = (open: boolean) => {
        setShowAuthRequiredDialog(open);
        if (!open) {
            pendingAuthActionRef.current = null;
        }
    };
    const requireAuthToken = (onAuthorized: (authToken: string) => void) => {
        const token = sharedAuthToken.trim();
        if (token) {
            onAuthorized(token);
            return;
        }
        pendingAuthActionRef.current = onAuthorized;
        setAuthPromptToken(sharedAuthToken);
        setShowAuthRequiredDialog(true);
    };
    const handleFetch = () => {
        requireAuthToken((authToken) => {
            onFetch(false, undefined, undefined, undefined, retweets, mode, privateType, authToken, false);
        });
    };
    const handleClipboardPaste = async () => {
        try {
            const clipboardText = (await navigator.clipboard.readText()).trim();
            if (clipboardText) {
                onUsernameChange(clipboardText);
            }
        }
        catch (error) {
            console.error("Failed to read clipboard:", error);
        }
    };
    const handleResume = () => {
        if (onResume) {
            requireAuthToken((authToken) => {
                onResume(authToken, undefined, retweets);
            });
        }
    };
    const handleFetchAll = () => {
        requireAuthToken((authToken) => {
            onFetchAll?.(authToken);
        });
    };
    const handleRetry = (accountId: string) => {
        requireAuthToken((authToken) => {
            onRetryAccount?.(accountId, authToken);
        });
    };
    const handleAuthPromptSubmit = () => {
        const token = authPromptToken.trim();
        if (!token) {
            toast.error("Please enter your auth token");
            return;
        }
        handleSharedTokenChange(token);
        const pendingAction = pendingAuthActionRef.current;
        pendingAuthActionRef.current = null;
        setShowAuthRequiredDialog(false);
        pendingAction?.(token);
    };
    const handleOpenFetchSettings = () => {
        pendingAuthActionRef.current = null;
        setShowAuthRequiredDialog(false);
        setShowSettingsDialog(true);
    };
    const openImportDialog = () => {
        setImportAccountsText(multipleAccounts.map((account) => account.username).join("\n"));
        setShowImportDialog(true);
    };
    const handleImportSubmit = () => {
        const accounts = importAccountsText
            .split(/[\n,;]+/)
            .map((entry) => entry.trim())
            .filter((entry) => entry.length > 0);
        if (accounts.length === 0) {
            toast.error("Please enter at least one account");
            return;
        }
        onImportAccounts?.(accounts);
        setShowImportDialog(false);
    };
    const formatTime = (seconds: number): string => {
        const mins = Math.floor(seconds / 60);
        const secs = seconds % 60;
        return `${mins}:${String(secs).padStart(2, "0")}`;
    };
    const activeCountdown = remainingTime !== null && remainingTime >= 0
        ? formatTime(remainingTime)
        : elapsedTime > 0
            ? formatTime(elapsedTime)
            : null;
    const fetchingLabel = activeCountdown ? `Fetching... (${activeCountdown})` : "Fetching...";
    const authTokenHelpText = mode === "private"
        ? "Use auth token from the account whose bookmarks or likes you want to fetch."
        : "Recommended to use a dummy account, not your main account. Excessive usage may cause suspension.";
    return (<div className="space-y-3">

      <div className="flex justify-center gap-2">
        <div className="flex w-fit gap-0.5 rounded-lg bg-muted p-0.5">
          <button type="button" onClick={() => onFetchTypeChange?.("single")} className={cn("flex cursor-pointer items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition-all", fetchType === "single"
            ? "bg-background text-foreground shadow-sm"
            : "text-muted-foreground hover:text-foreground")}>
            <User className="h-3 w-3"/>
            Single
          </button>
          <button type="button" onClick={() => onFetchTypeChange?.("multiple")} className={cn("flex cursor-pointer items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition-all", fetchType === "multiple"
            ? "bg-background text-foreground shadow-sm"
            : "text-muted-foreground hover:text-foreground")}>
            <Users className="h-3 w-3"/>
            Multiple
          </button>
        </div>

        {fetchType === "single" && (<div className="flex w-fit gap-0.5 rounded-lg bg-muted p-0.5">
            <button type="button" onClick={() => {
                onModeChange?.("public");
            }} className={cn("flex cursor-pointer items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition-all", mode === "public"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground")}>
              <Globe className="h-3 w-3"/>
              Public
            </button>
            <button type="button" onClick={() => {
                onModeChange?.("private");
            }} className={cn("flex cursor-pointer items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition-all", mode === "private"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground")}>
              <Lock className="h-3 w-3"/>
              Private
            </button>
          </div>)}
      </div>

      {fetchType === "multiple" && (<div className="space-y-2">
          <div className="flex items-center gap-2">
            <Label className="text-sm">Account List</Label>
            <Tooltip>
              <TooltipTrigger asChild>
                <Info className="h-4 w-4 cursor-help text-muted-foreground"/>
              </TooltipTrigger>
              <TooltipContent className="max-w-xs">
                <p className="text-sm">
                  One username or URL per line. Commas and semicolons are supported too.
                </p>
              </TooltipContent>
            </Tooltip>
          </div>

          <div className="flex flex-wrap items-center justify-between gap-2">
            <Button variant="outline" onClick={openImportDialog} disabled={isFetchingAll} className="flex w-64 items-center justify-center gap-2">
              <Users className="h-4 w-4"/>
              Input Account List
            </Button>

            <div className="ml-auto flex items-center gap-2">
              {isFetchingAll && (<Button variant="destructive" onClick={onStopAll} className="w-28">
                  <StopCircle className="h-4 w-4"/>
                  Stop All
                </Button>)}

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="outline" size="icon" onClick={() => setShowSettingsDialog(true)} disabled={loading || isFetchingAll} aria-label="Fetch settings">
                    <SlidersHorizontal className="h-4 w-4"/>
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Fetch Settings</TooltipContent>
              </Tooltip>

              <Button variant="default" onClick={handleFetchAll} disabled={multipleAccounts.length === 0 || isFetchingAll} className="flex w-36 items-center justify-center gap-2">
                {isFetchingAll ? (<>
                    <Spinner />
                    Fetching All...
                  </>) : (<>
                    <CloudDownload className="h-4 w-4"/>
                    Fetch All
                  </>)}
              </Button>
            </div>
          </div>
        </div>)}

      {fetchType === "single" && (mode === "public" || isLikesMode) && (<div className="space-y-2">
          <Label htmlFor="username">X/Twitter URL or Username</Label>

          <div className="flex gap-2">
            <div className="relative flex-1">
              <InputWithContext id="username" placeholder={isLikesMode
                ? "your_username or @your_username or https://x.com/your_username"
                : "masteraoko or @masteraoko or https://x.com/masteraoko"} value={username} onChange={(e) => onUsernameChange(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleFetch()} className="pr-8"/>
              {username && (<button type="button" className="absolute top-1/2 right-2 -translate-y-1/2 cursor-pointer text-muted-foreground transition-colors hover:text-foreground" onClick={() => onUsernameChange("")}>
                  <XCircle className="h-4 w-4"/>
                </button>)}
            </div>

            <Button variant="outline" size="icon" className="shrink-0" aria-label="Paste from clipboard" onClick={() => void handleClipboardPaste()}>
              <Clipboard className="h-4 w-4"/>
            </Button>

            <div className="flex items-center gap-2">
              {loading && (<Button variant="destructive" onClick={onStopFetch}>
                  <StopCircle className="h-4 w-4"/>
                  Stop
                </Button>)}

              {!loading && resumeInfo?.canResume && mode === "public" && (<>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button variant="outline" size="icon" onClick={onClearResume}>
                        <Trash2 className="h-4 w-4"/>
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Clear Resume</TooltipContent>
                  </Tooltip>

                  <Button variant="secondary" onClick={handleResume}>
                    <RotateCcw className="h-4 w-4"/>
                    Resume ({resumeInfo.mediaCount.toLocaleString()})
                  </Button>
                </>)}

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="outline" size="icon" onClick={() => setShowSettingsDialog(true)} disabled={loading} aria-label="Fetch settings">
                    <SlidersHorizontal className="h-4 w-4"/>
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Fetch Settings</TooltipContent>
              </Tooltip>

              <Button onClick={handleFetch} disabled={loading}>
                {loading ? (<>
                    <Spinner />
                    {fetchingLabel}
                  </>) : (<>
                    {isLikesMode ? <Heart className="h-4 w-4"/> : <CloudDownload className="h-4 w-4"/>}
                    {isLikesMode ? "Fetch Likes" : "Fetch"}
                  </>)}
              </Button>
            </div>
          </div>
        </div>)}

      <div className="flex items-center gap-2">
        {mode === "private" && fetchType === "single" && (<>
            <button type="button" onClick={() => {
                onModeChange?.("private", "bookmarks");
            }} className={cn("flex cursor-pointer items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium transition-all", privateType === "bookmarks"
                ? "border-primary bg-primary/10 text-primary"
                : "border-border hover:border-primary/50")}>
              <Bookmark className="h-4 w-4"/>
              My Bookmarks
            </button>

            <button type="button" onClick={() => {
                onModeChange?.("private", "likes");
            }} className={cn("flex cursor-pointer items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium transition-all", privateType === "likes"
                ? "border-primary bg-primary/10 text-primary"
                : "border-border hover:border-primary/50")}>
              <Heart className="h-4 w-4"/>
              My Likes
            </button>
          </>)}

        {fetchType === "single" && isBookmarksMode && (<div className="ml-auto flex items-center gap-2">
            {loading && (<Button variant="destructive" onClick={onStopFetch}>
                <StopCircle className="h-4 w-4"/>
                Stop
              </Button>)}

            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="outline" size="icon" onClick={() => setShowSettingsDialog(true)} disabled={loading} aria-label="Fetch settings">
                  <SlidersHorizontal className="h-4 w-4"/>
                </Button>
              </TooltipTrigger>
              <TooltipContent>Fetch Settings</TooltipContent>
            </Tooltip>

            <Button onClick={handleFetch} disabled={loading}>
              {loading ? (<>
                  <Spinner />
                  {fetchingLabel}
                </>) : (<>
                  <Bookmark className="h-4 w-4"/>
                  Fetch Bookmarks
                </>)}
            </Button>
          </div>)}
      </div>

      {fetchType === "multiple" && multipleAccounts.length > 0 && (<div className="rounded-lg border bg-muted/30 p-3">
          <div className="flex items-center gap-6 text-sm">
            <div className="flex items-center gap-2">
              <CheckCircle className="h-4 w-4 text-green-600 dark:text-green-400"/>
              <span className="text-muted-foreground">Completed:</span>
              <span className="font-medium">
                {formatNumberWithComma(multipleAccounts.filter((acc) => acc.status === "completed").length)}
              </span>
            </div>

            <div className="flex items-center gap-2">
              <AlertCircle className="h-4 w-4 text-yellow-600 dark:text-yellow-400"/>
              <span className="text-muted-foreground">Incomplete:</span>
              <span className="font-medium">
                {formatNumberWithComma(multipleAccounts.filter((acc) => acc.status === "incomplete").length)}
              </span>
            </div>

            <div className="flex items-center gap-2">
              <XCircle className="h-4 w-4 text-red-600 dark:text-red-400"/>
              <span className="text-muted-foreground">Failed:</span>
              <span className="font-medium">
                {formatNumberWithComma(multipleAccounts.filter((acc) => acc.status === "failed").length)}
              </span>
            </div>

            <div className="ml-auto flex items-center gap-3">
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground">Total:</span>
                <span className="font-medium">
                  {formatNumberWithComma(multipleAccounts.filter((acc) => acc.status === "completed" || acc.status === "incomplete" || acc.status === "failed").length)}/{formatNumberWithComma(multipleAccounts.length)}
                </span>
              </div>
              {!isFetchingAll && onOpenSavedAccounts && multipleAccounts.some((acc) => acc.status === "completed" || acc.status === "incomplete") && (<Button variant="secondary" size="sm" onClick={onOpenSavedAccounts} className="flex items-center gap-2">
                  <Database className="h-3.5 w-3.5"/>
                  Open Saved Accounts to Download
                </Button>)}
            </div>
          </div>
        </div>)}

      {fetchType === "multiple" && multipleAccounts.length > 0 && (<div className="space-y-2">
          <div className="flex items-center justify-between gap-2">
            <p className="text-sm text-muted-foreground">Accounts to Fetch ({formatNumberWithComma(multipleAccounts.length)})</p>
            <Button variant="outline" size="sm" onClick={onClearMultipleAccounts} disabled={isFetchingAll || isAnyMultipleAccountFetching} className="flex items-center gap-2">
              <Trash2 className="h-4 w-4"/>
              Clear List
            </Button>
          </div>
          <div className="max-h-100 space-y-2 overflow-y-auto">
            {multipleAccounts.map((account) => (<div key={account.id} className="rounded-lg border bg-card p-3">
                <div className="flex items-center gap-3">
                  {account.accountInfo ? (<img src={account.accountInfo.profile_image} alt={account.accountInfo.nick} className="h-10 w-10 rounded-full"/>) : (<div className="flex h-10 w-10 items-center justify-center rounded-full bg-muted">
                      <User className="h-5 w-5 text-muted-foreground"/>
                    </div>)}

                  <div className="min-w-0 flex-1">
                    {account.accountInfo ? (<>
                        <div className="flex items-center gap-2">
                          <span className="font-medium">{account.accountInfo.nick}</span>
                          <span className="text-sm text-muted-foreground">@{account.accountInfo.name}</span>
                        </div>
                        <div className="mt-1 flex items-center gap-2">
                          <span className="text-sm text-muted-foreground">
                            <span className="text-primary">{account.mediaCount.toLocaleString()}</span> items found
                          </span>
                          {account.showDiff && account.previousMediaCount > 0 && account.mediaCount > account.previousMediaCount && (<span className="text-sm font-medium text-green-600 dark:text-green-400">
                              +{account.mediaCount - account.previousMediaCount}
                            </span>)}
                        </div>
                      </>) : (<div className="flex items-center gap-2">
                        <span className="font-medium">@{account.username}</span>
                      </div>)}
                  </div>

                  <div className="flex items-center gap-2">
                    {account.status === "fetching" && (<>
                        <div className="flex w-21.25 items-center gap-1.5 rounded-md border bg-muted/50 px-3 py-1.5 text-sm">
                          <Clock className="h-3.5 w-3.5 shrink-0 text-muted-foreground"/>
                          <span className="font-mono">
                            {account.remainingTime !== null && account.remainingTime >= 0
                        ? formatTime(account.remainingTime)
                        : formatTime(account.elapsedTime)}
                          </span>
                        </div>
                        <Button variant="destructive" onClick={() => onStopAccount?.(account.id)} className="flex items-center gap-2">
                          <StopCircle className="h-4 w-4"/>
                          Stop
                        </Button>
                      </>)}

                    {account.status === "pending" && (<span className="flex items-center gap-1.5 rounded bg-gray-500/20 px-2 py-1 text-xs text-gray-600 dark:text-gray-400">
                        <Hourglass className="h-3 w-3"/>
                        Pending
                      </span>)}

                    {account.status === "completed" && (<span className="flex items-center gap-1.5 rounded bg-green-500/20 px-2 py-1 text-xs text-green-600 dark:text-green-400">
                        <CheckCircle className="h-3 w-3"/>
                        Completed
                      </span>)}

                    {account.status === "incomplete" && (<>
                        <span className="flex items-center gap-1.5 rounded bg-yellow-500/20 px-2 py-1 text-xs text-yellow-600 dark:text-yellow-400">
                          <AlertCircle className="h-3 w-3"/>
                          Incomplete
                        </span>
                        <Button variant="outline" size="sm" onClick={() => handleRetry(account.id)} disabled={isFetchingAll} className="flex items-center gap-2">
                          <RotateCcw className="h-3.5 w-3.5"/>
                          Retry
                        </Button>
                      </>)}

                    {account.status === "failed" && (<>
                        <span className="flex items-center gap-1.5 rounded bg-red-500/20 px-2 py-1 text-xs text-red-600 dark:text-red-400">
                          <XCircle className="h-3 w-3"/>
                          Failed
                        </span>
                        <Button variant="outline" size="sm" onClick={() => handleRetry(account.id)} disabled={isFetchingAll} className="flex items-center gap-2">
                          <RotateCcw className="h-3.5 w-3.5"/>
                          Retry
                        </Button>
                      </>)}

                    <Button variant="outline" size="sm" onClick={() => onRemoveMultipleAccount?.(account.id)} disabled={isFetchingAll || account.status === "fetching"} className="flex items-center gap-2">
                      <Trash2 className="h-3.5 w-3.5"/>
                      Delete
                    </Button>
                  </div>
                </div>
              </div>))}
          </div>
        </div>)}

      {!hasResult && mode === "public" && fetchType === "single" && (<FetchHistory history={history} onSelect={onHistorySelect} onRemove={onHistoryRemove}/>)}

      <Dialog open={showImportDialog} onOpenChange={setShowImportDialog}>
        <DialogContent className="sm:max-w-xl">
          <DialogHeader>
            <DialogTitle>Input Account List</DialogTitle>
            <DialogDescription>
              Enter one username or URL per line. Commas and semicolons also work.
            </DialogDescription>
          </DialogHeader>

          <textarea value={importAccountsText} onChange={(e) => setImportAccountsText(e.target.value)} placeholder={"masteraoko\nxbatchdemo\nhttps://x.com/takomayuyi"} className="border-input placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 min-h-64 w-full rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:ring-[3px]"/>

          <DialogFooter>
            <Button variant="outline" onClick={() => setShowImportDialog(false)}>Cancel</Button>
            <Button onClick={handleImportSubmit}>Import Accounts</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={showAuthRequiredDialog} onOpenChange={closeAuthRequiredDialog}>
        <DialogContent className="sm:max-w-xl">
          <DialogHeader>
            <DialogTitle>Cookie Required</DialogTitle>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="required-auth-token">Auth Token</Label>
              <div className="relative">
                <InputWithContext id="required-auth-token" type={showAuthToken ? "text" : "password"} placeholder="Enter your auth_token cookie value" value={authPromptToken} onChange={(e) => setAuthPromptToken(e.target.value)} onKeyDown={(e) => {
            if (e.key === "Enter") {
                handleAuthPromptSubmit();
            }
        }} className="pr-10"/>
                <button type="button" aria-label={showAuthToken ? "Hide auth token" : "Show auth token"} className="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground" onClick={() => setShowAuthToken((prev) => !prev)}>
                  {showAuthToken ? <EyeOff className="h-4 w-4"/> : <Eye className="h-4 w-4"/>}
                </button>
              </div>
            </div>

            <div className="space-y-3 rounded-lg border bg-muted/30 p-4 text-sm">
              <p>
                Tutorial:
                {" "}
                <a href="https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader#how-to-obtain-auth-token" target="_blank" rel="noreferrer" className="font-medium underline underline-offset-4">
                  afkarxyz/Twitter-X-Media-Batch-Downloader
                </a>
              </p>
              <ol className="list-decimal space-y-1 pl-5 text-muted-foreground">
                <li>
                  Go to
                  {" "}
                  <a href="https://www.x.com/" target="_blank" rel="noreferrer" className="underline underline-offset-4">
                    Twitter&apos;s website
                  </a>
                </li>
                <li>Log into your account</li>
                <li>
                  Open the Developer Tools by pressing
                  {" "}
                  <kbd className="rounded border bg-background px-1.5 py-0.5 font-mono text-xs">F12</kbd>
                </li>
                <li>
                  Navigate to the Application tab, then select Storage {"->"} Cookies
                </li>
                <li>Find and copy the <code>auth_token</code> value</li>
                <li>Do not log out of your account, as a new <code>auth_token</code> will be generated by Twitter</li>
              </ol>
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={handleOpenFetchSettings}>Open Fetch Settings</Button>
            <Button variant="outline" onClick={() => closeAuthRequiredDialog(false)}>Cancel</Button>
            <Button onClick={handleAuthPromptSubmit}>Save and Continue</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={showSettingsDialog} onOpenChange={setShowSettingsDialog}>
        <DialogContent className="sm:max-w-xl max-h-[85vh] flex flex-col overflow-hidden">
          <DialogHeader>
            <DialogTitle>Fetch Settings</DialogTitle>
          </DialogHeader>

          <div className="space-y-4 overflow-y-auto flex-1 min-h-0 -mx-2 px-2">
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Label htmlFor="shared-auth-token" className="text-sm">Auth Token</Label>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Info className="h-4 w-4 cursor-help text-muted-foreground"/>
                  </TooltipTrigger>
                  <TooltipContent className="max-w-sm">
                    <p className="text-sm">{authTokenHelpText}</p>
                  </TooltipContent>
                </Tooltip>
              </div>

              <InputWithContext id="shared-auth-token" type="text" placeholder="Enter your auth_token cookie value" value={sharedAuthToken} onChange={(e) => void handleSharedTokenChange(e.target.value)}/>

            </div>

            {fetchType === "single" && (<div className="flex items-center justify-between gap-4">
              <Label htmlFor="retweets" className="cursor-pointer text-sm">Include Reposts</Label>
              <Checkbox id="retweets" checked={retweets} onCheckedChange={(checked) => {
                const value = checked as boolean;
                updateSettings({ includeRetweets: value });
                setRetweets(value);
            }}/>
            </div>)}

            <div className="space-y-2">
              <Label className="text-sm">Include Media</Label>
              <div className="grid grid-cols-4 gap-2">
                <button type="button" className={pillClass(includePhotos)} onClick={() => { updateSettings({ includePhotos: !includePhotos }); setIncludePhotos((v) => !v); }}>Photos</button>
                <button type="button" className={pillClass(includeVideos)} onClick={() => { updateSettings({ includeVideos: !includeVideos }); setIncludeVideos((v) => !v); }}>Videos</button>
                <button type="button" className={pillClass(includeGifs)} onClick={() => { updateSettings({ includeGifs: !includeGifs }); setIncludeGifs((v) => !v); }}>GIFs</button>
                <button type="button" className={pillClass(includeText)} onClick={() => { updateSettings({ includeText: !includeText }); setIncludeText((v) => !v); }}>Text</button>
              </div>
            </div>

            <div className="space-y-2">
              <Label className="text-sm">Video Quality</Label>
              <div className="grid grid-cols-5 gap-2">
                {VIDEO_QUALITIES.map((q) => (<button key={q.value} type="button" className={pillClass(videoQuality === q.value)} onClick={() => { updateSettings({ videoQuality: q.value }); setVideoQuality(q.value); }}>{q.label}</button>))}
              </div>
            </div>

            <div className="space-y-2">
              <Label className="text-sm">Image Size</Label>
              <div className="grid grid-cols-4 gap-2">
                {IMAGE_SIZES.map((s) => (<button key={s.value} type="button" className={pillClass(imageSize === s.value)} onClick={() => { updateSettings({ imageSize: s.value }); setImageSize(s.value); }}>{s.label}</button>))}
              </div>
            </div>

            <div className="space-y-2">
              <Label className="text-sm">Avatar Size</Label>
              <div className="grid grid-cols-5 gap-2">
                {AVATAR_SIZES.map((s) => (<button key={s.value} type="button" className={pillClass(avatarSize === s.value)} onClick={() => { updateSettings({ avatarSize: s.value }); setAvatarSize(s.value); }}>{s.label}</button>))}
              </div>
            </div>

            <div className="space-y-2">
              <Label className="text-sm">Banner Size</Label>
              <div className="grid grid-cols-4 gap-2">
                {BANNER_SIZES.map((s) => (<button key={s.value} type="button" className={pillClass(bannerSize === s.value)} onClick={() => { updateSettings({ bannerSize: s.value }); setBannerSize(s.value); }}>{s.label}</button>))}
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button onClick={() => setShowSettingsDialog(false)}>Done</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>);
}
