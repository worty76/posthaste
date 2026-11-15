// Runtime theming. Every look's tokens are generated at build time from
// looks.ts through derive.ts and keyed by [data-look="<id>"], so switching
// the page is one attribute change on <html>, and any element can wear a
// different look by carrying the attribute itself (the look tiles do).

import { deriveTokens } from "./derive";
import { LOOKS, defaultLookId, lookById, type Look } from "./looks";

/** Recursive's casual and mono axes stand in for the app's font modes. */
const FONT_AXES: Record<Look["font"], { casl: number; mono: number }> = {
  system: { casl: 0, mono: 0 },
  rounded: { casl: 1, mono: 0 },
  serif: { casl: 0, mono: 0 },
  mono: { casl: 0, mono: 1 },
};

/** The custom properties a look sets, by name. */
export function lookVariables(look: Look): Record<string, string> {
  const t = deriveTokens(look);
  const axes = FONT_AXES[look.font];
  return {
    "--bg": look.colors.bg,
    "--surface": look.colors.surface,
    "--text": look.colors.text,
    "--accent": look.colors.accent,
    "--accent-ink": t.accentInk,
    "--accent-soft": t.accentSoft,
    "--on-accent": t.onAccent,
    "--muted": t.muted,
    "--faint": t.faint,
    "--border": t.border,
    "--border-strong": t.borderStrong,
    "--hover": t.hover,
    "--active": t.active,
    "--r-sm": t.radius[0],
    "--r-md": t.radius[1],
    "--r-lg": t.radius[2],
    "--casl": String(axes.casl),
    "--mono": String(axes.mono),
    "color-scheme": look.base,
  };
}

function block(selector: string, look: Look): string {
  const body = Object.entries(lookVariables(look))
    .map(([k, v]) => `${k}:${v}`)
    .join(";");
  return `${selector}{${body}}`;
}

/**
 * The stylesheet for every look. With no data-look on <html> (first paint,
 * or JavaScript off) the colour scheme picks the default look.
 */
export function themeCss(): string {
  const light = lookById(defaultLookId(false));
  const dark = lookById(defaultLookId(true));
  return [
    block(":root", light),
    `@media (prefers-color-scheme: dark){${block(":root:not([data-look])", dark)}}`,
    ...LOOKS.map((look) => block(`[data-look="${look.id}"]`, look)),
  ].join("\n");
}

/** Puts the whole page in a look. */
export function applyLook(id: string): void {
  const look = lookById(id);
  document.documentElement.dataset.look = look.id;
  document.querySelectorAll('meta[name="theme-color"]').forEach((meta) => {
    meta.setAttribute("content", look.colors.bg);
  });
}
