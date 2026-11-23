"use client";

import { ChatIcon, GitHubMark } from "./icons";
import { CaptureHero } from "./CaptureHero";
import { ContrastToy } from "./ContrastToy";
import { Downloads } from "./Downloads";
import { HeroActions } from "./HeroActions";
import { KeyboardBackdrop } from "./KeyboardBackdrop";
import { LangToggle } from "./LangToggle";
import { LookTiles } from "./LookTiles";
import { ShareCodeRow } from "./ShareCodeRow";
import { useSite } from "./SiteState";
import { StarChip } from "./StarChip";
import { StoryHeroLink } from "./StoryLinks";
import { storyVisible } from "@/lib/story";
import { PRODUCT, SYNC_VIDEO, type Download, type Platform } from "@/config";

export function SiteHeader({ stars }: { stars: number | null }) {
  const { copy } = useSite();
  return (
    <header className="page head">
      <a className="wordmark" href="/">
        <img src="/logo-256.png?v=2" alt="" width={22} height={22} />
        Posthaste
      </a>
      <nav className="head__nav" aria-label="Sections">
        {storyVisible() && <a href="/story/">{copy.nav.why}</a>}
        <a href="#features">{copy.nav.features}</a>
        <a href="#looks">{copy.nav.looks}</a>
        <a href="#compare">{copy.nav.compare}</a>
        <a href="#changelog">{copy.nav.changelog}</a>
      </nav>
      <span className="head__spacer" />
      <LangToggle />
      <a className="head__link" href={PRODUCT.discordUrl} rel="noopener noreferrer">
        <ChatIcon />
        Discord
      </a>
      <StarChip baked={stars} />
      <a className="head__icon" href={PRODUCT.repoUrl} aria-label="Posthaste on GitHub">
        <GitHubMark />
      </a>
    </header>
  );
}

/** Where each hero fact links, in the same order as the words in i18n. */
const FACT_ANCHORS = ["#free", "#features", "#features", "#local", "#looks", "#ai", "#features"];

export function HeroSection({ downloads }: { downloads: Record<Platform, Download | null> }) {
  const { copy } = useSite();
  return (
    <section className="page hero" aria-labelledby="hero-title">
      <KeyboardBackdrop />
      <p className="dict">
        <dfn>posthaste</dfn> <span className="dict__pos">{copy.hero.pos}</span>
        <span className="dict__def">{copy.hero.def}</span>
        {storyVisible() && (
          <a className="dict__see" href="/story/">
            {copy.hero.whyLink}
          </a>
        )}
      </p>
      <div className="hero__top">
        <h1 className="hero__title" id="hero-title">
          {copy.hero.title}
        </h1>
        <div className="hero__side">
          <p className="hero__sub">{copy.hero.sub1}</p>
          <p className="hero__sub">{copy.hero.sub2}</p>
          {copy.hero.sub3 && <p className="hero__sub">{copy.hero.sub3}</p>}
          <noscript>
            <p className="hero__sub">Turn on JavaScript to try the box below.</p>
          </noscript>
        </div>
      </div>
      {/* One anchor per fact, in the same order as the words in i18n. */}
      <ul className="facts" aria-label="Posthaste in short">
        {copy.hero.facts.map((label, i) => (
          <li key={label}>
            <a href={FACT_ANCHORS[i]}>{label}</a>
          </li>
        ))}
      </ul>
      <HeroActions downloads={downloads} />
      <StoryHeroLink />
      <CaptureHero />
    </section>
  );
}

export function LooksSection() {
  const { copy } = useSite();
  return (
    <section className="page looks" id="looks" aria-labelledby="looks-title">
      <h2 className="h2" id="looks-title">
        {copy.looks.heading}
      </h2>
      <p className="looks__lede">{copy.looks.lede}</p>
      <LookTiles />
      <div className="looks__tools">
        <div>
          <h3 className="h3">{copy.looks.makeOwn}</h3>
          <p className="quiet">{copy.looks.makeOwnHelp}</p>
          <ContrastToy />
        </div>
        <div>
          <h3 className="h3">{copy.looks.share}</h3>
          <ShareCodeRow />
        </div>
      </div>
    </section>
  );
}

export function LocalSection() {
  const { copy } = useSite();
  return (
    <section className="page local" id="local" aria-labelledby="local-title">
      <h2 className="h2" id="local-title">
        {copy.local.heading}
      </h2>
      <div className="local__cols">
        <div>
          <h3 className="h3">{copy.local.staysTitle}</h3>
          <ul className="ledger">
            {copy.local.stays.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
        <div>
          <h3 className="h3">{copy.local.leavesTitle}</h3>
          <ul className="ledger">
            {copy.local.leaves.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}

export function SyncSection() {
  const { copy } = useSite();
  return (
    <section className="page watch sync" id="sync" aria-labelledby="sync-title">
      <h2 className="h2" id="sync-title">
        {copy.sync.heading}
      </h2>
      <p className="watch__lede">{copy.sync.lede}</p>
      <div className="watch__frame">
        <video
          className="watch__video"
          controls
          playsInline
          preload="none"
          poster={SYNC_VIDEO.poster}
          aria-label={copy.sync.filmLabel}
        >
          <source src={SYNC_VIDEO.src} type="video/mp4" />
          <a href={SYNC_VIDEO.src}>MP4</a>
        </video>
      </div>
      <ul className="ledger sync__points">
        {copy.sync.points.map((line) => (
          <li key={line}>{line}</li>
        ))}
      </ul>
      <p className="quiet sync__phone">{copy.sync.phoneNote}</p>
    </section>
  );
}

export function GetSection({
  downloads,
  linuxExtras,
}: {
  downloads: Record<Platform, Download | null>;
  linuxExtras: Download[];
}) {
  const { copy } = useSite();
  return (
    <section className="page get" id="get" aria-labelledby="get-title">
      <h2 className="h2" id="get-title">
        {copy.get.heading}
      </h2>
      <p className="quiet">
        {copy.get.metaPrefix} {PRODUCT.version}.{" "}
        <a href={PRODUCT.releasesUrl}>{copy.get.allFiles}</a>
      </p>
      <Downloads
        downloads={downloads}
        linuxExtras={linuxExtras}
        releasesUrl={PRODUCT.releasesUrl}
      />
      <p className="get__note">{copy.get.note}</p>
      <details className="terminal">
        <summary>{copy.get.buildSummary}</summary>
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
  );
}
