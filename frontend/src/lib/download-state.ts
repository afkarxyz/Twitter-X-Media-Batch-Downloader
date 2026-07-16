import { useSyncExternalStore } from "react";
import { EventsOn } from "../../wailsjs/runtime/runtime";
export interface DownloadProgress {
    current: number;
    total: number;
    percent: number;
}
export interface DownloadItemStatus {
    tweet_id: number;
    index: number;
    status: "success" | "failed" | "skipped" | "cancelled";
}
export type DownloadScope = "media" | "database" | "database-bulk";
export type DownloadItemResult = "success" | "failed" | "skipped";
export interface DownloadState {
    active: boolean;
    scope: DownloadScope | null;
    accountId: number | null;
    progress: DownloadProgress | null;
    currentItemKey: string | null;
    itemKeyByIndex: Record<number, string>;
    itemStatusByKey: Record<string, DownloadItemResult>;
}
const listeners = new Set<() => void>();
let state: DownloadState = {
    active: false,
    scope: null,
    accountId: null,
    progress: null,
    currentItemKey: null,
    itemKeyByIndex: {},
    itemStatusByKey: {},
};
let unsubscribeEvents: (() => void) | null = null;
function emit() {
    for (const listener of listeners) {
        listener();
    }
}
function setState(next: Partial<DownloadState>) {
    state = { ...state, ...next };
    emit();
}
export function initDownloadProgressEvents() {
    if (unsubscribeEvents) {
        return unsubscribeEvents;
    }
    const unsubscribeProgress = EventsOn("download-progress", (progress: DownloadProgress) => {
        if (!state.scope) {
            return;
        }
        setState({ progress });
    });
    const unsubscribeItemStatus = EventsOn("download-item-status", (status: DownloadItemStatus) => {
        if (status.status === "cancelled") {
            return;
        }
        const itemKey = state.currentItemKey || state.itemKeyByIndex[status.index] || `${String(status.tweet_id)}-${status.index}`;
        setState({
            itemStatusByKey: {
                ...state.itemStatusByKey,
                [itemKey]: status.status,
            },
        });
    });
    unsubscribeEvents = () => {
        unsubscribeProgress();
        unsubscribeItemStatus();
        unsubscribeEvents = null;
    };
    return unsubscribeEvents;
}
export function beginDownload(progress: DownloadProgress, meta: Pick<DownloadState, "scope"> & Partial<Pick<DownloadState, "accountId" | "currentItemKey" | "itemKeyByIndex">>) {
    setState({
        active: true,
        scope: meta.scope,
        accountId: meta.accountId ?? null,
        progress,
        currentItemKey: meta.currentItemKey ?? null,
        itemKeyByIndex: meta.itemKeyByIndex ?? {},
    });
}
export function finishDownload() {
    setState({
        active: false,
        scope: null,
        accountId: null,
        progress: null,
        currentItemKey: null,
        itemKeyByIndex: {},
    });
}
export function setDownloadItemStatus(itemKey: string, status: DownloadItemResult) {
    setState({
        itemStatusByKey: {
            ...state.itemStatusByKey,
            [itemKey]: status,
        },
    });
}
export function subscribeDownloadState(listener: () => void) {
    listeners.add(listener);
    return () => {
        listeners.delete(listener);
    };
}
export function getDownloadState() {
    return state;
}
export function useDownloadState() {
    return useSyncExternalStore(subscribeDownloadState, getDownloadState, getDownloadState);
}
