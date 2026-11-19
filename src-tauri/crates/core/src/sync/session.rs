//! Transport-agnostic sync session state machine and length-prefixed JSON
//! message framing.
//!
//! Batches are 500 changes. A side sends `Ack` only after `apply_changes` has
//! committed, and advances `sent_up_to` only when it receives `Ack`, so a
//! connection dropped mid-session resumes cleanly without duplicates or skips.

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::sync::merge::{ApplyReport, Change};
use crate::Db;

pub const PROTOCOL_VERSION: u32 = 1;
pub const BATCH_SIZE: usize = 500;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024; // 1 MiB

/// Wire messages exchanged over any reliable byte stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello {
        protocol: u32,
        device_id: String,
        name: String,
        pair_secret: Option<String>,
    },
    Batch {
        changes: Vec<Change>,
        up_to: i64,
        more: bool,
    },
    Ack {
        up_to: i64,
    },
    Done,
    Refused {
        reason: String,
    },
}

/// Encodes a message as a 4-byte big-endian length prefix followed by JSON.
pub fn encode_frame(msg: &Message) -> Result<Vec<u8>> {
    let body = serde_json::to_vec(msg)?;
    if body.len() > MAX_FRAME_BYTES {
        return Err(CoreError::InvalidImport(format!(
            "sync frame too large ({} bytes)",
            body.len()
        )));
    }
    let len = (body.len() as u32).to_be_bytes();
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&len);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Decodes a single frame from a byte slice, checking the 1 MiB size limit
/// before reading or parsing the body.
pub fn decode_frame(bytes: &[u8]) -> Result<Message> {
    if bytes.len() < 4 {
        return Err(CoreError::InvalidImport("sync frame header truncated".into()));
    }
    let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(CoreError::InvalidImport(format!(
            "sync frame exceeds {MAX_FRAME_BYTES} byte limit"
        )));
    }
    if bytes.len() < 4 + len {
        return Err(CoreError::InvalidImport("sync frame body truncated".into()));
    }
    Ok(serde_json::from_slice(&bytes[4..4 + len])?)
}

/// Checks a 4-byte big-endian frame length header against `MAX_FRAME_BYTES`.
pub fn check_frame_len(len: usize) -> Result<usize> {
    if len > MAX_FRAME_BYTES {
        return Err(CoreError::InvalidImport(format!(
            "sync frame exceeds {MAX_FRAME_BYTES} byte limit"
        )));
    }
    Ok(len)
}

/// Pure state machine for one bidirectional sync session with a single peer.
#[derive(Debug)]
pub struct Session {
    local_device: String,
    local_name: String,
    peer_device: String,
    pair_secret: Option<String>,
    peer_verified: bool,
    peer_name: Option<String>,
    next_send_since: i64,
    in_flight_up_to: Option<i64>,
    sent_done: bool,
    received_done: bool,
    refused: Option<String>,
    total_applied: usize,
    total_conflicts: usize,
}

impl Session {
    pub fn new(local_device: impl Into<String>, peer_device: impl Into<String>) -> Self {
        Self {
            local_device: local_device.into(),
            local_name: String::new(),
            peer_device: peer_device.into(),
            pair_secret: None,
            peer_verified: false,
            peer_name: None,
            next_send_since: 0,
            in_flight_up_to: None,
            sent_done: false,
            received_done: false,
            refused: None,
            total_applied: 0,
            total_conflicts: 0,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.local_name = name.into();
        self
    }

    pub fn with_pair_secret(mut self, secret: impl Into<String>) -> Self {
        self.pair_secret = Some(secret.into());
        self
    }

    pub fn is_finished(&self) -> bool {
        self.refused.is_some() || (self.sent_done && self.received_done)
    }

    pub fn refused_reason(&self) -> Option<&str> {
        self.refused.as_deref()
    }

    pub fn peer_name(&self) -> Option<&str> {
        self.peer_name.as_deref()
    }

    pub fn total_applied(&self) -> usize {
        self.total_applied
    }

    pub fn total_conflicts(&self) -> usize {
        self.total_conflicts
    }

    /// Starts the session by emitting our `Hello`. Does not read any notes
    /// until the peer's `Hello` is verified.
    pub fn start(&mut self, db: &Db) -> Result<Vec<Message>> {
        if let Some(peer) = db.get_peer(&self.peer_device)? {
            self.next_send_since = peer.sent_up_to;
        }
        Ok(vec![Message::Hello {
            protocol: PROTOCOL_VERSION,
            device_id: self.local_device.clone(),
            name: self.local_name.clone(),
            pair_secret: self.pair_secret.clone(),
        }])
    }

    /// Handles one incoming message from the peer and returns the outgoing
    /// messages to send next.
    pub fn on_message(
        &mut self,
        db: &mut Db,
        msg: Message,
        now_ms: i64,
    ) -> Result<Vec<Message>> {
        self.on_message_with_pairing(db, msg, now_ms, |_, _| false)
    }

    /// Like [`Session::on_message`], with a callback that validates a
    /// one-time pairing secret when an unpaired device presents one.
    pub fn on_message_with_pairing<F>(
        &mut self,
        db: &mut Db,
        msg: Message,
        now_ms: i64,
        mut verify_pair: F,
    ) -> Result<Vec<Message>>
    where
        F: FnMut(&str, &str) -> bool,
    {
        if self.refused.is_some() {
            return Ok(Vec::new());
        }

        match msg {
            Message::Hello {
                protocol,
                device_id,
                name,
                pair_secret,
            } => {
                if protocol != PROTOCOL_VERSION {
                    let reason = format!("unsupported protocol {protocol}");
                    self.refused = Some(reason.clone());
                    return Ok(vec![Message::Refused { reason }]);
                }
                if device_id != self.peer_device {
                    let reason = "device id mismatch".to_string();
                    self.refused = Some(reason.clone());
                    return Ok(vec![Message::Refused { reason }]);
                }

                let paired = db.is_paired(&device_id)?;
                if !paired {
                    let accepted = pair_secret
                        .as_deref()
                        .is_some_and(|secret| verify_pair(&device_id, secret));
                    if !accepted {
                        let reason = "device is not paired".to_string();
                        self.refused = Some(reason.clone());
                        return Ok(vec![Message::Refused { reason }]);
                    }
                    let display_name = if name.trim().is_empty() {
                        "Other computer"
                    } else {
                        name.trim()
                    };
                    db.upsert_peer(&device_id, display_name, &[], now_ms)?;
                }

                self.peer_verified = true;
                if !name.trim().is_empty() {
                    self.peer_name = Some(name);
                }
                if let Some(peer) = db.get_peer(&self.peer_device)? {
                    self.next_send_since = peer.sent_up_to;
                }

                self.next_send_message(db)
            }

            Message::Batch {
                changes,
                up_to,
                more: _,
            } => {
                if !self.peer_verified {
                    let reason = "missing hello".to_string();
                    self.refused = Some(reason.clone());
                    return Ok(vec![Message::Refused { reason }]);
                }
                let report: ApplyReport = db.apply_changes(&self.peer_device, &changes, now_ms)?;
                self.total_applied += report.applied;
                self.total_conflicts += report.conflicts;
                db.update_peer_sync_time(&self.peer_device, now_ms)?;
                Ok(vec![Message::Ack { up_to }])
            }

            Message::Ack { up_to } => {
                if !self.peer_verified {
                    let reason = "missing hello".to_string();
                    self.refused = Some(reason.clone());
                    return Ok(vec![Message::Refused { reason }]);
                }
                db.update_peer_sent_up_to(&self.peer_device, up_to, now_ms)?;
                self.next_send_since = self.next_send_since.max(up_to);
                self.in_flight_up_to = None;
                self.next_send_message(db)
            }

            Message::Done => {
                if !self.peer_verified {
                    let reason = "missing hello".to_string();
                    self.refused = Some(reason.clone());
                    return Ok(vec![Message::Refused { reason }]);
                }
                self.received_done = true;
                db.update_peer_sync_time(&self.peer_device, now_ms)?;
                Ok(Vec::new())
            }

            Message::Refused { reason } => {
                self.refused = Some(reason);
                Ok(Vec::new())
            }
        }
    }

    fn next_send_message(&mut self, db: &Db) -> Result<Vec<Message>> {
        if self.sent_done || self.in_flight_up_to.is_some() {
            return Ok(Vec::new());
        }
        // Request one extra row to know whether more changes follow.
        let (mut changes, _) = db.changes_since(self.next_send_since, BATCH_SIZE + 1)?;
        let more = changes.len() > BATCH_SIZE;
        if more {
            changes.truncate(BATCH_SIZE);
        }
        if changes.is_empty() {
            self.sent_done = true;
            return Ok(vec![Message::Done]);
        }
        // Re-query the exact up_to for the truncated batch size.
        let (batch, up_to) = db.changes_since(self.next_send_since, BATCH_SIZE)?;
        self.in_flight_up_to = Some(up_to);
        Ok(vec![Message::Batch {
            changes: batch,
            up_to,
            more,
        }])
    }
}
