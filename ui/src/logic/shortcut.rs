//! Showing keyboard shortcuts the way each platform writes them.

use posthaste_model::Os;

/// Splits an accelerator such as `CmdOrCtrl+Shift+Space` into the key caps
/// to show on this platform.
pub fn format_accelerator(accelerator: &str, os: Os) -> Vec<String> {
    accelerator
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| key_cap(part, os).to_owned())
        .collect()
}

fn key_cap(part: &str, os: Os) -> &str {
    let lower = part.to_ascii_lowercase();
    if lower == "cmdorctrl" || lower == "commandorcontrol" {
        return mod_cap(os);
    }
    if os == Os::Macos {
        match lower.as_str() {
            "cmd" | "command" | "super" => return "⌘",
            "ctrl" | "control" => return "⌃",
            "alt" | "option" => return "⌥",
            "shift" => return "⇧",
            _ => {}
        }
    }
    match lower.as_str() {
        "super" if os == Os::Windows => return "Win",
        "super" => return "Super",
        "control" => return "Ctrl",
        _ => {}
    }
    match part {
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "Enter" => "↵",
        "Backquote" => "`",
        "Minus" => "-",
        "Equal" => "=",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Semicolon" => ";",
        "Quote" => "'",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        other => other,
    }
}

/// The key cap of the platform's primary modifier.
pub fn mod_cap(os: Os) -> &'static str {
    if os == Os::Macos {
        "⌘"
    } else {
        "Ctrl"
    }
}

/// Whether the platform's primary modifier is held: ⌘ on macOS, Ctrl
/// elsewhere. Shortcuts test this, never the two keys directly.
pub fn has_mod(meta: bool, ctrl: bool, os: Os) -> bool {
    if os == Os::Macos {
        meta
    } else {
        ctrl
    }
}

/// The modifier keys held during a key press.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

/// The name an accelerator uses for the physical key `code`, when it can be
/// part of a global shortcut.
fn key_name(code: &str) -> Option<&str> {
    const NAMED: [&str; 20] = [
        "Space",
        "Enter",
        "Tab",
        "Backquote",
        "Minus",
        "Equal",
        "BracketLeft",
        "BracketRight",
        "Backslash",
        "Semicolon",
        "Quote",
        "Comma",
        "Period",
        "Slash",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Insert",
        "Delete",
    ];
    let all =
        |rest: &str, test: fn(&u8) -> bool| !rest.is_empty() && rest.bytes().all(|b| test(&b));
    if let Some(letter) = code.strip_prefix("Key") {
        return (letter.len() == 1 && all(letter, u8::is_ascii_uppercase)).then_some(letter);
    }
    if let Some(digit) = code.strip_prefix("Digit") {
        return (digit.len() == 1 && all(digit, u8::is_ascii_digit)).then_some(digit);
    }
    if is_function_key(code) || matches!(code, "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight")
    {
        return Some(code);
    }
    NAMED.contains(&code).then_some(code)
}

fn is_function_key(name: &str) -> bool {
    name.strip_prefix('F')
        .is_some_and(|n| (1..=2).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Turns a key press into an accelerator such as `Ctrl+Shift+Space`. `None`
/// while only modifiers are held, and for a combination that would not make
/// a usable global shortcut: no modifier, or Shift alone, unless the key is
/// a function key.
pub fn accelerator_from_press(code: &str, held: Modifiers, os: Os) -> Option<String> {
    let key = key_name(code)?;
    if !held.ctrl && !held.alt && !held.meta && !is_function_key(key) {
        return None;
    }
    let mut parts = Vec::with_capacity(5);
    if held.meta {
        parts.push(if os == Os::Macos { "Cmd" } else { "Super" });
    }
    if held.ctrl {
        parts.push("Ctrl");
    }
    if held.alt {
        parts.push("Alt");
    }
    if held.shift {
        parts.push("Shift");
    }
    parts.push(key);
    Some(parts.join("+"))
}

/// The inbox shortcut a fresh install has on `os`.
pub fn default_inbox_shortcut(os: Os) -> &'static str {
    if os == Os::Macos {
        "Cmd+Ctrl+N"
    } else {
        "Ctrl+Alt+N"
    }
}

/// The Today shortcut a fresh install has on `os`.
pub fn default_today_shortcut(os: Os) -> &'static str {
    if os == Os::Macos {
        "Cmd+Ctrl+G"
    } else {
        "Ctrl+Alt+G"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_key_caps_in_the_platforms_own_notation() {
        assert_eq!(
            format_accelerator("CmdOrCtrl+Shift+Space", Os::Macos),
            ["⌘", "⇧", "Space"]
        );
        assert_eq!(
            format_accelerator("CmdOrCtrl+Shift+Space", Os::Windows),
            ["Ctrl", "Shift", "Space"]
        );
        assert_eq!(format_accelerator("Cmd+Ctrl+N", Os::Macos), ["⌘", "⌃", "N"]);
        assert_eq!(
            format_accelerator("Super+Alt+ArrowUp", Os::Windows),
            ["Win", "Alt", "↑"]
        );
        assert_eq!(
            format_accelerator("Super+Control+K", Os::Linux),
            ["Super", "Ctrl", "K"]
        );
    }

    #[test]
    fn ignores_stray_separators_and_spaces() {
        assert_eq!(format_accelerator(" Ctrl + + N ", Os::Linux), ["Ctrl", "N"]);
        assert!(format_accelerator("", Os::Linux).is_empty());
    }

    #[test]
    fn the_primary_modifier_is_cmd_on_macos_and_ctrl_elsewhere() {
        assert!(has_mod(true, false, Os::Macos));
        assert!(!has_mod(false, true, Os::Macos));
        assert!(has_mod(false, true, Os::Windows));
        assert!(!has_mod(true, false, Os::Linux));
    }

    #[test]
    fn a_key_press_becomes_an_accelerator() {
        let held = |ctrl, alt, shift, meta| Modifiers {
            ctrl,
            alt,
            shift,
            meta,
        };
        assert_eq!(
            accelerator_from_press("Space", held(true, false, true, false), Os::Linux).as_deref(),
            Some("Ctrl+Shift+Space")
        );
        assert_eq!(
            accelerator_from_press("KeyN", held(true, false, false, true), Os::Macos).as_deref(),
            Some("Cmd+Ctrl+N")
        );
        assert_eq!(
            accelerator_from_press("Digit4", held(false, true, false, true), Os::Windows)
                .as_deref(),
            Some("Super+Alt+4")
        );
        assert_eq!(
            accelerator_from_press("F5", Modifiers::default(), Os::Linux).as_deref(),
            Some("F5")
        );
    }

    #[test]
    fn unusable_combinations_are_refused() {
        let none = Modifiers::default();
        let shift = Modifiers {
            shift: true,
            ..none
        };
        let ctrl = Modifiers { ctrl: true, ..none };
        // No modifier, or Shift alone, would swallow ordinary typing.
        assert_eq!(accelerator_from_press("KeyA", none, Os::Linux), None);
        assert_eq!(accelerator_from_press("KeyA", shift, Os::Linux), None);
        // A modifier on its own is not a key yet.
        assert_eq!(accelerator_from_press("ControlLeft", ctrl, Os::Linux), None);
        assert_eq!(accelerator_from_press("MetaRight", ctrl, Os::Linux), None);
        // Keys an accelerator has no name for.
        assert_eq!(accelerator_from_press("NumpadAdd", ctrl, Os::Linux), None);
        assert_eq!(accelerator_from_press("Keyboard", ctrl, Os::Linux), None);
        assert_eq!(accelerator_from_press("F123", ctrl, Os::Linux), None);
    }

    #[test]
    fn default_shortcuts_follow_the_platform() {
        assert_eq!(default_inbox_shortcut(Os::Macos), "Cmd+Ctrl+N");
        assert_eq!(default_inbox_shortcut(Os::Windows), "Ctrl+Alt+N");
        assert_eq!(default_today_shortcut(Os::Macos), "Cmd+Ctrl+G");
        assert_eq!(default_today_shortcut(Os::Linux), "Ctrl+Alt+G");
    }
}
