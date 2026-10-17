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
| `ww::wwmath` | `Libraries/Source/WWVegas/WWMath/wwmath.h` (+ table init of `wwmath.cpp`) |
| `ww::vector2`, `ww::vector3`, `ww::vector4` | `WWMath/vector2.h`, `vector3.h`, `vector4.h` (the subset `Matrix3D` uses) |
| `ww::matrix3d` | `WWMath/matrix3d.h`, `matrix3d.cpp` (not `Get_Inverse`, the `Matrix3x3`/`Quaternion` setters, `Lerp`) |

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
  wasm builds need no C++ compiler). It needs an x86_64 host with g++ or clang++.
- `differential/shim/` holds minimal stand-ins for headers that cannot be
  compiled outside MSVC/Windows (each says what it replaces and why), and
  `differential/*.cpp` holds `extern "C"` wrappers. Where upstream is MSVC
  inline assembly with no C++ fallback (`BaseType.h`, the release path of
  `crc.h`), the reference is a GNU-asm transcription of the same
  instructions, labelled as such.
- The C++ is compiled with `-msse2 -mfpmath=sse -ffp-contract=off
  -fno-fast-math -fwrapv -fno-strict-aliasing`, so that its `float` arithmetic
  is the same IEEE single precision the Rust uses.
- Inputs: edge cases (zeros, signs, subnormals, `MIN`/`MAX`, inf, NaN,
  rounding boundaries, `lo > hi`) plus at least 100,000 seeded random inputs
  per function; for the RNGs, sequences of 1,000,000 draws per seed. Outputs
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
