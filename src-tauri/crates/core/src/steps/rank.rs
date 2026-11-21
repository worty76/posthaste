//! Order for steps: a rank is a base-62 fraction after the point, so a new
//! step can always be placed between two neighbours without renumbering.
//!
//! The same neighbours always give the same rank; ties between equal ranks
//! are broken by step id where the order is read. A rank is read in byte
//! order, which matches the digit order of the alphabet below.

/// The digit alphabet, lowest first: ASCII order, so byte order and digit
/// order agree.
const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Where a digit sits in [`DIGITS`]. `before`/`after` never carry anything
/// else, but a bad stored rank must not panic the writer.
fn digit_value(d: u8) -> u8 {
    match d {
        b'0'..=b'9' => d - b'0',
        b'A'..=b'Z' => d - b'A' + 10,
        b'a'..=b'z' => d - b'a' + 36,
        // Below every real digit: sorts before the neighbour it sits in.
        _ => 0,
    }
}

/// A rank strictly between `before` and `after`, or before `after` when
/// `before` is `None`, or after `before` when `after` is `None`. Both
/// neighbours must be given in order; a rank never ends in the lowest digit,
/// so there is always room between one rank and the next.
///
/// The ranks are read as fractions after the point: walking both, missing
/// `before` digits are the lowest digit and missing `after` digits are past
/// the highest, so the walk always ends at a gap of two or more.
pub(crate) fn rank_between(before: Option<&str>, after: Option<&str>) -> String {
    let low = before.unwrap_or("");
    let high = after.unwrap_or("");
    let mut out = Vec::with_capacity(low.len().max(high.len()) + 1);
    for position in 0..=low.len().max(high.len()) {
        let lo = low
            .as_bytes()
            .get(position)
            .map(|d| digit_value(*d) as u16)
            .unwrap_or(0);
        let hi = high
            .as_bytes()
            .get(position)
            .map(|d| digit_value(*d) as u16)
            .unwrap_or(DIGITS.len() as u16);
        let gap = hi.saturating_sub(lo);
        if gap >= 2 {
            out.push(DIGITS[(lo + gap / 2) as usize]);
            break;
        }
        // Equal digits, or a gap of one: copy the lower digit and keep
        // looking, there is always a wider gap one position further in.
        out.push(DIGITS[lo as usize]);
    }
    String::from_utf8(out).expect("ranks are ASCII")
}
