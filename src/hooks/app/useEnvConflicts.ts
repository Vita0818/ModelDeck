import { useCallback, useEffect, useState } from "react";
import type { AppId } from "@/lib/api";
import type { EnvConflict } from "@/types/env";
import { checkAllEnvConflicts, checkEnvConflicts } from "@/lib/api/env";

const DISMISS_KEY = "env_banner_dismissed";

const conflictKey = (conflict: EnvConflict) =>
  `${conflict.varName}:${conflict.sourcePath}`;

export interface EnvConflictsState {
  conflicts: EnvConflict[];
  showBanner: boolean;
  /** Hide for the rest of the session. */
  dismiss: () => void;
  /** Re-read conflicts after the user deletes one. */
  refresh: () => Promise<void>;
}

/**
 * Tracks conflicting environment variables: a full scan on startup, plus an
 * incremental scan whenever the user moves to another client's pages.
 */
export function useEnvConflicts(
  activeApp: AppId,
  isModelClient: boolean,
  isProvidersView: boolean,
): EnvConflictsState {
  const [conflicts, setConflicts] = useState<EnvConflict[]>([]);
  const [showBanner, setShowBanner] = useState(false);

  const revealUnlessDismissed = useCallback(() => {
    if (!sessionStorage.getItem(DISMISS_KEY)) {
      setShowBanner(true);
    }
  }, []);

  useEffect(() => {
    const checkEnvOnStartup = async () => {
      try {
        const allConflicts = await checkAllEnvConflicts();
        const flatConflicts = Object.values(allConflicts).flat();

        if (flatConflicts.length > 0) {
          setConflicts(flatConflicts);
          revealUnlessDismissed();
        }
      } catch (error) {
        console.error(
          "[App] Failed to check environment conflicts on startup:",
          error,
        );
      }
    };

    void checkEnvOnStartup();
  }, [revealUnlessDismissed]);

  useEffect(() => {
    const checkEnvOnSwitch = async () => {
      try {
        if (isProvidersView) {
          setShowBanner(false);
          return;
        }
        if (isModelClient || activeApp === "mcode") return;
        const found = await checkEnvConflicts(activeApp);

        if (found.length > 0) {
          setConflicts((prev) => {
            const existingKeys = new Set(prev.map(conflictKey));
            return [
              ...prev,
              ...found.filter((c) => !existingKeys.has(conflictKey(c))),
            ];
          });
          revealUnlessDismissed();
        }
      } catch (error) {
        console.error(
          "[App] Failed to check environment conflicts on app switch:",
          error,
        );
      }
    };

    void checkEnvOnSwitch();
  }, [activeApp, isModelClient, isProvidersView, revealUnlessDismissed]);

  const dismiss = useCallback(() => {
    setShowBanner(false);
    sessionStorage.setItem(DISMISS_KEY, "true");
  }, []);

  const refresh = useCallback(async () => {
    try {
      const allConflicts = await checkAllEnvConflicts();
      const flatConflicts = Object.values(allConflicts).flat();
      setConflicts(flatConflicts);
      if (flatConflicts.length === 0) {
        setShowBanner(false);
      }
    } catch (error) {
      console.error(
        "[App] Failed to re-check conflicts after deletion:",
        error,
      );
    }
  }, []);

  return { conflicts, showBanner, dismiss, refresh };
}
