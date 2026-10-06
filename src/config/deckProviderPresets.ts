import type { Protocol } from "@/lib/api/deck";
import raw from "./deckProviderPresets.json";

/**
 * Upstream catalog carried over from the per-harness preset files.
 *
 * Those catalogs described the same upstream once per client in a different
 * shape. `scripts/extract-deck-presets.ts` normalises them into the deck
 * resource model and merges on (baseUrl, protocol), the same identity the Rust
 * migration uses, so one entry here is one `UpstreamProvider`.
 *
 * Capability data is sparse by origin: only some of the source catalogs
 * recorded context windows, output limits, reasoning or image support. Fields
 * are therefore optional, and the editor leaves unknown limits for the user to
 * fill in rather than inventing a value.
 */
export interface DeckPresetModel {
  id: string;
  name: string;
  context?: number;
  output?: number;
  reasoning?: boolean;
  images?: boolean;
}

export interface DeckProviderPreset {
  id: string;
  name: string;
  baseUrl: string;
  protocol: Protocol;
  websiteUrl?: string;
  apiKeyUrl?: string;
  category?: string;
  icon?: string;
  iconColor?: string;
  models: DeckPresetModel[];
}

export const deckProviderPresets = raw as DeckProviderPreset[];

/** Case-insensitive match over display name, endpoint and model ids. */
export function searchDeckPresets(query: string): DeckProviderPreset[] {
  const q = query.trim().toLowerCase();
  if (!q) return deckProviderPresets;
  return deckProviderPresets.filter(
    (p) =>
      p.name.toLowerCase().includes(q) ||
      p.baseUrl.toLowerCase().includes(q) ||
      p.models.some(
        (m) =>
          m.id.toLowerCase().includes(q) || m.name.toLowerCase().includes(q),
      ),
  );
}
