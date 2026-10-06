import { describe, expect, it } from "vitest";
import {
  deckProviderPresets,
  searchDeckPresets,
} from "@/config/deckProviderPresets";
import { PROTOCOLS } from "@/lib/api/deck";

/**
 * The catalog is generated once by scripts/extract-deck-presets.ts from the
 * retired per-harness preset files. These guard the invariants the deck
 * resource model depends on, since the source files are gone and the data
 * cannot be re-derived.
 */
describe("deck provider presets", () => {
  it("carries the migrated catalog", () => {
    expect(deckProviderPresets.length).toBeGreaterThan(200);
  });

  it("gives every upstream a unique id", () => {
    const ids = deckProviderPresets.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("keeps one entry per endpoint and protocol pair", () => {
    const keys = deckProviderPresets.map((p) => `${p.baseUrl}|${p.protocol}`);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("only uses protocols the gateway can resolve", () => {
    for (const p of deckProviderPresets) {
      expect(PROTOCOLS).toContain(p.protocol);
    }
  });

  it("points every upstream at an absolute endpoint", () => {
    for (const p of deckProviderPresets) {
      expect(p.baseUrl).toMatch(/^https?:\/\//);
    }
  });

  it("never ships a credential", () => {
    const serialized = JSON.stringify(deckProviderPresets);
    expect(serialized).not.toMatch(/"apiKey"/);
    expect(serialized).not.toMatch(/sk-[A-Za-z0-9]{10}/);
  });

  it("keeps model ids unique within an upstream", () => {
    for (const p of deckProviderPresets) {
      const ids = p.models.map((m) => m.id);
      expect(new Set(ids).size).toBe(ids.length);
    }
  });

  it("records positive limits whenever a limit is known", () => {
    for (const p of deckProviderPresets) {
      for (const m of p.models) {
        if (m.context !== undefined) expect(m.context).toBeGreaterThan(0);
        if (m.output !== undefined) expect(m.output).toBeGreaterThan(0);
      }
    }
  });

  it("preserves the capability metadata that only Pi recorded", () => {
    const detailed = deckProviderPresets.filter((p) =>
      p.models.some((m) => m.context && m.reasoning !== undefined),
    );
    expect(detailed.length).toBeGreaterThan(50);
  });

  it("searches across name, endpoint and model id", () => {
    expect(searchDeckPresets("").length).toBe(deckProviderPresets.length);
    expect(searchDeckPresets("moonshot.cn").length).toBeGreaterThan(0);
    expect(searchDeckPresets("zzz-no-such-provider")).toHaveLength(0);
  });
});
