//! The spice's accounts of the day, as rules over the Rust state (Stage B
//! of `docs/readable-port-plan.md`: subsystem logic in Rust).
//!
//! The economy's record ([`super::economy::Economy`]) and the loose
//! variables of the day ([`g::SPICE_YESTERDAY`](super::globals::SPICE_YESTERDAY):
//! the stock at the last day's change, [`g::SPICE_PAID`](super::globals::SPICE_PAID):
//! paid out since). `new_day` (1C46h) and `update_totals` (C02Eh) in
//! `hand/tails.rs` are their 8086 frame.

/// The day's income: the stock now, plus what was paid out, less the
/// stock at the day's start (words, wrapping as the original adds); 0
/// when that is negative (true: the borrow the frame's count follows).
pub fn day_income(spice: u16, paid: u16, yesterday: u16) -> (u16, bool) {
    let (d, borrow) = spice.wrapping_add(paid).overflowing_sub(yesterday);
    (if borrow { 0 } else { d }, borrow)
}

/// The income's change from `before` to `now`: how much it fell and how
/// much it rose (one of them 0).
pub fn income_change(before: u16, now: u16) -> (u16, u16) {
    if before >= now { (before - now, 0) } else { (0, now - before) }
}

/// The income as `update_totals` keeps it during the day: the day's
/// figure when not below the one kept (`None`: kept).
pub fn raised_income(kept: u16, now: u16) -> Option<u16> {
    (now >= kept).then_some(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts() {
        assert_eq!(day_income(500, 100, 450), (150, false));
        assert_eq!(day_income(300, 0, 450), (0, true));
        assert_eq!(income_change(150, 100), (50, 0));
        assert_eq!(income_change(100, 150), (0, 50));
        assert_eq!(raised_income(100, 150), Some(150));
        assert_eq!(raised_income(100, 100), Some(100));
        assert_eq!(raised_income(100, 50), None);
    }
}
