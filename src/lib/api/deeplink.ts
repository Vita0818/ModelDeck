import { invoke } from "@tauri-apps/api/core";

export type ResourceType = "provider";

export interface DeepLinkImportRequest {
  version: string;
  resource: ResourceType;

  // Common fields
  app?:
    | "claude"
    | "claude-desktop"
    | "codex"
    | "grokbuild"
    | "opencode"
    | "hermes";
  name?: string;
  enabled?: boolean;

  // Provider fields
  homepage?: string;
  endpoint?: string;
  apiKey?: string;
  icon?: string;
  model?: string;
  notes?: string;
  haikuModel?: string;
  sonnetModel?: string;
  opusModel?: string;

  // Config file fields
  config?: string;
  configFormat?: string;
  configUrl?: string;
}

export type ImportResult = {
  type: "provider";
  id: string;
  requiresApply?: boolean;
};

export const deeplinkApi = {
  /**
   * Parse a deep link URL
   * @param url The ccswitch:// URL to parse
   * @returns Parsed deep link request
   */
  parseDeeplink: async (url: string): Promise<DeepLinkImportRequest> => {
    return invoke("parse_deeplink", { url });
  },

  /**
   * Merge configuration from Base64/URL into a deep link request
   * This is used to show the complete configuration in the confirmation dialog
   * @param request The deep link import request
   * @returns Merged deep link request with config fields populated
   */
  mergeDeeplinkConfig: async (
    request: DeepLinkImportRequest,
  ): Promise<DeepLinkImportRequest> => {
    return invoke("merge_deeplink_config", { request });
  },

  /**
   * Import a resource from a deep link request (unified handler)
   * @param request The deep link import request
   * @returns Import result based on resource type
   */
  importFromDeeplink: async (
    request: DeepLinkImportRequest,
  ): Promise<ImportResult> => {
    return invoke("import_from_deeplink_unified", { request });
  },
};
