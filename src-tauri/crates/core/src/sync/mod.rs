//! Pure sync engine: per-field hybrid logical clocks, device identity, and
//! peer metadata. No network or async code lives here.

pub mod hlc;
pub mod merge;
pub(crate) mod record;
pub mod session;
pub mod store;

pub use hlc::Hlc;
pub use merge::{ApplyReport, Change, CONFLICT_SOURCE};
pub use session::{
    check_frame_len, decode_frame, encode_frame, Message, Session, BATCH_SIZE, MAX_FRAME_BYTES,
    PROTOCOL_VERSION,
};
pub use store::{Field, Peer};

#[cfg(test)]
mod tests;
