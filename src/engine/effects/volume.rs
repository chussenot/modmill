//! Effects `Axy` (volume slide) and `Cxx` (set volume). Bead: modmill-4r0.13.

/// `Axy`: slide `volume` (0-64) up by `x` per tick if `x>0`, else down by
/// `y`, clamped to [0, 64].
///
/// Divergence risk (see `docs/format-notes/effect-timing.md` §6 for the
/// sibling divergence flags this inherits the pattern from): real
/// ProTracker modules only ever set one of the two nibbles per command,
/// but when both are nonzero authentic PT applies the up-nibble and
/// silently ignores the down-nibble rather than erroring, summing, or
/// preferring down. modmill matches that: up wins whenever it is nonzero.
pub fn slide(volume: u8, param: u8) -> u8 {
    let up = param >> 4;
    let down = param & 0x0F;
    if up > 0 {
        volume.saturating_add(up).min(64)
    } else {
        volume.saturating_sub(down)
    }
}

/// `Cxx`: set volume to `param`, clamped to 64. This one IS implemented
/// here (trivial, no per-tick state) rather than stubbed, since the
/// render loop needs a real value at tick 0 regardless of this bead's
/// status; `slide` above is the part this bead actually owns.
pub fn set(param: u8) -> u8 {
    param.min(64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_up_only_increases() {
        // param 0x50: up nibble = 5, down nibble = 0.
        assert_eq!(slide(10, 0x50), 15);
    }

    #[test]
    fn slide_up_only_clamps_at_64() {
        assert_eq!(slide(60, 0xF0), 64);
        assert_eq!(slide(64, 0x10), 64);
    }

    #[test]
    fn slide_down_only_decreases() {
        // param 0x05: up nibble = 0, down nibble = 5.
        assert_eq!(slide(20, 0x05), 15);
    }

    #[test]
    fn slide_down_only_clamps_at_zero() {
        assert_eq!(slide(3, 0x0F), 0);
        assert_eq!(slide(0, 0x05), 0);
    }

    #[test]
    fn slide_both_nonzero_up_wins_per_pt_convention() {
        // param 0xFF -> up=0xF (15), down=0xF (15). Documented convention:
        // up applies, down is ignored (matches authentic PT, diverges from
        // any clone that sums or prefers down).
        assert_eq!(slide(10, 0xFF), 25);
    }

    #[test]
    fn set_clamps_at_64() {
        assert_eq!(set(200), 64);
        assert_eq!(set(64), 64);
        assert_eq!(set(32), 32);
    }
}
