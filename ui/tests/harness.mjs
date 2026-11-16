// Runs the built Rust interface in a real browser with the desktop shell
// replaced by a fake `window.__TAURI__`. Tests then drive the window the way
// a person does: by role, by label, by key.
//
// Needs `npm run build` first (it serves dist/), and Chrome or Chromium:
// set CHROME_PATH if it is not found on its own.

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const DIST = fileURLToPath(new URL("../../dist/", import.meta.url));
const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".wasm": "application/wasm",
};

export const defaultSettings = {
  theme: "system",
  launch_at_login: false,
  start_minimized: false,
  capture_shortcut: "CmdOrCtrl+Shift+Space",
  inbox_shortcut: "Ctrl+Alt+N",
  today_shortcut: "Ctrl+Alt+G",
  capture_position: "center",
  today_position: "center",
  window_size: "medium",
  hide_after_save: true,
  hide_on_blur: true,
  enter_behavior: "save",
  draft_retention: "forever",
  look_id: null,
  language: "en",
  ai_provider: "none",
  ai_models: {},
  ai_base_urls: {},
};

function status(accelerator) {
  return { accelerator, registered: true, error: null };
}

let server;
let browser;
let origin;

export async function start() {
  server = createServer(async (request, response) => {
    const path = normalize(new URL(request.url, "http://x").pathname);
    try {
      const body = await readFile(join(DIST, path));
      response.writeHead(200, { "content-type": TYPES[extname(path)] ?? "application/octet-stream" });
      response.end(body);
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({
    executablePath: process.env.CHROME_PATH || undefined,
    channel: process.env.CHROME_PATH ? undefined : "chrome",
  });
}

export async function stop() {
  await browser?.close();
  await new Promise((resolve) => server.close(resolve));
}

/**
 * Opens one window of the Rust interface.
 *   label     the window label the shell would give it
 *   os        what `app_info` reports
 *   settings  overrides for the default settings
 *   draft     the stored draft, or null
 */
export async function open({ label = "capture", os = "linux", settings = {}, draft = null } = {}) {
  const context = await browser.newContext();
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));

  await page.addInitScript(
    ({ label, os, settings, draft, shortcuts }) => {
      const listeners = {};
      const mock = {
        /** Every command the interface ran, in order. */
        calls: [],
        settings,
        draft,
        /** cmd -> function(args): overrides the default answer. */
        handlers: {},
        /** Resolvers of commands a test is holding open, by command. */
        held: {},
        emit(name, payload = null) {
          for (const cb of listeners[name] ?? []) cb({ event: name, id: 0, payload });
        },
        /** Makes `cmd` wait until `release(cmd, …)`. */
        hold(cmd) {
          mock.handlers[cmd] = () =>
            new Promise((resolve, reject) => (mock.held[cmd] = { resolve, reject }));
        },
        release(cmd, value = null) {
          mock.held[cmd].resolve(value);
        },
        fail(cmd, message, code = "storage") {
          mock.handlers[cmd] = () => Promise.reject({ code, message });
        },
      };
      const defaults = {
        app_info: () => ({ version: "0.0.0", os, session: "x11", tray_available: true, startup_error: null }),
        settings_get: () => ({ settings: mock.settings, shortcuts }),
        look_active: () => null,
        categories_list: () => ["work", "home"],
        draft_get: () => mock.draft,
        draft_set: ({ content, category }) => {
          mock.draft = { content, category, updated_at: 1 };
          return null;
        },
        draft_clear: () => {
          mock.draft = null;
          return null;
        },
        capture_save: ({ content, category }) => ({
          id: "n1",
          content,
          status: "inbox",
          category,
          source: null,
          created_at: 1,
          updated_at: 1,
          completed_at: null,
          archived_at: null,
          has_details: false,
          attachment_count: 0,
          later_at: null,
        }),
      };
      window.__mock = mock;
      window.__TAURI__ = {
        core: {
          invoke: async (cmd, args) => {
            mock.calls.push({ cmd, args });
            const handler = mock.handlers[cmd] ?? defaults[cmd];
            return handler ? handler(args) : null;
          },
        },
        event: {
          listen: async (name, cb) => {
            (listeners[name] ??= []).push(cb);
            return () => {
              listeners[name] = listeners[name].filter((x) => x !== cb);
            };
          },
        },
        window: { getCurrentWindow: () => ({ label }) },
      };
    },
    {
      label,
      os,
      draft,
      settings: { ...defaultSettings, ...settings },
      shortcuts: {
        capture: status("CmdOrCtrl+Shift+Space"),
        inbox: status("Ctrl+Alt+N"),
        today: status("Ctrl+Alt+G"),
        limitation: null,
      },
    },
  );
  await page.goto(`${origin}/ui.html`);

  const mock = {
    calls: (cmd) =>
      page.evaluate((cmd) => window.__mock.calls.filter((c) => !cmd || c.cmd === cmd), cmd),
    emit: (name, payload) => page.evaluate(([n, p]) => window.__mock.emit(n, p), [name, payload]),
    hold: (cmd) => page.evaluate((cmd) => window.__mock.hold(cmd), cmd),
    release: (cmd, value) => page.evaluate(([c, v]) => window.__mock.release(c, v), [cmd, value]),
    fail: (cmd, message) => page.evaluate(([c, m]) => window.__mock.fail(c, m), [cmd, message]),
    answer: (cmd, value) =>
      page.evaluate(([c, v]) => (window.__mock.handlers[c] = () => v), [cmd, value]),
    /** Saves new settings and tells the window, as another window would. */
    changeSettings: (patch) =>
      page.evaluate((patch) => {
        Object.assign(window.__mock.settings, patch);
        window.__mock.emit("settings-changed", window.__mock.settings);
      }, patch),
    draft: () => page.evaluate(() => window.__mock.draft),
  };
  return { page, mock, errors, close: () => context.close() };
}
