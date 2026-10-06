import type { TFunction } from "i18next";
import type { AppId } from "@/lib/api";

export type View =
  | "providers"
  | "settings"
  | "prompts"
  | "skills"
  | "skillsDiscovery"
  | "mcp"
  | "agents"
  | "universal"
  | "sessions"
  | "workspace"
  | "openclawEnv"
  | "openclawTools"
  | "openclawAgents"
  | "hermesMemory"
  | "modelClients";

type TitleEntry =
  | string
  | { key: string; defaultValue?: string }
  | ((t: TFunction, appId: AppId) => string);

const VIEW_TITLES: Record<Exclude<View, "providers">, TitleEntry> = {
  settings: "settings.title",
  prompts: (t, appId) => t("prompts.title", { appName: t(`apps.${appId}`) }),
  skills: "skills.title",
  skillsDiscovery: "skills.title",
  mcp: "mcp.unifiedPanel.title",
  agents: "agents.title",
  universal: { key: "universalProvider.title", defaultValue: "统一供应商" },
  sessions: "sessionManager.title",
  workspace: "workspace.title",
  openclawEnv: "openclaw.env.title",
  openclawTools: "openclaw.tools.title",
  openclawAgents: "openclaw.agents.title",
  hermesMemory: "hermes.memory.title",
  modelClients: "modelClients.title",
};

/**
 * Header title for a secondary view. `providers` is the root view and renders
 * the client switcher instead of a title.
 */
export function resolveViewTitle(
  view: View,
  t: TFunction,
  sharedFeatureApp: AppId,
): string {
  if (view === "providers") return "";

  const entry = VIEW_TITLES[view];
  if (typeof entry === "function") return entry(t, sharedFeatureApp);
  if (typeof entry === "string") return t(entry);
  return t(entry.key, { defaultValue: entry.defaultValue });
}
