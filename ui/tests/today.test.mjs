import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, note, open, start, stop, text } from "./harness.mjs";

before(start);
after(stop);

// Sunday 15 November 2026, noon, in the harness's time zone (UTC+7).
const NOON = "2026-11-15T12:00:00+07:00";
const at = (iso) => new Date(`${iso}+07:00`).getTime();
const startOf = (day) => at(`${day}T00:00:00`);

function glance(patch = {}) {
  return { carried: [], today: [], done: 0, total: 0, ...patch };
}

/** Opens the Today window and waits for its first answer. */
async function today(options = {}) {
  const w = await open({ label: "today", now: NOON, ...options });
  await eventually(
    () => w.mock.calls("glance_get"),
    (c) => c.length > 0,
    "first load",
  );
  const rows = w.page.getByRole("listitem");
  const selected = w.page.locator('.today__row[aria-current="true"]');
  const key = (k) => w.page.keyboard.press(k);
  const calls = (cmd) => w.mock.calls(cmd);
  const lastBounds = async () => (await calls("glance_get")).at(-1).args;
  return { ...w, rows, selected, key, calls, lastBounds };
}

const three = () => {
  const a = note("pay rent", { created_at: at("2026-11-12T09:00:00") });
  const b = note("call mum", { created_at: at("2026-11-15T08:00:00") });
  const c = note("book dentist", { created_at: at("2026-11-15T09:00:00") });
  return { a, b, c, glance: glance({ carried: [a], today: [b, c], done: 0, total: 3 }) };
};

describe("today window", () => {
  it("renders carried-over and today groups, the count and the progress", async () => {
    const done = note("done one", { status: "completed", completed_at: at("2026-11-15T10:00:00") });
    const f = three();
    const w = await today({
      glance: glance({ carried: [f.a], today: [f.b, done], done: 1, total: 3 }),
    });
    await text(w.page.getByRole("region", { name: "Carried over" }), "pay rent");
    await text(w.page.locator('.today__group[aria-label="Today"]'), "call mum");
    await text(w.page.locator(".today__count"), "1 of 3 done");
    const bar = w.page.getByLabel("Notes done");
    assert.equal(await bar.evaluate((el) => `${el.value}/${el.max}`), "1/3");
    await text(w.page.locator(".today__date"), "Sunday 15 November");
    assert.deepEqual(await w.lastBounds(), {
      todayFrom: startOf("2026-11-15"),
      todayTo: startOf("2026-11-16"),
    });
    assert.equal(await w.page.getByRole("checkbox", { name: "Mark as not completed" }).count(), 1);
    assert.deepEqual(w.errors, []);
    await w.close();
  });

  it("recomputes the day bounds when shown again on a later day", async () => {
    const w = await today();
    await w.mock.setNow("2026-11-16T08:00:00+07:00");
    await w.mock.emit("today-shown");
    await eventually(
      w.lastBounds,
      (b) => b.todayFrom === startOf("2026-11-16"),
      "new day's bounds",
    );
    await text(w.page.locator(".today__date"), "Monday 16 November");
    await w.close();
  });

  it("toggles done with X on the selected carried-over note, and back", async () => {
    const f = three();
    const w = await today({ glance: f.glance });
    await w.rows.first().waitFor();
    await w.key("x");
    const [call] = await eventually(
      () => w.calls("note_set_status"),
      (c) => c.length === 1,
      "status set",
    );
    assert.deepEqual(call.args, { id: f.a.id, status: "completed" });

    await w.mock.setGlance(
      glance({ carried: [{ ...f.a, status: "completed", updated_at: 5 }], total: 1, done: 1 }),
    );
    await w.mock.emit("notes-changed");
    await text(w.page.locator(".today__count"), "All done");
    await w.key("x");
    const calls = await eventually(
      () => w.calls("note_set_status"),
      (c) => c.length === 2,
      "status set",
    );
    assert.equal(calls[1].args.status, "inbox");
    await w.close();
  });

  it("L sends the selected note to Later and selects the next row; on the last row, the one before", async () => {
    const f = three();
    const w = await today({ glance: f.glance });
    await w.rows.first().waitFor();
    await w.mock.setGlance(glance({ today: [f.b, f.c], total: 2 }));
    await w.key("l");
    const [call] = await eventually(
      () => w.calls("note_set_later"),
      (c) => c.length === 1,
      "sent to later",
    );
    assert.deepEqual(call.args, { id: f.a.id, later: true });
    await text(w.selected, "call mum");

    await w.key("ArrowDown");
    await text(w.selected, "book dentist");
    await w.mock.setGlance(glance({ today: [f.b], total: 1 }));
    await w.key("l");
    await text(w.selected, "call mum");
    await w.close();
  });

  it("ignores note keys with a modifier or while composing", async () => {
    const w = await today({ glance: three().glance });
    await w.rows.first().waitFor();
    await w.key("Control+l");
    await w.key("Control+x");
    await w.key("Control+Enter");
    await w.page.evaluate(() =>
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "l", isComposing: true, bubbles: true }),
      ),
    );
    await w.page.evaluate(() =>
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", isComposing: true, bubbles: true }),
      ),
    );
    await new Promise((r) => setTimeout(r, 150));
    for (const cmd of ["note_set_later", "note_set_status", "details_open", "today_dismiss"]) {
      assert.equal((await w.calls(cmd)).length, 0, cmd);
    }
    await w.close();
  });

  it("lists the keys in the footer", async () => {
    const w = await today({ glance: three().glance });
    const hints = w.page.locator(".today__hints");
    for (const hint of ["X done", "L later", "←→ day", "↵ details", "esc close"])
      await text(hints, hint);
    await w.close();
  });

  it("moves the selection with arrows and J/K, stopping at the ends", async () => {
    const w = await today({ glance: three().glance });
    await text(w.selected, "pay rent");
    await w.key("ArrowDown");
    await text(w.selected, "call mum");
    await w.key("j");
    await text(w.selected, "book dentist");
    await w.key("j");
    await text(w.selected, "book dentist");
    await w.key("k");
    await text(w.selected, "call mum");
    await w.key("ArrowUp");
    await w.key("ArrowUp");
    await text(w.selected, "pay rent");
    await w.close();
  });

  it("keeps the selected row in view while moving with the keyboard", async () => {
    const many = Array.from({ length: 30 }, (_, i) =>
      note(`note ${i}`, { created_at: at("2026-11-15T08:00:00") }),
    );
    const w = await today({ glance: glance({ today: many, total: 30 }) });
    await w.page.setViewportSize({ width: 520, height: 360 });
    await w.rows.first().waitFor();
    for (let i = 0; i < 25; i++) await w.key("ArrowDown");
    await text(w.selected, "note 25");
    const visible = await w.selected.evaluate((row) => {
      const list = row.closest(".today__list").getBoundingClientRect();
      const box = row.getBoundingClientRect();
      return box.top >= list.top - 1 && box.bottom <= list.bottom + 1;
    });
    assert.equal(visible, true);
    await w.close();
  });

  it("resets the selection when shown again, but keeps it across a notes change and a return from details", async () => {
    const w = await today({ glance: three().glance });
    await w.rows.first().waitFor();
    await w.key("ArrowDown");
    await text(w.selected, "call mum");
    await w.mock.emit("notes-changed");
    await w.mock.emit("today-returned");
    await eventually(
      () => w.calls("glance_get"),
      (c) => c.length >= 3,
      "reloaded",
    );
    await text(w.selected, "call mum");
    await w.mock.emit("today-shown");
    await text(w.selected, "pay rent");
    await w.close();
  });

  it("closes with Escape", async () => {
    const w = await today({ glance: three().glance });
    await w.key("Escape");
    await eventually(
      () => w.calls("today_dismiss"),
      (c) => c.length === 1,
      "dismissed",
    );
    await w.close();
  });

  it("shows an empty day with the capture shortcut", async () => {
    const w = await today();
    await text(w.page.locator(".empty"), "Nothing for today");
    await text(w.page.locator(".empty__text"), "Press CtrlShiftSpace anywhere to capture one.");
    await w.close();
  });

  it("points to the inbox when a group was capped", async () => {
    const f = three();
    const w = await today({ glance: { ...f.glance, total: 12 } });
    await text(w.page.locator(".today__more"), "and 9 more in the inbox");
    await w.close();
  });

  it("shows a database failure as an alert, never a blank window", async () => {
    const w = await open({ label: "today", now: NOON });
    await w.mock.fail("glance_get", "The database is locked.");
    await w.mock.emit("notes-changed");
    await text(w.page.getByRole("alert"), "The database is locked.");
    await w.close();
  });

  it("Enter opens Note details for the selected note; so does the open button", async () => {
    const f = three();
    const w = await today({ glance: f.glance });
    await w.rows.first().waitFor();
    await w.key("ArrowDown");
    await w.key("Enter");
    const [call] = await eventually(
      () => w.calls("details_open"),
      (c) => c.length === 1,
      "opened",
    );
    assert.deepEqual(call.args, { id: f.b.id, from: "today" });
    await w.rows.nth(2).getByRole("button", { name: "Open details (Enter)" }).click();
    const calls = await eventually(
      () => w.calls("details_open"),
      (c) => c.length === 2,
      "opened",
    );
    assert.equal(calls[1].args.id, f.c.id);
    await text(w.selected, "book dentist");
    await w.close();
  });

  it("marks rows that have a description or files", async () => {
    const withBoth = note("has both", {
      has_details: true,
      attachment_count: 2,
      created_at: at("2026-11-15T08:00:00"),
    });
    const oneFile = note("one file", {
      attachment_count: 1,
      created_at: at("2026-11-15T08:00:00"),
    });
    const plain = note("plain", { created_at: at("2026-11-15T08:00:00") });
    const w = await today({ glance: glance({ today: [withBoth, oneFile, plain], total: 3 }) });
    await w.rows.first().waitFor();
    assert.equal(
      await w.rows.nth(0).locator(".badges").getAttribute("aria-label"),
      "has details, 2 files",
    );
    assert.equal(await w.rows.nth(1).locator(".badges").getAttribute("aria-label"), "1 file");
    assert.equal(await w.rows.nth(2).locator(".badges").count(), 0);
    await w.close();
  });
});

describe("browsing days", () => {
  it("← shows the previous day, → comes back, and → stops at today", async () => {
    const w = await today({ glance: three().glance });
    await w.key("ArrowLeft");
    await eventually(
      w.lastBounds,
      (b) => b.todayFrom === startOf("2026-11-14"),
      "yesterday's bounds",
    );
    await text(w.page.locator(".today__date"), "Sat 14 Nov");
    await text(w.page.locator(".today__hints"), "As it was on this day");
    await w.key("ArrowRight");
    await eventually(w.lastBounds, (b) => b.todayFrom === startOf("2026-11-15"), "today's bounds");
    const before = (await w.calls("glance_get")).length;
    await w.key("ArrowRight");
    await new Promise((r) => setTimeout(r, 150));
    assert.equal((await w.calls("glance_get")).length, before, "no day after today");
    assert.equal(await w.page.getByRole("button", { name: "Next day" }).isDisabled(), true);
    await w.close();
  });

  it("T and the Back to today button return to today; the arrow buttons step days", async () => {
    const w = await today();
    await w.page.getByRole("button", { name: "Previous day" }).click();
    await w.page.getByRole("button", { name: "Previous day" }).click();
    await eventually(w.lastBounds, (b) => b.todayFrom === startOf("2026-11-13"), "two days back");
    await w.page.getByRole("button", { name: "Next day" }).click();
    await eventually(w.lastBounds, (b) => b.todayFrom === startOf("2026-11-14"), "one day back");
    await w.key("t");
    await eventually(w.lastBounds, (b) => b.todayFrom === startOf("2026-11-15"), "today");
    await w.key("ArrowLeft");
    await w.page.getByRole("button", { name: "Back to today" }).click();
    await text(w.page.locator(".today__date"), "Sunday 15 November");
    assert.equal(await w.page.getByRole("button", { name: "Back to today" }).count(), 0);
    // With the focused button gone, keys must still reach the window.
    await w.key("Escape");
    await eventually(
      () => w.calls("today_dismiss"),
      (c) => c.length === 1,
      "dismissed",
    );
    await w.close();
  });

  it("a past day is read-only: X and L do nothing, Enter still opens details", async () => {
    const f = three();
    const w = await today({ glance: f.glance });
    await w.rows.first().waitFor();
    await w.key("ArrowLeft");
    await text(w.page.getByRole("region", { name: "That day" }), "call mum");
    await w.key("x");
    await w.key("l");
    await w.key("Enter");
    await eventually(
      () => w.calls("details_open"),
      (c) => c.length === 1,
      "opened",
    );
    assert.equal((await w.calls("note_set_status")).length, 0);
    assert.equal((await w.calls("note_set_later")).length, 0);
    assert.equal(await w.rows.first().getByRole("checkbox").isDisabled(), true);
    await text(w.page.locator(".today__hints"), "T today");
    await w.close();
  });

  it("on a past day a note finished later shows as not done", async () => {
    const later = note("finished the next day", {
      status: "completed",
      created_at: at("2026-11-13T09:00:00"),
      completed_at: at("2026-11-15T09:00:00"),
    });
    const w = await today({ glance: glance({ today: [later], total: 1, done: 1 }) });
    await w.rows.first().waitFor();
    assert.equal(await w.rows.first().getAttribute("data-status"), "completed");
    await w.key("ArrowLeft");
    await eventually(
      () => w.rows.first().getAttribute("data-status"),
      (s) => s === "inbox",
      "not done then",
    );
    assert.equal(
      await w.page.getByRole("checkbox", { name: "Not done by the end of this day" }).count(),
      1,
    );
    await w.close();
  });

  it("shown again goes back to today; a notes change keeps the day", async () => {
    const w = await today();
    await w.key("ArrowLeft");
    await eventually(w.lastBounds, (b) => b.todayFrom === startOf("2026-11-14"), "yesterday");
    await w.mock.emit("notes-changed");
    await eventually(
      () => w.calls("glance_get"),
      (c) => c.length === 3,
      "reloaded",
    );
    assert.equal((await w.lastBounds()).todayFrom, startOf("2026-11-14"));
    await w.mock.emit("today-shown");
    await eventually(w.lastBounds, (b) => b.todayFrom === startOf("2026-11-15"), "today");
    await w.close();
  });

  it("an empty or capped past day does not talk about today or the inbox", async () => {
    const w = await today();
    await w.key("ArrowLeft");
    await text(w.page.locator(".empty"), "Nothing on this day");
    await text(w.page.locator(".empty__text"), "No notes were open or captured that day.");
    const f = three();
    await w.mock.setGlance({ ...f.glance, total: 12 });
    await w.mock.emit("notes-changed");
    await eventually(
      () => w.page.locator(".today__more").textContent(),
      (t) => t === "and 9 more",
      "capped line",
    );
    await w.close();
  });
});

describe("the calendar", () => {
  it("C opens the calendar on the shown month with its marks, and picking a day shows it", async () => {
    const w = await today();
    await w.mock.setMarks({
      [startOf("2026-11-10")]: { total: 3, open: 2 },
      [startOf("2026-11-11")]: { total: 1, open: 0 },
    });
    await w.key("c");
    const calendar = w.page.getByRole("dialog", { name: "Choose a day" });
    await text(calendar.locator(".calendar__month"), "November 2026");
    const [marks] = await eventually(
      () => w.calls("notes_day_marks"),
      (c) => c.length > 0,
      "marks asked",
    );
    assert.equal(marks.args.edges.length, 43);
    await eventually(
      () =>
        calendar
          .getByRole("gridcell", { name: "Tuesday, 10 November 2026, 2 open" })
          .getAttribute("data-mark"),
      (m) => m === "open",
      "open mark",
    );
    assert.equal(
      await calendar
        .getByRole("gridcell", { name: "Wednesday, 11 November 2026, all done" })
        .getAttribute("data-mark"),
      "done",
    );
    // British English starts the week on Monday.
    assert.equal(
      await calendar.locator(".calendar__weekday").first().getAttribute("title"),
      "Monday",
    );
    assert.equal(
      await calendar.getByRole("gridcell", { name: /^Monday, 16 November/ }).isDisabled(),
      true,
    );

    await calendar.getByRole("gridcell", { name: /^Tuesday, 10 November/ }).click();
    await eventually(w.lastBounds, (b) => b.todayFrom === startOf("2026-11-10"), "picked day");
    assert.equal(await calendar.count(), 0);
    await w.close();
  });

  it("moves by day and week with arrows, by month with Page keys, picks with Enter, and stops at the limit", async () => {
    const w = await today();
    await w.key("c");
    const calendar = w.page.getByRole("dialog", { name: "Choose a day" });
    const focused = () => w.page.evaluate(() => document.activeElement?.getAttribute("data-day"));
    await eventually(focused, (d) => d === "2026-11-15", "starts on the shown day");
    await w.key("ArrowRight");
    await new Promise((r) => setTimeout(r, 80));
    assert.equal(await focused(), "2026-11-15", "cannot move past today");
    await w.key("ArrowLeft");
    await eventually(focused, (d) => d === "2026-11-14", "a day back");
    await w.key("ArrowUp");
    await eventually(focused, (d) => d === "2026-11-07", "a week back");
    await w.key("PageUp");
    await eventually(focused, (d) => d === "2026-10-07", "a month back");
    await text(calendar.locator(".calendar__month"), "October 2026");
    await w.key("Enter");
    await eventually(w.lastBounds, (b) => b.todayFrom === startOf("2026-10-07"), "picked day");
    await w.close();
  });

  it("Esc with the calendar open closes only the calendar, and other keys stay inside it", async () => {
    const w = await today({ glance: three().glance });
    await w.rows.first().waitFor();
    await w.key("c");
    const calendar = w.page.getByRole("dialog", { name: "Choose a day" });
    await calendar.waitFor();
    await w.key("x");
    await w.key("l");
    await w.key("Escape");
    await eventually(
      () => calendar.count(),
      (n) => n === 0,
      "calendar closed",
    );
    assert.equal((await w.calls("today_dismiss")).length, 0);
    assert.equal((await w.calls("note_set_status")).length, 0);
    assert.equal((await w.calls("note_set_later")).length, 0);
    await w.close();
  });

  it("closes on a click outside, and T picks today", async () => {
    const w = await today();
    await w.key("ArrowLeft");
    await w.key("c");
    const calendar = w.page.getByRole("dialog", { name: "Choose a day" });
    await calendar.waitFor();
    await w.page.locator(".today__hints").click({ force: true });
    await eventually(
      () => calendar.count(),
      (n) => n === 0,
      "calendar closed",
    );
    await w.key("c");
    // The calendar takes focus a frame after it opens; its keys work from then.
    await eventually(
      () => w.page.evaluate(() => document.activeElement?.getAttribute("data-day")),
      (day) => day === "2026-11-14",
      "calendar focused",
    );
    await w.key("t");
    await eventually(w.lastBounds, (b) => b.todayFrom === startOf("2026-11-15"), "today");
    await w.close();
  });
});

describe("where a carried note came from", () => {
  it("tags carried rows with their day and leaves today's rows bare", async () => {
    const yesterday = note("from yesterday", { created_at: at("2026-11-14T09:00:00") });
    const f = three();
    const w = await today({
      glance: glance({ carried: [yesterday, f.a], today: [f.b], total: 3 }),
    });
    await text(w.rows.nth(0).locator(".today__origin"), "Yesterday");
    await eventually(
      () => w.rows.nth(1).locator(".today__origin").textContent(),
      (t) => t === "Thu 12 Nov · 3 days ago",
      "origin",
    );
    assert.equal(await w.rows.nth(2).locator(".today__origin").count(), 0);
    await w.close();
  });

  it("clicking the tag opens the inbox on that day, not the details", async () => {
    const f = three();
    const w = await today({ glance: f.glance });
    await w.page.getByRole("button", { name: "Show notes captured Thu 12 Nov" }).click();
    const [call] = await eventually(
      () => w.calls("inbox_open_date"),
      (c) => c.length === 1,
      "inbox opened",
    );
    assert.deepEqual(call.args, { day: "2026-11-12" });
    assert.equal((await w.calls("details_open")).length, 0);
    await w.close();
  });
});

describe("language", () => {
  it("shows Vietnamese text and Vietnamese dates when that is the chosen language", async () => {
    const f = three();
    const w = await today({ glance: f.glance, settings: { language: "vi" } });
    await text(w.page.locator(".today__count"), "Xong 0 trên 3");
    await text(w.page.getByRole("region", { name: "Chuyển từ trước" }), "pay rent");
    await eventually(
      () => w.page.locator(".today__date").textContent(),
      (t) => /Chủ Nhật/i.test(t) && /11/.test(t),
      "date in Vietnamese",
    );
    await text(w.page.locator(".today__hints"), "X xong");
    await w.close();
  });
});
