const plugin = require("tailwindcss/plugin");

/**
 * Apple-like design system.
 *
 * Colour, radius and elevation all resolve from the CSS variables in
 * index.css so light/dark stay in one place. The palette below is Apple's
 * system colour set, used for the few semantic accents (success, warning,
 * destructive) that tokens alone cannot express.
 */

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: ["selector", ".dark"],
  theme: {
    extend: {
      colors: {
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        /** Window chrome behind the content area. */
        canvas: "hsl(var(--canvas))",
        sidebar: {
          DEFAULT: "hsl(var(--sidebar))",
          foreground: "hsl(var(--sidebar-foreground))",
          accent: "hsl(var(--sidebar-accent))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        success: {
          DEFAULT: "hsl(var(--success))",
          foreground: "hsl(var(--success-foreground))",
        },
        warning: {
          DEFAULT: "hsl(var(--warning))",
          foreground: "hsl(var(--warning-foreground))",
        },
        border: "hsl(var(--border))",
        /** Hairline used inside grouped rows, lighter than --border. */
        separator: "hsl(var(--separator))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",

        // Apple system colours, for charts and status accents.
        system: {
          blue: "#0A84FF",
          green: "#34C759",
          indigo: "#5E5CE6",
          orange: "#FF9500",
          pink: "#FF2D55",
          purple: "#AF52DE",
          red: "#FF3B30",
          teal: "#5AC8FA",
          yellow: "#FFCC00",
        },
        gray: {
          50: "#FAFAFA",
          100: "#F2F2F7",
          200: "#E5E5EA",
          300: "#D1D1D6",
          400: "#C7C7CC",
          500: "#AEAEB2",
          600: "#8E8E93",
          700: "#636366",
          800: "#48484A",
          900: "#3A3A3C",
          950: "#1C1C1E",
        },
      },
      borderRadius: {
        sm: "calc(var(--radius) - 4px)",
        md: "calc(var(--radius) - 2px)",
        lg: "var(--radius)",
        xl: "calc(var(--radius) + 4px)",
        "2xl": "calc(var(--radius) + 8px)",
      },
      boxShadow: {
        // Apple elevation is a tight contact shadow plus a soft ambient one.
        xs: "0 1px 1px 0 rgb(0 0 0 / 0.04)",
        sm: "0 1px 2px 0 rgb(0 0 0 / 0.05), 0 1px 1px -1px rgb(0 0 0 / 0.04)",
        md: "0 2px 6px -1px rgb(0 0 0 / 0.08), 0 1px 2px -1px rgb(0 0 0 / 0.04)",
        lg: "0 8px 24px -4px rgb(0 0 0 / 0.12), 0 2px 6px -2px rgb(0 0 0 / 0.06)",
        xl: "0 16px 48px -8px rgb(0 0 0 / 0.18), 0 4px 12px -4px rgb(0 0 0 / 0.08)",
        focus: "0 0 0 3.5px hsl(var(--ring) / 0.35)",
      },
      fontFamily: {
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          "SF Pro Text",
          "Inter",
          '"Segoe UI"',
          "Roboto",
          '"Helvetica Neue"',
          "Arial",
          '"PingFang SC"',
          '"Hiragino Sans GB"',
          '"Microsoft YaHei"',
          "sans-serif",
        ],
        mono: [
          "ui-monospace",
          "SFMono-Regular",
          '"SF Mono"',
          "Menlo",
          "Consolas",
          '"Liberation Mono"',
          "monospace",
        ],
      },
      fontSize: {
        // Apple's text styles, trimmed to what this app actually needs.
        "2xs": ["0.6875rem", { lineHeight: "0.875rem", letterSpacing: "0" }],
        xs: ["0.75rem", { lineHeight: "1rem", letterSpacing: "0" }],
        sm: ["0.8125rem", { lineHeight: "1.125rem", letterSpacing: "-0.003em" }],
        base: ["0.875rem", { lineHeight: "1.25rem", letterSpacing: "-0.006em" }],
        lg: ["1rem", { lineHeight: "1.375rem", letterSpacing: "-0.011em" }],
        xl: ["1.1875rem", { lineHeight: "1.5rem", letterSpacing: "-0.014em" }],
        "2xl": ["1.5rem", { lineHeight: "1.875rem", letterSpacing: "-0.019em" }],
        "3xl": ["2rem", { lineHeight: "2.375rem", letterSpacing: "-0.022em" }],
      },
      transitionTimingFunction: {
        // Apple's standard ease and a gentle overshoot for entrances.
        apple: "cubic-bezier(0.25, 0.1, 0.25, 1)",
        spring: "cubic-bezier(0.22, 1, 0.36, 1)",
      },
      keyframes: {
        "fade-in": { from: { opacity: "0" }, to: { opacity: "1" } },
        "fade-out": { from: { opacity: "1" }, to: { opacity: "0" } },
        "zoom-in": {
          from: { opacity: "0", transform: "scale(0.96)" },
          to: { opacity: "1", transform: "scale(1)" },
        },
        "zoom-out": {
          from: { opacity: "1", transform: "scale(1)" },
          to: { opacity: "0", transform: "scale(0.96)" },
        },
        "slide-up": {
          from: { opacity: "0", transform: "translateY(6px)" },
          to: { opacity: "1", transform: "translateY(0)" },
        },
        "slide-down": {
          from: { opacity: "0", transform: "translateY(-6px)" },
          to: { opacity: "1", transform: "translateY(0)" },
        },
        "sheet-in": {
          from: { opacity: "0", transform: "translateY(12px) scale(0.98)" },
          to: { opacity: "1", transform: "translateY(0) scale(1)" },
        },
        "sheet-out": {
          from: { opacity: "1", transform: "translateY(0) scale(1)" },
          to: { opacity: "0", transform: "translateY(8px) scale(0.99)" },
        },
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
      },
      animation: {
        "fade-in": "fade-in 180ms cubic-bezier(0.25, 0.1, 0.25, 1)",
        "fade-out": "fade-out 140ms cubic-bezier(0.25, 0.1, 0.25, 1)",
        "zoom-in": "zoom-in 180ms cubic-bezier(0.22, 1, 0.36, 1)",
        "zoom-out": "zoom-out 130ms cubic-bezier(0.25, 0.1, 0.25, 1)",
        "slide-up": "slide-up 200ms cubic-bezier(0.22, 1, 0.36, 1)",
        "slide-down": "slide-down 200ms cubic-bezier(0.22, 1, 0.36, 1)",
        "sheet-in": "sheet-in 240ms cubic-bezier(0.22, 1, 0.36, 1)",
        "sheet-out": "sheet-out 160ms cubic-bezier(0.25, 0.1, 0.25, 1)",
        "accordion-down": "accordion-down 200ms cubic-bezier(0.25, 0.1, 0.25, 1)",
        "accordion-up": "accordion-up 200ms cubic-bezier(0.25, 0.1, 0.25, 1)",
      },
    },
  },
  plugins: [
    /**
     * Radix state helpers. The primitives drive entry and exit from
     * `data-state`, so expose that as `enter-*` / `exit-*` utilities instead
     * of pulling in tailwindcss-animate for the same four rules.
     */
    plugin(({ addUtilities, matchUtilities, theme }) => {
      addUtilities({
        ".animate-enter": {
          animationFillMode: "both",
        },
        ".drag-region": { "-webkit-app-region": "drag" },
        ".no-drag": { "-webkit-app-region": "no-drag" },
      });
      matchUtilities(
        {
          enter: (value) => ({ "&[data-state='open']": { animation: value } }),
          exit: (value) => ({ "&[data-state='closed']": { animation: value } }),
        },
        { values: theme("animation") },
      );
    }),
  ],
};
