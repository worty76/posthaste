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
  simple_looks: false,
  language: "en",
  ai_provider: "none",
  ai_models: {},
  ai_base_urls: {},
};

/** What the fake shell holds when a test gives it nothing. */
export function emptyStore() {
  return {
    notes: [],
    /** `{ note, deleted_at }`, newest deletion first. */
    trash: [],
    /** Description text by note id. */
    details: {},
    /** Attachments by note id. */
    attachments: {},
    /** The note the details window opens on. */
    target: null,
    /** What the file dialog answers: an attach report, or null for cancel. */
    pick: null,
    looks: [],
    /** The resolved scene `look_active_scene` answers, or null for Plain. */
    scene: null,
    /** The scene edit `look_scene` answers per look id. */
    sceneEdits: {},
    /** The resolved scene `look_scene_preview` answers, or null for empty. */
    previewScene: null,
    /** What `look_file_open` answers: a preview payload, or null for cancel. */
    fileOpen: null,
    ai: { backend: "keychain", providers: [] },
    /** The newer release `update_check` finds, or null. */
    update: null,
  };
}

function status(accelerator) {
  return { accelerator, registered: true, error: null };
}

let server;
let browser;
let origin;

export async function start() {
  server = createServer(async (request, response) => {
    const path = normalize(new URL(request.url, "http://x").pathname);
    // The browser asks for this by itself; the app has none.
    if (path === "/favicon.ico") return response.writeHead(204).end();
    try {
      const body = await readFile(join(DIST, path));
      response.writeHead(200, {
        "content-type": TYPES[extname(path)] ?? "application/octet-stream",
      });
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
 *   glance    what `glance_get` answers, or a function body of (from, to)
 *   now       a local date-time to pin the clock to, e.g. "2026-11-15T12:00:00"
 *   store     what the fake shell holds: see `emptyStore`
 *   info      overrides for what `app_info` answers
 */
export async function open({
  label = "capture",
  os = "linux",
  settings = {},
  draft = null,
  glance = null,
  now = null,
  locale = "en-GB",
  store = {},
  info = {},
} = {}) {
  // A fixed zone and locale, so dates read the same on every machine.
  const context = await browser.newContext({ timezoneId: "Asia/Ho_Chi_Minh", locale });
  const page = await context.newPage();
  // Pins `Date.now()`; timers keep running.
  if (now) await page.clock.setFixedTime(new Date(now));
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));

  await page.addInitScript(
    ({ label, os, settings, draft, glance, shortcuts, store, info }) => {
      const listeners = {};
      const mock = {
        /** Every command the interface ran, in order. */
        calls: [],
        /** Notes, trash, looks and the rest of what the shell would hold. */
        store,
        shortcuts,
        settings,
        draft,
        glance: glance ?? { carried: [], today: [], done: 0, total: 0 },
        /** Day counts for the calendar, keyed by the day's start instant. */
        marks: {},
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
        app_info: () => ({
          version: "0.0.0",
          os,
          session: "x11",
          tray_available: true,
          startup_error: null,
          ...info,
        }),
        settings_get: () => ({ settings: mock.settings, shortcuts }),
        look_active: () => null,
        look_scene: ({ id }) => store.sceneEdits[id] ?? { scene: {}, assets: [] },
        look_scene_preview: () =>
          store.previewScene ?? { main: {}, capture: {}, today: {}, details: {}, skipped: 0 },
        look_asset_pick: () => store.pick,
        look_file_export: () => "/home/me/look.phlook",
        look_file_open: () => store.fileOpen,
        look_file_install: () => {
          const saved = summary(`user:${store.looks.length}`, "From file", "dark");
          store.looks.push(saved);
          mock.emit("looks-changed");
          return saved;
        },
        look_file_discard: () => null,
        look_active_scene: () =>
          store.scene ?? { main: {}, capture: {}, today: {}, details: {}, skipped: 0 },
        look_asset: ({ id }) =>
          `data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==#${id}`,
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
        glance_get: () => mock.glance,
        notes_day_marks: ({ edges }) =>
          edges.slice(0, -1).map((start) => mock.marks[start] ?? { total: 0, open: 0 }),
        // The shell announces every change to notes; so does this.
        note_set_status: ({ id, status }) => {
          const found = note(id);
          if (found) found.status = status;
          return changed();
        },
        note_set_later: ({ id, later }) => {
          const found = note(id);
          if (found) found.later_at = later ? Date.now() : null;
          return changed();
        },
        note_update: ({ id, content, category }) => {
          Object.assign(note(id) ?? {}, { content, category, updated_at: Date.now() });
          return changed();
        },
        notes_list: ({ query }) => {
          const within = (n) =>
            (!query.text || n.content.toLowerCase().includes(query.text.toLowerCase())) &&
            (!query.statuses?.length || query.statuses.includes(n.status)) &&
            (query.created_from == null || n.created_at >= query.created_from) &&
            (query.created_to == null || n.created_at < query.created_to) &&
            (query.later == null || (n.later_at != null) === query.later);
          const found = store.notes.filter(within).sort((a, b) => b.created_at - a.created_at);
          return found.slice(0, query.limit ?? found.length);
        },
        notes_counts: ({ todayFrom, todayTo }) => {
          const open = store.notes.filter((n) => n.status === "inbox");
          return {
            inbox: open.filter((n) => n.later_at == null).length,
            today: store.notes.filter((n) => n.created_at >= todayFrom && n.created_at < todayTo)
              .length,
            later: open.filter((n) => n.later_at != null).length,
            archive: store.notes.length - open.length,
            trash: store.trash.length,
          };
        },
        note_delete: ({ id }) => {
          const found = note(id);
          store.notes = store.notes.filter((n) => n.id !== id);
          if (found) store.trash.unshift({ note: found, deleted_at: Date.now() });
          return changed();
        },
        note_undelete: ({ id }) => {
          const found = store.trash.find((t) => t.note.id === id);
          store.trash = store.trash.filter((t) => t.note.id !== id);
          if (found) store.notes.push(found.note);
          return changed();
        },
        trash_list: () => store.trash,
        trash_purge: ({ id }) => {
          store.trash = store.trash.filter((t) => t.note.id !== id);
          return changed();
        },
        trash_empty: ({ upTo }) => {
          const before = store.trash.length;
          store.trash = store.trash.filter((t) => t.deleted_at > upTo);
          changed();
          return before - store.trash.length;
        },
        details_current: () => store.target,
        note_detail_get: ({ id }) => {
          const found = note(id);
          if (!found) return Promise.reject({ code: "not_found", message: "That note is gone." });
          return {
            note: found,
            details: store.details[id] ?? "",
            attachments: store.attachments[id] ?? [],
          };
        },
        note_details_set: ({ id, details }) => {
          store.details[id] = details;
          return changed();
        },
        attachment_add_files: () => store.pick,
        attachment_add_paths: ({ noteId, paths }) => {
          const added = paths.map((path) =>
            attach(noteId, path.split("/").pop(), "text/plain", 12),
          );
          changed();
          return { added, failed: [] };
        },
        attachment_add_bytes: (bytes, options) => {
          const header = (name) => decodeURIComponent(options.headers[name]);
          mock.calls.at(-1).args = { headers: options.headers, size: bytes.length };
          const added = attach(
            header("x-note-id"),
            header("x-name"),
            header("x-mime"),
            bytes.length,
          );
          changed();
          return added;
        },
        attachment_preview: () => new Uint8Array([1, 2, 3]).buffer,
        attachment_save_copy: () => "/tmp/copy",
        attachment_remove: ({ id }) => {
          for (const key of Object.keys(store.attachments)) {
            store.attachments[key] = store.attachments[key].filter((a) => a.id !== id);
          }
          return changed();
        },
        settings_update: ({ settings }) => {
          mock.settings = settings;
          mock.emit("settings-changed", settings);
          return { settings, shortcuts: mock.shortcuts };
        },
        data_info: () => ({
          path: "/home/me/posthaste.db",
          size_bytes: 88000,
          note_count: store.notes.length,
          schema_version: 1,
        }),
        data_export: () => store.notes.length,
        data_import: () => ({ imported: 2, skipped_existing: 1, skipped_invalid: 0 }),
        data_backup: () => "/home/me/backup.db",
        data_clear: () => {
          store.notes = [];
          return { settings: mock.settings, shortcuts: mock.shortcuts };
        },
        look_list: () => store.looks,
        look_theme: ({ id }) => {
          const look = store.looks.find((l) => l.id === id);
          return {
            name: look?.name ?? "Look",
            author: null,
            base: look?.base ?? "dark",
            shape: "soft",
            font: "system",
            font_family: null,
            colors: { bg: "#101010", surface: "#202020", text: "#f0f0f0", accent: "#ff8800" },
          };
        },
        look_preview: ({ theme }) =>
          /^#[0-9a-f]{3}([0-9a-f]{3})?$/i.test(theme.colors.bg)
            ? {
                id: "preview",
                name: theme.name,
                author: null,
                base: theme.base,
                shape: theme.shape,
                font: theme.font,
                tokens: { "--bg": theme.colors.bg },
              }
            : Promise.reject({ code: "look_invalid", message: "Background is not a colour." }),
        look_save: ({ theme, id }) => {
          const saved = summary(id ?? `user:${store.looks.length}`, theme.name, theme.base);
          store.looks = store.looks.filter((l) => l.id !== saved.id).concat(saved);
          mock.emit("looks-changed");
          return saved;
        },
        look_import: ({ code }) => {
          if (!code.startsWith("posthaste-look:2:")) {
            return Promise.reject({ code: "look_invalid", message: "That is not a look code." });
          }
          const saved = summary(`user:${store.looks.length}`, "Imported", "dark");
          store.looks.push(saved);
          mock.emit("looks-changed");
          return saved;
        },
        look_export_code: ({ id }) => `posthaste-look:2:${id}`,
        look_delete: ({ id }) => {
          store.looks = store.looks.filter((l) => l.id !== id);
          if (mock.settings.look_id === id) mock.settings.look_id = null;
          mock.emit("looks-changed");
          mock.emit("settings-changed", mock.settings);
          return null;
        },
        ai_providers: () => store.ai,
        ai_key_set: ({ provider }) => {
          store.ai.providers.find((p) => p.id === provider).has_key = true;
          return store.ai;
        },
        ai_key_clear: ({ provider }) => {
          store.ai.providers.find((p) => p.id === provider).has_key = false;
          return store.ai;
        },
        ai_test: () => "Fixed text.",
        update_check: () => store.update,
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
      const note = (id) => store.notes.find((n) => n.id === id);
      const changed = () => (mock.emit("notes-changed"), null);
      const summary = (id, name, base) => ({
        id,
        name,
        author: null,
        builtin: id.startsWith("builtin:"),
        base,
        preview: { bg: "#101010", surface: "#202020", text: "#f0f0f0", accent: "#ff8800" },
      });
      let nextFile = 0;
      const attach = (noteId, name, mime, size) => {
        const file = { id: `a${++nextFile}`, note_id: noteId, name, mime, size, created_at: 1 };
        (store.attachments[noteId] ??= []).push(file);
        return file;
      };
      window.__mock = mock;
      window.__TAURI__ = {
        core: {
          invoke: async (cmd, args, options) => {
            mock.calls.push({ cmd, args });
            const handler = mock.handlers[cmd] ?? defaults[cmd];
            return handler ? handler(args, options) : null;
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
        webview: {
          getCurrentWebview: () => ({
            onDragDropEvent: async (cb) => {
              (listeners["drag-drop"] ??= []).push(cb);
              return () => {};
            },
          }),
        },
      };
    },
    {
      label,
      os,
      draft,
      glance,
      info,
      store: { ...emptyStore(), ...store },
      settings: { ...defaultSettings, ...settings },
      shortcuts: {
        capture: status("CmdOrCtrl+Shift+Space"),
        inbox: status("Ctrl+Alt+N"),
        today: status("Ctrl+Alt+G"),
        limitation: null,
      },
    },
  );
  await page.goto(`${origin}/index.html`);

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
    settings: () => page.evaluate(() => window.__mock.settings),
    store: () => page.evaluate(() => window.__mock.store),
    /** Changes what the fake shell holds: `(store) => { … }`, run in the page. */
    change: (edit, arg) =>
      page.evaluate(
        ([edit, arg]) => new Function("store", "arg", edit)(window.__mock.store, arg),
        [edit, arg],
      ),
    /** Files dragged over the window: "over", "leave", or "drop" with paths. */
    drag: (type, paths = []) =>
      page.evaluate(
        ([t, p]) => window.__mock.emit("drag-drop", { type: t, paths: p }),
        [type, paths],
      ),
    setGlance: (glance) => page.evaluate((g) => (window.__mock.glance = g), glance),
    setMarks: (marks) => page.evaluate((m) => (window.__mock.marks = m), marks),
  };
  mock.setNow = (now) => page.clock.setFixedTime(new Date(now));
  return { page, mock, errors, close: () => context.close() };
}

let nextNote = 0;

/** A note as the shell sends it. */
export function note(content, patch = {}) {
  nextNote += 1;
  return {
    id: `n${nextNote}`,
    content,
    status: "inbox",
    category: null,
    source: null,
    created_at: 0,
    updated_at: 0,
    completed_at: null,
    archived_at: null,
    has_details: false,
    attachment_count: 0,
    later_at: null,
    ...patch,
  };
}

/** Polls until `read()` satisfies `check`, the way a person waits. */
export async function eventually(read, check, message) {
  const deadline = Date.now() + 3000;
  let value;
  do {
    value = await read();
    if (check(value)) return value;
    await new Promise((resolve) => setTimeout(resolve, 25));
  } while (Date.now() < deadline);
  throw new Error(`${message}; last saw ${JSON.stringify(value)}`);
}

/** Waits for `locator`'s text to contain `expected`. */
export const text = (locator, expected) =>
  eventually(
    () => locator.textContent(),
    (t) => (t ?? "").includes(expected),
    `expected "${expected}"`,
  );
