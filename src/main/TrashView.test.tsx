import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Note, TrashedNote } from "../lib/types";
import { api, emit, note, resetIpc } from "../test/mockIpc";
import { TrashView } from "./TrashView";

vi.mock("../lib/ipc", async () => (await import("../test/mockIpc")).ipcMock);

const DAY = 86_400_000;

function trashed(n: Note, ago: number): TrashedNote {
  return { note: n, deleted_at: Date.now() - ago };
}

const show = vi.fn();
const fail = vi.fn();

beforeEach(() => {
  resetIpc();
  show.mockReset();
  fail.mockReset();
});

async function open(items: TrashedNote[]) {
  api.trashList.mockResolvedValue(items);
  render(<TrashView show={show} fail={fail} />);
  await waitFor(() => expect(api.trashList).toHaveBeenCalled());
  await act(async () => {});
  return userEvent.setup();
}

const rows = () => screen.getAllByRole("listitem");
const selected = () => rows().find((r) => r.getAttribute("aria-current") === "true");
const key = (k: string, init: Record<string, unknown> = {}) =>
  fireEvent.keyDown(window, { key: k, ...init });

describe("Trash", () => {
  it("lists deleted notes by the day they were deleted", async () => {
    await open([
      trashed(note("deleted just now"), 1000),
      trashed(note("deleted long ago"), 5 * DAY),
    ]);
    expect(screen.getByRole("region", { name: "Today" })).toHaveTextContent("deleted just now");
    expect(screen.getAllByRole("region")).toHaveLength(2);
  });

  it("says where each note returns and when it will be erased", async () => {
    await open([
      trashed(note("open one"), 1000),
      trashed(note("done one", { status: "completed", completed_at: 1 }), 2000),
      trashed(note("parked one", { later_at: 1 }), 3000),
      trashed(note("filed one", { status: "archived", archived_at: 1 }), 4000),
    ]);
    const [a, b, c, d] = rows();
    expect(a).toHaveTextContent("Inbox");
    expect(b).toHaveTextContent("Archive");
    expect(c).toHaveTextContent("Later");
    expect(d).toHaveTextContent("Archive");
    expect(a).toHaveTextContent(/Erased on/);
  });

  it("R and Enter restore the selected note and move to the next", async () => {
    const a = note("first");
    const b = note("second");
    const c = note("third");
    api.noteUndelete.mockImplementation(async (id: string) => ({ ...a, id }));
    await open([trashed(a, 1), trashed(b, 2), trashed(c, 3)]);
    key("j");
    key("r");
    await waitFor(() => expect(api.noteUndelete).toHaveBeenCalledWith(a.id));
    await waitFor(() => expect(selected()).toHaveTextContent("second"));
    key("Enter");
    await waitFor(() => expect(api.noteUndelete).toHaveBeenCalledWith(b.id));
  });

  it("the restore message offers Undo, which puts it back in the trash", async () => {
    const a = note("first");
    api.noteUndelete.mockResolvedValue(a);
    api.noteDelete.mockResolvedValue(undefined);
    await open([trashed(a, 1)]);
    key("j");
    key("r");
    await waitFor(() => expect(show).toHaveBeenCalled());
    const [text, opts] = show.mock.calls.at(-1)!;
    expect(text).toBe("Restored to Inbox");
    opts.action.run();
    expect(api.noteDelete).toHaveBeenCalledWith(a.id);
  });

  it("Delete asks first; Cancel keeps the note, confirming erases it", async () => {
    const a = note("to erase");
    const user = await open([trashed(a, 1)]);
    key("j");
    key("Delete");
    const dialog = await screen.findByRole("dialog", { name: "Delete this note for good?" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(api.trashPurge).not.toHaveBeenCalled();
    key("Backspace");
    const again = await screen.findByRole("dialog", { name: "Delete this note for good?" });
    await user.click(within(again).getByRole("button", { name: "Delete forever" }));
    await waitFor(() => expect(api.trashPurge).toHaveBeenCalledWith(a.id));
  });

  it("the row buttons restore and delete forever", async () => {
    const a = note("row actions");
    api.noteUndelete.mockResolvedValue(a);
    const user = await open([trashed(a, 1)]);
    await user.click(screen.getByRole("button", { name: "Restore (R)" }));
    await waitFor(() => expect(api.noteUndelete).toHaveBeenCalledWith(a.id));
    await user.click(screen.getByRole("button", { name: "Delete forever (Del)" }));
    expect(
      await screen.findByRole("dialog", { name: "Delete this note for good?" }),
    ).toBeInTheDocument();
  });

  it("Empty Trash asks first, then erases what was listed", async () => {
    api.trashEmpty.mockResolvedValue(2);
    const items = [trashed(note("a"), 1), trashed(note("b"), 2)];
    const user = await open(items);
    await user.click(screen.getByRole("button", { name: "Empty Trash" }));
    const dialog = await screen.findByRole("dialog", { name: "Empty the trash?" });
    expect(dialog).toHaveTextContent("2 notes");
    await user.click(within(dialog).getByRole("button", { name: "Empty Trash" }));
    // Only up to the newest deletion shown, so a note deleted since is spared.
    await waitFor(() => expect(api.trashEmpty).toHaveBeenCalledWith(items[0]!.deleted_at));
  });

  it("the delete dialog names the note and does not overpromise", async () => {
    await open([trashed(note("Reply to Minh about the invoice"), 1)]);
    key("j");
    key("Delete");
    const dialog = await screen.findByRole("dialog", { name: "Delete this note for good?" });
    expect(dialog).toHaveTextContent("Reply to Minh about the invoice");
    expect(dialog).not.toHaveTextContent("erased from this computer");
  });

  it("long notes are shortened in the dialog", async () => {
    await open([trashed(note("x".repeat(300)), 1)]);
    key("j");
    key("Delete");
    const dialog = await screen.findByRole("dialog", { name: "Delete this note for good?" });
    expect(dialog.textContent!.length).toBeLessThan(300);
    expect(dialog).toHaveTextContent("…");
  });

  it("keeps the selected row in view", async () => {
    const spy = vi.fn();
    Element.prototype.scrollIntoView = spy;
    await open([trashed(note("a"), 1), trashed(note("b"), 2)]);
    key("j");
    key("j");
    await waitFor(() => expect(spy).toHaveBeenCalledWith({ block: "nearest" }));
    expect(spy.mock.contexts.at(-1)).toHaveTextContent("b");
  });

  it("shows an empty state that explains the keep time", async () => {
    await open([]);
    expect(screen.getByText("Trash is empty")).toBeInTheDocument();
    expect(screen.getByText(/wait here for 30 days/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Empty Trash" })).toBeNull();
  });

  it("ignores keys while composing, with a modifier, or while typing", async () => {
    const a = note("guarded");
    await open([trashed(a, 1)]);
    key("j");
    key("r", { isComposing: true });
    key("r", { metaKey: true });
    key("Delete", { ctrlKey: true });
    const input = document.createElement("input");
    document.body.appendChild(input);
    fireEvent.keyDown(input, { key: "Backspace" });
    fireEvent.keyDown(input, { key: "r" });
    expect(api.noteUndelete).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
    input.remove();
  });

  it("offers no edit, complete or archive actions", async () => {
    await open([trashed(note("plain"), 1)]);
    expect(screen.queryByRole("button", { name: /Edit|Complete|Archive/ })).toBeNull();
    expect(screen.queryByRole("checkbox")).toBeNull();
  });

  it("reloads when notes change elsewhere", async () => {
    await open([]);
    act(() => emit("notes-changed"));
    await waitFor(() => expect(api.trashList).toHaveBeenCalledTimes(2));
  });
});
