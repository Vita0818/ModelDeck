import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { deckFixture } from "../fixtures/deck";
import type { DeckStore } from "@/lib/api/deck";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

describe("DSH and Copilot model connections", () => {
  beforeEach(() => invokeMock.mockReset());

  it.each([
    ["dsh", "DeepSeek Harness"],
    ["copilot", "GitHub Copilot CLI"],
  ] as const)(
    "saves, applies and restores %s through the shared resource graph",
    async (client, name) => {
      let current = deckFixture();
      const command = `'/tmp/ModelDeck launchers/${client}'`;
      invokeMock.mockImplementation(
        async (
          action: string,
          args?: { store?: DeckStore; client?: string; revision?: number },
        ) => {
          if (action === "get_deck") return current;
          if (action === "save_deck") {
            current = {
              ...current,
              store: { ...args!.store!, revision: current.store.revision + 1 },
            };
            return current;
          }
          if (action === "apply_deck_binding") {
            current = {
              ...current,
              commands: { [client]: command },
              store: {
                ...current.store,
                revision: current.store.revision + 1,
                activeBindings: { [client]: current.store.bindings[client] },
              },
            };
            return { view: current, command, restartRequired: false };
          }
          if (action === "reset_deck_binding") {
            current = {
              ...current,
              commands: {},
              store: {
                ...current.store,
                revision: current.store.revision + 1,
                activeBindings: {},
              },
            };
            return current;
          }
          return null;
        },
      );
      const { default: HarnessPage } = await import(
        "@/components/deck/HarnessPage"
      );
      const query = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      render(
        <QueryClientProvider client={query}>
          <HarnessPage client={client} onNavigate={() => {}} />
        </QueryClientProvider>,
      );
      expect(await screen.findByRole("heading", { name })).toBeInTheDocument();
      await screen.findByPlaceholderText("deck.executablePlaceholder");
      expect(!!screen.queryByText("deck.utilityModel")).toBe(client === "dsh");
      fireEvent.change(
        screen.getByPlaceholderText("deck.executablePlaceholder"),
        { target: { value: `/native/${client}` } },
      );
      fireEvent.click(
        screen.getByRole("button", { name: "deck.saveAndApply" }),
      );
      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith("apply_deck_binding", {
          client,
          revision: 1,
        }),
      );
      expect(await screen.findByText(command)).toBeInTheDocument();
      expect(current.store.bindings[client]).toEqual(
        expect.objectContaining({
          endpointId: "entry",
          model: "coding",
          executable: `/native/${client}`,
        }),
      );
      expect(current.store.bindings[client]).not.toHaveProperty("apiKey");
      fireEvent.click(
        screen.getByRole("button", { name: "deck.restoreNative" }),
      );
      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith("reset_deck_binding", {
          client,
          revision: 2,
        }),
      );
      await waitFor(() =>
        expect(screen.queryByText(command)).not.toBeInTheDocument(),
      );
    },
  );

  it.each(["dsh", "copilot"] as const)(
    "shows %s's launch command when reopening an active connection",
    async (client) => {
      const fixture = deckFixture();
      fixture.store.activeBindings[client] = fixture.store.bindings[client];
      fixture.commands = { [client]: `'/tmp/ModelDeck launchers/${client}'` };
      invokeMock.mockResolvedValue(fixture);
      const { default: HarnessPage } = await import(
        "@/components/deck/HarnessPage"
      );
      const query = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      render(
        <QueryClientProvider client={query}>
          <HarnessPage client={client} onNavigate={() => {}} />
        </QueryClientProvider>,
      );
      expect(
        await screen.findByText(fixture.commands[client]!),
      ).toBeInTheDocument();
    },
  );
});
