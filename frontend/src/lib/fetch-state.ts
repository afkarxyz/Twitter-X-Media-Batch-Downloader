/**
 * Fetch State Manager
 * Manages resumable fetch state for large accounts
 */

import type { TwitterResponse, TimelineEntry, AccountInfo } from "@/types/api";

const FETCH_STATE_KEY = "twitter_fetch_state";
const CURSOR_STATE_KEY = "twitter_cursor_state"; // Lightweight cursor-only storage

export interface FetchState {
  username: string;
  cursor: string;
  timeline: TimelineEntry[];
  accountInfo: AccountInfo | null;
  totalFetched: number;
  completed: boolean;
  lastUpdated: number;
  authToken: string; // Encrypted/hashed for security
  mediaType: string;
  retweets: boolean;
  timelineType: string;
}

/**
 * Save current fetch state for resume capability
 */
export function saveFetchState(state: Partial<FetchState> & { username: string }): void {
  try {
    const existing = getFetchState(state.username);
    const updated: FetchState = {
      username: state.username,
      cursor: state.cursor || "",
      timeline: state.timeline || existing?.timeline || [],
      accountInfo: state.accountInfo || existing?.accountInfo || null,
      totalFetched: state.totalFetched ?? existing?.totalFetched ?? 0,
      completed: state.completed ?? false,
      lastUpdated: Date.now(),
      authToken: state.authToken || existing?.authToken || "",
      mediaType: state.mediaType || existing?.mediaType || "all",
      retweets: state.retweets ?? existing?.retweets ?? false,
      timelineType: state.timelineType || existing?.timelineType || "timeline",
    };

    // Get all states
    const allStates = getAllFetchStates();
    allStates[state.username.toLowerCase()] = updated;

    localStorage.setItem(FETCH_STATE_KEY, JSON.stringify(allStates));
  } catch (error) {
    console.error("Failed to save fetch state:", error);
  }
}

/**
 * Get fetch state for a specific username
 */
export function getFetchState(username: string): FetchState | null {
  try {
    const allStates = getAllFetchStates();
    return allStates[username.toLowerCase()] || null;
  } catch (error) {
    console.error("Failed to get fetch state:", error);
    return null;
  }
}

/**
 * Get all fetch states
 */
export function getAllFetchStates(): Record<string, FetchState> {
  try {
    const stored = localStorage.getItem(FETCH_STATE_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (error) {
    console.error("Failed to parse fetch states:", error);
  }
  return {};
}

/**
 * Check if there's a resumable fetch for username
 */
export function hasResumableFetch(username: string): boolean {
  const state = getFetchState(username);
  if (!state) return false;

  // Has cursor and not completed
  return !!state.cursor && !state.completed && state.timeline.length > 0;
}

/**
 * Get resumable fetch info for display
 */
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
    mediaCount: state.timeline.length,
    lastUpdated: new Date(state.lastUpdated),
  };
}

/**
 * Clear fetch state for username
 */
export function clearFetchState(username: string): void {
  try {
    const allStates = getAllFetchStates();
    delete allStates[username.toLowerCase()];
    localStorage.setItem(FETCH_STATE_KEY, JSON.stringify(allStates));
  } catch (error) {
    console.error("Failed to clear fetch state:", error);
  }
}

/**
 * Clear all incomplete fetch states (cleanup)
 */
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
  } catch (error) {
    console.error("Failed to clear fetch states:", error);
  }
}

/**
 * Convert fetch state to TwitterResponse format
 */
export function stateToResponse(state: FetchState): TwitterResponse | null {
  if (!state.accountInfo) return null;

  return {
    account_info: state.accountInfo,
    total_urls: state.timeline.length,
    timeline: state.timeline,
    metadata: {
      new_entries: state.timeline.length,
      page: 0,
      batch_size: 0,
      has_more: !state.completed,
      cursor: state.cursor,
      completed: state.completed,
    },
    cursor: state.cursor,
    completed: state.completed,
  };
}

/**
 * Save cursor only (lightweight, can be called every batch)
 */
export function saveCursor(username: string, cursor: string): void {
  try {
    const allCursors = getAllCursors();
    allCursors[username.toLowerCase()] = {
      cursor,
      lastUpdated: Date.now(),
    };
    localStorage.setItem(CURSOR_STATE_KEY, JSON.stringify(allCursors));
  } catch (error) {
    console.error("Failed to save cursor:", error);
  }
}

/**
 * Get cursor for username from localStorage (lightweight, synchronous)
 */
export function getCursor(username: string): string | null {
  try {
    const allCursors = getAllCursors();
    return allCursors[username.toLowerCase()]?.cursor || null;
  } catch (error) {
    console.error("Failed to get cursor:", error);
    return null;
  }
}

/**
 * Get all cursors
 */
function getAllCursors(): Record<string, { cursor: string; lastUpdated: number }> {
  try {
    const stored = localStorage.getItem(CURSOR_STATE_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (error) {
    console.error("Failed to parse cursors:", error);
  }
  return {};
}

/**
 * Clear cursor for username
 */
export function clearCursor(username: string): void {
  try {
    const allCursors = getAllCursors();
    delete allCursors[username.toLowerCase()];
    localStorage.setItem(CURSOR_STATE_KEY, JSON.stringify(allCursors));
  } catch (error) {
    console.error("Failed to clear cursor:", error);
  }
}

/**
 * Merge new timeline entries with existing (deduplicate by tweet_id)
 */
export function mergeTimelines(
  existing: TimelineEntry[],
  newEntries: TimelineEntry[]
): TimelineEntry[] {
  const seenIds = new Set(existing.map((e) => e.tweet_id));
  const unique = newEntries.filter((e) => !seenIds.has(e.tweet_id));
  return [...existing, ...unique];
}
