/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "node:path";

// Pages: main UI, overlay, pop-up, crosshair and move layer. No remote content.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: "127.0.0.1" },
  build: {
    target: "es2023",
    sourcemap: false,
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "index.html"),
        overlay: resolve(import.meta.dirname, "overlay.html"),
        crosshair: resolve(import.meta.dirname, "crosshair.html"),
        popup: resolve(import.meta.dirname, "popup.html"),
        move: resolve(import.meta.dirname, "move.html"),
      },
    },
  },
  test: { include: ["src/**/*.test.ts"] },
});
