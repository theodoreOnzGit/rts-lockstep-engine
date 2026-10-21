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
| `common::discrete_circle` | `Include/Common/DiscreteCircle.h`, `Source/Common/DiscreteCircle.cpp`: the Bresenham scanlines of partition-grid circles (Phase 1 layer 1) |
| `common::ascii_string` | `Include/Common/AsciiString.h`, `Source/Common/System/AsciiString.cpp`: the engine string's exact semantics (C-string content, buffer or none, the `MAX_LEN` limit, ASCII-only case folding, `trim`, `toLower`, `nextToken`, `translate`); not `format` (Phase 1 layer 2) |
| `common::name_key_generator`, `common::well_known_keys` | `Include/Common/NameKeyGenerator.h`, `Source/Common/NameKeyGenerator.cpp` (the socket/chain structure, so keys come out in upstream's order, including its case-folding quirk), `StaticNameKey`, and the 127 keys of `Include/Common/WellKnownKeys.h` (Phase 1 layer 2) |
| `common::bit_flags`, `common::kind_of`, `common::object_status_types`, `common::disabled_types`, `common::model_state`, `common::special_power_mask_type`, `logic::armor_set`, `logic::weapon_set_flags` | `Include/Common/BitFlags.h`, `BitFlagsIO.h` (operations, names, INI parsing, xfer; the CRC branch hashes 4 bytes as the shipped Win32 build did) and its seven instantiations with their enums and name lists: `KindOf.h`/`.cpp`, `ObjectStatusTypes.h`/`.cpp`, `DisabledTypes.h`/`.cpp`, `ModelState.h` and `BitFlags.cpp`, `SpecialPowerMaskType.h` (names of `SpecialPower.cpp`), `GameLogic/ArmorSet.h` (the flags), `WeaponSetFlags.h` (names of `WeaponSet.cpp`) (Phase 1 layer 3) |
| `common::sparse_match_finder` | `Include/Common/SparseMatchFinder.h`: best-match lookup of armor/weapon/model entries by condition flags, with upstream's scan order and tie-break (Phase 1 layer 3) |
| `common::partition_solver` | `Include/Common/PartitionSolver.h`, `Source/Common/PartitionSolver.cpp`: first-fit-decreasing packer; equal sizes use a documented stable order, not STLport's `std::sort` (Phase 1 layer 1) |
| `common::dict` | `Include/Common/Dict.h`, `Source/Common/Dict.cpp`: the sorted key/value pairs with upstream's 24-bit key packing, Shell sort, binary search and rename-and-drop `remove` (Phase 1 layer 3) |
| `common::geometry` | `Include/Common/Geometry.h`, `Source/Common/System/Geometry.cpp`: `GeometryInfo` (sphere, cylinder, box): footprint and bounding-radius arithmetic, the five `Geometry*` INI fields (`GEOMETRY_FIELDS`), its xfer (Phase 1 layer 4) |
| `common::bezier_segment` | `Include/Common/BezierSegment.h`, `BezFwdIterator.h`, `Source/Common/Bezier/BezierSegment.cpp`, `BezFwdIterator.cpp`: cubic Bezier evaluation, subdivision, approximate length and forward-differenced sampling; the D3DX matrix calls as written-out formulas (Phase 1 layer 4) |
| `common::money` | `Include/Common/Money.h`, `Source/Common/RTS/Money.cpp`: the balance, withdraw and deposit, `parseMoneyAmount` and the xfer; the cash sounds are `logic::client_event::ClientEvent`s (Phase 1 layer 4) |
| `common::handicap` | `Include/Common/Handicap.h`, `Source/Common/RTS/Handicap.cpp`: the build-cost and build-time multipliers, read from `HANDICAP_*` dictionary keys (Phase 1 layer 4) |
| `common::random_value` | `GameEngine/Source/Common/RandomValue.cpp` and the Logic/Client/Audio RandomValue headers |
| `common::crc` | `GameEngine/Source/Common/crc.cpp`, `GameEngine/Include/Common/crc.h` |
| `common::base_type` | the fast float helpers, `REAL_TO_*` macros and `PI` of `Libraries/Include/Lib/BaseType.h` |
| `common::trig` | `GameEngine/Source/Common/System/Trig.cpp`, `Libraries/Include/Lib/trig.h`: the shipped `sinf`-based `Sin`/`Cos`/`Tan`/`ACos`/`ASin`, and the unused integer-table branch as `trig::table` |
| `common::quick_trig` | `GameEngine/Include/Common/QuickTrig.h`, `GameEngine/Source/Common/System/QuickTrig.cpp` |
| `common::xfer`, `xfer_crc`, `xfer_save`, `xfer_load` | `GameEngine/Source/Common/System/Xfer.cpp`, `XferCRC.cpp` (with `XferDeepCRC`), `XferSave.cpp`, `XferLoad.cpp` and their headers, `Snapshot.h`: the snapshot/version mechanism, blocks, typed xfers, the lockstep CRC; save/load on in-memory buffers, and (Phase 1) `xferKindOf`; not `xferMapName`, `xferScienceType`/`Vec`, `xferUpgradeMask`, which need engine name tables |
| `common::unicode_string` | `Include/Common/UnicodeString.h`, `Source/Common/System/UnicodeString.cpp`: the 16-bit string with `AsciiString`'s semantics (buffer or none, the 16,380-unit limit, `wcscmp`/`_wcsicmp` ordering, `trim` on `iswspace`, `nextToken`, `translate` sign-extending `char`); not `format` or `getBufferForRead` (Phase 1 layer 2) <!-- // --- p1w1-unicode --- -->|
| `common::ini` | `GameEngine/Source/Common/INI/INI.cpp`, `Include/Common/INI.h`: line reading, `strtok` tokenising, block dispatch, the field-parse tables (`initFromINI`, `initFromINIMulti`, catch-all entries), `loadDirectory`, the INI `XferCRC`, and the generic field parsers (integers, reals, bools, strings, string lists, percentages, angles, durations, velocities, colours, coordinates, bit strings, index and lookup lists, veterancy/death/damage flags, random variables). Not the 62 block handlers of `theTypeTable` (all build game objects not yet ported; listed in `ini::TYPE_TABLE`), nor the field parsers that look names up in game systems (`parseAndTranslateLabel`, `parseMappedImage`, `parseAnim2DTemplate`, the two audio-event parsers, `parseThingTemplate`, `parseArmorTemplate`, `parseWeaponTemplate`, `parseFXList`, `parseParticleSystemTemplate`, `parseDamageFX`, `parseObjectCreationList`, `parseUpgradeTemplate`, `parseSpecialPowerTemplate`, `parseScience`, `parseScienceVector`) |
| `common::game_common`, `logic::damage` | the frame-rate conversions and veterancy flags of `Include/Common/GameCommon.h` (+ `GameCommon.cpp`), and the damage/death type flags of `Include/GameLogic/Damage.h` (+ `Damage.cpp`), which the INI flag parsers use |
| `formats::big` | `.big` archives: `GameEngineDevice/Source/Win32Device/Common/Win32BIGFileSystem.cpp` (the parser), `Win32BIGFile.cpp`, `GameEngine/Source/Common/System/ArchiveFile.cpp`, `ArchiveFileSystem.cpp`, on in-memory byte buffers |
| `formats::csf` | `.csf` string files: the CSF path of `GameEngine/Source/GameClient/GameText.cpp` (`getCSFInfo`, `parseCSF`, `stripSpaces`, the sorted label table, `fetch`, `getStringsWithLabelPrefix`), with the file read through `RAMFile::read`'s exact semantics; not the `.str` text format nor map string files |
| `formats::data_chunk` | the chunk format of maps: the input side of `GameEngine/Source/Common/System/DataChunk.cpp` (`CachedFileInputStream` reads, `DataChunkTableOfContents`, `DataChunkInput`: registered parsers, nested `parse`, the chunk stack, the typed reads incl. `readDict` and `readNameKey`), on uncompressed in-memory bytes; names stand in for `NameKeyType`s. ~~Not the writer (`DataChunkOutput`)~~ **CORRECTED 2026-10-18**: the writer is `formats::data_chunk_output`. Not decompression (`CompressionManager`) |
| `ww::wwmath` | `Libraries/Source/WWVegas/WWMath/wwmath.h` (+ table init of `wwmath.cpp`) |
| `ww::vector2`, `ww::vector3`, `ww::vector4` | `WWMath/vector2.h`, `vector3.h`, `vector4.h` (the subset `Matrix3D` uses) |
| `ww::matrix3d` | `WWMath/matrix3d.h`, `matrix3d.cpp` (not `Get_Inverse`, the `Matrix3x3`/`Quaternion` setters, `Lerp`) |
| `ww::quat` | `WWMath/quat.h`, `quat.cpp`: the `Quaternion` data and `Build_Matrix3D` only |
| `ww::chunkio` | `WWLib/chunkio.h`, `chunkio.cpp`: `ChunkLoadClass` (chunks, micro-chunks, reads), with the file read through `GameFileClass` over `RAMFile` (the game's path for W3D files in `.big` archives); not the writer (`ChunkSaveClass`) |
| `ww::realcrc` | `WWLib/realcrc.h`, `realcrc.cpp`: `CRC_Memory` |
| `formats::w3d` | W3D models, parsed into plain structs (no rendering): `WW3D2/w3d_file.h` and `w3d_obsolete.h` (all chunk ids, flags and the 84 file structs); hierarchies (`htree.cpp`, `pivot.cpp`); animations (`hrawanim.cpp`, `hcanim.cpp`, `motchan.cpp`: raw, time-coded, adaptive-delta and bit channels); meshes (`MeshModelClass::Load_W3D` of `meshmdlio.cpp`, with the parts of `meshgeometry.cpp`, `meshmatdesc.cpp`, `vertmaterial.cpp`, `texture.cpp`, `aabtree.cpp`, `w3d_util.cpp`, `shader.h` and `dx8wrapper.h` it calls). Not the legacy material chunks (`MATERIALS3`, multi-material per-triangle materials), the processing after the chunks are read (`install_materials`, `post_process`, generated AABTrees), nor the asset manager's file loop (`Load_3D_Assets`) |
| `formats::texture` | textures into raw pixel buffers: Targa images as `WWLib/TARGA.CPP` reads them (`Open`, both `Load`s, run-length decoding, `InvertImage`, `XFlip`, `YFlip` with its byte-tail bug, palettes, Targa 2.0 footer and extension); DDS files as `WW3D2/ddsfile.cpp` reads them (header, level table, payload, `Get_Pixel`, `Get_4x4_Block`, the level and cube-map copies to every surface format, with upstream's DXT1 and DXT5 decompression; upstream has no DXT2-4 decoder); the formats of `ww3dformat.cpp`/`formconv.cpp`, `Write_B8G8R8A8` of `bitmaphandler.h` and `Recolor` of `colorspace.h`. Not Targa writing (`Save`, `EncodeImage`), nor the Direct3D surface overloads |
| `common::latch_restore` | `Include/Common/LatchRestore.h`: the scope guard that latches a variable and restores it when dropped, on every exit from the scope (Phase 1 layer 1) |
| `common::overridable` | `Include/Common/Overridable.h`, `Include/Common/Override.h`: the INI override chains of templates and `OVERRIDE<T>`, which reads the final override (Phase 1 layer 1) |
| `common::stl_typedefs` | `Include/Common/STLTypedefs.h`: the `AsciiString` comparators `less_than_nocase` and `equal_to`, and the `NoCaseAsciiKey` map key; not the `hash` functors, since no `hash_map` order is kept (Phase 1 layer 2) |
| `common::subsystem_interface` | `Include/Common/SubsystemInterface.h`, `Source/Common/System/SubsystemInterface.cpp`: the subsystem trait and its registry (init and post-process load in order, reset and shutdown in reverse), with no global list and without the `DUMP_PERF_STATS` timing (Phase 1 layer 2) |
<!-- --- p1w1-small --- -->
| `logic::armor` | `Include/GameLogic/Armor.h`, `Source/GameLogic/Object/Armor.cpp`: `ArmorTemplate` (38 damage coefficients, `adjustDamage` with the unresistable pass-through and the floor at 0), `Armor` (a handle by name key), `ArmorStore` (an ordered map where upstream has a `hash_map`) and the `Armor` INI block (Phase 1 layer 4). Not `INI::parseArmorTemplate` (the `Armor =` field of thing templates) <!-- --- p1w1-armor --- --> |
| `common::damage_fx` | `Include/Common/DamageFX.h`, `Source/Common/DamageFX.cpp`, `Source/Common/INI/INIDamageFX.cpp`: `DamageFX` (per damage type and veterancy level: major threshold, major and minor effect, throttle time), `DamageFXStore` and the `DamageFX` INI block. `FXList`s stay as names, never resolved (a cut point). Left out: `doDamageFX` and the `Object` forms of the getters (the Rust takes the veterancy level), `INI::parseDamageFX` (Phase 1 layer 4) |
| `logic::damage` (rest) | `DamageType` and its constants, `IsSubdualDamage`, `IsHealthDamagingDamage`, `DamageInfoInput`/`DamageInfoOutput`/`DamageInfo` with their xfer (the source template is kept as a name, a cut point), `DamageTypeFlags` as the generic `BitFlags` (Phase 1 layer 4) <!-- --- end p1w1-armor --- --> |
| `common::terrain_types` | `Include/Common/TerrainTypes.h`, `Source/Common/TerrainTypes.cpp`, `Source/Common/INI/INITerrain.cpp`: `TerrainType` (texture, class, blend edges, restrict construction), `TerrainTypeCollection` (in insertion order; upstream's list is newest first) and the `Terrain` INI block. Texture names stay names (a client cut point). No method takes an `Object` or a `Player` (Phase 1 layer 4) <!-- --- p1w2-terrain --- --> |
| `common::multiplayer_settings` | `Include/Common/MultiplayerSettings.h`, `Source/Common/MultiplayerSettings.cpp`, `Source/Common/INI/INIMultiplayer.cpp`: `MultiplayerSettings` (the six defaults, the colour list by key, the observer and random colours, the starting-money choices), `MultiplayerColorDefinition` and the `MultiplayerSettings`, `MultiplayerColor` and `MultiplayerStartingMoneyChoice` INI blocks. `m_initialCreditsMin`/`Max` are declared upstream and never used, so not translated (Phase 1 layer 4) |
| `logic::crate_system` | `Include/GameLogic/CrateSystem.h`, `Source/GameLogic/System/CrateSystem.cpp`, `Source/Common/INI/INICrate.cpp`: `CrateTemplate` (the `CrateData` fields; the killer science as its name key, whose validity check waits for the Science store), `CrateSystem` (`Overridable` override chains, `reset()` removes the overrides) and the `CrateData` INI block. No method takes an `Object` or a `Player` (Phase 1 layer 4) <!-- --- end p1w2-terrain --- --> |
<!-- --- p1w2-globaldata --- -->
| `common::global_data`, `logic::weapon_bonus` | `Include/Common/GlobalData.h`, `Source/Common/GlobalData.cpp`, `Source/Common/INI/INIGameData.cpp`: `GlobalData` (`TheGlobalData`), all 280 members of the retail configuration (half are client settings, kept as plain data; the 90 that game logic names are marked), its constructor's defaults, `setTimeOfDay`, the 336-entry `GameData` parse table (generated from the upstream header and table by `scripts/gen_global_data.py`), the `GameData` block with `INI_LOAD_CREATE_OVERRIDES`/`INI_LOAD_MULTIFILE`, and the `m_next` chain of overrides (`GlobalDataStore`); and `WeaponBonus`/`WeaponBonusSet` of `Weapon.h`/`Weapon.cpp` (the `WeaponBonus =` field). Not the executable CRC, user data directory, double-click time or user preferences (filesystem, registry, devices), nor the `_DEBUG`/`_INTERNAL`/`ALLOW_SURRENDER` members. Upstream's `newOverride` copies nothing (a no-op `operator=`): kept (Phase 1 layer 4) |
<!-- --- end p1w2-globaldata --- -->
<!-- --- p1w2-science --- -->
| `common::science` | `Include/Common/Science.h`, `Source/Common/RTS/Science.cpp`, and from `INI.cpp`/`Xfer.cpp` `parseScience`, `parseScienceVector`, `scanScience`, `parseAndTranslateLabel`, `xferScienceType`, `xferScienceVec`: `ScienceType` (a name key), `ScienceInfo`, `ScienceStore` with override chains, runtime root sciences, lookups, the `Science` INI block. Game text is a `GameTextFetch` argument. Not the three methods that take a `Player` (`playerHasPrereqsForScience`, `playerHasRootPrereqsForScience`, `getPurchasableSciences`; layer 7) (Phase 1 layer 4) |
| `logic::rank_info` | `Include/GameLogic/RankInfo.h`, `Source/GameLogic/System/RankInfo.cpp`: `RankInfo`, `RankInfoStore` (levels 1..n, overrides of existing levels only) and the `Rank` INI block (Phase 1 layer 4) |
<!-- --- end p1w2-science --- -->
<!-- --- p1w2-upgrade --- -->
| `common::upgrade` | `Include/Common/Upgrade.h`, `Source/Common/System/Upgrade.cpp`, `Source/Common/INI/INIUpgrade.cpp`, `Xfer::xferUpgradeMask`: `UpgradeTemplate` (type, name and key, display label, build time, cost, a bit of the 128-bit `UpgradeMaskType`, two sound names, academy type, button image name), `UpgradeCenter` (templates newest first as the linked list is, `DefaultUpgrade` copied into each new one, the three veterancy templates of `init`, the 129th template throws `std::out_of_range`, the `Upgrade` INI block), `Upgrade` (an instance, with its xfer), the `UPGRADE_MASK` helper functions, and `xfer_upgrade_mask` (names, not bits; verified by Rust tests only). Button images and sounds stay names (cut points). Left out for layers 7/8: `calcTimeToBuild(Player*)`, `calcCostToBuild(Player*)` (player-free results are `time_to_build_frames`, `cost_to_build`), `canAffordUpgrade(Player*)`, the per-player `Upgrade` lists (Phase 1 layer 4) |
| `common::special_power` | `Include/Common/SpecialPower.h`, `Source/Common/RTS/SpecialPower.cpp`, `Source/Common/INI/INISpecialPower.cpp`: `SpecialPowerTemplate` (reload time, required science, timers, radius, flags, two sound names, academy type) with its `Overridable` chain (getters read the final override; `getAcademyClassificationType` reads the head, as upstream), `SpecialPowerStore` (lookup by name, id and index; `reset` removing overrides; the `SpecialPower` INI block in `Overwrite` and `CREATE_OVERRIDES` loads) and the `SPECIALPOWERMASK` helper functions. `RequiredScience` takes a function saying which name keys are sciences, until `Science` is translated. Left out for layers 7/8: `canUseSpecialPower(Object*, ...)`. A template copied from a `DefaultSpecialPower` that has an override chain is an error (upstream shares the pointer and frees it at `reset`) (Phase 1 layer 4) |
| `common::academy_classification`, `common::audio_event_name` | `AcademyClassificationType` and its names (`Include/Common/AcademyStats.h`, `Source/Common/RTS/AcademyStats.cpp`; the rest of that file is layer 7), and the name of an `AudioEventRTS` as `INI::parseAudioEventRTS` sets it (`NoSound` changes nothing; the client resolves the name) (Phase 1 layer 4) |
<!-- --- end p1w2-upgrade --- -->
<!-- --- p1w3-thing --- -->
| `logic::thing` | `Include/Common/Thing.h`, `Source/Common/Thing/Thing.cpp`: the position, orientation and world transform (`Matrix3D`), the cached direction vector and the cached heights above terrain and water, the three kind-of queries forwarded to the template, bone-to-world conversion. Cut points: the template (`ThingTemplateView`), the terrain and gravity (`ThingContext`), the reaction to a transform change (returned as `TransformChange`), and the owner's height calculation (the `_by` variants) (Phase 1 layer 5) |
| `logic::object_iter` | `Include/GameLogic/ObjectIter.h`, `Source/GameLogic/Object/SimpleObjectIterator.cpp`: `SimpleObjectIterator` (insert at the head, walks with and without numeric keys, `makeEmpty`, `getCount`) and its bottom-up merge sort over the five `IterOrderType` orders, on an index arena. The build cost of an object is a closure argument (Phase 1 layer 5) |
<!-- --- end p1w3-thing --- -->
<!-- --- p1w3-playertemplate --- -->
| `common::player_template` | `Include/Common/PlayerTemplate.h`, `Source/Common/RTS/PlayerTemplate.cpp`: `PlayerTemplate` (the 45 fields of the `PlayerTemplate` block: sides, display name, `StartMoney` through `Money::init`+`deposit`, colour, starting building and ten units, production cost/time/veterancy maps keyed by name key, intrinsic sciences checked against the `ScienceStore`, command sets, shortcut bar, screens, music, medallions; the default `Handicap`), `PlayerTemplateStore` (`findPlayerTemplate` with the old maps' twelve sub-faction names mapped to their base faction and its fifteen function-static name keys, looked up once and kept; `getTemplateNumByName`, `getAllSideStrings`; `init()` clears, `reset()` keeps) and the `PlayerTemplate` INI block (a name seen before, or mapped from an old name, is updated in place and renamed). Images, sounds and music stay names (cut points). No method takes a `Player` or an `Object`. The cash sound and income record of `StartMoney`'s deposit are dropped (no player list exists then) (Phase 1 layer 4) |
<!-- --- end p1w3-playertemplate --- -->
<!-- --- p1w3-statemachine --- -->
| `common::state_machine` | `Include/Common/StateMachine.h`, `Source/Common/StateMachine.cpp`: `StateMachine` (the enter/update/exit protocol, `STATE_CONTINUE`/`SUCCESS`/`FAILURE` and sleeping states, success/failure successors, conditions, `EXIT_MACHINE_WITH_*`, the 20-deep transition limit, lock/halt, the goal, `xfer` with its snapshot-all-states branch), the four utility states, `isIn*State` with upstream's quirks. States and conditions are values of enums the caller defines (`StateBehavior`), dispatched by `match`; the owner `Object` and `TheGameLogic` are a context (`StateContext`: the frame and `findObjectByID`), a cut point per call (Phase 1 layer 5). The `STATE_MACHINE_DEBUG` code is not translated. A state id defined twice and unbounded re-entrancy are undefined upstream: errors, counted |
<!-- --- end p1w3-statemachine --- -->
<!-- --- p1w3-modules --- -->
| `common::module`, `common::module_factory` | `Include/Common/Module.h`, `Source/Common/Thing/Module.cpp`, `Include/Common/ModuleFactory.h`, `Source/Common/Thing/ModuleFactory.cpp`: `ModuleData` (the module tag, read before it is set is undefined upstream), `ModuleType` and the `MODULEINTERFACE_*` bits, `ObjectModule`/`DrawableModule` (a null module data is `INI_INVALID_DATA`), and the enums that replace the class hierarchy: `Module` (one variant per module class, so far only the abstract bases) and `ModuleDataKind`. `ModuleFactory` is the name to constructor table: all **205** `addModule` names of `init()` in order (199 in the shipped game, `WeaponBonusUpgrade` twice, `LaserUpdate` is a client-update module), each marked translated or not; **none is translated yet** (layer 9), and asking for one is an error, never a skip. Not the concrete modules, `friend_newModuleInstance`/`getModuleNameKey`, the `onCapture`-style virtuals that take a `Player` (Phase 1 layers 8/9) |
| `logic::module::{behavior,update,upgrade,die,create,damage,collide,destroy,body,contain,special_power,special_power_update}_module` | `Include/GameLogic/Module/*Module.h` with `Source/GameLogic/Object/*/*Module.cpp`: the base class of each interface. `UpdateModule` (the sleepy-update word: next call frame and phase in one `u32`, `frameToSleepTime`, wake frames, its xfer), `UpgradeModule` with `UpgradeMux`/`UpgradeMuxData` (the trigger, conflict, require-all and reset decisions; masks computed on first use, which assigns name keys), `DieMuxData::isDieApplicable`, `CreateModule`, `BodyModule` (damage scalar, defaults, `BodyDamageType`), `SpecialPowerModule`'s recharge and pause state, the data classes and their INI fields. Effects on objects, players, the UI and the game logic are arguments or traits (`UpgradeEffects`), listed as cut points for layer 8 in each file (Phase 1 layer 5) |
<!-- --- end p1w3-modules --- -->
<!-- --- p1w4-polygon --- -->
| `logic::polygon_trigger` | `Include/GameLogic/PolygonTrigger.h`, `Source/GameLogic/Map/PolygonTrigger.cpp`, the `K_TRIGGERS_VERSION_*` numbers of `Include/Common/MapReaderWriterInfo.h`: `PolygonTrigger` (the points, the bounds and radius, `pointInTrigger`, the centre's x and y, the water and river flags, the layer name), `PolygonTriggerList` (the chain and `s_currentID`, which are statics upstream), and the `PolygonTriggers` chunk, read (versions 1 to 4, the version-1 water area included) and written. Cut points: the ground height of the centre (`TerrainLogic`), the water extents (passed in), the chunk id the writer takes from the table of contents. The snapshot `xfer` and the editor mutators are not translated (Phase 1 layer 6) |
| `logic::map_object` | `Include/Common/MapObject.h`, `Source/GameClient/MapUtil.cpp` (`ParseObjectDataChunk`), `GameEngineDevice/.../WorldHeightMap.cpp` (the constructor and the waypoint accessors): `MapObject` (position, normalised angle, flags, the property `Dict`, the waypoint bit and name) and the `Object` chunk reader, which fills the waypoint map and keeps every object. The template is kept by name; the tech-building and supply-source classification waits for templates (cut point) (Phase 1 layer 6) |
<!-- --- end p1w4-polygon --- -->
<!-- --- p1w4-thingtemplate --- -->
| `common::thing_template`, `common::thing_factory`, `common::production_prerequisite`, `logic::weapon_set` | `Include/Common/ThingTemplate.h`, `Source/Common/Thing/ThingTemplate.cpp`, `Include/Common/ThingFactory.h`, `Source/Common/Thing/ThingFactory.cpp`, `Source/Common/INI/INIObject.cpp`, `ProductionPrerequisite.cpp`, `WeaponSet.cpp`: `ThingTemplate` (all **110** fields of the `Object` block and the **10** of `ObjectReskin`, the override behaviour of `Overridable`, build variations, armor/weapon sets with `SparseMatchFinder`, prerequisites, per-unit sounds and FX, the module lists with add/remove/replace/inheritable/like-kind rules), `ThingFactory` (template store with `findTemplate`, `findByTemplateID`, `newOverride`, `reset`, `isEquivalentTo`, `newObject`'s template side; upstream's hash_map of templates is a `BTreeMap`, its iteration order is not observable) and the `Object` / `ObjectReskin` blocks. Module blocks go through `ModuleFactory`: a name not translated yet is an **error** at template load, never skipped. Client fields (draw and client-update modules, shadows, audio, button images, FXLists) are stored as data and never interpreted; `newObject` creating a drawable is an `ObjectCreated` client event. `TransportSlotCount` and `StructureRubbleHeight` wrap as upstream's signed `Byte`. Cut points (layers 7/8): `calcCostToBuild`, `calcTimeToBuild`, `getBuildFacilityTemplate`, `getBuildable`'s script override, the `Player` members of `ProductionPrerequisite`, `friend_getAIModuleInfo`/`Locomotor`, `newObject` after the template is chosen (`createObject`, `onCreate`, partition registration), image resolution in `resolveNames` (Phase 1 layer 5) |
<!-- --- end p1w4-thingtemplate --- -->
<!-- --- p1w6-object --- -->
| `logic::weapon` | `Include/GameLogic/Weapon.h`, `Source/GameLogic/Object/Weapon.cpp` (data half), `INIWeapon.cpp`: `WeaponTemplate` (all 80 INI fields, getters under a `WeaponBonus`, `postProcessLoad`), `WeaponStore` and the `Weapon` block. ~~Not the `Weapon` instance, firing or damage application~~ **CORRECTED 2026-10-20** — those are `logic::weapon_fire`, `logic::weapon_set_state` and `logic::weapon_host`, next row (Phase 1 layer 8, wave 7 slice A) |
| `logic::weapon_fire`, `logic::weapon_set_state`, `logic::weapon_host` | The rest of `Weapon.cpp` and all of `WeaponSet.cpp` (Phase 1 layer 8, wave 7 slice B, `p1w8-sonnet`): the `Weapon` instance (ammo, reload with shared reload times, status, bonus, ranges, pitch and line of sight, pre-fire, `privateFireWeapon` and its wrappers, projectiles and lasers, `crc` and `xfer`), `fireWeaponTemplate`, `dealDamageInternal`, `estimateWeaponTemplateDamage`, `shouldProjectileCollideWith`, `WeaponStore::update`, `createAndFireTempWeapon`, `handleProjectileDetonation`, and the owned `WeaponSet` (update, locks, `chooseBestWeaponForTarget`, `getAbleToAttackSpecificObject`, `getAbleToUseWeaponAgainstTarget`, ammo pips, reload all, bonus change, `xfer`). What the weapon asks of the rest of the game is the `WeaponHost` trait (about 100 methods, the pattern of `FiringHost`); a sibling's shared reload time is returned as a `SharedReload` for the caller to apply; the weapon set is checked out of its object during a mutating call. A null `Object*` that upstream dereferences, and `newObject(NULL)` (which throws), are `WeaponError`. Kept upstream defects are listed in the module docs of `weapon_fire`. ~~Not yet: `Object` owning the real `WeaponSet` and a `WeaponHost` for `World`~~ **CORRECTED 2026-10-20 (`p1w8-sonnet`)** — `Object` owns the real `WeaponSet` (see the 107th row); a `WeaponHost` for `World` is still not written (layer 12: `World` has no `WeaponStore` or `ThingFactory` field yet) |
| `logic::world::{object, object_state, object_team, object_damage, object_create, object_transform, object_xfer, object_modules, object_cuts, object_events, module_call, module_probe}` | `Include/GameLogic/Object.h`, `Source/GameLogic/Object/Object.cpp` (Phase 1 layer 8): `Object` with its real fields on the world core (ids for pointers, an arena for the list): status and script status, the disabled states with their timers and edge, weapon-set / weapon-bonus / armor-set flags, team and player membership, defection and capture, veterancy, `attemptDamage` / `onDie` / healing / `kill`, `newObject` after the template side, `initObject`, `onDestroy` and `~Object`, the position and orientation setters with `reactToTransformChange` and the trigger areas, the module lists and their interfaces (create, destroy, die, damage, body, collide, contain, upgrade, special power) as one `ModuleCall` message type, vision, shroud and the partition maintenance, `crc` and `xfer` (version 9). The units not translated yet are cut points (`CutCall`, `ObjectClientEvent`, tables of `ObjectCuts`), listed in `docs/world-design.md`; ~~**not translated**: the weapon forwarders, `updateObjValuesFromMapProperties`, the command-button functions and the bone positions.~~ **CORRECTED 2026-10-20 (`p1w8-sonnet`)** — the weapon forwarders, `adjustModelConditionForWeaponStatus`, the three bone-position functions and `canProduceUpgrade` are `logic::world::object_weapons` (host-generic; see the weapon row); **still not translated**: `updateObjValuesFromMapProperties` and the `doCommandButton*` functions. ~~A call to a module whose body is running is not delivered (recorded in `reentrant_skips`)~~ **CORRECTED 2026-10-18** — such a call runs at once, as upstream's plain recursion (the running body lends itself, `World::lend_body`) |
<!-- --- end p1w6-object --- -->
<!-- --- p1w8-sonnet --- -->
| `logic::world::object_weapons` | The weapon half of `Object.cpp` (Phase 1 layer 8, wave 7): the weapon forwarders, `fireCurrentWeapon`, `preFireCurrentWeapon`, `adjustModelConditionForWeaponStatus`, `getCurrentWeapon`, the three bone-position functions, `getCommandSetString` and `canProduceUpgrade`, generic over `ObjectWeaponHost` / `ObjectBoneHost`. Not translated: `updateObjValuesFromMapProperties` and `doCommandButton*` |
| `logic::action_manager` | `Include/Common/ActionManager.h`, `Source/Common/RTS/ActionManager.cpp` (wave 7, `p1w8-sonnet`): all of `TheActionManager` as free functions over the `ActionHost` trait (what it asks of objects, players, the contain, dozer, supply, special-power and parking modules, shroud and the partition): the object-to-object `can...` queries (repair, heal, dock, enter, attack, garrison, hijack, capture, hack, snipe, defector, ...), `canResumeConstructionOf`, and the `canDoSpecialPowerAt...` family. A null object or player that upstream dereferences is a `WeaponError`. Done before `Player`/`Team`, which wait on untranslated glue (SidesList, AI, scripts, `Squad`) |
<!-- --- end p1w8-sonnet --- -->
<!-- --- p1w9-sonnet --- -->
| `logic::world::{team, team_ops, team_xfer}`, `logic::world::player::PlayerRelationMap` | `Include/Common/Team.h`, `Source/Common/RTS/Team.cpp`, and `PlayerRelationMap` of `Player.h`/`Player.cpp` (Phase 1 layer 7, wave 7, `p1w9-sonnet`): `TeamRelationMap`, `TeamTemplateInfo`, `Team`, `TeamPrototype` and `TeamFactory` in one `TeamArena` with every member function as a free function over the `TeamHost` trait (the arena, the objects of a team, the players and the player list, the script engine, the AI, the partition manager, waypoints, thing templates; the `World` adapter is layer 12's), the member lists and instance lists as the `DLINK` algorithm (including `isInList` counting another list's item as in the list, and an iterator that follows an object that moved to another team), the name-key-ordered prototype map (a duplicate name key is never entered but the prototype lives on, and removing it erases the other's entry), `xfer` of the factory, prototypes, teams and relation maps with the loads that recreate a missing team and give it its saved id, and `loadPostProcess`'s counter recovery (the next id skips one). A `NULL` controlling player, a team that names no team, a loop that never ends and an id shared by two live teams are errors, never guesses. Not `Player` or `PlayerList` (the next units) |
<!-- --- end p1w9-sonnet --- -->
<!-- --- p1w10-sonnet --- -->
| `logic::world::{player, player_ops, player_objects, player_squads, player_xfer}` | `Include/Common/Player.h`, `Source/Common/RTS/Player.cpp` (Phase 1 layer 7, wave 7, `p1w10-sonnet`, slices 1 to 3 of `Player`): the real `Player` (every field of the class in the existing `PlayerArena`; `Player::new` is the constructor's field assignments) and, as free functions over the `PlayerHost` trait (a `TeamHost` too, so a player's loops walk the real team arena and its `DLINK` lists): `init(pt)` (the template's money copied whole, the default starting cash when it is zero), the relationships (player and team overrides), the build list, `update` (the sabotage window, the academy statistics, the once-a-second retaliation message), `setPlayerType` and the AI forwarders, `setDefaultTeam`, the team-prototype list and every loop over a player's objects (counts, `hasAny...`, `findClosestByKindOf`, `iterateObjects`, `setObjectsEnabled`, `healAllObjects`, `updateTeamStates`), radar (the edge sounds) and power (`onPowerBrownOutChange`, with `Energy`'s notice delivered), the special power ready timers, the production changes by kind, `setAttackedBy`, and the sciences and ranks (`addScience` reaching the special power modules of every object, `setRankLevel`, `addSkillPoints`, `attemptToPurchaseScience`, `grantScience`, `resetRank`, `resetSciences`). The AI, the score keeper, the academy statistics, the client and the stores are the 80-method `PlayerHost`; the `World` adapter is layer 12\'s. ~~Not yet: the object-heavy half (`becomingTeamMember`, `becomingLocalPlayer`, the shortcut-power searches, `killPlayer`, the battle plans, upgrades, hotkey squads), `xfer`~~ **CORRECTED 2026-10-20** — slice 2 (`player_objects`: team membership, the local player, the shortcut-power searches, hunting, garrisoning, `transferAssetsFromThat`, selling, bounty, what can be built, the difficulty and battle-plan bonuses, vision spying), slice 2b (`player_squads`: the upgrade list, `Squad`, the hotkey squads and their messages) and slice 3 (`player_xfer`: `crc`, `xfer` version 8, `loadPostProcess`; `BuildListInfo::xfer` in `logic::sides_list`) are translated and differential-verified against the unchanged `Player.cpp`, `Squad.cpp`, `Upgrade.cpp` and the Xfer family. ~~Not yet: `initFromDict`, `PlayerList`~~ **CORRECTED 2026-10-21** — `initFromDict` (`player_init`, over `PlayerInitHost`: the sides list and the script lists) and `PlayerList` (`player_list`: `init`, `reset`, `newGame`, `validateTeam`, the mask lookups, `crc`, `xfer`) are translated and differential-verified (slice 4, `tests/differential_player.rs`, 3,000 programs, 0 mismatches) |
| `logic::world::build_assistant` | `Include/Common/BuildAssistant.h`, `Source/Common/System/BuildAssistant.cpp` (Phase 1 layer 7, wave 7, `p1w10-sonnet`): `BuildAssistant` as free functions over the 86-method `BuildAssistantHost` (objects, templates, the seven `Player` calls, terrain, partition queries, pathfinder cells, AI, production, command sets, audio): footprint sampling, `isLocationLegalToBuild`, `isLocationClearOfObjects`, `buildTiledLocations`, `buildObjectNow`/`buildObjectLineNow`, `canMakeUnit`/`isPossibleToMakeUnit`, `sellObject`, the sell list and its `update`/`xfer`. Differential-verified (`tests/differential_build_assistant.rs`: the unchanged `BuildAssistant.cpp`, 3,000 programs, 0 mismatches, 163 of 170 planted bugs caught by mismatches, 2 by a panic, 5 equivalent). The `World` adapter is layer 12's |
| `logic::modules::{weapon_set,weapon_bonus,armor,model_condition,experience_scalar,active_shroud,max_health,grant_science}_upgrade` | `Object/Upgrade/*.cpp` (Phase 1 layer 9, `p1w10-sonnet` verified the `p1l9-upgrades` drafts): the eight upgrade modules, wired into the object system. Differential-verified through `World` (`tests/differential_upgrade_modules.rs`: the eight unchanged `.cpp` files, 12,000 cases, 141,744 op dumps, 0 mismatches, 21 of 22 planted bugs caught, 1 equivalent) |
| `logic::modules::{status_bits,locomotor_set,passengers_fire,stealth,sub_objects,object_creation}_upgrade` | `Object/Upgrade/{StatusBits,LocomotorSet,PassengersFire,Stealth,SubObjects,ObjectCreation}Upgrade.cpp` (Phase 1 layer 9, `p1w10-sonnet`, upgrade batch B): six more upgrade modules wired into the object system (new cut calls `AiSetLocomotorUpgrade`, `ContainSetPassengerAllowedToFire`, `SpawnGiveSlavesStealthUpgrade`, `OclCreate`; new client events `DrawableShowSubObject`, `DrawableUpdateSubObjects`). Differential-verified (`tests/differential_upgrade_modules_b.rs`: the six unchanged `.cpp` files, 9,000 cases, 110,657 op dumps, 0 mismatches, 29 of 30 planted bugs caught, 1 equivalent) |
| `logic::modules::{cost_modifier,power_plant,radar,unpause_special_power}_upgrade` | `Object/Upgrade/{CostModifier,PowerPlant,Radar,UnpauseSpecialPower}Upgrade.cpp` (Phase 1 layer 9, `p1w11-sonnet`, upgrade batch C): four more upgrade modules, the first with `onDelete`, `onCapture` and a world-aware `loadPostProcess` (`UpgradeBehavior::{on_delete, on_capture, load_post_process_world}`; new calls `ModuleCall::{ExtendRods, ExtendRadar, LoadPostProcess, GetSpecialPowerTemplateId}`, new cut calls `PlayerAdd/RemoveKindOfProductionCostChange`, `PlayerAdd/RemovePowerBonus`; new store `GameStatics::special_power_store`). Differential-verified (`tests/differential_upgrade_modules_c.rs`: the four unchanged `.cpp` files, 9,000 cases, 118,456 op dumps, 0 mismatches, 29 of 29 planted bugs caught) |
| `logic::modules::replace_object_upgrade`, `logic::world::thing_factory_world` | `Object/Upgrade/ReplaceObjectUpgrade.cpp`, and `ThingFactory::{findTemplate, newObject}` over the world (Phase 1 layer 9, `p1w11-sonnet`): the upgrade that destroys its object and makes another from a template, and `World::thing_factory_find_template` / `World::thing_factory_new_object` (the template's final override, the build variation, the behavior modules in order; an untranslated class becomes a stub with the interfaces its registration recorded), for every later module that makes an object by template name. New store `GameStatics::thing_factory`. Differential-verified (`tests/differential_replace_object_upgrade.rs`: the unchanged `.cpp`, 9,000 cases, 177,582 op/object dumps, 0 mismatches, 8 of 8 planted bugs caught) |
| `logic::modules::{dam_die, special_power_completion_die, eject_pilot_die}`, `logic::modules::die_body` | `Object/Die/{DamDie, SpecialPowerCompletionDie, EjectPilotDie}.cpp` (Phase 1 layer 9, `p1w11-sonnet`, die batch D): three die modules over the new `DieBehavior` contract (`onDie` with the module lent to its slot, `isDieApplicable` from the object's veterancy and status), wired as object modules; new `ModuleCall::SetCreator`, cut calls `OclCreateWithSecondary` and `ScriptNotifyOfCompletedSpecialPower`, client event `AudioAddEvent`, stores `GameStatics::object_creation_list_store`, `World::is_significantly_above_terrain`. Differential-verified (`tests/differential_die_modules_d.rs`: the three unchanged `.cpp` files, 9,000 cases, 57,059 op dumps, 0 mismatches, 12 of 12 planted bugs caught, two of them by a C++ crash) |
| `logic::modules::{power_plant_update, radar_update}`, `logic::modules::update_behavior` | `Object/Update/{PowerPlantUpdate, RadarUpdate}.cpp` (Phase 1 layer 9, `p1w11-sonnet`, batch E): the first update modules that run inside the `World` over the new `UpdateBehavior` contract (the scheduler's `update()` with the module lent to its slot, the constructor's world half, the xfer chain over the slot's scheduling half, `setWakeFrame` through the sleepy list), wired as object modules; `ModuleCall::ExtendRods` / `ExtendRadar` reach them; `World::xfer_module_slot`. Differential-verified (`tests/differential_update_modules_e.rs`: the two unchanged `.cpp` files, 9,000 cases, 78,410 dump lines, 0 mismatches, 8 of 8 planted bugs caught) |
| `logic::modules::{crate_collide, heal_crate_collide, shroud_crate_collide, money_crate_collide, upgrade_pair}` | `Object/Collide/CrateCollide/{CrateCollide, HealCrateCollide, ShroudCrateCollide, MoneyCrateCollide}.cpp`, `parseUpgradePair` of `Update/AutoDepositUpdate.cpp` (Phase 1 layer 9, `p1w11-sonnet`, crate batch F): the crate base (`CrateCollideModuleData` and its field table macro, `isValidToExecute`, `onCollide`: execute, FX, destroy, animation) over the `CrateBehavior` contract, and three crates (the money crate keeps upstream's `static` inside the loop, as a per-world cache), wired as object modules; new `ModuleCall::WouldLikeToCollideWith` and the four `Is*CrateCollide` queries, cut calls `PlayerHealAllObjects`, `PartitionRevealMapForPlayer` and `InGameUiAddWorldAnimation`, `World::is_above_terrain`. Differential-verified (`tests/differential_crate_modules_f.rs`: the unchanged `.cpp` files, two mechanical edits in `build.rs`, 9,000 cases, 58,166 dump lines, 0 mismatches, 17 of 17 planted bugs caught, two of them by a C++ crash) |
| `logic::modules::{preorder,special_power,supply_center,supply_warehouse,lock_weapon,veterancy_gain,grant_upgrade}_create` | `Object/Create/*.cpp` (Phase 1 layer 9, `p1l9-creates`, wired by `p1w10-sonnet`): object modules with `ModuleCall::OnBuildComplete`, `impl CreateHost for World` (`logic/world/create_host_world.rs`). Verified standalone against the unchanged `.cpp` files (`tests/differential_create_modules.rs`), and through `World` (`create_host_world::tests`) |
| `logic::modules::{lifetime,deletion,base_regenerate,smart_bomb_target_homing}_update` | `Object/Update/{Lifetime,Deletion,BaseRenerate,SmartBombTargetHoming}Update.cpp` (Phase 1 layer 9, `p1l9-updates`, wired by `p1w10-sonnet`): update modules with the world half of their constructor (`UpdateBody::construct`), `impl UpdateHost/BaseRegenHost/SmartBombHost for World` (`logic/world/update_host_world.rs`). Verified standalone (`tests/differential_l9_updates.rs`) and through `World` (`update_host_world::tests`) |
| `logic::modules::{object_helper,object_smc_helper,object_repulsor_helper,object_weapon_status_helper,status_damage_helper,temp_weapon_bonus_helper,destroy_die,keep_object_die,upgrade_die}` | `Object/Helper/*.cpp`, `Object/Die/{Destroy,KeepObject,Upgrade}Die.cpp` (Phase 1 layer 9; the `p1l9-helpers` drafts rebuilt on the `UpdateModule`/`DieModule` bases over `HelperHost`/`DieHost`, wired into the object system on 2026-10-21: the helpers are what every object is made with, `World` implements `HelperHost`/`DieHost`, tests in `logic::world::helper_host_world`). Differential-verified (`tests/differential_helper_modules.rs`: the nine unchanged `.cpp` files, 12,000 programs, 133,142 ops, 0 mismatches, 24 of 24 planted bugs caught) |
| `logic::object_creation_list`, `logic::radius_decal`, `logic::modules::deliver_payload_data` | `Include/GameLogic/ObjectCreationList.h`, `Source/GameLogic/Object/ObjectCreationList.cpp`, `RadiusDecal.h/.cpp` (the template), `DeliverPayloadAIUpdate.h/.cpp` (`DeliverPayloadData`) (Phase 1 layer 8, wave 7, `p1w10-sonnet`): `ObjectCreationList` and `ObjectCreationListStore` with the five nugget classes as an enum, their INI blocks, and every `create` overload over the 95-method `OclHost` (objects, physics, the drawable and its debris interface, containment, the thing factory, terrain, pathfinder cells, the partition manager, weapons, particle systems, audio). Differential-verified (`tests/differential_object_creation_list.rs`: the unchanged `ObjectCreationList.cpp`, 3,000 cases, 0 mismatches, 134 of 135 planted bugs caught, 1 equivalent). The `World` adapter is layer 12's |
<!-- --- end p1w10-sonnet --- -->
<!-- --- p1w4-sides --- -->
| `formats::data_chunk_output` | `Include/Common/DataChunk.h`, `Source/Common/System/DataChunk.cpp`, the output side: `DataChunkOutput` (open and close chunks with the size patched on close, the primitives, `writeNameKey`, `writeDict`) and `DataChunkTableOfContents::write`, in memory instead of through the temporary file; `finish()` returns the file. Byte for byte against the C++ writer (Phase 1 layer 6) |
| `logic::scripts` | `Include/GameLogic/Scripts.h`, `Source/GameLogic/ScriptEngine/Scripts.cpp`: the script data model (`ScriptList`, `ScriptGroup`, `Script`, `OrCondition`, `Condition`, `ScriptAction`, `Parameter`: linked lists as vectors, `m_parms` with its empty slots), the parse of the `PlayerScriptsList` chunk with the repairs of old files (version 1 conditions, old actions, `Fundamentalist` models, obsolete upgrades, `KIND_OF` and `OBJECT_STATUS` names), the write, `duplicate`/`duplicateAndQualify`, the list editing functions. The condition and action templates are an input (`ScriptTemplates`, from `ScriptEngine::init`, layer 11); not the engine, the editor text (`getUiText`) nor the `Snapshot` methods (Phase 1 layer 6) |
| `logic::sides_list` | `Include/GameLogic/SidesList.h`, `Source/GameLogic/Map/SidesList.cpp`: `SidesList` (players and teams, `BuildListInfo`, parse and write of the `SidesList` chunk, `validateSides`, `addPlayerByTemplate`, `prepareForMP_or_Skirmish` with the skirmish scripts file as bytes). Template and player pointers are names; not `parseStructure` (INI, AI layer), the `Snapshot` methods, nor `Player` creation (Phase 1 layer 6) |
<!-- --- end p1w4-sides --- -->
<!-- --- p1w6-locomotor --- -->
| `logic::locomotor` | `Include/GameLogic/Locomotor.h`, `Include/GameLogic/LocomotorSet.h`, `Source/GameLogic/Object/Locomotor.cpp`: `LocomotorTemplate` (the 61-entry `Locomotor` INI block, `validate`), `LocomotorStore` (templates by name key with override chains, `reset`), `Locomotor` (the movement physics of every appearance: legs, wheels, treads, hover, thrust, wings, climber, other; braking, turning, the eight Z behaviours, ultra-accurate mode, `maintainCurrentPosition`, the limits per damage condition, `xfer`) and `LocomotorSet` (`findLocomotor`, `xferSelfAndCurLocoPtr`). Everything the object, `PhysicsBehavior`, the body module, `TerrainLogic`, `PartitionManager` and the pathfinder provide is the `LocomotorHost` trait (listed in the module docs). Doubles upstream promotes to (`fabs`, `sqrt`, `atan2` results in larger expressions) are reproduced. Undefined upstream, reported not copied: an uninitialised `relAngle` in the treads, `reset()` after an override-made template, legs with a minimum speed (endless recursion) |
<!-- --- end p1w6-locomotor --- -->

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
  line and asserted to occur once. The BitFlags build compiles `BitFlags.cpp`,
  `KindOf.cpp`, `ObjectStatusTypes.cpp` and `DisabledTypes.cpp` from copies
  that add the `template<>` MSVC did not require before each name list, and
  copies the name lists of `WeaponSet.cpp` and `SpecialPower.cpp` verbatim
  and `SparseMatchFinder.h` with the two `typename`s MSVC did not require
  (those files need most of the engine); it uses clang++ with
  `-fms-extensions -fdelayed-template-parsing` and includes `KindOf.h` before
  `Xfer.h`, whose `enum KindOfType;` forward declaration only MSVC accepted
  in any order (`differential/shim_flags/`). `Dict.cpp` keeps each string
  in a pointer-sized slot, so its build uses pointer-sized string shims
  (`differential/shim_dict/`), with the class names renamed by macro so
  they cannot be confused at link time with the other builds' shims. The
  real `AsciiString.cpp` is built the same way (`differential/shim_ascii/`),
  with `strcpy` as the forward copy MSVC's runtime makes (glibc's corrupts
  the overlapping copies `trim` and `nextToken` make; see
  `common::ascii_string`).
- `differential/shim/` holds minimal stand-ins for headers that cannot be
  compiled outside MSVC/Windows (each says what it replaces and why;
  `differential/shim_ini/` adds, for the INI build only, declaration-only
  stand-ins for the game systems `INI.cpp` includes but the tests never
  reach), and `differential/*.cpp` holds `extern "C"` wrappers. Where upstream is MSVC
  inline assembly with no C++ fallback (`BaseType.h`, the release path of
  `crc.h`), the reference is a GNU-asm transcription of the same
  instructions, labelled as such.
<!-- --- p1w1-small --- -->
- `differential/shim_stl/` builds `STLTypedefs.h` with the real `AsciiString.h` and `AsciiString.cpp` (classes renamed by macro, as in the AsciiString build), a `BaseType` forwarder to the shim (the real header is MSVC-only), and a `UnicodeString` with `compareNoCase` that aborts if called. `differential/shim_subsys/` builds `SubsystemInterface.cpp` with its own `PreRTS.h`, stand-ins for `INI.h` and `Xfer.h` (their bodies abort; the wrapper passes no INI paths), and forwarders to the shim tree for everything except the real `SubsystemInterface.h`.
<!-- --- end p1w1-small --- -->
<!-- --- p1w2-globaldata --- -->
- `differential/shim_globaldata/` builds `GlobalData.cpp` and `INIGameData.cpp` on the real `GlobalData.h` (clang++ with `-fms-extensions`, the shipped game's `NDEBUG` configuration, so the debug-only members do not exist). `GlobalData.cpp` is compiled from a copy that build.rs makes with one edit, asserted to match once: the tail of `parseGameDataDefinition` that applies the player's `OptionPreferences` is cut. The enumerations `GlobalData.h` forward-declares (`BodyDamageType`, `AIDebugOptions`, `TerrainLOD`), `WeaponBonus`/`WeaponBonusSet` and its member functions are copied verbatim from the real headers and `Weapon.cpp` into `OUT_DIR` (the rest of those files needs the engine). Stand-ins: the Windows calls of the constructor (no executable, My Documents or registry; a 500 ms double-click time), `CRC` (the C++ loop of `crc.h`), `Version`, `UserPreferences`, and `GameMemory.h` spelled so that `NEW GlobalData;` parses. `shim/Lib/BaseType.h` gained `RGBColor::setFromInt` and `shim/Common/AsciiString.h` `getCharAt`.
<!-- --- end p1w2-globaldata --- -->
<!-- --- p1w2-upgrade --- -->
- `differential/wrap_upgrade.cpp` runs the real `Upgrade.cpp`, `INIUpgrade.cpp`,
  `SpecialPower.cpp` (a copy in which the one definition of
  `SpecialPowerMaskType::s_bitNameList` is renamed so the real list can be
  dumped) and `INISpecialPower.cpp` behind the real `INI::load`, `Overridable.h`
  and `NameKeyGenerator`, included under renaming macros (`differential/shim_upgrade/upgrade_prelude.h`)
  so that they cannot clash with the Xfer build's aborting stand-ins for
  `UpgradeCenter`. `differential/shim_upgrade/` holds the stand-ins: a forwarder
  to the real `Upgrade.h` (the Xfer build's shim sits earlier on other include
  paths), an `AudioEventRTS` that keeps its name, `Player`, `Object`,
  `InGameUI` and `Image`. Three `wrap_ini.cpp` stand-ins (`friend_lookupScience`,
  `AudioEventRTS::setEventName`, `AudioManager::getInfoForAudioEvent`) and the
  two block stubs are now `WEAK`, like the Armor ones. Results
  (`tests/differential_upgrade.rs`, `tests/differential_special_power.rs`,
  2026-10-18, upstream `0a05454d`): 8,000 `Upgrade` files (7,781 compared, 63,201
  templates, 251 throwing `std::out_of_range` at the 129th mask bit, 300
  centers of 135 direct `newUpgrade` calls), 6,000 `SpecialPower` sequences
  (14,220 files in four load types, 15,456 final templates, 817 with an
  override chain, 2,126 resets, 193,754 getter results), 6,000 `Upgrade`
  instance xfers, 60,000 mask-helper cases: **no mismatch**; every comparison
  also catches its planted bug. Left out of the C++ comparison as undefined
  upstream (counted by the tests): numbers out of range of their type in a
  field, a `calcTimeToBuild` float-to-int out of range, and a template copied
  from a `DefaultSpecialPower` that has an override chain.
<!-- --- end p1w2-upgrade --- -->
<!-- --- p1w3-thing --- -->
- `differential/shim_thing/` and `differential/wrap_thing.cpp` compile `Thing.cpp` unchanged, and `differential/wrap_object_iter.cpp` compiles `SimpleObjectIterator.cpp` unchanged. The stand-ins (`ThingTemplate`, `GlobalData`, `TerrainLogic`, `Object`) are renamed by macro in the two wrappers, so they cannot clash with the other fixtures. `Coord3D` is copied verbatim from the real `BaseType.h` (its struct, found by build.rs), because `Thing.cpp` calls `Coord3D::crossProduct`. Results (`tests/differential_thing.rs`, `tests/differential_object_iter.rs`, 2026-10-18, upstream `0a05454d`): 4,000 random `Thing` programs (30,023 operations over 18 operation kinds, 0 mismatches) and 6,000 iterator programs (124,002 operations, 0 mismatches). Left out of the C++ as undefined upstream and checked to be `Err` in Rust: `Thing(NULL)`, `setPosition(NULL)`, and angles that `normalizeAngle` never reduces. All 27 planted bugs were caught.
<!-- --- end p1w3-thing --- -->
<!-- --- p1w3-playertemplate --- -->
- `differential/wrap_playertemplate.cpp` runs the real `PlayerTemplate.cpp` behind the real
  `INI::load`, `NameKeyGenerator`, `Money::deposit` (from the Money build), `Handicap` and the
  real `Science.cpp`, in the Science build's setup plus `differential/shim_playertemplate/`
  (a prelude adding three typedefs of the real `STLTypedefs.h`, and the INI build's one-lookup
  `Image.h`). It defines no symbol another fixture defines: `ThePlayerTemplateStore` and
  `INI::parsePlayerTemplateDefinition` (now `WEAK` in `wrap_ini.cpp`) are its own, and it
  installs the real Science lookup through `ref_science_lookup_hook` in its reset function. The
  one `PlayerTemplateStore` lives for the whole test, `init()` between sessions, because
  `findPlayerTemplate`'s fifteen function statics are initialised once per process. Results
  (`tests/differential_player_template.rs`, 2026-10-18, upstream `0a05454d`): 3,000 sessions,
  8,317 loads, 10,181 dumps of every field of the store, 852,334 lookups, the fifteen statics:
  **no mismatch**; every comparison also catches its planted bug. Left out of the C++
  comparison as undefined upstream (counted by the test): numbers out of range of `Int`.
<!-- --- end p1w3-playertemplate --- -->
<!-- --- p1w3-statemachine --- -->
- `differential/wrap_statemachine.cpp` runs the real `StateMachine.cpp`,
  compiled unchanged as its own translation unit, with `StateMachine.h` and the
  Xfer headers unchanged, against `differential/shim_statemachine/`: `GameLogic.h`
  (the frame and `findObjectByID`), `Object.h` (`getID`, `getPosition`),
  `ThingTemplate.h`, `GlobalData.h` (debug logging only) and a `GameState.h`
  that adds the `SaveCode` enum, which build.rs cuts verbatim out of the real
  header (asserted once). `statemachine_prelude.h` (forced in) renames
  `Object`, `GameLogic`, `TheGameLogic` and `ThingTemplate` to `Sm*` by macro, so
  that no symbol of the archive clashes with another fixture's stand-in or
  translation, and makes `private`/`protected` public for the wrapper's dump.
  The machine is driven through the real `XferSave`, `XferLoad`, `XferCRC` and
  `XferDeepCRC` of the Xfer build (this archive is linked ahead of it); loads go
  through a `GuardedLoad` that flags a `bool` byte other than 0/1 and a short read
  of a field, which are undefined or unspecified upstream. Results
  (`tests/differential_state_machine.rs`, 2026-10-18, upstream `0a05454d`): 6,000
  random state graphs with operation scripts, 5,897 run on both sides (131,922
  operations, 746,502 callbacks, 278,595 conditions, 66,723 re-entrant actions,
  6,324 saves, 3,632 CRC operations, 18,981 loads); **no mismatch** in the events,
  results, xfer bytes or CRCs; each comparison catches its planted bug. Left out of
  the C++ comparison as undefined upstream (counted by the test): a state id defined
  twice (103 cases). Found: a load that fails inside a field leaves the C++ field
  partly overwritten (`fread` into the destination) while the Rust typed Xfer
  methods assign only on success (`common::xfer`, unchanged; the test cuts streams at
  field boundaries).
<!-- --- end p1w3-statemachine --- -->
<!-- --- p1w3-modules --- -->
- `differential/wrap_module.cpp` runs the real `Module.cpp`, `BehaviorModule.cpp`,
  `UpdateModule.cpp`, `UpgradeModule.cpp`, `DieModule.cpp`, `CreateModule.cpp`,
  `DamageModule.cpp`, `CollideModule.cpp`, `DestroyModule.cpp` and `BodyModule.cpp`
  (with the unchanged headers) behind the real `INI`, `XferSave`/`XferLoad`/`XferCRC`
  and the **real `UpgradeCenter` of the Upgrade build** (the module tests define their
  upgrades through `ref_up_reset`/`ref_up_load`; the Upgrade classes keep the
  Upgrade build's renames, the stand-in classes of this build get names of their own,
  `differential/shim_module/module_prelude.h`). `differential/shim_module/` holds the
  stand-ins of `Thing`, `ThingTemplate`, `Object`, `GameLogic` (a frame counter that
  records `friend_awakenUpdateModule`), `Drawable`, `ExperienceTracker` and `FXList`;
  `shim/Common/AsciiString.h` gained `isNone`. `ModuleFactory.cpp` cannot be
  compiled (205 module headers): `build.rs` cuts the body of `init()` out of it
  verbatim and the wrapper preprocesses it with `addModule` as a recorder, with and
  without `ALLOW_SURRENDER`/`ALLOW_DEMORALIZE`. Not compiled, so tested by Rust unit
  tests against the upstream text: `SpecialPowerModule.cpp`,
  `SpecialPowerUpdateModule.cpp` (they need the player, the UI, the script engine
  and the thing factory), the `ModuleFactory` lookups (the 205 classes) and
  `ContainModuleInterface::getContainerPipsToShow`. Results
  (`tests/differential_module.rs`, 2026-10-18, upstream `0a05454d`): the 205 and the
  199 registration names in order; 37 constants; 6,000 module-data blocks over nine
  data classes (5,586 compared, 2,739 read, 2,847 threw with their messages;
  414 left out: a field line in a base `ModuleData` block reads a null parse table);
  6,000 `UpgradeModuleData` over 0 to 8 upgrades driven through 39,926
  `UpgradeMux` operations (5,338 sequences compared, 852 modules fired, 4,119
  resets, every effect in order, every field of the mux data after every
  operation, the name keys after); 6,000 `UpdateModule` sequences (44,719
  operations in the four phases); 40,656 `isDieApplicable` queries; 4,400 base-class
  xfers (save, load, CRC) over 11 classes: **no mismatch**; every comparison also
  catches its planted bug, and eight bugs put into the Rust source one at a time were
  each caught. Left out of the C++ comparison as undefined upstream (counted):
  an empty or `None` upgrade name that finds no upgrade, a field line in a base
  `ModuleData` block, `DEATH_NORMAL`/`LEVEL_REGULAR` (`1UL << -1`), a `Bool` byte
  above 1 read by `xferBool`.
<!-- --- end p1w3-modules --- -->
<!-- --- p1w4-polygon --- -->
- `differential/wrap_polygon_trigger.cpp` runs the real `PolygonTrigger.cpp` (through a build.rs copy that moves one loop declaration, as for `INI.cpp`) on `differential/shim_polygon/`, the real `DataChunk` reader and `Xfer` headers. `differential/wrap_map_object.cpp` runs `ParseObjectDataChunk`, cut verbatim out of `MapUtil.cpp` by build.rs. Results (`tests/differential_polygon_trigger.rs`, `tests/differential_map_object.rs`, 2026-10-18, upstream `0a05454d`): 6,000 polygon-trigger chunk files (4,422 parsed, 10,590 triggers, 204,361 comparisons: the chain, the written chunk bytes, 168,032 point queries, centres and radii) and 6,000 object-chunk files (4,753 parsed, 11,876 objects, 2,087 waypoints, 11,404 comparisons, 145 thrown `ErrorCode`s). **No mismatch** in either. Every comparison catches its planted bug (five polygon and four object bugs). Left out of the C++ comparison as undefined upstream (counted by the tests): chunk reads past the end of the file, a version-1 id at `Int` maximum, `Int` overflow in a query, the bounds or the centre, a water extent out of `Int` range, an angle `normalizeAngle` never reduces, and a table of contents past the end by over 10^5 entries. Shims changed: `MemoryPoolObject::deleteInstance` keeps its `if (this)`, and the chunk `Dict` stand-in gains `getType` and `getAsciiString`; the polygon writer stand-in takes `strlen` of a name, as upstream's `getLength` does.
<!-- --- end p1w4-polygon --- -->
<!-- --- p1w4-thingtemplate --- -->
- `differential/wrap_thingtemplate.cpp` runs the real `ThingTemplate.cpp`, `ThingFactory.cpp` and `INIObject.cpp` behind the real `INI::load`, field parsers, `NameKeyGenerator`, `ArmorStore`, `DamageFXStore`, `ScienceStore` and `SparseMatchFinder`, with the cut-out members of `WeaponSet.cpp`, `ProductionPrerequisite.cpp` and `ModuleFactory.cpp` (build.rs, each marker asserted once) and stand-ins renamed by macro (`Tt*`, `differential/shim_thingtemplate/`). The Object block reaches it through `ref_object_block_hook` in `wrap_ini.cpp`. The 205 module classes are not translated, so field comparisons use Object blocks without module sub-blocks plus a set of test modules both sides register; a real module block gives `ModuleNotTranslated` in Rust (C++ makes the class from the block, and for a name the factory has not heard of gets a NULL `ModuleData` and dereferences it). Results (`tests/differential_thing_template.rs`, 2026-10-18, upstream `0a05454d`): 3,000 Object-block sessions (7,562 loads, 17,699 template objects, 464,481 lookups) and 3,000 module-list sessions (8,171 loads, 18,467 template objects, 29,493 modules, 488,325 lookups): **no mismatch**; every comparison also catches its planted bug. The test found two upstream behaviours the first translation lacked: the signed `Byte` wrap, and a copied `SparseMatchFinder` cache that answers with a pointer into the source's vector. Left out of the C++ as undefined upstream (counted): integers and reals out of range, a template copied from one with an override chain, a `reset()` that deletes a template from the middle of the list, a null parse table.
<!-- --- end p1w4-thingtemplate --- -->
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

<!-- // --- p1w1-unicode --- -->
- The real `UnicodeString.cpp` is built like `AsciiString.cpp`
  (`differential/shim_unicode/`), with `-fshort-wchar` so that `WideChar` is
  16-bit as on Win32 and 16-bit stand-ins for the MSVC wide-string runtime
  (`wcslen`, `wcscpy`/`wcscat` as forward copies, `wcscmp` on unsigned units,
  `_wcsicmp` folding to lower case, `wcsspn`, `wcscspn`, `iswspace` for the C
  locale; each documented in `unicode_prelude.h`). `iswspace` of units from
  0x80 is not established for MSVC, so `trim` returns an error there rather
  than guess (see `common::unicode_string`).

<!-- --- p1w4-terrain --- -->
- `differential/wrap_terrain_logic.cpp` and `differential/shim_terrain_logic/` compile the real `TerrainLogic.cpp` (the one `static` edit and the extraction are done by `build.rs` into `OUT_DIR`, asserted once) over stand-ins for `TheTerrainVisual`, the object world, the pathfinder and the partition manager. Every call from terrain to those, and every `TheTerrainVisual` write, is logged and compared with the Rust (`TerrainWorld`, `TerrainVisualEvent`). The water grid and raw heights are logic-owned state; whether that is the final design is undecided (`docs/phase1-porting-graph.md`, p1w4-terrain). Results (`tests/differential_terrain_logic.rs`, 2026-10-18, upstream `0a05454d`): 22,139 programs compared, 346,279 operations, 0 mismatches; 10 of 11 planted bugs caught. Left out of the C++ as undefined upstream and checked to be `Err` in Rust: an uninitialised `pickBridge` position, stale water handles, a missing water transform, and 11 more kinds. `PolygonTrigger` and `MapObject` are stand-ins until their own translation lands.
<!-- --- end p1w4-terrain --- -->
<!-- --- p1w6-object --- -->
- `differential/wrap_object.cpp` and `differential/shim_object/` compile the text of `Object.cpp` and of the class `Object` (`build.rs` cuts the includes and the untranslated regions out in memory and writes them to `OUT_DIR/object_gen`, every marker asserted once; upstream is not edited) together with the verbatim texts of `Thing.cpp`, `ExperienceTracker.cpp`, seven functions of `Geometry.cpp`, the enums, `Coord3D` / `ICoord3D` / `Region3D` and the weapon-set model-condition map, on hand-written stand-ins for the teams, players, partition manager, radar, audio, AI, script engine, game logic and the module interfaces, all in the namespace `ObjOpWorld`. `tests/differential_object.rs` runs random programs on both sides and compares the trace (cut points, client events, module calls), the return values and the `XferSave` snapshot of every object after every operation; results in its header.
- `differential/wrap_weapon.cpp` also compiles the whole of `Weapon.cpp` and `WeaponSet.cpp` for `tests/differential_weapon_fire.rs` (`ref_weapon_fire_run`): `build.rs` replaces only the include block of each (`Weapon_cut.cpp`, `WeaponSet_cut.cpp` in `OUT_DIR`), cuts the bit-name list of `WeaponSetFlags` and the four INI parsers of `WeaponTemplateSet`, and adds one `Bool` to `const isProjectileDetonation = false;`, every edit asserted to apply once. The stand-in classes (`differential/shim_weapon/fire_standins.h`, renamed `WpnTest*` by `#define`) include an `Object` that embeds a **real** `WeaponSet`; `differential/shim_weapon/fire_driver.h` interprets a program of 37 operations that the Rust test runs on its own `WeaponHost`, and both write the trace lines that are compared. One stand-in differs on purpose (`MemoryPoolObjectHolder` tests for null, see the test header). Results (2026-10-20, upstream `0a05454d`): 30,000 programs, 24,082,803 trace lines, 0 mismatches, 1,363 programs excluded as undefined upstream; 21 planted bugs caught (three of them only after the generator was strengthened, two only at 30,000 programs).
- `differential/wrap_team.cpp` compiles the unchanged `Team.cpp` and `Team.h` (`build.rs` replaces only the include block of `Team.cpp`, asserted once; takes `PlayerRelationMap`'s class and five functions verbatim from `Player.h`/`Player.cpp`; and makes one portability edit to a copy of `GameCommon.h`, asserted once: clang reads the DLINK iterator's `OBJCLASS::dlink_next_X` only as `&OBJCLASS::dlink_next_X`, as MSVC does) on the stand-ins of `differential/shim_team/` (objects with the `DLINK`, players, the player list, thing templates, the AI, the contain module, scripts, waypoints, the partition manager, the game logic, the sides list and a small `Dict` with the real getters' meaning; every class renamed `TmTest*`), for `tests/differential_team.rs` (`ref_team_run`). Results (2026-10-20, upstream `0a05454d`): 3,000 programs, 4,410,485 trace lines, 0 mismatches, 428 programs excluded as undefined upstream; 61 planted bugs all caught (nine by crashing or not ending, one by the test's tripwire on `NULL` dereferences; three rounds of generator strengthening were needed, see the test header).
- `differential/wrap_player.cpp` (`p1w10-sonnet`) compiles the unchanged `Player.h` and the functions of `Player.cpp` (and of `Energy.cpp`, `Money.cpp`, `Handicap.cpp`, `MissionStats.cpp`, `PlayerList.cpp`) that the slice translates, each taken whole by `build.rs` into `OUT_DIR/player_gen/player_fns.inc` (asserted once and closed), with the unchanged `Team.h` and `Team.cpp` (the world a player's loops walk) and `BattlePlanUpdate.h`'s `BattlePlanStatus`/`BattlePlanBonuses` (`Player.h` forward-declares the enum, which clang accepts only after its definition), under a forced-include prelude (`differential/shim_player/player_prelude.h`: the renames of every class and global the archive defines or stands in for, `private` made public, the INI prelude done by hand with the real `Science.h`/`Upgrade.h`) on the stand-ins of `differential/shim_player` (the AI, the score keeper, the academy statistics, the rank and science stores, the client, the global data and game state, objects with special power modules), for `tests/differential_player.rs` (`ref_player_run`). Results (2026-10-20, upstream `0a05454d`): 3,000 programs, 5,254,698 trace lines, 0 mismatches, 27 programs excluded as undefined upstream; 77 planted bugs all caught (four rounds of generator strengthening, see the test header).
- The same harness also compiles the unchanged `ActionManager.cpp` and the weapon functions of `Object.cpp` (`build.rs` extracts them into `Object_weapons.cpp` and `ActionManager_cut.cpp` in `OUT_DIR`, each cut asserted to apply once) behind further stand-ins (`CommandButton`, the dozer, dock and special-power interfaces, ..., renamed `WpnTest*` by `#define` so no second strong definition of an upstream symbol exists). Results (2026-10-20, upstream `0a05454d`): 30,000 programs, 21,334,856 trace lines, 0 mismatches, 4,073 programs excluded as undefined upstream; 31 `ActionManager` planted bugs all caught (eight of them only after the generator was strengthened, at 30,000 programs; two make the program crash rather than differ).
<!-- --- end p1w6-object --- -->

## Licence

GPL-3.0-only, with the additional terms in [`NOTICE.md`](NOTICE.md), which
must accompany every copy. See [`LICENSE`](LICENSE).
