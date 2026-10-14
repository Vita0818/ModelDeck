import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "@/types";
import type { AppId } from "./types";

export interface ConfigTransferResult {
  success: boolean;
  message: string;
  filePath?: string;
  backupId?: string;
}

export interface LogConfig {
  enabled: boolean;
  level: "error" | "warn" | "info" | "debug" | "trace";
}

export interface BackupEntry {
  filename: string;
  sizeBytes: number;
  createdAt: string;
}

export const settingsApi = {
  get: (): Promise<Settings> => invoke("get_settings"),
  save: (settings: Settings): Promise<boolean> =>
    invoke("save_settings", { settings }),
  restart: (): Promise<boolean> => invoke("restart_app"),
  isPortable: (): Promise<boolean> => invoke("is_portable_mode"),
  getConfigDir: (app: AppId): Promise<string> =>
    invoke("get_config_dir", { app }),
  openConfigFolder: (app: AppId): Promise<void> =>
    invoke("open_config_folder", { app }),
  pickDirectory: (defaultPath?: string): Promise<string | null> =>
    invoke("pick_directory", { defaultPath }),
  selectConfigDirectory: (defaultPath?: string): Promise<string | null> =>
    invoke("pick_directory", { defaultPath }),
  getClaudeCodeConfigPath: (): Promise<string> =>
    invoke("get_claude_code_config_path"),
  getAppConfigPath: (): Promise<string> => invoke("get_app_config_path"),
  openAppConfigFolder: (): Promise<void> => invoke("open_app_config_folder"),
  getAppConfigDirOverride: (): Promise<string | null> =>
    invoke("get_app_config_dir_override"),
  setAppConfigDirOverride: (path: string | null): Promise<boolean> =>
    invoke("set_app_config_dir_override", { path }),
  setAutoLaunch: (enabled: boolean): Promise<boolean> =>
    invoke("set_auto_launch", { enabled }),
  getAutoLaunchStatus: (): Promise<boolean> => invoke("get_auto_launch_status"),
  saveFileDialog: (defaultName: string): Promise<string | null> =>
    invoke("save_file_dialog", { defaultName }),
  openFileDialog: (): Promise<string | null> => invoke("open_file_dialog"),
  exportConfigToFile: (filePath: string): Promise<ConfigTransferResult> =>
    invoke("export_config_to_file", { filePath }),
  importConfigFromFile: (filePath: string): Promise<ConfigTransferResult> =>
    invoke("import_config_from_file", { filePath }),
  getLogConfig: (): Promise<LogConfig> => invoke("get_log_config"),
  setLogConfig: (config: LogConfig): Promise<boolean> =>
    invoke("set_log_config", { config }),
};

export const backupsApi = {
  createDbBackup: (): Promise<string> => invoke("create_db_backup"),
  listDbBackups: (): Promise<BackupEntry[]> => invoke("list_db_backups"),
  restoreDbBackup: (filename: string): Promise<string> =>
    invoke("restore_db_backup", { filename }),
  renameDbBackup: (oldFilename: string, newName: string): Promise<string> =>
    invoke("rename_db_backup", { oldFilename, newName }),
  deleteDbBackup: (filename: string): Promise<void> =>
    invoke("delete_db_backup", { filename }),
};
