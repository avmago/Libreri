/// <reference types="vitest/config" />
import { cpSync, createReadStream, existsSync, statSync } from "node:fs";
import { extname, join, normalize } from "node:path";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;

/**
 * PDF.js needs its standard fonts, character maps, colour profiles and the
 * WebAssembly image decoders (JPEG 2000 and JBIG2, used by most scanned
 * PDFs) as plain files. Copy them from the package into public/pdfjs
 * (ignored by git) before dev/build.
 */
const MIME: Record<string, string> = {
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".wasm": "application/wasm",
  ".icc": "application/vnd.iccprofile",
};

function pdfjsAssets(): Plugin {
  const root = fileURLToPath(new URL("./public/pdfjs", import.meta.url));
  return {
    name: "libreri-pdfjs-assets",
    // In development, serve these files exactly as they are. Vite refuses to
    // let code import scripts from public/, but PDF.js loads its JavaScript
    // image decoders (the fallback when WebAssembly is unavailable) that way.
    configureServer(server) {
      server.middlewares.use("/pdfjs", (req, res, next) => {
        const path = normalize(join(root, decodeURIComponent((req.url ?? "").split("?")[0]!)));
        if (!path.startsWith(root) || !existsSync(path) || !statSync(path).isFile()) return next();
        res.setHeader("Content-Type", MIME[extname(path)] ?? "application/octet-stream");
        createReadStream(path).pipe(res);
      });
    },
    buildStart() {
      for (const dir of ["standard_fonts", "cmaps", "iccs", "wasm"]) {
        const from = fileURLToPath(new URL(`./node_modules/pdfjs-dist/${dir}`, import.meta.url));
        const to = fileURLToPath(new URL(`./public/pdfjs/${dir}`, import.meta.url));
        if (existsSync(from) && !existsSync(to)) cpSync(from, to, { recursive: true });
      }
    },
  };
}

/**
 * Excalidraw (handwriting canvases) loads its fonts as files from
 * `window.EXCALIDRAW_ASSET_PATH`. Copy them into public/excalidraw (ignored
 * by git), leaving out the 13 MB Chinese/Japanese handwriting font.
 */
function excalidrawAssets(): Plugin {
  return {
    name: "libreri-excalidraw-assets",
    buildStart() {
      const from = fileURLToPath(
        new URL("./node_modules/@excalidraw/excalidraw/dist/prod/fonts", import.meta.url),
      );
      const to = fileURLToPath(new URL("./public/excalidraw/fonts", import.meta.url));
      if (existsSync(from) && !existsSync(to))
        cpSync(from, to, { recursive: true, filter: (src) => !src.includes("Xiaolai") });
    },
  };
}

// https://tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react(), tailwindcss(), pdfjsAssets(), excalidrawAssets()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      // Not bundled: EPL-2.0, not GPL-compatible (see src/lib/stubs/elk.ts).
      "elkjs/lib/elk.bundled.js": fileURLToPath(new URL("./src/lib/stubs/elk.ts", import.meta.url)),
    },
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
