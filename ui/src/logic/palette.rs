//! Pure filtering and keyboard navigation for the Command Palette (`⌘K` / `Ctrl+K`).
//! Nothing here touches the DOM, so every ranking and wrap rule is unit-tested.

/// Which section an item belongs to in the palette list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteGroup {
    Navigate,
    Project,
    Action,
    Look,
}

/// One runnable entry shown in the Command Palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteItem {
    pub id: String,
    pub group: PaletteGroup,
    pub title: String,
    pub subtitle: Option<String>,
    pub shortcut: Vec<String>,
    pub color: Option<String>,
}

/// Filters and ranks palette items against `query`:
/// - empty query preserves original order
/// - prefix match on title ranks ahead of word-prefix match, then substring match
/// - subtitle is also searched as a fallback
pub fn filter_items(items: &[PaletteItem], query: &str) -> Vec<PaletteItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return items.to_vec();
    }
    let mut scored: Vec<(u8, usize, &PaletteItem)> = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let title = item.title.to_lowercase();
            let rank = if title.starts_with(&q) {
                0
            } else if title.split_whitespace().any(|w| w.starts_with(&q)) {
                1
            } else if title.contains(&q) {
                2
            } else if item
                .subtitle
                .as_deref()
                .is_some_and(|s| s.to_lowercase().contains(&q))
            {
                3
            } else {
                return None;
            };
            Some((rank, idx, item))
        })
        .collect();
    scored.sort_by_key(|(rank, idx, _)| (*rank, *idx));
    scored
        .into_iter()
        .map(|(_, _, item)| item.clone())
        .collect()
}

/// Moves the active selection index with wrap-around.
pub fn move_index(len: usize, current: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as isize;
    ((current as isize + delta).rem_euclid(len)) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, title: &str, group: PaletteGroup) -> PaletteItem {
        PaletteItem {
            id: id.into(),
            group,
            title: title.into(),
            subtitle: None,
            shortcut: Vec::new(),
            color: None,
        }
    }

    #[test]
    fn ranks_prefix_before_word_prefix_before_substring() {
        let items = vec![
            item("a", "Open Settings", PaletteGroup::Navigate),
            item("b", "Reset", PaletteGroup::Action),
            item("c", "Settings", PaletteGroup::Navigate),
        ];
        let found: Vec<_> = filter_items(&items, "set")
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(found, vec!["c", "a", "b"]);
    }

    #[test]
    fn move_index_wraps_in_both_directions() {
        assert_eq!(move_index(3, 0, -1), 2);
        assert_eq!(move_index(3, 2, 1), 0);
        assert_eq!(move_index(0, 0, 1), 0);
    }
}
