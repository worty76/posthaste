"use client";

// The hero's background: a faint keyboard whose keys light up as the
// visitor types, anywhere on the page. Decoration only: hidden from
// assistive tech, no pointer events, and still (no fade) with reduced motion.

import { useEffect, useRef } from "react";

const ROWS: string[][] = [
  ["`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "backspace"],
  ["tab", "q", "w", "e", "r", "t", "y", "u", "i", "o", "p", "[", "]", "\\"],
  ["capslock", "a", "s", "d", "f", "g", "h", "j", "k", "l", ";", "'", "enter"],
  ["shift", "z", "x", "c", "v", "b", "n", "m", ",", ".", "/", "shift-r"],
  ["control", "alt", "meta", " ", "meta-r", "alt-r", "arrowleft", "arrowdown", "arrowright"],
];

const LABEL: Record<string, string> = {
  backspace: "⌫",
  tab: "tab",
  capslock: "caps",
  enter: "⏎",
  shift: "⇧",
  "shift-r": "⇧",
  control: "ctrl",
  alt: "alt",
  "alt-r": "alt",
  meta: "⌘",
  "meta-r": "⌘",
  " ": "",
  arrowleft: "←",
  arrowdown: "↓",
  arrowright: "→",
};

/** Width in key units, for the keys that are not 1u. */
const WIDTH: Record<string, number> = {
  backspace: 2,
  tab: 1.5,
  "\\": 1.5,
  capslock: 1.75,
  enter: 2.25,
  shift: 2.25,
  "shift-r": 2.75,
  control: 1.5,
  alt: 1.25,
  meta: 1.5,
  " ": 6,
  "meta-r": 1.5,
  "alt-r": 1.25,
};

/** Which keys a keydown lights; both sides for modifiers. */
function keysFor(e: KeyboardEvent): string[] {
  const k = e.key.toLowerCase();
  if (k === "shift") return ["shift", "shift-r"];
  if (k === "meta") return ["meta", "meta-r"];
  if (k === "alt") return ["alt", "alt-r"];
  if (k === "arrowup") return ["arrowdown"];
  // Shifted characters light the key they live on.
  const code = e.code;
  if (/^Key[A-Z]$/.test(code)) return [code.slice(3).toLowerCase()];
  if (/^Digit\d$/.test(code)) return [code.slice(5)];
  return [k];
}

export function KeyboardBackdrop() {
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const timers = new Map<Element, number>();
    const onKey = (e: KeyboardEvent) => {
      const el = root.current;
      if (!el) return;
      for (const k of keysFor(e)) {
        const key = el.querySelector(`[data-k="${CSS.escape(k)}"]`);
        if (!key) continue;
        key.classList.add("is-lit");
        window.clearTimeout(timers.get(key));
        timers.set(
          key,
          window.setTimeout(() => key.classList.remove("is-lit"), 140),
        );
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      timers.forEach((t) => window.clearTimeout(t));
    };
  }, []);

  return (
    <div className="backdrop" aria-hidden="true">
      <div className="backdrop__board" ref={root}>
        {ROWS.map((row, i) => (
          <div className="backdrop__row" key={i}>
            {row.map((k) => (
              <span
                key={k}
                data-k={k}
                className="backdrop__key"
                data-w={String(WIDTH[k] ?? 1).replace(".", "_")}
              >
                {LABEL[k] ?? k}
              </span>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
