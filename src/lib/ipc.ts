// The only module that talks to the native side. Everything else imports
// `api` and `on`, which keeps components testable with a single mock.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import type {
  AppError,
  AppInfo,
  AttachReport,
  Attachment,
  DetailsTarget,
  NoteDetail,
  Counts,
  DataInfo,
  Draft,
  DayMark,
  Glance,
  ImportReport,
  LookDoc,
  LookSummary,
  Note,
  NoteQuery,
  ResolvedLook,
  Settings,
  SettingsState,
  Status,
  UpdateInfo,
  TrashedNote,
} from "./types";

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),

  captureSave: (content: string, category: string | null, hide: boolean) =>
    invoke<Note>("capture_save", { content, category, hide }),
  captureDismiss: () => invoke<void>("capture_dismiss"),
  captureOpen: () => invoke<void>("capture_open"),
  draftGet: () => invoke<Draft | null>("draft_get"),
  draftSet: (content: string, category: string | null) =>
    invoke<void>("draft_set", { content, category }),
  draftClear: () => invoke<void>("draft_clear"),

  notesList: (query: NoteQuery) => invoke<Note[]>("notes_list", { query }),
  notesCounts: (todayFrom: number, todayTo: number) =>
    invoke<Counts>("notes_counts", { todayFrom, todayTo }),
  categoriesList: () => invoke<string[]>("categories_list"),
  glanceGet: (todayFrom: number, todayTo: number) =>
    invoke<Glance>("glance_get", { todayFrom, todayTo }),
  todayDismiss: () => invoke<void>("today_dismiss"),
  notesDayMarks: (edges: number[]) => invoke<DayMark[]>("notes_day_marks", { edges }),
  inboxOpenDate: (day: string) => invoke<void>("inbox_open_date", { day }),
  noteUpdate: (id: string, content: string, category: string | null) =>
    invoke<Note>("note_update", { id, content, category }),
  noteSetStatus: (id: string, status: Status) => invoke<Note>("note_set_status", { id, status }),
  noteSetLater: (id: string, later: boolean) => invoke<Note>("note_set_later", { id, later }),
  noteDelete: (id: string) => invoke<void>("note_delete", { id }),
  noteUndelete: (id: string) => invoke<Note>("note_undelete", { id }),
  trashList: () => invoke<TrashedNote[]>("trash_list"),
  trashPurge: (id: string) => invoke<void>("trash_purge", { id }),
  trashEmpty: (upTo: number) => invoke<number>("trash_empty", { upTo }),

  detailsOpen: (id: string, from: DetailsTarget["from"]) =>
    invoke<void>("details_open", { id, from }),
  detailsDismiss: () => invoke<void>("details_dismiss"),
  detailsCurrent: () => invoke<DetailsTarget | null>("details_current"),
  noteDetailGet: (id: string) => invoke<NoteDetail>("note_detail_get", { id }),
  noteDetailsSet: (id: string, details: string) =>
    invoke<Note>("note_details_set", { id, details }),
  attachmentAddFiles: (noteId: string) =>
    invoke<AttachReport | null>("attachment_add_files", { noteId }),
  attachmentAddPaths: (noteId: string, paths: string[]) =>
    invoke<AttachReport>("attachment_add_paths", { noteId, paths }),
  /** Sends the bytes as a raw body; the name travels percent-encoded in a header. */
  attachmentAddBytes: (noteId: string, name: string, mime: string, bytes: Uint8Array) =>
    invoke<Attachment>("attachment_add_bytes", bytes, {
      headers: {
        "x-note-id": encodeURIComponent(noteId),
        "x-name": encodeURIComponent(name),
        "x-mime": encodeURIComponent(mime),
      },
    }),
  attachmentPreview: (id: string) => invoke<ArrayBuffer>("attachment_preview", { id }),
  attachmentOpen: (id: string) => invoke<void>("attachment_open", { id }),
  attachmentSaveCopy: (id: string) => invoke<string | null>("attachment_save_copy", { id }),
  attachmentRemove: (id: string) => invoke<void>("attachment_remove", { id }),

  settingsGet: () => invoke<SettingsState>("settings_get"),
  settingsUpdate: (settings: Settings) => invoke<SettingsState>("settings_update", { settings }),

  dataInfo: () => invoke<DataInfo>("data_info"),
  dataReveal: () => invoke<void>("data_reveal"),
  dataExport: () => invoke<number | null>("data_export"),
  dataImport: () => invoke<ImportReport | null>("data_import"),
  dataBackup: () => invoke<string | null>("data_backup"),
  dataClear: () => invoke<SettingsState>("data_clear", { confirm: "DELETE" }),

  lookList: () => invoke<LookSummary[]>("look_list"),
  lookActive: () => invoke<ResolvedLook | null>("look_active"),
  lookDoc: (id: string) => invoke<LookDoc>("look_doc", { id }),
  lookPreview: (doc: LookDoc) => invoke<ResolvedLook>("look_preview", { doc }),
  lookSave: (doc: LookDoc, id: string | null) => invoke<LookSummary>("look_save", { doc, id }),
  lookImport: (code: string) => invoke<LookSummary>("look_import", { code }),
  lookExportCode: (id: string) => invoke<string>("look_export_code", { id }),
  lookDelete: (id: string) => invoke<void>("look_delete", { id }),

  aiPolish: (text: string) => invoke<string>("ai_polish", { text }),

  /** Looks for a newer release. Null means the app is current. */
  checkForUpdate: async (): Promise<UpdateInfo | null> => {
    const update = await fetchUpdate();
    return update
      ? {
          version: update.version,
          currentVersion: update.currentVersion,
          notes: update.body ?? null,
        }
      : null;
  },
  /** Downloads, installs and restarts. Throws when the download fails. */
  installUpdate: async (): Promise<void> => {
    const update = await fetchUpdate();
    if (!update) return;
    try {
      await update.downloadAndInstall();
    } catch {
      throw new Error("The update could not be installed. Try again from Settings → Updates.");
    }
    await relaunch();
  },
};

/** The updater's own errors name transport details the user can't act on. */
async function fetchUpdate() {
  try {
    return await check();
  } catch (e) {
    throw new Error(
      /release json/i.test(String(e))
        ? "No update information has been published yet. Try again later."
        : "Could not reach the update server. Check your internet connection and try again.",
      { cause: e },
    );
  }
}

export interface Events {
  "notes-changed": null;
  "looks-changed": null;
  "settings-changed": Settings;
  "capture-shown": { source: string | null };
  "capture-hidden": null;
  "today-shown": null;
  "today-returned": null;
  "details-open": DetailsTarget;
  "details-close-requested": null;
  "inbox-show-date": string;
  navigate: string;
}

/** Subscribes to a native event. Returns a function that unsubscribes. */
export function on<K extends keyof Events>(name: K, handler: (payload: Events[K]) => void) {
  let active = true;
  const pending = listen<Events[K]>(name, (e) => {
    if (active) handler(e.payload);
  });
  return () => {
    active = false;
    void pending.then((unlisten) => unlisten());
  };
}

/** Paths of files dropped on this window. Returns a function that unsubscribes. */
export function onFileDrop(handler: (paths: string[]) => void, onHover?: (over: boolean) => void) {
  let active = true;
  const pending = getCurrentWebview().onDragDropEvent((e) => {
    if (!active) return;
    const p = e.payload;
    if (p.type === "drop") {
      onHover?.(false);
      if (p.paths.length > 0) handler(p.paths);
    } else if (p.type === "leave") {
      onHover?.(false);
    } else {
      onHover?.(true);
    }
  });
  return () => {
    active = false;
    void pending.then((unlisten) => unlisten());
  };
}

/** Normalises whatever a failed command threw into `{ code, message }`. */
export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "message" in e) {
    const o = e as { code?: unknown; message: unknown };
    return { code: typeof o.code === "string" ? o.code : "unknown", message: String(o.message) };
  }
  return { code: "unknown", message: typeof e === "string" ? e : "Something went wrong." };
}
