//! What the Today window does with a key, which row is selected, and how a
//! note relates to the day being looked at.

use std::collections::HashSet;

use posthaste_model::{Note, Status};

pub use super::selection::{moved_selection, neighbour, selection_after_load};

use super::dates::Day;
use super::sections;

/// What the window is showing when a key arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyContext {
    pub calendar_open: bool,
    /// Looking at today, not at an earlier day. Earlier days are read-only.
    pub viewing_today: bool,
    pub has_selection: bool,
    /// Focus is on a button, which handles its own Enter.
    pub on_button: bool,
}

/// One key press, reduced to what the Today window cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress<'a> {
    /// `KeyboardEvent.key`.
    pub key: &'a str,
    /// ⌘, Ctrl or Alt is held.
    pub modified: bool,
    /// An input method is composing.
    pub composing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    CloseCalendar,
    OpenCalendar,
    /// Show the day this many days away (-1 or 1).
    StepDay(i64),
    ShowToday,
    /// Hide the window.
    Dismiss,
    /// Move the selection this many rows (-1 or 1).
    Move(isize),
    /// Mark the selected note done, or not done.
    Toggle,
    /// Pin or unpin the selected note in the floating focus pill.
    Focus,
    /// Park the selected note in Later.
    SendToLater,
    /// Open the project picker for the selected note.
    SetProject,
    OpenDetails,
}

/// What a key press means. `None` leaves the key alone.
pub fn key_action(press: KeyPress<'_>, context: KeyContext) -> Option<KeyAction> {
    if press.composing || press.modified {
        return None;
    }
    // The calendar handles its own keys; only Esc reaches here, to close it.
    if context.calendar_open {
        return (press.key == "Escape").then_some(KeyAction::CloseCalendar);
    }
    let acts_on_note = context.has_selection && context.viewing_today;
    match press.key {
        "ArrowLeft" => Some(KeyAction::StepDay(-1)),
        "ArrowRight" => Some(KeyAction::StepDay(1)),
        "t" | "T" => Some(KeyAction::ShowToday),
        "c" | "C" => Some(KeyAction::OpenCalendar),
        "Escape" => Some(KeyAction::Dismiss),
        "ArrowDown" | "j" => Some(KeyAction::Move(1)),
        "ArrowUp" | "k" => Some(KeyAction::Move(-1)),
        "x" | "X" if acts_on_note => Some(KeyAction::Toggle),
        "f" | "F" if acts_on_note => Some(KeyAction::Focus),
        "l" | "L" if acts_on_note => Some(KeyAction::SendToLater),
        "p" | "P" if acts_on_note => Some(KeyAction::SetProject),
        "Enter" if context.has_selection && !context.on_button => Some(KeyAction::OpenDetails),
        _ => None,
    }
}

/// Visible note ids across Carried over and Today, each grouped by project
/// separately (so Carried over projects always precede Today projects).
pub fn visible_ids(carried: &[Note], today: &[Note], collapsed: &HashSet<String>) -> Vec<String> {
    let carried_sections = sections::group(carried);
    let today_sections = sections::group(today);
    let mut ids = sections::visible_order(&carried_sections, collapsed);
    ids.extend(sections::visible_order(&today_sections, collapsed));
    ids
}

/// The day to show for a request: `None` (follow today) for today or any
/// later day, because notes have no future.
pub fn clamp_to_past(requested: Option<Day>, today: Day) -> Option<Day> {
    requested.filter(|day| *day < today)
}

/// Whether a note counts as done on the day being viewed. Today that is its
/// status; on an earlier day, whether it was finished before that day ended.
pub fn is_done(note: &Note, viewing_today: bool, day_end_ms: i64) -> bool {
    if viewing_today {
        note.status == Status::Completed
    } else {
        note.completed_at.is_some_and(|at| at < day_end_ms)
    }
}

/// Where a carried note came from, relative to the day being viewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Yesterday,
    /// Captured on `day`. `days_ago` is set only while viewing today:
    /// "days ago" from a past day would be a riddle.
    Earlier {
        day: Day,
        days_ago: Option<i64>,
    },
}

/// `None` when the note was captured on the viewed day (or after it).
pub fn origin(captured: Day, viewed: Day, today: Day) -> Option<Origin> {
    let before = captured.days_until(viewed);
    if before <= 0 {
        return None;
    }
    let viewing_today = viewed == today;
    Some(if viewing_today && before == 1 {
        Origin::Yesterday
    } else {
        Origin::Earlier {
            day: captured,
            days_ago: viewing_today.then_some(before),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: KeyContext = KeyContext {
        calendar_open: false,
        viewing_today: true,
        has_selection: true,
        on_button: false,
    };

    fn press(key: &str) -> KeyPress<'_> {
        KeyPress {
            key,
            modified: false,
            composing: false,
        }
    }

    fn day(key: &str) -> Day {
        Day::parse(key).unwrap()
    }

    #[test]
    fn keys_move_the_selection_browse_days_and_act_on_the_note() {
        let expect = [
            ("ArrowDown", KeyAction::Move(1)),
            ("j", KeyAction::Move(1)),
            ("ArrowUp", KeyAction::Move(-1)),
            ("k", KeyAction::Move(-1)),
            ("ArrowLeft", KeyAction::StepDay(-1)),
            ("ArrowRight", KeyAction::StepDay(1)),
            ("t", KeyAction::ShowToday),
            ("C", KeyAction::OpenCalendar),
            ("x", KeyAction::Toggle),
            ("L", KeyAction::SendToLater),
            ("Enter", KeyAction::OpenDetails),
            ("Escape", KeyAction::Dismiss),
        ];
        for (key, action) in expect {
            assert_eq!(key_action(press(key), TODAY), Some(action), "{key}");
        }
        assert_eq!(key_action(press("q"), TODAY), None);
    }

    #[test]
    fn nothing_fires_with_a_modifier_or_while_composing() {
        for key in ["l", "x", "Enter", "Escape", "ArrowLeft", "t"] {
            let modified = KeyPress {
                modified: true,
                ..press(key)
            };
            let composing = KeyPress {
                composing: true,
                ..press(key)
            };
            assert_eq!(key_action(modified, TODAY), None, "{key} with a modifier");
            assert_eq!(key_action(composing, TODAY), None, "{key} while composing");
        }
    }

    #[test]
    fn a_past_day_is_read_only_but_still_opens_details() {
        let past = KeyContext {
            viewing_today: false,
            ..TODAY
        };
        assert_eq!(key_action(press("x"), past), None);
        assert_eq!(key_action(press("l"), past), None);
        assert_eq!(
            key_action(press("Enter"), past),
            Some(KeyAction::OpenDetails)
        );
        assert_eq!(key_action(press("t"), past), Some(KeyAction::ShowToday));
    }

    #[test]
    fn with_the_calendar_open_only_escape_gets_through_and_closes_it() {
        let open = KeyContext {
            calendar_open: true,
            ..TODAY
        };
        assert_eq!(
            key_action(press("Escape"), open),
            Some(KeyAction::CloseCalendar)
        );
        for key in ["x", "ArrowLeft", "Enter", "t", "j"] {
            assert_eq!(key_action(press(key), open), None, "{key}");
        }
    }

    #[test]
    fn note_keys_need_a_selection_and_enter_is_left_to_a_focused_button() {
        let empty = KeyContext {
            has_selection: false,
            ..TODAY
        };
        for key in ["x", "l", "Enter"] {
            assert_eq!(key_action(press(key), empty), None, "{key}");
        }
        assert_eq!(key_action(press("Escape"), empty), Some(KeyAction::Dismiss));
        let on_button = KeyContext {
            on_button: true,
            ..TODAY
        };
        assert_eq!(key_action(press("Enter"), on_button), None);
    }

    #[test]
    fn days_after_today_are_refused() {
        let today = day("2026-11-16");
        assert_eq!(
            clamp_to_past(Some(day("2026-11-15")), today),
            Some(day("2026-11-15"))
        );
        assert_eq!(clamp_to_past(Some(today), today), None);
        assert_eq!(clamp_to_past(Some(day("2026-11-17")), today), None);
        assert_eq!(clamp_to_past(None, today), None);
    }

    #[test]
    fn a_refetch_keeps_the_selection_unless_reset_or_gone() {
        let ids = ["a", "b", "c"];
        assert_eq!(
            selection_after_load(&ids, Some("b"), false).as_deref(),
            Some("b")
        );
        assert_eq!(
            selection_after_load(&ids, Some("b"), true).as_deref(),
            Some("a")
        );
        assert_eq!(
            selection_after_load(&ids, Some("gone"), false).as_deref(),
            Some("a")
        );
        assert_eq!(
            selection_after_load(&ids, None, false).as_deref(),
            Some("a")
        );
        assert_eq!(selection_after_load(&[], Some("b"), false), None);
    }

    #[test]
    fn the_selection_moves_one_row_and_stops_at_the_ends() {
        let ids = ["a", "b", "c"];
        assert_eq!(moved_selection(&ids, Some("a"), 1).as_deref(), Some("b"));
        assert_eq!(moved_selection(&ids, Some("c"), 1).as_deref(), Some("c"));
        assert_eq!(moved_selection(&ids, Some("a"), -1).as_deref(), Some("a"));
        assert_eq!(moved_selection(&ids, None, 1).as_deref(), Some("a"));
        assert_eq!(moved_selection(&[], None, 1), None);
    }

    #[test]
    fn a_note_leaving_the_list_hands_the_selection_to_a_neighbour() {
        let ids = ["a", "b", "c"];
        assert_eq!(neighbour(&ids, "a").as_deref(), Some("b"));
        assert_eq!(neighbour(&ids, "c").as_deref(), Some("b"));
        assert_eq!(neighbour(&["only"], "only"), None);
        assert_eq!(neighbour(&ids, "gone"), None);
    }

    fn note(status: Status, completed_at: Option<i64>) -> Note {
        Note {
            id: "n".into(),
            content: String::new(),
            status,
            category: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            completed_at,
            archived_at: None,
            has_details: false,
            attachment_count: 0,
            step_total: 0,
            step_done: 0,
            later_at: None,
        }
    }

    #[test]
    fn on_a_past_day_a_note_finished_later_is_not_done() {
        let finished_later = note(Status::Completed, Some(2_000));
        assert!(is_done(&finished_later, true, 1_000));
        assert!(!is_done(&finished_later, false, 1_000));
        assert!(is_done(&finished_later, false, 3_000));
        assert!(!is_done(&note(Status::Inbox, None), false, 3_000));
    }

    #[test]
    fn a_carried_note_says_where_it_came_from() {
        let today = day("2026-11-16");
        assert_eq!(origin(today, today, today), None);
        assert_eq!(
            origin(day("2026-11-15"), today, today),
            Some(Origin::Yesterday)
        );
        assert_eq!(
            origin(day("2026-11-13"), today, today),
            Some(Origin::Earlier {
                day: day("2026-11-13"),
                days_ago: Some(3)
            })
        );
        // Viewing a past day: the bare date, never "yesterday" or "days ago".
        let viewed = day("2026-11-10");
        assert_eq!(
            origin(day("2026-11-09"), viewed, today),
            Some(Origin::Earlier {
                day: day("2026-11-09"),
                days_ago: None
            })
        );
        assert_eq!(origin(day("2026-11-12"), viewed, today), None);
    }

    #[test]
    fn p_in_today_sets_the_selected_notes_project() {
        assert_eq!(key_action(press("p"), TODAY), Some(KeyAction::SetProject));
        assert_eq!(key_action(press("P"), TODAY), Some(KeyAction::SetProject));
        let past = KeyContext {
            viewing_today: false,
            ..TODAY
        };
        assert_eq!(key_action(press("p"), past), None);
        let empty = KeyContext {
            has_selection: false,
            ..TODAY
        };
        assert_eq!(key_action(press("p"), empty), None);
    }

    #[test]
    fn today_groups_carried_and_today_by_project_separately() {
        let mut c1 = note(Status::Inbox, None);
        c1.id = "c-work".into();
        c1.category = Some("work".into());
        let mut c2 = note(Status::Inbox, None);
        c2.id = "c-plain".into();
        let mut t1 = note(Status::Inbox, None);
        t1.id = "t-work".into();
        t1.category = Some("work".into());
        let mut t2 = note(Status::Inbox, None);
        t2.id = "t-home".into();
        t2.category = Some("home".into());

        let ids = visible_ids(
            &[c2.clone(), c1.clone()],
            &[t2.clone(), t1.clone()],
            &HashSet::new(),
        );
        // Carried over (work, then plain) comes before Today (home, then work).
        assert_eq!(ids, vec!["c-work", "c-plain", "t-home", "t-work"]);

        let collapsed = HashSet::from(["work".to_owned()]);
        assert_eq!(
            visible_ids(&[c2, c1], &[t2, t1], &collapsed),
            vec!["c-plain", "t-home"]
        );
    }
}
