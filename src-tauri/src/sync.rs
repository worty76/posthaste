//! Background peer-to-peer device sync service in the desktop shell.
//!
//! Owns the running `posthaste_sync_net::Net` endpoint (when enabled), accepts
//! incoming connections, periodically syncs with paired devices, and debounces
//! note-change triggers (2 s). All SQLite access goes through `AppState::with_db`
//! and is never held across an `.await`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use posthaste_core::sync::Session;
use posthaste_core::{now_ms, SyncDevice, SyncStatus, DEFAULT_SYNC_DEVICE_NAME};
use posthaste_sync_net::pairing::{hex_decode, hex_encode};
use posthaste_sync_net::{ActiveOffer, Net, PairingOffer, PeerInfo, Stream, OFFER_TTL_MS};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::error::{AppError, AppResult};
use crate::state::{lock, AppState};

const KEYCHAIN_ACCOUNT: &str = "sync-device-key";
const PERIODIC_INTERVAL: Duration = Duration::from_secs(60);
const DEBOUNCE_DELAY: Duration = Duration::from_secs(2);

struct Running {
    net: Net,
    relay_url: String,
    stop: Arc<AtomicBool>,
    accept_task: tauri::async_runtime::JoinHandle<()>,
    sync_task: tauri::async_runtime::JoinHandle<()>,
}

/// Debounce state for coalescing bursts of note writes into one sync pass.
#[derive(Debug, Default)]
pub(crate) struct Debouncer {
    pending_since: Option<Instant>,
}

impl Debouncer {
    pub fn mark(&mut self, now: Instant) {
        self.pending_since = Some(now);
    }

    pub fn due(&self, now: Instant, delay: Duration) -> bool {
        self.pending_since
            .is_some_and(|t| now.saturating_duration_since(t) >= delay)
    }

    pub fn take_if_due(&mut self, now: Instant, delay: Duration) -> bool {
        if self.due(now, delay) {
            self.pending_since = None;
            true
        } else {
            false
        }
    }

    pub fn is_pending(&self) -> bool {
        self.pending_since.is_some()
    }
}

/// Managed Tauri state for the background sync service.
pub struct SyncService {
    inner: tokio::sync::Mutex<Option<Running>>,
    active_offer: Arc<Mutex<ActiveOffer>>,
    online_peers: Arc<Mutex<HashSet<String>>>,
    last_error: Arc<Mutex<Option<String>>>,
    wake: Arc<Notify>,
    debouncer: Arc<Mutex<Debouncer>>,
}

impl Default for SyncService {
    fn default() -> Self {
        Self {
            inner: tokio::sync::Mutex::new(None),
            active_offer: Arc::new(Mutex::new(ActiveOffer::default())),
            online_peers: Arc::new(Mutex::new(HashSet::new())),
            last_error: Arc::new(Mutex::new(None)),
            wake: Arc::new(Notify::new()),
            debouncer: Arc::new(Mutex::new(Debouncer::default())),
        }
    }
}

impl SyncService {
    /// True when the iroh endpoint is currently running.
    #[cfg(test)]
    pub async fn is_running(&self) -> bool {
        self.inner.lock().await.is_some()
    }

    /// Starts sync at app launch if `settings.sync_enabled` is true.
    pub fn start_if_enabled(app: &AppHandle) {
        let state = app.state::<AppState>();
        if !state.settings().sync_enabled {
            return;
        }
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let svc = handle.state::<SyncService>();
            if let Err(e) = svc.ensure_started(&handle).await {
                log::warn!("sync could not start: {}", e.code);
                svc.set_error(Some(e.message));
                emit_sync_changed(&handle);
            }
        });
    }

    /// Called whenever a local note write succeeds; schedules a debounced sync
    /// 2 seconds later if sync is running.
    pub fn notify_note_changed(&self) {
        lock(&self.debouncer).mark(Instant::now());
        self.wake.notify_one();
    }

    /// Applies a relay URL change while sync is enabled (restarts endpoint).
    pub async fn reconfigure(&self, app: &AppHandle) -> AppResult<()> {
        let enabled = app.state::<AppState>().settings().sync_enabled;
        if !enabled {
            self.stop().await;
            emit_sync_changed(app);
            return Ok(());
        }
        let wanted_relay = app.state::<AppState>().settings().sync_relay_url;
        let needs_restart = {
            let guard = self.inner.lock().await;
            match guard.as_ref() {
                Some(r) => r.relay_url != wanted_relay,
                None => true,
            }
        };
        if needs_restart {
            self.stop().await;
            self.ensure_started(app).await?;
        }
        emit_sync_changed(app);
        Ok(())
    }

    /// Stops the iroh endpoint and background tasks. Nothing listens when off.
    pub async fn stop(&self) {
        let running = self.inner.lock().await.take();
        if let Some(r) = running {
            r.stop.store(true, Ordering::SeqCst);
            r.accept_task.abort();
            r.sync_task.abort();
            r.net.close().await;
            log::info!("sync stopped");
        }
        lock(&self.active_offer).clear();
        lock(&self.online_peers).clear();
    }

    async fn ensure_started(&self, app: &AppHandle) -> AppResult<Net> {
        {
            let guard = self.inner.lock().await;
            if let Some(r) = guard.as_ref() {
                return Ok(r.net.clone());
            }
        }

        let secret_key = load_or_create_secret_key(app).await?;
        let relay_url = app.state::<AppState>().settings().sync_relay_url;
        let net = Net::start(secret_key, Some(relay_url.as_str()))
            .await
            .map_err(|e| AppError::new("sync_start", e.to_string()))?;

        let device_id = net.device_id();
        app.state::<AppState>()
            .with_db(|db| db.set_sync_device_id(&device_id))?;

        let stop = Arc::new(AtomicBool::new(false));
        let accept_task = spawn_accept_loop(
            app.clone(),
            net.clone(),
            self.active_offer.clone(),
            self.online_peers.clone(),
            self.last_error.clone(),
            stop.clone(),
        );
        let sync_task = spawn_periodic_loop(
            app.clone(),
            net.clone(),
            self.online_peers.clone(),
            self.last_error.clone(),
            self.wake.clone(),
            self.debouncer.clone(),
            stop.clone(),
        );

        log::info!("sync started for device {}", Net::short_id(&device_id));
        *self.inner.lock().await = Some(Running {
            net: net.clone(),
            relay_url,
            stop,
            accept_task,
            sync_task,
        });
        self.set_error(None);
        // Trigger an immediate startup sync pass.
        self.wake.notify_one();
        Ok(net)
    }

    pub async fn status(&self, app: &AppHandle) -> AppResult<SyncStatus> {
        let state = app.state::<AppState>();
        let enabled = state.settings().sync_enabled;
        let (device_id, peers) =
            state.with_db(|db| Ok((db.sync_device_id()?, db.list_peers()?)))?;
        let online = lock(&self.online_peers).clone();
        let devices = peers
            .into_iter()
            .map(|p| SyncDevice {
                online: online.contains(&p.device_id),
                id: p.device_id,
                name: p.name,
                last_sync_at: p.last_sync_at,
            })
            .collect();
        let last_error = lock(&self.last_error).clone();
        Ok(SyncStatus {
            enabled,
            device_id,
            devices,
            last_error,
        })
    }

    pub async fn set_enabled(&self, app: &AppHandle, on: bool) -> AppResult<SyncStatus> {
        let mut settings = app.state::<AppState>().settings();
        if settings.sync_device_name.trim().is_empty() {
            settings.sync_device_name = default_device_name();
        }
        settings.sync_enabled = on;
        app.state::<AppState>()
            .with_db(|db| db.save_settings(&settings))?;
        *app.state::<AppState>()
            .settings
            .write()
            .unwrap_or_else(|e| e.into_inner()) = settings.clone();
        let _ = app.emit("settings-changed", crate::ai::for_interface(&settings));

        if on {
            if let Err(e) = self.ensure_started(app).await {
                self.set_error(Some(e.message.clone()));
                emit_sync_changed(app);
                return Err(e);
            }
        } else {
            self.stop().await;
            self.set_error(None);
        }
        emit_sync_changed(app);
        self.status(app).await
    }

    pub async fn create_pair_offer(&self, app: &AppHandle) -> AppResult<String> {
        let net = self.ensure_started(app).await?;
        let now = now_ms();
        let offer = PairingOffer {
            device_id: net.device_id(),
            addrs: net.local_addrs(),
            relay_url: net.relay_url().map(str::to_string),
            secret: Net::generate_pair_secret(),
            expires_at_ms: now + OFFER_TTL_MS,
        };
        let code = offer
            .encode()
            .map_err(|e| AppError::new("sync_pair", e.to_string()))?;
        lock(&self.active_offer).set(offer);
        Ok(code)
    }

    pub async fn join_pair_code(&self, app: &AppHandle, code: &str) -> AppResult<SyncDevice> {
        let now = now_ms();
        let offer = PairingOffer::decode(code, now)
            .map_err(|e| AppError::new("sync_pair", e.to_string()))?;
        let net = self.ensure_started(app).await?;
        let local_id = net.device_id();
        if offer.device_id == local_id {
            return Err(AppError::new(
                "sync_pair",
                "That pairing code is from this computer.",
            ));
        }

        let peer_info = PeerInfo {
            device_id: offer.device_id.clone(),
            addrs: offer.addrs.clone(),
            relay_url: offer.relay_url.clone(),
        };
        let secret_hex = offer.secret_hex();
        let local_name = device_display_name(app);

        // Pre-record the peer locally so the outgoing session can exchange notes
        // once the remote accepts our pairing secret; roll back on failure.
        app.state::<AppState>().with_db(|db| {
            db.upsert_peer(&offer.device_id, "Other computer", &offer.addrs, now)
        })?;

        let result = net
            .sync_with(&peer_info, |stream| {
                run_session(
                    app.clone(),
                    stream,
                    local_id.clone(),
                    local_name,
                    Some(secret_hex),
                    None,
                )
            })
            .await;

        match result {
            Ok(outcome) => {
                if let Some(reason) = outcome.refused {
                    let _ = app
                        .state::<AppState>()
                        .with_db(|db| db.remove_peer(&offer.device_id));
                    return Err(AppError::new("sync_refused", reason));
                }
                let peer_name = outcome
                    .peer_name
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "Other computer".into());
                let peer = app.state::<AppState>().with_db(|db| {
                    db.upsert_peer(&offer.device_id, &peer_name, &offer.addrs, now)
                })?;
                lock(&self.online_peers).insert(peer.device_id.clone());
                self.set_error(None);
                if outcome.applied > 0 {
                    let _ = app.emit("notes-changed", ());
                }
                emit_sync_changed(app);
                Ok(SyncDevice {
                    id: peer.device_id,
                    name: peer.name,
                    last_sync_at: peer.last_sync_at,
                    online: true,
                })
            }
            Err(e) => {
                let _ = app
                    .state::<AppState>()
                    .with_db(|db| db.remove_peer(&offer.device_id));
                let msg = e.to_string();
                self.set_error(Some(msg.clone()));
                emit_sync_changed(app);
                Err(AppError::new("sync_pair", msg))
            }
        }
    }

    pub async fn sync_now(&self, app: &AppHandle) -> AppResult<()> {
        let net = {
            let guard = self.inner.lock().await;
            guard.as_ref().map(|r| r.net.clone())
        };
        let Some(net) = net else {
            return Err(AppError::new("sync_disabled", "Sync is turned off."));
        };
        sync_all_peers(app, &net, &self.online_peers, &self.last_error).await;
        emit_sync_changed(app);
        Ok(())
    }

    pub async fn forget(&self, app: &AppHandle, id: &str) -> AppResult<()> {
        app.state::<AppState>()
            .with_db(|db| db.remove_peer(id))?;
        lock(&self.online_peers).remove(id);
        emit_sync_changed(app);
        Ok(())
    }

    fn set_error(&self, err: Option<String>) {
        *lock(&self.last_error) = err;
    }
}

struct SessionOutcome {
    applied: usize,
    refused: Option<String>,
    peer_name: Option<String>,
}

async fn run_session(
    app: AppHandle,
    mut stream: Stream,
    local_id: String,
    local_name: String,
    pair_secret: Option<String>,
    active_offer: Option<Arc<Mutex<ActiveOffer>>>,
) -> Result<SessionOutcome, posthaste_sync_net::NetError> {
    let remote_id = stream.remote_id().to_string();
    let mut session = Session::new(&local_id, &remote_id).with_name(local_name);
    if let Some(secret) = pair_secret {
        session = session.with_pair_secret(secret);
    }

    let initial = app
        .state::<AppState>()
        .with_db(|db| session.start(db))
        .map_err(|e| posthaste_sync_net::NetError::Protocol(e.message))?;
    for msg in initial {
        stream.send(&msg).await?;
    }

    while !session.is_finished() {
        let Some(msg) = stream.recv().await? else {
            break;
        };
        let now = now_ms();
        let offer_ref = active_offer.clone();
        let replies = app
            .state::<AppState>()
            .with_db(|db| {
                session.on_message_with_pairing(db, msg, now, |_, secret| {
                    offer_ref
                        .as_ref()
                        .is_some_and(|cell| lock(cell).take_if_valid(secret, now))
                })
            })
            .map_err(|e| posthaste_sync_net::NetError::Protocol(e.message))?;
        for reply in replies {
            stream.send(&reply).await?;
        }
    }

    stream.finish_and_close().await;
    Ok(SessionOutcome {
        applied: session.total_applied(),
        refused: session.refused_reason().map(str::to_string),
        peer_name: session.peer_name().map(str::to_string),
    })
}

fn spawn_accept_loop(
    app: AppHandle,
    net: Net,
    active_offer: Arc<Mutex<ActiveOffer>>,
    online_peers: Arc<Mutex<HashSet<String>>>,
    last_error: Arc<Mutex<Option<String>>>,
    stop: Arc<AtomicBool>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        while !stop.load(Ordering::SeqCst) {
            let Some(incoming) = net.accept().await else {
                break;
            };
            let stream = match incoming {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("incoming sync connection failed: {e}");
                    continue;
                }
            };
            let remote_id = stream.remote_id().to_string();
            let app_c = app.clone();
            let local_id = net.device_id();
            let local_name = device_display_name(&app);
            let offer_c = active_offer.clone();
            let online_c = online_peers.clone();
            let err_c = last_error.clone();

            tauri::async_runtime::spawn(async move {
                match run_session(
                    app_c.clone(),
                    stream,
                    local_id,
                    local_name,
                    None,
                    Some(offer_c),
                )
                .await
                {
                    Ok(outcome) => {
                        if let Some(reason) = outcome.refused {
                            log::info!(
                                "refused sync from {}: {reason}",
                                Net::short_id(&remote_id)
                            );
                        } else {
                            lock(&online_c).insert(remote_id.clone());
                            *lock(&err_c) = None;
                            if outcome.applied > 0 {
                                log::info!(
                                    "applied {} change(s) from {}",
                                    outcome.applied,
                                    Net::short_id(&remote_id)
                                );
                                let _ = app_c.emit("notes-changed", ());
                            }
                            emit_sync_changed(&app_c);
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "sync with {} interrupted: {e}",
                            Net::short_id(&remote_id)
                        );
                    }
                }
            });
        }
    })
}

fn spawn_periodic_loop(
    app: AppHandle,
    net: Net,
    online_peers: Arc<Mutex<HashSet<String>>>,
    last_error: Arc<Mutex<Option<String>>>,
    wake: Arc<Notify>,
    debouncer: Arc<Mutex<Debouncer>>,
    stop: Arc<AtomicBool>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        // Initial startup sync.
        sync_all_peers(&app, &net, &online_peers, &last_error).await;
        emit_sync_changed(&app);

        let mut last_periodic = Instant::now();
        while !stop.load(Ordering::SeqCst) {
            let pending = lock(&debouncer).is_pending();
            let wait = if pending {
                Duration::from_millis(250)
            } else {
                PERIODIC_INTERVAL
            };

            let _ = tokio::time::timeout(wait, wake.notified()).await;
            if stop.load(Ordering::SeqCst) {
                break;
            }

            let now = Instant::now();
            let debounced_due = lock(&debouncer).take_if_due(now, DEBOUNCE_DELAY);
            let periodic_due = now.saturating_duration_since(last_periodic) >= PERIODIC_INTERVAL;

            if debounced_due || periodic_due {
                last_periodic = now;
                sync_all_peers(&app, &net, &online_peers, &last_error).await;
                emit_sync_changed(&app);
            }
        }
    })
}

async fn sync_all_peers(
    app: &AppHandle,
    net: &Net,
    online_peers: &Arc<Mutex<HashSet<String>>>,
    last_error: &Arc<Mutex<Option<String>>>,
) {
    let peers = match app.state::<AppState>().with_db(|db| db.list_peers()) {
        Ok(p) => p,
        Err(_) => return,
    };
    if peers.is_empty() {
        return;
    }
    let local_id = net.device_id();
    let local_name = device_display_name(app);
    let mut any_error: Option<String> = None;

    for peer in peers {
        let info = PeerInfo {
            device_id: peer.device_id.clone(),
            addrs: peer.addrs.clone(),
            relay_url: net.relay_url().map(str::to_string),
        };
        let res = net
            .sync_with(&info, |stream| {
                run_session(
                    app.clone(),
                    stream,
                    local_id.clone(),
                    local_name.clone(),
                    None,
                    None,
                )
            })
            .await;

        match res {
            Ok(outcome) => {
                if let Some(reason) = outcome.refused {
                    lock(online_peers).remove(&peer.device_id);
                    any_error = Some(reason);
                } else {
                    lock(online_peers).insert(peer.device_id.clone());
                    if outcome.applied > 0 {
                        let _ = app.emit("notes-changed", ());
                    }
                }
            }
            Err(e) => {
                lock(online_peers).remove(&peer.device_id);
                log::info!(
                    "could not reach peer {}: {e}",
                    Net::short_id(&peer.device_id)
                );
                any_error = Some(e.to_string());
            }
        }
    }

    *lock(last_error) = any_error;
}

async fn load_or_create_secret_key(app: &AppHandle) -> AppResult<[u8; 32]> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        let secrets = handle.state::<crate::ai::SecretsCell>().wait();
        if let Some(hex) = secrets.get(&state, KEYCHAIN_ACCOUNT)? {
            if let Some(bytes) = hex_decode(hex.trim()) {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    return Ok(key);
                }
            }
        }
        let key = Net::generate_secret_key();
        secrets.set(&state, KEYCHAIN_ACCOUNT, &hex_encode(&key))?;
        Ok(key)
    })
    .await
    .map_err(|e| AppError::new("sync_key", e.to_string()))?
}

pub fn default_device_name() -> String {
    for var in ["COMPUTERNAME", "HOSTNAME"] {
        if let Ok(v) = std::env::var(var) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    DEFAULT_SYNC_DEVICE_NAME.to_string()
}

fn device_display_name(app: &AppHandle) -> String {
    let configured = app.state::<AppState>().settings().sync_device_name;
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        default_device_name()
    } else {
        trimmed.to_string()
    }
}

fn emit_sync_changed(app: &AppHandle) {
    let _ = app.emit("sync-changed", ());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debouncer_waits_for_quiet_period_and_coalesces_writes() {
        let mut d = Debouncer::default();
        let t0 = Instant::now();
        assert!(!d.is_pending());
        assert!(!d.take_if_due(t0, DEBOUNCE_DELAY));

        d.mark(t0);
        assert!(d.is_pending());
        assert!(!d.take_if_due(t0 + Duration::from_secs(1), DEBOUNCE_DELAY));

        // Another write resets the 2s quiet window.
        d.mark(t0 + Duration::from_secs(1));
        assert!(!d.take_if_due(t0 + Duration::from_secs(2), DEBOUNCE_DELAY));

        // 2s after the last write it fires once and clears.
        assert!(d.take_if_due(t0 + Duration::from_secs(3), DEBOUNCE_DELAY));
        assert!(!d.is_pending());
        assert!(!d.take_if_due(t0 + Duration::from_secs(10), DEBOUNCE_DELAY));
    }

    #[tokio::test]
    async fn disabled_service_has_no_running_endpoint() {
        let svc = SyncService::default();
        assert!(!svc.is_running().await);
        svc.stop().await;
        assert!(!svc.is_running().await);
    }
}
