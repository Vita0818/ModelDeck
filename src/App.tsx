import { ConfirmDialog } from "@/components/ConfirmDialog";
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
import type { VisibleApps } from "@/types";
import { useProvidersQuery, useSettingsQuery } from "@/lib/query";
import { providersApi, type AppId } from "@/lib/api";
import { useOpenHermesWebUI } from "@/hooks/useHermes";
import { hermesApi } from "@/lib/api/hermes";
import { useProxyStatus } from "@/hooks/useProxyStatus";
import { useUsageCacheBridge } from "@/hooks/useUsageCacheBridge";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { HEADER_HEIGHT, useWindowChrome } from "@/hooks/app/useWindowChrome";
import { useEnvConflicts } from "@/hooks/app/useEnvConflicts";
import { useStartupMigrations } from "@/hooks/app/useStartupMigrations";
import { useAppEventBridges } from "@/hooks/app/useAppEventBridges";
import { resolveViewTitle, type View } from "@/components/app/viewTitle";
import { extractErrorMessage } from "@/utils/errorUtils";
import { cn } from "@/lib/utils";
import { DRAG_REGION_ATTR, DRAG_REGION_STYLE } from "@/lib/platform";
import { AppSwitcher } from "@/components/AppSwitcher";
import { UpdateBadge } from "@/components/UpdateBadge";
import { EnvWarningBanner } from "@/components/env/EnvWarningBanner";
import { RoutingActivationBrand } from "@/components/proxy/RoutingActivationBrand";
import { FirstRunNoticeDialog } from "@/components/FirstRunNoticeDialog";
import { Button } from "@/components/ui/button";
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

  const {
    conflicts: envConflicts,
    showBanner: showEnvBanner,
    dismiss: dismissEnvBanner,
    refresh: refreshEnvConflicts,
  } = useEnvConflicts(activeApp, isModelClient, currentView === "providers");

  const mainScrollRef = useRef<HTMLElement>(null);

  useUsageCacheBridge();

  useLayoutEffect(() => {
    if (currentView !== "providers") return;

    const container = mainScrollRef.current;
    if (container) {
      container.scrollTop = 0;
      container.scrollLeft = 0;
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
  const { refetch } = useProvidersQuery(activeApp, {
    enabled: currentView !== "providers",
    isProxyRunning: currentAppUsesProxy && isProxyRunning,
  });
  const [launchDashboardOpen, setLaunchDashboardOpen] = useState(false);
  const openHermesWebUI = useOpenHermesWebUI(() =>
    setLaunchDashboardOpen(true),
  );

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
        default:
          return null;
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
                  disabled={modelClientBusy}
                  aria-label={t("common.back")}
                  onClick={() => setCurrentView("providers")}
                  className={cn(
                    "mr-2 rounded-lg",
                    modelClientBusy && "disabled:opacity-100",
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
        <DeckPanel
          client={activeClient}
          onBusyChange={setModelClientBusy}
          visible={currentView === "providers"}
        />
        {renderContent()}
      </main>

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
