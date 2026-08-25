# Fixture provenance

All `.mod` files in this directory are synthetic — hand-crafted for this
project by `scripts/gen_fixtures.py`, not downloaded from The Mod Archive
or any other source. No third-party sample or composition content is
included, so no attribution to an original author is owed or applicable.

Regenerate with:

```
python3 scripts/gen_fixtures.py
```

| file             | exercises                                             |
|------------------|--------------------------------------------------------|
| `minimal.mod`    | single sample, single pattern, no effects (baseline)   |
| `loop.mod`       | a real sustain loop (loop length > 1 word)              |
| `effects.mod`    | arpeggio, port up/down, tone portamento, vibrato, volslide, set-volume |
| `speed.mod`      | `F` effect on both sides of the speed/tempo boundary (0x20) |
| `jump.mod`       | pattern break (`D`) and position jump (`B`) across 3 patterns |
| `multitrack.mod` | all 4 channels active simultaneously                    |
