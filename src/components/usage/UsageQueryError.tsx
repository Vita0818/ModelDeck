import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";

export function UsageQueryError({ onRetry }: { onRetry: () => void }) {
  const { t } = useTranslation();
  return (
    <div
      role="alert"
      className="flex items-center justify-between gap-3 rounded-xl border border-destructive/30 bg-card p-4"
    >
      <p className="text-sm text-destructive">{t("usage.loadError")}</p>
      <Button variant="outline" size="sm" onClick={onRetry}>
        {t("usage.retry")}
      </Button>
    </div>
  );
}
