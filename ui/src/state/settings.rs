use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_model::{Settings, ShortcutReport};

use crate::ipc::{api, events};

/// Settings and shortcut registration status. Both are `None` until the
/// first answer arrives.
#[derive(Clone, Copy)]
pub struct SettingsHandle {
    pub settings: ReadSignal<Option<Settings>>,
    pub shortcuts: ReadSignal<Option<ShortcutReport>>,
}

/// Loads the settings and reloads them whenever any window changes them.
pub fn use_settings() -> SettingsHandle {
    let (settings, set_settings) = signal(None::<Settings>);
    let (shortcuts, set_shortcuts) = signal(None::<ShortcutReport>);

    let reload = move || {
        spawn_local(async move {
            // A failed read keeps what is on screen.
            if let Ok(state) = api::settings_get().await {
                set_settings.set(Some(state.settings));
                set_shortcuts.set(Some(state.shortcuts));
            }
        });
    };
    reload();
    // Refetch rather than trust the payload: shortcut status may differ too.
    events::on::<events::SettingsChanged>(move |_| reload());

    SettingsHandle {
        settings,
        shortcuts,
    }
}
