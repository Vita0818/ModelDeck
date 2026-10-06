import { useMemo } from "react";
import {
  AlertCircle,
  CheckCircle2,
  FolderOpen,
  Loader2,
  Save,
  XCircle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { useTranslation } from "react-i18next";
import type { ImportStatus } from "@/hooks/useImportExport";

interface ImportExportSectionProps {
  status: ImportStatus;
  selectedFile: string;
  errorMessage: string | null;
  backupId: string | null;
  isImporting: boolean;
  onSelectFile: () => Promise<void>;
  onImport: () => Promise<void>;
  onExport: () => Promise<void>;
  onClear: () => void;
}

export function ImportExportSection({
  status,
  selectedFile,
  errorMessage,
  backupId,
  isImporting,
  onSelectFile,
  onImport,
  onExport,
  onClear,
}: ImportExportSectionProps) {
  const { t } = useTranslation();

  const selectedFileName = useMemo(() => {
    if (!selectedFile) return "";
    const segments = selectedFile.split(/[\\/]/);
    return segments[segments.length - 1] || selectedFile;
  }, [selectedFile]);

  return (
    <section className="space-y-4">
      <header className="space-y-2">
        <h3 className="text-base font-semibold text-foreground">
          {t("settings.importExport")}
        </h3>
        <p className="text-sm text-muted-foreground">
          {t("settings.importExportHint")}
        </p>
      </header>

      <div className="space-y-4 rounded-lg border border-border bg-muted p-6">
        {/* Import and Export Buttons Side by Side */}
        <div className="grid grid-cols-2 gap-4 items-stretch">
          {/* Import Button */}
          <div className="relative">
            <Button
              type="button"
              className={`w-full h-auto py-3 px-4 bg-primary hover:bg-primary dark:bg-primary dark:hover:bg-primary text-white ${selectedFile && !isImporting ? "flex-col items-start" : "items-center"}`}
              onClick={!selectedFile ? onSelectFile : onImport}
              disabled={isImporting}
            >
              <div className="flex items-center gap-2 w-full justify-center">
                {isImporting ? (
                  <Loader2 className="size-4 animate-spin flex-shrink-0" />
                ) : selectedFile ? (
                  <CheckCircle2 className="size-4 flex-shrink-0" />
                ) : (
                  <FolderOpen className="size-4 flex-shrink-0" />
                )}
                <span className="font-medium">
                  {isImporting
                    ? t("settings.importing")
                    : selectedFile
                      ? t("settings.import")
                      : t("settings.selectConfigFile")}
                </span>
              </div>
              {selectedFile && !isImporting && (
                <div className="mt-2 w-full text-left">
                  <p className="text-xs font-mono text-white/80 truncate">
                    📄 {selectedFileName}
                  </p>
                </div>
              )}
            </Button>
            {selectedFile && (
              <button
                type="button"
                onClick={onClear}
                className="absolute -top-2 -right-2 size-6 rounded-full bg-destructive hover:bg-destructive text-white flex items-center justify-center shadow-lg transition-colors z-10"
                aria-label={t("common.clear")}
              >
                <XCircle className="size-4" />
              </button>
            )}
          </div>

          {/* Export Button */}
          <div>
            <Button
              type="button"
              className="w-full h-full py-3 px-4 bg-primary hover:bg-primary dark:bg-primary dark:hover:bg-primary text-white items-center"
              onClick={onExport}
            >
              <Save className="mr-2 size-4" />
              {t("settings.exportConfig")}
            </Button>
          </div>
        </div>

        <ImportStatusMessage
          status={status}
          errorMessage={errorMessage}
          backupId={backupId}
        />
      </div>
    </section>
  );
}

interface ImportStatusMessageProps {
  status: ImportStatus;
  errorMessage: string | null;
  backupId: string | null;
}

function ImportStatusMessage({
  status,
  errorMessage,
  backupId,
}: ImportStatusMessageProps) {
  const { t } = useTranslation();

  if (status === "idle") {
    return null;
  }

  const baseClass =
    "flex items-start gap-3 rounded-xl border p-4 text-sm leading-relaxed";

  if (status === "importing") {
    return (
      <div
        className={`${baseClass} border-primary/40/30 bg-primary/10 text-primary`}
      >
        <Loader2 className="mt-0.5 size-5 flex-shrink-0 animate-spin" />
        <div>
          <p className="font-semibold">{t("settings.importing")}</p>
          <p className="text-primary/80 dark:text-primary/80">
            {t("common.loading")}
          </p>
        </div>
      </div>
    );
  }

  if (status === "success") {
    return (
      <div
        className={`${baseClass} border-success/40/30 bg-success/10 text-success`}
      >
        <CheckCircle2 className="mt-0.5 size-5 flex-shrink-0" />
        <div className="space-y-1.5">
          <p className="font-semibold">{t("settings.importSuccess")}</p>
          {backupId ? (
            <p className="text-xs text-success/80 dark:text-success/80">
              {t("settings.backupId")}: {backupId}
            </p>
          ) : null}
          <p className="text-success/80 dark:text-success/80">
            {t("settings.autoReload")}
          </p>
        </div>
      </div>
    );
  }

  if (status === "partial-success") {
    return (
      <div
        className={`${baseClass} border-warning/40/30 bg-warning/10 text-warning`}
      >
        <AlertCircle className="mt-0.5 size-5 flex-shrink-0" />
        <div className="space-y-1.5">
          <p className="font-semibold">{t("settings.importPartialSuccess")}</p>
          <p className="text-warning/80 dark:text-warning/80">
            {t("settings.importPartialHint")}
          </p>
        </div>
      </div>
    );
  }

  const message = errorMessage || t("settings.importFailed");

  return (
    <div
      className={`${baseClass} border-destructive/40/30 bg-destructive/10 text-destructive`}
    >
      <AlertCircle className="mt-0.5 size-5 flex-shrink-0" />
      <div className="space-y-1.5">
        <p className="font-semibold">{t("settings.importFailed")}</p>
        <p className="text-destructive/80 dark:text-destructive/80">
          {message}
        </p>
      </div>
    </div>
  );
}
