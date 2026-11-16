import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, open, start, stop, text } from "./harness.mjs";

before(start);
after(stop);

/** Opens the capture window and waits until it is ready to type in. */
async function capture(options) {
  const window = await open({ label: "capture", ...options });
  const note = window.page.getByRole("textbox", { name: /^(Note|Ghi chú)$/ });
  await note.waitFor();
  const status = window.page.locator(".capture__status");
  const saves = () => window.mock.calls("capture_save");
  return { ...window, note, status, saves };
}

describe("capture input", () => {
  it("focuses the input without a click and shows the placeholder", async () => {
    const w = await capture();
    await eventually(() => w.note.evaluate((el) => el === document.activeElement), Boolean, "focus");
    assert.equal(await w.note.getAttribute("placeholder"), "What's on your mind?");
    assert.deepEqual(w.errors, []);
    await w.close();
  });

  it("refocuses the input every time the window is shown, and names where you came from", async () => {
    const w = await capture();
    await w.page.getByLabel("Category (optional)").focus();
    await w.mock.emit("capture-shown", { source: "Code" });
    await eventually(() => w.note.evaluate((el) => el === document.activeElement), Boolean, "focus");
    await text(w.page.locator(".capture__header"), "from Code");
    await w.close();
  });

  it("shows the registered shortcut as a hint", async () => {
    const w = await capture();
    await text(w.page.locator(".capture__header .kbd"), "CtrlShiftSpace");
    await w.close();
  });
});

describe("keyboard", () => {
  it("Enter saves, closes and clears the input", async () => {
    const w = await capture();
    await w.note.fill("buy milk");
    await w.note.press("Enter");
    const [save] = await eventually(w.saves, (c) => c.length === 1, "one save");
    assert.deepEqual(save.args, { content: "buy milk", category: null, hide: true });
    await eventually(() => w.note.inputValue(), (v) => v === "", "cleared");
    await w.close();
  });

  it("Shift+Enter inserts a newline instead of saving", async () => {
    const w = await capture();
    await w.note.fill("one");
    await w.note.press("Shift+Enter");
    await w.note.pressSequentially("two");
    assert.equal(await w.note.inputValue(), "one\ntwo");
    assert.equal((await w.saves()).length, 0);
    await w.close();
  });

  it("Ctrl+Enter saves and keeps the window open for another note", async () => {
    const w = await capture();
    await w.note.fill("first");
    await w.note.press("Control+Enter");
    const [save] = await eventually(w.saves, (c) => c.length === 1, "one save");
    assert.equal(save.args.hide, false);
    await text(w.status, "Saved");
    await eventually(() => w.note.evaluate((el) => el === document.activeElement), Boolean, "focus");
    await w.close();
  });

  it("uses Cmd rather than Ctrl as the modifier on macOS", async () => {
    const w = await capture({ os: "macos" });
    await w.note.fill("first");
    await w.note.press("Meta+Enter");
    const [save] = await eventually(w.saves, (c) => c.length === 1, "one save");
    assert.equal(save.args.hide, false);
    await text(w.page.locator(".capture__header .kbd"), "⌘⇧Space");
    await w.close();
  });

  it("Escape closes and keeps what was typed as a draft", async () => {
    const w = await capture();
    await w.note.fill("half a thought");
    await w.note.press("Escape");
    await eventually(() => w.mock.calls("capture_dismiss"), (c) => c.length === 1, "dismissed");
    assert.equal((await w.mock.draft()).content, "half a thought");
    assert.equal(await w.note.inputValue(), "half a thought");
    const order = (await w.mock.calls()).map((c) => c.cmd);
    assert.ok(order.indexOf("draft_set") < order.indexOf("capture_dismiss"), "draft stored first");
    await w.close();
  });

  it("does not save on the Enter that confirms an IME composition", async () => {
    const w = await capture();
    await w.note.fill("viet");
    await w.note.evaluate((el) =>
      el.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true, cancelable: true }),
      ),
    );
    await new Promise((r) => setTimeout(r, 150));
    assert.equal((await w.saves()).length, 0);
    await w.close();
  });

  it("with 'Enter adds a line', Enter is a newline and Ctrl+Enter saves and closes", async () => {
    const w = await capture({ settings: { enter_behavior: "newline" } });
    await text(w.page.locator(".capture__hints"), "Ctrl↵save");
    await w.note.fill("one");
    await w.note.press("Enter");
    assert.equal(await w.note.inputValue(), "one\n");
    assert.equal((await w.saves()).length, 0);
    await w.note.press("Control+Enter");
    const [save] = await eventually(w.saves, (c) => c.length === 1, "one save");
    assert.equal(save.args.hide, true);
    await w.close();
  });

  it("with 'Close after saving' off, Enter saves and stays open", async () => {
    const w = await capture({ settings: { hide_after_save: false } });
    await eventually(() => w.mock.calls("settings_get"), (c) => c.length > 0, "settings read");
    await w.note.fill("stay");
    await w.note.press("Enter");
    const [save] = await eventually(w.saves, (c) => c.length === 1, "one save");
    assert.equal(save.args.hide, false);
    await w.close();
  });
});

describe("empty input", () => {
  it("does not save nothing, and says so", async () => {
    const w = await capture();
    await w.note.fill("   ");
    await w.note.press("Enter");
    await text(w.status, "Nothing to save yet");
    assert.equal((await w.saves()).length, 0);
    assert.equal(await w.page.locator(".capture__panel").getAttribute("data-shake"), "true");
    await w.close();
  });
});

describe("save states", () => {
  it("shows Saving while waiting and reports success only after the database confirms", async () => {
    const w = await capture({ settings: { hide_after_save: false } });
    await w.mock.hold("capture_save");
    await w.note.fill("slow disk");
    await w.note.press("Enter");
    await text(w.status, "Saving…");
    assert.equal(await w.note.inputValue(), "slow disk", "text stays until committed");
    await w.mock.release("capture_save", { id: "n1" });
    await text(w.status, "Saved");
    assert.equal(await w.note.inputValue(), "");
    await w.close();
  });

  it("ignores a second Enter while a save is in flight", async () => {
    const w = await capture();
    await w.mock.hold("capture_save");
    await w.note.fill("once");
    await w.note.press("Enter");
    await text(w.status, "Saving…");
    await w.note.press("Enter");
    await w.mock.release("capture_save", { id: "n1" });
    await eventually(() => w.note.inputValue(), (v) => v === "", "cleared");
    assert.equal((await w.saves()).length, 1);
    await w.close();
  });

  it("passes the optional category along, trimmed", async () => {
    const w = await capture();
    await w.note.fill("call the bank");
    await w.page.getByLabel("Category (optional)").fill(" money ");
    await w.note.press("Enter");
    const [save] = await eventually(w.saves, (c) => c.length === 1, "one save");
    assert.equal(save.args.category, "money");
    await w.close();
  });

  it("Enter in the category returns to the note without saving", async () => {
    const w = await capture();
    const category = w.page.getByLabel("Category (optional)");
    await w.note.fill("x");
    await category.fill("work");
    await category.press("Enter");
    await eventually(() => w.note.evaluate((el) => el === document.activeElement), Boolean, "focus");
    assert.equal((await w.saves()).length, 0);
    await w.close();
  });
});

describe("error states", () => {
  it("keeps the text, explains the failure and re-drafts it when saving fails", async () => {
    const w = await capture();
    await w.mock.fail("capture_save", "The disk is full.");
    await w.note.fill("precious");
    await w.note.press("Enter");
    const alert = w.page.getByRole("alert");
    await text(alert, "Not saved: The disk is full.");
    assert.equal(await w.note.inputValue(), "precious");
    await eventually(w.mock.draft, (d) => d?.content === "precious", "draft rewritten");
    await w.close();
  });

  it("surfaces a draft that could not be written", async () => {
    const w = await capture();
    await w.mock.fail("draft_set", "Read-only file system.");
    await w.note.fill("typing");
    await text(w.page.getByRole("alert"), "Draft not saved: Read-only file system.");
    assert.equal(await w.note.inputValue(), "typing");
    await w.close();
  });
});

describe("drafts", () => {
  it("persists while typing, after a short pause", async () => {
    const w = await capture();
    await w.note.pressSequentially("abc");
    await eventually(w.mock.draft, (d) => d?.content === "abc", "draft stored");
    const writes = await w.mock.calls("draft_set");
    assert.ok(writes.length < 3, "not one write per key");
    await w.close();
  });

  it("restores a draft left by a previous session and offers to discard it", async () => {
    const w = await capture({ draft: { content: "from before", category: "work", updated_at: 1 } });
    await eventually(() => w.note.inputValue(), (v) => v === "from before", "restored");
    await text(w.status, "Draft restored");
    await w.page.getByRole("button", { name: "Discard" }).click();
    await eventually(() => w.note.inputValue(), (v) => v === "", "cleared");
    await eventually(() => w.mock.calls("draft_clear"), (c) => c.length === 1, "draft cleared");
    assert.equal(await w.mock.draft(), null);
    await w.close();
  });

  it("never lets a late draft write land after the save", async () => {
    const w = await capture();
    await w.note.pressSequentially("quick");
    await w.note.press("Enter"); // before the draft pause has passed
    await eventually(w.saves, (c) => c.length === 1, "one save");
    await new Promise((r) => setTimeout(r, 400));
    const order = (await w.mock.calls()).map((c) => c.cmd);
    assert.equal(order.lastIndexOf("draft_set") < order.indexOf("capture_save"), true);
    await w.close();
  });

  it("writes the draft when the window is hidden by something else", async () => {
    const w = await capture();
    await w.note.pressSequentially("unsaved");
    await w.mock.emit("capture-hidden");
    await eventually(w.mock.draft, (d) => d?.content === "unsaved", "draft stored");
    await w.close();
  });
});

describe("ai polish", () => {
  const ai = { settings: { ai_provider: "openai" } };

  it("replaces the note text with the polished version", async () => {
    const w = await capture(ai);
    await w.mock.answer("ai_polish", "Buy milk.");
    await w.note.fill("by milk");
    await w.note.press("Control+i");
    await eventually(() => w.note.inputValue(), (v) => v === "Buy milk.", "polished");
    await text(w.status, "Polished");
    await eventually(w.mock.draft, (d) => d?.content === "Buy milk.", "polished text drafted");
    await w.close();
  });

  it("keeps the note and shows the error when polishing fails", async () => {
    const w = await capture(ai);
    await w.mock.fail("ai_polish", "The AI service did not answer.");
    await w.note.fill("by milk");
    await w.page.getByRole("button", { name: "Polish note with AI" }).click();
    await text(w.page.getByRole("alert"), "The AI service did not answer.");
    assert.equal(await w.note.inputValue(), "by milk");
    await w.close();
  });

  it("shows the polish shortcut on the button", async () => {
    const w = await capture(ai);
    const button = w.page.getByRole("button", { name: "Polish note with AI" });
    await text(button, "CtrlIpolish");
    assert.equal(await button.getAttribute("aria-keyshortcuts"), "Control+I");
    await w.close();
  });

  it("locks the note while polishing and ignores a second polish", async () => {
    const w = await capture(ai);
    await w.mock.hold("ai_polish");
    await w.note.fill("by milk");
    await w.note.press("Control+i");
    await text(w.status, "Polishing…");
    assert.equal(await w.note.getAttribute("aria-busy"), "true");
    assert.equal(await w.note.evaluate((el) => el.readOnly), true);
    await w.note.press("Control+i");
    await w.mock.release("ai_polish", "Buy milk.");
    await eventually(() => w.note.inputValue(), (v) => v === "Buy milk.", "polished");
    assert.equal((await w.mock.calls("ai_polish")).length, 1);
    await w.close();
  });

  it("hides the polish button when AI is off", async () => {
    const w = await capture();
    await eventually(() => w.mock.calls("settings_get"), (c) => c.length > 0, "settings read");
    assert.equal(await w.page.getByRole("button", { name: "Polish note with AI" }).count(), 0);
    await w.close();
  });
});

describe("language", () => {
  it("shows Vietnamese when that is the chosen language", async () => {
    const w = await capture({ settings: { language: "vi" } });
    await eventually(() => w.note.getAttribute("placeholder"), (p) => p === "Bạn đang nghĩ gì?", "placeholder");
    assert.equal(await w.page.locator("html").getAttribute("lang"), "vi");
    await w.note.fill("   ");
    await w.note.press("Enter");
    await text(w.status, "Chưa có gì để lưu");
    await w.close();
  });

  it("switches language as soon as the setting changes, keeping what was typed", async () => {
    const w = await capture();
    await w.note.fill("đang gõ dở");
    await w.note.press("Shift+Enter");
    await w.mock.changeSettings({ language: "vi" });
    await text(w.page.locator(".capture__title"), "Ghi nhanh");
    await text(w.page.locator(".capture__hints"), "lưu");
    assert.equal(await w.note.inputValue(), "đang gõ dở\n");
    await w.close();
  });

  it("falls back to English for a language this build does not have", async () => {
    const w = await capture({ settings: { language: "xx" } });
    await eventually(() => w.mock.calls("settings_get"), (c) => c.length > 0, "settings read");
    await text(w.page.locator(".capture__title"), "Quick Note");
    await w.close();
  });
});

describe("appearance", () => {
  it("applies the active look's tokens, shape and font", async () => {
    const w = await open({ label: "capture" });
    await w.mock.answer("look_active", {
      id: "x", name: "X", author: null, base: "light", shape: "round", font: "serif",
      tokens: { "--bg": "rgb(1, 2, 3)" },
    });
    await w.mock.emit("looks-changed");
    const html = w.page.locator("html");
    await eventually(() => html.getAttribute("data-theme"), (t) => t === "light", "theme");
    assert.equal(await html.getAttribute("data-shape"), "round");
    assert.equal(await html.getAttribute("data-font"), "serif");
    assert.equal(await html.evaluate((el) => el.style.getPropertyValue("--bg")), "rgb(1, 2, 3)");
    // Back to Plain: the look's token must not be left behind.
    await w.mock.answer("look_active", null);
    await w.mock.emit("looks-changed");
    await eventually(() => html.evaluate((el) => el.style.getPropertyValue("--bg")), (v) => v === "", "token removed");
    assert.equal(await html.getAttribute("data-shape"), null);
    await w.close();
  });
});
