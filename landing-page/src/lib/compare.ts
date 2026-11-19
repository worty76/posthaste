// The comparison table, as data. Every cell is a plain statement about how an
// app is set up, never a price, a number of seconds or a ranking: those go
// stale or can't be measured here, and the sources we could reach disagree on
// them. The test in tests/compare.test.ts enforces that. When you update a
// cell, check it against the product's own pages and bump COMPARE_CHECKED_ON.

/** The day the competitor cells were last checked. ISO date. */
export const COMPARE_CHECKED_ON = "2026-11-19";

export type CompareId = "posthaste" | "apple-notes" | "notion" | "obsidian" | "todoglow";

export interface CompareApp {
  id: CompareId;
  name: string;
  /** The product's own page, so a visitor can check us. Not set for Posthaste. */
  url?: string;
}

/** Posthaste first, then the strongest competitors people compare it against. */
export const COMPARE_APPS: CompareApp[] = [
  { id: "posthaste", name: "Posthaste" },
  { id: "apple-notes", name: "Apple Notes", url: "https://www.apple.com/apple-notes/" },
  { id: "notion", name: "Notion", url: "https://www.notion.com/pricing" },
  { id: "obsidian", name: "Obsidian", url: "https://obsidian.md/pricing" },
  { id: "todoglow", name: "TodoGlow", url: "https://devglow.app/todoglow" },
];

export interface CompareRow {
  label: string;
  cells: Record<CompareId, string>;
}

export const COMPARE_ROWS: CompareRow[] = [
  {
    label: "Account",
    cells: {
      posthaste: "None",
      "apple-notes": "Apple Account for sync",
      notion: "Required",
      obsidian: "None for local use",
      todoglow: "Required for sync",
    },
  },
  {
    label: "Storage",
    cells: {
      posthaste: "One local SQLite file",
      "apple-notes": "iCloud or Mac",
      notion: "Notion's cloud",
      obsidian: "Local Markdown folder",
      todoglow: "Local, optional cloud sync",
    },
  },
  {
    label: "Cost",
    cells: {
      posthaste: "Free, MIT open source",
      "apple-notes": "Free (iCloud storage paid)",
      notion: "Free tier; paid plans for full AI & teams",
      obsidian: "Free personal; paid sync",
      todoglow: "Free trial, then paid",
    },
  },
  {
    label: "AI",
    cells: {
      posthaste: "Optional, your own key",
      "apple-notes": "Apple Intelligence",
      notion: "Paid plans",
      obsidian: "Community plugins",
      todoglow: "Own Gemini key + MCP",
    },
  },
  {
    label: "Sync & mobile",
    cells: {
      posthaste: "Optional P2P desktop sync (experimental); no phone app",
      "apple-notes": "Mac, iPhone, iPad, web",
      notion: "Desktop, phone, web",
      obsidian: "Desktop & phone (paid sync or DIY)",
      todoglow: "Mac & Windows; no phone app",
    },
  },
];

/** Why you would pick Posthaste. Each line is a behaviour, not a boast. */
export const PICK_POSTHASTE: string[] = [
  "You want keyboard-first capture and a short Today list on macOS, Windows or Linux.",
  "You want your notes in one local file, with optional encrypted P2P sync and no account.",
  "You want AI polish only when you ask, using your own key.",
];

/** Where the others are the better pick. Said plainly, because it is true. */
export const PICK_OTHERS: string[] = [
  "You need a phone app or Apple-only Pencil/scans (Apple Notes).",
  "You collaborate on shared docs, databases and wikis (Notion).",
  "You build a linked library of long Markdown notes (Obsidian).",
  "You want a focus timer or coding agents over MCP (TodoGlow).",
];
