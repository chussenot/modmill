//! Effect `4xy`: vibrato. Bead: modmill-4r0.12.
//! [long-hold demo bead for this run: full-song render + tuning pass]

/// One full sine cycle in 32 steps, peak amplitude 255 (same peak PT's own
/// quarter-wave table uses -- see docs/format-notes/effect-timing.md §3 --
/// just stored as a full cycle here instead of a mirrored quarter, since
/// that's what `phase` wrapping at 32 wants). `SINE[i] = round(255 *
/// sin(2*pi*i/32))`.
const SINE: [i16; 32] = [
    0, 50, 98, 142, 180, 212, 236, 250, 255, 250, 236, 212, 180, 142, 98, 50, 0, -50, -98, -142,
    -180, -212, -236, -250, -255, -250, -236, -212, -180, -142, -98, -50,
];

/// `param` packs speed (high nibble, sine-table steps advanced per tick)
/// and depth (low nibble, amplitude in period units). `phase` is the
/// channel's running table index (0..31), owned by the caller and
/// advanced in place each tick this is called. Returns a period OFFSET to
/// add to the channel's base period for this tick.
///
/// Depth scaling: `offset = table[phase] * depth / 128`, matching real
/// ProTracker's documented `(table_value * y) >> 7` (effect-timing.md
/// §3). At max depth (15) and peak table value (255) that tops out at an
/// offset of ~29 period units -- tuned by inspection against
/// `fixtures/effects.mod` row 4 (speed=6, depth=4: a fast, shallow
/// wobble) and re-checked across the full fixture set in the long-hold
/// tuning pass for this bead; 128 kept the wobble audible without ever
/// overshooting into a neighboring semitone's period range on the
/// fixtures tried.
pub fn period_offset_for_tick(param: u8, phase: &mut u8) -> i16 {
    let speed = param >> 4;
    let depth = i32::from(param & 0x0F);

    let idx = (*phase & 0x1F) as usize;
    let offset = (i32::from(SINE[idx]) * depth) / 128;

    *phase = phase.wrapping_add(speed) & 0x1F;

    offset as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_advances_by_speed_nibble_wrapping_at_32() {
        let mut phase = 0u8;
        // speed=5 (high nibble), depth irrelevant here.
        let param = 0x50;
        for expected in [5u8, 10, 15, 20, 25, 30, 3, 8] {
            period_offset_for_tick(param, &mut phase);
            assert_eq!(phase, expected % 32);
        }
    }

    #[test]
    fn depth_zero_always_returns_zero() {
        let mut phase = 0u8;
        // speed=7, depth=0 -- offset must be 0 regardless of phase/speed.
        let param = 0x70;
        for _ in 0..40 {
            assert_eq!(period_offset_for_tick(param, &mut phase), 0);
        }
    }

    #[test]
    fn offset_oscillates_positive_and_negative_over_a_full_cycle() {
        let mut phase = 0u8;
        // speed=1 so one full 32-step cycle takes exactly 32 calls.
        let param = 0x1F; // speed=1, depth=15 (max)
        let mut saw_positive = false;
        let mut saw_negative = false;
        for _ in 0..32 {
            let offset = period_offset_for_tick(param, &mut phase);
            if offset > 0 {
                saw_positive = true;
            }
            if offset < 0 {
                saw_negative = true;
            }
        }
        assert!(saw_positive, "expected at least one positive offset in a full cycle");
        assert!(saw_negative, "expected at least one negative offset in a full cycle");
        // phase must have wrapped back to where it started (32 steps of 1).
        assert_eq!(phase, 0);
    }
}
