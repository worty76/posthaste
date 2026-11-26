//! Controls that know nothing about notes. A component that needs `ipc`
//! belongs in `windows`, not here.

mod button;
mod calendar;
mod controls;
mod dialog;
mod empty_state;
mod icon;
mod kbd;
mod palette;
mod position_picker;
mod project_picker;
mod scene;
mod toasts;

pub use button::{Button, IconButton, Variant};
pub use calendar::{Calendar, DayMarks};
pub use controls::{Segmented, Switch};
pub use dialog::Dialog;
pub use empty_state::EmptyState;
pub use icon::{Icon, IconName};
pub use kbd::{caps, Kbd};
pub use palette::CommandPalette;
pub use position_picker::PositionPicker;
pub use project_picker::{ProjectDot, ProjectPicker};
pub use scene::SceneHost;
pub use toasts::{Toaster, Toasts};
