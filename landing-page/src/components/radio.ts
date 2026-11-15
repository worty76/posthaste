import type { KeyboardEvent } from "react";
import { LOOKS } from "@/lib/looks";

/** Arrow keys move through a radiogroup of looks, as the ARIA pattern
    expects; only the checked radio is in the tab order. */
export function lookRadioKeys(
  e: KeyboardEvent<HTMLElement>,
  current: string,
  set: (id: string) => void,
) {
  const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
  if (!step) return;
  e.preventDefault();
  const i = LOOKS.findIndex((l) => l.id === current);
  const next = LOOKS[(i + step + LOOKS.length) % LOOKS.length];
  if (!next) return;
  set(next.id);
  const group = e.currentTarget;
  requestAnimationFrame(() =>
    group.querySelector<HTMLElement>(`[data-look="${next.id}"]`)?.focus(),
  );
}
