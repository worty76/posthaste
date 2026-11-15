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
      ["Close, keep the draft", () => ["esc"]],
      ["Polish with AI, if on", (os) => [mod(os), "I"]],
    ],
  },
  {
    title: "In the inbox",
    rows: [
      ["Move", () => ["J", "K"]],
      ["Edit", () => ["E"]],
      ["Done", () => ["X"]],
      ["Archive", () => ["A"]],
      ["Search", () => ["/"]],
      ["Every shortcut", () => ["?"]],
    ],
  },
];

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
        {GROUPS.map((g) => (
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
    </div>
  );
}
