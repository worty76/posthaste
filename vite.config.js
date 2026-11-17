import { defineConfig } from "vite";

// Vite only bundles the stylesheets and serves the WebAssembly module; the
// interface itself is built by scripts/build-ui.sh.
export default defineConfig({
  // Tauri expects a fixed port and reads Rust errors from the same terminal.
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: { target: "es2022", sourcemap: false },
});
