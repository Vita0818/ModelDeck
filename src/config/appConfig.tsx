import React from "react";
import type { AppId } from "@/lib/api/types";
import type { VisibleApps } from "@/types";
import { resolveClientVisibility } from "./modeldeckClients";
import {
  ClaudeIcon,
  CodexIcon,
  GeminiIcon,
  OpenClawIcon,
} from "@/components/BrandIcons";
import { ProviderIcon } from "@/components/ProviderIcon";

export interface AppConfig {
  label: string;
  icon: React.ReactNode;
  activeClass: string;
  badgeClass: string;
}

export const APP_IDS: AppId[] = [
  "codex",
  "claude",
  "claude-desktop",
  "grokbuild",
  "opencode",
  "hermes",
];

export const DEFAULT_VISIBLE_APPS: VisibleApps = resolveClientVisibility();

/** App IDs shown in Skills panels. */
export const SKILLS_APP_IDS: AppId[] = [
  "claude",
  "codex",
  "grokbuild",
  "opencode",
  "hermes",
];

export type ProxyAppId = Extract<
  AppId,
  "claude" | "codex" | "gemini" | "grokbuild"
>;

/** Apps with a complete local gateway + failover data plane. */
export const PROXY_APP_IDS: ProxyAppId[] = ["claude", "codex", "grokbuild"];

export function isProxyAppId(appId: string): appId is ProxyAppId {
  return (PROXY_APP_IDS as string[]).includes(appId);
}

export type AdditiveAppId = Extract<
  AppId,
  "opencode" | "openclaw" | "hermes" | "pi" | "mcode"
>;

export const ADDITIVE_APP_IDS: AdditiveAppId[] = [
  "mcode",
  "opencode",
  "openclaw",
  "hermes",
  "pi",
];

export function isAdditiveAppId(appId: string): appId is AdditiveAppId {
  return (ADDITIVE_APP_IDS as string[]).includes(appId);
}

/** Pi has no native MCP registry; do not manufacture a disabled mirror. */
export type McpAppId = Exclude<AppId, "claude-desktop" | "openclaw" | "pi">;
export const MCP_APP_IDS: McpAppId[] = [
  "claude",
  "codex",
  "grokbuild",
  "opencode",
  "hermes",
];

export function isMcpAppId(appId: string): appId is McpAppId {
  return (MCP_APP_IDS as string[]).includes(appId);
}

export const APP_ICON_MAP: Record<AppId, AppConfig> = {
  claude: {
    label: "Claude",
    icon: <ClaudeIcon size={14} />,
    activeClass:
      "bg-warning/10 ring-1 ring-orange-500/20 hover:bg-warning/20 text-warning",
    badgeClass:
      "bg-warning/10 text-warning hover:bg-warning/20 border-0 gap-1.5",
  },
  "claude-desktop": {
    label: "Claude Desktop",
    icon: <ClaudeIcon size={14} />,
    activeClass:
      "bg-warning/10 ring-1 ring-amber-500/20 hover:bg-warning/20 text-warning",
    badgeClass:
      "bg-warning/10 text-warning hover:bg-warning/20 border-0 gap-1.5",
  },
  codex: {
    label: "Codex",
    icon: <CodexIcon size={14} />,
    activeClass:
      "bg-success/10 ring-1 ring-green-500/20 hover:bg-success/20 text-success",
    badgeClass:
      "bg-success/10 text-success hover:bg-success/20 border-0 gap-1.5",
  },
  gemini: {
    label: "Gemini",
    icon: <GeminiIcon size={14} />,
    activeClass:
      "bg-primary/10 ring-1 ring-blue-500/20 hover:bg-primary/20 text-primary",
    badgeClass:
      "bg-primary/10 text-primary hover:bg-primary/20 border-0 gap-1.5",
  },
  grokbuild: {
    label: "Grok Build",
    icon: (
      <ProviderIcon
        icon="grok"
        name="Grok Build"
        size={14}
        showFallback={false}
      />
    ),
    activeClass:
      "bg-primary/10 ring-1 ring-cyan-500/20 hover:bg-primary/20 text-primary",
    badgeClass:
      "bg-primary/10 text-primary hover:bg-primary/20 border-0 gap-1.5",
  },
  opencode: {
    label: "OpenCode",
    icon: (
      <ProviderIcon
        icon="opencode"
        name="OpenCode"
        size={14}
        showFallback={false}
      />
    ),
    activeClass:
      "bg-primary/10 ring-1 ring-indigo-500/20 hover:bg-primary/20 text-primary",
    badgeClass:
      "bg-primary/10 text-primary hover:bg-primary/20 border-0 gap-1.5",
  },
  openclaw: {
    label: "OpenClaw",
    icon: <OpenClawIcon size={14} />,
    activeClass:
      "bg-destructive/10 ring-1 ring-rose-500/20 hover:bg-destructive/20 text-destructive",
    badgeClass:
      "bg-destructive/10 text-destructive hover:bg-destructive/20 border-0 gap-1.5",
  },
  hermes: {
    label: "Hermes",
    icon: (
      <ProviderIcon
        icon="hermes"
        name="Hermes"
        size={14}
        showFallback={false}
      />
    ),
    activeClass:
      "bg-system-purple/10 ring-1 ring-violet-500/20 hover:bg-system-purple/20 text-system-purple dark:text-system-purple",
    badgeClass:
      "bg-system-purple/10 text-system-purple dark:text-system-purple hover:bg-system-purple/20 border-0 gap-1.5",
  },
  mcode: {
    label: "MiniMax Code",
    icon: <ProviderIcon icon="minimax" name="MiniMax Code" size={14} />,
    activeClass: "bg-warning/10 text-warning",
    badgeClass: "bg-warning/10 text-warning border-0 gap-1.5",
  },
  pi: {
    label: "Pi",
    icon: <ProviderIcon icon="pi" name="Pi" size={14} showFallback={false} />,
    activeClass:
      "bg-system-purple/10 ring-1 ring-fuchsia-500/20 hover:bg-system-purple/20 text-system-purple dark:text-system-purple",
    badgeClass:
      "bg-system-purple/10 text-system-purple dark:text-system-purple hover:bg-system-purple/20 border-0 gap-1.5",
  },
};

export function getAppLabel(appId: string): string {
  return APP_ICON_MAP[appId as AppId]?.label ?? appId;
}
