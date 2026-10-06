import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import type { Provider } from "@/types";
import { piApi, providersApi, type AppId } from "@/lib/api";
import { openclawKeys } from "@/hooks/useOpenClaw";
import { hermesKeys } from "@/hooks/useHermes";
import { extractErrorMessage } from "@/utils/errorUtils";
import { deepClone } from "@/utils/deepClone";

type DuplicatedProvider = Omit<Provider, "id" | "createdAt"> & {
  providerKey?: string;
  addToLive?: boolean;
};

/** Apps that key providers by id inside a live config file. */
const KEYED_APPS: readonly AppId[] = ["opencode", "openclaw", "hermes", "pi"];

export function generateUniqueProviderCopyKey(
  originalKey: string,
  existingKeys: string[],
): string {
  const baseKey = `${originalKey}-copy`;

  if (!existingKeys.includes(baseKey)) {
    return baseKey;
  }

  let counter = 2;
  while (existingKeys.includes(`${baseKey}-${counter}`)) {
    counter++;
  }
  return `${baseKey}-${counter}`;
}

const fetchLiveProviderIds = async (
  appId: AppId,
  queryClient: QueryClient,
): Promise<string[]> => {
  switch (appId) {
    case "opencode":
      return queryClient.ensureQueryData({
        queryKey: ["opencodeLiveProviderIds"],
        queryFn: () => providersApi.getOpenCodeLiveProviderIds(),
      });
    case "openclaw":
      return queryClient.ensureQueryData({
        queryKey: openclawKeys.liveProviderIds,
        queryFn: () => providersApi.getOpenClawLiveProviderIds(),
      });
    case "hermes":
      return queryClient.ensureQueryData({
        queryKey: hermesKeys.liveProviderIds,
        queryFn: () => providersApi.getHermesLiveProviderIds(),
      });
    default:
      return (
        await queryClient.ensureQueryData({
          queryKey: ["pi", "currentState"],
          queryFn: () => piApi.getCurrentState(),
        })
      ).enabledProviderIds;
  }
};

/**
 * Duplicates a provider, giving the copy a free key for apps that write a
 * live config and shifting the sort order of everything below it.
 */
export function useDuplicateProvider(
  activeApp: AppId,
  providers: Record<string, Provider>,
  addProvider: (provider: DuplicatedProvider) => Promise<void> | void,
) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  return useCallback(
    async (provider: Provider) => {
      const newSortIndex =
        provider.sortIndex !== undefined ? provider.sortIndex + 1 : undefined;

      const duplicatedProvider: DuplicatedProvider = {
        name: `${provider.name} copy`,
        settingsConfig: deepClone(provider.settingsConfig),
        websiteUrl: provider.websiteUrl,
        category: provider.category,
        sortIndex: newSortIndex, // 复制原 sortIndex + 1
        meta: provider.meta ? deepClone(provider.meta) : undefined,
        icon: provider.icon,
        iconColor: provider.iconColor,
      };

      if (KEYED_APPS.includes(activeApp)) {
        let liveProviderIds: string[] = [];
        try {
          liveProviderIds = await fetchLiveProviderIds(activeApp, queryClient);
        } catch (error) {
          console.error(
            "[App] Failed to load live provider IDs for duplication",
            error,
          );
          const errorMessage = extractErrorMessage(error);
          toast.error(
            t("provider.duplicateLiveIdsLoadFailed", {
              defaultValue: "读取配置中的供应商标识失败，请先修复配置后再试",
            }) + (errorMessage ? `: ${errorMessage}` : ""),
          );
          return;
        }
        const existingKeys = Array.from(
          new Set([...Object.keys(providers), ...liveProviderIds]),
        );
        duplicatedProvider.providerKey = generateUniqueProviderCopyKey(
          provider.id,
          existingKeys,
        );
        duplicatedProvider.addToLive = false;
      }

      if (provider.sortIndex !== undefined) {
        const updates = Object.values(providers)
          .filter(
            (p) =>
              p.sortIndex !== undefined &&
              p.sortIndex >= newSortIndex! &&
              p.id !== provider.id,
          )
          .map((p) => ({
            id: p.id,
            sortIndex: p.sortIndex! + 1,
          }));

        if (updates.length > 0) {
          try {
            await providersApi.updateSortOrder(updates, activeApp);
          } catch (error) {
            console.error("[App] Failed to update sort order", error);
            toast.error(
              t("provider.sortUpdateFailed", {
                defaultValue: "排序更新失败",
              }),
            );
            return; // 如果排序更新失败，不继续添加
          }
        }
      }

      await addProvider(duplicatedProvider);
    },
    [activeApp, addProvider, providers, queryClient, t],
  );
}
