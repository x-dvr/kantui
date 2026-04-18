/// Sparse-integer position step. Gives 2^21 insertions between any two
/// neighbours before a rebalance is needed.
pub(crate) const POSITION_STEP: i32 = 1024;

/// Position for an item appended after `last` (or `POSITION_STEP` if empty).
pub(crate) fn append_after(last: Option<i32>) -> i32 {
    match last {
        Some(p) => p.saturating_add(POSITION_STEP),
        None => POSITION_STEP,
    }
}

/// Position for an item inserted between two neighbours. Either side may be
/// `None` (meaning "start" or "end"). Returns `None` if the neighbours are
/// adjacent (caller must rebalance).
pub(crate) fn between(prev: Option<i32>, next: Option<i32>) -> Option<i32> {
    match (prev, next) {
        (None, None) => Some(POSITION_STEP),
        (Some(p), None) => Some(p.saturating_add(POSITION_STEP)),
        (None, Some(n)) => Some(n.saturating_sub(POSITION_STEP)),
        (Some(p), Some(n)) => {
            if n.saturating_sub(p) <= 1 {
                None
            } else {
                Some(p + (n - p) / 2)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_empty() {
        assert_eq!(append_after(None), POSITION_STEP);
    }

    #[test]
    fn append_after_existing() {
        assert_eq!(append_after(Some(10)), 10 + POSITION_STEP);
    }

    #[test]
    fn between_gap() {
        assert_eq!(between(Some(0), Some(1024)), Some(512));
    }

    #[test]
    fn between_no_gap() {
        assert_eq!(between(Some(5), Some(6)), None);
    }

    #[test]
    fn between_endpoints() {
        assert_eq!(between(None, Some(1024)), Some(0));
        assert_eq!(between(Some(1024), None), Some(2048));
    }
}
