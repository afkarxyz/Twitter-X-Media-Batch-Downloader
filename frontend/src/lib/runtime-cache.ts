export type DependencyName = "extractor" | "ffmpeg" | "exiftool";
export interface CachedDependencyStatus {
    installed: boolean | null;
    installedVersion: string | null;
    latestVersion: string | null;
}
export interface CachedMediaFolderStatus {
    folderExists: boolean;
    gifsFolderHasMP4: boolean;
}
const dependencyStatusCache: Record<DependencyName, CachedDependencyStatus> = {
    extractor: {
        installed: null,
        installedVersion: null,
        latestVersion: null,
    },
    ffmpeg: {
        installed: null,
        installedVersion: null,
        latestVersion: null,
    },
    exiftool: {
        installed: null,
        installedVersion: null,
        latestVersion: null,
    },
};
const mediaFolderStatusCache = new Map<string, CachedMediaFolderStatus>();
function getMediaFolderStatusKey(downloadPath: string, folderName: string): string {
    return `${downloadPath}::${folderName}`;
}
export function getCachedDependencyStatus(name: DependencyName): CachedDependencyStatus {
    return { ...dependencyStatusCache[name] };
}
export function setCachedDependencyStatus(name: DependencyName, status: Partial<CachedDependencyStatus>): void {
    dependencyStatusCache[name] = {
        ...dependencyStatusCache[name],
        ...status,
    };
}
export function getCachedMediaFolderStatus(downloadPath: string, folderName: string): CachedMediaFolderStatus | null {
    return mediaFolderStatusCache.get(getMediaFolderStatusKey(downloadPath, folderName)) ?? null;
}
export function setCachedMediaFolderStatus(downloadPath: string, folderName: string, status: CachedMediaFolderStatus): void {
    mediaFolderStatusCache.set(getMediaFolderStatusKey(downloadPath, folderName), status);
}
