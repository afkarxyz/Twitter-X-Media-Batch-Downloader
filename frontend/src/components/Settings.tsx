import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { InputWithContext } from "@/components/ui/input-with-context";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogFooter,
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
import { Settings as SettingsIcon, FolderOpen, Save, RotateCcw, X, Info, Download, Check, Volume2 } from "lucide-react";
import { Spinner } from "@/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Switch } from "@/components/ui/switch";
import { getSettings, getSettingsWithDefaults, saveSettings, resetToDefaultSettings, applyThemeMode, type Settings as SettingsType } from "@/lib/settings";
import { themes, applyTheme } from "@/lib/themes";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
import { SelectFolder, IsFFmpegInstalled, DownloadFFmpeg } from "../../wailsjs/go/main/App";

export function Settings() {
  const [open, setOpen] = useState(false);
  const [savedSettings, setSavedSettings] = useState<SettingsType>(getSettings());
  const [tempSettings, setTempSettings] = useState<SettingsType>(savedSettings);
  const [, setIsLoadingDefaults] = useState(false);
  const [isDark, setIsDark] = useState(document.documentElement.classList.contains('dark'));
  const [ffmpegInstalled, setFfmpegInstalled] = useState(false);
  const [downloadingFFmpeg, setDownloadingFFmpeg] = useState(false);

  useEffect(() => {
    applyThemeMode(savedSettings.themeMode);
    applyTheme(savedSettings.theme);

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      if (savedSettings.themeMode === "auto") {
        applyThemeMode("auto");
        applyTheme(savedSettings.theme);
      }
    };

    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, [savedSettings.themeMode, savedSettings.theme]);

  useEffect(() => {
    if (open) {
      applyThemeMode(tempSettings.themeMode);
      applyTheme(tempSettings.theme);
      
      setTimeout(() => {
        setIsDark(document.documentElement.classList.contains('dark'));
      }, 0);

      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handleChange = () => {
        if (tempSettings.themeMode === "auto") {
          applyThemeMode("auto");
          applyTheme(tempSettings.theme);
          setTimeout(() => {
            setIsDark(document.documentElement.classList.contains('dark'));
          }, 0);
        }
      };

      mediaQuery.addEventListener("change", handleChange);
      return () => mediaQuery.removeEventListener("change", handleChange);
    }
  }, [open, tempSettings.themeMode, tempSettings.theme]);

  useEffect(() => {
    const loadDefaults = async () => {
      if (!savedSettings.downloadPath) {
        setIsLoadingDefaults(true);
        const settingsWithDefaults = await getSettingsWithDefaults();
        setSavedSettings(settingsWithDefaults);
        setTempSettings(settingsWithDefaults);
        setIsLoadingDefaults(false);
      }
    };
    loadDefaults();
    
    // Check FFmpeg status
    IsFFmpegInstalled().then(setFfmpegInstalled);
  }, []);

  useEffect(() => {
    if (open) {
      setTempSettings(savedSettings);
    }
  }, [open, savedSettings]);

  const handleSave = () => {
    saveSettings(tempSettings);
    setSavedSettings(tempSettings);
    setOpen(false);
  };

  const handleReset = async () => {
    const defaultSettings = await resetToDefaultSettings();
    setTempSettings(defaultSettings);
    setSavedSettings(defaultSettings);
    applyThemeMode(defaultSettings.themeMode);
    applyTheme(defaultSettings.theme);
  };

  const handleCancel = () => {
    applyThemeMode(savedSettings.themeMode);
    applyTheme(savedSettings.theme);
    setTempSettings(savedSettings);
    setOpen(false);
  };

  const handleOpenChange = (newOpen: boolean) => {
    if (!newOpen) {
      applyThemeMode(savedSettings.themeMode);
      applyTheme(savedSettings.theme);
      setTempSettings(savedSettings);
    }
    setOpen(newOpen);
  };

  const handleBrowseFolder = async () => {
    try {
      const selectedPath = await SelectFolder(tempSettings.downloadPath || "");
      if (selectedPath && selectedPath.trim() !== "") {
        setTempSettings((prev) => ({ ...prev, downloadPath: selectedPath }));
      }
    } catch (error) {
      console.error("Error selecting folder:", error);
    }
  };

  const handleDownloadFFmpeg = async () => {
    setDownloadingFFmpeg(true);
    try {
      await DownloadFFmpeg();
      setFfmpegInstalled(true);
      toast.success("FFmpeg downloaded successfully");
    } catch (error) {
      toast.error("Failed to download FFmpeg");
      console.error("Error downloading FFmpeg:", error);
    } finally {
      setDownloadingFFmpeg(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogTrigger asChild>
        <Button variant="outline" size="icon">
          <SettingsIcon className="h-5 w-5" />
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-[480px] flex flex-col p-6 [&>button]:hidden" aria-describedby={undefined}>
        <div className="absolute right-4 top-4">
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 opacity-70 hover:opacity-100"
            onClick={handleCancel}
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
        <DialogTitle className="text-sm font-medium">Settings</DialogTitle>

        {/* 2 Column Grid Layout */}
        <div className="grid grid-cols-2 gap-x-6 gap-y-4 py-2">
          {/* LEFT COLUMN */}
          {/* Download Path */}
          <div className="space-y-2 col-span-2">
            <Label htmlFor="download-path">Download Path</Label>
            <div className="flex gap-2">
              <InputWithContext
                id="download-path"
                value={tempSettings.downloadPath}
                onChange={(e) => setTempSettings((prev) => ({ ...prev, downloadPath: e.target.value }))}
                placeholder="C:\Users\YourUsername\Pictures"
              />
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button type="button" onClick={handleBrowseFolder} size="icon">
                    <FolderOpen className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Browse Folder</TooltipContent>
              </Tooltip>
            </div>
          </div>

          {/* LEFT: Mode */}
          <div className="space-y-2">
            <Label htmlFor="theme-mode">Mode</Label>
            <Select 
              value={tempSettings.themeMode} 
              onValueChange={(value) => setTempSettings((prev) => ({ ...prev, themeMode: value as any }))}
            >
              <SelectTrigger id="theme-mode">
                <SelectValue placeholder="Select theme mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="auto">Auto</SelectItem>
                <SelectItem value="light">Light</SelectItem>
                <SelectItem value="dark">Dark</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* RIGHT: GIF Conversion */}
          <div className="space-y-2">
            <Label className="flex items-center gap-2">
              GIF Conversion
              <Tooltip>
                <TooltipTrigger asChild>
                  <Info className="h-3.5 w-3.5 text-muted-foreground cursor-help" />
                </TooltipTrigger>
                <TooltipContent side="top">
                  <p>FFmpeg is required to convert Twitter's MP4 to actual GIF format</p>
                </TooltipContent>
              </Tooltip>
            </Label>
            <div className="h-9 flex items-center">
              {ffmpegInstalled ? (
                <div className="flex items-center gap-2 text-sm text-green-600 dark:text-green-400">
                  <Check className="h-4 w-4" />
                  Installed
                </div>
              ) : (
                <Button
                  variant="outline"
                  size="sm"
                  className="h-9"
                  onClick={handleDownloadFFmpeg}
                  disabled={downloadingFFmpeg}
                >
                  {downloadingFFmpeg ? (
                    <>
                      <Spinner />
                      Downloading...
                    </>
                  ) : (
                    <>
                      <Download className="h-4 w-4" />
                      Download FFmpeg
                    </>
                  )}
                </Button>
              )}
            </div>
          </div>

          {/* LEFT: Accent */}
          <div className="space-y-2">
            <Label htmlFor="theme">Accent</Label>
            <Select 
              value={tempSettings.theme} 
              onValueChange={(value) => setTempSettings((prev) => ({ ...prev, theme: value }))}
            >
              <SelectTrigger id="theme">
                <SelectValue placeholder="Select a theme" />
              </SelectTrigger>
              <SelectContent>
                {themes.map((theme) => (
                  <SelectItem key={theme.name} value={theme.name}>
                    <span className="flex items-center gap-2">
                      <span 
                        className="w-3 h-3 rounded-full border border-border" 
                        style={{ 
                          backgroundColor: isDark
                            ? theme.cssVars.dark.primary 
                            : theme.cssVars.light.primary 
                        }}
                      />
                      {theme.label}
                    </span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* RIGHT: Sound Effects */}
          <div className="space-y-2">
            <Label htmlFor="sfx-enabled" className="flex items-center gap-2">
              <Volume2 className="h-4 w-4 text-muted-foreground" />
              Sound Effects
            </Label>
            <div className="h-9 flex items-center">
              <Switch
                id="sfx-enabled"
                checked={tempSettings.sfxEnabled}
                onCheckedChange={(checked) => setTempSettings(prev => ({ ...prev, sfxEnabled: checked }))}
              />
            </div>
          </div>
        </div>
        <DialogFooter className="gap-2 sm:justify-between">
          <Button variant="outline" onClick={handleReset} className="gap-1.5">
            <RotateCcw className="h-4 w-4" />
            Reset to Default
          </Button>
          <div className="flex gap-2">
            <Button variant="outline" onClick={handleCancel}>
              Cancel
            </Button>
            <Button onClick={handleSave} className="gap-1.5">
              <Save className="h-4 w-4" />
              Save Changes
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
