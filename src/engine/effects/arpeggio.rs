//! Effect `0xy`: arpeggio. Bead: modmill-4r0.10.
//! [long-hold/kill-respawn liveness demo bead for this run]

/// `base_period` is the period set by the channel's last note trigger.
/// `param` packs two semitone offsets (x = high nibble, y = low nibble).
/// Cycles base/base+x/base+y across ticks 1, 2, 3 (then repeats).
/// Returns `base_period` unchanged until implemented.
pub fn period_for_tick(base_period: u16, _param: u8, _tick_in_row: u32) -> u16 {
    base_period
}
