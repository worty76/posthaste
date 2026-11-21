import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, note, open, start, stop, text } from "./harness.mjs";

before(start);
after(stop);

const NOON = "2026-11-15T12:00:00+07:00";
const at = (iso) => new Date(`${iso}+07:00`).getTime();

/** Opens the details window on one note and waits for it to show. */
async function details({ patch = {}, text: body = "", files = [], steps = [], ...options } = {}) {
  const n = note("call the bank", {
    created_at: at("2026-11-15T09:05:00"),
    updated_at: at("2026-11-15T09:05:00"),
    ...patch,
  });
  const w = await open({
    label: "details",
    now: NOON,
    store: {
      notes: [n],
      details: { [n.id]: body },
      attachments: {
        [n.id]: files.map((f, i) => ({ id: `f${i}`, note_id: n.id, created_at: 1, ...f })),
      },
      steps: { [n.id]: steps.map((st, i) => ({ id: `s${i + 1}`, note_id: n.id, ...st })) },
      target: { id: n.id, from: "inbox" },
    },
    ...options,
  });
  const title = w.page.locator(".details__title");
  const field = w.page.locator(".details__text");
  await eventually(
    () => title.inputValue(),
    (v) => v === n.content,
    "the note to load",
  );
  const toast = w.page.locator(".toast");
  return { ...w, n, title, field, toast, calls: (cmd) => w.mock.calls(cmd) };
}

describe("steps", () => {
  const step = (text, patch = {}) => ({
    text,
    done: false,
    rank: "V",
    deleted_at: null,
    ...patch,
  });

  /** Opens the steps editor on the first step by keyboard. */
  async function editing(w, name) {
    const row = w.page.getByRole("checkbox", { name });
    await row.focus();
    await w.page.keyboard.press("Enter");
    return w.page.getByRole("textbox", { name: "Steps" });
  }

  it("adds, ticks, reorders and removes steps by keyboard", async () => {
    const w = await details();
    await w.page.getByRole("button", { name: "Add a step" }).click();
    const input = w.page.getByRole("textbox", { name: "Add a step" });
    await eventually(
      () => input.evaluate((el) => el === document.activeElement),
      Boolean,
      "add line focus",
    );
    await input.fill("first step");
    await eventually(
      () => w.calls("steps_add"),
      (c) => c.length === 1,
      "row created on the first keystroke",
    );
    await text(w.page.getByRole("checkbox", { name: "first step" }), "first step");
    await input.fill("first step, longer");
    await eventually(
      () => w.calls("step_set_text").then((c) => c.length),
      (c) => c === 1,
      "typing writes",
    );

    // Enter opens the add line again for the next step.
    await w.page.keyboard.press("Enter");
    await input.fill("second step");
    await eventually(
      () => w.calls("steps_add").then((c) => c.length),
      (c) => c === 2,
      "second row",
    );

    // Space ticks the selected step.
    const row = w.page.getByRole("checkbox", { name: "second step" });
    await row.focus();
    await w.page.keyboard.press(" ");
    await eventually(
      () => w.calls("step_set_done").then((c) => c.length),
      (c) => c === 1,
      "ticked",
    );
    assert.equal((await w.calls("step_set_done"))[0].args.done, true);

    // Ctrl+ArrowUp reorders it above the first step.
    await w.page.keyboard.press("Control+ArrowUp");
    await eventually(
      () => w.calls("step_move").then((c) => c.length),
      (c) => c === 1,
      "moved",
    );

    // Backspace removes it, with an undo toast.
    await row.focus();
    await w.page.keyboard.press("Backspace");
    await text(w.toast.first(), "Step removed.");
    await eventually(
      () => w.calls("step_remove").then((c) => c.length),
      (c) => c === 1,
      "removed",
    );
    await w.close();
  });

  it("undo restores a removed step", async () => {
    const w = await details({ steps: [step("only step")] });
    const row = w.page.getByRole("checkbox", { name: "only step" });
    await row.focus();
    await w.page.keyboard.press("Delete");
    await text(w.toast.first(), "Step removed.");
    await eventually(
      () => w.mock.store().then((store) => store.steps[w.n.id][0].deleted_at),
      (v) => v != null,
      "tombstoned",
    );
    await w.toast.first().getByRole("button", { name: "Undo" }).click();
    await eventually(
      () => w.mock.store().then((store) => store.steps[w.n.id][0].deleted_at),
      (v) => v == null,
      "restored",
    );
    assert.equal((await w.calls("step_restore")).length, 1);
    await w.close();
  });

  it("escape saves and keeps text", async () => {
    const w = await details({ steps: [step("typed")] });
    const input = await editing(w, "typed");
    await input.fill("typed more");
    await w.page.keyboard.press("Escape");
    await eventually(
      () => w.calls("step_set_text").then((c) => c.length),
      (c) => c === 1,
      "saved before leaving",
    );
    await eventually(
      () => w.page.getByRole("checkbox", { name: "typed more" }).count(),
      (c) => c === 1,
      "editor closed, text kept",
    );
    // Esc with no editor open closes the window, as today.
    await w.page.keyboard.press("Escape");
    await eventually(
      () => w.calls("details_dismiss").then((c) => c.length),
      (c) => c === 1,
      "window closes",
    );
    await w.close();
  });

  it("closing details flushes pending writes", async () => {
    const w = await details({ steps: [step("half typed")] });
    const input = await editing(w, "half typed");
    await input.fill("half typed and more");
    // Esc leaves the editor and saves; Esc again closes the window.
    await w.page.keyboard.press("Escape");
    await w.page.keyboard.press("Escape");
    await eventually(
      () => w.calls("details_dismiss").then((c) => c.length),
      (c) => c === 1,
      "hidden",
    );
    const all = (await w.calls()).map((c) => c.cmd);
    assert.ok(all.indexOf("step_set_text") < all.indexOf("details_dismiss"), "saved before hiding");
    await w.close();
  });

  it("failed step save keeps editor content", async () => {
    const w = await details({ steps: [step("kept")] });
    await w.mock.fail("step_set_text", "The disk is full.", "storage");
    const input = await editing(w, "kept");
    await input.fill("kept and edited");
    await text(w.page.getByRole("alert"), "The disk is full.");
    await eventually(
      () => input.inputValue(),
      (v) => v === "kept and edited",
      "text stays",
    );
    await w.close();
  });

  it("a trashed note's steps are read-only", async () => {
    const w = await details({ steps: [step("gone soon")] });
    await w.mock.change(
      "store.notes = []; store.trash = [{ note: arg, deleted_at: Date.now() }]",
      w.n,
    );
    await w.mock.emit("notes-changed");
    await eventually(
      () => w.calls("details_dismiss").then((c) => c.length),
      (c) => c === 1,
      "the window leaves a trashed note",
    );
    await w.close();
  });
});

describe("details window", () => {
  it("shows the note, when it was captured, and puts the caret in the description", async () => {
    const w = await details({ text: "ask about the fee", patch: { source: "Mail" } });
    assert.equal(await w.field.inputValue(), "ask about the fee");
    await text(w.page.locator(".details__meta"), "Created today, 9:05 · from Mail");
    await eventually(
      () => w.field.evaluate((el) => el === document.activeElement),
      Boolean,
      "focus",
    );
    assert.equal(
      await w.page.getByRole("button", { name: "Inbox" }).getAttribute("aria-pressed"),
      "false",
    );
    assert.deepEqual(w.errors, []);
    await w.close();
  });

  it("says how long an open note has been carried, and when it was edited", async () => {
    const w = await details({
      patch: { created_at: at("2026-11-12T09:05:00"), updated_at: at("2026-11-14T18:30:00") },
    });
    await text(
      w.page.locator(".details__meta"),
      "Created Thursday 12 November, 9:05 · carried over 3 days · edited yesterday, 18:30",
    );
    await w.close();
  });

  it("stores the description shortly after typing stops", async () => {
    const w = await details();
    await w.field.fill("ask about the fee");
    await text(w.page.locator(".details__saved"), "Saving…");
    await eventually(
      () => w.calls("note_details_set"),
      (c) => c.length === 1,
      "one write",
    );
    assert.deepEqual((await w.calls("note_details_set"))[0].args, {
      id: w.n.id,
      details: "ask about the fee",
    });
    await text(w.page.locator(".details__saved"), "Saved");
    await w.close();
  });

  it("stores a new title, but never an empty one", async () => {
    const w = await details({ patch: { category: "money" } });
    await w.title.fill("call the bank today");
    await eventually(
      () => w.calls("note_update"),
      (c) => c.length === 1,
      "title write",
    );
    assert.deepEqual((await w.calls("note_update"))[0].args, {
      id: w.n.id,
      content: "call the bank today",
      category: "money",
    });
    await w.title.fill("");
    await w.field.focus();
    assert.equal(await w.title.inputValue(), "call the bank today");
    assert.equal((await w.calls("note_update")).length, 1);
    await w.close();
  });

  it("writes unsaved text before closing on Esc", async () => {
    const w = await details();
    await w.field.fill("half a thought");
    await w.page.keyboard.press("Escape");
    await eventually(
      () => w.calls("details_dismiss"),
      (c) => c.length === 1,
      "dismiss",
    );
    const all = (await w.calls()).map((c) => c.cmd);
    assert.ok(
      all.indexOf("note_details_set") < all.indexOf("details_dismiss"),
      "saved before hiding",
    );
    await w.close();
  });

  it("keeps unsaved typing when the note changes elsewhere", async () => {
    const w = await details({ text: "old" });
    await w.mock.hold("note_details_set");
    await w.field.fill("typed here");
    await w.mock.change("store.details[arg] = 'changed elsewhere'", w.n.id);
    await w.mock.emit("notes-changed");
    await new Promise((r) => setTimeout(r, 150));
    assert.equal(await w.field.inputValue(), "typed here");
    await w.close();
  });

  it("marks the note done with the button and with Ctrl+Enter", async () => {
    const w = await details();
    await w.page.getByRole("button", { name: "Inbox" }).click();
    await text(w.page.locator(".details__status"), "Done");
    await w.page.keyboard.press("Control+Enter");
    await text(w.page.locator(".details__status"), "Inbox");
    assert.deepEqual(
      (await w.calls("note_set_status")).map((c) => c.args.status),
      ["completed", "inbox"],
    );
    await w.close();
  });

  it("moves to another note, saving the first one's text", async () => {
    const w = await details();
    const other = note("second note");
    await w.mock.change("store.notes.push(arg)", other);
    await w.field.fill("for the first");
    await w.mock.emit("details-open", { id: other.id, from: "today" });
    await eventually(
      () => w.title.inputValue(),
      (v) => v === "second note",
      "second note",
    );
    assert.equal(await w.field.inputValue(), "");
    assert.deepEqual((await w.calls("note_details_set"))[0].args, {
      id: w.n.id,
      details: "for the first",
    });
    await w.close();
  });

  it("closes when the note was deleted elsewhere", async () => {
    const w = await details();
    await w.mock.change("store.notes = []");
    await w.mock.emit("notes-changed");
    await eventually(
      () => w.calls("details_dismiss"),
      (c) => c.length === 1,
      "dismiss",
    );
    await w.close();
  });

  it("lists attachments with size, type and a thumbnail for images", async () => {
    const w = await details({
      files: [
        { name: "receipt.pdf", mime: "application/pdf", size: 88000 },
        { name: "photo.png", mime: "image/png", size: 2516582 },
      ],
    });
    const tiles = w.page.locator(".tile");
    await eventually(
      () => tiles.count(),
      (c) => c === 2,
      "two tiles",
    );
    await text(tiles.nth(0), "PDF");
    await text(tiles.nth(0), "86 KB");
    await text(tiles.nth(1), "2.4 MB");
    await eventually(
      () => tiles.nth(1).locator("img").getAttribute("src"),
      (s) => s === "data:image/png;base64,AQID",
      "thumbnail",
    );
    await text(w.page.locator(".details__count"), "2");
    await w.close();
  });

  it("opens, saves a copy of, and removes a file", async () => {
    const w = await details({
      files: [{ name: "receipt.pdf", mime: "application/pdf", size: 10 }],
    });
    await w.page.getByRole("button", { name: "Open receipt.pdf", exact: true }).click();
    await eventually(
      () => w.calls("attachment_open"),
      (c) => c.length === 1,
      "open",
    );
    await w.page.getByRole("button", { name: "Save a copy of receipt.pdf" }).click();
    await text(w.page.locator(".details__saved"), "Saved a copy of receipt.pdf");
    await w.page.locator(".tile__main").focus();
    await w.page.keyboard.press("Delete");
    await text(w.page.locator(".details__saved"), "Removed receipt.pdf");
    await eventually(
      () => w.page.locator(".tile").count(),
      (c) => c === 0,
      "tile gone",
    );
    await eventually(
      () =>
        w.page
          .getByRole("button", { name: "Add files" })
          .evaluate((el) => el === document.activeElement),
      Boolean,
      "focus on Add files",
    );
    await w.close();
  });

  it("attaches dropped files and shows the drop state", async () => {
    const w = await details();
    await w.mock.drag("over");
    await text(w.page.locator(".details__drop-hint"), "Drop to attach");
    await w.mock.drag("drop", ["/home/me/a.txt", "/home/me/b.txt"]);
    await text(w.page.locator(".details__saved"), "Attached 2 files");
    await text(w.page.locator(".details__drop-hint"), "Drop files here");
    assert.deepEqual((await w.calls("attachment_add_paths"))[0].args, {
      noteId: w.n.id,
      paths: ["/home/me/a.txt", "/home/me/b.txt"],
    });
    await w.close();
  });

  it("adds files from the dialog with Ctrl+O and names each refusal", async () => {
    const w = await details();
    await w.mock.change("store.pick = arg", {
      added: [
        { id: "x", note_id: w.n.id, name: "ok.txt", mime: "text/plain", size: 1, created_at: 1 },
      ],
      failed: [{ name: "huge.zip", code: "too_large", message: "This file is over 15 MB." }],
    });
    await w.page.keyboard.press("Control+o");
    await text(w.page.locator(".details__saved"), "Attached ok.txt");
    await text(w.page.getByRole("alert"), "huge.zip: This file is over 15 MB.");
    await w.close();
  });

  it("refuses more files when the note is full, without asking the shell", async () => {
    const files = Array.from({ length: 20 }, (_, i) => ({
      name: `f${i}.txt`,
      mime: "text/plain",
      size: 1,
    }));
    const w = await details({ files });
    await w.page.getByRole("button", { name: "Add files" }).click();
    await text(w.page.getByRole("alert"), "This note already has 20 files");
    assert.equal((await w.calls("attachment_add_files")).length, 0);
    await w.close();
  });

  it("attaches a pasted image under a dated name", async () => {
    const w = await details();
    await w.page.evaluate(() => {
      const data = new DataTransfer();
      data.items.add(new File([new Uint8Array([9, 8, 7])], "image.png", { type: "image/png" }));
      window.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, cancelable: true }));
    });
    await text(w.page.locator(".details__saved"), "Attached the pasted image");
    const call = (await w.calls("attachment_add_bytes"))[0].args;
    assert.equal(call.size, 3);
    assert.equal(decodeURIComponent(call.headers["x-name"]), "Pasted image 2026-11-15 12.00.png");
    assert.equal(call.headers["x-mime"], encodeURIComponent("image/png"));
    await w.close();
  });

  it("follows the language setting", async () => {
    const w = await details({ settings: { language: "vi" } });
    await text(w.page.locator(".details__meta"), "Tạo hôm nay");
    await text(w.page.locator(".details__status"), "Hộp thư");
    assert.equal(await w.page.getByRole("button", { name: "Thêm tệp" }).count(), 1);
    await w.close();
  });
});
