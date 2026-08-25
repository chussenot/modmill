//! Smoke test for effects `1xx`/`2xx`/`3xx` (bead modmill-4r0.11): render
//! `fixtures/effects.mod` end-to-end and assert it doesn't panic and
//! produces non-empty audio. Deliberately NOT a byte-exact hash lock --
//! other effect beads (arpeggio, vibrato, volume slide) share this fixture
//! and are still landing, which would make a hash-lock flaky.

use std::path::Path;
use std::process::Command;

#[test]
fn effects_mod_renders_without_panicking_and_produces_audio() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("portamento_smoke.wav");
    let status = Command::new(env!("CARGO_BIN_EXE_modmill"))
        .args(["render", "fixtures/effects.mod", "-o"])
        .arg(&out)
        .status()
        .expect("running modmill render");
    assert!(status.success(), "render should succeed, not panic");

    let bytes = std::fs::read(&out).expect("reading rendered WAV");
    // WAV header alone is 44 bytes; require actual sample data beyond that.
    assert!(
        bytes.len() > 44,
        "rendered WAV should contain audio data, got {} bytes",
        bytes.len()
    );
}
