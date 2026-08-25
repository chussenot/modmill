//! The 4-channel/64-row cell grid, one per pattern block.
//! Bead: modmill-4r0.6. See docs/format-notes/pattern-tables.md (cell
//! bit-packing) and docs/format-notes/effect-timing.md (effect/param
//! semantics, including the D-effect BCD-row gotcha) for the inherited
//! handoffs.

use super::Pattern;

/// `order` is the full 128-entry order table; only `order[..song_length]`
/// is meaningful. Pattern blocks are stored in block-index order in the
/// file, independent of play order, so the number of blocks to parse is
/// `max(order[..song_length]) + 1`, not `song_length`.
pub fn parse(_bytes: &[u8], _order: &[u8], _song_length: u8) -> anyhow::Result<Vec<Pattern>> {
    unimplemented!("modmill-4r0.6: parse the pattern cell grid")
}
