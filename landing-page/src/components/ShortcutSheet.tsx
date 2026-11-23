"use client";

// The app's keys, grouped by where you are. Global shortcuts follow the
// visitor's system; the toggle shows the others. Facts from the README.

import { useState } from "react";
import { Keys, OS_LABEL, SHORTCUTS, useOs, type Os } from "./os";

type Row = [label: string, keys: (os: Os) => string[]];

const mod = (os: Os) => (os === "mac" ? "⌘" : "Ctrl");

const GROUPS: { title: string; rows: Row[] }[] = [
  {
    title: "From any app",
    rows: [
      ["Write a note", (os) => SHORTCUTS[os].capture],
      ["See today", (os) => SHORTCUTS[os].today],
      ["Open the inbox", (os) => SHORTCUTS[os].inbox],
    ],
  },
  {
    title: "While writing",
    rows: [
      ["Save and close", () => ["Enter"]],
      ["New line", () => ["Shift", "Enter"]],
      ["Save, write another", (os) => [mod(os), "Enter"]],
      ["Pick project", (os) => [mod(os), "P"]],
      ["Polish with AI", (os) => [mod(os), "I"]],
      ["Close, keep draft", () => ["esc"]],
    ],
  },
  {
    title: "In the inbox",
    rows: [
      ["Move", () => ["↑", "↓"]],
      ["Edit / Details", () => ["E", "O"]],
      ["Done / Later", () => ["X", "L"]],
      ["Set project", () => ["P"]],
      ["Search", () => ["/"]],
      ["All shortcuts", () => ["?"]],
    ],
  },
  {
    title: "In note details",
    rows: [
      ["Open from Today", () => ["Enter"]],
      ["Add files", (os) => [mod(os), "O"]],
      ["Mark done", (os) => [mod(os), "Enter"]],
      ["Save and close", () => ["esc"]],
    ],
  },
];

/** Three columns; the short first column carries the note-details keys. */
const COLUMNS = [["From any app", "In note details"], ["While writing"], ["In the inbox"]];

const ORDER: Os[] = ["mac", "windows", "linux"];

export function ShortcutSheet() {
  const detected = useOs();
  const [picked, setPicked] = useState<Os | null>(null);
  const os = picked ?? detected ?? "mac";

  return (
    <div className="sheet">
      <div className="sheet__os" role="group" aria-label="Show keys for">
        {ORDER.map((o) => (
          <button
            key={o}
            type="button"
            className="sheet__os-btn"
            aria-pressed={o === os}
            onClick={() => setPicked(o)}
          >
            {OS_LABEL[o]}
          </button>
        ))}
      </div>
      <div className="sheet__groups">
        {COLUMNS.map((titles) => (
          <div className="sheet__col" key={titles.join("+")}>
            {titles
              .map((t) => GROUPS.find((g) => g.title === t)!)
              .map((g) => (
                <div className="sheet__group" key={g.title}>
                  <h3 className="sheet__title">{g.title}</h3>
                  <dl className="sheet__rows">
                    {g.rows.map(([label, keys]) => (
                      <div className="sheet__row" key={label}>
                        <dt>{label}</dt>
                        <dd>
                          <Keys keys={keys(os)} />
                        </dd>
                      </div>
                    ))}
                  </dl>
                </div>
              ))}
          </div>
        ))}
      </div>
    </div>
  );
}
