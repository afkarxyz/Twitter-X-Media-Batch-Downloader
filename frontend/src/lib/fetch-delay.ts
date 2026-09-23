export function normalizeFetchDelay(value: unknown): number {
    return typeof value === "number" && Number.isFinite(value)
        ? Math.round(Math.min(3600, Math.max(0, value)) * 1000) / 1000
        : 0;
}

// One gate shared by all fetch paths, so switching accounts also respects pacing.
export function createFetchDelayGate({
    now = () => performance.now(),
    random = Math.random,
    sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms)),
} = {}) {
    let lastStarted: number | null = null;
    return async (baseSeconds: number, extraSeconds: number, stopped: () => boolean): Promise<boolean> => {
        const delay = (normalizeFetchDelay(baseSeconds) + random() * normalizeFetchDelay(extraSeconds)) * 1000;
        while (!stopped()) {
            const remaining = lastStarted === null ? 0 : lastStarted + delay - now();
            if (remaining <= 0) {
                lastStarted = now();
                return true;
            }
            // Recheck the shared timestamp after each wait to space concurrent callers too.
            await sleep(Math.min(remaining, 100));
        }
        return false;
    };
}
