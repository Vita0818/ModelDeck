import { useEffect } from "react";

type IdleHandle = { cancel: () => void };

const scheduleIdle = (run: () => void): IdleHandle => {
  if (typeof window.requestIdleCallback === "function") {
    const id = window.requestIdleCallback(run, { timeout: 3000 });
    return { cancel: () => window.cancelIdleCallback?.(id) };
  }
  const id = window.setTimeout(run, 1200);
  return { cancel: () => window.clearTimeout(id) };
};

/**
 * Warm up lazily loaded chunks once the app is idle.
 *
 * Code-splitting keeps a chunk off the critical path, but it also means the
 * first interaction pays the download. Prefetching during idle time gives us
 * the smaller initial bundle without the click-time stall.
 */
export function useIdlePreload(loaders: Array<() => Promise<unknown>>): void {
  useEffect(() => {
    const handle = scheduleIdle(() => {
      for (const load of loaders) {
        void load().catch(() => {
          // A failed prefetch is harmless: React.lazy retries on render.
        });
      }
    });

    return handle.cancel;
    // Loader identities are module-level constants at every call site.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
