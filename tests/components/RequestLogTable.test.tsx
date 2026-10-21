import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RequestLogTable } from "@/components/usage/RequestLogTable";
import type { UsageRangeSelection } from "@/types/usage";

const useRequestLogsMock = vi.hoisted(() => vi.fn());

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (
      key: string,
      options?: {
        defaultValue?: string;
      },
    ) => options?.defaultValue ?? key,
    i18n: {
      resolvedLanguage: "en",
      language: "en",
    },
  }),
}));

vi.mock("@/lib/query/usage", () => ({
  useRequestLogs: (args: unknown) => useRequestLogsMock(args),
}));

vi.mock("@/components/ui/button", () => ({
  Button: ({ children, ...props }: any) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock("@/components/ui/input", () => ({
  Input: (props: any) => <input {...props} />,
}));

vi.mock("@/components/ui/select", () => ({
  Select: ({ children }: any) => <div>{children}</div>,
  SelectTrigger: ({ children, ...props }: any) => (
    <button type="button" {...props}>
      {children}
    </button>
  ),
  SelectValue: ({ placeholder }: any) => <span>{placeholder ?? null}</span>,
  SelectContent: () => null,
  SelectItem: () => null,
}));

vi.mock("@/components/ui/table", () => ({
  Table: ({ children }: any) => <table>{children}</table>,
  TableBody: ({ children }: any) => <tbody>{children}</tbody>,
  TableCell: ({ children, ...props }: any) => <td {...props}>{children}</td>,
  TableHead: ({ children, ...props }: any) => <th {...props}>{children}</th>,
  TableHeader: ({ children }: any) => <thead>{children}</thead>,
  TableRow: ({ children }: any) => <tr>{children}</tr>,
}));

describe("RequestLogTable", () => {
  beforeEach(() => {
    useRequestLogsMock.mockReset();
    useRequestLogsMock.mockImplementation(
      ({ page = 0, pageSize = 20 }: { page?: number; pageSize?: number }) => ({
        data: {
          data: [],
          total: 120,
          page,
          pageSize,
        },
        isLoading: false,
      }),
    );
  });

  it("resets pagination when the dashboard range changes", async () => {
    const initialRange: UsageRangeSelection = { preset: "today" };
    const nextRange: UsageRangeSelection = {
      preset: "custom",
      customStartDate: 1_710_000_000,
      customEndDate: 1_710_086_400,
    };

    const { rerender } = render(
      <RequestLogTable
        range={initialRange}
        rangeLabel="Today"
        appType="all"
        refreshIntervalMs={0}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "2" }));

    await waitFor(() => {
      expect(useRequestLogsMock).toHaveBeenLastCalledWith(
        expect.objectContaining({
          page: 1,
          range: initialRange,
        }),
      );
    });

    rerender(
      <RequestLogTable
        range={nextRange}
        rangeLabel="Custom"
        appType="all"
        refreshIntervalMs={0}
      />,
    );

    await waitFor(() => {
      expect(useRequestLogsMock).toHaveBeenLastCalledWith(
        expect.objectContaining({
          page: 0,
          range: nextRange,
        }),
      );
    });
  });

  it("resets pagination when the dashboard app filter changes", async () => {
    const range: UsageRangeSelection = { preset: "today" };
    const { rerender } = render(
      <RequestLogTable
        range={range}
        rangeLabel="Today"
        appType="all"
        refreshIntervalMs={0}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "2" }));

    await waitFor(() => {
      expect(useRequestLogsMock).toHaveBeenLastCalledWith(
        expect.objectContaining({
          page: 1,
          range,
        }),
      );
    });

    rerender(
      <RequestLogTable
        range={range}
        rangeLabel="Today"
        appType="claude"
        refreshIntervalMs={0}
      />,
    );

    await waitFor(() => {
      expect(useRequestLogsMock).toHaveBeenLastCalledWith(
        expect.objectContaining({
          page: 0,
          range,
        }),
      );
    });
  });

  it("resets pagination when the end changes between fixed and live", async () => {
    const range: UsageRangeSelection = {
      preset: "custom",
      customStartDate: 100,
      customEndDate: 200,
      liveEndTime: false,
    };
    const { rerender } = render(
      <RequestLogTable
        range={range}
        rangeLabel="Custom"
        refreshIntervalMs={0}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "2" }));
    rerender(
      <RequestLogTable
        range={{ ...range, liveEndTime: true }}
        rangeLabel="Live"
        refreshIntervalMs={0}
      />,
    );
    await waitFor(() =>
      expect(useRequestLogsMock).toHaveBeenLastCalledWith(
        expect.objectContaining({ page: 0 }),
      ),
    );
  });

  it("recovers when a refreshed result has fewer pages", async () => {
    const { rerender } = render(
      <RequestLogTable
        range={{ preset: "today" }}
        rangeLabel="Today"
        refreshIntervalMs={0}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "6" }));
    useRequestLogsMock.mockReturnValue({
      data: { data: [], total: 21, page: 5, pageSize: 20 },
      isLoading: false,
    });
    rerender(
      <RequestLogTable
        range={{ preset: "today" }}
        rangeLabel="Today"
        refreshIntervalMs={0}
      />,
    );
    await waitFor(() =>
      expect(useRequestLogsMock).toHaveBeenLastCalledWith(
        expect.objectContaining({ page: 1 }),
      ),
    );
  });

  it("shows a retryable error instead of presenting failed queries as no data", () => {
    const refetch = vi.fn();
    useRequestLogsMock.mockReturnValue({
      isLoading: false,
      isError: true,
      refetch,
    });
    render(
      <RequestLogTable
        range={{ preset: "today" }}
        rangeLabel="Today"
        refreshIntervalMs={0}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("usage.loadError");
    expect(screen.queryByText("usage.noData")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "usage.retry" }));
    expect(refetch).toHaveBeenCalledOnce();
  });

  it.each([
    [0, "700"],
    [1, "500"],
    [2, "1,000"],
  ])(
    "renders fresh input using storage semantics %s",
    (inputTokenSemantics, expected) => {
      useRequestLogsMock.mockReturnValue({
        data: {
          total: 1,
          page: 0,
          pageSize: 20,
          data: [
            {
              requestId: "fixture",
              appType: "codex",
              providerName: "Fixture",
              model: "fixture-model",
              inputTokens: 1000,
              inputTokenSemantics,
              outputTokens: 100,
              cacheReadTokens: 300,
              cacheCreationTokens: 200,
              totalCostUsd: "0.01",
              latencyMs: 1000,
              statusCode: 200,
              costMultiplier: "1",
              createdAt: 1000,
            },
          ],
        },
        isLoading: false,
      });
      render(
        <RequestLogTable
          range={{ preset: "today" }}
          rangeLabel="Today"
          refreshIntervalMs={0}
        />,
      );
      expect(screen.getByText(expected, { exact: true })).toBeInTheDocument();
    },
  );
});
