// The inbox's trash, for the undo demo. A deleted note waits 30 days and
// Undo brings it back, as in the app. Pure, so it is tested without a browser.

export const TRASH_MS = 30 * 24 * 60 * 60 * 1000;

export interface TrashNote {
  id: number;
  text: string;
  /** Null while the note is in the list. */
  deletedAt: number | null;
}

export type TrashAction =
  | { type: "delete"; id: number; now: number }
  | { type: "undo"; id: number; now: number }
  | { type: "purge"; now: number };

export function trashReducer(notes: TrashNote[], action: TrashAction): TrashNote[] {
  switch (action.type) {
    case "delete":
      return notes.map((n) =>
        n.id === action.id && n.deletedAt === null ? { ...n, deletedAt: action.now } : n,
      );
    case "undo":
      return notes.map((n) =>
        n.id === action.id && n.deletedAt !== null && action.now - n.deletedAt < TRASH_MS
          ? { ...n, deletedAt: null }
          : n,
      );
    case "purge":
      return notes.filter((n) => n.deletedAt === null || action.now - n.deletedAt < TRASH_MS);
  }
}
