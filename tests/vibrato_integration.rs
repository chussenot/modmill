//! Smoke test for effect `4` (vibrato), bead modmill-4r0.12: rendering
//! `fixtures/effects.mod` end-to-end must not panic and must produce
//! non-empty audio. Not a byte-exact hash -- sibling effect beads
//! (arpeggio, portamento) are still landing, so the file's bytes aren't
//! pinned yet.

use std::path::Path;
use std::process::Command;

#[test]
fn effects_mod_renders_without_panic_and_is_non_empty() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("vibrato_effects.wav");
    let status = Command::new(env!("CARGO_BIN_EXE_modmill"))
        .args(["render", "fixtures/effects.mod", "-o"])
        .arg(&out)
        .status()
        .expect("running modmill render");
    assert!(status.success(), "render of fixtures/effects.mod exited non-zero");

    let bytes = std::fs::read(&out).expect("reading rendered WAV");
    // A WAV header alone is 44 bytes; require real PCM data past that.
    assert!(bytes.len() > 44, "rendered WAV has no audio data: {} bytes", bytes.len());
}
