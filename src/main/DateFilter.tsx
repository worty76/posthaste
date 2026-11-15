import { useCallback, useEffect, useRef, useState } from "react";
import { Calendar } from "../components/Calendar";
import { Icon } from "../components/Icon";
import { dayKey, dayRangeFromInput } from "../lib/dates";
import { api, on } from "../lib/ipc";
import type { DayMark } from "../lib/types";

/** The inbox's "Captured on" filter: a button that opens a calendar showing
 * which days have notes. `value` is `YYYY-MM-DD` or "". */
export function DateFilter({
  value,
  onChange,
}: {
  value: string;
  onChange: (day: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [marks, setMarks] = useState<Record<string, DayMark>>({});
  const edges = useRef<number[]>([]);
  const button = useRef<HTMLButtonElement>(null);

  const loadMarks = useCallback(() => {
    const e = edges.current;
    if (e.length < 2) return;
    api.notesDayMarks(e).then(
      (list) => {
        const next: Record<string, DayMark> = {};
        list.forEach((m, i) => {
          if (m.total > 0) next[dayKey(e[i]!)] = m;
        });
        setMarks(next);
      },
      () => setMarks({}),
    );
  }, []);

  useEffect(() => (open ? on("notes-changed", loadMarks) : undefined), [open, loadMarks]);

  const close = () => {
    setOpen(false);
    button.current?.focus();
  };
  const pick = (day: string) => {
    onChange(day);
    close();
  };

  const range = value ? dayRangeFromInput(value) : null;
  const label = range
    ? new Intl.DateTimeFormat(undefined, {
        weekday: "short",
        day: "numeric",
        month: "short",
        year: "numeric",
      }).format(range.from)
    : "Captured on";

  return (
    <div className="datefilter">
      <button
        ref={button}
        type="button"
        className="datefilter__button field"
        aria-label={range ? `Captured on ${label}` : "Captured on"}
        aria-expanded={open}
        aria-haspopup="dialog"
        title="Show notes captured on a day"
        data-empty={!range || undefined}
        // While open, a press here toggles; it is not an "outside" click.
        onMouseDown={(e) => open && e.stopPropagation()}
        onClick={() => setOpen((v) => !v)}
      >
        <Icon name="calendar" />
        {label}
      </button>
      {open && (
        <div className="datefilter__pop">
          <Calendar
            value={value}
            marks={marks}
            onMonthChange={(e) => {
              edges.current = e;
              loadMarks();
            }}
            onPick={pick}
            onClear={value ? () => pick("") : undefined}
            onClose={close}
          />
        </div>
      )}
    </div>
  );
}
