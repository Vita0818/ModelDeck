import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DeckPanel from "@/components/deck/DeckPanel";
import { deckFixture } from "../fixtures/deck";
import { CLIENT_IDS } from "@/config/modeldeckClients";
import type { DeckStore, DeckView } from "@/lib/api/deck";

const { invoke, success, error } = vi.hoisted(() => ({
  invoke: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("sonner", () => ({ toast: { success, error } }));
let view: DeckView;
const onBusy = vi.fn();
const renderPanel = () =>
  render(<DeckPanel client="codex" onBusyChange={onBusy} />);

describe("ModelDeck shared resources and harness bindings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    view = deckFixture();
    invoke.mockImplementation(
      async (
        command: string,
        args?: { store?: DeckStore; client?: string },
      ) => {
        if (command === "get_deck") return structuredClone(view);
        if (command === "save_deck") {
          view = {
            ...view,
            store: {
              ...structuredClone(args!.store!),
              revision: view.store.revision + 1,
            },
          };
          return structuredClone(view);
        }
        if (command === "apply_deck_binding")
          throw new Error("Native runtime is unavailable");
        throw new Error(`Unexpected command: ${command}`);
      },
    );
  });

  it("uses one catalog for all ten harnesses and hides upstream details from their selection page", async () => {
    const { rerender } = renderPanel();
    await screen.findByTestId("deck-panel");
    for (const client of CLIENT_IDS) {
      rerender(<DeckPanel client={client} onBusyChange={onBusy} />);
      expect(screen.getByLabelText("deck.selectedEndpoint")).toHaveValue(
        "entry",
      );
      expect(
        screen.queryByDisplayValue("https://private-upstream.test/v1"),
      ).not.toBeInTheDocument();
      expect(
        screen.queryByDisplayValue("upstream-only-secret"),
      ).not.toBeInTheDocument();
      expect(screen.queryByText("Private provider")).not.toBeInTheDocument();
      expect(screen.queryByText("private-model-id")).not.toBeInTheDocument();
    }
    expect(invoke.mock.calls.filter(([c]) => c === "get_deck")).toHaveLength(1);
  });

  it("saves only references in the harness binding without activating it", async () => {
    renderPanel();
    await screen.findByTestId("deck-panel");
    fireEvent.click(screen.getByRole("button", { name: "deck.saveSelection" }));
    await waitFor(() =>
      expect(success).toHaveBeenCalledWith("deck.selectionSaved"),
    );
    const call = invoke.mock.calls.find(([c]) => c === "save_deck")!;
    expect(call[1].store.bindings.codex).toEqual({
      endpointId: "entry",
      model: "coding",
      utilityModel: "",
      executable: "",
    });
    expect(call[1].store.models).toEqual(deckFixture().store.models);
    expect(call[1].store.activeBindings).toEqual({});
    expect(
      invoke.mock.calls.some(
        ([c]) => c === "save_model_client" || c === "apply_model_client",
      ),
    ).toBe(false);
  });

  it("keeps the active selection unchanged when native apply fails", async () => {
    renderPanel();
    await screen.findByTestId("deck-panel");
    fireEvent.click(screen.getByRole("button", { name: "deck.saveAndApply" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Native runtime is unavailable",
    );
    expect(screen.getByText("deck.nativeSelection")).toBeInTheDocument();
    expect(success).not.toHaveBeenCalledWith("deck.applied");
  });

  it("keeps resource drafts when switching harnesses and saves shared model metadata once", async () => {
    const { rerender } = renderPanel();
    await screen.findByTestId("deck-panel");
    fireEvent.click(screen.getByRole("tab", { name: "deck.resourceLayer" }));
    fireEvent.change(screen.getByLabelText("deck.context"), {
      target: { value: "1000000" },
    });
    rerender(<DeckPanel client="muse" onBusyChange={onBusy} />);
    fireEvent.click(screen.getByRole("tab", { name: "deck.resourceLayer" }));
    expect(screen.getByLabelText("deck.context")).toHaveValue(1000000);
    fireEvent.click(screen.getByRole("button", { name: "deck.saveResources" }));
    await waitFor(() =>
      expect(success).toHaveBeenCalledWith("deck.resourcesSaved"),
    );
    expect(view.store.models.resource.context).toBe(1000000);
    expect(Object.keys(view.store.models)).toEqual(["resource"]);
    for (const binding of Object.values(view.store.bindings))
      expect(binding).not.toHaveProperty("models");
  });

  it("changes the shared route tier without copying model definitions into harnesses", async () => {
    renderPanel();
    await screen.findByTestId("deck-panel");
    fireEvent.click(screen.getByRole("tab", { name: "deck.resourceLayer" }));
    fireEvent.click(screen.getByRole("tab", { name: "deck.endpoints" }));
    fireEvent.change(screen.getAllByLabelText("deck.tier")[0], {
      target: { value: "max" },
    });
    fireEvent.click(screen.getByRole("button", { name: "deck.saveResources" }));
    await waitFor(() =>
      expect(view.store.endpoints.entry.routes.coding.effort).toBe("max"),
    );
    expect(view.store.bindings).toEqual(deckFixture().store.bindings);
    expect(view.store.models).toEqual(deckFixture().store.models);
  });

  it("retains the environment credential field while clearing and retyping its name", async () => {
    renderPanel();
    await screen.findByTestId("deck-panel");
    fireEvent.click(screen.getByRole("tab", { name: "deck.resourceLayer" }));
    fireEvent.change(screen.getByLabelText("deck.credentialSource"), {
      target: { value: "env" },
    });
    fireEvent.change(screen.getByLabelText("deck.keyVariable"), {
      target: { value: "" },
    });
    expect(screen.getByLabelText("deck.keyVariable")).toHaveValue("");
    expect(screen.queryByLabelText("deck.apiKey")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("deck.keyVariable"), {
      target: { value: "DECK_SHARED_KEY" },
    });
    fireEvent.click(screen.getByRole("button", { name: "deck.saveResources" }));
    await waitFor(() =>
      expect(view.store.providers.upstream.keyEnv).toBe("DECK_SHARED_KEY"),
    );
    expect(view.store.providers.upstream.apiKey).toBe("");
  });

  it("blocks deleting resources referenced by endpoint mappings", async () => {
    renderPanel();
    await screen.findByTestId("deck-panel");
    fireEvent.click(screen.getByRole("tab", { name: "deck.resourceLayer" }));
    fireEvent.click(screen.getByRole("button", { name: "deck.deleteModel" }));
    expect(error).toHaveBeenCalledWith("deck.resourceInUse");
    expect(screen.getByLabelText("deck.upstreamModel")).toHaveValue(
      "private-model-id",
    );
  });

  it("shows the Cursor ingress requirement and prevents applying an unusable endpoint", async () => {
    render(<DeckPanel client="cursor-agent" onBusyChange={onBusy} />);
    await screen.findByTestId("deck-panel");
    expect(
      screen.getByText("Configure public HTTPS ingress"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "deck.saveAndApply" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "deck.saveSelection" }),
    ).toBeEnabled();
  });
});
