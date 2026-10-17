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

Only a first batch of self-contained foundations exists:

| Rust module | Translated from (`GeneralsMD/Code/...`) |
|---|---|
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

**Proposal, not yet approved:** `XferCRC`/`XferDeepCRC` carry a NaN guard
(`common::xfer_crc::nan_guard`): a NaN `Real` xfered into a CRC panics in debug
builds and tests and is hashed as one canonical NaN in release builds, because
NaN bits are not portable between native and wasm peers.
`XferCrc::new_without_nan_guard` keeps upstream's raw-bit behaviour. The
change is a single commit so it can be reverted on its own.

There is no renderer, audio, networking or game logic yet.

## Floating point and determinism

All logic arithmetic is plain IEEE-754 `f32`, one rounding per operation, no
fused multiply-add, in upstream's operation order. Every elementary function
goes through the owner's `petir` crate (`petir::Real`, pure Rust, forwarding to
the [`libm`](https://crates.io/crates/libm) crate), never `std`, so native and
wasm builds compute the same bits. The precision is upstream's: where upstream
calls a C double function on a `float` (`(float)acos(val)`), the Rust evaluates
petir's `f64` function and rounds once; where it calls `sinf`, petir's `f32`
one. petir is a git dependency pinned to outram-park-backend commit
`21a93d94` (`develop`; `main` has no `Real` trait). This makes Rust-to-Rust lockstep
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
  `differential/wrap_w3d_mesh.cpp`, labelled as such.
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
