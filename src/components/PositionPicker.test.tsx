import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CapturePosition } from "../lib/types";
import { PositionPicker } from "./PositionPicker";

function setup(value: CapturePosition = "center") {
  const onChange = vi.fn();
  render(<PositionPicker label="Capture position" value={value} onChange={onChange} />);
  return onChange;
}

const spot = (name: string) => screen.getByRole("radio", { name });

describe("PositionPicker", () => {
  it("offers nine named spots in reading order, with the current one checked", () => {
    setup("top_right");
    const group = screen.getByRole("radiogroup", { name: "Capture position" });
    const names = Array.from(group.querySelectorAll('[role="radio"]')).map((r) =>
      r.getAttribute("aria-label"),
    );
    expect(names).toEqual([
      "Top left",
      "Top center",
      "Top right",
      "Middle left",
      "Center",
      "Middle right",
      "Bottom left",
      "Bottom center",
      "Bottom right",
    ]);
    expect(spot("Top right")).toHaveAttribute("aria-checked", "true");
    expect(spot("Center")).toHaveAttribute("aria-checked", "false");
  });

  it("only the checked spot is in the tab order", () => {
    setup("bottom_left");
    expect(spot("Bottom left")).toHaveAttribute("tabindex", "0");
    expect(spot("Center")).toHaveAttribute("tabindex", "-1");
  });

  it("picks a spot on click", () => {
    const onChange = setup();
    fireEvent.click(spot("Bottom right"));
    expect(onChange).toHaveBeenCalledWith("bottom_right");
  });

  it("moves through the grid with the arrow keys and stops at the edges", () => {
    const onChange = setup("center");
    const group = screen.getByRole("radiogroup");
    fireEvent.keyDown(group, { key: "ArrowUp" });
    expect(onChange).toHaveBeenLastCalledWith("top");
    fireEvent.keyDown(group, { key: "ArrowRight" });
    expect(onChange).toHaveBeenLastCalledWith("right");
    fireEvent.keyDown(group, { key: "ArrowDown" });
    expect(onChange).toHaveBeenLastCalledWith("bottom");
    fireEvent.keyDown(group, { key: "ArrowLeft" });
    expect(onChange).toHaveBeenLastCalledWith("left");
    onChange.mockClear();
  });

  it("does nothing past an edge", () => {
    const onChange = setup("top_left");
    const group = screen.getByRole("radiogroup");
    fireEvent.keyDown(group, { key: "ArrowUp" });
    fireEvent.keyDown(group, { key: "ArrowLeft" });
    expect(onChange).not.toHaveBeenCalled();
  });
});
