//! Decisions of the Note details window.

use posthaste_model::{Note, Status};

use super::dates::Day;

/// What the button in the header says about the note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    Done,
    Archived,
    Later,
    Inbox,
}

pub fn badge(note: &Note) -> Badge {
    match note.status {
        Status::Completed => Badge::Done,
        Status::Archived => Badge::Archived,
        Status::Inbox if note.later_at.is_some() => Badge::Later,
        Status::Inbox => Badge::Inbox,
    }
}

/// How many days an open note has been carried to the next one. Zero for a
/// note that is done, archived or parked: nothing carries those.
pub fn carried_days(note: &Note, captured: Day, today: Day) -> u64 {
    if note.status != Status::Inbox || note.later_at.is_some() {
        return 0;
    }
    u64::try_from(captured.days_until(today)).unwrap_or(0)
}

/// Whether the note was changed long enough after it was captured to say so.
pub fn was_edited(note: &Note) -> bool {
    note.updated_at - note.created_at > 60_000
}

/// What a field shows after the note was fetched again. The stored text
/// replaces the field only when the user has nothing unsaved in it: when the
/// window opened on this note (`fresh`), or the field still shows what was
/// last stored.
pub fn field_after_load(current: &str, last_stored: &str, stored: &str, fresh: bool) -> String {
    if fresh || current == last_stored {
        stored.to_owned()
    } else {
        current.to_owned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Close,
    ToggleDone,
    AddFiles,
}

/// One key press, reduced to what the decision needs.
#[derive(Debug, Clone, Copy)]
pub struct KeyPress<'a> {
    pub key: &'a str,
    /// The platform's primary modifier is held.
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
    /// An input method is composing: the key belongs to it.
    pub composing: bool,
}

pub fn key_action(press: KeyPress<'_>) -> Option<KeyAction> {
    if press.composing {
        return None;
    }
    match press.key {
        "Escape" => Some(KeyAction::Close),
        "Enter" if press.primary => Some(KeyAction::ToggleDone),
        "o" | "O" if press.primary && !press.shift && !press.alt => Some(KeyAction::AddFiles),
        _ => None,
    }
}

/// Categories to offer while editing: a prefix match, case-insensitive, in
/// the order given, at most eight.
pub fn category_suggestions(all: &[String], typed: &str) -> Vec<String> {
    let prefix = typed.trim().to_lowercase();
    all.iter()
        .filter(|c| prefix.is_empty() || c.to_lowercase().starts_with(&prefix))
        .take(8)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(status: Status, later_at: Option<i64>) -> Note {
        Note {
            id: "n".into(),
            content: "text".into(),
            status,
            category: None,
            source: None,
            created_at: 1_000,
            updated_at: 1_000,
            completed_at: None,
            archived_at: None,
            has_details: false,
            attachment_count: 0,
            later_at,
        }
    }

    fn day(day: u8) -> Day {
        Day {
            year: 2026,
            month: 11,
            day,
        }
    }

    #[test]
    fn the_badge_names_where_the_note_is() {
        assert_eq!(badge(&note(Status::Completed, None)), Badge::Done);
        assert_eq!(badge(&note(Status::Archived, None)), Badge::Archived);
        assert_eq!(badge(&note(Status::Inbox, Some(5))), Badge::Later);
        assert_eq!(badge(&note(Status::Inbox, None)), Badge::Inbox);
    }

    #[test]
    fn only_an_open_unparked_note_is_carried() {
        assert_eq!(
            carried_days(&note(Status::Inbox, None), day(10), day(13)),
            3
        );
        assert_eq!(
            carried_days(&note(Status::Inbox, None), day(13), day(13)),
            0
        );
        assert_eq!(
            carried_days(&note(Status::Inbox, Some(1)), day(10), day(13)),
            0
        );
        assert_eq!(
            carried_days(&note(Status::Completed, None), day(10), day(13)),
            0
        );
        // A clock set back must not show a negative count.
        assert_eq!(
            carried_days(&note(Status::Inbox, None), day(14), day(13)),
            0
        );
    }

    #[test]
    fn an_edit_shows_after_a_minute() {
        let mut n = note(Status::Inbox, None);
        n.updated_at = n.created_at + 60_000;
        assert!(!was_edited(&n));
        n.updated_at += 1;
        assert!(was_edited(&n));
    }

    #[test]
    fn a_refetch_never_replaces_unsaved_typing() {
        // Nothing unsaved: the stored text wins.
        assert_eq!(field_after_load("old", "old", "new", false), "new");
        // The user typed since the last save: their text stays.
        assert_eq!(field_after_load("typed", "old", "new", false), "typed");
        // Opening a note always shows that note.
        assert_eq!(field_after_load("typed", "old", "new", true), "new");
    }

    #[test]
    fn keys() {
        let press = |key, primary| KeyPress {
            key,
            primary,
            shift: false,
            alt: false,
            composing: false,
        };
        assert_eq!(key_action(press("Escape", false)), Some(KeyAction::Close));
        assert_eq!(
            key_action(press("Enter", true)),
            Some(KeyAction::ToggleDone)
        );
        assert_eq!(key_action(press("Enter", false)), None);
        assert_eq!(key_action(press("o", true)), Some(KeyAction::AddFiles));
        assert_eq!(key_action(press("O", true)), Some(KeyAction::AddFiles));
        assert_eq!(key_action(press("o", false)), None);
        let shifted = KeyPress {
            shift: true,
            ..press("o", true)
        };
        assert_eq!(key_action(shifted), None);
        let composing = KeyPress {
            composing: true,
            ..press("Escape", false)
        };
        assert_eq!(key_action(composing), None);
    }

    #[test]
    fn category_suggestions_filter_by_prefix_and_keep_use_order() {
        let all = ["work".to_owned(), "ideas".to_owned(), "Workshop".to_owned()];
        assert_eq!(category_suggestions(&all, "w"), ["work", "Workshop"]);
        assert_eq!(category_suggestions(&all, "W"), ["work", "Workshop"]);
        assert_eq!(category_suggestions(&all, "wor"), ["work", "Workshop"]);
        assert_eq!(category_suggestions(&all, "ideas"), ["ideas"]);
        assert!(category_suggestions(&all, "zzz").is_empty());
        // Nothing typed: every category, in the order given.
        assert_eq!(
            category_suggestions(&all, ""),
            ["work", "ideas", "Workshop"]
        );
        assert_eq!(
            category_suggestions(&all, "  "),
            ["work", "ideas", "Workshop"]
        );
    }

    #[test]
    fn category_suggestions_are_capped_at_eight() {
        let all: Vec<String> = (0..12).map(|i| format!("cat{i}")).collect();
        assert_eq!(category_suggestions(&all, "cat").len(), 8);
        assert_eq!(category_suggestions(&all[9..], "cat").len(), 3);
    }
}
