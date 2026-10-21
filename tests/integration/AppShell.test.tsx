import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { deckFixture } from "../fixtures/deck";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: async () => false,
    onResized: async () => () => undefined,
    setDecorations: async () => undefined,
  }),
}));
vi.mock("sonner", () => ({
  toast: Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
  }),
}));

// Heavy leaves are irrelevant to navigation; stub them so the shell is the
// only thing under test.
vi.mock("@/components/settings/SettingsPage", () => ({
  SettingsPage: () => <div data-testid="settings-page" />,
}));
vi.mock("@/components/usage/UsageDashboard", () => ({
  UsageDashboard: (props: {
    refreshIntervalMs?: number;
    sessionAutoSyncEnabled?: boolean;
    onRefreshIntervalChange: (next: number) => Promise<boolean>;
    onSessionAutoSyncEnabledChange: (next: boolean) => Promise<boolean>;
  }) => (
    <div data-testid="usage-page">
      <span data-testid="saved-usage-refresh">{props.refreshIntervalMs}</span>
      <span data-testid="saved-session-sync">
        {String(props.sessionAutoSyncEnabled)}
      </span>
      <button onClick={() => void props.onRefreshIntervalChange(5000)}>
        save-usage-refresh
      </button>
      <button onClick={() => void props.onSessionAutoSyncEnabledChange(true)}>
        enable-session-sync
      </button>
    </div>
  ),
}));
vi.mock("@/components/DeepLinkImportDialog", () => ({
  DeepLinkImportDialog: () => null,
}));
vi.mock("@/components/FirstRunNoticeDialog", () => ({
  FirstRunNoticeDialog: () => null,
}));
vi.mock("@/components/UpdateBadge", () => ({ UpdateBadge: () => null }));
vi.mock("@/components/env/EnvWarningBanner", () => ({
  EnvWarningBanner: () => null,
}));

function renderApp(App: React.ComponentType) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>,
  );
}

describe("App shell", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_deck") return deckFixture();
      if (cmd === "get_settings") return { language: "en" };
      if (cmd === "check_all_env_conflicts") return {};
      return null;
    });
  });

  afterEach(() => localStorage.clear());

  it("opens on a client and shows its harness page", async () => {
    const { default: App } = await import("@/App");
    renderApp(App);

    expect(await screen.findByTestId("sidebar")).toBeInTheDocument();
    expect(
      await screen.findByRole("heading", { name: "Codex CLI" }),
    ).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("get_migration_result");
    expect(invokeMock).not.toHaveBeenCalledWith("get_skills_migration_result");
  });

  it("recovers a removed memory destination to the last client while keeping usage", async () => {
    localStorage.setItem("modeldeck-route", "hermesMemory");
    localStorage.setItem("modeldeck-last-client", "hermes");
    const { default: App } = await import("@/App");
    renderApp(App);

    expect(
      await screen.findByRole("heading", { name: "Hermes" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /memory|hermes\.memory/i }),
    ).not.toBeInTheDocument();
    expect(localStorage.getItem("modeldeck-route")).toBe("client:hermes");

    fireEvent.click(screen.getByRole("button", { name: /nav\.usage/ }));
    expect(await screen.findByTestId("usage-page")).toBeInTheDocument();
  });

  it("keeps the sidebar visible on every destination", async () => {
    const { default: App } = await import("@/App");
    renderApp(App);

    fireEvent.click(await screen.findByRole("button", { name: /nav\.usage/ }));
    expect(await screen.findByTestId("usage-page")).toBeInTheDocument();
    // The old shell hid navigation on secondary views; it must stay put now.
    expect(screen.getByTestId("sidebar")).toBeInTheDocument();
  });

  it("reads and persists usage settings without overwriting other preferences", async () => {
    let settings = {
      language: "en",
      showInTray: false,
      minimizeToTrayOnClose: true,
      usageDashboardRefreshIntervalMs: 10000,
      sessionAutoSyncEnabled: false,
    };
    invokeMock.mockImplementation(
      async (command: string, args?: { settings: typeof settings }) => {
        if (command === "get_settings") return settings;
        if (command === "save_settings") {
          settings = args!.settings;
          return true;
        }
        if (command === "get_deck") return deckFixture();
        if (command === "check_all_env_conflicts") return {};
        return null;
      },
    );
    localStorage.setItem("modeldeck-route", "usage");
    const { default: App } = await import("@/App");
    renderApp(App);
    await waitFor(() =>
      expect(screen.getByTestId("saved-usage-refresh")).toHaveTextContent(
        "10000",
      ),
    );
    expect(screen.getByTestId("saved-session-sync")).toHaveTextContent("false");

    fireEvent.click(screen.getByRole("button", { name: "save-usage-refresh" }));
    await waitFor(() =>
      expect(screen.getByTestId("saved-usage-refresh")).toHaveTextContent(
        "5000",
      ),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "enable-session-sync" }),
    );
    await waitFor(() =>
      expect(screen.getByTestId("saved-session-sync")).toHaveTextContent(
        "true",
      ),
    );
    expect(settings).toEqual({
      language: "en",
      showInTray: false,
      minimizeToTrayOnClose: true,
      usageDashboardRefreshIntervalMs: 5000,
      sessionAutoSyncEnabled: true,
    });
  });

  it("navigates to the resource layer as a peer of the clients", async () => {
    const { default: App } = await import("@/App");
    renderApp(App);

    fireEvent.click(
      await screen.findByRole("button", { name: /nav\.resource\.providers/ }),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: "nav.resource.providers" }),
      ).toBeInTheDocument(),
    );
  });

  it("does not reset the page when switching clients", async () => {
    const { default: App } = await import("@/App");
    renderApp(App);

    fireEvent.click(
      await screen.findByRole("button", { name: /nav\.resource\.providers/ }),
    );
    await screen.findByRole("heading", { name: "nav.resource.providers" });

    // Selecting a client is an explicit navigation, so it does move; the
    // regression this guards is the reverse -- resources surviving a client
    // change is what the old shell could not do.
    fireEvent.click(screen.getByRole("button", { name: /Claude Code/ }));
    expect(
      await screen.findByRole("heading", { name: "Claude Code" }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("sidebar")).toBeInTheDocument();
  });

  it("restores the last destination across restarts", async () => {
    localStorage.setItem("modeldeck-route", "resources:endpoints");
    const { default: App } = await import("@/App");
    renderApp(App);

    expect(
      await screen.findByRole("heading", { name: "nav.resource.endpoints" }),
    ).toBeInTheDocument();
  });

  it("saves the selection before applying its new revision and reflects the active binding", async () => {
    const fixture = deckFixture();
    invokeMock.mockImplementation(
      async (cmd: string, args?: Record<string, unknown>) => {
        if (cmd === "get_deck") return fixture;
        if (cmd === "get_settings") return { language: "en" };
        if (cmd === "check_all_env_conflicts") return {};
        if (cmd === "save_deck") {
          return {
            ...fixture,
            store: { ...(args?.store as typeof fixture.store), revision: 1 },
          };
        }
        if (cmd === "apply_deck_binding") {
          expect(args).toEqual({ client: "codex", revision: 1 });
          return {
            view: {
              ...fixture,
              store: {
                ...fixture.store,
                revision: 2,
                activeBindings: { codex: fixture.store.bindings.codex },
              },
            },
            restartRequired: false,
          };
        }
        return null;
      },
    );
    const { default: App } = await import("@/App");
    renderApp(App);
    await screen.findByText("Shared endpoint");
    fireEvent.click(screen.getByRole("button", { name: "deck.saveAndApply" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("apply_deck_binding", {
        client: "codex",
        revision: 1,
      }),
    );
    expect(await screen.findByText("deck.activeBinding")).toBeInTheDocument();
    const mutations = invokeMock.mock.calls
      .filter(([cmd]) => ["save_deck", "apply_deck_binding"].includes(cmd))
      .map(([cmd]) => cmd);
    expect(mutations).toEqual(["save_deck", "apply_deck_binding"]);
  });

  it("fetches the deck once and shares it across pages", async () => {
    const { default: App } = await import("@/App");
    renderApp(App);

    await screen.findByRole("heading", { name: "Codex CLI" });
    fireEvent.click(
      screen.getByRole("button", { name: /nav\.resource\.matrix/ }),
    );
    await screen.findByRole("heading", { name: "nav.resource.matrix" });

    const deckCalls = invokeMock.mock.calls.filter(
      ([cmd]) => cmd === "get_deck",
    );
    expect(deckCalls).toHaveLength(1);
  });
});
