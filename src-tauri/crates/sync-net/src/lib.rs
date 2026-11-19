//! Peer-to-peer QUIC transport for Posthaste device sync, built on iroh 1.x.
//!
//! Endpoints always start from `presets::Minimal` (no n0 DNS/Pkarr lookup) and
//! use either a user-configured custom relay URL or `RelayMode::Disabled`
//! (same-network only).

pub mod conn;
pub mod pairing;

use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

use iroh::endpoint::presets;
use iroh::{
    Endpoint, EndpointAddr, EndpointId, RelayMap, RelayMode, RelayUrl, SecretKey, TransportAddr,
};

pub use conn::Stream;
pub use pairing::{ActiveOffer, PairingOffer, OFFER_TTL_MS, PAIR_PREFIX};

pub const ALPN: &[u8] = b"posthaste/sync/1";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("Invalid relay address: {0}")]
    InvalidRelay(String),
    #[error("Invalid peer address: {0}")]
    InvalidPeer(String),
    #[error("Could not start sync network: {0}")]
    Bind(String),
    #[error("Could not reach the other computer: {0}")]
    Connect(String),
    #[error("Sync connection interrupted: {0}")]
    Io(String),
    #[error("Sync protocol error: {0}")]
    Protocol(String),
    #[error("{0}")]
    Pairing(String),
}

/// Addressing information used to dial a paired peer.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub device_id: String,
    pub addrs: Vec<String>,
    pub relay_url: Option<String>,
}

/// Running iroh endpoint owned by the shell's background sync service.
#[derive(Clone)]
pub struct Net {
    endpoint: Endpoint,
    relay_url: Option<String>,
}

impl Net {
    /// Starts an iroh endpoint with the given 32-byte Ed25519 secret key and
    /// optional custom relay URL. Never uses n0's public relays or lookup.
    pub async fn start(secret_key: [u8; 32], relay_url: Option<&str>) -> Result<Self, NetError> {
        let trimmed = relay_url.map(str::trim).filter(|s| !s.is_empty());
        let (relay_mode, stored_relay) = match trimmed {
            None => (RelayMode::Disabled, None),
            Some(url_str) => {
                let parsed: RelayUrl = url_str
                    .parse()
                    .map_err(|e: iroh::RelayUrlParseError| NetError::InvalidRelay(e.to_string()))?;
                (
                    RelayMode::Custom(RelayMap::from(parsed)),
                    Some(url_str.to_string()),
                )
            }
        };

        let key = SecretKey::from_bytes(&secret_key);
        let endpoint = Endpoint::builder(presets::Minimal)
            .relay_mode(relay_mode)
            .secret_key(key)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| NetError::Bind(e.to_string()))?;

        Ok(Self {
            endpoint,
            relay_url: stored_relay,
        })
    }

    /// Generates a fresh 32-byte Ed25519 secret key using iroh's CSPRNG.
    pub fn generate_secret_key() -> [u8; 32] {
        let key = SecretKey::generate();
        key.to_bytes()
    }

    /// Generates a random 16-byte pairing secret.
    pub fn generate_pair_secret() -> [u8; 16] {
        let key = SecretKey::generate().to_bytes();
        let mut secret = [0u8; 16];
        secret.copy_from_slice(&key[..16]);
        secret
    }

    /// This endpoint's public key string (`EndpointId`).
    pub fn device_id(&self) -> String {
        self.endpoint.id().to_string()
    }

    /// Short 8-character prefix safe for logs.
    pub fn short_id(device_id: &str) -> &str {
        &device_id[..device_id.len().min(8)]
    }

    /// Currently bound direct IP:port addresses for inclusion in pairing offers.
    pub fn local_addrs(&self) -> Vec<String> {
        self.endpoint
            .addr()
            .addrs
            .iter()
            .filter_map(|a| match a {
                TransportAddr::Ip(sock) => Some(sock.to_string()),
                _ => None,
            })
            .collect()
    }

    /// Configured custom relay URL, if any.
    pub fn relay_url(&self) -> Option<&str> {
        self.relay_url.as_deref()
    }

    /// Dials `peer` on `ALPN`, opens a bidirectional stream, and runs `run`.
    pub async fn sync_with<F, Fut, T>(&self, peer: &PeerInfo, run: F) -> Result<T, NetError>
    where
        F: FnOnce(Stream) -> Fut,
        Fut: Future<Output = Result<T, NetError>>,
    {
        let addr = build_endpoint_addr(peer, self.relay_url.as_deref())?;
        let conn = tokio::time::timeout(CONNECT_TIMEOUT, self.endpoint.connect(addr, ALPN))
            .await
            .map_err(|_| NetError::Connect("connection timed out".into()))?
            .map_err(|e| NetError::Connect(e.to_string()))?;

        let remote_id = conn.remote_id().to_string();
        let (send, recv) = conn
            .open_bi()
            .await
            .map_err(|e| NetError::Io(e.to_string()))?;
        let stream = Stream::new(remote_id, conn, send, recv);
        run(stream).await
    }

    /// Accepts the next incoming connection on `ALPN`, returning `None` once
    /// the endpoint is closed.
    pub async fn accept(&self) -> Option<Result<Stream, NetError>> {
        let incoming = self.endpoint.accept().await?;
        let connecting = match incoming.accept() {
            Ok(c) => c,
            Err(e) => return Some(Err(NetError::Connect(e.to_string()))),
        };
        let conn = match connecting.await {
            Ok(c) => c,
            Err(e) => return Some(Err(NetError::Connect(e.to_string()))),
        };
        let remote_id = conn.remote_id().to_string();
        let (send, recv) = match conn.accept_bi().await {
            Ok(pair) => pair,
            Err(e) => return Some(Err(NetError::Io(e.to_string()))),
        };
        Some(Ok(Stream::new(remote_id, conn, send, recv)))
    }

    /// Closes the endpoint and stops listening for incoming connections.
    pub async fn close(self) {
        self.endpoint.close().await;
    }
}

fn build_endpoint_addr(
    peer: &PeerInfo,
    fallback_relay: Option<&str>,
) -> Result<EndpointAddr, NetError> {
    let id: EndpointId = peer
        .device_id
        .parse()
        .map_err(|e: iroh::KeyParsingError| NetError::InvalidPeer(e.to_string()))?;

    let mut transport_addrs = Vec::new();
    for raw in &peer.addrs {
        if let Ok(sock) = raw.parse::<SocketAddr>() {
            transport_addrs.push(TransportAddr::Ip(sock));
        }
    }
    let relay = peer
        .relay_url
        .as_deref()
        .or(fallback_relay)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(url_str) = relay {
        if let Ok(url) = url_str.parse::<RelayUrl>() {
            transport_addrs.push(TransportAddr::Relay(url));
        }
    }

    Ok(EndpointAddr::from_parts(id, transport_addrs))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use posthaste_core::sync::Session;
    use posthaste_core::{Db, NewNote};

    use super::*;
    use crate::pairing::{ActiveOffer, PairingOffer, OFFER_TTL_MS};

    #[test]
    fn a_pairing_code_round_trips() {
        let offer = PairingOffer {
            device_id: "abcdef1234567890".into(),
            addrs: vec!["127.0.0.1:4242".into()],
            relay_url: Some("https://relay.posthaste.ink".into()),
            secret: [7u8; 16],
            expires_at_ms: 1_000_000 + OFFER_TTL_MS,
        };
        let code = offer.encode().unwrap();
        assert!(code.starts_with(PAIR_PREFIX));
        let decoded = PairingOffer::decode(&code, 1_000_000).unwrap();
        assert_eq!(decoded, offer);
    }

    #[test]
    fn an_expired_or_foreign_code_is_refused() {
        assert!(PairingOffer::decode("not-a-code", 1_000).is_err());
        assert!(PairingOffer::decode("posthaste-look:2:AAAA", 1_000).is_err());

        let offer = PairingOffer {
            device_id: "dev-1".into(),
            addrs: vec![],
            relay_url: None,
            secret: [3u8; 16],
            expires_at_ms: 5_000,
        };
        let code = offer.encode().unwrap();
        assert!(PairingOffer::decode(&code, 5_001).is_err());
    }

    #[test]
    fn an_offer_works_once() {
        let offer = PairingOffer {
            device_id: "dev-1".into(),
            addrs: vec![],
            relay_url: None,
            secret: [9u8; 16],
            expires_at_ms: 10_000,
        };
        let secret = offer.secret_hex();
        let mut active = ActiveOffer::default();
        active.set(offer);

        assert!(!active.take_if_valid("0000", 5_000));
        assert!(active.take_if_valid(&secret, 5_000));
        // Second use is refused.
        assert!(!active.take_if_valid(&secret, 5_001));
    }

    #[tokio::test]
    async fn two_nets_sync_in_memory_dbs_over_localhost() {
        let net_a = Net::start([1u8; 32], None).await.unwrap();
        let net_b = Net::start([2u8; 32], None).await.unwrap();
        let id_a = net_a.device_id();
        let id_b = net_b.device_id();

        let mut db_a = Db::open_in_memory().unwrap();
        let mut db_b = Db::open_in_memory().unwrap();
        db_a.set_sync_device_id(&id_a).unwrap();
        db_b.set_sync_device_id(&id_b).unwrap();
        db_a.upsert_peer(&id_b, "B", &net_b.local_addrs(), 1_000)
            .unwrap();
        db_b.upsert_peer(&id_a, "A", &net_a.local_addrs(), 1_000)
            .unwrap();

        let na = db_a
            .create_note_at(
                NewNote {
                    content: "from A over iroh".into(),
                    category: None,
                    source: None,
                },
                2_000,
            )
            .unwrap();
        let nb = db_b
            .create_note_at(
                NewNote {
                    content: "from B over iroh".into(),
                    category: None,
                    source: None,
                },
                2_100,
            )
            .unwrap();

        let db_b_arc = Arc::new(Mutex::new(db_b));
        let db_b_srv = db_b_arc.clone();
        let net_b_srv = net_b.clone();
        let id_b_srv = id_b.clone();

        let server = tokio::spawn(async move {
            let mut stream = net_b_srv.accept().await.unwrap().unwrap();
            let remote = stream.remote_id().to_string();
            let mut session = Session::new(&id_b_srv, &remote).with_name("B");
            let initial = {
                let db = db_b_srv.lock().unwrap();
                session.start(&db).unwrap()
            };
            for msg in initial {
                stream.send(&msg).await.unwrap();
            }
            while !session.is_finished() {
                let Some(msg) = stream.recv().await.unwrap() else {
                    break;
                };
                let replies = {
                    let mut db = db_b_srv.lock().unwrap();
                    session.on_message(&mut db, msg, 3_000).unwrap()
                };
                for r in replies {
                    stream.send(&r).await.unwrap();
                }
            }
            stream.finish_and_close().await;
        });

        let peer_b = PeerInfo {
            device_id: id_b.clone(),
            addrs: net_b.local_addrs(),
            relay_url: None,
        };
        let db_a_arc = Arc::new(Mutex::new(db_a));
        let db_a_cli = db_a_arc.clone();
        net_a
            .sync_with(&peer_b, |mut stream| async move {
                let remote = stream.remote_id().to_string();
                let mut session = Session::new(&id_a, &remote).with_name("A");
                let initial = {
                    let db = db_a_cli.lock().unwrap();
                    session.start(&db).unwrap()
                };
                for msg in initial {
                    stream.send(&msg).await?;
                }
                while !session.is_finished() {
                    let Some(msg) = stream.recv().await? else {
                        break;
                    };
                    let replies = {
                        let mut db = db_a_cli.lock().unwrap();
                        session.on_message(&mut db, msg, 3_000).unwrap()
                    };
                    for r in replies {
                        stream.send(&r).await?;
                    }
                }
                stream.finish_and_close().await;
                Ok(())
            })
            .await
            .unwrap();

        server.await.unwrap();
        net_a.close().await;
        net_b.close().await;

        assert_eq!(
            db_a_arc.lock().unwrap().get_note(&nb.id).unwrap().content,
            "from B over iroh"
        );
        assert_eq!(
            db_b_arc.lock().unwrap().get_note(&na.id).unwrap().content,
            "from A over iroh"
        );
    }
}
