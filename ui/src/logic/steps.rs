//! Decisions for the Steps list in Note details: what a key does on the
//! list, how a paste becomes steps, and what the done/total badge says.

use super::details::KeyPress;
use posthaste_model::NewStep;

/// What a key does on the Steps list. The window turns each of these into
/// its action; the list's own state (which row, whether a step's text is
/// being edited) is the window's to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKey {
    Up,
    Down,
    /// Space on a selected, not-editing step.
    Toggle,
    /// Enter or F2 on a selected step.
    Edit,
    Remove,
    /// The platform's primary modifier with an arrow.
    MoveUp,
    MoveDown,
    /// Enter while editing: save and open a new step below — or, on an
    /// empty line, leave without keeping anything.
    Enter,
    /// Escape while editing: save what is there and leave edit mode.
    Leave,
}

/// What a key does on the Steps list. `editing`: the caret is in a step's
/// text, so typing and plain arrows belong to the text field, never to the
/// selection. `text_empty`: the edited line holds nothing (yet).
pub fn key_action(press: KeyPress<'_>, editing: bool, text_empty: bool) -> Option<StepKey> {
    if press.composing {
        return None;
    }
    if editing {
        // Mod+Enter and Mod+O stay with the window, wherever focus sits.
        if press.primary {
            return None;
        }
        return match press.key {
            "Enter" if text_empty => Some(StepKey::Leave),
            "Enter" => Some(StepKey::Enter),
            "Escape" => Some(StepKey::Leave),
            // Typing and cursor keys belong to the text field.
            _ => None,
        };
    }
    if press.primary {
        return match press.key {
            "ArrowUp" => Some(StepKey::MoveUp),
            "ArrowDown" => Some(StepKey::MoveDown),
            _ => None,
        };
    }
    match press.key {
        "ArrowUp" | "k" => Some(StepKey::Up),
        "ArrowDown" | "j" => Some(StepKey::Down),
        " " => Some(StepKey::Toggle),
        "Enter" | "F2" | "e" | "E" => Some(StepKey::Edit),
        "Backspace" | "Delete" => Some(StepKey::Remove),
        _ => None,
    }
}

/// One step per non-empty line. A leading `- [ ]`, `- [x]`, `-` or `*` is
/// stripped; `[x]` and `[X]` keep their done state.
pub fn split_paste(text: &str) -> Vec<NewStep> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let mut done = false;
            let mut rest = line;
            for prefix in [
                "- [x] ", "- [X] ", "* [x] ", "* [X] ", "- [ ] ", "* [ ] ", "- ", "* ", "-", "*",
            ] {
                if let Some(stripped) = rest.strip_prefix(prefix) {
                    done = prefix.contains('x') || prefix.contains('X');
                    rest = stripped;
                    break;
                }
            }
            let rest = rest.trim();
            (!rest.is_empty()).then(|| NewStep {
                text: rest.to_string(),
                done,
            })
        })
        .collect()
}

/// The badge beside a note's marks: `done/total`, hidden without steps.
pub fn badge_text(done: u32, total: u32) -> Option<String> {
    (total > 0).then(|| format!("{done}/{total}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: &str) -> KeyPress<'_> {
        KeyPress {
            key,
            primary: false,
            shift: false,
            alt: false,
            composing: false,
        }
    }

    #[test]
    fn space_ticks_only_when_not_editing() {
        assert_eq!(key_action(press(" "), false, false), Some(StepKey::Toggle));
        // While editing, Space types a space.
        assert_eq!(key_action(press(" "), true, false), None);
    }

    #[test]
    fn arrow_keys_while_editing_are_left_to_the_text_field() {
        assert_eq!(
            key_action(press("ArrowUp"), false, false),
            Some(StepKey::Up)
        );
        assert_eq!(
            key_action(press("ArrowDown"), false, false),
            Some(StepKey::Down)
        );
        assert_eq!(key_action(press("ArrowUp"), true, false), None);
        assert_eq!(key_action(press("ArrowLeft"), true, false), None);
        assert_eq!(key_action(press("ArrowRight"), true, false), None);
        // Selection keys never fire in edit mode either.
        assert_eq!(key_action(press("F2"), true, false), None);
        assert_eq!(key_action(press("Backspace"), true, false), None);
        assert_eq!(key_action(press("k"), true, false), None);
    }

    #[test]
    fn enter_on_an_empty_new_step_leaves_without_saving() {
        assert_eq!(key_action(press("Enter"), true, true), Some(StepKey::Leave));
        // With text, Enter saves and opens the next step.
        assert_eq!(
            key_action(press("Enter"), true, false),
            Some(StepKey::Enter)
        );
        assert_eq!(
            key_action(press("Enter"), false, false),
            Some(StepKey::Edit)
        );
        assert_eq!(key_action(press("F2"), false, false), Some(StepKey::Edit));
        assert_eq!(key_action(press("e"), false, false), Some(StepKey::Edit));
        assert_eq!(key_action(press("E"), false, false), Some(StepKey::Edit));
        // Esc while editing saves and leaves edit mode; the window closes
        // itself, as today, when no step is being edited.
        assert_eq!(
            key_action(press("Escape"), true, false),
            Some(StepKey::Leave)
        );
        assert_eq!(key_action(press("Escape"), false, false), None);
    }

    #[test]
    fn remove_and_reorder_are_reachable_by_key() {
        assert_eq!(
            key_action(press("Delete"), false, false),
            Some(StepKey::Remove)
        );
        assert_eq!(
            key_action(press("Backspace"), false, false),
            Some(StepKey::Remove)
        );
        let primary = |key| KeyPress {
            primary: true,
            ..press(key)
        };
        assert_eq!(
            key_action(primary("ArrowUp"), false, false),
            Some(StepKey::MoveUp)
        );
        assert_eq!(
            key_action(primary("ArrowDown"), false, false),
            Some(StepKey::MoveDown)
        );
        // An IME composition never reaches the list.
        let composing = KeyPress {
            composing: true,
            ..press("Enter")
        };
        assert_eq!(key_action(composing, false, false), None);
        // Mod+Enter belongs to the window, even while editing.
        let primary = KeyPress {
            primary: true,
            ..press("Enter")
        };
        assert_eq!(key_action(primary, true, false), None);
        assert_eq!(key_action(primary, false, false), None);
    }

    #[test]
    fn split_paste_makes_one_step_per_line_and_keeps_checked_items_done() {
        let steps = split_paste("- [ ] buy milk\nbuy eggs\n- [x] call mum\n\n* [X] water\n  \n-\n");
        let expected = [
            NewStep {
                text: "buy milk".into(),
                done: false,
            },
            NewStep {
                text: "buy eggs".into(),
                done: false,
            },
            NewStep {
                text: "call mum".into(),
                done: true,
            },
            NewStep {
                text: "water".into(),
                done: true,
            },
        ];
        assert_eq!(steps.as_slice(), expected.as_slice());
        assert!(split_paste("   ").is_empty());
        assert!(split_paste("").is_empty());
    }

    #[test]
    fn badge_text_reads_done_of_total_and_is_hidden_without_steps() {
        assert_eq!(badge_text(2, 5).as_deref(), Some("2/5"));
        assert_eq!(badge_text(0, 1).as_deref(), Some("0/1"));
        assert_eq!(badge_text(5, 5).as_deref(), Some("5/5"));
        assert_eq!(badge_text(0, 0), None);
    }
}
