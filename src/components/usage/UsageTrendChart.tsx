import { Suspense, lazy } from "react";
import { Loader2 } from "lucide-react";
import type { UsageRangeSelection } from "@/types/usage";

export {
  buildUsageTrendChartData,
  createUsageTrendTokenTickFormatter,
  formatUsageTrendTickLabel,
  formatUsageTrendTokenTickLabel,
} from "./usageTrendChartData";
export type {
  UsageTrendChartPoint,
  UsageTrendStatLike,
} from "./usageTrendChartData";

export interface UsageTrendChartProps {
  range: UsageRangeSelection;
  rangeLabel: string;
  appType?: string;
  providerName?: string;
  model?: string;
  refreshIntervalMs: number;
}

/**
 * Recharts pulls in a large dependency graph, so the chart body lives in a
 * lazily loaded chunk. The usage dashboard is not part of the first screen.
 */
const UsageTrendChartView = lazy(() => import("./UsageTrendChart.view"));

function UsageTrendChartFallback() {
  return (
    <div className="flex h-[350px] items-center justify-center rounded-xl bg-card border border-border">
      <Loader2 className="size-8 animate-spin text-muted-foreground/30" />
    </div>
  );
}

export function UsageTrendChart(props: UsageTrendChartProps) {
  return (
    <Suspense fallback={<UsageTrendChartFallback />}>
      <UsageTrendChartView {...props} />
    </Suspense>
  );
}
