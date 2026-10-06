import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { deckFixture } from "../fixtures/deck";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));
vi.mock("sonner", () => ({
  toast: Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  }),
}));

function wrap(ui: React.ReactElement) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

describe("deck pages render", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (c: string) =>
      c === "get_deck" ? deckFixture() : null,
    );
  });

  it("providers page", async () => {
    const { default: P } = await import("@/components/deck/ProvidersPage");
    wrap(<P />);
    expect(
      await screen.findByRole("heading", { name: "nav.resource.providers" }),
    ).toBeTruthy();
    expect(await screen.findByDisplayValue("Private provider")).toBeTruthy();
  });

  it("endpoints page", async () => {
    const { default: P } = await import("@/components/deck/EndpointsPage");
    wrap(<P />);
    expect(
      await screen.findByRole("heading", { name: "nav.resource.endpoints" }),
    ).toBeTruthy();
    expect(await screen.findByDisplayValue("Shared endpoint")).toBeTruthy();
  });

  it("matrix page", async () => {
    const { default: P } = await import("@/components/deck/MatrixPage");
    wrap(<P />);
    expect(
      await screen.findByRole("heading", { name: "nav.resource.matrix" }),
    ).toBeTruthy();
    expect(await screen.findByRole("table")).toBeTruthy();
  });

  it("harness page shows the selection", async () => {
    const { default: P } = await import("@/components/deck/HarnessPage");
    wrap(<P client="codex" onNavigate={() => {}} />);
    expect(
      await screen.findByRole("heading", { name: "Codex CLI" }),
    ).toBeTruthy();
  });

  it("surfaces the endpoint token the gateway generated", async () => {
    const { default: P } = await import("@/components/deck/EndpointsPage");
    wrap(<P />);
    // Hidden by default, revealed on demand -- but present, which the old
    // panel never showed even though Cursor needs it.
    expect(await screen.findByDisplayValue("gateway-only-token")).toBeTruthy();
  });

  it("marks a model missing its limits as incomplete", async () => {
    const { default: P } = await import("@/components/deck/ProvidersPage");
    wrap(<P />);
    // The heading renders during loading too, so wait for real data.
    await screen.findByDisplayValue("Private provider");
    fireEvent.click(screen.getByRole("button", { name: /deck\.addModel/ }));
    // A fresh model has no id or limits, so it must not look ready to route.
    expect(await screen.findAllByText("deck.incomplete")).toHaveLength(1);
  });

  it("shows unsaved state only after an edit", async () => {
    const { default: P } = await import("@/components/deck/ProvidersPage");
    wrap(<P />);
    const name = await screen.findByDisplayValue("Private provider");
    expect(screen.queryByText("deck.unsavedChanges")).toBeNull();
    fireEvent.change(name, { target: { value: "Renamed" } });
    await waitFor(() =>
      expect(screen.getByText("deck.unsavedChanges")).toBeTruthy(),
    );
  });
});
