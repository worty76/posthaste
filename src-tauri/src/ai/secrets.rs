//! Where AI provider keys live: the system keychain (macOS Keychain,
//! Windows Credential Manager, Linux Secret Service). On a system with no
//! usable keychain they fall back to a row in the database, kept apart from
//! the settings the interface can see, and Settings says so.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// The keychain service name keys are filed under.
const SERVICE: &str = "dev.posthaste.desktop";

/// A place secrets can be kept. The real one is the OS keychain; tests use a
/// map.
pub trait KeyStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

/// The OS keychain, through the `keyring` crate.
pub struct Keychain;

impl KeyStore for Keychain {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(SERVICE, account).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, account)
            .and_then(|entry| entry.set_password(secret))
            .map_err(|e| e.to_string())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, account).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Which store keys are going to.
pub use posthaste_core::KeyStore as Backend;

/// Managed Tauri state: the keychain, and whether it currently works.
pub struct Secrets {
    store: Box<dyn KeyStore>,
    usable: AtomicBool,
}

impl Secrets {
    /// Uses the OS keychain when it answers. Blocking: keychains may show a
    /// prompt or wait on D-Bus, so call this off the main thread.
    pub fn detect() -> Self {
        Self::with_keychain(Box::new(Keychain))
    }

    pub fn with_keychain(store: Box<dyn KeyStore>) -> Self {
        let secrets = Secrets {
            store,
            usable: AtomicBool::new(false),
        };
        secrets.probe();
        secrets
    }

    /// Reading an entry that does not exist succeeds on a working keychain
    /// and fails when there is none (no Secret Service running on Linux,
    /// for one).
    fn probe(&self) -> bool {
        let usable = match self.store.get("probe") {
            Ok(_) => true,
            Err(e) => {
                log::warn!("no usable keychain, AI keys go to the database for now: {e}");
                false
            }
        };
        self.usable.store(usable, Ordering::Relaxed);
        usable
    }

    fn keychain(&self) -> Option<&dyn KeyStore> {
        self.usable
            .load(Ordering::Relaxed)
            .then_some(self.store.as_ref())
    }

    pub fn backend(&self) -> Backend {
        if self.keychain().is_some() {
            Backend::Keychain
        } else {
            Backend::Database
        }
    }

    /// The key, from the keychain or, failing that, a copy saved in the
    /// database while there was no keychain.
    pub fn get(&self, state: &AppState, account: &str) -> AppResult<Option<String>> {
        if let Some(k) = self.keychain() {
            if let Some(secret) = k.get(account).map_err(keychain_error)? {
                return Ok(Some(secret));
            }
        }
        state.with_db(|db| db.secret_get(account))
    }

    /// Whether a key is saved for `account`, without touching the keychain:
    /// reading it can make the system ask for the user's password, and
    /// Settings asks about every provider each time it opens. The answer
    /// comes from a marker kept beside the key. It holds no secret.
    pub fn has(&self, state: &AppState, account: &str) -> bool {
        state
            .with_db(|db| db.secret_get(&marker(account)))
            .ok()
            .flatten()
            .is_some()
    }

    /// Records that a key is saved for `account`, wherever it went.
    pub fn note_saved(&self, state: &AppState, account: &str) -> AppResult<()> {
        state.with_db(|db| db.secret_set(&marker(account), "1"))
    }

    /// Saves to the keychain when there is one. A keychain that was not
    /// running at startup (a login item launched before the keyring daemon)
    /// is tried again before falling back to the database.
    pub fn set(&self, state: &AppState, account: &str, secret: &str) -> AppResult<()> {
        if self.keychain().is_some() || self.probe() {
            self.store.set(account, secret).map_err(keychain_error)?;
            // An older database copy must not outlive the new key.
            state.with_db(|db| db.secret_delete(account))?;
        } else {
            state.with_db(|db| db.secret_set(account, secret))?;
        }
        self.note_saved(state, account)
    }

    /// Removes the key from wherever it may be.
    pub fn delete(&self, state: &AppState, account: &str) -> AppResult<()> {
        state.with_db(|db| db.secret_delete(&marker(account)))?;
        state.with_db(|db| db.secret_delete(account))?;
        match self.keychain() {
            Some(k) => k.delete(account).map_err(keychain_error),
            None => Ok(()),
        }
    }

    /// Once per installation: notes which providers already have a key
    /// (saved by a version that kept no marker) and removes keys of
    /// providers that no longer exist. This is the one time the keychain is
    /// read for every provider; a prompt answered with Deny leaves that
    /// provider showing no key, and is not asked again.
    pub fn index_existing(&self, state: &AppState, accounts: &[&str], retired: &[&str]) {
        let done = state.with_db(|db| db.secret_get(INDEXED)).ok().flatten();
        if done.is_some() {
            return;
        }
        for account in retired {
            let _ = self.delete(state, account);
        }
        for account in accounts {
            let found = self
                .get(state, account)
                .ok()
                .flatten()
                .is_some_and(|k| !k.trim().is_empty());
            if found {
                if let Err(e) = self.note_saved(state, account) {
                    log::warn!("could not note a saved key: {}", e.message);
                }
            }
        }
        if let Err(e) = state.with_db(|db| db.secret_set(INDEXED, "1")) {
            log::warn!("could not note that keys were indexed: {}", e.message);
        }
    }

    /// Moves keys saved in the database while there was no keychain into
    /// the keychain, now that there is one. A key already in the keychain is
    /// newer and wins. Anything the keychain refuses stays where it is.
    pub fn adopt_database_keys(&self, state: &AppState, accounts: &[&str]) {
        let Some(k) = self.keychain() else { return };
        for account in accounts {
            let Ok(Some(secret)) = state.with_db(|db| db.secret_get(account)) else {
                continue;
            };
            let moved = match k.get(account) {
                Ok(Some(_)) => true,
                Ok(None) => k.set(account, &secret).is_ok(),
                Err(_) => false,
            };
            if moved {
                if let Err(e) = state.with_db(|db| db.secret_delete(account)) {
                    log::warn!(
                        "a key moved to the keychain stayed in the database: {}",
                        e.message
                    );
                }
            } else {
                log::warn!("the keychain refused a key; it stays in the database");
            }
        }
    }
}

/// Database row noting that a key is saved for an account.
fn marker(account: &str) -> String {
    format!("have:{account}")
}

/// Database row noting that existing keys have been indexed.
const INDEXED: &str = "have:indexed";

fn keychain_error(detail: String) -> AppError {
    AppError::new(
        "keychain",
        format!("The system keychain could not be used. ({detail})"),
    )
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// An in-memory keychain. `broken` behaves like a system with none;
    /// `deny_writes` like a user clicking Deny on a keychain prompt.
    #[derive(Default)]
    pub struct FakeKeychain {
        pub items: Mutex<HashMap<String, String>>,
        pub broken: AtomicBool,
        pub deny_writes: AtomicBool,
        /// How many times a key was read.
        pub reads: std::sync::atomic::AtomicUsize,
    }

    impl FakeKeychain {
        pub fn broken() -> Arc<Self> {
            let k = Arc::new(FakeKeychain::default());
            k.broken.store(true, Ordering::SeqCst);
            k
        }
        pub fn has(&self, account: &str) -> Option<String> {
            self.items.lock().unwrap().get(account).cloned()
        }
    }

    impl KeyStore for Arc<FakeKeychain> {
        fn get(&self, account: &str) -> Result<Option<String>, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.broken.load(Ordering::SeqCst) {
                return Err("no secret service".into());
            }
            Ok(self.items.lock().unwrap().get(account).cloned())
        }
        fn set(&self, account: &str, secret: &str) -> Result<(), String> {
            if self.broken.load(Ordering::SeqCst) || self.deny_writes.load(Ordering::SeqCst) {
                return Err("denied".into());
            }
            self.items
                .lock()
                .unwrap()
                .insert(account.into(), secret.into());
            Ok(())
        }
        fn delete(&self, account: &str) -> Result<(), String> {
            if self.broken.load(Ordering::SeqCst) {
                return Err("no secret service".into());
            }
            self.items.lock().unwrap().remove(account);
            Ok(())
        }
    }

    pub fn state() -> AppState {
        AppState::new(Ok(posthaste_core::Db::open_in_memory().unwrap()))
    }

    #[test]
    fn keys_go_to_a_working_keychain_and_never_the_database() {
        let keychain = Arc::new(FakeKeychain::default());
        let secrets = Secrets::with_keychain(Box::new(keychain.clone()));
        let state = state();
        assert_eq!(secrets.backend(), Backend::Keychain);

        secrets.set(&state, "ai:openai", "sk-1").unwrap();
        assert_eq!(
            secrets.get(&state, "ai:openai").unwrap().as_deref(),
            Some("sk-1")
        );
        assert_eq!(keychain.has("ai:openai").as_deref(), Some("sk-1"));
        assert_eq!(
            state.with_db(|db| db.secret_get("ai:openai")).unwrap(),
            None
        );

        secrets.delete(&state, "ai:openai").unwrap();
        assert_eq!(secrets.get(&state, "ai:openai").unwrap(), None);
    }

    #[test]
    fn without_a_keychain_keys_fall_back_to_the_database() {
        let secrets = Secrets::with_keychain(Box::new(FakeKeychain::broken()));
        let state = state();
        assert_eq!(secrets.backend(), Backend::Database);

        secrets.set(&state, "ai:gemini", "g-1").unwrap();
        assert_eq!(
            secrets.get(&state, "ai:gemini").unwrap().as_deref(),
            Some("g-1")
        );
        secrets.delete(&state, "ai:gemini").unwrap();
        assert_eq!(secrets.get(&state, "ai:gemini").unwrap(), None);
    }

    #[test]
    fn deleting_also_clears_a_key_left_in_the_database() {
        let keychain = Arc::new(FakeKeychain::default());
        let secrets = Secrets::with_keychain(Box::new(keychain));
        let state = state();
        state
            .with_db(|db| db.secret_set("ai:openai", "old"))
            .unwrap();
        secrets.delete(&state, "ai:openai").unwrap();
        assert_eq!(
            state.with_db(|db| db.secret_get("ai:openai")).unwrap(),
            None
        );
    }

    #[test]
    fn a_key_left_in_the_database_is_still_found_when_a_keychain_appears() {
        let state = state();
        state
            .with_db(|db| db.secret_set("ai:openai", "sk-db"))
            .unwrap();
        let keychain = Arc::new(FakeKeychain::default());
        let secrets = Secrets::with_keychain(Box::new(keychain.clone()));
        assert_eq!(
            secrets.get(&state, "ai:openai").unwrap().as_deref(),
            Some("sk-db")
        );
    }

    #[test]
    fn keys_from_the_database_move_into_a_keychain_that_appears() {
        let state = state();
        state
            .with_db(|db| db.secret_set("ai:openai", "sk-db"))
            .unwrap();
        state
            .with_db(|db| db.secret_set("ai:gemini", "g-db"))
            .unwrap();
        let keychain = Arc::new(FakeKeychain::default());
        // A newer keychain key is kept; the database copy is dropped.
        keychain
            .items
            .lock()
            .unwrap()
            .insert("ai:gemini".into(), "g-new".into());
        let secrets = Secrets::with_keychain(Box::new(keychain.clone()));

        secrets.adopt_database_keys(&state, &["ai:openai", "ai:gemini", "ai:custom"]);

        assert_eq!(keychain.has("ai:openai").as_deref(), Some("sk-db"));
        assert_eq!(keychain.has("ai:gemini").as_deref(), Some("g-new"));
        for account in ["ai:openai", "ai:gemini"] {
            assert_eq!(
                state.with_db(|db| db.secret_get(account)).unwrap(),
                None,
                "{account}"
            );
        }
    }

    #[test]
    fn a_database_key_stays_put_if_the_keychain_refuses_it() {
        let state = state();
        state
            .with_db(|db| db.secret_set("ai:openai", "sk-db"))
            .unwrap();
        let keychain = Arc::new(FakeKeychain::default());
        keychain.deny_writes.store(true, Ordering::SeqCst);
        let secrets = Secrets::with_keychain(Box::new(keychain));
        secrets.adopt_database_keys(&state, &["ai:openai"]);
        assert_eq!(
            state
                .with_db(|db| db.secret_get("ai:openai"))
                .unwrap()
                .as_deref(),
            Some("sk-db")
        );
    }

    #[test]
    fn a_keychain_that_starts_late_is_used_for_the_next_save() {
        // Launch at login can start the app before the keyring daemon.
        let state = state();
        let keychain = FakeKeychain::broken();
        let secrets = Secrets::with_keychain(Box::new(keychain.clone()));
        assert_eq!(secrets.backend(), Backend::Database);

        keychain.broken.store(false, Ordering::SeqCst);
        secrets.set(&state, "ai:openai", "sk-1").unwrap();

        assert_eq!(secrets.backend(), Backend::Keychain);
        assert_eq!(keychain.has("ai:openai").as_deref(), Some("sk-1"));
        assert_eq!(
            state.with_db(|db| db.secret_get("ai:openai")).unwrap(),
            None
        );
    }
}
