/**
 * Chart series colours.
 *
 * Recharts needs literal colours rather than CSS variables, so the Apple
 * system palette is declared once here instead of being spread across the
 * chart markup. These hues hold their contrast on both the light and dark
 * canvas, which is why the same value is used for both themes.
 */
export const CHART_COLORS = {
  input: "#0A84FF", // systemBlue
  output: "#34C759", // systemGreen
  cacheCreation: "#FF9500", // systemOrange
  cacheRead: "#AF52DE", // systemPurple
  cost: "#FF2D55", // systemPink
} as const;

export type ChartSeries = keyof typeof CHART_COLORS;
