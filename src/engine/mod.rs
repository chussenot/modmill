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

mod effects;
use effects::{arpeggio, portamento, sequencing::{decode_pattern_break_row, decode_position_jump}, vibrato, volume};

const SAMPLE_RATE: u32 = 44_100;
const DEFAULT_SPEED: u32 = 6;
const DEFAULT_BPM: f64 = 125.0;
/// Amiga PAL master clock constant: freq_hz = this / (period * 2).
const AMIGA_CLOCK: f64 = 7_093_789.2;
/// Bytes per pattern block (64 rows * 4 channels * 4 bytes/cell) -- derived
/// from the parser's own row/channel constants rather than a bare magic
/// number, so it can't drift out of sync with `src/parser/patterns.rs`.
const PATTERN_BLOCK_BYTES: usize = ROWS_PER_PATTERN * CHANNELS * 4;

/// Which per-tick effect (if any) is active on a channel for the row
/// currently playing. Set once per row from the cell's effect nibble;
/// `None` variants (or a nibble not in W3's scope) mean "no per-tick
/// modulation this row" -- the plain-note / F / D / B / C case.
#[derive(Clone, Copy, Default, PartialEq)]
enum RowEffect {
    #[default]
    None,
    Arpeggio,
    Slide,
    TonePortamento,
    Vibrato,
    VolumeSlide,
}

#[derive(Clone, Copy, Default)]
struct Channel {
    /// 0-based index into `module.samples` / the extracted PCM table.
    sample_idx: Option<usize>,
    /// Fractional read position, in sample bytes (1 byte = 1 8-bit frame).
    pos: f64,
    /// `pos` advance per output frame for the CURRENT tick: freq_hz / SAMPLE_RATE.
    /// Recomputed every tick since an active effect can change the
    /// effective period tick-by-tick.
    step: f64,
    volume: u8,
    playing: bool,
    /// Period set by this channel's last note trigger. Arpeggio/vibrato
    /// compute their per-tick period relative to this; it does not itself
    /// change tick-to-tick.
    base_period: u16,
    /// Persistent working period for slide (1/2) and tone-portamento (3),
    /// which mutate cumulatively tick-to-tick rather than being computed
    /// fresh from `base_period` each time.
    current_period: u16,
    /// This row's active per-tick effect and its param, latched at tick 0.
    row_effect: RowEffect,
    row_param: u8,
    /// `Slide` only: the raw effect nibble (1 or 2), so `portamento::slide_period`
    /// knows the direction.
    slide_nibble: u8,
    /// Tone-portamento's target period (effect `3`), separate from
    /// `row_param` since the target comes from the cell's period field.
    tone_target: u16,
    /// Vibrato's running sine-table index, owned by the channel so it can
    /// be advanced in place each tick; not reset between rows (real PT
    /// does not reset vibrato phase on a non-retriggering row).
    vibrato_phase: u8,
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
    let order_table_len = module.header.order.len();
    // Order-table slot + row within its pattern. Indices rather than a
    // `for` iterator because `D`/`B` can redirect either one.
    let mut order_idx: usize = 0;
    let mut row_idx: usize = 0;

    while order_idx < song_len {
        let block = module.header.order[order_idx];
        let Some(pattern) = module.patterns.get(block as usize) else {
            order_idx += 1;
            row_idx = 0;
            continue;
        };
        if row_idx >= pattern.len() {
            // Jump target past this pattern's rows (e.g. a malformed D
            // param) -- treat like falling off the end of the pattern.
            order_idx += 1;
            row_idx = 0;
            continue;
        }

        let row = &pattern[row_idx];
        // `D` (pattern break) / `B` (position jump): row-level, decided once
        // per row regardless of which channel carries them. If both appear
        // on the same row, `B` picks the destination pattern (order-table
        // slot) and `D` picks the row within it -- inherited handoff on
        // bead:modmill-4r0.6.
        let mut pattern_break_row: Option<u8> = None;
        let mut position_jump_slot: Option<u8> = None;

        for (ch_i, cell) in row.iter().enumerate() {
            let ch = &mut channels[ch_i];
            // Per-row effect latch: cleared unless this cell sets one, so an
            // effect never silently continues into a row that doesn't name it.
            ch.row_effect = RowEffect::None;

            // Effect handling. F/D/B are row-level and immediate (tick 0);
            // C (set volume) is also immediate. The rest are per-tick and
            // only latched here -- applied starting tick 1 in the mix loop
            // below, per the inherited effect-timing handoff.
            match cell.effect {
                0x0 if cell.param == 0 => {} // no effect present
                0x0 => {
                    ch.row_effect = RowEffect::Arpeggio;
                    ch.row_param = cell.param;
                }
                0x1 | 0x2 => {
                    ch.row_effect = RowEffect::Slide;
                    ch.row_param = cell.param;
                    ch.slide_nibble = cell.effect;
                }
                0x3 => {
                    ch.row_effect = RowEffect::TonePortamento;
                    ch.row_param = cell.param;
                    if cell.period != 0 {
                        ch.tone_target = cell.period;
                    }
                }
                0x4 => {
                    ch.row_effect = RowEffect::Vibrato;
                    ch.row_param = cell.param;
                }
                0xA => {
                    ch.row_effect = RowEffect::VolumeSlide;
                    ch.row_param = cell.param;
                }
                0xC => ch.volume = volume::set(cell.param),
                0xF => {
                    if cell.param < 0x20 {
                        speed = cell.param as u32;
                    } else {
                        bpm = cell.param as f64;
                    }
                }
                0xD => pattern_break_row = Some(decode_pattern_break_row(cell.param)),
                0xB => position_jump_slot = Some(decode_position_jump(cell.param)),
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

            // Note trigger: only when a period is present. Tone-portamento
            // (3) uses the cell's period as a *target*, not a trigger --
            // real ProTracker glides toward it without retriggering the
            // sample.
            if cell.period != 0 && cell.effect != 0x3 {
                let instrument = if cell.sample != 0 {
                    Some(cell.sample as usize)
                } else {
                    ch.sample_idx.map(|i| i + 1)
                };
                if let Some(instr) = instrument
                    && instr >= 1
                    && instr <= module.samples.len()
                    && !pcm[instr - 1].is_empty()
                {
                    ch.sample_idx = Some(instr - 1);
                    ch.pos = 0.0;
                    ch.base_period = cell.period;
                    ch.current_period = cell.period;
                    ch.step = (AMIGA_CLOCK / (cell.period as f64 * 2.0)) / SAMPLE_RATE as f64;
                    ch.volume = module.samples[instr - 1].volume;
                    ch.playing = true;
                    // vibrato_phase deliberately NOT reset here: real
                    // ProTracker does not reset the vibrato sine-table
                    // position on a plain new note (docs/format-notes/
                    // effect-timing.md §6) -- only an explicit "retrigger
                    // vibrato waveform" effect would, and that's out of
                    // scope for modmill. modmill-89e.
                }
            }
        }

        // Row duration: `speed` ticks at `2.5 / bpm` seconds/tick, mixed one
        // tick at a time so a per-tick effect can change pitch/volume
        // mid-row (starting tick 1, per the inherited handoff).
        let tick_seconds = 2.5 / bpm;
        for tick in 0..speed {
            if tick >= 1 {
                for ch in channels.iter_mut() {
                    if !ch.playing {
                        continue;
                    }
                    let effective_period = match ch.row_effect {
                        RowEffect::None => ch.current_period,
                        RowEffect::Arpeggio => {
                            arpeggio::period_for_tick(ch.base_period, ch.row_param, tick)
                        }
                        RowEffect::Slide => {
                            ch.current_period =
                                portamento::slide_period(ch.current_period, ch.row_param, ch.slide_nibble);
                            ch.current_period
                        }
                        RowEffect::TonePortamento => {
                            ch.current_period = portamento::tone_portamento_step(
                                ch.current_period,
                                ch.tone_target,
                                ch.row_param,
                            );
                            ch.current_period
                        }
                        RowEffect::Vibrato => {
                            let offset =
                                vibrato::period_offset_for_tick(ch.row_param, &mut ch.vibrato_phase);
                            (ch.base_period as i32 + offset as i32).clamp(1, u16::MAX as i32) as u16
                        }
                        RowEffect::VolumeSlide => {
                            ch.volume = volume::slide(ch.volume, ch.row_param);
                            ch.current_period
                        }
                    };
                    if effective_period != 0 {
                        let freq = AMIGA_CLOCK / (effective_period as f64 * 2.0);
                        ch.step = freq / SAMPLE_RATE as f64;
                    }
                }
            }
            // Tick 0 needs no recompute: the trigger handling above already
            // set `step` from the note's base period.

            elapsed_target += tick_seconds;
            let target_frames = (elapsed_target * SAMPLE_RATE as f64).round() as u64;
            let frames_this_tick = target_frames.saturating_sub(frames_emitted);
            frames_emitted = target_frames;

            for _ in 0..frames_this_tick {
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

        // Advance to the next row, honoring any D/B decided on this row.
        match (position_jump_slot, pattern_break_row) {
            (Some(slot), maybe_break_row) => {
                order_idx = (slot as usize).min(order_table_len.saturating_sub(1));
                row_idx = maybe_break_row.unwrap_or(0) as usize;
            }
            (None, Some(break_row)) => {
                // Break with no jump always targets the *next* order-table
                // entry; falling off the end ends the song.
                order_idx += 1;
                row_idx = break_row as usize;
            }
            (None, None) => {
                row_idx += 1;
                if row_idx >= pattern.len() {
                    order_idx += 1;
                    row_idx = 0;
                }
            }
        }
    }

    writer.finalize().context("finalizing WAV output")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Cell, Header, Sample};
    use std::fs;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        fs::read(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("modmill-engine-test-{name}-{}.wav", std::process::id()))
    }

    /// A one-sample, one-pattern `Module` (built directly, no file needed)
    /// plus matching `raw_bytes` for `extract_pcm` -- everything before
    /// the PCM payload is unused filler, since `render_to_wav` reads
    /// pattern/sample structure from `module`, not by re-parsing `bytes`.
    fn synthetic_module(rows: Vec<[Cell; CHANNELS]>) -> (Module, Vec<u8>) {
        let pcm: Vec<i8> = (0..64).map(|i: i32| ((i * 5) % 256 - 128) as i8).collect();
        let module = Module {
            header: Header { name: "synthetic".into(), song_length: 1, restart_position: 0, order: vec![0] },
            samples: vec![Sample {
                name: "s".into(),
                length_bytes: pcm.len() as u32,
                finetune: 0,
                volume: 64,
                loop_start_bytes: 0,
                // Looped across its whole length so it keeps sounding for
                // the entire row (a one-shot sample this short exhausts
                // within tick 0, before vibrato -- ticks 1+ -- ever gets a
                // chance to influence pitch, which would make this test
                // pass for the wrong reason).
                loop_len_bytes: pcm.len() as u32,
            }],
            patterns: vec![rows],
        };
        let pattern_bytes = module.patterns.len() * PATTERN_BLOCK_BYTES;
        let mut raw = vec![0u8; PATTERN_DATA_OFFSET + pattern_bytes];
        raw.extend(pcm.iter().map(|&b| b as u8));
        (module, raw)
    }

    fn render_bytes(module: &Module, raw: &[u8], name: &str) -> Vec<u8> {
        let out = temp_path(name);
        render_to_wav(module, raw, &out).expect("render");
        let bytes = fs::read(&out).expect("read rendered wav");
        let _ = fs::remove_file(&out);
        bytes
    }

    fn vibrato_cell() -> Cell {
        // period 428 (C-3-ish), sample 1, effect 4 (vibrato), param 0x1F:
        // speed=1 (phase advances by 1/tick, slow enough not to wrap
        // within one row's ticks), depth=15 (max, so the offset is easy
        // to tell apart from silence/no-modulation).
        Cell { sample: 1, period: 428, effect: 0x4, param: 0x1F }
    }

    #[test]
    fn vibrato_phase_is_not_reset_on_a_plain_new_note() {
        // modmill-89e: real ProTracker does not reset the vibrato phase
        // on a plain retrigger (docs/format-notes/effect-timing.md §6).
        // With speed=1 the phase advances by 1 each tick and default
        // speed=6 gives 5 modulated ticks/row (ticks 1..=5), so row 2 of
        // a two-row vibrato pattern starts at phase 5, not phase 0.
        //
        // Regression shape: render a single vibrato row alone (always
        // starts at phase 0) and compare it against the SECOND row of a
        // two-row vibrato pattern. If phase were wrongly reset on each
        // trigger, both rows would start from phase 0 and their audio
        // would be byte-identical; with phase correctly carried over,
        // they must differ.
        let one_row: [Cell; CHANNELS] = [vibrato_cell(), Cell::default(), Cell::default(), Cell::default()];
        let (single_module, single_raw) = synthetic_module(vec![one_row]);
        let single_wav = render_bytes(&single_module, &single_raw, "vib-single");

        let (two_module, two_raw) = synthetic_module(vec![one_row, one_row]);
        let two_wav = render_bytes(&two_module, &two_raw, "vib-two");

        // Row length in bytes of PCM data within the WAV: PAL default
        // speed=6, bpm=125 -> 6 * (2.5/125) = 0.12s/row -> round(0.12 *
        // 44100) = 5292 frames/row * 2 bytes/frame (16-bit mono).
        const ROW_FRAMES: usize = 5292;
        const ROW_BYTES: usize = ROW_FRAMES * 2;

        let single_row_pcm = &single_wav[single_wav.len() - ROW_BYTES..];
        let two_wav_row2_pcm = &two_wav[two_wav.len() - ROW_BYTES..];

        assert_ne!(
            single_row_pcm, two_wav_row2_pcm,
            "row 2 of a retriggered vibrato note must differ from a fresh \
             single row -- if this fails, vibrato_phase is being reset on \
             note trigger again (modmill-89e regressed)"
        );
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

    #[test]
    fn pattern_break_and_position_jump_do_not_panic_and_produce_audio() {
        // jump.mod: pattern0 row0 carries D10 (BCD -> row-10 break),
        // pattern1 row0 carries B02 (jump to order slot 2). Exercises the
        // D/B sequencing wired into the render loop for modmill-4r0.14.
        let bytes = fixture("jump.mod");
        let module = crate::parser::parse(&bytes).expect("parse jump.mod");

        let out = temp_path("jump");
        render_to_wav(&module, &bytes, &out).expect("render jump.mod");

        let mut reader = hound::WavReader::open(&out).expect("reopen wav");
        assert!(reader.len() > 0, "expected non-empty audio output");
        let _: Vec<i16> = reader.samples::<i16>().map(|s| s.expect("valid sample")).collect();

        let _ = fs::remove_file(&out);
    }
}
