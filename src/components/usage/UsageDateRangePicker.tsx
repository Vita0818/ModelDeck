import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  CalendarDays,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { getUsageRangePresetLabel, resolveUsageRange } from "@/lib/usageRange";
import { getLocaleFromLanguage } from "./format";
import type { UsageRangePreset, UsageRangeSelection } from "@/types/usage";

type DraftField = "start" | "end";
type DraftDateTime = { date: string; time: string };

const PRESETS: UsageRangePreset[] = ["today", "1d", "7d", "14d", "30d"];

interface UsageDateRangePickerProps {
  selection: UsageRangeSelection;
  onApply: (selection: UsageRangeSelection) => void;
  triggerLabel: string;
}

/* ── helpers ── */

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

function toTs(d: Date): number {
  return Math.floor(d.getTime() / 1000);
}

function fromTs(ts: number): Date {
  return new Date(ts * 1000);
}

function fmtDate(ts: number): string {
  const d = fromTs(ts);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function fmtTime(ts: number): string {
  const d = fromTs(ts);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

function toDraft(ts: number): DraftDateTime {
  return { date: fmtDate(ts), time: fmtTime(ts) };
}

// Keep incomplete native-input values as text until confirmation. Converting
// each keystroke into a Date makes clearing/editing a field jump back to its
// previous value. Fixed end times include the entire selected minute.
function parseDraft(value: DraftDateTime, end = false): number | null {
  if (
    !/^\d{4}-\d{2}-\d{2}$/.test(value.date) ||
    !/^\d{2}:\d{2}$/.test(value.time)
  ) {
    return null;
  }
  const [year, month, day] = value.date.split("-").map(Number);
  const [hour, minute] = value.time.split(":").map(Number);
  const date = new Date(year, month - 1, day, hour, minute, end ? 59 : 0);
  if (
    date.getFullYear() !== year ||
    date.getMonth() !== month - 1 ||
    date.getDate() !== day ||
    date.getHours() !== hour ||
    date.getMinutes() !== minute
  )
    return null;
  return toTs(date);
}

function getCalendarDays(month: Date): Date[] {
  const first = new Date(month.getFullYear(), month.getMonth(), 1);
  const gridStart = new Date(first);
  gridStart.setDate(first.getDate() - first.getDay());
  return Array.from({ length: 42 }, (_, i) => {
    const d = new Date(gridStart);
    d.setDate(gridStart.getDate() + i);
    return d;
  });
}

/* ── component ── */

export function UsageDateRangePicker({
  selection,
  onApply,
  triggerLabel,
}: UsageDateRangePickerProps) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const [activeField, setActiveField] = useState<DraftField>("start");
  const resolvedRange = useMemo(
    () => resolveUsageRange(selection),
    [selection],
  );
  const [draftStart, setDraftStart] = useState(() =>
    toDraft(resolvedRange.startDate),
  );
  const [draftEnd, setDraftEnd] = useState(() =>
    toDraft(resolvedRange.endDate),
  );
  const [draftLiveEnd, setDraftLiveEnd] = useState(
    selection.preset === "custom" ? (selection.liveEndTime ?? false) : false,
  );
  const [displayMonth, setDisplayMonth] = useState(
    () =>
      new Date(
        fromTs(resolvedRange.startDate).getFullYear(),
        fromTs(resolvedRange.startDate).getMonth(),
        1,
      ),
  );
  const [error, setError] = useState<string | null>(null);

  const language = i18n.resolvedLanguage || i18n.language || "en";
  const locale = getLocaleFromLanguage(language);

  // Reset draft when popover opens
  useEffect(() => {
    if (!open) return;
    const r = resolveUsageRange(selection);
    setDraftStart(toDraft(r.startDate));
    setDraftEnd(toDraft(r.endDate));
    setDraftLiveEnd(
      selection.preset === "custom" ? (selection.liveEndTime ?? false) : false,
    );
    setDisplayMonth(
      new Date(
        fromTs(r.startDate).getFullYear(),
        fromTs(r.startDate).getMonth(),
        1,
      ),
    );
    setActiveField("start");
    setError(null);
  }, [open, selection]);

  // Keep draftEnd ticking when live mode is active and popover is open
  useEffect(() => {
    if (!open || !draftLiveEnd) return;
    const tick = () => setDraftEnd(toDraft(Math.floor(Date.now() / 1000)));
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [open, draftLiveEnd]);

  const calendarDays = useMemo(
    () => getCalendarDays(displayMonth),
    [displayMonth],
  );

  const weekdayLabels = useMemo(
    () =>
      Array.from({ length: 7 }, (_, i) =>
        new Intl.DateTimeFormat(locale, { weekday: "narrow" }).format(
          new Date(2024, 0, 7 + i),
        ),
      ),
    [locale],
  );

  const startTs = parseDraft(draftStart);
  const endTs = parseDraft(draftEnd, true);
  const startDay = startTs == null ? null : fromTs(startTs);
  const endDay = endTs == null ? null : fromTs(endTs);
  const today = new Date();

  /* Pick a date from the calendar */
  const handleDatePick = (day: Date) => {
    setError(null);

    const date = fmtDate(toTs(day));
    if (activeField === "start" || draftLiveEnd) {
      setDraftStart({ date, time: "00:00" });
      if (!draftLiveEnd) {
        if (endTs == null || toTs(startOfDay(day)) > endTs) {
          setDraftEnd({ date, time: "23:59" });
        }
        setActiveField("end");
      }
    } else {
      // An end-date edit must never silently overwrite the start date.
      setDraftEnd({ date, time: "23:59" });
    }

    // Navigate calendar if the day is outside the displayed month
    if (
      day.getMonth() !== displayMonth.getMonth() ||
      day.getFullYear() !== displayMonth.getFullYear()
    ) {
      setDisplayMonth(new Date(day.getFullYear(), day.getMonth(), 1));
    }
  };

  const handleApply = () => {
    setError(null);
    const start = parseDraft(draftStart);
    const end = draftLiveEnd
      ? Math.floor(Date.now() / 1000)
      : parseDraft(draftEnd, true);
    if (start == null || end == null) {
      setError(t("usage.invalidTimeRange"));
      return;
    }
    if (start > end) {
      setError(t("usage.invalidTimeRangeOrder", "开始时间不能晚于结束时间"));
      return;
    }
    onApply({
      preset: "custom",
      customStartDate: start,
      customEndDate: draftLiveEnd ? undefined : end,
      liveEndTime: draftLiveEnd,
    });
    setOpen(false);
  };

  const goToToday = () => {
    setDisplayMonth(new Date(today.getFullYear(), today.getMonth(), 1));
  };

  /* ── Field card (start / end) ── */
  const renderField = (field: DraftField) => {
    const isActive = activeField === field;
    const isEndLive = field === "end" && draftLiveEnd;
    const value = field === "start" ? draftStart : draftEnd;
    const setValue = field === "start" ? setDraftStart : setDraftEnd;
    const label =
      field === "start"
        ? t("usage.startTime", "开始时间")
        : t("usage.endTime", "结束时间");

    return (
      <div
        className={cn(
          "rounded-lg border px-3 py-2 transition-all",
          isEndLive
            ? "border-border bg-secondary cursor-not-allowed opacity-50"
            : isActive
              ? "border-primary ring-1 ring-primary/30 bg-primary/5 cursor-pointer"
              : "border-border hover:border-border cursor-pointer",
        )}
        onFocusCapture={() => {
          if (!isEndLive) setActiveField(field);
        }}
      >
        <button
          type="button"
          disabled={isEndLive}
          aria-pressed={isActive && !isEndLive}
          onClick={() => setActiveField(field)}
          className="mb-1.5 block w-full text-left text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
        >
          {label}
        </button>
        <div className="flex items-center gap-1.5">
          <Input
            type="date"
            className={cn(
              "h-7 flex-1 border-0 bg-transparent p-0 text-sm shadow-none focus-visible:ring-0",
              isEndLive && "pointer-events-none",
            )}
            aria-label={`${label} (${t("usage.date")})`}
            value={value.date}
            onChange={(e) => {
              if (isEndLive) return;
              const date = e.target.value;
              setValue((previous) => ({ ...previous, date }));
              const parsed = parseDraft({ ...value, date });
              if (parsed != null) {
                const d = fromTs(parsed);
                setDisplayMonth(new Date(d.getFullYear(), d.getMonth(), 1));
              }
              setError(null);
            }}
            onFocus={() => {
              if (!isEndLive) setActiveField(field);
            }}
            disabled={isEndLive}
          />
          <Input
            type="time"
            step={60}
            className={cn(
              "h-7 w-[90px] flex-none border-0 bg-transparent p-0 text-sm shadow-none focus-visible:ring-0",
              isEndLive && "pointer-events-none",
            )}
            aria-label={`${label} (${t("usage.time")})`}
            value={value.time}
            onChange={(e) => {
              if (isEndLive) return;
              const time = e.target.value;
              setValue((previous) => ({ ...previous, time }));
              setError(null);
            }}
            onFocus={() => {
              if (!isEndLive) setActiveField(field);
            }}
            disabled={isEndLive}
          />
        </div>
      </div>
    );
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant={selection.preset === "custom" ? "default" : "outline"}
          className="h-9 w-[100px] justify-start gap-1.5 text-xs"
          title={triggerLabel}
        >
          <CalendarDays className="size-4 shrink-0" />
          <span className="truncate flex-1">{triggerLabel}</span>
          <ChevronDown className="size-3.5 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      {/* Dismissal removes the portal immediately, without waiting for a CSS
          exit-animation event from the webview. */}
      {open && (
        <PopoverContent
          className="usage-range-popover w-[620px] max-w-[calc(100vw-2rem)] max-h-[calc(100dvh-2rem)] overflow-y-auto p-3"
          align="end"
          aria-label={t("usage.customRange")}
        >
          {/* Preset shortcuts */}
          <div className="flex flex-wrap gap-1.5 pb-2 border-b border-border">
            {PRESETS.map((preset) => (
              <Button
                key={preset}
                type="button"
                size="sm"
                variant={selection.preset === preset ? "default" : "outline"}
                className="h-7 px-2.5 text-xs"
                onClick={() => {
                  onApply({ preset });
                  setOpen(false);
                }}
              >
                {getUsageRangePresetLabel(preset, t)}
              </Button>
            ))}
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="ml-auto size-7"
              aria-label={t("common.close")}
              onClick={() => setOpen(false)}
            >
              <X className="size-4" />
            </Button>
          </div>

          <div className="usage-range-layout flex flex-col gap-3">
            {/* Left: date fields */}
            <div className="usage-range-fields space-y-2">
              <p className="text-xs text-muted-foreground">
                {t("usage.calendarRangeHint")}
              </p>
              {renderField("start")}
              {renderField("end")}

              <label className="flex items-center gap-2 cursor-pointer select-none">
                <Checkbox
                  checked={draftLiveEnd}
                  onCheckedChange={(checked) => {
                    const live = checked === true;
                    setDraftLiveEnd(live);
                    if (live) {
                      setDraftEnd(toDraft(Math.floor(Date.now() / 1000)));
                      setActiveField("start");
                    }
                  }}
                />
                <span className="text-xs text-muted-foreground">
                  {t("usage.liveEndTime", "结束时间跟随当前时刻")}
                </span>
              </label>

              {error && (
                <p role="alert" className="text-xs text-destructive">
                  {error}
                </p>
              )}

              <div className="flex gap-2 pt-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="flex-1"
                  onClick={() => setOpen(false)}
                >
                  {t("common.cancel")}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  className="flex-1"
                  onClick={handleApply}
                >
                  {t("common.confirm")}
                </Button>
              </div>
            </div>

            {/* Right: calendar */}
            <div className="usage-range-calendar rounded-lg border border-border bg-secondary p-2.5">
              {/* Month navigation */}
              <div className="flex items-center justify-between mb-1.5">
                <Button
                  type="button"
                  size="icon"
                  variant="ghost"
                  className="size-7"
                  aria-label={t("usage.previousMonth")}
                  onClick={() =>
                    setDisplayMonth(
                      new Date(
                        displayMonth.getFullYear(),
                        displayMonth.getMonth() - 1,
                        1,
                      ),
                    )
                  }
                >
                  <ChevronLeft className="size-3.5" />
                </Button>
                <button
                  type="button"
                  className="text-sm font-medium hover:text-primary transition-colors"
                  onClick={goToToday}
                  title={t("usage.presetToday", { defaultValue: "当天" })}
                >
                  {displayMonth.toLocaleDateString(locale, {
                    year: "numeric",
                    month: "long",
                  })}
                </button>
                <Button
                  type="button"
                  size="icon"
                  variant="ghost"
                  className="size-7"
                  aria-label={t("usage.nextMonth")}
                  onClick={() =>
                    setDisplayMonth(
                      new Date(
                        displayMonth.getFullYear(),
                        displayMonth.getMonth() + 1,
                        1,
                      ),
                    )
                  }
                >
                  <ChevronRight className="size-3.5" />
                </Button>
              </div>

              {/* Weekday headers */}
              <div className="grid grid-cols-7 text-center text-[11px] text-muted-foreground mb-0.5">
                {weekdayLabels.map((label, i) => (
                  <div key={i} className="py-0.5">
                    {label}
                  </div>
                ))}
              </div>

              {/* Day grid */}
              <div className="grid grid-cols-7 gap-px">
                {calendarDays.map((day) => {
                  const isCurrentMonth =
                    day.getMonth() === displayMonth.getMonth();
                  const isToday = isSameDay(day, today);
                  const isStart = startDay != null && isSameDay(day, startDay);
                  const isEnd = endDay != null && isSameDay(day, endDay);
                  const dayStart = startOfDay(day);
                  const inRange =
                    startDay != null &&
                    endDay != null &&
                    dayStart >= startOfDay(startDay) &&
                    dayStart <= startOfDay(endDay);
                  const isEndpoint = isStart || isEnd;

                  return (
                    <button
                      key={day.toISOString()}
                      type="button"
                      aria-label={day.toLocaleDateString(locale)}
                      aria-current={isToday ? "date" : undefined}
                      aria-pressed={isEndpoint}
                      className={cn(
                        "relative h-7 rounded text-xs transition-colors",
                        !isCurrentMonth && "text-muted-foreground/30",
                        isCurrentMonth && !inRange && "hover:bg-muted",
                        inRange && !isEndpoint && "bg-primary/10 text-primary",
                        isEndpoint &&
                          "bg-primary text-primary-foreground font-medium",
                        isToday && !isEndpoint && "ring-1 ring-primary/40",
                      )}
                      onClick={() => handleDatePick(day)}
                    >
                      {day.getDate()}
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
        </PopoverContent>
      )}
    </Popover>
  );
}
