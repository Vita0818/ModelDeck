import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { useDeck, useSaveDeck } from "@/lib/query/deck";
import type { DeckStore } from "@/lib/api/deck";
import { extractErrorMessage } from "@/utils/errorUtils";

export const newId = () => crypto.randomUUID();

/** Only the resource half of the store is editable from these pages. */
const resourceFingerprint = (store: DeckStore) =>
  JSON.stringify([store.providers, store.models, store.endpoints]);

/**
 * Local draft over the deck store.
 *
 * Resource edits are batched and committed explicitly so a half-finished
 * provider never reaches the backend's reference validation. Bindings are
 * taken from the server copy on save: this page never owns them.
 */
export function useResourceDraft() {
  const { t } = useTranslation();
  const { data: view, isLoading, error, refetch } = useDeck();
  const save = useSaveDeck();
  const [draft, setDraft] = useState<DeckStore>();

  useEffect(() => {
    if (view) setDraft(structuredClone(view.store));
  }, [view]);

  const dirty = useMemo(
    () =>
      Boolean(
        draft &&
          view &&
          resourceFingerprint(draft) !== resourceFingerprint(view.store),
      ),
    [draft, view],
  );

  const commit = useCallback(async () => {
    if (!draft || !view) return;
    try {
      await save.mutateAsync({
        ...draft,
        revision: view.store.revision,
        bindings: view.store.bindings,
        activeBindings: view.store.activeBindings,
      });
      toast.success(t("deck.resourcesSaved"));
    } catch (e) {
      toast.error(extractErrorMessage(e));
    }
  }, [draft, view, save, t]);

  const discard = useCallback(() => {
    if (view) setDraft(structuredClone(view.store));
  }, [view]);

  return {
    view,
    draft,
    setDraft,
    dirty,
    commit,
    discard,
    saving: save.isPending,
    isLoading,
    error,
    refetch,
  };
}
