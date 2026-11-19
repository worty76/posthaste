// Everything that can change lives here. Nothing else on the site hard-codes
// product facts, and no download URL is ever invented.

/** One installer per platform, as named in the GitHub release. */
export interface Download {
  url: string;
  /** e.g. "Apple Silicon", ".AppImage". */
  label: string;
}

export type Platform = "mac" | "windows" | "linux";

const RELEASE = "https://github.com/worty76/posthaste/releases/download/v0.6.1";

export const PRODUCT = {
  name: "Posthaste",
  /** From src-tauri/Cargo.toml [workspace.package] version. */
  version: "0.6.1",
  repoUrl: "https://github.com/worty76/Posthaste",
  releasesUrl: "https://github.com/worty76/Posthaste/releases/latest",
  /** The community server. A plain link in the header; the page loads nothing from Discord. */
  discordUrl: "https://discord.gg/t3Pm7c7JE2",
  /** The canonical home; canonical and share URLs use it. */
  siteUrl: "https://posthaste.ink" as string | null,
  /**
   * The v0.6.1 installers, named the way the release workflow names them.
   * The build replaces these with the latest release's assets
   * when it can reach the GitHub API (see getDownloads in app/page.tsx);
   * null would mean "not released for this platform".
   */
  downloads: {
    mac: { url: `${RELEASE}/Posthaste_0.6.1_aarch64.dmg`, label: "Apple Silicon, .dmg" },
    windows: { url: `${RELEASE}/Posthaste_0.6.1_x64-setup.exe`, label: "64-bit installer, .exe" },
    linux: { url: `${RELEASE}/Posthaste_0.6.1_amd64.AppImage`, label: "x86-64, .AppImage" },
  } as Record<Platform, Download | null>,
  /** Extra Linux packages from the same release. */
  linuxExtras: [
    { url: `${RELEASE}/Posthaste_0.6.1_amd64.deb`, label: ".deb" },
    { url: `${RELEASE}/Posthaste-0.6.1-1.x86_64.rpm`, label: ".rpm" },
  ] as Download[],
} as const;

/** The demo film in `public/demo/`. Same-origin files; `seconds` is the film's own length. */
export const DEMO_VIDEO = {
  src: "/demo/posthaste-demo.mp4?v=3",
  poster: "/demo/posthaste-demo-poster.jpg?v=3",
  seconds: 73,
} as const;

/** Release notes written for people, by version. They replace the automatic
    release body (commit subjects) in the changelog. */
export const RELEASE_NOTES: Record<string, string[]> = {
  "0.6.1": [
    "If you ever open an older copy of Posthaste on a newer database, the screen now offers a one-click Update button instead of leaving you stuck.",
    "Development builds now use a separate database file so testing never touches your installed app's notes.",
  ],
  "0.6.0": [
    "Experimental device sync: pair two of your own computers with a one-time code under Settings → Devices and keep notes in sync with no account and no central server.",
    "Notes travel directly over encrypted peer-to-peer QUIC (iroh) on the same network, or through a relay that cannot read them when your computers are on different networks.",
    "Edits merge per field, and if you edit the same note on two computers while offline, both versions survive — the losing text is saved as a new inbox note.",
  ],
  "0.5.0": [
    "Three fonts ship inside the app — Be Vietnam Pro, Nunito and Quicksand — so Blueberry, Sakura night and Strawberry Milk read the same on every computer, with Vietnamese covered.",
    "A look can name any font installed on your computer, the way code editors do: type its name in the editor, and if a friend lacks it they still get a readable fallback.",
    "There is no more Plain: Posthaste always wears a look, and light or dark follows the look you picked. The old Light/Dark/System setting is gone.",
    "File dialogs now open attached to the window that asked for them, instead of hiding behind it.",
  ],
  "0.4.0": [
    "Make Posthaste yours: pick a look from the gallery, or make your own with the live editor — every colour, corner and font, previewed as you type.",
    "A look can carry a backdrop image behind your notes, dimmed so the text stays readable.",
    "Share looks two ways: a short code for colour-only looks, or a look file that carries its images with it. Importing checks everything before anything is applied.",
    "Simple looks: keep a look's colours and hide its decorations, one switch in Settings.",
    "Windows take the keyboard the moment they open, even when another app is in front.",
  ],
  "0.3.3": [
    "Posthaste now speaks Vietnamese as well as English. Pick the language in Settings and every window, the tray menu and the file dialogs follow.",
    "The whole interface is rebuilt in Rust, in place of TypeScript.",
    "On a Mac, the note window opens as a panel that does not pull the app you are working in behind it.",
  ],
  "0.3.2": [
    "Choose where the capture and Today windows open, from nine spots on the screen, each set separately.",
    "Export your notes as a Markdown checklist that opens in any editor, and import it back. JSON is still there if you want it.",
    "AI polish works with Claude, OpenAI, Gemini, OpenRouter, Ollama or any OpenAI-compatible server. Keys are kept in your system keychain.",
    "On a Mac, the capture and Today windows no longer pull the app you are working in behind them.",
    "The note window opens on the desktop and display you are using.",
    "Today follows the selected note when you move with the keyboard, and rows show a clear hover in the light theme.",
  ],
  "0.3.1": [
    "Later: set a note aside without archiving it, and bring it back when you are ready.",
    "A calendar to pick a day, with a dot on every day that has notes. Browse earlier days in the Today window.",
    "Notes carried over from an earlier day say where they came from.",
    "Trash: deleted notes wait for 30 days, so you can restore them or delete them for good.",
  ],
  "0.3.0": [
    "Note details: give any note a description and attach files, images and documents.",
    "Search now finds words inside descriptions.",
    "New installers for macOS, Windows and Linux.",
  ],
  "0.2.2": [
    "The Today window: today's notes and the unfinished ones from before, on its own shortcut.",
    "Change the Today shortcut in Settings.",
  ],
  "0.2.1": ["Clicking the page you are already on no longer shows a blank window."],
  "0.2.0": ["Installers for macOS, Windows and Linux."],
};

/** The changelog when GitHub can't be reached at build time. Dates (UTC) and
    links checked against the published releases on 15 Nov 2026. */
export const FALLBACK_RELEASES: { version: string; date: string; url: string; notes: string[] }[] =
  (
    [
      ["0.6.1", "2026-11-19"],
      ["0.6.0", "2026-11-19"],
      ["0.5.0", "2026-11-19"],
      ["0.4.0", "2026-11-18"],
      ["0.3.3", "2026-11-17"],
      ["0.3.2", "2026-11-16"],
      ["0.3.1", "2026-11-15"],
      ["0.3.0", "2026-11-15"],
      ["0.2.2", "2026-11-14"],
      ["0.2.1", "2026-11-14"],
      ["0.2.0", "2026-11-14"],
    ] as const
  ).map(([version, date]) => ({
    version,
    date,
    url: `https://github.com/worty76/Posthaste/releases/tag/v${version}`,
    notes: RELEASE_NOTES[version] ?? [],
  }));
