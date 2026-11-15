import { CaptureHero } from "@/components/CaptureHero";
import { ContrastToy } from "@/components/ContrastToy";
import { Downloads } from "@/components/Downloads";
import { GitHubMark } from "@/components/icons";
import { KeyboardBackdrop } from "@/components/KeyboardBackdrop";
import { LookTiles } from "@/components/LookTiles";
import { ShortcutSheet } from "@/components/ShortcutSheet";
import { ShareCodeRow } from "@/components/ShareCodeRow";
import { SiteStateProvider } from "@/components/SiteState";
import { StarChip } from "@/components/StarChip";
import { TodayCount, TodayDemo } from "@/components/TodayDemo";
import { PRODUCT, type Download, type Platform } from "@/config";

interface Release {
  downloads: Record<Platform, Download | null>;
  linuxExtras: Download[];
}

/** The latest release's installers, read at build time. When the API is
    unreachable the confirmed links in config.ts stay. */
async function getRelease(): Promise<Release> {
  const fallback: Release = { downloads: PRODUCT.downloads, linuxExtras: PRODUCT.linuxExtras };
  try {
    const res = await fetch("https://api.github.com/repos/worty76/Posthaste/releases/latest", {
      cache: "no-store",
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return fallback;
    const data = (await res.json()) as {
      assets?: { name?: string; browser_download_url?: string }[];
    };
    const found: Release = {
      downloads: { mac: null, windows: null, linux: null },
      linuxExtras: [],
    };
    for (const a of data.assets ?? []) {
      const name = (a.name ?? "").toLowerCase();
      const url = a.browser_download_url;
      if (!url) continue;
      if (name.endsWith(".dmg") && !found.downloads.mac) {
        found.downloads.mac = {
          url,
          label: name.includes("aarch64") ? "Apple Silicon, .dmg" : ".dmg",
        };
      } else if (name.endsWith("x64-setup.exe") && !found.downloads.windows) {
        found.downloads.windows = { url, label: "64-bit installer, .exe" };
      } else if (name.endsWith(".appimage") && !found.downloads.linux) {
        found.downloads.linux = { url, label: "x86-64, .AppImage" };
      } else if (name.endsWith(".deb")) {
        found.linuxExtras.push({ url, label: ".deb" });
      } else if (name.endsWith(".rpm")) {
        found.linuxExtras.push({ url, label: ".rpm" });
      }
    }
    // A release without installers (a broken CI run) must not blank the page.
    const any = Object.values(found.downloads).some(Boolean);
    return any ? found : fallback;
  } catch {
    return fallback;
  }
}

/** Stargazer count, baked in at build time; StarChip refreshes it from the
    same-origin stars.json. Missing means the chip does not render. */
async function getStars(): Promise<number | null> {
  try {
    const res = await fetch("https://api.github.com/repos/worty76/Posthaste", {
      cache: "no-store",
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return null;
    const data = (await res.json()) as { stargazers_count?: unknown };
    return typeof data.stargazers_count === "number" ? data.stargazers_count : null;
  } catch {
    return null;
  }
}

export default async function Page() {
  const [stars, release] = await Promise.all([getStars(), getRelease()]);

  return (
    <SiteStateProvider>
      <header className="page head">
        <a className="wordmark" href="/">
          <img src="/logo-256.png?v=2" alt="" width={22} height={22} />
          Posthaste
        </a>
        <span className="head__spacer" />
        <StarChip baked={stars} />
        <a className="head__icon" href={PRODUCT.repoUrl} aria-label="Posthaste on GitHub">
          <GitHubMark />
        </a>
        <a className="btn btn--small" href="#get">
          Download
        </a>
      </header>

      <main>
        <section className="page hero" aria-labelledby="hero-title">
          <KeyboardBackdrop />
          <div className="hero__top">
            <h1 className="hero__title" id="hero-title">
              Get it out of your head in two seconds.
            </h1>
            <div className="hero__side">
              <p className="hero__sub">
                Posthaste lives on your keyboard. A shortcut opens it over any app, Enter saves, and
                you&apos;re back where you were. Try it here: the box is ready.
              </p>
              <noscript>
                <p className="hero__sub">Turn on JavaScript to try the box below.</p>
              </noscript>
            </div>
          </div>
          <CaptureHero />
        </section>

        <section className="page today" aria-labelledby="today-title">
          <div className="today__copy">
            <h2 className="h2" id="today-title">
              Later, see what&apos;s left.
            </h2>
            <p>
              Another shortcut opens a small window with today&apos;s notes and the unfinished ones
              from before. X ticks one off. Esc puts it away and you&apos;re back again.
            </p>
            <TodayCount />
            <p className="quiet">
              Everything else waits in the inbox, one more shortcut away. Search finds a note by any
              word in it. A deleted note waits 24 hours in the trash, so Undo always works.
            </p>
          </div>
          <TodayDemo />
        </section>

        <section className="page keys-section" aria-labelledby="keys-title">
          <div className="keys-section__copy">
            <h2 className="h2" id="keys-title">
              Every move is a key.
            </h2>
            <p>
              No menus to dig through. The mouse works too, but the keyboard is faster once your
              hands know the way. Every global shortcut can be changed in Settings, and <kbd>?</kbd>{" "}
              lists them all inside the app.
            </p>
          </div>
          <ShortcutSheet />
        </section>

        <section className="page looks" aria-labelledby="looks-title">
          <h2 className="h2" id="looks-title">
            Your colors, on all of it.
          </h2>
          <p className="looks__lede">
            Pick a look and this page changes the way the app does: colors, corners, even the shape
            of the letters.
          </p>
          <LookTiles />
          <div className="looks__tools">
            <div>
              <h3 className="h3">Or make your own</h3>
              <p className="quiet">
                Four colors. If the text would be hard to read, Posthaste says why and won&apos;t
                save it.
              </p>
              <ContrastToy />
            </div>
            <div>
              <h3 className="h3">Share it as one line</h3>
              <ShareCodeRow />
            </div>
          </div>
        </section>

        <section className="page local" aria-labelledby="local-title">
          <h2 className="h2" id="local-title">
            On your computer, not ours.
          </h2>
          <div className="local__cols">
            <div>
              <h3 className="h3">Stays on your computer</h3>
              <ul className="ledger">
                <li>
                  Your notes, drafts and looks, in one file. Export and backup make copies you keep.
                </li>
                <li>No account. No analytics. No telemetry.</li>
                <li>The app&apos;s logs never include what you wrote.</li>
                <li>
                  A note is saved before the window closes, and a half-typed draft survives a crash.
                </li>
              </ul>
            </div>
            <div>
              <h3 className="h3">Leaves only when</h3>
              <ul className="ledger">
                <li>
                  <strong>You press Polish.</strong> AI polish is off until you pick a provider
                  (Anthropic, OpenAI or Gemini) and paste your own API key. Then the one note you
                  polish goes straight to that provider. Nothing else does.
                </li>
                <li>
                  <strong>The app checks for an update.</strong> At launch and every six hours it
                  asks GitHub whether a newer version exists. Installing one is your click.
                </li>
              </ul>
            </div>
          </div>
        </section>

        <section className="page get" id="get" aria-labelledby="get-title">
          <h2 className="h2" id="get-title">
            Free, and open source.
          </h2>
          <p className="quiet">
            MIT licensed. Version {PRODUCT.version}.{" "}
            <a href={PRODUCT.releasesUrl}>All files and release notes</a>
          </p>
          <Downloads
            downloads={release.downloads}
            linuxExtras={release.linuxExtras}
            releasesUrl={PRODUCT.releasesUrl}
          />
          <p className="get__note">
            Used every day on macOS and tested on Linux. The Windows installer is built
            automatically but hasn&apos;t been tried on a Windows PC yet, so expect rough edges. On
            an Intel Mac, build it yourself for now.
          </p>
          <details className="terminal">
            <summary>Comfortable with a terminal? Build it yourself</summary>
            <pre>
              <code>
                {`git clone ${PRODUCT.repoUrl}
cd Posthaste
npm install
npm run tauri build`}
              </code>
            </pre>
            <p className="quiet">
              Needs Node 20+, Rust and the Tauri system packages. The{" "}
              <a href={`${PRODUCT.repoUrl}#build-from-source`}>README</a> lists them.
            </p>
          </details>
        </section>
      </main>

      <footer className="page foot">
        <span>This page has no analytics, no cookies, and asks no other site for anything.</span>
        <span className="foot__meta">
          <a href={PRODUCT.repoUrl}>Source on GitHub</a>
          <span>Posthaste {PRODUCT.version}</span>
        </span>
      </footer>
    </SiteStateProvider>
  );
}
