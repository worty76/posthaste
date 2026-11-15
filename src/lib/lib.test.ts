import {
  attachmentKind,
  batchRefusal,
  formatBytes,
  pastedImageName,
  toDataUrl,
  typeLabel,
} from "./attachments";
import { describe, expect, it, vi } from "vitest";
import {
  dayKey,
  dayLabel,
  dayRange,
  dayRangeFromInput,
  daysBetween,
  erasedOn,
  groupByDeletedDay,
  groupByDay,
  monthEdges,
  monthGrid,
  originLabel,
  weekStart,
} from "./dates";
import { DraftSync } from "./draftSync";
import { buildQuery, isSearching, NO_FILTERS } from "./query";
import { acceleratorFromEvent, formatAccelerator } from "./shortcut";
import { resolveTheme } from "../hooks/useTheme";
import type { Note } from "./types";

const at = (y: number, m: number, d: number, h = 12) => new Date(y, m - 1, d, h).getTime();
const n = (content: string, created_at: number) => ({ content, created_at }) as Note;

describe("dates", () => {
  it("bounds a local calendar day, start inclusive and end exclusive", () => {
    const { from, to } = dayRange(at(2026, 11, 13, 21));
    expect(new Date(from)).toEqual(new Date(2026, 10, 13, 0, 0, 0, 0));
    expect(new Date(to)).toEqual(new Date(2026, 10, 14, 0, 0, 0, 0));
  });

  it("reads a date input value as that local day", () => {
    expect(dayRangeFromInput("2026-11-13")).toEqual(dayRange(at(2026, 11, 13)));
    expect(dayRangeFromInput("")).toBeNull();
    expect(dayRangeFromInput("13/11/2026")).toBeNull();
  });

  it("labels days relative to now", () => {
    const now = at(2026, 11, 13, 9);
    expect(dayLabel(at(2026, 11, 13, 1), now)).toBe("Today");
    // Late yesterday is "Yesterday" even though it is under 24 hours ago.
    expect(dayLabel(at(2026, 11, 12, 23), now)).toBe("Yesterday");
    expect(dayLabel(at(2026, 11, 9), now, "en-US")).toBe("Monday, November 9");
    expect(dayLabel(at(2025, 12, 31), now, "en-US")).toBe("Wednesday, December 31, 2025");
  });

  it("groups notes by capture day, keeping order", () => {
    const now = at(2026, 11, 13, 20);
    const groups = groupByDay(
      [n("c", at(2026, 11, 13, 19)), n("b", at(2026, 11, 13, 8)), n("a", at(2026, 11, 12, 23))],
      now,
    );
    expect(groups.map((g) => [g.label, g.notes.map((x) => x.content)])).toEqual([
      ["Today", ["c", "b"]],
      ["Yesterday", ["a"]],
    ]);
    expect(groupByDay([], now)).toEqual([]);
  });

  it("keys a day as local YYYY-MM-DD", () => {
    expect(dayKey(at(2026, 11, 9, 23))).toBe("2026-11-09");
    expect(dayKey(at(2026, 1, 1, 0))).toBe("2026-01-01");
  });

  it("lays out a month as six weeks from the chosen first weekday", () => {
    // 1 November 2026 is a Sunday.
    const mon = monthGrid(2026, 10, 1);
    expect(mon).toHaveLength(42);
    expect(mon[0]).toMatchObject({ key: "2026-10-26", inMonth: false });
    expect(mon[6]).toMatchObject({ key: "2026-11-01", inMonth: true });
    const sun = monthGrid(2026, 10, 0);
    expect(sun[0]).toMatchObject({ key: "2026-11-01", inMonth: true });
    expect(sun.filter((c) => c.inMonth)).toHaveLength(30);
    expect(monthGrid(2026, 10, 6)[0]!.key).toBe("2026-10-31");
  });

  it("gives month edges at every local midnight, one day apart, DST included", () => {
    // March holds a DST change in most zones that have one.
    const grid = monthGrid(2026, 2, 1);
    const edges = monthEdges(grid);
    expect(edges).toHaveLength(43);
    for (let i = 0; i < 42; i++) {
      const span = edges[i + 1]! - edges[i]!;
      expect(span).toBeGreaterThanOrEqual(23 * 3_600_000);
      expect(span).toBeLessThanOrEqual(25 * 3_600_000);
      expect(dayKey(edges[i]!)).toBe(grid[i]!.key);
    }
  });

  it("counts whole calendar days between two times", () => {
    expect(daysBetween(at(2026, 11, 9, 23), at(2026, 11, 15, 1))).toBe(6);
    expect(daysBetween(at(2026, 11, 15, 1), at(2026, 11, 15, 23))).toBe(0);
  });

  it("says where a carried note came from", () => {
    const now = at(2026, 11, 15, 9);
    const today = at(2026, 11, 15);
    expect(originLabel(at(2026, 11, 15, 2), today, now, "en-GB")).toBeNull();
    expect(originLabel(at(2026, 11, 14, 22), today, now, "en-GB")).toBe("Yesterday");
    expect(originLabel(at(2026, 11, 9, 9), today, now, "en-GB")).toBe("Mon 9 Nov · 6 days ago");
    // Viewing a past day: just the date, since "days ago" would mislead.
    expect(originLabel(at(2026, 11, 9, 9), at(2026, 11, 12), now, "en-GB")).toBe("Mon 9 Nov");
    expect(originLabel(at(2025, 12, 30, 9), today, now, "en-GB")).toBe(
      "Tue 30 Dec 2025 · 320 days ago",
    );
  });

  it("groups trashed notes by the day they were deleted, keeping order", () => {
    const now = at(2026, 11, 15, 20);
    const items = [
      { id: "c", deleted_at: at(2026, 11, 15, 9) },
      { id: "b", deleted_at: at(2026, 11, 14, 22) },
      { id: "a", deleted_at: at(2026, 11, 9, 8) },
    ];
    const groups = groupByDeletedDay(items, now, "en-US");
    expect(groups.map((g) => [g.label, g.items.map((i) => i.id)])).toEqual([
      ["Today", ["c"]],
      ["Yesterday", ["b"]],
      ["Monday, November 9", ["a"]],
    ]);
    expect(groupByDeletedDay([], now)).toEqual([]);
  });

  it("says when a trashed note will be erased, across a month end", () => {
    expect(erasedOn(at(2026, 11, 15, 9), 30 * 86_400_000, "en-GB")).toBe("Erased on 15 Dec");
    expect(erasedOn(at(2026, 12, 20, 9), 30 * 86_400_000, "en-GB")).toBe("Erased on 19 Jan 2027");
  });

  it("starts the week on Monday when the locale says nothing", () => {
    expect(weekStart("xx")).toBe(1);
    expect([0, 1, 6]).toContain(weekStart("en-US"));
  });
});

describe("query", () => {
  const now = at(2026, 11, 13);

  it("scopes each view when not searching", () => {
    expect(buildQuery("inbox", NO_FILTERS, now)).toEqual({ statuses: ["inbox"], later: false });
    expect(buildQuery("later", NO_FILTERS, now)).toEqual({ statuses: ["inbox"], later: true });
    expect(buildQuery("archive", NO_FILTERS, now)).toEqual({ statuses: ["completed", "archived"] });
    const { from, to } = dayRange(now);
    expect(buildQuery("today", NO_FILTERS, now)).toEqual({
      created_from: from,
      created_to: to,
      later: false,
    });
  });

  it("searches every note, including the archive, whatever the view", () => {
    const q = buildQuery("later", { ...NO_FILTERS, text: "  index " }, now);
    expect(q).toMatchObject({ text: "index", statuses: [] });
    // A search ignores the Later split: parked notes are found too.
    expect(q.later).toBeUndefined();
  });

  it("applies the status and date filters to a search", () => {
    const q = buildQuery("inbox", { text: "x", status: "open", date: "2026-11-01" }, now);
    expect(q.statuses).toEqual(["inbox"]);
    expect(q.created_from).toBe(dayRange(at(2026, 11, 1)).from);
    expect(buildQuery("inbox", { text: "x", status: "completed", date: "" }, now).statuses).toEqual(
      ["completed"],
    );
  });

  it("treats a date alone as a search, and whitespace as none", () => {
    expect(isSearching({ ...NO_FILTERS, date: "2026-11-01" })).toBe(true);
    expect(isSearching({ ...NO_FILTERS, text: "   " })).toBe(false);
  });
});

describe("shortcut", () => {
  const key = (code: string, mods: Partial<KeyboardEvent> = {}) => ({
    code,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...mods,
  });

  it("builds an accelerator from a key press", () => {
    expect(acceleratorFromEvent(key("Space", { ctrlKey: true, shiftKey: true }), "linux")).toBe(
      "Ctrl+Shift+Space",
    );
    expect(acceleratorFromEvent(key("KeyN", { metaKey: true, ctrlKey: true }), "macos")).toBe(
      "Cmd+Ctrl+N",
    );
    expect(acceleratorFromEvent(key("Digit1", { altKey: true }), "windows")).toBe("Alt+1");
    expect(acceleratorFromEvent(key("F9"), "windows")).toBe("F9");
  });

  it("waits while only modifiers are held and rejects unusable combinations", () => {
    expect(acceleratorFromEvent(key("ShiftLeft", { shiftKey: true }), "linux")).toBeNull();
    expect(acceleratorFromEvent(key("KeyA"), "linux")).toBeNull();
    expect(acceleratorFromEvent(key("KeyA", { shiftKey: true }), "linux")).toBeNull();
    expect(acceleratorFromEvent(key("Unidentified", { ctrlKey: true }), "linux")).toBeNull();
  });

  it("shows key caps in the platform's own notation", () => {
    expect(formatAccelerator("CmdOrCtrl+Shift+Space", "macos")).toEqual(["⌘", "⇧", "Space"]);
    expect(formatAccelerator("CmdOrCtrl+Shift+Space", "windows")).toEqual([
      "Ctrl",
      "Shift",
      "Space",
    ]);
    expect(formatAccelerator("Cmd+Ctrl+N", "macos")).toEqual(["⌘", "⌃", "N"]);
    expect(formatAccelerator("Super+Alt+ArrowUp", "windows")).toEqual(["Win", "Alt", "↑"]);
  });
});

describe("theme", () => {
  it("follows the system only when asked to", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
});

describe("DraftSync", () => {
  it("writes only the latest text after the pause", async () => {
    vi.useFakeTimers();
    const write = vi.fn().mockResolvedValue(undefined);
    const sync = new DraftSync(write, vi.fn(), 250);
    sync.schedule("h", null);
    sync.schedule("he", null);
    sync.schedule("hello", "ideas");
    expect(write).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(250);
    expect(write).toHaveBeenCalledTimes(1);
    expect(write).toHaveBeenCalledWith("hello", "ideas");
    vi.useRealTimers();
  });

  it("flush writes immediately", async () => {
    const write = vi.fn().mockResolvedValue(undefined);
    const sync = new DraftSync(write, vi.fn(), 10_000);
    sync.schedule("urgent", null);
    await sync.flush();
    expect(write).toHaveBeenCalledWith("urgent", null);
  });

  it("cancel drops pending text but waits for a write already in flight", async () => {
    let finish!: () => void;
    const write = vi.fn(() => new Promise<void>((r) => (finish = r)));
    const sync = new DraftSync(write, vi.fn(), 10_000);
    sync.schedule("first", null);
    void sync.flush();
    await Promise.resolve();
    sync.schedule("second", null);

    let cancelled = false;
    const done = sync.cancel().then(() => (cancelled = true));
    await Promise.resolve();
    expect(cancelled).toBe(false); // still waiting on "first"
    finish();
    await done;
    expect(write).toHaveBeenCalledTimes(1); // "second" was never written
  });

  it("reports a failed write and keeps working", async () => {
    const onError = vi.fn();
    const write = vi
      .fn()
      .mockRejectedValueOnce(new Error("disk full"))
      .mockResolvedValue(undefined);
    const sync = new DraftSync(write, onError, 10_000);
    sync.schedule("a", null);
    await sync.flush();
    expect(onError).toHaveBeenCalledTimes(1);
    sync.schedule("b", null);
    await sync.flush();
    expect(write).toHaveBeenLastCalledWith("b", null);
  });
});

describe("attachments", () => {
  it("formats sizes the way a person reads them", () => {
    expect(formatBytes(860)).toBe("860 B");
    expect(formatBytes(88_064)).toBe("86 KB");
    expect(formatBytes(2_516_582)).toBe("2.4 MB");
    expect(formatBytes(15 * 1024 * 1024)).toBe("15 MB");
  });

  it("sorts files into kinds and short type labels", () => {
    expect(attachmentKind({ mime: "image/png" })).toBe("image");
    expect(attachmentKind({ mime: "application/pdf" })).toBe("pdf");
    expect(attachmentKind({ mime: "text/plain" })).toBe("text");
    expect(attachmentKind({ mime: "application/zip" })).toBe("other");
    expect(typeLabel({ name: "server.log", mime: "text/plain" })).toBe("LOG");
    expect(typeLabel({ name: "notes", mime: "application/octet-stream" })).toBe("FILE");
    expect(typeLabel({ name: ".env", mime: "application/octet-stream" })).toBe("FILE");
  });

  it("refuses a batch only when the note is already full", () => {
    expect(batchRefusal(1, 19)).toBeNull();
    expect(batchRefusal(3, 20)).toMatch(/already has 20 files/);
    expect(batchRefusal(0, 20)).toBeNull();
  });

  it("encodes bytes as a data URL, also past the chunk size", () => {
    expect(toDataUrl(new Uint8Array([1, 2, 3]), "image/png")).toBe("data:image/png;base64,AQID");
    const big = new Uint8Array(0x8000 * 2 + 5).fill(65);
    const url = toDataUrl(big, "image/png");
    expect(atob(url.split(",")[1]!)).toHaveLength(big.length);
  });

  it("names pasted images by date and type", () => {
    const t = new Date(2026, 10, 15, 15, 4).getTime();
    expect(pastedImageName(t, "image/png")).toBe("Pasted image 2026-11-15 15.04.png");
    expect(pastedImageName(t, "image/jpeg")).toBe("Pasted image 2026-11-15 15.04.jpg");
  });
});
