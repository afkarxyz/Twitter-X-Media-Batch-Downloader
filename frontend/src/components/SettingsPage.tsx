import { useEffect, useState, useCallback, useRef } from "react";
import { flushSync } from "react-dom";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { InputWithContext } from "@/components/ui/input-with-context";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue, } from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Check, CircleQuestionMark, Download, ExternalLink, FolderOpen, MonitorCog, PackageSearch, Plus, RotateCcw, Save, Trash2, FileSignature } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, } from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { getSettings, getSettingsWithDefaults, saveSettings, resetToDefaultSettings, applyThemeMode, applyFont, getFontOptions, parseGoogleFontUrl, loadGoogleFontUrl, loadCustomFonts, saveCustomFonts, DEFAULT_FILENAME_TEMPLATE, FILENAME_TEMPLATE_VARIABLES, renderFilenameTemplate, SAMPLE_FILENAME_DATA, DEFAULT_FOLDER_TEMPLATE, FOLDER_TEMPLATE_VARIABLES, renderFolderTemplate, SAMPLE_FOLDER_DATA, type Settings as SettingsType, type FontFamily, type GifQuality, type GifResolution, type CustomFontFamily } from "@/lib/settings";
import { FormatEditor } from "@/components/FormatEditor";
import { getCachedDependencyStatus, setCachedDependencyStatus } from "@/lib/runtime-cache";
import { baseColors, getThemesForBaseColor, normalizeThemeName, applyTheme, type BaseColorName } from "@/lib/themes";
import { compareVersionNumbers } from "@/lib/version";
import { SelectFolder, IsExtractorInstalled, DownloadExtractor, IsFFmpegInstalled, DownloadFFmpeg, IsExifToolInstalled, DownloadExifTool } from "../../wailsjs/go/main/App";
import { toastWithSound as toast } from "@/lib/toast-with-sound";
interface DependencyVersionStatus {
    installed: boolean;
    installed_version?: string;
    latest_version?: string;
}
type DependencyVersionMethod = "GetExtractorVersionStatus" | "GetFFmpegVersionStatus" | "GetExifToolVersionStatus";
type SettingsTab = "general" | "downloads" | "naming" | "dependencies";
const THEME_PREVIEW_DEBOUNCE_MS = 50;
interface SettingsPageProps {
    onUnsavedChangesChange?: (hasUnsavedChanges: boolean) => void;
    onResetRequest?: (resetFn: () => void) => void;
}
function getDependencyVersionStatus(methodName: DependencyVersionMethod) {
    const app = (window as Window & {
        go?: {
            main?: {
                App?: Partial<Record<DependencyVersionMethod, () => Promise<DependencyVersionStatus>>>;
            };
        };
    }).go?.main?.App;
    const method = app?.[methodName];
    return method ? method() : Promise.resolve(null);
}
function hasNewDependencyVersion(installedVersion: string | null, latestVersion: string | null) {
    if (!installedVersion || !latestVersion) {
        return false;
    }
    return compareVersionNumbers(latestVersion, installedVersion) > 0;
}
function buildDependencyVersionText(name: string, installed: boolean, installedVersion: string | null, latestVersion: string | null, updateAvailable: boolean) {
    if (installed) {
        if (!installedVersion) {
            return `${name} installed`;
        }
        return updateAvailable && latestVersion
            ? `${name} ${installedVersion} (New Version Available: ${latestVersion})`
            : `${name} ${installedVersion}`;
    }
    return latestVersion ? `Latest ${name}: ${latestVersion}` : null;
}
export function SettingsPage({ onUnsavedChangesChange, onResetRequest }: SettingsPageProps) {
    const cachedExtractorStatus = getCachedDependencyStatus("extractor");
    const cachedFfmpegStatus = getCachedDependencyStatus("ffmpeg");
    const cachedExiftoolStatus = getCachedDependencyStatus("exiftool");
    const [savedSettings, setSavedSettings] = useState<SettingsType>(getSettings());
    const [tempSettings, setTempSettings] = useState<SettingsType>(savedSettings);
    const [isDark, setIsDark] = useState(document.documentElement.classList.contains("dark"));
    const [activeTab, setActiveTab] = useState<SettingsTab>("general");
    const [showAddFontDialog, setShowAddFontDialog] = useState(false);
    const [addFontUrl, setAddFontUrl] = useState("");
    const [extractorInstalled, setExtractorInstalled] = useState(cachedExtractorStatus.installed ?? false);
    const [extractorInstalledVersion, setExtractorInstalledVersion] = useState<string | null>(cachedExtractorStatus.installedVersion);
    const [extractorLatestVersion, setExtractorLatestVersion] = useState<string | null>(cachedExtractorStatus.latestVersion);
    const [downloadingExtractor, setDownloadingExtractor] = useState(false);
    const [ffmpegInstalled, setFfmpegInstalled] = useState(cachedFfmpegStatus.installed ?? false);
    const [ffmpegInstalledVersion, setFfmpegInstalledVersion] = useState<string | null>(cachedFfmpegStatus.installedVersion);
    const [ffmpegLatestVersion, setFfmpegLatestVersion] = useState<string | null>(cachedFfmpegStatus.latestVersion);
    const [downloadingFFmpeg, setDownloadingFFmpeg] = useState(false);
    const [exiftoolInstalled, setExiftoolInstalled] = useState(cachedExiftoolStatus.installed ?? false);
    const [exiftoolInstalledVersion, setExiftoolInstalledVersion] = useState<string | null>(cachedExiftoolStatus.installedVersion);
    const [exiftoolLatestVersion, setExiftoolLatestVersion] = useState<string | null>(cachedExiftoolStatus.latestVersion);
    const [downloadingExifTool, setDownloadingExifTool] = useState(false);
    const [showResetConfirm, setShowResetConfirm] = useState(false);
    const extractorUpdateAvailable = hasNewDependencyVersion(extractorInstalledVersion, extractorLatestVersion);
    const ffmpegUpdateAvailable = hasNewDependencyVersion(ffmpegInstalledVersion, ffmpegLatestVersion);
    const exiftoolUpdateAvailable = hasNewDependencyVersion(exiftoolInstalledVersion, exiftoolLatestVersion);
    const parsedAddFont = parseGoogleFontUrl(addFontUrl);
    const fontOptions = getFontOptions(tempSettings.customFonts);
    const availableThemes = getThemesForBaseColor(tempSettings.baseColor);
    const hasUnsavedChanges = JSON.stringify(savedSettings) !== JSON.stringify(tempSettings);
    const themePreviewTimerRef = useRef<number | null>(null);
    const selectedThemeConfigRef = useRef({
        baseColor: tempSettings.baseColor,
        theme: tempSettings.theme,
    });
    const cancelThemePreview = useCallback(() => {
        if (themePreviewTimerRef.current !== null) {
            window.clearTimeout(themePreviewTimerRef.current);
            themePreviewTimerRef.current = null;
        }
    }, []);
    const previewThemeConfig = useCallback((themeName: SettingsType["theme"], baseColorName: BaseColorName) => {
        cancelThemePreview();
        themePreviewTimerRef.current = window.setTimeout(() => {
            themePreviewTimerRef.current = null;
            applyTheme(themeName, baseColorName);
        }, THEME_PREVIEW_DEBOUNCE_MS);
    }, [cancelThemePreview]);
    const previewTheme = useCallback((themeName: SettingsType["theme"]) => {
        previewThemeConfig(themeName, selectedThemeConfigRef.current.baseColor);
    }, [previewThemeConfig]);
    const previewBaseColor = useCallback((baseColorName: BaseColorName) => {
        const themeName = normalizeThemeName(selectedThemeConfigRef.current.theme, baseColorName);
        previewThemeConfig(themeName, baseColorName);
    }, [previewThemeConfig]);
    const restoreSelectedTheme = useCallback(() => {
        cancelThemePreview();
        const selectedTheme = selectedThemeConfigRef.current;
        applyTheme(selectedTheme.theme, selectedTheme.baseColor);
    }, [cancelThemePreview]);
    const handleThemeChange = useCallback((themeName: SettingsType["theme"]) => {
        cancelThemePreview();
        const baseColorName = selectedThemeConfigRef.current.baseColor;
        selectedThemeConfigRef.current = { baseColor: baseColorName, theme: themeName };
        applyTheme(themeName, baseColorName);
        setTempSettings((prev) => ({ ...prev, theme: themeName }));
    }, [cancelThemePreview]);
    const handleBaseColorChange = useCallback((baseColorName: BaseColorName) => {
        cancelThemePreview();
        const themeName = normalizeThemeName(selectedThemeConfigRef.current.theme, baseColorName);
        selectedThemeConfigRef.current = { baseColor: baseColorName, theme: themeName };
        applyTheme(themeName, baseColorName);
        setTempSettings((prev) => ({ ...prev, baseColor: baseColorName, theme: themeName }));
    }, [cancelThemePreview]);
    const resetToSaved = useCallback(() => {
        cancelThemePreview();
        const freshSavedSettings = getSettings();
        selectedThemeConfigRef.current = {
            baseColor: freshSavedSettings.baseColor,
            theme: freshSavedSettings.theme,
        };
        flushSync(() => {
            setTempSettings(freshSavedSettings);
            setIsDark(document.documentElement.classList.contains("dark"));
        });
    }, [cancelThemePreview]);
    useEffect(() => {
        selectedThemeConfigRef.current = {
            baseColor: tempSettings.baseColor,
            theme: tempSettings.theme,
        };
    }, [tempSettings.baseColor, tempSettings.theme]);
    useEffect(() => () => {
        cancelThemePreview();
        const persistedSettings = getSettings();
        applyTheme(persistedSettings.theme, persistedSettings.baseColor);
    }, [cancelThemePreview]);
    useEffect(() => {
        if (onResetRequest) {
            onResetRequest(resetToSaved);
        }
    }, [onResetRequest, resetToSaved]);
    useEffect(() => {
        onUnsavedChangesChange?.(hasUnsavedChanges);
    }, [hasUnsavedChanges, onUnsavedChangesChange]);
    const extractorVersionText = extractorInstalled
        ? extractorInstalledVersion
            ? extractorUpdateAvailable && extractorLatestVersion
                ? `${extractorInstalledVersion} (New Version Available: ${extractorLatestVersion})`
                : extractorInstalledVersion
            : "Installed (version unavailable)"
        : extractorLatestVersion
            ? `Latest: ${extractorLatestVersion}`
            : null;
    const ffmpegVersionText = buildDependencyVersionText("FFmpeg", ffmpegInstalled, ffmpegInstalledVersion, ffmpegLatestVersion, ffmpegUpdateAvailable);
    const exiftoolVersionText = buildDependencyVersionText("ExifTool", exiftoolInstalled, exiftoolInstalledVersion, exiftoolLatestVersion, exiftoolUpdateAvailable);
    const loadDependencyStatus = async () => {
        try {
            const [extractorVersionStatus, ffmpegVersionStatus, exiftoolVersionStatus, extractor, ffmpeg, exiftool] = await Promise.all([
                getDependencyVersionStatus("GetExtractorVersionStatus"),
                getDependencyVersionStatus("GetFFmpegVersionStatus"),
                getDependencyVersionStatus("GetExifToolVersionStatus"),
                IsExtractorInstalled(),
                IsFFmpegInstalled(),
                IsExifToolInstalled(),
            ]);
            const nextExtractorInstalled = extractorVersionStatus?.installed ?? extractor;
            const nextExtractorInstalledVersion = extractorVersionStatus?.installed_version?.trim() || null;
            const nextExtractorLatestVersion = extractorVersionStatus?.latest_version?.trim() || null;
            const nextFfmpegInstalled = ffmpegVersionStatus?.installed ?? ffmpeg;
            const nextFfmpegInstalledVersion = ffmpegVersionStatus?.installed_version?.trim() || null;
            const nextFfmpegLatestVersion = ffmpegVersionStatus?.latest_version?.trim() || null;
            const nextExiftoolInstalled = exiftoolVersionStatus?.installed ?? exiftool;
            const nextExiftoolInstalledVersion = exiftoolVersionStatus?.installed_version?.trim() || null;
            const nextExiftoolLatestVersion = exiftoolVersionStatus?.latest_version?.trim() || null;
            setCachedDependencyStatus("extractor", {
                installed: nextExtractorInstalled,
                installedVersion: nextExtractorInstalledVersion,
                latestVersion: nextExtractorLatestVersion,
            });
            setCachedDependencyStatus("ffmpeg", {
                installed: nextFfmpegInstalled,
                installedVersion: nextFfmpegInstalledVersion,
                latestVersion: nextFfmpegLatestVersion,
            });
            setCachedDependencyStatus("exiftool", {
                installed: nextExiftoolInstalled,
                installedVersion: nextExiftoolInstalledVersion,
                latestVersion: nextExiftoolLatestVersion,
            });
            setExtractorInstalled(nextExtractorInstalled);
            setExtractorInstalledVersion(nextExtractorInstalledVersion);
            setExtractorLatestVersion(nextExtractorLatestVersion);
            setFfmpegInstalled(nextFfmpegInstalled);
            setFfmpegInstalledVersion(nextFfmpegInstalledVersion);
            setFfmpegLatestVersion(nextFfmpegLatestVersion);
            setExiftoolInstalled(nextExiftoolInstalled);
            setExiftoolInstalledVersion(nextExiftoolInstalledVersion);
            setExiftoolLatestVersion(nextExiftoolLatestVersion);
        }
        catch (error) {
            console.error("Failed to check dependency status:", error);
        }
    };
    useEffect(() => {
        applyThemeMode(savedSettings.themeMode);
        applyTheme(savedSettings.theme, savedSettings.baseColor);
        const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
        const handleChange = () => {
            if (savedSettings.themeMode === "auto") {
                applyThemeMode("auto");
                applyTheme(savedSettings.theme, savedSettings.baseColor);
            }
        };
        mediaQuery.addEventListener("change", handleChange);
        return () => mediaQuery.removeEventListener("change", handleChange);
    }, [savedSettings.themeMode, savedSettings.baseColor, savedSettings.theme]);
    useEffect(() => {
        applyThemeMode(tempSettings.themeMode);
        applyTheme(tempSettings.theme, tempSettings.baseColor);
        applyFont(tempSettings.fontFamily, tempSettings.customFonts);
        setTimeout(() => {
            setIsDark(document.documentElement.classList.contains("dark"));
        }, 0);
    }, [tempSettings.themeMode, tempSettings.baseColor, tempSettings.theme, tempSettings.fontFamily, tempSettings.customFonts]);
    useEffect(() => {
        if (showAddFontDialog && parsedAddFont) {
            loadGoogleFontUrl(parsedAddFont.url, "twitter-media-add-font-preview");
        }
    }, [showAddFontDialog, parsedAddFont]);
    useEffect(() => {
        const loadDefaults = async () => {
            if (!savedSettings.downloadPath) {
                const settingsWithDefaults = await getSettingsWithDefaults();
                setSavedSettings(settingsWithDefaults);
                setTempSettings(settingsWithDefaults);
            }
        };
        void loadDefaults();
        void loadDependencyStatus();
    }, []);
    useEffect(() => {
        const syncCustomFonts = async () => {
            const customFonts = await loadCustomFonts();
            setSavedSettings((prev) => ({ ...prev, customFonts }));
            setTempSettings((prev) => ({ ...prev, customFonts }));
        };
        void syncCustomFonts();
    }, []);
    const handleSave = () => {
        saveSettings(tempSettings);
        setSavedSettings(tempSettings);
        toast.success("Settings saved");
        onUnsavedChangesChange?.(false);
    };
    const handleReset = async () => {
        const defaultSettings = await resetToDefaultSettings();
        setTempSettings(defaultSettings);
        setSavedSettings(defaultSettings);
        applyThemeMode(defaultSettings.themeMode);
        applyTheme(defaultSettings.theme, defaultSettings.baseColor);
        applyFont(defaultSettings.fontFamily, defaultSettings.customFonts);
        setShowResetConfirm(false);
        toast.success("Settings reset to default");
    };
    const handleBrowseFolder = async () => {
        try {
            const selectedPath = await SelectFolder(tempSettings.downloadPath || "");
            if (selectedPath && selectedPath.trim() !== "") {
                setTempSettings((prev) => ({ ...prev, downloadPath: selectedPath }));
            }
        }
        catch (error) {
            console.error("Error selecting folder:", error);
            toast.error(`Error selecting folder: ${error}`);
        }
    };
    const closeAddFontDialog = () => {
        setShowAddFontDialog(false);
        setAddFontUrl("");
    };
    const handleAddFont = async () => {
        if (!parsedAddFont) {
            toast.error("Enter a valid Google Fonts URL");
            return;
        }
        const existingFonts = tempSettings.customFonts || [];
        const existingIndex = existingFonts.findIndex((font) => font.value === parsedAddFont.value || font.url === parsedAddFont.url);
        const customFonts = existingIndex >= 0
            ? existingFonts.map((font, index) => index === existingIndex ? parsedAddFont : font)
            : [...existingFonts, parsedAddFont];
        const savedCustomFonts = await saveCustomFonts(customFonts);
        setSavedSettings((prev) => ({ ...prev, customFonts: savedCustomFonts }));
        setTempSettings((prev) => ({
            ...prev,
            customFonts: savedCustomFonts,
            fontFamily: parsedAddFont.value,
        }));
        closeAddFontDialog();
        toast.success(`${parsedAddFont.label} added`);
    };
    const handleDeleteCustomFont = async (fontValue: CustomFontFamily) => {
        const customFonts = (tempSettings.customFonts || []).filter((font) => font.value !== fontValue);
        const savedCustomFonts = await saveCustomFonts(customFonts);
        const shouldResetSavedFont = savedSettings.fontFamily === fontValue;
        const shouldResetTempFont = tempSettings.fontFamily === fontValue;
        const nextSavedSettings: SettingsType = {
            ...savedSettings,
            customFonts: savedCustomFonts,
            fontFamily: shouldResetSavedFont ? "google-sans" : savedSettings.fontFamily,
        };
        setSavedSettings(nextSavedSettings);
        setTempSettings((prev) => ({
            ...prev,
            customFonts: savedCustomFonts,
            fontFamily: shouldResetTempFont ? "google-sans" : prev.fontFamily,
        }));
        if (shouldResetSavedFont) {
            saveSettings(nextSavedSettings);
        }
        toast.success("Font deleted");
    };
    const handleDownloadExtractor = async () => {
        setDownloadingExtractor(true);
        try {
            const successMessage = extractorInstalled
                ? extractorUpdateAvailable
                    ? "Xtractor updated successfully"
                    : "Xtractor reinstalled successfully"
                : "Xtractor downloaded successfully";
            await DownloadExtractor();
            await loadDependencyStatus();
            toast.success(successMessage);
        }
        catch (error) {
            toast.error("Failed to download xtractor");
            console.error("Error downloading extractor:", error);
        }
        finally {
            setDownloadingExtractor(false);
        }
    };
    const handleDownloadFFmpeg = async () => {
        setDownloadingFFmpeg(true);
        try {
            const successMessage = ffmpegInstalled
                ? ffmpegUpdateAvailable
                    ? "FFmpeg updated successfully"
                    : "FFmpeg reinstalled successfully"
                : "FFmpeg downloaded successfully";
            await DownloadFFmpeg();
            await loadDependencyStatus();
            toast.success(successMessage);
        }
        catch (error) {
            toast.error("Failed to download FFmpeg");
            console.error("Error downloading FFmpeg:", error);
        }
        finally {
            setDownloadingFFmpeg(false);
        }
    };
    const handleDownloadExifTool = async () => {
        setDownloadingExifTool(true);
        try {
            const successMessage = exiftoolInstalled
                ? exiftoolUpdateAvailable
                    ? "ExifTool updated successfully"
                    : "ExifTool reinstalled successfully"
                : "ExifTool downloaded successfully";
            await DownloadExifTool();
            await loadDependencyStatus();
            toast.success(successMessage);
        }
        catch (error) {
            toast.error("Failed to download ExifTool");
            console.error("Error downloading ExifTool:", error);
        }
        finally {
            setDownloadingExifTool(false);
        }
    };
    return (<div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Settings</h1>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setShowResetConfirm(true)} className="gap-1.5">
            <RotateCcw className="h-4 w-4"/>
            Reset to Default
          </Button>
          <Button onClick={handleSave} className="gap-1.5">
            <Save className="h-4 w-4"/>
            Save Changes
          </Button>
        </div>
      </div>

      <div className="flex gap-2 border-b">
        <Button variant={activeTab === "general" ? "default" : "ghost"} size="sm" onClick={() => setActiveTab("general")} className="rounded-b-none gap-2">
          <MonitorCog className="h-4 w-4"/>
          General
        </Button>
        <Button variant={activeTab === "downloads" ? "default" : "ghost"} size="sm" onClick={() => setActiveTab("downloads")} className="rounded-b-none gap-2">
          <Download className="h-4 w-4"/>
          Downloads
        </Button>
        <Button variant={activeTab === "naming" ? "default" : "ghost"} size="sm" onClick={() => setActiveTab("naming")} className="rounded-b-none gap-2">
          <FileSignature className="h-4 w-4"/>
          Naming
        </Button>
        <Button variant={activeTab === "dependencies" ? "default" : "ghost"} size="sm" onClick={() => setActiveTab("dependencies")} className="rounded-b-none gap-2">
          <PackageSearch className="h-4 w-4"/>
          Dependencies
        </Button>
      </div>

      <div className="pt-4">
        {activeTab === "general" && (<div className="grid grid-cols-1 gap-4 md:grid-cols-[minmax(0,1fr)_1px_minmax(0,1fr)] md:gap-6">
            <div className="space-y-4">
              <div className="grid grid-cols-[8rem_8rem] gap-4">
                <div className="min-w-0 space-y-2">
                  <Label htmlFor="theme-mode">Mode</Label>
                  <Select value={tempSettings.themeMode} onValueChange={(value: "auto" | "light" | "dark") => setTempSettings((prev) => ({ ...prev, themeMode: value }))}>
                    <SelectTrigger id="theme-mode" className="w-full">
                      <SelectValue placeholder="Select theme mode"/>
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="auto">Auto</SelectItem>
                      <SelectItem value="light">Light</SelectItem>
                      <SelectItem value="dark">Dark</SelectItem>
                    </SelectContent>
                  </Select>
                </div>

                <div className="min-w-0 space-y-2">
                  <Label htmlFor="base-color">Base Color</Label>
                  <Select value={tempSettings.baseColor} onValueChange={(value) => handleBaseColorChange(value as BaseColorName)} onOpenChange={(open) => {
                if (!open) {
                    restoreSelectedTheme();
                }
            }}>
                    <SelectTrigger id="base-color" className="w-full">
                      <SelectValue placeholder="Select a base color"/>
                    </SelectTrigger>
                    <SelectContent onMouseLeave={restoreSelectedTheme}>
                      {baseColors.map((baseColor) => (<SelectItem key={baseColor.name} value={baseColor.name} onMouseMove={() => previewBaseColor(baseColor.name)} onFocus={() => previewBaseColor(baseColor.name)}>
                          <span className="flex items-center gap-2">
                            <span className="h-3 w-3 rounded-full border border-border" style={{
                    backgroundColor: isDark
                        ? baseColor.cssVars.dark["muted-foreground"]
                        : baseColor.cssVars.light["muted-foreground"],
                }}/>
                            {baseColor.label}
                          </span>
                        </SelectItem>))}
                    </SelectContent>
                  </Select>
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="theme">Theme</Label>
                <Select value={tempSettings.theme} onValueChange={(value) => handleThemeChange(value as SettingsType["theme"])} onOpenChange={(open) => {
                if (!open) {
                    restoreSelectedTheme();
                }
            }}>
                  <SelectTrigger id="theme" className="w-32">
                    <SelectValue placeholder="Select a theme"/>
                  </SelectTrigger>
                  <SelectContent onMouseLeave={restoreSelectedTheme}>
                    {availableThemes.map((theme) => (<SelectItem key={theme.name} value={theme.name} onMouseMove={() => previewTheme(theme.name)} onFocus={() => previewTheme(theme.name)}>
                        <span className="flex items-center gap-2">
                          <span className="h-3 w-3 rounded-full border border-border" style={{
                    backgroundColor: isDark
                        ? theme.cssVars.dark[theme.name === tempSettings.baseColor ? "muted-foreground" : "primary"]
                        : theme.cssVars.light[theme.name === tempSettings.baseColor ? "muted-foreground" : "primary"],
                }}/>
                          {theme.label}
                        </span>
                      </SelectItem>))}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <Label htmlFor="font">Font</Label>
                <div className="flex flex-wrap items-center gap-2">
                  <Select value={tempSettings.fontFamily} onValueChange={(value: FontFamily) => setTempSettings((prev) => ({ ...prev, fontFamily: value }))}>
                    <SelectTrigger id="font" className="max-w-full min-w-40">
                      <SelectValue placeholder="Select a font"/>
                    </SelectTrigger>
                    <SelectContent>
                      {fontOptions.map((font) => (<SelectItem key={font.value} value={font.value}>
                          <span style={{ fontFamily: font.fontFamily }}>{font.label}</span>
                        </SelectItem>))}
                    </SelectContent>
                  </Select>
                  <Button type="button" variant="outline" onClick={() => setShowAddFontDialog(true)} className="shrink-0 gap-1.5">
                    <Plus className="h-4 w-4"/>
                    Add Font
                  </Button>
                </div>
                {tempSettings.customFonts.length > 0 && (<div className="space-y-2 rounded-lg border bg-muted/20 p-3">
                    <p className="text-xs font-medium text-muted-foreground">Custom Fonts</p>
                    <div className="space-y-2">
                      {tempSettings.customFonts.map((font) => (<div key={font.value} className="flex items-center justify-between gap-3 rounded-md border bg-background/70 px-3 py-2">
                          <div className="min-w-0">
                            <p className="truncate text-sm font-medium" style={{ fontFamily: font.fontFamily }}>
                              {font.label}
                            </p>
                            <p className="truncate text-xs text-muted-foreground">{font.url}</p>
                          </div>
                          <Button type="button" variant="ghost" size="icon" className="h-8 w-8 shrink-0 text-muted-foreground hover:text-destructive" onClick={() => void handleDeleteCustomFont(font.value as CustomFontFamily)}>
                            <Trash2 className="h-4 w-4"/>
                          </Button>
                        </div>))}
                    </div>
                  </div>)}
              </div>

              <div className="flex items-center gap-3 pt-2">
                <Switch id="sfx-enabled" checked={tempSettings.sfxEnabled} onCheckedChange={(checked) => setTempSettings((prev) => ({ ...prev, sfxEnabled: checked }))}/>
                <Label htmlFor="sfx-enabled" className="cursor-pointer text-sm">Sound Effects</Label>
              </div>

              <div className="flex items-center gap-3 pt-1">
                <Switch id="show-update-notifications" checked={tempSettings.showUpdateNotifications} onCheckedChange={(checked) => setTempSettings((prev) => ({ ...prev, showUpdateNotifications: checked }))}/>
                <Label htmlFor="show-update-notifications" className="cursor-pointer text-sm">Update Notifications</Label>
              </div>
            </div>

            <div aria-hidden="true" className="hidden bg-border md:block"/>

            <div className="space-y-6">
              <div className="space-y-2">
                <Label htmlFor="download-path">Download Path</Label>
                <div className="flex gap-2">
                  <InputWithContext id="download-path" value={tempSettings.downloadPath} onChange={(e) => setTempSettings((prev) => ({ ...prev, downloadPath: e.target.value }))} placeholder="C:\Users\YourUsername\Pictures"/>
                  <Button type="button" onClick={handleBrowseFolder} className="gap-1.5">
                    <FolderOpen className="h-4 w-4"/>
                    Browse
                  </Button>
                </div>
              </div>

              <div className="space-y-4">
                <Label>File Handling</Label>
                <div className="flex items-center gap-3">
                  <Switch id="skip-existing-files" checked={tempSettings.skipExistingFiles} onCheckedChange={(checked) => setTempSettings((prev) => ({ ...prev, skipExistingFiles: checked }))}/>
                  <Label htmlFor="skip-existing-files" className="cursor-pointer font-normal">Skip Existing Files</Label>
                </div>
                <div className="flex items-center gap-3">
                  <Switch id="delete-incomplete-files" checked={tempSettings.deleteIncompleteFiles} onCheckedChange={(checked) => setTempSettings((prev) => ({ ...prev, deleteIncompleteFiles: checked }))}/>
                  <Label htmlFor="delete-incomplete-files" className="cursor-pointer font-normal">Delete Incomplete Files</Label>
                </div>
              </div>

              <div className="space-y-3">
                <Label className="flex items-center gap-2">
                  GIF Conversion
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <CircleQuestionMark className="h-3.5 w-3.5 cursor-help text-muted-foreground"/>
                    </TooltipTrigger>
                    <TooltipContent side="top">
                      <p>Quality settings for converting Twitter's MP4 into actual GIF files.</p>
                    </TooltipContent>
                  </Tooltip>
                </Label>
                <div className="flex items-center gap-3">
                  <Switch id="auto-convert-gifs" checked={tempSettings.autoConvertGifs} onCheckedChange={(checked) => setTempSettings((prev) => ({ ...prev, autoConvertGifs: checked }))} disabled={!ffmpegInstalled}/>
                  <Label htmlFor="auto-convert-gifs" className={!ffmpegInstalled ? "text-muted-foreground" : "cursor-pointer"}>Auto Convert GIFs</Label>
                </div>
                <div className="flex items-center justify-between gap-4">
                  <Label htmlFor="gif-quality" className={!ffmpegInstalled || !tempSettings.autoConvertGifs ? "text-muted-foreground" : undefined}>GIF Quality</Label>
                  <div className="flex items-center gap-2">
                    <Select value={tempSettings.gifQuality} onValueChange={(value: GifQuality) => {
                setTempSettings((prev) => ({
                    ...prev,
                    gifQuality: value,
                }));
            }} disabled={!ffmpegInstalled || !tempSettings.autoConvertGifs}>
                      <SelectTrigger id="gif-quality" className="w-fit">
                        <SelectValue placeholder="Select quality"/>
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="fast">Fast</SelectItem>
                        <SelectItem value="better">Better</SelectItem>
                      </SelectContent>
                    </Select>
                    <Select value={tempSettings.gifResolution} onValueChange={(value: GifResolution) => setTempSettings((prev) => ({ ...prev, gifResolution: value }))} disabled={!ffmpegInstalled || !tempSettings.autoConvertGifs}>
                      <SelectTrigger id="gif-resolution" className="w-fit">
                        <SelectValue placeholder="Resolution"/>
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="original">Original</SelectItem>
                        <SelectItem value="high">High (800px)</SelectItem>
                        <SelectItem value="medium">Medium (600px)</SelectItem>
                        <SelectItem value="low">Low (400px)</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>
              </div>
            </div>
          </div>)}

        {activeTab === "downloads" && (<div className="grid grid-cols-1 gap-4 md:grid-cols-[minmax(0,1fr)_1px_minmax(0,1fr)] md:gap-6">
            <div className="space-y-4">
                  <div className="flex items-center gap-2">
                    <Label>Download Controls</Label>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <CircleQuestionMark className="h-3.5 w-3.5 cursor-help text-muted-foreground"/>
                      </TooltipTrigger>
                      <TooltipContent side="top" className="max-w-xs">
                        <p>The speed limit is shared by all concurrent downloads. Delay spaces out media requests and adds an optional random variation.</p>
                      </TooltipContent>
                    </Tooltip>
                  </div>

                  <div className="grid grid-cols-2 gap-4">
                    <div className="space-y-1.5">
                      <Label htmlFor="concurrent-downloads" className="flex items-center gap-2 text-xs">
                        Concurrent Downloads
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <CircleQuestionMark className="h-3.5 w-3.5 cursor-help text-muted-foreground"/>
                          </TooltipTrigger>
                          <TooltipContent side="top">
                            <p>How many media files can download at the same time.</p>
                          </TooltipContent>
                        </Tooltip>
                      </Label>
                      <Select value={String(tempSettings.concurrentDownloads || 10)} onValueChange={(value) => setTempSettings((prev) => ({ ...prev, concurrentDownloads: parseInt(value, 10) }))}>
                        <SelectTrigger id="concurrent-downloads" className="w-24">
                          <SelectValue placeholder="10"/>
                        </SelectTrigger>
                        <SelectContent>
                          {[1, 2, 3, 5, 8, 10, 15, 20, 25, 30, 40, 50].map((value) => (<SelectItem key={value} value={String(value)}>
                              {value}
                            </SelectItem>))}
                        </SelectContent>
                      </Select>
                    </div>

                    <div className="space-y-1.5">
                      <Label htmlFor="retry-attempts" className="flex items-center gap-2 text-xs">
                        Retry Attempts
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <CircleQuestionMark className="h-3.5 w-3.5 cursor-help text-muted-foreground"/>
                          </TooltipTrigger>
                          <TooltipContent side="top">
                            <p>How many times to retry a failed download before giving up.</p>
                          </TooltipContent>
                        </Tooltip>
                      </Label>
                      <Select value={String(tempSettings.retryAttempts)} onValueChange={(value) => setTempSettings((prev) => ({ ...prev, retryAttempts: parseInt(value, 10) }))}>
                        <SelectTrigger id="retry-attempts" className="w-24">
                          <SelectValue placeholder="1"/>
                        </SelectTrigger>
                        <SelectContent>
                          {[1, 2, 3, 4, 5].map((value) => (<SelectItem key={value} value={String(value)}>
                              {value}
                            </SelectItem>))}
                        </SelectContent>
                      </Select>
                    </div>
                  </div>

                  <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                    <div className="space-y-1.5">
                      <Label htmlFor="download-speed-limit" className="text-xs">Speed Limit</Label>
                      <div className="flex items-center gap-2">
                        <InputWithContext id="download-speed-limit" type="number" min="0" max="10485760" step="1" value={tempSettings.downloadSpeedLimitKBps} onChange={(e) => {
                const value = Number(e.target.value);
                setTempSettings((prev) => ({ ...prev, downloadSpeedLimitKBps: Number.isFinite(value) ? Math.min(10485760, Math.max(0, Math.round(value))) : 0 }));
            }} className="w-24"/>
                        <span className="text-xs text-muted-foreground">KB/s</span>
                      </div>
                    </div>

                    <div className="space-y-1.5">
                      <Label htmlFor="download-delay" className="text-xs">Base Delay</Label>
                      <div className="flex items-center gap-2">
                        <InputWithContext id="download-delay" type="number" min="0" max="3600" step="0.1" value={tempSettings.downloadDelaySeconds} onChange={(e) => {
                const value = Number(e.target.value);
                setTempSettings((prev) => ({ ...prev, downloadDelaySeconds: Number.isFinite(value) ? Math.min(3600, Math.max(0, value)) : 0 }));
            }} className="w-24"/>
                        <span className="text-xs text-muted-foreground">sec</span>
                      </div>
                    </div>

                    <div className="space-y-1.5">
                      <Label htmlFor="download-delay-jitter" className="text-xs">Random Extra</Label>
                      <div className="flex items-center gap-2">
                        <InputWithContext id="download-delay-jitter" type="number" min="0" max="3600" step="0.1" value={tempSettings.downloadDelayJitterSeconds} onChange={(e) => {
                const value = Number(e.target.value);
                setTempSettings((prev) => ({ ...prev, downloadDelayJitterSeconds: Number.isFinite(value) ? Math.min(3600, Math.max(0, value)) : 0 }));
            }} className="w-24"/>
                        <span className="text-xs text-muted-foreground">sec</span>
                      </div>
                    </div>
                  </div>
            </div>

            <div aria-hidden="true" className="hidden bg-border md:block"/>

            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="proxy" className="flex items-center gap-2">
                  Proxy
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <CircleQuestionMark className="h-3.5 w-3.5 cursor-help text-muted-foreground"/>
                    </TooltipTrigger>
                    <TooltipContent side="top">
                      <p>Supports one proxy or multiple proxies separated by commas. Requests will rotate through them.</p>
                    </TooltipContent>
                  </Tooltip>
                </Label>
                <InputWithContext id="proxy" value={tempSettings.proxy || ""} onChange={(e) => setTempSettings((prev) => ({ ...prev, proxy: e.target.value }))} placeholder="http://proxy1:port, socks5://proxy2:port (optional)" className="max-w-md"/>
              </div>

              <div className="space-y-2">
                <Label>Fetch Delay</Label>
                <div className="flex flex-wrap gap-4">
                  {([
                    ["fetchDelaySeconds", "Base Delay"],
                    ["fetchDelayJitterSeconds", "Random Extra"],
                  ] as const).map(([key, label]) => (
                    <div key={key} className="space-y-1.5">
                      <Label htmlFor={key} className="text-xs">{label}</Label>
                      <div className="flex items-center gap-2">
                        <InputWithContext id={key} type="number" min="0" max="3600" step="0.1" value={tempSettings[key]} onChange={(e) => {
                          const value = Number(e.target.value);
                          setTempSettings((prev) => ({ ...prev, [key]: Number.isFinite(value) ? Math.min(3600, Math.max(0, value)) : 0 }));
                        }} className="w-24"/>
                        <span className="text-xs text-muted-foreground">sec</span>
                      </div>
                    </div>
                  ))}
                </div>
                <p className="text-xs text-muted-foreground">Spaces out fetch batch starts, including account switches and retries. Random Extra adds between 0 and the chosen number of seconds. Set both to 0 to disable.</p>
                <p className="text-xs text-muted-foreground">Suggested starting point: 5 sec base + 5 sec random extra. No delay guarantees avoidance of rate limits. Waiting counts toward Fetch Timeout; increase it for longer delays.</p>
                <p className="text-xs text-muted-foreground">Applies between extractor runs, including date-range fetches, rather than individual requests inside the extractor.</p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="fetch-timeout" className="flex items-center gap-2">
                  Fetch Timeout
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <CircleQuestionMark className="h-3.5 w-3.5 cursor-help text-muted-foreground"/>
                    </TooltipTrigger>
                    <TooltipContent side="top">
                      <p>Timeout in seconds. Fetch stops automatically when reached</p>
                    </TooltipContent>
                  </Tooltip>
                </Label>
                <InputWithContext id="fetch-timeout" type="number" value={tempSettings.fetchTimeout || 60} onChange={(e) => {
                const inputValue = e.target.value;
                if (inputValue === "") {
                    setTempSettings((prev) => ({ ...prev, fetchTimeout: 60 }));
                    return;
                }
                const value = parseInt(inputValue, 10);
                if (!isNaN(value)) {
                    setTempSettings((prev) => ({ ...prev, fetchTimeout: value }));
                }
            }} onBlur={(e) => {
                const value = parseInt(e.target.value, 10);
                if (isNaN(value) || value < 30) {
                    setTempSettings((prev) => ({ ...prev, fetchTimeout: 30 }));
                }
                else if (value > 900) {
                    setTempSettings((prev) => ({ ...prev, fetchTimeout: 900 }));
                }
            }} placeholder="60" className="w-24"/>
              </div>

            </div>
          </div>)}

        {activeTab === "naming" && (<div className="grid grid-cols-1 gap-6 md:grid-cols-2 md:gap-8 items-start">
            <div className="md:pr-8 md:border-r border-border">
              <FormatEditor title="Folder" value={tempSettings.folderTemplate} defaultValue={DEFAULT_FOLDER_TEMPLATE} tokens={FOLDER_TEMPLATE_VARIABLES} placeholder={DEFAULT_FOLDER_TEMPLATE} render={(t) => renderFolderTemplate(t, SAMPLE_FOLDER_DATA)} onChange={(next) => setTempSettings((prev) => ({ ...prev, folderTemplate: next }))}/>
            </div>
            <div>
              <FormatEditor title="Filename" value={tempSettings.filenameTemplate} defaultValue={DEFAULT_FILENAME_TEMPLATE} tokens={FILENAME_TEMPLATE_VARIABLES} placeholder={DEFAULT_FILENAME_TEMPLATE} suffix=".jpg" render={(t) => renderFilenameTemplate(t, SAMPLE_FILENAME_DATA)} onChange={(next) => setTempSettings((prev) => ({ ...prev, filenameTemplate: next }))}/>
            </div>
          </div>)}

        {activeTab === "dependencies" && (<div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <div className="space-y-4">
              <div className="space-y-2">
                <Label className="flex flex-wrap items-center gap-2">
                  <span>Core Xtractor</span>
                  {extractorVersionText && (<span className={extractorUpdateAvailable
                    ? "text-xs font-normal text-amber-600 dark:text-amber-400"
                    : "text-xs font-normal text-muted-foreground"}>
                      {extractorVersionText}
                    </span>)}
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <CircleQuestionMark className="h-3.5 w-3.5 cursor-help text-muted-foreground"/>
                    </TooltipTrigger>
                    <TooltipContent side="top">
                      <p>Required to fetch media from Twitter/X. Downloaded from the xtractor-binaries GitHub releases.</p>
                    </TooltipContent>
                  </Tooltip>
                </Label>
                <div className="flex min-h-9 items-center gap-3">
                  <Button variant="outline" size="sm" className="h-9" onClick={handleDownloadExtractor} disabled={downloadingExtractor}>
                    {downloadingExtractor ? (<>
                        <Spinner />
                        Downloading...
                      </>) : (<>
                        <Download className="h-4 w-4"/>
                        {extractorInstalled ? extractorUpdateAvailable ? "Update Xtractor" : "Reinstall Xtractor" : "Download Xtractor"}
                      </>)}
                  </Button>
                  {extractorInstalled && (<div className="flex items-center gap-2 text-sm text-green-600 dark:text-green-400">
                      <Check className="h-4 w-4"/>
                      Installed
                    </div>)}
                </div>
              </div>

              <div className="space-y-2">
                <Label className="flex flex-wrap items-center gap-2">
                  <span>GIF Conversion</span>
                  {ffmpegVersionText && (<span className={ffmpegUpdateAvailable
                    ? "text-xs font-normal text-amber-600 dark:text-amber-400"
                    : "text-xs font-normal text-muted-foreground"}>
                      {ffmpegVersionText}
                    </span>)}
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <CircleQuestionMark className="h-3.5 w-3.5 cursor-help text-muted-foreground"/>
                    </TooltipTrigger>
                    <TooltipContent side="top">
                      <p>FFmpeg is required to convert Twitter's MP4 to actual GIF format</p>
                    </TooltipContent>
                  </Tooltip>
                </Label>
                <div className="flex h-9 items-center">
                  <Button variant="outline" size="sm" className="h-9" onClick={handleDownloadFFmpeg} disabled={downloadingFFmpeg}>
                    {downloadingFFmpeg ? (<>
                        <Spinner />
                        Downloading...
                      </>) : (<>
                        <Download className="h-4 w-4"/>
                        {ffmpegInstalled ? ffmpegUpdateAvailable ? "Update FFmpeg" : "Reinstall FFmpeg" : "Download FFmpeg"}
                      </>)}
                  </Button>
                  {ffmpegInstalled && (<div className="ml-3 flex items-center gap-2 text-sm text-green-600 dark:text-green-400">
                      <Check className="h-4 w-4"/>
                      Installed
                    </div>)}
                </div>
              </div>

              <div className="space-y-2">
                <Label className="flex flex-wrap items-center gap-2">
                  <span>Metadata Embedding</span>
                  {exiftoolVersionText && (<span className={exiftoolUpdateAvailable
                    ? "text-xs font-normal text-amber-600 dark:text-amber-400"
                    : "text-xs font-normal text-muted-foreground"}>
                      {exiftoolVersionText}
                    </span>)}
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <CircleQuestionMark className="h-3.5 w-3.5 cursor-help text-muted-foreground"/>
                    </TooltipTrigger>
                    <TooltipContent side="top">
                      <p>ExifTool is required to embed tweet URL and original filename into media file metadata</p>
                    </TooltipContent>
                  </Tooltip>
                </Label>
                <div className="flex h-9 items-center">
                  <Button variant="outline" size="sm" className="h-9" onClick={handleDownloadExifTool} disabled={downloadingExifTool}>
                    {downloadingExifTool ? (<>
                        <Spinner />
                        Downloading...
                      </>) : (<>
                        <Download className="h-4 w-4"/>
                        {exiftoolInstalled ? exiftoolUpdateAvailable ? "Update ExifTool" : "Reinstall ExifTool" : "Download ExifTool"}
                      </>)}
                  </Button>
                  {exiftoolInstalled && (<div className="ml-3 flex items-center gap-2 text-sm text-green-600 dark:text-green-400">
                      <Check className="h-4 w-4"/>
                      Installed
                    </div>)}
                </div>
              </div>
            </div>

            <div />
          </div>)}
      </div>

      <Dialog open={showAddFontDialog} onOpenChange={(open) => open ? setShowAddFontDialog(true) : closeAddFontDialog()}>
        <DialogContent className="sm:max-w-115 [&>button]:hidden">
          <DialogHeader>
            <div className="flex items-center justify-between gap-3">
              <DialogTitle>Add Font</DialogTitle>
              <a href="https://fonts.google.com" target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground hover:underline">
                Open Google Fonts
                <ExternalLink className="h-3 w-3"/>
              </a>
            </div>
            <DialogDescription />
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="google-font-url">Google Font URL</Label>
              <Input id="google-font-url" value={addFontUrl} onChange={(event) => setAddFontUrl(event.target.value)} onKeyDown={(event) => {
            if (event.key === "Enter" && parsedAddFont) {
                void handleAddFont();
            }
        }} placeholder="https://fonts.google.com/specimen/Ubuntu" autoFocus/>
              {addFontUrl.trim() && !parsedAddFont && (<p className="text-xs text-destructive">
                  Enter a valid Google Fonts URL.
                </p>)}
            </div>
            <div className="rounded-md border bg-muted/20 p-4">
              <p className="mb-2 text-xs font-medium text-muted-foreground">Preview</p>
              <p className="text-2xl font-semibold leading-tight" style={{ fontFamily: parsedAddFont?.fontFamily }}>
                Aa The quick brown fox
              </p>
              <p className="mt-2 text-sm text-muted-foreground" style={{ fontFamily: parsedAddFont?.fontFamily }}>
                Twitter/X Media Batch Downloader
              </p>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeAddFontDialog}>
              Cancel
            </Button>
            <Button onClick={() => void handleAddFont()} disabled={!parsedAddFont}>
              Add
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={showResetConfirm} onOpenChange={setShowResetConfirm}>
        <DialogContent className="max-w-md [&>button]:hidden">
          <DialogHeader>
            <DialogTitle>Reset to Default?</DialogTitle>
            <DialogDescription>
              This will reset all settings to their default values. Your custom configurations will be lost.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowResetConfirm(false)}>Cancel</Button>
            <Button onClick={handleReset}>Reset</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>);
}
