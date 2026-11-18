import { STORY, storyHeroLine, storyVisible } from "@/lib/story";

/** The maker's teaser under the hero buttons. Nothing renders without one. */
export function StoryHeroLink() {
  const teaser = storyHeroLine();
  if (!teaser) return null;
  return (
    <p className="hero__why">
      <span>{teaser} </span>
      <a href="/story/">Read the story</a>
    </p>
  );
}

/** The maker's short note above the free card. Needs the story visible and a note. */
export function MakerNote() {
  if (!storyVisible() || !STORY.note) return null;
  return (
    <section className="page maker" id="maker" aria-labelledby="maker-title">
      <h2 className="h3" id="maker-title">
        A note from the maker
      </h2>
      <p className="maker__note">{STORY.note}</p>
      <a className="btn btn--ghost" href="/story/">
        Read the whole story
      </a>
    </section>
  );
}
