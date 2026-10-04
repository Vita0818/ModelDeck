import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ModelClientsPanel from "@/components/model-clients/ModelClientsPanel";

const { invoke, success } = vi.hoisted(() => ({
  invoke: vi.fn(),
  success: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("sonner", () => ({ toast: { success } }));

const model = {
  id: "vendor/model",
  name: "Native name",
  context: 1050000,
  output: 128000,
  reasoning: ["high", "xhigh", "max"],
};
const profile = (keyEnv: string) => ({
  endpoint: "https://openrouter.ai/api/v1",
  protocol: "openai-chat",
  apiKey: "",
  keyEnv,
  models: [model],
  defaultModel: model.id,
  effort: "max",
  executable: "/test/native",
  enabled: false,
  localToken: "",
});

describe("ModelClientsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invoke.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_model_clients":
          return {
            profiles: {
              antigravity: profile("AGY_OR_KEY"),
              "cursor-agent": profile("CURSOR_CLI_OR_KEY"),
            },
            active: {},
          };
        case "inspect_model_clients":
          return [];
        case "get_model_client_catalog_sources":
          return [{ id: "openrouter", name: "OpenRouter", count: 2 }];
        case "import_model_client_catalog":
          return [
            model,
            {
              ...model,
              id: "other/model",
              name: "Another native name",
              context: 200000,
            },
          ];
        case "save_model_client":
          return undefined;
        case "apply_model_client":
          throw new Error("Local runtime is unavailable");
        default:
          throw new Error(`Unexpected invocation: ${command}`);
      }
    });
  });

  it("keeps each client's key binding and drafts separate", async () => {
    render(<ModelClientsPanel />);
    await screen.findByDisplayValue("AGY_OR_KEY");
    fireEvent.change(screen.getByLabelText("modelClients.endpoint"), {
      target: { value: "http://localhost:1234/v1" },
    });
    fireEvent.click(screen.getByRole("tab", { name: "Cursor Agent CLI" }));
    expect(screen.getByLabelText("modelClients.envName")).toHaveValue(
      "CURSOR_CLI_OR_KEY",
    );
    expect(screen.getByLabelText("modelClients.endpoint")).toHaveValue(
      "https://openrouter.ai/api/v1",
    );
    fireEvent.click(screen.getByRole("tab", { name: "Antigravity CLI" }));
    expect(screen.getByLabelText("modelClients.endpoint")).toHaveValue(
      "http://localhost:1234/v1",
    );
    fireEvent.click(screen.getByRole("button", { name: "modelClients.save" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        "save_model_client",
        expect.objectContaining({
          client: "antigravity",
          profile: expect.objectContaining({ keyEnv: "AGY_OR_KEY" }),
        }),
      ),
    );
    expect(invoke.mock.calls.some(([c]) => c === "apply_model_client")).toBe(
      false,
    );
  });

  it("imports the selected provider's metadata without duplicating models or changing credentials", async () => {
    render(<ModelClientsPanel />);
    await screen.findByDisplayValue("AGY_OR_KEY");
    fireEvent.click(
      screen.getByRole("button", { name: "modelClients.importModels" }),
    );
    await screen.findByDisplayValue("Another native name");
    expect(screen.getAllByDisplayValue("vendor/model")).toHaveLength(1);
    expect(screen.getByLabelText("modelClients.envName")).toHaveValue(
      "AGY_OR_KEY",
    );
    expect(invoke).toHaveBeenCalledWith("import_model_client_catalog", {
      providerId: "openrouter",
    });
    expect(screen.getByLabelText("modelClients.effort")).toHaveValue("max");
  });

  it("surfaces native capability failures without claiming the connection was applied", async () => {
    render(<ModelClientsPanel />);
    await screen.findByDisplayValue("AGY_OR_KEY");
    fireEvent.click(screen.getByRole("tab", { name: "Cursor Agent CLI" }));
    fireEvent.click(screen.getByRole("button", { name: "modelClients.apply" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Local runtime is unavailable",
    );
    expect(success).not.toHaveBeenCalledWith("modelClients.applied");
    const writes = invoke.mock.calls.filter(
      ([c]) => c === "save_model_client" || c === "apply_model_client",
    );
    expect(writes.map(([c]) => c)).toEqual([
      "save_model_client",
      "apply_model_client",
    ]);
  });
});
