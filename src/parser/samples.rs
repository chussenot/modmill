//! The 31 sample headers: name, length, finetune, volume, loop start/length.
//! Bead: modmill-4r0.5. See docs/format-notes/samples.md for the
//! inherited handoff (units are WORDS, loop_len<=1 means no loop, etc).

use super::{Sample, SAMPLE_COUNT};

const HEADER_SIZE: usize = 30;
const BASE_OFFSET: usize = 20;

/// Parse all 31 sample headers starting at file offset 20.
///
/// `length`/`loop_start`/`loop_len` are stored on disk in WORDS; this
/// returns them converted to BYTES. `loop_len_words <= 1` is ProTracker's
/// "no loop" convention, represented here as `loop_start_bytes = 0` and
/// `loop_len_bytes = 0` (rather than passing through the raw 0/2-byte
/// values, which would look like a spurious tiny loop).
pub fn parse(bytes: &[u8]) -> anyhow::Result<Vec<Sample>> {
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);

    for i in 0..SAMPLE_COUNT {
        let offset = BASE_OFFSET + i * HEADER_SIZE;
        let header = bytes
            .get(offset..offset + HEADER_SIZE)
            .ok_or_else(|| anyhow::anyhow!("truncated sample header {i} at offset {offset}"))?;

        let name = String::from_utf8_lossy(&header[0..22])
            .trim_end_matches('\0')
            .to_string();

        let length_words = u16::from_be_bytes([header[22], header[23]]);

        // Finetune: signed low nibble. 0..7 -> 0..+7, 8..15 -> -8..-1.
        let finetune_nibble = header[24] & 0x0F;
        let finetune = if finetune_nibble >= 8 {
            finetune_nibble as i8 - 16
        } else {
            finetune_nibble as i8
        };

        let volume = header[25];
        let loop_start_words = u16::from_be_bytes([header[26], header[27]]);
        let loop_len_words = u16::from_be_bytes([header[28], header[29]]);

        let (loop_start_bytes, loop_len_bytes) = if loop_len_words <= 1 {
            (0, 0)
        } else {
            (
                loop_start_words as u32 * 2,
                loop_len_words as u32 * 2,
            )
        };

        samples.push(Sample {
            name,
            length_bytes: length_words as u32 * 2,
            finetune,
            volume,
            loop_start_bytes,
            loop_len_bytes,
        });
    }

    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &[u8] = include_bytes!("../../fixtures/minimal.mod");
    const LOOP: &[u8] = include_bytes!("../../fixtures/loop.mod");
    const MULTITRACK: &[u8] = include_bytes!("../../fixtures/multitrack.mod");

    #[test]
    fn parses_all_31_samples() {
        let samples = parse(MINIMAL).unwrap();
        assert_eq!(samples.len(), SAMPLE_COUNT);
    }

    #[test]
    fn minimal_sample0_has_no_loop() {
        // loop_len_words = 1 (the disabled-loop marker) -> represented as
        // loop_start_bytes = 0, loop_len_bytes = 0.
        let samples = parse(MINIMAL).unwrap();
        let s = &samples[0];
        assert_eq!(s.name, "basic");
        assert_eq!(s.length_bytes, 32); // 16 words
        assert_eq!(s.finetune, 0);
        assert_eq!(s.volume, 64);
        assert_eq!(s.loop_start_bytes, 0);
        assert_eq!(s.loop_len_bytes, 0);
    }

    #[test]
    fn loop_mod_sample0_has_real_sustain_loop() {
        // length 32 words/64 bytes, loop_start 8 words/16 bytes,
        // loop_len 16 words/32 bytes.
        let samples = parse(LOOP).unwrap();
        let s = &samples[0];
        assert_eq!(s.name, "loopy");
        assert_eq!(s.length_bytes, 64);
        assert_eq!(s.finetune, 0);
        assert_eq!(s.volume, 48);
        assert_eq!(s.loop_start_bytes, 16);
        assert_eq!(s.loop_len_bytes, 32);
        assert!(s.loop_start_bytes + s.loop_len_bytes <= s.length_bytes);
    }

    #[test]
    fn multitrack_sample1_has_nonzero_finetune() {
        let samples = parse(MULTITRACK).unwrap();
        let s = &samples[1];
        assert_eq!(s.name, "bass");
        assert_eq!(s.finetune, 1);
    }
}
