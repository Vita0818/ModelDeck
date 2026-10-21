import { describe, expect, it } from "vitest";
import { resolveUsageRange } from "@/lib/usageRange";

describe("resolveUsageRange", () => {
  it("counts local calendar days across daylight saving changes", () => {
    // Also run with TZ=America/New_York to cover the 23-hour spring day.
    const now = new Date(2024, 2, 11, 0, 15).getTime();
    const range = resolveUsageRange({ preset: "7d" }, now);
    expect(new Date(range.startDate * 1000)).toEqual(new Date(2024, 2, 5));
    expect(range.endDate).toBe(Math.floor(now / 1000));
  });

  it("keeps fixed ranges stable and resolves live ends from the current clock", () => {
    const selection = {
      preset: "custom" as const,
      customStartDate: 100,
      customEndDate: 200,
    };
    expect(resolveUsageRange(selection, 300_000)).toEqual({
      startDate: 100,
      endDate: 200,
    });
    expect(
      resolveUsageRange({ ...selection, liveEndTime: true }, 300_000),
    ).toEqual({ startDate: 100, endDate: 300 });
    expect(
      resolveUsageRange({ ...selection, liveEndTime: true }, 400_000),
    ).toEqual({ startDate: 100, endDate: 400 });
  });
});
