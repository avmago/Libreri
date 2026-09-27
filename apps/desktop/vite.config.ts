/// <reference types="vitest/config" />
import { cpSync, existsSync } from "node:fs";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;

/**
 * PDF.js needs its standard fonts and character maps as plain files. Copy
 * them from the package into public/pdfjs (ignored by git) before dev/build.
 */
function pdfjsAssets(): Plugin {
  return {
    name: "libreri-pdfjs-assets",
    buildStart() {
      for (const dir of ["standard_fonts", "cmaps"]) {
        const from = fileURLToPath(new URL(`./node_modules/pdfjs-dist/${dir}`, import.meta.url));
        const to = fileURLToPath(new URL(`./public/pdfjs/${dir}`, import.meta.url));
        if (existsSync(from) && !existsSync(to)) cpSync(from, to, { recursive: true });
      }
    },
  };
}

// https://tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react(), tailwindcss(), pdfjsAssets()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: "es2022",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,
  },
});
