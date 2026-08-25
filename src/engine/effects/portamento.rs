//! Effects `1xx`/`2xx` (slide up/down) and `3xx` (tone portamento).
//! Bead: modmill-4r0.11.

/// `1xx`/`2xx`: slide the current period by `param` per tick (1 = toward
/// lower period/higher pitch, 2 = toward higher period/lower pitch).
/// Returns `period` unchanged until implemented.
pub fn slide_period(period: u16, _param: u8, _effect_nibble: u8) -> u16 {
    period
}

/// `3xx`: glide `period` toward `target_period` at rate `param` per tick,
/// without overshooting. Returns `period` unchanged until implemented.
pub fn tone_portamento_step(period: u16, _target_period: u16, _param: u8) -> u16 {
    period
}
