"use client";

// The loop in one row: catch a note, see what's left, tick it off. All three
// read the same demo state as the hero, so what you type above shows up here.

import { useSite } from "./SiteState";
import { Keys, SHORTCUTS, useOs } from "./os";
import { TodayCount, TodayDemo } from "./TodayDemo";
import { doneNotes, savedStack } from "@/lib/demo";

export function Moments() {
  const { demo, dispatch, copy } = useSite();
  const os = useOs() ?? "mac";
  const caught = savedStack(demo).length;
  const done = doneNotes(demo);

  return (
    <section className="page moments" id="loop" aria-labelledby="loop-title">
      <h2 className="h2" id="loop-title">
        {copy.moments.heading}
      </h2>
      <ol className="moments__row">
        <li className="moment">
          <p className="moment__step">{copy.moments.step1}</p>
          <p className="moment__keys">
            <Keys keys={SHORTCUTS[os].capture} />
          </p>
          <p>{copy.moments.step1Desc}</p>
          <p className="quiet" aria-live="polite">
            {caught === 0 ? copy.moments.step1Empty : copy.moments.step1Count(caught)}
          </p>
        </li>
        <li className="moment moment--today">
          <p className="moment__step">{copy.moments.step2}</p>
          <p className="moment__keys">
            <Keys keys={SHORTCUTS[os].today} />
          </p>
          <TodayDemo />
        </li>
        <li className="moment">
          <p className="moment__step">{copy.moments.step3}</p>
          <p className="moment__keys">
            <Keys keys={["X"]} />
          </p>
          <TodayCount />
          {done.length === 0 ? (
            <p className="quiet">{copy.moments.step3Hint}</p>
          ) : (
            <ul className="done-list">
              {done.map((n) => (
                <li key={n.id}>
                  <span className="done-list__text">{n.text}</span>
                  <button
                    type="button"
                    className="link-button"
                    onClick={() => dispatch({ type: "toggle", id: n.id })}
                  >
                    {copy.moments.undo}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </li>
      </ol>
    </section>
  );
}
