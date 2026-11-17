//! Which row of a list is selected, as ids. Every list in the app moves
//! its selection through these, so they all behave the same at the ends.

/// Which row is selected after the list was fetched again. The current one
/// if it is still listed and `reset` is off; otherwise the first row.
pub fn selection_after_load(ids: &[&str], current: Option<&str>, reset: bool) -> Option<String> {
    let kept = current.filter(|id| !reset && ids.contains(id));
    kept.or(ids.first().copied()).map(str::to_owned)
}

/// The row `delta` away from the selected one. Stays put at either end, and
/// starts at the first row when nothing is selected.
pub fn moved_selection(ids: &[&str], current: Option<&str>, delta: isize) -> Option<String> {
    let index = current.and_then(|id| ids.iter().position(|row| *row == id));
    let target = match index {
        Some(index) => index
            .checked_add_signed(delta)
            .filter(|i| *i < ids.len())
            .unwrap_or(index),
        None => 0,
    };
    ids.get(target).map(|id| (*id).to_owned())
}

/// The row to select when `id` leaves the list: the next one, else the one
/// before, so the next key acts on the row the eye is already on.
pub fn neighbour(ids: &[&str], id: &str) -> Option<String> {
    let index = ids.iter().position(|row| *row == id)?;
    let next = ids.get(index + 1);
    let previous = index.checked_sub(1).and_then(|i| ids.get(i));
    next.or(previous).map(|id| (*id).to_owned())
}

/// Like [`moved_selection`], for a list where nothing may be selected:
/// moving up from nothing starts at the last row.
pub fn moved_from_none(ids: &[&str], current: Option<&str>, delta: isize) -> Option<String> {
    let known = current.is_some_and(|id| ids.contains(&id));
    if known || delta > 0 {
        moved_selection(ids, current, delta)
    } else {
        ids.last().map(|id| (*id).to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_up_from_nothing_starts_at_the_bottom() {
        let ids = ["a", "b", "c"];
        assert_eq!(moved_from_none(&ids, None, -1).as_deref(), Some("c"));
        assert_eq!(moved_from_none(&ids, None, 1).as_deref(), Some("a"));
        assert_eq!(moved_from_none(&ids, Some("b"), -1).as_deref(), Some("a"));
        assert_eq!(
            moved_from_none(&ids, Some("gone"), -1).as_deref(),
            Some("c")
        );
        assert_eq!(moved_from_none(&[], None, 1), None);
    }
}
