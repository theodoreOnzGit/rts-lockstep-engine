# Phase 1 porting graph: GameLogic and what it needs from Common

Drafted 2026-10-17, against upstream commit `0a05454d8574207440a5fb15241b98ad0b435590`.
This is the plan for `SCOPE.md` section 5, Phase 1 (GameLogic, headless): what
there is to translate, in what order, where the client has to be cut away, and
how each piece can be checked against the original C++.

## How the numbers were taken

Every number here is measured by `scripts/porting_graph.py`, which reads only
the pinned `upstream/` submodule (`python3 -I scripts/porting_graph.py --layers`,
`--table`, `--upward`, `--json`). Nothing is estimated. The rules it applies:

- **Lines** are newline counts (what `wc -l` prints), comments included.
- **Scope.** All of `GameEngine/Source/GameLogic/` and `GameEngine/Include/GameLogic/`
  (527 files: 186,556 lines of `.cpp`, 42,471 of headers), the engine headers of
  the precompiled header `Include/Precompiled/PreRTS.h`, and then, transitively,
  every `Include/Common/` or `Include/GameLogic/` header those files include,
  with the `.cpp` of the same name. That closure is **739 files, 308,324 lines,
  in 398 units**.
- A **unit** is a header plus the `.cpp` file(s) with the same basename
  (`Object.h` + `Object.cpp`); most modules are one unit each.
- **Dependencies** are `#include` lines (comments stripped), resolved
  case-insensitively against `GameEngine/Include` as Windows did. `PreRTS.h`,
  which every `.cpp` includes, is **not** counted as an edge from every file:
  it would make every file depend on `Thing.h` and `GlobalData.h` whether it
  uses them or not. Its engine headers are: `Lib/Basetype.h`,
  `Common/STLTypedefs.h`, `Errors.h`, `Debug.h`, `AsciiString.h`,
  `SubsystemInterface.h`, `GameCommon.h`, `GameMemory.h`, `GameType.h`,
  `GlobalData.h`, `INI.h`, `KindOf.h`, `DisabledTypes.h`, `NameKeyGenerator.h`,
  `GameClient/ClientRandomValue.h`, `GameLogic/LogicRandomValue.h`,
  `ObjectStatusTypes.h`, `Thing.h`, `UnicodeString.h`. A translated module
  imports what it actually uses, so this does not change the order below.
- **Client/device/network touches** are include lines naming `GameClient/`,
  `GameNetwork/`, `W3DDevice/`, `Win32Device/` or an audio header, plus counted
  references (comments stripped) to the client-side singletons (`TheAudio`,
  `TheInGameUI`, `TheTacticalView`, `TheGameClient`, ...; the list is
  `CLIENT_GLOBALS` in the script). 203 of the 398 units have at least one.
- **Replaced infrastructure** (layer R below) is not a dependency: edges into
  it are dropped before levels are computed, because Rust replaces it rather
  than translating it.

### The include graph is one knot; the header graph is not

With `.cpp` files counted, the include graph has a strongly connected component
of 344 units (281,463 lines): `Object.cpp` includes `Player.h`, `Player.cpp`
includes `Object.h`, and so on. So there is no topological order of whole
units. With **headers only** the graph is a DAG apart from one 3-unit cycle
(`BitFlagsIO.h` <-> `ModelState.h` <-> `Xfer.h`, a template-instantiation
detail). The script therefore computes:

- the *header level*: 1 + the highest header level among the headers a header
  includes (the cycle collapsed);
- the *unit level*: 1 + the highest header level among everything the unit's
  header **and** `.cpp` include.

A unit at level *n* can be translated once the declarations it uses exist. The
measured level histogram (units / lines):

| level | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| units | 37 | 5 | 5 | 2 | 10 | 5 | 5 | 6 | 21 | 5 | 9 | 13 | 91 | 29 | 155 |
| lines | 5,689 | 2,207 | 2,059 | 251 | 2,329 | 1,392 | 7,330 | 3,617 | 8,168 | 4,701 | 3,022 | 7,850 | 39,272 | 30,695 | 189,742 |

71 % of the lines (220,437) sit at levels 13 and 14: everything that includes
`Object.h`, `Player.h`, `GameLogic.h` or `ThingTemplate.h`. The levels are a
guide, not the order: they rank units by the deepest header they include,
which for a big `.cpp` is nearly always the top. The translation layers below
are assigned by hand from these levels and the dependency lists, and the
script then checks them: an **upward include** is a unit including the header
of a unit in a later layer. Each one is a call the lower layer must make
through an interface, or a method that moves up a layer. There are 701; their
distribution is in the layer table, and `--upward` lists every one.

## The layers

The order to translate in, bottom up. Sizes are the measured lines of the
units assigned to the layer (`--layers`). "Touching" counts units with any
client/audio/network include or reference.

| layer | contents | units | files | lines (h+cpp) | client/audio/net-touching units | upward includes |
|---|---|---:|---:|---:|---:|---:|
| D | done in Phase 0 (fully or partly) | 19 | 34 | 2458+6729 = 9187 | 2 | 0 |
| 1 | value types and enums | 17 | 21 | 1891+350 = 2241 | 0 | 0 |
| 2 | strings, names, subsystem registry | 6 | 10 | 2589+1240 = 3829 | 0 | 1 |
| 3 | flag sets and small containers | 11 | 16 | 2090+1067 = 3157 | 0 | 0 |
| 4 | static data from INI (templates and stores) | 17 | 34 | 3207+5595 = 8802 | 7 | 20 |
| 5 | thing and module system | 19 | 36 | 4367+6582 = 10949 | 6 | 238 |
| 6 | map data and terrain | 5 | 9 | 2265+7479 = 9744 | 3 | 15 |
| 7 | players, teams and economy | 13 | 26 | 3108+14976 = 18084 | 8 | 67 |
| 8 | object core, weapons, movement, partition | 11 | 20 | 4703+22545 = 27248 | 5 | 69 |
| 9 | object modules | 197 | 388 | 21406+70057 = 91463 | 121 | 261 |
| 10 | AI and pathfinding | 29 | 55 | 8940+54836 = 63776 | 20 | 24 |
| 11 | map scripts and victory | 4 | 8 | 1180+21283 = 22463 | 4 | 6 |
| 12 | game loop, commands, replay, save | 6 | 11 | 1745+12164 = 13909 | 6 | 0 |
| C | client/device/shell side: behind a cut-point interface | 30 | 46 | 3732+8831 = 12563 | 16 | 0 |
| R | replaced, not translated | 14 | 25 | 2435+8474 = 10909 | 5 | 0 |

Layers 1-12 add up to **275,665 lines** to translate. Plus 495 lines of
`Libraries/Include/Lib/BaseType.h`, outside `GameEngine` and so outside the
script, whose value types (`Coord3D`, `Region3D`, `RGBColor`, ...) belong to
layer 1.

### D. Done in Phase 0 (9,187 lines)

`RandomValue` (all three streams), `crc`, `QuickTrig`, `Trig`, the `Xfer`
family (`Xfer`, `XferCRC`, `XferDeepCRC`, `XferSave`, `XferLoad`, `Snapshot`),
the INI reader core, `DataChunk` (reader), the `.big` file system, and the
damage/death flag parts of `Damage.h`. **Partly done** and to be finished in
their layers: `INI.cpp`'s 62 block handlers and the field parsers that look
names up in game systems (layers 4-5), `Xfer`'s ~~`xferKindOf`,~~
`xferUpgradeMask`, `xferScienceType`, `xferMapName` (layers 3-4; `xferKindOf`
done with layer 3),
`Damage.cpp`'s `DamageInfo` and its xfer (layer 4).

### 1. Value types and enums (2,241 lines + BaseType.h)

Leaves: no logic dependencies at all. `BaseType.h`'s `Coord2D/3D`, `ICoord`,
`Region2D/3D`, `RGBColor`, `RealRange` methods and `sqr`/`clamp`/`sign`/
`rad2deg`/`deg2rad`; `GameType.h` (object/drawable ids, time of day, weather,
weapon slots, pathfind layers, with the name tables of `GameType.cpp`);
`Errors.h`; the remaining parts of `GameCommon.h`; the small enum headers
(`Powers.h`, `WeaponSetType.h`, `WeaponStatus.h`, `WeaponBonusConditionFlags.h`,
`ObjectScriptStatusBits.h`, `SpecialPowerType.h`, `ThingSort.h`);
`DiscreteCircle`, `PartitionSolver`; the helper templates `Overridable`/
`Override` (INI overrides of templates) and `LatchRestore` (a scope guard).
`FPUControl.h` is the 24-bit x87 mode that `SCOPE.md` section 3 replaces with
plain `f32`; it is documented, not translated.

*Differential route:* direct. A wrapper includes the headers and calls the
inline functions; `GameType.cpp` and friends compile unchanged.

### 2. Strings, names, subsystem registry (3,829 lines)

`AsciiString`/`UnicodeString` (copy-on-write buffers; the port keeps their
*semantics* on owned `Vec<u8>`/`Vec<u16>`: `compareNoCase` is ASCII-only
`stricmp`, `nextToken`, `trim`, `toLower`, `format`), `NameKeyGenerator`
(string -> integer keys, assigned in first-seen order: **key values are part of
the lockstep state** because `Dict`s and module tags store them),
`WellKnownKeys` (924 lines of `StaticNameKey`s), `STLTypedefs.h` (the hash and
compare functors behind every `hash_map`, see "Determinism hazards"), and
`SubsystemInterface` (the init/reset/update registry: becomes an explicit
struct of subsystems owned by the logic, no globals).

*Differential route:* direct for `NameKeyGenerator.cpp` (it already compiles in
the `DataChunk` build). `AsciiString.cpp`/`UnicodeString.cpp` compile against
the shim `GameMemory.h` with a `malloc` allocator.

### 3. Flag sets and small containers (3,157 lines)

`BitFlags<N>` (a `std::bitset` with a name table) and `BitFlagsIO.h` (its INI
parsing and xfer, which writes flag **names**, not bits), and its
instantiations: `KindOf`, `ObjectStatusTypes`, `DisabledTypes`,
`ModelState` (model condition flags, needed by logic because modules test
them), `WeaponSetFlags`, `ArmorSetFlags`, `SpecialPowerMaskType`;
`SparseMatchFinder` (best-match lookup of armor/weapon sets by flags, with a
`std::map` cache keyed by the flags; its `hash_map` cache is commented out); `Dict` (a sorted array of `NameKeyType` -> typed value, the
map-object property bag).

*Differential route:* direct. `BitFlags.cpp`, `KindOf.cpp`,
`ObjectStatusTypes.cpp`, `DisabledTypes.cpp` and `Dict.cpp` have no client
includes; their xfer goes through the Phase 0 `Xfer` C++.

### 4. Static data from INI (8,802 lines)

The template stores that INI fills before a game starts: `Geometry`
(`GeometryInfo`, footprint and bounding radius arithmetic), `TerrainTypes`,
`Science`, `Upgrade` (templates and masks), `Money`, `Handicap`, `RankInfo`,
`Armor`, `Damage` (`DamageInfo`), `DamageFX`, `MultiplayerSettings`,
`GlobalData` (the logic fields; 1,801 lines, half of them client settings),
`PlayerTemplate`, `SpecialPower` (templates), `CrateSystem`, `BezierSegment`.
This layer finishes the INI block handlers of Phase 0 for these blocks.

*Cut points:* `Upgrade`, `PlayerTemplate` and `ThingTemplate` look up button
images (`TheMappedImageCollection`, 21 references), `DamageFX` and others
name `FXList`s. Logic stores these as **names** (or `NameKeyType`s) and never
resolves them; the client resolves them later. `Money::deposit/withdraw` call
`TheAudio` (4 references) for the cash sound: a client event.

*Differential route:* direct, through INI text. Feed the same INI blocks to
both sides and compare the parsed store field by field (the INI build already
links `INI.cpp`). 20 upward includes, all of them `Player.h`/`ThingTemplate.h`
for methods that take a player (they move to layer 7 or take the player's
fields as arguments).

### 5. Thing and module system (10,949 lines)

`Module`/`ModuleData` and the module-interface headers (`BehaviorModule`,
`BodyModule`, `CollideModule`, `ContainModule`, `CreateModule`, `DamageModule`,
`DestroyModule`, `DieModule`, `UpdateModule` with its sleepy-update phases,
`UpgradeModule`, `SpecialPowerModule`, `SpecialPowerUpdateModule`),
`ModuleFactory` (the registry: **205 registrations**, which is why it has 238
upward includes: it names every module), `ThingTemplate` (2,393 lines: the
INI `Object` block), `ThingFactory`, `Thing` (position/orientation via
`Matrix3D`), `ObjectIter`, `StateMachine` (the generic state machine AI and
some modules derive from).

*Rust shape:* modules become **enums** (one variant per module class, each
holding its data and module data), dispatched by `match`, so there are no
trait objects and dispatch order is explicit. `ModuleFactory` becomes the
name -> variant constructor table. It is filled in as layer 9 lands; a module
name that is not translated yet is an **error** at template load, never a
silent skip.

*Cut points:* `ThingTemplate` holds client fields (draw modules, shadows,
audio, button images, `FXList`s): stored as data, never interpreted by logic.
`ThingFactory::newObject` also creates the `Drawable` (`TheGameClient`): that
becomes an event "object created" on the client interface.

*Differential route:* `ThingTemplate` parsing direct through INI text,
comparing parsed fields; `ModuleFactory` by the list of registered names.
`StateMachine.cpp` needs an `Object` stand-in.

### 6. Map data and terrain (9,744 lines)

`MapObject` (map objects with their `Dict`), `SidesList` (players and teams of
a map, build lists), `PolygonTrigger` (trigger areas: point-in-polygon tests),
`TerrainLogic` (height map, bridges, water levels, waypoints; 3,417 lines),
`Scripts` (the script/condition/action data model, parsed from map chunks; not
the engine).

*Cut points:* `TerrainLogic` calls `TheTerrainVisual` 32 times. Most are
writes (`setRawMapHeight` ~~20~~ **19** (**CORRECTED 2026-10-18**: the 20th grep hit is a comment), bibs, water transforms: client notifications).
**Three are reads that feed logic**: `getWaterGridHeight` (TerrainLogic.cpp
2164, 2222), `getWaterTransform`/`getWaterGridResolution` (2316, 2502, 2515)
and `getRawMapHeight` (2899). Upstream the water grid lives in the client.
For lockstep the water grid state must move into logic (or be proved
irrelevant on shipped maps); it cannot be read from a renderer.

*Differential route:* direct for `PolygonTrigger.cpp` and `SidesList.cpp`
through the Phase 0 `DataChunk` C++; `TerrainLogic.cpp` with a
`TerrainVisual` stand-in that records calls and answers reads from a table.

### 7. Players, teams and economy (18,084 lines)

`Player` (5,387 lines), `PlayerList`, `Team` (teams, prototypes, relations),
`Energy` (power), `ScoreKeeper`, `MissionStats`, `AcademyStats`,
`ResourceGatheringManager`, `ProductionPrerequisite`, `TunnelTracker`,
`CaveSystem`, `ActionManager` (the can-X-do-Y rules), `BuildAssistant`
(placement legality).

*Cut points:* `Player` touches `TheControlBar` 16, `TheEva` 10, `TheAudio` 8,
`TheInGameUI` 4, `TheRadar` 4, `TheGameText` 3: all notifications (UI dirty,
voice events, radar events), one interface. `Player.cpp:3155` reads
`TheInGameUI->isClientQuiet()`, local UI state, only to decide whether to
play a sound. `BuildAssistant.cpp:502` reads `TheTerrainVisual->getTerrainTile`
for placement: a logic read from the client, same treatment as the water grid.
`PlayerList` reads `TheNetwork` (2) for slot data: comes from the game setup
instead.

*Upward includes (67):* `Player`/`Team`/`ActionManager` call into `Object`,
`AIUpdate`, `PartitionManager`, docks, production. In Rust they take the world
(`&mut` arenas of objects/players/teams) as an argument; a method whose body
is mostly higher-layer calls is translated with that higher layer.

### 8. Object core, weapons, movement, partition (27,248 lines)

`Object` (7,250 lines), `ObjectTypes`, `ExperienceTracker`, `FiringTracker`,
`Weapon` (4,420 lines: templates, firing, projectiles, damage application),
`WeaponSet`, `LocomotorSet`, `Locomotor` (3,318 lines: movement physics, a
heavy float user), `PartitionManager` (7,454 lines: the spatial grid,
shroud/visibility, distance queries, `iterateObjectsInRange` with its sort
modes), `SimpleObjectIterator`, `ObjectCreationList`.

*Rust shape:* objects live in an arena keyed by `ObjectID` (upstream already
uses ids for anything persistent: `findObjectByID` is called **373 times**);
pointers between objects become ids. The object list order of upstream
(`m_objList`, newest first) is reproduced exactly, because update order and
iteration results depend on it.

*Cut points:* `Object` references `TheAudio` 17, `TheRadar` 12,
`TheInGameUI` 3, `TheControlBar` 2, `TheEva` 2: notifications. Its
`Drawable` pointer becomes a client-side mapping from `ObjectID`.
`FiringTracker.cpp:166` reads `TheAudio->isCurrentlyPlaying` to restart a
looping sound only (client-only effect). `PartitionManager` uses `TheDisplay`
5 (debug drawing) and `TheGhostObjectManager` 2 (fog-of-war ghosts: client).

*Differential route:* `Weapon`/`Locomotor` maths and `PartitionManager` queries
are the highest-value comparisons in Phase 1 (float-heavy). They need
`Object` stand-ins on the C++ side, or a C++ build of the whole of layers 1-8
with client stand-ins. Which is cheaper is decided when layer 7 is done.

### 9. Object modules (91,463 lines, 197 units)

Everything under `Object/Behavior`, `Body`, `Collide` (and `CrateCollide`),
`Contain`, `Create`, `Damage`, `Destroy`, `Die`, `Helper`, `SpecialPower`,
`Update` (except AI), `Upgrade`. The largest: `SpecialAbilityUpdate` 2,367,
`GarrisonContain` 2,310, `PhysicsUpdate` 2,232, `OpenContain` 2,114,
`FlightDeckBehavior` 1,960, `ActiveBody` 1,833. Each module is a variant of
its layer-5 enum and registers in the module table.

*Cut points:* 121 of 197 units touch the client; most is FX/particles/audio
on events. Reads: `TheGameClient->findDrawableByID` in `GarrisonContain`,
`PrisonBehavior`, `ChinookAIUpdate`, `ParticleUplinkCannonUpdate` (8),
`LaserUpdate`, and `TheParticleSystemManager->findParticleSystem` in
`ActiveBody`, `FirestormDynamicGeometryInfoUpdate`, `WaveGuideUpdate`,
`BoneFXUpdate`, `LaserUpdate`: they move a client object; logic emits the
request with the id and never reads back. `ActiveBody.cpp:1441-1453` and
`ProductionUpdate.cpp:958` read the local selection (`TheInGameUI`) to refresh
the UI: client-side.

*Upward includes (261):* 98 to `GameLogic.h` (`getFrame`, `findObjectByID`,
`destroyObject`: the world context), 72 to `AIUpdate.h`, 23 to `AIPathfind.h`,
19 to `ScriptEngine.h`. Modules that are mostly AI calls are translated after
layer 10.

*Differential route:* per module, through a C++ build of layers 1-8 with
stand-ins; or, where that build is not yet available, against upstream logic
with hand-checked cases (stated per module).

### 10. AI and pathfinding (63,776 lines)

`AIPathfind` (12,108 lines: the A* pathfinder over the cell grid, path
optimisation; the biggest float/integer hotspot), `AIStates` (7,484),
`AIUpdate` (6,110) and its 15 subclasses under `Update/AIUpdate/`
(`JetAIUpdate` 2,818, `DozerAIUpdate` 2,814, ...), `AIPlayer` and
`AISkirmishPlayer` (computer opponents), `AIGroup`, `AI.cpp` (`TheAI`), the
guard machines (`AIGuard`, `AIGuardRetaliate`, `AITNGuard`), `AIDock`,
`TurretAI`, `Squad`, `AIStateMachine.h`.

*Cut points:* few; `AIPlayer` reads `TheTerrainVisual` 6 times (to be checked
like layer 6), `AIGroup` and `AIStates` notify the control bar.

*Differential route:* the pathfinder is self-contained enough to compare on
generated cell maps (start/goal pairs -> identical paths, cell by cell).

### 11. Map scripts and victory (22,463 lines)

`ScriptEngine` (10,859 lines: evaluation, timers, counters, flags, named
objects, attack priorities), `ScriptActions` (7,966), `ScriptConditions`
(3,185), `VictoryConditions`.

*Cut points:* the heaviest client user after `GameLogic.cpp`:
`ScriptActions` calls `TheTacticalView` 59 times (camera), `TheControlBar` 23,
`TheInGameUI` 22, `TheAudio` 16, `TheRadar` 8, `TheDisplay` 8. **Reads that
change logic**: `ScriptConditions.cpp:2755` and `GameLogic.cpp:3627` read
`TheTacticalView->isCameraMovementFinished()` (scripted cinematics freeze
logic time until the camera arrives), `ScriptConditions.cpp:2691` reads
`TheAudio->hasMusicTrackCompleted`, `ScriptEngine.cpp:7304, 7339` take
speech/sound lengths from `TheAudio->getAudioLengthMS` to time script waits,
`ScriptConditions.cpp:1408-1436` read the local selection. These are
single-player behaviours upstream; headless they need defined answers (camera
moves finish after their scripted duration; audio lengths come from the audio
data table, not a device). Each answer is documented where it is defined.

### 12. Game loop, commands, replay, save (13,909 lines)

`GameLogic` (5,529 lines: `update()`, object registration and destruction,
the sleepy-update heap, `getCRC`), `GameLogicDispatch` (2,016: executes
`GameMessage` commands), `MessageStream` (`GameMessage`, `CommandList`),
`Recorder` (replay files), `GameState`/`GameStateMap` (save games).

`GameLogic::update()` (GameLogic.cpp 3572-3830) is, in order: `setFPMode`;
start a pending game; `TheGameClient->setFrame`; `TheScriptEngine->UPDATE`;
the time-freeze check (camera, above); `TheTerrainLogic->UPDATE`; the logic
CRC every `getCRCInterval()` frames in multiplayer or every
`REPLAY_CRC_INTERVAL` in solo/replay, appended as `MSG_LOGIC_CRC`;
`TheStatsCollector`; `TheRecorder->UPDATE`; `processCommandList`; the sleepy
updates (a heap ordered by next call frame; skipped when the object is
disabled unless the module processes that disable type); `TheAI`,
`TheBuildAssistant`, `ThePartitionManager` updates; `processDestroyList`;
`TheWeaponStore`, `TheLocomotorStore`, `TheVictoryConditions` updates;
`checkDisabledStatus` over the object list; `m_frame++`.

*Cut points:* `GameLogic.cpp` itself: `TheRecorder` 20, `TheTacticalView` 13,
`TheNetwork` 12, `TheGameClient` 10, `TheInGameUI` 8, plus the load screen,
shell and GameSpy includes. Load screens, shell, GameSpy and LAN go; the
network becomes the command source; the recorder becomes the replay source.

*Gate:* Phase 1 gate (a), identical per-frame CRCs native and wasm, needs this
layer; gate (b), original replays, needs `Recorder` and every layer below.

### C. Client, device and shell side (12,563 lines, not translated in Phase 1)

Audio (`GameAudio`, `AudioEventRTS`, `GameSounds`, `GameMusic`, the audio
headers), user preferences, `GameLOD`, `Radar` (the minimap and radar events:
logic only notifies it), `GhostObject` (fog-of-war ghosts), `DrawModule`,
`ClientUpdateModule`, `StatsCollector`, `GameEngine` (the frame driver),
`FunctionLexicon` (GUI callback names), `MapReaderWriterInfo` (editor),
`Version`, `QuotedPrintable`. Logic reaches them only through the client
interface below.

### R. Replaced, not translated (10,909 lines)

`GameMemory` (pools: Rust ownership), `Debug`, `StackDump`, `SystemInfo`,
`CriticalSection` (logic is single-threaded), `PerfTimer`, `PerfMetrics`,
`UnitTimings`, `MiniLog`, `CRCDebug` (debug-build CRC logging),
`CopyProtection`, `CDManager`, `Registry`, `CommandLine`, `List` (`LList`, an
intrusive list; owned `Vec`s instead).

## Cut points: the client interface

Logic never calls the client directly. Every call above goes through one
interface that logic owns and the client implements later:

- **Events out (no return value).** Audio events (`addAudioEvent` 142 calls,
  `removeAudioEvent` 61), FX lists and particle systems
  (`createParticleSystem` 35, `findTemplate` 28, ...), UI notifications
  (`TheInGameUI->message` 34, `addFloatingText` 22, `TheControlBar->markUIDirty`
  19, `TheEva->setShouldPlay` 48), radar events (`tryInfiltrationEvent` 12,
  `createEvent` 9), drawable lifecycle (`destroyDrawable` 19, created with
  objects), terrain visual updates (`setRawMapHeight` 20, `addFactionBib` 9),
  camera commands from scripts (`TheTacticalView`, 60+ distinct calls).
- **Shape.** An enum of events (`ClientEvent::AudioEvent { .. }`, ...) pushed
  onto a per-frame queue that logic owns, so a headless run simply drops or
  records them and tests can compare the queue. No trait objects; the client
  drains the queue after each logic frame.
- **Reads back (the ones that can change logic).** Listed above: water grid
  height and transform, raw map height, terrain tile, camera-movement-finished
  and time-frozen, audio lengths and music-track completion, local selection,
  `TheNetwork` player state. Each becomes either logic-owned state (water,
  terrain tiles, audio lengths from data) or a value from the game setup
  (network), with the headless default documented at its definition.
- **Measured call counts** (`grep -ohE "\bTheX->[A-Za-z_]+"` over
  `Source/GameLogic` and all of `Source/Common`, distinct methods called):
  `TheAudio` 46, `TheTacticalView` 46, `TheInGameUI` 33, `TheTerrainVisual`
  21, `TheControlBar` 19, `TheGameClient` 17, `TheParticleSystemManager` 15,
  `TheRadar` 15, `TheDisplay` 13, `TheNetwork` 9, `TheGhostObjectManager` 8,
  `TheEva` 2, `TheGameText` 2 (`fetch`, 91 calls: display strings, not logic).
  The call counts quoted in this section are over `Source/GameLogic`,
  `Source/Common/RTS`, `Thing`, `System` and the top-level `Source/Common/*.cpp`
  (not `Source/Common/Audio`, which is the audio system itself).

The other kind of cut is the **world context**: `TheGameLogic->getFrame()` (553
calls), `findObjectByID` (373), `destroyObject` (86), `getFirstObject` (28).
These are the services of layer 12 that every lower layer uses; in Rust they
are a context argument (`&mut World` or a frame number) instead of a global.

## Determinism hazards found while measuring

- **`hash_map` iteration.** 10 live `hash_map` typedefs in scope (one more is
  commented out), all in headers of `Player`, `Team`, `ThingFactory`,
  `DamageFX`, `Armor`, `GameLogic` (2), `SparseMatchFinder`, `GameAudio` and
  `GameSpeech.cpp`. Lookups are harmless; iteration is not. Four are iterated,
  all to write save files: `PlayerRelationMap::xfer` (Player.cpp 248),
  `TeamRelationMap::xfer` (Team.cpp 104), and `GameLogic::xfer` over the
  buildable-status and control-bar overrides (GameLogic.cpp 4925, 4965). So
  the byte order of those save blocks is STLport's hash order. The port uses
  ordered maps; a byte-compatible save writer, if ever wanted, must reproduce
  STLport's bucket order. (`SparseMatchFinder`'s cache is commented out
  upstream.)
- **Pointer-keyed maps.** `ScoreKeeper::ObjectCountMap` and
  `ScriptEngine::AttackPriorityMap` are `std::map<const ThingTemplate*, Int>`,
  ordered by **address**. `ScoreKeeper.cpp:161, 174` iterate it (score
  totals: order-independent sums); `AttackPriorityMap` is iterated only in
  debug logging. The port keys them by template id. Any new iteration found in
  translation is recorded where it occurs.
- **Signed `char` hashing.** `NameKeyGenerator`'s hash adds `Byte` (`char`,
  signed on MSVC x86) values, so bytes >= 0x80 add sign-extended values; and
  its lowercase variant passes negative values to `tolower` (undefined). The
  translation reproduces the first and treats the second as an error.
- **x87 precision.** `setFPMode()` on every logic entry (`FPUControl.h`):
  `SCOPE.md` section 3, unchanged.
- **`std::sort` with ties.** Logic calls `std::sort` in one place,
  `PartitionSolver::solve` (PartitionSolver.cpp 94, 97; used by
  `PartitionManager` and `ScriptActions`). `std::sort` is not stable, so the
  order of equal-sized entries is whatever the shipped STLport 4.x
  introsort produced; a Rust (or libstdc++) sort orders ties differently.
  Rust-to-Rust lockstep only needs one fixed algorithm, but agreement with
  original replays needs STLport's (not in the repository; the
  `stlport.diff` at its root names the files EA patched). Other logic sorts
  are hand-written (`Dict`'s Shell sort, `SimpleObjectIterator`'s list merge
  sort) and are translated as written.
  **Handled (p1w1-partition):** `common::partition_solver` uses a stable sort
  by size, largest first (ties keep input order), documented in its module
  docs; its differential test compares the C++ (libstdc++'s sort) only on
  problems with distinct sizes in both lists and counts the tied ones.
- **Found while testing layer 3** (added 2026-10-17): `XferLoad::xferAsciiString`
  reads into a function-static buffer, so two loads on different threads
  corrupt each other (the tests ran in parallel and caught it). The engine
  loads on one thread; the Rust `XferLoad` has no shared buffer.
- **The CRC of every `BitFlags`** (found in layer 3): `BitFlags::xfer` in
  CRC mode hashes `sizeof(this)`, the size of the pointer: 4 bytes on Win32,
  so only bits 0-31 of `KindOf`, `ObjectStatus`, `ModelCondition` and the
  other flag sets enter the lockstep CRC. Reproduced as shipped
  (`common::bit_flags`).

## Progress (updated with each commit of this branch)

| layer | translated so far | differential test |
|---|---|---|
| 1 | `BaseType.h` value types and templates; `GameType.h`/`.cpp`; `GameCommon.h` (all but the intrusive-list macros) and `GameCommon.cpp`; `Errors.h`; `SpecialPowerType.h`; `ThingSort.h`; `Powers.h`; `WeaponSetType.h`; `WeaponStatus.h`; `WeaponBonusConditionFlags.h`; `ObjectScriptStatusBits.h`; `DiscreteCircle`; `LatchRestore`; `Overridable`/`Override` | `tests/differential_value_types.rs`, `tests/differential_latch_override.rs` |
| 2 | `NameKeyGenerator`, `StaticNameKey`, `WellKnownKeys.h`; `AsciiString` (all but `format`); `STLTypedefs.h` (the `AsciiString` comparators; the hash functors are not translated, see `common::stl_typedefs`); `SubsystemInterface` (the registry) | `tests/differential_name_key.rs`, `tests/differential_ascii_string.rs`, `tests/differential_stl_typedefs.rs`, `tests/differential_subsystem_interface.rs` |
| 3 | `BitFlags`/`BitFlagsIO` and all seven instantiations (`KindOf`, `ObjectStatusTypes`, `DisabledTypes`, `ModelState` flags, `ArmorSet` flags, `WeaponSetFlags`, `SpecialPowerMaskType`); `SparseMatchFinder`; `Dict`; `Xfer::xferKindOf` | `tests/differential_bit_flags.rs`, `tests/differential_dict.rs` |
| 4 (part) | `Geometry` (`GeometryInfo`: arithmetic, INI fields, xfer); `BezierSegment` and `BezFwdIterator`; `Money` (client sounds as `logic::client_event::ClientEvent`, academy-stats record as the returned flag); `Handicap` | `tests/differential_geometry.rs`, `tests/differential_bezier.rs`, `tests/differential_money.rs`, `tests/differential_handicap.rs` |
| 4 (in progress) | `Armor` (`ArmorTemplate`, `ArmorStore`, the `Armor` block), `Damage` (`DamageInfo`, its xfer, `DamageTypeFlags` as `BitFlags`), `DamageFX` (templates, store, the `DamageFX` block; `FXList`s stay names) | `tests/differential_armor.rs` |
| 4 (in progress) | `TerrainTypes` (`TerrainType`, `TerrainTypeCollection`, the `Terrain` block), `MultiplayerSettings` (colours, starting money, the three blocks), `CrateSystem` (`CrateTemplate`, override chains and `reset`, the `CrateData` block). Upstream's `ScienceStore` validity check and the `Science` lookup are left out (the killer science is its name key). No method in the three files takes an `Object` or a `Player` | `tests/differential_terrain.rs` <!-- --- p1w2-terrain --- --> <!-- --- end p1w2-terrain --- --> |
<!-- --- p1w2-upgrade --- -->
| 4 (wave 2, `p1w2-upgrade`) | `Upgrade` (`UpgradeTemplate`, `UpgradeCenter` with its mask bits, `Upgrade` instances and their xfer, the `Upgrade` block, `Xfer::xferUpgradeMask` as `common::upgrade::xfer_upgrade_mask`), `SpecialPower` (`SpecialPowerTemplate` with its override chain, `SpecialPowerStore`, the `SpecialPower` block); button images and sounds stay names | `tests/differential_upgrade.rs`, `tests/differential_special_power.rs` |
<!-- --- end p1w2-upgrade --- -->
<!-- --- p1w3-statemachine --- -->
| 5 (wave 3, `p1w3-statemachine`) | `StateMachine` (`common::state_machine`): `State`/`StateMachine` as `StateBehavior` over a caller's state enum, the enter/update/exit protocol, sleeping states, success/failure successors and conditions, the exits, lock/halt/goal, `xfer`; the four utility states. `Object` and `TheGameLogic` are the `StateContext` cut points | `tests/differential_state_machine.rs` |
<!-- --- end p1w3-statemachine --- -->
<!-- --- p1w3-modules --- -->
| 5 (wave 3, `p1w3-modules`) | `Module`/`ModuleData`, the module-interface headers (`BehaviorModule`, `BodyModule`, `CollideModule`, `ContainModule` (the data only), `CreateModule`, `DamageModule`, `DestroyModule`, `DieModule`, `UpdateModule` with its sleepy-update word, `UpgradeModule` with `UpgradeMux`, `SpecialPowerModule`, `SpecialPowerUpdateModule`) as the variants of the `Module` and `ModuleDataKind` enums, and `ModuleFactory` as the name to constructor table of all 205 `init()` registrations, none translated yet | `tests/differential_module.rs` |
<!-- --- end p1w3-modules --- -->
<!-- --- p1w4-polygon --- -->
| 6 (wave 4, `p1w4-polygon`) | `PolygonTrigger` (`logic::polygon_trigger`: points, bounds, radius, `pointInTrigger`, the water and river flags, the `PolygonTriggers` chunk read (versions 1 to 4) and written; `PolygonTriggerList` holds the chain and `s_currentID`), and `MapObject` (`logic::map_object`: position, normalised angle, flags, the property `Dict`, the waypoint bit and name, the `Object` chunk reader, which fills the waypoint map). Cut points: the template is kept by name, the centre's ground height, the water extents, the writer's chunk id, the tech and supply classification | `tests/differential_polygon_trigger.rs`, `tests/differential_map_object.rs` |
<!-- --- end p1w4-polygon --- -->
<!-- --- p1w4-thingtemplate --- -->
| 5 (wave 4, `p1w4-thingtemplate`) | `ThingTemplate` (`common::thing_template`: the `Object` and `ObjectReskin` fields, override behaviour, build variations, module lists through `ModuleFactory`), `ThingFactory` (`common::thing_factory`: store, lookup, `newObject`'s template side), `ProductionPrerequisite` data, `WeaponTemplateSet`. Cut points: build cost/time and the build facility (`Player`), `getBuildable` script override, `Locomotor`/`friend_getAIModuleInfo` (layer 9), `newObject` after the template choice (layer 8), image resolution | `tests/differential_thing_template.rs` |
<!-- --- end p1w4-thingtemplate --- -->
<!-- --- p1w6-partition --- -->
| 8 (wave 6, `p1w6-partition`) | `PartitionManager.h/.cpp` whole (`logic::world::partition`, `world.partition`): grid, `PartitionData` and COI lists, `getClosestObjects` ring table, `iterate*` with the merge sort, the 33 filters, shroud counters and the unlook queue, threat and value maps, store/restore fog, line of sight, `findPositionAround`, contact list, xfer. Cut points: `TheDisplay`, `TheGhostObjectManager` (events), `Object`/terrain/players/pathfinder (host traits) | `tests/differential_partition.rs` (whole file under clang++) |
<!-- --- end p1w6-partition --- -->

Layer 4, not translated: `RankInfo` (`GameLogic/System/RankInfo.cpp`).
It needs `Overridable`/`Override` (layer 1, open), `UnicodeString` (layer 2,
open) for the rank name, `Science` for `ScienceVec`, and `parseAndTranslateLabel`
(the text tables). It is left for after those.

Findings from the layer-4 differential runs (each is now a test): upstream's
`sqr(m_height*0.5)` takes a `double` argument (the `0.5` literal), so the box
bounding sphere is computed in double; a `float` version overflowed for
heights near 1e19. `Geometry`'s random disc loop is `while (distSqr > max)`,
which a NaN leaves, so its negation is not the translation. `BezFwdIterator::start`
returns before setting the first point for one step, so a one-sample curve is
the zero point. `Bool` is one byte in the shim `BaseType.h` (the CRC and
layout tests read it as one).

Still open in layers 1-3 (after wave 1, 2026-10-18):
`AsciiString::format` (the MSVC runtime's `_vsnprintf`); `UnicodeString::format`
(`_vsnwprintf`) and `getBufferForRead`, and `iswspace` for units 0x80 and above
in the MSVC C locale (until pinned down, `trim` returns an error there; the
type holds display text, not lockstep state); the `UnicodeString` comparator
~~of `STLTypedefs.h`, now unblocked~~ **done 2026-10-18** (`p1w2-dictmove`:
`less_than_nocase<UnicodeString>` and `equal_to<UnicodeString>`, 100,000 and
50,000 comparisons, 0 mismatches, in `tests/differential_stl_typedefs.rs`);
~~moving `formats::data_chunk`'s name-keyed `Dict` stand-in onto
`common::dict` and `common::name_key_generator`~~ **done 2026-10-18**
(`p1w2-dictmove`: `readDict` and `readNameKey` take a `NameKeyGenerator` and
fill a `common::dict::Dict`; the chunk differential test gives the same
counts as before the move).
~~`UnicodeString`, `STLTypedefs.h`, `SubsystemInterface`, `Overridable`,
`LatchRestore`, `PartitionSolver`; folding `logic::damage`'s hand-written
`DamageTypeFlags` into the generic `BitFlags`~~ **done in wave 1**
(`p1w1-unicode`, `p1w1-small`, `p1w1-partition`, `p1w1-armor`, merged into
`phase1-gamelogic` 2026-10-18).

Fixed while merging wave 1: `AsciiString`'s derived `PartialEq`/`Hash` told a
string with no buffer from an empty one with a buffer; upstream's
`operator==` compares the characters only (`strcmp` of `str()`), so equality
and hashing now do too (found by the `STLTypedefs` differential test).

Layer 4 left after wave 1: ~~`RankInfo`, `Science`~~ **done in `p1w2-science`**, `TerrainTypes`, `Upgrade`, `MultiplayerSettings`,
`GlobalData`, `PlayerTemplate`, `SpecialPower`, `CrateSystem`; and the
`Object`-taking parts of `DamageFX` and the `INI::parseArmorTemplate` /
`INI::parseDamageFX` field parsers.

<!-- --- p1w2-upgrade --- -->
**CORRECTED 2026-10-18 (wave 2, `p1w2-upgrade`):** `Upgrade` and `SpecialPower`
are translated (the paragraph "Layer 4 left after wave 1" above lists them as open). What they cut, and what moves to the
layers that have players and objects (7/8), is:

- `UpgradeTemplate::calcTimeToBuild(Player*)` and `calcCostToBuild(Player*)`:
  in the shipped build neither reads the player (the `buildsInstantly()` cheat
  is debug-only; handicaps are a `@todo`), so their results are
  `time_to_build_frames()` and `cost_to_build()`; the `Player` signatures move
  up. `UpgradeCenter::canAffordUpgrade(Player*, ...)` (money check, the
  `GUI:NotEnoughMoneyToUpgrade` message) moves up, and so do granting,
  queueing and the per-player lists of `Upgrade` instances (`friend_getNext`
  and `friend_getPrev` link them into `Player`/`Object` lists).
- `SpecialPowerStore::canUseSpecialPower(Object*, ...)` (the object's module
  can run the power; the controlling player has the required science) moves up
  with `Object::getSpecialPowerModule` and `Player::hasScience`.
- `UpgradeTemplate::cacheButtonImage`/`getButtonImage` and
  `UpgradeCenter::reset` (which only calls it): the client resolves
  `ButtonImage`, which stays a name. `AudioEventRTS` fields stay names
  (`common::audio_event_name`).
- `INI::parseUpgradeTemplate` and `INI::parseSpecialPowerTemplate` (the
  fields of command buttons and modules): name lookups in the two stores,
  with their users.
- `RequiredScience`: needs `ScienceStore::friend_lookupScience`
  (`NAMEKEY` of the name, `INI_INVALID_DATA` unless it is a defined science).
  The parser takes a function saying which name keys are sciences, to be
  replaced by the translated `ScienceStore` (wave 2, `p1w2-science`); the
  `ScienceType` alias in `common::special_power` goes with it.
- `AcademyClassificationType` and its names are in
  `common::academy_classification`; the rest of `AcademyStats` (layer 7)
  should import them.

Findings from the wave-2 differential runs (all now tests): `Xfer.h` includes
`Upgrade.h`, so the real header is read while the BitFlags prelude is, before
any wrapper line (the renaming macros live in the prelude for that reason).
The `std::out_of_range` of the 129th upgrade reaches `INI::load` wrapped in an
`INIException`, as every handler's exception does. The constructor of
`SpecialPowerTemplate` leaves `m_academyClassificationType` unset (the memory
pools zero it, so `ACT_NONE`), and `getAcademyClassificationType` is the one
getter that reads the head and not the final override. A template copied from
`DefaultSpecialPower` copies the `Overridable` part too: its `m_isOverride`
(so a default made by an override load makes overrides, deleted by `reset`),
and its next-override pointer, which the copy then shares with the default;
`reset` frees that node from the default and leaves the copy dangling. The
Rust has owned chains and returns an error for that case
(`Undefined::AliasedOverrideChain`); nothing in the game's own data is known to
override `DefaultSpecialPower`. `Xfer::xferUpgradeMask` is checked by Rust
unit tests only: the Xfer build stands `TheUpgradeCenter` in with functions
that abort.
<!-- --- end p1w2-upgrade --- -->
<!-- --- p1w3-modules --- -->
**Thing and module system, the module bases (wave 3, `p1w3-modules`, 2026-10-18).**
The design of section 5 is in place for the bases: `common::module::Module` is an
enum with one variant per module class (so far the abstract bases only, each
holding its base's struct), `ModuleDataKind` the same for the data (`Arc`s
shared by the modules of one INI block), dispatch is a `match`. `ModuleFactory`
(`common::module_factory`) records all **205** registrations of `init()` in order;
`translated_module(name)` is the one place layer 9 adds an arm per class; a name
that is registered but not translated is `ModuleFactoryError::NotTranslated`,
and asking for a name that is not registered, or is one of the six under
`#ifdef ALLOW_SURRENDER`/`ALLOW_DEMORALIZE` (neither is defined anywhere), is
an error too. What the registrations measured, by `build.rs` and the tests:

- 205 calls, 199 compiled into the shipped game, 198 distinct keys
  (`WeaponBonusUpgrade` is registered twice). Each registration assigns a name
  key to `'0' + type + name` in `init()` order, so `ModuleFactory::init` assigns
  them in that order, and **every lookup assigns a key** to a name it has not
  seen (found or not): the factory functions take the `NameKeyGenerator`.
- The type that goes into the key is the class's `getModuleType()`: 201 behavior
  and 4 client-update modules, one of which is `LaserUpdate`, listed among the
  update modules. Read from the headers once (a script over `Include/`), not
  compiled.
- `UpdateModule`'s `getUpdatePhase()` is a constant of each concrete class, so the
  Rust keeps it as a field set by the constructor. Its `FIX_OLD_SAVES` block is
  compiled only under `_DEBUG`/`_INTERNAL`: not in the shipped build, not here.
- `UpgradeMuxData` computes its masks the first time anyone asks and assigns name
  keys then (`findUpgrade` calls `nameToKey`), so the same call order is part of
  lockstep state; its `mutable` members are an `RwLock`.
- `ModuleData::m_moduleTagNameKey` is never initialised upstream;
  `Module::friend_newModuleData` runs `initFromINI(data, 0)`, a null parse table
  that any field line dereferences; `TheMaxHealthChangeTypeNames` has three names
  for four enumerators and no `NULL`; `SpecialPowerModule` has two `getReadyFrame`
  overloads (the non-`const` one returns `m_availableOnFrame` as it is) and a
  `syncReadyFrameToStatusQuo` that is declared and never defined.
- Cut points for layer 8: every `Object`, `Player`, `TheGameLogic`, UI, audio and
  script-engine call is an argument, a returned value or an `UpgradeEffects`
  method (see each file's header); `ObjectModule::onCapture`, the twenty
  interface-acquisition virtuals and `friend_newModuleInstance` arrive with the
  concrete modules.

Findings from the wave-3 differential runs (all now tests): the unchanged
`UpdateModule.cpp`, `UpgradeModule.cpp` and `Module.cpp` compile under clang
with stand-ins for `Thing`, `Object`, `GameLogic`, `FXList` and `Drawable` and
two enums the headers forward-declare (`StaticGameLODLevel`, `CommandOption`,
which MSVC accepts incomplete, copied from their headers); `xferBool` stores a
raw byte in a `bool`, so a byte above 1 is not comparable.
<!-- --- end p1w3-modules --- -->
<!-- --- p1w4-polygon --- -->
**Map data, trigger areas and map objects (wave 4, `p1w4-polygon`, 2026-10-18).** Layer 6 starts with the two classes of the map file that the game reads without the terrain: `PolygonTrigger` (trigger areas and water areas) and `MapObject` (the objects placed on a map, with their property sheets). The statics of `PolygonTrigger` are the fields of `PolygonTriggerList`, and the chunk reader is generic over its context, as the layer-5 readers are. Upstream's cached bounds are recomputed by each query instead (the same values; zero-filled upstream for a trigger without points, which is what the query returns). Undefined inputs (an `Int` overflow in the box or the edges, a `Real` out of `Int` range, a version-1 id at `Int` maximum, an angle that `normalizeAngle` never reduces) are `Err`s, recorded by the parser in its context.
- `differential/wrap_polygon_trigger.cpp` includes the unchanged `PolygonTrigger.cpp`, after a build.rs copy that moves one loop declaration (MSVC's for-scope; the upstream file is never edited). Its engine singletons are renamed by macro. `differential/wrap_map_object.cpp` includes the `ParseObjectDataChunk` text cut verbatim out of `MapUtil.cpp`. The shims that changed for these builds are marked `p1w4-polygon`: the polygon writer stand-in takes `strlen` of a name, as upstream's `getLength` does (AsciiString.h line 412; the shim's own `getLength` is the `std::string` size, left as it is for the Xfer fixtures, which write NUL-containing strings in full), `MemoryPoolObject::deleteInstance` keeps its `if (this)` (GameMemory.h line 756, which GCC drops unless `-fno-delete-null-pointer-checks` is given, as the polygon build does), and the chunk `Dict` stand-in gains `getType` and `getAsciiString`.
- Results (`tests/differential_polygon_trigger.rs`, `tests/differential_map_object.rs`, 2026-10-18, upstream `0a05454d`): 6,000 polygon-trigger files (4,422 parsed, 10,590 triggers, 204,361 comparisons, 168,032 point queries), 6,000 object-chunk files (4,753 parsed, 11,876 objects, 2,087 waypoints, 145 thrown `ErrorCode`s compared by value). No mismatch. Planted bugs: five in `polygon_trigger.rs` (the crossing test's `>`, the half-height formula, the edge's `<=`, the version-2 water read, the writer's river byte), caught by the query, radius, chain and write comparisons, the version-2 water read being caught by the clean-file assertion rather than by a mismatch; four in `map_object.rs` (the z rule, the waypoint type, the waypoint's map insertion, the waypoint name), each caught.
- A harness gap found on the way and closed: an undefined read in a damaged file was silently excluded, so a misparse on the Rust side could hide. Clean files are now generated well formed and asserted never to be excluded (except water extents, an input of their own).
- Not translated: `TerrainLogic`'s ground height (a cut point), `GlobalData` (the water extents are passed in), the templates (names kept), the tech and supply classification, the snapshot `xfer` and `crc`, the editor mutators of `PolygonTrigger`, the render objects and `TheWorldDict` of `MapObject`.
<!-- --- end p1w4-polygon --- -->

<!-- --- p1w3-statemachine --- -->
**StateMachine (wave 3, branch `p1w3-statemachine`, 2026-10-18).** The shape layer 10
(`AIStateMachine`, `AIStates`) builds on: a state is a value of the caller's enum
`S: StateBehavior`, held in a `BTreeMap<StateId, _>` by the machine; a condition is a
value of the caller's enum `S::Condition` (upstream's `StateTransFuncPtr` + `void*`);
the callbacks (`on_enter`, `on_exit`, `update`, `test`) are associated functions that
take `(machine, context, state id)` and reach their own data with `machine.state_mut(id)`
at the point of use, so that a state may call `set_state` on its own machine from inside
a callback, as upstream's states do, with no aliased borrow. `S::Ctx: StateContext` is
the owner `Object` and the world.

*Cut points (every `Object`/`GameLogic` call in the two files):*
`TheGameLogic->getFrame()` -> `StateContext::frame`;
`TheGameLogic->findObjectByID()` -> `StateContext::find_object`;
`Object::getID()`/`getPosition()` in `setGoalObject` -> the `ObjectRef` argument;
`getMachineOwner()`/`getOwner()` -> the context; the owner's template name,
`TheGlobalData->m_stateMachineDebug` and `TheObjectIDToDebug` (debug logging only) -> not
translated, with the rest of the `STATE_MACHINE_DEBUG` code.

*Undefined upstream, errors in the Rust, counted:* a state id defined twice (the second
`State` is orphaned and leaked), and unbounded re-entrancy (stack overflow; the Rust stops
at `MAX_NESTING`). *Kept as upstream:* `isInForceAttackState` calls `isIdle()`; a sleep
ending exactly at frame 2^32 wraps to 0 and is no sleep; saving a machine with no current
state writes `INVALID_STATE_ID` and then heals it. *Found by the differential test:* a load
that fails inside a field leaves the C++ field partly overwritten (`fread` writes the bytes
that exist before `XFER_READ_ERROR`), the Rust typed Xfer methods (`common::xfer`, not
changed here) assign only on success; unobservable unless the failed load's machine is
inspected.
<!-- --- end p1w3-statemachine --- -->

Two translation choices await the owner, both formally undefined C++ on a
function's normal path, where MSVC's behaviour gives the intended result and
the translation follows it instead of returning an error:

- `DiscreteCircle` shifts negative `Int`s left (`(1 - radius) << 1`),
  undefined before C++20; translated as multiplication by 2, as MSVC (and
  C++20) define it (`common::discrete_circle`).
- `AsciiString::set` copies a string's own tail into its buffer with an
  overlapping `strcpy` (from `trim` and `nextToken`); translated as the
  forward copy MSVC's `strcpy` makes. glibc's `strcpy` corrupts the result,
  so the differential reference is compiled with a forward `strcpy`
  (`common::ascii_string`).

<!-- --- p1w2-globaldata --- -->
**GlobalData (wave 2, branch `p1w2-globaldata`, 2026-10-18).** `common::global_data`
(`GlobalData`, the `GameData` block, the override chain) and
`logic::weapon_bonus` (`WeaponBonus`, `WeaponBonusSet`: the `WeaponBonus =`
field needs them); differential test `tests/differential_globaldata.rs`
(336 of 336 table entries, 5,842 random cases, 11,682 loads, no mismatch).
The retail configuration only: members and `GameData` fields under
`_DEBUG`/`_INTERNAL`, `DUMP_PERF_STATS`, `ALLOW_SURRENDER` and
`_ALLOW_DEBUG_CHEATS_IN_RELEASE` do not exist, as in the shipped game. Left
out: the executable CRC, the user data directory, `GetDoubleClickTime()` (500
ms) and the user preferences applied after each parse (filesystem, registry,
devices); the version. `GlobalData` is a layer-4 unit but is read by almost
everything: the 90 members that `Source/GameLogic` and `Include/GameLogic`
name are marked on the struct and listed in `READ_BY_GAME_LOGIC`.

Findings (each is a test):

- **Awaits the owner: `newOverride` copies nothing.** Upstream's
  `*override = *TheWritableGlobalData` calls `GlobalData`'s private copy
  assignment, declared `{ DEBUG_CRASH(("unimplemented")); return *this; }`, a
  no-op in a release build. A `GameData` block loaded with
  `INI_LOAD_CREATE_OVERRIDES` (a map's) therefore starts from the constructor's
  defaults, not from the previous values, despite the function's comment. The
  translation does the same; copying instead would be a behaviour change.
- **`DrawSkyBox` is a `Real`** parsed with `parseBool` (one byte written into
  its low byte), `DebugAI` an enumeration parsed the same way, and
  `FirewallPortAllocationDelta` a `Short` parsed with `parseInt` (four bytes,
  two of them padding). Kept (little endian, as on x86).
- **`parseCoord3D`/`parseCoord2D` store component by component**, so a failing
  `Y` leaves `X` stored. `Ini::parse_coord3d`/`parse_coord2d` (Phase 0) return
  the whole value or nothing; the `GameData` table reads the components
  itself. No other table uses them yet. (`differential_ini.rs` cannot see the
  difference: it logs a value only after a successful parse.)
- `m_infantryLightScale[0]`, `m_useAlternateMouse` and `m_doubleClickAttackMove`
  are never set by the constructor (the engine's allocator zero-fills them).
- A `GameData` file naming `Wireframe`, `HealthBonus_Regular`,
  `DoubleClickTimeMS` or the prison bounty fields is an unknown field in the
  shipped game, and in the translation.
<!-- --- end p1w2-globaldata --- -->

## Differential-test routes, by rule

The `differential route` column of the unit table is derived from the
measured facts by these rules (nothing in it is a per-file guess):

- **direct**: the unit has a `.cpp`; compile it unchanged with shims, as
  Phase 0 did;
- **header**: header-only; a wrapper includes it and instantiates or calls it;
- **stand-ins for n later-layer unit(s)**: the unit includes headers of later
  layers, so the C++ build needs declaration-only stand-ins (or link stubs)
  for those classes until they are translated;
- **client stand-ins that log calls**: the unit touches the client; the
  stand-ins record each call so the test compares the call log with the Rust
  event queue;
- **none**: layers C and R.

Where a comparison turns out to be impossible, the module says so and is
tested against upstream logic with hand-checked cases instead.

## Per-unit table

Generated by `python3 -I scripts/porting_graph.py --table`; one row per unit,
sorted by layer. `level` is the measured unit level. Dependency lists are
units (lower-case basenames) and are truncated after 12 (users after 8) with
the full count in brackets; `--json` has them all.

| unit | layer | files | lines (h+cpp) | level | depends on (units) | used by (units) | client/audio/net touch | differential route |
|---|---|---|---:|---:|---|---|---|---|
| `archivefile` | D | Include/Common/ArchiveFile.h<br>Source/Common/System/ArchiveFile.cpp | 76+235 | 7 | archivefilesystem, asciistring, file | archivefilesystem | none | direct: compile unchanged with shims |
| `archivefilesystem` | D | Include/Common/ArchiveFileSystem.h<br>Source/Common/System/ArchiveFileSystem.cpp | 191+327 | 8 | archivefile, asciistring, filesystem, stltypedefs, subsysteminterface | archivefile, commandline, filesystem, gameengine | none | direct: compile unchanged with shims |
| `audiorandomvalue` | D | Include/Common/AudioRandomValue.h | 47+0 | 0 | - | - | none | header: through a wrapper that instantiates it |
| `clientrandomvalue` | D | Include/GameClient/ClientRandomValue.h | 95+0 | 0 | - | - | none | header: through a wrapper that instantiates it |
| `crc` | D | Include/Common/crc.h<br>Source/Common/crc.cpp | 127+83 | 0 | - | globaldata, randomvalue, weapon, xfercrc | none | direct: compile unchanged with shims |
| `datachunk` | D | Include/Common/DataChunk.h<br>Source/Common/System/DataChunk.cpp | 238+988 | 7 | dict, file, filesystem, mapreaderwriterinfo | gamelogic, playerlist, polygontrigger, scriptengine, scripts, sideslist, terrainlogic | none | direct: compile unchanged with shims |
| `file` | D | Include/Common/file.h<br>Source/Common/System/File.cpp | 194+258 | 2 | asciistring | archivefile, audioeventrts, datachunk, filesystem, gameengine, gamestate, gamestatemap, globaldata, ... (10) | none | direct: compile unchanged with shims |
| `filesystem` | D | Include/Common/FileSystem.h<br>Source/Common/System/FileSystem.cpp | 158+332 | 7 | archivefilesystem, file, localfilesystem, stltypedefs, subsysteminterface | archivefilesystem, audioeventrts, datachunk, gameaudio, gameengine, gamestate, gamestatemap, globaldata, ... (14) | audio GameAudio.h; refs TheAudiox2 | direct: compile unchanged with shims; client stand-ins that log calls |
| `ini` | D | Include/Common/INI.h<br>Source/Common/INI/INI.cpp | 420+1980 | 13 | armor, asciistring, damagefx, experiencetracker, file, filesystem, fpucontrol, gamecommon, iniexception, objectcreationlist, science, scriptengine, ... (20) | armor, autodepositupdate, autohealbehavior, bitflagsio, bonefxupdate, countermeasuresbehavior, cratesystem, createcratedie, ... (62) | inc Anim2D.h, Color.h, FXList.h, GameText.h, Image.h, ParticleSys.h; audio GameAudio.h; refs TheAnim2DCollectionx3, TheAudiox2, TheFXListStorex1, TheGameTextx1, TheMappedImageCollectionx2, TheParticleSystemManagerx1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `localfilesystem` | D | Include/Common/LocalFileSystem.h<br>Source/Common/System/LocalFileSystem.cpp | 58+103 | 5 | subsysteminterface | commandline, filesystem, gameengine | none | direct: compile unchanged with shims |
| `logicrandomvalue` | D | Include/GameLogic/LogicRandomValue.h | 95+0 | 0 | - | audioeventrts, physicsupdate | none | header: through a wrapper that instantiates it |
| `quicktrig` | D | Include/Common/QuickTrig.h<br>Source/Common/System/QuickTrig.cpp | 158+82 | 0 | - | tensileformationupdate | none | direct: compile unchanged with shims |
| `randomvalue` | D | Include/Common/RandomValue.h<br>Source/Common/RandomValue.cpp | 44+441 | 11 | crc, gamelogic | aistates, aiupdate, autodepositupdate, bonefxupdate, chinookaiupdate, commandbuttonhuntupdate, createcratedie, crushdie, ... (55) | none | direct: compile unchanged with shims |
| `snapshot` | D | Include/Common/Snapshot.h<br>Source/Common/System/Snapshot.cpp | 73+56 | 6 | asciistring, gamestate | academystats, ai, aipathfind, aiplayer, cavesystem, damage, energy, experiencetracker, ... (38) | none | direct: compile unchanged with shims |
| `xfer` | D | Include/Common/Xfer.h<br>Source/Common/System/Xfer.cpp | 190+869 | 8 | bitflagsio, gamestate, modelstate, science, stltypedefs, upgrade | academystats, activebody, activeshroudupgrade, ai, aigroup, aiguard, aiguardretaliate, aipathfind, ... (248) | none | direct: compile unchanged with shims |
| `xfercrc` | D | Include/Common/XferCRC.h<br>Source/Common/System/XferCRC.cpp | 74+357 | 10 | crc, snapshot, xfer, xferdeepcrc | ai, aigroup, aipathfind, aistates, aiupdate, gameengine, gamelogic, ini, ... (11) | none | direct: compile unchanged with shims |
| `xferdeepcrc` | D | Include/Common/XferDeepCRC.h | 69+0 | 9 | xfer, xfercrc | gamelogic, xfercrc | none | header: through a wrapper that instantiates it |
| `xferload` | D | Include/Common/XferLoad.h<br>Source/Common/System/XferLoad.cpp | 73+260 | 8 | gamestate, snapshot, xfer | gamestate | none | direct: compile unchanged with shims |
| `xfersave` | D | Include/Common/XferSave.h<br>Source/Common/System/XferSave.cpp | 78+358 | 8 | snapshot, xfer | gamestate | none | direct: compile unchanged with shims |
| `discretecircle` | 1 | Include/Common/DiscreteCircle.h<br>Source/Common/DiscreteCircle.cpp | 79+113 | 0 | - | partitionmanager | none | direct: compile unchanged with shims |
| `errors` | 1 | Include/Common/Errors.h | 72+0 | 0 | - | asciistring, dict, gamememory, playerlist, sideslist, statemachine, unicodestring | none | header: through a wrapper that instantiates it |
| `fpucontrol` | 1 | Include/GameLogic/FPUControl.h | 42+0 | 0 | - | gamelogic, ini | none | header: through a wrapper that instantiates it |
| `gamecommon` | 1 | Include/Common/GameCommon.h<br>Source/Common/System/GameCommon.cpp | 510+66 | 0 | - | damagefx, dumbprojectilebehavior, experiencetracker, gameengine, gamelogic, globaldata, ini, locomotorset, ... (20) | none | direct: compile unchanged with shims |
| `gametype` | 1 | Include/Common/GameType.h<br>Source/Common/System/GameType.cpp | 196+46 | 0 | - | ai, aipathfind, armorset, audioeventrts, behaviormodule, damage, drawmodule, dumbprojectilebehavior, ... (27) | none | direct: compile unchanged with shims |
| `iniexception` | 1 | Include/Common/INIException.h | 51+0 | 0 | - | gameengine, ini | none | header: through a wrapper that instantiates it |
| `latchrestore` | 1 | Include/Common/LatchRestore.h | 117+0 | 0 | - | aipathfind, gamelogic, gamestate, scriptengine | none | header: through a wrapper that instantiates it |
| `objectscriptstatusbits` | 1 | Include/GameLogic/ObjectScriptStatusBits.h | 47+0 | 0 | - | object | none | header: through a wrapper that instantiates it |
| `overridable` | 1 | Include/Common/Overridable.h | 127+0 | 0 | - | cratesystem, override, science, specialpower, thingtemplate | none | header: through a wrapper that instantiates it |
| `override` | 1 | Include/Common/Override.h | 141+0 | 1 | overridable | cratesystem, locomotor, specialpower, thing | none | header: through a wrapper that instantiates it |
| `partitionsolver` | 1 | Include/Common/PartitionSolver.h<br>Source/Common/PartitionSolver.cpp | 87+125 | 0 | - | scriptactions | none | direct: compile unchanged with shims |
| `powers` | 1 | Include/GameLogic/Powers.h | 62+0 | 0 | - | thingtemplate | none | header: through a wrapper that instantiates it |
| `specialpowertype` | 1 | Include/Common/SpecialPowerType.h | 138+0 | 0 | - | specialpower, specialpowermasktype | none | header: through a wrapper that instantiates it |
| `thingsort` | 1 | Include/Common/ThingSort.h | 81+0 | 0 | - | thingtemplate | none | header: through a wrapper that instantiates it |
| `weaponbonusconditionflags` | 1 | Include/GameLogic/WeaponBonusConditionFlags.h | 34+0 | 0 | - | containmodule, dumbprojectilebehavior, missileaiupdate, object, weapon | none | header: through a wrapper that instantiates it |
| `weaponsettype` | 1 | Include/GameLogic/WeaponSetType.h | 64+0 | 0 | - | weaponset, weaponsetflags | none | header: through a wrapper that instantiates it |
| `weaponstatus` | 1 | Include/GameLogic/WeaponStatus.h | 43+0 | 0 | - | fireweaponupdate, weapon | none | header: through a wrapper that instantiates it |
| `asciistring` | 2 | Include/Common/AsciiString.h<br>Source/Common/System/AsciiString.cpp | 657+396 | 1 | errors | archivefile, archivefilesystem, audioeventinfo, audioeventrts, audiosettings, cdmanager, crcdebug, file, ... (19) | none | direct: compile unchanged with shims |
| `namekeygenerator` | 2 | Include/Common/NameKeyGenerator.h<br>Source/Common/NameKeyGenerator.cpp | 156+240 | 5 | asciistring, subsysteminterface | armor, damagefx, dict, functionlexicon, gameengine, gamelogicdispatch, locomotor, module, ... (16) | none | direct: compile unchanged with shims |
| `stltypedefs` | 2 | Include/Common/STLTypedefs.h | 225+0 | 2 | asciistring, gamecommon, unicodestring | ai, archivefilesystem, armor, audioeventinfo, beziersegment, bitflags, buildassistant, createcratedie, ... (35) | none | header: through a wrapper that instantiates it |
| `subsysteminterface` | 2 | Include/Common/SubsystemInterface.h<br>Source/Common/System/SubsystemInterface.cpp | 174+242 | 11 | gamelogic, ini, stltypedefs, xfer | actionmanager, ai, archivefilesystem, buildassistant, cavesystem, cdmanager, filesystem, functionlexicon, ... (26) | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `unicodestring` | 2 | Include/Common/UnicodeString.h<br>Source/Common/System/UnicodeString.cpp | 453+362 | 1 | errors | academystats, debug, gamestate, oclupdate, player, playertemplate, rankinfo, science, ... (10) | none | direct: compile unchanged with shims |
| `wellknownkeys` | 2 | Include/Common/WellKnownKeys.h | 924+0 | 6 | namekeygenerator | aiplayer, aiskirmishplayer, gamelogic, gamestate, object, player, playerlist, sideslist, ... (10) | none | header: through a wrapper that instantiates it |
| `armorset` | 3 | Include/GameLogic/ArmorSet.h | 103+0 | 5 | gametype, sparsematchfinder | activebody, bitflags, bodymodule, thingtemplate | none | header: through a wrapper that instantiates it |
| `bitflags` | 3 | Include/Common/BitFlags.h<br>Source/Common/BitFlags.cpp | 333+190 | 8 | armorset, bitflagsio, modelstate, stltypedefs | bitflagsio, damage, disabledtypes, dynamicaudioeventinfo, kindof, modelstate, objectstatustypes, sparsematchfinder, ... (10) | none | direct: compile unchanged with shims |
| `bitflagsio` | 3 | Include/Common/BitFlagsIO.h | 233+0 | 8 | bitflags, ini, xfer | activebody, autohealbehavior, battleplanupdate, bitflags, costmodifierupgrade, countermeasuresbehavior, cratecollide, cratesystem, ... (26) | none | header: through a wrapper that instantiates it |
| `dict` | 3 | Include/Common/Dict.h<br>Source/Common/Dict.cpp | 368+550 | 6 | errors, namekeygenerator | datachunk, handicap, mapobject, object, sideslist | none | direct: compile unchanged with shims |
| `disabledtypes` | 3 | Include/Common/DisabledTypes.h<br>Source/Common/System/DisabledTypes.cpp | 119+60 | 8 | bitflags, bitflagsio | object, player, updatemodule | none | direct: compile unchanged with shims |
| `kindof` | 3 | Include/Common/KindOf.h<br>Source/Common/System/KindOf.cpp | 224+183 | 8 | bitflags, bitflagsio | autofindhealingupdate, battleplanupdate, checkpointupdate, cleanuphazardupdate, commandbuttonhuntupdate, costmodifierupgrade, demotrapupdate, empupdate, ... (26) | none | direct: compile unchanged with shims |
| `modelstate` | 3 | Include/Common/ModelState.h | 266+0 | 8 | bitflags, bitflagsio, ini | animationsteeringupdate, battlebusslowdeathbehavior, bitflags, drawmodule, garrisoncontain, modelconditionupgrade, objectcreationupgrade, opencontain, ... (16) | none | header: through a wrapper that instantiates it |
| `objectstatustypes` | 3 | Include/Common/ObjectStatusTypes.h<br>Source/Common/System/ObjectStatusTypes.cpp | 145+84 | 8 | bitflags, bitflagsio | damage, diemodule, gamelogic, grantupgradecreate, object, partitionmanager, scriptconditions, scripts, ... (9) | none | direct: compile unchanged with shims |
| `sparsematchfinder` | 3 | Include/Common/SparseMatchFinder.h | 224+0 | 4 | bitflags, stltypedefs | armorset, weaponset | none | header: through a wrapper that instantiates it |
| `specialpowermasktype` | 3 | Include/Common/SpecialPowerMaskType.h | 38+0 | 8 | bitflags, bitflagsio, specialpowertype | object, specialpower | none | header: through a wrapper that instantiates it |
| `weaponsetflags` | 3 | Include/GameLogic/WeaponSetFlags.h | 37+0 | 1 | weaponsettype | object, weaponset | none | header: through a wrapper that instantiates it |
| `armor` | 4 | Include/GameLogic/Armor.h<br>Source/GameLogic/Object/Armor.cpp | 134+153 | 13 | damage, ini, namekeygenerator, stltypedefs, thingfactory | activebody, gameengine, ini, thingtemplate | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `bezfwditerator` | 4 | Include/Common/BezFwdIterator.h<br>Source/Common/Bezier/BezFwdIterator.cpp | 50+124 | 4 | beziersegment | beziersegment | none | direct: compile unchanged with shims |
| `beziersegment` | 4 | Include/Common/BezierSegment.h<br>Source/Common/Bezier/BezierSegment.cpp | 73+246 | 5 | bezfwditerator, stltypedefs | bezfwditerator, dumbprojectilebehavior | none | direct: compile unchanged with shims |
| `cratesystem` | 4 | Include/GameLogic/CrateSystem.h<br>Source/GameLogic/System/CrateSystem.cpp | 116+229 | 8 | bitflagsio, ini, overridable, override | createcratedie, gameengine, gamelogic | none | direct: compile unchanged with shims |
| `damage` | 4 | Include/GameLogic/Damage.h<br>Source/GameLogic/System/Damage.cpp | 376+197 | 13 | bitflags, bitflagsio, gametype, objectstatustypes, snapshot, thingfactory, thingtemplate, xfer | activebody, ai, armor, bodymodule, containmodule, damagefx, damagemodule, diemodule, ... (29) | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `damagefx` | 4 | Include/Common/DamageFX.h<br>Source/Common/DamageFX.cpp | 169+313 | 13 | damage, gamecommon, gamelogic, ini, namekeygenerator, object, stltypedefs, thingfactory, thingtemplate | activebody, gameengine, ini, thingtemplate | inc FXList.h, GameClient.h, InGameUI.h; audio GameAudio.h | direct: compile unchanged with shims; stand-ins for 4 later-layer unit(s); client stand-ins that log calls |
| `geometry` | 4 | Include/Common/Geometry.h<br>Source/Common/System/Geometry.cpp | 199+604 | 8 | asciistring, ini, randomvalue, snapshot, xfer | dynamicgeometryinfoupdate, firestormdynamicgeometryinfoupdate, object, partitionmanager, thingtemplate | none | direct: compile unchanged with shims |
| `globaldata` | 4 | Include/Common/GlobalData.h<br>Source/Common/GlobalData.cpp | 545+1256 | 11 | ai, asciistring, bodymodule, crc, file, filesystem, gamecommon, gametype, ini, money, stltypedefs, subsysteminterface, ... (15) | actionmanager, activebody, aipathfind, aiplayer, aiskirmishplayer, aistates, aiupdate, audioeventrts, ... (41) | inc Color.h, TerrainVisual.h, FirewallHelper.h; audio GameAudio.h | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `handicap` | 4 | Include/Common/Handicap.h<br>Source/Common/RTS/Handicap.cpp | 121+126 | 14 | dict, player, thingtemplate | player, playertemplate | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `money` | 4 | Include/Common/Money.h<br>Source/Common/RTS/Money.cpp | 106+145 | 14 | player, playerlist, snapshot, xfer | aistates, dozeraiupdate, globaldata, multiplayersettings, player, playertemplate, powtruckaiupdate, statscollector, ... (9) | audio GameAudio.h, MiscAudio.h; refs TheAudiox4 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `multiplayersettings` | 4 | Include/Common/MultiplayerSettings.h<br>Source/Common/MultiplayerSettings.cpp | 147+174 | 4 | ini, money | gameengine, gamelogic, gamelogicdispatch, userpreferences | inc Color.h, GameInfo.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `playertemplate` | 4 | Include/Common/PlayerTemplate.h<br>Source/Common/RTS/PlayerTemplate.cpp | 237+418 | 14 | asciistring, gamecommon, handicap, ini, money, namekeygenerator, player, science, stltypedefs, subsysteminterface, unicodestring | academystats, gameengine, gamelogic, gamelogicdispatch, oclupdate, player, playerlist, scriptactions, ... (12) | inc Image.h; refs TheMappedImageCollectionx5 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `rankinfo` | 4 | Include/GameLogic/RankInfo.h<br>Source/GameLogic/System/RankInfo.cpp | 82+183 | 14 | ini, player, science, stltypedefs, unicodestring | gameengine, player | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `science` | 4 | Include/Common/Science.h<br>Source/Common/RTS/Science.cpp | 140+383 | 14 | ini, namekeygenerator, overridable, player, unicodestring | cashbountypower, gameengine, ini, oclspecialpower, particleuplinkcannonupdate, player, playertemplate, productionprerequisite, ... (16) | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `specialpower` | 4 | Include/Common/SpecialPower.h<br>Source/Common/RTS/SpecialPower.cpp | 195+381 | 14 | bitflags, bitflagsio, object, overridable, override, player, science, specialpowermasktype, specialpowertype, subsysteminterface | academystats, actionmanager, aigroup, aiplayer, aiskirmishplayer, commandbuttonhuntupdate, defectorspecialpower, gameengine, ... (22) | audio AudioEventRTS.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `terraintypes` | 4 | Include/Common/TerrainTypes.h<br>Source/Common/TerrainTypes.cpp | 248+165 | 5 | ini, subsysteminterface | buildassistant, gameengine | none | direct: compile unchanged with shims |
| `upgrade` | 4 | Include/Common/Upgrade.h<br>Source/Common/System/Upgrade.cpp | 269+498 | 14 | ini, player, snapshot, xfer | aigroup, aiplayer, aiupdate, armorupgrade, bunkerbusterbehavior, gameengine, gamelogicdispatch, grantupgradecreate, ... (20) | inc Image.h, InGameUI.h; audio AudioEventRTS.h; refs TheInGameUIx1, TheMappedImageCollectionx2 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `behaviormodule` | 5 | Include/GameLogic/Module/BehaviorModule.h<br>Source/GameLogic/Object/Behavior/BehaviorModule.cpp | 283+72 | 8 | gametype, module, xfer | autohealbehavior, battlebusslowdeathbehavior, bodymodule, bridgebehavior, bridgescaffoldbehavior, bridgetowerbehavior, buildassistant, bunkerbusterbehavior, ... (43) | none | direct: compile unchanged with shims |
| `bodymodule` | 5 | Include/GameLogic/Module/BodyModule.h<br>Source/GameLogic/Object/Body/BodyModule.cpp | 308+76 | 10 | armorset, behaviormodule, damage, module, xfer | actionmanager, activebody, aigroup, aiguard, aiguardretaliate, aistates, aitnguard, aiupdate, ... (71) | none | direct: compile unchanged with shims |
| `collidemodule` | 5 | Include/GameLogic/Module/CollideModule.h<br>Source/GameLogic/Object/Collide/CollideModule.cpp | 106+71 | 8 | behaviormodule, module, xfer | actionmanager, aiguard, aiguardretaliate, aitnguard, cratecollide, dumbprojectilebehavior, fireweaponcollide, minefieldbehavior, ... (15) | none | direct: compile unchanged with shims |
| `containmodule` | 5 | Include/GameLogic/Module/ContainModule.h | 214+0 | 10 | damage, module, weaponbonusconditionflags | academystats, actionmanager, activebody, ai, aigroup, aipathfind, aistates, aiupdate, ... (59) | none | header: through a wrapper that instantiates it |
| `createmodule` | 5 | Include/GameLogic/Module/CreateModule.h<br>Source/GameLogic/Object/Create/CreateModule.cpp | 89+94 | 8 | behaviormodule, module, xfer | buildassistant, cavecontain, dozeraiupdate, gamelogic, grantupgradecreate, lockweaponcreate, object, preordercreate, ... (17) | none | direct: compile unchanged with shims |
| `damagemodule` | 5 | Include/GameLogic/Module/DamageModule.h<br>Source/GameLogic/Object/Damage/DamageModule.cpp | 117+71 | 10 | behaviormodule, damage, module, xfer | activebody, autohealbehavior, baseregenerateupdate, bonefxdamage, bridgebehavior, bridgetowerbehavior, countermeasuresbehavior, fireweaponwhendamagedbehavior, ... (19) | none | direct: compile unchanged with shims |
| `destroymodule` | 5 | Include/GameLogic/Module/DestroyModule.h<br>Source/GameLogic/Object/Destroy/DestroyModule.cpp | 70+89 | 8 | behaviormodule, module, xfer | gamelogic, object | none | direct: compile unchanged with shims |
| `diemodule` | 5 | Include/GameLogic/Module/DieModule.h<br>Source/GameLogic/Object/Die/DieModule.cpp | 112+129 | 12 | behaviormodule, damage, experiencetracker, gamelogic, module, object, objectstatustypes, xfer | activebody, bridgebehavior, bridgetowerbehavior, bunkerbusterbehavior, createcratedie, createobjectdie, crushdie, damdie, ... (37) | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `module` | 5 | Include/Common/Module.h<br>Source/Common/Thing/Module.cpp | 324+334 | 12 | bodymodule, collidemodule, containmodule, damagemodule, diemodule, gamelogic, ini, namekeygenerator, object, snapshot, thing, thingtemplate, ... (16) | aidock, behaviormodule, bodymodule, clientupdatemodule, collidemodule, containmodule, converttocarbombcratecollide, converttohijackedvehiclecratecollide, ... (34) | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `modulefactory` | 5 | Include/Common/ModuleFactory.h<br>Source/Common/Thing/ModuleFactory.cpp | 131+731 | 14 | activebody, activeshroudupgrade, aiupdate, animationsteeringupdate, armorupgrade, asciistring, assaulttransportaiupdate, assistedtargetingupdate, autodepositupdate, autofindhealingupdate, autohealbehavior, baikonurlaunchpower, ... (206) | gameengine, object, thingfactory, thingtemplate | inc AnimatedParticleSysBoneClientUpdate.h, BeaconClientUpdate.h, SwayClientUpdate.h | direct: compile unchanged with shims; stand-ins for 200 later-layer unit(s); client stand-ins that log calls |
| `objectiter` | 5 | Include/GameLogic/ObjectIter.h | 161+0 | 1 | gametype | aigroup, battleplanupdate, commandbuttonhuntupdate, empupdate, gamelogicdispatch, generateminefieldbehavior, neutronmissileslowdeathupdate, partitionmanager, ... (10) | none | header: through a wrapper that instantiates it |
| `specialpowermodule` | 5 | Include/GameLogic/Module/SpecialPowerModule.h<br>Source/GameLogic/Object/SpecialPower/SpecialPowerModule.cpp | 184+877 | 14 | behaviormodule, deletionupdate, gamelogic, globaldata, ini, module, object, player, playerlist, science, scriptengine, specialpower, ... (17) | actionmanager, aigroup, baikonurlaunchpower, battleplanupdate, cashbountypower, cashhackspecialpower, cleanupareapower, defectorspecialpower, ... (26) | inc ControlBar.h, Eva.h, InGameUI.h; audio AudioEventRTS.h, GameAudio.h; refs TheAudiox2, TheEvax15, TheInGameUIx4 | direct: compile unchanged with shims; stand-ins for 6 later-layer unit(s); client stand-ins that log calls |
| `specialpowerupdatemodule` | 5 | Include/GameLogic/Module/SpecialPowerUpdateModule.h<br>Source/GameLogic/Object/Update/SpecialPowerUpdateModule.cpp | 84+121 | 14 | gamelogic, gametype, module, object, player, playerlist, science, specialpower, specialpowermodule, xfer | aigroup, battleplanupdate, missilelauncherbuildingupdate, particleuplinkcannonupdate, specialabilityupdate, specialpowermodule, spectregunshipdeploymentupdate, spectregunshipupdate | none | direct: compile unchanged with shims; stand-ins for 4 later-layer unit(s) |
| `statemachine` | 5 | Include/Common/StateMachine.h<br>Source/Common/StateMachine.cpp | 478+901 | 12 | errors, gamelogic, gamestate, gametype, globaldata, modelstate, object, snapshot, thingtemplate, xfer | aistatemachine, assaulttransportaiupdate, deliverpayloadaiupdate, deploystyleaiupdate, dozeraiupdate, hackinternetaiupdate, supplytruckaiupdate, turretai, ... (9) | none | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s) |
| `thing` | 5 | Include/Common/Thing.h<br>Source/Common/Thing/Thing.cpp | 199+392 | 14 | globaldata, kindof, namekeygenerator, override, player, playerlist, team, terrainlogic, thingfactory, thingtemplate | activebody, autodepositupdate, autohealbehavior, bonefxupdate, countermeasuresbehavior, empupdate, fireweaponwhendamagedbehavior, fireweaponwhendeadbehavior, ... (20) | none | direct: compile unchanged with shims; stand-ins for 4 later-layer unit(s) |
| `thingfactory` | 5 | Include/Common/ThingFactory.h<br>Source/Common/Thing/ThingFactory.cpp | 140+550 | 14 | asciistring, createmodule, filesystem, gamelogic, ini, mapobject, modulefactory, object, partitionmanager, player, playerlist, productionprerequisite, ... (16) | aipathfind, aiplayer, aiskirmishplayer, aistates, aiupdate, armor, assaulttransportaiupdate, assistedtargetingupdate, ... (73) | inc Drawable.h, GameClient.h; audio GameAudio.h; refs TheGameClientx1 | direct: compile unchanged with shims; stand-ins for 7 later-layer unit(s); client stand-ins that log calls |
| `thingtemplate` | 5 | Include/Common/ThingTemplate.h<br>Source/Common/Thing/ThingTemplate.cpp | 780+1613 | 14 | aiupdate, armor, armorset, bitflagsio, damagefx, filesystem, gamecommon, geometry, globaldata, ini, kindof, messagestream, ... (30) | academystats, actionmanager, activebody, ai, aigroup, aiguard, aiguardretaliate, aipathfind, ... (138) | inc Color.h, Drawable.h, FXList.h, GameText.h, Image.h, Shadow.h; audio AudioEventRTS.h, GameAudio.h; refs TheAudiox2, TheFXListStorex1, TheMappedImageCollectionx3 | direct: compile unchanged with shims; stand-ins for 8 later-layer unit(s); client stand-ins that log calls |
| `updatemodule` | 5 | Include/GameLogic/Module/UpdateModule.h<br>Source/GameLogic/Object/Update/UpdateModule.cpp | 369+202 | 11 | behaviormodule, disabledtypes, gamelogic, gametype, module, xfer | aidock, aiplayer, aiskirmishplayer, aiupdate, animationsteeringupdate, assistedtargetingupdate, autodepositupdate, autofindhealingupdate, ... (87) | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `upgrademodule` | 5 | Include/GameLogic/Module/UpgradeModule.h<br>Source/GameLogic/Object/Upgrade/UpgradeModule.cpp | 218+259 | 8 | behaviormodule, module, stltypedefs, upgrade, xfer | activeshroudupgrade, armorupgrade, autohealbehavior, commandsetupgrade, costmodifierupgrade, countermeasuresbehavior, experiencescalarupgrade, fireoclafterweaponcooldownupdate, ... (31) | inc Drawable.h, FXList.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `mapobject` | 6 | Include/Common/MapObject.h | 188+0 | 7 | dict | buildassistant, gamelogic, gamestate, partitionmanager, polygontrigger, scriptactions, scriptconditions, scriptengine, ... (10) | inc TerrainRoads.h | header: through a wrapper that instantiates it; client stand-ins that log calls |
| `polygontrigger` | 6 | Include/GameLogic/PolygonTrigger.h<br>Source/GameLogic/Map/PolygonTrigger.cpp | 157+559 | 8 | datachunk, mapobject, mapreaderwriterinfo, snapshot, stltypedefs, terrainlogic, xfer | aiguard, aiguardretaliate, aistates, aitnguard, aiupdate, gamelogic, object, partitionmanager, ... (12) | none | direct: compile unchanged with shims |
| `scripts` | 6 | Include/GameLogic/Scripts.h<br>Source/GameLogic/ScriptEngine/Scripts.cpp | 1141+2718 | 14 | ai, bordercolors, containmodule, datachunk, gamestate, kindof, object, objectstatustypes, player, radar, scriptengine, sideslist, ... (15) | gamelogic, player, scriptconditions, scriptengine, sideslist, terrainlogic | inc ShellHooks.h, NetworkDefs.h | direct: compile unchanged with shims; stand-ins for 5 later-layer unit(s); client stand-ins that log calls |
| `sideslist` | 6 | Include/GameLogic/SidesList.h<br>Source/GameLogic/Map/SidesList.cpp | 396+1168 | 11 | ai, datachunk, dict, errors, gamestate, gametype, playertemplate, scripts, snapshot, stltypedefs, wellknownkeys, xfer | ai, aiplayer, aiskirmishplayer, gameengine, gamelogic, gamestate, player, playerlist, ... (12) | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `terrainlogic` | 6 | Include/GameLogic/TerrainLogic.h<br>Source/GameLogic/Map/TerrainLogic.cpp | 383+3034 | 13 | ai, aipathfind, bodymodule, bridgebehavior, bridgetowerbehavior, damage, datachunk, gamelogic, gamestate, ghostobject, mapobject, object, ... (23) | activebody, aipathfind, aiplayer, aiskirmishplayer, aistatemachine, battleplanupdate, bridgebehavior, bridgetowerbehavior, ... (32) | inc TerrainRoads.h, TerrainVisual.h; refs TheGhostObjectManagerx4, TheRadarx1, TheTacticalViewx1, TheTerrainVisualx32 | direct: compile unchanged with shims; stand-ins for 8 later-layer unit(s); client stand-ins that log calls |
| `academystats` | 7 | Include/Common/AcademyStats.h<br>Source/Common/RTS/AcademyStats.cpp | 280+1269 | 14 | containmodule, energy, gamelogic, object, player, playerlist, playertemplate, snapshot, specialpower, thingtemplate, unicodestring, xfer | player | inc ControlBar.h, GameText.h; refs TheControlBarx1, TheGameTextx31 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `actionmanager` | 7 | Include/Common/ActionManager.h<br>Source/Common/RTS/ActionManager.cpp | 115+2093 | 14 | bodymodule, collidemodule, containmodule, dozeraiupdate, experiencetracker, globaldata, object, partitionmanager, player, playerlist, railedtransportdockupdate, railroadguideaiupdate, ... (23) | aigroup, aistates, aiupdate, chinookaiupdate, commandbuttonhuntupdate, dozeraiupdate, gameengine, jetaiupdate, ... (14) | inc Drawable.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 12 later-layer unit(s); client stand-ins that log calls |
| `buildassistant` | 7 | Include/Common/BuildAssistant.h<br>Source/Common/System/BuildAssistant.cpp | 220+1604 | 14 | ai, aipathfind, aiupdate, behaviormodule, containmodule, createmodule, globaldata, mapobject, object, parkingplacebehavior, partitionmanager, player, ... (20) | aigroup, aiplayer, aiskirmishplayer, autodepositupdate, dozeraiupdate, gameengine, gamelogic, gamelogicdispatch, ... (12) | inc ControlBar.h, Drawable.h, InGameUI.h, TerrainVisual.h; audio GameAudio.h; refs TheAudiox1, TheControlBarx1, TheTerrainVisualx11 | direct: compile unchanged with shims; stand-ins for 7 later-layer unit(s); client stand-ins that log calls |
| `cavesystem` | 7 | Include/GameLogic/CaveSystem.h<br>Source/GameLogic/System/CaveSystem.cpp | 75+196 | 12 | gamestate, snapshot, subsysteminterface, tunneltracker, xfer | cavecontain, gameengine, gamelogic | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `energy` | 7 | Include/Common/Energy.h<br>Source/Common/RTS/Energy.cpp | 127+304 | 14 | gamelogic, object, player, playerlist, snapshot, thingtemplate, xfer | academystats, player | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `missionstats` | 7 | Include/Common/MissionStats.h<br>Source/Common/RTS/MissionStats.cpp | 90+113 | 14 | gamecommon, player, snapshot, xfer | player | none | direct: compile unchanged with shims |
| `player` | 7 | Include/Common/Player.h<br>Source/Common/RTS/Player.cpp | 830+4557 | 13 | academystats, actionmanager, ai, aipathfind, aiskirmishplayer, aiupdate, autodepositupdate, battleplanupdate, bitflagsio, buildassistant, containmodule, disabledtypes, ... (54) | academystats, actionmanager, activebody, ai, aidock, aigroup, aipathfind, aiplayer, ... (153) | inc ControlBar.h, Drawable.h, Eva.h, GameClient.h, GameText.h, GameInfo.h; audio MiscAudio.h; refs TheAudiox8, TheControlBarx16, TheEvax10, TheGameClientx2, TheGameTextx3, TheInGameUIx4, TheMessageStreamx1, TheRadarx4 | direct: compile unchanged with shims; stand-ins for 18 later-layer unit(s); client stand-ins that log calls |
| `playerlist` | 7 | Include/Common/PlayerList.h<br>Source/Common/RTS/PlayerList.cpp | 175+493 | 14 | datachunk, errors, gamecommon, gamestate, globaldata, namekeygenerator, object, player, playertemplate, sideslist, snapshot, subsysteminterface, ... (15) | academystats, actionmanager, activebody, ai, aiplayer, aiskirmishplayer, aistates, aiupdate, ... (63) | inc NetworkDefs.h; refs TheNetworkx2 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `productionprerequisite` | 7 | Include/Common/ProductionPrerequisite.h<br>Source/Common/RTS/ProductionPrerequisite.cpp | 125+321 | 14 | gamecommon, object, player, science, thingfactory, thingtemplate | player, thingfactory, thingtemplate | inc Drawable.h, GameText.h; refs TheGameTextx2 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `resourcegatheringmanager` | 7 | Include/Common/ResourceGatheringManager.h<br>Source/Common/RTS/ResourceGatheringManager.cpp | 75+291 | 13 | actionmanager, gamelogic, gametype, object, partitionmanager, snapshot, supplycenterdockupdate, supplytruckaiupdate, supplywarehousedockupdate, updatemodule, xfer | player, supplycentercreate, supplytruckaiupdate, supplywarehousecreate, workeraiupdate | none | direct: compile unchanged with shims; stand-ins for 6 later-layer unit(s) |
| `scorekeeper` | 7 | Include/Common/ScoreKeeper.h<br>Source/Common/RTS/ScoreKeeper.cpp | 145+568 | 14 | gamelogic, gamestate, kindof, object, player, snapshot, thingfactory, thingtemplate, xfer | player | none | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s) |
| `team` | 7 | Include/Common/Team.h<br>Source/Common/RTS/Team.cpp | 760+2755 | 14 | aiupdate, bodymodule, containmodule, gamestate, gametype, object, partitionmanager, player, playerlist, playertemplate, polygontrigger, scriptactions, ... (20) | actionmanager, activebody, aiguard, aiguardretaliate, aiplayer, aiskirmishplayer, aistates, aitnguard, ... (33) | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 6 later-layer unit(s); client stand-ins that log calls |
| `tunneltracker` | 7 | Include/Common/TunnelTracker.h<br>Source/Common/RTS/TunnelTracker.cpp | 91+412 | 14 | ai, aipathfind, bodymodule, containmodule, gamelogic, gamestate, gametype, globaldata, kindof, object, partitionmanager, snapshot, ... (14) | aitnguard, cavecontain, cavesystem, player, tunnelcontain | inc ControlBar.h, Drawable.h | direct: compile unchanged with shims; stand-ins for 7 later-layer unit(s); client stand-ins that log calls |
| `experiencetracker` | 8 | Include/GameLogic/ExperienceTracker.h<br>Source/GameLogic/Object/ExperienceTracker.cpp | 77+276 | 12 | gamecommon, gamelogic, gametype, object, snapshot, thingtemplate, xfer | actionmanager, assaulttransportaiupdate, converttocarbombcratecollide, converttohijackedvehiclecratecollide, createcratedie, deploystyleaiupdate, diemodule, ejectpilotdie, ... (35) | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `firingtracker` | 8 | Include/GameLogic/FiringTracker.h<br>Source/GameLogic/Object/FiringTracker.cpp | 96+395 | 12 | gamelogic, gametype, object, objecthelper, thingtemplate, updatemodule, weapon, xfer | object | audio AudioEventRTS.h, AudioHandleSpecialValues.h, GameAudio.h; refs TheAudiox6 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `locomotor` | 8 | Include/GameLogic/Locomotor.h<br>Source/GameLogic/Object/Locomotor.cpp | 510+2808 | 12 | ai, aipathfind, aiupdate, bodymodule, damage, gamelogic, ini, locomotorset, namekeygenerator, object, override, partitionmanager, ... (14) | aigroup, aipathfind, aistates, aiupdate, chinookaiupdate, deliverpayloadaiupdate, deploystyleaiupdate, dozeraiupdate, ... (28) | none | direct: compile unchanged with shims; stand-ins for 5 later-layer unit(s) |
| `locomotorset` | 8 | Include/GameLogic/LocomotorSet.h | 114+0 | 3 | gamecommon, snapshot, stltypedefs | aipathfind, aiupdate, locomotor | none | header: through a wrapper that instantiates it |
| `object` | 8 | Include/GameLogic/Object.h<br>Source/GameLogic/Object/Object.cpp | 829+6421 | 14 | ai, aipathfind, aiupdate, autohealbehavior, behaviormodule, bitflagsio, bodymodule, buildassistant, collidemodule, containmodule, countermeasuresbehavior, createmodule, ... (74) | academystats, actionmanager, activebody, activeshroudupgrade, aidock, aiguard, aiguardretaliate, aipathfind, ... (236) | inc Anim2D.h, Color.h, ControlBar.h, Drawable.h, Eva.h, GameClient.h, InGameUI.h; audio AudioEventInfo.h, DynamicAudioEventInfo.h, MiscAudio.h; refs TheAnim2DCollectionx2, TheAudiox17, TheControlBarx2, TheEvax2, TheGameClientx1, TheInGameUIx3, TheRadarx12 | direct: compile unchanged with shims; stand-ins for 30 later-layer unit(s); client stand-ins that log calls |
| `objectcreationlist` | 8 | Include/GameLogic/ObjectCreationList.h<br>Source/GameLogic/Object/ObjectCreationList.cpp | 224+1635 | 14 | aipathfind, bodymodule, containmodule, deliverpayloadaiupdate, drawmodule, experiencetracker, floatupdate, gamelod, gamelogic, globaldata, ini, lifetimeupdate, ... (25) | battlebusslowdeathbehavior, bonefxupdate, bridgebehavior, createobjectdie, ejectpilotdie, fireoclafterweaponcooldownupdate, firespreadupdate, fireweaponwhendamagedbehavior, ... (25) | inc Drawable.h, FXList.h, ParticleSys.h, Shadow.h; audio AudioEventRTS.h; refs TheAudiox2, TheParticleSystemManagerx2 | direct: compile unchanged with shims; stand-ins for 9 later-layer unit(s); client stand-ins that log calls |
| `objecttypes` | 8 | Include/GameLogic/ObjectTypes.h<br>Source/GameLogic/Object/ObjectTypes.cpp | 90+208 | 14 | asciistring, gamestate, player, snapshot, thingfactory, thingtemplate, xfer | scriptactions, scriptconditions, scriptengine | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `partitionmanager` | 8 | Include/GameLogic/PartitionManager.h<br>Source/GameLogic/Object/PartitionManager.cpp | 1601+5853 | 14 | actionmanager, aipathfind, aiupdate, bodymodule, collidemodule, containmodule, discretecircle, gamecommon, gameengine, gamelogic, gamestate, geometry, ... (30) | actionmanager, activebody, ai, aiguard, aiguardretaliate, aipathfind, aiplayer, aiskirmishplayer, ... (99) | inc ControlBar.h, Display.h, Line2D.h; refs TheDisplayx5, TheGhostObjectManagerx2, TheRadarx5 | direct: compile unchanged with shims; stand-ins for 7 later-layer unit(s); client stand-ins that log calls |
| `simpleobjectiterator` | 8 | Source/GameLogic/Object/SimpleObjectIterator.cpp | 0+273 | 12 | object, objectiter, thingtemplate | - | none | direct: compile unchanged with shims |
| `weapon` | 8 | Include/GameLogic/Weapon.h<br>Source/GameLogic/Object/Weapon.cpp | 889+3531 | 14 | aipathfind, aiupdate, assaulttransportaiupdate, assistedtargetingupdate, behaviormodule, bodymodule, containmodule, crc, damage, experiencetracker, gamecommon, gamelogic, ... (29) | actionmanager, activebody, ai, aipathfind, aistates, aiupdate, assaulttransportaiupdate, assistedtargetingupdate, ... (55) | inc Drawable.h, FXList.h, InGameUI.h, ParticleSys.h; audio AudioEventRTS.h, GameAudio.h; refs TheAudiox1 | direct: compile unchanged with shims; stand-ins for 10 later-layer unit(s); client stand-ins that log calls |
| `weaponset` | 8 | Include/GameLogic/WeaponSet.h<br>Source/GameLogic/Object/WeaponSet.cpp | 273+1145 | 14 | ai, aiupdate, bitflagsio, containmodule, damage, gametype, ini, kindof, modelstate, object, player, playerlist, ... (21) | assistedtargetingupdate, battleplanupdate, deliverpayloadaiupdate, fireoclafterweaponcooldownupdate, fireweaponpower, lockweaponcreate, object, objectcreationlist, ... (10) | none | direct: compile unchanged with shims; stand-ins for 4 later-layer unit(s) |
| `activebody` | 9 | Include/GameLogic/Module/ActiveBody.h<br>Source/GameLogic/Object/Body/ActiveBody.cpp | 179+1654 | 14 | ai, aipathfind, aiupdate, armor, armorset, bitflagsio, bodymodule, bridgebehavior, containmodule, damage, damagefx, damagemodule, ... (26) | battleplanupdate, highlanderbody, immortalbody, modulefactory, structurebody, undeadbody | inc ControlBar.h, Drawable.h, InGameUI.h, ParticleSys.h; refs TheAudiox1, TheControlBarx2, TheInGameUIx2, TheParticleSystemManagerx9 | direct: compile unchanged with shims; stand-ins for 5 later-layer unit(s); client stand-ins that log calls |
| `activeshroudupgrade` | 9 | Include/GameLogic/Module/ActiveShroudUpgrade.h<br>Source/GameLogic/Object/Upgrade/ActiveShroudUpgrade.cpp | 80+130 | 12 | object, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `animationsteeringupdate` | 9 | Include/GameLogic/Module/AnimationSteeringUpdate.h<br>Source/GameLogic/Object/Update/AnimationSteeringUpdate.cpp | 87+177 | 12 | gamelogic, modelstate, object, physicsupdate, updatemodule, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `armorupgrade` | 9 | Include/GameLogic/Module/ArmorUpgrade.h<br>Source/GameLogic/Object/Upgrade/ArmorUpgrade.cpp | 94+145 | 14 | bodymodule, object, player, upgrade, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `assistedtargetingupdate` | 9 | Include/GameLogic/Module/AssistedTargetingUpdate.h<br>Source/GameLogic/Object/Update/AssistedTargetingUpdate.cpp | 87+201 | 14 | aiupdate, laserupdate, object, player, thingfactory, updatemodule, weapon, weaponset, xfer | modulefactory, weapon | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `autodepositupdate` | 9 | Include/GameLogic/Module/AutoDepositUpdate.h<br>Source/GameLogic/Object/Update/AutoDepositUpdate.cpp | 144+291 | 14 | aiupdate, buildassistant, gamelogic, ini, object, player, randomvalue, thing, thingtemplate, updatemodule, xfer | modulefactory, moneycratecollide, player | inc Color.h, GameText.h, InGameUI.h; refs TheGameTextx2, TheInGameUIx2 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `autofindhealingupdate` | 9 | Include/GameLogic/Module/AutoFindHealingUpdate.h<br>Source/GameLogic/Object/Update/AutoFindHealingUpdate.cpp | 86+228 | 10 | kindof, updatemodule | modulefactory | none | direct: compile unchanged with shims |
| `autohealbehavior` | 9 | Include/GameLogic/Module/AutoHealBehavior.h<br>Source/GameLogic/Object/Behavior/AutoHealBehavior.cpp | 194+381 | 14 | behaviormodule, bitflagsio, bodymodule, damagemodule, gamelogic, ini, object, partitionmanager, player, thing, thingtemplate, updatemodule, ... (14) | minefieldbehavior, modulefactory, object | inc Anim2D.h, InGameUI.h, ParticleSys.h; refs TheAnim2DCollectionx2, TheInGameUIx1, TheParticleSystemManagerx3 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `baikonurlaunchpower` | 9 | Include/GameLogic/Module/BaikonurLaunchPower.h<br>Source/GameLogic/Object/SpecialPower/BaikonurLaunchPower.cpp | 89+162 | 14 | gamelogic, object, player, specialpowermodule, thingfactory, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `baseregenerateupdate` | 9 | Include/GameLogic/Module/BaseRegenerateUpdate.h | 85+0 | 11 | damagemodule, updatemodule | baserenerateupdate, modulefactory | none | header: through a wrapper that instantiates it |
| `baserenerateupdate` | 9 | Source/GameLogic/Object/Update/BaseRenerateUpdate.cpp | 0+186 | 12 | baseregenerateupdate, bodymodule, gamelogic, globaldata, object, xfer | - | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `battlebusslowdeathbehavior` | 9 | Include/GameLogic/Module/BattleBusSlowDeathBehavior.h<br>Source/GameLogic/Object/Behavior/BattleBusSlowDeathBehavior.cpp | 93+326 | 12 | aiupdate, behaviormodule, containmodule, gamelogic, modelstate, object, objectcreationlist, physicsupdate, slowdeathbehavior, xfer | modulefactory | inc FXList.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `battleplanupdate` | 9 | Include/GameLogic/Module/BattlePlanUpdate.h<br>Source/GameLogic/Object/Update/BattlePlanUpdate.cpp | 196+942 | 14 | activebody, aiupdate, bitflagsio, gamelogic, kindof, object, objectiter, partitionmanager, physicsupdate, player, playerlist, radar, ... (21) | modulefactory, player | inc ControlBar.h, Drawable.h, FXList.h, GameClient.h, GameText.h, ParticleSys.h; refs TheAudiox34, TheGameTextx3, TheInGameUIx3, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `bonefxdamage` | 9 | Include/GameLogic/Module/BoneFXDamage.h<br>Source/GameLogic/Object/Damage/BoneFXDamage.cpp | 70+118 | 12 | bonefxupdate, damagemodule, gamelogic, object, xfer | bonefxupdate, modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `bonefxupdate` | 9 | Include/GameLogic/Module/BoneFXUpdate.h<br>Source/GameLogic/Object/Update/BoneFXUpdate.cpp | 281+664 | 12 | aiupdate, bodymodule, bonefxdamage, gamelogic, gamestate, ini, object, objectcreationlist, randomvalue, thing, thingtemplate, updatemodule, ... (13) | bonefxdamage, modulefactory, structurecollapseupdate, structuretoppleupdate | inc Drawable.h, FXList.h, ParticleSys.h; refs TheParticleSystemManagerx2 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `bridgebehavior` | 9 | Include/GameLogic/Module/BridgeBehavior.h<br>Source/GameLogic/Object/Behavior/BridgeBehavior.cpp | 201+1483 | 13 | ai, aipathfind, behaviormodule, bodymodule, bridgescaffoldbehavior, damagemodule, diemodule, gamelogic, object, objectcreationlist, partitionmanager, physicsupdate, ... (18) | activebody, bridgetowerbehavior, dozeraiupdate, modulefactory, terrainlogic, workeraiupdate | inc FXList.h, InGameUI.h, Line2D.h, TerrainRoads.h; audio AudioEventRTS.h; refs TheAudiox2, TheFXListStorex2, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `bridgescaffoldbehavior` | 9 | Include/GameLogic/Module/BridgeScaffoldBehavior.h<br>Source/GameLogic/Object/Behavior/BridgeScaffoldBehavior.cpp | 117+362 | 12 | behaviormodule, gamelogic, object, updatemodule, xfer | bridgebehavior, modulefactory | inc InGameUI.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `bridgetowerbehavior` | 9 | Include/GameLogic/Module/BridgeTowerBehavior.h<br>Source/GameLogic/Object/Behavior/BridgeTowerBehavior.cpp | 99+335 | 12 | behaviormodule, bodymodule, bridgebehavior, damagemodule, diemodule, gamelogic, object, terrainlogic, thingtemplate, xfer | dozeraiupdate, modulefactory, terrainlogic, workeraiupdate | inc InGameUI.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `bunkerbusterbehavior` | 9 | Include/GameLogic/Module/BunkerBusterBehavior.h<br>Source/GameLogic/Object/Behavior/BunkerBusterBehavior.cpp | 107+297 | 14 | aiupdate, behaviormodule, containmodule, diemodule, gamelogic, object, player, updatemodule, upgrade, weapon, xfer | modulefactory | inc FXList.h, TerrainVisual.h; refs TheTerrainVisualx1 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `cashbountypower` | 9 | Include/GameLogic/Module/CashBountyPower.h<br>Source/GameLogic/Object/SpecialPower/CashBountyPower.cpp | 131+220 | 14 | object, player, science, specialpowermodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `cashhackspecialpower` | 9 | Include/GameLogic/Module/CashHackSpecialPower.h<br>Source/GameLogic/Object/SpecialPower/CashHackSpecialPower.cpp | 90+219 | 14 | object, partitionmanager, player, specialpowermodule, team, xfer | modulefactory | inc GameText.h, InGameUI.h; refs TheGameTextx2, TheInGameUIx2 | direct: compile unchanged with shims; client stand-ins that log calls |
| `cavecontain` | 9 | Include/GameLogic/Module/CaveContain.h<br>Source/GameLogic/Object/Contain/CaveContain.cpp | 131+436 | 14 | aiupdate, cavesystem, createmodule, gamestate, object, opencontain, partitionmanager, player, playerlist, team, thingtemplate, tunneltracker, ... (13) | modulefactory, scriptactions | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `checkpointupdate` | 9 | Include/GameLogic/Module/CheckpointUpdate.h<br>Source/GameLogic/Object/Update/CheckpointUpdate.cpp | 93+223 | 12 | ai, aiupdate, kindof, object, thingtemplate, updatemodule, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `cleanupareapower` | 9 | Include/GameLogic/Module/CleanupAreaPower.h<br>Source/GameLogic/Object/SpecialPower/CleanupAreaPower.cpp | 79+143 | 14 | cleanuphazardupdate, object, player, specialpowermodule, thingtemplate, xfer | modulefactory | none | direct: compile unchanged with shims |
| `cleanuphazardupdate` | 9 | Include/GameLogic/Module/CleanupHazardUpdate.h<br>Source/GameLogic/Object/Update/CleanupHazardUpdate.cpp | 97+376 | 10 | kindof, updatemodule | cleanupareapower, modulefactory | none | direct: compile unchanged with shims |
| `commandbuttonhuntupdate` | 9 | Include/GameLogic/Module/CommandButtonHuntUpdate.h<br>Source/GameLogic/Object/Update/CommandButtonHuntUpdate.cpp | 92+477 | 14 | actionmanager, gamelogic, kindof, object, objectiter, partitionmanager, player, randomvalue, scriptengine, specialpower, thingtemplate, updatemodule, ... (13) | modulefactory, scriptactions | inc ControlBar.h, Drawable.h; refs TheControlBarx2 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `commandsetupgrade` | 9 | Include/GameLogic/Module/CommandSetUpgrade.h<br>Source/GameLogic/Object/Upgrade/CommandSetUpgrade.cpp | 74+143 | 14 | object, player, upgrademodule, xfer | modulefactory | inc ControlBar.h; refs TheControlBarx3 | direct: compile unchanged with shims; client stand-ins that log calls |
| `converttocarbombcratecollide` | 9 | Include/GameLogic/Module/ConvertToCarBombCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/ConvertToCarBombCrateCollide.cpp | 93+200 | 14 | aiupdate, cratecollide, experiencetracker, module, object, player, radar, scriptengine, thingtemplate, xfer | modulefactory | inc FXList.h; refs TheRadarx2 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `converttohijackedvehiclecratecollide` | 9 | Include/GameLogic/Module/ConvertToHijackedVehicleCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/ConvertToHijackedVehicleCrateCollide.cpp | 87+299 | 14 | aiupdate, containmodule, cratecollide, dozeraiupdate, experiencetracker, hijackerupdate, module, object, partitionmanager, player, playerlist, radar, ... (15) | modulefactory | inc Drawable.h, Eva.h, InGameUI.h; refs TheAudiox1, TheEvax1, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `costmodifierupgrade` | 9 | Include/GameLogic/Module/CostModifierUpgrade.h<br>Source/GameLogic/Object/Upgrade/CostModifierUpgrade.cpp | 119+213 | 14 | bitflagsio, kindof, object, player, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `countermeasuresbehavior` | 9 | Include/GameLogic/Module/CountermeasuresBehavior.h<br>Source/GameLogic/Object/Behavior/CountermeasuresBehavior.cpp | 197+418 | 14 | behaviormodule, bitflagsio, bodymodule, damagemodule, gamelogic, ini, object, partitionmanager, physicsupdate, player, thing, thingfactory, ... (16) | jetaiupdate, modulefactory, object | inc Anim2D.h, InGameUI.h, ParticleSys.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `cratecollide` | 9 | Include/GameLogic/Module/CrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/CrateCollide.cpp | 113+284 | 14 | bitflagsio, collidemodule, gamelogic, object, player, xfer | converttocarbombcratecollide, converttohijackedvehiclecratecollide, healcratecollide, moneycratecollide, sabotagecommandcentercratecollide, sabotagefakebuildingcratecollide, sabotageinternetcentercratecollide, sabotagemilitaryfactorycratecollide, ... (16) | inc Anim2D.h, Drawable.h, FXList.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheAnim2DCollectionx2, TheAudiox4, TheInGameUIx1 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `createcratedie` | 9 | Include/GameLogic/Module/CreateCrateDie.h<br>Source/GameLogic/Object/Die/CreateCrateDie.cpp | 100+293 | 14 | aiupdate, cratesystem, diemodule, experiencetracker, gamelogic, ini, object, partitionmanager, player, playerlist, randomvalue, stltypedefs, ... (15) | modulefactory | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `createobjectdie` | 9 | Include/GameLogic/Module/CreateObjectDie.h<br>Source/GameLogic/Object/Die/CreateObjectDie.cpp | 77+189 | 13 | aiupdate, bodymodule, diemodule, gamelogic, ini, object, objectcreationlist, thingfactory, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `crushdie` | 9 | Include/GameLogic/Module/CrushDie.h<br>Source/GameLogic/Object/Die/CrushDie.cpp | 106+248 | 14 | bodymodule, diemodule, gamelogic, ini, object, player, playerlist, randomvalue, thingfactory, thingtemplate, xfer | modulefactory, physicsupdate | inc Drawable.h; audio AudioEventRTS.h; refs TheAudiox1 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `damdie` | 9 | Include/GameLogic/Module/DamDie.h<br>Source/GameLogic/Object/Die/DamDie.cpp | 68+149 | 12 | aiupdate, diemodule, gamelogic, object, randomvalue, xfer | modulefactory | inc ParticleSys.h, TerrainVisual.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `defaultproductionexitupdate` | 9 | Include/GameLogic/Module/DefaultProductionExitUpdate.h<br>Source/GameLogic/Object/Update/ProductionExitUpdate/DefaultProductionExitUpdate.cpp | 133+224 | 12 | ai, aipathfind, aiupdate, ini, object, randomvalue, thingtemplate, updatemodule, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s) |
| `defectorspecialpower` | 9 | Include/GameLogic/Module/DefectorSpecialPower.h<br>Source/GameLogic/Object/SpecialPower/DefectorSpecialPower.cpp | 85+175 | 14 | object, player, specialpower, specialpowermodule, team, xfer | modulefactory | none | direct: compile unchanged with shims |
| `deletionupdate` | 9 | Include/GameLogic/Module/DeletionUpdate.h<br>Source/GameLogic/Object/Update/DeletionUpdate.cpp | 91+150 | 12 | gamelogic, object, randomvalue, updatemodule, xfer | modulefactory, object, specialpowermodule | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `demoralizespecialpower` | 9 | Include/GameLogic/Module/DemoralizeSpecialPower.h<br>Source/GameLogic/Object/SpecialPower/DemoralizeSpecialPower.cpp | 87+233 | 12 | aiupdate, containmodule, object, partitionmanager, specialpowermodule, xfer | modulefactory | inc FXList.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `demotrapupdate` | 9 | Include/GameLogic/Module/DemoTrapUpdate.h<br>Source/GameLogic/Object/Update/DemoTrapUpdate.cpp | 92+321 | 10 | kindof, updatemodule | modulefactory, railroadguideaiupdate | none | direct: compile unchanged with shims |
| `destroydie` | 9 | Include/GameLogic/Module/DestroyDie.h<br>Source/GameLogic/Object/Die/DestroyDie.cpp | 58+105 | 11 | diemodule, gamelogic, ini, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `dockupdate` | 9 | Include/GameLogic/Module/DockUpdate.h<br>Source/GameLogic/Object/Update/DockUpdate/DockUpdate.cpp | 160+638 | 12 | gamelogic, ini, object, partitionmanager, stltypedefs, updatemodule, xfer | prisondockupdate, railedtransportdockupdate, supplycenterdockupdate, supplywarehousedockupdate | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `dumbprojectilebehavior` | 9 | Include/GameLogic/Module/DumbProjectileBehavior.h<br>Source/GameLogic/Object/Behavior/DumbProjectileBehavior.cpp | 130+799 | 14 | behaviormodule, beziersegment, collidemodule, containmodule, gamecommon, gamelogic, gamestate, gametype, globaldata, ini, missileaiupdate, object, ... (22) | modulefactory | inc Drawable.h, FXList.h | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `dynamicgeometryinfoupdate` | 9 | Include/GameLogic/Module/DynamicGeometryInfoUpdate.h<br>Source/GameLogic/Object/Update/DynamicGeometryInfoUpdate.cpp | 108+265 | 12 | geometry, object, updatemodule, xfer | firestormdynamicgeometryinfoupdate, modulefactory | none | direct: compile unchanged with shims |
| `dynamicshroudclearingrangeupdate` | 9 | Include/GameLogic/Module/DynamicShroudClearingRangeUpdate.h<br>Source/GameLogic/Object/Update/DynamicShroudClearingRangeUpdate.cpp | 134+397 | 12 | gamelogic, object, updatemodule, xfer | modulefactory | inc RadiusDecal.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `ejectpilotdie` | 9 | Include/GameLogic/Module/EjectPilotDie.h<br>Source/GameLogic/Object/Die/EjectPilotDie.cpp | 77+152 | 14 | diemodule, experiencetracker, gamelogic, ini, object, objectcreationlist, player, thingfactory, thingtemplate, xfer | helicopterslowdeathupdate, hijackerupdate, modulefactory | audio GameAudio.h; refs TheAudiox2 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `empupdate` | 9 | Include/GameLogic/Module/EMPUpdate.h<br>Source/GameLogic/Object/Update/EMPUpdate.cpp | 242+585 | 14 | aiupdate, containmodule, diemodule, gamelogic, ini, kindof, object, objectiter, partitionmanager, physicsupdate, player, randomvalue, ... (16) | modulefactory | inc Drawable.h, ParticleSys.h; refs TheParticleSystemManagerx2 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `enemynearupdate` | 9 | Include/GameLogic/Module/EnemyNearUpdate.h<br>Source/GameLogic/Object/Update/EnemyNearUpdate.cpp | 89+153 | 12 | ai, aiupdate, kindof, object, thingtemplate, updatemodule, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `experiencescalarupgrade` | 9 | Include/GameLogic/Module/ExperienceScalarUpgrade.h<br>Source/GameLogic/Object/Upgrade/ExperienceScalarUpgrade.cpp | 78+127 | 12 | experiencetracker, object, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `fireoclafterweaponcooldownupdate` | 9 | Include/GameLogic/Module/FireOCLAfterWeaponCooldownUpdate.h<br>Source/GameLogic/Object/Update/FireOCLAfterWeaponCooldownUpdate.cpp | 112+266 | 14 | aiupdate, bodymodule, gamelogic, lifetimeupdate, object, objectcreationlist, player, thingtemplate, updatemodule, upgrademodule, weapon, weaponset, ... (13) | modulefactory | inc Drawable.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `firespreadupdate` | 9 | Include/GameLogic/Module/FireSpreadUpdate.h<br>Source/GameLogic/Object/Update/FireSpreadUpdate.cpp | 81+220 | 12 | flammableupdate, gamelogic, object, objectcreationlist, partitionmanager, randomvalue, updatemodule, xfer | flammableupdate, modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `firestormdynamicgeometryinfoupdate` | 9 | Include/GameLogic/Module/FirestormDynamicGeometryInfoUpdate.h<br>Source/GameLogic/Object/Update/FirestormDynamicGeometryInfoUpdate.cpp | 90+322 | 14 | damage, dynamicgeometryinfoupdate, gamelogic, geometry, object, partitionmanager, player, terrainlogic, xfer | modulefactory | inc FXList.h, GameClient.h, InGameUI.h, ParticleSys.h; refs TheGameClientx1, TheParticleSystemManagerx2 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `fireweaponcollide` | 9 | Include/GameLogic/Module/FireWeaponCollide.h<br>Source/GameLogic/Object/Collide/FireWeaponCollide.cpp | 86+175 | 12 | collidemodule, object, weapon, xfer | modulefactory | none | direct: compile unchanged with shims |
| `fireweaponpower` | 9 | Include/GameLogic/Module/FireWeaponPower.h<br>Source/GameLogic/Object/SpecialPower/FireWeaponPower.cpp | 89+221 | 14 | aiupdate, gamelogic, object, player, specialpowermodule, thingfactory, weaponset, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `fireweaponupdate` | 9 | Include/GameLogic/Module/FireWeaponUpdate.h<br>Source/GameLogic/Object/Update/FireWeaponUpdate.cpp | 80+179 | 12 | gamelogic, object, updatemodule, weapon, weaponstatus, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `fireweaponwhendamagedbehavior` | 9 | Include/GameLogic/Module/FireWeaponWhenDamagedBehavior.h<br>Source/GameLogic/Object/Behavior/FireWeaponWhenDamagedBehavior.cpp | 178+365 | 12 | aiupdate, behaviormodule, bodymodule, damagemodule, gamelogic, ini, object, objectcreationlist, randomvalue, thing, thingtemplate, updatemodule, ... (15) | modulefactory | inc Drawable.h, FXList.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `fireweaponwhendeadbehavior` | 9 | Include/GameLogic/Module/FireWeaponWhenDeadBehavior.h<br>Source/GameLogic/Object/Behavior/FireWeaponWhenDeadBehavior.cpp | 133+168 | 14 | aiupdate, behaviormodule, bodymodule, diemodule, gamelogic, ini, object, objectcreationlist, player, randomvalue, thing, thingtemplate, ... (15) | modulefactory | inc Drawable.h, FXList.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `flammableupdate` | 9 | Include/GameLogic/Module/FlammableUpdate.h<br>Source/GameLogic/Object/Update/FlammableUpdate.cpp | 120+326 | 12 | bodymodule, damagemodule, firespreadupdate, gamelogic, object, updatemodule, xfer | firespreadupdate, modulefactory, neutronmissileslowdeathupdate | audio AudioEventRTS.h, GameAudio.h; refs TheAudiox2 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `flightdeckbehavior` | 9 | Include/GameLogic/Module/FlightDeckBehavior.h<br>Source/GameLogic/Object/Behavior/FlightDeckBehavior.cpp | 233+1727 | 14 | ai, aipathfind, aiupdate, behaviormodule, diemodule, gamelogic, jetaiupdate, object, partitionmanager, player, productionupdate, team, ... (17) | modulefactory | inc Drawable.h, ParticleSys.h | direct: compile unchanged with shims; stand-ins for 5 later-layer unit(s); client stand-ins that log calls |
| `floatupdate` | 9 | Include/GameLogic/Module/FloatUpdate.h<br>Source/GameLogic/Object/Update/FloatUpdate.cpp | 78+185 | 12 | gamelogic, object, terrainlogic, updatemodule, xfer | modulefactory, objectcreationlist | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `fxlistdie` | 9 | Include/GameLogic/Module/FXListDie.h<br>Source/GameLogic/Object/Die/FXListDie.cpp | 136+137 | 14 | aiupdate, damage, diemodule, gamelogic, ini, object, player, thingtemplate, upgrademodule, weapon, xfer | modulefactory | inc FXList.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `garrisoncontain` | 9 | Include/GameLogic/Module/GarrisonContain.h<br>Source/GameLogic/Object/Contain/GarrisonContain.cpp | 249+2061 | 14 | aipathfind, aiupdate, bodymodule, gamelogic, gamestate, modelstate, object, opencontain, partitionmanager, player, playerlist, randomvalue, ... (17) | modulefactory | inc Drawable.h, GameClient.h, InGameUI.h, View.h; refs TheGameClientx2, TheInGameUIx1 | direct: compile unchanged with shims; stand-ins for 4 later-layer unit(s); client stand-ins that log calls |
| `generateminefieldbehavior` | 9 | Include/GameLogic/Module/GenerateMinefieldBehavior.h<br>Source/GameLogic/Object/Behavior/GenerateMinefieldBehavior.cpp | 139+589 | 14 | aiupdate, behaviormodule, bodymodule, diemodule, gamelogic, globaldata, ini, object, objectcreationlist, objectiter, partitionmanager, player, ... (20) | deliverpayloadaiupdate, modulefactory | inc Drawable.h, FXList.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `grantscienceupgrade` | 9 | Include/GameLogic/Module/GrantScienceUpgrade.h<br>Source/GameLogic/Object/Upgrade/GrantScienceUpgrade.cpp | 87+144 | 14 | object, player, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `grantstealthbehavior` | 9 | Include/GameLogic/Module/GrantStealthBehavior.h<br>Source/GameLogic/Object/Behavior/GrantStealthBehavior.cpp | 112+255 | 14 | behaviormodule, bitflagsio, bodymodule, containmodule, damagemodule, gamelogic, ini, object, partitionmanager, player, stealthupdate, thing, ... (16) | modulefactory | inc Anim2D.h, InGameUI.h, ParticleSys.h; refs TheParticleSystemManagerx2 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `grantupgradecreate` | 9 | Include/GameLogic/Module/GrantUpgradeCreate.h<br>Source/GameLogic/Object/Create/GrantUpgradeCreate.cpp | 82+182 | 14 | createmodule, object, objectstatustypes, player, upgrade, xfer | modulefactory | none | direct: compile unchanged with shims |
| `healcontain` | 9 | Include/GameLogic/Module/HealContain.h<br>Source/GameLogic/Object/Contain/HealContain.cpp | 77+222 | 12 | bodymodule, damage, gamelogic, object, opencontain, updatemodule, xfer | modulefactory, tunnelcontain | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `healcratecollide` | 9 | Include/GameLogic/Module/HealCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/HealCrateCollide.cpp | 60+106 | 14 | cratecollide, module, object, player, xfer | modulefactory | audio AudioEventRTS.h, MiscAudio.h; refs TheAudiox2 | direct: compile unchanged with shims; client stand-ins that log calls |
| `heightdieupdate` | 9 | Include/GameLogic/Module/HeightDieUpdate.h<br>Source/GameLogic/Object/Update/HeightDieUpdate.cpp | 83+326 | 12 | containmodule, gamelogic, object, partitionmanager, physicsupdate, terrainlogic, thingtemplate, updatemodule, xfer | modulefactory | inc InGameUI.h, ParticleSys.h; refs TheParticleSystemManagerx1 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `helicopterslowdeathupdate` | 9 | Include/GameLogic/Module/HelicopterSlowDeathUpdate.h<br>Source/GameLogic/Object/Update/HelicopterSlowDeathUpdate.cpp | 117+566 | 13 | aiupdate, bodymodule, ejectpilotdie, gamelogic, globaldata, locomotor, object, objectcreationlist, physicsupdate, randomvalue, slowdeathbehavior, thingfactory, ... (14) | modulefactory | inc Drawable.h, FXList.h, InGameUI.h, ParticleSys.h; audio AudioEventRTS.h, GameAudio.h; refs TheAudiox2, TheParticleSystemManagerx1 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `helixcontain` | 9 | Include/GameLogic/Module/HelixContain.h<br>Source/GameLogic/Object/Contain/HelixContain.cpp | 134+481 | 14 | bodymodule, gamelogic, object, partitionmanager, player, thingfactory, thingtemplate, transportcontain, weapon, xfer | modulefactory | inc ControlBar.h, Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `highlanderbody` | 9 | Include/GameLogic/Module/HighlanderBody.h<br>Source/GameLogic/Object/Body/HighlanderBody.cpp | 62+101 | 12 | activebody, xfer | modulefactory | none | direct: compile unchanged with shims |
| `hijackerupdate` | 9 | Include/GameLogic/Module/HijackerUpdate.h<br>Source/GameLogic/Object/Update/HijackerUpdate.cpp | 97+261 | 14 | aiupdate, containmodule, ejectpilotdie, experiencetracker, gamelogic, object, partitionmanager, player, thingfactory, thingtemplate, updatemodule, xfer | converttohijackedvehiclecratecollide, modulefactory, sabotagecommandcentercratecollide, sabotagefakebuilding, sabotageinternetcentercratecollide, sabotagemilitaryfactorycratecollide, sabotagepowerplantcratecollide, sabotagesuperweaponcratecollide, ... (11) | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `hivestructurebody` | 9 | Include/GameLogic/Module/HiveStructureBody.h<br>Source/GameLogic/Object/Body/HiveStructureBody.cpp | 86+171 | 13 | containmodule, damage, gamelogic, object, spawnbehavior, structurebody, thingtemplate, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `hordeupdate` | 9 | Include/GameLogic/Module/HordeUpdate.h<br>Source/GameLogic/Object/Update/HordeUpdate.cpp | 129+441 | 14 | aiupdate, bitflagsio, kindof, partitionmanager, player, thingtemplate, updatemodule, upgrade, weapon, xfer | aiupdate, modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `immortalbody` | 9 | Include/GameLogic/Module/ImmortalBody.h<br>Source/GameLogic/Object/Body/ImmortalBody.cpp | 62+106 | 12 | activebody, damage, object, xfer | modulefactory | none | direct: compile unchanged with shims |
| `inactivebody` | 9 | Include/GameLogic/Module/InactiveBody.h<br>Source/GameLogic/Object/Body/InactiveBody.cpp | 74+205 | 12 | bodymodule, damage, diemodule, gamelogic, object, thingtemplate, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `instantdeathbehavior` | 9 | Include/GameLogic/Module/InstantDeathBehavior.h<br>Source/GameLogic/Object/Behavior/InstantDeathBehavior.cpp | 81+215 | 12 | aiupdate, bodymodule, diemodule, gamelod, gamelogic, ini, object, objectcreationlist, randomvalue, thing, thingtemplate, weapon, ... (13) | modulefactory | inc Drawable.h, FXList.h, InGameUI.h; refs TheFXListStorex1 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `internethackcontain` | 9 | Include/GameLogic/Module/InternetHackContain.h<br>Source/GameLogic/Object/Contain/InternetHackContain.cpp | 73+131 | 13 | aiupdate, object, transportcontain | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `jetslowdeathbehavior` | 9 | Include/GameLogic/Module/JetSlowDeathBehavior.h<br>Source/GameLogic/Object/Behavior/JetSlowDeathBehavior.cpp | 110+393 | 12 | aiupdate, behaviormodule, gamelogic, globaldata, locomotor, object, objectcreationlist, physicsupdate, slowdeathbehavior, thingtemplate, xfer | modulefactory | inc FXList.h, InGameUI.h; audio AudioEventRTS.h; refs TheAudiox2 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `keepobjectdie` | 9 | Include/GameLogic/Module/KeepObjectDie.h<br>Source/GameLogic/Object/Die/KeepObjectDie.cpp | 61+102 | 11 | diemodule, gamelogic, ini, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `laserupdate` | 9 | Include/GameLogic/Module/LaserUpdate.h<br>Source/GameLogic/Object/Update/LaserUpdate.cpp | 120+494 | 12 | clientupdatemodule, drawmodule, gamelogic, object, thingtemplate, xfer | assistedtargetingupdate, modulefactory, specialabilityupdate, weapon | inc Drawable.h, GameClient.h, ParticleSys.h; refs TheGameClientx4, TheParticleSystemManagerx8 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `lifetimeupdate` | 9 | Include/GameLogic/Module/LifetimeUpdate.h<br>Source/GameLogic/Object/Update/LifetimeUpdate.cpp | 90+141 | 12 | gamelogic, object, randomvalue, updatemodule, xfer | fireoclafterweaponcooldownupdate, modulefactory, objectcreationlist, stickybombupdate | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `lockweaponcreate` | 9 | Include/GameLogic/Module/LockWeaponCreate.h<br>Source/GameLogic/Object/Create/LockWeaponCreate.cpp | 76+134 | 12 | createmodule, object, weaponset, xfer | modulefactory | none | direct: compile unchanged with shims |
| `locomotorsetupgrade` | 9 | Include/GameLogic/Module/LocomotorSetUpgrade.h<br>Source/GameLogic/Object/Upgrade/LocomotorSetUpgrade.cpp | 63+97 | 12 | aiupdate, object, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `maxhealthupgrade` | 9 | Include/GameLogic/Module/MaxHealthUpgrade.h<br>Source/GameLogic/Object/Upgrade/MaxHealthUpgrade.cpp | 80+132 | 12 | bodymodule, experiencetracker, object, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `minefieldbehavior` | 9 | Include/GameLogic/Module/MinefieldBehavior.h<br>Source/GameLogic/Object/Behavior/MinefieldBehavior.cpp | 149+707 | 12 | aiupdate, autohealbehavior, behaviormodule, bodymodule, collidemodule, damagemodule, diemodule, gamelogic, gamestate, object, objectcreationlist, physicsupdate, ... (16) | modulefactory | inc Drawable.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `missilelauncherbuildingupdate` | 9 | Include/GameLogic/Module/MissileLauncherBuildingUpdate.h<br>Source/GameLogic/Object/Update/MissileLauncherBuildingUpdate.cpp | 139+329 | 14 | aiupdate, gamelogic, globaldata, ini, object, player, specialpower, specialpowermodule, specialpowerupdatemodule, xfer | modulefactory | inc Drawable.h, FXList.h; audio AudioEventRTS.h, GameAudio.h; refs TheAudiox6 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `mobmemberslavedupdate` | 9 | Include/GameLogic/Module/MobMemberSlavedUpdate.h<br>Source/GameLogic/Object/Update/MobMemberSlavedUpdate.cpp | 137+471 | 13 | aipathfind, aiupdate, bodymodule, damage, gamelogic, ini, locomotor, object, partitionmanager, randomvalue, spawnbehavior, terrainlogic, ... (16) | modulefactory | inc Drawable.h, InGameUI.h, ParticleSys.h | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `mobnexuscontain` | 9 | Include/GameLogic/Module/MobNexusContain.h<br>Source/GameLogic/Object/Contain/MobNexusContain.cpp | 108+497 | 14 | ai, aipathfind, aiupdate, bodymodule, locomotor, object, opencontain, physicsupdate, player, stealthupdate, thingfactory, thingtemplate, ... (13) | modulefactory, scriptactions | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `modelconditionupgrade` | 9 | Include/GameLogic/Module/ModelConditionUpgrade.h<br>Source/GameLogic/Object/Upgrade/ModelConditionUpgrade.cpp | 68+119 | 12 | modelstate, object, upgrademodule | modulefactory | none | direct: compile unchanged with shims |
| `moneycratecollide` | 9 | Include/GameLogic/Module/MoneyCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/MoneyCrateCollide.cpp | 93+135 | 14 | autodepositupdate, cratecollide, module, object, player, xfer | modulefactory | audio AudioEventRTS.h, MiscAudio.h; refs TheAudiox2 | direct: compile unchanged with shims; client stand-ins that log calls |
| `neutonblastbehavior` | 9 | Source/GameLogic/Object/Behavior/NeutonBlastBehavior.cpp | 0+204 | 14 | aiupdate, containmodule, gamelogic, neutronblastbehavior, object, partitionmanager, player, playerlist | - | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `neutronblastbehavior` | 9 | Include/GameLogic/Module/NeutronBlastBehavior.h | 99+0 | 11 | diemodule, updatemodule | modulefactory, neutonblastbehavior | none | header: through a wrapper that instantiates it |
| `neutronmissileslowdeathupdate` | 9 | Include/GameLogic/Module/NeutronMissileSlowDeathUpdate.h<br>Source/GameLogic/Object/Update/NeutronMissileSlowDeathUpdate.cpp | 116+528 | 14 | flammableupdate, gamelogic, gamestate, object, objectiter, partitionmanager, physicsupdate, player, slowdeathbehavior, terrainlogic, toppleupdate, xfer | modulefactory | inc Drawable.h, FXList.h, GameClient.h; refs TheGameClientx1 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `neutronmissileupdate` | 9 | Include/GameLogic/Module/NeutronMissileUpdate.h<br>Source/GameLogic/Object/Update/NeutronMissileUpdate.cpp | 145+642 | 12 | aiupdate, diemodule, experiencetracker, gamelogic, gamestate, gametype, globaldata, ini, object, partitionmanager, physicsupdate, randomvalue, ... (17) | modulefactory | inc Drawable.h, FXList.h, InGameUI.h, ParticleSys.h, RadiusDecal.h; refs TheParticleSystemManagerx2 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `objectcreationupgrade` | 9 | Include/GameLogic/Module/ObjectCreationUpgrade.h<br>Source/GameLogic/Object/Upgrade/ObjectCreationUpgrade.cpp | 82+139 | 14 | modelstate, object, objectcreationlist, player, upgrademodule, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `objectdefectionhelper` | 9 | Include/GameLogic/Module/ObjectDefectionHelper.h<br>Source/GameLogic/Object/Helper/ObjectDefectionHelper.cpp | 88+185 | 12 | gamelogic, object, objecthelper, xfer | object | inc Drawable.h; audio MiscAudio.h; refs TheAudiox4 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `objecthelper` | 9 | Include/GameLogic/Module/ObjectHelper.h<br>Source/GameLogic/Object/Helper/ObjectHelper.cpp | 68+104 | 12 | gamelogic, object, updatemodule, xfer | firingtracker, objectdefectionhelper, objectrepulsorhelper, objectsmchelper, objectweaponstatushelper, statusdamagehelper, subdualdamagehelper, tempweaponbonushelper | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `objectrepulsorhelper` | 9 | Include/GameLogic/Module/ObjectRepulsorHelper.h<br>Source/GameLogic/Object/Helper/ObjectRepulsorHelper.cpp | 63+93 | 12 | object, objecthelper, xfer | object | none | direct: compile unchanged with shims |
| `objectsmchelper` | 9 | Include/GameLogic/Module/ObjectSMCHelper.h<br>Source/GameLogic/Object/Helper/ObjectSMCHelper.cpp | 63+93 | 12 | object, objecthelper, xfer | object | none | direct: compile unchanged with shims |
| `objectweaponstatushelper` | 9 | Include/GameLogic/Module/ObjectWeaponStatusHelper.h<br>Source/GameLogic/Object/Helper/ObjectWeaponStatusHelper.cpp | 85+80 | 12 | object, objecthelper, thingtemplate, xfer | object | none | direct: compile unchanged with shims |
| `oclspecialpower` | 9 | Include/GameLogic/Module/OCLSpecialPower.h<br>Source/GameLogic/Object/SpecialPower/OCLSpecialPower.cpp | 108+294 | 14 | object, objectcreationlist, partitionmanager, player, randomvalue, science, specialpowermodule, terrainlogic, thingfactory, xfer | modulefactory | none | direct: compile unchanged with shims |
| `oclupdate` | 9 | Include/GameLogic/Module/OCLUpdate.h<br>Source/GameLogic/Object/Update/OCLUpdate.cpp | 101+320 | 14 | gamelogic, object, objectcreationlist, partitionmanager, player, playertemplate, randomvalue, terrainlogic, unicodestring, updatemodule, xfer | modulefactory, sabotagecommandcentercratecollide, sabotageinternetcentercratecollide, sabotagemilitaryfactorycratecollide, sabotagesuperweaponcratecollide, sabotagesupplycentercratecollide, sabotagesupplydropzonecratecollide | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `opencontain` | 9 | Include/GameLogic/Module/OpenContain.h<br>Source/GameLogic/Object/Contain/OpenContain.cpp | 288+1826 | 14 | aipathfind, aiupdate, behaviormodule, bitflagsio, bodymodule, collidemodule, containmodule, damagemodule, diemodule, gamelogic, gamestate, kindof, ... (24) | cavecontain, gamelogic, gamelogicdispatch, garrisoncontain, healcontain, mobnexuscontain, modulefactory, parachutecontain, ... (14) | inc ControlBar.h, Drawable.h, InGameUI.h; audio AudioEventRTS.h, GameAudio.h; refs TheAudiox5 | direct: compile unchanged with shims; stand-ins for 4 later-layer unit(s); client stand-ins that log calls |
| `overchargebehavior` | 9 | Include/GameLogic/Module/OverchargeBehavior.h<br>Source/GameLogic/Object/Behavior/OverchargeBehavior.cpp | 118+328 | 14 | behaviormodule, bodymodule, damagemodule, gamelogic, object, player, playerlist, powerplantupdate, radar, updatemodule, xfer | aigroup, modulefactory, object | inc InGameUI.h; refs TheInGameUIx1, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `overlordcontain` | 9 | Include/GameLogic/Module/OverlordContain.h<br>Source/GameLogic/Object/Contain/OverlordContain.cpp | 132+638 | 14 | bodymodule, experiencetracker, gamelogic, object, partitionmanager, player, thingfactory, thingtemplate, transportcontain, xfer | modulefactory | inc ControlBar.h, Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `parachutecontain` | 9 | Include/GameLogic/Module/ParachuteContain.h<br>Source/GameLogic/Object/Contain/ParachuteContain.cpp | 118+791 | 14 | aipathfind, aiupdate, locomotor, object, opencontain, partitionmanager, physicsupdate, player, randomvalue, thingtemplate, xfer | deliverpayloadaiupdate, modulefactory | inc Drawable.h; refs TheAudiox1 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `parkingplacebehavior` | 9 | Include/GameLogic/Module/ParkingPlaceBehavior.h<br>Source/GameLogic/Object/Behavior/ParkingPlaceBehavior.cpp | 218+1101 | 13 | ai, aipathfind, aiupdate, behaviormodule, diemodule, gamelogic, jetaiupdate, object, productionupdate, team, terrainlogic, thingtemplate, ... (14) | buildassistant, jetaiupdate, modulefactory, productionupdate | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 5 later-layer unit(s); client stand-ins that log calls |
| `particleuplinkcannonupdate` | 9 | Include/GameLogic/Module/ParticleUplinkCannonUpdate.h<br>Source/GameLogic/Object/Update/ParticleUplinkCannonUpdate.cpp | 245+1474 | 9 | kindof, science, specialpowerupdatemodule | modulefactory | refs TheAudiox29, TheGameClientx16, TheParticleSystemManagerx9 | direct: compile unchanged with shims; client stand-ins that log calls |
| `passengersfireupgrade` | 9 | Include/GameLogic/Module/PassengersFireUpgrade.h<br>Source/GameLogic/Object/Upgrade/PassengersFireUpgrade.cpp | 62+106 | 12 | containmodule, object, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `physicsupdate` | 9 | Include/GameLogic/Module/PhysicsUpdate.h<br>Source/GameLogic/Object/Update/PhysicsUpdate.cpp | 312+1920 | 12 | aiupdate, behaviormodule, bodymodule, collidemodule, containmodule, crushdie, gamelogic, logicrandomvalue, object, scriptengine, terrainlogic, thingtemplate, ... (15) | aipathfind, aistates, aiupdate, animationsteeringupdate, assaulttransportaiupdate, battlebusslowdeathbehavior, battleplanupdate, bridgebehavior, ... (41) | audio AudioEventRTS.h, GameAudio.h; refs TheAudiox2 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `pilotfindvehicleupdate` | 9 | Include/GameLogic/Module/PilotFindVehicleUpdate.h<br>Source/GameLogic/Object/Update/PilotFindVehicleUpdate.cpp | 85+225 | 14 | actionmanager, kindof, player, randomvalue, thingtemplate, updatemodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `pointdefenselaserupdate` | 9 | Include/GameLogic/Module/PointDefenseLaserUpdate.h<br>Source/GameLogic/Object/Update/PointDefenseLaserUpdate.cpp | 93+421 | 10 | kindof, updatemodule | modulefactory | none | direct: compile unchanged with shims |
| `poisonedbehavior` | 9 | Include/GameLogic/Module/PoisonedBehavior.h<br>Source/GameLogic/Object/Behavior/PoisonedBehavior.cpp | 100+249 | 12 | behaviormodule, damage, damagemodule, gamelogic, object, updatemodule, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `powerplantupdate` | 9 | Include/GameLogic/Module/PowerPlantUpdate.h<br>Source/GameLogic/Object/Update/PowerPlantUpdate.cpp | 103+166 | 12 | gamelogic, modelstate, object, updatemodule, xfer | modulefactory, object, overchargebehavior, powerplantupgrade | inc Drawable.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `powerplantupgrade` | 9 | Include/GameLogic/Module/PowerPlantUpgrade.h<br>Source/GameLogic/Object/Upgrade/PowerPlantUpgrade.cpp | 67+179 | 14 | modelstate, object, player, powerplantupdate, upgrademodule, xfer | modulefactory, object | inc Drawable.h, InGameUI.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `powtruckbehavior` | 9 | Include/GameLogic/Module/POWTruckBehavior.h<br>Source/GameLogic/Object/Behavior/POWTruckBehavior.cpp | 75+157 | 13 | aiupdate, object, opencontain, powtruckaiupdate, thingtemplate, xfer | modulefactory | inc InGameUI.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `preordercreate` | 9 | Include/GameLogic/Module/PreorderCreate.h<br>Source/GameLogic/Object/Create/PreorderCreate.cpp | 62+114 | 14 | createmodule, object, player, xfer | modulefactory | none | direct: compile unchanged with shims |
| `prisonbehavior` | 9 | Include/GameLogic/Module/PrisonBehavior.h<br>Source/GameLogic/Object/Behavior/PrisonBehavior.cpp | 89+478 | 14 | gamestate, object, opencontain, player, randomvalue, thingfactory, xfer | modulefactory, propagandacenterbehavior | inc Drawable.h, GameClient.h, InGameUI.h, Line2D.h; refs TheGameClientx4 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `prisondockupdate` | 9 | Include/GameLogic/Module/PrisonDockUpdate.h<br>Source/GameLogic/Object/Update/DockUpdate/PrisonDockUpdate.cpp | 63+129 | 13 | containmodule, dockupdate, object, powtruckaiupdate, thingtemplate, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `productionupdate` | 9 | Include/GameLogic/Module/ProductionUpdate.h<br>Source/GameLogic/Object/Update/ProductionUpdate.cpp | 272+1407 | 14 | bitflagsio, buildassistant, createmodule, diemodule, gamelogic, gamestate, modelstate, object, parkingplacebehavior, player, radar, scriptengine, ... (18) | aigroup, aiplayer, aiskirmishplayer, buildassistant, flightdeckbehavior, gamelogicdispatch, modulefactory, object, ... (10) | inc ControlBar.h, Drawable.h, Eva.h, GameText.h, InGameUI.h; refs TheAudiox5, TheControlBarx1, TheEvax1, TheGameTextx2, TheInGameUIx2, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `projectilestreamupdate` | 9 | Include/GameLogic/Module/ProjectileStreamUpdate.h<br>Source/GameLogic/Object/Update/ProjectileStreamUpdate.cpp | 83+273 | 12 | gamelogic, object, updatemodule, xfer | modulefactory, weapon | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `proneupdate` | 9 | Include/GameLogic/Module/ProneUpdate.h<br>Source/GameLogic/Object/Update/ProneUpdate.cpp | 84+163 | 12 | damage, object, updatemodule, xfer | aiupdate, modulefactory | inc Drawable.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `propagandacenterbehavior` | 9 | Include/GameLogic/Module/PropagandaCenterBehavior.h<br>Source/GameLogic/Object/Behavior/PropagandaCenterBehavior.cpp | 88+282 | 14 | aiupdate, gamelogic, object, player, prisonbehavior, thingtemplate, xfer | modulefactory | inc InGameUI.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `propagandatowerbehavior` | 9 | Include/GameLogic/Module/PropagandaTowerBehavior.h<br>Source/GameLogic/Object/Behavior/PropagandaTowerBehavior.cpp | 117+649 | 14 | behaviormodule, bodymodule, diemodule, gamelogic, gamestate, object, partitionmanager, player, playerlist, updatemodule, upgrade, weapon, ... (13) | modulefactory | inc Drawable.h, FXList.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `queueproductionexitupdate` | 9 | Include/GameLogic/Module/QueueProductionExitUpdate.h<br>Source/GameLogic/Object/Update/ProductionExitUpdate/QueueProductionExitUpdate.cpp | 130+353 | 12 | ai, aipathfind, aiupdate, ini, object, physicsupdate, randomvalue, thingtemplate, updatemodule, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s) |
| `radarupdate` | 9 | Include/GameLogic/Module/RadarUpdate.h<br>Source/GameLogic/Object/Update/RadarUpdate.cpp | 92+173 | 12 | gamelogic, modelstate, object, updatemodule, xfer | modulefactory, object, radarupgrade | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `radarupgrade` | 9 | Include/GameLogic/Module/RadarUpgrade.h<br>Source/GameLogic/Object/Upgrade/RadarUpgrade.cpp | 83+183 | 14 | modelstate, object, player, radarupdate, upgrademodule, xfer | modulefactory, object | inc Drawable.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `radiusdecalupdate` | 9 | Include/GameLogic/Module/RadiusDecalUpdate.h<br>Source/GameLogic/Object/Update/RadiusDecalUpdate.cpp | 89+127 | 12 | gamelogic, object, randomvalue, updatemodule, xfer | modulefactory, objectcreationlist | inc RadiusDecal.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `railedtransportcontain` | 9 | Include/GameLogic/Module/RailedTransportContain.h<br>Source/GameLogic/Object/Contain/RailedTransportContain.cpp | 61+158 | 13 | object, railedtransportdockupdate, transportcontain, xfer | modulefactory | none | direct: compile unchanged with shims |
| `railedtransportdockupdate` | 9 | Include/GameLogic/Module/RailedTransportDockUpdate.h<br>Source/GameLogic/Object/Update/DockUpdate/RailedTransportDockUpdate.cpp | 114+575 | 12 | aiupdate, containmodule, dockupdate, gamelogic, object, opencontain, partitionmanager, thingtemplate, xfer | actionmanager, modulefactory, railedtransportaiupdate, railedtransportcontain | inc Drawable.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `rebuildholebehavior` | 9 | Include/GameLogic/Module/RebuildHoleBehavior.h<br>Source/GameLogic/Object/Behavior/RebuildHoleBehavior.cpp | 124+491 | 13 | aiupdate, behaviormodule, bodymodule, diemodule, gamelogic, gamestate, object, scriptengine, stickybombupdate, thingfactory, thingtemplate, updatemodule, ... (13) | aiplayer, aiskirmishplayer, modulefactory, object, rebuildholeexposedie | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 4 later-layer unit(s); client stand-ins that log calls |
| `rebuildholeexposedie` | 9 | Include/GameLogic/Module/RebuildHoleExposeDie.h<br>Source/GameLogic/Object/Die/RebuildHoleExposeDie.cpp | 77+231 | 14 | ai, aipathfind, aiupdate, bodymodule, diemodule, gamelogic, ini, object, player, playerlist, rebuildholebehavior, scriptengine, ... (14) | modulefactory | inc SelectionXlat.h | direct: compile unchanged with shims; stand-ins for 5 later-layer unit(s); client stand-ins that log calls |
| `repairdockupdate` | 9 | Include/GameLogic/Module/RepairDockUpdate.h<br>Source/GameLogic/Object/Update/DockUpdate/RepairDockUpdate.cpp | 80+200 | 12 | bodymodule, object, supplycenterdockupdate, xfer | modulefactory | none | direct: compile unchanged with shims |
| `replaceobjectupgrade` | 9 | Include/GameLogic/Module/ReplaceObjectUpgrade.h<br>Source/GameLogic/Object/Upgrade/ReplaceObjectUpgrade.cpp | 69+149 | 14 | ai, aipathfind, createmodule, gamelogic, object, player, thingfactory, thingtemplate, upgrademodule | modulefactory | none | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s) |
| `riderchangecontain` | 9 | Include/GameLogic/Module/RiderChangeContain.h<br>Source/GameLogic/Object/Contain/RiderChangeContain.cpp | 121+495 | 14 | ai, aipathfind, aiupdate, bodymodule, experiencetracker, locomotor, object, physicsupdate, player, playerlist, stealthupdate, thingfactory, ... (15) | modulefactory | inc ControlBar.h, Drawable.h, InGameUI.h; refs TheControlBarx1, TheInGameUIx5, TheMessageStreamx3 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `sabotagecommandcentercratecollide` | 9 | Include/GameLogic/Module/SabotageCommandCenterCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/SabotageCommandCenterCrateCollide.cpp | 91+193 | 14 | aiupdate, containmodule, cratecollide, dozeraiupdate, experiencetracker, hijackerupdate, module, object, oclupdate, partitionmanager, player, playerlist, ... (18) | modulefactory | inc Drawable.h, Eva.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheEvax1, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `sabotagefakebuilding` | 9 | Source/GameLogic/Object/Collide/CrateCollide/SabotageFakeBuilding.cpp | 0+188 | 14 | aiupdate, containmodule, dozeraiupdate, experiencetracker, hijackerupdate, object, partitionmanager, player, playerlist, radar, sabotagefakebuildingcratecollide, scriptengine, ... (14) | - | inc Drawable.h, Eva.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheEvax1, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `sabotagefakebuildingcratecollide` | 9 | Include/GameLogic/Module/SabotageFakeBuildingCrateCollide.h | 91+0 | 10 | cratecollide, module | modulefactory, sabotagefakebuilding | none | header: through a wrapper that instantiates it |
| `sabotageinternetcentercratecollide` | 9 | Include/GameLogic/Module/SabotageInternetCenterCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/SabotageInternetCenterCrateCollide.cpp | 94+231 | 14 | aiupdate, behaviormodule, containmodule, cratecollide, dozeraiupdate, experiencetracker, hijackerupdate, module, object, oclupdate, partitionmanager, player, ... (18) | modulefactory | inc Drawable.h, Eva.h, GameText.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheEvax1, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `sabotagemilitaryfactorycratecollide` | 9 | Include/GameLogic/Module/SabotageMilitaryFactoryCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/SabotageMilitaryFactoryCrateCollide.cpp | 94+188 | 14 | aiupdate, containmodule, cratecollide, dozeraiupdate, experiencetracker, hijackerupdate, module, object, oclupdate, partitionmanager, player, playerlist, ... (16) | modulefactory | inc Drawable.h, Eva.h, GameText.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheEvax1, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `sabotagepowerplantcratecollide` | 9 | Include/GameLogic/Module/SabotagePowerPlantCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/SabotagePowerPlantCrateCollide.cpp | 94+190 | 14 | aiupdate, containmodule, cratecollide, dozeraiupdate, experiencetracker, hijackerupdate, module, object, partitionmanager, player, playerlist, radar, ... (15) | modulefactory | inc Drawable.h, Eva.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheEvax1, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `sabotagesuperweaponcratecollide` | 9 | Include/GameLogic/Module/SabotageSuperweaponCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/SabotageSuperweaponCrateCollide.cpp | 91+193 | 14 | aiupdate, containmodule, cratecollide, dozeraiupdate, experiencetracker, hijackerupdate, module, object, oclupdate, partitionmanager, player, playerlist, ... (18) | modulefactory | inc Drawable.h, Eva.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheEvax1, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `sabotagesupplycentercratecollide` | 9 | Include/GameLogic/Module/SabotageSupplyCenterCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/SabotageSupplyCenterCrateCollide.cpp | 94+221 | 14 | aiupdate, containmodule, cratecollide, dozeraiupdate, experiencetracker, hijackerupdate, module, object, oclupdate, partitionmanager, player, playerlist, ... (16) | modulefactory | inc Drawable.h, Eva.h, GameText.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheEvax2, TheGameTextx2, TheInGameUIx2, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `sabotagesupplydropzonecratecollide` | 9 | Include/GameLogic/Module/SabotageSupplyDropzoneCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/SabotageSupplyDropzoneCrateCollide.cpp | 94+232 | 14 | aiupdate, containmodule, cratecollide, dozeraiupdate, experiencetracker, hijackerupdate, module, object, oclupdate, partitionmanager, player, playerlist, ... (16) | modulefactory | inc Drawable.h, Eva.h, GameText.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheEvax2, TheGameTextx2, TheInGameUIx2, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `salvagecratecollide` | 9 | Include/GameLogic/Module/SalvageCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/SalvageCrateCollide.cpp | 117+300 | 14 | cratecollide, experiencetracker, kindof, module, object, player, randomvalue, stltypedefs, thingtemplate, xfer | modulefactory | inc GameText.h, InGameUI.h; audio AudioEventRTS.h, MiscAudio.h; refs TheAudiox6, TheGameTextx1, TheInGameUIx1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `shroudcratecollide` | 9 | Include/GameLogic/Module/ShroudCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/ShroudCrateCollide.cpp | 60+105 | 14 | cratecollide, module, partitionmanager, player, xfer | modulefactory | audio AudioEventRTS.h, MiscAudio.h; refs TheAudiox2 | direct: compile unchanged with shims; client stand-ins that log calls |
| `slavedupdate` | 9 | Include/GameLogic/Module/SlavedUpdate.h<br>Source/GameLogic/Object/Update/SlavedUpdate.cpp | 181+807 | 13 | aipathfind, aiupdate, bodymodule, damage, gamelogic, ini, locomotor, object, partitionmanager, randomvalue, team, terrainlogic, ... (15) | modulefactory, slowdeathbehavior | inc Drawable.h, ParticleSys.h; audio MiscAudio.h; refs TheAudiox2, TheParticleSystemManagerx2 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `slowdeathbehavior` | 9 | Include/GameLogic/Module/SlowDeathBehavior.h<br>Source/GameLogic/Object/Behavior/SlowDeathBehavior.cpp | 182+584 | 12 | aiupdate, behaviormodule, bodymodule, diemodule, gamelod, gamelogic, ini, object, objectcreationlist, physicsupdate, randomvalue, slavedupdate, ... (17) | battlebusslowdeathbehavior, helicopterslowdeathupdate, jetslowdeathbehavior, modulefactory, neutronmissileslowdeathupdate, undeadbody | inc Drawable.h, FXList.h, InGameUI.h; refs TheFXListStorex1 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `smartbombtargethomingupdate` | 9 | Include/GameLogic/Module/SmartBombTargetHomingUpdate.h<br>Source/GameLogic/Object/Update/SmartBombTargetHomingUpdate.cpp | 88+152 | 12 | gamelogic, object, randomvalue, updatemodule, xfer | deliverpayloadaiupdate, modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `spawnbehavior` | 9 | Include/GameLogic/Module/SpawnBehavior.h<br>Source/GameLogic/Object/Behavior/SpawnBehavior.cpp | 231+1162 | 14 | aiupdate, behaviormodule, bodymodule, damagemodule, diemodule, experiencetracker, gamelogic, gamestate, ini, object, partitionmanager, player, ... (17) | actionmanager, aigroup, hivestructurebody, mobmemberslavedupdate, modulefactory, object, stealthupdate, stealthupgrade, ... (9) | inc Drawable.h, InGameUI.h | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `spawnpointproductionexitupdate` | 9 | Include/GameLogic/Module/SpawnPointProductionExitUpdate.h<br>Source/GameLogic/Object/Update/ProductionExitUpdate/SpawnPointProductionExitUpdate.cpp | 104+239 | 12 | ai, aipathfind, gamelogic, ini, object, terrainlogic, updatemodule, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `specialability` | 9 | Include/GameLogic/Module/SpecialAbility.h<br>Source/GameLogic/Object/SpecialPower/SpecialAbility.cpp | 68+142 | 14 | object, player, specialpowermodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `specialabilityupdate` | 9 | Include/GameLogic/Module/SpecialAbilityUpdate.h<br>Source/GameLogic/Object/Update/SpecialAbilityUpdate.cpp | 289+2078 | 14 | aipathfind, aiupdate, containmodule, experiencetracker, gamelogic, globaldata, ini, laserupdate, object, partitionmanager, physicsupdate, player, ... (24) | actionmanager, modulefactory, object, squishcollide | inc ControlBar.h, Drawable.h, Eva.h, FXList.h, GameText.h, InGameUI.h, ParticleSys.h; audio AudioEventRTS.h, GameAudio.h, MiscAudio.h; refs TheAudiox9, TheEvax4, TheGameTextx2, TheInGameUIx2, TheParticleSystemManagerx1, TheRadarx2 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `specialpowercompletiondie` | 9 | Include/GameLogic/Module/SpecialPowerCompletionDie.h<br>Source/GameLogic/Object/Die/SpecialPowerCompletionDie.cpp | 93+131 | 14 | diemodule, ini, object, player, scriptengine, specialpower, xfer | modulefactory, objectcreationlist, weapon | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `specialpowercreate` | 9 | Include/GameLogic/Module/SpecialPowerCreate.h<br>Source/GameLogic/Object/Create/SpecialPowerCreate.cpp | 61+113 | 12 | createmodule, object, specialpowermodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `spectregunshipdeploymentupdate` | 9 | Include/GameLogic/Module/SpectreGunshipDeploymentUpdate.h<br>Source/GameLogic/Object/Update/SpectreGunshipDeploymentUpdate.cpp | 151+334 | 9 | kindof, specialpowerupdatemodule | modulefactory | inc ParticleSys.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `spectregunshipupdate` | 9 | Include/GameLogic/Module/SpectreGunshipUpdate.h<br>Source/GameLogic/Object/Update/SpectreGunshipUpdate.cpp | 176+918 | 9 | kindof, specialpowerupdatemodule | modulefactory | inc ParticleSys.h; refs TheAudiox4, TheParticleSystemManagerx1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `spyvisionspecialpower` | 9 | Include/GameLogic/Module/SpyVisionSpecialPower.h<br>Source/GameLogic/Object/SpecialPower/SpyVisionSpecialPower.cpp | 77+167 | 12 | containmodule, object, specialpowermodule, spyvisionupdate, xfer | modulefactory | none | direct: compile unchanged with shims |
| `spyvisionupdate` | 9 | Include/GameLogic/Module/SpyVisionUpdate.h<br>Source/GameLogic/Object/Update/SpyVisionUpdate.cpp | 151+281 | 14 | gamelogic, player, playerlist, updatemodule, upgrademodule, xfer | modulefactory, sabotageinternetcentercratecollide, spyvisionspecialpower | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `squishcollide` | 9 | Include/GameLogic/Module/SquishCollide.h<br>Source/GameLogic/Object/Collide/SquishCollide.cpp | 63+175 | 13 | aiupdate, collidemodule, damage, hijackerupdate, object, partitionmanager, physicsupdate, specialabilityupdate, thingfactory, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `statusbitsupgrade` | 9 | Include/GameLogic/Module/StatusBitsUpgrade.h<br>Source/GameLogic/Object/Upgrade/StatusBitsUpgrade.cpp | 112+148 | 12 | object, objectstatustypes, upgrademodule, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `statusdamagehelper` | 9 | Include/GameLogic/Module/StatusDamageHelper.h<br>Source/GameLogic/Object/Helper/StatusDamageHelper.cpp | 70+137 | 12 | gamelogic, object, objecthelper, xfer | object | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `stealthdetectorupdate` | 9 | Include/GameLogic/Module/StealthDetectorUpdate.h<br>Source/GameLogic/Object/Update/StealthDetectorUpdate.cpp | 103+469 | 14 | bitflagsio, containmodule, damage, object, partitionmanager, player, playerlist, radar, stealthupdate, thingtemplate, updatemodule, xfer | battleplanupdate, modulefactory | inc Drawable.h, Eva.h, GameText.h, InGameUI.h, ParticleSys.h; audio MiscAudio.h; refs TheAudiox5, TheEvax4, TheGameTextx2, TheInGameUIx2, TheParticleSystemManagerx3, TheRadarx3 | direct: compile unchanged with shims; client stand-ins that log calls |
| `stealthupdate` | 9 | Include/GameLogic/Module/StealthUpdate.h<br>Source/GameLogic/Object/Update/StealthUpdate.cpp | 185+1229 | 14 | aiupdate, containmodule, damage, gamestate, object, partitionmanager, physicsupdate, player, playerlist, radar, spawnbehavior, team, ... (17) | aigroup, aistates, grantstealthbehavior, mobnexuscontain, modulefactory, object, opencontain, partitionmanager, ... (16) | inc ControlBar.h, Drawable.h, Eva.h, FXList.h, GameClient.h; refs TheAudiox6, TheControlBarx1, TheGameClientx2, TheInGameUIx2, TheRadarx2 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `stealthupgrade` | 9 | Include/GameLogic/Module/StealthUpgrade.h<br>Source/GameLogic/Object/Upgrade/StealthUpgrade.cpp | 63+106 | 12 | object, spawnbehavior, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `stickybombupdate` | 9 | Include/GameLogic/Module/StickyBombUpdate.h<br>Source/GameLogic/Object/Update/StickyBombUpdate.cpp | 103+335 | 14 | aiupdate, bodymodule, gamelogic, lifetimeupdate, object, objectiter, partitionmanager, player, thingtemplate, updatemodule, weapon, xfer | modulefactory, object, rebuildholebehavior, scriptactions, specialabilityupdate | inc Drawable.h, FXList.h, InGameUI.h; refs TheAudiox2 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `structurebody` | 9 | Include/GameLogic/Module/StructureBody.h<br>Source/GameLogic/Object/Body/StructureBody.cpp | 84+108 | 12 | activebody, damage, object, xfer | hivestructurebody, modulefactory | none | direct: compile unchanged with shims |
| `structurecollapseupdate` | 9 | Include/GameLogic/Module/StructureCollapseUpdate.h<br>Source/GameLogic/Object/Update/StructureCollapseUpdate.cpp | 154+408 | 12 | aiupdate, behaviormodule, bonefxupdate, diemodule, gamelogic, globaldata, ini, object, objectcreationlist, randomvalue, thing, thingtemplate, ... (14) | modulefactory | inc Drawable.h, FXList.h, InGameUI.h; refs TheFXListStorex1 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `structuretoppleupdate` | 9 | Include/GameLogic/Module/StructureToppleUpdate.h<br>Source/GameLogic/Object/Update/StructureToppleUpdate.cpp | 201+634 | 12 | aiupdate, behaviormodule, bonefxupdate, diemodule, gamelogic, ini, object, objectcreationlist, randomvalue, scriptengine, thing, thingtemplate, ... (15) | modulefactory | inc Drawable.h, FXList.h, InGameUI.h, ParticleSys.h; refs TheParticleSystemManagerx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `subdualdamagehelper` | 9 | Include/GameLogic/Module/SubdualDamageHelper.h<br>Source/GameLogic/Object/Helper/SubdualDamageHelper.cpp | 68+129 | 12 | bodymodule, object, objecthelper, xfer | object | none | direct: compile unchanged with shims |
| `subobjectsupgrade` | 9 | Include/GameLogic/Module/SubObjectsUpgrade.h<br>Source/GameLogic/Object/Upgrade/SubObjectsUpgrade.cpp | 94+166 | 14 | object, player, upgrademodule, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `supplycentercreate` | 9 | Include/GameLogic/Module/SupplyCenterCreate.h<br>Source/GameLogic/Object/Create/SupplyCenterCreate.cpp | 61+119 | 14 | createmodule, player, playerlist, resourcegatheringmanager, xfer | modulefactory | none | direct: compile unchanged with shims |
| `supplycenterdockupdate` | 9 | Include/GameLogic/Module/SupplyCenterDockUpdate.h<br>Source/GameLogic/Object/Update/DockUpdate/SupplyCenterDockUpdate.cpp | 73+221 | 14 | dockupdate, ini, player, supplytruckaiupdate, xfer | actionmanager, modulefactory, repairdockupdate, resourcegatheringmanager, supplytruckaiupdate, workeraiupdate | inc Color.h, GameText.h, InGameUI.h; refs TheGameTextx1, TheInGameUIx1 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `supplycenterproductionexitupdate` | 9 | Include/GameLogic/Module/SupplyCenterProductionExitUpdate.h<br>Source/GameLogic/Object/Update/ProductionExitUpdate/SupplyCenterProductionExitUpdate.cpp | 119+253 | 13 | ai, aipathfind, aiupdate, object, randomvalue, supplytruckaiupdate, thingtemplate, updatemodule, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 4 later-layer unit(s) |
| `supplywarehousecreate` | 9 | Include/GameLogic/Module/SupplyWarehouseCreate.h<br>Source/GameLogic/Object/Create/SupplyWarehouseCreate.cpp | 61+110 | 14 | createmodule, player, playerlist, resourcegatheringmanager, xfer | modulefactory | none | direct: compile unchanged with shims |
| `supplywarehousecripplingbehavior` | 9 | Include/GameLogic/Module/SupplyWarehouseCripplingBehavior.h<br>Source/GameLogic/Object/Behavior/SupplyWarehouseCripplingBehavior.cpp | 95+184 | 12 | behaviormodule, bodymodule, damagemodule, gamelogic, object, updatemodule, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `supplywarehousedockupdate` | 9 | Include/GameLogic/Module/SupplyWarehouseDockUpdate.h<br>Source/GameLogic/Object/Update/DockUpdate/SupplyWarehouseDockUpdate.cpp | 82+234 | 13 | aipathfind, dockupdate, globaldata, ini, object, partitionmanager, supplytruckaiupdate, xfer | actionmanager, aiplayer, modulefactory, resourcegatheringmanager, scriptactions, scriptconditions, supplytruckaiupdate, workeraiupdate | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `techbuildingbehavior` | 9 | Include/GameLogic/Module/TechBuildingBehavior.h<br>Source/GameLogic/Object/Behavior/TechBuildingBehavior.cpp | 84+181 | 14 | diemodule, object, player, playerlist, thingtemplate, updatemodule, xfer | modulefactory | inc FXList.h, InGameUI.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `tempweaponbonushelper` | 9 | Include/GameLogic/Module/TempWeaponBonusHelper.h<br>Source/GameLogic/Object/Helper/TempWeaponBonusHelper.cpp | 72+151 | 12 | gamelogic, object, objecthelper, weapon, xfer | object | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `tensileformationupdate` | 9 | Include/GameLogic/Module/TensileFormationUpdate.h<br>Source/GameLogic/Object/Update/TensileFormationUpdate.cpp | 94+481 | 12 | ai, aipathfind, bodymodule, gamelogic, object, partitionmanager, quicktrig, terrainlogic, updatemodule, xfer | modulefactory | inc Drawable.h, TerrainVisual.h; audio AudioEventRTS.h; refs TheAudiox1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `toppleupdate` | 9 | Include/GameLogic/Module/ToppleUpdate.h<br>Source/GameLogic/Object/Update/ToppleUpdate.cpp | 133+465 | 14 | ai, aipathfind, behaviormodule, collidemodule, damage, gamelogic, object, physicsupdate, player, playerlist, randomvalue, scriptengine, ... (16) | modulefactory, neutronmissileslowdeathupdate, object, waveguideupdate | inc Drawable.h, FXList.h, SwayClientUpdate.h | direct: compile unchanged with shims; stand-ins for 4 later-layer unit(s); client stand-ins that log calls |
| `transitiondamagefx` | 9 | Include/GameLogic/Module/TransitionDamageFX.h<br>Source/GameLogic/Object/Damage/TransitionDamageFX.cpp | 267+469 | 12 | bodymodule, damagemodule, gamelogic, object, objectcreationlist, xfer | modulefactory | inc Drawable.h, FXList.h, ParticleSys.h; refs TheParticleSystemManagerx2 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `transportcontain` | 9 | Include/GameLogic/Module/TransportContain.h<br>Source/GameLogic/Object/Contain/TransportContain.cpp | 121+709 | 14 | ai, aipathfind, aiupdate, bodymodule, locomotor, object, opencontain, physicsupdate, player, stealthupdate, thingfactory, thingtemplate, ... (14) | helixcontain, internethackcontain, modulefactory, overlordcontain, railedtransportcontain, riderchangecontain, scriptactions | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `tunnelcontain` | 9 | Include/GameLogic/Module/TunnelContain.h<br>Source/GameLogic/Object/Contain/TunnelContain.cpp | 140+552 | 14 | aiupdate, createmodule, diemodule, healcontain, object, opencontain, partitionmanager, player, randomvalue, thingtemplate, tunneltracker, xfer | modulefactory, tunneltracker | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `undeadbody` | 9 | Include/GameLogic/Module/UndeadBody.h<br>Source/GameLogic/Object/Body/UndeadBody.cpp | 77+187 | 12 | activebody, object, slowdeathbehavior, xfer | modulefactory | none | direct: compile unchanged with shims |
| `unitcratecollide` | 9 | Include/GameLogic/Module/UnitCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/UnitCrateCollide.cpp | 88+132 | 14 | cratecollide, module, object, partitionmanager, player, thingfactory, xfer | modulefactory | audio AudioEventRTS.h, MiscAudio.h; refs TheAudiox2 | direct: compile unchanged with shims; client stand-ins that log calls |
| `unpausespecialpowerupgrade` | 9 | Include/GameLogic/Module/UnpauseSpecialPowerUpgrade.h<br>Source/GameLogic/Object/Upgrade/UnpauseSpecialPowerUpgrade.cpp | 80+134 | 12 | object, specialpower, specialpowermodule, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `upgradedie` | 9 | Include/GameLogic/Module/UpgradeDie.h<br>Source/GameLogic/Object/Die/UpgradeDie.cpp | 83+127 | 12 | diemodule, gamelogic, ini, object, thingtemplate, upgrade, xfer | modulefactory | inc Drawable.h | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `veterancycratecollide` | 9 | Include/GameLogic/Module/VeterancyCrateCollide.h<br>Source/GameLogic/Object/Collide/CrateCollide/VeterancyCrateCollide.cpp | 97+210 | 14 | aiupdate, cratecollide, experiencetracker, module, object, partitionmanager, player, scriptengine, xfer | modulefactory | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `veterancygaincreate` | 9 | Include/GameLogic/Module/VeterancyGainCreate.h<br>Source/GameLogic/Object/Create/VeterancyGainCreate.cpp | 75+139 | 14 | createmodule, experiencetracker, gamecommon, object, player, science, xfer | modulefactory | none | direct: compile unchanged with shims |
| `waveguideupdate` | 9 | Include/GameLogic/Module/WaveGuideUpdate.h<br>Source/GameLogic/Object/Update/WaveGuideUpdate.cpp | 115+936 | 13 | aipathfind, aiupdate, gamelogic, object, partitionmanager, physicsupdate, radar, terrainlogic, thingfactory, thingtemplate, toppleupdate, updatemodule, ... (13) | modulefactory | inc Drawable.h, ParticleSys.h, TerrainVisual.h; refs TheAudiox2, TheParticleSystemManagerx16, TheRadarx1, TheTerrainVisualx1 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `weaponbonusupdate` | 9 | Include/GameLogic/Module/WeaponBonusUpdate.h<br>Source/GameLogic/Object/Update/WeaponBonusUpdate.cpp | 112+206 | 12 | containmodule, object, partitionmanager, updatemodule, weapon | modulefactory | none | direct: compile unchanged with shims |
| `weaponbonusupgrade` | 9 | Include/GameLogic/Module/WeaponBonusUpgrade.h<br>Source/GameLogic/Object/Upgrade/WeaponBonusUpgrade.cpp | 96+133 | 12 | object, upgrademodule, weapon, xfer | modulefactory | none | direct: compile unchanged with shims |
| `weaponsetupgrade` | 9 | Include/GameLogic/Module/WeaponSetUpgrade.h<br>Source/GameLogic/Object/Upgrade/WeaponSetUpgrade.cpp | 63+97 | 12 | object, upgrademodule, xfer | modulefactory | none | direct: compile unchanged with shims |
| `ai` | 10 | Include/GameLogic/AI.h<br>Source/GameLogic/AI/AI.cpp | 1024+1046 | 14 | aipathfind, aiupdate, containmodule, damage, gamestate, gametype, partitionmanager, player, playerlist, scriptengine, sideslist, snapshot, ... (18) | activebody, aigroup, aiguard, aiguardretaliate, aipathfind, aiplayer, aiskirmishplayer, aitnguard, ... (41) | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `aidock` | 10 | Include/GameLogic/AIDock.h<br>Source/GameLogic/AI/AIDock.cpp | 208+819 | 14 | aistatemachine, aiupdate, module, object, player, supplytruckaiupdate, updatemodule | aistates | none | direct: compile unchanged with shims |
| `aigroup` | 10 | Source/GameLogic/AI/AIGroup.cpp | 0+3318 | 14 | actionmanager, ai, aipathfind, aiupdate, bodymodule, buildassistant, containmodule, locomotor, objectiter, overchargebehavior, player, productionupdate, ... (21) | - | inc ControlBar.h, Drawable.h, Line2D.h; refs TheControlBarx1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `aiguard` | 10 | Include/GameLogic/AIGuard.h<br>Source/GameLogic/AI/AIGuard.cpp | 280+904 | 13 | ai, aipathfind, aistatemachine, aiupdate, bodymodule, collidemodule, gamelogic, object, partitionmanager, polygontrigger, team, thingtemplate, ... (13) | aistates | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `aiguardretaliate` | 10 | Include/GameLogic/AIGuardRetaliate.h<br>Source/GameLogic/AI/AIGuardRetaliate.cpp | 265+860 | 13 | ai, aipathfind, aistatemachine, aiupdate, bodymodule, collidemodule, gamelogic, object, partitionmanager, polygontrigger, team, thingtemplate, ... (13) | aistates | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `aipathfind` | 10 | Include/GameLogic/AIPathfind.h<br>Source/GameLogic/AI/AIPathfind.cpp | 985+11123 | 14 | ai, aiupdate, containmodule, gamelogic, gametype, globaldata, latchrestore, locomotor, locomotorset, object, partitionmanager, physicsupdate, ... (21) | activebody, ai, aigroup, aiguard, aiguardretaliate, aiplayer, aiskirmishplayer, aistates, ... (51) | inc Line2D.h | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `aiplayer` | 10 | Include/GameLogic/AIPlayer.h<br>Source/GameLogic/AI/AIPlayer.cpp | 304+3855 | 14 | ai, aipathfind, aiupdate, buildassistant, dozeraiupdate, gamelogic, gamestate, globaldata, object, partitionmanager, player, playerlist, ... (28) | aiskirmishplayer | inc ControlBar.h, TerrainVisual.h; refs TheControlBarx1, TheTerrainVisualx6 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `aiskirmishplayer` | 10 | Include/GameLogic/AISkirmishPlayer.h<br>Source/GameLogic/AI/AISkirmishPlayer.cpp | 125+1238 | 14 | ai, aipathfind, aiplayer, aiupdate, buildassistant, dozeraiupdate, gamelogic, globaldata, object, partitionmanager, player, playerlist, ... (24) | player, scriptactions | inc TerrainVisual.h; refs TheTerrainVisualx1 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `aistatemachine` | 10 | Include/GameLogic/AIStateMachine.h | 1334+0 | 9 | statemachine, terrainlogic | aidock, aiguard, aiguardretaliate, aistates, aitnguard, aiupdate, chinookaiupdate, jetaiupdate | audio AudioEventRTS.h | header: through a wrapper that instantiates it; client stand-ins that log calls |
| `aistates` | 10 | Source/GameLogic/AI/AIStates.cpp | 0+7484 | 14 | actionmanager, aidock, aiguard, aiguardretaliate, aipathfind, aistatemachine, aitnguard, aiupdate, bodymodule, containmodule, globaldata, jetaiupdate, ... (30) | - | inc ControlBar.h, FXList.h, InGameUI.h; audio AudioHandleSpecialValues.h, GameAudio.h; refs TheAudiox5, TheControlBarx1 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `aitnguard` | 10 | Include/GameLogic/AITNGuard.h<br>Source/GameLogic/AI/AITNGuard.cpp | 251+878 | 14 | ai, aipathfind, aistatemachine, aiupdate, bodymodule, collidemodule, gamelogic, object, partitionmanager, player, polygontrigger, team, ... (14) | aistates | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `aiupdate` | 10 | Include/GameLogic/Module/AIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate.cpp | 806+5304 | 14 | actionmanager, ai, aipathfind, aistatemachine, bodymodule, containmodule, deliverpayloadaiupdate, gamelogic, gamestate, globaldata, hackinternetaiupdate, hordeupdate, ... (33) | activebody, ai, aidock, aigroup, aiguard, aiguardretaliate, aipathfind, aiplayer, ... (112) | inc ControlBar.h, Drawable.h, InGameUI.h; refs TheAudiox1, TheControlBarx3 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `assaulttransportaiupdate` | 10 | Include/GameLogic/Module/AssaultTransportAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/AssaultTransportAIUpdate.cpp | 128+541 | 14 | aiupdate, bodymodule, containmodule, experiencetracker, object, partitionmanager, physicsupdate, player, statemachine, thingfactory, weapon | modulefactory, weapon | inc Drawable.h, InGameUI.h | direct: compile unchanged with shims; client stand-ins that log calls |
| `chinookaiupdate` | 10 | Include/GameLogic/Module/ChinookAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/ChinookAIUpdate.cpp | 145+1677 | 14 | actionmanager, aipathfind, aistatemachine, containmodule, drawmodule, gamestate, globaldata, locomotor, partitionmanager, physicsupdate, player, playerlist, ... (18) | modulefactory | inc Drawable.h, GameClient.h, ParticleSys.h; refs TheGameClientx1, TheParticleSystemManagerx2 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `deliverpayloadaiupdate` | 10 | Include/GameLogic/Module/DeliverPayloadAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/DeliverPayloadAIUpdate.cpp | 388+1246 | 14 | aiupdate, bodymodule, containmodule, generateminefieldbehavior, locomotor, object, parachutecontain, partitionmanager, physicsupdate, player, randomvalue, smartbombtargethomingupdate, ... (18) | aiupdate, modulefactory, objectcreationlist, scriptactions | inc Drawable.h, FXList.h, InGameUI.h, RadiusDecal.h; refs TheAudiox1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `deploystyleaiupdate` | 10 | Include/GameLogic/Module/DeployStyleAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/DeployStyleAIUpdate.cpp | 121+503 | 14 | aiupdate, bodymodule, containmodule, experiencetracker, locomotor, object, partitionmanager, physicsupdate, player, statemachine, thingfactory, thingtemplate, ... (13) | modulefactory | inc Drawable.h, InGameUI.h; refs TheAudiox2 | direct: compile unchanged with shims; client stand-ins that log calls |
| `dozeraiupdate` | 10 | Include/GameLogic/Module/DozerAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/DozerAIUpdate.cpp | 309+2505 | 14 | actionmanager, aipathfind, aiupdate, bodymodule, bridgebehavior, bridgetowerbehavior, buildassistant, createmodule, gamestate, globaldata, locomotor, money, ... (21) | actionmanager, aiplayer, aiskirmishplayer, converttohijackedvehiclecratecollide, modulefactory, object, sabotagecommandcentercratecollide, sabotagefakebuilding, ... (15) | inc Drawable.h, GameText.h, InGameUI.h; refs TheAudiox3, TheGameTextx2, TheInGameUIx5, TheRadarx1 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `hackinternetaiupdate` | 10 | Include/GameLogic/Module/HackInternetAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/HackInternetAIUpdate.cpp | 233+605 | 14 | aiupdate, bodymodule, containmodule, experiencetracker, object, physicsupdate, player, statemachine, thingfactory, thingtemplate | aiupdate, modulefactory | inc Drawable.h, GameText.h, InGameUI.h; refs TheAudiox3, TheGameTextx1, TheInGameUIx1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `jetaiupdate` | 10 | Include/GameLogic/Module/JetAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/JetAIUpdate.cpp | 189+2629 | 13 | actionmanager, aipathfind, aistatemachine, aiupdate, bodymodule, countermeasuresbehavior, experiencetracker, globaldata, locomotor, object, parkingplacebehavior, partitionmanager, ... (17) | aistates, flightdeckbehavior, modulefactory, parkingplacebehavior | inc Drawable.h, GameClient.h; audio MiscAudio.h; refs TheAudiox7, TheGameClientx2 | direct: compile unchanged with shims; client stand-ins that log calls |
| `missileaiupdate` | 10 | Include/GameLogic/Module/MissileAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/MissileAIUpdate.cpp | 152+926 | 12 | aipathfind, aiupdate, bitflagsio, containmodule, experiencetracker, gamelogic, gametype, globaldata, ini, locomotor, object, partitionmanager, ... (19) | dumbprojectilebehavior, modulefactory | inc Drawable.h, FXList.h, ParticleSys.h; refs TheParticleSystemManagerx3 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `powtruckaiupdate` | 10 | Include/GameLogic/Module/POWTruckAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/POWTruckAIUpdate.cpp | 150+917 | 14 | actionmanager, aipathfind, aiupdate, globaldata, locomotor, money, object, opencontain, partitionmanager, player, thingtemplate | modulefactory, powtruckbehavior, prisondockupdate | inc InGameUI.h; refs TheGameTextx1, TheInGameUIx1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `railedtransportaiupdate` | 10 | Include/GameLogic/Module/RailedTransportAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/RailedTransportAIUpdate.cpp | 102+409 | 12 | aiupdate, gamestate, locomotor, object, railedtransportdockupdate, thingtemplate | modulefactory | none | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s) |
| `railroadguideaiupdate` | 10 | Include/GameLogic/Module/RailroadGuideAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/RailroadGuideAIUpdate.cpp | 309+1649 | 14 | ai, aipathfind, aiupdate, containmodule, demotrapupdate, locomotor, object, partitionmanager, physicsupdate, player, thingfactory, thingtemplate | actionmanager, modulefactory, scriptactions | inc Drawable.h, Statistics.h; refs TheAudiox11 | direct: compile unchanged with shims; client stand-ins that log calls |
| `squad` | 10 | Include/GameLogic/Squad.h<br>Source/GameLogic/AI/Squad.cpp | 100+271 | 13 | ai, gamelogic, gamestate, object, snapshot, team, xfer | aistates, partitionmanager, player | none | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s) |
| `supplytruckaiupdate` | 10 | Include/GameLogic/Module/SupplyTruckAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/SupplyTruckAIUpdate.cpp | 242+838 | 14 | aiupdate, object, partitionmanager, player, resourcegatheringmanager, statemachine, supplycenterdockupdate, supplywarehousedockupdate, thingtemplate, workeraiupdate | actionmanager, aidock, aiplayer, chinookaiupdate, modulefactory, player, resourcegatheringmanager, supplycenterdockupdate, ... (11) | inc Drawable.h, InGameUI.h; refs TheAudiox1, TheInGameUIx17 | direct: compile unchanged with shims; client stand-ins that log calls |
| `transportaiupdate` | 10 | Include/GameLogic/Module/TransportAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/TransportAIUpdate.cpp | 70+237 | 12 | aiupdate, behaviormodule, containmodule, object, randomvalue | modulefactory | none | direct: compile unchanged with shims |
| `turretai` | 10 | Include/GameLogic/TurretAI.h<br>Source/GameLogic/AI/TurretAI.cpp | 378+1466 | 12 | aiupdate, gamelogic, object, partitionmanager, randomvalue, statemachine, terrainlogic, thingtemplate, weapon, weaponset, xfer | aistates, aiupdate | audio GameAudio.h; refs TheAudiox2 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `wanderaiupdate` | 10 | Include/GameLogic/Module/WanderAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/WanderAIUpdate.cpp | 68+103 | 12 | aiupdate, object, randomvalue | modulefactory | none | direct: compile unchanged with shims |
| `workeraiupdate` | 10 | Include/GameLogic/Module/WorkerAIUpdate.h<br>Source/GameLogic/Object/Update/AIUpdate/WorkerAIUpdate.cpp | 274+1485 | 14 | actionmanager, aipathfind, aiupdate, bodymodule, bridgebehavior, bridgetowerbehavior, buildassistant, createmodule, dozeraiupdate, gamestate, globaldata, locomotor, ... (26) | modulefactory, supplytruckaiupdate | inc Drawable.h, GameText.h, InGameUI.h; refs TheAudiox3 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `scriptactions` | 11 | Include/GameLogic/ScriptActions.h<br>Source/GameLogic/ScriptEngine/ScriptActions.cpp | 398+7568 | 14 | ai, aipathfind, aiskirmishplayer, aiupdate, cavecontain, commandbuttonhuntupdate, containmodule, deliverpayloadaiupdate, gameengine, locomotor, mapobject, mobnexuscontain, ... (33) | gamelogic, gamelogicdispatch, scriptengine, team, victoryconditions | inc Anim2D.h, CampaignManager.h, CommandXlat.h, ControlBar.h, Drawable.h, Eva.h, GUICallbacks.h, GadgetStaticText.h, GameClient.h, GameText.h, GlobalLanguage.h, InGameUI.h, LookAtXlat.h, MessageBox.h, Mouse.h, Snow.h, View.h; audio AudioAffect.h, AudioHandleSpecialValues.h, GameAudio.h; refs TheAudiox16, TheControlBarx23, TheDisplayx8, TheEvax1, TheGameClientx1, TheGameTextx4, TheInGameUIx22, TheMousex2, TheRadarx8, TheSnowManagerx1, TheTacticalViewx59, TheWindowManagerx6 | direct: compile unchanged with shims; client stand-ins that log calls |
| `scriptconditions` | 11 | Include/GameLogic/ScriptConditions.h<br>Source/GameLogic/ScriptEngine/ScriptConditions.cpp | 193+2992 | 14 | aiupdate, bodymodule, containmodule, gameengine, gamelogic, mapobject, object, objectstatustypes, objecttypes, partitionmanager, player, playerlist, ... (22) | gamelogic, scriptengine | inc ControlBar.h, Drawable.h, GameClient.h, InGameUI.h, View.h; refs TheAudiox1, TheControlBarx1, TheInGameUIx3, TheTacticalViewx1 | direct: compile unchanged with shims; stand-ins for 1 later-layer unit(s); client stand-ins that log calls |
| `scriptengine` | 11 | Include/GameLogic/ScriptEngine.h<br>Source/GameLogic/ScriptEngine/ScriptEngine.cpp | 506+10353 | 14 | aiupdate, datachunk, file, filesystem, gameengine, gamelogic, gamestate, gametype, latchrestore, mapobject, messagestream, objecttypes, ... (27) | ai, aipathfind, aiplayer, aiskirmishplayer, aistates, aiupdate, commandbuttonhuntupdate, converttocarbombcratecollide, ... (38) | inc CampaignManager.h, MessageBox.h, ParticleSys.h, Shell.h, View.h; refs TheAudiox3, TheMessageStreamx1, TheParticleSystemManagerx14 | direct: compile unchanged with shims; stand-ins for 3 later-layer unit(s); client stand-ins that log calls |
| `victoryconditions` | 11 | Include/GameLogic/VictoryConditions.h<br>Source/GameLogic/ScriptEngine/VictoryConditions.cpp | 83+370 | 14 | gamecommon, gameengine, gamelogic, kindof, partitionmanager, player, playerlist, playertemplate, radar, recorder, scriptactions, subsysteminterface | gameengine, gamelogic, gamelogicdispatch, player, scriptactions, scriptconditions | inc Diplomacy.h, GUICallbacks.h, GameClient.h, GameText.h, InGameUI.h, MessageBox.h, GameInfo.h, NetworkDefs.h; audio AudioEventRTS.h, GameAudio.h; refs TheAudiox1, TheGameClientx1, TheInGameUIx1, TheRadarx3, TheRecorderx2 | direct: compile unchanged with shims; stand-ins for 2 later-layer unit(s); client stand-ins that log calls |
| `gamelogic` | 12 | Include/GameLogic/GameLogic.h<br>Source/GameLogic/System/GameLogic.cpp | 432+5097 | 14 | ai, aipathfind, aiupdate, bodymodule, buildassistant, cavesystem, cratesystem, createmodule, datachunk, destroymodule, fpucontrol, gamecommon, ... (52) | academystats, activebody, aiguard, aiguardretaliate, aipathfind, aiplayer, aiskirmishplayer, aitnguard, ... (148) | inc CampaignManager.h, ControlBar.h, Drawable.h, GUICallbacks.h, GameClient.h, GameText.h, GameWindowTransitions.h, InGameUI.h, LoadScreen.h, MapUtil.h, Mouse.h, ParticleSys.h, Snow.h, TerrainVisual.h, View.h, Water.h, BuddyThread.h, PeerDefs.h, PersistentStorageThread.h, ThreadUtils.h, LANAPICallbacks.h, NetworkDefs.h, NetworkInterface.h; audio AudioAffect.h, AudioHandleSpecialValues.h, GameAudio.h; refs TheAudiox3, TheCommandListx3, TheControlBarx6, TheDisplayx6, TheGameClientx10, TheGameSpyInfox2, TheGameTextx3, TheGhostObjectManagerx6, TheInGameUIx8, TheLANx2, TheMessageStreamx3, TheMousex6, TheNetworkx12, TheParticleSystemManagerx3, TheRadarx3, TheRecorderx20, TheShellx5, TheTacticalViewx13, TheTerrainVisualx1, TheWindowManagerx1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `gamelogicdispatch` | 12 | Source/GameLogic/System/GameLogicDispatch.cpp | 0+2016 | 14 | ai, aipathfind, aiupdate, bodymodule, buildassistant, gameengine, gamelogic, globaldata, locomotor, messagestream, multiplayersettings, namekeygenerator, ... (32) | - | inc CommandXlat.h, ControlBar.h, Drawable.h, Eva.h, GameText.h, GameWindowManager.h, GameWindowTransitions.h, GuiCallbacks.h, InGameUI.h, KeyDefs.h, LookAtXlat.h, BeaconClientUpdate.h, Mouse.h, ParticleSys.h, Shell.h, NetworkInterface.h; audio GameAudio.h; refs TheAudiox6, TheControlBarx7, TheDisplayx2, TheEvax1, TheGameTextx5, TheInGameUIx9, TheMousex3, TheNetworkx3, TheRadarx1, TheRecorderx5, TheShellx3, TheTacticalViewx2, TheWindowManagerx4 | direct: compile unchanged with shims; client stand-ins that log calls |
| `gamestate` | 12 | Include/Common/GameState.h<br>Source/Common/System/SaveGame/GameState.cpp | 247+1666 | 13 | file, filesystem, gameengine, gamelogic, gamestatemap, ghostobject, latchrestore, mapobject, partitionmanager, playerlist, radar, randomvalue, ... (23) | activebody, ai, aiplayer, aiupdate, bonefxupdate, cavecontain, cavesystem, chinookaiupdate, ... (45) | inc CampaignManager.h, GadgetListBox.h, GameClient.h, GameText.h, InGameUI.h, MapUtil.h, MessageBox.h, ParticleSys.h, TerrainVisual.h, NetworkDefs.h; refs TheGameClientx1, TheGameTextx9, TheGhostObjectManagerx3, TheInGameUIx3, TheMessageStreamx1, TheParticleSystemManagerx1, TheRadarx1, TheTacticalViewx1, TheTerrainVisualx1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `gamestatemap` | 12 | Include/Common/GameStateMap.h<br>Source/Common/System/SaveGame/GameStateMap.cpp | 73+527 | 11 | file, filesystem, gamelogic, gamestate, globaldata, snapshot, subsysteminterface, xfer | gameengine, gamestate | inc CampaignManager.h, GameClient.h, MapUtil.h, GameInfo.h; refs TheGameClientx2 | direct: compile unchanged with shims; client stand-ins that log calls |
| `messagestream` | 12 | Include/Common/MessageStream.h<br>Source/Common/MessageStream.cpp | 835+1239 | 14 | gamecommon, gamelogic, player, playerlist, recorder, subsysteminterface | gameengine, gamelogicdispatch, partitionmanager, player, recorder, scriptengine, thingtemplate | inc InGameUI.h; refs TheCommandListx3, TheMessageStreamx2, TheRecorderx3 | direct: compile unchanged with shims; client stand-ins that log calls |
| `recorder` | 12 | Include/Common/Recorder.h<br>Source/Common/Recorder.cpp | 158+1619 | 14 | filesystem, gameengine, gamelogic, globaldata, messagestream, player, playerlist, randomvalue, version | gameengine, gamelogic, gamelogicdispatch, messagestream, victoryconditions | inc GameText.h, GameWindow.h, GameWindowManager.h, InGameUI.h, Shell.h, GameInfo.h, GameMessageParser.h, PeerDefs.h, LANAPICallbacks.h, NetworkUtil.h; refs TheCommandListx4, TheGameSpyInfox1, TheGameTextx1, TheLANx4, TheMessageStreamx2, TheNetworkx6, TheRecorderx2, TheWindowManagerx1 | direct: compile unchanged with shims; client stand-ins that log calls |
| `audioaffect` | C | Include/Common/AudioAffect.h | 46+0 | 0 | - | - | none | none: interface; logic tests record its calls |
| `audioeventinfo` | C | Include/Common/AudioEventInfo.h | 137+0 | 3 | asciistring, stltypedefs | - | none | none: interface; logic tests record its calls |
| `audioeventrts` | C | Include/Common/AudioEventRTS.h<br>Source/Common/Audio/AudioEventRTS.cpp | 217+851 | 14 | asciistring, file, filesystem, gamelogic, gametype, globaldata, logicrandomvalue, object, player | - | inc Drawable.h, GameClient.h; audio AudioEventInfo.h, AudioEventRTS.h, AudioRandomValue.h, AudioSettings.h, GameAudio.h, GameSounds.h; refs TheAudiox6, TheGameClientx2 | none: interface; logic tests record its calls |
| `audiohandlespecialvalues` | C | Include/Common/AudioHandleSpecialValues.h | 45+0 | 0 | - | - | none | none: interface; logic tests record its calls |
| `audiorequest` | C | Include/Common/AudioRequest.h<br>Source/Common/Audio/AudioRequest.cpp | 61+34 | 0 | - | - | audio AudioRequest.h, GameAudio.h | none: interface; logic tests record its calls |
| `audiosettings` | C | Include/Common/AudioSettings.h | 94+0 | 2 | asciistring | - | none | none: interface; logic tests record its calls |
| `bordercolors` | C | Include/Common/BorderColors.h | 41+0 | 0 | - | scripts | none | none: interface; logic tests record its calls |
| `clientupdatemodule` | C | Include/Common/ClientUpdateModule.h | 69+0 | 7 | module | laserupdate | none | none: interface; logic tests record its calls |
| `custommatchpreferences` | C | Include/Common/CustomMatchPreferences.h | 94+0 | 4 | userpreferences | userpreferences | none | none: interface; logic tests record its calls |
| `drawmodule` | C | Include/Common/DrawModule.h<br>Source/Common/Thing/DrawModule.cpp | 262+75 | 8 | gametype, modelstate, module, xfer | chinookaiupdate, laserupdate, objectcreationlist | inc Color.h | none: interface; logic tests record its calls |
| `dynamicaudioeventinfo` | C | Include/Common/DynamicAudioEventInfo.h<br>Source/Common/Audio/DynamicAudioEventInfo.cpp | 162+233 | 8 | bitflags, xfer | - | audio AudioEventInfo.h, DynamicAudioEventInfo.h | none: interface; logic tests record its calls |
| `functionlexicon` | C | Include/Common/FunctionLexicon.h<br>Source/Common/System/FunctionLexicon.cpp | 154+717 | 6 | namekeygenerator, subsysteminterface | gameengine | inc GUICallbacks.h, Gadget.h, GameWindow.h, GameWindowManager.h, WindowLayout.h | none: interface; logic tests record its calls |
| `gameaudio` | C | Include/Common/GameAudio.h<br>Source/Common/Audio/GameAudio.cpp | 377+1170 | 14 | filesystem, gameengine, gamelogic, osdisplay, player, playerlist, stltypedefs, subsysteminterface, terrainlogic, userpreferences | - | inc ControlBar.h, Drawable.h, View.h; audio AudioAffect.h, AudioEventInfo.h, AudioEventRTS.h, AudioHandleSpecialValues.h, AudioRequest.h, AudioSettings.h, GameAudio.h, GameMusic.h, GameSounds.h, MiscAudio.h; refs TheAudiox20, TheControlBarx1, TheTacticalViewx3 | none: interface; logic tests record its calls |
| `gameengine` | C | Include/Common/GameEngine.h<br>Source/Common/GameEngine.cpp | 119+1009 | 13 | actionmanager, ai, archivefilesystem, armor, buildassistant, cavesystem, cratesystem, damage, damagefx, file, filesystem, functionlexicon, ... (49) | gameaudio, gamelogic, gamelogicdispatch, gamestate, object, partitionmanager, recorder, scriptactions, ... (11) | inc Display.h, Drawable.h, FXList.h, GUICallbacks.h, GameClient.h, GameText.h, GameWindowManager.h, GlobalLanguage.h, Keyboard.h, MapUtil.h, MetaEvent.h, ParticleSys.h, Shell.h, TerrainRoads.h, Water.h, GameResultsThread.h, LANAPI.h, NetworkInterface.h, WebBrowser.h; audio AudioAffect.h, GameAudio.h; refs TheAudiox9, TheCommandListx5, TheFXListStorex2, TheGameClientx4, TheGameTextx3, TheMessageStreamx4, TheNetworkx13, TheParticleSystemManagerx3, TheRadarx3, TheRecorderx10, TheTacticalViewx1, TheWindowManagerx1 | none: interface; logic tests record its calls |
| `gamelod` | C | Include/Common/GameLOD.h<br>Source/Common/GameLOD.cpp | 252+707 | 4 | userpreferences | gameengine, gamelogic, instantdeathbehavior, objectcreationlist, slowdeathbehavior | inc GameClient.h, ParticleSys.h, TerrainVisual.h; refs TheGameClientx5, TheTerrainVisualx4 | none: interface; logic tests record its calls |
| `gamemusic` | C | Include/Common/GameMusic.h<br>Source/Common/Audio/GameMusic.cpp | 118+130 | 4 | ini | - | audio AudioEventRTS.h, AudioRequest.h, GameAudio.h, GameMusic.h; refs TheAudiox4 | none: interface; logic tests record its calls |
| `gamesounds` | C | Include/Common/GameSounds.h<br>Source/Common/Audio/GameSounds.cpp | 103+332 | 14 | gametype, partitionmanager, player, playerlist, subsysteminterface | - | audio AudioEventInfo.h, AudioEventRTS.h, AudioRequest.h, GameAudio.h, GameSounds.h; refs TheAudiox10 | none: interface; logic tests record its calls |
| `gamespymiscpreferences` | C | Include/Common/GameSpyMiscPreferences.h | 61+0 | 4 | userpreferences | gamelogic, userpreferences | none | none: interface; logic tests record its calls |
| `ghostobject` | C | Include/GameLogic/GhostObject.h<br>Source/GameLogic/Object/GhostObject.cpp | 108+224 | 12 | gamelogic, object, snapshot, xfer | gamelogic, gamestate, partitionmanager, terrainlogic | refs TheGhostObjectManagerx2 | none: interface; logic tests record its calls |
| `ignorepreferences` | C | Include/Common/IgnorePreferences.h | 86+0 | 4 | userpreferences | userpreferences | none | none: interface; logic tests record its calls |
| `ladderpreferences` | C | Include/Common/LadderPreferences.h | 80+0 | 4 | userpreferences | userpreferences | none | none: interface; logic tests record its calls |
| `mapreaderwriterinfo` | C | Include/Common/MapReaderWriterInfo.h | 131+0 | 0 | - | datachunk, polygontrigger | none | none: interface; logic tests record its calls |
| `miscaudio` | C | Include/Common/MiscAudio.h | 79+0 | 0 | - | - | audio AudioEventRTS.h | none: interface; logic tests record its calls |
| `osdisplay` | C | Include/Common/OSDisplay.h | 65+0 | 0 | - | gameaudio | none | none: interface; logic tests record its calls |
| `quickmatchpreferences` | C | Include/Common/QuickmatchPreferences.h | 82+0 | 4 | userpreferences | userpreferences | none | none: interface; logic tests record its calls |
| `quotedprintable` | C | Include/Common/QuotedPrintable.h<br>Source/Common/System/QuotedPrintable.cpp | 40+205 | 0 | - | userpreferences | none | none: interface; logic tests record its calls |
| `radar` | C | Include/Common/Radar.h<br>Source/Common/System/Radar.cpp | 299+1568 | 14 | containmodule, gamelogic, gamestate, globaldata, object, partitionmanager, player, playerlist, stealthupdate, subsysteminterface, terrainlogic, thingtemplate, ... (13) | aiupdate, battleplanupdate, bridgebehavior, converttocarbombcratecollide, converttohijackedvehiclecratecollide, dozeraiupdate, gameengine, gamelogic, ... (33) | inc Color.h, ControlBar.h, Display.h, Drawable.h, Eva.h, GameWindowManager.h, InGameUI.h; audio GameAudio.h, MiscAudio.h; refs TheAudiox9, TheControlBarx1, TheEvax2, TheInGameUIx5, TheRadarx2, TheWindowManagerx1 | none: interface; logic tests record its calls |
| `statscollector` | C | Include/Common/StatsCollector.h<br>Source/Common/StatsCollector.cpp | 123+425 | 14 | filesystem, gamelogic, globaldata, money, object, player, playerlist | gamelogic, gamelogicdispatch | inc MapUtil.h, LANAPICallbacks.h, NetworkUtil.h; refs TheLANx3, TheNetworkx2 | none: interface; logic tests record its calls |
| `userpreferences` | C | Include/Common/UserPreferences.h<br>Source/Common/UserPreferences.cpp | 152+968 | 14 | custommatchpreferences, gamespymiscpreferences, ignorepreferences, ladderpreferences, multiplayersettings, player, playertemplate, quickmatchpreferences, quotedprintable, stltypedefs | custommatchpreferences, gameaudio, gameengine, gamelod, gamespymiscpreferences, globaldata, ignorepreferences, ladderpreferences, ... (9) | inc ChallengeGenerals.h, MapUtil.h, PeerDefs.h; refs TheGameSpyInfox4 | none: interface; logic tests record its calls |
| `version` | C | Include/Common/version.h<br>Source/Common/version.cpp | 75+183 | 0 | - | commandline, gameengine, globaldata, recorder | inc GameText.h; refs TheGameTextx8 | none: interface; logic tests record its calls |
| `cdmanager` | R | Include/Common/CDManager.h<br>Source/Common/System/CDManager.cpp | 191+297 | 11 | asciistring, gamelogic, list, subsysteminterface | - | none | none: replaced |
| `commandline` | R | Include/Common/CommandLine.h<br>Source/Common/CommandLine.cpp | 36+1297 | 7 | archivefilesystem, localfilesystem, version | - | inc GameText.h, TerrainVisual.h | none: replaced |
| `copyprotection` | R | Include/Common/CopyProtection.h<br>Source/Common/System/CopyProtection.cpp | 55+233 | 0 | - | - | none | none: replaced |
| `crcdebug` | R | Include/Common/CRCDebug.h<br>Source/Common/CRCDebug.cpp | 128+273 | 11 | asciistring, gamelogic | - | inc InGameUI.h, IPEnumeration.h; refs TheInGameUIx2 | none: replaced |
| `criticalsection` | R | Include/Common/CriticalSection.h<br>Source/Common/System/CriticalSection.cpp | 108+39 | 0 | - | - | none | none: replaced |
| `debug` | R | Include/Common/Debug.h<br>Source/Common/System/Debug.cpp | 248+788 | 2 | unicodestring | - | inc GameText.h, Keyboard.h, Mouse.h; refs TheGameTextx3, TheKeyboardx2, TheMousex2 | none: replaced |
| `gamememory` | R | Include/Common/GameMemory.h<br>Source/Common/System/GameMemory.cpp | 905+3583 | 6 | errors, globaldata | - | inc ClientRandomValue.h | none: replaced |
| `list` | R | Include/Common/List.h<br>Source/Common/System/List.cpp | 181+450 | 0 | - | cdmanager | none | none: replaced |
| `perfmetrics` | R | Include/Common/PerfMetrics.h | 48+0 | 0 | - | - | none | none: replaced |
| `perftimer` | R | Include/Common/PerfTimer.h<br>Source/Common/PerfTimer.cpp | 332+682 | 11 | gamecommon, gamelogic, globaldata | - | inc DebugDisplay.h, Display.h, GraphDraw.h | none: replaced |
| `registry` | R | Include/Common/Registry.h<br>Source/Common/System/registry.cpp | 55+197 | 2 | asciistring | - | none | none: replaced |
| `stackdump` | R | Include/Common/StackDump.h<br>Source/Common/System/StackDump.cpp | 72+635 | 0 | - | - | none | none: replaced |
| `systeminfo` | R | Include/Common/SystemInfo.h | 37+0 | 0 | - | - | none | none: replaced |
| `unittimings` | R | Include/Common/UnitTimings.h | 39+0 | 0 | - | - | none | none: replaced |

<!-- --- p1w2-science --- -->
## Wave 2: Science and RankInfo (`p1w2-science`)

**Translated:** `common::science` (`ScienceType`, `ScienceVec`, `ScienceInfo`,
`ScienceStore`, the `Science` block, `INI::parseScience`,
`INI::parseScienceVector`, `INI::scanScience`, `INI::parseAndTranslateLabel`,
`Xfer::xferScienceType`/`xferScienceVec`) and `logic::rank_info` (`RankInfo`,
`RankInfoStore`, the `Rank` block). Verified by `tests/differential_science.rs`
against the unchanged `Science.cpp`, `RankInfo.cpp`, `INI.cpp`, `Xfer.cpp`.

**Moved to layer 7 (take a `Player`):** `ScienceStore::playerHasPrereqsForScience`,
`playerHasRootPrereqsForScience`, `getPurchasableSciences`.

**Choices:** `TheGameText->fetch` is a `GameTextFetch` argument (display strings stay
`UnicodeString`); a prerequisite cycle made by overrides recurses for ever in
`addRootSciences` and is `Undefined::ScienceCycle`; a failed `Science` block stays in the
store (with `SCIENCE_INVALID`), a failed new `Rank` block is not added.
<!-- --- end p1w2-science --- -->
<!-- --- p1w3-thing --- -->
## Layer 5, part: Thing and SimpleObjectIterator (p1w3-thing)

Translated: `logic::thing` (`Thing.h`, `Thing.cpp`) and `logic::object_iter`
(`ObjectIter.h`, `SimpleObjectIterator.cpp`), upstream commit `0a05454d`.
Cut points: the template and its kind-of queries (`ThingTemplateView`); the
terrain and `TheGlobalData->m_gravity` (`ThingContext`); `reactToTransformChange`
(returned `TransformChange`); the bridge-aware height calculation (`_by`
closures); the object's build cost (closure of `sort`).

Results (2026-10-18): `Thing` 4,000 programs, 30,023 operations, 0 mismatches;
`SimpleObjectIterator` 6,000 programs, 124,002 operations, 0 mismatches. Planted
bugs: 19 of 19 `Thing` and 8 of 8 iterator comparisons caught.

Upstream quirk kept, not fixed: `setOrientation` on a
`KINDOF_STICK_TO_TERRAIN_SLOPE` thing caches the position from before the
terrain alignment, so `getPosition()` can disagree with the transform's
translation. The differential test pins this behaviour.
<!-- --- end p1w3-thing --- -->

<!-- --- p1w3-playertemplate --- -->
## Wave 3: PlayerTemplate (`p1w3-playertemplate`)

**Translated:** `common::player_template` (`PlayerTemplate`, `PlayerTemplateStore`, the
`PlayerTemplate` block, `parseStartMoney`, `parseProductionCostChange`,
`parseProductionTimeChange`, `parseProductionVeterancyLevel`). Verified by
`tests/differential_player_template.rs` against the unchanged `PlayerTemplate.cpp`, `INI.cpp`,
`Science.cpp`, `Money.cpp`, `Handicap.cpp`.

**Moved to layer 7 (take a `Player` or an `Object`):** none. `PlayerTemplate.cpp` includes
`Common/Player.h` and does not use it; `Player` reads the getters.

**Choices:** the five image getters return the names (`TheMappedImageCollection` is the client);
`StartMoney`'s deposit drops its cash sound and `recordIncome()` (`ThePlayerList` does not exist
yet when the templates are read, see the module docs); `findPlayerTemplate`'s fifteen
`static NameKeyType`s are kept in the store, looked up on the first call and never again, so one
store models the process; a block named like one of the old maps' twelve sub-factions updates the
base faction's template and renames it to the name as written.
<!-- --- end p1w3-playertemplate --- -->

<!-- --- p1w4-sides --- -->
## Wave 4: SidesList and the Scripts data model (`p1w4-sides`, 2026-10-18)

**Translated (layer 6):** `logic::sides_list` (`SidesList`, `SidesInfo`, `TeamsInfo`/`TeamsInfoRec`,
`BuildListInfo`; `ParseSidesDataChunk`, `WriteSidesDataChunk`, `validateSides`, `addPlayerByTemplate`,
`prepareForMP_or_Skirmish` with `ParsePlayersDataChunk`/`ParseTeamsDataChunk`, the finders, `removeSide`/`removeTeam`),
`logic::scripts` (`ScriptList`, `ScriptGroup`, `Script`, `OrCondition`, `Condition`, `ScriptAction`, `Parameter`,
`Template`; every `Parse*DataChunk` and `Write*DataChunk`, the repairs of old files, `duplicate`, `duplicateAndQualify`,
`qualify`, `updateFrom`, the list editing functions), and `formats::data_chunk_output` (the writer half of
`DataChunk.cpp`, which the Phase 0 reader left out). `SidesList.cpp` 1,168 lines + `.h` 396, `Scripts.cpp` 2,718 + `.h`
1,141.

**Cut points (listed once, as the task asks):**

1. **`ScriptEngine::init`'s tables** (layer 11): the 109 condition and 344 action templates (internal name key,
   parameter types) are the input `ScriptTemplates`. Nothing here evaluates a script.
2. **Editor text:** `getUiText`, `getUiStrings` of `Script`, `Condition`, `ScriptAction`, `Parameter`; the
   `Surfaces` and `ShakeIntensities` tables.
3. **Save games:** the `Snapshot` methods (`crc`, `xfer`, `loadPostProcess`) of `SidesList`, `BuildListInfo`,
   `ScriptList`, `ScriptGroup`, `Script`.
4. **UI to engine:** `SignalUIInteraction` and `TheShellHookNames`.
5. **INI:** `BuildListInfo::parseStructure` (the AI's `BuildList` block; `AISideBuildList` is layer 10).
6. **The file system:** the skirmish scripts file is bytes (`SidesCtx::skirmish_scripts_file`).
7. **Players and templates are names:** a side is a dictionary (`playerName`, `playerFaction`, ...), a build list entry holds
   the building's and the thing template's names, a team its owner's name. No `Player`, `Team`, `ThingTemplate` is created
   or looked up (layers 7 and 5).
8. **Memory pools and the renderer:** `newInstance`/`deleteInstance`, `BuildListInfo::m_renderObj`/`m_shadowObj`.

**Shape:** linked lists are vectors (`m_nextScript`, `m_nextGroup`, `m_nextOr`, `m_nextAndCondition`, `m_nextAction`,
`m_nextBuildList`); `m_parms` stays an array of up to 12 optional parameters with a signed count (the parse leaves slots
empty and counts negative); the three enumerations a file's `readInt` is cast to are `i32`; the process-wide state
(`ScriptList::m_curId`, `s_readLists`/`s_numInReadList`, `static_readPlayerNames`, `TheSidesList`, the name keys) is an
explicit context (`SidesCtx`, `ScriptsState`) that tests keep across cases, as the original keeps it. `userData` of the
nested `parse` calls is the object last added to the list being read.

**Differential test:** `tests/differential_sides_scripts.rs` (methodology in its header). `SidesList.cpp` (one mechanical edit
by `build.rs` in a copy, asserted to match once: the call that opens the skirmish scripts file) and `Scripts.cpp`
(unchanged) are compiled with the Phase 0 `DataChunk.cpp` and `NameKeyGenerator.cpp` under `differential/shim_sides/`, driven by
`differential/wrap_sides.cpp`. Result (2026-10-18, `cargo test --release --features differential --test
differential_sides_scripts`, seed fixed): **8,000 synthetic files generated** (6,000 `SidesList` files, 2,000 `PlayerScriptsList`
files), **7,014 compared, 0 mismatches**, 129.7 MB of log (dumps of every field before and after up to 8 operations, the bytes
written, the group counter), 18.1 MB of written chunk files compared byte for byte. The parse succeeded in 6,525 files,
stopped (returned false) in 262 and threw in 227 (`ERROR_BUG` 171, an unknown `KIND_OF` name, `ERROR_CORRUPT_FILE_FORMAT`
55, `ERROR_OUT_OF_MEMORY` 1); 981 files were **undefined upstream** and not given to the C++ (`ParameterIndexOutOfRange` 474,
`UninitialisedRead` 158, `NullParameter` 104, `ActionTypeOutOfRange` 85, `ConditionTypeOutOfRange` 46, `NameTooLong` 42,
`ParameterTypeOutOfRange` 35, `KindOfIndexOutOfRange` 24, and 13 with a string holding a 0, see below), 5 had a table of
contents declaring more than 10^5 names. What the compared files held: 15,267 sides, 20,076 build list entries, 18,583
script lists, 46,639 scripts, 17,995 groups, 67,834 conditions (11,749 made false by a bad count or an unknown name), 114,615
actions (19,537 made no-ops), 275,851 parameters; each of the 26 operations ran 165 to 713 times; `prepareForMP_or_Skirmish` read
the standard scripts file 100 times and gave scripts to a skirmish side 40 times. The constants (92: the enumerators the
translation names, `NUM_ITEMS` of the three tables, `MAX_PARMS`, `KINDOF_COUNT`, `OBJECT_STATUS_COUNT`) are compared with
the C++ by `constants_match_upstream`.

**Planted bugs** (a deliberate error in the translation, one at a time, to see the comparison fail; run on 2,000 files
generated by the same seed (`SIDES_CASES0=1500 SIDES_CASES1=500`, 1,734 compared), all removed afterwards):

| planted | files that caught it |
|---|---|
| build entry `z` not forced to 0 (parse field) | 928 |
| `TEAM_FOLLOW_WAYPOINTS` repair defaults its flag to 0 (old-file repair) | 224 |
| `whiner`/`unsellable` written in the other order (written bytes) | 816 |
| no `validateSides` after the write (state after the write) | 297 |
| `ScriptGroup::duplicate` takes no number of the group counter (counter) | 1,731 |
| allies/enemies keep the player itself (`validateSides`) | 510 |
| `qualify` of a `SIDE` compares the unqualified name | 9 |
| `static_readPlayerNames` not cleared after the skirmish file | 3 |
| `health` read as its absolute value | 211 |
| type tag in `keyAndType` off by one (writer) | 1,573 |
| `CRUSHER` read to index 1 instead of the current index | 23 |
| `Fundamentalist` replaced by `Gla` | 549 |
| version 1 condition repair adds `SURFACES_ALLOWED` 2 instead of 3 | 219 |
| `addScript` with a negative position goes to the front | 43 |
| `WriteParameter` leaves the `KIND_OF` string | 76 |
| leftover read lists not dropped by the next `ParseScriptsDataChunk` | 3 |
| a constant (`PERCENT` 50 instead of 51) | 1 of 92 |

(Two other plants, reading the build entry's extra fields only from version 4 and leaving the count of the version 1 condition
repair at 2, made the Rust choose operations the C++ then ran into undefined behaviour with: the test process died with
SIGSEGV. They are not in the table.)

**Which numbers to quote:** the 7,014 compared files and 0 mismatches; the 981 undefined files are not a result about the
translation (the Rust refuses them, the C++ would read through a null or out-of-bounds pointer or an unspecified enum).

**Upstream behaviour found and kept (all pinned by the test):**

- `PlayerScriptsList` is written with version **5**: `K_SCRIPTS_DATA_VERSION_1` follows `K_SCRIPT_CONDITION_VERSION_4 = 4`
  in an enum.
- A condition or action whose name key matches no template becomes `CONDITION_FALSE` / `NO_OP`, but its parameters are read
  by the count in the file and compared with the **last** template's count, because the search loop leaves `ct`/`at` there.
- The version 1 repair `m_parms[m_numParms] = SURFACES_ALLOWED; m_numParms = 3` sets the count to 3, not one more; with a
  count of 0 or 1 it leaves empty slots below the count (writing such a condition dereferences `NULL`).
- `KIND_OF_PARAM` `MISSILE`: the retry loop compares the string with the literal it was just set to, so it stops at index 0
  and the outer loop goes on from 1 until it meets `SMALL_MISSILE`'s real index; `CRUSHER` and the others take the current
  index, which is 0 (the check is made on the first pass).
- `ParseSidesDataChunk` breaks out of its loop at 16 sides but not out of the file: the unread sides' bytes are read as the
  teams and scripts that follow. `ParseScriptsDataChunk` returns false for a 17th script list, which `ParseSidesDataChunk`
  turns into `ERROR_CORRUPT_FILE_FORMAT`; the previous read lists are gone but their count stays.
- `ParseScript` returns `NULL` if its children fail to parse, and both callers store the result.
- `WriteSidesDataChunk` runs `validateSides` after the chunk is closed (it can change the list the caller still holds);
  `WriteParameter` replaces the string of a `KIND_OF_PARAM`.
- `SidesInfo::operator=` copies the script list with `ScriptList::duplicate`, which takes a number of the group counter per
  group, so `removeSide` and `prepareForMP_or_Skirmish` advance the counter; `operator=` on itself does nothing.
- `validateSides` restarts its team-name check with a `goto` after `removeTeam`, which refuses to remove the last team: a
  list with one team named like a player would loop forever (unreachable after the default teams are made; the Rust returns
  `MapUndefined::NonTermination`).
- `Condition::duplicate` after the first condition of a chain copies as many parameters as the original has, from slots
  that exist only up to the template's count: a condition made false by an unknown name keeps the file's count and its
  copy dereferences `NULL`.

**Harness record.** New: `differential/shim_sides/` (stand-ins for `PlayerTemplate.h`, `AI.h`, `Radar.h`, `ThingTemplate.h`,
`ContainModule.h`, `View.h`, `Player.h`, `Object.h`, `NetworkDefs.h`, `KindOf.h`, `GameState.h` with `SaveCode`, `ScriptEngine.h`
with the two tables; each says what it stands in for), `differential/wrap_sides.cpp`, one `build.rs` block (the lib is
compiled ahead of `upstream_chunk`, `-fno-delete-null-pointer-checks` because upstream calls `deleteInstance()` on `NULL`).
Added to shared shims, each in a `p1w4-sides` block: `AsciiString::startsWith` and `nextToken(tok)`,
`UnicodeString::translate(AsciiString)`, `Dict::clear`/`getBool`/`getAsciiString` of the chunk build's stand-in, the declaration of
`KindOfMaskType::getBitNames` (defined in `wrap_sides.cpp` over the real name table). `wrap_chunk.cpp`'s
`getPath_UserData` stand-in, which aborted, calls a hook (`ref_chunk_user_data_path_hook`) that `wrap_sides.cpp` installs, so
`DataChunkOutput` can write its temporary file. **Limit of the stand-in:** the `std::string` `AsciiString` of this build counts
bytes past an embedded 0 in `getLength()` where the engine's `strlen`s, so a file with a string holding a 0 would make the C++
write more than the engine would; `DataChunkInput::embedded_nul_strings` (added, additive) counts such strings and the test
leaves those files out (13 of 8,000).
Knobs of the test: `SIDES_CASES0`, `SIDES_CASES1` (files of each kind), `SIDES_NOOPS` (no operations),
`SIDES_DUMP_DIR` (write the logs of a mismatching file there).
<!-- --- end p1w4-sides --- -->
<!-- --- p1w4-terrain --- -->
## Layer 6, part: TerrainLogic (p1w4-terrain)

Translated: `logic::terrain_logic` (`TerrainLogic.h`, `TerrainLogic.cpp`: the
map-chunk parse of waypoints, links, triggers and bridges, height-map ground
queries, bridges and their layers, water areas and water changes over time,
waypoints and paths, flatten and crater, `xfer`), `logic::terrain_visual` (the
logic-owned state of what `TheTerrainVisual` is read for, and the client
events for what it is written), and the petir-side helpers `ww::plane` and
`ww::tri`. Upstream commit `0a05454d`.

**Corrected count.** ~~`TerrainLogic` calls `TheTerrainVisual` 32 times, with
`setRawMapHeight` 20~~ **CORRECTED 2026-10-18**: 32 calls in total
(`grep -c TheTerrainVisual`), of which `setRawMapHeight` is **19** (the 20th
grep hit is a comment at line 2717). The three reads are as the paragraph above
says.

**Water grid and raw heights (recorded, NOT decided).** The Rust holds both as
logic state: `WaterGrid` (transform, resolution, mesh heights, clamps) and
`LogicHeightMap` (raw heights, lowered only by `setRawMapHeight`). Where
upstream reads them: `getWaterGridHeight` 2164 and 2222, `getWaterTransform`
2316 and 2515, `getWaterGridResolution` 2502, `getRawMapHeight` 2899. What the
client would otherwise supply: the mesh vertex heights, which
`WaterRenderObjClass::update()` simulates **per client frame** (only on a
client frame whose logic frame differs from the last seen), so `isUnderwater`
over a grid would depend on the client's frame rate. That simulation is not
translated; heights are set by the embedder or stay 0. Undecided: whether the
grid becomes simulated logic state or is shown irrelevant. Not checked: whether
any shipped map has a `WaveGuide1` waypoint and is in
`m_vertexWaterAvailableMaps` (the map files are not here). Until then a grid
water map is not lockstep-safe. A fourth read lives outside `TerrainLogic.cpp`
(`W3DTerrainLogic` ground height through the render object, which reads the
same logic height map; translated). `isCliffCell` and `isClearLineOfSight` are
not translated.

**Cut points.** Writes to `TheTerrainVisual` are `TerrainVisualEvent`s (load,
enable grid, height clamps, transform, resolution, attenuation, raw height).
Calls into other layers go through the `TerrainWorld` trait (no trait object;
a type parameter): object lookup and position/orientation/layer reads, bridge
tower and landmark objects (`ThingFactory::newObject`, `ThingTemplate`
lookups), `TheAI->pathfinder()` (layer changes, wall height), the partition
manager (`unRegisterObject`/`registerObject` of bridge objects), body-module
damage and repair queries, `TheGameLogic->getFrame()`, and the boundary-change
notification. `PolygonTrigger` and `MapObject` are minimal stand-ins (`TriggerArea`,
`WaypointMapObject`) to be replaced when the sibling `PolygonTrigger`/`MapObject`
work lands.

**Choices.** Lists are `Vec`s in upstream order (waypoints walked in reverse,
bridges and triggers head first). Undefined-upstream inputs are `Err` and are
counted (`undefined_inputs`): an uninitialised `pickBridge` position, a stale
water handle, a missing water transform, and 11 others.

**Upstream quirks kept and pinned:** the `update()` copy loop that removes a
finished water entry (copies entry `j` over `i` for every `j`); `setGridResolution`
comparing `m_gridCellsY != m_gridCellsY`; `flatten` and `crater` loops starting
`j` at 0; the `tri.h` degenerate case; the swapped comments of `isCellOnEnd` and
`isCellOnSide`.

**Results (2026-10-18, `tests/differential_terrain_logic.rs`, 8 tests):** 24,000
programs, 22,139 compared, 346,279 operations, 0 mismatches in results, the
world log or the client-event log; floats bitwise. 11 planted bugs: 10 caught
(two only after the generator was strengthened), 1 not caught (a 0.001 bias in
a tie-break). Table in the test's module doc.
<!-- --- end p1w4-terrain --- -->
