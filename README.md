# rts-lockstep-engine

A Rust translation of the real-time-strategy engine source code that
Electronic Arts released under the GPL-3.0 in 2025 (the *Command & Conquer
Generals: Zero Hour* code, [electronicarts/CnC_Generals_Zero_Hour](https://github.com/electronicarts/CnC_Generals_Zero_Hour),
commit `0a05454d8574207440a5fb15241b98ad0b435590`). The goal is a pure-Rust,
wasm-compatible engine built around deterministic lockstep simulation. The
plan is in [`SCOPE.md`](SCOPE.md).

- **This is a modified version.** It is translated to Rust and changed in
  structure; it is not the original program.
- **Not affiliated with or endorsed by Electronic Arts.** No trademark rights
  are granted or claimed. See [`NOTICE.md`](NOTICE.md) for EA's additional
  terms under GPL section 7, which apply to this project.
- **No game assets are included.** Models, textures, maps, audio and text
  remain EA's. Later phases will read them from a copy of the game that you
  own.

## Status: early, nothing playable

Phase 0 (formats) is done; Phase 1 (game logic) has started, bottom up, in
the layer order of [`docs/phase1-porting-graph.md`](docs/phase1-porting-graph.md)
(measured by `scripts/porting_graph.py`). What exists:

| Rust module | Translated from (`GeneralsMD/Code/...`) |
|---|---|
| `common::base_type` (value types) | the methods of `Coord2D/3D`, `ICoord2D/3D`, `Region2D/3D`, `IRegion2D/3D`, `RGBColor`, `RealRange`, and `sqr`, `clamp`, `sign`, `rad2deg`, `deg2rad`, `min`, `max` of `Libraries/Include/Lib/BaseType.h` (Phase 1 layer 1) |
| `common::game_type`, `common::game_common` (rest), `common::errors`, `common::special_power_type`, `common::thing_sort`, `logic::powers`, `logic::weapon_types`, `logic::object_script_status_bits` | `Include/Common/GameType.h` (+ `GameType.cpp`), the rest of `GameCommon.h` (+ `normalizeAngle`, `TheRelationshipNames`), `Errors.h`, `SpecialPowerType.h`, `ThingSort.h`, `Include/GameLogic/Powers.h`, `WeaponSetType.h`, `WeaponStatus.h`, `WeaponBonusConditionFlags.h`, `ObjectScriptStatusBits.h` (Phase 1 layer 1) |
| `common::name_key_generator`, `common::well_known_keys` | `Include/Common/NameKeyGenerator.h`, `Source/Common/NameKeyGenerator.cpp` (the socket/chain structure, so keys come out in upstream's order, including its case-folding quirk), `StaticNameKey`, and the 127 keys of `Include/Common/WellKnownKeys.h` (Phase 1 layer 2) |
| `common::random_value` | `GameEngine/Source/Common/RandomValue.cpp` and the Logic/Client/Audio RandomValue headers |
| `common::crc` | `GameEngine/Source/Common/crc.cpp`, `GameEngine/Include/Common/crc.h` |
| `common::base_type` | the fast float helpers, `REAL_TO_*` macros and `PI` of `Libraries/Include/Lib/BaseType.h` |
| `common::trig` | `GameEngine/Source/Common/System/Trig.cpp`, `Libraries/Include/Lib/trig.h`: the shipped `sinf`-based `Sin`/`Cos`/`Tan`/`ACos`/`ASin`, and the unused integer-table branch as `trig::table` |
| `common::quick_trig` | `GameEngine/Include/Common/QuickTrig.h`, `GameEngine/Source/Common/System/QuickTrig.cpp` |
| `common::xfer`, `xfer_crc`, `xfer_save`, `xfer_load` | `GameEngine/Source/Common/System/Xfer.cpp`, `XferCRC.cpp` (with `XferDeepCRC`), `XferSave.cpp`, `XferLoad.cpp` and their headers, `Snapshot.h`: the snapshot/version mechanism, blocks, typed xfers, the lockstep CRC; save/load on in-memory buffers (not `xferMapName`, `xferScienceType`/`Vec`, `xferKindOf`, `xferUpgradeMask`, which need engine name tables) |
| `common::ini` | `GameEngine/Source/Common/INI/INI.cpp`, `Include/Common/INI.h`: line reading, `strtok` tokenising, block dispatch, the field-parse tables (`initFromINI`, `initFromINIMulti`, catch-all entries), `loadDirectory`, the INI `XferCRC`, and the generic field parsers (integers, reals, bools, strings, string lists, percentages, angles, durations, velocities, colours, coordinates, bit strings, index and lookup lists, veterancy/death/damage flags, random variables). Not the 62 block handlers of `theTypeTable` (all build game objects not yet ported; listed in `ini::TYPE_TABLE`), nor the field parsers that look names up in game systems (`parseAndTranslateLabel`, `parseMappedImage`, `parseAnim2DTemplate`, the two audio-event parsers, `parseThingTemplate`, `parseArmorTemplate`, `parseWeaponTemplate`, `parseFXList`, `parseParticleSystemTemplate`, `parseDamageFX`, `parseObjectCreationList`, `parseUpgradeTemplate`, `parseSpecialPowerTemplate`, `parseScience`, `parseScienceVector`) |
| `common::game_common`, `logic::damage` | the frame-rate conversions and veterancy flags of `Include/Common/GameCommon.h` (+ `GameCommon.cpp`), and the damage/death type flags of `Include/GameLogic/Damage.h` (+ `Damage.cpp`), which the INI flag parsers use |
| `formats::big` | `.big` archives: `GameEngineDevice/Source/Win32Device/Common/Win32BIGFileSystem.cpp` (the parser), `Win32BIGFile.cpp`, `GameEngine/Source/Common/System/ArchiveFile.cpp`, `ArchiveFileSystem.cpp`, on in-memory byte buffers |
| `formats::csf` | `.csf` string files: the CSF path of `GameEngine/Source/GameClient/GameText.cpp` (`getCSFInfo`, `parseCSF`, `stripSpaces`, the sorted label table, `fetch`, `getStringsWithLabelPrefix`), with the file read through `RAMFile::read`'s exact semantics; not the `.str` text format nor map string files |
| `formats::data_chunk` | the chunk format of maps: the input side of `GameEngine/Source/Common/System/DataChunk.cpp` (`CachedFileInputStream` reads, `DataChunkTableOfContents`, `DataChunkInput`: registered parsers, nested `parse`, the chunk stack, the typed reads incl. `readDict` and `readNameKey`), on uncompressed in-memory bytes; names stand in for `NameKeyType`s. Not the writer (`DataChunkOutput`), nor decompression (`CompressionManager`) |
| `ww::wwmath` | `Libraries/Source/WWVegas/WWMath/wwmath.h` (+ table init of `wwmath.cpp`) |
| `ww::vector2`, `ww::vector3`, `ww::vector4` | `WWMath/vector2.h`, `vector3.h`, `vector4.h` (the subset `Matrix3D` uses) |
| `ww::matrix3d` | `WWMath/matrix3d.h`, `matrix3d.cpp` (not `Get_Inverse`, the `Matrix3x3`/`Quaternion` setters, `Lerp`) |
| `ww::quat` | `WWMath/quat.h`, `quat.cpp`: the `Quaternion` data and `Build_Matrix3D` only |
| `ww::chunkio` | `WWLib/chunkio.h`, `chunkio.cpp`: `ChunkLoadClass` (chunks, micro-chunks, reads), with the file read through `GameFileClass` over `RAMFile` (the game's path for W3D files in `.big` archives); not the writer (`ChunkSaveClass`) |
| `ww::realcrc` | `WWLib/realcrc.h`, `realcrc.cpp`: `CRC_Memory` |
| `formats::w3d` | W3D models, parsed into plain structs (no rendering): `WW3D2/w3d_file.h` and `w3d_obsolete.h` (all chunk ids, flags and the 84 file structs); hierarchies (`htree.cpp`, `pivot.cpp`); animations (`hrawanim.cpp`, `hcanim.cpp`, `motchan.cpp`: raw, time-coded, adaptive-delta and bit channels); meshes (`MeshModelClass::Load_W3D` of `meshmdlio.cpp`, with the parts of `meshgeometry.cpp`, `meshmatdesc.cpp`, `vertmaterial.cpp`, `texture.cpp`, `aabtree.cpp`, `w3d_util.cpp`, `shader.h` and `dx8wrapper.h` it calls). Not the legacy material chunks (`MATERIALS3`, multi-material per-triangle materials), the processing after the chunks are read (`install_materials`, `post_process`, generated AABTrees), nor the asset manager's file loop (`Load_3D_Assets`) |
| `formats::texture` | textures into raw pixel buffers: Targa images as `WWLib/TARGA.CPP` reads them (`Open`, both `Load`s, run-length decoding, `InvertImage`, `XFlip`, `YFlip` with its byte-tail bug, palettes, Targa 2.0 footer and extension); DDS files as `WW3D2/ddsfile.cpp` reads them (header, level table, payload, `Get_Pixel`, `Get_4x4_Block`, the level and cube-map copies to every surface format, with upstream's DXT1 and DXT5 decompression; upstream has no DXT2-4 decoder); the formats of `ww3dformat.cpp`/`formconv.cpp`, `Write_B8G8R8A8` of `bitmaphandler.h` and `Recolor` of `colorspace.h`. Not Targa writing (`Save`, `EncodeImage`), nor the Direct3D surface overloads |

**Approved by the owner (2026-10-17):** `XferCRC`/`XferDeepCRC` carry a NaN guard
(`common::xfer_crc::nan_guard`): a NaN `Real` xfered into a CRC panics in debug
builds and tests and is hashed as one canonical NaN in release builds, because
NaN bits are not portable between native and wasm peers.
`XferCrc::new_without_nan_guard` keeps upstream's raw-bit behaviour, for the
differential tests. The rule behind it: game logic must never store a NaN.

There is no renderer, audio or networking yet, and of the game logic only
the bottom layers listed above.

## Floating point and determinism

All logic arithmetic is plain IEEE-754 `f32`, one rounding per operation, no
fused multiply-add, in upstream's operation order. Every elementary function
goes through the owner's `petir` crate (`petir::Real`, pure Rust, forwarding to
the [`libm`](https://crates.io/crates/libm) crate), never `std`, so native and
wasm builds compute the same bits. The precision is upstream's: where upstream
calls a C double function on a `float` (`(float)acos(val)`), the Rust evaluates
petir's `f64` function and rounds once; where it calls `sinf`, petir's `f32`
one. petir comes from crates.io, pinned to exactly `0.0.1`. This makes Rust-to-Rust lockstep
possible. It does **not** make this crate bit-compatible with the original
Windows build, which ran on the x87 FPU at 24-bit precision and, in several
`wwmath.h` functions, used x87 instructions (`fsin`, `fcos`, `fistp`, a
Newton-iteration `Inv_Sqrt`) where this translation follows upstream's
portable C++ branch. See `src/ww/wwmath.rs` and `SCOPE.md` section 3.

## Verification: code-to-code differential tests

Every translated function is checked against the **original C++**, compiled
from a pinned copy of EA's repository:

```sh
git submodule update --init          # upstream/ at commit 0a05454d...
cargo test --release --features differential
```

- `upstream/` is EA's repository as a git submodule. Its files are never
  edited. `build.rs` compiles the needed `.cpp` files with the `cc` crate,
  only when the `differential` feature is on (off by default, so normal and
  wasm builds need no C++ compiler). It needs an x86_64 host with g++ and,
  for the W3D files, clang++ (their headers use MSVC template code that
  g++ rejects; clang accepts it with `-fms-extensions
  -fdelayed-template-parsing`).
- One exception to "unchanged": `INI.cpp` is written in MSVC 6/7's dialect
  in two places that g++ rejects and no shim can supply (a loop variable
  used after its `for`, and a `void*` bound to `const void*&`). `build.rs`
  compiles a copy with exactly those two mechanical edits, each asserted to
  match once, and documents them. The W3D build does the same for the same
  MSVC loop-scope rule in `motchan.cpp`, `htree.cpp`, `meshmdlio.cpp` and
  `meshmatdesc.cpp`, a missing `template<>` in `multilist.cpp`, and adds
  `this->` to the dependent-base calls of `Vector.H`; `meshmatdesc.cpp`'s
  copy also renames `Is_Empty` so the test can snapshot the mesh at the
  point where its loading ends. W3D functions that live in files which
  cannot compile without Direct3D or the collision system
  (`meshgeometry.cpp`, `vertmaterial.cpp`, `texture.cpp`, `aabtree.cpp`, the
  x87 `DX8Wrapper::Convert_Color`) are transcribed into
  `differential/wrap_w3d_mesh.cpp`, labelled as such. The texture build
  compiles `ddsfile.cpp` and `formconv.cpp` with the same kind of edits
  (loop scope, MSVC's `unsigned short(x)` cast), reads `ddsfile.h` with
  its `void*` member spelled as 4 bytes, and compiles `TARGA.CPP` with
  `long` as `int` (both 32 bits on Win32) and its `_swapBytes` MSVC
  assembly as a GNU-asm transcription of the same instructions; the
  Direct3D 8 and DirectDraw constants come from documented SDK values
  (`differential/shim_w3d/d3d8.h`, `ddraw.h`). `Lib/BaseType.h` cannot be
  compiled by g++ at all (MSVC `__int64`, `__asm`); for its value types the
  build copies two segments of its text verbatim (the templates, and the
  derived types with their methods), each located by its first and last
  line and asserted to occur once.
- `differential/shim/` holds minimal stand-ins for headers that cannot be
  compiled outside MSVC/Windows (each says what it replaces and why;
  `differential/shim_ini/` adds, for the INI build only, declaration-only
  stand-ins for the game systems `INI.cpp` includes but the tests never
  reach), and `differential/*.cpp` holds `extern "C"` wrappers. Where upstream is MSVC
  inline assembly with no C++ fallback (`BaseType.h`, the release path of
  `crc.h`), the reference is a GNU-asm transcription of the same
  instructions, labelled as such.
- The C++ is compiled with `-msse2 -mfpmath=sse -ffp-contract=off
  -fno-fast-math -fwrapv -fno-strict-aliasing`, so that its `float` arithmetic
  is the same IEEE single precision the Rust uses.
- Inputs: edge cases (zeros, signs, subnormals, `MIN`/`MAX`, inf, NaN,
  rounding boundaries, `lo > hi`) plus at least 100,000 seeded random inputs
  per function; for the RNGs, sequences of 1,000,000 draws per seed; for
  the file-format readers, 1,500 to 30,000 generated files per reader
  (each test prints its counts, including the files left out of the C++
  comparison because upstream is undefined on them). Outputs
  are compared as raw bits. The one exception is a NaN on both sides with a
  different sign or payload, which is counted and reported but not failed,
  because Rust leaves NaN bits unspecified (see `tests/support/mod.rs`).
- **Planted bugs** (from Phase 1 on): each comparison is also run against a
  deliberately wrong variant of the Rust (a plausible slip: `<=` for `<`, a
  missing clamp, 256 for 255, a swapped component), which must be caught
  (`Check::finish_planted`, printed as `PLANTED ... caught n`). That shows
  the inputs can tell the translation from a near miss.
- The C++ calls to the elementary functions (`sinf`, `cosf`, `acosf`, ... and
  the double `sin`, `acos`, `asin`, `atan`, `atan2`, `sqrt`) are bound to the
  same petir functions in the test binaries, so the gated tests check the
  translation. A shim `math.h` keeps g++ from resolving `acos(float)` to a
  `float` overload, so the C++ computes in double as upstream wrote it. How
  far petir is from glibc is measured separately
  (`tests/differential_libm_vs_glibc.rs`): the `f32` functions differ by up to
  1 ULP on 0.9-5.8% of inputs depending on the function (plus NaN-sign
  differences outside the domain of `acosf`/`asinf`); the `f64` functions
  rounded to `f32` matched glibc on all ~100k inputs per function.

**What this verifies:** that the Rust computes what the upstream C++ source
computes when compiled for SSE. **What it does not verify:** agreement with
the original x87 Win32 executable or its replays; that is a separate
experiment planned in `SCOPE.md`.

## Licence

GPL-3.0-only, with the additional terms in [`NOTICE.md`](NOTICE.md), which
must accompany every copy. See [`LICENSE`](LICENSE).
