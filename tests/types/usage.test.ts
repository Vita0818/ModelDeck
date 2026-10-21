import { describe, expect, it } from "vitest";
import { getCacheWriteAvailability, getFreshInputTokens } from "@/types/usage";

describe("getCacheWriteAvailability", () => {
  it("distinguishes cache-write support across fixed protocols", () => {
    expect(getCacheWriteAvailability(["claude"])).toBe("ok");
    expect(getCacheWriteAvailability(["pi"])).toBe("partial");
    expect(getCacheWriteAvailability(["codex", "gemini"])).toBe("na");
    expect(getCacheWriteAvailability(["claude", "codex"])).toBe("partial");
    expect(getCacheWriteAvailability([])).toBe("ok");
  });
});

describe("getFreshInputTokens", () => {
  it.each(["codex", "gemini", "grokbuild"])(
    "honors row semantics for %s",
    (appType) => {
      const tokens = {
        appType,
        inputTokens: 1000,
        cacheReadTokens: 300,
        cacheCreationTokens: 200,
      };
      expect(getFreshInputTokens(tokens)).toBe(700);
      expect(getFreshInputTokens({ ...tokens, inputTokenSemantics: 0 })).toBe(
        700,
      );
      expect(getFreshInputTokens({ ...tokens, inputTokenSemantics: 1 })).toBe(
        500,
      );
      expect(getFreshInputTokens({ ...tokens, inputTokenSemantics: 2 })).toBe(
        1000,
      );
      expect(
        getFreshInputTokens({
          ...tokens,
          inputTokens: 100,
          inputTokenSemantics: 1,
        }),
      ).toBe(100);
    },
  );

  it("keeps Anthropic fresh input unchanged", () => {
    expect(
      getFreshInputTokens({
        appType: "claude",
        inputTokens: 1000,
        cacheReadTokens: 300,
        cacheCreationTokens: 200,
        inputTokenSemantics: 1,
      }),
    ).toBe(1000);
  });
});
