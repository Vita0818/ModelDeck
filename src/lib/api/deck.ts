import { invoke } from "@tauri-apps/api/core";
import type { ClientId } from "@/config/modeldeckClients";

export type Protocol =
  | "openai-chat"
  | "openai-responses"
  | "anthropic"
  | "gemini";
export const PROTOCOLS: Protocol[] = [
  "openai-chat",
  "openai-responses",
  "anthropic",
  "gemini",
];
export const PROTOCOL_LABELS: Record<Protocol, string> = {
  "openai-chat": "OpenAI Chat Completions",
  "openai-responses": "OpenAI Responses",
  anthropic: "Anthropic Messages",
  gemini: "Gemini",
};
export interface UpstreamProvider {
  id: string;
  name: string;
  baseUrl: string;
  protocol: Protocol;
  apiKey: string;
  keyEnv: string;
  keySource?: "stored" | "env" | null;
}
export interface ModelResource {
  id: string;
  providerId: string;
  upstreamId: string;
  name: string;
  context: number;
  output: number;
  reasoning: string[];
  reasoningSupported?: boolean | null;
  tools: boolean;
  images: boolean;
}
export interface ModelRoute {
  modelId: string;
  effort: string;
}
export interface ExposedEndpoint {
  id: string;
  name: string;
  protocols: Protocol[];
  publicUrl: string;
  token: string;
  routes: Record<string, ModelRoute>;
}
export interface HarnessBinding {
  inputProtocol?: Protocol;
  endpointId: string;
  model: string;
  utilityModel: string;
  executable: string;
}
export interface DeckStore {
  schemaVersion: number;
  revision: number;
  providers: Record<string, UpstreamProvider>;
  models: Record<string, ModelResource>;
  endpoints: Record<string, ExposedEndpoint>;
  bindings: Partial<Record<ClientId, HarnessBinding>>;
  activeBindings: Partial<Record<ClientId, HarnessBinding>>;
  migration: {
    importedProviders: number;
    importedModels: number;
    retained: string[];
  };
}
export interface Compatibility {
  client: ClientId;
  endpointId: string;
  models: string[];
  issue?: string | null;
}
export interface DeckView {
  store: DeckStore;
  compatibility: Compatibility[];
  origin: string;
}

export const deckApi = {
  get: () => invoke<DeckView>("get_deck"),
  save: (store: DeckStore) => invoke<DeckView>("save_deck", { store }),
  apply: (client: ClientId, revision: number) =>
    invoke<{ view: DeckView; restartRequired: boolean; command?: string }>(
      "apply_deck_binding",
      { client, revision },
    ),
  restore: (client: ClientId, revision: number) =>
    invoke<DeckView>("reset_deck_binding", { client, revision }),
};
