use leptos::prelude::*;
use posthaste_i18n::{Key, Translator};

use super::SettingsHandle;

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
