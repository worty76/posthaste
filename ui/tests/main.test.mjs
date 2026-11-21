import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, note, open, start, stop, text } from "./harness.mjs";

before(start);
after(stop);

const NOON = "2026-11-15T12:00:00+07:00";
const at = (iso) => new Date(`${iso}+07:00`).getTime();

const notes = () => [
  note("pay rent", { created_at: at("2026-11-12T09:00:00") }),
  note("call mum", { created_at: at("2026-11-15T08:00:00") }),
  note("book dentist", { created_at: at("2026-11-15T09:00:00"), category: "health" }),
  note("old receipt", { created_at: at("2026-11-10T09:00:00"), status: "archived" }),
  note("someday trip", { created_at: at("2026-11-11T09:00:00"), later_at: 5 }),
];

/** Opens the main window and waits for its first list. */
async function main({ store = {}, ...options } = {}) {
  const w = await open({
    label: "main",
    now: NOON,
    store: { notes: notes(), ...store },
    ...options,
  });
  await eventually(
    () => w.mock.calls("notes_list"),
    (c) => c.length > 0,
    "first list",
  );
  const rows = w.page.locator(".note");
  const selected = w.page.locator('.note[aria-current="true"]');
  const key = (k) => w.page.keyboard.press(k);
  const toast = w.page.locator(".toast");
  const contents = () => rows.locator(".note__content").allTextContents();
  /** Presses `/` and waits for the caret to be in the search box. */
  const search = async () => {
    await key("/");
    const box = w.page.getByLabel("Search all notes");
    await eventually(
      () => box.evaluate((el) => el === document.activeElement),
      Boolean,
      "search focus",
    );
  };
  return { ...w, rows, selected, key, toast, contents, search, calls: (cmd) => w.mock.calls(cmd) };
}

describe("inbox", () => {
  it("lists open notes by project section, newest first, with counts in the sidebar", async () => {
    const w = await main();
    await eventually(w.contents, (c) => c.length === 3, "three notes");
    assert.deepEqual(await w.contents(), ["book dentist", "call mum", "pay rent"]);
    await text(w.page.locator(".proj-section__header").first(), "health");
    await text(w.page.locator(".proj-section__header").first(), "1");
    await text(w.page.locator(".nav__item").nth(0), "3");
    await text(w.page.locator(".nav__item").nth(2), "1");
    await text(w.rows.first(), "health");
    assert.deepEqual(w.errors, []);
    await w.close();
  });

  it("opens each section with its digit and asks for that section's notes", async () => {
    const w = await main();
    await w.key("4");
    await eventually(w.contents, (c) => c.join() === "old receipt", "archive");
    await text(w.page.locator(".content__title"), "Archive");
    await w.key("3");
    await eventually(w.contents, (c) => c.join() === "someday trip", "later");
    await w.key("2");
    await eventually(w.contents, (c) => c.join() === "book dentist,call mum", "today");
    const query = (await w.calls("notes_list")).at(-1).args.query;
    assert.equal(query.created_from, at("2026-11-15T00:00:00"));
    assert.equal(query.limit, 201);
    await w.close();
  });

  it("completes the selected note, moves on, and undoes it", async () => {
    const w = await main();
    await w.key("j");
    await text(w.selected, "book dentist");
    await w.key("x");
    await text(w.toast, "Completed");
    await eventually(w.contents, (c) => c.length === 2, "note left the inbox");
    await text(w.selected, "call mum");
    await w.toast.getByRole("button", { name: "Undo" }).click();
    await eventually(w.contents, (c) => c.length === 3, "note is back");
    assert.deepEqual(
      (await w.calls("note_set_status")).map((c) => c.args.status),
      ["completed", "inbox"],
    );
    await w.close();
  });

  it("archives, parks and deletes with a key each", async () => {
    const w = await main();
    await w.key("j");
    await w.key("a");
    await text(w.toast.last(), "Archived");
    await eventually(w.contents, (c) => c.length === 2, "archived");
    await w.key("l");
    await text(w.toast.last(), "Moved to Later");
    await eventually(w.contents, (c) => c.length === 1, "parked");
    await w.key("Delete");
    await text(w.toast.last(), "Moved to Trash");
    await eventually(w.contents, (c) => c.length === 0, "deleted");
    await text(w.page.locator(".empty"), "Your inbox is clear");
    await text(w.page.locator(".nav__item").nth(4), "1");
    await w.close();
  });

  it("edits in place: Enter saves, Esc cancels", async () => {
    const w = await main();
    await w.key("j");
    await w.key("e");
    const field = w.page.getByLabel("Edit note");
    await field.fill("book dentist for Monday");
    await w.page.getByLabel("Category (optional)").fill("teeth");
    await w.key("Enter");
    await eventually(w.contents, (c) => c[0] === "book dentist for Monday", "edit stored");
    assert.deepEqual((await w.calls("note_update"))[0].args.category, "teeth");
    await w.key("e");
    await w.page.getByLabel("Edit note").fill("thrown away");
    await w.key("Escape");
    await eventually(
      () => w.page.getByLabel("Edit note").count(),
      (c) => c === 0,
      "editor closed",
    );
    assert.equal((await w.calls("note_update")).length, 1);
    await w.close();
  });

  it("opens details from the inbox and copies a note", async () => {
    const w = await main();
    await w.page.evaluate(() => (navigator.clipboard.writeText = async (t) => (window.copied = t)));
    await w.key("j");
    await w.key("o");
    await eventually(
      () => w.calls("details_open"),
      (c) => c.length === 1,
      "details",
    );
    assert.deepEqual((await w.calls("details_open"))[0].args.from, "inbox");
    await w.key("c");
    await text(w.toast, "Copied");
    assert.equal(await w.page.evaluate(() => window.copied), "book dentist");
    await w.close();
  });

  it("searches every note, narrows by status, and Esc ends the search", async () => {
    const w = await main();
    await w.search();
    await w.page.keyboard.type("re");
    await eventually(
      w.contents,
      (c) => c.join() === "pay rent,old receipt",
      "matches across sections",
    );
    await text(w.page.locator(".content__title"), "Search");
    await text(w.page.locator(".filters__count"), "2 notes");
    await w.page.getByRole("radio", { name: "Archived" }).click();
    await eventually(w.contents, (c) => c.join() === "old receipt", "narrowed");
    await text(w.page.locator(".filters__count"), "1 note");
    await w.page.getByLabel("Search all notes").fill("zzz");
    await text(w.page.locator(".empty"), "Nothing contains “zzz” with these filters");
    await w.page.getByLabel("Search all notes").press("Escape");
    await eventually(w.contents, (c) => c.length === 3, "inbox again");
    await w.close();
  });

  it("goes from the search box to the first result with the down arrow", async () => {
    const w = await main();
    await w.search();
    await w.page.keyboard.type("mum");
    await eventually(w.contents, (c) => c.length === 1, "one match");
    await w.key("ArrowDown");
    await text(w.selected, "call mum");
    await w.close();
  });

  it("filters by the day a note was captured", async () => {
    const w = await main();
    await w.mock.emit("inbox-show-date", "2026-11-10");
    await eventually(w.contents, (c) => c.join() === "old receipt", "that day's notes");
    await text(w.page.locator(".datefilter__button"), "10 Nov 2026");
    await w.page.locator(".datefilter__button").click();
    await w.page.getByRole("button", { name: "Clear", exact: true }).click();
    await eventually(w.contents, (c) => c.length === 3, "filter cleared");
    await w.close();
  });

  it("shows why each empty section is empty", async () => {
    const w = await main({ store: { notes: [] } });
    await text(w.page.locator(".empty"), "Press CtrlShiftSpace in any application");
    await w.key("3");
    await text(w.page.locator(".empty"), "Nothing parked");
    await w.key("4");
    await text(w.page.locator(".empty"), "The archive is empty");
    await w.key("2");
    await text(w.page.locator(".empty"), "Nothing captured today");
    await w.close();
  });

  it("says when the capture shortcut is not active", async () => {
    const w = await main();
    await w.mock.answer("settings_get", {
      settings: await w.mock.settings(),
      shortcuts: {
        capture: { accelerator: "Ctrl+Space", registered: false, error: "Another app uses it." },
        inbox: { accelerator: "", registered: false, error: null },
        today: { accelerator: "", registered: false, error: null },
        limitation: null,
      },
    });
    await w.mock.emit("settings-changed", await w.mock.settings());
    await text(
      w.page.getByRole("alert"),
      "The capture shortcut isn't active. Another app uses it.",
    );
    await w.page.getByRole("button", { name: "Change shortcut" }).click();
    await text(w.page.locator(".content__title"), "Settings");
    await w.close();
  });

  it("offers a newer release and installs it", async () => {
    const w = await main({
      store: { update: { version: "9.9.9", current_version: "0.0.0", notes: null } },
    });
    await text(w.page.locator(".banner"), "Posthaste 9.9.9 is available.");
    await w.page.getByRole("button", { name: "Update", exact: true }).click();
    await eventually(
      () => w.calls("update_install"),
      (c) => c.length === 1,
      "install",
    );
    await w.close();
  });

  it("lists the keys with ? and creates a note with N", async () => {
    const w = await main();
    await w.key("?");
    await text(w.page.getByRole("dialog"), "Move through notes");
    await w.key("x");
    assert.equal((await w.calls("note_set_status")).length, 0, "keys belong to the dialog");
    await w.page.getByRole("button", { name: "Close" }).click();
    await w.key("n");
    await eventually(
      () => w.calls("capture_open"),
      (c) => c.length === 1,
      "capture",
    );
    await w.close();
  });

  it("collapses and expands the sidebar with the toggle button and Ctrl+B, and saves it", async () => {
    const w = await main();
    const sidebar = w.page.locator(".sidebar");
    const column = w.page.locator(".content__column");
    const widthBefore = await column.evaluate((el) => Math.round(el.getBoundingClientRect().width));
    const toggle = w.page.getByRole("button", { name: "Collapse sidebar (Ctrl+B)" });
    assert.equal(await toggle.getAttribute("aria-expanded"), "true");

    await toggle.click();
    const expand = w.page.getByRole("button", { name: "Expand sidebar (Ctrl+B)" });
    await eventually(
      () => expand.getAttribute("aria-expanded"),
      (v) => v === "false",
      "sidebar collapsed",
    );
    assert.equal(await sidebar.getAttribute("data-collapsed"), "");
    assert.equal((await w.mock.settings()).sidebar_collapsed, true);
    assert.equal(await sidebar.evaluate((el) => Math.round(el.getBoundingClientRect().width)), 56);
    const widthAfter = await column.evaluate((el) => Math.round(el.getBoundingClientRect().width));
    assert.equal(widthAfter - widthBefore, 228 - 56);

    // Icon rail buttons keep their accessible names and still navigate.
    await w.page.getByRole("button", { name: "Archive", exact: true }).click();
    await text(w.page.locator(".content__title"), "Archive");

    // Ctrl+B expands it back.
    await w.key("Control+b");
    await eventually(
      () => w.page.getByRole("button", { name: "Collapse sidebar (Ctrl+B)" }).count(),
      (n) => n === 1,
      "sidebar expanded",
    );
    assert.equal((await w.mock.settings()).sidebar_collapsed, false);
    assert.equal(await sidebar.evaluate((el) => Math.round(el.getBoundingClientRect().width)), 228);

    // At the minimum window width (620px), the sidebar stays a vertical left
    // column (180px expanded, 56px collapsed) and nothing overflows horizontally.
    await w.page.setViewportSize({ width: 620, height: 480 });
    assert.equal(await sidebar.evaluate((el) => Math.round(el.getBoundingClientRect().width)), 180);
    await w.key("Control+b");
    await eventually(
      () => sidebar.evaluate((el) => Math.round(el.getBoundingClientRect().width)),
      (width) => width === 56,
      "collapsed at 620px",
    );
    const overflow = await w.page
      .locator(".content__scroll")
      .evaluate((el) => el.scrollWidth - el.clientWidth);
    assert.equal(overflow, 0);
    await w.close();

    // On macOS the collapsed rail stays wide enough to frame the traffic lights.
    const mac = await main({ os: "macos", settings: { sidebar_collapsed: true } });
    assert.equal(
      await mac.page
        .locator(".sidebar")
        .evaluate((el) => Math.round(el.getBoundingClientRect().width)),
      80,
    );
    await mac.close();
  });

  it("a note with steps shows done of total", async () => {
    const w = await main({
      store: {
        notes: notes().concat([
          note("with steps", {
            created_at: at("2026-11-15T10:00:00"),
            step_total: 5,
            step_done: 2,
          }),
        ]),
      },
    });
    await eventually(w.contents, (c) => c.length === 4, "four notes");
    const row = w.page.locator(".note", { hasText: "with steps" });
    await text(row, "2/5");
    const badge = row.locator(".badges");
    await eventually(
      () => badge.getAttribute("aria-label"),
      (v) => v === "2 of 5 steps done",
      "accessible label",
    );
    await w.close();
  });

  it("pressing # and enter filters the inbox by category", async () => {
    const w = await main();
    await w.mock.answer("categories_counts", [
      { name: "health", count: 1 },
      { name: "work", count: 2 },
    ]);
    await w.key("#");
    const picker = w.page.locator(".cat-picker");
    await text(picker, "health");
    await w.key("Enter");
    await eventually(w.contents, (c) => c.join() === "book dentist", "filtered");
    await text(w.page.locator(".catfilter"), "health");
    assert.equal((await w.calls("notes_list")).at(-1).args.query.category, "health");
    await w.close();
  });

  it("the filter line clears with escape", async () => {
    const w = await main();
    await w.mock.answer("categories_counts", [{ name: "work", count: 2 }]);
    await w.key("#");
    await text(w.page.locator(".cat-picker"), "work");
    await w.key("Enter");
    await text(w.page.locator(".empty"), "No notes in this category here.");
    await w.key("Escape");
    await eventually(w.contents, (c) => c.length === 3, "filter cleared");
    await w.key("Escape");
    await eventually(
      () => w.selected.count(),
      (c) => c === 0,
      "selection cleared next",
    );
    await w.close();
  });

  it("the filter line clears with its × and the picker is gone", async () => {
    const w = await main();
    await w.mock.answer("categories_counts", [{ name: "health", count: 1 }]);
    await w.key("#");
    await text(w.page.locator(".cat-picker"), "health");
    await w.key("Enter");
    await eventually(w.contents, (c) => c.length === 1, "filtered");
    await w.page.getByRole("button", { name: "Clear the category filter" }).click();
    await eventually(w.contents, (c) => c.length === 3, "filter cleared");
    await w.close();
  });

  it("clicking a row's category filters by it", async () => {
    const w = await main();
    await w.rows.first().locator(".note__category").click();
    await eventually(w.contents, (c) => c.join() === "book dentist", "only that category");
    await text(w.page.locator(".catfilter"), "health");
    await w.close();
  });

  it("the filter stays when switching views", async () => {
    const w = await main();
    await w.mock.answer("categories_counts", [{ name: "health", count: 1 }]);
    await w.key("#");
    await text(w.page.locator(".cat-picker"), "health");
    await w.key("Enter");
    await eventually(w.contents, (c) => c.length === 1, "filtered");
    await w.key("3");
    await text(w.page.locator(".catfilter"), "health");
    const query = (await w.calls("notes_list")).at(-1).args.query;
    assert.equal(query.category, "health");
    await w.key("Escape");
    await eventually(w.contents, (c) => c.join() === "someday trip", "later again");
    await w.close();
  });

  it("the picker narrows as you type and shows counts", async () => {
    const w = await main();
    await w.mock.answer("categories_counts", [
      { name: "health", count: 1 },
      { name: "work", count: 2 },
      { name: "home", count: 3 },
    ]);
    await w.key("#");
    const picker = w.page.locator(".cat-picker");
    await text(picker, "health");
    await text(picker, "3");
    await w.page.keyboard.type("ho");
    await text(picker, "home");
    assert.equal(await picker.locator(".cat-picker__item").count(), 1);
    await w.key("Escape");
    await eventually(
      () => picker.count(),
      (c) => c === 0,
      "picker closed",
    );
    await w.close();
  });

  it("the picker says so when no category is in use", async () => {
    const w = await main();
    await w.mock.answer("categories_counts", []);
    await w.key("#");
    await text(w.page.locator(".cat-picker"), "Add one with # when you capture a note.");
    await w.close();
  });

  it("the inbox shows project sections with counts", async () => {
    const w = await main({
      store: {
        notes: [
          note("buy milk", { created_at: at("2026-11-15T10:00:00"), category: "home" }),
          note("fix sink", { created_at: at("2026-11-15T09:00:00"), category: "home" }),
          note("draft spec", { created_at: at("2026-11-15T08:00:00"), category: "work" }),
          note("call mum", { created_at: at("2026-11-15T11:00:00") }),
        ],
      },
    });
    await eventually(w.contents, (c) => c.length === 4, "four notes");
    // Sections ordered by newest note; no-project group last.
    assert.deepEqual(await w.contents(), ["buy milk", "fix sink", "draft spec", "call mum"]);
    const headers = w.page.locator(".proj-section__header");
    assert.equal(await headers.count(), 2);
    await text(headers.nth(0), "home");
    await text(headers.nth(0), "2");
    await text(headers.nth(1), "work");
    await text(headers.nth(1), "1");
    await w.close();
  });

  it("a header collapses and expands by keyboard", async () => {
    const w = await main({
      store: {
        notes: [
          note("buy milk", { created_at: at("2026-11-15T10:00:00"), category: "home" }),
          note("draft spec", { created_at: at("2026-11-15T08:00:00"), category: "work" }),
        ],
      },
    });
    await eventually(w.contents, (c) => c.length === 2, "two notes");
    const header = w.page.locator(".proj-section__header").first();
    assert.equal(await header.getAttribute("aria-expanded"), "true");
    await header.focus();
    await w.key("ArrowLeft");
    assert.equal(await header.getAttribute("aria-expanded"), "false");
    await eventually(w.contents, (c) => c.join() === "draft spec", "collapsed");
    await w.key("ArrowRight");
    assert.equal(await header.getAttribute("aria-expanded"), "true");
    await eventually(w.contents, (c) => c.length === 2, "expanded");
    await w.key("Enter");
    assert.equal(await header.getAttribute("aria-expanded"), "false");
    await eventually(w.contents, (c) => c.join() === "draft spec", "toggled with enter");
    await w.close();
  });

  it("arrow keys skip a collapsed section", async () => {
    const w = await main({
      store: {
        notes: [
          note("alpha one", { created_at: at("2026-11-15T10:00:00"), category: "alpha" }),
          note("beta one", { created_at: at("2026-11-15T09:00:00"), category: "beta" }),
          note("beta two", { created_at: at("2026-11-15T08:30:00"), category: "beta" }),
          note("gamma one", { created_at: at("2026-11-15T08:00:00"), category: "gamma" }),
        ],
      },
    });
    await eventually(w.contents, (c) => c.length === 4, "four notes");
    // Select beta one, then collapse beta: selection moves to gamma one.
    await w.key("ArrowDown");
    await w.key("ArrowDown");
    await text(w.selected, "beta one");
    await w.page.locator(".proj-section__header").nth(1).click();
    await eventually(w.contents, (c) => c.join() === "alpha one,gamma one", "beta collapsed");
    await text(w.selected, "gamma one");
    // Moving up skips collapsed beta and lands on alpha one.
    await w.key("ArrowUp");
    await text(w.selected, "alpha one");
    // Moving down skips collapsed beta and lands on gamma one.
    await w.key("ArrowDown");
    await text(w.selected, "gamma one");
    await w.close();
  });

  it("search results are not sectioned", async () => {
    const w = await main({
      store: {
        notes: [
          note("buy apples", { created_at: at("2026-11-15T10:00:00"), category: "home" }),
          note("buy paper", { created_at: at("2026-11-15T09:00:00"), category: "work" }),
        ],
      },
    });
    await eventually(
      () => w.page.locator(".proj-section__header").count(),
      (n) => n === 2,
      "sectioned in inbox",
    );
    await w.search();
    await w.page.keyboard.type("buy");
    await eventually(
      () => w.page.locator(".proj-section__header").count(),
      (n) => n === 0,
      "no section headers in search",
    );
    await eventually(w.contents, (c) => c.length === 2, "both search hits shown");
    await w.close();
  });

  it("notes without a project have no header", async () => {
    const w = await main({
      store: {
        notes: [
          note("plain one", { created_at: at("2026-11-15T10:00:00") }),
          note("plain two", { created_at: at("2026-11-15T09:00:00") }),
        ],
      },
    });
    await eventually(w.contents, (c) => c.length === 2, "two notes");
    assert.equal(await w.page.locator(".proj-section__header").count(), 0);
    await w.close();
  });

  it("is written in Vietnamese when that is the language", async () => {
    const w = await main({ settings: { language: "vi" }, locale: "vi-VN" });
    await text(w.page.locator(".content__title"), "Hộp thư");
    await text(w.page.locator(".nav"), "Hôm nay");
    await text(w.page.locator(".nav"), "Thùng rác");
    await w.close();
  });

  it("explains itself when the database could not be opened", async () => {
    const w = await open({
      label: "main",
      info: { startup_error: { code: "storage", message: "The disk is full." } },
    });
    await text(w.page.getByRole("alert"), "Posthaste can't open its database");
    await text(w.page.getByRole("alert"), "The disk is full.");
    await w.close();
  });
});

describe("trash", () => {
  const trashed = () => [
    { note: note("thrown one"), deleted_at: at("2026-11-15T10:00:00") },
    { note: note("thrown two", { status: "completed" }), deleted_at: at("2026-11-14T10:00:00") },
  ];
  const trash = async () => {
    const w = await main({ store: { notes: [], trash: trashed() } });
    await w.key("5");
    await eventually(w.contents, (c) => c.length === 2, "two trashed notes");
    return w;
  };

  it("says where each note returns to and when it is erased", async () => {
    const w = await trash();
    await text(w.rows.nth(0), "Inbox");
    await text(w.rows.nth(0), "Erased on 15 Dec");
    await text(w.rows.nth(1), "Archive");
    await text(w.page.locator(".trash__note"), "after 30 days");
    await w.close();
  });

  it("restores with R and can take that back", async () => {
    const w = await trash();
    await w.key("j");
    await w.key("r");
    await text(w.toast, "Restored to Inbox");
    await eventually(w.contents, (c) => c.join() === "thrown two", "one left");
    await text(w.selected, "thrown two");
    await w.toast.getByRole("button", { name: "Undo" }).click();
    await eventually(w.contents, (c) => c.length === 2, "back in the trash");
    await w.close();
  });

  it("asks before erasing one note for good", async () => {
    const w = await trash();
    await w.key("j");
    await w.key("Delete");
    const dialog = w.page.getByRole("dialog");
    await text(dialog, "Delete this note for good?");
    await text(dialog, "thrown one");
    assert.equal((await w.calls("trash_purge")).length, 0);
    await dialog.getByRole("button", { name: "Delete forever" }).click();
    await text(w.toast, "Deleted for good");
    await eventually(w.contents, (c) => c.length === 1, "erased");
    await w.close();
  });

  it("empties the trash after a confirmation", async () => {
    const w = await trash();
    await w.page.getByRole("button", { name: "Empty Trash" }).click();
    const dialog = w.page.getByRole("dialog");
    await text(dialog, "2 notes will be deleted");
    await dialog.getByRole("button", { name: "Empty Trash" }).click();
    await text(w.toast, "Deleted 2 notes for good");
    await text(w.page.locator(".empty"), "Trash is empty");
    assert.deepEqual((await w.calls("trash_empty"))[0].args, { upTo: at("2026-11-15T10:00:00") });
    await w.close();
  });

  it("offers one-click update when the database is from a newer version", async () => {
    const w = await open({
      label: "main",
      info: {
        startup_error: {
          code: "db_newer_schema",
          message:
            "This database was created by a newer version of Posthaste (schema 7, this version understands up to 6).",
        },
      },
      store: {
        update: {
          version: "0.6.0",
          current_version: "0.5.0",
          notes: null,
        },
      },
    });
    const fatal = w.page.locator(".fatal");
    await text(fatal, "Update Posthaste to the latest version to open your notes again.");
    const btn = fatal.getByRole("button", { name: "Update to 0.6.0" });
    await btn.click();
    await eventually(
      async () => (await w.mock.calls("update_install")).length,
      (n) => n === 1,
      "update_install called",
    );
    assert.deepEqual(w.errors, []);
    await w.close();
  });
});
