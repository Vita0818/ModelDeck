import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import {
  afterAll,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
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
  const originalScrollIntoView = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "scrollIntoView",
  );
  beforeAll(() => {
    // JSDOM omits layout scrolling, which the native select uses for focus.
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      writable: true,
      value: vi.fn(),
    });
  });
  afterAll(() => {
    if (originalScrollIntoView) {
      Object.defineProperty(
        HTMLElement.prototype,
        "scrollIntoView",
        originalScrollIntoView,
      );
    } else {
      Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
    }
  });
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

  it("lets a migrated environment credential switch to a stored key and save", async () => {
    const fixture = deckFixture();
    fixture.store.providers.upstream.apiKey = "";
    fixture.store.providers.upstream.keyEnv = "OR_TEST_KEY";
    fixture.store.providers.upstream.keySource = "env";
    invokeMock.mockImplementation(
      async (command: string, args?: { store: typeof fixture.store }) => {
        if (command === "get_deck") return fixture;
        if (command === "save_deck")
          return { ...fixture, store: { ...args!.store, revision: 1 } };
        return null;
      },
    );
    const { default: P } = await import("@/components/deck/ProvidersPage");
    wrap(<P />);
    await screen.findByDisplayValue("OR_TEST_KEY");
    fireEvent.click(
      screen.getByRole("combobox", { name: "deck.credentialSource" }),
    );
    fireEvent.click(
      await screen.findByRole("option", { name: "deck.storedKey" }),
    );
    const key = await screen.findByPlaceholderText("sk-...");
    fireEvent.change(key, { target: { value: "test-only-key" } });
    fireEvent.click(screen.getByRole("button", { name: "deck.saveResources" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "save_deck",
        expect.objectContaining({
          store: expect.objectContaining({
            providers: expect.objectContaining({
              upstream: expect.objectContaining({
                keySource: "stored",
                keyEnv: "",
                apiKey: "test-only-key",
              }),
            }),
          }),
        }),
      ),
    );
  });

  it("keeps environment input visible when selecting a new environment source", async () => {
    const { default: P } = await import("@/components/deck/ProvidersPage");
    wrap(<P />);
    await screen.findByDisplayValue("Private provider");
    fireEvent.click(
      screen.getByRole("combobox", { name: "deck.credentialSource" }),
    );
    fireEvent.click(
      await screen.findByRole("option", { name: "deck.environmentKey" }),
    );
    expect(
      await screen.findByPlaceholderText("OPENAI_API_KEY"),
    ).toBeInTheDocument();
    expect(screen.queryByPlaceholderText("sk-...")).not.toBeInTheDocument();
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

  it("shows the selected model context, output limit and actual route tier", async () => {
    const { default: P } = await import("@/components/deck/HarnessPage");
    wrap(<P client="codex" onNavigate={() => {}} />);
    expect(await screen.findByTestId("model-settings")).toBeInTheDocument();
    expect(screen.getByText("200,000 tokens")).toBeInTheDocument();
    expect(screen.getByText("8,192 tokens")).toBeInTheDocument();
    expect(screen.getByText("high")).toBeInTheDocument();
  });

  it("offers only supported reasoning tiers and saves the selected tier", async () => {
    const fixture = deckFixture();
    invokeMock.mockImplementation(
      async (command: string, args?: { store: typeof fixture.store }) => {
        if (command === "get_deck") return fixture;
        if (command === "save_deck")
          return { ...fixture, store: { ...args!.store, revision: 1 } };
        return null;
      },
    );
    const { default: P } = await import("@/components/deck/EndpointsPage");
    wrap(<P />);
    const tier = await screen.findByRole("combobox", {
      name: "deck.effort: coding",
    });
    fireEvent.click(tier);
    expect(
      await screen.findByRole("option", { name: "max" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "medium" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("option", { name: "max" }));
    fireEvent.click(screen.getByRole("button", { name: "deck.saveResources" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "save_deck",
        expect.objectContaining({
          store: expect.objectContaining({
            endpoints: expect.objectContaining({
              entry: expect.objectContaining({
                routes: expect.objectContaining({
                  coding: expect.objectContaining({ effort: "max" }),
                }),
              }),
            }),
          }),
        }),
      ),
    );
  });

  it.each([
    { levels: ["ultra", "max", "high"], expected: "max" },
    { levels: ["low", "xhigh", "high"], expected: "xhigh" },
  ])(
    "defaults a new route to $expected for $levels",
    async ({ levels, expected }) => {
      const fixture = deckFixture();
      fixture.store.models.resource.reasoning = levels;
      invokeMock.mockImplementation(
        async (command: string, args?: { store: typeof fixture.store }) => {
          if (command === "get_deck") return fixture;
          if (command === "save_deck")
            return { ...fixture, store: { ...args!.store, revision: 1 } };
          return null;
        },
      );
      const { default: P } = await import("@/components/deck/EndpointsPage");
      wrap(<P />);
      const alias = await screen.findByRole("textbox", {
        name: "deck.newAlias",
      });
      fireEvent.change(alias, { target: { value: "new-route" } });
      fireEvent.keyDown(alias, { key: "Enter" });
      expect(
        await screen.findByRole("combobox", { name: "deck.effort: new-route" }),
      ).toHaveTextContent(expected);
      fireEvent.click(
        screen.getByRole("button", { name: "deck.saveResources" }),
      );
      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith(
          "save_deck",
          expect.objectContaining({
            store: expect.objectContaining({
              endpoints: expect.objectContaining({
                entry: expect.objectContaining({
                  routes: expect.objectContaining({
                    "new-route": { modelId: "resource", effort: expected },
                  }),
                }),
              }),
            }),
          }),
        ),
      );
    },
  );

  it("uses the new model's highest tier when the previous tier is incompatible", async () => {
    const fixture = deckFixture();
    fixture.store.models.other = {
      ...fixture.store.models.resource,
      id: "other",
      name: "Other model",
      upstreamId: "other-model",
      reasoning: ["low"],
    };
    invokeMock.mockImplementation(
      async (command: string, args?: { store: typeof fixture.store }) => {
        if (command === "get_deck") return fixture;
        if (command === "save_deck")
          return { ...fixture, store: { ...args!.store, revision: 1 } };
        return null;
      },
    );
    const { default: P } = await import("@/components/deck/EndpointsPage");
    wrap(<P />);
    fireEvent.click(
      await screen.findByRole("combobox", { name: "deck.selectModel: coding" }),
    );
    fireEvent.click(
      await screen.findByRole("option", { name: "Other model · other-model" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "deck.saveResources" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "save_deck",
        expect.objectContaining({
          store: expect.objectContaining({
            endpoints: expect.objectContaining({
              entry: expect.objectContaining({
                routes: expect.objectContaining({
                  coding: expect.objectContaining({
                    modelId: "other",
                    effort: "low",
                  }),
                }),
              }),
            }),
          }),
        }),
      ),
    );
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
