# CLO3D container formats — observed layout notes

**Status: partial, observed from shipped sample files only.** This is *not* a
complete format specification and must not be treated as one. It records what
can be established from bytes actually present in the shipped application, with
explicit boundaries on what remains unknown.

## Samples examined

| File | Version field | Size |
|---|---|---|
| `Preset/Bond/Fusible (Nonwoven).zfab` | 7 | 9,996 B |
| `Preset/Bond_Defual/Bond.zfab` | 9 | 13,659 B |
| `Preset/Zipper/Tape/Trim_Hardware.zfab` | — | 7,257 B |
| `Preset/Fold3D/folding fabric.zfab` | — | 13,367 B |
| `Preset/Avatar/Converting/Female/Genesis8_Female.avt` | 11 | 1,225,020 B |
| `Preset/Avatar/Converting/Male/Genesis9_Male.avt` | — | 2,348,096 B |

## Common header (verified identical shape across `.zfab` and `.avt`)

```
offset  bytes            observed meaning
0x00    4                ASCII magic with a leading space:
                           .zfab -> " FAB"
                           .avt  -> " AVT"
0x04    4                ASCII spaces (padding to 8)
0x08    8                "    CLO "  (product tag, spaces around "CLO")
0x10    u32 LE           format version  (7, 9 for .zfab; 11 for .avt)
0x14    u32 LE           region A start offset
0x18    u32 LE           region A end offset
0x1c    u32 LE           region B start offset
0x20    u32 LE           region B end offset
0x24    ...              further fields, see below
```

For every sample, **region B end ≤ file size** and the two regions do not
overlap. Region A lengths measured: 24 B, 24 B, 24 B for the three digitised
samples — but inspection of the bytes shows region A is **not** a fixed record:
it contains a `NULL` string inside it (`4e 55 4c 4c`) in the `.avt` sample, so
it is variable-length key/value data whose bounds the header simply delimits.
The apparent 24-byte consistency is coincidental for these samples.

### Bytes at 0x24–0x3c (unknown, but structurally repeated)

Two 12-byte groups with an identical internal shape appear in all three
digitised samples:

```
0x24  u32   a         (varies; 2739313865 / 449232188 / 3040665256)
0x28  u32   376 or 368
0x2c  u32   2008      (constant across all samples)
     --- repeated at 0x30 with identical values ---
0x30  u32   a         (same value as 0x24)
0x34  u32   376 or 368
0x38  u32   2008
0x3c  u32   count     (6 in one .zfab, 9 in the other)
```

`2008` and `376`/`368` are constants across files of different versions and
types. The first value is high-entropy and differs per file, consistent with a
checksum, hash, or 64-bit timestamp split across a boundary. **Not resolved.**

## Region B — compressed payload

Region B begins with 8 bytes that are not a zlib header, then compressed data.
For the `.avt` sample, standard zlib streams (`78 9c`, `78 01`, `78 da`) begin
at offsets 294, 3273, 41,603, … — i.e. the container embeds **multiple**
compressed members, not one. The application links `libzstd`, `liblzma` and
`libbrotli*`, so the codec for a given member cannot be assumed to be zlib from
a stream signature alone.

### PNG payload in `.avt`, and an `IDATx` chunk tag

The `.avt` sample contains a **valid PNG image** beginning at offset 244:
`IHDR` at 244, `pHYs` at 269. Immediately before it, at offset 289, is
`UIDATx` — i.e. PNG's `IDAT` preceded by a one-byte tag `x`/`U`. The `.zfab`
samples likewise contain the tag `@zmffh`. These 4–8 byte ASCII tags appear to
name typed sub-chunks. **The tag vocabulary is not enumerated.**

## What is established vs. unknown

**Established (observed):**
- ASCII magic + product tag, a version word, and two bounded, non-overlapping
  regions per file.
- The formats are container/chunk based with embedded compressed members and,
  in `.avt`, an embedded PNG texture.
- Serialization on the code side is `Marvelous::MVVariant` — a tagged variant
  tree (`bool/char/int/float/double/int64/string/bytearray/vec2/vec3/vec4/mat3/
  mat4`, plus `Array`/`List`/`Map`) with `readFromFile(FILE*, unsigned)` taking
  a version word. See `ARCHITECTURE.md` §3.1.

**Unknown (not guessed):**
- The key vocabulary and value semantics inside region A and the variant tree.
- Which codec applies to which compressed member.
- The meaning of the high-entropy field at `0x24`/`0x30`.
- Whether region A is length-prefixed internally or purely bounded by the
  header.

## Consequence for ClothCraft

ClothCraft does **not** parse these formats. Doing so correctly requires the
unknown key vocabulary, and guessing it would produce a reader that silently
misinterprets real documents. This file exists to record the recovered
structure for future work and to keep the boundary between *observed structure*
and *assumed schema* explicit.
