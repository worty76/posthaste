"use client";

// The six looks as chips. Picking one re-themes the whole page at once.

import { useSite } from "./SiteState";
import { lookRadioKeys } from "./radio";
import { LOOKS, lookById } from "@/lib/looks";

export function LookChips() {
  const { lookId, setLook } = useSite();

  return (
    <div className="chips">
      <div
        className="chips__row"
        role="radiogroup"
        aria-label="Look"
        onKeyDown={(e) => lookRadioKeys(e, lookId, setLook)}
      >
        {LOOKS.map((look, i) => (
          <button
            key={look.id}
            type="button"
            role="radio"
            aria-checked={look.id === lookId}
            tabIndex={look.id === lookId ? 0 : -1}
            className="chip"
            data-look={look.id}
            title={`${look.name} (${i + 1})`}
            aria-label={look.name}
            onClick={() => setLook(look.id)}
          >
            <span className="chip__dot" />
          </button>
        ))}
      </div>
      <p className="chips__name">
        <span>{lookById(lookId).name}</span>
        <span className="chips__tip">or press 1 to 6</span>
      </p>
    </div>
  );
}
