import { useEffect, useRef } from "react";
import type { CapturePosition } from "../lib/types";

/** The nine spots in reading order, with the names a screen reader says. */
const SPOTS: { value: CapturePosition; label: string }[] = [
  { value: "top_left", label: "Top left" },
  { value: "top", label: "Top center" },
  { value: "top_right", label: "Top right" },
  { value: "left", label: "Middle left" },
  { value: "center", label: "Center" },
  { value: "right", label: "Middle right" },
  { value: "bottom_left", label: "Bottom left" },
  { value: "bottom", label: "Bottom center" },
  { value: "bottom_right", label: "Bottom right" },
];

const STEP: Record<string, [number, number]> = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
};

/**
 * A small picture of a display with nine spots: where a floating window
 * opens. A radio group; the arrow keys move through the grid.
 */
export function PositionPicker({
  label,
  value,
  onChange,
  disabled,
}: {
  label: string;
  value: CapturePosition;
  onChange: (value: CapturePosition) => void;
  disabled?: boolean;
}) {
  const group = useRef<HTMLDivElement>(null);
  const index = Math.max(
    0,
    SPOTS.findIndex((s) => s.value === value),
  );

  // Keyboard focus follows the checked spot, as in any radio group.
  useEffect(() => {
    const root = group.current;
    if (root && root.contains(document.activeElement)) {
      root.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
    }
  }, [value]);

  return (
    <div
      ref={group}
      className="position"
      role="radiogroup"
      aria-label={label}
      onKeyDown={(e) => {
        const step = STEP[e.key];
        if (!step || disabled) return;
        e.preventDefault();
        const column = (index % 3) + step[0];
        const row = Math.floor(index / 3) + step[1];
        if (column < 0 || column > 2 || row < 0 || row > 2) return;
        onChange(SPOTS[row * 3 + column]!.value);
      }}
    >
      {SPOTS.map((spot, i) => (
        <button
          key={spot.value}
          type="button"
          role="radio"
          className="position__spot"
          aria-label={spot.label}
          aria-checked={i === index}
          tabIndex={i === index ? 0 : -1}
          title={spot.label}
          disabled={disabled}
          onClick={() => onChange(spot.value)}
        >
          <span className="position__mark" aria-hidden="true" />
        </button>
      ))}
    </div>
  );
}
