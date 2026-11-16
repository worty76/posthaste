import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  // Tauri expects a fixed port and reads Rust errors from the same terminal.
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "es2022",
    sourcemap: false,
    // Two pages while windows move from React to Rust: index.html is the
    // React interface, ui.html the Rust one (ui/).
    rollupOptions: { input: { main: "index.html", ui: "ui.html" } },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    restoreMocks: true,
    // The marketing site in landing-page/ has its own runner and alias setup.
    // The Rust interface in ui/ is tested in a real browser: `npm run test:ui`.
    exclude: ["**/node_modules/**", "landing-page/**", "ui/**"],
  },
});
