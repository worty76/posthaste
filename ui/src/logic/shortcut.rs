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
}
