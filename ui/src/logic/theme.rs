use posthaste_model::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Light,
    Dark,
}

impl Scheme {
    /// The value of `data-theme` on `<html>`.
    pub fn as_str(self) -> &'static str {
        match self {
            Scheme::Light => "light",
            Scheme::Dark => "dark",
        }
    }
}

/// Which scheme to paint: the setting, or the OS while it says "system".
pub fn resolve_theme(theme: Theme, system_dark: bool) -> Scheme {
    match theme {
        Theme::Light => Scheme::Light,
        Theme::Dark => Scheme::Dark,
        Theme::System if system_dark => Scheme::Dark,
        Theme::System => Scheme::Light,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_system_only_when_asked_to() {
        assert_eq!(resolve_theme(Theme::System, true), Scheme::Dark);
        assert_eq!(resolve_theme(Theme::System, false), Scheme::Light);
        assert_eq!(resolve_theme(Theme::Light, true), Scheme::Light);
        assert_eq!(resolve_theme(Theme::Dark, false), Scheme::Dark);
    }
}
