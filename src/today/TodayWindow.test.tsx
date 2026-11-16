import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { dayRange } from "../lib/dates";
import type { Glance } from "../lib/types";
import { api, emit, note, resetIpc } from "../test/mockIpc";
import { TodayWindow } from "./TodayWindow";

vi.mock("../lib/ipc", async () => (await import("../test/mockIpc")).ipcMock);

const DAY = 86_400_000;
// Fixed local noon, so day bounds are deterministic in any timezone.
const NOON = new Date(2026, 10, 15, 12, 0, 0).getTime();

function glance(patch: Partial<Glance> = {}): Glance {
  return { carried: [], today: [], done: 0, total: 0, ...patch };
}

let dateNow: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  resetIpc();
  dateNow = vi.spyOn(Date, "now").mockReturnValue(NOON);
});

afterEach(() => {
  vi.restoreAllMocks();
});

async function open() {
  render(<TodayWindow os="linux" />);
  await waitFor(() => expect(api.glanceGet).toHaveBeenCalled());
  await act(async () => {});
  return userEvent.setup();
}

const progressBar = () => screen.getByLabelText("Notes done") as HTMLProgressElement;
const rows = () => screen.getAllByRole("listitem");
const selectedRow = () => rows().find((r) => r.getAttribute("aria-current") === "true");
const key = (key: string, init: Record<string, unknown> = {}) =>
  fireEvent.keyDown(document.querySelector(".today")!, { key, ...init });

describe("today window", () => {
  it("renders carried-over and today groups, the count and the progress", async () => {
    api.glanceGet.mockResolvedValue(
      glance({
        carried: [note("carry over from yesterday")],
        today: [note("made this morning"), note("finished at lunch", { status: "completed" })],
        done: 2,
        total: 5,
      }),
    );
    await open();

    const carried = screen.getByText("carry over from yesterday");
    const today = screen.getByText("made this morning");
    expect(screen.getByText("Carried over")).toBeInTheDocument();
    expect(screen.getByText("Today")).toBeInTheDocument();
    // Carried over comes first.
    expect(carried.compareDocumentPosition(today) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByText("2 of 5 done")).toBeInTheDocument();
    expect(progressBar().value).toBe(2);
    expect(progressBar().max).toBe(5);
  });

  it("recomputes the day bounds when shown again on a later day", async () => {
    await open();
    const first = dayRange(NOON);
    expect(api.glanceGet).toHaveBeenNthCalledWith(1, first.from, first.to);

    dateNow.mockReturnValue(NOON + 2 * DAY);
    act(() => emit("today-shown"));

    const second = dayRange(NOON + 2 * DAY);
    expect(second.from).not.toBe(first.from);
    await waitFor(() => expect(api.glanceGet).toHaveBeenNthCalledWith(2, second.from, second.to));
  });

  it("toggles done with X on the selected carried-over note", async () => {
    const carried = note("carry over from yesterday");
    api.glanceGet.mockResolvedValue(glance({ carried: [carried], done: 0, total: 1 }));
    await open();
    key("x");
    await waitFor(() => expect(api.noteSetStatus).toHaveBeenCalledWith(carried.id, "completed"));
  });

  it("marks a completed note not done with X", async () => {
    const done = note("finished at lunch", { status: "completed" });
    api.glanceGet.mockResolvedValue(glance({ today: [done], done: 1, total: 1 }));
    await open();
    key("x");
    await waitFor(() => expect(api.noteSetStatus).toHaveBeenCalledWith(done.id, "inbox"));
  });

  it("L sends the selected note to Later and selects the next row", async () => {
    const a = note("first");
    const b = note("second");
    api.glanceGet.mockResolvedValueOnce(glance({ carried: [a, b], total: 2 }));
    api.noteSetLater.mockResolvedValue({ ...a, later_at: NOON });
    await open();
    api.glanceGet.mockResolvedValue(glance({ carried: [b], total: 1 }));
    key("l");
    await waitFor(() => expect(api.noteSetLater).toHaveBeenCalledWith(a.id, true));
    await waitFor(() => expect(screen.queryByText("first")).not.toBeInTheDocument());
    expect(selectedRow()).toHaveTextContent("second");
  });

  it("L on the last row selects the row before it", async () => {
    const a = note("first");
    const b = note("second");
    api.glanceGet.mockResolvedValueOnce(glance({ carried: [a, b], total: 2 }));
    api.noteSetLater.mockResolvedValue({ ...b, later_at: NOON });
    await open();
    key("ArrowDown");
    api.glanceGet.mockResolvedValue(glance({ carried: [a], total: 1 }));
    key("l");
    await waitFor(() => expect(api.noteSetLater).toHaveBeenCalledWith(b.id, true));
    await waitFor(() => expect(screen.queryByText("second")).not.toBeInTheDocument());
    expect(selectedRow()).toHaveTextContent("first");
  });

  it("ignores L with a modifier or while composing", async () => {
    api.glanceGet.mockResolvedValue(glance({ carried: [note("first")], total: 1 }));
    await open();
    key("l", { ctrlKey: true });
    key("l", { isComposing: true });
    expect(api.noteSetLater).not.toHaveBeenCalled();
  });

  it("lists L in the footer", async () => {
    await open();
    expect(document.querySelector(".today__hints")).toHaveTextContent("later");
  });

  it("moves the selection with arrows and J/K", async () => {
    api.glanceGet.mockResolvedValue(
      glance({
        carried: [note("carried")],
        today: [note("first today"), note("second today")],
        done: 0,
        total: 3,
      }),
    );
    await open();
    expect(selectedRow()).toHaveTextContent("carried");

    key("ArrowDown");
    expect(selectedRow()).toHaveTextContent("first today");
    key("j");
    expect(selectedRow()).toHaveTextContent("second today");
    key("ArrowUp");
    expect(selectedRow()).toHaveTextContent("first today");
    key("k");
    expect(selectedRow()).toHaveTextContent("carried");
  });

  it("keeps the selected row in view while moving with the keyboard", async () => {
    const spy = vi.fn();
    Element.prototype.scrollIntoView = spy;
    api.glanceGet.mockResolvedValue(
      glance({ today: [note("first"), note("second"), note("third")], total: 3 }),
    );
    await open();
    key("ArrowDown");
    key("ArrowDown");
    await waitFor(() => expect(spy).toHaveBeenCalledWith({ block: "nearest" }));
    expect(spy.mock.contexts.at(-1)).toHaveTextContent("third");
  });

  it("resets the selection to the first row when shown again", async () => {
    api.glanceGet.mockResolvedValue(
      glance({ carried: [note("carried")], today: [note("made today")], done: 0, total: 2 }),
    );
    await open();
    key("ArrowDown");
    expect(selectedRow()).toHaveTextContent("made today");

    act(() => emit("today-shown"));
    // The reset lands when the fresh list arrives.
    await waitFor(() => expect(selectedRow()).toHaveTextContent("carried"));
  });

  it("keeps the selection on the same note across a notes-changed refetch", async () => {
    const kept = note("made today");
    api.glanceGet.mockResolvedValue(
      glance({ carried: [note("carried")], today: [kept], done: 0, total: 2 }),
    );
    await open();
    key("ArrowDown");
    expect(selectedRow()).toHaveTextContent("made today");

    act(() => emit("notes-changed"));
    await waitFor(() => expect(api.glanceGet).toHaveBeenCalledTimes(2));
    expect(selectedRow()).toHaveTextContent("made today");
  });

  it("closes with Escape but not while an IME composition is active", async () => {
    await open();
    key("Escape");
    await waitFor(() => expect(api.todayDismiss).toHaveBeenCalledTimes(1));

    key("Escape", { isComposing: true });
    expect(api.todayDismiss).toHaveBeenCalledTimes(1);
  });

  it("shows an empty glance with the capture shortcut", async () => {
    await open();
    expect(screen.getByText("Nothing for today")).toBeInTheDocument();
    expect(screen.getByRole("progressbar").getAttribute("max")).toBe("0");
  });

  it("says all done when every note for today is done", async () => {
    api.glanceGet.mockResolvedValue(
      glance({
        today: [note("one", { status: "completed" }), note("two", { status: "completed" })],
        done: 2,
        total: 2,
      }),
    );
    await open();
    expect(screen.getByText("All done")).toBeInTheDocument();
    expect(screen.queryByText(/of .* done/)).toBeNull();
  });

  it("points to the inbox when a group was capped", async () => {
    api.glanceGet.mockResolvedValue(
      glance({
        carried: Array.from({ length: 50 }, (_, i) => note(`carried ${i}`)),
        done: 10,
        total: 60,
      }),
    );
    await open();
    expect(rows()).toHaveLength(50);
    expect(screen.getByText("and 10 more in the inbox")).toBeInTheDocument();
  });

  it("shows a database failure as an alert, never a blank window", async () => {
    api.glanceGet.mockRejectedValue({
      code: "db_open",
      message: "The database is not available.",
    });
    render(<TodayWindow os="linux" />);
    expect(await screen.findByRole("alert")).toHaveTextContent("The database is not available.");
    expect(screen.queryByText("Carried over")).toBeNull();
  });

  it("Enter opens Note details for the selected note", async () => {
    api.glanceGet.mockResolvedValue(
      glance({
        carried: [note("carried", { id: "c1" })],
        today: [note("made today", { id: "t1" })],
        total: 2,
      }),
    );
    await open();
    key("ArrowDown");
    key("Enter");
    expect(api.detailsOpen).toHaveBeenCalledWith("t1", "today");
  });

  it("Enter with a modifier held does nothing", async () => {
    api.glanceGet.mockResolvedValue(glance({ today: [note("made today")], total: 1 }));
    await open();
    key("Enter", { metaKey: true });
    key("Enter", { ctrlKey: true });
    expect(api.detailsOpen).not.toHaveBeenCalled();
  });

  it("the open button opens details too", async () => {
    api.glanceGet.mockResolvedValue(
      glance({ today: [note("made today", { id: "t9" })], total: 1 }),
    );
    const user = await open();
    await user.click(screen.getByRole("button", { name: "Open details (Enter)" }));
    expect(api.detailsOpen).toHaveBeenCalledWith("t9", "today");
  });

  it("marks rows that have a description or files", async () => {
    api.glanceGet.mockResolvedValue(
      glance({
        today: [
          note("plain"),
          note("with more", { has_details: true, attachment_count: 2 }),
          note("one file", { attachment_count: 1 }),
        ],
        total: 3,
      }),
    );
    await open();
    expect(screen.getByLabelText("has details, 2 files")).toHaveTextContent("2");
    expect(screen.getByLabelText("1 file")).toBeInTheDocument();
    expect(rows()[0]!.querySelector(".badges")).toBeNull();
  });

  it("coming back from details keeps the selection", async () => {
    api.glanceGet.mockResolvedValue(
      glance({ carried: [note("carried")], today: [note("made today")], total: 2 }),
    );
    await open();
    key("ArrowDown");
    act(() => emit("today-returned"));
    await waitFor(() => expect(api.glanceGet).toHaveBeenCalledTimes(2));
    expect(selectedRow()).toHaveTextContent("made today");
  });
});

describe("browsing days", () => {
  const lastRange = () => api.glanceGet.mock.calls.at(-1) as [number, number];
  const header = () => screen.getByRole("heading", { level: 1 });

  it("← shows the previous day, → comes back, and → stops at today", async () => {
    await open();
    key("ArrowLeft");
    const yesterday = dayRange(NOON - DAY);
    await waitFor(() => expect(lastRange()).toEqual([yesterday.from, yesterday.to]));
    expect(header()).toHaveTextContent(
      new Intl.DateTimeFormat(undefined, { weekday: "short" }).format(NOON - DAY),
    );
    key("ArrowLeft");
    key("ArrowRight");
    await waitFor(() => expect(lastRange()).toEqual([yesterday.from, yesterday.to]));
    key("ArrowRight");
    const today = dayRange(NOON);
    await waitFor(() => expect(lastRange()).toEqual([today.from, today.to]));
    const calls = api.glanceGet.mock.calls.length;
    key("ArrowRight");
    await act(async () => {});
    expect(api.glanceGet).toHaveBeenCalledTimes(calls);
  });

  it("T and the Back to today button return to today", async () => {
    const user = await open();
    key("ArrowLeft");
    key("ArrowLeft");
    key("t");
    const today = dayRange(NOON);
    await waitFor(() => expect(lastRange()).toEqual([today.from, today.to]));
    key("ArrowLeft");
    await user.click(await screen.findByRole("button", { name: "Back to today" }));
    await waitFor(() => expect(lastRange()).toEqual([today.from, today.to]));
    expect(screen.queryByRole("button", { name: "Back to today" })).toBeNull();
  });

  it("the arrow buttons step days too", async () => {
    const user = await open();
    expect(screen.getByRole("button", { name: "Next day" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Previous day" }));
    const yesterday = dayRange(NOON - DAY);
    await waitFor(() => expect(lastRange()).toEqual([yesterday.from, yesterday.to]));
    expect(screen.getByRole("button", { name: "Next day" })).toBeEnabled();
  });

  it("C opens the calendar and picking a day shows it", async () => {
    await open();
    key("c");
    const cal = await screen.findByRole("dialog", { name: "Choose a day" });
    await waitFor(() => expect(api.notesDayMarks).toHaveBeenCalled());
    // Future days cannot be picked.
    const tomorrow = cal.querySelector<HTMLElement>(`[data-day="2026-11-16"]`);
    if (tomorrow) expect(tomorrow).toBeDisabled();
    fireEvent.click(cal.querySelector<HTMLElement>(`[data-day="2026-11-09"]`)!);
    const picked = dayRange(new Date(2026, 10, 9).getTime());
    await waitFor(() => expect(lastRange()).toEqual([picked.from, picked.to]));
    expect(screen.queryByRole("dialog", { name: "Choose a day" })).toBeNull();
  });

  it("Esc with the calendar open closes only the calendar", async () => {
    await open();
    key("c");
    const cal = await screen.findByRole("dialog", { name: "Choose a day" });
    fireEvent.keyDown(cal.querySelector('[role="grid"]')!, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Choose a day" })).toBeNull();
    expect(api.todayDismiss).not.toHaveBeenCalled();
    key("Escape");
    expect(api.todayDismiss).toHaveBeenCalled();
  });

  it("other keys do nothing while the calendar is open", async () => {
    const n = note("open note");
    api.glanceGet.mockResolvedValue(glance({ carried: [n], total: 1 }));
    await open();
    key("c");
    await screen.findByRole("dialog", { name: "Choose a day" });
    key("x");
    key("l");
    expect(api.noteSetStatus).not.toHaveBeenCalled();
    expect(api.noteSetLater).not.toHaveBeenCalled();
  });

  it("a past day is read-only: X and L do nothing, Enter still opens details", async () => {
    const n = note("from before", { created_at: NOON - 5 * DAY });
    api.glanceGet.mockResolvedValue(glance({ carried: [n], total: 1 }));
    await open();
    key("ArrowLeft");
    await waitFor(() => expect(api.glanceGet).toHaveBeenCalledTimes(2));
    key("x");
    key("l");
    expect(api.noteSetStatus).not.toHaveBeenCalled();
    expect(api.noteSetLater).not.toHaveBeenCalled();
    expect(screen.getByRole("checkbox")).toBeDisabled();
    key("Enter");
    expect(api.detailsOpen).toHaveBeenCalledWith(n.id, "today");
    expect(screen.getByText("As it was on this day")).toBeInTheDocument();
  });

  it("on a past day a note finished later shows as not done", async () => {
    const yesterdayEnd = dayRange(NOON - DAY).to;
    const later = note("finished today", {
      status: "completed",
      completed_at: yesterdayEnd + 1000,
      created_at: NOON - 3 * DAY,
    });
    const then = note("finished then", {
      status: "completed",
      completed_at: yesterdayEnd - 1000,
      created_at: NOON - DAY,
    });
    api.glanceGet.mockResolvedValue(glance({ carried: [later], today: [then], done: 1, total: 2 }));
    await open();
    key("ArrowLeft");
    await waitFor(() => expect(api.glanceGet).toHaveBeenCalledTimes(2));
    const boxes = screen.getAllByRole("checkbox");
    expect(boxes[0]).toHaveAttribute("aria-checked", "false");
    expect(boxes[1]).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText("1 of 2 done")).toBeInTheDocument();
  });

  it("shown again goes back to today; a notes change keeps the day", async () => {
    await open();
    key("ArrowLeft");
    const yesterday = dayRange(NOON - DAY);
    await waitFor(() => expect(lastRange()).toEqual([yesterday.from, yesterday.to]));
    act(() => emit("notes-changed"));
    await waitFor(() => expect(api.glanceGet).toHaveBeenCalledTimes(3));
    expect(lastRange()).toEqual([yesterday.from, yesterday.to]);
    act(() => emit("today-shown"));
    const today = dayRange(NOON);
    await waitFor(() => expect(lastRange()).toEqual([today.from, today.to]));
  });

  it("ignores day keys while composing or with a modifier", async () => {
    await open();
    key("ArrowLeft", { isComposing: true });
    key("ArrowLeft", { metaKey: true });
    key("t", { ctrlKey: true });
    key("c", { altKey: true });
    await act(async () => {});
    expect(api.glanceGet).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog", { name: "Choose a day" })).toBeNull();
  });
});

describe("where a carried note came from", () => {
  it("tags carried rows with their day and leaves today's rows bare", async () => {
    api.glanceGet.mockResolvedValue(
      glance({
        carried: [
          note("from monday", { created_at: new Date(2026, 10, 9, 9).getTime() }),
          note("from yesterday", { created_at: NOON - DAY }),
        ],
        today: [note("made today", { created_at: NOON - 1000 })],
        total: 3,
      }),
    );
    await open();
    const [mon, yest, today] = rows();
    expect(mon!.querySelector(".today__origin")).toHaveTextContent(/6 days ago/);
    expect(yest!.querySelector(".today__origin")).toHaveTextContent("Yesterday");
    expect(today!.querySelector(".today__origin")).toBeNull();
  });

  it("clicking the tag opens the inbox on that day, not the details", async () => {
    const created = new Date(2026, 10, 9, 9).getTime();
    api.glanceGet.mockResolvedValue(
      glance({ carried: [note("old", { created_at: created })], total: 1 }),
    );
    const user = await open();
    await user.click(screen.getByRole("button", { name: /Show notes captured/ }));
    expect(api.inboxOpenDate).toHaveBeenCalledWith("2026-11-09");
    expect(api.detailsOpen).not.toHaveBeenCalled();
  });
});

describe("keys after focus is lost", () => {
  it("Esc and day keys still work when focus falls to the page", async () => {
    await open();
    fireEvent.keyDown(document.body, { key: "ArrowLeft" });
    const yesterday = dayRange(NOON - DAY);
    await waitFor(() =>
      expect(api.glanceGet.mock.calls.at(-1)).toEqual([yesterday.from, yesterday.to]),
    );
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(api.todayDismiss).toHaveBeenCalled();
  });

  it("an empty past day does not talk about today", async () => {
    await open();
    key("ArrowLeft");
    await waitFor(() => expect(api.glanceGet).toHaveBeenCalledTimes(2));
    expect(screen.getByText("Nothing on this day")).toBeInTheDocument();
    expect(screen.queryByText("Nothing for today")).toBeNull();
  });

  it("a capped past day does not point to the inbox", async () => {
    api.glanceGet.mockResolvedValue(glance({ carried: [note("one")], total: 9 }));
    await open();
    expect(screen.getByText(/more in the inbox/)).toBeInTheDocument();
    key("ArrowLeft");
    await waitFor(() => expect(api.glanceGet).toHaveBeenCalledTimes(2));
    expect(screen.queryByText(/more in the inbox/)).toBeNull();
    expect(screen.getByText("and 8 more")).toBeInTheDocument();
  });
});
