//! The 4-channel/64-row cell grid, one per pattern block.
//! Bead: modmill-4r0.6. See docs/format-notes/pattern-tables.md (cell
//! bit-packing) and docs/format-notes/effect-timing.md (effect/param
//! semantics, including the D-effect BCD-row gotcha) for the inherited
//! handoffs.
//!
//! This module extracts the raw 4 fields (sample, period, effect nibble,
//! param byte) off the wire only. It does not interpret effect semantics
//! (no tick timing, no BCD decoding of D's param) -- that belongs to the
//! engine, later.

use anyhow::{bail, Context};

use super::{Cell, Pattern, CHANNELS, ROWS_PER_PATTERN, TAG_OFFSET};

const CELL_BYTES: usize = 4;
const ROW_BYTES: usize = CHANNELS * CELL_BYTES;
const PATTERN_BYTES: usize = ROWS_PER_PATTERN * ROW_BYTES;

/// `mod.rs`'s `PATTERN_DATA_OFFSET`/`TAG_OFFSET` are short by 130 bytes:
/// they account for the 20-byte name and 31*30 sample headers, but not the
/// 1-byte song_length + 1-byte restart_position + 128-byte order table
/// that sit between the sample headers and the "M.K." tag. Verified with
/// `xxd fixtures/effects.mod`: the tag is at file offset 1080 and pattern
/// data starts at 1084, not 950/954 as `mod.rs` computes. Flagged to the
/// orchestrator and w2-header via pact (mod.rs is shared, not edited here);
/// this local constant is the corrected offset used only by this module.
const PATTERN_DATA_OFFSET: usize = TAG_OFFSET + 1 + 1 + 128 + 4;

fn decode_cell(b: &[u8]) -> Cell {
    let (b0, b1, b2, b3) = (b[0], b[1], b[2], b[3]);
    Cell {
        sample: (b0 & 0xF0) | (b2 >> 4),
        period: (u16::from(b0 & 0x0F) << 8) | u16::from(b1),
        effect: b2 & 0x0F,
        param: b3,
    }
}

/// `order` is the full 128-entry order table; only `order[..song_length]`
/// is meaningful. Pattern blocks are stored in block-index order in the
/// file, independent of play order, so the number of blocks to parse is
/// `max(order[..song_length]) + 1`, not `song_length`.
pub fn parse(bytes: &[u8], order: &[u8], song_length: u8) -> anyhow::Result<Vec<Pattern>> {
    let used_order = &order[..(song_length as usize).min(order.len())];
    let block_count = used_order.iter().copied().max().map(|m| m as usize + 1).unwrap_or(0);

    let mut patterns = Vec::with_capacity(block_count);
    for block in 0..block_count {
        let start = PATTERN_DATA_OFFSET + block * PATTERN_BYTES;
        let end = start + PATTERN_BYTES;
        let block_bytes = bytes
            .get(start..end)
            .with_context(|| format!("pattern block {block} truncated (need {end} bytes)"))?;

        let mut pattern: Pattern = Vec::with_capacity(ROWS_PER_PATTERN);
        for row in 0..ROWS_PER_PATTERN {
            let row_bytes = &block_bytes[row * ROW_BYTES..(row + 1) * ROW_BYTES];
            let mut cells = [Cell::default(); CHANNELS];
            for (ch, cell) in cells.iter_mut().enumerate() {
                *cell = decode_cell(&row_bytes[ch * CELL_BYTES..(ch + 1) * CELL_BYTES]);
            }
            pattern.push(cells);
        }
        patterns.push(pattern);
    }

    if block_count == 0 {
        bail!("song_length/order produced zero pattern blocks");
    }

    Ok(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        fs::read(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
    }

    // effects.mod: single pattern, order=[0], song_length=1. 8 rows of
    // single-channel effects, per fixtures/effects.mod (scripts/gen_fixtures.py)
    // and docs/format-notes/effect-timing.md's confirmed xxd dump.
    #[test]
    fn effects_mod_raw_effect_bytes() {
        let bytes = fixture("effects.mod");
        let order = [0u8; 128];
        let patterns = parse(&bytes, &order, 1).expect("parse effects.mod");
        assert_eq!(patterns.len(), 1);
        let pat = &patterns[0];

        // (period, sample, effect, param) expected per row, channel 0.
        let expected = [
            (428u16, 1u8, 0x00u8, 0x37u8), // arpeggio +3 +7
            (0, 0, 0x01, 0x08),            // slide up
            (0, 0, 0x02, 0x08),            // slide down
            (339, 1, 0x03, 0x04),          // tone portamento toward E-3
            (428, 1, 0x04, 0x64),          // vibrato speed 6 depth 4
            (0, 0, 0x0A, 0x0F),            // volume slide up
            (0, 0, 0x0A, 0xF0),            // volume slide down
            (0, 0, 0x0C, 0x20),            // set volume 32
        ];

        for (row, &(period, sample, effect, param)) in expected.iter().enumerate() {
            let cell = pat[row][0];
            assert_eq!(cell.period, period, "row {row} period");
            assert_eq!(cell.sample, sample, "row {row} sample");
            assert_eq!(cell.effect, effect, "row {row} effect");
            assert_eq!(cell.param, param, "row {row} param");
        }
    }

    // jump.mod: order=[0,1,2], song_length=3 -> 3 pattern blocks (block
    // count comes from max(order)+1, not song_length, per the w1-patterns
    // handoff -- here they happen to be equal, but the logic must not
    // hardcode that coincidence).
    //
    // Per the w1-effects handoff: D's param is packed BCD and jump.mod's
    // pattern0 row0 D-param is *raw* 0x10 on the wire (decimal-10 once
    // BCD-decoded downstream, NOT hex 16 as the fixture generator's own
    // comment claims). This module does not decode BCD -- it must read
    // 0x10 faithfully off the wire and leave interpretation to the engine.
    #[test]
    fn jump_mod_three_blocks_raw_db_bytes() {
        let bytes = fixture("jump.mod");
        let mut order = [0u8; 128];
        order[0] = 0;
        order[1] = 1;
        order[2] = 2;
        let patterns = parse(&bytes, &order, 3).expect("parse jump.mod");
        assert_eq!(patterns.len(), 3);

        // pattern0 row0 ch0: 01 ac 1d 10 -> effect=D(0x0D) param=0x10 (raw)
        let cell0 = patterns[0][0][0];
        assert_eq!(cell0.sample, 1);
        assert_eq!(cell0.period, 428);
        assert_eq!(cell0.effect, 0x0D);
        assert_eq!(cell0.param, 0x10);

        // pattern1 row0 ch0: 01 7d 1b 02 -> effect=B(0x0B) param=0x02 (raw)
        let cell1 = patterns[1][0][0];
        assert_eq!(cell1.sample, 1);
        assert_eq!(cell1.period, 381);
        assert_eq!(cell1.effect, 0x0B);
        assert_eq!(cell1.param, 0x02);

        // pattern2 row0 ch0: 01 53 10 00 -> no effect
        let cell2 = patterns[2][0][0];
        assert_eq!(cell2.sample, 1);
        assert_eq!(cell2.period, 339);
        assert_eq!(cell2.effect, 0x00);
        assert_eq!(cell2.param, 0x00);
    }
}
