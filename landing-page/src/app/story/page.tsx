import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { SiteFooter } from "@/components/SiteFooter";
import { PRODUCT } from "@/config";
import { STORY, storyVisible } from "@/lib/story";

const TITLE = `${STORY.title} | The story behind the app`;

export const metadata: Metadata = {
  title: TITLE,
  description: STORY.description,
  openGraph: {
    title: TITLE,
    description: STORY.description,
    type: "article",
    images: ["/og.png"],
  },
  twitter: {
    card: "summary_large_image",
    title: TITLE,
    description: STORY.description,
    images: ["/og.png"],
  },
  alternates: { canonical: "/story/" },
  robots: STORY.published ? undefined : { index: false, follow: false },
};

export default function StoryPage() {
  if (!storyVisible()) notFound();

  return (
    <>
      <header className="page head">
        <a className="wordmark" href="/">
          <img src="/logo-256.png?v=2" alt="" width={22} height={22} />
          Posthaste
        </a>
        <span className="head__spacer" />
        <a className="btn btn--small" href="/#get">
          Download
        </a>
      </header>

      <main className="page story">
        <p className="dict">
          <dfn>posthaste</dfn> <span className="dict__pos">adverb</span>
          <span className="dict__def">with great speed; right away.</span>
        </p>
        <h1 className="story__title">{STORY.title}.</h1>
        <p className="story__lede">{STORY.lede}</p>

        <div className="story__beats">
          {STORY.beats.map((beat) => (
            <section className="story__beat" key={beat.id} aria-labelledby={`story-${beat.id}`}>
              <h2 className="h3 story__heading" id={`story-${beat.id}`}>
                {beat.heading}
              </h2>
              <div className="story__body">
                {beat.body.map((paragraph) => (
                  <p key={paragraph}>{paragraph}</p>
                ))}
              </div>
            </section>
          ))}
        </div>

        <div className="story__end">
          <p className="story__lede">It is free: no plans, no trial, no account.</p>
          <div className="hero__actions">
            <a className="btn btn--cta" href="/#get">
              Download Posthaste
            </a>
            <a className="btn btn--ghost" href={PRODUCT.discordUrl} rel="noopener noreferrer">
              Say hello on Discord
            </a>
            <a className="btn btn--ghost" href={PRODUCT.repoUrl} rel="noopener noreferrer">
              Star on GitHub
            </a>
          </div>
        </div>
      </main>

      <SiteFooter />
    </>
  );
}
