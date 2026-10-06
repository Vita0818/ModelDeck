import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from "recharts";
import { useUsageTrends } from "@/lib/query/usage";
import { Loader2 } from "lucide-react";
import { fmtInt, fmtUsd, getLocaleFromLanguage } from "./format";
import { resolveUsageRange } from "@/lib/usageRange";
import { CHART_COLORS } from "./chartPalette";
import type { UsageTrendChartProps } from "./UsageTrendChart";
import {
  buildUsageTrendChartData,
  createUsageTrendTokenTickFormatter,
  formatUsageTrendTickLabel,
  formatUsageTrendTokenTickLabel,
  type UsageTrendChartPoint,
} from "./usageTrendChartData";

export default function UsageTrendChartView({
  range,
  rangeLabel,
  appType,
  providerName,
  model,
  refreshIntervalMs,
}: UsageTrendChartProps) {
  const { t, i18n } = useTranslation();
  const { startDate, endDate } = resolveUsageRange(range);
  const { data: trends, isLoading } = useUsageTrends(
    range,
    { appType, providerName, model },
    {
      refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
    },
  );

  const durationSeconds = Math.max(endDate - startDate, 0);
  const isHourly = durationSeconds <= 24 * 60 * 60;
  const language = i18n.resolvedLanguage || i18n.language || "en";
  const dateLocale = getLocaleFromLanguage(language);
  const tokenTickFormatter = useMemo(
    () => createUsageTrendTokenTickFormatter(dateLocale),
    [dateLocale],
  );

  const chartData = useMemo(
    () =>
      buildUsageTrendChartData(trends, {
        isHourly,
        dateLocale,
        startDate,
        endDate,
      }),
    [trends, isHourly, dateLocale, startDate, endDate],
  );

  if (isLoading) {
    return (
      <div className="flex h-[350px] items-center justify-center rounded-xl bg-card border border-border">
        <Loader2 className="size-8 animate-spin text-muted-foreground/30" />
      </div>
    );
  }

  const CustomTooltip = ({ active, payload }: any) => {
    if (active && payload && payload.length) {
      const point = payload[0]?.payload as UsageTrendChartPoint | undefined;
      const heading = point?.tooltipLabel ?? point?.label ?? "";
      return (
        <div className="rounded-lg border bg-background/95 p-3 shadow-lg">
          <p className="mb-2 font-medium">{heading}</p>
          {payload.map((entry: any, index: number) => (
            <div
              key={index}
              className="flex items-center gap-2 text-sm"
              style={{ color: entry.color }}
            >
              <div
                className="size-2 rounded-full"
                style={{ backgroundColor: entry.color }}
              />
              <span className="font-medium">{entry.name}:</span>
              <span>
                {entry.dataKey === "cost"
                  ? fmtUsd(entry.value, 6)
                  : fmtInt(entry.value, dateLocale)}
              </span>
            </div>
          ))}
        </div>
      );
    }
    return null;
  };

  return (
    <div className="rounded-xl border border-border bg-card p-6 shadow-xs">
      <div className="mb-6 flex items-center justify-between">
        <h3 className="text-lg font-semibold">
          {t("usage.trends", "使用趋势")}
        </h3>
        <p className="text-sm text-muted-foreground">{rangeLabel}</p>
      </div>

      <div className="h-[350px] w-full">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart
            data={chartData}
            margin={{ top: 10, right: 10, left: 0, bottom: 0 }}
          >
            <defs>
              <linearGradient id="colorInput" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor={CHART_COLORS.input}
                  stopOpacity={0.2}
                />
                <stop
                  offset="95%"
                  stopColor={CHART_COLORS.input}
                  stopOpacity={0}
                />
              </linearGradient>
              <linearGradient id="colorOutput" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor={CHART_COLORS.output}
                  stopOpacity={0.2}
                />
                <stop
                  offset="95%"
                  stopColor={CHART_COLORS.output}
                  stopOpacity={0}
                />
              </linearGradient>
              <linearGradient
                id="colorCacheCreation"
                x1="0"
                y1="0"
                x2="0"
                y2="1"
              >
                <stop
                  offset="5%"
                  stopColor={CHART_COLORS.cacheCreation}
                  stopOpacity={0.2}
                />
                <stop
                  offset="95%"
                  stopColor={CHART_COLORS.cacheCreation}
                  stopOpacity={0}
                />
              </linearGradient>
              <linearGradient id="colorCacheRead" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor={CHART_COLORS.cacheRead}
                  stopOpacity={0.2}
                />
                <stop
                  offset="95%"
                  stopColor={CHART_COLORS.cacheRead}
                  stopOpacity={0}
                />
              </linearGradient>
            </defs>
            <CartesianGrid
              strokeDasharray="3 3"
              vertical={false}
              stroke="hsl(var(--border))"
              opacity={0.4}
            />
            <XAxis
              dataKey="xKey"
              axisLine={false}
              tickLine={false}
              tick={{ fill: "hsl(var(--muted-foreground))", fontSize: 12 }}
              dy={10}
              tickFormatter={(value) =>
                formatUsageTrendTickLabel(String(value), chartData)
              }
              allowDuplicatedCategory={false}
            />
            <YAxis
              yAxisId="tokens"
              width={72}
              axisLine={false}
              tickLine={false}
              tickMargin={8}
              tick={{ fill: "hsl(var(--muted-foreground))", fontSize: 12 }}
              tickFormatter={(value) =>
                formatUsageTrendTokenTickLabel(value, tokenTickFormatter)
              }
            />
            <YAxis
              yAxisId="cost"
              orientation="right"
              width={56}
              axisLine={false}
              tickLine={false}
              tickMargin={8}
              tick={{ fill: "hsl(var(--muted-foreground))", fontSize: 12 }}
              tickFormatter={(value) => `$${value}`}
            />
            <Tooltip content={<CustomTooltip />} />
            <Legend />
            <Area
              yAxisId="tokens"
              type="monotone"
              dataKey="inputTokens"
              name={t("usage.inputTokens", "输入 Tokens")}
              stroke={CHART_COLORS.input}
              fillOpacity={1}
              fill="url(#colorInput)"
              strokeWidth={2}
            />
            <Area
              yAxisId="tokens"
              type="monotone"
              dataKey="outputTokens"
              name={t("usage.outputTokens", "输出 Tokens")}
              stroke={CHART_COLORS.output}
              fillOpacity={1}
              fill="url(#colorOutput)"
              strokeWidth={2}
            />
            <Area
              yAxisId="tokens"
              type="monotone"
              dataKey="cacheCreationTokens"
              name={t("usage.cacheCreationTokens", "缓存创建")}
              stroke={CHART_COLORS.cacheCreation}
              fillOpacity={1}
              fill="url(#colorCacheCreation)"
              strokeWidth={2}
            />
            <Area
              yAxisId="tokens"
              type="monotone"
              dataKey="cacheReadTokens"
              name={t("usage.cacheReadTokens", "缓存命中")}
              stroke={CHART_COLORS.cacheRead}
              fillOpacity={1}
              fill="url(#colorCacheRead)"
              strokeWidth={2}
            />
            <Area
              yAxisId="cost"
              type="monotone"
              dataKey="cost"
              name={t("usage.cost", "成本")}
              stroke={CHART_COLORS.cost}
              fill="none"
              strokeWidth={2}
              strokeDasharray="4 4"
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
