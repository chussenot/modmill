#!/usr/bin/env python3
"""Hand-craft ProTracker (31-sample, M.K.) .mod fixtures for modmill.

Not part of the shipped crate — a one-off dev tool. Run manually to
regenerate fixtures/*.mod. See tests/tiny_mod.rs for the in-Rust fixture
that keeps CI independent of this script and of fixtures/ entirely.
"""
import struct
import sys
from pathlib import Path

FIXTURES = Path(__file__).resolve().parent.parent / "fixtures"

PERIODS = {  # a few standard Amiga/PT periods, octave 3
    "C-3": 428, "D-3": 381, "E-3": 339, "F-3": 320, "G-3": 285, "A-3": 254,
}


def sample_header(name: bytes, length_words: int, finetune: int, volume: int,
                   loop_start_words: int, loop_len_words: int) -> bytes:
    name = name[:22].ljust(22, b"\x00")
    return name + struct.pack(">HbBHH", length_words, finetune, volume,
                               loop_start_words, loop_len_words)


def cell(period: int, sample: int, effect: int, param: int) -> bytes:
    # 4-byte cell: [sample_hi(4)|period_hi(4)] [period_lo(8)]
    # [sample_lo(4)|effect(4)] [param(8)]
    b0 = ((sample & 0xF0)) | ((period >> 8) & 0x0F)
    b1 = period & 0xFF
    b2 = ((sample & 0x0F) << 4) | (effect & 0x0F)
    b3 = param & 0xFF
    return bytes([b0, b1, b2, b3])


EMPTY = cell(0, 0, 0, 0)


def pattern(rows: list[list[bytes]]) -> bytes:
    """rows: up to 64 rows, each a list of up to 4 channel cells."""
    out = bytearray()
    for r in range(64):
        row = rows[r] if r < len(rows) else []
        for ch in range(4):
            out += row[ch] if ch < len(row) else EMPTY
    assert len(out) == 1024
    return bytes(out)


def build_mod(name: str, samples: list[dict], patterns: list[bytes],
              order: list[int]) -> bytes:
    out = bytearray()
    out += name.encode("ascii")[:20].ljust(20, b"\x00")
    for i in range(31):
        if i < len(samples):
            s = samples[i]
            out += sample_header(s["name"].encode("ascii"), s["length_words"],
                                  s["finetune"], s["volume"],
                                  s["loop_start_words"], s["loop_len_words"])
        else:
            out += sample_header(b"", 0, 0, 0, 0, 1)
    out += bytes([len(order)])
    out += bytes([0x7F])  # restart position, conventional NoiseTracker value
    order_table = order + [0] * (128 - len(order))
    out += bytes(order_table[:128])
    out += b"M.K."
    for p in patterns:
        out += p
    for s in samples:
        out += s["data"]
    return bytes(out)


def pcm(n: int, kind: str) -> bytes:
    """Deterministic signed-8-bit PCM: a simple ramp/square/silence."""
    if kind == "silence":
        return bytes([0]) * n
    if kind == "square":
        return bytes([64 if (i // 8) % 2 == 0 else 192 for i in range(n)])
    if kind == "ramp":
        return bytes([(i * 3) & 0xFF for i in range(n)])
    raise ValueError(kind)


def main():
    FIXTURES.mkdir(exist_ok=True)

    # 1. minimal.mod — one sample, one pattern, no effects.
    smp = {"name": "basic", "length_words": 16, "finetune": 0, "volume": 64,
           "loop_start_words": 0, "loop_len_words": 1, "data": pcm(32, "square")}
    pat = pattern([[cell(PERIODS["C-3"], 1, 0, 0)]])
    Path(FIXTURES / "minimal.mod").write_bytes(
        build_mod("minimal", [smp], [pat], [0]))

    # 2. loop.mod — sample with a real sustain loop (loop_len > 1).
    smp = {"name": "loopy", "length_words": 32, "finetune": 0, "volume": 48,
           "loop_start_words": 8, "loop_len_words": 16,
           "data": pcm(64, "ramp")}
    rows = [[cell(PERIODS["C-3"], 1, 0, 0)]] + [[] for _ in range(62)] + \
           [[cell(0, 0, 0x0C, 0)]]  # row 63: C00 explicit full volume, no-op check
    pat = pattern(rows)
    Path(FIXTURES / "loop.mod").write_bytes(
        build_mod("loop", [smp], [pat], [0]))

    # 3. effects.mod — arpeggio, slides, tone-portamento, vibrato, volslide.
    smp = {"name": "tone", "length_words": 20, "finetune": 0, "volume": 64,
           "loop_start_words": 0, "loop_len_words": 1, "data": pcm(40, "ramp")}
    rows = [
        [cell(PERIODS["C-3"], 1, 0x00, 0x37)],   # arpeggio +3 +7
        [cell(0, 0, 0x01, 0x08)],                # portamento up
        [cell(0, 0, 0x02, 0x08)],                # portamento down
        [cell(PERIODS["E-3"], 1, 0x03, 0x04)],   # tone portamento toward E-3
        [cell(PERIODS["C-3"], 1, 0x04, 0x64)],   # vibrato speed 6 depth 4
        [cell(0, 0, 0x0A, 0x0F)],                # volume slide up
        [cell(0, 0, 0x0A, 0xF0)],                # volume slide down
        [cell(0, 0, 0x0C, 0x20)],                # set volume 32
    ]
    pat = pattern(rows)
    Path(FIXTURES / "effects.mod").write_bytes(
        build_mod("effects", [smp], [pat], [0]))

    # 4. speed.mod — F effect crossing the speed/tempo boundary (0x20).
    smp = {"name": "beep", "length_words": 10, "finetune": 0, "volume": 64,
           "loop_start_words": 0, "loop_len_words": 1, "data": pcm(20, "square")}
    rows = [
        [cell(PERIODS["C-3"], 1, 0x0F, 0x03)],   # speed = 3 ticks/row
        [cell(0, 0, 0x0F, 0x78)],                # tempo = 120 BPM
    ]
    pat = pattern(rows)
    Path(FIXTURES / "speed.mod").write_bytes(
        build_mod("speed", [smp], [pat], [0]))

    # 5. jump.mod — pattern break (D) and position jump (B) across 3 patterns.
    smp = {"name": "click", "length_words": 8, "finetune": 0, "volume": 64,
           "loop_start_words": 0, "loop_len_words": 1, "data": pcm(16, "square")}
    pat0 = pattern([[cell(PERIODS["C-3"], 1, 0x0D, 0x10)]])   # break to row 16 of next order slot
    pat1 = pattern([[cell(PERIODS["D-3"], 1, 0x0B, 0x02)]])   # jump to order slot 2
    pat2 = pattern([[cell(PERIODS["E-3"], 1, 0, 0)]])
    Path(FIXTURES / "jump.mod").write_bytes(
        build_mod("jump", [smp], [pat0, pat1, pat2], [0, 1, 2]))

    # 6. multitrack.mod — all 4 channels active, exercises panning/mix.
    smp1 = {"name": "lead", "length_words": 12, "finetune": 0, "volume": 64,
            "loop_start_words": 0, "loop_len_words": 1, "data": pcm(24, "ramp")}
    smp2 = {"name": "bass", "length_words": 12, "finetune": 1, "volume": 50,
            "loop_start_words": 0, "loop_len_words": 1, "data": pcm(24, "square")}
    rows = [[
        cell(PERIODS["C-3"], 1, 0, 0),
        cell(PERIODS["E-3"], 2, 0, 0),
        cell(PERIODS["G-3"], 1, 0, 0),
        cell(PERIODS["A-3"], 2, 0, 0),
    ]]
    pat = pattern(rows)
    Path(FIXTURES / "multitrack.mod").write_bytes(
        build_mod("multitrack", [smp1, smp2], [pat], [0]))

    names = ["minimal", "loop", "effects", "speed", "jump", "multitrack"]
    print(f"wrote {len(names)} fixtures: {', '.join(names)}")


if __name__ == "__main__":
    sys.exit(main())
