import { useCallback, useEffect, useRef, useState } from "react";
import { Button, IconButton } from "../components/Button";
import { Dialog } from "../components/Dialog";
import { EmptyState } from "../components/EmptyState";
import { NoteBadges } from "../details/NoteBadges";
import { erasedOn, groupByDeletedDay } from "../lib/dates";
import { api, on } from "../lib/ipc";
import { TRASH_RETENTION_MS, type Note, type TrashedNote } from "../lib/types";

type Show = (
  text: string,
  opts?: { kind?: "info" | "error"; action?: { label: string; run: () => void } },
) => void;

/** Where a restored note goes back to, in the words of the sidebar. */
export function returnsTo(note: Note): string {
  // Completed notes are listed in Archive, the same as archived ones.
  if (note.status !== "inbox") return "Archive";
  if (note.later_at !== null) return "Later";
  return "Inbox";
}

/** The note's first line, shortened, to name it in a confirmation. */
function excerpt(content: string, max = 80): string {
  const line = content.split("\n")[0]!.trim();
  return line.length > max ? `${line.slice(0, max - 1).trimEnd()}…` : line;
}

function isTyping(target: EventTarget | null) {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
}

/** The Trash section: deleted notes for 30 days, to restore or erase for good. */
export function TrashView({ show, fail }: { show: Show; fail: (e: unknown) => void }) {
  const [items, setItems] = useState<TrashedNote[] | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<{ kind: "one"; note: Note } | { kind: "all" } | null>(
    null,
  );
  const requestId = useRef(0);

  const load = useCallback(() => {
    const id = ++requestId.current;
    api.trashList().then(
      (list) => {
        if (id === requestId.current) setItems(list);
      },
      (e) => {
        if (id === requestId.current) fail(e);
      },
    );
  }, [fail]);

  useEffect(() => load(), [load]);
  useEffect(() => on("notes-changed", load), [load]);

  const list = items ?? [];
  const index = list.findIndex((t) => t.note.id === selectedId);

  /** Keeps the selection on a neighbour after a row leaves the list. */
  const selectNeighbour = (id: string) => {
    const i = list.findIndex((t) => t.note.id === id);
    const next = list[i + 1] ?? list[i - 1];
    setSelectedId(next?.note.id ?? null);
  };

  const restore = (note: Note) => {
    selectNeighbour(note.id);
    api.noteUndelete(note.id).then(
      () =>
        show(`Restored to ${returnsTo(note)}`, {
          action: { label: "Undo", run: () => void api.noteDelete(note.id).catch(fail) },
        }),
      fail,
    );
  };

  const erase = () => {
    const c = confirm;
    setConfirm(null);
    if (!c) return;
    if (c.kind === "one") {
      selectNeighbour(c.note.id);
      api.trashPurge(c.note.id).then(() => show("Deleted for good"), fail);
    } else {
      setSelectedId(null);
      api
        // Only what the user saw: up to the newest deletion in the list.
        .trashEmpty(list[0]?.deleted_at ?? 0)
        .then((n) => show(`Deleted ${n} ${n === 1 ? "note" : "notes"} for good`), fail);
    }
  };

  const keys = useRef<(e: KeyboardEvent) => void>(() => undefined);
  keys.current = (e: KeyboardEvent) => {
    if (e.isComposing || e.metaKey || e.ctrlKey || e.altKey) return;
    if (document.querySelector("dialog[open]") || isTyping(e.target)) return;
    const current = index >= 0 ? list[index] : undefined;
    const move = (delta: number) => {
      e.preventDefault();
      const next = list[index < 0 ? (delta > 0 ? 0 : list.length - 1) : index + delta] ?? current;
      if (next) setSelectedId(next.note.id);
    };
    if (e.key === "ArrowDown" || e.key === "j") return move(1);
    if (e.key === "ArrowUp" || e.key === "k") return move(-1);
    if (!current) return;
    if (e.key.toLowerCase() === "r" || e.key === "Enter") {
      if (e.key === "Enter" && (e.target as HTMLElement | null)?.closest?.("button")) return;
      e.preventDefault();
      restore(current.note);
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      setConfirm({ kind: "one", note: current.note });
    }
  };
  useEffect(() => {
    const key = (e: KeyboardEvent) => keys.current(e);
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);

  if (items === null) return null;

  // Keyboard selection can move past the visible rows: keep it on screen, so
  // Delete never acts on a row the user cannot see.
  function scrollToSelected(el: HTMLLIElement | null) {
    el?.scrollIntoView?.({ block: "nearest" });
  }

  const now = Date.now();
  const groups = groupByDeletedDay(list, now);
  const plural = (n: number) => `${n} ${n === 1 ? "note" : "notes"}`;

  return (
    <>
      {list.length === 0 ? (
        <EmptyState title="Trash is empty">
          Deleted notes wait here for 30 days before they are erased for good.
        </EmptyState>
      ) : (
        <>
          <div className="trash__bar">
            <p className="trash__note">Deleted notes are erased for good after 30 days.</p>
            <Button variant="quiet" onClick={() => setConfirm({ kind: "all" })}>
              Empty Trash
            </Button>
          </div>
          {groups.map((g) => (
            <section key={g.key} className="day" aria-label={g.label}>
              <h2 className="day__label">
                {g.label}
                <span className="day__count">{g.items.length}</span>
              </h2>
              <ul className="notes">
                {g.items.map(({ note, deleted_at }) => (
                  <li
                    key={note.id}
                    ref={note.id === selectedId ? scrollToSelected : undefined}
                    className="note note--trash"
                    data-selected={note.id === selectedId}
                    aria-current={note.id === selectedId || undefined}
                    onClick={() => setSelectedId(note.id)}
                  >
                    <div className="note__body">
                      <p className="note__content selectable" data-clamped="true">
                        {note.content}
                      </p>
                      <div className="note__meta">
                        <span className="trash__returns">{returnsTo(note)}</span>
                        <span>{erasedOn(deleted_at, TRASH_RETENTION_MS)}</span>
                        <NoteBadges note={note} />
                      </div>
                    </div>
                    <div className="note__actions" onClick={(e) => e.stopPropagation()}>
                      <IconButton
                        icon="restore"
                        label="Restore (R)"
                        onClick={() => restore(note)}
                      />
                      <IconButton
                        icon="trash"
                        label="Delete forever (Del)"
                        danger
                        onClick={() => setConfirm({ kind: "one", note })}
                      />
                    </div>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </>
      )}

      <Dialog
        open={confirm?.kind === "one"}
        onClose={() => setConfirm(null)}
        title="Delete this note for good?"
        actions={
          <>
            <Button autoFocus onClick={() => setConfirm(null)}>
              Cancel
            </Button>
            <Button variant="danger" onClick={erase}>
              Delete forever
            </Button>
          </>
        }
      >
        {confirm?.kind === "one" && (
          <p className="dialog__quote selectable">{excerpt(confirm.note.content)}</p>
        )}
        <p className="dialog__text">It will be deleted with its files and can't be restored.</p>
      </Dialog>
      <Dialog
        open={confirm?.kind === "all"}
        onClose={() => setConfirm(null)}
        title="Empty the trash?"
        actions={
          <>
            <Button autoFocus onClick={() => setConfirm(null)}>
              Cancel
            </Button>
            <Button variant="danger" onClick={erase}>
              Empty Trash
            </Button>
          </>
        }
      >
        <p className="dialog__text">
          {plural(list.length)} will be deleted with their files and can't be restored.
        </p>
      </Dialog>
    </>
  );
}
