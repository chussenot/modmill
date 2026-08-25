//! Effects `Axy` (volume slide) and `Cxx` (set volume). Bead: modmill-4r0.13.

/// `Axy`: slide `volume` (0-64) up by `x` per tick if `x>0`, else down by
/// `y`, clamped to [0, 64]. Returns `volume` unchanged until implemented.
pub fn slide(volume: u8, _param: u8) -> u8 {
    volume
}

/// `Cxx`: set volume to `param`, clamped to 64. This one IS implemented
/// here (trivial, no per-tick state) rather than stubbed, since the
/// render loop needs a real value at tick 0 regardless of this bead's
/// status; `slide` above is the part this bead actually owns.
pub fn set(param: u8) -> u8 {
    param.min(64)
}
