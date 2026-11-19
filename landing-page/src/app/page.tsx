import { CaptureHero } from "@/components/CaptureHero";
import { Changelog } from "@/components/Changelog";
import { Compare } from "@/components/Compare";
import { ContrastToy } from "@/components/ContrastToy";
import { DemoVideo } from "@/components/DemoVideo";
import { Downloads } from "@/components/Downloads";
import { Features } from "@/components/Features";
import { FreeCard } from "@/components/FreeCard";
import { HeroActions } from "@/components/HeroActions";
import { ChatIcon, GitHubMark } from "@/components/icons";
import { KeyboardBackdrop } from "@/components/KeyboardBackdrop";
import { LookTiles } from "@/components/LookTiles";
import { Moments } from "@/components/Moments";
import { ReleaseBanner } from "@/components/ReleaseBanner";
import { ScrollChoreography } from "@/components/ScrollChoreography";
import { ShareCodeRow } from "@/components/ShareCodeRow";
import { SiteFooter } from "@/components/SiteFooter";
import { SiteStateProvider } from "@/components/SiteState";
import { StarChip } from "@/components/StarChip";
import { MakerNote, StoryHeroLink } from "@/components/StoryLinks";
import { PRODUCT, type Download, type Platform } from "@/config";
import { fetchReleases } from "@/lib/releases";
import { storyVisible } from "@/lib/story";

interface Installers {
  downloads: Record<Platform, Download | null>;
  linuxExtras: Download[];
}

/** The latest release's installers, read at build time. When the API is
    unreachable the confirmed links in config.ts stay. */
async function getInstallers(): Promise<Installers> {
  const fallback: Installers = { downloads: PRODUCT.downloads, linuxExtras: PRODUCT.linuxExtras };
  try {
    const res = await fetch("https://api.github.com/repos/worty76/Posthaste/releases/latest", {
      cache: "no-store",
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return fallback;
    const data = (await res.json()) as {
      assets?: { name?: string; browser_download_url?: string }[];
    };
    const found: Installers = {
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
  const [stars, release, releases] = await Promise.all([
    getStars(),
    getInstallers(),
    fetchReleases(4),
  ]);

  return (
    <SiteStateProvider>
      <ScrollChoreography />
      <ReleaseBanner release={releases[0]} />
      <header className="page head">
        <a className="wordmark" href="/">
          <img src="/logo-256.png?v=2" alt="" width={22} height={22} />
          Posthaste
        </a>
        <nav className="head__nav" aria-label="Sections">
          {storyVisible() && <a href="/story/">Why Posthaste</a>}
          <a href="#features">Features</a>
          <a href="#looks">Looks</a>
          <a href="#compare">Compare</a>
          <a href="#changelog">Changelog</a>
        </nav>
        <span className="head__spacer" />
        <a className="head__link" href={PRODUCT.discordUrl} rel="noopener noreferrer">
          <ChatIcon />
          Discord
        </a>
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
          <p className="dict">
            <dfn>posthaste</dfn> <span className="dict__pos">adverb</span>
            <span className="dict__def">with great speed; right away.</span>
            {storyVisible() && (
              <a className="dict__see" href="/story/">
                Why it exists
              </a>
            )}
          </p>
          <div className="hero__top">
            <h1 className="hero__title" id="hero-title">
              Your to-do notes, at the speed of your keyboard.
            </h1>
            <div className="hero__side">
              <p className="hero__sub">Posthaste is a to-do notes app for your keyboard.</p>
              <div style={{ height: "2rem" }} />
              <p className="hero__sub">With shortcuts open it over any app</p>
              <div style={{ height: "2rem" }} />
              <p className="hero__sub">
                Help you fast write and organize your notes in your daily life
              </p>
              <noscript>
                <p className="hero__sub">Turn on JavaScript to try the box below.</p>
              </noscript>
            </div>
          </div>
          <ul className="facts" aria-label="Posthaste in short">
            <li>
              <a href="#features">Minimal</a>
            </li>
            <li>
              <a href="#free">Free</a>
            </li>
            <li>
              <a href="#ai">Optional AI</a>
            </li>
            <li>
              <a href="#ai">Customizable</a>
            </li>
          </ul>
          <HeroActions downloads={release.downloads} />
          <StoryHeroLink />
          <CaptureHero />
        </section>

        <Moments />

        <Features />

        <DemoVideo />

        <section className="page looks" id="looks" aria-labelledby="looks-title">
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
                  Your notes, drafts and looks, in one file. Export gives you a Markdown checklist;
                  backup copies everything.
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
                  <strong>AI or not.</strong> It's your choice.
                </li>
              </ul>
            </div>
          </div>
        </section>

        <Compare />

        <Changelog releases={releases} />

        <MakerNote />

        <FreeCard />

        <section className="page get" id="get" aria-labelledby="get-title">
          <h2 className="h2" id="get-title">
            Get Posthaste.
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

      <SiteFooter />
    </SiteStateProvider>
  );
}
