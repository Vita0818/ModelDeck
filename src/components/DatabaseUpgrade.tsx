import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { exit } from "@tauri-apps/plugin-process";
import { AlertTriangle, FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";

interface DatabaseUpgradeProps {
  payload: {
    path?: string;
    error?: string;
    kind?: string;
    db_version?: number;
    supported_version?: number;
  };
}

/** An unsupported database is kept read-only until a compatible build is used. */
export function DatabaseUpgrade({ payload }: DatabaseUpgradeProps) {
  const { t } = useTranslation();
  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-6 text-foreground">
      <div className="w-full max-w-lg space-y-5 rounded-2xl border border-border bg-card p-7 shadow-xl">
        <div className="flex items-start gap-4">
          <AlertTriangle className="size-8 shrink-0 text-destructive" />
          <div className="space-y-2">
            <h1 className="text-lg font-semibold">{t("dbUpgrade.title")}</h1>
            <p className="text-sm text-muted-foreground">
              {t("modeldeck.databaseCompatibility")}
            </p>
            {payload.db_version != null &&
              payload.supported_version != null && (
                <p className="text-xs text-muted-foreground">
                  {t("dbUpgrade.versionInfo", {
                    db: payload.db_version,
                    supported: payload.supported_version,
                  })}
                </p>
              )}
          </div>
        </div>
        <div className="space-y-1 rounded-lg bg-muted p-3 text-xs text-muted-foreground">
          {payload.error && <p className="break-words">{payload.error}</p>}
          {payload.path && <p className="break-all">{payload.path}</p>}
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            onClick={() => void invoke("open_app_config_folder")}
          >
            <FolderOpen className="size-4" />
            {t("dbUpgrade.openConfigDir")}
          </Button>
          <Button
            variant="ghost"
            className="ml-auto"
            onClick={() => void exit(0)}
          >
            {t("dbUpgrade.quit")}
          </Button>
        </div>
      </div>
    </div>
  );
}
