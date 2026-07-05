import { useEffect, useState } from "react";
import { AlertCircle, Download, Package } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, } from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { compareVersionNumbers } from "@/lib/version";
import { Quit } from "../../wailsjs/runtime/runtime";
import { DownloadExtractor, GetExtractorVersionStatus } from "../../wailsjs/go/main/App";
interface DependencySetupDialogProps {
    onInstalled?: () => void;
}
interface ExtractorVersionStatus {
    installed: boolean;
    installed_version?: string;
    latest_version?: string;
}
export function DependencySetupDialog({ onInstalled }: DependencySetupDialogProps) {
    const [open, setOpen] = useState(false);
    const [checking, setChecking] = useState(true);
    const [downloading, setDownloading] = useState(false);
    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    const [updateRequired, setUpdateRequired] = useState(false);
    const [installedVersion, setInstalledVersion] = useState<string | null>(null);
    const [releaseVersion, setReleaseVersion] = useState<string | null>(null);
    const [releaseStatusMessage, setReleaseStatusMessage] = useState("Checking latest GitHub release...");
    useEffect(() => {
        let cancelled = false;
        const checkDependency = async () => {
            try {
                const status = await GetExtractorVersionStatus() as ExtractorVersionStatus;
                const nextInstalledVersion = status.installed_version?.trim() || null;
                const nextLatestVersion = status.latest_version?.trim() || null;
                const nextUpdateRequired = !!(status.installed && nextLatestVersion && (!nextInstalledVersion || compareVersionNumbers(nextLatestVersion, nextInstalledVersion) > 0));
                if (!cancelled) {
                    setUpdateRequired(nextUpdateRequired);
                    setInstalledVersion(nextInstalledVersion);
                    setReleaseVersion(nextLatestVersion);
                    setReleaseStatusMessage(nextLatestVersion ? `Latest GitHub release: ${nextLatestVersion}` : "Latest GitHub release unavailable.");
                    setOpen(!status.installed || nextUpdateRequired);
                    setErrorMessage(null);
                }
            }
            catch {
                if (!cancelled) {
                    setOpen(true);
                    setErrorMessage("Couldn't verify Xtractor. Download it to continue.");
                }
            }
            finally {
                if (!cancelled) {
                    setChecking(false);
                }
            }
        };
        void checkDependency();
        return () => {
            cancelled = true;
        };
    }, []);
    const handleDownload = async () => {
        setDownloading(true);
        setErrorMessage(null);
        try {
            await DownloadExtractor();
            setOpen(false);
            toast.success(updateRequired ? "Xtractor updated" : "Xtractor installed");
            onInstalled?.();
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            setErrorMessage(message || "Failed to download Xtractor.");
            toast.error("Failed to download Xtractor");
        }
        finally {
            setDownloading(false);
        }
    };
    const handleExit = () => {
        if (!downloading) {
            void Quit();
        }
    };
    if (checking && !open) {
        return null;
    }
    return (<Dialog open={open} onOpenChange={(nextOpen) => {
            if (nextOpen) {
                setOpen(true);
            }
        }}>
      <DialogContent className="max-w-xl [&>button]:hidden" showCloseButton={false} onEscapeKeyDown={(event) => event.preventDefault()} onInteractOutside={(event) => event.preventDefault()}>
        <DialogHeader>
          <div className="mb-2 flex h-12 w-12 items-center justify-center rounded-2xl border bg-muted/50">
            <Package className="h-6 w-6"/>
          </div>
          <DialogTitle>{updateRequired ? "Update Xtractor" : "Download Xtractor"}</DialogTitle>
          <DialogDescription>
            {updateRequired ? "A newer Xtractor binary is available or the installed version could not be verified." : "Xtractor is required before you can use this app."}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          {updateRequired && releaseVersion && (<p className="text-sm text-amber-600 dark:text-amber-400">
              Installed: {installedVersion || "unknown"} - Latest: {releaseVersion}
            </p>)}
          <p className="text-sm text-muted-foreground">{releaseVersion ? `Latest GitHub release: ${releaseVersion}` : releaseStatusMessage}</p>

          {errorMessage && (<div className="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
              <AlertCircle className="mt-0.5 h-4 w-4 shrink-0"/>
              <p>{errorMessage}</p>
            </div>)}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={handleExit} disabled={downloading}>
            Exit App
          </Button>
          <Button onClick={handleDownload} disabled={downloading}>
            {downloading ? (<>
                <Spinner />
                {updateRequired ? "Updating Xtractor..." : "Downloading Xtractor..."}
              </>) : (<>
                <Download className="h-4 w-4"/>
                {updateRequired ? "Update Xtractor" : "Download Xtractor"}
              </>)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>);
}
