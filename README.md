# modmill

`modmill` is a ProTracker 31-sample `.mod` parser and offline WAV renderer,
built as a proving ground for the pact 0.15 multi-agent coordination
protocol. It reads a `.mod` file's header, sample table, order table and
pattern data, and either dumps that structure as JSON or renders it,
deterministically, to a PCM WAV file using PAL Amiga timing.

## Usage

Parse a module and print its structure as JSON:

```
$ modmill parse fixtures/effects.mod --json
{
  "name": "effects",
  "samples": [ { "name": "basic", "length": 32, "finetune": 0, "volume": 64, "loop_start": 0, "loop_len": 0 }, ... ],
  "order": [0],
  "patterns": [ { "rows": [ [ { "note": "C-3", "sample": 1, "effect": 0, "param": 55 }, ... ], ... ] } ]
}
```

Render a module to a WAV file:

```
$ modmill render fixtures/effects.mod -o out.wav
```

## Collecting and playing real-world modules

`fixtures/*.mod` are synthetic (see `fixtures/ATTRIBUTION.md`) and are the
committed conformance suite — CI never downloads anything. To hear modmill
against real music instead, `scripts/fetch-mods.sh` pulls public-domain
ProTracker modules from The Mod Archive into `testdata/mods/` (gitignored:
real test data, not a fixture):

```
$ scripts/fetch-mods.sh discover 4      # auto-harvest 4 public-domain M.K. modules
# or, to pick specific ones by hand: fill in the MODS array at the top of
# the script (moduleid|filename|title|author|license-url), then just run
# scripts/fetch-mods.sh with no arguments.
```

Each download is verified as a 4-channel `M.K.` module from its own bytes
(not from site markup, which the script's header explains was tried first
and proved unreliable), smoke-tested through `modmill parse`, and recorded
with its title, source URL and sha256 in `testdata/mods/ATTRIBUTION.md`.

Then render and play it with whatever WAV player you have — `aplay`
(ALSA, Linux) or `mpv` both just work on a plain PCM WAV:

```
$ cargo run --release -- render testdata/mods/modarchive-32703.mod -o /tmp/song.wav
$ aplay /tmp/song.wav        # or: mpv /tmp/song.wav
```

(`--release` matters here — the mixer's linear-interpolation resampling is
fast enough in release mode to render in a fraction of a second; a debug
build works too, just slower.)

## Scope: implemented effects

Effects are applied per-tick, per ProTracker's documented tick timing:
row-level commands (`C`, `D`, `B`, `F`) are decided once at tick 0;
per-tick commands (`0`, `1`, `2`, `3`, `4`, `A`) first change the sounding
state going into tick 1, not tick 0.

| Nibble | Effect | Description |
|---|---|---|
| `0` | Arpeggio | Cycles the sounding pitch by `tick % 3`: base note, then `+x` semitones, then `+y` semitones (params `xy`), repeating for the row. |
| `1` / `2` | Slide up / down | Adjusts period by `param` every tick starting tick 1 (up = period decreases, i.e. pitch rises); clamped to the canonical Amiga period range `[113, 856]`. |
| `3` | Tone portamento | Steps the period toward a target note by up to `param` per tick, snapping exactly to the target instead of overshooting; does not retrigger the sample. |
| `4` | Vibrato | Oscillates period around the base pitch using a per-channel sine phase; speed = high nibble of `param` (phase steps/tick), depth = low nibble (amplitude). |
| `A` | Volume slide | Adjusts channel volume every tick starting tick 1; high nibble of `param` is the up-amount, low nibble is the down-amount, clamped to `[0, 64]`. |
| `C` | Set volume | Sets channel volume once, at tick 0, to `param` (a plain 0-64 value, clamped to 64). |
| `D` | Pattern break | Row-level; jumps to the next order-table slot (or the slot named by a co-occurring `B`), starting at a row decoded from `param` as packed BCD: `(param >> 4) * 10 + (param & 0xF)`. |
| `B` | Position jump | Row-level; jumps to the order-table slot given by `param` (a plain binary index, 0-127), starting at row 0 unless combined with `D`. |
| `F` | Set speed/tempo | Row-level; `param < 0x20` sets speed (ticks per row), `param >= 0x20` sets tempo (BPM) directly. |

When `D` and `B` land on the same row, `B` picks the destination pattern
(order-table slot) and `D` picks the destination row within it.

## Out of scope

Anything not listed above is a silent no-op, logged once per occurrence to
stderr, per `docs/spec.md`:

- **All other effect nibbles/commands** not in the table above (e.g. `5`-`9`,
  `E` sub-effects, tremolo, sample offset, retrigger, fine slides, etc.).
- **Non-`M.K.` format tags**: `4CHN`, `6CHN`, `8CHN` (FastTracker
  channel-count extensions), `FLT4`/`FLT8` (StarTrekker), `M!K!`
  (ProTracker >64-pattern variant), `CD81` (Falcon), and any other 4-byte
  tag at file offset 1080.
- **The 15-sample Ultimate SoundTracker/NoiseTracker format**, which has no
  format tag at all and a different header layout entirely.
- **Whole-song looping via the restart-position byte** (offset 951):
  exposed as informational metadata in `parse --json`, but not consumed by
  `render` — `docs/spec.md` does not call for it, and authentic ProTracker
  itself generally ignores this byte.
- **Vibrato/tremolo waveform retrigger**: an explicit "retrigger vibrato
  waveform" effect exists in real PT but is out of scope here; modmill's
  vibrato phase is carried across a plain retrigger instead (matching
  authentic PT — see `modmill-89e`), so only that dedicated effect would
  ever reset it.
- **Sample flags/fields beyond name, length, finetune, volume, and loop
  start/length**: any other malformed or out-of-range sample data (e.g. a
  loop region exceeding the declared sample length) is clamped rather than
  reproducing PT's undefined out-of-bounds read.

## Findings from implementation (W3 handoffs)

Each in-scope effect was implemented against `docs/format-notes/*.md` and
checked against the fixtures; a few things diverged from what the docs
claimed, or from what a naive reading would suggest:

- **Arpeggio (`0`)**: `docs/format-notes/effect-timing.md` §2 turned out to
  be fully accurate — no divergence between the doc and the implementation.
  Semitone-to-period conversion (`period = base_period / 2^(semitones/12)`,
  rounded) was the one detail the doc left unspecified, since it's a
  synthesis detail rather than a PT folklore trap.
- **Pattern break (`D`)**: `fixtures/jump.mod`'s own generator comment
  claims `D10` breaks "to row 16 of next order slot," but `D`'s param is
  packed BCD, not hex/decimal — `(0x10 >> 4) * 10 + (0x10 & 0xF)` decodes
  to row **10**, not 16. If any other doc or fixture comment claims
  otherwise, that reading is wrong per the sequencing bead's handoff;
  modmill implements the BCD decode.
- **Volume slide (`A`)**: `effect-timing.md` never states what happens when
  both nibbles of the param are nonzero (real PT modules never do this,
  but a hand-crafted one could). modmill's choice: the up-nibble wins and
  the down-nibble is silently ignored, matching authentic ProTracker.
- **Vibrato (`4`)**: implemented with a simpler 32-entry full-cycle signed
  sine table (rather than the doc's 64-position mirrored-quarter+sign-bit
  scheme) — functionally equivalent, same peak amplitude. `src/engine/mod.rs`
  originally reset vibrato phase on every note trigger, diverging from
  `effect-timing.md` §6's claim that authentic PT does *not* reset phase on
  a plain new note; fixed post-run (`modmill-89e`) — phase now carries
  across a plain retrigger, with a regression test built from a synthetic
  `Module` guarding against the reset coming back.
- **Portamento (`1`/`2`/`3`)**: the docs describe the per-tick slide math
  but say nothing about bounds. modmill clamps slid periods to the
  canonical Amiga period-table extremes `[113, 856]` to avoid underflow or
  an absurd value on a long slide.

## Determinism

Rendering the same input file always produces a byte-identical WAV. There
is no perceptual or approximate comparison; conformance is judged by exact
output equality. Note: a byte-exact golden-hash test across all in-scope
effects sharing `fixtures/effects.mod` was deliberately not landed yet, since
not every effect bead had finished when portamento's tests were written —
see `tests/portamento_integration.rs` for the smoke-test standing in for it
in the meantime.
