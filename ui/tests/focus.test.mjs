import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, note, open, start, stop, text } from "./harness.mjs";

before(start);
after(stop);

describe("focus pill window", () => {
  it("shows the focused note's first line, step progress, and handles complete and clear", async () => {
    const task = note("Fix auth token refresh\nExtra detail line", {
      id: "n-focus",
      step_total: 4,
      step_done: 2,
    });
    const w = await open({
      label: "focus",
      store: { notes: [task], focusedId: task.id },
    });
    const pill = w.page.getByRole("region", { name: "Current focus" });
    await text(pill, "Fix auth token refresh");
    await text(pill, "2/4");
    assert.equal(
      await w.page.locator(".focus-pill__title").getAttribute("data-tauri-drag-region"),
      "",
    );

    await w.page.getByRole("button", { name: "Mark as completed" }).click();
    await eventually(
      () => w.mock.calls("note_set_status"),
      (c) => c.length === 1 && c[0].args.id === task.id && c[0].args.status === "completed",
      "note completed",
    );

    await w.page.getByRole("button", { name: "Clear focus" }).click();
    await eventually(
      () => w.mock.calls("focus_clear"),
      (c) => c.length === 1,
      "focus cleared",
    );
    assert.equal(await pill.count(), 0);
    assert.deepEqual(w.errors, []);
    await w.close();
  });

  it("updates live when focus-changed arrives from another window", async () => {
    const first = note("First task", { id: "n1" });
    const second = note("Second task", { id: "n2", step_total: 3, step_done: 1 });
    const w = await open({ label: "focus", store: { notes: [first, second] } });
    const pill = w.page.getByRole("region", { name: "Current focus" });
    await eventually(
      () => w.mock.calls("focus_get"),
      (c) => c.length > 0,
      "initial focus_get",
    );
    assert.equal(await pill.count(), 0);

    await w.mock.emit("focus-changed", second);
    await text(pill, "Second task");
    await text(pill, "1/3");

    await w.mock.emit("focus-changed", null);
    await eventually(
      () => pill.count(),
      (n) => n === 0,
      "pill hidden after clear",
    );
    assert.deepEqual(w.errors, []);
    await w.close();
  });

  it("enables running marquee text only when a title overflows the pill", async () => {
    const shortNote = note("Short task", { id: "n-short" });
    const longNote = note(
      "Refactor authentication token refresh flow across sleep wake cycles and verify keychain fallback",
      { id: "n-long" },
    );
    const w = await open({
      label: "focus",
      store: { notes: [shortNote, longNote], focusedId: shortNote.id },
    });
    await w.page.setViewportSize({ width: 340, height: 56 });
    const title = w.page.locator(".focus-pill__title");
    await eventually(
      () => title.getAttribute("data-overflow"),
      (v) => v === "false",
      "short title stays static",
    );

    await w.mock.emit("focus-changed", longNote);
    await eventually(
      () => title.getAttribute("data-overflow"),
      (v) => v === "true",
      "long title enables marquee",
    );
    assert.equal(await w.page.locator(".focus-pill__copy--dup").count(), 1);
    assert.deepEqual(w.errors, []);
    await w.close();
  });
});
