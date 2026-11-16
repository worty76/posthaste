//! Posthaste storage layer. No UI or desktop dependencies: everything here
//! can be tested against a plain SQLite file.

mod db;
mod details;
mod error;
pub mod look;
pub mod migrations;
mod model;
mod transfer;

pub use db::{now_ms, Db, RETIRED_AI_PROVIDERS};
pub use error::{CoreError, Result};
pub use model::*;

#[cfg(test)]
mod tests;
