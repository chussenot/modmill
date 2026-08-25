//! Per-effect-family decode/step helpers. One file per bead
//! (modmill-4r0.10..14). Each function here is pure (no engine state) so
//! the render loop in `super` owns all mutable per-channel state and these
//! modules can be implemented concurrently without touching it.
//!
//! Per the inherited effect-timing handoff: arpeggio/slides/tone-portamento
//! /vibrato/volume-slide only change the sounding state starting TICK 1 of
//! a row (tick 0 is the note-trigger tick); `super::render_to_wav` only
//! calls these `*_tick` functions for `tick_in_row >= 1`.
//!
//! Until a bead lands, its module returns the input unchanged (a true
//! no-op) so a render exercising an unfinished effect still produces
//! valid, unpanicking (if not yet correct) audio.

pub mod arpeggio;
pub mod portamento;
pub mod sequencing;
pub mod vibrato;
pub mod volume;
