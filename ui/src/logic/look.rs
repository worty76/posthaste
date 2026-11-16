//! Small decisions of the look editor. Whether a look is valid is never
//! decided here: the shell checks every look before it is shown or saved.

/// Longest name a look can have (the look format's limit).
pub const MAX_NAME_CHARS: usize = 40;

/// What to hand the browser's colour picker while the hex field is being
/// typed. The picker only takes `#rrggbb`, so the short form is widened;
/// anything else goes through as typed and the browser ignores it.
pub fn picker_value(typed: &str) -> String {
    let hex = typed.trim().to_ascii_lowercase();
    let digits: Vec<char> = hex.strip_prefix('#').unwrap_or_default().chars().collect();
    if digits.len() == 3 && digits.iter().all(char::is_ascii_hexdigit) {
        let mut wide = String::from("#");
        for digit in digits {
            wide.push(digit);
            wide.push(digit);
        }
        return wide;
    }
    hex
}

/// A name cut to what the look format allows.
pub fn fit_name(name: &str) -> String {
    name.chars().take(MAX_NAME_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picker_gets_full_hex() {
        assert_eq!(picker_value("#abc"), "#aabbcc");
        assert_eq!(picker_value(" #A1B2C3 "), "#a1b2c3");
        assert_eq!(picker_value("#ab"), "#ab");
        assert_eq!(picker_value("#xyz"), "#xyz");
        assert_eq!(picker_value(""), "");
    }

    #[test]
    fn names_are_cut_by_characters() {
        assert_eq!(fit_name("Matcha copy"), "Matcha copy");
        assert_eq!(fit_name(&"ê".repeat(50)).chars().count(), MAX_NAME_CHARS);
    }
}
