import { describe, expect, it } from "vitest";
import { BLOCKS } from "../src/lib/blocks";
import { demoReducer, doneNotes, initialDemo, searchNotes, todayView } from "../src/lib/demo";
import { TRASH_MS, trashReducer, type TrashNote } from "../src/lib/trash";

describe("moments row", () => {
  it("ticking a note off moves it to Done and raises the Today count", () => {
    let s = initialDemo();
    s = demoReducer(s, { type: "input", text: "Call the dentist", now: 0 });
    s = demoReducer(s, { type: "save", now: 1500 });
    const before = todayView(s).done;
    const id = s.notes[0]!.id;
    s = demoReducer(s, { type: "toggle", id });
    expect(todayView(s).done).toBe(before + 1);
    expect(doneNotes(s).map((n) => n.id)).toContain(id);
    s = demoReducer(s, { type: "toggle", id });
    expect(doneNotes(s).map((n) => n.id)).not.toContain(id);
  });
});

describe("inbox search", () => {
  const notes = [
    { text: "Ship v0.2.3 on Friday" },
    { text: "Call Minh" },
    { text: "Friday lunch" },
  ];
  it("filters as you type, every word must match, case ignored", () => {
    expect(searchNotes(notes, "friday")).toHaveLength(2);
    expect(searchNotes(notes, "FRIDAY ship")).toEqual([notes[0]]);
    expect(searchNotes(notes, "  ")).toEqual(notes);
  });
});

describe("trash", () => {
  const start: TrashNote[] = [{ id: 1, text: "a", deletedAt: null }];
  it("undo restores a deleted note within 30 days", () => {
    const del = trashReducer(start, { type: "delete", id: 1, now: 1000 });
    expect(del[0]?.deletedAt).toBe(1000);
    expect(
      trashReducer(del, { type: "undo", id: 1, now: 1000 + TRASH_MS - 1 })[0]?.deletedAt,
    ).toBeNull();
  });
  it("after 30 days undo does nothing and purge removes it", () => {
    const del = trashReducer(start, { type: "delete", id: 1, now: 0 });
    expect(trashReducer(del, { type: "undo", id: 1, now: TRASH_MS })[0]?.deletedAt).toBe(0);
    expect(trashReducer(del, { type: "purge", now: TRASH_MS })).toHaveLength(0);
  });
});

describe("feature blocks copy", () => {
  it("every block has a heading and a sentence, with no stock phrases", () => {
    expect(BLOCKS.length).toBeGreaterThanOrEqual(5);
    for (const b of BLOCKS) {
      expect(b.title.trim()).not.toBe("");
      expect(b.fact).toMatch(/[.!?]$/);
      expect(b.fact).not.toMatch(/seamless|effortless|in a blink|supercharge|unlock|revolutioni/i);
    }
  });
});
