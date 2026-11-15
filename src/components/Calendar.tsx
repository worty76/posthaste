import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { dayKey, dayRangeFromInput, monthEdges, monthGrid, weekStart } from "../lib/dates";
import type { DayMark } from "../lib/types";
import { IconButton } from "./Button";
import { Kbd } from "./Kbd";

interface CalendarProps {
  /** Selected day as `YYYY-MM-DD`, or "" for none. */
  value: string;
  /** Notes per day, keyed `YYYY-MM-DD`. Days not listed have none. */
  marks: Record<string, DayMark>;
  /** Last day that can be picked, `YYYY-MM-DD`. */
  max?: string;
  weekStartsOn?: 0 | 1 | 6;
  /** Called with the visible grid's day edges whenever the month changes. */
  onMonthChange?: (edges: number[]) => void;
  onPick: (day: string) => void;
  onClear?: () => void;
  onClose: () => void;
}

/** Moves a `YYYY-MM-DD` key by days or months, in local calendar terms. */
function shift(key: string, days: number, months = 0): string {
  const from = dayRangeFromInput(key)?.from ?? Date.now();
  const d = new Date(from);
  if (months !== 0) {
    const day = d.getDate();
    d.setDate(1);
    d.setMonth(d.getMonth() + months);
    const last = new Date(d.getFullYear(), d.getMonth() + 1, 0).getDate();
    d.setDate(Math.min(day, last));
  }
  d.setDate(d.getDate() + days);
  return dayKey(d.getTime());
}

const monthOf = (key: string) => key.slice(0, 7);

/** A one-month day picker that shows which days have notes: a filled dot
 * when some are still open, a hollow one when all of them are done. */
export function Calendar({
  value,
  marks,
  max,
  weekStartsOn,
  onMonthChange,
  onPick,
  onClear,
  onClose,
}: CalendarProps) {
  const today = dayKey(Date.now());
  const [focused, setFocused] = useState(value || today);
  const rootRef = useRef<HTMLDialogElement>(null);
  const firstDay = useMemo(() => weekStartsOn ?? weekStart(), [weekStartsOn]);

  const year = Number(focused.slice(0, 4));
  const month = Number(focused.slice(5, 7)) - 1;
  const cells = useMemo(() => monthGrid(year, month, firstDay), [year, month, firstDay]);
  const monthKey = monthOf(focused);

  const reportMonth = useRef(onMonthChange);
  reportMonth.current = onMonthChange;
  useEffect(() => {
    reportMonth.current?.(monthEdges(cells));
  }, [cells]);

  useEffect(() => {
    rootRef.current?.querySelector<HTMLElement>(`[data-day="${focused}"]`)?.focus();
  }, [focused]);

  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  useEffect(() => {
    const outside = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) onCloseRef.current();
    };
    document.addEventListener("mousedown", outside);
    return () => document.removeEventListener("mousedown", outside);
  }, []);

  const allowed = (key: string) => !max || key <= max;
  const go = (key: string) => {
    if (allowed(key)) setFocused(key);
    else if (max && monthOf(max) >= monthOf(key)) setFocused(max);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.nativeEvent.isComposing || e.metaKey || e.ctrlKey || e.altKey) return;
    const moves: Record<string, () => string> = {
      ArrowLeft: () => shift(focused, -1),
      ArrowRight: () => shift(focused, 1),
      ArrowUp: () => shift(focused, -7),
      ArrowDown: () => shift(focused, 7),
      PageUp: () => shift(focused, 0, -1),
      PageDown: () => shift(focused, 0, 1),
    };
    const move = moves[e.key];
    if (move) {
      e.preventDefault();
      e.stopPropagation();
      go(move());
    } else if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      if (allowed(focused)) onPick(focused);
    } else if (e.key.toLowerCase() === "t") {
      e.preventDefault();
      e.stopPropagation();
      onPick(today);
    }
  };

  // Keys stay inside: the window behind has its own single-key shortcuts.
  const onRootKeyDown = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape" && !e.nativeEvent.isComposing) {
      e.preventDefault();
      onClose();
    }
  };

  const monthLabel = new Intl.DateTimeFormat(undefined, { month: "long", year: "numeric" }).format(
    new Date(year, month, 1),
  );
  const weekdays = cells.slice(0, 7).map((c) => ({
    short: new Intl.DateTimeFormat(undefined, { weekday: "narrow" }).format(c.t),
    long: new Intl.DateTimeFormat(undefined, { weekday: "long" }).format(c.t),
  }));
  const fullDate = new Intl.DateTimeFormat(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: "numeric",
  });
  const nextMonthAllowed = !max || monthOf(shift(focused, 0, 1)) <= monthOf(max);

  return (
    // Non-modal: it sits beside its button and closes on an outside click.
    <dialog
      ref={rootRef}
      open
      className="calendar"
      aria-label="Choose a day"
      onKeyDown={onRootKeyDown}
    >
      <div className="calendar__head">
        <IconButton icon="prev" label="Previous month" onClick={() => go(shift(focused, 0, -1))} />
        <p className="calendar__month" aria-live="polite">
          {monthLabel}
        </p>
        <IconButton
          icon="next"
          label="Next month"
          disabled={!nextMonthAllowed}
          onClick={() => go(shift(focused, 0, 1))}
        />
      </div>
      <div className="calendar__grid" role="grid" aria-label={monthLabel} onKeyDown={onKeyDown}>
        <div className="calendar__row" role="row">
          {weekdays.map((w) => (
            <span key={w.long} className="calendar__weekday" role="columnheader" title={w.long}>
              {w.short}
            </span>
          ))}
        </div>
        {[0, 1, 2, 3, 4, 5].map((week) => (
          <div key={week} className="calendar__row" role="row">
            {cells.slice(week * 7, week * 7 + 7).map((c) => {
              const mark = marks[c.key];
              const state = !mark || mark.total === 0 ? undefined : mark.open > 0 ? "open" : "done";
              const note =
                state === "open" ? `, ${mark!.open} open` : state === "done" ? ", all done" : "";
              return (
                <button
                  key={c.key}
                  type="button"
                  role="gridcell"
                  className="calendar__day"
                  data-day={c.key}
                  data-outside={!c.inMonth || undefined}
                  data-today={c.key === today || undefined}
                  data-mark={state}
                  aria-selected={c.key === value}
                  aria-label={fullDate.format(c.t) + note}
                  tabIndex={c.key === focused ? 0 : -1}
                  disabled={!allowed(c.key)}
                  onClick={() => onPick(c.key)}
                  onFocus={() => {
                    if (c.key !== focused && monthOf(c.key) === monthKey) setFocused(c.key);
                  }}
                >
                  {Number(c.key.slice(8))}
                </button>
              );
            })}
          </div>
        ))}
      </div>
      <div className="calendar__foot">
        <button type="button" className="calendar__link" onClick={() => onPick(today)}>
          Today
        </button>
        {onClear && (
          <button type="button" className="calendar__link" onClick={onClear}>
            Clear
          </button>
        )}
      </div>
      <p className="calendar__hints">
        <Kbd keys={["←", "→", "↑", "↓"]} /> day <Kbd keys={["PgUp", "PgDn"]} /> month{" "}
        <Kbd keys={["T"]} /> today
      </p>
    </dialog>
  );
}
