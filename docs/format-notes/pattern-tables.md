# Pattern & period tables — format notes

Date: 2026-08-25
Bead: modmill-4r0.3
Author: w1-patterns

Scope: the Amiga/ProTracker note→period table, the 4-byte pattern-cell bit
layout, and the order-table / song-length / restart-position semantics.
Verified against `fixtures/minimal.mod` and `fixtures/jump.mod` with `xxd`,
cross-checked against `scripts/gen_fixtures.py` (the fixtures' own source of
truth for what values it wrote).

## 1. The period table

ProTracker note pitch is stored as an Amiga hardware **period** (the number
of PAL clock ticks between sample-word fetches), not a frequency or a MIDI
note number. **Lower period = higher pitch.** This is a fixed lookup table
in real ProTracker (`.PeriodTable`, one row per finetune value), not
something computed at runtime — a parser should embed the table, not derive
periods algorithmically. For reference/derivation only, period and frequency
relate via the PAL Amiga clock constant:

```
freq_hz ≈ 7093789.2 / (period * 2)
```

e.g. period 428 → ≈ 8287.14 Hz, the standard Amiga "middle" sample rate.

The canonical finetune-0 table spans 3 octaves / 36 entries, periods
**decreasing monotonically** as pitch rises, from 856 (lowest, C-1) down to
113 (highest, B-3):

| note | C   | C#  | D   | D#  | E   | F   | F#  | G   | G#  | A   | A#  | B   |
|------|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|
| oct1 | 856 | 808 | 762 | 720 | 678 | 640 | 604 | 570 | 538 | 508 | 480 | 453 |
| oct2 | 428 | 404 | 381 | 360 | 339 | 320 | 302 | 285 | 269 | 254 | 240 | 226 |
| oct3 | 214 | 202 | 190 | 180 | 170 | 160 | 151 | 143 | 135 | 127 | 120 | 113 |

**Finding — fixture dict keys are mislabeled by one octave, not the
periods.** `scripts/gen_fixtures.py`'s `PERIODS` dict (lines 14-16) is
commented `"a few standard Amiga/PT periods, octave 3"` and maps
`"C-3": 428, "D-3": 381, "E-3": 339, "F-3": 320, "G-3": 285, "A-3": 254`.
Those six numeric values are exactly the canonical **octave-2** row
(C-2..A-2) above, not octave 3 (canonical C-3 is 214, not 428). The task
brief for this bead even names the two anchors that confirm the canonical
table: "C-3=428" and "a high note around period 113 at the top of the
range" — 113 is canonically B-3, in the table where C-3=214, so the 428
anchor is the mismatch. The numeric periods observed in the fixtures are
correct standard PT periods; only the Python dict's *note-name labels* are
off by an octave. **Parser/formatter code (modmill-4r0.4, modmill-4r0.6)
should derive note names from a canonical period table, not from these
fixture-script dict keys**, or fixture-derived test assertions about note
names will be wrong by an octave even though period values are right.

## 2. Pattern cell bit layout (4 bytes/cell)

Each cell is 4 bytes, packing period (12 bits), sample number (6 bits),
effect command (4 bits) and effect param (8 bits):

```
byte0 = (sample_number & 0xF0) | ((period >> 8) & 0x0F)
byte1 =  period & 0xFF
byte2 = ((sample_number & 0x0F) << 4) | effect_nibble
byte3 =  effect_param
```

i.e. the sample number's nibbles are split across byte0 (high nibble) and
byte2 (low nibble, sharing byte2 with the effect command nibble); the period
is split across the low nibble of byte0 and all of byte1. This matches
`scripts/gen_fixtures.py`'s `cell()` (lines 26-33) exactly.

Confirmed against actual fixture bytes at pattern-data offset 1084
(= 20 name + 31×30 sample headers + 2 length/restart + 128 order + 4 tag):

- **`fixtures/minimal.mod`**, offset 1084, row 0 ch 0:
  bytes `01 AC 10 00`. Decodes as period `0x01AC` = 428, sample `1`,
  effect `0`, param `0` — matches `gen_fixtures.py`'s
  `cell(PERIODS["C-3"], 1, 0, 0)` for the minimal fixture's single note.

- **`fixtures/jump.mod`**, pattern 0 (offset 1084): bytes `01 AC 1D 10`.
  Period `0x01AC`=428, sample `1`, byte2 `0x1D` = `(1<<4)|0xD` → effect
  `0xD` (pattern break) with param `0x10`=16 — matches the comment "break
  to row 16 of next order slot".
- Pattern 1 (offset 1084 + 1024 = 2108): bytes `01 7D 1B 02`. Period
  `0x017D`=381, sample `1`, byte2 `0x1B` = `(1<<4)|0xB` → effect `0xB`
  (position jump) with param `0x02` — matches "jump to order slot 2".
- Pattern 2 (offset 1084 + 2048 = 3132): bytes `01 53 10 00`. Period
  `0x0153`=339, sample `1`, effect `0`, param `0`.

Sample number is 6 bits (1-31 valid, 0 = no sample change), reconstructed as
`(byte0 & 0xF0) | (byte2 >> 4)`. Period is reconstructed as
`((byte0 & 0x0F) << 8) | byte1`, and should be looked up against the table
in §1 (or matched to nearest entry, tolerating finetune-shifted values) to
get a note name — it is not itself a note index.

## 3. Order table, song length, restart-position byte

Layout, byte-exact (all fixtures, both files checked with `xxd`):

- Offset 0-19: 20-byte song name (`"minimal\0\0..."`, `"jump\0\0..."`).
- Offset 20 + i×30 for i in 0..31: 31 sample headers, 30 bytes each
  (22-byte name, then big-endian `length_words`(u16), `finetune`(i8),
  `volume`(u8), `loop_start_words`(u16), `loop_len_words`(u16) — confirmed
  byte-for-byte against `minimal.mod` offset 20: `62617369 6300...`
  ("basic\0"×17) then `00 10` (length=16 words) `00` (finetune 0)
  `40` (volume 64) `00 00` (loop start 0) `00 01` (loop len 1 word)).
- Offset 950: 1-byte **song length** — number of *valid* entries in the
  order table, range 1-128. `minimal.mod` = `01` (1 pattern played);
  `jump.mod` = `03` (3 patterns played). This is a count, not a max index.
- Offset 951: 1-byte **restart position**. Both fixtures use the
  conventional value `0x7F` (127) written by `gen_fixtures.py` unconditionally
  (comment: "restart position, conventional NoiseTracker value"). Historically
  NoiseTracker used this byte as the order-table index to jump back to once
  the whole song finishes playing (whole-song looping); ProTracker itself
  generally ignores it, though some later players honor it. `docs/spec.md`
  does not call for whole-song looping, so **treat this as an informational
  metadata field to expose in `parse --json`, not something `render`
  consumes** — flagging in case modmill-4r0.4 wants to surface it in the
  JSON header output regardless.
- Offset 952-1079: 128-byte **order table**. Entries are **pattern indices**
  into the sequence of pattern data blocks that follow the tag — NOT pattern
  data, offsets, or row numbers. Only the first `song_length` entries are
  meaningful; the rest is padding (both fixtures pad with `0x00`, confirmed:
  `minimal.mod` order table is all zeros — `[0]` + 127 zero-pad; `jump.mod`
  is `00 01 02` + 125 zero-pad, i.e. play pattern-block 0, then 1, then 2).
  A parser must stop reading the order table at `song_length`, not at the
  first non-meaningful trailing zero (zero is itself a valid pattern index,
  reused as the padding value).
- Offset 1080-1083: 4-byte format tag, `"M.K."` in both fixtures.
- Offset 1084 onward: pattern data blocks, 1024 bytes each (64 rows × 4
  channels × 4-byte cells, see §2), stored back-to-back in the order they
  are referenced by increasing pattern-block index (block 0 first, etc.) —
  *not* reordered to match the order table. The order table is what maps
  playback position → block index; block storage order is independent of
  play order (`jump.mod`'s blocks happen to be stored 0,1,2 which is also
  its play order, but nothing in the format requires that).
- Sample PCM data follows all pattern blocks, concatenated in sample-header
  order, length in bytes = `2 * length_words` per header.

## 4. Format tag ("M.K." vs. others) — scope note

`"M.K."` at offset 1080 marks the standard 31-sample ProTracker format,
which is the **only** format `docs/spec.md` puts in scope for this project
("31-sample format only (no 15-sample / non-`M.K.` variants)"). Documented
here only so nobody re-derives this from scratch mid-parser:

- Other 4-byte tags seen in the wild at the same offset: `"4CHN"`, `"6CHN"`,
  `"8CHN"` (FastTracker channel-count extensions), `"FLT4"`/`"FLT8"`
  (StarTrekker), `"M!K!"` (ProTracker variant for >64 patterns), `"CD81"`
  (Falcon), among others.
- The original Ultimate SoundTracker/NoiseTracker **15-sample format** has
  no tag at all — no 4 magic bytes at offset 1080, because that format's
  header doesn't have this field: only 15 sample headers, a different song
  name length convention, and the whole file is ~130 bytes shorter as a
  result. A parser must not assume offset 1080 is always meaningful; it must
  be prepared to see file lengths/structure inconsistent with the 31-sample
  layout and treat that as "not our format."
- Per spec.md, any non-`M.K.` tag (or its absence) is a silent no-op, logged
  once to stderr and listed under "Out of scope" — modmill should detect and
  reject/skip these cleanly rather than mis-parse them as if they were
  31-sample `M.K.` files.
