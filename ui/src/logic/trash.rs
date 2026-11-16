//! Decisions of the Trash section.

use posthaste_model::{Note, Status};

/// The section a restored note goes back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Returns {
    Inbox,
    Later,
    /// Completed notes are listed in Archive, the same as archived ones.
    Archive,
}

pub fn returns_to(note: &Note) -> Returns {
    if note.status != Status::Inbox {
        Returns::Archive
    } else if note.later_at.is_some() {
        Returns::Later
    } else {
        Returns::Inbox
    }
}

/// The note's first line, shortened to `max` characters, to name it in a
/// confirmation.
pub fn excerpt(content: &str, max: usize) -> String {
    let line = content.lines().next().unwrap_or_default().trim();
    if line.chars().count() <= max {
        return line.to_owned();
    }
    let kept: String = line.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Move(isize),
    Restore,
    /// Asks first: erasing cannot be undone.
    ConfirmErase,
}

#[derive(Debug, Clone, Copy)]
pub struct KeyPress<'a> {
    pub key: &'a str,
    /// Ctrl, Alt or the command key is held.
    pub modified: bool,
    pub composing: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct KeyContext {
    /// A dialog is open, or the key was typed into a field.
    pub busy: bool,
    pub has_selection: bool,
    /// Focus is on a button, which handles its own Enter.
    pub on_button: bool,
}

pub fn key_action(press: KeyPress<'_>, context: KeyContext) -> Option<KeyAction> {
    if press.composing || press.modified || context.busy {
        return None;
    }
    match press.key {
        "ArrowDown" | "j" => return Some(KeyAction::Move(1)),
        "ArrowUp" | "k" => return Some(KeyAction::Move(-1)),
        _ => {}
    }
    if !context.has_selection {
        return None;
    }
    match press.key {
        "r" | "R" => Some(KeyAction::Restore),
        "Enter" if !context.on_button => Some(KeyAction::Restore),
        "Delete" | "Backspace" => Some(KeyAction::ConfirmErase),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(status: Status, later_at: Option<i64>) -> Note {
        Note {
            id: "n".into(),
            content: String::new(),
            status,
            category: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            completed_at: None,
            archived_at: None,
            has_details: false,
            attachment_count: 0,
            later_at,
        }
    }

    #[test]
    fn a_restored_note_returns_to_the_section_it_left() {
        assert_eq!(returns_to(&note(Status::Inbox, None)), Returns::Inbox);
        assert_eq!(returns_to(&note(Status::Inbox, Some(1))), Returns::Later);
        assert_eq!(returns_to(&note(Status::Completed, None)), Returns::Archive);
        assert_eq!(returns_to(&note(Status::Archived, None)), Returns::Archive);
    }

    #[test]
    fn the_excerpt_is_the_first_line_cut_to_fit() {
        assert_eq!(excerpt("  buy milk  \nand eggs", 80), "buy milk");
        assert_eq!(excerpt("", 80), "");
        assert_eq!(excerpt("abcdef", 4), "abc…");
        assert_eq!(excerpt("ab cdef", 4), "ab…");
        // Counted in characters, so a cut never lands inside one.
        assert_eq!(excerpt("việc cần làm", 5), "việc…");
    }

    #[test]
    fn keys() {
        let press = |key| KeyPress {
            key,
            modified: false,
            composing: false,
        };
        let on = |has_selection, on_button| KeyContext {
            busy: false,
            has_selection,
            on_button,
        };
        assert_eq!(
            key_action(press("j"), on(false, false)),
            Some(KeyAction::Move(1))
        );
        assert_eq!(
            key_action(press("ArrowUp"), on(true, false)),
            Some(KeyAction::Move(-1))
        );
        assert_eq!(
            key_action(press("r"), on(true, false)),
            Some(KeyAction::Restore)
        );
        assert_eq!(key_action(press("r"), on(false, false)), None);
        assert_eq!(
            key_action(press("Enter"), on(true, false)),
            Some(KeyAction::Restore)
        );
        assert_eq!(key_action(press("Enter"), on(true, true)), None);
        assert_eq!(
            key_action(press("Delete"), on(true, false)),
            Some(KeyAction::ConfirmErase)
        );
        let busy = KeyContext {
            busy: true,
            ..on(true, false)
        };
        assert_eq!(key_action(press("Delete"), busy), None);
        let modified = KeyPress {
            modified: true,
            ..press("r")
        };
        assert_eq!(key_action(modified, on(true, false)), None);
    }
}
