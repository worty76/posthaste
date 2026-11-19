import { Changelog } from "@/components/Changelog";
import { Compare } from "@/components/Compare";
import { DemoVideo } from "@/components/DemoVideo";
import { Features } from "@/components/Features";
import { FreeCard } from "@/components/FreeCard";
import {
  GetSection,
  HeroSection,
  LocalSection,
  LooksSection,
  SiteHeader,
} from "@/components/LocalizedSections";
import { Moments } from "@/components/Moments";
import { ReleaseBanner } from "@/components/ReleaseBanner";
import { ScrollChoreography } from "@/components/ScrollChoreography";
import { SiteFooter } from "@/components/SiteFooter";
import { SiteStateProvider } from "@/components/SiteState";
import { MakerNote } from "@/components/StoryLinks";
import { PRODUCT, type Download, type Platform } from "@/config";
import { fetchReleases } from "@/lib/releases";

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
      <SiteHeader stars={stars} />

      <main>
        <HeroSection downloads={release.downloads} />
        <Moments />
        <Features />
        <DemoVideo />
        <LooksSection />
        <LocalSection />
        <Compare />
        <Changelog releases={releases} />
        <MakerNote />
        <FreeCard />
        <GetSection downloads={release.downloads} linuxExtras={release.linuxExtras} />
      </main>

      <SiteFooter />
    </SiteStateProvider>
  );
}
