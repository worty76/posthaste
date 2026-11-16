//! Controls that know nothing about notes. A component that needs `ipc`
//! belongs in `windows`, not here.

mod button;
mod calendar;
mod empty_state;
mod icon;
mod kbd;

pub use button::IconButton;
pub use calendar::{Calendar, DayMarks};
pub use empty_state::EmptyState;
pub use icon::{Icon, IconName};
pub use kbd::{caps, Kbd};
