import {
  Suspense,
  lazy,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useQueryClient } from "@tanstack/react-query";
import {
  Settings,
  ArrowLeft,
  Minus,
  Maximize2,
  Minimize2,
  X,
  Brain,
  BarChart2,
  FolderOpen,
  KeyRound,
  Shield,
  Cpu,
  LayoutDashboard,
  Loader2,
} from "lucide-react";
import type { Provider, VisibleApps } from "@/types";
import { useProvidersQuery, useSettingsQuery } from "@/lib/query";
import {
  providersApi,
  settingsApi,
  type AppId,
  type ProviderSwitchEvent,
} from "@/lib/api";
import { useProviderActions } from "@/hooks/useProviderActions";
import { openclawKeys, useOpenClawHealth } from "@/hooks/useOpenClaw";
import { hermesKeys, useOpenHermesWebUI } from "@/hooks/useHermes";
import { hermesApi } from "@/lib/api/hermes";
import { useProxyStatus } from "@/hooks/useProxyStatus";
import { useUsageCacheBridge } from "@/hooks/useUsageCacheBridge";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { useLastValidValue } from "@/hooks/useLastValidValue";
import { useIdlePreload } from "@/hooks/useIdlePreload";
import { HEADER_HEIGHT, useWindowChrome } from "@/hooks/app/useWindowChrome";
import { useEnvConflicts } from "@/hooks/app/useEnvConflicts";
import { useStartupMigrations } from "@/hooks/app/useStartupMigrations";
import { useAppEventBridges } from "@/hooks/app/useAppEventBridges";
import { useDuplicateProvider } from "@/hooks/app/useDuplicateProvider";
import { resolveViewTitle, type View } from "@/components/app/viewTitle";
import {
  extractErrorMessage,
  translatePiProviderMutationError,
} from "@/utils/errorUtils";
import { isTextEditableTarget } from "@/utils/domUtils";
import { cn } from "@/lib/utils";
import { DRAG_REGION_ATTR, DRAG_REGION_STYLE } from "@/lib/platform";
import { AppSwitcher } from "@/components/AppSwitcher";
import { ProviderList } from "@/components/providers/ProviderList";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { UpdateBadge } from "@/components/UpdateBadge";
import { EnvWarningBanner } from "@/components/env/EnvWarningBanner";
import { RoutingActivationBrand } from "@/components/proxy/RoutingActivationBrand";
import { FirstRunNoticeDialog } from "@/components/FirstRunNoticeDialog";
import { Button } from "@/components/ui/button";
import {
  useDisableCurrentOmo,
  useDisableCurrentOmoSlim,
} from "@/lib/query/omo";
import { invalidatePiProviderCaches, usePiCurrentState } from "@/lib/query/pi";
import OpenClawHealthBanner from "@/components/openclaw/OpenClawHealthBanner";
import DeckPanel from "@/components/deck/DeckPanel";
import { isProxyAppId } from "@/config/appConfig";
import {
  CLIENT_IDS,
  CLIENT_STORAGE_KEY,
  initialClient,
  isClientId,
  isModelClientId,
  resolveClientVisibility,
  type ClientId,
} from "@/config/modeldeckClients";

// The provider form tree carries every preset catalog (~18k lines of static
// data), so it is split out and prefetched while the app is idle instead of
// being parsed before the first paint.
const loadAddProviderDialog = () =>
  import("@/components/providers/AddProviderDialog");
const loadEditProviderDialog = () =>
  import("@/components/providers/EditProviderDialog");

const AddProviderDialog = lazy(() =>
  loadAddProviderDialog().then((m) => ({ default: m.AddProviderDialog })),
);
const EditProviderDialog = lazy(() =>
  loadEditProviderDialog().then((m) => ({ default: m.EditProviderDialog })),
);

const PROVIDER_DIALOG_PRELOADERS = [
  loadAddProviderDialog,
  loadEditProviderDialog,
];

// Secondary views and modals are code-split: only the provider list is part of
// the first screen, so everything else loads the first time it is opened.
const SettingsPage = lazy(() =>
  import("@/components/settings/SettingsPage").then((m) => ({
    default: m.SettingsPage,
  })),
);
const HermesMemoryPanel = lazy(
  () => import("@/components/hermes/HermesMemoryPanel"),
);
const AgentsPanel = lazy(() =>
  import("@/components/agents/AgentsPanel").then((m) => ({
    default: m.AgentsPanel,
  })),
);
const UniversalProviderPanel = lazy(() =>
  import("@/components/universal").then((m) => ({
    default: m.UniversalProviderPanel,
  })),
);
const WorkspaceFilesPanel = lazy(
  () => import("@/components/workspace/WorkspaceFilesPanel"),
);
const EnvPanel = lazy(() => import("@/components/openclaw/EnvPanel"));
const ToolsPanel = lazy(() => import("@/components/openclaw/ToolsPanel"));
const AgentsDefaultsPanel = lazy(
  () => import("@/components/openclaw/AgentsDefaultsPanel"),
);
const UsageScriptModal = lazy(() => import("@/components/UsageScriptModal"));
const DeepLinkImportDialog = lazy(() =>
  import("@/components/DeepLinkImportDialog").then((m) => ({
    default: m.DeepLinkImportDialog,
  })),
);

function ViewLoadingFallback() {
  return (
    <div
      className="flex flex-1 items-center justify-center py-16"
      role="status"
      aria-busy="true"
    >
      <Loader2 className="h-6 w-6 animate-spin text-muted-foreground/40" />
    </div>
  );
}

const VIEW_STORAGE_KEY = "cc-switch-last-view";
const VALID_VIEWS: View[] = [
  "providers",
  "settings",
  "agents",
  "universal",
  "workspace",
  "hermesMemory",
];

const getInitialView = (): View => {
  const saved = localStorage.getItem(VIEW_STORAGE_KEY) as View | null;
  if (saved === "universal") return "providers";
  if (saved && VALID_VIEWS.includes(saved)) {
    return saved;
  }
  return "providers";
};

function App() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const [activeClient, setActiveClient] = useState<ClientId>(initialClient);
  // Keep the last provider client while a model adapter is open. Existing
  // provider hooks retain their state without targeting another client's data.
  const [activeApp, setActiveApp] = useState<AppId>(() => {
    const client = initialClient();
    return isModelClientId(client) ? "claude" : client;
  });
  const isModelClient = isModelClientId(activeClient);
  const [modelClientBusy, setModelClientBusy] = useState(false);
  const sharedFeatureApp: AppId =
    activeApp === "claude-desktop" ? "claude" : activeApp;
  const [currentView, setCurrentView] = useState<View>(getInitialView);
  const [settingsDefaultTab, setSettingsDefaultTab] = useState("general");
  const [isAddOpen, setIsAddOpen] = useState(false);

  useEffect(() => {
    localStorage.setItem(VIEW_STORAGE_KEY, currentView);
  }, [currentView]);

  const { data: settingsData } = useSettingsQuery();
  const {
    useAppWindowControls,
    dragBarHeight,
    contentTopOffset,
    isWindowMaximized,
    minimize: handleWindowMinimize,
    toggleMaximize: handleWindowToggleMaximize,
    close: handleWindowClose,
  } = useWindowChrome();
  useStartupMigrations();
  useAppEventBridges();
  const visibleApps = useMemo<VisibleApps>(
    () => resolveClientVisibility(settingsData?.visibleApps),
    [settingsData?.visibleApps],
  );

  const selectClient = (client: ClientId) => {
    if (modelClientBusy || !isClientId(client)) return;
    setActiveClient(client);
    setCurrentView("providers");
    if (!isModelClientId(client)) {
      setActiveApp(client);
    }
  };
  useEffect(() => {
    localStorage.setItem(CLIENT_STORAGE_KEY, activeClient);
  }, [activeClient]);
  useEffect(() => {
    if (!visibleApps[activeClient]) {
      selectClient(CLIENT_IDS.find((id) => visibleApps[id]) ?? "codex");
    }
  }, [visibleApps, activeClient, modelClientBusy]);
  useTauriEvent<{ client: string }>("modeldeck-client-selected", (payload) => {
    if (isClientId(payload.client)) selectClient(payload.client);
  });

  const [editingProvider, setEditingProvider] = useState<Provider | null>(null);
  const [usageProvider, setUsageProvider] = useState<Provider | null>(null);
  const [confirmAction, setConfirmAction] = useState<{
    provider: Provider;
    action: "remove" | "delete";
  } | null>(null);
  const {
    conflicts: envConflicts,
    showBanner: showEnvBanner,
    dismiss: dismissEnvBanner,
    refresh: refreshEnvConflicts,
  } = useEnvConflicts(activeApp, isModelClient, currentView === "providers");

  const effectiveEditingProvider = useLastValidValue(editingProvider);
  const effectiveUsageProvider = useLastValidValue(usageProvider);

  // Latches: once a provider dialog has been opened we keep it mounted.
  const [hasOpenedAddDialog, setHasOpenedAddDialog] = useState(false);
  const [hasOpenedEditDialog, setHasOpenedEditDialog] = useState(false);

  useEffect(() => {
    if (isAddOpen) setHasOpenedAddDialog(true);
  }, [isAddOpen]);

  useEffect(() => {
    if (editingProvider) setHasOpenedEditDialog(true);
  }, [editingProvider]);

  useIdlePreload(PROVIDER_DIALOG_PRELOADERS);
  const mainScrollRef = useRef<HTMLElement>(null);
  const providerScrollContainerRef = useRef<HTMLDivElement>(null);

  useUsageCacheBridge();

  useLayoutEffect(() => {
    if (currentView !== "providers") return;

    for (const container of [
      mainScrollRef.current,
      providerScrollContainerRef.current,
    ]) {
      if (container) {
        container.scrollTop = 0;
        container.scrollLeft = 0;
      }
    }
  }, [activeApp, currentView]);

  const {
    isRunning: isProxyRunning,
    takeoverStatus,
    status: proxyStatus,
  } = useProxyStatus();
  const proxyAppId = isProxyAppId(activeApp) ? activeApp : null;
  const currentAppUsesProxy =
    !isModelClient && (proxyAppId !== null || activeApp === "claude-desktop");
  const isCurrentAppTakeoverActive = proxyAppId
    ? takeoverStatus?.[proxyAppId] || false
    : false;
  const activeProviderId = useMemo(() => {
    if (!proxyAppId) return undefined;
    const target = proxyStatus?.active_targets?.find(
      (t) => t.app_type === proxyAppId,
    );
    return target?.provider_id;
  }, [proxyStatus?.active_targets, proxyAppId]);

  const { data, isLoading, refetch } = useProvidersQuery(activeApp, {
    enabled: currentView !== "providers",
    isProxyRunning: currentAppUsesProxy && isProxyRunning,
  });
  const { data: piCurrentState } = usePiCurrentState(activeApp === "pi");
  const providers = useMemo(() => data?.providers ?? {}, [data]);
  const currentProviderId = data?.currentProviderId ?? "";
  const isOpenClawView =
    activeApp === "openclaw" &&
    (currentView === "providers" ||
      currentView === "workspace" ||
      currentView === "openclawEnv" ||
      currentView === "openclawTools" ||
      currentView === "openclawAgents");
  const { data: openclawHealthWarnings = [] } =
    useOpenClawHealth(isOpenClawView);
  const {
    addProvider,
    updateProvider,
    switchProvider,
    deleteProvider,
    saveUsageScript,
    setAsDefaultModel,
  } = useProviderActions(
    activeApp,
    currentAppUsesProxy && isProxyRunning,
    isProxyRunning && isCurrentAppTakeoverActive,
  );
  const handleEnablePiProvider = async (provider: Provider) => {
    try {
      await providersApi.switch(provider.id, "pi");
      await invalidatePiProviderCaches(queryClient);
      await providersApi.updateTrayMenu().catch((error) => {
        console.error(
          "Failed to update tray menu after enabling Pi provider",
          error,
        );
      });
      toast.success(
        t("pi.provider.enabled", {
          defaultValue: "已在 Pi 中启用",
        }),
        { closeButton: true },
      );
    } catch (error) {
      const detail = extractErrorMessage(error);
      toast.error(
        t("pi.provider.enableFailed", {
          defaultValue: "无法在 Pi 中启用此供应商",
        }),
        {
          description:
            translatePiProviderMutationError(detail, t) || detail || undefined,
          closeButton: true,
        },
      );
    }
  };

  const disableOmoMutation = useDisableCurrentOmo();
  const handleDisableOmo = () => {
    disableOmoMutation.mutate(undefined, {
      onSuccess: () => {
        toast.success(t("omo.disabled", { defaultValue: "OMO 已停用" }));
      },
      onError: (error: Error) => {
        toast.error(
          t("omo.disableFailed", {
            defaultValue: "停用 OMO 失败: {{error}}",
            error: extractErrorMessage(error),
          }),
        );
      },
    });
  };

  const disableOmoSlimMutation = useDisableCurrentOmoSlim();
  const handleDisableOmoSlim = () => {
    disableOmoSlimMutation.mutate(undefined, {
      onSuccess: () => {
        toast.success(t("omo.disabled", { defaultValue: "OMO 已停用" }));
      },
      onError: (error: Error) => {
        toast.error(
          t("omo.disableFailed", {
            defaultValue: "停用 OMO 失败: {{error}}",
            error: extractErrorMessage(error),
          }),
        );
      },
    });
  };

  useEffect(() => {
    let unsubscribe: (() => void) | undefined;
    let active = true;

    const setupListener = async () => {
      try {
        const off = await providersApi.onSwitched(
          async (event: ProviderSwitchEvent) => {
            if (event.appType === activeApp) {
              await refetch();
            }
            if (event.appType === "pi") {
              await invalidatePiProviderCaches(queryClient);
            }
          },
        );
        if (!active) {
          off();
          return;
        }
        unsubscribe = off;
      } catch (error) {
        console.error("[App] Failed to subscribe provider switch event", error);
      }
    };

    void setupListener();
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [activeApp, queryClient, refetch]);

  const currentViewRef = useRef(currentView);
  const managementBusy = modelClientBusy;
  const managementBusyRef = useRef(false);
  managementBusyRef.current = managementBusy;

  useEffect(() => {
    currentViewRef.current = currentView;
  }, [currentView]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "," && (event.metaKey || event.ctrlKey)) {
        if (managementBusyRef.current) {
          event.preventDefault();
          return;
        }
        event.preventDefault();
        setCurrentView("settings");
        return;
      }

      if (event.key !== "Escape" || event.defaultPrevented) return;

      if (document.body.style.overflow === "hidden") return;

      const view = currentViewRef.current;
      if (view === "providers") return;
      if (managementBusyRef.current) return;

      if (isTextEditableTarget(event.target)) return;

      event.preventDefault();
      setCurrentView("providers");
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  const [launchDashboardOpen, setLaunchDashboardOpen] = useState(false);
  const openHermesWebUI = useOpenHermesWebUI(() =>
    setLaunchDashboardOpen(true),
  );

  const handleOpenWebsite = async (url: string) => {
    try {
      await settingsApi.openExternal(url);
    } catch (error) {
      const detail =
        extractErrorMessage(error) ||
        t("notifications.openLinkFailed", {
          defaultValue: "链接打开失败",
        });
      toast.error(detail);
    }
  };

  const handleEditProvider = async ({
    provider,
    originalId,
  }: {
    provider: Provider;
    originalId?: string;
  }) => {
    await updateProvider(provider, originalId);
    setEditingProvider(null);
  };

  const handleConfirmAction = async () => {
    if (!confirmAction) return;
    const { provider, action } = confirmAction;

    if (action === "remove") {
      // Remove from live config only (for additive mode apps like OpenCode/OpenClaw)
      // Does NOT delete from database - provider remains in the list
      try {
        await providersApi.removeFromLiveConfig(provider.id, activeApp);
      } catch (error) {
        const detail = extractErrorMessage(error);
        const description =
          activeApp === "pi"
            ? translatePiProviderMutationError(detail, t) || detail
            : detail;
        if (activeApp === "pi") {
          void invalidatePiProviderCaches(queryClient).catch(() => undefined);
        }
        toast.error(t("notifications.removeFromConfigFailed"), {
          description: description || t("common.unknown"),
          closeButton: true,
        });
        return;
      }
      if (activeApp === "pi") {
        await invalidatePiProviderCaches(queryClient);
      }
      // Invalidate queries to refresh the isInConfig state
      if (activeApp === "opencode") {
        await queryClient.invalidateQueries({
          queryKey: ["opencodeLiveProviderIds"],
        });
      } else if (activeApp === "openclaw") {
        await queryClient.invalidateQueries({
          queryKey: openclawKeys.liveProviderIds,
        });
        await queryClient.invalidateQueries({
          queryKey: openclawKeys.health,
        });
      } else if (activeApp === "hermes") {
        await queryClient.invalidateQueries({
          queryKey: hermesKeys.liveProviderIds,
        });
      } else if (activeApp === "mcode") {
        await queryClient.invalidateQueries({
          queryKey: ["providers", "mcode"],
        });
      }
      toast.success(
        activeApp === "pi"
          ? t("pi.provider.removed", {
              defaultValue: "已从 Pi 移除",
            })
          : t("notifications.removeFromConfigSuccess", {
              defaultValue: "已从配置移除",
            }),
        { closeButton: true },
      );
    } else {
      await deleteProvider(provider.id);
    }
    setConfirmAction(null);
  };

  const handleDuplicateProvider = useDuplicateProvider(
    activeApp,
    providers,
    addProvider,
  );

  const confirmActionMessage = useMemo(() => {
    if (!confirmAction) return "";

    const message =
      confirmAction.action === "remove"
        ? t("confirm.removeProviderMessage", {
            name: confirmAction.provider.name,
          })
        : t("confirm.deleteProviderMessage", {
            name: confirmAction.provider.name,
          });
    const isPiGlobalDefault =
      activeApp === "pi" &&
      piCurrentState?.defaultProviderId === confirmAction.provider.id;

    return isPiGlobalDefault
      ? `${message}\n\n${t("confirm.piDefaultProviderWarning")}`
      : message;
  }, [activeApp, confirmAction, piCurrentState?.defaultProviderId, t]);

  const handleOpenTerminal = async (provider: Provider) => {
    try {
      const selectedDir = await settingsApi.pickDirectory();
      if (!selectedDir) {
        return;
      }

      await providersApi.openTerminal(provider.id, activeApp, {
        cwd: selectedDir,
      });
      toast.success(
        t("provider.terminalOpened", {
          defaultValue: "终端已打开",
        }),
      );
    } catch (error) {
      console.error("[App] Failed to open terminal", error);
      const errorMessage = extractErrorMessage(error);
      toast.error(
        t("provider.terminalOpenFailed", {
          defaultValue: "打开终端失败",
        }) + (errorMessage ? `: ${errorMessage}` : ""),
      );
    }
  };

  const handleImportSuccess = async () => {
    try {
      await queryClient.invalidateQueries({
        queryKey: ["providers"],
        refetchType: "all",
      });
      await queryClient.refetchQueries({
        queryKey: ["providers"],
        type: "all",
      });
    } catch (error) {
      console.error("[App] Failed to refresh providers after import", error);
      await refetch();
    }
    try {
      await providersApi.updateTrayMenu();
    } catch (error) {
      console.error("[App] Failed to refresh tray menu", error);
    }
  };

  const renderContent = () => {
    // Provider cards contain nested exit animations. Unmount that tree when
    // opening this independent page, so WebKit cannot leave navigation waiting
    // for an occluded card animation to finish.
    if (currentView === "providers") return null;
    const content = (() => {
      switch (currentView) {
        case "settings":
          return (
            <SettingsPage
              open={true}
              onOpenChange={() => setCurrentView("providers")}
              onImportSuccess={handleImportSuccess}
              defaultTab={settingsDefaultTab}
            />
          );
        case "hermesMemory":
          return <HermesMemoryPanel />;
        case "agents":
          return (
            <AgentsPanel onOpenChange={() => setCurrentView("providers")} />
          );
        case "universal":
          return (
            <div className="px-6 pt-4">
              <UniversalProviderPanel />
            </div>
          );

        case "workspace":
          return <WorkspaceFilesPanel />;
        case "openclawEnv":
          return <EnvPanel />;
        case "openclawTools":
          return <ToolsPanel />;
        case "openclawAgents":
          return <AgentsDefaultsPanel />;
        default:
          return (
            <div className="px-6 flex flex-col flex-1 min-h-0 overflow-hidden">
              <div
                ref={providerScrollContainerRef}
                className="flex-1 overflow-y-auto overflow-x-hidden pb-12 px-1"
              >
                <div key={activeApp} className="space-y-4">
                  <ProviderList
                    providers={providers}
                    currentProviderId={currentProviderId}
                    appId={activeApp}
                    isLoading={isLoading}
                    isProxyRunning={currentAppUsesProxy && isProxyRunning}
                    isProxyTakeover={
                      isProxyRunning && isCurrentAppTakeoverActive
                    }
                    activeProviderId={activeProviderId}
                    onSwitch={
                      activeApp === "pi"
                        ? handleEnablePiProvider
                        : switchProvider
                    }
                    onEdit={(provider) => {
                      setEditingProvider(provider);
                    }}
                    onDelete={(provider) =>
                      setConfirmAction({ provider, action: "delete" })
                    }
                    onRemoveFromConfig={
                      activeApp === "opencode" ||
                      activeApp === "openclaw" ||
                      activeApp === "hermes" ||
                      activeApp === "pi" ||
                      activeApp === "mcode"
                        ? (provider) =>
                            setConfirmAction({ provider, action: "remove" })
                        : undefined
                    }
                    onDisableOmo={
                      activeApp === "opencode" ? handleDisableOmo : undefined
                    }
                    onDisableOmoSlim={
                      activeApp === "opencode"
                        ? handleDisableOmoSlim
                        : undefined
                    }
                    onDuplicate={handleDuplicateProvider}
                    onConfigureUsage={setUsageProvider}
                    onOpenWebsite={handleOpenWebsite}
                    onOpenTerminal={
                      activeApp === "claude" ? handleOpenTerminal : undefined
                    }
                    onCreate={() => setIsAddOpen(true)}
                    onSetAsDefault={
                      activeApp === "openclaw"
                        ? setAsDefaultModel
                        : activeApp === "hermes"
                          ? switchProvider
                          : undefined
                    }
                  />
                </div>
              </div>
            </div>
          );
      }
    })();

    return (
      <div key={currentView} className="flex flex-1 min-h-0 flex-col">
        <Suspense fallback={<ViewLoadingFallback />}>{content}</Suspense>
      </div>
    );
  };

  return (
    <div
      className="flex flex-col h-screen overflow-hidden bg-background text-foreground selection:bg-primary/30 pb-4"
      style={{ overflowX: "hidden", paddingTop: contentTopOffset }}
    >
      {(dragBarHeight > 0 || useAppWindowControls) && (
        <div
          className="fixed top-0 left-0 right-0 z-[70] flex items-center justify-end px-2"
          data-tauri-drag-region
          style={{ WebkitAppRegion: "drag", height: dragBarHeight } as any}
        >
          {useAppWindowControls && (
            <div
              className="flex items-center gap-1"
              style={{ WebkitAppRegion: "no-drag" } as any}
            >
              <Button
                variant="ghost"
                size="icon"
                onClick={() => void handleWindowMinimize()}
                title={t("header.windowMinimize")}
                className="h-7 w-7"
              >
                <Minus className="w-4 h-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => void handleWindowToggleMaximize()}
                title={
                  isWindowMaximized
                    ? t("header.windowRestore")
                    : t("header.windowMaximize")
                }
                className="h-7 w-7"
              >
                {isWindowMaximized ? (
                  <Minimize2 className="w-4 h-4" />
                ) : (
                  <Maximize2 className="w-4 h-4" />
                )}
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => void handleWindowClose()}
                title={t("header.windowClose")}
                className="h-7 w-7 hover:bg-red-500/15 hover:text-red-500"
              >
                <X className="w-4 h-4" />
              </Button>
            </div>
          )}
        </div>
      )}
      {showEnvBanner && envConflicts.length > 0 && (
        <EnvWarningBanner
          conflicts={envConflicts}
          onDismiss={dismissEnvBanner}
          onDeleted={refreshEnvConflicts}
        />
      )}

      <header
        className="fixed z-50 w-full transition-all duration-300 bg-background/80 backdrop-blur-md"
        {...DRAG_REGION_ATTR}
        style={
          {
            ...DRAG_REGION_STYLE,
            top: dragBarHeight,
            height: HEADER_HEIGHT,
          } as any
        }
      >
        <div
          className="flex h-full items-center justify-between gap-2 px-6"
          {...DRAG_REGION_ATTR}
          style={{ ...DRAG_REGION_STYLE } as any}
        >
          <div
            className="flex items-center gap-1"
            style={{ WebkitAppRegion: "no-drag" } as any}
          >
            {currentView !== "providers" ? (
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="icon"
                  disabled={managementBusy}
                  aria-label={t("common.back")}
                  onClick={() => setCurrentView("providers")}
                  className={cn(
                    "mr-2 rounded-lg",
                    managementBusy && "disabled:opacity-100",
                  )}
                >
                  <ArrowLeft className="w-4 h-4" />
                </Button>
                <h1 className="text-lg font-semibold">
                  {resolveViewTitle(currentView, t, sharedFeatureApp)}
                </h1>
              </div>
            ) : (
              <div className="flex items-center gap-2">
                <RoutingActivationBrand
                  active={
                    !isModelClient &&
                    isProxyRunning &&
                    isCurrentAppTakeoverActive
                  }
                  contextKey={activeClient}
                  ready={
                    proxyStatus !== undefined && takeoverStatus !== undefined
                  }
                />
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => {
                    setSettingsDefaultTab("general");
                    setCurrentView("settings");
                  }}
                  disabled={modelClientBusy}
                  title={t("common.settings")}
                  className="hover:bg-black/5 dark:hover:bg-white/5"
                >
                  <Settings className="w-4 h-4" />
                </Button>
                <UpdateBadge
                  onClick={() => {
                    setSettingsDefaultTab("about");
                    setCurrentView("settings");
                  }}
                />
                {!isModelClient && isCurrentAppTakeoverActive && (
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => {
                      setSettingsDefaultTab("usage");
                      setCurrentView("settings");
                    }}
                    title={t("usage.title", {
                      defaultValue: "使用统计",
                    })}
                    className="hover:bg-black/5 dark:hover:bg-white/5"
                  >
                    <BarChart2 className="w-4 h-4" />
                  </Button>
                )}
              </div>
            )}
          </div>

          <div className="flex flex-1 min-w-0 items-center justify-end gap-1.5">
            {/* 弹性中段：空间不足时由 AppSwitcher 自行收纳溢出应用；
                justify-end + overflow-hidden 只裁剪 resize 瞬间的过渡帧 */}
            <div className="flex flex-1 min-w-0 items-center justify-end overflow-hidden py-4">
              {currentView === "providers" && (
                <AppSwitcher
                  activeApp={activeClient}
                  onSwitch={selectClient}
                  disabled={modelClientBusy}
                  visibleApps={visibleApps}
                />
              )}
            </div>
            {/* 固定右端：主操作（添加供应商等）shrink-0，任何配置下不被挤出 */}
            <div className="flex shrink-0 items-center py-4">
              <div
                className="flex shrink-0 items-center gap-1.5"
                style={{ WebkitAppRegion: "no-drag" } as any}
              >
                {currentView === "providers" && !isModelClient && (
                  <>
                    <div className="flex items-center gap-1 p-1 bg-muted rounded-xl">
                      <div
                        key={
                          activeApp === "openclaw"
                            ? "openclaw"
                            : activeApp === "hermes"
                              ? "hermes"
                              : activeApp === "grokbuild"
                                ? "grokbuild"
                                : "default"
                        }
                        className="flex items-center gap-1"
                      >
                        {activeApp === "hermes" ? (
                          <>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => setCurrentView("hermesMemory")}
                              className="text-muted-foreground hover:text-foreground hover:bg-black/5 dark:hover:bg-white/5 w-8 px-2"
                              title={t("hermes.memory.title")}
                            >
                              <Brain className="w-4 h-4" />
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => void openHermesWebUI()}
                              className="text-muted-foreground hover:text-foreground hover:bg-black/5 dark:hover:bg-white/5 w-8 px-2"
                              title={t("hermes.webui.open")}
                            >
                              <LayoutDashboard className="w-4 h-4" />
                            </Button>
                          </>
                        ) : activeApp === "openclaw" ? (
                          <>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => setCurrentView("workspace")}
                              className="text-muted-foreground hover:text-foreground hover:bg-black/5 dark:hover:bg-white/5 w-8 px-2"
                              title={t("workspace.manage")}
                            >
                              <FolderOpen className="w-4 h-4" />
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => setCurrentView("openclawEnv")}
                              className="text-muted-foreground hover:text-foreground hover:bg-black/5 dark:hover:bg-white/5 w-8 px-2"
                              title={t("openclaw.env.title")}
                            >
                              <KeyRound className="w-4 h-4" />
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => setCurrentView("openclawTools")}
                              className="text-muted-foreground hover:text-foreground hover:bg-black/5 dark:hover:bg-white/5 w-8 px-2"
                              title={t("openclaw.tools.title")}
                            >
                              <Shield className="w-4 h-4" />
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => setCurrentView("openclawAgents")}
                              className="text-muted-foreground hover:text-foreground hover:bg-black/5 dark:hover:bg-white/5 w-8 px-2"
                              title={t("openclaw.agents.title")}
                            >
                              <Cpu className="w-4 h-4" />
                            </Button>
                          </>
                        ) : (
                          <></>
                        )}
                      </div>
                    </div>
                  </>
                )}
              </div>
            </div>
          </div>
        </div>
      </header>

      <main
        ref={mainScrollRef}
        className="flex-1 min-h-0 flex flex-col overflow-y-auto"
      >
        {isOpenClawView && openclawHealthWarnings.length > 0 && (
          <OpenClawHealthBanner warnings={openclawHealthWarnings} />
        )}
        <DeckPanel
          client={activeClient}
          onBusyChange={setModelClientBusy}
          visible={currentView === "providers"}
        />
        {renderContent()}
      </main>

      {/* Mounted on first open and kept mounted afterwards, so the panel still
          plays its exit animation when closed. */}
      {hasOpenedAddDialog && (
        <Suspense fallback={null}>
          <AddProviderDialog
            open={isAddOpen}
            onOpenChange={setIsAddOpen}
            appId={activeApp}
            onSubmit={addProvider}
          />
        </Suspense>
      )}

      {hasOpenedEditDialog && (
        <Suspense fallback={null}>
          <EditProviderDialog
            open={Boolean(editingProvider)}
            provider={effectiveEditingProvider}
            onOpenChange={(open) => {
              if (!open) {
                setEditingProvider(null);
              }
            }}
            onSubmit={handleEditProvider}
            appId={activeApp}
            isProxyTakeover={isCurrentAppTakeoverActive}
          />
        </Suspense>
      )}

      {effectiveUsageProvider && (
        <Suspense fallback={null}>
          <UsageScriptModal
            key={effectiveUsageProvider.id}
            provider={effectiveUsageProvider}
            appId={activeApp}
            isOpen={Boolean(usageProvider)}
            onClose={() => setUsageProvider(null)}
            onSave={(script) => {
              if (usageProvider) {
                void saveUsageScript(usageProvider, script);
              }
            }}
          />
        </Suspense>
      )}

      <ConfirmDialog
        isOpen={Boolean(confirmAction)}
        title={
          confirmAction?.action === "remove"
            ? t("confirm.removeProvider")
            : t("confirm.deleteProvider")
        }
        message={confirmActionMessage}
        onConfirm={() => void handleConfirmAction()}
        onCancel={() => setConfirmAction(null)}
      />

      <ConfirmDialog
        isOpen={launchDashboardOpen}
        title={t("hermes.webui.launchConfirmTitle")}
        message={t("hermes.webui.launchConfirmMessage")}
        confirmText={t("hermes.webui.launchConfirmAction")}
        variant="info"
        onConfirm={() => {
          setLaunchDashboardOpen(false);
          void (async () => {
            try {
              await hermesApi.launchDashboard();
              toast.success(t("hermes.webui.launching"));
            } catch (error) {
              toast.error(t("hermes.webui.launchFailed"), {
                description: extractErrorMessage(error) || undefined,
              });
            }
          })();
        }}
        onCancel={() => setLaunchDashboardOpen(false)}
      />

      <Suspense fallback={null}>
        <DeepLinkImportDialog />
      </Suspense>
      <FirstRunNoticeDialog />
    </div>
  );
}

export default App;
