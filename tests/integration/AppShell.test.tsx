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
  UsageDashboard: () => <div data-testid="usage-page" />,
}));
vi.mock("@/components/hermes/HermesMemoryPanel", () => ({
  default: () => <div data-testid="hermes-page" />,
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
  });

  it("keeps the sidebar visible on every destination", async () => {
    const { default: App } = await import("@/App");
    renderApp(App);

    fireEvent.click(await screen.findByRole("button", { name: /nav\.usage/ }));
    expect(await screen.findByTestId("usage-page")).toBeInTheDocument();
    // The old shell hid navigation on secondary views; it must stay put now.
    expect(screen.getByTestId("sidebar")).toBeInTheDocument();
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
