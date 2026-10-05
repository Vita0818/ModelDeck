import type { AppId } from "@/lib/api/types";
import type { VisibleApps } from "@/types";

export type ModelClientId = "antigravity" | "cursor" | "cursor-agent" | "muse";
export type ProviderClientId = Extract<
  AppId,
  "codex" | "claude" | "claude-desktop" | "grokbuild" | "opencode" | "hermes"
>;
export type ClientId = ProviderClientId | ModelClientId;

// Product scope. Keep inherited database IDs separate from the clients offered
// by ModelDeck so old data cannot restore unsupported tools to the interface.
export const MODELDECK_CLIENTS: {
  id: ClientId;
  name: string;
  icon: string;
  badge?: "terminal" | "desktop";
}[] = [
  { id: "codex", name: "Codex CLI", icon: "openai" },
  { id: "claude", name: "Claude Code", icon: "claude", badge: "terminal" },
  {
    id: "claude-desktop",
    name: "Claude Desktop",
    icon: "claude",
    badge: "desktop",
  },
  { id: "antigravity", name: "Antigravity CLI", icon: "gemini" },
  { id: "cursor-agent", name: "Cursor CLI", icon: "cursor", badge: "terminal" },
  { id: "cursor", name: "Cursor GUI", icon: "cursor", badge: "desktop" },
  { id: "grokbuild", name: "Grok", icon: "grok" },
  { id: "muse", name: "Muse Code", icon: "meta" },
  { id: "opencode", name: "OpenCode", icon: "opencode" },
  { id: "hermes", name: "Hermes", icon: "hermes" },
];

export const CLIENT_IDS = MODELDECK_CLIENTS.map(({ id }) => id);
export const CLIENT_STORAGE_KEY = "modeldeck-last-client";

export function isClientId(value: string | null): value is ClientId {
  return MODELDECK_CLIENTS.some(({ id }) => id === value);
}

export function isModelClientId(value: string): value is ModelClientId {
  return ["antigravity", "cursor", "cursor-agent", "muse"].includes(value);
}

export function resolveClientVisibility(
  saved?: Partial<VisibleApps>,
): VisibleApps {
  const visibility: VisibleApps = {
    claude: true,
    "claude-desktop": true,
    codex: true,
    gemini: false,
    grokbuild: true,
    opencode: true,
    openclaw: false,
    hermes: true,
    pi: false,
    mcode: false,
    antigravity: true,
    cursor: true,
    "cursor-agent": true,
    muse: true,
  };
  for (const id of CLIENT_IDS) {
    if (typeof saved?.[id] === "boolean") visibility[id] = saved[id];
  }
  // The upstream default hid Hermes. First adoption exposes the requested set;
  // subsequent ModelDeck visibility choices are preserved.
  if (saved && saved.antigravity === undefined) visibility.hermes = true;
  if (!CLIENT_IDS.some((id) => visibility[id])) visibility.codex = true;
  return visibility;
}

export function initialClient(): ClientId {
  const saved = localStorage.getItem(CLIENT_STORAGE_KEY);
  if (isClientId(saved)) return saved;
  if (localStorage.getItem("cc-switch-last-view") === "modelClients")
    return "antigravity";
  const legacy = localStorage.getItem("cc-switch-last-app");
  return isClientId(legacy) ? legacy : "codex";
}
