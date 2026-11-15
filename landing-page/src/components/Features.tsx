import { AiPolishDemo, CaptureMini, InboxDemo, UndoDemo } from "./MiniDemos";
import { ShortcutSheet } from "./ShortcutSheet";
import { BLOCKS, type BlockId } from "@/lib/blocks";

const DEMOS: Partial<Record<BlockId, React.ReactNode>> = {
  capture: <CaptureMini />,
  inbox: <InboxDemo />,
  undo: <UndoDemo />,
  ai: <AiPolishDemo />,
  looks: (
    <p className="mini mini--looks">
      <a href="#looks">Try the six looks below</a>, or press <kbd>1</kbd> to <kbd>6</kbd> anywhere
      on this page.
    </p>
  ),
  keys: <ShortcutSheet />,
};

/** What the app does, one working piece per block. Wide and narrow blocks
    alternate so the section isn't a grid of identical cards. */
export function Features() {
  return (
    <section className="page features" id="features" aria-labelledby="features-title">
      <h2 className="h2" id="features-title">
        Small, and it does all of this.
      </h2>
      <div className="features__grid">
        {BLOCKS.map((b) => (
          <article
            key={b.id}
            className={`feature feature--${b.id}`}
            id={b.id === "ai" ? "ai" : undefined}
          >
            <h3 className="h3">{b.title}</h3>
            <p className="feature__fact">{b.fact}</p>
            {DEMOS[b.id]}
          </article>
        ))}
      </div>
    </section>
  );
}
