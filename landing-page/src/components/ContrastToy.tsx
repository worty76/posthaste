"use client";

// "Make your own", shrunk to the page: four colours, a live preview, and
// the app's real check. The wording of a rejection is the app's wording.
// It starts from whichever look the page is wearing.

import { useEffect, useMemo, useState } from "react";
import { useSite } from "./SiteState";
import { lookById } from "@/lib/looks";
import { contrast, parseHex, toHex } from "@/lib/contrast";
import { TEXT_CONTRAST_MIN, ACCENT_FILL_MIN } from "@/lib/derive";

type Colors = { bg: string; surface: string; text: string; accent: string };

const FIELDS: { key: keyof Colors; label: string }[] = [
  { key: "bg", label: "Background" },
  { key: "surface", label: "Surface" },
  { key: "text", label: "Text" },
  { key: "accent", label: "Accent" },
];

export function ContrastToy() {
  const { lookId } = useSite();
  const look = lookById(lookId);
  const [colors, setColors] = useState<Colors>(look.colors);

  useEffect(() => setColors(look.colors), [look]);

  const warnings = useMemo(() => {
    const bg = parseHex(colors.bg);
    const text = parseHex(colors.text);
    const accent = parseHex(colors.accent);
    const out: string[] = [];
    if (bg && text) {
      const ratio = contrast(text, bg);
      if (ratio < TEXT_CONTRAST_MIN) {
        out.push(
          `Text and background are too similar to read comfortably (${ratio.toFixed(1)}:1, needs 4.5:1).`,
        );
      }
    }
    if (bg && accent && contrast(accent, bg) < ACCENT_FILL_MIN) {
      out.push(
        "The accent is almost invisible on the background. Pick a color that stands out more.",
      );
    }
    return out;
  }, [colors]);

  const set = (key: keyof Colors) => (value: string) => {
    const hex = parseHex(value);
    if (hex) setColors((c) => ({ ...c, [key]: toHex(hex) }));
  };

  // Text on the accent: first of text, bg, black, white that reads (the app's rule).
  const accent = parseHex(colors.accent);
  const onAccent =
    [colors.text, colors.bg, "#000000", "#ffffff"].find((c) => {
      const fg = parseHex(c);
      return accent && fg && contrast(accent, fg) >= TEXT_CONTRAST_MIN;
    }) ?? "#000000";

  // The visitor's own colours: the one place on the page that cannot come
  // from the look stylesheet, so they are set on the preview element.
  const vars = {
    "--toy-bg": colors.bg,
    "--toy-surface": colors.surface,
    "--toy-text": colors.text,
    "--toy-accent": colors.accent,
    "--toy-on-accent": onAccent,
  } as React.CSSProperties;

  return (
    <div className="toy">
      <div className="toy__fields">
        {FIELDS.map((f) => (
          <label className="toy__field" key={f.key}>
            <input
              type="color"
              value={colors[f.key]}
              onChange={(e) => set(f.key)(e.target.value)}
            />
            {f.label}
          </label>
        ))}
      </div>
      <div className="toy__preview" style={vars}>
        <span className="toy__card">Return the library books</span>
        <span className="toy__btn">Save</span>
      </div>
      <div className="toy__verdict" aria-live="polite">
        {warnings.length === 0 ? (
          <p className="toy__ok">Readable. The app would save this.</p>
        ) : (
          warnings.map((w) => (
            <p className="toy__warn" key={w}>
              {w}
            </p>
          ))
        )}
        <button type="button" className="link-button" onClick={() => setColors(look.colors)}>
          Back to {look.name}
        </button>
      </div>
    </div>
  );
}
