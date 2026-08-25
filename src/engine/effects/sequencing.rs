//! `D` pattern-break and `B` position-jump param decoding. Bead: modmill-4r0.14
//!
//! Inherited from modmill-4r0.2 (handoff on bead:modmill-4r0.6, high
//! confidence): D's param is packed BCD (two decimal digits), NOT plain
//! hex/binary like B -- jump.mod's pattern0 row0 has D10 (raw 0x10), which
//! decodes to row 10 decimal, not row 16. B's param is a plain 0-based
//! order-table index (confirmed jump.mod B02 -> order slot 2 -> pattern2).

/// Decode a `D` (pattern break) effect param: packed BCD, two decimal
/// digits. E.g. raw `0x10` -> row 10 (not row 16).
pub fn decode_pattern_break_row(param: u8) -> u8 {
    (param >> 4) * 10 + (param & 0x0F)
}

/// Decode a `B` (position jump) effect param: a plain (non-BCD) order-table
/// index, passed through as-is.
pub fn decode_position_jump(param: u8) -> u8 {
    param
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_break_decodes_bcd() {
        assert_eq!(decode_pattern_break_row(0x10), 10, "jump.mod's D10 must decode to row 10, not 16");
        assert_eq!(decode_pattern_break_row(0x00), 0);
        assert_eq!(decode_pattern_break_row(0x23), 23);
        assert_eq!(decode_pattern_break_row(0x59), 59);
    }

    #[test]
    fn position_jump_is_passthrough() {
        for raw in [0u8, 1, 2, 63, 127, 255] {
            assert_eq!(decode_position_jump(raw), raw);
        }
    }
}
