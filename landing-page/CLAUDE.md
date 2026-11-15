<!-- Generated from AGENTS.md by scripts/sync-rules.sh. Edit AGENTS.md, then run `npm run sync:rules`. -->

# landing-page/ — the Posthaste marketing site

This folder is the marketing site only. It is a Next.js (static export) app:
`npm install && npm run dev` here, `npm run build` writes plain files to
`out/` for any static host. It never imports from the app's `src/` or
`src-tauri/`. The product facts it states live in `src/config.ts` and
`src/lib/looks.ts`.

Honesty rules (binding): no invented proof (testimonials, counts, ratings,
logos, statistics). The only timing on the page is the one measured in the
visitor's own tab. Download links must point at files that exist in a
published release (`config.ts` holds confirmed links; the build replaces
them with the latest release's assets when it can reach the GitHub API).
Platform claims follow `docs/platform-notes.md`. Privacy wording must match
what the app does: no account, no analytics, no telemetry; AI polish sends
the one note you polish to the provider you chose, with your key; the app
checks GitHub for updates at launch and every six hours. No features beyond
the shipped app. The page itself makes no third-party requests: no
analytics, no cookies, no CDN fonts or embeds.

Looks: the single source of look data is `src/lib/looks.ts`. `src/lib/theme.ts`
turns every look into a CSS block keyed by `[data-look="<id>"]` (through
`src/lib/derive.ts`), injected in `layout.tsx` at build time. Switching the
page is one attribute on `<html>`; any element carrying `data-look` wears
that look (the tiles and chips do). No colour values in components or CSS;
the visitor's own colours in the contrast toy are the one exception.

Type: one self-hosted variable font, Recursive (OFL, `src/fonts/`), subset
to Latin plus key symbols, weights 450 to 800, slant -12 to 0. Its `CASL` and
`MONO` axes stand in for the app's font modes and follow the look via
`--casl` and `--mono`.

Motion: the main gesture (save, switch look, tick a note off) is a hard
cut. No transitions on it, no scroll-triggered animation. The hero's
background keyboard (`KeyboardBackdrop`) only moves in answer to the
visitor's own keystrokes, and is still under reduced motion.

Shortcuts: the copy does not lean on one key combination; the app is
keyboard-first and will gain shortcuts. Specific keys live in one place,
the "Every move is a key" sheet (`ShortcutSheet`), with facts from the
README.

Demo state lives in `src/lib/demo.ts` (a pure reducer) and
`src/components/SiteState.tsx`. It is memory only: nothing typed into the
demo is stored or sent.

The GitHub star count in the header is fetched from the GitHub API at build
time and baked into the static HTML. On the mini PC, an hourly cron
(~/bin/update-stars.sh) writes the live count to stars.json inside the
served folder, and StarChip re-reads that file from the visitor's browser —
a same-origin request, so the page still makes no third-party requests.
Deploy with `rsync -az --delete --exclude=stars.json out/ …` so the hourly
refresh survives re-uploads.

Copy tone: plain language, short sentences, and every sentence names a key,
a number or a behaviour of the product. No stock phrases ("seamless",
"in a blink", "effortless"). Developer details live only in the quiet
"Comfortable with a terminal?" block of the Get-it section.

The share image `public/og.png` is rendered from `public/og-src.svg` with
`rsvg-convert` (instructions in the SVG).

Tests: `npm test` (share-code, contrast, derivation pinned to the app's Rust
golden values, theme, demo). Formatting: `npm run format`. Never edit files
outside `landing-page/` and `docs/` when working on the site.
