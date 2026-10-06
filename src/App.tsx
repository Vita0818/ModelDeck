import { Suspense, lazy, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useQueryClient } from "@tanstack/react-query";
import { Loader2, Maximize2, Minimize2, Minus, X } from "lucide-react";
import type { VisibleApps } from "@/types";
import { useSettingsQuery } from "@/lib/query";
import { providersApi, type AppId } from "@/lib/api";
import { useOpenHermesWebUI } from "@/hooks/useHermes";
import { hermesApi } from "@/lib/api/hermes";
import { useUsageCacheBridge } from "@/hooks/useUsageCacheBridge";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { useWindowChrome } from "@/hooks/app/useWindowChrome";
import { useEnvConflicts } from "@/hooks/app/useEnvConflicts";
import { useStartupMigrations } from "@/hooks/app/useStartupMigrations";
import { useAppEventBridges } from "@/hooks/app/useAppEventBridges";
import { useDeck } from "@/lib/query/deck";
import { extractErrorMessage } from "@/utils/errorUtils";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EnvWarningBanner } from "@/components/env/EnvWarningBanner";
import { FirstRunNoticeDialog } from "@/components/FirstRunNoticeDialog";
import { UpdateBadge } from "@/components/UpdateBadge";
import { Button } from "@/components/ui/button";
import { Sidebar } from "@/components/app/Sidebar";
import { loadRoute, persistRoute, type Route } from "@/components/app/routes";
import HarnessPage from "@/components/deck/HarnessPage";
import {
  CLIENT_IDS,
  isClientId,
  isModelClientId,
  resolveClientVisibility,
  type ClientId,
} from "@/config/modeldeckClients";

const ProvidersPage = lazy(() => import("@/components/deck/ProvidersPage"));
const EndpointsPage = lazy(() => import("@/components/deck/EndpointsPage"));
const MatrixPage = lazy(() => import("@/components/deck/MatrixPage"));
const SettingsPage = lazy(() =>
  import("@/components/settings/SettingsPage").then((m) => ({
    default: m.SettingsPage,
  })),
);
const UsageDashboard = lazy(() =>
  import("@/components/usage/UsageDashboard").then((m) => ({
    default: m.UsageDashboard,
  })),
);
const HermesMemoryPanel = lazy(
  () => import("@/components/hermes/HermesMemoryPanel"),
);
const DeepLinkImportDialog = lazy(() =>
  import("@/components/DeepLinkImportDialog").then((m) => ({
    default: m.DeepLinkImportDialog,
  })),
);

const SIDEBAR_WIDTH = 216;

function PageFallback() {
  return (
    <div
      className="flex flex-1 items-center justify-center"
      role="status"
      aria-busy="true"
    >
      <Loader2 className="size-5 animate-spin text-muted-foreground/40" />
    </div>
  );
}

/** Deck resources are per-client only for Hermes' memory tool. */
const toAppId = (client: ClientId): AppId =>
  isModelClientId(client) ? "claude" : client;

function App() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const [route, setRoute] = useState<Route>(loadRoute);
  const [launchDashboardOpen, setLaunchDashboardOpen] = useState(false);

  useEffect(() => persistRoute(route), [route]);

  const { data: settingsData } = useSettingsQuery();
  const { data: deck } = useDeck();
  const {
    useAppWindowControls,
    dragBarHeight,
    isWindowMaximized,
    minimize,
    toggleMaximize,
    close,
  } = useWindowChrome();

  useStartupMigrations();
  useAppEventBridges();
  useUsageCacheBridge();
  useOpenHermesWebUI(() => setLaunchDashboardOpen(true));

  const visibleApps = useMemo<VisibleApps>(
    () => resolveClientVisibility(settingsData?.visibleApps),
    [settingsData?.visibleApps],
  );

  // The app scopes a few client-specific features; everything else is global.
  const activeClient = route.kind === "client" ? route.client : undefined;
  const activeApp = activeClient ? toAppId(activeClient) : "claude";

  const {
    conflicts: envConflicts,
    showBanner: showEnvBanner,
    dismiss: dismissEnvBanner,
    refresh: refreshEnvConflicts,
  } = useEnvConflicts(activeApp, !activeClient, false);

  // Fall back to a visible client if the current one gets hidden in settings.
  useEffect(() => {
    if (activeClient && !visibleApps[activeClient]) {
      const next = CLIENT_IDS.find((id) => visibleApps[id]);
      if (next) setRoute({ kind: "client", client: next as ClientId });
    }
  }, [visibleApps, activeClient]);

  useTauriEvent<{ client: string }>("modeldeck-client-selected", (payload) => {
    if (isClientId(payload.client)) {
      setRoute({ kind: "client", client: payload.client });
    }
  });

  const appliedClients = useMemo(
    () =>
      new Set(
        Object.keys(deck?.store.activeBindings ?? {}).filter((id) =>
          isClientId(id),
        ) as ClientId[],
      ),
    [deck],
  );

  const handleImportSuccess = async () => {
    await queryClient.invalidateQueries({ queryKey: ["deck"] });
    try {
      await providersApi.updateTrayMenu();
    } catch (error) {
      console.error("[App] Failed to refresh tray menu", error);
    }
  };

  const content = () => {
    switch (route.kind) {
      case "resources":
        return route.tab === "providers" ? (
          <ProvidersPage />
        ) : route.tab === "endpoints" ? (
          <EndpointsPage />
        ) : (
          <MatrixPage />
        );
      case "client":
        return <HarnessPage client={route.client} onNavigate={setRoute} />;
      case "usage":
        return <UsageDashboard />;
      case "settings":
        return (
          <SettingsPage
            open
            onOpenChange={() => undefined}
            onImportSuccess={handleImportSuccess}
            defaultTab={route.tab ?? "general"}
          />
        );
      case "hermesMemory":
        return <HermesMemoryPanel />;
    }
  };

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-canvas text-foreground selection:bg-primary/25">
      {/* Title bar. On macOS this is the traffic-light inset; on Linux the
          app draws its own controls when the user opts in. */}
      {(dragBarHeight > 0 || useAppWindowControls) && (
        <div
          data-tauri-drag-region
          style={{ height: dragBarHeight }}
          className="z-50 flex shrink-0 items-center justify-end bg-sidebar/80 px-2 backdrop-blur-xl"
        >
          {useAppWindowControls && (
            <div className="no-drag flex items-center gap-0.5">
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => void minimize()}
                title={t("header.windowMinimize")}
              >
                <Minus />
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => void toggleMaximize()}
                title={t(
                  isWindowMaximized
                    ? "header.windowRestore"
                    : "header.windowMaximize",
                )}
              >
                {isWindowMaximized ? <Minimize2 /> : <Maximize2 />}
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => void close()}
                title={t("header.windowClose")}
                className="hover:bg-destructive hover:text-destructive-foreground"
              >
                <X />
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

      <div className="flex min-h-0 flex-1">
        <Sidebar
          route={route}
          onNavigate={setRoute}
          visibleApps={visibleApps}
          appliedClients={appliedClients}
          showHermesMemory={Boolean(visibleApps.hermes)}
          width={SIDEBAR_WIDTH}
        />
        <main className="flex min-w-0 flex-1 flex-col bg-background">
          <Suspense fallback={<PageFallback />}>{content()}</Suspense>
        </main>
      </div>

      <UpdateBadge
        className="fixed bottom-4 right-4 z-40"
        onClick={() => setRoute({ kind: "settings", tab: "about" })}
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
