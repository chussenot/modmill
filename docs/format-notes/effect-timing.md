# Effect timing folklore (ProTracker 31-sample `.mod`)

Date: 2026-08-25
Bead: modmill-4r0.2
Scope: the effects listed in `docs/spec.md` — `0` arpeggio, `1`/`2`
slide up/down, `3` tone portamento, `4` vibrato, `A` volume slide,
`C` set volume, `D` pattern break, `B` position jump, `F` set speed/tempo.

All byte values below were read directly out of `fixtures/effects.mod`,
`fixtures/speed.mod` and `fixtures/jump.mod` with `xxd`, cross-checked
against the values `scripts/gen_fixtures.py` wrote. Pattern data starts at
file offset **1084** (`20` title + `31*30` sample headers + `1` song
length + `1` restart byte + `128` order table + `4` `M.K.` tag). Each row
is `4` channels × `4` bytes = `16` bytes; a channel's cell is
`offset = 1084 + row*16 + channel*4`. Cell layout (per `gen_fixtures.py`
and `docs/spec.md`'s implied layout):

```
byte0 = (sample_hi<<0 already in top nibble) | period_hi_nibble
byte1 = period_lo_byte
byte2 = (sample_lo_nibble<<4) | effect_nibble
byte3 = effect_param
```

## 1. Tick within the row each effect first acts on

ProTracker's row loop runs `speed` ticks per row (tick 0..speed-1). Tick 0
is the "note trigger" tick: any new note/instrument/volume-column value on
the row is latched and the sample restarts (if applicable) at rest — i.e.
un-slid pitch, un-arpeggiated pitch, base volume. Continuous per-tick
effects then step forward on ticks 1..speed-1. Row-level commands are
processed once, during tick-0 row setup, and never re-applied on later
ticks of the same row.

| Effect | Byte | First acts on | Notes |
|---|---|---|---|
| Arpeggio | `0xy` | every tick, but tick 0's table index is 0 → base note, so it *reads* as "tick 0 = plain note" | see §2 |
| Slide up | `1xx` | tick 1 | period -= xx each tick, ticks 1..speed-1 |
| Slide down | `2xx` | tick 1 | period += xx each tick, ticks 1..speed-1 |
| Tone portamento | `3xx` | tick 1 | period steps toward target by xx each tick, ticks 1..speed-1; tick 0 keeps the just-triggered period |
| Vibrato | `4xy` | tick 1 | sine offset is 0 at tick 0's phase; see §3 |
| Volume slide | `Axy` | tick 1 | volume +x or -y each tick, ticks 1..speed-1 |
| Set volume | `Cxx` | tick 0, once | row-level, not per-tick |
| Pattern break | `Dxx` | tick 0, once | row-level; only takes effect when the row finishes (i.e. after all speed ticks), but the *decision* is latched at tick 0 |
| Position jump | `Bxx` | tick 0, once | row-level, same latch-at-tick-0 / act-at-row-end timing as `D` |
| Set speed/tempo | `Fxx` | tick 0, once | row-level; changes the tick counter/BPM used for the *rest of playback*, starting from this row |

Confirmed in `fixtures/effects.mod`, pattern offset 1084:

```
row0 (0x043c): 01 ac 10 37   -> period=0x1AC(428,C-3) sample=1 effect=0 param=0x37 (arpeggio +3 +7)
row1 (0x044c): 00 00 01 08   -> effect=1 param=0x08   (slide up, no new note -> continues from current period)
row2 (0x045c): 00 00 02 08   -> effect=2 param=0x08   (slide down)
row3 (0x046c): 01 53 13 04   -> period=0x153(339,E-3) effect=3 param=0x04 (tone portamento toward E-3)
row4 (0x047c): 01 ac 14 64   -> period=0x1AC(428,C-3) effect=4 param=0x64 (vibrato speed=6 depth=4)
row5 (0x048c): 00 00 0a 0f   -> effect=A param=0x0F   (volume slide up 15/tick)
row6 (0x049c): 00 00 0a f0   -> effect=A param=0xF0   (volume slide down 15/tick)
row7 (0x04ac): 00 00 0c 20   -> effect=C param=0x20   (set volume 32, decimal — C's param is a plain 0-64 volume, not BCD)
```

Rows 1/2/5/6 carry no new note (period/sample = 0), which is exactly how
real PT expresses "continue sliding the currently-playing note" — the
slide/volume-slide effects have nothing to retrigger, so their per-tick
math is the only thing happening on that row.

## 2. Arpeggio's 3-note cycle

`0xy`: `x` and `y` are semitone offsets (0-15 each) above the row's base
note. ProTracker cycles the *sounding* pitch by `tick % 3`:

- `tick % 3 == 0` → base note (offset 0)
- `tick % 3 == 1` → base + `x` semitones
- `tick % 3 == 2` → base + `y` semitones

then repeats for the remaining ticks in the row. Because tick 0's index is
always 0 (offset 0), arpeggio is consistent with the "tick 0 = plain note"
row above without needing a special case — the modulo-3 table already
produces the base pitch there.

`fixtures/effects.mod` row0: `effect=0 param=0x37` → `x=3, y=7`: at
speed 6 the per-tick pitch sequence is `base, +3, +7, base, +3, +7`.

## 3. Vibrato's sine-table stepping

`4xy`: `x` = speed (table position increment per tick), `y` = depth
(amplitude divisor). ProTracker keeps a per-channel phase counter
`pos` (0..63, wrapping) and a fixed 32-entry table of sine magnitudes
(`0,24,49,74,97,120,141,161,180,197,212,224,235,244,250,253,` mirrored
descending back to `24`) representing one quarter-then-mirrored full
cycle:

- table index = `pos & 31`
- sign = negative if `pos & 32` is set (second half of the cycle)
- `delta = (table[index] * y) >> 7`, added to (or subtracted from) the
  row's base period
- `pos` advances by `x` **after** tick 0 — i.e. `pos` starts at whatever
  it was left at (it does *not* reset on a new note unless the effect is
  freshly triggered on a row with no prior vibrato state), and the first
  increment happens going into tick 1, matching the general "tick 1+"
  rule in §1.

`fixtures/effects.mod` row4: `effect=4 param=0x64` → `x=6` (speed),
`y=4` (depth): the pitch wobbles at a fast rate with a shallow (depth 4
of max ~255) swing around C-3 (period 428).

## 4. `F` effect: speed vs. tempo boundary at `0x20`

`Fxx`: plain binary parameter (not BCD — contrast with `D` below).

- `xx < 0x20` (0-31 decimal) → sets **speed**: ticks per row.
- `xx >= 0x20` (32-255 decimal) → sets **tempo**: BPM directly.
- `xx == 0x00` is a documented real-PT edge case: speed 0 means "advance
  0 ticks per row", which freezes row advancement — some historical PT
  versions use this deliberately as a player-side pause/halt trick, but
  it is easy to turn into an infinite loop in a naive re-implementation.
  Flag it rather than silently treating it as a no-op.

Confirmed in `fixtures/speed.mod`, pattern offset 1084:

```
row0 (0x043c): 01 ac 1f 03   -> effect=F param=0x03  (0x03 < 0x20 -> speed = 3 ticks/row)
row1 (0x044c): 00 00 0f 78   -> effect=F param=0x78  (0x78 = 120 >= 0x20 -> tempo = 120 BPM)
```

`0x78` = 120 decimal is exactly the BPM value the fixture comment claims
("tempo = 120 BPM"), confirming `F`'s param is read as a plain integer,
not decimal-digit-packed.

## 5. `D` pattern break and `B` position jump

**`Bxx`** — position jump: `xx` is the **0-based order-table index** to
jump to; playback continues at row 0 of that pattern unless combined with
`D` on the same row (see precedence below). `xx` is a plain binary index,
0-127.

**`Dxx`** — pattern break: jump to the *next* slot in the order table,
starting at a given row of that pattern. **The critical folklore point:
`xx` is packed BCD (two decimal digits), not a raw hex/binary row
number** — `target_row = (xx >> 4) * 10 + (xx & 0x0F)`. This is a
long-documented ProTracker quirk (it appears in the classic MOD-format
spec notes as "yy is written as two decimal digits, not hexadecimal —
some players get this wrong") and is a real trap for a from-scratch
implementer.

Confirmed in `fixtures/jump.mod`:

```
order table (offset 950): length=0x03, restart=0x7F, order=[0x00,0x01,0x02, 0,0,...]
pattern0 row0 (offset 1084): 01 ac 1d 10  -> effect=D param=0x10
pattern1 row0 (offset 2108): 01 7d 1b 02  -> effect=B param=0x02
pattern2 row0 (offset 3132): 01 53 10 00  -> no effect
```

`D`'s param here is `0x10`. The fixture generator's comment claims this
breaks "to row 16 of next order slot" — that reading treats `0x10` as
plain hex/decimal (16). **Real ProTracker's BCD decode gives
`(0x1>>4... wait: hi nibble=1, lo nibble=0) = 1*10 + 0 = row 10`, not
16.** This is exactly the divergence this bead exists to catch: if
modmill decodes `D`'s param as a plain integer instead of BCD, every
pattern-break target row will be wrong for any param whose hex value
differs from its two-decimal-digit reading (anything ≥ `0x0A`). **Action
for the patterns module (modmill-4r0.6): decode `D`'s param as BCD
(`(param >> 4) * 10 + (param & 0xF)`), matching real ProTracker — and note
that this fixture's own comment is written in the wrong (hex) convention,
so don't copy the comment's row number into a test's expected value.**

`B`'s param here is `0x02`, a plain index into the order table — and the
order table at offset 950 confirms slots `[0]=pattern0, [1]=pattern1,
[2]=pattern2`, so `B02` targets pattern2, matching the fixture comment
("jump to order slot 2").

**Precedence when `B` and `D` land on the same row** (typically on two
different channels): real ProTracker lets **`D` win the row target** and
**`B` win the pattern/position target** — i.e. the combined effect is
"jump to the order-table slot named by `B`'s param, starting at the row
named by `D`'s (BCD-decoded) param." If only `D` appears, the position
target defaults to the next order-table slot (`current + 1`) with `D`'s
row. If only `B` appears, the row target defaults to row 0 of `B`'s
target slot.

## 6. Known ProTracker-vs-clone divergences to flag

- **Pattern-break BCD decoding** (see §5): many non-PT-authentic
  players/parsers treat `Dxx` as a raw row number instead of two packed
  decimal digits. OpenMPT's "ProTracker 1/3 compatibility" mode
  implements the BCD decode; its default/FastTracker-2 mode historically
  did too for `.mod` (XM's pattern break kept the same BCD convention for
  compatibility) — but bespoke or "intuitive" reimplementations frequently
  get this wrong. Decide and document modmill's choice explicitly (this
  note recommends BCD, matching authentic PT).
- **`F00` (speed = 0)**: authentic PT freezes row advancement; some
  clones/trackers clamp it to a minimum speed or silently no-op it to
  avoid a hang. Pick one and document it — don't let it become an
  accidental infinite loop in the renderer.
- **Tone portamento (`3xx`) and instrument/sample retrigger**: authentic
  PT does not restart the sample position when a `3xx` row also carries a
  new note — the slide continues instead. FastTracker2/XM-descended
  players are more willing to retrigger on a new instrument number even
  during a portamento slide. modmill's spec targets PT semantics; don't
  import FT2/XM retrigger behavior here.
- **Vibrato/tremolo phase reset**: authentic PT does *not* reset the
  vibrato sine-table position (`pos` in §3) on a plain new note — only an
  explicit "retrigger vibrato waveform" effect (out of scope for modmill)
  resets it. A naive implementation that resets vibrato phase on every
  note will drift from authentic PT output and fail the byte-identical
  WAV conformance suite.
- **Arpeggio at low speed**: at `speed < 3` the 3-entry cycle never
  completes within a row (e.g. speed=2 only ever plays `base, +x`); this
  is correct PT behavior, not a bug — don't special-case it to "always
  play all three notes."

## 7. Summary for implementers

- Row-level effects (`C`, `D`, `B`, `F`) are decided once at tick 0 and
  applied at the appropriate point (immediately for `C`/`F`, at row-end
  for `D`/`B`'s pattern transition); per-tick effects (`0`,`1`,`2`,`3`,`4`,`A`)
  first change the sounding state going into tick 1, not tick 0.
- `D`'s param is BCD two-digit decimal; `B` and `F`'s params are plain
  binary. Don't apply the same parsing function to both.
- `F`'s speed/tempo boundary is `0x20`: `<0x20` → speed (ticks/row),
  `>=0x20` → tempo (BPM). `F00` needs an explicit policy.
- When `D` and `B` co-occur on a row: `B` picks the destination pattern
  (order-table slot), `D` picks the destination row within it.
