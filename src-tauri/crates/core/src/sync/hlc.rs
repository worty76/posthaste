//! Hybrid Logical Clock (HLC) for total, monotonic ordering of per-field edits
//! across devices without requiring synchronised wall clocks.

use std::fmt;

/// A hybrid logical clock timestamp.
///
/// Serialised as `{wall_ms:013}-{counter:06}-{device}` so plain string
/// comparison matches causal and tie-break order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hlc {
    pub wall_ms: i64,
    pub counter: u32,
    pub device: String,
}

impl Hlc {
    pub fn new(wall_ms: i64, counter: u32, device: impl Into<String>) -> Self {
        Self {
            wall_ms: wall_ms.max(0),
            counter,
            device: device.into(),
        }
    }

    /// Parses a formatted HLC string (`{wall_ms:013}-{counter:06}-{device}`).
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.splitn(3, '-');
        let wall_ms: i64 = parts.next()?.parse().ok()?;
        let counter: u32 = parts.next()?.parse().ok()?;
        let device = parts.next()?;
        if device.is_empty() || wall_ms < 0 {
            return None;
        }
        Some(Self {
            wall_ms,
            counter,
            device: device.to_string(),
        })
    }

    /// Advances the local clock for a new local event. Never moves backwards
    /// even if the system wall clock jumps into the past.
    pub fn tick(last: Option<&Hlc>, now_ms: i64, device: &str) -> Self {
        let now_ms = now_ms.max(0);
        match last {
            Some(prev) if now_ms <= prev.wall_ms => Self {
                wall_ms: prev.wall_ms,
                counter: prev.counter.saturating_add(1),
                device: device.to_string(),
            },
            _ => Self {
                wall_ms: now_ms,
                counter: 0,
                device: device.to_string(),
            },
        }
    }

    /// Standard HLC receive rule: merges an incoming remote timestamp into the
    /// local clock so subsequent local events sort strictly after both `last`
    /// and `incoming`.
    pub fn observe(last: Option<&Hlc>, incoming: &Hlc, now_ms: i64, device: &str) -> Self {
        let now_ms = now_ms.max(0);
        let prev_wall = last.map(|l| l.wall_ms).unwrap_or(0);
        let prev_counter = last.map(|l| l.counter).unwrap_or(0);
        let max_wall = now_ms.max(prev_wall).max(incoming.wall_ms);

        let counter = if max_wall == prev_wall && max_wall == incoming.wall_ms {
            prev_counter.max(incoming.counter).saturating_add(1)
        } else if max_wall == prev_wall {
            prev_counter.saturating_add(1)
        } else if max_wall == incoming.wall_ms {
            incoming.counter.saturating_add(1)
        } else {
            0
        };

        Self {
            wall_ms: max_wall,
            counter,
            device: device.to_string(),
        }
    }

    pub fn format(&self) -> String {
        format!("{:013}-{:06}-{}", self.wall_ms, self.counter, self.device)
    }
}

impl fmt::Display for Hlc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:013}-{:06}-{}", self.wall_ms, self.counter, self.device)
    }
}
