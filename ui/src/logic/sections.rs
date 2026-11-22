//! Grouping a note list into flat project sections and moving the selection
//! across only the notes that are currently visible.

use std::collections::{HashMap, HashSet};

use posthaste_model::Note;

/// One project section in a list. `project` is `None` for notes without a
/// project, which render last with no header and cannot collapse.
#[derive(Debug, PartialEq, Eq)]
pub struct Section<'a> {
    pub project: Option<&'a str>,
    pub notes: Vec<&'a Note>,
}

/// Groups `notes` by project in order of each section's first note (lists
/// arrive newest first), placing the no-project group last.
pub fn group(notes: &[Note]) -> Vec<Section<'_>> {
    let mut sections: Vec<Section<'_>> = Vec::new();
    let mut by_project: HashMap<&str, usize> = HashMap::new();
    let mut plain: Vec<&Note> = Vec::new();

    for note in notes {
        match note.category.as_deref() {
            Some(project) => {
                if let Some(&idx) = by_project.get(project) {
                    sections[idx].notes.push(note);
                } else {
                    let idx = sections.len();
                    by_project.insert(project, idx);
                    sections.push(Section {
                        project: Some(project),
                        notes: vec![note],
                    });
                }
            }
            None => plain.push(note),
        }
    }

    if !plain.is_empty() {
        sections.push(Section {
            project: None,
            notes: plain,
        });
    }
    sections
}

/// Note ids in display order, skipping collapsed project sections. The
/// no-project group can never collapse.
pub fn visible_order(sections: &[Section<'_>], collapsed: &HashSet<String>) -> Vec<String> {
    let mut ids = Vec::new();
    for section in sections {
        if section
            .project
            .is_some_and(|project| collapsed.contains(project))
        {
            continue;
        }
        for note in &section.notes {
            ids.push(note.id.clone());
        }
    }
    ids
}

/// Moves the selection by `delta` within `order`, clamping at the ends.
pub fn move_selection(order: &[String], current: Option<&str>, delta: i32) -> Option<String> {
    if order.is_empty() {
        return None;
    }
    let index = current.and_then(|id| order.iter().position(|row| row == id));
    let target = match index {
        Some(index) => index
            .checked_add_signed(delta as isize)
            .filter(|i| *i < order.len())
            .unwrap_or(index),
        None if delta >= 0 => 0,
        None => order.len() - 1,
    };
    order.get(target).cloned()
}

/// Next selection after collapsing `project`. Keeps `current` if it is not in
/// `project`; otherwise moves to the first visible note after `project`, or
/// the last visible note before it.
pub fn selection_after_collapse(
    sections: &[Section<'_>],
    collapsed: &HashSet<String>,
    project: &str,
    current: Option<&str>,
) -> Option<String> {
    let current = current?;
    let target = sections
        .iter()
        .position(|section| section.project == Some(project))?;
    let in_section = sections[target].notes.iter().any(|note| note.id == current);
    if !in_section {
        return Some(current.to_owned());
    }

    let is_visible = |section: &Section<'_>| {
        !section
            .project
            .is_some_and(|name| name == project || collapsed.contains(name))
    };

    // Prefer the next visible note after the collapsed header.
    if let Some(next) = sections[target + 1..]
        .iter()
        .filter(|section| is_visible(section))
        .find_map(|section| section.notes.first())
    {
        return Some(next.id.clone());
    }

    // Fall back to the last visible note before it.
    sections[..target]
        .iter()
        .rev()
        .filter(|section| is_visible(section))
        .find_map(|section| section.notes.last())
        .map(|note| note.id.clone())
}

#[cfg(test)]
mod tests {
    use posthaste_model::Status;

    use super::*;

    fn note(id: &str, category: Option<&str>, created_at: i64) -> Note {
        Note {
            id: id.into(),
            content: id.into(),
            category: category.map(str::to_owned),
            source: None,
            status: Status::Inbox,
            later_at: None,
            created_at,
            updated_at: created_at,
            completed_at: None,
            archived_at: None,
            has_details: false,
            attachment_count: 0,
            step_total: 0,
            step_done: 0,
        }
    }

    #[test]
    fn group_orders_sections_by_newest_note_and_puts_no_project_last() {
        let notes = vec![
            note("n1", None, 500),
            note("n2", Some("alpha"), 400),
            note("n3", Some("beta"), 300),
            note("n4", Some("alpha"), 200),
            note("n5", None, 100),
        ];
        let sections = group(&notes);
        let names: Vec<Option<&str>> = sections.iter().map(|s| s.project).collect();
        assert_eq!(names, vec![Some("alpha"), Some("beta"), None]);
    }

    #[test]
    fn group_keeps_order_inside_a_section() {
        let notes = vec![
            note("a1", Some("alpha"), 400),
            note("b1", Some("beta"), 300),
            note("a2", Some("alpha"), 200),
            note("a3", Some("alpha"), 100),
        ];
        let sections = group(&notes);
        let alpha_ids: Vec<&str> = sections[0].notes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(alpha_ids, vec!["a1", "a2", "a3"]);
    }

    #[test]
    fn visible_order_skips_collapsed_sections() {
        let notes = vec![
            note("a1", Some("alpha"), 400),
            note("a2", Some("alpha"), 350),
            note("b1", Some("beta"), 300),
            note("u1", None, 200),
        ];
        let sections = group(&notes);
        let collapsed = HashSet::from(["alpha".to_owned()]);
        assert_eq!(visible_order(&sections, &collapsed), vec!["b1", "u1"]);
    }

    #[test]
    fn the_no_project_group_cannot_collapse() {
        let notes = vec![note("a1", Some("alpha"), 300), note("u1", None, 200)];
        let sections = group(&notes);
        let collapsed = HashSet::from([String::new(), "alpha".to_owned()]);
        assert_eq!(visible_order(&sections, &collapsed), vec!["u1"]);
    }

    #[test]
    fn moving_never_lands_on_a_collapsed_note() {
        let notes = vec![
            note("a1", Some("alpha"), 400),
            note("b1", Some("beta"), 300),
            note("b2", Some("beta"), 250),
            note("c1", Some("gamma"), 200),
        ];
        let sections = group(&notes);
        let collapsed = HashSet::from(["beta".to_owned()]);
        let order = visible_order(&sections, &collapsed);
        assert_eq!(move_selection(&order, Some("a1"), 1).as_deref(), Some("c1"));
        assert_eq!(
            move_selection(&order, Some("c1"), -1).as_deref(),
            Some("a1")
        );
    }

    #[test]
    fn collapsing_the_selected_section_moves_selection_to_the_next_visible_note() {
        let notes = vec![
            note("a1", Some("alpha"), 400),
            note("b1", Some("beta"), 300),
            note("b2", Some("beta"), 250),
            note("c1", Some("gamma"), 200),
        ];
        let sections = group(&notes);
        let collapsed = HashSet::new();
        assert_eq!(
            selection_after_collapse(&sections, &collapsed, "beta", Some("b1")).as_deref(),
            Some("c1")
        );
        // When collapsing the last visible section, falls back to the previous visible note.
        assert_eq!(
            selection_after_collapse(&sections, &collapsed, "gamma", Some("c1")).as_deref(),
            Some("b2")
        );
        // Selection outside the collapsed section is untouched.
        assert_eq!(
            selection_after_collapse(&sections, &collapsed, "beta", Some("a1")).as_deref(),
            Some("a1")
        );
    }
}
