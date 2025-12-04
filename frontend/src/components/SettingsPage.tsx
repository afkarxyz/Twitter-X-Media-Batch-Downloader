import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { InputWithContext } from "@/components/ui/input-with-context";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { FolderOpen, Save, RotateCcw, Info, Download, Check, Volume2 } from "lucide-react";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { getSettings, getSettingsWithDefaults, saveSettings, resetToDefaultSettings, applyThemeMode, applyFont, FONT_OPTIONS, type Settings as SettingsType, type FontFamily, type GifQuality, type GifResolution } from "@/lib/settings";
import { themes, applyTheme } from "@/lib/themes";
import { SelectFolder, IsFFmpegInstalled, DownloadFFmpeg } from "../../wailsjs/go/main/App";
import { toastWithSound as toast } from "@/lib/toast-with-sound";

export function SettingsPage() {
  const [savedSettings, setSavedSettings] = useState<SettingsType>(getSettings());
  const [tempSettings, setTempSettings] = useState<SettingsType>(savedSettings);
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
    applyThemeMode(tempSettings.themeMode);
    applyTheme(tempSettings.theme);
    applyFont(tempSettings.fontFamily);
    setTimeout(() => {
      setIsDark(document.documentElement.classList.contains('dark'));
    }, 0);
  }, [tempSettings.themeMode, tempSettings.theme, tempSettings.fontFamily]);

  useEffect(() => {
    const loadDefaults = async () => {
      if (!savedSettings.downloadPath) {
        const settingsWithDefaults = await getSettingsWithDefaults();
        setSavedSettings(settingsWithDefaults);
        setTempSettings(settingsWithDefaults);
      }
    };
    loadDefaults();
    
    // Check FFmpeg status
    IsFFmpegInstalled().then(setFfmpegInstalled);
  }, []);

  const handleSave = () => {
    saveSettings(tempSettings);
    setSavedSettings(tempSettings);
    toast.success("Settings saved");
  };

  const handleReset = async () => {
    const defaultSettings = await resetToDefaultSettings();
    setTempSettings(defaultSettings);
    setSavedSettings(defaultSettings);
    applyThemeMode(defaultSettings.themeMode);
    applyTheme(defaultSettings.theme);
    applyFont(defaultSettings.fontFamily);
    toast.success("Settings reset to default");
  };

  const handleBrowseFolder = async () => {
    try {
      const selectedPath = await SelectFolder(tempSettings.downloadPath || "");
      if (selectedPath && selectedPath.trim() !== "") {
        setTempSettings((prev) => ({ ...prev, downloadPath: selectedPath }));
      }
    } catch (error) {
      console.error("Error selecting folder:", error);
      toast.error(`Error selecting folder: ${error}`);
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
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Settings</h1>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Left Column */}
        <div className="space-y-4">
          {/* Download Path */}
          <div className="space-y-2">
            <Label htmlFor="download-path">Download Path</Label>
            <div className="flex gap-2">
              <InputWithContext
                id="download-path"
                value={tempSettings.downloadPath}
                onChange={(e) => setTempSettings((prev) => ({ ...prev, downloadPath: e.target.value }))}
                placeholder="C:\Users\YourUsername\Pictures"
              />
              <Button type="button" onClick={handleBrowseFolder} className="gap-1.5">
                <FolderOpen className="h-4 w-4" />
                Browse
              </Button>
            </div>
          </div>

          {/* Theme Mode */}
          <div className="space-y-2">
            <Label htmlFor="theme-mode">Mode</Label>
            <Select
              value={tempSettings.themeMode}
              onValueChange={(value: "auto" | "light" | "dark") => setTempSettings((prev) => ({ ...prev, themeMode: value }))}
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

          {/* Accent */}
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
                          backgroundColor: isDark ? theme.cssVars.dark.primary : theme.cssVars.light.primary
                        }}
                      />
                      {theme.label}
                    </span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* Font */}
          <div className="space-y-2">
            <Label htmlFor="font">Font</Label>
            <Select
              value={tempSettings.fontFamily}
              onValueChange={(value: FontFamily) => setTempSettings((prev) => ({ ...prev, fontFamily: value }))}
            >
              <SelectTrigger id="font">
                <SelectValue placeholder="Select a font" />
              </SelectTrigger>
              <SelectContent>
                {FONT_OPTIONS.map((font) => (
                  <SelectItem key={font.value} value={font.value}>
                    <span style={{ fontFamily: font.fontFamily }}>{font.label}</span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* Sound Effects */}
          <div className="flex items-center gap-3 pt-2">
            <Volume2 className="h-4 w-4 text-muted-foreground" />
            <Label htmlFor="sfx-enabled" className="cursor-pointer text-sm">Sound Effects</Label>
            <Switch
              id="sfx-enabled"
              checked={tempSettings.sfxEnabled}
              onCheckedChange={(checked) => setTempSettings(prev => ({ ...prev, sfxEnabled: checked }))}
            />
          </div>
        </div>

        {/* Right Column */}
        <div className="space-y-4">
          {/* GIF Conversion */}
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

          {/* GIF Quality - only show if FFmpeg installed */}
          {ffmpegInstalled && (
            <>
              <div className="space-y-2">
                <Label htmlFor="gif-quality">GIF Quality</Label>
                <Select
                  value={tempSettings.gifQuality}
                  onValueChange={(value: GifQuality) => {
                    setTempSettings((prev) => ({
                      ...prev,
                      gifQuality: value,
                      // Auto-select "original" when switching to "better"
                      gifResolution: value === "better" ? "original" : prev.gifResolution,
                    }));
                  }}
                >
                  <SelectTrigger id="gif-quality">
                    <SelectValue placeholder="Select quality" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="fast">Fast (Simple conversion)</SelectItem>
                    <SelectItem value="better">Better (Optimized palette)</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              {/* GIF Resolution - only show if Better quality selected */}
              {tempSettings.gifQuality === "better" && (
                <div className="space-y-2">
                  <Label htmlFor="gif-resolution">GIF Resolution</Label>
                  <Select
                    value={tempSettings.gifResolution}
                    onValueChange={(value: GifResolution) => setTempSettings((prev) => ({ ...prev, gifResolution: value }))}
                  >
                    <SelectTrigger id="gif-resolution">
                      <SelectValue placeholder="Select resolution" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="original">Original (No scaling, 15 fps)</SelectItem>
                      <SelectItem value="high">High (800px, 15 fps)</SelectItem>
                      <SelectItem value="medium">Medium (600px, 10 fps)</SelectItem>
                      <SelectItem value="low">Low (400px, 8 fps)</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {/* Actions */}
      <div className="flex gap-2 justify-between pt-4 border-t">
        <Button variant="outline" onClick={handleReset} className="gap-1.5">
          <RotateCcw className="h-4 w-4" />
          Reset to Default
        </Button>
        <Button onClick={handleSave} className="gap-1.5">
          <Save className="h-4 w-4" />
          Save Changes
        </Button>
      </div>
    </div>
  );
}
