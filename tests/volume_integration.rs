//! Smoke test for `A` (volume slide) / `C` (set volume) end-to-end
//! rendering, bead modmill-4r0.13. Deliberately not a golden-hash test:
//! sibling effect beads (arpeggio/vibrato/portamento) are still landing
//! against the same `fixtures/effects.mod`, so a byte-exact hash here
//! would be flaky against unrelated, unmerged work. Only checks that
//! rendering the fixture end-to-end does not panic and produces audio.

use std::path::Path;
use std::process::Command;

#[test]
fn effects_mod_renders_without_panicking_and_produces_audio() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("volume_effects.wav");
    let status = Command::new(env!("CARGO_BIN_EXE_modmill"))
        .args(["render", "fixtures/effects.mod", "-o"])
        .arg(&out)
        .status()
        .expect("running modmill render");
    assert!(status.success(), "render of fixtures/effects.mod failed");

    let mut reader = hound::WavReader::open(&out).expect("reopen rendered wav");
    assert!(reader.len() > 0, "expected non-empty audio output");
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .map(|s| s.expect("valid sample"))
        .collect();
    assert!(!samples.is_empty(), "expected decodable PCM samples");
}
