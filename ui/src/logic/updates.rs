//! When the app looks for a newer release.

/// One small request at most every 6 hours: often enough that a release is
/// noticed the same day, rarely enough that nobody notices the request.
pub const CHECK_EVERY_MS: i64 = 6 * 60 * 60 * 1000;

/// Whether it is time to check again. `last_check` is `None` until the
/// first check of this launch.
pub fn is_due(last_check: Option<i64>, now: i64) -> bool {
    last_check.is_none_or(|last| now - last >= CHECK_EVERY_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_once_per_launch_and_then_every_six_hours() {
        assert!(is_due(None, 0));
        assert!(!is_due(Some(1_000), 1_000 + CHECK_EVERY_MS - 1));
        assert!(is_due(Some(1_000), 1_000 + CHECK_EVERY_MS));
    }
}
