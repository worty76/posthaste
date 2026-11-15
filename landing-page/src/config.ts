// Everything that can change lives here. Nothing else on the site hard-codes
// product facts, and no download URL is ever invented.

/** One installer per platform, as named in the GitHub release. */
export interface Download {
  url: string;
  /** e.g. "Apple Silicon", ".AppImage". */
  label: string;
}

export type Platform = "mac" | "windows" | "linux";

const RELEASE = "https://github.com/worty76/posthaste/releases/download/v0.3.0";

export const PRODUCT = {
  name: "Posthaste",
  /** From src-tauri/Cargo.toml [workspace.package] version. */
  version: "0.3.0",
  repoUrl: "https://github.com/worty76/Posthaste",
  releasesUrl: "https://github.com/worty76/Posthaste/releases/latest",
  /** The community server. A plain link in the header; the page loads nothing from Discord. */
  discordUrl: "https://discord.gg/t3Pm7c7JE2",
  /** The canonical home; canonical and share URLs use it. */
  siteUrl: "https://posthaste.ink" as string | null,
  /**
   * The v0.3.0 installers, checked against the published release on
   * 15 Nov 2026. The build replaces these with the latest release's assets
   * when it can reach the GitHub API (see getDownloads in app/page.tsx);
   * null would mean "not released for this platform".
   */
  downloads: {
    mac: { url: `${RELEASE}/Posthaste_0.3.0_aarch64.dmg`, label: "Apple Silicon, .dmg" },
    windows: { url: `${RELEASE}/Posthaste_0.3.0_x64-setup.exe`, label: "64-bit installer, .exe" },
    linux: { url: `${RELEASE}/Posthaste_0.3.0_amd64.AppImage`, label: "x86-64, .AppImage" },
  } as Record<Platform, Download | null>,
  /** Extra Linux packages from the same release. */
  linuxExtras: [
    { url: `${RELEASE}/Posthaste_0.3.0_amd64.deb`, label: ".deb" },
    { url: `${RELEASE}/Posthaste-0.3.0-1.x86_64.rpm`, label: ".rpm" },
  ] as Download[],
} as const;

/** The demo film in `public/demo/`. Same-origin files; `seconds` is the film's own length. */
export const DEMO_VIDEO = {
  src: "/demo/posthaste-demo.mp4",
  poster: "/demo/posthaste-demo-poster.jpg",
  seconds: 73,
} as const;

/** Release notes written for people, by version. They replace the automatic
    release body (commit subjects) in the changelog. */
export const RELEASE_NOTES: Record<string, string[]> = {
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
