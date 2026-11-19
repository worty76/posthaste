//! Single-use, short-lived pairing offers encoded as `posthaste-pair:1:<base32>`.

use serde::{Deserialize, Serialize};

use crate::NetError;

pub const PAIR_PREFIX: &str = "posthaste-pair:1:";
pub const OFFER_TTL_MS: i64 = 10 * 60 * 1000; // 10 minutes
const MAX_CODE_LEN: usize = 4096;

/// Payload carried inside a pairing code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingOffer {
    pub device_id: String,
    pub addrs: Vec<String>,
    pub relay_url: Option<String>,
    pub secret: [u8; 16],
    pub expires_at_ms: i64,
}

impl PairingOffer {
    /// Encodes this offer as `posthaste-pair:1:<base32-no-padding>`.
    pub fn encode(&self) -> Result<String, NetError> {
        let json = serde_json::to_vec(self).map_err(|e| NetError::Pairing(e.to_string()))?;
        Ok(format!("{PAIR_PREFIX}{}", base32_encode(&json)))
    }

    /// Decodes a pairing code and verifies prefix, format, and expiry at `now_ms`.
    pub fn decode(code: &str, now_ms: i64) -> Result<Self, NetError> {
        let cleaned: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        if cleaned.len() > MAX_CODE_LEN {
            return Err(NetError::Pairing("That pairing code is too long.".into()));
        }
        let Some(encoded) = cleaned.strip_prefix(PAIR_PREFIX) else {
            return Err(NetError::Pairing(
                "That isn't a Posthaste pairing code.".into(),
            ));
        };
        let bytes = base32_decode(encoded).ok_or_else(|| {
            NetError::Pairing("That pairing code is damaged or incomplete.".into())
        })?;
        let offer: PairingOffer = serde_json::from_slice(&bytes)
            .map_err(|_| NetError::Pairing("That pairing code could not be read.".into()))?;
        if offer.device_id.trim().is_empty() {
            return Err(NetError::Pairing(
                "That pairing code is missing a device id.".into(),
            ));
        }
        if now_ms > offer.expires_at_ms {
            return Err(NetError::Pairing(
                "That pairing code has expired. Generate a new one on the other computer.".into(),
            ));
        }
        Ok(offer)
    }

    /// Hex-encoded secret sent in `Hello.pair_secret`.
    pub fn secret_hex(&self) -> String {
        hex_encode(&self.secret)
    }
}

/// In-memory holder for one active, single-use pairing offer on the offering
/// device. Dropped automatically after one successful join or after 10 minutes.
#[derive(Debug, Default)]
pub struct ActiveOffer {
    offer: Option<PairingOffer>,
}

impl ActiveOffer {
    pub fn set(&mut self, offer: PairingOffer) {
        self.offer = Some(offer);
    }

    pub fn clear(&mut self) {
        self.offer = None;
    }

    /// Validates `secret_hex` against the active offer at `now_ms`.
    /// Consumes the offer on success so a code can only be used once.
    pub fn take_if_valid(&mut self, secret_hex: &str, now_ms: i64) -> bool {
        let Some(offer) = &self.offer else {
            return false;
        };
        if now_ms > offer.expires_at_ms {
            self.offer = None;
            return false;
        }
        let expected = offer.secret_hex();
        if constant_time_eq(expected.as_bytes(), secret_hex.as_bytes()) {
            self.offer = None;
            true
        } else {
            false
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let (chunks, _) = bytes.as_chunks::<2>();
    for chunk in chunks {
        let hi = from_hex_digit(chunk[0])?;
        let lo = from_hex_digit(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn from_hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

const B32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

fn base32_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() * 8).div_ceil(5));
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &b in data {
        buf = (buf << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buf >> bits) & 0x1f) as usize;
            out.push(B32_ALPHABET[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((buf << (5 - bits)) & 0x1f) as usize;
        out.push(B32_ALPHABET[idx] as char);
    }
    out
}

fn base32_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 5 / 8);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for b in input.bytes() {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a',
            b'2'..=b'7' => b - b'2' + 26,
            _ => return None,
        };
        buf = (buf << 5) | u32::from(val);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    if bits > 0 && (buf & ((1u32 << bits) - 1)) != 0 {
        return None;
    }
    Some(out)
}
