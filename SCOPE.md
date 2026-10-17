# Scope: a pure-Rust, wasm-compatible translation of the Generals engine

Status: scoping only, nothing is implemented. Drafted 2026-10-17.

## 1. Upstream

- **Source:** `github.com/electronicarts/CnC_Generals_Zero_Hour`, commit
  `0a05454d8574207440a5fb15241b98ad0b435590` (2025-02-27), the only commit.
- **Licence:** GPL-3.0 with **Section 7 additional terms** (see §6). A Rust
  translation is a derivative work: it must be GPL-3.0 and carry those terms.
- **Two trees:** `Generals/` (base game) and `GeneralsMD/` (Zero Hour). Zero
  Hour is the superset. **Translate `GeneralsMD/` only.** Base-game behaviour
  becomes data/INI differences later if wanted.
- **Not included upstream** (the README lists them as missing): DirectX 8/9 SDK,
  STLport, Miles Sound System, Bink video, GameSpy SDK, SafeDisk, the 3ds Max
  SDK, NVASM, zlib, LZH-Light. None are in the repo, so none can be translated.
  Each needs a replacement or removal (§4).
- **Game assets are not GPL.** `.big` archives, W3D models, textures, maps,
  audio and strings stay EA's. The crate ships no assets: the user supplies an
  owned install (Steam / EA App "Ultimate Collection").

## 2. Size of the job (measured, C/C++ lines incl. headers, `GeneralsMD/Code`)

| Area | Lines | In scope? | How |
|---|---:|---|---|
| `GameEngine/Source/GameLogic` | 186,556 | **Yes, the core** | Translate line by line |
| ├─ `Object/` (233 `.cpp`: behaviours, bodies, updates, weapons…) | 116,614 | Yes | Translate |
| ├─ `AI/` (pathfinding, AI players, states, squads) | 33,262 | Yes | Translate |
| ├─ `ScriptEngine/` (map scripts) | 24,001 | Yes | Translate |
| └─ `System/`, `Map/` | 12,679 | Yes | Translate |
| `GameEngine/Source/GameClient` (GUI 76,585) | 117,001 | Yes | Translate the logic, re-target the drawing |
| `GameEngine/Source/Common` (INI, RTS, Xfer, file system, audio front end) | 65,800 | Yes | Translate |
| `GameEngine/Source/GameNetwork` | 36,610 | Partly | Lockstep core yes; GameSpy/WOL/NAT no |
| `GameEngineDevice` (W3D 77,096, Miles, Win32, Bink) | 95,181 | **Rewrite** | Platform layer, see §4 |
| `Libraries/WWVegas/WWLib`, `WWMath`, `WWSaveLoad`, `WWDebug` | ~110,000 | Yes | Translate (keep the float maths identical, §3) |
| `Libraries/WWVegas/WW3D2` (renderer on DX8) | 132,427 | **Rewrite** around asset formats | Keep W3D loaders and scene logic, replace D3D |
| `Libraries/WWVegas/WWAudio`, `WPAudio` | ~30,000 | Rewrite | Pure-Rust audio |
| `Tools/` (WorldBuilder, W3D exporter, launchers, matchbot…) | 258,162 | **No** | Out of scope for the engine crate |

**In scope: roughly 600–700k lines of C++**, of which ~500k is a translation
and ~200k is replaced by a new platform layer. For calibration, this is a
multi-person-year port. Plan it in phases that are each runnable and testable
on their own (§5).

## 3. The hard part: deterministic lockstep

Generals multiplayer and replays are **lockstep**: each peer runs the same
`GameLogic` from the same commands, and divergence is detected by CRC
(`XferCRC.cpp`, 14 files in `GameEngine` use it). That shapes the whole port:

- **Logic is separable from the client.** `GameLogic` never reads `GameClient`
  state for decisions. Keep that boundary as a hard module boundary in Rust,
  so the logic runs headless (CI, servers, replay checks).
- **Floating point.** `GameLogic.cpp:198 setFPMode()` sets the x87 to
  **24-bit precision, round-to-nearest** on every logic entry. That is close to,
  but not identical to, IEEE `f32` (x87 keeps an extended exponent range, and
  ~~`fsin`/`fsqrt` differ from libm~~ `fsin`/`fcos` differ from any software
  `sinf`/`cosf`; **corrected 2026-10-17**: `fsqrt` is correctly rounded, so it
  matches `sqrtf`). Consequences:
  - Rust↔Rust lockstep (native ↔ wasm included) is achievable with plain `f32`,
    no fused multiply-add, and ~~**pure-Rust `libm`**~~ **petir** (the owner's
    no_std numerics crate, which delegates to pure-Rust `libm`; owner decision
    2026-10-17) for transcendentals. Never use `std` `sin`/`cos` in logic: they
    call the platform's libm. petir is `f64` only so far, so upstream's `f32`
    calls (`sinf`, `cosf`, `tanf`) stay on `libm`'s `f32` functions until petir
    gains them.
  - **Which trig the logic really uses (found 2026-10-17).** GameLogic never
    calls `WWMath::Sin`/`Cos` or their x87 assembly. Its ~120 `Sin`/`Cos` calls
    go to `Common/System/Trig.cpp`, which `#define`s `DEFAULT_TRIG`, so they are
    the 2003 Microsoft C runtime's `sinf`/`cosf` under the 24-bit FPU mode. It
    also calls `sqrt` ~80 times and `atan2` 28 times directly. Matching original
    replays therefore depends on that runtime's results, not on WWMath's
    assembly (which matters only to rendering). `Trig.cpp` also holds an
    unused integer lookup-table trig.
  - **NaN.** `XferCRC` hashes raw float bytes, and IEEE 754, Rust and wasm leave
    the bits of a NaN result unspecified, so a NaN in game state could give a
    native and a wasm peer different CRCs from identical behaviour. Proposed
    rule (not yet approved): logic never stores a NaN; the CRC path panics on
    one in tests and canonicalises it in release builds.
  - **Bit-exact compatibility with original replays and the original
    `generals.exe` is NOT assumed.** It is an experiment with a pass/fail gate
    (Phase 1). It is not a design promise.
- **RNG.** `GameLogicRandomValue` (`LogicRandomValue.h`) is a separate stream
  from the client RNG. Translate it exactly, with wrapping arithmetic.
- **Iteration order.** Any `HashMap` in logic becomes an ordered container or
  a deterministically seeded one, because iteration order changes results.
- **Keep WWMath's own vector/matrix code** rather than swapping in `glam`, so
  the operation order (and so the rounding) matches upstream.

## 4. wasm compatibility: what changes

Target `wasm32-unknown-unknown` (browser), plus native for testing.

| Upstream dependency | Browser constraint | Replacement (pure Rust) |
|---|---|---|
| Direct3D 8 fixed-function + shaders (117 files) | No D3D | `wgpu` (WebGPU, WebGL2 fallback); port the DX8 fixed-function states to WGSL |
| Win32 window/input (`Win32Device`) | No Win32 | `winit` (works on web) |
| Miles audio + Asimp3 MP3 | No Miles | `cpal` (Web Audio backend) + `symphonia` for MP3/WAV |
| Bink video | Proprietary codec | **Drop** (skip cutscenes), or let the user transcode them to an open format |
| GameSpy / WOL online | Service is dead | **Drop** |
| LAN over UDP (`udp.cpp`, `LANAPI`) | No UDP in browsers | Keep the lockstep layer behind a transport enum: WebRTC data channel (browser), UDP (native) |
| Threads (WWLib `thread.cpp`, WWAudio, WPAudio) | No `std::thread` on `wasm32-unknown-unknown` | Single-threaded frame loop; audio on the Web Audio callback |
| Local file system, `.big` archives | No file system | Asset-source enum: native directory, or browser file picker / IndexedDB cache |
| zlib, LZH-Light (missing upstream) | — | `miniz_oxide`; LZH-Light needs a clean reimplementation if any shipped asset uses it (check first) |
| `__asm` (30 files; in engine code only `PerfTimer`, `crc.h`, `StackDump`) | No inline asm | Rewrite in plain Rust |
| SafeDisk, STLport, Max SDK, NVASM | — | Remove |
| Memory: x86 32-bit pointers, struct packing in save/replay files | wasm32 is 32-bit too, but layout is not C++ | Read/write formats explicitly field by field, never by `transmute` |

Also needed: the browser cannot block, so asset loading is async and the main
loop is driven by `requestAnimationFrame` (via `winit`'s event loop).

## 5. Phases (each ends in something runnable and a gate that can fail)

0. **Formats, headless.** `.big` archive reader, INI parser (`Common/INI`), CSF
   strings, `DataChunk` map files, W3D mesh/hierarchy/animation, TGA/DDS,
   `Xfer` save/load. *Gate:* parse every INI, map and W3D in a user's own
   install with zero errors, and round-trip `Xfer` saves.
1. **GameLogic, headless.** ThingFactory/templates from INI, `Object/` modules,
   AI and pathfinding, ScriptEngine, players, RNG, `XferCRC`. *Gates:*
   (a) native and wasm runs of the same scripted skirmish give identical CRCs
   every frame; (b) **experiment:** play back an original `.rep` replay and
   report the first frame where the CRC diverges, if any (honest result,
   whichever way it goes).
2. **Renderer.** W3D scene, terrain, water, particles, shroud, decals on
   `wgpu`. *Gate:* screenshots of fixed camera views on shipped maps, compared
   by eye against the original game, with the differences listed.
3. **Client and UI.** `GameClient` (WND GUI, input, message stream, in-game
   UI, shell menus) → a playable single-player skirmish in the browser.
4. **Audio.** Event audio, music, speech.
5. **Multiplayer.** Lockstep over WebRTC/UDP, with a disconnect manager.
   Campaign and challenge modes ride on Phase 1's ScriptEngine.

## 6. Licence and naming

The EA Section 7 terms bind every fork: **no EA trademarks** in a distributed
modification, **no claimed affiliation**, and modified versions must be
**marked as modified** and not presented as the original. They are reproduced
verbatim in [`NOTICE.md`](NOTICE.md), which must accompany every copy.

**Decided 2026-10-17:** the repo was created as ~~`cnc-game-engines`~~ and
renamed to **`rts-lockstep-engine`**, because "cnc" is arguably a use of the
"Command & Conquer" trademark. The crate takes the same name. The README may
name the original game only factually, to say where the code came from and
which game data it reads.

Every translated file keeps EA's copyright header plus a "translated to Rust,
modified" notice.

## 7. Prior art to read before translating

- **TheSuperHackers/GeneralsGameCode**: the main community fork of the EA
  source. It fixes bugs and modernises the build. Read its fixes so the port
  doesn't carry known bugs (but note that fixing logic bugs changes replay
  compatibility).
- **OpenSAGE**: an independent C# reimplementation of the SAGE engine. Useful
  for format notes (W3D, `.big`, maps). Check its licence before reusing
  anything beyond facts about the formats.

## 8. Crate layout (one crate, `rts-lockstep-engine`)

```
src/
  lib.rs
  common/      // INI, Xfer, file system, RTS (players, teams, money), CRC, RNG
  ww/          // WWLib, WWMath, WWSaveLoad translations
  formats/     // big, w3d, csf, map DataChunk, tga/dds
  logic/       // GameLogic: object/, ai/, script/, map/, system/
  client/      // GameClient: gui/, input/, message_stream/, drawable/
  device/      // platform: render (wgpu), audio, input, assets, transport
  net/         // lockstep, commands, frame data
```

Note that ~600k translated lines in one crate means long compile times. Splitting
`logic` into its own crate later costs little if the module boundary is kept
clean from day one.
