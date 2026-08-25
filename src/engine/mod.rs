//! Mixer, PAL tick clock, and effect execution. Bead: modmill-4r0.8
//! (core: mixer/PAL timing/F effect) plus one bead per effect family
//! under src/engine/effects/ (modmill-4r0.10..14).

use std::path::Path;

use crate::parser::Module;

pub fn render_to_wav(_module: &Module, _out: &Path) -> anyhow::Result<()> {
    unimplemented!("modmill-4r0.8: mixer/resampler + PAL timing + F effect")
}
