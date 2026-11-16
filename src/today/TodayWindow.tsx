import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { IconButton } from "../components/Button";
import { EmptyState } from "../components/EmptyState";
import { NoteBadges } from "../details/NoteBadges";
import { Icon } from "../components/Icon";
import { Kbd } from "../components/Kbd";
import { useAppearance } from "../hooks/useAppearance";
import { useSettings } from "../hooks/useSettings";
import { Calendar } from "../components/Calendar";
import { dayKey, dayRange, dayRangeFromInput, originLabel, startOfDay } from "../lib/dates";
import { api, on, toAppError } from "../lib/ipc";
import { formatAccelerator } from "../lib/shortcut";
import type { DayMark, Glance, Note, Os } from "../lib/types";

export function TodayWindow({ os }: { os: Os }) {
  const { settings, shortcuts } = useSettings();
  useAppearance(settings);

  const [glance, setGlance] = useState<Glance | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  /** The day being looked at: null follows today, across midnight too. */
  const [viewDay, setViewDay] = useState<number | null>(null);
  const viewDayRef = useRef<number | null>(null);
  viewDayRef.current = viewDay;
  const [calendarOpen, setCalendarOpen] = useState(false);
  const [marks, setMarks] = useState<Record<string, DayMark>>({});

  /** Refetches with the viewed day's bounds; when following today they are
   * recomputed, so the window is never stale across midnight. `reset` drops
   * the selection back to the first row. */
  const load = useCallback(async (reset: boolean) => {
    const { from, to } = dayRange(viewDayRef.current ?? Date.now());
    try {
      const g = await api.glanceGet(from, to);
      setGlance(g);
      setError(null);
      setSelectedId((current) => {
        if (!reset) {
          const rows = [...g.carried, ...g.today];
          if (current && rows.some((n) => n.id === current)) return current;
        }
        return g.carried[0]?.id ?? g.today[0]?.id ?? null;
      });
    } catch (e) {
      setError(toAppError(e).message);
    }
  }, []);

  useEffect(() => {
    void load(false);
    listRef.current?.focus();
  }, [load]);

  // Shown again by the shortcut: today's bounds may have moved, and the
  // user's eyes start at the top.
  useEffect(
    () =>
      on("today-shown", () => {
        viewDayRef.current = null;
        setViewDay(null);
        setCalendarOpen(false);
        void load(true);
        listRef.current?.focus();
      }),
    [load],
  );
  // Back from Note details: same day, same selection, eyes on the list.
  useEffect(
    () =>
      on("today-returned", () => {
        void load(false);
        listRef.current?.focus();
      }),
    [load],
  );
  // The OS can give the window focus after the page asked for it (coming
  // back from Note details): put focus back on the list so keys work.
  useEffect(() => {
    const focus = () => {
      if (!document.activeElement || document.activeElement === document.body) {
        listRef.current?.focus();
      }
    };
    window.addEventListener("focus", focus);
    return () => window.removeEventListener("focus", focus);
  }, []);
  // Notes changed elsewhere (capture, inbox): refresh, keep the selection.
  useEffect(() => on("notes-changed", () => void load(false)), [load]);

  /** Shows a day, or today for null. Days after today are refused. */
  const showDay = useCallback(
    (day: number | null) => {
      const today = startOfDay(Date.now());
      const next = day === null || startOfDay(day) >= today ? null : startOfDay(day);
      if (next === viewDayRef.current) return;
      viewDayRef.current = next;
      setViewDay(next);
      void load(true);
    },
    [load],
  );
  const shownDay = viewDay ?? startOfDay(Date.now());
  const isToday = viewDay === null;
  const dayEnd = dayRange(shownDay).to;
  const stepDay = (delta: number) => {
    const d = new Date(shownDay);
    d.setDate(d.getDate() + delta);
    showDay(d.getTime());
  };
  /** Done as of the end of the viewed day; today, simply the status. */
  const isDone = (note: Note) =>
    isToday
      ? note.status === "completed"
      : note.completed_at !== null && note.completed_at < dayEnd;

  const loadMarks = (edges: number[]) => {
    if (edges.length < 2) return;
    api.notesDayMarks(edges).then(
      (list) => {
        const next: Record<string, DayMark> = {};
        list.forEach((m, i) => {
          if (m.total > 0) next[dayKey(edges[i]!)] = m;
        });
        setMarks(next);
      },
      () => setMarks({}),
    );
  };
  const closeCalendar = () => {
    setCalendarOpen(false);
    listRef.current?.focus();
  };

  const rows = useMemo(() => (glance ? [...glance.carried, ...glance.today] : []), [glance]);
  const selectedIndex = rows.findIndex((n) => n.id === selectedId);
  const selected = selectedIndex >= 0 ? rows[selectedIndex] : undefined;

  const toggle = (note: Note) => {
    const status = note.status === "completed" ? "inbox" : "completed";
    void api.noteSetStatus(note.id, status).then(
      () => undefined,
      (e) => setError(toAppError(e).message),
    );
  };

  /** Parks the note in Later and keeps the selection on a neighbour, so the
   * next key acts on the row the eye is already on. */
  const sendToLater = (note: Note) => {
    const i = rows.findIndex((n) => n.id === note.id);
    const neighbour = rows[i + 1] ?? rows[i - 1] ?? null;
    void api.noteSetLater(note.id, true).then(
      () => {
        setSelectedId(neighbour?.id ?? null);
        void load(false);
      },
      (e) => setError(toAppError(e).message),
    );
  };

  /** The inbox, filtered to the day the note was captured. */
  const showCapturedDay = (note: Note) => {
    void api.inboxOpenDate(dayKey(note.created_at)).catch((e) => setError(toAppError(e).message));
  };

  const openDetails = (note: Note) => {
    setSelectedId(note.id);
    void api.detailsOpen(note.id, "today").catch((e) => setError(toAppError(e).message));
  };

  const onKeyDown = (e: globalThis.KeyboardEvent) => {
    if (e.isComposing) return;
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    // The calendar handles its own keys; only Esc reaches here to close it.
    if (calendarOpen) {
      if (e.key === "Escape") {
        e.preventDefault();
        closeCalendar();
      }
      return;
    }
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      return stepDay(-1);
    }
    if (e.key === "ArrowRight") {
      e.preventDefault();
      return stepDay(1);
    }
    if (e.key.toLowerCase() === "t") {
      e.preventDefault();
      return showDay(null);
    }
    if (e.key.toLowerCase() === "c") {
      e.preventDefault();
      setCalendarOpen(true);
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      void api.todayDismiss().catch(() => undefined);
      return;
    }
    const move = (delta: number) => {
      e.preventDefault();
      const next = rows[selectedIndex < 0 ? 0 : selectedIndex + delta] ?? selected;
      if (next) setSelectedId(next.id);
    };
    if (e.key === "ArrowDown" || e.key === "j") return move(1);
    if (e.key === "ArrowUp" || e.key === "k") return move(-1);
    if (e.key.toLowerCase() === "x" && selected && isToday) {
      e.preventDefault();
      toggle(selected);
    }
    if (e.key.toLowerCase() === "l" && selected && isToday) {
      e.preventDefault();
      sendToLater(selected);
    }
    if (e.key === "Enter" && selected) {
      // Buttons handle their own Enter.
      if ((e.target as HTMLElement | null)?.closest?.("button")) return;
      e.preventDefault();
      openDetails(selected);
    }
  };

  const sameYear = new Date(shownDay).getFullYear() === new Date(Date.now()).getFullYear();
  // On the window, not the panel: focus can fall to the page when the
  // focused button or list goes away (Back to today, an empty day), and Esc
  // must still close.
  const keyHandler = useRef(onKeyDown);
  keyHandler.current = onKeyDown;
  useEffect(() => {
    const key = (e: globalThis.KeyboardEvent) => keyHandler.current(e);
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);

  // A past day shares the row with "Back to today", so it is written shorter.
  const dateLabel = new Intl.DateTimeFormat(undefined, {
    weekday: isToday ? "long" : "short",
    month: isToday ? "long" : "short",
    day: "numeric",
    year: sameYear ? undefined : "numeric",
  }).format(shownDay);

  const captureKeys =
    shortcuts?.capture.registered && shortcuts.capture.accelerator
      ? formatAccelerator(shortcuts.capture.accelerator, os)
      : null;

  const shown = glance ? glance.carried.length + glance.today.length : 0;
  const more = glance ? glance.total - shown : 0;
  const allDone = glance !== null && glance.total > 0 && glance.done === glance.total;
  const groups = glance
    ? [
        { label: "Carried over", notes: glance.carried },
        { label: isToday ? "Today" : "That day", notes: glance.today },
      ].filter((g) => g.notes.length > 0)
    : [];

  return (
    <div className="today">
      <section className="today__panel" aria-label="Today">
        <header className="today__header" data-tauri-drag-region>
          <div className="today__day">
            <IconButton icon="prev" label="Previous day" onClick={() => stepDay(-1)} />
            <h1 className="today__date">
              <button
                type="button"
                className="today__date-button"
                title="Choose a day (C)"
                aria-haspopup="dialog"
                aria-expanded={calendarOpen}
                onMouseDown={(e) => calendarOpen && e.stopPropagation()}
                onClick={() => (calendarOpen ? closeCalendar() : setCalendarOpen(true))}
              >
                {dateLabel}
              </button>
            </h1>
            <IconButton
              icon="next"
              label="Next day"
              disabled={isToday}
              onClick={() => stepDay(1)}
            />
            {!isToday && (
              <button type="button" className="today__back" onClick={() => showDay(null)}>
                Back to today
              </button>
            )}
          </div>
          {calendarOpen && (
            <div className="today__calendar">
              <Calendar
                value={dayKey(shownDay)}
                marks={marks}
                max={dayKey(Date.now())}
                onMonthChange={loadMarks}
                onPick={(day) => {
                  closeCalendar();
                  const range = dayRangeFromInput(day);
                  if (range) showDay(range.from);
                }}
                onClose={closeCalendar}
              />
            </div>
          )}
          {glance && (
            <>
              <p className="today__count" data-all-done={allDone || undefined}>
                {allDone ? "All done" : `${glance.done} of ${glance.total} done`}
              </p>
              <progress
                className="today__bar"
                value={glance.done}
                max={glance.total}
                aria-label="Notes done"
              />
            </>
          )}
        </header>

        {error ? (
          <p className="today__error" role="alert">
            {error}
          </p>
        ) : glance && glance.total === 0 ? (
          <EmptyState title={isToday ? "Nothing for today" : "Nothing on this day"}>
            {!isToday ? (
              "No notes were open or captured that day."
            ) : captureKeys ? (
              <>
                Press <Kbd keys={captureKeys} /> anywhere to capture one.
              </>
            ) : (
              "Capture one from anywhere with Posthaste."
            )}
          </EmptyState>
        ) : (
          <>
            <div ref={listRef} className="today__list" tabIndex={-1}>
              {groups.map((g) => (
                <section key={g.label} className="today__group" aria-label={g.label}>
                  <h2 className="today__group-label">{g.label}</h2>
                  <ul className="today__rows">
                    {g.notes.map((n) => (
                      <GlanceRow
                        key={n.id}
                        note={n}
                        selected={n.id === selectedId}
                        done={isDone(n)}
                        origin={originLabel(n.created_at, shownDay, Date.now())}
                        onOrigin={showCapturedDay}
                        readOnly={!isToday}
                        onToggle={toggle}
                        onOpen={openDetails}
                      />
                    ))}
                  </ul>
                </section>
              ))}
            </div>
            {more > 0 && (
              <p className="today__more">
                {isToday ? `and ${more} more in the inbox` : `and ${more} more`}
              </p>
            )}
          </>
        )}

        <footer className="today__hints" aria-hidden="true">
          {!isToday && <span className="today__hint today__past">As it was on this day</span>}
          <span className="today__hint">
            <Kbd keys={["↑", "↓"]} />
          </span>
          <span className="today__hint">
            <Kbd keys={["←", "→"]} /> day
          </span>
          {isToday ? (
            <>
              <span className="today__hint">
                <Kbd keys={["X"]} /> done
              </span>
              <span className="today__hint">
                <Kbd keys={["L"]} /> later
              </span>
            </>
          ) : (
            <span className="today__hint">
              <Kbd keys={["T"]} /> today
            </span>
          )}
          <span className="today__hint">
            <Kbd keys={["↵"]} /> details
          </span>
          <span className="today__hint">
            <Kbd keys={["esc"]} /> close
          </span>
        </footer>
      </section>
    </div>
  );
}

function GlanceRow({
  note,
  selected,
  done,
  readOnly,
  origin,
  onOrigin,
  onToggle,
  onOpen,
}: {
  note: Note;
  selected: boolean;
  /** Done as of the viewed day, which on a past day may differ from status. */
  done: boolean;
  readOnly: boolean;
  /** Where it came from, for a note captured before the viewed day. */
  origin: string | null;
  onOrigin: (note: Note) => void;
  onToggle: (note: Note) => void;
  onOpen: (note: Note) => void;
}) {
  const ref = useRef<HTMLLIElement>(null);
  // Moving with the keyboard must never leave the selection off screen.
  useEffect(() => {
    if (selected) ref.current?.scrollIntoView?.({ block: "nearest" });
  }, [selected]);

  return (
    <li
      ref={ref}
      className="today__row"
      data-selected={selected}
      data-status={done ? "completed" : "inbox"}
      aria-current={selected || undefined}
    >
      <button
        type="button"
        role="checkbox"
        className="check"
        aria-checked={done}
        aria-label={
          readOnly
            ? done
              ? "Done by the end of this day"
              : "Not done by the end of this day"
            : done
              ? "Mark as not completed"
              : "Mark as completed"
        }
        disabled={readOnly}
        onClick={() => onToggle(note)}
      >
        <Icon name="check" size={12} />
      </button>
      <p className="today__text selectable">{note.content}</p>
      {origin && (
        <button
          type="button"
          className="today__origin"
          aria-label={`Show notes captured ${origin.split(" · ")[0]}`}
          title="Show the notes captured that day"
          onClick={() => onOrigin(note)}
        >
          {origin}
        </button>
      )}
      <NoteBadges note={note} />
      <IconButton
        icon="open"
        className="today__open"
        label="Open details (Enter)"
        onClick={() => onOpen(note)}
      />
    </li>
  );
}
