//! Effects `1xx`/`2xx` (slide up/down) and `3xx` (tone portamento).
//! Bead: modmill-4r0.11.

/// Amiga/ProTracker period-table extremes (see `docs/format-notes/pattern-tables.md`
/// §1): 856 = lowest pitch (C-1), 113 = highest pitch (B-3). Slides clamp to
/// this range so a long run of `1xx`/`2xx` can't walk the period past the
/// real hardware table into nonsense (or, for slide-up, underflow).
const PERIOD_MIN: u16 = 113;
const PERIOD_MAX: u16 = 856;

/// `1xx`/`2xx`: slide the current period by `param` per tick (nibble `1` =
/// slide up = pitch up = period decreases; nibble `2` = slide down = pitch
/// down = period increases), clamped to the Amiga period-table range.
pub fn slide_period(period: u16, param: u8, effect_nibble: u8) -> u16 {
    let delta = param as i32;
    let new_period = if effect_nibble == 0x1 {
        period as i32 - delta
    } else {
        period as i32 + delta
    };
    new_period.clamp(PERIOD_MIN as i32, PERIOD_MAX as i32) as u16
}

/// `3xx`: glide `period` toward `target_period` by up to `param` per tick,
/// snapping exactly to `target_period` once within `param` of it (never
/// overshoots).
pub fn tone_portamento_step(period: u16, target_period: u16, param: u8) -> u16 {
    let diff = target_period as i32 - period as i32;
    let step = param as i32;
    if diff.unsigned_abs() <= step as u32 {
        target_period
    } else if diff > 0 {
        period + step as u16
    } else {
        period - step as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_up_decreases_period_by_param_each_call() {
        let mut period = 428u16;
        period = slide_period(period, 8, 0x1);
        assert_eq!(period, 420);
        period = slide_period(period, 8, 0x1);
        assert_eq!(period, 412);
    }

    #[test]
    fn slide_down_increases_period_by_param_each_call() {
        let mut period = 428u16;
        period = slide_period(period, 8, 0x2);
        assert_eq!(period, 436);
        period = slide_period(period, 8, 0x2);
        assert_eq!(period, 444);
    }

    #[test]
    fn slide_up_clamps_at_period_min() {
        let period = slide_period(120, 50, 0x1);
        assert_eq!(period, PERIOD_MIN);
    }

    #[test]
    fn slide_down_clamps_at_period_max() {
        let period = slide_period(850, 50, 0x2);
        assert_eq!(period, PERIOD_MAX);
    }

    #[test]
    fn tone_portamento_converges_upward_without_overshoot() {
        let mut period = 300u16;
        let target = 339u16;
        period = tone_portamento_step(period, target, 10);
        assert_eq!(period, 310);
        period = tone_portamento_step(period, target, 10);
        assert_eq!(period, 320);
        period = tone_portamento_step(period, target, 10);
        assert_eq!(period, 330);
        // Remaining distance (9) is less than param (10): snap exactly.
        period = tone_portamento_step(period, target, 10);
        assert_eq!(period, target);
        // Once at target, further steps are a no-op.
        period = tone_portamento_step(period, target, 10);
        assert_eq!(period, target);
    }

    #[test]
    fn tone_portamento_converges_downward_without_overshoot() {
        let mut period = 428u16;
        let target = 400u16;
        period = tone_portamento_step(period, target, 12);
        assert_eq!(period, 416);
        // Remaining distance (16) is greater than param (12): keep stepping.
        period = tone_portamento_step(period, target, 12);
        assert_eq!(period, 404);
        // Remaining distance (4) is less than param (12): snap exactly.
        period = tone_portamento_step(period, target, 12);
        assert_eq!(period, target);
    }

    #[test]
    fn tone_portamento_snaps_exactly_when_close() {
        // diff == param exactly: still snaps (uses <=, not <).
        assert_eq!(tone_portamento_step(330, 339, 9), 339);
        assert_eq!(tone_portamento_step(339, 330, 9), 330);
    }
}
