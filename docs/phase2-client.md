# Phase 2: the client (rendering, input, UI, audio)

Owner direction, 2026-11-03: **web first, native too; Bevy; Web Audio; wgpu; `outram-blender` (outram-park-backend) where it helps.**

## The rule the client keeps

The client never changes the simulation. The game logic (`rts-lockstep-engine`, phase 1) stays the only thing that decides
what happens. It runs on fixed 30 Hz frames, and its CRCs stay identical between the native and wasm builds. The client:

- **reads** the `World` after each logic frame: object positions, orientations, geometry, models, health, statuses, and the
  terrain;
- **consumes** the client events the logic already emits (`ClientEvent`, `ObjectClientEvent`): effects, sounds, UI notices;
- **sends** player orders only as the game's own command messages (`GameMessage`), which the logic applies on a frame
  boundary, as upstream's lockstep does.

Bevy's own `f32` maths, frame timing and ECS order therefore cannot reach a CRC. Drawing interpolates between the last two
logic frames.

## Shape of the code

- A new package, `client/` (`rts-client`), in a Cargo workspace with the logic crate as the root package. The logic crate's
  dependencies do not change (still `petir` only), and its gates (`cargo test` at the root, `scripts/gate_a.sh`) do not build
  Bevy.
- **The client's translations that need no renderer** (the selection, the command translators) go in the logic crate's
  `src/client/`, with no new dependency, behind host traits: tested without Bevy, and the same on the web and natively.
- **Bevy, a pinned exact version,** with default features off and only what we draw with: the renderer, the core pipeline,
  PBR, assets, windowing and input.
  - **Web build:** `wasm32-unknown-unknown` with `wasm-bindgen`, on WebGL2 first, because every browser has it. A WebGPU
    build follows once the first works; Bevy builds one backend per wasm target.
  - **Native build:** wgpu's Vulkan, Metal or DX12 through Bevy.
- **Audio** sits behind our own small trait. On the web it is the **Web Audio API** through `web-sys`. Natively it is a
  Rust backend, chosen when audio starts (milestone C6).
- **`outram-blender`** is used only where it saves work: mesh operations on loaded models (normals, decimation for level of
  detail, booleans for the terrain cut-outs), or generated placeholder geometry. It pulls `faer`, so it comes in only when a
  milestone needs it, with its default `gpu` feature off.

## The game's files

The art, sounds, maps and UI layouts are EA's and are **not** shipped or committed. The client reads them from a copy of
*Generals: Zero Hour* the player owns:

- `.big` archives;
- W3D models and animations;
- DDS/TGA textures;
- WND layouts;
- CSF strings;
- the client's INI files.

~~"`.big` archives (`formats::big`, already ported); W3D models and animations (`ww::chunkio` is already ported)"~~, which implied only those two were done. **Corrected 2026-11-04:** phase 0 already
translated much more (`src/formats/`, checked by reading each module's header):
- `formats::big`: `ArchiveFile`, `ArchiveFileSystem` and `Win32BIGFileSystem`'s parser, over in-memory buffers;
- `formats::csf`: the CSF loader of `GameText.cpp`;
- `formats::w3d`: the `Load_W3D` of meshes (`meshmdlio.cpp`, `meshgeometry.cpp`), hierarchies (`htree.cpp`), raw and
  compressed animations (`hrawanim.cpp`, `hcanim.cpp`) and motion channels (`motchan.cpp`), into plain structs;
- `formats::texture`: DDS with upstream's DXT1/DXT5 decompression (`ddsfile.cpp`) and Targa.

Not yet translated:
- ~~HLODs and aggregates, and the asset manager's file loop;~~ **Corrected 2026-11-05 (#10, step 3):** translated
  in #10 steps 1 and 2 (`formats::w3d::hlod`, `collect`, `agg_def`, the box/sphere/ring prototypes, `assetmgr`);
- ~~animation sampling and morph animations;~~ **Corrected 2026-11-05 (#10, step 3):** translated: morph animations
  load (`formats::w3d::hmorphanim`, step 2), and channels, raw, compressed and morph animations are sampled and
  hierarchies posed (`motchan`, `hanim`, `hmorphanim`, `htree`: `Base_Update`, `Anim_Update`, `Blend_Update`,
  `Simple_Evaluate_Pivot`, bone capture), compared bit for bit with upstream's compiled code
  (`tests/differential_w3d.rs`, section 13). Still missing: `HTreeClass::Combo_Update` (`HAnimComboClass`);
- ~~the local and composite file systems, and random-access archive reads (archives are read whole into memory today);~~
  **Corrected 2026-11-05 (#24):** translated in batch F1 (#9): `TheFileSystem` over a local device, archives mounted by
  their directory and members read by range; in the browser the art archives are read by range from the player's
  files (`RangedDevice`, #24);
- ~~the terrain's tile data and textures;~~ **Corrected 2026-11-05 (#11):** translated (`client::terrain_height_map`:
  the blend tile data, the tiles, the atlas, the UVs and `updateVB`'s vertices); still missing: the vertex lighting,
  the custom blend edges, water and shoreline (see #11's history entry below);
- WND layouts.

Until the owner supplies a copy for testing, the milestones that need no art come first, and the loaders are tested on
files generated by the tests.

## Milestones

Each one is runnable on the web and natively, and each one is tested.

| | Milestone | Needs the game's files | Done when |
|---|---|---|---|
| C1 | **Viewer, no art.** The running `World` drawn from its own data: the height-map terrain; each object as its geometry (box, cylinder or sphere at its major and minor radii and height), coloured by player; health bars. An RTS camera (pan, zoom, rotate), with on-screen buttons as well as mouse and touch. | no | The gate game plays in a browser tab and a native window, at 30 logic frames a second, with the units visibly fighting. A headless test checks the scene against the `World` frame by frame. |
| C2 | **Selection and orders.** Clicking and box-selecting units; right-click moves and attacks, as `GameMessage`s into the logic. | no | You can play the gate map: move, attack, build with the dozer. The logic's CRCs with and without the client attached stay identical for the same command stream. |
| C3 | **The game's art.** `.big`, W3D meshes, hierarchies, animations, DDS/TGA textures, and the terrain's tile textures and blending. | yes | A Zero Hour map and its units drawn with their real models, animated by their model condition states. |
| C4 | **Effects.** FX lists, particle systems, decals, tracers, lasers and explosions, from the client events. | yes | Weapons and deaths look right side by side with the original. |
| C5 | **UI.** The window manager driven by the WND layouts, the control bar (command sets the logic already loads), the in-game UI, radar, EVA, and the shell menus. | yes | A skirmish can be started and played from the menus. |
| C6 | **Audio.** Audio events from the logic and the UI; positional sound, music, voice lines. Web Audio on the web. | yes | Sound plays on both. |
| C7 | **Polish.** Water, shadows, shroud and fog, weather, the terrain's visual details. | yes | Screenshots compared with the original. |

C1 and C2 make the game playable without art. C3 is the step that most needs the owner's game files.

## Verification

- **The logic stays as it was gated.** The client is attached in a test that replays a command stream and checks the CRCs
  match a run without the client.
- **What is drawn is checked against the `World`,** headless: object transforms, colours and counts per frame.
- **The loaders are tested** on generated files, and on the owner's files when they are supplied (those tests are skipped
  without them, never faked).
- **Looks are compared** with screenshots of the original (needs Windows and the game; the owner's call).

## Development history

**#11 (2026-11-05, GitHub #11): the terrain drawn as the game draws it.**
- **Translated** (engine crate, `client::terrain_height_map`, from `WorldHeightMap.cpp`, `TerrainTex.cpp`,
  `HeightMap.cpp`): the rest of `ParseBlendTileData` (edge classes, blend tiles with their `FLAG_VAL`, cliff infos; the
  logic's parse now keeps the indices and the chunk's remaining bytes, consuming the chunk as before) and the
  constructor's "patch bad maps"; `readTexClass`, `countTiles`, `readTiles`; `updateTileTexturePositions`,
  `getTerrainTexture` and `TerrainTextureClass::update` (the A1R5G5B5 atlas with its wrap-around borders);
  `getUVForNdx`, `getUVData`, `getUVForTileIndex` (cliff infos and `DO_OLD_UV`'s stretch), `getAlphaUVData`,
  `getExtraAlphaUVData`; `HeightMapRenderObjClass::updateVB` (positions, normals, both UV sets, alpha, the
  `FLIP_TRIANGLES` diagonal) over the whole map, `getXWithOrigin`/`getYWithOrigin`, and `renderExtraBlendTiles`'
  cells. Quirks kept are listed in the module's docs.
- **New** (client, `client/src/terrain.rs`): the look (textured, shaded without textures, flat without a height map)
  and three Bevy meshes in place of upstream's passes (base, blend with vertex alpha, third layer), with depth biases;
  the atlas with 3 box-filtered mip levels. Picking meets the logic's ground height. On the web, `TerrainZH.big` is no
  longer skipped and is read by range (#24); the terrain is built again once its Targas arrive.
- *Results:* 7 engine unit tests and 4 client tests (predictions in their docs), all as predicted, except one client
  test whose first run failed: an unserved read of a loose ranged file opens as empty, not `None`, so "waiting" is now
  told by the device's unserved-read count. Differential (`tests/differential_terrain_mesh.rs`, upstream's code cut
  verbatim by `build.rs`): first run 235,104 mismatches, all from the wrapper lacking `FLIP_TRIANGLES`; then 3,000
  synthetic maps, 1,701,051 cells, 6,804,204 vertices and 3,138,387,968 atlas words, **0 mismatches**; four planted
  bugs caught, two equivalent ones not. Gate (a) passes (last CRC `0bf44067`); the gate game's CRCs are unchanged with
  the terrain built every frame. Seen textured natively (Xvfb) and in headless Chromium on a synthetic install.

**#24 (2026-11-05, GitHub #24): the browser reads the art archives by range, and draws the game's models.**
- **The device** (engine crate, `common::file_system::RangedDevice`, new: upstream reads through Windows' synchronous
  `ReadFile`): files known by length, with the ranges that have arrived. A read of bytes not held returns nothing (the
  open fails, as `FileSystem::open_file` checks the member's length) and lists the range as wanted, once;
  `LocalDevice::ready` and `unserved_reads` (defaults: always ready, 0) let a caller tell "not yet" from "not there".
  `ArchiveFile::directory_prefix_needed` runs the translated directory read on the bytes at hand and says how many it
  would ask for, so the page fetches exactly the prefix `FileSystem::init` will read (64 KiB, then four times as much).
  The translation's structure (`FileSystem`, `ArchiveFileSystem`, `Win32BIGFileSystem::init`) is unchanged.
- **Why asynchronous fetches and not `FileSystemSyncAccessHandle`:** a synchronous read needs a worker, and Bevy runs
  the game on the page's main thread, which may not block (`Atomics.wait` is refused there, and SharedArrayBuffer needs
  cross-origin isolation headers GitHub Pages does not send). Moving Bevy into a worker with an OffscreenCanvas is a
  larger change. So the page reads each wanted range with `File.slice(...).arrayBuffer()` (the picked File, or the
  OPFS file on a later visit), off the frame, and the game draws the object's shape until its model arrives.
- **The page** (`web/index.html`): the art archives (`W3D*`, `Textures*`) and loose `Art/` models and textures are kept
  in OPFS and handed over by length and directory (`import_ranged`); the others whole, as before (`import_file`, now with
  the page's key for the file so the art can read their members back too). Every 30 ms it reads the wanted ranges
  (`import_wanted`) and hands them back (`import_range`). `Terrain*`, `Shaders*`, `Window*` and the sound archives stay
  skipped.
- **The drawables** (`client/src/drawables.rs`): the draw modules ask the asset manager through `Deferred`: a model
  whose file is being fetched gives no render object for now, and an animation is not looked for while its file (or a
  model's) is being fetched, so upstream's permanent missing-animation table is not filled by a fetch in flight. When
  the bytes arrive, each drawable lacking its model or animation is made again with its flags. A render model or
  textures whose read waited are made again on a later frame. The HUD shows the models drawn and the files being read;
  `art_status()` reports them to the page.

*Predictions before the runs* are in each test's doc. *Results:*
- `the_ranged_device_reads_what_is_used_and_matches_the_memory_device` (engine): all held on the first run: a
  3,000-entry directory took two reads (262,144 bytes); members read through the ranged path equal the memory device's
  byte for byte; the 1 MiB member never opened was never read (held 569,349 of the archive's 1,532,841 bytes and the loose file's 5).
- `the_demo_reads_its_models_by_range_and_draws_shapes_until_they_arrive` (client): all three objects posed from frame
  1 (frame 0: shapes), the runner's moving model one frame later than with the files at hand (3 against 2), four ranges
  read (6,852 bytes of a 4.2 MB archive), the logic's CRCs equal to the run with the files at hand. **One miss:** I
  predicted 3 drawables made again; there were 6: the later fetches (the runner's moving model, both soldiers' firing
  model) remake too, as designed.
- `write_and_play_the_test_install_with_art_when_asked` (client, run with the test install): the Tank and the Barracks
  given `Draw = W3DModelDraw` with the generated robot in a 33.6 MB `W3DZH.big` (a 32 MiB member nobody opens);
  natively through the page's import path, 4 models posed from frame 1, 4,068 bytes of `W3DZH.big` read (0.012 %).
- **Browser check** (headless Chromium, `build_web.sh`'s page, the same install picked with *…or its .big files*): the
  page played, `art_status()` gave `models=4 names=robot missing=0 held=78071 total=33567038 arrived=4068 ranges=1`,
  the page read one range of 4,068 bytes, and the robots were drawn textured
  ([screenshot](images/c24_web_art_ranged.png)). The wasm memory after the game started was 85,065,728 bytes with the
  art against 84,803,584 for the same install without it: 256 KiB more, where reading `W3DZH.big` whole would add at
  least its 33.6 MB. On a second visit, *Play with saved files* read the same range from the OPFS copy and drew 4
  models. The parent-folder pick (C3d's check) still plays.
- Full run: the engine's lib tests 918 pass (917 before); the client's 14 (11 before; one of the three new runs only
  when asked); clippy clean on the lib (the client's one warning, a complex type in `app.rs`, predates this); the web
  build builds; gate (a) identical over 300 frames native and wasm (last CRC `0bf44067`).

**Not done yet:** the model viewer over the imported files (`?model=NAME&anim=H.A`); a real Zero Hour install (none
here): its first load will show how many ranges a skirmish start reads and how long the shapes show; the terrain and
shader archives.

**#10 step 5 (2026-11-05, GitHub #25): drawables. Each object's model and animation chosen by its model condition
flags, and drawn.**
- **`W3DModelDraw`, translated** (engine crate, `client::w3d_model_draw`): the module data's INI parse
  (`buildFieldParse` and `parseConditionState`: default, normal, alias and transition states, the ignore mask, every
  per-state field; the tank, truck, tank-truck, supply, dependency and science subclasses' fields, parsed only), read
  from the `Draw` blocks the logic already keeps as tokens (written back as INI text and read by the translated INI
  reader); `findBestInfo` through the existing `SparseMatchFinder`; `setModelState` (pending states, transitions,
  `WaitForStateToFinishIfPossible`), `adjustAnimation` (random choice with the client's own generator, idle animations,
  start frames, `MAINTAIN_FRAME_ACROSS_STATES`, speed factors), `doDrawModule`'s animation half, the loop and
  completion durations, pausing, `DistanceCovered` speed; and `Animatable3DObjClass`'s single-animation playback over
  the existing `Compute_Current_Frame`.
- **Drawables** (`client::drawable`): `Drawable`'s flags with `DIRTY_CONDITION_FLAGS`, body damage states,
  `getShouldAnimate`, the draw modules' LOD filter; fed by replaying the logic's client events after each frame.
- **The client** (`client/src/drawables.rs`, `app.rs`): the drawables run every rendered frame, the chosen models are
  loaded through the W3D asset manager and posed (`Anim_Update`); an object with no model, or no art, is still its shape.
  The art is the player's folder with `--zh` natively; the demo and the web page have none yet (#24).
- **A defect found in the logic's loader, fixed:** `AliasConditionState` was read as a nested block; upstream's
  `PARSE_ALIAS` reads one line. A real `Draw` block with an alias would have swallowed the lines after it (see
  `CLIENT_NESTED_BLOCKS`; the differential stand-in in `wrap_thingtemplate.cpp` had the same mistake and is corrected).

*Predictions before the runs* are in each test's doc. *Results:* two misses, both in test data (the bit is
`REALLYDAMAGED`, not `REALLY_DAMAGED`; a looping frame came out 2.000001 in `f32`); bisecting the first found the alias
defect. In the demo with generated W3D models, the runner's model changed at logic frame 2 when it moved and back at frame
133 when it stopped, and both soldiers showed their firing model. The gate game's CRCs with the drawables fed every frame
equal the plain run's (120 frames). Full run: the engine's lib tests 917 pass (903 before), the client's 11 (9 before);
differential `differential_w3d` 25 pass, `differential_thing_template` 3 pass (7,766 INI loads, 0 mismatches), and the
existing `SparseMatchFinder<ModelConditionFlags>` check 60,000 queries, 0 mismatches; clippy clean (lib); the web build
builds; gate (a) identical over 300 frames native and wasm (last CRC `0bf44067`).
**Not done yet** (#25's remaining work): turret and recoil bones (rung 4), the bone caches, sub-object hide and show,
muzzle flashes, particle systems, shadows, tracks, the subclasses' runtime (treads, tires), the other draw modules
(`W3DDefaultDraw`, trees, lasers, ...), and a differential of `parseConditionState` against upstream's compiled code.

**Batch #10 step 4 (2026-11-05, GitHub #10): W3D models drawn in the Bevy client, with meshes, materials, textures and
skinning.**
- **The render side without a device, translated** (engine crate, `formats::w3d`): `shader` (`ShaderClass::Apply` as
  plain device states, `Get_SS_Category`, `Guess_Sort_Level`), `vertmaterial` (`VertexMaterialClass` from the file:
  colours, opacity, flags, the mapper kind of each stage), `meshmdl` (`install_materials`' `Post_Load_Process` and
  `post_process` with the game's settings; the DX8 renderer's polygon groups per pass; `get_deformed_vertices`), `render`
  (an HLOD's top LOD and aggregates bound to their bones and posed, `Compute_Current_Frame`, the texture lookup: the
  lower-cased name, the DDS first, then the Targa, through the game's file search, decoded to RGBA8). `synth` writes W3D
  files for the tests and placeholder art (new code).
- **The Bevy mapping** (`client/src/model.rs`, new): one mesh and one `StandardMaterial` per polygon group, blend pairs
  mapped to alpha modes, alpha test as a mask at upstream's `0x60`, culling, lighting, colours, the stage-0 texture;
  sub-objects placed by their bones' transforms; skins rewritten every frame from the CPU deformation (as upstream
  deforms on the CPU). What Bevy's material cannot reproduce is listed per group (detail stage, specular, fog, mappers,
  screen blending, depth functions).
- **A model viewer**: `rts-client --model NAME [--anim HIERARCHY.ANIM] [--zh <folder>]` natively; without `--zh` it shows
  a generated robot (a textured rigid body, a skinned arm, a hidden helper, a looping swing)
  ([screenshot](images/c3_model_viewer_demo.png), software Vulkan under Xvfb). The web page does not import the art
  archives yet (its `SKIP` list), so the viewer has no real models in the browser.

*Predictions before the runs* are in each test's doc. *Results:* the lib's 9 new unit tests and the client's 2 held
except two misses, both in test data: a 2x2 Targa's rows were not flipped (upstream's `_swapBytes` leaves each row's
`bytes & 15` tail in place, so an 8-byte row does not move: kept, documented), and the demo scene counted the hidden
helper's texture. **Differential against upstream's compiled code** (`tests/differential_w3d.rs`, section 14; verbatim
segments of shader.cpp, meshgeometry.cpp and vp.cpp): `Apply`, `Get_SS_Category` and `Guess_Sort_Level` on 633,824
shaders (every combination of the fields `Apply` branches on, with every texture-op cap and with none, and 200,000
random bits, caps, fog and cull settings), 0 mismatches, 90,464 left out as undefined (`dstBlendLUT` past its end, an
eighth as predicted); the skin deformation over hierarchies posed by upstream's `Base_Update`, 3,061 cases (72,322
vertices, both overloads), 0 mismatches. Two mutants of the Rust were caught (`srcBlendLUT[3]` "fixed" to `INVSRCALPHA`:
68,056 mismatches; the normals' translation not zeroed: 3,054).
**Not checked:** a real Zero Hour model (no game files here); the front face is assumed counter-clockwise in the model's
space, which `MeshModel::winding_agrees_with_normals` will check on real art.

**Batch C3b (2026-11-05, GitHub #3): `OnlineChatColors`, before a real install's first load.**
Reading upstream's block table (INI.cpp `theTypeTable`) against the files the logic loads: upstream parses every file with
its whole table, the port with the logic's blocks only, so a client block inside a file the logic reads stops the port's
load. `OnlineChatColors` (the online lobby's colours, Chat.cpp) has no file of its own: it is read wherever it is met, and
the game's `Multiplayer.ini` (a logic file) is where it sits. It is now parsed as upstream parses it
(`GameSpyColorFieldParse`, `parseColorInt` into `GameSpyColor`), kept on the load and used by nothing yet. *Prediction:*
the load reads past it, sets the named colours, and refuses an unknown field. *Result:* held on the first run. Other client
blocks still stop the load if a logic file carries one (`a_missing_file_or_a_client_block_fails_the_load`); none is known.
Full run (2026-11-05): 1193 tests pass in 203 suites; clippy clean; gate (a) identical over 300 frames native and wasm (last CRC `0bf44067`).

**Batch C3 (2026-11-05, GitHub #3, #5): the player's Zero Hour folder in the browser, and the page on GitHub Pages.**
- **The page** (`client/web/index.html`): *Choose Zero Hour folder* (or its `.big` files) keeps the archives the logic
  reads (all but the art and sound ones: `Audio*`, `Speech*`, `Music*`, `Textures*`, `Terrain*`, `W3D*`, `Shaders*`,
  `Window*`), and the loose `Data\` and `Maps\` files, in that browser's OPFS, and hands them to the game; on a later visit
  *Play with saved files* reads them back. Nothing is uploaded; there is no server side.
- **The game** (`client/src/import.rs`): the files go on a memory device under the game's folder, `FileSystem::init` mounts
  the archives, `World::from_install` loads the text, the logic's INI and the map cache (I57), and the new
  `World::quick_skirmish` starts a skirmish on the first multiplayer map: the local player with the first playable side
  against a medium computer player with the next. The app swaps the demo for it and rebuilds the ground. Natively:
  `rts-client --zh <folder>`.
- **Publishing** (`.github/workflows/pages.yml`): every push to `phase1-gamelogic` or `main` builds the page with
  `scripts/build_web.sh` and deploys it to GitHub Pages. The repository's Pages source must be *GitHub Actions*, and the
  `github-pages` environment must allow the branch.

*Predictions before the runs:* the queue gives the files once and only after the import's end; a folder without archives
does not start; `quick_skirmish` on the test install picks the duel map, mounts its two files and both players have their
Barracks and Tank after 300 frames; in a browser, picking the test install's four archives keeps them (offered again after a
reload) and starts the game.

*Results:* the unit tests held on the first run. **The browser check** (headless Chromium, software WebGL2, the page built by
`build_web.sh`, the test install's archives written by `install_world`'s `write_the_test_install_when_asked`) found one
defect: the game said "the game has no local player". The local player exists from the second frame (the first dispatches
`MSG_NEW_GAME`); the client had read it after one. It now waits for it (pinned in `install_world`'s test). Then the page
logged "playing your game: on Desert Duel (2) | 11 templates, 1 maps, 0 archive errors, 4 files imported", drew both
players' Barracks and Tank on the map ([screenshot](images/c3_web_import_test_install.png)), and after a reload offered the
four saved files. **Not checked:** a real Zero Hour folder (none is in this repository); its first load is the next check.
Full run: 1198 tests pass (2026-11-05: the engine's 1192 in 204 suites, the client's 6); clippy clean on both crates (the client's one finding, a redundant cast in C1's `view.rs`, fixed); gate (a) identical over 300 frames native and wasm (last CRC `0bf44067`).

**Batch C2d (2026-11-05, GitHub #6): the client takes the player's input.**
- `client::world_host::WorldClient` (engine crate) answers every question of the selection, command, control bar and
  placement translators from a `World`: relationships through the teams, garrisons through the action manager, the
  shroud and stealth for the pick filter, hotkey squads, upgrades, sciences. `pick_kinds` translates
  `getPickTypesForContext` and `PickDrawableStruct`'s kinds. Its messages go on the World's command list as the local
  player's. Not read from the World yet: a production module's construction-special button (no module call returns it).
  Not written: the single-use command flag, which upstream's client writes on the logic object directly; this client
  writes the World only through command messages (`AGENTS.md`).
- `client/src/input.rs` (`Controls`): pointer events into the translators, in the message stream's order (placement,
  selection, commands); `client/src/demo.rs`: the demo game the window runs until the player's files are loaded, the gate's
  own test data with selectable, sighted Runners, Soldiers, a WorkDozer that can build a Depot and the Caster.
- `client/src/app.rs`: a left click selects or orders, a left drag selects in a box (drawn), the hint is in the status line,
  selected units have rings, the first selected unit's buttons run along the bottom, a construct button places a building,
  Escape cancels. Picking is by projected position (18 pixels), the ground by the camera ray on the ground plane.

*Predictions before the runs:* the questions a hovering player asks every frame (18,000 over 300 frames: hints on every
object and four ground points, normal and force attack, and `EVALUATE_ONLY` on every object) send nothing and leave the gate
game's CRCs equal to the plain run's frame by frame; in the demo, a click selects a Runner, a click on the ground moves it,
a box selects both Soldiers, a click on a distant enemy orders an attack without selecting it, and the dozer's button and a
click build a Depot.

*Results:* all held on the first run: 18,000 questions, 5,100 hints, 0 messages, CRCs identical over 300 frames; in the demo
the Runner closed from 160 to 75.5 on the point in 90 frames and the Depot stood as player 1's a frame after the click. The
window itself (`app.rs`) is GUI glue no test reaches; it builds natively and for wasm. Full run: 1198 tests pass (2026-11-05: the engine's 1192 in 204 suites, the client's 6); clippy clean on both crates (the client's one finding, a redundant cast in C1's `view.rs`, fixed); gate (a) identical over 300 frames native and wasm (last CRC `0bf44067`).

**Batch C2c, part 2 (2026-11-05, GitHub #6): building placement, and a defect in `canMakeUnit`.**
`client::place_event` translates `PlaceEventTranslator`: with a building at the cursor (from a dozer's construct button), the
left button anchors it, a drag of 5 pixels or more stretches it, and the click builds when `canMakeUnit` and
`isLocationLegalToBuild` (with upstream's flags, the shroud among them) agree: `MSG_DOZER_CONSTRUCT(template, place, angle)`,
`MSG_DOZER_CONSTRUCT_LINE` with the end for a wall, or `MSG_DO_SPECIAL_POWER_AT_LOCATION` from the builder for a construction
special; otherwise `displayCantBuildMessage`'s reason. InGameUI gains the placement anchor and angle (`handleBuildPlacements`'
turn to face the drag), and `placeBuildAvailable` takes the template's placement view angle.

**A defect found by the end-to-end test, in the logic:** `BuildAssistant::canMakeUnit` refused on `cost >= money` where
upstream refuses on `cost > money` (BuildAssistant.cpp:1343). A player with exactly the cost could not build, nor could one
with no money build something free. ~~`>=`~~ corrected to `>`; the upgrade check (`canAffordUpgrade`, `money >= cost`) was
checked and matches upstream.

*Predictions before the runs:* in each test's doc. In short: the button starts placing at the template's view angle; the
button down anchors; 3 pixels do not stretch, 10 do, and the icon turns to face the drag; the click sends
`MSG_DOZER_CONSTRUCT` and ends the placement, a wall sends the line with its end, an illegal place shows its reason and keeps
the building at the cursor; in the gate World, a WorkDozer's construct button and a click at (150, 150) start a Depot there,
which the dozer finishes within 600 frames.

*Results:* the unit test held on the first run. The end-to-end test found the `canMakeUnit` defect above (the free Depot was
refused with no money); after the fix, the Depot started at (150.0, 150.0) and was finished 203 frames later. The test's
dozer is `SELECTABLE` and has a vision of 250, as shipped dozers do: placement requires a revealed site. Full run:
1189 tests pass in 203 suites (2026-11-05), the differential tests among them; clippy clean; gate (a) identical over 300
frames native and wasm (last CRC `0bf44067`: the fix does not move the gate game, whose builds are paid for).

**Batch C2c, part 1 (2026-11-05, GitHub #6): the control bar's buttons, and the cursor modes they start.**
`client::control_bar` translates `ControlBar::processCommandUI` (ControlBarCommandProcessing.cpp): a button needing a target
becomes InGameUI's pending GUI command; the others build a unit (`MSG_QUEUE_UNIT_CREATE` with a new production id, after
`canMakeUnit`'s refusals and their messages), start a building's or a construction special's placement, cancel, queue and
cancel upgrades, stop, sell, toggle attack move, evacuate, exit a container, fire, switch weapons, use a special power (from
the unit or the shortcut bar), hack, overcharge, select every unit of a type, or buy a science. `GuiButton` is a button as the
client reads it (`command_world::gui_button` makes it from the World's button facts).

`evaluateContextCommand` gains the pending GUI command's branch: the shrubbery, mine and relationship filters, then
`canSelectedObjectsDoAction` (car bomb, hijack, sabotage, combat drop into), `canSelectedObjectsDoSpecialPower` and
`canSelectedObjectsEffectivelyUseWeapon` (InGameUI.cpp), answered through the action manager; `issueFireWeaponCommand`,
`issueCombatDropCommand` and the shortcut half of `issueSpecialPowerCommand` (a power whose destination the player then
steers selects its source). An invalid mode returns `MSG_INVALID_GUICOMMAND_HINT` whatever the type, as upstream does. A
construction special issues nothing here (upstream's switch for it has no matching case); the placement makes it, in part 2.

Not here: the buttons' windows, images and sounds, the radius cursor (the view, C2d); building placement
(`PlaceEventTranslator`, part 2).

*Predictions before the runs:* in each test's doc. In short: the build button makes `MSG_QUEUE_UNIT_CREATE(template, id)`,
or without money only the EVA's voice and `GUI:NotEnoughMoneyToBuild`; a positional power becomes the pending command and
sends nothing; select-all-of-type selects 1 and 3 of {1, 2, 3}; a pending power at a position, allowed, sends
`MSG_DO_SPECIAL_POWER_AT_LOCATION(7, pos, INVALID_ANGLE, INVALID_ID, options, INVALID_ID)` and clears the mode; a weapon
mode on an ally is invalid. In the gate World, the Caster's drop button, then a click at (200, 200), drops one Pup there.

*Results:* the unit tests held on the first run. The end-to-end test held after one correction to its Caster: it has no
vision, so all of player 1's map was shrouded and the daisy-cutter-type power was rightly refused (upstream refuses damaging
powers on shrouded cells); with a vision of 250 the Pup stood at (200.0, 200.0) a frame after the click. Full run (2026-11-05): 1187 tests pass in 203 suites; clippy found six needless `&mut` borrows in the new tests,
fixed (clean after); gate (a) identical over 300 frames native and wasm (last CRC `0bf44067`).

**Batch C2b (2026-11-05, GitHub #6): a click orders the selection.**
`client::command_xlat` translates `CommandXlat.cpp`'s command translator and the selection questions of `InGameUI.cpp` it
asks:
- `evaluateContextCommand` with no GUI command pending: in upstream's order of precedence, the special power destination
  override, resume construction, dock (all must qualify), repair, get repaired, get healed, hijack, car bomb, sabotage,
  salvage, enter, attack (now or after moving), capture and the three hacks (through the first selected unit's command
  set, upstream's "auto specials"), the rally point, the impossible shot, and otherwise a move (or waypoint, attack move,
  force move), with each one's hint for a mouse-over;
- `evaluateForceAttack`, `issueMoveToLocationCommand`, `issueAttackCommand` (one message for a group, else one per
  selected unit), `createEnterMessage`, `issueSpecialPowerCommand` (for the auto specials), and the
  `MSG_MOUSE_LEFT_CLICK`, alternate mouse `MSG_MOUSE_RIGHT_CLICK` and mouse-over hint cases;
- `areSelectedObjectsControllable`, `canSelectedObjectsDoAction` and `getCanSelectedObjectsAttack` with their `ANY`/`ALL`
  rules, and `canSelectedObjectsOverrideSpecialPowerDestination`.

`client::command_world` answers the per-object questions from a `World`, through the action manager already translated
(`canDockAt`, `canEnterObject`, `getCanAttackObject`, ...), and `canObjectForceAttack` with its spawn-slave and Firebase-rider
cases. `InGameUI` gains the waypoint, force move and attack move modes. The stub of C2a is gone: the selection translator
asks `evaluate_context_command(draw, pos, EVALUATE_ONLY)`, as upstream asks `TheGameClient`.

Not translated yet: a pending GUI command (special powers, fire weapon, combat drop and the car bomb, hijack and sabotage
modes, from the control bar: C2c), unit voices and the stats collector (no messages), and the double-click guard order
(off in the shipped options).

*Predictions before the runs:* written in each test's doc. In short: the ground gives `MSG_DO_MOVETO` (no group, no move);
an attackable enemy `MSG_DO_ATTACK_OBJECT`, hinted "after moving" when out of range; an enterable unit of mine
`MSG_ENTER(INVALID_ID, it)`, which wins over attacking it; one of mine with nothing to do, no command and no move; the
infantry capture button issues its special power at the building. In the gate World, a click moves the selected Runner
toward the point, and a click on an enemy beyond vision makes the selected Soldier close on it, the enemy not selected.

*Results:* the translator's tests held on the first run. The end-to-end test held after three corrections to its World,
none to the code: the recorder's skirmish Tank has no AI, so with the logic live it can neither be grouped nor move (the C2a
test passes because its World leaves the AI ablated); the gate World was not a game in progress, and the dispatcher makes a
command's group only in a game; and the gate's units are not `SELECTABLE`, which `Squad::getLiveObjects` requires. With
selectable copies of the gate's Runner and Soldier, the Runner went 160 -> 75.2 from its goal in 90 frames, and the Soldier
320 -> 235.1 from the enemy, which did not move. Full run (2026-11-05): 1184 tests pass in 203 suites; clippy found two `field_reassign_with_default` in the new tests, fixed (clean after); gate (a) identical over 300 frames native and wasm (last CRC `0bf44067`).

**Batch C2a (2026-11-04, GitHub #6): the selection, from a click to the logic's selected group.**
The client's renderer-free translations go in the logic crate's `src/client/`, beside the logic and with no new dependency, so
that they are tested without Bevy and run the same on the web and natively. Three pieces:
- `client::in_game_ui`: `InGameUI`'s selection list (`selectDrawable`, `deselectDrawable`, `deselectAllDrawables`, the
  counts and the force-attack and prefer-selection modes). The list keeps upstream's order, newest first.
- `client::selection_info`: `SelectionInfo` and `contextCommandForNewSelection`, which counts what a click would pick by whose
  it is and decides whether it is an order to the current selection. The order itself (`evaluateContextCommand`) is
  `CommandXlat`'s, C2b; ~~until then the host answers "no order", so a click selects~~ **since C2b** the selection
  translator asks the command translator.
- `client::selection_xlat`: `addDrawableToList` (the pick filter: kinds, the fire base's gunners picking their container,
  fogged and stealthed enemies refused) and `SelectionTranslator`'s `MSG_MOUSE_LEFT_CLICK`. It emits
  `MSG_DESTROY_SELECTED_GROUP`, `MSG_CREATE_SELECTED_GROUP` and `MSG_REMOVE_FROM_SELECTED_GROUP`, as upstream does.
- `client::selection_groups`: the double click (`selectMatchingAcrossScreen`, or with alt `AcrossMap`: every unit of mine
  like the selected ones, within the selection cap) and the control groups (`MSG_META_CREATE/SELECT/ADD/VIEW_TEAMn`, with
  the double press within 20 frames that looks at the group). Upstream orders the matched templates by pointer
  (`std::set<const ThingTemplate*>`); here they go in the template store's order, so the ids' order in that one message
  can differ from a given run of the original.

Each reaches the game through a host trait (picking needs the view; the facts of objects come from the `World`). Not
translated yet: the raw button feedback (the drag rectangle's hints, C2d with the view), and the debug-only click modes.

*Predictions before the runs:* written in each test's doc. In short: a click on my unit deselects all, then makes a new group
of it; a drag over two units and a building takes the units only; shift and a click on a selected unit removes it; a fogged
enemy is not picked; in the recorder's skirmish, a click on the tank makes the player's selection `[tank]` after one frame,
a drag over both makes it `[tank]`, and a click on the Barracks makes it `[barracks]`; a double click selects the screen's
units like it (alt: the map's); ctrl+3 makes group 3 in the drawable list's order, 3 selects it without a
`MSG_DESTROY_SELECTED_GROUP`, and 3 again within 20 frames only looks at it; viewing group 0 does nothing (upstream's range
is 1 to 10).

*Results:* all held but one, and that one was the fixture. The end-to-end test first picked nothing: the test world's
templates carry no `SELECTABLE` kind, so the pick filter refuses them, as upstream's would. The test's host now picks every
object (the filter has its own test). Full run (2026-11-05): 1181 tests pass in 203 suites, clippy clean, gate (a) identical over 300 frames native and wasm (last CRC `0bf44067`).

**Batch F1 (2026-11-04, GitHub #9): `TheFileSystem`, and archives read by their directory only.**
`src/common/file_system.rs` translates three pieces:
- `FileSystem.cpp`: `openFile`, `doesFileExist`, `getFileInfo` and `getFileListInDirectory`. The local files come first, then the archives.
- `Win32LocalFileSystem`'s existence check, file info and recursive directory listing, over a `LocalDevice`. A device is the browser's storage of the imported folders (#3), a folder on disk (`NativeDevice`, which finds each path component without case, as a Windows install does) or memory (`MemoryDevice`).
- `Win32BIGFileSystem::init` and `loadBigFilesFromDirectory`. They load every `*.big` under the game's folder, subfolders included, then every one under the *Generals* install path, without overwriting. The first archive to supply a path keeps it.

`formats::big` gains `ArchiveFile::open_archive_directory`, `member_range` and the `ArchiveFileSystem` counterparts. Their two jobs:
- The directory is parsed by the existing `open_archive_file`, on a prefix of the archive: 64 KiB, then four times as much while the parse stops at the prefix's end.
- A member is read from its range when it is opened, so a gigabyte archive is never held whole.

`FileSystem` implements the logic's `GameDataFiles`, so `load_game_data` reads a real install's INI through it.

Two places where the translation differs:
- upstream's existence cache (`m_fileExist`, keyed by `nameToLowercaseKey`) is not kept;
- `FindFirstFile`'s wildcard is matched by `SearchStringMatches` without case.

*Predictions before the runs:*
- a loose file wins over an archived one;
- between archives, the first loaded keeps a path, so Zero Hour's `INIZH.big` serves `Data\INI\Weapon.ini` over the Generals `INI.big`;
- a path only Generals has is served by Generals' archive;
- an archive in a subfolder is found;
- no archive is held whole;
- the directory-only parse equals the whole parse, for a 3,000-entry directory read in more than one prefix;
- the gate's INI read from inside an archive gives the same templates and name keys as the same INI given as loose files.

*Results:* all held but one, and that one was the test's layout, not the code. The first run put the Generals folder inside the game's folder. `loadBigFilesFromDirectory("", "*.big")` searches subfolders, so it found that archive first (name order) and it kept `Weapon.ini`, exactly as upstream would. The test now puts the two folders side by side, as a Steam install and the browser import do. For that, the local file system takes the game's folder within the device (`game_dir`), as upstream's paths are relative to its current directory, with absolute paths (`\Generals\`) for the install path. The full run (2026-11-04): 203 test binaries, 1,163 tests, none failed; clippy no warnings; gate (a) PASS, last CRC `0bf44067` (unchanged: the gate reads its INI from memory).
