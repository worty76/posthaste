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
the shipped app. The app's promise (no analytics, no telemetry) is about the
app only; the site measures itself, see "Site measurement" below.

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

Comparison (`Compare`, data in `src/lib/compare.ts`): states how other apps
are set up (account, where notes live, free or paid, AI model, sync and phone
app), never a price, a speed or a ranking. Sources disagree on prices and
plans change, and the honesty rules forbid timings we did not measure.
`tests/compare.test.ts` rejects currency, per-month figures, superlatives and
empty cells. Posthaste's own AI is described as optional (your key, the one
note you polish). AI is a headline feature, so the title and intro may name it, but
always as something the visitor switches on (off by default), never as required and
never a feature the app does not ship yet. Keep a row where Posthaste gives
something up, keep the "pick the others when" list, and when you change a
cell, check it on the product's own page and bump `COMPARE_CHECKED_ON`.

Search: `app/robots.ts`, `app/sitemap.ts` and the JSON-LD in `lib/seo.ts`
(injected in `layout.tsx`) are the only SEO files. The JSON-LD states facts
(free, OS list, licence) and never ratings, reviews or counts. The sitemap URL
must equal the canonical link. The hero says "note app" in plain words; do not
stuff keywords. `tests/seo.test.ts` pins these.

Page order (v3, plan in `docs/superpowers/plans/2026-11-15-website-v3-master-plan.md`):
release banner, header with section links, a definition hero (the word's
meaning, the to-do notes line, four fact links, the download for this OS and
the demo film), the loop (catch, see what's left, tick off: three live demos
sharing the hero's state), feature blocks (copy in `src/lib/blocks.ts`, each
with a small working demo), the film, looks, local, the sync film (`SyncSection`, copy in `i18n.ts`, film in `SYNC_VIDEO`), compare, changelog
(`src/lib/releases.ts`, read from GitHub at build time, hand-written notes in
`RELEASE_NOTES` in `config.ts` win over commit subjects), the free card, get
it. Posthaste is described as a to-do notes app. When a competitor's own page
does not state a fact, the compare cell says "Not stated" rather than guess.

The story page (`/story/`, copy and switches in `src/lib/story.ts`) is hidden
until the maker sets `published: true`. While hidden nothing links to it, the
sitemap omits it and the route builds as a noindex 404; `STORY_PREVIEW=1` shows
it for review builds. When published it adds a nav link, a line beside the
hero's dictionary definition, the optional `teaser` under the hero buttons and
the optional `note` (the maker note above the free card). `teaser` and `note`
are the maker's own words: never write them on their behalf, and the beats
state only facts the site already backs (honesty rules above). `trailingSlash`
is on so `/story/` resolves on any static host. `tests/story.test.ts` pins this.

Demo state lives in `src/lib/demo.ts` (a pure reducer) and
`src/components/SiteState.tsx`. It is memory only: nothing typed into the
demo is stored or sent.

Site measurement (`src/components/Analytics.tsx`, `src/lib/tracking.ts`,
`src/lib/analytics.ts`): the maintainer
needs to know how many people visit, where they come from and how many
download, so when built with `NEXT_PUBLIC_DL_COUNTER=/dl` the site sends a
first-party `POST /visit` and routes installer links through `/dl` on the
origin server. Only daily aggregates are kept (day, file kind,
version, two-letter country, and source/referrer host), published to
`stats.json` alongside `stars.json` and reported to Discord each day. Nothing
identifies a visitor: no IP address, no user agent, no visitor cookie, no id is
stored. Allowed: counts, referrers, countries, per day or page. Not allowed: ad
or social trackers, fingerprinting, per-person profiles, session replay,
selling or sharing the data, CDN fonts or embeds, and anything typed into the
demo. The footer (`footer.privacy`, English and Vietnamese) must say what is
measured and that the app itself sends nothing; keep it true. Both `/visit` and
`/dl` skip a browser carrying the maintainer's opt-out (`ph_skip`, set at
`/dl/skip?key=…`, never for a visitor). `tests/download-counter.test.ts` pins
this.

The GitHub star count in the header is fetched from the GitHub API at build
time and baked into the static HTML. On the mini PC, an hourly cron
(~/bin/update-stars.sh) writes the live count to stars.json inside the
served folder, and StarChip re-reads that file from the visitor's browser —
a same-origin request.
Deploy with `rsync -az --delete --exclude=stars.json --exclude=stats.json out/ …`
so the hourly star refresh and live stats survive re-uploads.

Copy tone: plain language, short sentences, and every sentence names a key,
a number or a behaviour of the product. No stock phrases ("seamless",
"in a blink", "effortless"). Developer details live only in the quiet
"Comfortable with a terminal?" block of the Get-it section.

The share image `public/og.png` is rendered from `public/og-src.svg` with
`rsvg-convert` (instructions in the SVG).

Tests: `npm test` (share-code, contrast, derivation pinned to the app's Rust
golden values, theme, demo, release config sync). Formatting: `npm run format`.
When bumping a release version, follow `docs/release-guide.md` so `RELEASE`,
`PRODUCT.version`, all 5 installer URLs, `RELEASE_NOTES`, and
`FALLBACK_RELEASES` in `src/config.ts` all stay in sync with
`src-tauri/Cargo.toml`. Never edit files outside `landing-page/` and `docs/`
when working on the site alone.
