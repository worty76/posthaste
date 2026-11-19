//! Pure sync engine: per-field hybrid logical clocks, device identity, and
//! peer metadata. No network or async code lives here.

pub mod hlc;
pub mod merge;
pub(crate) mod record;
pub mod store;

pub use hlc::Hlc;
pub use merge::{ApplyReport, Change, CONFLICT_SOURCE};
pub use store::{Field, Peer};

#[cfg(test)]
mod tests;
