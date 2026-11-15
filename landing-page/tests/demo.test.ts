import { describe, expect, it } from "vitest";
import {
  MAX_NOTE_CHARS,
  demoReducer,
  formatSeconds,
  initialDemo,
  savedStack,
  todayView,
  type DemoState,
} from "@/lib/demo";

function typeThenSave(state: DemoState, text: string, start: number, end: number) {
  let s = demoReducer(state, { type: "input", text: text.slice(0, 1), now: start });
  s = demoReducer(s, { type: "input", text, now: start + 50 });
  return demoReducer(s, { type: "save", now: end });
}

describe("capture demo", () => {
  it("times from the first keystroke to Enter", () => {
    const s = typeThenSave(initialDemo(), "Buy oat milk", 1000, 2840);
    expect(s.lastSavedMs).toBe(1840);
    expect(s.draft).toBe("");
    expect(s.startedAt).toBeNull();
    expect(savedStack(s)[0]).toMatchObject({ text: "Buy oat milk", savedInMs: 1840 });
  });

  it("does not start the clock on whitespace, and resets it when the field is emptied", () => {
    let s = demoReducer(initialDemo(), { type: "input", text: "  ", now: 100 });
    expect(s.startedAt).toBeNull();
    s = demoReducer(s, { type: "input", text: "a", now: 200 });
    expect(s.startedAt).toBe(200);
    s = demoReducer(s, { type: "input", text: "", now: 300 });
    expect(s.startedAt).toBeNull();
    s = demoReducer(s, { type: "input", text: "b", now: 900 });
    s = demoReducer(s, { type: "save", now: 1400 });
    expect(s.lastSavedMs).toBe(500);
  });

  it("ignores Enter on an empty or blank field", () => {
    const start = initialDemo();
    const s = demoReducer(demoReducer(start, { type: "input", text: "   ", now: 1 }), {
      type: "save",
      now: 2,
    });
    expect(s.notes).toEqual(start.notes);
    expect(s.lastSavedMs).toBeNull();
  });

  it("trims what it saves and caps the length", () => {
    const long = "x".repeat(MAX_NOTE_CHARS + 20);
    let s = demoReducer(initialDemo(), { type: "input", text: long, now: 0 });
    expect(s.draft).toHaveLength(MAX_NOTE_CHARS);
    s = typeThenSave(initialDemo(), "  hello  ", 0, 10);
    expect(savedStack(s)[0]?.text).toBe("hello");
  });

  it("clear empties the field without saving", () => {
    let s = demoReducer(initialDemo(), { type: "input", text: "half a thought", now: 0 });
    s = demoReducer(s, { type: "clear" });
    expect(s.draft).toBe("");
    expect(savedStack(s)).toHaveLength(0);
  });

  it("keeps only the most recent saves in the stack, newest first", () => {
    let s = initialDemo();
    for (let i = 1; i <= 6; i++) s = typeThenSave(s, `note ${i}`, i * 1000, i * 1000 + 500);
    expect(savedStack(s).map((n) => n.text)).toEqual(["note 6", "note 5", "note 4", "note 3"]);
  });
});

describe("formatSeconds", () => {
  it("rounds to one decimal", () => {
    expect(formatSeconds(1840)).toBe("1.8 s");
    expect(formatSeconds(1850)).toBe("1.9 s");
    expect(formatSeconds(12_049)).toBe("12.0 s");
  });

  it("never shows zero for a real save", () => {
    expect(formatSeconds(0)).toBe("0.1 s");
    expect(formatSeconds(40)).toBe("0.1 s");
  });
});

describe("Today window demo", () => {
  it("counts the seeded notes: two carried and open, one done today", () => {
    const v = todayView(initialDemo());
    expect(v.carried).toHaveLength(2);
    expect(v.done).toBe(1);
    expect(v.total).toBe(3);
  });

  it("adds what the visitor captures to today, open before done", () => {
    const s = typeThenSave(initialDemo(), "Call the dentist", 0, 900);
    const v = todayView(s);
    expect(v.total).toBe(4);
    expect(v.today.map((n) => n.text)).toEqual(["Call the dentist", "Return the library books"]);
  });

  it("completing a carried note moves it to today and counts it", () => {
    const start = initialDemo();
    const carriedId = todayView(start).carried[0]!.id;
    const v = todayView(demoReducer(start, { type: "toggle", id: carriedId }));
    expect(v.carried).toHaveLength(1);
    expect(v.today.some((n) => n.id === carriedId && n.done)).toBe(true);
    expect(v.done).toBe(2);
    expect(v.total).toBe(3);
  });

  it("un-completing a note takes it out of the count again", () => {
    const start = initialDemo();
    const doneId = start.notes.find((n) => n.done)!.id;
    const v = todayView(demoReducer(start, { type: "toggle", id: doneId }));
    expect(v.done).toBe(0);
    expect(v.total).toBe(3);
  });
});
