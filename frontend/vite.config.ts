import path from "node:path";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// https://vite.dev/config/
export default defineConfig({
  // Relative asset URLs: the same build works from the root of
  // demo.curvyo.org and from a sub-path (curvyo.github.io/curvyo/), and
  // inside the Tauri webview (docs/deploy-demo.md).
  base: "./",
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  // Tauri's own `devUrl` (curvyo-app/tauri.conf.json) is hardcoded to
  // http://localhost:1420 — this must match exactly, and `strictPort`
  // makes a port conflict a loud startup failure instead of Vite
  // silently drifting to a different port the webview never loads.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
