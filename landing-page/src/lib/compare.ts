// The comparison table, as data. Every cell is a plain statement about how an
// app is set up, never a price, a number of seconds or a ranking: those go
// stale or can't be measured here, and the sources we could reach disagree on
// them. The test in tests/compare.test.ts enforces that. When you update a
// cell, check it against the product's own pages and bump COMPARE_CHECKED_ON.

/** The day the competitor cells were last checked. ISO date. */
export const COMPARE_CHECKED_ON = "2026-11-15";

export type CompareId =
  "posthaste" | "octarine" | "type" | "stik" | "apple-notes" | "notion" | "obsidian" | "evernote";

export interface CompareApp {
  id: CompareId;
  name: string;
  /** The product's own page, so a visitor can check us. Not set for Posthaste. */
  url?: string;
}

/** Posthaste first, then the capture apps closest to it, then the big note apps. */
export const COMPARE_APPS: CompareApp[] = [
  { id: "posthaste", name: "Posthaste" },
  { id: "octarine", name: "Octarine", url: "https://octarine.app/" },
  { id: "type", name: "Type", url: "https://apps.apple.com/app/id1664249924" },
  { id: "stik", name: "Stik", url: "https://github.com/0xMassi/stik_app" },
  { id: "apple-notes", name: "Apple Notes", url: "https://www.apple.com/apple-notes/" },
  { id: "notion", name: "Notion", url: "https://www.notion.com/pricing" },
  { id: "obsidian", name: "Obsidian", url: "https://obsidian.md/pricing" },
  { id: "evernote", name: "Evernote", url: "https://evernote.com/compare-plans" },
];

export interface CompareRow {
  label: string;
  cells: Record<CompareId, string>;
}

export const COMPARE_ROWS: CompareRow[] = [
  {
    label: "Account",
    cells: {
      posthaste: "None. Nothing to sign up for.",
      octarine: "Not stated on its site",
      type: "Not stated on its App Store page",
      stik: "None",
      "apple-notes": "An Apple Account, to sync",
      notion: "Required",
      obsidian: "None for the app itself",
      evernote: "Required",
    },
  },
  {
    label: "Where your notes live",
    cells: {
      posthaste: "One file on your computer",
      octarine: "Markdown files in a folder you choose",
      type: "Text files in a folder you choose",
      stik: "Markdown files in your Documents folder",
      "apple-notes": "iCloud, or on your Mac",
      notion: "Notion's servers",
      obsidian: "Markdown files in a folder you choose",
      evernote: "Evernote's cloud",
    },
  },
  {
    label: "Cost",
    cells: {
      posthaste: "Free, MIT licence",
      octarine: "A free plan. Tasks, views and full AI are a one-time paid upgrade",
      type: "A free trial, then a one-time purchase",
      stik: "Open source, MIT licence",
      "apple-notes": "Free. Extra iCloud storage is paid",
      notion: "A free plan. Team features and full AI are on paid plans",
      obsidian: "Free for personal use. Sync is a paid add-on",
      evernote: "A free plan holds fifty notes and one notebook",
    },
  },
  {
    label: "AI",
    cells: {
      posthaste: "Optional. Your own key, and only the note you polish",
      octarine: "Optional. Your own key or a local model",
      type: "None listed",
      stik: "On-device search and folder suggestions, no API key",
      "apple-notes": "Apple Intelligence tools on supported devices",
      notion: "A limited trial on Free and Plus, full access on Business and above",
      obsidian: "Through community plugins",
      evernote: "Offered on paid plans",
    },
  },
  {
    label: "Sync and phone app",
    cells: {
      posthaste: "No sync, no phone app",
      octarine: "Mac, Windows, Linux and iPhone. Git or any folder sync",
      type: "Mac only. No sync or phone app listed",
      stik: "Mac only. Sync through git or a synced folder",
      "apple-notes": "Mac, iPhone, iPad and the web",
      notion: "Web, desktop and phone",
      obsidian: "Desktop and phone. Sync is paid, or bring your own",
      evernote: "Desktop, phone and web",
    },
  },
];

/** Why you would pick Posthaste. Each line is a behaviour, not a boast. */
export const PICK_POSTHASTE: string[] = [
  "You want a thought saved before it slips, then to be back in your work.",
  "You want your notes in a file you can export, not on someone else's server.",
  "You want AI cleanup on your terms: your key, one note, only when you press Polish.",
  "You want a short list of what's left today, not a pile of files.",
  "You work on Windows or Linux as well as a Mac.",
];

/** Where the others are the better pick. Said plainly, because it is true. */
export const PICK_OTHERS: string[] = [
  "You need your notes on your phone. Posthaste has no sync and no phone app.",
  "You plan or write with other people, or build pages and tables. That is Notion's job.",
  "You keep a linked library of long notes. Obsidian is built for that.",
  "You live on Apple devices and want scans and drawings beside your notes. That is Apple Notes.",
  "You want one workspace for linked notes, tasks and boards. Octarine does that.",
  "You want quick notes as plain text or Markdown files you open in any editor, on a Mac. Type and Stik do that.",
];
