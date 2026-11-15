"use client";

// The Today window, live: today's notes and the unfinished ones from before,
// fed by whatever the visitor captured in the hero. J/K or arrows move,
// X ticks a note off, like the app.

import { useEffect, useState } from "react";
import { useSite } from "./SiteState";
import { Keys } from "./os";
import { todayView, type DemoNote } from "@/lib/demo";

/** The big count beside the window. */
export function TodayCount() {
  const { demo } = useSite();
  const { done, total } = todayView(demo);
  return (
    <p className="today-count" aria-live="polite">
      <span className="today-count__num">{done}</span>
      <span className="today-count__of"> of {total} done</span>
    </p>
  );
}

function Row({
  note,
  selected,
  onToggle,
  onSelect,
}: {
  note: DemoNote;
  selected: boolean;
  onToggle: () => void;
  onSelect: () => void;
}) {
  return (
    <li
      className="glance__row"
      data-done={note.done}
      aria-current={selected || undefined}
      onClick={onSelect}
    >
      <button
        type="button"
        role="checkbox"
        aria-checked={note.done}
        aria-label={note.done ? `Mark "${note.text}" as not done` : `Mark "${note.text}" as done`}
        className="glance__check"
        tabIndex={-1}
        onClick={(e) => {
          e.stopPropagation();
          onToggle();
        }}
      />
      <span className="glance__text">{note.text}</span>
    </li>
  );
}

export function TodayDemo() {
  const { demo, dispatch } = useSite();
  const view = todayView(demo);
  const order = [...view.carried, ...view.today];
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const selected = order.find((n) => n.id === selectedId) ?? order[0];
  const index = selected ? order.indexOf(selected) : -1;

  const move = (by: number) => {
    const next = order[Math.min(order.length - 1, Math.max(0, index + by))];
    if (next) setSelectedId(next.id);
  };

  // The visitor's own date, filled in after load: the static HTML was
  // built on another day.
  const [date, setDate] = useState("Today");
  useEffect(() => {
    setDate(
      new Intl.DateTimeFormat("en", { weekday: "long", month: "long", day: "numeric" }).format(
        new Date(),
      ),
    );
  }, []);

  const row = (n: DemoNote) => (
    <Row
      key={n.id}
      note={n}
      selected={n.id === selected?.id}
      onSelect={() => setSelectedId(n.id)}
      onToggle={() => {
        setSelectedId(n.id);
        dispatch({ type: "toggle", id: n.id });
      }}
    />
  );

  return (
    <div
      className="glance"
      tabIndex={0}
      aria-label="Today window demo. J and K move, X marks the selected note done."
      onKeyDown={(e) => {
        const k = e.key.toLowerCase();
        if (k === "j" || e.key === "ArrowDown") move(1);
        else if (k === "k" || e.key === "ArrowUp") move(-1);
        else if (k === "x" && selected) dispatch({ type: "toggle", id: selected.id });
        else return;
        e.preventDefault();
      }}
    >
      <div className="glance__head">
        <span className="glance__date">{date}</span>
        <span className="glance__count">
          {view.total > 0 && view.done === view.total
            ? "All done"
            : `${view.done} of ${view.total} done`}
        </span>
        <progress className="glance__bar" value={view.done} max={Math.max(1, view.total)}>
          {view.done} of {view.total}
        </progress>
      </div>
      {view.carried.length > 0 && (
        <>
          <p className="glance__group">Carried over</p>
          <ul className="glance__list">{view.carried.map(row)}</ul>
        </>
      )}
      <p className="glance__group">Today</p>
      <ul className="glance__list">{view.today.map(row)}</ul>
      <p className="glance__keys" aria-hidden="true">
        <Keys keys={["J"]} />
        <Keys keys={["K"]} /> move <Keys keys={["X"]} /> done <Keys keys={["esc"]} /> close
      </p>
    </div>
  );
}
