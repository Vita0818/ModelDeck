import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { PROTOCOL_LABELS } from "@/lib/api/deck";
import {
  searchDeckPresets,
  type DeckProviderPreset,
} from "@/config/deckProviderPresets";

/**
 * Picks one upstream from the carried-over catalog. Selecting a preset only
 * seeds the resource fields; credentials are never part of a preset.
 */
export default function PresetPicker({
  onPick,
  onCancel,
}: {
  onPick: (preset: DeckProviderPreset) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const results = useMemo(() => searchDeckPresets(query), [query]);

  return (
    <div className="space-y-3">
      <div className="relative">
        <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          autoFocus
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("deck.presetSearchPlaceholder")}
          aria-label={t("deck.presetSearchPlaceholder")}
          className="pl-9"
        />
      </div>

      <div className="max-h-80 space-y-1 overflow-y-auto">
        {results.map((p) => (
          <button
            key={p.id}
            type="button"
            onClick={() => onPick(p)}
            className="w-full rounded-md px-3 py-2 text-left hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <span className="flex items-baseline justify-between gap-3">
              <span className="truncate text-sm font-medium">{p.name}</span>
              <span className="shrink-0 text-xs text-muted-foreground">
                {PROTOCOL_LABELS[p.protocol]}
              </span>
            </span>
            <span className="block truncate text-xs text-muted-foreground">
              {p.baseUrl}
              {p.models.length > 0 &&
                ` · ${t("deck.presetModelCount", { count: p.models.length })}`}
            </span>
          </button>
        ))}
        {!results.length && (
          <p className="px-3 py-6 text-center text-sm text-muted-foreground">
            {t("deck.presetNoResults")}
          </p>
        )}
      </div>

      <div className="flex justify-end">
        <Button variant="ghost" size="sm" onClick={onCancel}>
          {t("common.cancel")}
        </Button>
      </div>
    </div>
  );
}
