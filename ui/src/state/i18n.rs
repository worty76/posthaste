use leptos::prelude::*;
use posthaste_i18n::{Key, PluralKey, Translator};
use posthaste_model::CommandError;

use super::SettingsHandle;
use crate::ipc;

/// The active language. Reading it inside a view makes that text follow
/// the language setting without a reload. Windows pass it down as a prop.
#[derive(Clone, Copy)]
pub struct I18n {
    translator: Memo<Translator>,
}

impl I18n {
    pub fn translator(&self) -> Translator {
        self.translator.get()
    }

    /// A message that re-renders when the language changes.
    pub fn text(&self, key: Key) -> impl Fn() -> &'static str + Copy {
        let translator = self.translator;
        move || translator.get().text(key)
    }

    /// The same, as a signal: for a control that takes its label as one.
    pub fn label(&self, key: Key) -> Signal<String> {
        let translator = self.translator;
        Signal::derive(move || translator.get().text(key).to_owned())
    }
}

/// The language of `settings`. Also keeps `<html lang dir>` in step with it.
pub fn use_i18n(settings: SettingsHandle) -> I18n {
    let translator = Memo::new(move |_| {
        settings.settings.with(|s| match s {
            Some(s) => Translator::new(&s.language),
            None => Translator::default(),
        })
    });
    Effect::new(move |_| {
        let language = translator.get().language();
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("lang", language.code);
            let _ = root.set_attribute("dir", language.direction.as_str());
        }
    });
    I18n { translator }
}

/// Something to tell the user, kept as a key and its values instead of
/// finished text, so a line already on screen follows a language change.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Plain(Key),
    /// A message with `{placeholders}` and what goes in them.
    With(Key, Vec<(&'static str, String)>),
    /// A message that changes with a number, and its other values.
    Count(PluralKey, u64, Vec<(&'static str, String)>),
    /// A command that failed, in the shell's own words.
    Failure(CommandError),
}

impl Message {
    /// A message with one placeholder.
    pub fn with(key: Key, name: &'static str, value: impl Into<String>) -> Message {
        Message::With(key, vec![(name, value.into())])
    }

    pub fn count(key: PluralKey, count: u64) -> Message {
        Message::Count(key, count, Vec::new())
    }

    pub fn text(&self, t: Translator) -> String {
        fn borrowed<'a>(values: &'a [(&'static str, String)]) -> Vec<(&'a str, &'a str)> {
            values
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect()
        }
        match self {
            Message::Plain(key) => t.text(*key).to_owned(),
            Message::With(key, values) => t.format(*key, &borrowed(values)),
            Message::Count(key, count, values) => t.plural(*key, *count, &borrowed(values)),
            Message::Failure(error) => ipc::message(error, t),
        }
    }
}
