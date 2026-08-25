//! Mixer, PAL tick clock, and effect execution. Bead: modmill-4r0.8
//! (core: mixer/PAL timing/F effect) plus one bead per effect family
//! under src/engine/effects/ (modmill-4r0.10..14).
//!
//! Scope for this bead: PAL timing (default 125 BPM / speed 6), note
//! triggering + linear-interpolation resampling + looping, 4-channel
//! mixing down to mono 16-bit PCM, and the `F` (set speed/tempo) effect.
//! Every other effect nibble (0,1,2,3,4,A,C,D,B, and anything unlisted)
//! is a no-op: the note plays with no pitch/volume modulation, and a
//! one-line notice is logged to stderr the first time each nibble is
//! seen (deduped by nibble, not per-row). Those effects land in later
//! gated beads.
//!
//! Output is mono. The 4 Amiga channels are summed into one PCM stream
//! rather than preserved as a 2-channel hard-left/right pan image --
//! simpler, and the bead's acceptance criteria only asks for
//! deterministic audio output, not stereo imaging.
//!
//! Determinism: no randomness, no wall-clock reads, no hash-map
//! iteration -- row/tick timing is accumulated as exact `f64` targets
//! (see `elapsed_target` below) so repeated renders of the same input
//! produce byte-identical WAV output.

use std::path::Path;

use anyhow::Context;

use crate::parser::{Module, CHANNELS, PATTERN_DATA_OFFSET, ROWS_PER_PATTERN};

const SAMPLE_RATE: u32 = 44_100;
const DEFAULT_SPEED: u32 = 6;
const DEFAULT_BPM: f64 = 125.0;
/// Amiga PAL master clock constant: freq_hz = this / (period * 2).
const AMIGA_CLOCK: f64 = 7_093_789.2;
/// Bytes per pattern block (64 rows * 4 channels * 4 bytes/cell) -- derived
/// from the parser's own row/channel constants rather than a bare magic
/// number, so it can't drift out of sync with `src/parser/patterns.rs`.
const PATTERN_BLOCK_BYTES: usize = ROWS_PER_PATTERN * CHANNELS * 4;

#[derive(Clone, Copy, Default)]
struct Channel {
    /// 0-based index into `module.samples` / the extracted PCM table.
    sample_idx: Option<usize>,
    /// Fractional read position, in sample bytes (1 byte = 1 8-bit frame).
    pos: f64,
    /// `pos` advance per output frame: freq_hz / SAMPLE_RATE.
    step: f64,
    volume: u8,
    playing: bool,
}

/// Extract each of the 31 samples' raw PCM bytes from the original file.
///
/// Cross-module concern: the parser (`crate::parser::samples`) only reads
/// sample *header* metadata (length/finetune/volume/loop points); it never
/// carries the raw 8-bit PCM payload. That payload sits in the file
/// immediately after all pattern blocks, samples back-to-back in the same
/// order as `module.samples`: offset = `PATTERN_DATA_OFFSET` + (number of
/// pattern blocks * bytes-per-block) + (running sum of preceding samples'
/// `length_bytes`).
fn extract_pcm(bytes: &[u8], module: &Module) -> Vec<Vec<i8>> {
    let mut offset = PATTERN_DATA_OFFSET + module.patterns.len() * PATTERN_BLOCK_BYTES;
    module
        .samples
        .iter()
        .map(|s| {
            let len = s.length_bytes as usize;
            let start = offset.min(bytes.len());
            let end = (offset + len).min(bytes.len());
            offset += len;
            bytes[start..end].iter().map(|&b| b as i8).collect()
        })
        .collect()
}

pub fn render_to_wav(module: &Module, raw_bytes: &[u8], out: &Path) -> anyhow::Result<()> {
    let pcm = extract_pcm(raw_bytes, module);

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(out, spec)
        .with_context(|| format!("creating WAV writer for {}", out.display()))?;

    let mut speed: u32 = DEFAULT_SPEED;
    let mut bpm: f64 = DEFAULT_BPM;
    let mut channels = [Channel::default(); CHANNELS];
    // Dedup effect-nibble warnings: index by nibble (0..=0xF), fire once.
    let mut warned = [false; 16];

    let mut frames_emitted: u64 = 0;
    let mut elapsed_target: f64 = 0.0;

    let song_len = (module.header.song_length as usize).min(module.header.order.len());
    for &block in &module.header.order[..song_len] {
        let Some(pattern) = module.patterns.get(block as usize) else {
            continue;
        };
        for row in pattern {
            for (ch_i, cell) in row.iter().enumerate() {
                // Effect handling: only F (speed/tempo) is in scope.
                match cell.effect {
                    0x0 if cell.param == 0 => {} // no effect present
                    0xF => {
                        if cell.param < 0x20 {
                            speed = cell.param as u32;
                        } else {
                            bpm = cell.param as f64;
                        }
                    }
                    other => {
                        let idx = other as usize;
                        if !warned[idx] {
                            eprintln!(
                                "modmill: effect {other:X} is out of scope for this bead; \
                                 playing note(s) without it"
                            );
                            warned[idx] = true;
                        }
                    }
                }

                // Note trigger: only when a period is present.
                if cell.period != 0 {
                    let instrument = if cell.sample != 0 {
                        Some(cell.sample as usize)
                    } else {
                        channels[ch_i].sample_idx.map(|i| i + 1)
                    };
                    if let Some(instr) = instrument
                        && instr >= 1
                        && instr <= module.samples.len()
                        && !pcm[instr - 1].is_empty()
                    {
                        let freq = AMIGA_CLOCK / (cell.period as f64 * 2.0);
                        channels[ch_i] = Channel {
                            sample_idx: Some(instr - 1),
                            pos: 0.0,
                            step: freq / SAMPLE_RATE as f64,
                            volume: module.samples[instr - 1].volume,
                            playing: true,
                        };
                    }
                }
            }

            // Row duration: `speed` ticks at `2.5 / bpm` seconds/tick.
            let row_seconds = speed as f64 * (2.5 / bpm);
            elapsed_target += row_seconds;
            let target_frames = (elapsed_target * SAMPLE_RATE as f64).round() as u64;
            let frames_this_row = target_frames.saturating_sub(frames_emitted);
            frames_emitted = target_frames;

            for _ in 0..frames_this_row {
                let mut mixed: i32 = 0;
                for ch in channels.iter_mut() {
                    if !ch.playing {
                        continue;
                    }
                    let Some(si) = ch.sample_idx else {
                        continue;
                    };
                    let data = &pcm[si];
                    let i0 = ch.pos.floor() as usize;
                    if data.is_empty() || i0 >= data.len() {
                        ch.playing = false;
                        continue;
                    }
                    let frac = ch.pos - i0 as f64;
                    let s0 = data[i0] as f64;
                    let s1 = if i0 + 1 < data.len() { data[i0 + 1] as f64 } else { s0 };
                    let interp = s0 + (s1 - s0) * frac;
                    let amp = interp * (ch.volume as f64 / 64.0) * 256.0;
                    mixed += amp.round() as i32;

                    ch.pos += ch.step;
                    let sample = &module.samples[si];
                    if sample.loop_len_bytes > 0 {
                        let loop_end = (sample.loop_start_bytes + sample.loop_len_bytes) as f64;
                        while ch.pos >= loop_end {
                            ch.pos -= sample.loop_len_bytes as f64;
                        }
                    } else if ch.pos >= data.len() as f64 {
                        ch.playing = false;
                    }
                }
                let clamped = mixed.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                writer.write_sample(clamped)?;
            }
        }
    }

    writer.finalize().context("finalizing WAV output")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        fs::read(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("modmill-engine-test-{name}-{}.wav", std::process::id()))
    }

    #[test]
    fn minimal_render_is_deterministic() {
        let bytes = fixture("minimal.mod");
        let module = crate::parser::parse(&bytes).expect("parse minimal.mod");

        let out_a = temp_path("minimal-a");
        let out_b = temp_path("minimal-b");
        render_to_wav(&module, &bytes, &out_a).expect("render a");
        render_to_wav(&module, &bytes, &out_b).expect("render b");

        let data_a = fs::read(&out_a).expect("read a");
        let data_b = fs::read(&out_b).expect("read b");
        assert_eq!(data_a, data_b, "repeated renders must be byte-identical");

        let _ = fs::remove_file(&out_a);
        let _ = fs::remove_file(&out_b);
    }

    #[test]
    fn speed_effect_does_not_panic_and_produces_audio() {
        let bytes = fixture("speed.mod");
        let module = crate::parser::parse(&bytes).expect("parse speed.mod");

        let out = temp_path("speed");
        render_to_wav(&module, &bytes, &out).expect("render speed.mod");

        let mut reader = hound::WavReader::open(&out).expect("reopen wav");
        let frame_count = reader.len();
        assert!(frame_count > 0, "expected non-empty audio output");
        // Touch the sample stream so a corrupt/truncated write would fail here too.
        let _: Vec<i16> = reader.samples::<i16>().map(|s| s.expect("valid sample")).collect();

        let _ = fs::remove_file(&out);
    }

    #[test]
    fn wav_header_is_sane() {
        let bytes = fixture("minimal.mod");
        let module = crate::parser::parse(&bytes).expect("parse minimal.mod");

        let out = temp_path("header");
        render_to_wav(&module, &bytes, &out).expect("render minimal.mod");

        let reader = hound::WavReader::open(&out).expect("reopen wav");
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, SAMPLE_RATE);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.bits_per_sample, 16);
        assert_eq!(spec.sample_format, hound::SampleFormat::Int);

        let _ = fs::remove_file(&out);
    }
}
