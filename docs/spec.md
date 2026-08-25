# modmill — ProTracker .mod parser & offline renderer

## Scope

`modmill` reads 31-sample ProTracker `.mod` files and either dumps their
structure as JSON or renders them to a PCM WAV file, deterministically.

```
modmill parse <file> --json      # header, samples, pattern table, note/effect grid
modmill render <file> -o out.wav # 31-sample ProTracker semantics, PAL timing
```

## `parse --json`

Emits: module name, sample table (name, length, finetune, volume, loop
start/length), the pattern order table, and for every pattern a 4-channel
grid of (note, sample, effect, effect-param) cells.

## `render`

- 31-sample format only (no 15-sample / non-`M.K.` variants).
- PAL timing: CIA default tempo of 125 BPM / speed 6 (i.e. 6 ticks per row
  at 50Hz row-tick rate unless changed by an `F` effect).
- Effects in scope, applied per-tick per ProTracker's documented tick
  timing (arpeggio/vibrato/slides step every tick after the first; slides
  and volume changes take effect starting tick 1):
  - `0` arpeggio
  - `1` / `2` portamento up/down (slide)
  - `3` tone portamento
  - `4` vibrato
  - `A` volume slide
  - `C` set volume
  - `D` pattern break
  - `B` position jump
  - `F` set speed/tempo (`<0x20` = speed in ticks/row, `>=0x20` = BPM)
- Anything else (samples with unsupported flags, effects outside the list
  above, non-`M.K.` tags) is a silent no-op, logged once per occurrence to
  stderr, and listed under "Out of scope" in the README.

## Determinism

Rendering the same input file always produces a byte-identical WAV. The
golden-hash-per-fixture table under `fixtures/` *is* the conformance
suite — no perceptual/approximate comparison.

## House rules

- `clippy -D warnings`, no `unsafe`.
- Dependencies: the pact five (workspace-standard crates — see
  `Cargo.toml`) plus `hound` for WAV encoding. `hound` is a ~600-line,
  zero-dependency, actively maintained crate that does exactly WAV
  header/PCM writing with no scope creep — cheaper to depend on than to
  hand-roll and maintain a byte-exact WAV writer ourselves.
