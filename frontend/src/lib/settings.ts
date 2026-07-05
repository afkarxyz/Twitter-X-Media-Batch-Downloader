import { GetDefaults } from "../../wailsjs/go/main/App";
export type BuiltInFontFamily = "google-sans" | "inter" | "poppins" | "roboto" | "dm-sans" | "plus-jakarta-sans" | "manrope" | "space-grotesk" | "noto-sans" | "nunito-sans" | "figtree" | "raleway" | "public-sans" | "outfit" | "jetbrains-mono" | "geist-sans" | "bricolage-grotesque";
export type CustomFontFamily = `custom-${string}`;
export type FontFamily = BuiltInFontFamily | CustomFontFamily;
export interface CustomFontOption {
    value: CustomFontFamily;
    label: string;
    fontFamily: string;
    url: string;
}
export interface FontOption {
    value: FontFamily;
    label: string;
    fontFamily: string;
    url?: string;
}
export type GifQuality = "fast" | "better";
export type GifResolution = "original" | "high" | "medium" | "low";
export type VideoQuality = "highest" | "720" | "480" | "360" | "lowest";
export type ImageSize = "orig" | "large" | "medium" | "small";
export type AvatarSize = "orig" | "400x400" | "200x200" | "bigger" | "normal";
export type BannerSize = "orig" | "1500x500" | "600x200" | "300x100";
export const VIDEO_QUALITIES: {
    value: VideoQuality;
    label: string;
}[] = [
    { value: "highest", label: "Highest" },
    { value: "720", label: "720p" },
    { value: "480", label: "480p" },
    { value: "360", label: "360p" },
    { value: "lowest", label: "Lowest" },
];
export const IMAGE_SIZES: {
    value: ImageSize;
    label: string;
}[] = [
    { value: "orig", label: "Original" },
    { value: "large", label: "Large" },
    { value: "medium", label: "Medium" },
    { value: "small", label: "Small" },
];
export const AVATAR_SIZES: {
    value: AvatarSize;
    label: string;
}[] = [
    { value: "orig", label: "Original" },
    { value: "400x400", label: "400px" },
    { value: "200x200", label: "200px" },
    { value: "bigger", label: "Bigger" },
    { value: "normal", label: "Normal" },
];
export const BANNER_SIZES: {
    value: BannerSize;
    label: string;
}[] = [
    { value: "orig", label: "Original" },
    { value: "1500x500", label: "1500×500" },
    { value: "600x200", label: "600×200" },
    { value: "300x100", label: "300×100" },
];
export const DEFAULT_FILENAME_TEMPLATE = "{handle}_{date}_{tweet_id}";
export interface TemplateToken {
    key: string;
    description: string;
    example: string;
}
export const FILENAME_TEMPLATE_VARIABLES: TemplateToken[] = [
    { key: "{handle}", description: "Author handle", example: "elonmusk" },
    { key: "{name}", description: "Author display name", example: "Elon Musk" },
    { key: "{date}", description: "Tweet date & time", example: "20250620_104556" },
    { key: "{tweet_id}", description: "Tweet ID", example: "1989033613824815258" },
    { key: "{media_id}", description: "Unique media ID (prevents overwrites)", example: "Gx1aB2cXkAA7Yz9" },
    { key: "{type}", description: "Media type", example: "photo" },
];
export interface FilenameTemplateData {
    username?: string;
    accountName?: string;
    date?: string;
    tweetId?: string;
    index?: number;
    mediaID?: string;
    type?: string;
}
export const SAMPLE_FILENAME_DATA: FilenameTemplateData = {
    username: "elonmusk",
    accountName: "Elon Musk",
    date: "20250620_104556",
    tweetId: "1989033613824815258",
    index: 1,
    mediaID: "Gx1aB2cXkAA7Yz9",
    type: "photo",
};
function sanitizeTemplatePart(input: string): string {
    const cleaned = input
        .replace(/[\u0000-\u001f\\/:*?"<>|]/g, "")
        .trim()
        .replace(/[. ]+$/g, "");
    return /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?$/i.test(cleaned) ? `${cleaned}_` : cleaned;
}
export function renderFilenameTemplate(template: string, data: FilenameTemplateData): string {
    if (!template)
        return "";
    return sanitizeTemplatePart(template
        .replace(/\{handle\}/g, sanitizeTemplatePart(data.username ?? ""))
        .replace(/\{name\}/g, sanitizeTemplatePart(data.accountName ?? ""))
        .replace(/\{username\}/g, sanitizeTemplatePart(data.username ?? ""))
        .replace(/\{account_name\}/g, sanitizeTemplatePart(data.accountName ?? ""))
        .replace(/\{date\}/g, sanitizeTemplatePart(data.date ?? ""))
        .replace(/\{tweet_id\}/g, sanitizeTemplatePart(data.tweetId ?? ""))
        .replace(/\{index\}/g, String(data.index ?? 1).padStart(2, "0"))
        .replace(/\{media_id\}/g, sanitizeTemplatePart(data.mediaID ?? ""))
        .replace(/\{type\}/g, sanitizeTemplatePart(data.type ?? ""))
        .trim());
}
export const DEFAULT_FOLDER_TEMPLATE = "{handle}";
export const FOLDER_TEMPLATE_VARIABLES: TemplateToken[] = [
    { key: "{handle}", description: "Author handle", example: "elonmusk" },
    { key: "{name}", description: "Author display name", example: "Elon Musk" },
    { key: "{date}", description: "Download date", example: "20250620" },
];
export interface FolderTemplateData {
    username?: string;
    accountName?: string;
    date?: string;
}
export const SAMPLE_FOLDER_DATA: FolderTemplateData = {
    username: "elonmusk",
    accountName: "Elon Musk",
    date: "20250620",
};
export function renderFolderTemplate(template: string, data: FolderTemplateData): string {
    if (!template)
        return "";
    return sanitizeTemplatePart(template
        .replace(/\{handle\}/g, sanitizeTemplatePart(data.username ?? ""))
        .replace(/\{name\}/g, sanitizeTemplatePart(data.accountName ?? ""))
        .replace(/\{username\}/g, sanitizeTemplatePart(data.username ?? ""))
        .replace(/\{account_name\}/g, sanitizeTemplatePart(data.accountName ?? ""))
        .replace(/\{date\}/g, sanitizeTemplatePart(data.date ?? ""))
        .trim());
}
const DEFAULT_CONCURRENT_DOWNLOADS = 10;
const MIN_CONCURRENT_DOWNLOADS = 1;
const MAX_CONCURRENT_DOWNLOADS = 50;
const DEFAULT_RETRY_ATTEMPTS = 1;
const MIN_RETRY_ATTEMPTS = 1;
const MAX_RETRY_ATTEMPTS = 5;
export interface Settings {
    downloadPath: string;
    concurrentDownloads: number;
    skipExistingFiles: boolean;
    deleteIncompleteFiles: boolean;
    retryAttempts: number;
    theme: string;
    themeMode: "auto" | "light" | "dark";
    fontFamily: FontFamily;
    customFonts: CustomFontOption[];
    sfxEnabled: boolean;
    autoConvertGifs: boolean;
    gifQuality: GifQuality;
    gifResolution: GifResolution;
    proxy: string;
    fetchTimeout: number;
    includePhotos: boolean;
    includeVideos: boolean;
    includeGifs: boolean;
    includeText: boolean;
    includeRetweets: boolean;
    videoQuality: VideoQuality;
    imageSize: ImageSize;
    avatarSize: AvatarSize;
    bannerSize: BannerSize;
    filenameTemplate: string;
    folderTemplate: string;
}
export const DEFAULT_SETTINGS: Settings = {
    downloadPath: "",
    concurrentDownloads: DEFAULT_CONCURRENT_DOWNLOADS,
    skipExistingFiles: true,
    deleteIncompleteFiles: true,
    retryAttempts: DEFAULT_RETRY_ATTEMPTS,
    theme: "yellow",
    themeMode: "auto",
    fontFamily: "google-sans",
    customFonts: [],
    sfxEnabled: true,
    autoConvertGifs: false,
    gifQuality: "fast",
    gifResolution: "original",
    proxy: "",
    fetchTimeout: 60,
    includePhotos: true,
    includeVideos: true,
    includeGifs: true,
    includeText: false,
    includeRetweets: false,
    videoQuality: "highest",
    imageSize: "orig",
    avatarSize: "orig",
    bannerSize: "orig",
    filenameTemplate: DEFAULT_FILENAME_TEMPLATE,
    folderTemplate: DEFAULT_FOLDER_TEMPLATE,
};
export const FONT_OPTIONS: FontOption[] = [
    {
        value: "bricolage-grotesque",
        label: "Bricolage Grotesque",
        fontFamily: '"Bricolage Grotesque", system-ui, sans-serif',
    },
    {
        value: "dm-sans",
        label: "DM Sans",
        fontFamily: '"DM Sans", system-ui, sans-serif',
    },
    {
        value: "figtree",
        label: "Figtree",
        fontFamily: '"Figtree", system-ui, sans-serif',
    },
    {
        value: "geist-sans",
        label: "Geist Sans",
        fontFamily: '"Geist", system-ui, sans-serif',
    },
    {
        value: "google-sans",
        label: "Google Sans",
        fontFamily: '"Google Sans Flex", system-ui, sans-serif',
    },
    {
        value: "inter",
        label: "Inter",
        fontFamily: '"Inter", system-ui, sans-serif',
    },
    {
        value: "jetbrains-mono",
        label: "JetBrains Mono",
        fontFamily: '"JetBrains Mono", ui-monospace, monospace',
    },
    {
        value: "manrope",
        label: "Manrope",
        fontFamily: '"Manrope", system-ui, sans-serif',
    },
    {
        value: "noto-sans",
        label: "Noto Sans",
        fontFamily: '"Noto Sans", system-ui, sans-serif',
    },
    {
        value: "nunito-sans",
        label: "Nunito Sans",
        fontFamily: '"Nunito Sans", system-ui, sans-serif',
    },
    {
        value: "outfit",
        label: "Outfit",
        fontFamily: '"Outfit", system-ui, sans-serif',
    },
    {
        value: "plus-jakarta-sans",
        label: "Plus Jakarta Sans",
        fontFamily: '"Plus Jakarta Sans", system-ui, sans-serif',
    },
    {
        value: "poppins",
        label: "Poppins",
        fontFamily: '"Poppins", system-ui, sans-serif',
    },
    {
        value: "public-sans",
        label: "Public Sans",
        fontFamily: '"Public Sans", system-ui, sans-serif',
    },
    {
        value: "raleway",
        label: "Raleway",
        fontFamily: '"Raleway", system-ui, sans-serif',
    },
    {
        value: "roboto",
        label: "Roboto",
        fontFamily: '"Roboto", system-ui, sans-serif',
    },
    {
        value: "space-grotesk",
        label: "Space Grotesk",
        fontFamily: '"Space Grotesk", system-ui, sans-serif',
    },
];
const BUILT_IN_FONT_VALUES = new Set(FONT_OPTIONS.map((font) => font.value));
const SETTINGS_KEY = "twitter-media-downloader-settings";
const GOOGLE_FONT_LINK_ID_PREFIX = "twitter-media-custom-font-";
const GOOGLE_FONTS_CSS_HOST = "fonts.googleapis.com";
const GOOGLE_FONTS_SPECIMEN_HOST = "fonts.google.com";
function extractGoogleFontInputUrl(input: string): string {
    const trimmed = input.trim();
    const hrefMatch = trimmed.match(/\bhref=["']([^"']+)["']/i);
    if (hrefMatch?.[1]) {
        return hrefMatch[1];
    }
    const importMatch = trimmed.match(/@import\s+url\(["']?([^"')]+)["']?\)/i);
    if (importMatch?.[1]) {
        return importMatch[1];
    }
    return trimmed;
}
function coerceGoogleFontUrl(rawUrl: string): string {
    const trimmed = rawUrl.trim();
    if (/^https?:\/\//i.test(trimmed)) {
        return trimmed;
    }
    if (/^(fonts\.googleapis\.com|fonts\.google\.com)\//i.test(trimmed)) {
        return `https://${trimmed}`;
    }
    return trimmed;
}
function normalizeFontLabel(label: string): string {
    return label.replace(/\+/g, " ").replace(/\s+/g, " ").trim();
}
function slugifyFontLabel(label: string): string {
    return label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "font";
}
function toFontFamilyCss(label: string): string {
    const escapedLabel = label.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
    return `"${escapedLabel}", system-ui, sans-serif`;
}
function buildGoogleFontsCssUrl(label: string): string {
    const url = new URL("https://fonts.googleapis.com/css2");
    url.searchParams.set("family", label);
    url.searchParams.set("display", "swap");
    return url.toString();
}
function extractSpecimenFontLabel(parsed: URL): string {
    const segments = parsed.pathname.split("/").filter(Boolean);
    const specimenIndex = segments.findIndex((segment) => segment.toLowerCase() === "specimen");
    const specimenName = specimenIndex >= 0 ? segments[specimenIndex + 1] : "";
    return normalizeFontLabel(decodeURIComponent(specimenName || ""));
}
function normalizeGoogleFontCssUrl(rawUrl: string): string | null {
    try {
        const parsed = new URL(coerceGoogleFontUrl(extractGoogleFontInputUrl(rawUrl)));
        if (parsed.protocol !== "https:") {
            return null;
        }
        if (parsed.hostname === GOOGLE_FONTS_SPECIMEN_HOST) {
            const label = extractSpecimenFontLabel(parsed);
            return label ? buildGoogleFontsCssUrl(label) : null;
        }
        if (parsed.hostname !== GOOGLE_FONTS_CSS_HOST || (parsed.pathname !== "/css" && parsed.pathname !== "/css2")) {
            return null;
        }
        if (parsed.searchParams.getAll("family").length === 0) {
            return null;
        }
        if (!parsed.searchParams.has("display")) {
            parsed.searchParams.set("display", "swap");
        }
        return parsed.toString();
    }
    catch {
        return null;
    }
}
export function parseGoogleFontUrl(rawUrl: string): CustomFontOption | null {
    const normalizedUrl = normalizeGoogleFontCssUrl(rawUrl);
    if (!normalizedUrl) {
        return null;
    }
    const parsed = new URL(normalizedUrl);
    const family = parsed.searchParams.getAll("family")[0];
    const label = normalizeFontLabel((family || "").split(":")[0] || "");
    if (!label) {
        return null;
    }
    return {
        value: `custom-${slugifyFontLabel(label)}` as CustomFontFamily,
        label,
        fontFamily: toFontFamilyCss(label),
        url: normalizedUrl,
    };
}
function normalizeCustomFonts(customFonts: unknown): CustomFontOption[] {
    if (!Array.isArray(customFonts)) {
        return [];
    }
    const normalizedFonts: CustomFontOption[] = [];
    const seenValues = new Set<string>();
    const seenUrls = new Set<string>();
    for (const item of customFonts) {
        if (!item || typeof item !== "object") {
            continue;
        }
        const rawUrl = (item as {
            url?: unknown;
        }).url;
        if (typeof rawUrl !== "string") {
            continue;
        }
        const parsed = parseGoogleFontUrl(rawUrl);
        if (!parsed || seenValues.has(parsed.value) || seenUrls.has(parsed.url)) {
            continue;
        }
        seenValues.add(parsed.value);
        seenUrls.add(parsed.url);
        normalizedFonts.push(parsed);
    }
    return normalizedFonts;
}
function normalizeFontFamily(fontFamily: unknown, customFonts: CustomFontOption[]): FontFamily {
    if (typeof fontFamily !== "string") {
        return DEFAULT_SETTINGS.fontFamily;
    }
    if (BUILT_IN_FONT_VALUES.has(fontFamily as BuiltInFontFamily)) {
        return fontFamily as BuiltInFontFamily;
    }
    const customFont = customFonts.find((font) => font.value === fontFamily);
    return customFont ? customFont.value : DEFAULT_SETTINGS.fontFamily;
}
function normalizeConcurrentDownloads(value: unknown): number {
    if (typeof value !== "number" || !Number.isFinite(value)) {
        return DEFAULT_CONCURRENT_DOWNLOADS;
    }
    const rounded = Math.round(value);
    if (rounded < MIN_CONCURRENT_DOWNLOADS) {
        return MIN_CONCURRENT_DOWNLOADS;
    }
    if (rounded > MAX_CONCURRENT_DOWNLOADS) {
        return MAX_CONCURRENT_DOWNLOADS;
    }
    return rounded;
}
function normalizeBoolean(value: unknown, fallback: boolean): boolean {
    if (typeof value === "boolean") {
        return value;
    }
    return fallback;
}
function normalizeRetryAttempts(value: unknown): number {
    if (typeof value !== "number" || !Number.isFinite(value)) {
        return DEFAULT_RETRY_ATTEMPTS;
    }
    const rounded = Math.round(value);
    if (rounded < MIN_RETRY_ATTEMPTS) {
        return MIN_RETRY_ATTEMPTS;
    }
    if (rounded > MAX_RETRY_ATTEMPTS) {
        return MAX_RETRY_ATTEMPTS;
    }
    return rounded;
}
function normalizeTemplateTokens(value: unknown, fallback: string): string {
    if (typeof value !== "string") {
        return fallback;
    }
    return value
        .replace(/\{username\}/g, "{handle}")
        .replace(/\{account_name\}/g, "{name}");
}
function normalizeFilenameTemplate(value: unknown): string {
    const template = normalizeTemplateTokens(value, DEFAULT_FILENAME_TEMPLATE);
    return template === "{handle}_{date}_{tweet_id}_{index}" ? DEFAULT_FILENAME_TEMPLATE : template;
}
function normalizeFolderTemplate(value: unknown): string {
    return normalizeTemplateTokens(value, DEFAULT_FOLDER_TEMPLATE);
}
export function getFontOptions(customFonts: CustomFontOption[] = []): FontOption[] {
    return [...FONT_OPTIONS, ...normalizeCustomFonts(customFonts)];
}
export function loadGoogleFontUrl(url: string, id = `${GOOGLE_FONT_LINK_ID_PREFIX}preview`): void {
    const normalizedUrl = normalizeGoogleFontCssUrl(url);
    if (!normalizedUrl) {
        return;
    }
    let link = document.getElementById(id) as HTMLLinkElement | null;
    if (!link) {
        link = document.createElement("link");
        link.id = id;
        link.rel = "stylesheet";
        document.head.appendChild(link);
    }
    if (link.href !== normalizedUrl) {
        link.href = normalizedUrl;
    }
}
function loadCustomFontStylesheets(customFonts: CustomFontOption[]): void {
    for (const font of normalizeCustomFonts(customFonts)) {
        loadGoogleFontUrl(font.url, `${GOOGLE_FONT_LINK_ID_PREFIX}${font.value}`);
    }
}
export function applyFont(fontFamily: FontFamily, customFonts: CustomFontOption[] = []): void {
    const fontOptions = getFontOptions(customFonts);
    loadCustomFontStylesheets(customFonts);
    const font = fontOptions.find((option) => option.value === fontFamily) || FONT_OPTIONS.find((option) => option.value === DEFAULT_SETTINGS.fontFamily);
    if (font) {
        document.documentElement.style.setProperty("--font-sans", font.fontFamily);
        document.body.style.fontFamily = font.fontFamily;
    }
}
async function fetchDefaultPath(): Promise<string> {
    try {
        const data = await GetDefaults();
        return data.downloadPath || "";
    }
    catch (error) {
        console.error("Failed to fetch default path:", error);
        return "";
    }
}
function toNormalizedSettings(settings: Partial<Settings>): Settings {
    const customFonts = normalizeCustomFonts(settings.customFonts);
    return {
        ...DEFAULT_SETTINGS,
        ...settings,
        concurrentDownloads: normalizeConcurrentDownloads(settings.concurrentDownloads),
        skipExistingFiles: normalizeBoolean(settings.skipExistingFiles, DEFAULT_SETTINGS.skipExistingFiles),
        deleteIncompleteFiles: normalizeBoolean(settings.deleteIncompleteFiles, DEFAULT_SETTINGS.deleteIncompleteFiles),
        retryAttempts: normalizeRetryAttempts(settings.retryAttempts),
        autoConvertGifs: normalizeBoolean(settings.autoConvertGifs, DEFAULT_SETTINGS.autoConvertGifs),
        filenameTemplate: normalizeFilenameTemplate(settings.filenameTemplate),
        folderTemplate: normalizeFolderTemplate(settings.folderTemplate),
        customFonts,
        fontFamily: normalizeFontFamily(settings.fontFamily, customFonts),
    };
}
export function getSettings(): Settings {
    try {
        const stored = localStorage.getItem(SETTINGS_KEY);
        if (stored) {
            return toNormalizedSettings(JSON.parse(stored) as Partial<Settings>);
        }
    }
    catch (error) {
        console.error("Failed to load settings:", error);
    }
    return DEFAULT_SETTINGS;
}
export async function getSettingsWithDefaults(): Promise<Settings> {
    const settings = getSettings();
    if (!settings.downloadPath) {
        settings.downloadPath = await fetchDefaultPath();
    }
    return toNormalizedSettings(settings);
}
export function saveSettings(settings: Settings): void {
    try {
        const normalizedSettings = toNormalizedSettings(settings);
        localStorage.setItem(SETTINGS_KEY, JSON.stringify(normalizedSettings));
    }
    catch (error) {
        console.error("Failed to save settings:", error);
    }
}
export function updateSettings(partial: Partial<Settings>): Settings {
    const current = getSettings();
    const updated = toNormalizedSettings({ ...current, ...partial });
    saveSettings(updated);
    return updated;
}
export async function resetToDefaultSettings(): Promise<Settings> {
    const defaultPath = await fetchDefaultPath();
    const defaultSettings = toNormalizedSettings({
        ...DEFAULT_SETTINGS,
        downloadPath: defaultPath,
    });
    saveSettings(defaultSettings);
    return defaultSettings;
}
export async function loadCustomFonts(): Promise<CustomFontOption[]> {
    return normalizeCustomFonts(getSettings().customFonts);
}
export async function saveCustomFonts(customFonts: CustomFontOption[]): Promise<CustomFontOption[]> {
    const normalizedFonts = normalizeCustomFonts(customFonts);
    const settings = getSettings();
    saveSettings({
        ...settings,
        customFonts: normalizedFonts,
        fontFamily: normalizeFontFamily(settings.fontFamily, normalizedFonts),
    });
    return normalizedFonts;
}
export function applyThemeMode(mode: "auto" | "light" | "dark"): void {
    if (mode === "auto") {
        const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
        if (prefersDark) {
            document.documentElement.classList.add("dark");
        }
        else {
            document.documentElement.classList.remove("dark");
        }
    }
    else if (mode === "dark") {
        document.documentElement.classList.add("dark");
    }
    else {
        document.documentElement.classList.remove("dark");
    }
}
