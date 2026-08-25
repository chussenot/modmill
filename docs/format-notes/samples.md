# Sample header semantics (2026-08-25)

Scope: ProTracker 31-sample `.mod` sample headers — field encoding and loop
quirks a parser/renderer must handle. Byte offsets below are absolute file
offsets, cited from `fixtures/minimal.mod` and `fixtures/loop.mod` (both
read with `xxd`).

## Layout

The song name occupies file offset `0..19` (20 bytes). The 31 sample
headers follow immediately, 30 bytes each, starting at offset `20`:

| field       | size | offset (within header) | encoding                     |
|-------------|------|-------------------------|-------------------------------|
| name        | 22 B | 0..21                   | ASCII, NUL-padded             |
| length      | 2 B  | 22..23                  | big-endian, **in words**      |
| finetune    | 1 B  | 24                      | low nibble, signed            |
| volume      | 1 B  | 25                      | 0..64                         |
| loop_start  | 2 B  | 26..27                  | big-endian, **in words**      |
| loop_len    | 2 B  | 28..29                  | big-endian, **in words**      |

Sample *i* (0-indexed) header starts at file offset `20 + i*30`.

## Observed bytes

**`fixtures/minimal.mod`, sample 0** (file offset `0x14`/20..`0x31`/49):

```
0000014: 6261 7369 6300 0000 0000 0000 0000 0000  basic...........
0000024: 0000 0000 0000 0010 0040 0000 0001        .........@....
```

- name: `"basic"` + NUL padding (offset 20..41)
- length: bytes 42..43 = `00 10` = 16 words = **32 bytes**
- finetune: byte 44 = `00`
- volume: byte 45 = `40` = **64** (max)
- loop_start: bytes 46..47 = `00 00` = 0 words
- loop_len: bytes 48..49 = `00 01` = **1 word** → no-loop convention (see below)

**`fixtures/loop.mod`, sample 0** (file offset `0x14`/20..`0x31`/49):

```
0000014: 6c6f 6f70 7900 0000 0000 0000 0000 0000  loopy...........
0000024: 0000 0000 0000 0020 0030 0008 0010        ....... .0....
```

- name: `"loopy"` + NUL padding
- length: bytes 42..43 = `00 20` = 32 words = **64 bytes**
- finetune: byte 44 = `00`
- volume: byte 45 = `30` = **48**
- loop_start: bytes 46..47 = `00 08` = **8 words = byte offset 16**
- loop_len: bytes 48..49 = `00 10` = **16 words = 32 bytes**

This is a genuine sustain loop: the region `[16, 48)` (bytes) repeats
after the sample first plays through from 0, and `16 + 32 = 48 <= 64`
(the total sample length), so the loop fits entirely inside the sample.

## Units: words, not bytes

`length`, `loop_start`, and `loop_len` are all stored **in words** (2
bytes each). Every one of them must be multiplied by 2 to get a byte
offset/count before indexing into the raw sample PCM data. Getting this
wrong is the single most common .mod-parsing bug: it silently produces
loop points at half the intended offset.

## Finetune: signed nibble

`finetune` is stored in a full byte but only the **low nibble** (bits
0..3) is meaningful; ProTracker itself only ever writes 0..15 there. It
represents a signed 4-bit value, values 0..7 mean finetune 0..+7 and
values 8..15 mean finetune -8..-1 (i.e. interpret the nibble as two's
complement: `if nibble >= 8 { nibble as i8 - 16 } else { nibble as i8 }`).
Both fixtures inspected have finetune byte `0x00` (finetune 0), so no
nonzero example was observed in `fixtures/`; implementers should still
mask to the low nibble defensively in case a byte has stray high bits.

## Volume

`volume` is a linear 0..64 value (0x00..0x40); 64 is full volume. Neither
fixture exceeds this range, but a parser should clamp/reject values > 64
from malformed files rather than trust them blindly, since PT's own UI
never writes above 64.

## Loop convention: `loop_len <= 1` means "no loop"

ProTracker's convention (and the one both fixtures encode): a sample has
**no loop** when `loop_len_words <= 1`. `minimal.mod` sample 0 sets
`loop_start = 0`, `loop_len = 1` — this is the canonical "disabled loop"
encoding, *not* a 2-byte loop at the start of the sample. A parser must
special-case `loop_len_words <= 1` before computing byte offsets, or it
will produce a spurious 0- or 2-byte loop region instead of treating the
sample as one-shot.

## One-shot vs. sustain loop

- **One-shot** (`loop_len_words <= 1`): the sample plays once, start to
  end, and stops (or the channel goes silent) when it reaches
  `length_bytes`. `minimal.mod` sample 0 is this case.
- **Sustain loop** (`loop_len_words > 1`): the sample plays once from
  byte 0 to `loop_start_bytes + loop_len_bytes` (the initial "attack"
  portion before the loop point contributes only if `loop_start > 0`),
  then repeats the region `[loop_start_bytes, loop_start_bytes +
  loop_len_bytes)` indefinitely until the channel is retriggered, muted,
  or the row's note changes. `loop.mod` sample 0 is this case: attack is
  bytes `[0, 48)` the first time through, then it loops `[16, 48)`.

## Loop region exceeding sample length

Nothing in the header format prevents `loop_start_bytes +
loop_len_bytes > length_bytes` in a malformed or hand-edited module.
Real ProTracker does not validate this at load time — it will happily
read/loop past the declared sample length into whatever PCM data follows
in the sample pool (typically the next sample's data, or silence at
EOF), producing audible garbage. A robust parser should treat this as an
out-of-range condition and either clamp `loop_start + loop_len` to
`length`, or treat the sample as unlooped, rather than reproducing PT's
undefined out-of-bounds read.
