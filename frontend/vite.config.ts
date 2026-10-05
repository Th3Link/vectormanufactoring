import path from "node:path";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  // Tauri's own `devUrl` (vecmanf-app/tauri.conf.json) is hardcoded to
  // http://localhost:1420 — this must match exactly, and `strictPort`
  // makes a port conflict a loud startup failure instead of Vite
  // silently drifting to a different port the webview never loads.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
