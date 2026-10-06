import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * Tailwind silently drops a class it cannot parse, so a colour typo costs no
 * error at build time and simply renders nothing. These guard the two ways
 * that has already happened: a token carrying two opacity suffixes, and an
 * alpha that is not on the generated scale.
 */

const ROOT = join(__dirname, "..", "..");

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sources(path);
    return /\.tsx?$/.test(entry.name) ? [path] : [];
  });
}

const SOURCES = sources(join(ROOT, "src"));

const COLOUR_PREFIX =
  "bg|text|border|ring|from|to|via|outline|divide|placeholder|caret|decoration|fill|stroke|shadow|accent";

/** Palette names the design tokens replaced; none should remain in source. */
const RAW_PALETTE =
  "slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|" +
  "teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose";

function scan(pattern: RegExp): string[] {
  const hits: string[] = [];
  for (const file of SOURCES) {
    const text = readFileSync(file, "utf8");
    text.split("\n").forEach((line, i) => {
      for (const m of line.matchAll(pattern)) {
        hits.push(`${file.slice(ROOT.length + 1)}:${i + 1} ${m[0]}`);
      }
    });
  }
  return hits;
}

describe("tailwind colour tokens", () => {
  it("never carries two opacity suffixes", () => {
    // `border-destructive/40/70` parses as nothing and draws no border.
    expect(
      scan(
        new RegExp(`\\b(?:${COLOUR_PREFIX})-[a-z][a-z-]*/\\d+/\\d+\\b`, "g"),
      ),
    ).toEqual([]);
  });

  it("only uses alpha values Tailwind generates", () => {
    // The default scale runs 0..100 in steps of five; `/12` yields no rule.
    const offScale = scan(
      new RegExp(`\\b(?:${COLOUR_PREFIX})-[a-z][a-z-]*/(\\d+)\\b`, "g"),
    ).filter((hit) => {
      const alpha = Number(hit.split("/").pop());
      return alpha % 5 !== 0 || alpha > 100;
    });
    expect(offScale).toEqual([]);
  });

  it("names design tokens rather than raw palette shades", () => {
    expect(
      scan(
        new RegExp(
          `\\b(?:${COLOUR_PREFIX})-(?:${RAW_PALETTE})-\\d{2,3}\\b`,
          "g",
        ),
      ),
    ).toEqual([]);
  });
});
