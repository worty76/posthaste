//! State every window shares: the settings, the language and the look.
//! Each is loaded once per window and kept current from the shell's events.

mod appearance;
mod i18n;
mod scene;
mod settings;
mod updates;

pub use appearance::{use_appearance, Appearance};
pub use i18n::{use_i18n, I18n, Message};
pub use scene::{
    fetch_asset, seed_assets, trim_assets, use_asset_urls, use_scene, use_scene_preview,
};
pub use settings::{use_settings, SettingsHandle};
pub use updates::{use_updates, Updates};
