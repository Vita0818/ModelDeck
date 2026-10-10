import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { codeInspectorPlugin } from "code-inspector-plugin";

export default defineConfig(({ command }) => ({
  root: "src",
  plugins: [
    command === "serve" &&
      codeInspectorPlugin({
        bundler: "vite",
      }),
    react(),
  ].filter(Boolean),
  base: "./",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          // The CommonJS helpers must initialize before their consumers.
          if (id.includes("commonjsHelpers.js")) return "react";
          if (!id.includes("node_modules")) return undefined;
          if (id.includes("recharts") || id.includes("d3-")) return "charts";
          // Keep the umbrella package with its modules: putting `codemirror`
          // in vendor creates a vendor/editor cycle that crashes on startup.
          if (
            id.includes("/codemirror/") ||
            id.includes("@codemirror") ||
            id.includes("@lezer")
          )
            return "editor";
          if (id.includes("framer-motion")) return "motion";
          if (id.includes("@radix-ui")) return "radix";
          if (id.includes("i18next")) return "i18n";
          if (
            // Match package boundaries so @floating-ui/react-dom stays in
            // vendor rather than creating a react/vendor initialization cycle.
            id.includes("/node_modules/react/") ||
            id.includes("/node_modules/react-dom/") ||
            id.includes("/node_modules/scheduler/")
          )
            return "react";
          return "vendor";
        },
      },
    },
  },
  server: {
    port: 3000,
    strictPort: true,
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_"],
}));
