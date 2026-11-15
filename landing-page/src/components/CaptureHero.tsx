"use client";

// The hero is the capture window. Type, press Enter, and the note is saved
// with a hard cut and timed from your first keystroke. Esc clears.

import { useEffect, useRef } from "react";
import { useSite } from "./SiteState";
import { Keys, OsKeys, useBrowserName } from "./os";
import { MAX_NOTE_CHARS, formatSeconds, savedStack } from "@/lib/demo";

export function CaptureHero() {
  const { demo, dispatch } = useSite();
  const browser = useBrowserName();
  const input = useRef<HTMLInputElement>(null);
  const stack = savedStack(demo);

  // Focused on arrival, like the real window, but only where there is a
  // keyboard: on a phone it would throw up the on-screen keyboard unasked.
  useEffect(() => {
    if (window.matchMedia("(hover: hover) and (pointer: fine)").matches && !location.hash) {
      input.current?.focus({ preventScroll: true });
    }
  }, []);

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
          onChange={(e) =>
            dispatch({ type: "input", text: e.target.value, now: performance.now() })
          }
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
            This is a demo: nothing here is stored or sent. In the app, <OsKeys which="capture" />{" "}
            opens this window from anywhere and Enter writes the note to a file on your computer.
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
