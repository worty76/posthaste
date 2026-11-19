//! Light or dark, as the worn look's base decides. The interface paints
//! one scheme at a time and never mixes.

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
