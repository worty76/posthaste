"use client";

// Six tiles, each wearing its own look (the data-look attribute scopes the
// tokens). Pressing one puts the whole page in it.

import { useSite } from "./SiteState";
import { lookRadioKeys } from "./radio";
import { LOOKS } from "@/lib/looks";

const SAMPLE: Record<string, string> = {
  "strawberry-milk": "Smoothie: banana, oat milk",
  matcha: "Call grandma on Sunday",
  "lavender-haze": "Sticker ideas for the zine",
  "midnight-grape": "Draft the demo script",
  blueberry: "Finish chapter four",
  ink: "Gather receipts, ugh",
};

export function LookTiles() {
  const { lookId, setLook } = useSite();
  return (
    <div
      className="tiles"
      role="radiogroup"
      aria-label="Look gallery"
      onKeyDown={(e) => lookRadioKeys(e, lookId, setLook)}
    >
      {LOOKS.map((look, i) => (
        <button
          key={look.id}
          type="button"
          role="radio"
          aria-checked={look.id === lookId}
          tabIndex={look.id === lookId ? 0 : -1}
          className="tile"
          data-look={look.id}
          onClick={() => setLook(look.id)}
        >
          <span className="tile__panel" aria-hidden="true">
            <span className="tile__mark" />
            <span className="tile__note">{SAMPLE[look.id]}</span>
          </span>
          <span className="tile__name">
            {look.name}
            <span className="tile__key">{i + 1}</span>
          </span>
        </button>
      ))}
    </div>
  );
}
