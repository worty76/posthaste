import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Calendar } from "./Calendar";

// Sunday 15 November 2026, local noon.
const NOW = new Date(2026, 10, 15, 12).getTime();

beforeEach(() => {
  vi.spyOn(Date, "now").mockReturnValue(NOW);
});
afterEach(() => vi.restoreAllMocks());

function setup(props: Partial<Parameters<typeof Calendar>[0]> = {}) {
  const handlers = {
    onPick: vi.fn(),
    onClose: vi.fn(),
    onClear: vi.fn(),
    onMonthChange: vi.fn(),
  };
  render(<Calendar value="" marks={{}} weekStartsOn={1} {...handlers} {...props} />);
  return handlers;
}

const grid = () => screen.getByRole("grid");
const cell = (key: string) => document.querySelector<HTMLElement>(`[data-day="${key}"]`)!;
const press = (key: string, init: Record<string, unknown> = {}) =>
  fireEvent.keyDown(grid(), { key, ...init });

describe("Calendar", () => {
  it("opens on the chosen day's month, or today's, and reports the month's edges", () => {
    const h = setup();
    expect(screen.getByText("November 2026")).toBeInTheDocument();
    expect(h.onMonthChange).toHaveBeenCalledTimes(1);
    expect(h.onMonthChange.mock.calls[0]![0]).toHaveLength(43);
    expect(document.activeElement).toBe(cell("2026-11-15"));
    expect(cell("2026-11-15")).toHaveAttribute("data-today", "true");
  });

  it("starts on the selected day when there is one", () => {
    setup({ value: "2026-09-03" });
    expect(screen.getByText("September 2026")).toBeInTheDocument();
    expect(cell("2026-09-03")).toHaveAttribute("aria-selected", "true");
  });

  it("moves by day and week with arrows and by month with Page keys", () => {
    const h = setup({ value: "2026-10-14" });
    press("ArrowRight");
    expect(document.activeElement).toBe(cell("2026-10-15"));
    press("ArrowDown");
    expect(document.activeElement).toBe(cell("2026-10-22"));
    press("ArrowUp");
    press("ArrowLeft");
    expect(document.activeElement).toBe(cell("2026-10-14"));
    press("PageUp");
    expect(screen.getByText("September 2026")).toBeInTheDocument();
    expect(document.activeElement).toBe(cell("2026-09-14"));
    press("PageDown");
    press("PageDown");
    expect(screen.getByText("November 2026")).toBeInTheDocument();
    expect(h.onMonthChange).toHaveBeenCalledTimes(4);
  });

  it("changes month with the arrow buttons", () => {
    setup();
    fireEvent.click(screen.getByRole("button", { name: "Previous month" }));
    expect(screen.getByText("October 2026")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Next month" }));
    expect(screen.getByText("November 2026")).toBeInTheDocument();
  });

  it("picks with Enter or a click, and T picks today", () => {
    const h = setup({ value: "2026-11-02" });
    press("ArrowRight");
    press("Enter");
    expect(h.onPick).toHaveBeenLastCalledWith("2026-11-03");
    fireEvent.click(cell("2026-11-10"));
    expect(h.onPick).toHaveBeenLastCalledWith("2026-11-10");
    press("t");
    expect(h.onPick).toHaveBeenLastCalledWith("2026-11-15");
    fireEvent.click(screen.getByRole("button", { name: "Today" }));
    expect(h.onPick).toHaveBeenCalledTimes(4);
  });

  it("closes with Esc and clears with Clear", () => {
    const h = setup({ value: "2026-11-02" });
    press("Escape");
    expect(h.onClose).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(h.onClear).toHaveBeenCalled();
  });

  it("has no Clear without a clear handler", () => {
    render(<Calendar value="" marks={{}} onPick={vi.fn()} onClose={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "Clear" })).toBeNull();
  });

  it("closes on a click outside", () => {
    const h = setup();
    fireEvent.mouseDown(document.body);
    expect(h.onClose).toHaveBeenCalled();
  });

  it("disables days after max and does not move onto them", () => {
    const h = setup({ max: "2026-11-15" });
    expect(cell("2026-11-16")).toBeDisabled();
    press("ArrowRight");
    expect(document.activeElement).toBe(cell("2026-11-15"));
    fireEvent.click(screen.getByRole("button", { name: "Next month" }));
    expect(h.onMonthChange).toHaveBeenCalledTimes(1);
  });

  it("marks days with open notes as filled and finished days as hollow", () => {
    setup({
      marks: {
        "2026-11-09": { total: 3, open: 1 },
        "2026-11-10": { total: 2, open: 0 },
      },
    });
    expect(cell("2026-11-09")).toHaveAttribute("data-mark", "open");
    expect(cell("2026-11-09")).toHaveAccessibleName(/1 open/);
    expect(cell("2026-11-10")).toHaveAttribute("data-mark", "done");
    expect(cell("2026-11-10")).toHaveAccessibleName(/all done/);
    expect(cell("2026-11-11")).not.toHaveAttribute("data-mark");
  });

  it("ignores keys while composing or with a modifier", () => {
    const h = setup();
    press("ArrowRight", { isComposing: true });
    press("ArrowRight", { metaKey: true });
    press("t", { ctrlKey: true });
    expect(document.activeElement).toBe(cell("2026-11-15"));
    expect(h.onPick).not.toHaveBeenCalled();
  });
});
