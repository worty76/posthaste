//! The nine spots a floating window can open at, as a 3×3 grid.

use posthaste_model::CapturePosition;

/// The spots in reading order: left to right, top to bottom.
pub const SPOTS: [CapturePosition; 9] = [
    CapturePosition::TopLeft,
    CapturePosition::Top,
    CapturePosition::TopRight,
    CapturePosition::Left,
    CapturePosition::Center,
    CapturePosition::Right,
    CapturePosition::BottomLeft,
    CapturePosition::Bottom,
    CapturePosition::BottomRight,
];

/// The spot an arrow key moves to from `from`. `None` for any other key,
/// and at the edge of the grid.
pub fn step(from: CapturePosition, key: &str) -> Option<CapturePosition> {
    let (columns, rows): (i8, i8) = match key {
        "ArrowLeft" => (-1, 0),
        "ArrowRight" => (1, 0),
        "ArrowUp" => (0, -1),
        "ArrowDown" => (0, 1),
        _ => return None,
    };
    let index = SPOTS.iter().position(|spot| *spot == from)? as i8;
    let (column, row) = (index % 3 + columns, index / 3 + rows);
    if !(0..3).contains(&column) || !(0..3).contains(&row) {
        return None;
    }
    SPOTS.get((row * 3 + column) as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_walk_the_grid_and_stop_at_its_edges() {
        use CapturePosition::*;
        assert_eq!(step(Center, "ArrowLeft"), Some(Left));
        assert_eq!(step(Center, "ArrowRight"), Some(Right));
        assert_eq!(step(Center, "ArrowUp"), Some(Top));
        assert_eq!(step(Center, "ArrowDown"), Some(Bottom));
        assert_eq!(step(TopLeft, "ArrowLeft"), None);
        assert_eq!(step(TopLeft, "ArrowUp"), None);
        assert_eq!(step(BottomRight, "ArrowRight"), None);
        assert_eq!(step(BottomRight, "ArrowDown"), None);
        // The end of a row does not wrap onto the next.
        assert_eq!(step(TopRight, "ArrowRight"), None);
        assert_eq!(step(Center, "Enter"), None);
    }
}
