import { useTranslation } from "react-i18next";
import { Loader2, Save, Undo2 } from "lucide-react";
import { Button } from "@/components/ui/button";

/**
 * Unsaved-changes bar for the resource pages.
 *
 * Resource edits are batched locally, so the only signal that work is pending
 * has to be explicit. The bar occupies no space until something is dirty.
 */
export function ResourceToolbar({
  dirty,
  saving,
  commit,
  discard,
}: {
  dirty: boolean;
  saving: boolean;
  commit: () => void | Promise<void>;
  discard: () => void;
}) {
  const { t } = useTranslation();
  if (!dirty) return null;

  return (
    <div
      role="status"
      className="flex animate-slide-down items-center gap-3 rounded-xl border border-primary/30 bg-primary/[0.07] px-3.5 py-2.5"
    >
      <span className="min-w-0 flex-1 text-base text-foreground">
        {t("deck.unsavedChanges")}
      </span>
      <Button variant="ghost" size="sm" onClick={discard} disabled={saving}>
        <Undo2 />
        {t("common.discard")}
      </Button>
      <Button size="sm" onClick={() => void commit()} disabled={saving}>
        {saving ? <Loader2 className="animate-spin" /> : <Save />}
        {t("deck.saveResources")}
      </Button>
    </div>
  );
}
