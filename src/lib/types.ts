// Mirrors the serde types in src-tauri/crates/core/src/model.rs. Keep in step.

export type Status = "inbox" | "completed" | "archived";

/** Timestamps are UTC milliseconds since the Unix epoch. */
export interface Note {
  id: string;
  content: string;
  status: Status;
  category: string | null;
  source: string | null;
  created_at: number;
  updated_at: number;
  completed_at: number | null;
  archived_at: number | null;
  /** True when the note has a description (the text is in `NoteDetail`). */
  has_details: boolean;
  /** How many files are attached. */
  attachment_count: number;
  /** When the note was parked in Later; null when it is not. */
  later_at: number | null;
}

/** A file attached to a note. The bytes stay in the database until asked for. */
export interface Attachment {
  id: string;
  note_id: string;
  name: string;
  mime: string;
  size: number;
  created_at: number;
}

/** Everything the Note details window shows. */
export interface NoteDetail {
  note: Note;
  details: string;
  attachments: Attachment[];
}

export interface AttachFailure {
  name: string;
  code: string;
  message: string;
}

/** A batch of files: the ones stored and the ones refused, by name. */
export interface AttachReport {
  added: Attachment[];
  failed: AttachFailure[];
}

/** Which note the details window shows and where Esc returns to. */
export interface DetailsTarget {
  id: string;
  from: "today" | "inbox";
}

/** Limits enforced by the Rust side; mirrored for messages shown before a call. */
export const MAX_ATTACHMENT_BYTES = 15 * 1024 * 1024;
export const MAX_ATTACHMENTS_PER_NOTE = 20;

export interface NoteQuery {
  text?: string;
  statuses?: Status[];
  created_from?: number | null;
  created_to?: number | null;
  /** true: only notes in Later; false: only notes not in Later; unset: either. */
  later?: boolean;
  limit?: number;
  offset?: number;
}

export interface Counts {
  inbox: number;
  today: number;
  later: number;
  archive: number;
  /** Notes in the trash that can still be restored. */
  trash: number;
}

/** How long a deleted note waits in the trash (mirrors the Rust constant). */
export const TRASH_RETENTION_MS = 30 * 86_400_000;

/** A note in the trash, with when it was put there. */
export interface TrashedNote {
  note: Note;
  deleted_at: number;
}

/** Notes captured on one day, and how many of them are still open. */
export interface DayMark {
  total: number;
  open: number;
}

/** What the Today window shows for one day. Carry-over is derived: nothing is moved. */
export interface Glance {
  carried: Note[];
  today: Note[];
  done: number;
  total: number;
}

export interface Draft {
  content: string;
  category: string | null;
  updated_at: number;
}

export type Theme = "system" | "light" | "dark";
export type AiProvider = "none" | "anthropic" | "openai" | "gemini";
export type CapturePosition = "center" | "upper";
/** Shown as Compact, Standard and Roomy. */
export type WindowSize = "small" | "medium" | "large";
export type EnterBehavior = "save" | "newline";
export type DraftRetention = "forever" | "week" | "day";

export interface Settings {
  theme: Theme;
  launch_at_login: boolean;
  start_minimized: boolean;
  capture_shortcut: string;
  inbox_shortcut: string;
  today_shortcut: string;
  capture_position: CapturePosition;
  window_size: WindowSize;
  hide_after_save: boolean;
  hide_on_blur: boolean;
  enter_behavior: EnterBehavior;
  draft_retention: DraftRetention;
  look_id: string | null;
  ai_provider: AiProvider;
  ai_api_key: string;
  ai_model: string;
}

// Mirrors the look types in src-tauri/crates/core/src/look/. Keep in step.

export type Base = "light" | "dark";
export type Shape = "sharp" | "soft" | "round";
export type FontFace = "system" | "rounded" | "serif" | "mono";

export interface LookColors {
  bg: string;
  surface: string;
  text: string;
  accent: string;
  bg_sunken?: string | null;
  surface_raised?: string | null;
  danger?: string | null;
  ok?: string | null;
}

export interface LookDoc {
  format: string;
  version: number;
  name: string;
  author?: string | null;
  base: Base;
  colors: LookColors;
  shape: Shape;
  font: FontFace;
}

export interface LookPreview {
  bg: string;
  surface: string;
  text: string;
  accent: string;
}

export interface LookSummary {
  id: string;
  name: string;
  author: string | null;
  builtin: boolean;
  base: Base;
  preview: LookPreview;
}

export interface ResolvedLook {
  id: string;
  name: string;
  author: string | null;
  base: Base;
  shape: Shape;
  font: FontFace;
  tokens: Record<string, string>;
}

export interface ShortcutStatus {
  accelerator: string;
  registered: boolean;
  error: string | null;
}

export interface ShortcutReport {
  capture: ShortcutStatus;
  inbox: ShortcutStatus;
  today: ShortcutStatus;
  limitation: string | null;
}

export interface SettingsState {
  settings: Settings;
  shortcuts: ShortcutReport;
}

export interface AppError {
  code: string;
  message: string;
}

export type Os = "macos" | "windows" | "linux";

export interface AppInfo {
  version: string;
  os: Os;
  session: "macos" | "windows" | "x11" | "wayland" | "unknown";
  tray_available: boolean;
  startup_error: AppError | null;
}

/** An update the updater plugin found. Null when the app is current. */
export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
}

export interface DataInfo {
  path: string;
  size_bytes: number;
  note_count: number;
  schema_version: number;
}

export interface ImportReport {
  imported: number;
  skipped_existing: number;
  skipped_invalid: number;
}

export type View = "inbox" | "today" | "later" | "archive" | "trash" | "settings";

export const DEFAULT_CAPTURE_SHORTCUT = "CmdOrCtrl+Shift+Space";
export const defaultInboxShortcut = (os: Os) => (os === "macos" ? "Cmd+Ctrl+N" : "Ctrl+Alt+N");
export const defaultTodayShortcut = (os: Os) => (os === "macos" ? "Cmd+Ctrl+G" : "Ctrl+Alt+G");
