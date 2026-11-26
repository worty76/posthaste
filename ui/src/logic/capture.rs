//! What the capture window does with a key, and which keys it advertises.

use posthaste_i18n::{key, Key};
use posthaste_model::{EnterBehavior, Os, Settings};

use super::shortcut::mod_cap;

/// The two capture settings that change what keys mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRules {
    /// Enter saves (and Shift+Enter is a new line), or the reverse.
    pub enter_saves: bool,
    /// A save closes the window.
    pub hide_after_save: bool,
}

impl Default for KeyRules {
    fn default() -> Self {
        KeyRules {
            enter_saves: true,
            hide_after_save: true,
        }
    }
}

impl KeyRules {
    /// The rules for these settings; the defaults until settings arrive.
    pub fn from_settings(settings: Option<&Settings>) -> Self {
        settings.map_or_else(KeyRules::default, |s| KeyRules {
            enter_saves: s.enter_behavior == EnterBehavior::Save,
            hide_after_save: s.hide_after_save,
        })
    }
}

/// One key press, reduced to what the capture window cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress<'a> {
    /// `KeyboardEvent.key`.
    pub key: &'a str,
    pub shift: bool,
    /// The platform's primary modifier is held.
    pub primary: bool,
    /// An input method is composing (Telex, CJK, dead keys).
    pub composing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Close the window, keeping the text as a draft.
    Dismiss,
    /// Rewrite the note with AI.
    Polish,
    /// Open the project picker.
    PickProject,
    /// Toggle the active project at 0-based slot `0..9` (`Mod+1`..`Mod+9`).
    ToggleProject(usize),
    /// Save the note; `hide` closes the window afterwards.
    Save { hide: bool },
}

/// What a key press means. `None` leaves the key to the text field.
pub fn key_action(press: KeyPress<'_>, rules: KeyRules) -> Option<KeyAction> {
    // Enter while composing confirms the composition; it must never save.
    if press.composing {
        return None;
    }
    if press.key == "Escape" {
        return Some(KeyAction::Dismiss);
    }
    if press.primary && press.key.eq_ignore_ascii_case("i") {
        return Some(KeyAction::Polish);
    }
    if press.primary && press.key.eq_ignore_ascii_case("p") {
        return Some(KeyAction::PickProject);
    }
    if press.primary && !press.shift && press.key.len() == 1 {
        let b = press.key.as_bytes()[0];
        if (b'1'..=b'9').contains(&b) {
            return Some(KeyAction::ToggleProject((b - b'1') as usize));
        }
    }
    if press.key != "Enter" {
        return None;
    }
    if rules.enter_saves {
        if press.shift && !press.primary {
            return None; // a new line
        }
        // Enter saves and closes; with the modifier it stays for another note.
        Some(KeyAction::Save {
            hide: !press.primary && rules.hide_after_save,
        })
    } else if press.primary {
        Some(KeyAction::Save {
            hide: rules.hide_after_save,
        })
    } else {
        None
    }
}

/// Whether `press` is a paste chord handled at the app level on Linux, where
/// WebKitGTK ignores virtual-keyboard `Ctrl+V` and `Shift+Insert`.
pub fn is_paste_chord(press: KeyPress<'_>, os: Os) -> bool {
    if os != Os::Linux || press.composing {
        return false;
    }
    (press.primary && !press.shift && press.key.eq_ignore_ascii_case("v"))
        || (press.shift && !press.primary && press.key == "Insert")
}

/// Replaces the UTF-16 code-unit range `start..end` in `current` with
/// `inserted`, returning the new string and the UTF-16 caret offset after
/// `inserted`. Out-of-range or inverted offsets are clamped safely.
pub fn splice_utf16(current: &str, start: usize, end: usize, inserted: &str) -> (String, u32) {
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let start_byte = utf16_to_byte(current, start);
    let end_byte = utf16_to_byte(current, end);
    let mut next = String::with_capacity(current.len() - (end_byte - start_byte) + inserted.len());
    next.push_str(&current[..start_byte]);
    next.push_str(inserted);
    next.push_str(&current[end_byte..]);
    let caret = current[..start_byte].encode_utf16().count() + inserted.encode_utf16().count();
    (next, caret as u32)
}

fn utf16_to_byte(s: &str, utf16_offset: usize) -> usize {
    let mut units = 0;
    for (byte_idx, ch) in s.char_indices() {
        if units >= utf16_offset {
            return byte_idx;
        }
        units += ch.len_utf16();
    }
    s.len()
}

/// How much room a hint needs before it is shown, for the stylesheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintWeight {
    /// Always shown.
    Core,
    /// Dropped first when the window is narrow.
    Extra,
    /// Dropped when the status line needs the room.
    Spare,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub keys: Vec<&'static str>,
    pub label: Key,
    pub weight: HintWeight,
}

/// The key hints in the footer, matching what `key_action` will do.
pub fn hints(rules: KeyRules, os: Os) -> Vec<Hint> {
    let (save, new_line) = if rules.enter_saves {
        (vec!["↵"], vec!["⇧", "↵"])
    } else {
        (vec![mod_cap(os), "↵"], vec!["↵"])
    };
    vec![
        Hint {
            keys: save,
            label: key::CAPTURE_HINT_SAVE,
            weight: HintWeight::Core,
        },
        Hint {
            keys: new_line,
            label: key::CAPTURE_HINT_NEW_LINE,
            weight: HintWeight::Extra,
        },
        Hint {
            keys: vec!["esc"],
            label: key::CAPTURE_HINT_CLOSE,
            weight: HintWeight::Spare,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: &str) -> KeyPress<'_> {
        KeyPress {
            key,
            shift: false,
            primary: false,
            composing: false,
        }
    }

    const ENTER_SAVES: KeyRules = KeyRules {
        enter_saves: true,
        hide_after_save: true,
    };
    const ENTER_ADDS_LINE: KeyRules = KeyRules {
        enter_saves: false,
        hide_after_save: true,
    };

    #[test]
    fn enter_saves_and_closes() {
        assert_eq!(
            key_action(press("Enter"), ENTER_SAVES),
            Some(KeyAction::Save { hide: true })
        );
    }

    #[test]
    fn shift_enter_is_a_new_line() {
        let shift_enter = KeyPress {
            shift: true,
            ..press("Enter")
        };
        assert_eq!(key_action(shift_enter, ENTER_SAVES), None);
    }

    #[test]
    fn mod_enter_saves_and_stays_open_for_another_note() {
        let mod_enter = KeyPress {
            primary: true,
            ..press("Enter")
        };
        assert_eq!(
            key_action(mod_enter, ENTER_SAVES),
            Some(KeyAction::Save { hide: false })
        );
    }

    #[test]
    fn with_close_after_saving_off_enter_saves_and_stays() {
        let rules = KeyRules {
            hide_after_save: false,
            ..ENTER_SAVES
        };
        assert_eq!(
            key_action(press("Enter"), rules),
            Some(KeyAction::Save { hide: false })
        );
    }

    #[test]
    fn with_enter_adds_a_line_only_mod_enter_saves() {
        assert_eq!(key_action(press("Enter"), ENTER_ADDS_LINE), None);
        let mod_enter = KeyPress {
            primary: true,
            ..press("Enter")
        };
        assert_eq!(
            key_action(mod_enter, ENTER_ADDS_LINE),
            Some(KeyAction::Save { hide: true })
        );
    }

    #[test]
    fn the_enter_that_confirms_a_composition_never_saves() {
        let composing = KeyPress {
            composing: true,
            ..press("Enter")
        };
        assert_eq!(key_action(composing, ENTER_SAVES), None);
        let escape = KeyPress {
            composing: true,
            ..press("Escape")
        };
        assert_eq!(key_action(escape, ENTER_SAVES), None);
    }

    #[test]
    fn escape_dismisses_and_mod_i_polishes() {
        assert_eq!(
            key_action(press("Escape"), ENTER_SAVES),
            Some(KeyAction::Dismiss)
        );
        for key in ["i", "I"] {
            let mod_i = KeyPress {
                primary: true,
                ..press(key)
            };
            assert_eq!(key_action(mod_i, ENTER_SAVES), Some(KeyAction::Polish));
        }
        assert_eq!(key_action(press("i"), ENTER_SAVES), None);
    }

    #[test]
    fn mod_p_opens_the_project_picker() {
        for key in ["p", "P"] {
            let mod_p = KeyPress {
                primary: true,
                ..press(key)
            };
            assert_eq!(key_action(mod_p, ENTER_SAVES), Some(KeyAction::PickProject));
        }
        assert_eq!(key_action(press("p"), ENTER_SAVES), None);
    }

    #[test]
    fn mod_1_through_9_toggles_a_project_slot() {
        for (i, digit) in ["1", "2", "3", "4", "5", "6", "7", "8", "9"]
            .into_iter()
            .enumerate()
        {
            let mod_digit = KeyPress {
                primary: true,
                ..press(digit)
            };
            assert_eq!(
                key_action(mod_digit, ENTER_SAVES),
                Some(KeyAction::ToggleProject(i))
            );
            assert_eq!(key_action(press(digit), ENTER_SAVES), None);
        }
        let mod_zero = KeyPress {
            primary: true,
            ..press("0")
        };
        assert_eq!(key_action(mod_zero, ENTER_SAVES), None);
        let mod_shift_one = KeyPress {
            primary: true,
            shift: true,
            ..press("1")
        };
        assert_eq!(key_action(mod_shift_one, ENTER_SAVES), None);
    }

    #[test]
    fn rules_follow_the_settings_and_default_until_they_load() {
        assert_eq!(KeyRules::from_settings(None), ENTER_SAVES);
        let settings = Settings {
            enter_behavior: EnterBehavior::Newline,
            hide_after_save: false,
            ..Settings::default()
        };
        assert_eq!(
            KeyRules::from_settings(Some(&settings)),
            KeyRules {
                enter_saves: false,
                hide_after_save: false
            }
        );
    }

    #[test]
    fn hints_name_the_keys_that_actually_save() {
        let caps = |rules, os| -> Vec<Vec<&str>> {
            hints(rules, os).into_iter().map(|h| h.keys).collect()
        };
        assert_eq!(
            caps(ENTER_SAVES, Os::Macos),
            [vec!["↵"], vec!["⇧", "↵"], vec!["esc"]]
        );
        assert_eq!(
            caps(ENTER_ADDS_LINE, Os::Macos),
            [vec!["⌘", "↵"], vec!["↵"], vec!["esc"]]
        );
        assert_eq!(caps(ENTER_ADDS_LINE, Os::Linux)[0], ["Ctrl", "↵"]);
    }

    #[test]
    fn paste_chords_only_match_on_linux() {
        let ctrl_v = KeyPress {
            primary: true,
            ..press("v")
        };
        let ctrl_upper_v = KeyPress {
            primary: true,
            ..press("V")
        };
        let shift_insert = KeyPress {
            shift: true,
            ..press("Insert")
        };
        assert!(is_paste_chord(ctrl_v, Os::Linux));
        assert!(is_paste_chord(ctrl_upper_v, Os::Linux));
        assert!(is_paste_chord(shift_insert, Os::Linux));
        assert!(!is_paste_chord(ctrl_v, Os::Macos));
        assert!(!is_paste_chord(ctrl_v, Os::Windows));
        assert!(!is_paste_chord(shift_insert, Os::Macos));
        assert!(!is_paste_chord(shift_insert, Os::Windows));
        assert!(!is_paste_chord(
            KeyPress {
                composing: true,
                ..ctrl_v
            },
            Os::Linux
        ));
        assert!(!is_paste_chord(
            KeyPress {
                shift: true,
                ..ctrl_v
            },
            Os::Linux
        ));
    }

    #[test]
    fn splice_utf16_replaces_selection_and_places_caret_after_paste() {
        assert_eq!(
            splice_utf16("hello world", 6, 11, "Posthaste"),
            ("hello Posthaste".to_owned(), 15)
        );
        // Emoji is 2 UTF-16 code units.
        assert_eq!(splice_utf16("a😀b", 3, 3, "X"), ("a😀Xb".to_owned(), 4));
        // Out-of-range offsets clamp to the end.
        assert_eq!(splice_utf16("hi", 99, 99, "!"), ("hi!".to_owned(), 3));
    }
}
