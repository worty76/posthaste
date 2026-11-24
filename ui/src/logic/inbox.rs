//! Decisions of the main window: which notes a section asks for, how they
//! are grouped, and what each key does.

use posthaste_model::{Note, NoteQuery, Status};

use super::dates::Day;

/// A section of the main window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    Inbox,
    Today,
    Later,
    Archive,
    Trash,
    Settings,
    Project(String),
    ManageProjects,
}

impl View {
    /// The sections that list notes, in sidebar order. Their place here is
    /// also the digit key that opens them.
    pub const LISTS: [View; 5] = [
        View::Inbox,
        View::Today,
        View::Later,
        View::Archive,
        View::Trash,
    ];

    /// The name the shell uses in its `navigate` event.
    pub fn parse(name: &str) -> Option<View> {
        Some(match name {
            "inbox" => View::Inbox,
            "today" => View::Today,
            "later" => View::Later,
            "archive" => View::Archive,
            "trash" => View::Trash,
            "settings" => View::Settings,
            _ => return None,
        })
    }
}

/// The status control of a search.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatusFilter {
    #[default]
    All,
    Open,
    Completed,
    Archived,
}

/// What the search box and the filter controls hold.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    pub text: String,
    pub status: StatusFilter,
    /// The day notes were captured on, when one is picked.
    pub date: Option<Day>,
}

impl Filters {
    /// A search is on when there is text or a day. Status alone narrows;
    /// it does not start a search.
    pub fn is_searching(&self) -> bool {
        !self.text.trim().is_empty() || self.date.is_some()
    }
}

/// What to ask the database for. Outside a search each section has a fixed
/// scope; a search looks across every note, narrowed by the filters.
/// `today` and `date` are the `[from, to)` instants of those days.
pub fn build_query(
    view: View,
    filters: &Filters,
    today: (i64, i64),
    date: Option<(i64, i64)>,
) -> NoteQuery {
    if filters.is_searching() {
        let statuses = match filters.status {
            StatusFilter::All => Vec::new(),
            StatusFilter::Open => vec![Status::Inbox],
            StatusFilter::Completed => vec![Status::Completed],
            StatusFilter::Archived => vec![Status::Archived],
        };
        return NoteQuery {
            text: filters.text.trim().to_owned(),
            statuses,
            created_from: date.map(|(from, _)| from),
            created_to: date.map(|(_, to)| to),
            ..NoteQuery::default()
        };
    }
    match view {
        View::Today => NoteQuery {
            created_from: Some(today.0),
            created_to: Some(today.1),
            later: Some(false),
            ..NoteQuery::default()
        },
        View::Later => NoteQuery {
            statuses: vec![Status::Inbox],
            later: Some(true),
            ..NoteQuery::default()
        },
        View::Archive => NoteQuery {
            statuses: vec![Status::Completed, Status::Archived],
            ..NoteQuery::default()
        },
        View::Inbox | View::Trash | View::Settings | View::ManageProjects => NoteQuery {
            statuses: vec![Status::Inbox],
            later: Some(false),
            ..NoteQuery::default()
        },
        View::Project(name) => NoteQuery {
            category: Some(name),
            statuses: Vec::new(),
            later: None,
            ..NoteQuery::default()
        },
    }
}

/// Day bucket label for a project page's notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayLabel {
    Today,
    Yesterday,
    Earlier(i64),
}

const DAY_MS: i64 = 86_400_000;

/// Groups a project's notes by local day (newest day first), placing open
/// notes before finished notes within each day (each newest first).
pub fn group_by_day(notes: &[Note], today: (i64, i64)) -> Vec<(DayLabel, Vec<&Note>)> {
    let mut by_offset: Vec<(i64, Vec<&Note>)> = Vec::new();
    for note in notes {
        let offset = (note.created_at - today.0).div_euclid(DAY_MS);
        if let Some((_, list)) = by_offset.iter_mut().find(|(day, _)| *day == offset) {
            list.push(note);
        } else {
            by_offset.push((offset, vec![note]));
        }
    }
    by_offset.sort_by_key(|a| std::cmp::Reverse(a.0));
    by_offset
        .into_iter()
        .map(|(offset, mut list)| {
            list.sort_by(|a, b| {
                let a_done = a.status != Status::Inbox;
                let b_done = b.status != Status::Inbox;
                a_done
                    .cmp(&b_done)
                    .then_with(|| b.created_at.cmp(&a.created_at))
            });
            let label = match offset {
                d if d >= 0 => DayLabel::Today,
                -1 => DayLabel::Yesterday,
                d => DayLabel::Earlier(today.0 + d * DAY_MS),
            };
            (label, list)
        })
        .collect()
}

/// Splits items into runs that share a key, keeping their order. Notes come
/// newest first, so grouping by day gives one group per day.
pub fn group_runs<T, K: PartialEq>(
    items: impl IntoIterator<Item = T>,
    key_of: impl Fn(&T) -> K,
) -> Vec<(K, Vec<T>)> {
    let mut groups: Vec<(K, Vec<T>)> = Vec::new();
    for item in items {
        let key = key_of(&item);
        match groups.last_mut() {
            Some((last, run)) if *last == key => run.push(item),
            _ => groups.push((key, vec![item])),
        }
    }
    groups
}

/// Whether a note is long enough to be shown clamped, with "Show more".
pub fn is_long(content: &str) -> bool {
    const LINES: usize = 5;
    const CHARS: usize = 420;
    content.chars().count() > CHARS || content.lines().count() > LINES
}

/// Why a list has nothing in it, which decides what it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Empty {
    /// No note contains the text. `filtered`: a day or a status narrows it.
    NoMatch {
        filtered: bool,
    },
    /// No note was captured on the picked day.
    NothingThatDay,
    /// A project page with no notes yet.
    Project,
    Today,
    Later,
    Archive,
    Inbox,
}

pub fn empty_reason(view: &View, filters: &Filters) -> Empty {
    if filters.is_searching() {
        return if filters.text.trim().is_empty() {
            Empty::NothingThatDay
        } else {
            Empty::NoMatch {
                filtered: filters.date.is_some() || filters.status != StatusFilter::All,
            }
        };
    }
    match view {
        View::Today => Empty::Today,
        View::Later => Empty::Later,
        View::Archive => Empty::Archive,
        View::Project(_) => Empty::Project,
        View::Inbox | View::Trash | View::Settings | View::ManageProjects => Empty::Inbox,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Go(View),
    FocusSearch,
    NewNote,
    ToggleSidebar,
    Help,
    /// Esc in the search box: end the search and leave the box.
    LeaveSearch,
    /// ↓ in the search box: on to the first result.
    SearchToList,
    Move(isize),
    ClearSearch,
    /// `#`: open the project picker in jump mode.
    PickCategory,
    /// `P`: set the selected note's project.
    SetProject,
    /// `M` on a project page: open the project menu.
    ProjectMenu,
    ClearSelection,
    Edit,
    ToggleComplete,
    ToggleFocus,
    Archive,
    Restore,
    ToggleLater,
    Copy,
    OpenDetails,
    Delete,
}

/// What a key does, and whether the selection then moves to the next row
/// because the note it was on is about to leave the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyed {
    pub action: KeyAction,
    pub advance: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct KeyPress<'a> {
    pub key: &'a str,
    /// The platform's primary modifier is held.
    pub primary: bool,
    pub alt: bool,
    pub composing: bool,
}

#[derive(Debug, Clone)]
pub struct KeyContext {
    pub view: View,
    pub searching: bool,
    pub dialog_open: bool,
    /// The key was typed into a field.
    pub typing: bool,
    /// That field is the search box.
    pub in_search: bool,
    /// Focus is on a button or link, which handles its own Enter and Space.
    pub on_control: bool,
    pub has_rows: bool,
    /// The status of the selected note, when one is selected.
    pub selected: Option<Status>,
}

pub fn key_action(press: KeyPress<'_>, context: &KeyContext) -> Option<Keyed> {
    let plain = |action| {
        Some(Keyed {
            action,
            advance: false,
        })
    };
    if context.dialog_open || press.composing {
        return None;
    }
    if press.primary {
        return match press.key {
            "," => plain(KeyAction::Go(View::Settings)),
            "f" | "F" => plain(KeyAction::FocusSearch),
            "n" | "N" => plain(KeyAction::NewNote),
            "b" | "B" => plain(KeyAction::ToggleSidebar),
            _ => None,
        };
    }
    if context.typing {
        return match press.key {
            "Escape" if context.in_search => plain(KeyAction::LeaveSearch),
            "ArrowDown" if context.in_search && context.has_rows => plain(KeyAction::SearchToList),
            _ => None,
        };
    }
    if press.alt {
        return None;
    }
    match press.key {
        "1" => return plain(KeyAction::Go(View::Inbox)),
        "2" => return plain(KeyAction::Go(View::Today)),
        "3" => return plain(KeyAction::Go(View::Later)),
        "4" => return plain(KeyAction::Go(View::Archive)),
        "5" => return plain(KeyAction::Go(View::Trash)),
        "?" => return plain(KeyAction::Help),
        "/" => return plain(KeyAction::FocusSearch),
        "n" => return plain(KeyAction::NewNote),
        "#" => return plain(KeyAction::PickCategory),
        _ => {}
    }
    // Settings and ManageProjects have no note list, and Trash handles its own.
    let lists_notes = match context.view {
        View::Settings | View::ManageProjects => false,
        View::Trash => context.searching,
        _ => true,
    };
    if !lists_notes {
        return None;
    }
    match press.key {
        "ArrowDown" | "j" => return plain(KeyAction::Move(1)),
        "ArrowUp" | "k" => return plain(KeyAction::Move(-1)),
        "m" | "M" if matches!(context.view, View::Project(_)) && !context.searching => {
            return plain(KeyAction::ProjectMenu)
        }
        "Escape" if context.searching => return plain(KeyAction::ClearSearch),
        "Escape" => return plain(KeyAction::ClearSelection),
        _ => {}
    }
    let status = context.selected?;
    // The note leaves the list only when the list is a section, not a search.
    let leaves = |from: View| !context.searching && context.view == from;
    let keyed = |action, advance| Some(Keyed { action, advance });
    match press.key {
        "Enter" if context.on_control => None,
        "Enter" | "e" => plain(KeyAction::Edit),
        " " if context.on_control => None,
        " " | "x" => keyed(KeyAction::ToggleComplete, leaves(View::Inbox)),
        "f" | "F" if status == Status::Inbox => plain(KeyAction::ToggleFocus),
        "a" if status != Status::Archived => keyed(KeyAction::Archive, leaves(View::Inbox)),
        "r" if status != Status::Inbox => keyed(KeyAction::Restore, leaves(View::Archive)),
        "l" => keyed(
            KeyAction::ToggleLater,
            !context.searching && !matches!(context.view, View::Archive | View::Project(_)),
        ),
        "c" => plain(KeyAction::Copy),
        "o" => plain(KeyAction::OpenDetails),
        "p" | "P" => plain(KeyAction::SetProject),
        "Delete" | "Backspace" => keyed(KeyAction::Delete, true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: (i64, i64) = (1_000, 2_000);

    fn searching(text: &str) -> Filters {
        Filters {
            text: text.into(),
            ..Filters::default()
        }
    }

    #[test]
    fn each_section_asks_for_its_own_notes() {
        let none = Filters::default();
        let inbox = build_query(View::Inbox, &none, TODAY, None);
        assert_eq!(inbox.statuses, [Status::Inbox]);
        assert_eq!(inbox.later, Some(false));

        let today = build_query(View::Today, &none, TODAY, None);
        assert_eq!(
            (today.created_from, today.created_to),
            (Some(1_000), Some(2_000))
        );
        assert_eq!(today.later, Some(false));
        assert!(today.statuses.is_empty());

        let later = build_query(View::Later, &none, TODAY, None);
        assert_eq!(later.statuses, [Status::Inbox]);
        assert_eq!(later.later, Some(true));

        let archive = build_query(View::Archive, &none, TODAY, None);
        assert_eq!(archive.statuses, [Status::Completed, Status::Archived]);
        assert_eq!(archive.later, None);
    }

    #[test]
    fn a_search_looks_through_every_note() {
        let query = build_query(View::Archive, &searching("  milk "), TODAY, None);
        assert_eq!(query.text, "milk");
        assert!(query.statuses.is_empty());
        assert_eq!(query.later, None);
        assert_eq!(query.created_from, None);

        let narrowed = Filters {
            status: StatusFilter::Open,
            ..searching("milk")
        };
        let query = build_query(View::Inbox, &narrowed, TODAY, Some((50, 60)));
        assert_eq!(query.statuses, [Status::Inbox]);
        assert_eq!((query.created_from, query.created_to), (Some(50), Some(60)));
    }

    #[test]
    fn a_status_alone_does_not_start_a_search() {
        let only_status = Filters {
            status: StatusFilter::Completed,
            ..Filters::default()
        };
        assert!(!only_status.is_searching());
        assert!(!searching("   ").is_searching());
        assert!(searching("a").is_searching());
        let by_day = Filters {
            date: Day::new(2026, 11, 15),
            ..Filters::default()
        };
        assert!(by_day.is_searching());
    }

    #[test]
    fn notes_group_into_runs_in_order() {
        let groups = group_runs([1, 1, 2, 2, 2, 1], |n| *n);
        assert_eq!(groups, [(1, vec![1, 1]), (2, vec![2, 2, 2]), (1, vec![1])]);
        assert!(group_runs(Vec::<i32>::new(), |n| *n).is_empty());
    }

    #[test]
    fn long_notes_are_clamped() {
        assert!(!is_long("short"));
        assert!(!is_long("1\n2\n3\n4\n5"));
        assert!(is_long("1\n2\n3\n4\n5\n6"));
        assert!(is_long(&"x".repeat(421)));
        assert!(!is_long(&"x".repeat(420)));
    }

    #[test]
    fn an_empty_list_says_why() {
        let none = Filters::default();
        assert_eq!(empty_reason(&View::Inbox, &none), Empty::Inbox);
        assert_eq!(empty_reason(&View::Today, &none), Empty::Today);
        assert_eq!(empty_reason(&View::Later, &none), Empty::Later);
        assert_eq!(empty_reason(&View::Archive, &none), Empty::Archive);
        assert_eq!(
            empty_reason(&View::Project("work".into()), &none),
            Empty::Project
        );
        assert_eq!(
            empty_reason(&View::Inbox, &searching("x")),
            Empty::NoMatch { filtered: false }
        );
        let by_day = Filters {
            date: Day::new(2026, 11, 15),
            ..Filters::default()
        };
        assert_eq!(empty_reason(&View::Inbox, &by_day), Empty::NothingThatDay);
        let both = Filters {
            date: Day::new(2026, 11, 15),
            ..searching("x")
        };
        assert_eq!(
            empty_reason(&View::Inbox, &both),
            Empty::NoMatch { filtered: true }
        );
    }

    fn press(key: &str) -> KeyPress<'_> {
        KeyPress {
            key,
            primary: false,
            alt: false,
            composing: false,
        }
    }

    fn on(view: View, selected: Option<Status>) -> KeyContext {
        KeyContext {
            view,
            searching: false,
            dialog_open: false,
            typing: false,
            in_search: false,
            on_control: false,
            has_rows: true,
            selected,
        }
    }

    fn action(press: KeyPress<'_>, context: &KeyContext) -> Option<KeyAction> {
        key_action(press, context).map(|keyed| keyed.action)
    }

    #[test]
    fn keys_work_from_anywhere_with_the_primary_modifier() {
        let typing = KeyContext {
            typing: true,
            ..on(View::Inbox, None)
        };
        let primary = |key| KeyPress {
            primary: true,
            ..press(key)
        };
        assert_eq!(
            action(primary(","), &typing),
            Some(KeyAction::Go(View::Settings))
        );
        assert_eq!(action(primary("f"), &typing), Some(KeyAction::FocusSearch));
        assert_eq!(action(primary("n"), &typing), Some(KeyAction::NewNote));
        assert_eq!(
            action(primary("b"), &typing),
            Some(KeyAction::ToggleSidebar)
        );
        assert_eq!(
            action(primary("B"), &typing),
            Some(KeyAction::ToggleSidebar)
        );
        assert_eq!(
            action(primary("x"), &on(View::Inbox, Some(Status::Inbox))),
            None
        );
    }

    #[test]
    fn typing_in_a_field_is_left_alone() {
        let field = KeyContext {
            typing: true,
            ..on(View::Inbox, Some(Status::Inbox))
        };
        for key in ["x", "a", "1", "Delete", "Escape", "ArrowDown", "?"] {
            assert_eq!(action(press(key), &field), None, "{key}");
        }
        let search = KeyContext {
            in_search: true,
            ..field.clone()
        };
        assert_eq!(
            action(press("Escape"), &search),
            Some(KeyAction::LeaveSearch)
        );
        assert_eq!(
            action(press("ArrowDown"), &search),
            Some(KeyAction::SearchToList)
        );
        let nothing_found = KeyContext {
            has_rows: false,
            ..search.clone()
        };
        assert_eq!(action(press("ArrowDown"), &nothing_found), None);
        let composing = KeyPress {
            composing: true,
            ..press("Escape")
        };
        assert_eq!(action(composing, &search), None);
    }

    #[test]
    fn digits_open_sections_everywhere() {
        let settings = on(View::Settings, None);
        assert_eq!(
            action(press("1"), &settings),
            Some(KeyAction::Go(View::Inbox))
        );
        assert_eq!(
            action(press("5"), &settings),
            Some(KeyAction::Go(View::Trash))
        );
        assert_eq!(action(press("?"), &settings), Some(KeyAction::Help));
        assert_eq!(action(press("/"), &settings), Some(KeyAction::FocusSearch));
        assert_eq!(action(press("n"), &settings), Some(KeyAction::NewNote));
        // No list there: nothing moves.
        assert_eq!(action(press("j"), &settings), None);
        let alt = KeyPress {
            alt: true,
            ..press("1")
        };
        assert_eq!(action(alt, &settings), None);
    }

    #[test]
    fn trash_keeps_its_own_keys_unless_searching() {
        let trash = on(View::Trash, Some(Status::Inbox));
        assert_eq!(action(press("j"), &trash), None);
        assert_eq!(action(press("Delete"), &trash), None);
        let searching = KeyContext {
            searching: true,
            ..trash
        };
        assert_eq!(action(press("j"), &searching), Some(KeyAction::Move(1)));
    }

    #[test]
    fn list_keys() {
        let inbox = on(View::Inbox, None);
        assert_eq!(action(press("ArrowDown"), &inbox), Some(KeyAction::Move(1)));
        assert_eq!(action(press("k"), &inbox), Some(KeyAction::Move(-1)));
        assert_eq!(
            action(press("Escape"), &inbox),
            Some(KeyAction::ClearSelection)
        );
        let searching = KeyContext {
            searching: true,
            ..inbox.clone()
        };
        assert_eq!(
            action(press("Escape"), &searching),
            Some(KeyAction::ClearSearch)
        );
        // Note keys need a note.
        for key in ["x", "e", "a", "l", "c", "o", "Delete", "Enter"] {
            assert_eq!(action(press(key), &inbox), None, "{key}");
        }
    }

    #[test]
    fn note_keys_and_when_the_selection_moves_on() {
        let open = on(View::Inbox, Some(Status::Inbox));
        let keyed = |key, context: &KeyContext| key_action(press(key), context);
        let is = |action, advance| Some(Keyed { action, advance });
        assert_eq!(keyed("e", &open), is(KeyAction::Edit, false));
        assert_eq!(keyed("Enter", &open), is(KeyAction::Edit, false));
        assert_eq!(keyed("x", &open), is(KeyAction::ToggleComplete, true));
        assert_eq!(keyed(" ", &open), is(KeyAction::ToggleComplete, true));
        assert_eq!(keyed("a", &open), is(KeyAction::Archive, true));
        assert_eq!(keyed("l", &open), is(KeyAction::ToggleLater, true));
        assert_eq!(keyed("c", &open), is(KeyAction::Copy, false));
        assert_eq!(keyed("o", &open), is(KeyAction::OpenDetails, false));
        assert_eq!(keyed("Backspace", &open), is(KeyAction::Delete, true));
        // An open note is already in the inbox.
        assert_eq!(keyed("r", &open), None);

        let archived = on(View::Archive, Some(Status::Archived));
        assert_eq!(keyed("a", &archived), None);
        assert_eq!(keyed("r", &archived), is(KeyAction::Restore, true));
        assert_eq!(keyed("x", &archived), is(KeyAction::ToggleComplete, false));
        assert_eq!(keyed("l", &archived), is(KeyAction::ToggleLater, false));

        // In a search the note stays listed, so the selection stays put.
        let found = KeyContext {
            searching: true,
            ..open
        };
        assert_eq!(keyed("x", &found), is(KeyAction::ToggleComplete, false));
        assert_eq!(keyed("a", &found), is(KeyAction::Archive, false));
        assert_eq!(keyed("Delete", &found), is(KeyAction::Delete, true));
    }

    #[test]
    fn a_focused_button_keeps_enter_and_space() {
        let on_button = KeyContext {
            on_control: true,
            ..on(View::Inbox, Some(Status::Inbox))
        };
        assert_eq!(action(press("Enter"), &on_button), None);
        assert_eq!(action(press(" "), &on_button), None);
        assert_eq!(action(press("e"), &on_button), Some(KeyAction::Edit));
        assert_eq!(
            action(press("x"), &on_button),
            Some(KeyAction::ToggleComplete)
        );
    }

    #[test]
    fn an_open_dialog_takes_every_key() {
        let dialog = KeyContext {
            dialog_open: true,
            ..on(View::Inbox, Some(Status::Inbox))
        };
        assert_eq!(action(press("x"), &dialog), None);
        assert_eq!(action(press("1"), &dialog), None);
    }

    #[test]
    fn hash_opens_the_project_picker_everywhere() {
        assert_eq!(
            action(press("#"), &on(View::Inbox, None)),
            Some(KeyAction::PickCategory)
        );
        assert_eq!(
            action(press("#"), &on(View::Settings, None)),
            Some(KeyAction::PickCategory)
        );
    }

    #[test]
    fn hash_while_typing_in_a_field_is_left_to_the_field() {
        let field = KeyContext {
            typing: true,
            ..on(View::Inbox, None)
        };
        assert_eq!(action(press("#"), &field), None);
    }

    #[test]
    fn escape_clears_search_then_selection() {
        let searching = KeyContext {
            searching: true,
            ..on(View::Inbox, None)
        };
        assert_eq!(
            action(press("Escape"), &searching),
            Some(KeyAction::ClearSearch)
        );
        assert_eq!(
            action(press("Escape"), &on(View::Inbox, None)),
            Some(KeyAction::ClearSelection)
        );
    }

    #[test]
    fn section_names_from_the_shell() {
        assert_eq!(View::parse("inbox"), Some(View::Inbox));
        assert_eq!(View::parse("settings"), Some(View::Settings));
        assert_eq!(View::parse("nowhere"), None);
    }

    fn note_at(id: &str, status: Status, created_at: i64) -> Note {
        Note {
            id: id.into(),
            content: id.into(),
            category: Some("work".into()),
            source: None,
            status,
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
    fn project_view_queries_every_status_for_that_project() {
        let query = build_query(
            View::Project("work".into()),
            &Filters::default(),
            TODAY,
            None,
        );
        assert_eq!(query.category.as_deref(), Some("work"));
        assert!(query.statuses.is_empty());
        assert_eq!(query.later, None);
    }

    #[test]
    fn group_by_day_puts_open_notes_before_finished_within_a_day() {
        let day = 86_400_000i64;
        let today = (10 * day, 11 * day);
        let notes = vec![
            note_at("done-new", Status::Completed, 10 * day + 900),
            note_at("open-old", Status::Inbox, 10 * day + 200),
            note_at("arch-mid", Status::Archived, 10 * day + 500),
            note_at("open-new", Status::Inbox, 10 * day + 800),
        ];
        let groups = group_by_day(&notes, today);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, DayLabel::Today);
        let ids: Vec<&str> = groups[0].1.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["open-new", "open-old", "done-new", "arch-mid"]);
    }

    #[test]
    fn group_by_day_orders_days_newest_first() {
        let day = 86_400_000i64;
        let today = (10 * day, 11 * day);
        let notes = vec![
            note_at("old", Status::Inbox, 8 * day + 100),
            note_at("today", Status::Inbox, 10 * day + 100),
            note_at("yest", Status::Inbox, 9 * day + 100),
        ];
        let groups = group_by_day(&notes, today);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, DayLabel::Today);
        assert_eq!(groups[1].0, DayLabel::Yesterday);
        assert_eq!(groups[2].0, DayLabel::Earlier(8 * day));
    }
}
