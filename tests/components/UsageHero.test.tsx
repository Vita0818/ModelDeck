import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UsageHero } from "@/components/usage/UsageHero";

const summaryQuery = vi.hoisted(() => vi.fn());
vi.mock("@/lib/query/usage", () => ({
  useUsageSummaryByApp: (...args: unknown[]) => summaryQuery(...args),
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

describe("UsageHero", () => {
  beforeEach(() => summaryQuery.mockReset());

  it("shows reported cache writes even for a Codex session", () => {
    summaryQuery.mockReturnValue({
      data: [
        {
          appType: "codex",
          summary: {
            totalRequests: 1,
            totalCost: "0.01",
            totalInputTokens: 700,
            totalOutputTokens: 100,
            totalCacheReadTokens: 300,
            totalCacheCreationTokens: 123,
            realTotalTokens: 1223,
            cacheHitRate: 300 / 1123,
            successRate: 100,
          },
        },
      ],
      isLoading: false,
    });
    render(
      <UsageHero
        range={{ preset: "today" }}
        appType="codex"
        refreshIntervalMs={0}
      />,
    );
    expect(screen.getByText("123", { exact: true })).toBeInTheDocument();
    expect(screen.queryByText("N/A")).not.toBeInTheDocument();
    expect(screen.getByTitle("1,223")).toBeInTheDocument();
  });

  it("does not display a zero total when the summary request failed", () => {
    const refetch = vi.fn();
    summaryQuery.mockReturnValue({ isLoading: false, isError: true, refetch });
    render(<UsageHero range={{ preset: "today" }} refreshIntervalMs={0} />);
    expect(screen.getByRole("alert")).toHaveTextContent("usage.loadError");
    expect(screen.queryByText("usage.realTotal")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "usage.retry" }));
    expect(refetch).toHaveBeenCalledOnce();
  });
});
