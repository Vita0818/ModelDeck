import type { DeckView } from "@/lib/api/deck";
import { CLIENT_IDS } from "@/config/modeldeckClients";

export function deckFixture(): DeckView {
  return {
    origin: "http://127.0.0.1:15721",
    store: {
      schemaVersion: 1,
      revision: 0,
      providers: {
        upstream: {
          id: "upstream",
          name: "Private provider",
          baseUrl: "https://private-upstream.test/v1",
          protocol: "openai-chat",
          apiKey: "upstream-only-secret",
          keyEnv: "",
        },
      },
      models: {
        resource: {
          id: "resource",
          providerId: "upstream",
          upstreamId: "private-model-id",
          name: "Private model",
          context: 200000,
          output: 8192,
          reasoning: ["high", "max"],
          tools: true,
          images: false,
        },
      },
      endpoints: {
        entry: {
          id: "entry",
          name: "Shared endpoint",
          protocols: ["openai-chat", "openai-responses", "anthropic", "gemini"],
          publicUrl: "",
          token: "gateway-only-token",
          routes: {
            coding: { modelId: "resource", effort: "high" },
            "claude-sonnet-5": { modelId: "resource", effort: "max" },
          },
        },
      },
      bindings: Object.fromEntries(
        CLIENT_IDS.map((id) => [
          id,
          {
            endpointId: "entry",
            model: id === "claude-desktop" ? "claude-sonnet-5" : "coding",
            utilityModel: "",
            executable: "",
          },
        ]),
      ),
      activeBindings: {},
      migration: { importedProviders: 1, importedModels: 1, retained: [] },
    },
    compatibility: CLIENT_IDS.map((client) => ({
      client,
      endpointId: "entry",
      models: client === "claude-desktop" ? ["claude-sonnet-5"] : ["coding"],
      issue: ["cursor", "cursor-agent"].includes(client)
        ? "Configure public HTTPS ingress"
        : null,
    })),
  };
}
