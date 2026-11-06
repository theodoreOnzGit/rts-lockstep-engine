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
- **Audio** sits behind our own small trait. On the web it is the **Web Audio API** through `web-sys`. ~~Natively it is a
  Rust backend, chosen when audio starts (milestone C6).~~ **Corrected 2026-11-06 (#13):** the trait is in place
  (`client::game_audio::AudioDevice`, the client's `audio::AudioOutput`); natively the output is still silent (it keeps
  the clock and the lengths, so the manager runs as with sound): no Rust backend is chosen yet (see #13's entry below).
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
- ~~WND layouts.~~ **Corrected 2026-11-05 (#12):** translated: the WND reader (`client::window_script`,
  `GameWindowManagerScript.cpp`), the mapped images (`client::image`), the control bar schemes
  (`client::control_bar_scheme`) and the control bar's command windows (`client::control_bar_command`); still missing:
  the window manager's input and drawing, the gadgets' own behaviour, and every layout but the control bar's (see #12's
  history entry below).

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

**#59 (2026-11-06, GitHub #59): the drawables saved and loaded.**
- **Upstream** (`Drawable::xfer` v7 and `xferDrawableModules`, `TintEnvelope::xfer`, `Anim2D::xfer` (Drawable.cpp,
  Anim2D.cpp); `DynamicAudioEventInfo::xferNoName`; `Module`, `DrawableModule`, `DrawModule::xfer`; `W3DModelDraw::xfer` v2
  and the xfers of every draw class the port makes from `W3DModelDrawModuleData`: `W3DTankDraw`, `W3DTruckDraw`,
  `W3DTankTruckDraw`, `W3DPoliceCarDraw`, `W3DOverlordTankDraw`, `W3DOverlordTruckDraw`, `W3DOverlordAircraftDraw`,
  `W3DSupplyDraw`, `W3DScienceModelDraw`, `W3DDependencyModelDraw`; `GameClient::xfer`'s drawable part, `shouldSaveDrawable`,
  `GameClient::loadPostProcess`, `Drawable::loadPostProcess`). New file `src/client/drawable_save.rs`; `save_game.rs`'s
  `CHUNK_GameClient` now writes and reads the drawables (`SaveHost::with_drawables`).
- **The load's order is upstream's:** `CHUNK_GameLogic` has made the objects; `CHUNK_GameClient` clears the client's
  drawables and, per saved drawable, takes its object's drawable (made as the object's creation makes it upstream; made
  again from the saved template when it differs, upstream's disguised bomb truck) or a new one without an object, and xfers
  into it. The condition flags are read before the modules and forced onto them (`replaceModelConditionFlags(TRUE)`), so the
  animation's saved fraction lands in the right animation; the post-process gives each drawable with an object its
  object's transform and raises the drawable id counter.
- **Read as upstream reads it, all versions:** the drawable's versions 1 to 7 (condition state after the modules before 2,
  position and orientation in 3 and 4, the ambient switches from 4 and 6, the custom sound from 7), `W3DModelDraw` 1 and 2.
  A module the port has not got (a client update module, `W3DDefaultDraw`, trees, ...) is found by its tag as upstream and,
  where upstream skips it, its block is kept and written back; a drawable of an unknown template is skipped by its size.
- **Kept but unused** (read, held in `DrawableSaveFields`, written back): the drawable id, the transform, both tint
  envelopes and the tint statuses, the terrain decal, the six opacities, `m_status`, the fade members, the flash count and
  colour, the second material pass opacity, the instance matrix, identity flag and scale, the shroud status object, the
  expiration date, the icons (templates unchecked: no `Anim2DCollection`), unknown modules' blocks,
  `W3DDependencyModelDraw::m_dependencyCleared`, the drawables without objects (`ClientDrawables::object_less`, not drawn),
  the animation mode `W3DModelDraw::xfer` writes ("note, this will be ignored", upstream too).
- **Used:** the condition flags, the animation frame (a fraction of its frames), the recoil infos, the `showSubObject` list,
  `m_locoInfo` (but its yaw and pitch modulators, which upstream does not save), the hidden and hidden-by-stealth flags, and
  the ambient sound: each drawable's switches and custom sound are copied from the client audio before a save
  (`AmbientSounds::save_into`) and given back after a load once the audio is up (`AmbientSounds::restore_saved`: the custom
  description rebuilt from its base and the saved overrides, then `loadPostProcess`'s start or stop). `CustomAmbient::Info`
  now carries the original name and the override bits `xferNoName` writes.
- **Upstream's choices kept, against the brief:** the tank and truck draws save **no** tread, wheel or police-car frame
  state ("John A and Mark W say there is no data to save here"), so tread offsets start at 0 after a load; the stealth look is
  written but a load sets it to `STEALTHLOOK_NONE` for the next stealth update to set; a transition state's animation is not
  saved.
- **The client:** `saves::save_game`, `load_game` and `client_from_save` take the drawables (`saves::Drawables`: the
  drawables, the art, the clock); the application passes its own (`WorldDrawables::with_assets`) and, on a load, makes the
  new game's drawables before the load (`Game::from_loaded`). A drawable remade when its art arrives (the browser) now keeps
  its saved members, client physics and `showSubObject` lists.
- *Predictions and results* (in the tests' doc comments): `save_game`'s `the_drawables_round_trip_through_a_save` (the
  flags, the module's state, the animation frame 7 of `tanka.move` restored exactly at the load's clock, the treads at 0 as
  upstream saves none, the stealth look reset with the drawable still hidden by stealth, the hidden flag, the turret list,
  recoil, client physics, tint, icon, opacity, ambient saves, a kept client update module and an object-less drawable all
  back; the second and third saves' `CHUNK_GameClient` equal; the logic CRC unchanged by the save and the client's load
  equal to the World's alone for 30 frames). **Miss:** the table was predicted to hold one template; it holds six, as the
  client's draw makes a drawable for each of the gate game's fifteen objects. `drawable_ambient`'s
  `ambient_sounds_restore_from_a_save` (override bits 73, the custom description rebuilt, the post-process plays 9's and
  11's): first run. `saves`' demo test now saves and loads the drawables: they come back by object and template.
- **Gates:** lib 1029 (1027 + 2), client 41 (one test extended), clippy clean, web build clean, gate (a) PASS (last
  `0bf44067`, unchanged), gate (b) passes (in the lib tests).
- **Not done** (`issue59_followups`): no `Anim2DCollection` (icon templates unchecked); the kept members drive nothing yet;
  object-less drawables and client update modules are kept, not drawn or run; no original save has been read (none here).

**#62 (2026-11-06, GitHub #62): campaign missions and the Generals' Challenge started from the client; mission saves.**
- **Upstream read first:** `GameLogic::startNewGame`'s `loadMapINI` (the map's folder: `map.ini`, `solo.ini`, `map.str`,
  `AssetUsage.txt`, through `TheFileSystem`), `FileTransfer.cpp`'s map files (`GetINIFromMap`, `GetStrFileFromMap`,
  `GetSoloINIFromMap`: what a peer needs of a map), `CampaignManager::getCurrentMap` (the campaign's `Map` field, the
  path `startNewGame`'s pending file takes), `ScoreScreen.cpp` (`finishSinglePlayerInit`, the Continue button,
  `startNextCampaignGame`) and `GameState.cpp` (`missionSave`, `saveGame`'s mission map, `xferSaveData`'s two blocks,
  `loadGame`'s mission branch).
- **Design: option (b), the map read by range, as the art is.** The page hands the logic's archives (`MapsZH.big` among
  them) over whole; after the start the logic's memory device is dropped, but the art's device holds every archive by its
  directory and the page's key (`import::Starting`), so any member can be read back by range. A chosen mission waits
  (`PendingMission`, `src/logic/world/campaign_world.rs`) until **every** file of its map's folder has arrived
  (`World::mount_map_when_ready`, new: the folder read; if any read waited, the device has asked for its bytes and
  nothing is mounted), then mounts them all whole and starts. A mission never starts from part of its folder, so every
  peer reads the same bytes; holding every campaign map in memory (option a) was not needed. Natively (`--zh`) the art's
  device is the folder on disk and the mount is immediate. The mission starts in a copy of the install's World
  (`Started::base`), as a save's load does (the port has no `clearGameData`).
- **The flow** (`client/src/campaigns.rs`, plain Rust; `app.rs` glue): a `C` button under the zoom column's `S` opens a panel
  (the game paused under it): Close, the difficulty (Easy/Normal/Hard, normal by default), the status and the briefing,
  then each campaign of `Campaign.ini` that is not a challenge's (the file's order: USA, China, GLA and the training
  campaign in a real install) with every mission, and the Generals' Challenge's generals of `ChallengeMode.ini`, each a
  button (`list_button`, wrapping at phone width). Upstream's menus start a campaign's first mission only; the list
  offers every one (`start_campaign` for a first, `start_campaign_mission` otherwise). The challenge's locked generals
  (`StartsEnabled`, battle honours) are offered, marked: the port keeps no honours. **The game's end:** the script
  engine's end-game timer running out (upstream appends `MSG_CLEAR_GAME_DATA` there, a cut in the port) is
  `World::single_player_game_over`; the client then runs `finishSinglePlayerInit` on a copy of the World's campaign (the
  client never changes the simulation), opens the panel with Save and Continue after a victory (Retry after a defeat:
  the same mission, as upstream's Continue then), nothing at a campaign's end, and Continue starts
  `startNextCampaignGame`'s mission, read by range as the first. **Briefings:** the movies are Bink (never released); the
  mission's location and objective lines are shown from the game text.
- **Part 2, mission saves (translated):** `GameState::missionSave` (`World::mission_save_description`: `GUI:MissionSave`
  with the campaign's name and the next mission's number; `World::mission_save`: file type `SAVE_FILE_TYPE_MISSION`, the
  mission map, only `CHUNK_GameState` and `CHUNK_Campaign`) runs at a victory's score screen, as upstream's
  `finishSinglePlayerInit` does, and the save goes into the save folder. `loadGame`'s mission branch
  (`World::start_loaded_mission`: `InitRandom(0)`, the pending file the mission map, `MSG_NEW_GAME`) runs at the end of
  `World::load_game`; the saves panel's Load of a mission save goes to the campaign flow
  (`PendingMission::from_mission_save`), so its map is read by range too.
- **Defect found and fixed:** `NewGameHost::sn_has_map_cache` answered the stand-in `cuts.new_game.has_map_cache`, false in
  every World but two tests', so `loadMapINI` returned at once and **no install game ever read its map's `map.ini`,
  `solo.ini` or `map.str`** (a campaign mission's overrides lost). Now true, as the save side's `has_map_cache` has been
  since batch I55. No existing test moved (all lib tests pass with the fix; the duel's `map.ini` is empty).
- *Predictions and results* (the tests' doc comments): `campaign_world`'s `a_mission_read_by_range_starts_and_victory_moves_to_the_next`
  (one round of three ranges, 3 files mounted, the members' bytes, under 64 KiB arrived of the 314 KB archive; the game
  over exactly 120 frames after the start frames; `USA02` one round of one range; Retry after a defeat; errors) — **miss
  on the first run:** the mission's `map.ini` was never read, the defect above; as predicted after the fix.
  `a_mission_read_by_range_runs_as_one_read_whole` (ranges served in order, in reverse, and the archives whole: the same
  CRC on each of 300 frames, `99dd30e6` at 300; the challenge twice for 200): as predicted, first run.
  `a_mission_save_loads_the_next_mission` (two blocks, type 1, map `USA02`, description `MISSING: 'GUI:MissionSave'`, the
  load at `m02`/hard, one round, the tank at 180, two loads agree for 150 frames, a normal save refused): first run.
  `campaigns`' `the_list_names_the_campaigns_missions_and_generals`: first run. `campaigns`'
  `a_mission_from_the_list_runs_on_the_browsers_import_when_asked` (`RTS_CAMPAIGN_INSTALL`, the archives written by
  `write_the_campaign_install_when_asked`; the client's own import path, `import::start_from_files`): `[3]` ranges, the
  score screen at frame 120, the mission save loading `usa mission 2` after `[1]`, the Continue then reading nothing, two
  runs' CRCs equal for 150 frames — as predicted, first run once the prediction was corrected before it ran (it first
  expected Continue to read `USA02`'s range, which the mission save's load had already read).
- **Checked in headless Chromium** (`$S/webcheck/check62.js`, the campaign install written by
  `write_the_campaign_install_when_asked`, 1000 x 700 and 390 x 844 touch): the import plays the duel; `C` opens the list
  (`Campaign usa`: m01, m02; the Generals' Challenge: General 0); m01 starts (`playing usa mission 1`), its map read from
  `MapsZH.big` in 3 ranges, 1,577 bytes; the score screen opens at logic frame 121 to 123 (`over victorious | mission saved
  00000000.sav`); Save and Continue starts `usa mission 2`, which runs on; no horizontal overflow, panel open or not.
- **Gates:** lib 1039 (1035 + 4), client 43 (41 + 2; with `RTS_CAMPAIGN_INSTALL` set, so the gated test ran), clippy
  clean, web build clean (no warnings), gate (a) PASS (last `0bf44067`, unchanged).
- **Not done:** the game's own menus (C5: main menu, difficulty, challenge menu, score screen with its stats); the
  challenge's win/loss display and sounds, battle honours, the final victory movie; a mission save's map label (the
  played game's map is not the saving World's); the challenge's `TheChallengeGameInfo` after a mission save's load
  (upstream's mission save holds no `CHUNK_GameLogic` either); a save of a mission started from the list loads into the
  install's World with the map the save embeds but without the folder's `map.ini` (the load does not read the folder by
  range yet).

**#61 (2026-11-06, GitHub #61): the campaign manager and the Generals' Challenge.**
- **Translated:** `CampaignManager.cpp` whole (`src/client/campaign_manager.rs`: the `Campaign` INI with its `Mission` blocks,
  `newCampaign`, `newMission`, `getMission`, `getNextMission`, `setCampaign`, `setCampaignAndMission`, `gotoNextMission`,
  `getCurrentCampaign`, `getCurrentMission`, `getCurrentMap`, `getCurrentMissionNumber`, the victory flag, the rank points
  and difficulty, `xfer` v5 and `loadPostProcess`) and `ChallengeGenerals.cpp` whole (`src/client/challenge_generals.rs`:
  the `ChallengeGenerals` INI, its twelve `GeneralPersona` blocks, the lookups by campaign, general and template name, the
  player template number and difficulty). The World holds both (`World::campaign`, `World::challenge_generals`), as the
  logic reads them; `World::from_install` reads `Data\INI\Campaign.ini` and `ChallengeMode.ini` (upstream's
  `GameClient::init`), each when present (**deviation:** upstream's `INI::load` throws on a missing file).
- **Cuts made live:** `CampaignStandIn` (`cuts.campaign`, removed: `CHUNK_Campaign` and `CHUNK_GameLogic` now xfer the real
  manager, `TheChallengeGameInfo` included, so a campaign's or a challenge's save loads instead of failing with
  `SC_INVALID_DATA`); `cuts.new_game.campaign` (`startNewGame`'s and the script engine's current campaign, never set);
  `cuts.challenge_generals_locked` (`populateRandomSideAndColor`'s locked generals, never filled); the script actions'
  `TheCampaignManager` (`sa_has_campaign_manager` was false, so `VICTORY`, `DEFEAT`, `QUICKVICTORY` and the local defeat
  never set the victory flag); `startNewGame`'s `SetVictorious(FALSE)`; the save summary's campaign and mission number
  (`GameStateHost::current_campaign`, was `None`); `CHUNK_Campaign`'s post-process.
- **Starting a mission** (`src/logic/world/campaign_world.rs`, the menus' logic parts, C5's menus not translated):
  `World::start_campaign` (`setCampaign`, `prepareCampaignGame`, `setupGameStart`, `doGameStart`: `MSG_NEW_GAME` with
  `GAME_SINGLE_PLAYER`), `World::start_challenge` (`setGeneralCampaign` and the Play button: `TheChallengeGameInfo`'s slot 0),
  `World::finish_single_player` (`finishSinglePlayerInit`'s `gotoNextMission`) and `World::start_next_campaign_game`
  (`startNextCampaignGame`). The logic's `startNewGame` already handled `GAME_SINGLE_PLAYER` (the map's sides, the human
  one local, the others its computer players, its scripts; `GameSource::Challenge` for a challenge): nothing there changed.
  **Deviation:** `clearGameData` is a cut, so the next mission starts in a World without a game given the campaign's state
  (`World::carry_campaign_from`), as a save's load does.
- **An upstream quirk kept:** `CampaignManager::xfer` reads the rank points and difficulty before `setCampaignAndMission`, and
  outside a campaign that is `setCampaign("")`, which resets them to 0 and normal: a skirmish save's load now ends with them
  so (the stand-in kept the saved values; `game_logic_save_host_world`'s round-trip test now runs in a campaign).
- *Predictions and results* (each test's doc comment; lib 1035, was 1029: six new, `a_campaign_save_fails_cleanly`
  replaced): `campaign_manager`'s `a_synthetic_campaign_ini_parses` and `the_campaign_moves_through_its_missions`,
  `challenge_generals`' `a_synthetic_challenge_mode_ini_parses`, `campaign_world`'s
  `a_campaign_mission_starts_and_victory_moves_to_the_next`, `a_campaign_save_loads_and_runs_on` (two loads agree for 150
  frames) and `a_challenge_starts_and_its_save_loads`, and `save_game`'s `a_campaign_save_loads_and_runs_on` (the client's
  path: a campaign's and a challenge's save, two loads agree for 150 frames, `CHUNK_Campaign` round-trips) all as predicted
  on their first full run but one **miss**: the challenge's game has two tanks, not one (the challenge slot's own starting
  unit, `placeNetworkBuildingsForPlayer`'s, which the prediction forgot). One **fixture** fix before it: the tests' map cache
  lacked the challenge's map, which `populateRandomStartPosition` dereferences unchecked (`Undefined(NullObject)`); the
  shipped cache lists every map. Client 41, clippy clean, gate (a) `0bf44067`, gate (b) passes.
- **Not done:** ~~the client's mission list (a campaign map must be on the World's disk, and the client keeps no logic file
  system after the start: holding every campaign map in memory, or reading them by range as the art is read, is the work)~~
  (**done 2026-11-06, #62**, its entry: read by range);
  a differential of the Campaign INI against the compiled CampaignManager.cpp; ~~mission saves (`GameState::missionSave`, the
  score screen's Save and Continue, `SAVE_FILE_TYPE_MISSION`'s load)~~ (**done 2026-11-06, #62**); the battle honours and
  final movie of a campaign's end.

**#58 (2026-11-06, GitHub #58): a load starts the save's own game.**
- **Upstream** (`GameStateMap::xfer`, GameStateMap.cpp): the load extracts the save's map, sets the game mode, reads
  `TheSkirmishGameInfo` (`SkirmishGameInfo::xfer` v4, already translated in `src/common/game_info.rs`) and calls `startNewGame(TRUE)` inside the
  block, before the players and objects are read; `GameState::loadGame` resets the engine first. The port's
  `GameStateMapHost::start_new_game` was a no-op and the client started the quick skirmish's two slots on the save's map
  first, so a save of any other game failed in its teams' block.
- **Chosen: the World's block starts the game, as upstream's does**, not the client in the same order. Inside the block the
  object id counter and `TheSkirmishGameInfo` are set before the start, as upstream's (the start makes the bridges and wall
  pieces, whose ids follow the counter), and the World alone (`World::load_game`) and the client's whole file
  (`client::save_game`) take one path. A World opts in with `SaveSide::new_game_on_load` (a World with no game: upstream's
  `TheGameEngine->reset()`); off, a load is into a World made from the save's map, as the gate (b) and earlier save tests
  make it. The start sets the new game's map name to the extracted copy, its pristine map name and save directory from
  `TheGameState`, gives the World a recorder if it has none (upstream's is an engine subsystem), and runs
  `World::start_new_game(true)`. The client's host then makes the radar's new map (that start's `TheRadar->newMap`, the
  client's) before `CHUNK_Radar` is read. `import::world_for_save` is now a copy of the install's World with the flag set;
  the local player is read after the load.
- **`CHUNK_Campaign`** is registered (second, as upstream) and read by `CampaignManager::xfer` over the World's campaign
  stand-in. A campaign's or a challenge's save fails cleanly there (`SC_INVALID_DATA`, `CampaignManager::xfer: InvalidData`
  in the load's report): the port has no campaigns (`Campaign.ini`, `setCampaignAndMission`, `TheChallengeGameInfo` not
  translated). Gap, below. **Corrected 2026-11-06 (#61):** translated; such saves load (#61's entry).
- **Defects found and fixed:** (1) `TheSkirmishGameInfo` was held twice (the new game's skirmish entry and a save-side
  copy that nothing filled), so every skirmish save wrote an empty slot list; the save side now reads and writes the new
  game's (`take_skirmish_game_info`, `set_skirmish_game_info`); (2) the campaign stand-in's `CampaignManager::xfer` v5 left
  out `TheChallengeGenerals`' player template number, so the port's `CHUNK_GameLogic` was four bytes short of upstream's
  (and an original save's misread from there): written and read now. Saves the port wrote before this change no longer
  load (their `CHUNK_GameLogic` lacks the four bytes, and they have no `CHUNK_Campaign`, which is skipped, so only the
  first matters).
- *Predictions and results:* `install_world`'s `a_save_starts_its_own_game_on_load` (World: a four-player game, three
  sides, two teams, the human at the third start; two loads agree for 150 frames; the save into the quick skirmish's game
  fails, **miss:** in `CHUNK_TeamFactory`, one block before the predicted `CHUNK_Players`) and `save_game`'s
  `a_four_player_client_save_starts_its_own_game` (client path: the radar's extent is the map's; ablated, the empty World's,
  and the test fails), and `save_game`'s `a_campaign_save_fails_cleanly` (a save naming campaign `usa`, mission `m1`, fails in
  `CHUNK_Campaign`, as predicted; **corrected 2026-11-06 (#61):** replaced by `a_campaign_save_loads_and_runs_on`, the
  port now having campaigns). Lib 1027, client 41, clippy clean, gate (a) `0bf44067`, gate (b) passes.
- **Not done:** the client crate's own `client_from_save` on an install's save is not reached by a client test (the
  install fixtures are the engine's test code); campaign saves (above; **done 2026-11-06**, #61's entry); #59 (`Drawable::xfer`; **done 2026-11-06**, its
  entry).

**#4 (2026-11-06, GitHub #4): saved games in the client.**
- **The whole file.** `src/client/save_game.rs` registers upstream's sixteen `SNAPSHOT_SAVELOAD` blocks in
  `GameState::init`'s order (`CHUNK_Campaign` excepted, the World's note; ~~excepted~~ **corrected 2026-11-06 (#58):** all
  seventeen, `CHUNK_Campaign` registered) and writes and reads the client's beside the
  World's. Per block: `CHUNK_Radar` (`Radar::xfer`, `xferRadarObjectList`, `RadarObject::xfer`) and `CHUNK_TacticalView`
  (`View::xfer`) translated whole; `CHUNK_InGameUI` (`InGameUI::xfer` v3: named timers, superweapon timers) translated whole
  and kept as read (the port's in-game UI does not show them yet); `CHUNK_GameClient` (`GameClient::xfer` v3,
  `xferDrawableTOC`) translated except each drawable's own data (`Drawable::xfer`), read past by its block size
  (**corrected 2026-11-06 (#59):** `Drawable::xfer` translated, see #59's entry);
  `CHUNK_ParticleSystem` (`ParticleSystemManager::xfer`) the id counter translated, the systems read past;
  `CHUNK_GhostObject` (`W3DGhostObjectManager::xfer`, `GhostObjectManager::xfer`) the local player translated, the ghost
  objects read past; `CHUNK_TerrainVisual` (`W3DTerrainVisual::xfer`, `TerrainVisual::xfer`, `WaterRenderObjClass::xfer`)
  made the World's own block, since the logic height map and water grid it holds are the World's (a version 3 block's tree
  and prop buffers are read past; a save writes version 2, which upstream loads without emptying its trees). A save's
  loaded ~~drawables,~~ particle systems and ghost objects start from their objects' state, not the save's (**corrected
  2026-11-06 (#59):** the drawables are the save's).
- **A load reports where it stopped** (`TracingLoad`, `LoadFailure`): the block, the offset in it, the last reads and the
  blocks' recorded errors. `read_save_summary` reads a save's `SaveGameInfo` and map without a game (the list, and the map
  a load starts from).
- **Defects found and fixed:** (1) a World started from an install had no `TheGlobalData->m_mapName` for its save (the new
  game's copy was set, the save side's was empty), so `GameStateMap` could not embed the map: `map_name` now falls back to
  the new game's; (2) `CHUNK_TerrainVisual` was not saved, so a height a script or a crater lowered was lost on a load;
  (3) after a load every object lacked its radar data (`on_radar`, which upstream's radar block gives back): only client
  feedback reads it, and the client's radar block now restores it (a World loading alone still lacks it).
- **The client** (`client/src/saves.rs`, plain Rust; `app.rs` glue): an `S` button in the zoom column opens a panel (the game
  paused under it, the pointer its) with *Save game*, *Close* and the saves newest first (description, map label, date,
  campaign side), each with *Load* and *Export*. Natively the folder is the game's own (`Documents\Command and Conquer
  Generals Zero Hour Data\Save`, `RTS_ZH_USER_DATA` to override), so an original game's saves are listed as they are; in the
  browser OPFS `Save/`, which the page reads at start and writes as asked; export is a download, import a file picker (the
  page's button, shown with the panel). Saves are never uploaded. A save is named `%08d.sav` by
  `GameState::find_next_save_filename`. A load starts the save's map first: the demo's (`demo_map_world`) or the install's
  World copied before its game started (`import::Started::base`, `import::world_for_save`, ~~the quick skirmish's slots~~
  **corrected 2026-11-06 (#58):** the save's own game and slots, started by its `CHUNK_GameStateMap`).
- *Predictions and results:* in the tests' doc comments (`save_game` 3, `saves` 2). **Misses:** two in the engine's test
  and one in the client's, all fixtures (an unknown template in the load's world, the local player the game start sets, an
  order in the selection's own frame); the browser check's first pass pressed nothing (Bevy's picking needs the pointer
  over a button before the press: the check now hovers, and holds a touch); the radar's fallback box covered the new
  button (moved up by one button, its test corrected); on a phone the page's import button covered *Close* (the panel now
  leaves it room on a second row).
- **Checked in headless Chromium** (`$S/webcheck/check4.js`, demo game, 1000 x 700 and 390 x 844 touch): save (status
  `saved 00000000.sav`, the file in OPFS `Save/`), reload the page (the save still listed), load from the list (status
  `loaded 00000000.sav`; frames 152 to 167 to 219 desktop, 252 to 286 to 378 phone: the game runs on from the saved
  frame); no horizontal overflow.
- **Not run:** an original game's save (`RTS_ZH_SAVE` with `RTS_ZH_DIR`, `save_game`'s third test): none is here, so it
  was skipped.
- **Not yet:** ~~`Drawable::xfer` (the drawables' looks)~~ (**done 2026-11-06, #59**), `ParticleSystem::xfer`, `W3DGhostObject::xfer`, the tree and prop
  buffers; a load starting the save's own skirmish slots or a campaign mission; the game's own save/load menu (C5); the
  in-game UI's timers shown.
- **Gates:** lib 1024 (1021 + 3), client 41 (39 + 2), clippy clean, web build clean, gate (a) PASS (last `0bf44067`,
  unchanged), gate (b) (`save_load_tests`) passes.


**#13 (2026-11-06, GitHub #13): audio, rung 1 (the manager, translated) and rung 2 in part (the device, the browser's
output).**
- **Translated** (engine crate, `src/client/`, no renderer): `audio_info` (`AudioEventInfo` and its INI blocks
  `AudioEvent`, `DialogEvent`, `MusicTrack` from INIAudioEventInfo.cpp; `AudioSettings` and
  `parseAudioSettingsDefinition` from GameAudio.cpp with OptionsMenu.cpp's preference getters; `MiscAudio` from
  INIMiscAudio.cpp; the track list); `audio_event` (all of AudioEventRTS.cpp: `generateFilename` with its random choice
  that avoids repeats, the in-turn order, the type's folder, extension and localized file; `generatePlayInfo` with the
  pitch and volume shifts and the attack and decay files; the portions, loops, owners, positions and players);
  `game_audio` (GameAudio.cpp's `addAudioEvent`, `shouldPlayLocally`, volumes, overrides, microphone and zoom;
  GameSounds.cpp's `canPlayNow`; GameMusic.cpp; and the device-independent bookkeeping of MilesAudioManager.cpp, read
  for what the device must do: the request list and delays, the 2-D and 3-D sample pools, `doesViolateLimit`, the
  priorities and interrupts, the voices, loops and portions, `getEffectiveVolume`'s distance roll-off, the playing
  list's culling, music fades). `client_audio` turns the logic's sound events and the FX lists' sound requests back
  into `addAudioEvent` calls. `formats::wav` is new (Miles was never released): RIFF WAVE and IMA ADPCM, decoded in
  steps.
- **Upstream facts found on the way:** `SoundsNight`/`SoundsEvening`/`SoundsMorning` are parsed but never read
  (`generateFilename` picks from `Sounds` only); a `MusicTrack DefaultMusicTrack` block lists itself as a track (its
  own lookup of the default finds it); `checkForSample` compares the `Type` bits with `AT_SoundEffect` (kept);
  `killLowestPrioritySoundImmediately` erases a 2-D sound from the 3-D list (undefined; not copied); localized paths
  carry a double backslash (kept; the file system skips it).
- **The device** (client crate, `src/audio.rs`, new): `ClientDevice` plays the manager's voices from the art's file
  system, so in the browser the sound archives (`AudioZH.big`, `SpeechZH.big`, `MusicZH.big`, which the page no longer
  skips) are read by range as the art is; WAV is decoded in Rust at most 256 KiB of input per frame, MP3 by the
  browser's `decodeAudioData` off the main thread. The browser's output is the Web Audio API (`web-sys`, `js-sys`,
  `wasm-bindgen-futures`, at the versions Bevy already locks): a source, a gain and a stereo panner per voice, on the
  `AudioContext` the page creates and resumes in the press that opens the folder picker or plays the saved files.
  Positional gain is the manager's `getEffectiveVolume` each logic frame, and the pan (new) the sine of the bearing
  from the camera's heading; the microphone follows the camera rig. Natively the output is silent for now.
- *Predictions and results:* the unit tests' predictions are in their doc comments; four missed on the first run and
  are recorded there (the default music track listed; a 100 ms delay waits four frames, not three, in `float`; a quoted
  provider name; the ADPCM round trip's error at block starts, from the test's encoder). The device's file state first
  advanced one step per frame, so a file that arrived played a frame late; it now goes as far as it can in a frame.
- **The differential** (`tests/differential_audio.rs`, `differential/wrap_audio.cpp`, `shim_audio`): the INI
  (4,000 cases: 6,039 loads, 1,220 succeeding and 4,819 failing on damaged values, every description, track, setting,
  misc sound and preference compared) and the events (4,000 cases, 13,949 states after `generateFilename` and
  `generatePlayInfo`, and the audio and logic generators' words): **0 mismatches** on the first run. Two planted bugs
  (the repeat rule at more than one sound; a pitch divisor) were caught (2,027 and 1,257 mismatches).
- **The CRCs:** the FX gun test (`drawables`) now also plays the audio: CRCs equal over 200 frames, 5 voices played.
- **Not done** (issues filed): a native output (no backend chosen; ALSA is not in this build environment); ~~the
  drawables' ambient sounds and the map's customised ones (`DynamicAudioEventInfo`);~~ **Corrected 2026-11-06 (#51):**
  the ambient sounds and the map's customised ones are played (see #51's parts 2 and 3 below); ~~the scripts' sounds, speech and
  music (cut points in the logic);~~ **Corrected 2026-11-06 (#51):** the scripts' sounds, speech and music are played
  (see #51's entry below); ~~unit voices on selection and orders;~~ **Corrected 2026-11-06 (#51):** unit voices are
  played (see #51's parts 2 and 3); music tracks started by the game; ~~EVA;~~ **Corrected 2026-11-06 (#55):** EVA is
  played (see #56/#55's entry below); the
  low-pass occlusion of off-screen sounds; a check in a real browser with a real install.

**#35, the rest (2026-11-06, GitHub #35): the radar and the minimap.**
- **Logic or client state?** Client. `Radar::crc` and `RadarObject::crc` write nothing (Radar.cpp:1376), so the radar is
  not in the logic's CRC and gate (a) cannot move for it. `Radar::xfer` is saved (`CHUNK_Radar`: hidden, forced, both
  object lists as ids and colours, the 64 events, the two indices); ~~this port's save does not carry that block yet (a
  gap).~~ **Corrected 2026-11-06 (#4):** the client's save writes and reads it (see #4's entry). The logic reads back only whether an object has radar data (`m_radarData`, the World's `on_radar`, already
  kept); `tryEvent`'s answer gates only client feedback. So the logic's radar calls are client events and the client's
  radar takes them, reading the World.
- **Translated:** `src/client/radar.rs` (Radar.cpp: the two object lists sorted by priority with each object's colour,
  disguise and garrison included; `newMap`'s sampling and averages; `worldToRadar`, `radarToWorld(2D)`,
  `findDrawPositions`, `localPixelToRadar`, `screenPixelToWorld`, `objectUnderRadarPixel`; the events with their colour
  table, `createPlayerEvent`, expiry in `update`, `getLastEventLoc`; `tryEvent`, `tryUnderAttackEvent` with its message,
  sound and EVA (`EVA_BaseUnderAttack`, `EVA_AllyUnderAttack`, Radar.cpp:1222-1224), `tryInfiltrationEvent`; `hide`,
  `forceOn`, `refreshTerrain`, `queueTerrainRefresh`); `src/client/w3d_radar.rs` (W3DRadar.cpp: `buildTerrainTexture`'s
  sampling with `interpolateColorForHeight`, water and bridges; `renderObjectList`'s dots, shroud and stealth twinkle;
  `setShroudLevel`; `drawEvents` with the generic and beacon triangles and the "RadarEvent" sound; `reconstructViewBox`,
  `drawViewBox`; `draw` and `W3DLeftHUDDraw`'s radar test); `TileData::doMip`/`getRGBDataForWidth(1)` and
  `WorldHeightMap::getTerrainColorAt` (`terrain_height_map`); `src/client/water.rs` (Water.ini's `WaterSet` and
  `WaterTransparency`, for `m_radarColor`).
- **The logic's side:** the script actions `RADAR_CREATE_EVENT`, `OBJECT_CREATE_RADAR_EVENT`, `TEAM_CREATE_RADAR_EVENT`,
  `RADAR_DISABLE`/`ENABLE`, `RADAR_FORCE_ENABLE`/`REVERT_TO_NORMAL`, `REFRESH_RADAR` and the victory conditions' `forceOn`
  keep their cut records and now also push `ClientEvent::Radar` (as #55 did for EVA); the dozer's construction complete
  pushes `RadarCreateEvent(RADAR_EVENT_CONSTRUCTION)`. The radar's host (`src/client/radar_world.rs`) reads the World;
  the partition's shroud is read with `get_shroud_status_for_player_at_cell` for the local player; `hasRadar` (with
  power's `m_radarDisabled`) through `player_has_radar`.
- **The client:** drawn in `ControlBar.wnd:LeftHUD` when the game's control bar is there, else (no game files) in a box
  of its own above the zoom column; the demo forces the radar on (its player has no radar building). A press on it is
  `LeftHUDInput` through `control_bar_callback` (look, move, attack move, a pending special power); a look moves the
  camera. The stealth detectors' feedback the radar refuses no longer plays. Without game files the terrain is the
  untextured terrain's green (new: upstream always has tiles).
- *Predictions and results:* in the tests' doc comments (`radar` 6, `w3d_radar` 3, `water` 1, `radar_world` 1,
  `terrain_height_map` 1; the client's `input` and `ui` one each). **Misses:** the client CRC test predicted the gate game's
  objects on the radar; none have a `RadarPriority`, so none were listed (the test now makes its own); the fallback box's
  first place (the top right) covered the phone's four-line status line (the phone check's screenshot), so it moved above
  the zoom column. The rest held on the first run, including `tryEvent`'s throttling read from upstream's unparenthesised
  distance (any two points of a map are "close").
- **Not yet:** ~~`CHUNK_Radar` in the save;~~ (**corrected 2026-11-06, #4:** saved and loaded) the hero icon's image (a white square stands in); `triggerRadarAttackGlow`;
  the dozer's construction message and voice; a map's own `WaterTransparency`; the beacon's radar pulse
  (`BeaconClientUpdate`, not translated); `GameLogic::startNewGame`'s replay `forceOn`.
- **Gates:** lib 1021 (1009 + 12), client 39 (37 + 2), clippy clean, web build clean, the page checked at 390 x 844 and
  1000 x 700 (no overflow, the radar drawn, a tap moves the camera). Gate (a): *predicted* unchanged (nothing in the
  logic's CRC changed); *result* PASS, last CRC `0bf44067`.

**#56 and #55 (2026-11-06, GitHub #56, #55): the demo's local player; the dispatcher's client calls; EVA.**
- **#56 (a defect):** the demo game never set the logic's local player (only tests did), so the logic's feedback to the
  local player never fired in the app. Upstream sets it in `PlayerList::newGame` (`GameLogic::startNewGame`): the first
  side marked `multiplayerIsLocal`, else (no network) the first `playerIsHuman`, else the first non-neutral player made
  human, each through `setLocalPlayer` (which tells the player it became local). An imported game runs the translated
  `startNewGame`, so it already had one (`import.rs` reads it); the demo now sets `Player1` through
  `player_list_set_local_player` at its game start. **A second gap behind it:** the dispatcher's client calls
  (`ClientCall`: `TheInGameUI->message`, `TheAudio->addAudioEvent`, `TheEva->setShouldPlay(EVA_BeaconDetected)`, ...) went
  only to the cut log, so even with a local player no client saw `GUI:RallyPointSet`. They are now client events too
  (`ClientEvent::Dispatch`); the client takes the messages (`WorldClient::take_logic_messages`) and plays the sounds.
  **Not in the CRC or the save:** `PlayerList::crc`/`xfer` and `Player::crc` do not write the local player, and gate (a)
  does not use the demo.
- **#55, EVA:** `src/client/eva.rs` translates Eva.cpp (the `EvaEvent` INI of `Data\INI\Eva.ini`, `EvaCheckInfo`,
  `setShouldPlay`, `setEvaEnabled`, `update` with its per-message intervals and expiry, `processPlayingMessages` with the
  priority and the side's sounds through `GameClientRandomValue`), played through the audio manager
  (`ClientAudio::update_eva`, once per logic frame). The logic's `setShouldPlay` calls already were client events but for
  two: the beacon's (now `ClientEvent::Dispatch`) and the script action `EVA_SET_ENABLED_DISABLED` (now
  `ClientEvent::EvaSetEnabled`). The client's own: insufficient funds (`WorldClient::eva_insufficient_funds`) and
  superweapon ready (`InGameUI::postDraw`'s EVA half, `SuperweaponTimers`, read through the new
  `special_power_body::peek_special_power_module`). ~~Not asked yet: base and ally under attack (`Radar::tryUnderAttackEvent`;
  the radar's events are #35's).~~ **Corrected 2026-11-06 (#35):** asked for by the radar's `tryUnderAttackEvent` (see #35's
  radar entry above).
- *Predictions and results:* in the tests' doc comments (`client/src/input.rs`
  `the_demo_sets_the_local_player_and_a_rally_point_gives_the_local_message`, `src/client/eva.rs`'s four): all held on
  the first run. The CRCs are equal with EVA attached (lib) and with the neutral player local (client).
- **Gates:** lib 1009 (1004 + 5), client 37 (36 + 1), clippy clean, web build clean. Gate (a): *predicted* unchanged
  (the gate game does not run the demo's start and client events are not CRC'd); *result* PASS, last CRC `0bf44067`.

**#52 (2026-11-06, GitHub #52): script speech and sound waits last the files' length (the owner's option 2).**
- **Translated:** `AudioManager::getAudioLengthMS` (GameAudio.cpp:944: a copy of the event, `generateFilename`,
  `generatePlayInfo`, the attack + sound + decay files' lengths summed as `Real`s) and `MilesAudioManager::getFileLengthMS`
  (MilesAudioManager.cpp:2595: 0 for an empty name or a file that does not open, else `INT_TO_REAL` of
  `AIL_stream_ms_position`'s integer total), as `logic::audio_length`. The paths are made by the client's translated
  `AudioEventRTS::generateFilename`/`generatePlayInfo` (prefix, extension, `adjustForLocalization`).
- **New (Miles was never released):** the lengths themselves. WAV from `formats::wav` (now `parse_wav_ranged`, which reads
  only the chunk headers, and `WavInfo::sample_frames`, the decoder's exact count for PCM and IMA ADPCM, short last block
  included); MP3 from the new `formats::mp3` (frame headers, ID3v2/ID3v1 skipped, Xing/`Info` and `VBRI` frame counts;
  else the audio bytes over the first frame's bit rate; a full frame walk, `mp3_walk`, is there for comparison and is not
  used by the logic). Each file's length is whole milliseconds, truncated.
- **The fixed choice: index 0 of each list.** Upstream's copy is not logical, so a `RANDOM` sound and every attack and
  decay are picked with each machine's own *audio* stream (upstream can desync there); the logic must not draw from its
  own stream, because upstream does not. Index 0 is the file upstream picks for a non-`RANDOM` sound (`++(-1)`), music and
  dialog have one `Filename`, it needs no generator and it is one of upstream's own possible answers.
- **Miles, what is not known:** truncation vs rounding (at most 1 ms per file); whether ADPCM is counted by samples or by
  `nAvgBytesPerSec`; whether Miles believes a Xing header or estimates every MP3 from its first frame's bit rate. A few
  ms moves a wait only when the total crosses a multiple of 33.33 ms. Upstream's own comment ("to get the file info
  without actually opening the file") is why a header-only MP3 length is preferred to a frame walk.
- **How the bytes reach the logic.** `World::from_install` reads the headers after `load_game_data`
  (`GameDataLoad::load_audio_lengths`, kept through `from_statics`/`into_statics` in `GameStatics::audio_lengths_ms`),
  through `GameDataFiles`' new `file_exists`/`file_size`/`read_file_range` (`FileSystem::read_file_range` reads an
  archive member's range, never the member). Natively that is the install's file system. In the browser the audio
  archives are on the art's ranged device, so `client::import::Starting` runs the same read each frame until no range is
  missing (`poll_start`), then loads with `World::from_install_with_audio(logic files, art files, language)`: the load
  reads only bytes already held, so every machine computes the same lengths. Each file costs a 512-byte prefix (an MP3
  also its 4 KiB frame window and its last 128 bytes). The language is `GetRegistryLanguage()`'s, as upstream: machines
  with different languages read different speech files, as upstream's do.
- **`cuts.audio_lengths_ms` is gone**; `se_audio_length_ms` reads `statics.audio_lengths_ms`.
- **Tests (predictions in each doc comment, written before the runs):** `formats::wav`'s
  `lengths_from_the_header_equal_the_decoded_count` (PCM 8/16-bit, IMA mono and stereo with short last blocks: the header
  count equals the decoder's, 100, 1000, 100, 47, 46 ms; a parse with the data unreadable gives the same info): held.
  `formats::mp3`'s test (CBR behind ID3v2 and before ID3v1: walk 300 frames, 7836 ms, header estimate 7825 ms; Xing
  `Info` 7836 ms; MPEG-2 Xing 26122 ms; VBRI 1306 ms; junk and a false sync skipped): one miss, in the fixture (`f3 50
  c0` is 40 kbit/s in the MPEG-2 table, not 64; fixed to `f3 80 c0`). `audio_length`'s first-of-each-list test (`Boom`
  100 + 400 + 50 = 550 ms with 9000 ms files second in each list; localized `Talk` 1234 ms; MP3 7825 ms; a missing file
  0): held. The archive-by-range test (the browser's way): held, lengths equal the in-memory ones; 3 waiting passes,
  11,091 bytes fetched of 284,250 at first, then 6,095 after the fix below. **The script wait**
  (`a_speech_wait_ends_on_the_frame_its_files_length_gives`): `HAS_FINISHED_SPEECH` of `Talk` (1234 ms) asked at frame
  100 is first true at 137 (`REAL_TO_UNSIGNEDINT(37.02)`), again asked at 137 true at 174; `HAS_FINISHED_AUDIO` of
  `Boom` (550 ms, 16.5 frames) true at 116; a length-0 event true at once: all held.
- **Client** (`a_start_waits_for_the_sound_headers`): two misses on the first runs. The test archive was smaller than
  the directory read, so it was held whole and the start never waited (a fixture error: a 300 KiB member was put first).
  Then 4,622 bytes were fetched where under 2,048 was predicted: on the first pass the 512-byte prefix was not held, so
  the reader fell through to the MP3 reader and asked for its 4 KiB window of a WAV. Fixed in the logic: a file whose
  prefix is not there is skipped until it is. Then held.
- **Gate (a):** the gate game has no speech or sound waits and loads no audio. *Predicted:* unchanged. *Result:* PASS,
  300 frames, last CRC `0bf44067`. Lib 999 (994 + 5), client 34 (33 + 1), clippy clean, web build clean.
- **Not done:** no check against a real install (none at hand) or against Miles (never released); ~~a map's `map.ini`
  audio redefinitions (as #50).~~ **Corrected 2026-11-06 (#51):** a map's `map.ini` audio events now reach the logic's
  and the client's descriptions (see #51's parts 2 and 3); their lengths are still the install's (computed at load,
  before the map).

**#51 (2026-11-06, GitHub #51): audio follow-ups, parts 2 and 3 (unit voices; ambient sounds; `map.ini` audio).**
- **Translated, unit voices** (`client::unit_voice`, new): CommandXlat.cpp's `PickAndPlayInfo` and
  `pickAndPlayUnitVoiceResponse` (CommandXlat.cpp:295-753: every case, the move voice "upgraded" by crush, salvage and the
  GLA worker shoes, the weapon damage types, the special power module's `InitiateSound`, the car bomb's extra voices) and
  `translateGameMessage`'s `MSG_CREATE_SELECTED_GROUP`/`MSG_SELECT_TEAMn` case (only locally controlled units speak).
  Called where upstream calls it: `issueMoveToLocationCommand`, `issueAttackCommand`, the three branches of
  `issueSpecialPowerCommand`, both of `issueCombatDropCommand`, `issueFireWeaponCommand` at an object,
  `createEnterMessage`, both of `evaluateForceAttack`, and `evaluateContextCommand`'s resume construction, dock, repair,
  get repaired, get healed and salvage (`client::command_xlat`); `ControlBar::processCommandUI`'s evacuate, hack internet
  and switch weapon (`client::control_bar`); `PlaceEventTranslator`'s construct (`client::place_event`). The selection
  sounds of SelectionXlat.cpp are the `MSG_CREATE_SELECTED_GROUP` messages it sends (its own `m_noCanDoSound` plays are in
  debug builds only). `Object::defect`'s `VoiceDefect` is played from the object's template (`client_audio`).
- **Reads, all without side effects** (`UnitVoiceHost`, answered by `WorldClient`): the template's sounds; the AI's
  `isMoving` (the module's own answer, as the control bar reads it) and `isWaitingForPath` (`World::ai_peek`);
  `canCrushOrSquish` (`&self`); the weapon set's weapons and the store's templates; `isUsingAirborneLocomotor`; the
  player's completed upgrades (the worker shoes found by name, no name key made); the special power module's
  `getInitiateSound()` (a new `ModuleCall::GetSpecialPowerInitiateSound`, a getter of the module data); the logic's copy of
  the audio descriptions for `isValidAudioEvent`. Upstream's `setPlayingAudioIndex` on the template's `const` event (so
  the next voice is another file) is kept by the client, by template and sound (`ClientAudio::voice_indices`). The voices
  go to `WorldClient::voices`; the application plays them with the client's audio random.
- **Translated, ambient sounds** (`client::drawable_ambient`, new): Drawable.cpp's `startAmbientSound` (both),
  `stopAmbientSound`, `enableAmbientSound`, `enableAmbientSoundFromScript`, `getAmbientSoundByDamage` (`SoundAmbient`,
  `SoundAmbientDamaged`, `SoundAmbientReallyDamaged`, `SoundAmbientRubble`, the pristine one standing in for a missing
  damaged one), the custom sound (`setCustomSoundAmbientOff`/`Info`, `clearCustomSoundAmbient`, `mangleCustomAudioName`,
  `getNoSoundMarker`), `onLevelStart`, `update`'s restart of a sound that loops for ever, and the constructor's and
  `reactToBodyDamageStateChange`'s starts (not while the map loads). The map's `DynamicAudioEventInfo`: the logic already
  sent the object's `objectSoundAmbient*` properties (`DrawableAmbientSoundProperties`) and had the property block
  translated (`apply_ambient_sound_properties`); the client now runs it on the drawable, copying the description with
  its overrides under `" CUSTOM <id> <name>"` (`addAudioEventInfo`). The state is the client audio's, by object id.
- **Cuts made client events:** `doEnableObjectSound` (`sa_drawable_enable_ambient_sound`) and the AI's
  `drawable_enable_ambient_sound` / `drawable_stop_ambient_sound` (the jets' engine sound) push
  `ObjectClientEvent::DrawableEnableAmbientSoundFromScript` / `DrawableEnableAmbientSound` / `DrawableStopAmbientSound`,
  as 6320292 did for the scripts' audio; `startNewGame`'s `setLoadingMap` and `onLevelStart` loop also push
  `ClientEvent::LoadingMap` and `DrawablesOnLevelStart` (their cut records are kept). None is saved or in the CRC.
- **`map.ini` audio:** upstream's `loadMapINI` reads `map.ini` with `INI::load`, whose `theTypeTable` holds the audio's
  `AudioEvent`, `DialogEvent` and `MusicTrack` rows. The logic's block table (`game_data`'s `BLOCKS`, used by I33's
  `load_ini_overrides`) now has them: the descriptions go to `GameStatics::audio` (so #50's logical draws follow a
  redefined event) and are listed (`AudioTables::logic_ini_events`); the client copies each listed description into its
  own tables (`ClientAudio::sync_logic_ini_audio`), so every machine has the same ones. Before this, an audio block in a
  `map.ini` stopped the load there with `INI_UNKNOWN_TOKEN`.
- **Defect fixed on the way:** `CommandTranslator::observe_message` left `m_teamExists` as it was for `MSG_SELECT_TEAMn`;
  upstream's case sets it (written up for an issue).
- **Tests (predictions in each doc comment, written before the runs):** `the_unit_voices_questions_leave_the_crcs_alone`
  (every voice case asked 21,600 times over 300 frames of #33's world: CRCs equal, nothing sent, both special power
  modules' `InitiateSound` found, the gunner's weapon read): held. `ambient_sounds_follow_the_drawable`: held after one
  prediction I corrected while reading the test over, before the run (`onLevelStart` visits every drawable).
  `a_maps_ini_redefines_audio_for_the_logic_and_the_client` (`Boom` redefined keeps its `Limit` with no
  `DefaultSoundEffect`, as upstream; `MapSong` a track; the client synced the same): held. Client:
  `voiced_units_selected_and_ordered_leave_the_crcs_alone` (two voiced soldiers selected by a click and a drag, ordered
  to move and attack; with the audio and every voice case asked each frame): CRCs equal over 90 frames, the same six
  messages, `SoldierSelect` twice (files `sel1`, `sel2`: the kept index 1), `SoldierMove`, `SoldierAttack`, three
  looping hums still playing. One miss, in the test: the hums were not in `ClientAudio`'s log (the ambient sounds go to
  the manager directly); the assertion reads the drawables' sounds instead.
- **Gates:** lib 1002 (999 + 3), client 35 (34 + 1), clippy clean, web build clean. Gate (a): *predicted* unchanged (the
  gate game runs no translator and no `startNewGame`); *result* PASS, 300 frames, last CRC `0bf44067`.
- **Not done** (written up for issues): GUICommandTranslator.cpp is not translated, so its five voices (guard at an
  object and a position, attack move, fire weapon at a position, evacuate at a position) and ControlBarCallback.cpp's two
  are not played; ~~EVA;~~ **Corrected 2026-11-06 (#55):** EVA is played (see #56/#55's entry); the ambient sound's `setTimeOfDay` restart and its save-game state; the lengths (#52) of events a
  `map.ini` redefines are the install's; `map.ini`'s `AudioSettings`/`MiscAudio` blocks are not read; no check with a
  real install.

**#51 (2026-11-06, GitHub #51): audio follow-ups, part 1 (the scripts' audio).**
- **Translated:** the `TheAudio` calls of ScriptActions.cpp's `doPlaySoundEffect`, `doPlaySoundEffectAt`,
  `doSoundPlayFromNamed`, `doSpeechPlay`, `doMusicTrackChange`, `doUnitSelectAndPlay`, `doAudioSetVolume` (music, sound,
  speech), `doAmbientSoundsPause`, the `SOUND_*`/`AUDIO_*` enable, remove and volume-override actions,
  `SUSPEND_BACKGROUND_SOUNDS`/`RESUME_BACKGROUND_SOUNDS`, and `ScriptEngine::loadPostProcess`'s current track. The
  logic's `sa_audio_*` (and `se_audio_add_event`) were cut points; each now pushes a
  `ClientEvent::ScriptAudio(ScriptAudioEvent)`, and `client_audio::ClientAudio::apply_script_event` makes upstream's call
  on the manager. `pauseAmbient` does nothing (`MilesAudioManager::pauseAmbient` is empty upstream).
- **No logic state changes:** the event is pushed on the client queue (not saved, not in the CRC), takes none of the
  logic's made-up audio handles (`cuts.last_audio_handle`, which modules keep and save), and makes no logic random draw:
  ~~a logical event's file is picked from the client's own copy of the logic stream, as before (#50 is open).~~
  **Corrected 2026-11-06 (#50):** a logical event's `GameLogicRandomValue` draws are now the logic's (see #50's entry
  below). The
  engine's saved `m_currentTrackName` is set as before. Gate (a): last CRC `0bf44067`, unchanged.
- **Not done:** ~~`getAudioLengthMS` is still 0 to the logic (the script engine's `isSpeechComplete`/`isAudioComplete`
  read `cuts.audio_lengths_ms`, which nothing fills): answering it from the device's files changes when script waits end,
  so it is a question for the owner.~~ **Corrected 2026-11-06 (#52):** the owner chose option 2; the logic reads each
  event's length from the files at load (see #52's entry). Music next/previous are not script actions upstream (CommandXlat's demo keys
  `MSG_META_DEMO_MUSIC_NEXT_TRACK`/`PREV_TRACK`); the manager has them, nothing calls them. ~~Unit voices, selection
  sounds, `VoiceDefect`, EVA, the drawables' ambient sounds (with `doEnableObjectSound`, still a cut) and the map's
  customised ambient sounds remain (parts 2 and 3).~~ **Corrected 2026-11-06 (#51):** all of these but EVA are done (see
  the entry for parts 2 and 3 below).

**#50 (2026-11-06, GitHub #50): the logic makes a logical audio event's random draws (the owner's option (a)).**
- **Translated:** the logic half of `AudioManager::addAudioEvent` (GameAudio.cpp:420-457: the empty/`NoSound` name and
  no-description returns, the category and `getDisallowSpeech` returns, then `generateFilename` and `generatePlayInfo`)
  and the `m_isLogicalAudio` branches of `AudioEventRTS::generateFilename` and `generatePlayInfo` (AudioEventRTS.cpp:
  318-424), as `logic::logical_audio::logical_audio_draws`. Only the four script actions that call
  `setIsLogicalAudio(true)` (ScriptActions.cpp:356, 373, 2756, 2770) reach it, through `World::sa_audio_add_event`.
- **Where and when the logic draws:** in the script action, at the `TheAudio->addAudioEvent` call, from `World::random`
  (`theGameLogicSeed`), in upstream's order: one `GameLogicRandomValue(0, Sounds - 1)` for a sound effect with a
  non-empty `Sounds` and `Control = RANDOM` (**also for a single file**: `GetGameLogicRandomValue(0, 0)` advances the
  state, since `delta` is 1); one `(0, Attack - 1)` if `Attack` is non-empty; one `(0, Decay - 1)` if `Decay` is
  non-empty (both whatever the type, even with no `Sounds`). Music and speech pick their `Filename` without a draw.
- **The values reach the client** in `ScriptAudioEvent::AddEvent(event, LogicalAudioPicks)`; the client's
  `AudioEventRts` takes each value in place of its own draw (`logic_picks`), so it plays the files every machine's
  logic picked and never reads a logic stream. Delay, pitch and volume stay the audio stream's.
- **The descriptions reach the logic** as `GameStatics::audio` (`client::audio_info::AudioTables`, the same crate, no
  new dependency): `load_game_data` reads `AudioManager::init`'s files (`AUDIO_INI_FILES`) with the audio's own loader,
  with no user preferences (they set volumes only). A missing or bad audio file is skipped, as the client's loader
  skips it (upstream's `INI::load` would throw); every machine reads the same files.
- **`m_playingAudioIndex`:** each of the four actions' `AudioEventRTS` is a local, constructed with -1, and
  `setPlayingAudioIndex` copies the pick back into that local only; nothing persists between calls, so the "not the
  one played last" bump (`which == m_playingAudioIndex && size > 2`) never fires for a script's sound. The bump is not
  a draw, so the logic sends the raw value and the client's event applies upstream's bump to it.
- **Upstream desyncs with audio off.** `isOn` is each player's own (`-noaudio` sets `m_audioOn` false, and
  `GameEngine::init` turns every category off with it; the options menu's sound/speech switches; the debug
  `MSG_META_DEMO_TOGGLE_SOUND`). A player with sound effects off returns from `addAudioEvent` before
  `generateFilename`, so makes none of these logic draws; the first random script sound then leaves that player's
  logic stream behind everyone else's, and the game desyncs. The same holds for speech with an `Attack`/`Decay` list,
  and while uninterruptable speech plays (`getDisallowSpeech()`, set by the device when the stream *opens* and cleared
  when it ends: device- and wall-clock-dependent). **This engine draws as an audio-on upstream with no
  uninterruptable speech playing, on every machine**, whatever the player's settings.
- **Tests (predictions in each doc comment):** `logical_audio`'s order-and-count test (8 names: 1 draw for 4 files, 1
  for 1 `RANDOM` file, 3 with attack and decay, 1 decay-only with no `Sounds`, 1 for a dialog with `Attack`, 0 for the
  rest); `a_scripts_logical_sound_draws_upstreams_logic_values_and_the_client_plays_them` (`PLAY_SOUND_EFFECT` of a
  5-file random sound with 3 attacks: exactly 2 logic draws, sound 2 and attack 1; the client started `a1.wav` and
  holds `r2.wav`, its logic state untouched; a single non-random file: none; a single random file: 1). One miss, in the
  test's host (it said every file exists, so the localized file was preferred), not in the logic.
- **Differential** (`tests/differential_audio.rs`, `audio_events_match_upstream`): logical events now run as
  `addAudioEvent` runs them, the Rust logic drawing first and the client event playing the values, against the compiled
  upstream drawing from its own logic stream. *Predicted:* about 3,500 logical events and 2,300 logic draws in 4,000
  cases, no mismatch. *Result:* 3,425 logical events, 2,274 logic draws; 13,949 per-call states and 4,000 generator
  states compared, 0 mismatches. A check that it can fail: skipping the attack draw gave 480 mismatches.
- **Gate (a):** the gate game plays no script sound and loads no audio INI. *Predicted:* unchanged. *Result:* PASS, 300
  frames, last CRC `0bf44067`, unchanged. Lib 994 (992 + 2), client 33, clippy clean, web build clean.
- **Not done:** ~~a map's `map.ini` can redefine `AudioEvent`s upstream (`INI::load` with `theTypeTable`); the map loader
  does not read audio blocks, so the logic's and the client's descriptions are the install's.~~ **Corrected 2026-11-06
  (#51):** the logic's INI loads read `AudioEvent`, `DialogEvent` and `MusicTrack` blocks into `GameStatics::audio`, and
  the client applies them (see #51's parts 2 and 3).

**#19 (2026-11-06, GitHub #19): animations sampled with the shipped game's `Float_To_Long`.**
- **The decision** (the owner's option 2): the client's W3D animation evaluation uses the Win32 branch of
  `WWMath::Float_To_Long` (wwmath.h: `fld [f]` / `fistp [i]`), which the shipped game ran under the round-to-nearest
  control word its `setFPMode` (GameLogic.cpp) sets after `_fpreset` (`_RC_NEAR`; `_RC_CHOP` is commented out there).
  The portable truncation `(long) f` stays available as a named choice. The logic never calls either: nothing under
  `src/logic` or `src/common` reaches `formats::w3d` or the conversion, and gate (a) keeps its CRCs.
- **Translated:** `ww::wwmath::float_to_long_fistp` (round to nearest, ties to even; NaN, the infinities and anything
  rounding outside `i32` give the x87 integer indefinite `i32::MIN`, the masked invalid-operation result, so defined)
  and `FloatToLongMode { Win32Fistp, PortableTruncate }`. Its call sites are `hrawanim.cpp`'s three
  `Float_To_Long(frame - 0.499999f)` (`HRawAnim::frames`) and the raw fast path of `HTreeClass::Anim_Update`
  (`anim_update_raw`), the only ones in the translated W3D code. A raw animation carries its branch
  (`HRawAnim::frame_rounding`, `with_frame_rounding`; `HAnim::frame_rounding`, `set_frame_rounding`), `Win32Fistp`
  after loading, so the model viewer (`ModelScene`) and the drawables, which both get their animations from the asset
  manager's loader, sample as the game did. `Get_Visibility`'s `(int)frame` is a C cast and still truncates.
- **The differential** (`tests/differential_w3d.rs`, section 13): `build.rs` now compiles `htree.cpp` and `hrawanim.cpp`
  as copies whose `Float_To_Long` calls (1 and 3, asserted) go through `Ref_W3D_Float_To_Long` in `wrap_w3d.cpp`, which
  is either `WWMath::Float_To_Long` itself or the `flds`/`fistpl` pair, chosen at run time. Each Rust branch runs
  against its C++ counterpart on the same 6,000 files and scripts. Portable: 66,396 of 121,232 ops compared, 0
  mismatches, max ULP 0 (as recorded for #10). Win32: 67,413 compared, **0 mismatches, max ULP 0**; `FloatToInt`
  stops fall from 410 to 247 (none left in the raw fast path). The conversion alone, against a real x87 `fistp`
  (every tie `k + 0.5` for `|k| <= 4096` with its neighbours, values around +-2^31, the specials and 2^20 random bit
  patterns): 1,081,413 compared, 0 mismatches.
- **The earlier crash** (#10's planted bug: with rounding, the C++ crashed where the Rust went on), investigated. It
  does not recur with the branches matched, and no crossed run crashed here either (2,244 and 2,193 mismatching files).
  It was a comparison of two branches, not a defect of either: the ops a crossed run sends the C++ undefined are 147
  NaN or out-of-range raw frames (defined under `fistp`) and 28 frame-dependent undefined reads and writes of
  `motchan.h` (an overlong channel vector written past its `Vector3` or `Matrix3D`, a 3-float quaternion read as 4)
  that occur only at frames inside the channel, so a different frame moves them. Not a negative index: rounding's
  frame -1 and `i32::MIN` fall below every `FirstFrame` and give the identity in both languages. Nothing was changed in
  behaviour; the exclusions are counted in the test's printed line.
- *Predictions before the runs* (in each test's doc), and the results: frame 2.3 samples keys 2 and 3 at ratio
  0.29999995 under Win32 (X = 28.0) and keys 1 and 2 at 1.2999999 under the portable branch (X = 49.0, an
  extrapolation): held. The fast path shows frame 2 at 1.5 and 2.5 (ties to even), 0 at 2.6 (past the end) under
  Win32, 1, 2 and 2 under truncation: held. On a synthetic 30-key sway of amplitude 10, the interpolated pose differs by
  at most 0.212 predicted, **0.2105 measured** (at frame 8.48); the fast path by one key step, 2.09 predicted,
  **2.0791 measured: a miss** (the largest step over integer keys is at `cos(pi/30)`, not 1). The demo robot's arm at
  frame 1.25: yaw 33.75 degrees under Win32, 56.25 under truncation: held. The differential's 0 mismatches: held.
- **Gates (2026-11-06):** lib tests 943 pass; differential W3D 27 pass (25 plus the Win32 run and the `fistp` check);
  client tests 23 pass; clippy clean; web build OK; gate (a) identical over 300 frames native and wasm (last CRC
  `0bf44067`).
- **Not done:** the other four Win32 branches of `wwmath.h` (`Sin`, `Cos`, `Sqrt`, `Inv_Sqrt` on the x87) are still the
  portable ones in the client; a real Zero Hour animation was not compared (no game files here).

**#12 (2026-11-05, GitHub #12): the control bar from the game's own files.**
- **Translated** (engine crate, plain Rust):
  - `client::image` (Image.cpp, INIMappedImage.cpp): `MappedImage` blocks (`Texture`, `TextureWidth`/`Height`,
    `parseImageCoords`, `parseImageStatus` with the rotated swap), `ImageCollection::addImage`, `findImageByName`, `load(512)`.
  - `client::window_script` (GameWindowManagerScript.cpp, with `winCreate`'s region, `WinInstanceData::init` and
    `RAMFile`'s reads): every function of the reader, `winCreateFromScript` to `parseScreenRect`, the 46 field parsers,
    `getDataTemplate`, the obsolete `parseData`, `setWindowText`, `createGadget`, `createWindow`, `parseChildWindows`,
    the layout block. The static 2048-byte buffer and `strtok`'s state are modelled byte for byte, so a field without a
    quote takes a stale one as upstream does; the gadgets' part draw data are the file's globals, never reset.
  - `client::control_bar_scheme` (ControlBarScheme.cpp, INIControlBarScheme.cpp): the scheme INI (image and animating
    parts, the named images, corners and colours), `newControlBarScheme`, `findControlBarScheme`,
    `setControlBarSchemeByPlayerTemplate`, `ControlBarScheme::init` (the fixed windows placed and given their images),
    `drawBackground`/`drawForeground` as draw lists.
  - `client::control_bar_command` (ControlBarCommand.cpp, `setControlCommand`): `populateCommand` (script-only and
    exit-container buttons, the special power science rule and the purchase-science images), `getCommandAvailability`
    in full (hidden, restricted, not ready with its clock, can't afford, active), the command windows' part of
    `updateContextCommand`, `populateBuildQueue` and the first entry's clock. Every World question goes through
    `ControlBarStateHost`; `client::world_host::WorldClient` answers it from the World, read only. Not answered from
    the World yet, and answered as noted in the code: a unit moving (`MUST_BE_STOPPED`), an upgrade in production, the
    dozer's pending build task, a weapon's status, rappellers, a contain's count, a dock, a special power's readiness,
    overcharge, the selected weapons, hacking, battle plans and the purchase-science sets.
- **New** (client, `client/src/ui.rs` and `app.rs`): the control bar is read from the art's files when a game starts
  (`Window\ControlBar.wnd` at its 800x600 creation size, the side's scheme), and drawn with Bevy UI nodes in a box of the
  layout's proportions along the bottom, as wide as the screen allows (the whole width on a phone): the scheme's
  background, the windows with `WIN_STATUS_IMAGE` as their state's first image (the mapped image's rectangle of its
  texture page), the others as their colour, text centred; the selected object's command buttons with their images
  (their CSF label where the image is missing), dimmed when disabled, a clock over a charging button, a click through
  `processCommandUI`; the money as `GUI:ControlBarMoneyDisplay` formats it; a power bar (new drawing). Without the
  game's files the debug panel stays; on a screen under 700 pixels it also stays, above the bar, for its finger-sized
  buttons, now with the buttons' CSF labels. On the web `WindowZH.big` is no longer skipped and is read by range.
- *Results:* 6 engine and 4 client tests (predictions in their docs; two client tests run only with their paths: the UI install writer and the owner's copy, `RTS_ZH_DIR`, not run here). Two predictions missed and were corrected: the
  parent window's id (the layout block's names take keys 1 and 2 before it) and a miscount of a synthetic layout's
  windows; the UI install test's "waits once for the page's ranges" did not wait (the synthetic archives are smaller
  than the page's 64 KiB prefix). **Differential** (`tests/differential_wnd.rs`, GameWindowManagerScript.cpp cut verbatim
  by `build.rs`, each case in a forked child): first run 23 mismatches, all the C++ crashing where the Rust went on:
  `DATA` on a tab control or combo box leaves the gadget data NULL, which the gadget dereferences (undefined, now
  reported); then 3,000 synthetic cases: 1,782 files read and 6,012 windows compared in 1,072 identical cases, 1,425
  stops agreed, 503 undefined cases not compared, **0 mismatches**. One planted bug (the `STATUS`/`STYLE` names compared
  with case) was caught: 839 of the 1,072 cases then differed. The CRCs: the control
  bar's questions for every player-1 object of the gate game (16,500 command windows over 300 frames) and the demo's
  bar for 60 frames leave the logic's CRCs equal to runs without them. Seen natively (Xvfb) and in headless Chromium on
  a synthetic install, at 1000x700 and 390x844 (no horizontal overflow).

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
modes, from the control bar: C2c), ~~unit voices~~ (**corrected 2026-11-06, #51:** played since #51) and the stats
collector (no messages), and the double-click guard order
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
