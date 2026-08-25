//! The 31 sample headers: name, length, finetune, volume, loop start/length.
//! Bead: modmill-4r0.5. See docs/format-notes/samples.md for the
//! inherited handoff (units are WORDS, loop_len<=1 means no loop, etc).

use super::Sample;

pub fn parse(_bytes: &[u8]) -> anyhow::Result<Vec<Sample>> {
    unimplemented!("modmill-4r0.5: parse the 31 sample headers")
}
