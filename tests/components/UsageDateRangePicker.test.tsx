import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { UsageDateRangePicker } from "@/components/usage/UsageDateRangePicker";
import type { UsageRangeSelection } from "@/types/usage";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

const ts = (day: number, hour = 0, minute = 0, second = 0) =>
  new Date(2026, 9, day, hour, minute, second).getTime() / 1000;

function setup(
  selection: UsageRangeSelection = {
    preset: "custom",
    customStartDate: ts(10),
    customEndDate: ts(12, 23, 59, 59),
  },
) {
  const onApply = vi.fn();
  const view = render(
    <UsageDateRangePicker
      selection={selection}
      triggerLabel="range"
      onApply={onApply}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "range" }));
  return { onApply, ...view };
}

afterEach(() => vi.useRealTimers());

describe("UsageDateRangePicker", () => {
  it.each(["common.cancel", "common.close"])(
    "dismisses with %s and discards edits",
    async (name) => {
      const { onApply } = setup();
      fireEvent.change(screen.getByLabelText("usage.startTime (usage.time)"), {
        target: { value: "09:30" },
      });
      fireEvent.click(screen.getByRole("button", { name }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(onApply).not.toHaveBeenCalled();
      fireEvent.click(screen.getByRole("button", { name: "range" }));
      expect(screen.getByLabelText("usage.startTime (usage.time)")).toHaveValue(
        "00:00",
      );
    },
  );

  it("dismisses on Escape, outside click, and trigger toggle", async () => {
    const user = userEvent.setup();
    const { onApply } = setup();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "range" }));
    await user.click(document.body);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "range" }));
    await user.click(screen.getByRole("button", { name: "range" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(onApply).not.toHaveBeenCalled();
  });

  it("selects complete calendar days and closes on confirmation", () => {
    const { onApply } = setup();
    fireEvent.click(screen.getByRole("button", { name: "10/5/2026" }));
    expect(
      screen.getByRole("button", { name: "usage.endTime" }),
    ).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "10/6/2026" }));
    fireEvent.click(screen.getByRole("button", { name: "common.confirm" }));
    expect(onApply).toHaveBeenCalledWith({
      preset: "custom",
      customStartDate: ts(5),
      customEndDate: ts(6, 23, 59, 59),
      liveEndTime: false,
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("keeps the start date when editing an earlier end date and explains the invalid order", () => {
    const { onApply } = setup();
    fireEvent.click(screen.getByRole("button", { name: "usage.endTime" }));
    fireEvent.click(screen.getByRole("button", { name: "10/5/2026" }));
    expect(screen.getByLabelText("usage.startTime (usage.date)")).toHaveValue(
      "2026-10-10",
    );
    expect(screen.getByLabelText("usage.endTime (usage.date)")).toHaveValue(
      "2026-10-05",
    );
    fireEvent.click(screen.getByRole("button", { name: "common.confirm" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "usage.invalidTimeRangeOrder",
    );
    expect(onApply).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("starts a new full day when the chosen start moves beyond the old end", () => {
    const { onApply } = setup();
    fireEvent.click(screen.getByRole("button", { name: "10/20/2026" }));
    fireEvent.click(screen.getByRole("button", { name: "common.confirm" }));
    expect(onApply).toHaveBeenCalledWith(
      expect.objectContaining({
        customStartDate: ts(20),
        customEndDate: ts(20, 23, 59, 59),
      }),
    );
  });

  it("allows empty input while editing, validates it, and includes the selected end minute", () => {
    const { onApply } = setup();
    const start = screen.getByLabelText("usage.startTime (usage.time)");
    fireEvent.change(start, { target: { value: "" } });
    expect(start).toHaveValue("");
    fireEvent.click(screen.getByRole("button", { name: "common.confirm" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "usage.invalidTimeRange",
    );
    expect(onApply).not.toHaveBeenCalled();
    fireEvent.change(start, { target: { value: "09:30" } });
    fireEvent.change(screen.getByLabelText("usage.endTime (usage.time)"), {
      target: { value: "09:30" },
    });
    fireEvent.change(screen.getByLabelText("usage.endTime (usage.date)"), {
      target: { value: "2026-10-10" },
    });
    fireEvent.click(screen.getByRole("button", { name: "common.confirm" }));
    expect(onApply).toHaveBeenCalledWith(
      expect.objectContaining({
        customStartDate: ts(10, 9, 30),
        customEndDate: ts(10, 9, 30, 59),
      }),
    );
  });

  it("resolves live end at confirmation and disables only the end inputs", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(2026, 9, 21, 12, 0, 10));
    const { onApply } = setup();
    fireEvent.click(screen.getByRole("checkbox"));
    expect(screen.getByLabelText("usage.endTime (usage.date)")).toBeDisabled();
    expect(screen.getByLabelText("usage.startTime (usage.date)")).toBeEnabled();
    vi.setSystemTime(new Date(2026, 9, 21, 12, 5, 40));
    fireEvent.click(screen.getByRole("button", { name: "common.confirm" }));
    expect(onApply).toHaveBeenCalledWith({
      preset: "custom",
      customStartDate: ts(10),
      customEndDate: undefined,
      liveEndTime: true,
    });
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
  });

  it("applies presets immediately and reopens a relative range using the current clock", () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(2026, 9, 21, 12, 0));
    const { onApply } = setup({ preset: "today" });
    fireEvent.click(screen.getByRole("button", { name: "usage.preset7d" }));
    expect(onApply).toHaveBeenCalledWith({ preset: "7d" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    vi.setSystemTime(new Date(2026, 9, 22, 12, 30));
    fireEvent.click(screen.getByRole("button", { name: "range" }));
    expect(screen.getByLabelText("usage.startTime (usage.date)")).toHaveValue(
      "2026-10-22",
    );
    expect(screen.getByLabelText("usage.endTime (usage.time)")).toHaveValue(
      "12:30",
    );
  });
});
