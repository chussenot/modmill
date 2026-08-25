//! Effect `0xy`: arpeggio. Bead: modmill-4r0.10.

/// Semitone-offset -> period conversion: the Amiga period scale is
/// linear-in-log-frequency, so halving the period raises pitch by an
/// octave (12 semitones) -- `period = base / 2^(semitones/12)`, rounded to
/// the nearest integer period (see docs/format-notes/effect-timing.md §2).
fn period_for_offset(base_period: u16, semitones: u8) -> u16 {
    if semitones == 0 {
        return base_period;
    }
    let ratio = 2f64.powf(f64::from(semitones) / 12.0);
    (f64::from(base_period) / ratio).round() as u16
}

/// `base_period` is the period set by the channel's last note trigger.
/// `param` packs two semitone offsets (x = high nibble, y = low nibble).
/// Cycles the sounding pitch by `tick_in_row % 3`: 0 -> base note, 1 ->
/// base+x semitones, 2 -> base+y semitones, then repeats
/// (docs/format-notes/effect-timing.md §2). Only called for
/// `tick_in_row >= 1` by the render loop -- tick 0 already triggered the
/// note at `base_period`, which is also what `tick_in_row % 3 == 0`
/// produces, so no special-casing is needed there.
pub fn period_for_tick(base_period: u16, param: u8, tick_in_row: u32) -> u16 {
    match tick_in_row % 3 {
        1 => period_for_offset(base_period, param >> 4),
        2 => period_for_offset(base_period, param & 0x0F),
        _ => base_period,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_multiple_of_three_returns_base_period_unchanged() {
        for tick in [0, 3, 6, 9] {
            assert_eq!(period_for_tick(428, 0x37, tick), 428);
        }
    }

    #[test]
    fn tick_mod_three_one_returns_x_semitones_up() {
        // param 0x37: x=3, y=7.
        let expected = period_for_offset(428, 3);
        assert_eq!(period_for_tick(428, 0x37, 1), expected);
        assert_eq!(period_for_tick(428, 0x37, 4), expected);
    }

    #[test]
    fn tick_mod_three_two_returns_y_semitones_up() {
        let expected = period_for_offset(428, 7);
        assert_eq!(period_for_tick(428, 0x37, 2), expected);
        assert_eq!(period_for_tick(428, 0x37, 5), expected);
    }

    #[test]
    fn period_for_offset_matches_equal_tempered_scale() {
        // One octave up (12 semitones) exactly halves the period.
        assert_eq!(period_for_offset(856, 12), 428);
        // Zero semitones is a no-op.
        assert_eq!(period_for_offset(428, 0), 428);
    }
}
