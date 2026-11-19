// State for the hands-on demo: the capture field in the hero and the Today
// window further down share it. Pure, so it is tested without a browser.
// Nothing here is stored or sent anywhere; it lives in the tab's memory.

export interface DemoNote {
  id: number;
  text: string;
  /** Milliseconds from the first keystroke to Enter; null for seeded notes. */
  savedInMs: number | null;
  done: boolean;
  /** Seeded "from yesterday" notes, shown under Carried over. */
  carried: boolean;
}

export interface DemoState {
  draft: string;
  /** When the first character of the current draft was typed. */
  startedAt: number | null;
  notes: DemoNote[];
  /** The timing of the most recent save, for the stamp beside the field. */
  lastSavedMs: number | null;
  nextId: number;
}

export type DemoAction =
  | { type: "input"; text: string; now: number }
  | { type: "save"; now: number }
  | { type: "clear" }
  | { type: "toggle"; id: number }
  | { type: "reset" };

/** Notes the visitor did not write: obviously made up, never quotes. */
const SEEDED: Omit<DemoNote, "id">[] = [
  { text: "Reply to the landlord about the heater", savedInMs: null, done: false, carried: true },
  { text: "Idea: sticker sheet for the zine", savedInMs: null, done: false, carried: true },
  { text: "Return the library books", savedInMs: null, done: true, carried: false },
];

export const MAX_NOTE_CHARS = 140;
/** The stack under the field keeps the most recent few. */
export const STACK_SIZE = 4;

export function initialDemo(): DemoState {
  return {
    draft: "",
    startedAt: null,
    notes: SEEDED.map((n, i) => ({ ...n, id: i + 1 })),
    lastSavedMs: null,
    nextId: SEEDED.length + 1,
  };
}

export function demoReducer(state: DemoState, action: DemoAction): DemoState {
  switch (action.type) {
    case "input": {
      const text = action.text.slice(0, MAX_NOTE_CHARS);
      const empty = text.trim() === "";
      return {
        ...state,
        draft: text,
        // The clock starts on the first real character and resets when the
        // field is emptied, so pauses before typing never count.
        startedAt: empty ? null : (state.startedAt ?? action.now),
      };
    }
    case "save": {
      const text = state.draft.trim();
      if (text === "") return state;
      const savedInMs = Math.max(0, action.now - (state.startedAt ?? action.now));
      return {
        ...state,
        draft: "",
        startedAt: null,
        lastSavedMs: savedInMs,
        nextId: state.nextId + 1,
        notes: [{ id: state.nextId, text, savedInMs, done: false, carried: false }, ...state.notes],
      };
    }
    case "clear":
      return { ...state, draft: "", startedAt: null };
    case "reset":
      return initialDemo();
    case "toggle":
      return {
        ...state,
        notes: state.notes.map((n) => (n.id === action.id ? { ...n, done: !n.done } : n)),
      };
  }
}

/** "1.8 s". One decimal, never "0.0 s" for a real save. */
export function formatSeconds(ms: number): string {
  const s = Math.max(0.1, Math.round(ms / 100) / 10);
  return `${s.toFixed(1)} s`;
}

/** Notes the visitor saved, newest first, for the stack under the field. */
export function savedStack(state: DemoState): DemoNote[] {
  return state.notes.filter((n) => n.savedInMs !== null).slice(0, STACK_SIZE);
}

export interface TodayView {
  carried: DemoNote[];
  today: DemoNote[];
  done: number;
  total: number;
}

/** The Today window's rule: open carried notes first, then today's, open before done. */
export function todayView(state: DemoState): TodayView {
  const carried = state.notes.filter((n) => n.carried && !n.done);
  const todayNotes = state.notes.filter((n) => !n.carried || n.done);
  const today = [...todayNotes.filter((n) => !n.done), ...todayNotes.filter((n) => n.done)];
  const all = [...carried, ...today];
  return { carried, today, done: all.filter((n) => n.done).length, total: all.length };
}

// ---- the self-playing capture ----
// On arrival the hero types two notes by itself, so the loop shows without a
// single keystroke. The moment the visitor touches the field it steps aside.

/** Delay before the first scripted character. */
export const DEMO_START_DELAY = 1400;
/** Delay between scripted characters: quick, but readable. */
export const DEMO_TYPE_MS = 55;
/** Delay from the last character to the scripted Enter. */
export const DEMO_ENTER_PAUSE = 400;
/** Delay from a save to the first character of the next note. */
export const DEMO_NOTE_PAUSE = 1900;
/** How long a finished cycle's stack stays up before it is cleared. */
export const DEMO_HOLD = 2400;

/** One scripted moment: the draft text at `at` ms from start, and whether
    Enter is pressed with it. */
export interface ScriptStep {
  at: number;
  text: string;
  save: boolean;
}

/** The keystroke plan for `notes`: type character by character, Enter after
    each, a breath between notes. Deterministic, so it is tested; the hero
    only fills in the clock. */
export function demoScript(notes: string[]): ScriptStep[] {
  const steps: ScriptStep[] = [];
  let at = DEMO_START_DELAY;
  let text = "";
  for (const note of notes) {
    for (const char of [...note]) {
      text += char;
      steps.push({ at, text, save: false });
      at += DEMO_TYPE_MS;
    }
    at += DEMO_ENTER_PAUSE;
    steps.push({ at, text, save: true });
    at += DEMO_NOTE_PAUSE;
    text = "";
  }
  return steps;
}

/** When the next cycle should start typing, from the moment the last one
    saved: the stack holds a beat, is cleared, then a breath. */
export function nextCycleDelay(steps: ScriptStep[]): number {
  return (steps.at(-1)?.at ?? 0) + DEMO_HOLD;
}

/** Notes the visitor ticked off, for the Done column. */
export function doneNotes(state: DemoState): DemoNote[] {
  return state.notes.filter((n) => n.done);
}

/** The inbox search rule: every word must appear, case ignored. */
export function searchNotes<T extends { text: string }>(notes: T[], query: string): T[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return notes;
  return notes.filter((n) => {
    const t = n.text.toLowerCase();
    return words.every((w) => t.includes(w));
  });
}
