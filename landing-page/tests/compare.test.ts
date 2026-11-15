import { describe, expect, it } from "vitest";
import {
  COMPARE_APPS,
  COMPARE_CHECKED_ON,
  COMPARE_ROWS,
  PICK_OTHERS,
  PICK_POSTHASTE,
  VS_TODOGLOW,
} from "../src/lib/compare";

const allText = [
  ...COMPARE_ROWS.flatMap((r) => [r.label, ...Object.values(r.cells)]),
  ...PICK_POSTHASTE,
  ...PICK_OTHERS,
  ...VS_TODOGLOW.ours,
  ...VS_TODOGLOW.theirs,
].join("\n");

describe("comparison data", () => {
  it("puts Posthaste first, then eight other apps with unique ids", () => {
    expect(COMPARE_APPS[0]?.id).toBe("posthaste");
    expect(COMPARE_APPS).toHaveLength(9);
    expect(new Set(COMPARE_APPS.map((a) => a.id)).size).toBe(COMPARE_APPS.length);
  });

  it("fills every cell for every app, with no empty or placeholder text", () => {
    for (const row of COMPARE_ROWS) {
      for (const app of COMPARE_APPS) {
        const cell = row.cells[app.id];
        expect(cell, `${row.label} / ${app.id}`).toBeTruthy();
        expect(cell.trim()).not.toBe("");
        expect(cell).not.toMatch(/^(n\/a|tbd|\?|-|—)$/i);
      }
      expect(Object.keys(row.cells).sort()).toEqual(COMPARE_APPS.map((a) => a.id).sort());
    }
  });

  it("states no prices or quantities that go stale (sources disagree and plans change)", () => {
    expect(allText).not.toMatch(/[$€£]/);
    expect(allText).not.toMatch(/\d+\s*(%|\/\s?(mo|month|yr|year))/i);
  });

  it("makes no unmeasurable claims: no speed numbers, no superlatives", () => {
    expect(allText).not.toMatch(
      /\b(fastest|best|faster than|slower|instant(ly)?|seamless|effortless)\b/i,
    );
    expect(allText).not.toMatch(/\d+\s?(ms|milliseconds?|seconds?)\b/i);
  });

  it("is dated, and links every competitor to an https page of its own", () => {
    expect(COMPARE_CHECKED_ON).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    for (const app of COMPARE_APPS.slice(1)) {
      expect(app.url, app.id).toMatch(/^https:\/\//);
    }
  });

  it("includes a row where Posthaste is the one giving something up", () => {
    const sync = COMPARE_ROWS.find((r) => /sync|phone/i.test(r.label));
    expect(sync).toBeDefined();
    expect(sync?.cells.posthaste).toMatch(/no sync|no phone/i);
  });

  it("says where the other apps are the better pick, as well as where Posthaste is", () => {
    expect(PICK_POSTHASTE.length).toBeGreaterThanOrEqual(2);
    expect(PICK_OTHERS.length).toBeGreaterThanOrEqual(2);
  });

  it("sets Posthaste next to TodoGlow in both directions, without claiming what its page does not say", () => {
    expect(VS_TODOGLOW.ours.length).toBeGreaterThanOrEqual(3);
    expect(VS_TODOGLOW.theirs.length).toBeGreaterThanOrEqual(3);
    // Absence is only ever "doesn't list" / "page doesn't say", never "no X" about TodoGlow.
    for (const line of VS_TODOGLOW.ours) expect(line).not.toMatch(/todoglow (has|offers) no\b/i);
    expect(COMPARE_APPS.find((a) => a.id === "todoglow")?.url).toBe("https://devglow.app/todoglow");
  });
});
