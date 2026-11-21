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
}
