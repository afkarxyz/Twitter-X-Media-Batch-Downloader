import { useEffect, useState } from "react";
import { AlertCircle, Download, Package } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, } from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { Quit } from "../../wailsjs/runtime/runtime";
import { DownloadExtractor, IsExtractorInstalled } from "../../wailsjs/go/main/App";

interface DependencySetupDialogProps {
    onInstalled?: () => void;
}

interface GithubReleaseResponse {
    tag_name?: string;
}

const extractorReleaseAPIURL = "https://api.github.com/repos/afkarxyz/xtractor-binaries/releases/latest";

export function DependencySetupDialog({ onInstalled }: DependencySetupDialogProps) {
    const [open, setOpen] = useState(false);
    const [checking, setChecking] = useState(true);
    const [downloading, setDownloading] = useState(false);
    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    const [releaseVersion, setReleaseVersion] = useState<string | null>(null);
    const [releaseStatusMessage, setReleaseStatusMessage] = useState("Checking latest GitHub release...");

    useEffect(() => {
        let cancelled = false;
        const abortController = new AbortController();

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
                    setErrorMessage("Couldn't verify Xtractor. Download it to continue.");
                }
            }
            finally {
                if (!cancelled) {
                    setChecking(false);
                }
            }
        };

        const loadReleaseInfo = async () => {
            try {
                const response = await fetch(extractorReleaseAPIURL, {
                    headers: {
                        Accept: "application/vnd.github+json",
                    },
                    signal: abortController.signal,
                });

                if (!response.ok) {
                    throw new Error(`GitHub release status ${response.status}`);
                }

                const release = (await response.json()) as GithubReleaseResponse;
                const version = release.tag_name?.trim();

                if (!version) {
                    throw new Error("Missing GitHub release tag");
                }

                if (!cancelled) {
                    setReleaseVersion(version);
                    setReleaseStatusMessage(`Latest GitHub release: ${version}`);
                }
            }
            catch {
                if (!cancelled && !abortController.signal.aborted) {
                    setReleaseVersion(null);
                    setReleaseStatusMessage("Latest GitHub release unavailable.");
                }
            }
        };

        void checkDependency();
        void loadReleaseInfo();

        return () => {
            cancelled = true;
            abortController.abort();
        };
    }, []);

    const handleDownload = async () => {
        setDownloading(true);
        setErrorMessage(null);
        try {
            await DownloadExtractor();
            setOpen(false);
            toast.success("Xtractor installed");
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
          <DialogTitle>Download Xtractor</DialogTitle>
          <DialogDescription>Xtractor is required before you can use this app.</DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
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
                Downloading Xtractor...
              </>) : (<>
                <Download className="h-4 w-4"/>
                Download Xtractor
              </>)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>);
}
