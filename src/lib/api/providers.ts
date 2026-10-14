import { invoke } from "@tauri-apps/api/core";
import type { Provider } from "@/types";
import type { AppId } from "./types";

/** Names for historical usage records; resources use deckApi. */
export const providersApi = {
  getAll: (appId: AppId): Promise<Record<string, Provider>> =>
    invoke("get_providers", { app: appId }),
  getCurrent: (appId: AppId): Promise<string> =>
    invoke("get_current_provider", { app: appId }),
  updateTrayMenu: (): Promise<boolean> => invoke("update_tray_menu"),
};
