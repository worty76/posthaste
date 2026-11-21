//! Posthaste storage layer. No UI or desktop dependencies: everything here
//! can be tested against a plain SQLite file.

mod db;
mod details;
mod error;
pub mod look;
mod markdown;
pub mod migrations;
mod steps;
pub mod sync;
mod transfer;

pub use db::{now_ms, Db, RETIRED_AI_PROVIDERS};
pub use error::{CoreError, Result};
pub use posthaste_model::*;
pub use transfer::ExportFormat;

#[cfg(test)]
mod tests;
