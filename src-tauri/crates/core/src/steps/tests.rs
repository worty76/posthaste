//! Tests for the fractional ranks that order steps.

use super::rank::rank_between;

/// A small deterministic generator, so the tests need no dependency.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        // xorshift64*, adequate for choosing insert positions.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

#[test]
fn rank_between_none_and_none_is_not_empty() {
    let rank = rank_between(None, None);
    assert!(!rank.is_empty());
    assert!(rank.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn rank_between_sorts_strictly_between_its_neighbours() {
    let mut random = Random(0x5EED_2026_1121);
    let mut ranks: Vec<String> = Vec::new();
    for _ in 0..2_000 {
        // Insert at a random position of the growing list.
        let at = random.below(ranks.len() + 1);
        let before = if at == 0 {
            None
        } else {
            Some(ranks[at - 1].as_str())
        };
        let after = ranks.get(at).map(|r| r.as_str());
        let rank = rank_between(before, after);
        if let Some(before) = before {
            assert!(rank.as_bytes() > before.as_bytes(), "{rank} <= {before}");
        }
        if let Some(after) = after {
            assert!(rank.as_bytes() < after.as_bytes(), "{rank} >= {after}");
        }
        ranks.insert(at, rank);
    }
    // The whole list stays sorted, so the same data always reads the same.
    let sorted = ranks.clone();
    let mut sorted = sorted;
    sorted.sort();
    assert_eq!(ranks, sorted);
}

#[test]
fn rank_before_the_first_and_after_the_last_stay_ordered() {
    let mut ranks: Vec<String> = Vec::new();
    for _ in 0..500 {
        let rank = rank_between(None, ranks.first().map(|r| r.as_str()));
        if let Some(first) = ranks.first() {
            assert!(rank.as_bytes() < first.as_bytes());
        }
        ranks.insert(0, rank);
    }
    for _ in 0..500 {
        let rank = rank_between(ranks.last().map(|r| r.as_str()), None);
        if let Some(last) = ranks.last() {
            assert!(rank.as_bytes() > last.as_bytes());
        }
        ranks.push(rank);
    }
    let mut sorted = ranks.clone();
    sorted.sort();
    assert_eq!(ranks, sorted);
}

#[test]
fn ranks_never_end_in_the_lowest_digit() {
    // A rank ending in the lowest digit would leave no room to insert
    // directly before it but after the rank it follows.
    let mut random = Random(99);
    let mut ranks: Vec<String> = Vec::new();
    for _ in 0..500 {
        let at = random.below(ranks.len() + 1);
        let before = if at == 0 {
            None
        } else {
            Some(ranks[at - 1].as_str())
        };
        let rank = rank_between(before, ranks.get(at).map(|r| r.as_str()));
        assert!(!rank.ends_with('0'), "{rank}");
        ranks.insert(at, rank);
    }
}
