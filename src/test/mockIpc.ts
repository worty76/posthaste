// A stand-in for src/lib/ipc.ts. Tests set what commands return and fire
// native events with `emit`.

import { vi } from "vitest";
import type { AiOverview, AiProviderInfo, Note, Settings, SettingsState } from "../lib/types";

export const defaultSettings: Settings = {
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
  look_id: "builtin:ink",
  language: "en",
  ai_provider: "none",
  ai_models: {},
  ai_base_urls: {},
};

export function settingsState(patch: Partial<Settings> = {}): SettingsState {
  return {
    settings: { ...defaultSettings, ...patch },
    shortcuts: {
      capture: { accelerator: "CmdOrCtrl+Shift+Space", registered: true, error: null },
      inbox: { accelerator: "Ctrl+Alt+N", registered: true, error: null },
      today: { accelerator: "Ctrl+Alt+G", registered: true, error: null },
      limitation: null,
    },
  };
}

let seq = 0;
export function note(content: string, patch: Partial<Note> = {}): Note {
  seq += 1;
  return {
    id: `note-${seq}`,
    content,
    status: "inbox",
    category: null,
    source: null,
    created_at: Date.now() - seq * 1000,
    updated_at: Date.now(),
    completed_at: null,
    archived_at: null,
    has_details: false,
    attachment_count: 0,
    later_at: null,
    ...patch,
  };
}

const handlers = new Map<string, Set<(payload: unknown) => void>>();

export function emit(name: string, payload: unknown = null) {
  handlers.get(name)?.forEach((h) => h(payload));
}

export const api = {
  appInfo: vi.fn(),
  captureSave: vi.fn(),
  captureDismiss: vi.fn(),
  captureOpen: vi.fn(),
  draftGet: vi.fn(),
  draftSet: vi.fn(),
  draftClear: vi.fn(),
  notesList: vi.fn(),
  notesCounts: vi.fn(),
  categoriesList: vi.fn(),
  glanceGet: vi.fn(),
  notesDayMarks: vi.fn(),
  inboxOpenDate: vi.fn(),
  todayDismiss: vi.fn(),
  noteUpdate: vi.fn(),
  noteSetStatus: vi.fn(),
  noteSetLater: vi.fn(),
  noteDelete: vi.fn(),
  noteUndelete: vi.fn(),
  trashList: vi.fn(),
  trashPurge: vi.fn(),
  trashEmpty: vi.fn(),
  detailsOpen: vi.fn(),
  detailsDismiss: vi.fn(),
  detailsCurrent: vi.fn(),
  noteDetailGet: vi.fn(),
  noteDetailsSet: vi.fn(),
  attachmentAddFiles: vi.fn(),
  attachmentAddPaths: vi.fn(),
  attachmentAddBytes: vi.fn(),
  attachmentPreview: vi.fn(),
  attachmentOpen: vi.fn(),
  attachmentSaveCopy: vi.fn(),
  attachmentRemove: vi.fn(),
  settingsGet: vi.fn(),
  settingsUpdate: vi.fn(),
  dataInfo: vi.fn(),
  dataReveal: vi.fn(),
  dataExport: vi.fn(),
  dataImport: vi.fn(),
  dataBackup: vi.fn(),
  dataClear: vi.fn(),
  lookList: vi.fn(),
  lookActive: vi.fn(),
  lookDoc: vi.fn(),
  lookPreview: vi.fn(),
  lookSave: vi.fn(),
  lookImport: vi.fn(),
  lookExportCode: vi.fn(),
  lookDelete: vi.fn(),
  aiPolish: vi.fn(),
  aiProviders: vi.fn(),
  aiKeySet: vi.fn(),
  aiKeyClear: vi.fn(),
  aiKeyPage: vi.fn(),
  aiTest: vi.fn(),
  checkForUpdate: vi.fn(),
  installUpdate: vi.fn(),
};

/** Resets every command to a harmless default. Call in `beforeEach`. */
export function resetIpc() {
  handlers.clear();
  Object.values(api).forEach((fn) => fn.mockReset().mockResolvedValue(undefined));
  api.settingsGet.mockResolvedValue(settingsState());
  api.settingsUpdate.mockImplementation(async (s: Settings) => settingsState(s));
  api.aiProviders.mockResolvedValue(aiOverview());
  api.draftGet.mockResolvedValue(null);
  api.notesList.mockResolvedValue([]);
  api.notesCounts.mockResolvedValue({ inbox: 0, today: 0, later: 0, archive: 0, trash: 0 });
  api.trashList.mockResolvedValue([]);
  api.trashPurge.mockResolvedValue(undefined);
  api.trashEmpty.mockResolvedValue(0);
  api.categoriesList.mockResolvedValue([]);
  api.notesDayMarks.mockResolvedValue([]);
  api.inboxOpenDate.mockResolvedValue(undefined);
  api.glanceGet.mockResolvedValue({ carried: [], today: [], done: 0, total: 0 });
  api.dataInfo.mockResolvedValue({
    path: "/data/posthaste.db",
    size_bytes: 4096,
    note_count: 0,
    schema_version: 2,
  });
  api.lookList.mockResolvedValue([]);
  api.lookActive.mockResolvedValue(null);
  api.checkForUpdate.mockResolvedValue(null);
  api.detailsCurrent.mockResolvedValue(null);
  dropHandlers.clear();
}

const dropHandlers = new Set<(paths: string[]) => void>();

/** Simulates files dropped on the window. */
export function dropFiles(paths: string[]) {
  dropHandlers.forEach((h) => h(paths));
}

export const ipcMock = {
  api,
  on: (name: string, handler: (payload: unknown) => void) => {
    if (!handlers.has(name)) handlers.set(name, new Set());
    handlers.get(name)!.add(handler);
    return () => handlers.get(name)?.delete(handler);
  },
  onFileDrop: (handler: (paths: string[]) => void) => {
    dropHandlers.add(handler);
    return () => dropHandlers.delete(handler);
  },
  toAppError: (e: unknown) => {
    if (e && typeof e === "object" && "message" in e) {
      const o = e as { code?: unknown; message: unknown };
      return { code: typeof o.code === "string" ? o.code : "test", message: String(o.message) };
    }
    return { code: "unknown", message: "Something went wrong." };
  },
};

/** The native provider table, as `ai_providers` reports it. */
export const aiProviderList: AiProviderInfo[] = [
  {
    id: "anthropic",
    name: "Claude",
    key: "required",
    has_key: false,
    key_url: "console.anthropic.com/settings/keys",
    key_help: "An Anthropic API key.",
    default_model: "claude-haiku-4-5",
    base_url: "api.anthropic.com/v1",
    base_url_editable: false,
  },
  {
    id: "openai",
    name: "OpenAI",
    key: "required",
    has_key: false,
    key_url: "platform.openai.com/api-keys",
    key_help: "An OpenAI API key.",
    default_model: "gpt-4o-mini",
    base_url: "api.openai.com/v1",
    base_url_editable: false,
  },
  {
    id: "ollama",
    name: "Ollama",
    key: "none",
    has_key: false,
    key_url: null,
    key_help: "Runs on this computer. Notes never leave it.",
    default_model: "llama3.2",
    base_url: "localhost:11434/v1",
    base_url_editable: true,
  },
  {
    id: "custom",
    name: "OpenAI-compatible",
    key: "optional",
    has_key: false,
    key_url: null,
    key_help: "Any server that speaks the OpenAI chat format.",
    default_model: "",
    base_url: "",
    base_url_editable: true,
  },
];

export function aiOverview(
  keyed: string[] = [],
  backend: AiOverview["backend"] = "keychain",
): AiOverview {
  return {
    backend,
    providers: aiProviderList.map((p) => ({ ...p, has_key: keyed.includes(p.id) })),
  };
}
