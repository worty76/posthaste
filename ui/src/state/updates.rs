use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_model::{CommandError, UpdateInfo};

use crate::ipc::api;
use crate::logic::updates::{is_due, CHECK_EVERY_MS};
use crate::time;

/// What the update banner and the Settings row share. `found` stays `None`
/// until a check finds something newer than this build.
#[derive(Clone, Copy)]
pub struct Updates {
    pub found: RwSignal<Option<UpdateInfo>>,
    /// An install is running.
    pub installing: RwSignal<bool>,
    /// When this launch last checked. In memory only, so the app checks
    /// once per launch and then every six hours.
    last_check: StoredValue<Option<i64>>,
}

impl Updates {
    /// Looks for a newer release now. Fails when the check itself does;
    /// the caller decides whether to show that.
    pub async fn check(self) -> Result<Option<UpdateInfo>, CommandError> {
        let found = api::update_check().await?;
        self.last_check.set_value(Some(time::now_ms()));
        self.found.set(found.clone());
        Ok(found)
    }

    /// Installs the newest release. On success the app restarts and this
    /// never returns.
    pub async fn install(self) -> Result<(), CommandError> {
        self.installing.set(true);
        let outcome = api::update_install().await;
        self.installing.set(false);
        outcome
    }

    fn check_when_due(self) {
        if is_due(self.last_check.get_value(), time::now_ms()) {
            spawn_local(async move {
                // A failed background check stays quiet: offline is normal.
                let _ = self.check().await;
            });
        }
    }
}

/// Checks for a newer release at launch and then every six hours.
pub fn use_updates() -> Updates {
    let updates = Updates {
        found: RwSignal::new(None),
        installing: RwSignal::new(false),
        last_check: StoredValue::new(None),
    };
    updates.check_when_due();
    let every = Duration::from_millis(CHECK_EVERY_MS as u64);
    if let Ok(timer) = set_interval_with_handle(move || updates.check_when_due(), every) {
        on_cleanup(move || timer.clear());
    }
    updates
}
