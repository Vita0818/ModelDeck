import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettingsQuery } from "@/lib/query";
import { isLinux, isWindows } from "@/lib/platform";
import { extractErrorMessage } from "@/utils/errorUtils";

const DEFAULT_DRAG_BAR_HEIGHT = isWindows() || isLinux() ? 0 : 28; // px
export const HEADER_HEIGHT = 64; // px

export interface WindowChrome {
  /** Linux-only in-app window buttons, driven by the user setting. */
  useAppWindowControls: boolean;
  dragBarHeight: number;
  /** Top padding the main content needs to clear drag bar + header. */
  contentTopOffset: number;
  isWindowMaximized: boolean;
  minimize: () => Promise<void>;
  toggleMaximize: () => Promise<void>;
  close: () => Promise<void>;
}

/**
 * Owns the native window chrome: layout offsets, maximized state tracking and
 * the title-bar button handlers.
 */
export function useWindowChrome(): WindowChrome {
  const { t } = useTranslation();
  const { data: settingsData } = useSettingsQuery();
  const [isWindowMaximized, setIsWindowMaximized] = useState(false);

  const useAppWindowControls =
    isLinux() && (settingsData?.useAppWindowControls ?? false);
  const dragBarHeight = useAppWindowControls ? 32 : DEFAULT_DRAG_BAR_HEIGHT;

  useEffect(() => {
    let active = true;
    let unlistenResize: (() => void) | undefined;

    const setupWindowStateSync = async () => {
      try {
        const currentWindow = getCurrentWindow();
        const syncWindowMaximizedState = async () => {
          const maximized = await currentWindow.isMaximized();
          if (active) {
            setIsWindowMaximized(maximized);
          }
        };

        await syncWindowMaximizedState();
        unlistenResize = await currentWindow.onResized(() => {
          void syncWindowMaximizedState();
        });
      } catch (error) {
        console.error("[App] Failed to sync window maximized state", error);
      }
    };

    void setupWindowStateSync();
    return () => {
      active = false;
      unlistenResize?.();
    };
  }, []);

  useEffect(() => {
    // settingsData 未加载时跳过，避免用 fallback false 覆盖 Rust 侧已设好的装饰状态
    if (!settingsData) return;

    const syncWindowDecorations = async () => {
      try {
        await getCurrentWindow().setDecorations(!useAppWindowControls);
      } catch (error) {
        console.error("[App] Failed to update window decorations", error);
      }
    };

    void syncWindowDecorations();
  }, [useAppWindowControls, settingsData]);

  const notifyWindowControlError = useCallback(
    (error: unknown) => {
      toast.error(
        t("notifications.windowControlFailed", {
          defaultValue: "窗口控制失败：{{error}}",
          error: extractErrorMessage(error),
        }),
      );
    },
    [t],
  );

  const minimize = useCallback(async () => {
    try {
      await getCurrentWindow().minimize();
    } catch (error) {
      console.error("[App] Failed to minimize window", error);
      notifyWindowControlError(error);
    }
  }, [notifyWindowControlError]);

  const toggleMaximize = useCallback(async () => {
    try {
      const currentWindow = getCurrentWindow();
      await currentWindow.toggleMaximize();
      setIsWindowMaximized(await currentWindow.isMaximized());
    } catch (error) {
      console.error("[App] Failed to toggle maximize", error);
      notifyWindowControlError(error);
    }
  }, [notifyWindowControlError]);

  const close = useCallback(async () => {
    try {
      await getCurrentWindow().close();
    } catch (error) {
      console.error("[App] Failed to close window", error);
      notifyWindowControlError(error);
    }
  }, [notifyWindowControlError]);

  return {
    useAppWindowControls,
    dragBarHeight,
    contentTopOffset: dragBarHeight + HEADER_HEIGHT,
    isWindowMaximized,
    minimize,
    toggleMaximize,
    close,
  };
}
