import type { TimelineEntry } from "@/types/api";
const FETCH_STATE_KEY = "twitter_fetch_state";
const CURSOR_STATE_KEY = "twitter_cursor_state";
export interface FetchState {
    username: string;
    cursor: string;
    totalFetched: number;
    completed: boolean;
    lastUpdated: number;
    mediaType: string;
    retweets: boolean;
    timelineType: string;
}
function normalizeFetchState(state: FetchState | null | undefined): FetchState | null {
    if (!state) {
        return null;
    }
    return {
        ...state,
        cursor: state.cursor || "",
        totalFetched: state.totalFetched ?? 0,
        mediaType: state.mediaType || "all",
        retweets: state.retweets ?? false,
        timelineType: state.timelineType || "timeline",
    };
}
export function saveFetchState(state: Partial<FetchState> & {
    username: string;
}): void {
    try {
        const existing = getFetchState(state.username);
        const updated: FetchState = {
            username: state.username,
            cursor: state.cursor || "",
            totalFetched: state.totalFetched ?? existing?.totalFetched ?? 0,
            completed: state.completed ?? false,
            lastUpdated: Date.now(),
            mediaType: state.mediaType || existing?.mediaType || "all",
            retweets: state.retweets ?? existing?.retweets ?? false,
            timelineType: state.timelineType || existing?.timelineType || "timeline",
        };
        const allStates = getAllFetchStates();
        allStates[state.username.toLowerCase()] = updated;
        localStorage.setItem(FETCH_STATE_KEY, JSON.stringify(allStates));
    }
    catch (error) {
        console.error("Failed to save fetch state:", error);
    }
}
export function getFetchState(username: string): FetchState | null {
    try {
        const allStates = getAllFetchStates();
        return normalizeFetchState(allStates[username.toLowerCase()]);
    }
    catch (error) {
        console.error("Failed to get fetch state:", error);
        return null;
    }
}
export function getAllFetchStates(): Record<string, FetchState> {
    try {
        const stored = localStorage.getItem(FETCH_STATE_KEY);
        if (stored) {
            const parsed = JSON.parse(stored);
            if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
                const normalized: Record<string, FetchState> = {};
                for (const [key, value] of Object.entries(parsed as Record<string, FetchState | null | undefined>)) {
                    const state = normalizeFetchState(value);
                    if (state) {
                        normalized[key] = state;
                    }
                }
                return normalized;
            }
        }
    }
    catch (error) {
        console.error("Failed to parse fetch states:", error);
    }
    return {};
}
export function hasResumableFetch(username: string): boolean {
    const state = getFetchState(username);
    if (!state)
        return false;
    return !!state.cursor && !state.completed && state.totalFetched > 0;
}
export function getResumableInfo(username: string): {
    canResume: boolean;
    mediaCount: number;
    lastUpdated: Date | null;
} {
    const state = getFetchState(username);
    if (!state || state.completed || !state.cursor) {
        return { canResume: false, mediaCount: 0, lastUpdated: null };
    }
    return {
        canResume: true,
        mediaCount: state.totalFetched,
        lastUpdated: new Date(state.lastUpdated),
    };
}
export function clearFetchState(username: string): void {
    try {
        const allStates = getAllFetchStates();
        delete allStates[username.toLowerCase()];
        localStorage.setItem(FETCH_STATE_KEY, JSON.stringify(allStates));
    }
    catch (error) {
        console.error("Failed to clear fetch state:", error);
    }
}
export function clearAllIncompleteFetchStates(): void {
    try {
        const allStates = getAllFetchStates();
        const completed: Record<string, FetchState> = {};
        for (const [key, state] of Object.entries(allStates)) {
            if (state.completed) {
                completed[key] = state;
            }
        }
        localStorage.setItem(FETCH_STATE_KEY, JSON.stringify(completed));
    }
    catch (error) {
        console.error("Failed to clear fetch states:", error);
    }
}
export function saveCursor(username: string, cursor: string): void {
    try {
        const allCursors = getAllCursors();
        allCursors[username.toLowerCase()] = {
            cursor,
            lastUpdated: Date.now(),
        };
        localStorage.setItem(CURSOR_STATE_KEY, JSON.stringify(allCursors));
    }
    catch (error) {
        console.error("Failed to save cursor:", error);
    }
}
export function getCursor(username: string): string | null {
    try {
        const allCursors = getAllCursors();
        return allCursors[username.toLowerCase()]?.cursor || null;
    }
    catch (error) {
        console.error("Failed to get cursor:", error);
        return null;
    }
}
function getAllCursors(): Record<string, {
    cursor: string;
    lastUpdated: number;
}> {
    try {
        const stored = localStorage.getItem(CURSOR_STATE_KEY);
        if (stored) {
            return JSON.parse(stored);
        }
    }
    catch (error) {
        console.error("Failed to parse cursors:", error);
    }
    return {};
}
export function clearCursor(username: string): void {
    try {
        const allCursors = getAllCursors();
        delete allCursors[username.toLowerCase()];
        localStorage.setItem(CURSOR_STATE_KEY, JSON.stringify(allCursors));
    }
    catch (error) {
        console.error("Failed to clear cursor:", error);
    }
}
export function mergeTimelines(existing: TimelineEntry[], newEntries: TimelineEntry[]): TimelineEntry[] {
    const existingTimeline = Array.isArray(existing) ? existing : [];
    const incomingTimeline = Array.isArray(newEntries) ? newEntries : [];
    const seenIds = new Set(existingTimeline.map((e) => e.tweet_id));
    const unique = incomingTimeline.filter((e) => !seenIds.has(e.tweet_id));
    return [...existingTimeline, ...unique];
}
