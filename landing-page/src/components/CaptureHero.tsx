"use client";

// The hero is the capture window. Type, press Enter, and the note is saved
// with a hard cut and timed from your first keystroke. Esc clears.

import { useCallback, useEffect, useRef } from "react";
import { useSite } from "./SiteState";
import { Keys, useBrowserName } from "./os";
import {
  DEMO_START_DELAY,
  MAX_NOTE_CHARS,
  demoScript,
  formatSeconds,
  nextCycleDelay,
  savedStack,
} from "@/lib/demo";

export function CaptureHero() {
  const { demo, dispatch, copy, lang } = useSite();
  const browser = useBrowserName();
  const input = useRef<HTMLInputElement>(null);
  const stack = savedStack(demo);
  // Cancels the self-playing script; assigned while it runs, always safe
  // to call.
  const stopScript = useRef<() => void>(() => {});

  // Focused on arrival, like the real window, but only where there is a
  // keyboard: on a phone it would throw up the on-screen keyboard unasked.
  useEffect(() => {
    if (window.matchMedia("(hover: hover) and (pointer: fine)").matches && !location.hash) {
      input.current?.focus({ preventScroll: true });
    }
  }, []);

  // Plays the capture loop on repeat: types two notes, saves each, holds
  // the stack, clears, and starts again. The first real interaction — a
  // click into the field, a keystroke or paste in it, leaving the tab —
  // stops it for good; the script never fights a person.
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches || location.hash) {
      return;
    }
    let cancelled = false;
    const timers = new Set<number>();
    const at = (ms: number, run: () => void) => {
      const id = window.setTimeout(() => {
        timers.delete(id);
        if (!cancelled) run();
      }, ms);
      timers.add(id);
    };
    const stop = () => {
      cancelled = true;
      for (const t of timers) window.clearTimeout(t);
      timers.clear();
    };
    stopScript.current = stop;

    const runCycle = (delay: number) => {
      const steps = demoScript(copy.demo.notes);
      for (const step of steps) {
        at(step.at + delay, () => {
          dispatch({
            type: step.save ? "save" : "input",
            text: step.text,
            now: performance.now(),
          });
        });
      }
      // A whole cycle ran with nobody touching anything: clear its notes
      // and play again.
      at(nextCycleDelay(steps) + delay, () => {
        dispatch({ type: "reset" });
        runCycle(DEMO_START_DELAY);
      });
    };
    runCycle(DEMO_START_DELAY);

    const onKey = (e: KeyboardEvent) => {
      if (!e.metaKey && !e.ctrlKey && !e.altKey) stop();
    };
    const onVisibility = () => {
      if (document.hidden) stop();
    };
    window.addEventListener("keydown", onKey);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      stop();
      stopScript.current = () => {};
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [copy.demo.notes, lang, dispatch]);

  // A real edit in the field — typed, pasted, autofilled — takes over.
  const onUserInput = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      stopScript.current();
      dispatch({ type: "input", text: e.target.value, now: performance.now() });
    },
    [dispatch],
  );

  return (
    <div className="capture">
      <div className="capture__panel">
        <div className="capture__head" aria-hidden="true">
          <span className="capture__mark" />
          <span className="capture__name">Posthaste</span>
          <span className="capture__source">from {browser}</span>
        </div>
        <label className="visually-hidden" htmlFor="capture-input">
          Type a note, then press Enter to save it
        </label>
        <input
          id="capture-input"
          ref={input}
          className="capture__input"
          type="text"
          autoComplete="off"
          spellCheck={false}
          maxLength={MAX_NOTE_CHARS}
          placeholder="What's on your mind?"
          value={demo.draft}
          onMouseDown={() => stopScript.current()}
          onChange={onUserInput}
          onKeyDown={(e) => {
            if (e.nativeEvent.isComposing) return;
            if (e.key === "Enter") {
              e.preventDefault();
              dispatch({ type: "save", now: performance.now() });
            } else if (e.key === "Escape" && demo.draft !== "") {
              e.preventDefault();
              dispatch({ type: "clear" });
            }
          }}
        />
        <div className="capture__foot">
          <span className="capture__status" role="status" aria-live="polite">
            {demo.lastSavedMs !== null && demo.draft === ""
              ? `Saved in ${formatSeconds(demo.lastSavedMs)}.`
              : ""}
          </span>
          <span className="capture__hint" aria-hidden="true">
            <Keys keys={["Enter"]} /> save <Keys keys={["esc"]} /> clear
          </span>
        </div>
      </div>

      <ol className="stack" aria-label="Notes you saved in this demo">
        {stack.length === 0 ? (
          <li className="stack__empty">
            This is a demo: nothing here is stored or sent. In the app, a shortcut opens this window
            from anywhere and Enter writes the note to a file on your computer.
          </li>
        ) : (
          stack.map((n) => (
            <li className="stack__note" key={n.id}>
              <span className="stack__time">{formatSeconds(n.savedInMs ?? 0)}</span>
              <span className="stack__text">{n.text}</span>
            </li>
          ))
        )}
      </ol>
    </div>
  );
}
