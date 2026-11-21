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
  look_id: "builtin:ink",
  simple_looks: false,
  sidebar_collapsed: false,
  language: "en",
  ai_provider: "none",
  ai_models: {},
  ai_base_urls: {},
  sync_enabled: false,
  sync_device_name: "This computer",
  sync_relay_url: "https://relay.posthaste.ink",
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
    /** Steps by note id, in storage order. */
    steps: {},
    /** Explicit project rows `{ name, archived }`. */
    projects: [],
    /** The note the details window opens on. */
    target: null,
    /** What the file dialog answers: an attach report, or null for cancel. */
    pick: null,
    looks: [],
    /** The resolved scene `look_active_scene` answers, or null for none. */
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
    /** Sync status and paired devices for Settings -> Devices. */
    sync: {
      enabled: false,
      device_id: "local-dev-01",
      devices: [],
      last_error: null,
      offer: "posthaste-pair:1:TESTCODE123",
    },
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
        shortcuts_set_paused: () => null,
        // Every install wears a look: the shell falls back to the factory
        // default when a selection cannot resolve, and so does this.
        look_active: () =>
          store.activeLook ?? {
            id: mock.settings.look_id,
            name: "Look",
            author: null,
            base: "dark",
            shape: "soft",
            font: "system",
            tokens: { "--bg": "#101010" },
          },
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
        projects_list: ({ includeArchived }) => listProjects(Boolean(includeArchived)),
        project_create: ({ name }) => {
          const cleaned = cleanProject(name);
          if (!cleaned) {
            return Promise.reject({
              code: "project_invalid",
              message: "A project name is between 1 and 40 characters.",
            });
          }
          if (listProjects(true).some((p) => p.name === cleaned)) {
            return Promise.reject({
              code: "project_exists",
              message: "A project with that name already exists.",
            });
          }
          store.projects.push({ name: cleaned, archived: false });
          changed();
          return { name: cleaned, archived: false, open: 0, total: 0, last_at: 0 };
        },
        project_rename: ({ from, to, merge }) => {
          const src = cleanProject(from);
          const dst = cleanProject(to);
          if (!src || !dst) {
            return Promise.reject({
              code: "project_invalid",
              message: "A project name is between 1 and 40 characters.",
            });
          }
          if (src === dst) return 0;
          const all = listProjects(true);
          if (!all.some((p) => p.name === src)) {
            return Promise.reject({ code: "not_found", message: "That note no longer exists." });
          }
          if (all.some((p) => p.name === dst) && !merge) {
            return Promise.reject({
              code: "project_exists",
              message: "A project with that name already exists.",
            });
          }
          let moved = 0;
          for (const n of store.notes) {
            if (n.category === src) {
              n.category = dst;
              moved += 1;
            }
          }
          const oldRow = store.projects.find((p) => p.name === src);
          store.projects = store.projects.filter((p) => p.name !== src);
          if (!store.projects.some((p) => p.name === dst)) {
            store.projects.push({ name: dst, archived: Boolean(oldRow?.archived) });
          }
          changed();
          return moved;
        },
        project_set_archived: ({ name, archived }) => {
          const cleaned = cleanProject(name);
          const existing = store.projects.find((p) => p.name === cleaned);
          if (existing) existing.archived = Boolean(archived);
          else if (cleaned) store.projects.push({ name: cleaned, archived: Boolean(archived) });
          return changed();
        },
        project_clear: ({ name }) => {
          const cleaned = cleanProject(name);
          let cleared = 0;
          for (const n of store.notes) {
            if (n.category === cleaned) {
              n.category = null;
              cleared += 1;
            }
          }
          store.projects = store.projects.filter((p) => p.name !== cleaned);
          changed();
          return cleared;
        },
        note_set_project: ({ id, project }) => {
          const found = note(id);
          if (!found) {
            return Promise.reject({ code: "not_found", message: "That note no longer exists." });
          }
          const cleaned = project == null ? null : cleanProject(project);
          found.category = cleaned;
          if (cleaned) {
            const row = store.projects.find((p) => p.name === cleaned);
            if (row) row.archived = false;
            else store.projects.push({ name: cleaned, archived: false });
          }
          changed();
          return { ...found };
        },
        draft_get: () => mock.draft,
        draft_set: ({ content, category }) => {
          mock.draft = { content, category, updated_at: 1 };
          return null;
        },
        draft_clear: () => {
          mock.draft = null;
          return null;
        },
        capture_save: ({ content, category }) => {
          const now = Date.now();
          const saved = {
            id: `n-${store.notes.length + 1}-${now}`,
            content,
            category: category == null ? null : cleanProject(category),
            source: null,
            status: "inbox",
            later_at: null,
            created_at: now,
            updated_at: now,
            completed_at: null,
            archived_at: null,
            has_details: false,
            attachment_count: 0,
            step_total: 0,
            step_done: 0,
          };
          store.notes.unshift(saved);
          changed();
          return saved;
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
            (!query.category || n.category === query.category) &&
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
            steps: (store.steps[id] ?? [])
              .filter((st) => st.deleted_at == null)
              .sort((x, y) => (x.rank < y.rank ? -1 : x.rank > y.rank ? 1 : 0)),
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
          // The store moves a deleted selected look to the factory
          // default in the same transaction as the delete.
          if (mock.settings.look_id === id) mock.settings.look_id = "builtin:ink";
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
        sync_status: () => ({
          enabled: store.sync.enabled,
          device_id: store.sync.device_id,
          devices: store.sync.devices,
          last_error: store.sync.last_error,
        }),
        sync_set_enabled: ({ on }) => {
          store.sync.enabled = on;
          mock.settings.sync_enabled = on;
          mock.emit("settings-changed", mock.settings);
          mock.emit("sync-changed");
          return {
            enabled: store.sync.enabled,
            device_id: store.sync.device_id,
            devices: store.sync.devices,
            last_error: store.sync.last_error,
          };
        },
        sync_pair_offer: () => store.sync.offer,
        sync_pair_join: ({ code }) => {
          if (!code.startsWith("posthaste-pair:1:")) {
            return Promise.reject({
              code: "sync_pair",
              message: "That isn't a Posthaste pairing code.",
            });
          }
          const dev = {
            id: `peer-${store.sync.devices.length + 1}`,
            name: "Work laptop",
            last_sync_at: Date.now(),
            online: true,
          };
          store.sync.devices.push(dev);
          mock.emit("sync-changed");
          return dev;
        },
        sync_now: () => {
          mock.emit("sync-changed");
          return null;
        },
        sync_forget: ({ id }) => {
          store.sync.devices = store.sync.devices.filter((d) => d.id !== id);
          mock.emit("sync-changed");
          return null;
        },
        steps_add: ({ noteId, after, lines }) => {
          const list = (store.steps[noteId] ??= []);
          let rank = after ? (list.find((st) => st.id === after)?.rank ?? "") : "";
          const last = list[list.length - 1];
          if (after == null && last) rank = last.rank;
          const added = lines.map((line) => {
            const step = {
              id: `s${++nextStep}`,
              note_id: noteId,
              text: line.text,
              done: line.done,
              rank: rankBetween(rank, null),
              deleted_at: null,
            };
            list.push(step);
            rank = step.rank;
            return { ...step };
          });
          refreshCounts(noteId);
          changed();
          return added;
        },
        step_set_text: ({ id, text }) => {
          const found = step(id);
          if (!found) return Promise.reject({ code: "not_found", message: "gone" });
          found.text = text;
          refreshCounts(found.note_id);
          changed();
          return { ...found };
        },
        step_set_done: ({ id, done }) => {
          const found = step(id);
          if (!found) return Promise.reject({ code: "not_found", message: "gone" });
          found.done = done;
          refreshCounts(found.note_id);
          changed();
          return { ...found };
        },
        step_move: ({ id, after }) => {
          const found = step(id);
          if (!found) return Promise.reject({ code: "not_found", message: "gone" });
          const list = store.steps[found.note_id];
          const live = list.filter((st) => st.deleted_at == null);
          const before = after ? live.find((st) => st.id === after) : null;
          const lower = before ? before.rank : "";
          const upper = before
            ? (live.find((st) => st.rank > before.rank)?.rank ?? null)
            : (live[0]?.rank ?? null);
          found.rank = rankBetween(lower, upper);
          refreshCounts(found.note_id);
          changed();
          return { ...found };
        },
        step_remove: ({ id }) => {
          const found = step(id);
          if (!found) return Promise.reject({ code: "not_found", message: "gone" });
          found.deleted_at = Date.now();
          refreshCounts(found.note_id);
          changed();
          return null;
        },
        step_restore: ({ id }) => {
          const found = step(id);
          if (!found) return Promise.reject({ code: "not_found", message: "gone" });
          found.deleted_at = null;
          refreshCounts(found.note_id);
          changed();
          return { ...found };
        },
      };
      const note = (id) => store.notes.find((n) => n.id === id);
      const cleanProject = (raw) => {
        if (typeof raw !== "string") return null;
        const c = raw.trim().replace(/^#+/, "").trim();
        return c ? [...c].slice(0, 40).join("") : null;
      };
      const listProjects = (includeArchived) => {
        const names = new Set((store.projects ?? []).map((p) => p.name));
        for (const n of store.notes) {
          if (n.category) names.add(n.category);
        }
        const out = [];
        for (const name of names) {
          const row = (store.projects ?? []).find((p) => p.name === name);
          const archived = Boolean(row?.archived);
          if (!includeArchived && archived) continue;
          const live = store.notes.filter((n) => n.category === name);
          const open = live.filter((n) => n.status === "inbox").length;
          const total = live.length;
          const last_at = live.reduce((m, n) => Math.max(m, n.created_at ?? 0), 0);
          out.push({ name, archived, open, total, last_at });
        }
        out.sort(
          (a, b) => b.last_at - a.last_at || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0),
        );
        return out;
      };
      let nextStep = 0;
      const step = (id) =>
        Object.values(store.steps)
          .flat()
          .find((st) => st.id === id);
      const rankBetween = (low, high) => {
        // A rank strictly between, in the same spirit as the real one.
        if (high != null && low.length < high.length) return low + "m";
        return low + "n";
      };
      const refreshCounts = (noteId) => {
        const live = (store.steps[noteId] ?? []).filter((st) => st.deleted_at == null);
        const found = note(noteId);
        if (!found) return;
        found.step_total = live.length;
        found.step_done = live.filter((st) => st.done).length;
      };
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
    step_total: 0,
    step_done: 0,
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
