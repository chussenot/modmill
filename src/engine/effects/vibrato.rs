//! Effect `4xy`: vibrato. Bead: modmill-4r0.12.
//! [long-hold demo bead for this run: full-song render + tuning pass]

/// `param` packs speed (high nibble, sine-table steps per tick) and depth
/// (low nibble, amplitude). `phase` is the channel's running table index,
/// owned by the caller and advanced in place each tick this is called.
/// Returns a period OFFSET to add to the channel's base period (0 until
/// implemented).
pub fn period_offset_for_tick(_param: u8, _phase: &mut u8) -> i16 {
    0
}
