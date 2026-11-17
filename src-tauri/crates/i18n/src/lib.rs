//! The interface text, in every language Posthaste ships.
//!
//! The text lives in `locales/<code>/*.json` at the repository root. The
//! build script checks those files and embeds them, so this crate does no
//! I/O and works the same in the desktop shell and in the WebAssembly UI:
//! one catalogue, read by both.
//!
//! ```
//! use posthaste_i18n::{key, Translator};
//!
//! let t = Translator::new("vi");
//! assert_eq!(t.text(key::CAPTURE_STATUS_SAVED), "Đã lưu");
//! assert_eq!(t.format(key::CAPTURE_FROM_SOURCE, &[("app", "Notes")]), "từ Notes");
//! ```

include!(concat!(env!("OUT_DIR"), "/catalogue.rs"));

/// The language every other one is checked against and falls back to.
pub const SOURCE_LANGUAGE: &str = "en";

/// The name of one message. Only the generated `key` module makes these,
/// so a key that compiles is a key that exists in English.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key(&'static str);

impl Key {
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// The name of a message that changes with a number ("1 file", "2 files").
/// Read it with [`Translator::plural`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluralKey(&'static str);

/// How a language chooses a plural form from a number. Each language names
/// its rule in `meta.json`; a language whose rule is not here yet needs a
/// variant added, with the CLDR rule it implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralRule {
    /// One form for every number (Vietnamese, Japanese, Chinese).
    Other,
    /// `one` for exactly 1, `other` otherwise (English, German, Spanish).
    OneOther,
}

impl PluralRule {
    fn category(self, count: u64) -> &'static str {
        match self {
            PluralRule::OneOther if count == 1 => "one",
            _ => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Ltr,
    Rtl,
}

impl Direction {
    /// The value of the HTML `dir` attribute.
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Ltr => "ltr",
            Direction::Rtl => "rtl",
        }
    }
}

/// One shipped language.
#[derive(Debug, PartialEq, Eq)]
pub struct Language {
    /// What the `language` setting stores, e.g. "vi".
    pub code: &'static str,
    /// The language's own name for itself, for the picker.
    pub name: &'static str,
    pub direction: Direction,
    plural: PluralRule,
    /// Sorted by key.
    messages: &'static [(&'static str, &'static str)],
}

impl Language {
    fn lookup(&self, key: &str) -> Option<&'static str> {
        self.messages
            .binary_search_by(|(k, _)| (*k).cmp(key))
            .ok()
            .map(|i| self.messages[i].1)
    }

    /// The form of a plural message for `count`: the language's category
    /// for that number, or its `other` form when that category has no text.
    fn lookup_plural(&self, key: PluralKey, count: u64) -> Option<&'static str> {
        let form = |category: &str| self.lookup(&format!("{}#{category}", key.0));
        form(self.plural.category(count)).or_else(|| form("other"))
    }
}

/// Every shipped language, English first.
pub fn languages() -> &'static [Language] {
    LANGUAGES
}

fn source() -> &'static Language {
    &LANGUAGES[0]
}

/// Looks messages up in one language, falling back to English.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Translator {
    language: &'static Language,
}

impl Default for Translator {
    fn default() -> Self {
        Translator { language: source() }
    }
}

impl Translator {
    /// The translator for `code`. A code that is not shipped gives English,
    /// so a setting written by a newer version still shows an interface.
    pub fn new(code: &str) -> Self {
        let language = LANGUAGES
            .iter()
            .find(|l| l.code == code)
            .unwrap_or_else(source);
        Translator { language }
    }

    pub fn language(&self) -> &'static Language {
        self.language
    }

    /// The message as written, for text with no placeholders.
    pub fn text(&self, key: Key) -> &'static str {
        self.language
            .lookup(key.0)
            .or_else(|| source().lookup(key.0))
            // Unreachable for generated keys; the key is still better
            // than nothing on screen.
            .unwrap_or(key.0)
    }

    /// The message with each `{name}` replaced by its value.
    pub fn format(&self, key: Key, values: &[(&str, &str)]) -> String {
        fill(self.text(key), values)
    }

    /// The form of a plural message that fits `count`, with `{count}` and
    /// each other `{name}` filled in.
    pub fn plural(&self, key: PluralKey, count: u64, values: &[(&str, &str)]) -> String {
        let text = self
            .language
            .lookup_plural(key, count)
            .or_else(|| source().lookup_plural(key, count))
            .unwrap_or(key.0);
        let count = count.to_string();
        let mut all = vec![("count", count.as_str())];
        all.extend_from_slice(values);
        fill(text, &all)
    }

    /// The text before and after `{name}`, for a message with a control in
    /// the middle of it ("Press [keys] anywhere to capture one.").
    pub fn around(&self, key: Key, name: &str) -> (&'static str, &'static str) {
        let text = self.text(key);
        match text.split_once(&format!("{{{name}}}")) {
            Some(parts) => parts,
            None => (text, ""),
        }
    }
}

/// Replaces each `{name}` in `text` with its value. A placeholder with no
/// value is left as written.
fn fill(text: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            rest = &rest[open..];
            break;
        };
        let name = &after[..close];
        match values.iter().find(|(n, _)| *n == name) {
            Some((_, value)) => out.push_str(value),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_first_and_is_the_default() {
        assert_eq!(languages()[0].code, SOURCE_LANGUAGE);
        assert_eq!(Translator::default().language().code, "en");
    }

    #[test]
    fn an_unknown_language_code_shows_english() {
        let t = Translator::new("xx");
        assert_eq!(t.language().code, "en");
        assert_eq!(t.text(key::CAPTURE_STATUS_SAVED), "Saved");
    }

    #[test]
    fn vietnamese_text_is_used_when_chosen() {
        let t = Translator::new("vi");
        assert_eq!(t.language().name, "Tiếng Việt");
        assert_eq!(t.text(key::CAPTURE_STATUS_SAVED), "Đã lưu");
    }

    #[test]
    fn placeholders_are_filled_in_any_word_order() {
        let en = Translator::new("en");
        assert_eq!(
            en.format(key::CAPTURE_FROM_SOURCE, &[("app", "Code")]),
            "from Code"
        );
        let vi = Translator::new("vi");
        assert_eq!(
            vi.format(key::CAPTURE_STATUS_NOT_SAVED, &[("reason", "đĩa đầy")]),
            "Chưa lưu được: đĩa đầy"
        );
    }

    #[test]
    fn a_placeholder_without_a_value_is_left_visible() {
        let en = Translator::new("en");
        assert_eq!(en.format(key::CAPTURE_FROM_SOURCE, &[]), "from {app}");
    }

    #[test]
    fn a_missing_translation_falls_back_to_english() {
        let partial = Language {
            code: "zz",
            name: "Partial",
            direction: Direction::Ltr,
            plural: PluralRule::Other,
            messages: &[],
        };
        let partial: &'static Language = Box::leak(Box::new(partial));
        let t = Translator { language: partial };
        assert_eq!(t.text(key::CAPTURE_STATUS_SAVED), "Saved");
    }

    /// The build already rejects extra keys and mismatched placeholders;
    /// this is what catches a key nobody translated yet.
    #[test]
    fn every_language_is_complete() {
        let english = source();
        for language in languages() {
            let missing: Vec<&str> = english
                .messages
                .iter()
                .map(|(k, _)| *k)
                // A language needs every message, but of a plural message
                // only the forms its own rule uses; `other` is always one.
                .filter(|k| !k.contains('#') || k.ends_with("#other"))
                .filter(|k| language.lookup(k).is_none())
                .collect();
            assert!(
                missing.is_empty(),
                "locales/{} has no text for: {}",
                language.code,
                missing.join(", ")
            );
        }
    }

    #[test]
    fn a_plural_message_follows_each_languages_own_rule() {
        let en = Translator::new("en");
        assert_eq!(en.plural(key::COMMON_BADGES_FILES, 1, &[]), "1 file");
        assert_eq!(en.plural(key::COMMON_BADGES_FILES, 2, &[]), "2 files");
        assert_eq!(en.plural(key::COMMON_BADGES_FILES, 0, &[]), "0 files");
        let vi = Translator::new("vi");
        assert_eq!(vi.plural(key::COMMON_BADGES_FILES, 1, &[]), "1 tệp");
        assert_eq!(vi.plural(key::COMMON_BADGES_FILES, 5, &[]), "5 tệp");
    }

    #[test]
    fn a_message_can_be_split_around_a_placeholder() {
        let en = Translator::new("en");
        assert_eq!(
            en.around(key::TODAY_EMPTY_CAPTURE_WITH_SHORTCUT, "keys"),
            ("Press ", " anywhere to capture one.")
        );
        assert_eq!(en.around(key::CAPTURE_STATUS_SAVED, "keys"), ("Saved", ""));
    }

    #[test]
    fn messages_are_sorted_for_lookup() {
        for language in languages() {
            assert!(language.messages.windows(2).all(|w| w[0].0 < w[1].0));
        }
    }
}
