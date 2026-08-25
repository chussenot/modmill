//! Song name, song length, restart position, order table, M.K. tag.
//! Bead: modmill-4r0.4. See docs/format-notes/samples.md and
//! docs/format-notes/pattern-tables.md for the inherited handoffs.

use super::Header;

pub fn parse(_bytes: &[u8]) -> anyhow::Result<Header> {
    unimplemented!("modmill-4r0.4: parse song name / song length / restart / order / M.K. tag")
}
