import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useQueryClient } from "@tanstack/react-query";
import { providersApi } from "@/lib/api";
import { useTauriEvent } from "@/hooks/useTauriEvent";

interface SyncStatusUpdatedPayload {
  source?: string;
  status?: string;
  error?: string;
}

/**
 * App-wide backend event subscriptions: cache invalidation after external
 * mutations plus the toasts for background sync failures.
 */
export function useAppEventBridges(): void {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  useTauriEvent("universal-provider-synced", async () => {
    await queryClient.invalidateQueries({ queryKey: ["providers"] });
    try {
      await providersApi.updateTrayMenu();
    } catch (error) {
      console.error("[App] Failed to update tray menu", error);
    }
  });

  // Shared body for the two remote-sync channels. Not a hook itself, so the
  // `useTauriEvent` calls below stay at the top level of this hook.
  const handleSyncStatus = async (
    payload: SyncStatusUpdatedPayload | null | undefined,
    failureKey: string,
  ) => {
    const statusPayload = payload ?? {};
    await queryClient.invalidateQueries({ queryKey: ["settings"] });
    if (statusPayload.source !== "auto" || statusPayload.status !== "error") {
      return;
    }
    toast.error(
      t(failureKey, {
        error: statusPayload.error || t("common.unknown"),
      }),
    );
  };

  useTauriEvent<SyncStatusUpdatedPayload | null | undefined>(
    "webdav-sync-status-updated",
    (payload) =>
      handleSyncStatus(payload, "settings.webdavSync.autoSyncFailedToast"),
  );

  useTauriEvent<SyncStatusUpdatedPayload | null | undefined>(
    "s3-sync-status-updated",
    (payload) =>
      handleSyncStatus(payload, "settings.s3Sync.autoSyncFailedToast"),
  );

  useTauriEvent<{ appType: string; providerName: string }>(
    "proxy-official-warning",
    (payload) => {
      toast.warning(
        t("notifications.proxyOfficialWarning", {
          name: payload.providerName,
          defaultValue: `当前供应商 ${payload.providerName} 是官方供应商，建议切换到第三方供应商后再使用代理接管`,
        }),
        { duration: 8000 },
      );
    },
  );
}
