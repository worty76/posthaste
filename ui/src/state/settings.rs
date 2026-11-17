use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_model::{CommandError, Settings, SettingsState, ShortcutReport};

use crate::ipc::{api, events};

/// Settings and shortcut registration status. Both are `None` until the
/// first answer arrives.
#[derive(Clone, Copy)]
pub struct SettingsHandle {
    pub settings: ReadSignal<Option<Settings>>,
    pub shortcuts: ReadSignal<Option<ShortcutReport>>,
    set_settings: WriteSignal<Option<Settings>>,
    set_shortcuts: WriteSignal<Option<ShortcutReport>>,
}

impl SettingsHandle {
    /// Shows what the shell says is now in force.
    pub fn accept(self, state: SettingsState) {
        self.set_settings.set(Some(state.settings));
        self.set_shortcuts.set(Some(state.shortcuts));
    }

    /// Changes the settings and saves them. On failure the old settings
    /// stay, on screen and on disk.
    pub async fn update(self, change: impl FnOnce(&mut Settings)) -> Result<(), CommandError> {
        let Some(mut next) = self.settings.get_untracked() else {
            return Ok(());
        };
        change(&mut next);
        self.accept(api::settings_update(&next).await?);
        Ok(())
    }
}

/// Loads the settings and reloads them whenever any window changes them.
pub fn use_settings() -> SettingsHandle {
    let (settings, set_settings) = signal(None::<Settings>);
    let (shortcuts, set_shortcuts) = signal(None::<ShortcutReport>);
    let handle = SettingsHandle {
        settings,
        shortcuts,
        set_settings,
        set_shortcuts,
    };

    let reload = move || {
        spawn_local(async move {
            // A failed read keeps what is on screen.
            if let Ok(state) = api::settings_get().await {
                handle.accept(state);
            }
        });
    };
    reload();
    // Refetch rather than trust the payload: shortcut status may differ too.
    events::on::<events::SettingsChanged>(move |_| reload());

    handle
}
