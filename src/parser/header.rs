//! Song name, song length, restart position, order table, M.K. tag.
//! Bead: modmill-4r0.4. See docs/format-notes/samples.md and
//! docs/format-notes/pattern-tables.md for the inherited handoffs.
//!
//! NOTE on offsets: `super::TAG_OFFSET` (950) is the shared contract's
//! computed tag position, but it only accounts for name(20) + 31 sample
//! headers(930) and skips the song-length/restart/order-table block (130
//! bytes) that sits between the sample headers and the tag in the real
//! ProTracker layout. Verified against fixtures/minimal.mod: b"M.K." is at
//! byte 1080, not 950 (byte 950 is the song-length byte). This module uses
//! the locally-defined, fixture-verified offsets below rather than the
//! (mis-)shared constants; flagged to the orchestrator via pact message
//! since PATTERN_DATA_OFFSET (TAG_OFFSET + 4) is downstream of the same bug.

use super::Header;
use anyhow::{bail, Context};

const NAME_OFFSET: usize = 0;
const NAME_LEN: usize = 20;
const SONG_LENGTH_OFFSET: usize = 950;
const RESTART_OFFSET: usize = 951;
const ORDER_OFFSET: usize = 952;
const ORDER_LEN: usize = 128;
const TAG_OFFSET: usize = ORDER_OFFSET + ORDER_LEN; // 1080
const TAG: &[u8; 4] = b"M.K.";

pub fn parse(bytes: &[u8]) -> anyhow::Result<Header> {
    let tag = bytes
        .get(TAG_OFFSET..TAG_OFFSET + 4)
        .context("file too short to contain the format tag")?;
    if tag != TAG {
        bail!("unsupported format tag {:?}; only M.K. is in scope", tag);
    }

    let name_bytes = bytes
        .get(NAME_OFFSET..NAME_OFFSET + NAME_LEN)
        .context("file too short to contain the song name")?;
    let name = String::from_utf8_lossy(name_bytes)
        .trim_end_matches('\0')
        .to_string();

    let song_length = *bytes
        .get(SONG_LENGTH_OFFSET)
        .context("file too short to contain the song length")?;
    let restart_position = *bytes
        .get(RESTART_OFFSET)
        .context("file too short to contain the restart position")?;
    let order = bytes
        .get(ORDER_OFFSET..ORDER_OFFSET + ORDER_LEN)
        .context("file too short to contain the order table")?
        .to_vec();

    Ok(Header { name, song_length, restart_position, order })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal() {
        let bytes = std::fs::read("fixtures/minimal.mod").unwrap();
        let header = parse(&bytes).unwrap();
        assert_eq!(header.name, "minimal");
        assert_eq!(header.song_length, 1);
        assert_eq!(header.restart_position, 0x7F);
        assert_eq!(&header.order[..1], &[0]);
        assert_eq!(header.order.len(), 128);
    }

    #[test]
    fn parses_jump_order_table() {
        // jump.mod is built with order = [0, 1, 2] (gen_fixtures.py), and
        // per the inherited handoff the order table holds pattern block
        // indices, not data/offsets -- only order[..song_length] is
        // meaningful, so we only assert the first 3 entries.
        let bytes = std::fs::read("fixtures/jump.mod").unwrap();
        let header = parse(&bytes).unwrap();
        assert_eq!(header.name, "jump");
        assert_eq!(header.song_length, 3);
        assert_eq!(&header.order[..3], &[0, 1, 2]);
    }

    #[test]
    fn rejects_bad_tag() {
        let mut bytes = std::fs::read("fixtures/minimal.mod").unwrap();
        bytes[TAG_OFFSET] = b'X';
        assert!(parse(&bytes).is_err());
    }
}
