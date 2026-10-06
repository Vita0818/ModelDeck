/**
 * One-shot extractor: legacy per-client provider presets -> deck resource presets.
 *
 * The legacy catalogs were maintained separately for each harness, so the same
 * upstream appears up to six times in different shapes. This normalises them
 * into the deck resource model and merges on (baseUrl, protocol), which is the
 * same identity the Rust migration uses.
 *
 * Capability data only exists in some catalogs. Pi carries the full set
 * (context, output, reasoning, image input) and therefore wins; the others
 * contribute endpoint and protocol coverage plus whatever limits they record.
 *
 * Run with: pnpm vite-node scripts/extract-deck-presets.ts
 */
import { writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { piProviderPresets } from "../src/config/piProviderPresets";
import { opencodeProviderPresets } from "../src/config/opencodeProviderPresets";
import { openclawProviderPresets } from "../src/config/openclawProviderPresets";
import { hermesProviderPresets } from "../src/config/hermesProviderPresets";
import { codexProviderPresets } from "../src/config/codexProviderPresets";
import { providerPresets as claudeProviderPresets } from "../src/config/claudeProviderPresets";
import { geminiProviderPresets } from "../src/config/geminiProviderPresets";
import { grokBuildProviderPresets } from "../src/config/grokBuildProviderPresets";
import { claudeDesktopProviderPresets } from "../src/config/claudeDesktopProviderPresets";

type Protocol = "openai-chat" | "openai-responses" | "anthropic" | "gemini";

interface PresetModel {
  id: string;
  name: string;
  context?: number;
  output?: number;
  reasoning?: boolean;
  images?: boolean;
}

interface DeckPreset {
  id: string;
  name: string;
  baseUrl: string;
  protocol: Protocol;
  websiteUrl?: string;
  apiKeyUrl?: string;
  category?: string;
  icon?: string;
  iconColor?: string;
  models: PresetModel[];
}

const rec = (v: unknown): Record<string, any> | undefined =>
  v && typeof v === "object" && !Array.isArray(v) ? (v as any) : undefined;

/** Normalise the various per-client protocol spellings onto deck's enum. */
function toProtocol(raw: unknown, fallback: Protocol): Protocol {
  const v = String(raw ?? "").toLowerCase();
  if (!v) return fallback;
  if (v.includes("responses")) return "openai-responses";
  if (v.includes("anthropic") || v.includes("messages")) return "anthropic";
  if (v.includes("google") || v.includes("gemini")) return "gemini";
  if (v.includes("bedrock")) return "anthropic";
  if (v.includes("completions") || v.includes("chat") || v.includes("openai"))
    return "openai-chat";
  return fallback;
}

/** Trailing slashes and casing only create duplicate upstreams. */
const normaliseUrl = (u: string) => u.trim().replace(/\/+$/, "");

const slug = (s: string) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");

const out = new Map<string, DeckPreset>();
let considered = 0;

function add(p: DeckPreset) {
  considered += 1;
  if (!p.baseUrl) return;
  const key = `${normaliseUrl(p.baseUrl)}|${p.protocol}`;
  const existing = out.get(key);
  if (!existing) {
    out.set(key, { ...p, baseUrl: normaliseUrl(p.baseUrl) });
    return;
  }
  // Merge: keep the first name, union the models, prefer entries that carry
  // capability data over bare id/name pairs.
  for (const m of p.models) {
    const hit = existing.models.find((x) => x.id === m.id);
    if (!hit) {
      existing.models.push(m);
      continue;
    }
    hit.name ||= m.name;
    hit.context ??= m.context;
    hit.output ??= m.output;
    hit.reasoning ??= m.reasoning;
    hit.images ??= m.images;
  }
  existing.websiteUrl ||= p.websiteUrl;
  existing.apiKeyUrl ||= p.apiKeyUrl;
  existing.category ||= p.category;
  existing.icon ||= p.icon;
  existing.iconColor ||= p.iconColor;
}

const common = (p: any) => ({
  websiteUrl: p.websiteUrl,
  apiKeyUrl: p.apiKeyUrl,
  category: p.category,
  icon: p.icon,
  iconColor: p.iconColor,
});

// ---- Pi: the only catalog with complete capability metadata -----------------
for (const p of piProviderPresets as any[]) {
  const c = p.settingsConfig ?? {};
  add({
    id: slug(p.name),
    name: p.name,
    baseUrl: c.baseUrl ?? "",
    protocol: toProtocol(c.api, "openai-chat"),
    ...common(p),
    models: (c.models ?? []).map((m: any) => ({
      id: m.id,
      name: m.name ?? m.id,
      context: m.contextWindow,
      output: m.maxTokens,
      reasoning: m.reasoning,
      images: Array.isArray(m.input) ? m.input.includes("image") : undefined,
    })),
  });
}

// ---- OpenClaw: models carry contextWindow ----------------------------------
for (const p of openclawProviderPresets as any[]) {
  const c = p.settingsConfig ?? {};
  add({
    id: slug(p.name),
    name: p.name,
    baseUrl: c.baseUrl ?? "",
    protocol: toProtocol(c.api, "openai-chat"),
    ...common(p),
    models: (c.models ?? []).map((m: any) => ({
      id: m.id,
      name: m.name ?? m.id,
      context: m.contextWindow,
      output: m.maxTokens,
    })),
  });
}

// ---- Hermes: snake_case, models carry context_length ------------------------
for (const p of hermesProviderPresets as any[]) {
  const c = p.settingsConfig ?? {};
  add({
    id: slug(p.name),
    name: p.name,
    baseUrl: c.base_url ?? "",
    protocol: toProtocol(c.api_mode, "openai-chat"),
    ...common(p),
    models: (c.models ?? []).map((m: any) => ({
      id: m.id,
      name: m.name ?? m.id,
      context: m.context_length,
      output: m.max_tokens,
    })),
  });
}

// ---- OpenCode: nested under options, models keyed by id --------------------
for (const p of opencodeProviderPresets as any[]) {
  const c = p.settingsConfig ?? {};
  const opts = rec(c.options) ?? {};
  const models = rec(c.models) ?? {};
  add({
    id: slug(p.name),
    name: p.name,
    baseUrl: opts.baseURL ?? opts.baseUrl ?? "",
    protocol: toProtocol(c.npm, "openai-chat"),
    ...common(p),
    models: Object.entries(models).map(([id, m]) => ({
      id,
      name: (rec(m)?.name as string) ?? id,
      context: rec(rec(m)?.limit)?.context,
      output: rec(rec(m)?.limit)?.output,
    })),
  });
}

// ---- Claude / Claude Desktop: env-var based, Anthropic protocol -------------
for (const list of [claudeProviderPresets, claudeDesktopProviderPresets]) {
  for (const p of list as any[]) {
    const env = rec(rec(p.settingsConfig)?.env) ?? {};
    add({
      id: slug(p.name),
      name: p.name,
      baseUrl: env.ANTHROPIC_BASE_URL ?? "",
      protocol: "anthropic",
      ...common(p),
      models: [],
    });
  }
}

// ---- Codex / Grok Build: TOML model_providers ------------------------------
for (const list of [codexProviderPresets, grokBuildProviderPresets]) {
  for (const p of list as any[]) {
    const cfg = rec(p.settingsConfig) ?? {};
    const providers = rec(cfg.model_providers) ?? {};
    for (const entry of Object.values(providers)) {
      const e = rec(entry);
      if (!e?.base_url) continue;
      add({
        id: slug(p.name),
        name: p.name,
        baseUrl: e.base_url,
        protocol: toProtocol(e.wire_api, "openai-responses"),
        ...common(p),
        models: [],
      });
    }
  }
}

// ---- Gemini -----------------------------------------------------------------
for (const p of geminiProviderPresets as any[]) {
  const env = rec(rec(p.settingsConfig)?.env) ?? {};
  add({
    id: slug(p.name),
    name: p.name,
    baseUrl: env.GOOGLE_GEMINI_BASE_URL ?? env.GEMINI_BASE_URL ?? "",
    protocol: "gemini",
    ...common(p),
    models: [],
  });
}

// ---- Emit -------------------------------------------------------------------
const presets = [...out.values()]
  .map((p) => ({
    ...p,
    models: p.models
      .filter((m) => m.id)
      .sort((a, b) => a.id.localeCompare(b.id)),
  }))
  .sort(
    (a, b) =>
      a.name.localeCompare(b.name) || a.baseUrl.localeCompare(b.baseUrl),
  );

// Disambiguate ids that collapsed after slugging.
const seen = new Map<string, number>();
for (const p of presets) {
  const n = seen.get(p.id) ?? 0;
  seen.set(p.id, n + 1);
  if (n > 0) p.id = `${p.id}-${n + 1}`;
}

const withModels = presets.filter((p) => p.models.length > 0).length;
const modelCount = presets.reduce((n, p) => n + p.models.length, 0);
const byProtocol = presets.reduce<Record<string, number>>((acc, p) => {
  acc[p.protocol] = (acc[p.protocol] ?? 0) + 1;
  return acc;
}, {});

const here = path.dirname(fileURLToPath(import.meta.url));
const dest = path.join(here, "../src/config/deckProviderPresets.json");
writeFileSync(dest, `${JSON.stringify(presets, null, 2)}\n`);

console.log(`source entries   : ${considered}`);
console.log(`merged upstreams : ${presets.length}`);
console.log(`with models      : ${withModels}`);
console.log(`model resources  : ${modelCount}`);
console.log(`by protocol      :`, byProtocol);
console.log(`written          : ${dest}`);
