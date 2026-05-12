import { useEffect, useState } from "react";
import { AlertCircle, Download, Package } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, } from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { DownloadExtractor, IsExtractorInstalled } from "../../wailsjs/go/main/App";
interface DependencySetupDialogProps {
    onInstalled?: () => void;
}
export function DependencySetupDialog({ onInstalled }: DependencySetupDialogProps) {
    const [open, setOpen] = useState(false);
    const [checking, setChecking] = useState(true);
    const [downloading, setDownloading] = useState(false);
    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    useEffect(() => {
        let cancelled = false;
        const checkDependency = async () => {
            try {
                const installed = await IsExtractorInstalled();
                if (!cancelled) {
                    setOpen(!installed);
                    setErrorMessage(null);
                }
            }
            catch {
                if (!cancelled) {
                    setOpen(true);
                    setErrorMessage("Couldn't verify the core dependency. You can install it now.");
                }
            }
            finally {
                if (!cancelled) {
                    setChecking(false);
                }
            }
        };
        checkDependency();
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
            toast.success("Core dependency installed");
            onInstalled?.();
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            setErrorMessage(message || "Failed to download dependency.");
            toast.error("Failed to download dependency");
        }
        finally {
            setDownloading(false);
        }
    };
    if (checking && !open) {
        return null;
    }
    return (<Dialog open={open} onOpenChange={(nextOpen) => !downloading && setOpen(nextOpen)}>
      <DialogContent className="max-w-xl [&>button]:hidden" showCloseButton={false}>
        <DialogHeader>
          <div className="mb-2 flex h-12 w-12 items-center justify-center rounded-2xl border bg-muted/50">
            <Package className="h-6 w-6"/>
          </div>
          <DialogTitle>Download Core Dependency</DialogTitle>
          <DialogDescription>
            This app now uses a prebuilt xtractor binary from GitHub releases. Download it once so fetching can work on this device.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          <div className="rounded-lg border bg-muted/40 p-3 text-sm text-muted-foreground">
            The binary is stored in the app data folder and reused on the next launch. Optional tools like FFmpeg and ExifTool stay available in Settings.
          </div>

          {errorMessage && (<div className="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
              <AlertCircle className="mt-0.5 h-4 w-4 shrink-0"/>
              <p>{errorMessage}</p>
            </div>)}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)} disabled={downloading}>
            Later
          </Button>
          <Button onClick={handleDownload} disabled={downloading}>
            {downloading ? (<>
                <Spinner />
                Downloading...
              </>) : (<>
                <Download className="h-4 w-4"/>
                Download Dependency
              </>)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>);
}
