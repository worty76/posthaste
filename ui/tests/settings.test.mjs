import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, open, start, stop, text } from "./harness.mjs";

before(start);
after(stop);

const look = (id, name, base = "dark") => ({
  id,
  name,
  author: null,
  builtin: id.startsWith("builtin:"),
  base,
  preview: { bg: "#101010", surface: "#202020", text: "#f0f0f0", accent: "#ff8800" },
});
const provider = (patch = {}) => ({
  id: "openai",
  name: "OpenAI",
  key: "required",
  has_key: false,
  key_url: "https://example.com/keys",
  key_help: "Keys start with sk-.",
  default_model: "small-1",
  base_url: "https://api.example.com/v1",
  base_url_editable: false,
  ...patch,
});

/** Opens the main window on Settings. */
async function settings(options = {}) {
  const w = await open({ label: "main", ...options });
  await w.page.getByRole("button", { name: "Settings", exact: true }).click();
  await eventually(
    () => w.page.locator(".settings").count(),
    (c) => c === 1,
    "settings to show",
  );
  const saved = async () => (await w.mock.calls("settings_update")).at(-1)?.args.settings;
  const toast = w.page.locator(".toast");
  return { ...w, saved, toast, calls: (cmd) => w.mock.calls(cmd) };
}

describe("settings", () => {
  it("changes the language at once, in every part of the window", async () => {
    const w = await settings();
    const picker = w.page.getByLabel("Language");
    assert.deepEqual(await picker.locator("option").allTextContents(), ["English", "Tiếng Việt"]);
    await picker.selectOption("vi");
    await text(w.page.locator(".content__title"), "Cài đặt");
    await text(w.page.locator("#s-general"), "Chung");
    await text(w.page.locator(".nav"), "Hộp thư");
    assert.equal((await w.saved()).language, "vi");
    await w.page.getByLabel("Ngôn ngữ").selectOption("en");
    await text(w.page.locator(".content__title"), "Settings");
    assert.deepEqual(w.errors, []);
    await w.close();
  });

  it("applies a switch and a choice as they are made", async () => {
    const w = await settings();
    await w.page.getByRole("switch", { name: "Launch at login" }).click();
    await eventually(w.saved, (s) => s?.launch_at_login === true, "switch saved");
    assert.equal(
      await w.page.getByRole("switch", { name: "Launch at login" }).getAttribute("aria-checked"),
      "true",
    );
    await w.page
      .getByRole("radiogroup", { name: "Enter key" })
      .getByRole("radio", { name: "Adds a line" })
      .click();
    await eventually(w.saved, (s) => s?.enter_behavior === "newline", "choice saved");
    await w.page
      .getByRole("radiogroup", { name: "Window size" })
      .getByRole("radio", { name: "Standard" })
      .press("ArrowRight");
    await eventually(w.saved, (s) => s?.window_size === "large", "arrow key saved");
    await w.close();
  });

  it("keeps the old value and says why when a change is refused", async () => {
    const w = await settings();
    await w.mock.fail("settings_update", "The settings could not be saved.");
    await w.page.getByRole("switch", { name: "Start minimized" }).click();
    await text(w.toast, "The settings could not be saved.");
    assert.equal(
      await w.page.getByRole("switch", { name: "Start minimized" }).getAttribute("aria-checked"),
      "false",
    );
    await w.close();
  });

  it("moves a window position with the arrow keys", async () => {
    const w = await settings();
    const grid = w.page.getByRole("radiogroup", { name: "Capture window position" });
    await grid.getByRole("radio", { name: "Center", exact: true }).press("ArrowUp");
    await eventually(w.saved, (s) => s?.capture_position === "top", "position saved");
    await w.close();
  });

  it("records a shortcut, ignores unusable ones, and restores the default", async () => {
    const w = await settings();
    const recorder = w.page.getByRole("button", { name: "Capture shortcut: change" });
    await recorder.click();
    await text(w.page.locator('.recorder[data-recording="true"]'), "Press shortcut…");
    await w.page.keyboard.press("a");
    assert.equal((await w.calls("settings_update")).length, 0, "a bare letter is not a shortcut");
    await w.page.keyboard.press("Control+Alt+KeyK");
    await eventually(w.saved, (s) => s?.capture_shortcut === "Ctrl+Alt+K", "shortcut saved");
    await w.page.getByRole("button", { name: "Restore default" }).first().click();
    await eventually(
      w.saved,
      (s) => s?.capture_shortcut === "CmdOrCtrl+Shift+Space",
      "default back",
    );
    await text(w.page.locator(".status").first(), "Registered and active.");
    await w.close();
  });

  it("exports, imports and backs up, saying what happened", async () => {
    const w = await settings();
    await text(w.page.locator(".path"), "/home/me/posthaste.db");
    await text(w.page.locator("#s-data + .row"), "0 notes, 86 KB.");
    await w.page.getByRole("button", { name: "Import…" }).click();
    await text(w.toast.last(), "Imported 2 notes, skipped 1");
    await w.page.getByRole("button", { name: "Back up…" }).click();
    await text(w.toast.last(), "Backup created");
    await w.page.getByRole("button", { name: "Export…" }).click();
    await text(w.toast.last(), "Exported 0 notes");
    assert.equal((await w.calls("data_export"))[0].args.utcOffsetMinutes, 420);
    await w.close();
  });

  it("clears everything only after the word is typed", async () => {
    const w = await settings();
    await w.page.getByRole("button", { name: "Clear data…" }).click();
    const dialog = w.page.getByRole("dialog");
    const confirm = dialog.getByRole("button", { name: "Delete everything" });
    assert.equal(await confirm.isDisabled(), true);
    await dialog.getByLabel("Type DELETE to confirm").fill("DELETE");
    await confirm.click();
    await text(w.toast, "All data cleared");
    assert.deepEqual((await w.calls("data_clear"))[0].args, { confirm: "DELETE" });
    await w.close();
  });

  it("checks for updates on request", async () => {
    const w = await settings();
    await w.page.getByRole("button", { name: "Check for updates" }).click();
    await text(w.page.locator("#s-updates ~ .row"), "You're on the latest version.");
    await w.mock.change("store.update = arg", {
      version: "9.9.9",
      current_version: "0.0.0",
      notes: null,
    });
    await w.page.getByRole("button", { name: "Check for updates" }).click();
    await text(w.page.locator("#s-updates ~ .row"), "Posthaste 9.9.9 is ready to install.");
    await w.page.getByRole("button", { name: "Update to 9.9.9" }).click();
    await eventually(
      () => w.calls("update_install"),
      (c) => c.length === 1,
      "install",
    );
    await w.close();
  });
});

describe("AI polish settings", () => {
  const ai = (patch) => ({ ai: { backend: "keychain", providers: [provider(patch)] } });

  it("sets up a provider: key, model and a test", async () => {
    const w = await settings({ store: ai() });
    await w.page.getByLabel("Polish with").selectOption("openai");
    await eventually(w.saved, (s) => s?.ai_provider === "openai", "provider saved");
    await text(
      w.page.locator(".ai__about"),
      "Keys start with sk-. Polishing sends the note text to OpenAI.",
    );
    const key = w.page.getByLabel("OpenAI API key");
    await key.fill("sk-secret");
    await key.press("Enter");
    await text(w.page.locator("#s-ai ~ .row").nth(1), "Saved in your system keychain.");
    assert.deepEqual((await w.calls("ai_key_set"))[0].args, {
      provider: "openai",
      key: "sk-secret",
    });
    assert.equal(
      JSON.stringify(await w.calls("settings_update")).includes("sk-secret"),
      false,
      "the key never goes into settings",
    );
    await w.page.getByLabel("OpenAI model").fill("big-2");
    await eventually(w.saved, (s) => s?.ai_models.openai === "big-2", "model saved");
    await w.page.getByRole("button", { name: "Test" }).click();
    await text(w.page.locator(".ai__result"), "It works. It returned: Fixed text.");
    await w.close();
  });

  it("removes a saved key and shows a failed test in the shell's words", async () => {
    const w = await settings({ store: ai({ has_key: true }), settings: { ai_provider: "openai" } });
    await w.mock.fail("ai_test", "The key was refused.");
    await w.page.getByRole("button", { name: "Test" }).click();
    await text(w.page.locator(".ai__result"), "The key was refused.");
    await w.page.getByRole("button", { name: "Remove" }).click();
    await eventually(
      () => w.page.getByLabel("OpenAI API key").count(),
      (c) => c === 1,
      "key field back",
    );
    await w.close();
  });

  it("keeps an unknown provider selected instead of turning polish off", async () => {
    const w = await settings({ store: ai(), settings: { ai_provider: "gone" } });
    await text(w.page.getByLabel("Polish with"), "gone (not available)");
    assert.equal(await w.page.getByLabel("Polish with").inputValue(), "gone");
    await w.close();
  });
});

describe("looks", () => {
  const looks = () => ({ looks: [look("builtin:ink", "Ink"), look("user:1", "Mine")] });

  it("picks a look by click and by arrow key", async () => {
    const w = await settings({ store: looks() });
    await w.page.getByRole("radio", { name: "Mine" }).click();
    await eventually(w.saved, (s) => s?.look_id === "user:1", "look saved");
    await w.page.getByRole("radio", { name: "Mine" }).press("ArrowRight");
    await eventually(w.saved, (s) => s?.look_id === null, "Plain is after the last look");
    await w.close();
  });

  it("shares a look as a code and adds one from a code", async () => {
    const w = await settings({ store: looks(), settings: { look_id: "user:1" } });
    await w.page.evaluate(() => (navigator.clipboard.writeText = async (t) => (window.copied = t)));
    await w.page.getByRole("button", { name: "Copy share code" }).click();
    await text(w.toast, "Copied share code");
    assert.equal(await w.page.evaluate(() => window.copied), "posthaste-look:user:1");

    await w.page.getByRole("button", { name: "Add from code…" }).click();
    const dialog = w.page.getByRole("dialog");
    await dialog.getByLabel("Look code").fill("nonsense");
    await dialog.getByRole("button", { name: "Add look" }).click();
    await text(dialog.getByRole("alert"), "That is not a look code.");
    await dialog.getByLabel("Look code").fill("posthaste-look:abc");
    await dialog.getByRole("button", { name: "Add look" }).click();
    await text(w.toast.last(), 'Added "Imported"');
    await eventually(w.saved, (s) => s?.look_id === "user:2", "new look selected");
    await w.close();
  });

  it("copies a bundled look into the editor, previews it, and saves", async () => {
    const w = await settings({ store: looks(), settings: { look_id: "builtin:ink" } });
    await w.page.getByRole("button", { name: "Make a copy" }).click();
    const dialog = w.page.getByRole("dialog", { name: "New look" });
    assert.equal(await dialog.getByLabel("Name").inputValue(), "Ink copy");
    await text(dialog.getByRole("status"), "Looks good");
    await dialog.getByLabel("Background", { exact: true }).fill("#abc");
    await eventually(
      () => w.page.evaluate(() => document.documentElement.style.getPropertyValue("--bg")),
      (v) => v === "#abc",
      "the window to wear the draft",
    );
    assert.equal(await dialog.getByLabel("Background picker").inputValue(), "#aabbcc");
    await dialog.getByLabel("Background", { exact: true }).fill("blue-ish");
    await text(dialog.getByRole("alert"), "Background is not a colour.");
    assert.equal(await dialog.getByRole("button", { name: "Save" }).isDisabled(), true);
    await dialog.getByLabel("Background", { exact: true }).fill("#123456");
    await text(dialog.getByRole("status"), "Looks good");
    await dialog.getByRole("button", { name: "Save" }).click();
    await text(w.toast, 'Saved "Ink copy"');
    assert.equal((await w.calls("look_save"))[0].args.id, null);
    await w.close();
  });

  it("deletes a look of the user's after asking", async () => {
    const w = await settings({ store: looks(), settings: { look_id: "user:1" } });
    await w.page.getByRole("button", { name: "Delete", exact: true }).click();
    const dialog = w.page.getByRole("dialog");
    await text(dialog, "Delete “Mine”?");
    await dialog.getByRole("button", { name: "Delete" }).click();
    await eventually(
      () => w.page.getByRole("radio", { name: "Mine" }).count(),
      (c) => c === 0,
      "look gone",
    );
    await w.close();
  });
});
