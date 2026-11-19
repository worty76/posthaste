"use client";

import { AiPolishDemo, CaptureMini, DetailsDemo, InboxDemo, UndoDemo } from "./MiniDemos";
import { ShortcutSheet } from "./ShortcutSheet";
import { useSite } from "./SiteState";
import { BLOCKS_BY_LANG, type BlockId } from "@/lib/blocks";

/** What the app does, one working piece per block. Wide and narrow blocks
    alternate so the section isn't a grid of identical cards. */
export function Features() {
  const { lang, copy } = useSite();
  const blocks = BLOCKS_BY_LANG[lang];

  const demos: Partial<Record<BlockId, React.ReactNode>> = {
    capture: <CaptureMini />,
    inbox: <InboxDemo />,
    details: <DetailsDemo />,
    undo: <UndoDemo />,
    ai: <AiPolishDemo />,
    looks: (
      <p className="mini mini--looks">
        <a href="#looks">{copy.features.looksLink}</a>
        {copy.features.looksTail}
      </p>
    ),
    keys: <ShortcutSheet />,
  };

  return (
    <section className="page features" id="features" aria-labelledby="features-title">
      <h2 className="h2" id="features-title">
        {copy.features.heading}
      </h2>
      <div className="features__grid">
        {blocks.map((b) => (
          <article
            key={b.id}
            className={`feature feature--${b.id}`}
            id={b.id === "ai" ? "ai" : undefined}
          >
            <h3 className="h3">{b.title}</h3>
            <p className="feature__fact">{b.fact}</p>
            {demos[b.id]}
          </article>
        ))}
      </div>
    </section>
  );
}
