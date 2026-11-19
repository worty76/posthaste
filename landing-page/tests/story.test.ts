import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { STORY, storyHeroLine, storyVisible } from "../src/lib/story";

const texts = [
  STORY.title,
  STORY.description,
  STORY.lede,
  STORY.teaser,
  STORY.note,
  ...STORY.beats.flatMap((b) => [b.heading, ...b.body]),
];

afterEach(() => vi.unstubAllEnvs());

describe("story copy", () => {
  it("has beats with unique ids, headings and non-empty paragraphs", () => {
    expect(STORY.beats.length).toBeGreaterThan(0);
    expect(new Set(STORY.beats.map((b) => b.id)).size).toBe(STORY.beats.length);
    for (const b of STORY.beats) {
      expect(b.heading.trim()).not.toBe("");
      expect(b.body.length).toBeGreaterThan(0);
      for (const p of b.body) {
        expect(p.trim()).not.toBe("");
        expect(p.length).toBeLessThanOrEqual(600);
      }
    }
  });

  it("keeps the optional lines short", () => {
    expect(STORY.teaser.length).toBeLessThanOrEqual(140);
    expect(STORY.note.length).toBeLessThanOrEqual(400);
  });

  it("holds no placeholder text", () => {
    for (const t of texts) expect(t).not.toMatch(/\b(todo|tbd|lorem|fixme)\b/i);
  });

  it("invents no proof: no figures with a unit, no currency, no superlatives", () => {
    const forbidden =
      /[$€£]\s?\d|\d\s?%|\b(best|fastest|#1|leading|trusted by|loved by|thousands|millions|award)/i;
    for (const t of texts) expect(t).not.toMatch(forbidden);
  });

  it("needs a lede once published", () => {
    if (STORY.published) expect(STORY.lede.trim()).not.toBe("");
  });
});

describe("story switch", () => {
  it("is visible when published, or when previewing", () => {
    vi.stubEnv("STORY_PREVIEW", "");
    expect(storyVisible()).toBe(STORY.published);
    vi.stubEnv("STORY_PREVIEW", "1");
    expect(storyVisible()).toBe(true);
  });

  it("gives the hero teaser only while visible and written", () => {
    vi.stubEnv("STORY_PREVIEW", "");
    expect(storyHeroLine() === null).toBe(!STORY.published || !STORY.teaser);
    vi.stubEnv("STORY_PREVIEW", "1");
    expect(storyHeroLine()).toBe(STORY.teaser || null);
  });

  it("is linked only through the visibility check", () => {
    const read = (f: string) => readFileSync(join(__dirname, "../src", f), "utf8");
    expect(read("components/LocalizedSections.tsx")).toMatch(/storyVisible\(\) && <a href="\/story\/"/);
    expect(read("components/LocalizedSections.tsx")).toMatch(/storyVisible\(\) && \(\s*<a className="dict__see"/);
    expect(read("components/SiteFooter.tsx")).toMatch(/storyVisible\(\) && <a href="\/story\/"/);
    expect(read("components/StoryLinks.tsx")).toMatch(/storyHeroLine\(\)/);
    expect(read("app/story/page.tsx")).toMatch(/if \(!storyVisible\(\)\) notFound\(\)/);
  });
});
