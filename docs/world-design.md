# The world core (`logic::world`), layers 7-8

Wave 5, branch `p1w5-world`. Read this before translating `Player`, `Team`,
`Object`, `Weapon`, `Locomotor`, `PartitionManager` or any object module. The
code is in `src/logic/world/`; each file's module docs are the reference for
what it translates from `GameLogic.cpp`, `Object.cpp`, `PlayerList.cpp` and
`Team.cpp`.

## The design in one page

1. **One context, `World`, passed as `&mut World`.** It replaces the globals
   `TheGameLogic` (`getFrame` 553 calls, `findObjectByID` 373, `destroyObject`
   86, `getFirstObject` 28), `ThePlayerList`, `TheTeamFactory` and
   `TheTerrainLogic`, and owns the client-event queue. Its fields are public
   and disjoint (`objects`, `sched`, `players`, `teams`, `terrain`, `events`,
   `reads`), so code inside `World` methods borrows them separately; code
   outside calls `world.method(..)`.
2. **Pointers become ids; objects live in an arena.** `ObjectArena` is
   `m_objList` (an intrusive list, **newest first**, links held as ids beside
   the object) plus `m_objVector` (the id table, 8192 entries, doubling).
   `findObjectByID` is `world.find_object(id)`; it **finds destroyed objects**
   until the end of the frame, exactly as upstream, so callers test
   `is_destroyed()`. Ids come from a counter starting at 1 and are never
   reused. Players are an array of 16 indexed by player index; teams are
   ordered maps keyed by `TeamId`, with `TeamFactory`'s pre-incremented
   counters.
3. **Destruction is deferred as upstream does it.** `destroy_object` runs its
   steps in upstream's order (module `onDestroy`, mark destroyed, stop AI,
   queue, `Object::onDestroy`, wall piece, UI) and only queues; the object
   leaves the list, the table and the scheduler in `process_destroy_list`,
   which `end_of_frame` runs (entries queued during the pass are visited in
   the same pass).
4. **Update order is simulation state, so the sleepy heap is translated step
   for step.** `SleepyScheduler` is `m_sleepyUpdates`: equal keys are called in
   the order the heap's swaps left them, and the erase order in
   `processDestroyList` (last collected first) shapes it. It is verified word
   for word against upstream's own text (below).
5. **Re-entrancy by checkout.** A running update module is *checked out* of its
   slot (`World::with_body`): its scheduling half (`UpdateModule`: key and heap
   index) stays in the arena, its behaviour half (an `UpdateBody` enum value)
   is moved out for the duration of the call. So `body.update(&mut World, me)`
   has `&mut self` and `&mut World` without aliasing, can reach its own object
   and every other object by id, can create, destroy and wake things, and the
   scheduler can re-heapify under it. The same pattern is
   `common::state_machine`'s (callbacks receive the machine and the context
   separately and re-fetch their state by id at each use); a state machine's
   `Ctx` is `World` (it implements `StateContext`) and its owner is
   `world.owner()` (set by the update loop, or by `world.with_owner(id, ..)`
   for a call from outside an update).
6. **One client interface.** `ClientEvent` (`logic::client_event`) is the only
   thing logic says to the client; `world.events` is the per-frame queue, which
   the client drains with `world.take_client_events()` after each logic frame.
   `TerrainVisualEvent` nests as `ClientEvent::Terrain`.
   Reads in the other direction are `world.reads` (`ClientReads`) or game
   setup, never a call (table below).

## Why these choices

- **Arena with ids, not `Rc`/`Arc` graphs.** The CRC, save files, scripts and
  the network all already speak `ObjectID`; a pointer between objects is a
  dangling-pointer bug waiting to be translated. An id that no longer resolves
  is `None`, which is exactly what upstream's `findObjectByID` returns.
- **Newest-first list.** `getFirstObject()` loops are everywhere (CRC, terrain
  bridge scans, script conditions, the disabled pass). A different order
  changes results that depend on iteration order and the CRC of the world.
- **Checkout instead of `RefCell`/locks.** Interior mutability would turn
  upstream's re-entrancy (an update destroying an object that is mid-update, a
  damage call landing on the object that fired) into a runtime panic. With
  checkout the only unavailable thing is the *behaviour half of the module that
  is running*, which upstream also guards (`m_curUpdateModule`). A call that
  needs that same module while it runs gets `None` from `with_body` (it must
  defer; see "Open questions").
- **`World` has public fields, no lifetimes, no trait objects.** Enum
  dispatch (`UpdateBody`), generics for closures (`with_body`, `with_owner`).
- **Terrain is a field, checked out for its own update.** `TerrainLogic`
  mutates itself through a `TerrainWorld`; the full-game adapter (layer 12)
  implements `TerrainWorld` over `World` and takes the terrain out with
  `std::mem::take(&mut world.terrain)` for the call (it has a `Default`), the
  same pattern as module bodies. World methods that need the terrain read it
  as a sibling field.

## How later layers plug in

| you translate | you do |
|---|---|
| `Object` (layer 8) | Grow `world::object::Object` in place (it is the shell). Keep the contract below. Its `ThingContext` (ground height, gravity) is built from `world.terrain` and global data as a sibling-field borrow inside a `World` method. |
| an update module (layer 9) | Add a variant to `UpdateBody` and an arm to `UpdateBody::update`, in a `// --- p1wN-name ---` block. Its `update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime` is your port. |
| `Player`, `Team` (layer 7) | Fill `world::player::Player`, `world::team::{Team, TeamPrototype}` in place; keep the arena APIs (`get_nth`, `set_local_player`, `allocate_team_id`, `recover_counters`). Their `becomingLocalPlayer(..)`, EVA, radar and UI calls are `ClientEvent`s. |
| `Weapon`, `Locomotor`, `PartitionManager` | Plain functions or structs taking `&mut World`/`&World` plus ids. `PartitionManager` becomes a field of `World` (add it in a marker block) and its queries return ids. **`PartitionManager` is done (wave 6, `p1w6-partition`): `world.partition`, see "The partition manager" below.** |
| a hook | The `World::hook_*` methods are the places `destroyObject`, `processDestroyList` and the end of frame call into later layers; fill them in place. `Hook` and `world.hook_log` pin the order in tests. |
| a new client call | Add a `ClientEvent` variant naming the upstream call it replaces. |

### What the Object translator must keep

1. `Object.id`, `.status` (the `DESTROYED` bit), `.disabled` and the
   per-behaviour `BehaviorSlot { update: UpdateModule, body: Option<UpdateBody> }`
   for every behaviour with an update interface, in module order. The
   scheduler addresses a module as `UpdateHandle { object, index }`, `index`
   being the position in `Object::behaviors`; if `behaviors` grows other
   module kinds, keep `UpdateHandle.index` a position in the update-capable
   list, or change `ObjectArena::update_module{,_mut}`/`take_body`/`put_body`
   (the only four places that decode it).
2. The list links are the arena's, not the object's. Do not add
   `m_prev`/`m_next`.
3. An object under construction is **not in the arena**: allocate its id
   (`world.allocate_object_id()`), build it and its modules (a module sets its
   wake frame directly on its own `UpdateModule`), then `world.register_object(obj)`
   as `Object`'s constructor ends with `TheGameLogic->registerObject(this)`.
4. `~Object` is `World::hook_delete_instance(obj)` (the object is already out
   of the list and table). Keep the `ObjectDestroyed` event it pushes.

### Worked example: an update module wakes, finds another object, damages it, emits an event

The executable form is `ProbeUpdate` (`world/update_body.rs`) and the tests
`a_module_can_wake_another_and_a_spawn_joins_the_queue` and
`damage_through_the_cut_point_is_recorded_on_the_target` in `game_logic.rs`.
The shape a real module takes (a made-up `FlareUpdate`, one variant of
`UpdateBody`):

```rust
// in UpdateBody:  Flare(FlareUpdate),
// in UpdateBody::update:  UpdateBody::Flare(f) => f.update(world, me),

impl FlareUpdate {
    /// Runs when the scheduler's heap says this module is due.
    pub fn update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        // 1. other objects are found by id; `None` is upstream's NULL
        let Some(target) = world.find_object(self.target) else { return UPDATE_SLEEP_FOREVER };
        if target.is_destroyed() { return UPDATE_SLEEP_FOREVER; }

        // 2. damage goes through the cut point (BodyModule::attemptDamage once layer 9 lands)
        let mut info = DamageInfo::default();
        info.input.source_id = me.object;
        info.input.amount = self.damage;
        world.attempt_damage(self.target, &mut info);

        // 3. wake a neighbour module: setWakeFrame = now + delay, then the heap is fixed
        let when = UpdateModule::set_wake_frame(world.frame(), UpdateSleepTime(2));
        let _ = world.awaken_update_module(self.sibling.object, self.sibling, when);

        // 4. tell the client; logic never sees the answer
        world.events.push(ClientEvent::Audio {
            event_name: b"FlareHit".to_vec(), object_id: me.object, player_index: -1 });

        // the return value is the next wake: sleep 10 frames (a value below 1 means 1)
        UpdateSleepTime(10)
    }
}
```

What happens around it, per frame (`World::run_logic_frame`): the loop peeks the
heap; while the top is due it checks the owner's disabled mask, sets
`m_curUpdateModule`/`owner`, checks the body out, calls `update`, puts it
back, sets `next_call = now + returned sleep` and rebalances index 0. A module
that calls `awaken_update_module` on itself is ignored (its return value wins);
on another module it re-heapifies at that module's index. `destroy_object` on
the target made earlier in the frame is *visible* (still found, marked
destroyed) until `end_of_frame`.

## The client interface

`ClientEvent` variants so far: `MoneySound` (layer 4), `ObjectCreated`
(layer 5), and from this wave `ObjectDestroyed` (`sendObjectDestroyed` ->
`destroyDrawable`), `Audio` (`addAudioEvent`, name + object + player) and
`Terrain(TerrainVisualEvent)`. Everything else in the porting doc's lists is
added by the first caller, naming the upstream call. `ClientEvent` lost
`Copy`, `Eq` and `Hash` (the nested terrain events carry floats); no code used
them. `TerrainLogic` keeps its own queue (its differential tests read it);
`World::flush_terrain_events` (called by `take_client_events`) moves its events
onto the world queue in order. `Money::deposit/withdraw` and
`ThingFactory::new_drawable` already take `&mut Vec<ClientEvent>`; pass
`world.events.as_vec_mut()`.

### Reads that feed logic (every one from the porting doc, and where it went)

| upstream read | disposition |
|---|---|
| water grid height and transform; raw map height | logic-owned already: `TerrainVisualState` (layer 6) |
| terrain tile for placement (`BuildAssistant` -> `getTerrainTile`) | logic-owned map data (the tile table is in the map, layer 6); no client call |
| `isTimeFrozen`, `isCameraMovementFinished`, tactical-view position, view filter | `world.reads.camera_*`. Headless default: not frozen, finished, origin, no filter. A client feeds them at the start of the logic frame. |
| `getAudioLengthMS` | `world.reads.audio_length_ms` (name -> ms), filled from the audio files at load, the same on every peer; unknown = 0 |
| `hasMusicTrackCompleted` | `world.reads.music_track_completions`; headless: never |
| local selection (`getAllSelectedDrawables`, `getFirstSelectedDrawable`, `getFrameSelectionChanged`, `getSelectCount`) | `world.reads.local_selection` (object ids) and `frame_selection_changed`; headless: none |
| `TheNetwork` player/slot state (`PlayerList`, 2; others) | game setup, filled when the game starts (layer 7 adds the struct); not a call |
| `TheInGameUI->isClientQuiet()` (`Player.cpp:3155`) | decides only whether to play a sound: the decision moves to the client; the audio event carries what it needs. Not logic state. |
| `TheAudio->isCurrentlyPlaying` (`FiringTracker.cpp:166`) | restarts a looping sound only: client-only; the tracker emits "start/stop looping fire sound" events and the client dedupes |
| `TheInGameUI->getInputEnabled()` (`m_inputEnabledMemory`) | UI memory; becomes a pair of events (set input enabled) and a client field |
| `TheGameText->fetch` (91 calls) | display strings only; already `GameTextFetch` at load time |
| `TheDisplay` (5), `TheGhostObjectManager` (2) | debug drawing and fog ghosts: client side, driven by object/partition events |

## Existing cut points: what was unified and what was not

- **`ClientEvent` and `TerrainVisualEvent`**: unified (nesting, above). The
  terrain differential tests are untouched; `differential_money` gained one
  match arm for the new variants.
- **`StateContext`** (`common::state_machine`): `World` implements it
  (`frame`, `find_object`). Done.
- **`TerrainWorld`** (`logic::terrain_logic`, 20 methods): *not* implemented
  on `World`, and deliberately not changed (its recorder differential test
  must stay identical). Half of it is `World` today (`frame`, `object_exists`
  = `find_object(id).is_some()`, `destroy_object`, `attempt_damage`); the rest
  needs the partition manager, pathfinder and body module (layers 8-10). The
  layer-12 `Game` adapter implements it over `World` + those.
- **`ThingContext` / `ThingTemplateView`** (`logic::thing`): not changed;
  both are borrowed views the `Thing` base asks for. `Thing`'s owner (`Object`)
  builds the `ThingContext` from `world.terrain` and global data as above.
- **`UpgradeEffects`** (`logic::module::upgrade_module`): not changed; a trait
  with a recorder implementation in tests. Its production implementer is a
  `World` method taking the object id, when `Object` lands.
- **`GameTextFetch`** (`common::science`): load-time string lookup, not a
  world concern; unchanged. (It is a `dyn` object in existing code, from before
  this wave's rule.)

## The partition manager (`logic::world::partition`, wave 6)

`PartitionManager.h/.cpp` (7,454 lines) is `world.partition`, a
`PartitionManager` (layer 8). It owns the cell grid, every object's footprint in
it, the dirty list and the collision pass, the **shroud and fog of war** and the
**threat and value maps** of every player, and the queries (closest object,
range with its sort orders and 33 filters, potential collisions, distances,
line of sight, cell walks, `findPositionAround`). Files: `collide` (shape tests,
distance procs), `cell`, `data`, `manager` (grid, registration, `update`),
`shroud`, `query`, `filters`, `xfer`, `scenario` (a table-driven host for tests).

**How it reaches the rest of the game.** Objects are `ObjectId`s. What upstream
reads from `Object`, `Player`, `Team`, `TheTerrainLogic`, `TheAI`,
`TheActionManager`, `TheGlobalData` and the client comes through two generic
traits, `PartitionHost` (the clock, the terrain, the players, an object's
position/orientation/shape/kind/status, the two re-entrant callbacks
`onPartitionCellChange` and `onCollide`, the random generator, the pathfinder
questions of `findPositionAround`) and `PartitionFilterHost` (what the 33
filters ask an object). Static dispatch, no trait objects. The `&mut self`
callbacks receive the manager (`&mut PartitionManager`) because upstream's
callee calls straight back into `ThePartitionManager` (a collision handler
unregisters objects; a cell change looks and unlooks). So the manager is **checked
out of `World` for the call**, as the terrain is:
`let mut pm = std::mem::take(&mut world.partition); pm.update(&mut adapter);
world.partition = pm;` where `adapter` borrows the other `World` fields. Writing
that adapter is the first job once `Object` has its fields (the layer-8 Object
agent, or layer 12): each method names the upstream call it replaces.

| host question | answered by |
|---|---|
| position, orientation, `GeometryInfo`, kind mask, status bits, destroyed | `Object` / `Thing` |
| player mask, count, local player, relationships, `getPlayersWithRelationship` | `Player`, `PlayerList`, `Team` (layer 7) |
| ground height, extent, line of sight, cliffs, water, layers | `TerrainLogic` (already `world.terrain`) |
| pathfinder cell types, `doesPathExist`, `isViewBlockedByObstacle` | the pathfinder (layer 10) |
| contain / stealth / body / AI facts for the filters, action-manager checks | modules (layer 9), `ActionManager`, `Squad` |
| `m_partitionCellSize`, `m_unlookPersistDuration` | global data (layer 4, `GlobalData`) |

**Cut points (client).** `TheDisplay`/`TheRadar` shroud writes and the
`TheGhostObjectManager` calls are `PartitionClientEvent`s (forwarded as
`ClientEvent::Partition` by `World::flush_partition_events`, which
`take_client_events` calls). The ghost object itself is the host's
(`add_ghost_object` decides whether a building gets one); the manager keeps the
ghost's geometry, because a ghost-only module still occupies cells, and emits
`GhostSnapShot`/`GhostFreeSnapShot`/`GhostRemoved`/`GhostParentGone`. Debug
drawing (`addIcon`, the threat-map overlay) and the perf counters are not translated.

**Simulation-state orders reproduced exactly.** Every intrusive list (the
cell's intersections, the module list, the dirty list, the contact list) is
prepended to and drained as upstream does; `m_radiusVec` is built in the
double-loop order; a range query appends to a `SimpleObjectIterator`, which
prepends, so the unsorted result is the reverse of the visit order and
`iterateAllObjects` lists the oldest registration first; the merge sort of
`logic::object_iter` decides ties.

**Upstream quirks kept, found by the differential test or by reading.** The
grid of a 400-unit map with 40-unit cells is 11 x 11, not 10 x 10:
`REAL_TO_INT_CEIL(10.0)` is `fast_float_ceil(10.0)`, which adds 0.99999994 and
rounds to 11 in `float`. `calcMaxCoiForShape` has no `break` after the sphere
and cylinder case, so every shape is sized by the box formula.
`PartitionFilterPlayerAffiliation` takes the `PlayerList.h` flags
(`ALLOW_ENEMIES = 4`), not `PartitionFilterRelationship`'s (`1 << ENEMIES = 1`).
`getNearestGroupWithValue` passes `valueRequired` where it means `greaterThan`.
`iterateCellsBreadthFirst` expands the neighbours of the cell processed *before*
the one it pops, and tests `curY + 1 > 0` for `curY + 1 < height`, which reads out
of bounds on the last row (here: skipped, and reported to the caller).
`DISABLE_INVALID_PREVENTION` is defined, so `OBJECTSHROUD_INVALID_BUT_PREVIOUS_VALID`
is never produced; `FASTER_GCO` is defined, so `CellOutwardIterator` is dead.

## Verification

- Unit tests in each file (`cargo test --release --lib world`).
- `tests/differential_world.rs`: the sleepy heap, `registerObject`, ids and
  table, `processDestroyList`, `destroyAllObjectsImmediate`, `loadPostProcess`,
  `friend_awakenUpdateModule`, the module loop and the object list, compiled
  from upstream's text (cut verbatim by `build.rs`), 3000 random programs,
  72,519 update calls (34,840 deciding a tie), 3,974 deletions, 80,462 full
  snapshots, 0 mismatches; a planted stable-sort scheduler is caught on 29,792
  of 45,446 words. Method, stand-ins and what is *not* upstream text are in the
  test's and the wrapper's headers.
- `tests/differential_partition.rs`: the whole of `PartitionManager.cpp`
  compiled under clang++ (renamed classes, an `Object` stand-in, upstream's own
  `Geometry`, `DiscreteCircle`, `BitFlags` and `SimpleObjectIterator`; the file is
  never edited, `build.rs` makes mechanical edits into `OUT_DIR`). Both sides run
  the same random worlds and a 32-operation query/mutation program. Result
  (2026-10-18, 5000 cases): 436,339 operations, 33,827,114 words compared
  (result lists in order, distances bitwise, shroud states, CRC, state dumps),
  0 mismatches; 16,697 operations withheld because upstream's behaviour is
  undefined there (see below). Planted bugs, each run at 300 cases (3000 where
  noted) and each caught, then reverted: prepend changed in a cell list, ring
  table order reversed, `addLooker` min removed, contact list oldest first,
  `calcMaxCoi` by sphere formula, ping-pong sign flipped, one CRC coordinate
  skipped, a circle row shortened, `PlayerAffiliation` with the wrong flag,
  a BFS neighbour dropped (3000 cases; first missed at 300, so the generator
  gave the nearest-group query three slots), `>=` for `>` in the value test
  (3000 cases, same fix). Not exercised as a planted bug: the
  `iterate_all_objects` insertion order (its order is compared in every case).
- Undefined upstream behaviour is withheld, not copied: the breadth-first walk
  off the last row, `restoreFoggedCells` before any store (division by zero),
  null attacker, and uninitialised collision location/normal (the C++ side is
  built with `-ftrivial-auto-var-init=zero`).
- Not compiled from upstream (hand-written stand-in, Rust unit tests): the
  hook order of `destroyObject`.

## Deliberate differences from upstream

See the module docs of `game_logic.rs` and `object.rs`: `reset()` destroys with
the id table intact; an object queued twice is deleted once; `destroyObject`
of a missing object is a no-op; ids that upstream mishandles (negative, 0, 2^28
or more, duplicates) are `ArenaError`s; `load_post_process` on an empty world
leaves the counter at 0 and the first registration is refused (`BadId(0)`),
where upstream registers an object no lookup can find.

## Left for later waves

- ~~`Object`'s real fields and `Thing` base; `BehaviorModule` lists beyond the
  update-capable ones (destroy/die/damage/create interfaces dispatch).~~
  **DONE 2026-10-18 (wave 6, `p1w6-object`)**: see "Object (layer 8)" below.
- The `hook_*` bodies of layers 7, 9 and 10 (the Object-owned ones are filled:
  destroy modules, `onDestroy`, `~Object`, `checkDisabledStatus`, stop-AI,
  wall piece, special-power UI). ~~`attempt_damage` -> `BodyModule`~~ **DONE**:
  `attempt_damage` is in `object_damage.rs` and sends `AttemptDamage` to the
  body module.
- ~~`PartitionManager` (a `World` field)~~ **CORRECTED 2026-10-18** (wave 6,
  `p1w6-partition`): translated, see "The partition manager". Still left: the
  pathfinder, the AI, `ScriptEngine`, `GameLogic::update`'s remaining steps and
  `GameLogic::xfer`/CRC (layer 12), and the `World` implementation of the
  partition host traits (now unblocked: the real `Object` landed in the same
  wave).
- ~~`Player`/`Team` contents; the `TeamFactory` creation functions.~~ **CORRECTED 2026-10-20 (`p1w9-sonnet`)** — `Team`,
  `TeamPrototype`, `TeamFactory`, `TeamRelationMap`, `TeamTemplateInfo` and `PlayerRelationMap` are translated (see
  "Player and Team shape" below). ~~`Player` and `PlayerList` contents are still left.~~ **CORRECTED 2026-10-20
  (`p1w10-sonnet`)** — `Player`'s data and the first slice of its operations are translated (below); the object-facing
  half, `xfer`, `initFromDict` and `PlayerList` are still left. ~~`initFromDict` and `PlayerList` are still left.~~ **CORRECTED 2026-10-21** — both are translated (slice 4, below).
- The `GameSetup` struct (network slots), and the audio-length table loader.
- `popSleepyUpdate` is translated but unused (upstream never calls it).

## Open questions for the lead

1. ~~**Same-module re-entrancy.** `with_body` on a module that is running
   returns `None`.~~ **CORRECTED 2026-10-18** — see "Object (layer 8)": a module call to a running module now
   runs at once through `World::lend_body`; `with_body` itself still returns `None` for a body that is out
   and has not lent itself (the update loop never meets that). Original text: `with_body` on a module that is running
   returns `None`. Upstream allows `AIUpdate::update` to cause a call back into
   its own `AIUpdate` interface (e.g. via another object's group command).
   Proposal: such cross-calls go through a small per-world "deferred command"
   queue drained after the call returns; decide when layer 10 meets the first
   real case.
2. **`ObjectCreated` has no object id**, so the client cannot pair a drawable
   with its object. Proposal: add `object_id` to it when `Object::new` lands
   (`new_drawable` is called after the id is allocated); then `ObjectDestroyed`
   is the matching end.
3. **Terrain ownership.** `world.terrain` is a field, checked out by `take`
   for its own update. If you prefer `Game { world, terrain }`, only the
   field moves.

### Decisions (lead, 2026-10-18)

1. **No deferred-command queue by default.** Upstream makes these calls
   synchronously, so deferring them would change the order of effects, which
   is simulation state. When layer 10 meets the first real case, the fix must
   keep the synchronous order (for example by routing the call to the
   checked-out body that the caller already holds). A queue is acceptable only
   if it is shown to produce the same order of effects.
2. **Yes:** `ObjectCreated` gains `object_id` when `Object::new` lands.
   **DONE 2026-10-18 (`p1w6-object`)**: `ClientEvent::ObjectCreated { template_id,
   drawable_status_bits, object_id }`, sent by `World::send_object_created`.
3. **Keep `world.terrain`** as a field, checked out for its own update.

### Player and Team shape (lead, 2026-10-20; relayed to `p1w9-sonnet`)

`Player`/`PlayerList` and `Team`/`TeamFactory`/`TeamPrototype` live in the existing `World` arenas (`PlayerArena`,
`TeamArena`); `Player` already carries Energy, ScoreKeeper, MissionStats and AcademyStats as fields, and the shells
become the real translations in place. Anything they call that is not translated yet (the SidesList glue, the AI, the
script engine, `Squad`, the client UI) is a cut point, done the way `ActionHost` and `WeaponHost` are done: a host
trait (generic, no `dyn`) that the differential fixture stands in for.

**Done for `Team` (`p1w9-sonnet`):** `logic::world::team` (data, the arena, the `DLINK` lists), `team_ops` (every member
function of `Team`, `TeamPrototype` and `TeamFactory` as a free function over `TeamHost`, a trait the owner of the
`TeamArena` implements) and `team_xfer` (save, load and post-process). `TeamHost` has the arena as two methods, so a
host call that changes team membership (`Object::setTeam`) reaches the arena through the host. The `World` adapter of
`TeamHost` is layer 12's, with the other host adapters. Until then `Object` keeps its team and member lists in
`ObjectCuts` (`team_members`, `team_players`, `team_names`), unchanged, and `TeamArena` is the data a later
adapter connects it to: **left for the adapter**, not forgotten.

**Done for `Player`, slice 1 (`p1w10-sonnet`, 2026-10-20):** `logic::world::player` (the whole data of `class Player`,
in the existing `PlayerArena`) and `logic::world::player_ops` (free functions over `PlayerHost`, a `TeamHost`
supertrait: the player arena as two methods, the global data and game state as one `PlayerGlobals` value, template
values as a `PlayerTemplateView` the adapter fills from a `PlayerTemplate`, the AI as 25 methods, the score keeper and
the academy statistics as calls, the rank and science stores, and the client as `ClientEvent::Player`). The sub-objects
that are other units and verified by their own tests are held by the player and called directly (`Money`, `Handicap`,
`Energy`, `MissionStats`); `Energy`'s `OwnerNotice` is delivered by `player_deliver_brown_out_notice`. A player's loops
walk the real team arena (its prototypes, their instance lists, the teams' member lists, with the `DLINK` semantic of
`team.rs`). `Player::new` is the constructor's field assignments; the `init( NULL )` it ends with is `player_init`,
which whoever builds the world calls for all sixteen. **Left for the layer-12 adapter**: `PlayerHost` and
`TeamHost` for `World` (the `TeamHost` player methods are one-liners over `player_ops`, see
`tests/differential_player.rs`'s `Sim`), and `PlayerTemplateView::from_template`.

**Done for `Player`, slices 2, 2b and 3 (`p1w10-sonnet`, 2026-10-20):** `player_objects` (`PlayerObjectHost`: what `Player` asks of objects, the AI, the action manager, the build assistant and the body), `player_squads` (`PlayerSquadHost`: upgrades, `Squad` including its `m_objectsCached`, which upstream leaves filled and `Squad::xfer` then refuses) and `player_xfer` (`PlayerXferHost`: the upgrade centre with the name keys, and two hooks for the units that are not in the player's own data, the score keeper and the AI). A nested `xferSnapshot` of something that needs the host is `xfer_nested`, as in `team_xfer.rs`. **Left for the layer-12 adapter:** the three host traits for `World`. **Done for `Player`, slice 4 (`p1w10-sonnet`, 2026-10-21):** `player_init` (`Player::initFromDict` over `PlayerInitHost`, 24 methods that name every question and change it makes to the sides list, the skirmish sides, the script lists and the other services) and `player_list` (`PlayerList`'s functions over the same host; the list itself stays `PlayerArena`). **Left for the layer-12 adapter:** `PlayerInitHost` for `World`, answered from the real `SidesList` and `logic::scripts`. **Done for `BuildAssistant` (`p1w10-sonnet`, 2026-10-21):** `build_assistant` (`BuildAssistantHost`, the world `BuildAssistant.cpp` reaches; `Player` enters as seven forwarders to the verified `player_*` functions, the partition manager as five queries whose iteration order is the host's, the pathfinder as one cell-type query). **Left for the layer-12 adapter:** `BuildAssistantHost` for `World` (the sell list lives in `BuildAssistant`, a field of `World`). **Done for `ObjectCreationList` (`p1w10-sonnet`, 2026-10-21):** `object_creation_list` (`OclHost`, 95 methods, each named after the upstream call it stands for: the objects and their modules, physics, the drawable and its debris interface, containment, the thing factory, the terrain, the pathfinder, the partition manager, the weapon store, particle systems, audio). Every `create` returns the first object made (upstream's `Object *`). **Left for the layer-12 adapter:** `OclHost` for `World` (the thing factory and the weapon store are not `World` fields yet; the `GenericDebris` template is cached by the host, as upstream's function-local `static` caches it) and the `OclParseEnv` (the weapon templates by name). **Open (owner):** `Squad`'s home: it is declared in `player.rs` because the player owns the ten hotkey squads; the AI group translation (layer 10) will want it as its own module.

**Upgrade batch B (`p1w10-sonnet`, 2026-10-21):** upgrade modules reach other modules and the drawable through cut calls (`CutCall::AiSetLocomotorUpgrade`, `ContainSetPassengerAllowedToFire`, `SpawnGiveSlavesStealthUpgrade`, `OclCreate`) and client events (`DrawableShowSubObject`, `DrawableUpdateSubObjects`); the layer-12 consumer of `OclCreate` looks the list up by name and does nothing for an unknown name, as `INI::parseObjectCreationList` answers NULL.

<!-- --- p1w6-object --- -->
## Object (layer 8, wave 6)

`Object.h`/`Object.cpp` (7,250 lines) on this core. Files, all in
`src/logic/world/`:

| file | what |
|---|---|
| `object.rs` | the `Object` struct (every field of `Object.h` that is data), `ModuleSlot`, `BehaviorSlot`, the helper kinds, the arena's `insert_unlisted` / `link_at_head` / `rekey` |
| `module_call.rs`, `module_probe.rs` | `ModuleCall` / `ModuleReply`: the calls `Object` makes through the module interfaces as one message type; the scripted `ProbeModule` the tests use |
| `object_modules.rs` | the module lists: interface lookups, upgrades, special powers, collide, countermeasures, create/destroy/die loops |
| `object_state.rs` | status, script status, disabled states, weapon-set / weapon-bonus / armor-set flags, selectability, geometry, containment, vision and shroud, the partition maintenance |
| `object_team.rs` | team and player membership as ids, `setTeam` and its reactions, defection, capture, radar |
| `object_damage.rs` | `attemptDamage`, healing, `kill`, `onDie`, `scoreTheKill`, veterancy, crush/squish, `isAbleToAttack`, the model-condition helpers |
| `object_create.rs` | `newObject` after the template side (`World::new_object`), `initObject`, `onDestroy`, `~Object` (`delete_instance`), drawable binding |
| `object_transform.rs` | the position / orientation setters and `reactToTransformChange`, trigger areas, layers |
| `object_xfer.rs` | `crc`, `xfer` (version 9) and `loadPostProcess`, `setID` (`ObjectArena::rekey`) |
| `object_cuts.rs`, `object_events.rs` | the cut points (below) |

**Module dispatch.** `Object::modules` is `m_behaviors`; each `ModuleSlot` has
the interface mask of its class (`MODULEINTERFACE_*`) and, for a module with an
update interface, the index of its `BehaviorSlot` in `Object::behaviors` (the
contract of "What the Object translator must keep", kept). `World::module_call(id,
index, ModuleCall)` checks the module's body out of its slot, calls
`UpdateBody::module_call` and puts it back, so a module may call back into the
world. A loop over the modules skips a module whose mask lacks the interface
(`getDie()` is `NULL`), as upstream does.

~~**Deviation to decide (needs the lead).** A call to a module **whose body is running** is *not delivered*: it is recorded in `world.cuts.reentrant_skips`. Upstream makes the plain recursive call (`ActiveBody::attemptDamage` -> `Object::onDie` -> the same body's `DieModuleInterface`). Decision 1 (2026-10-18) asks for the synchronous order. Layer 9's first body that is both a body and a die module (`ActiveBody` with `onDie`) will hit it; the fix must route the call to the body the caller holds. Until then it is visible (`reentrant_skips`) and the differential test models it identically on both sides.~~
**CORRECTED 2026-10-18 (`p1w7-sonnet`)** — a call that reaches a running module **runs at once**,
inside the outer call, on the module's live state, in upstream's order. The running body *lends itself* to
its slot around every call into the world that can reach its own object: `World::lend_body(me, body, |w| ..)`
puts the body back in the slot, runs the closure (a nested `module_call` to the same module finds the body,
takes it, runs it, puts it back), then takes the body back and returns it to the caller, who stores it into
its own `self` (`ProbeModule::lend` is the pattern). No deferred queue: the order of effects is upstream's
call order. A module that does not lend itself leaves the call undelivered, which is recorded in
`world.cuts.reentrant_skips` as a tripwire (it must stay empty; the differential test asserts it);
`cuts.reentrant_delivered` counts the calls that ran re-entrantly. **Layer 9 rule:** a body that calls into
the world (damage, die, status, any `Object` function) while holding `&mut self` wraps that call in
`lend`. The same pattern serves an update module's `update()` (its `ModuleHandle` is the slot of its
`ModuleSlot`; `take_module_body` reads the `BehaviorSlot` body for a module with an update interface).
Proof: `tests/differential_object.rs` (5,292 re-entrant calls in the default run, 0 programs differ, two
planted bugs caught), `object_tests.rs`.

**Cut points (each is a `CutCall`, an `ObjectClientEvent` or a table of
`ObjectCuts`; the list the lead asked for):**

| unit | calls |
|---|---|
| `Player` (7) | `becomingTeamMember`, score keeper, skill points, bounty, difficulty / battle-plan bonuses, `addRadar` / `removeRadar`; queries (`getPlayerMask`, colour, completed upgrades, relationship to a team, production veterancy ...) are tables |
| `Team` (7) | member list, `notifyTeamOfObjectDeath`, `setEnteredExited`, controlling player, name, template info, attack priority |
| `Energy`, `AcademyStats` | `objectEnteringInfluence`, `objectLeavingInfluence`, `adjustPower`, `recordMine` |
| `PartitionManager` (wave 6) | `registerObject`, `unRegisterObject`, shroud reveal / cover, value and threat maps, `makeDirty`; `iteratePotentialCollisions` and the booby-trap range query are tables |
| `ExperienceTracker` (wave 6) | default copies of its logic over `Object::experience` (`cut_experience_*`), checked against the real `ExperienceTracker.cpp` |
| `WeaponSet` / `Weapon` (8) | `updateWeaponSet`, `weaponSetOnWeaponBonusChange`, slot queries (these act on the `WeaponSetRecord`, no weapon in any slot); **the 25 forwarders (`fireCurrentWeapon` ...) are not translated.** ~~`Weapon` and `WeaponSet` themselves are a cut point~~ **CORRECTED 2026-10-20 (`p1w8-sonnet`)** — `logic::weapon_fire` / `weapon_set_state` hold the translation, written against the `WeaponHost` trait (`logic::weapon_host`); the `Object` side (the owned `WeaponSet`, `WeaponHost` for `World`) is not joined up yet |
| AI, pathfinder (10) | `setAttitude`, `setAttackInfo`, `aiIdle`, `transferAttack`, `stop`, dozer `cancelTask`, `updatePos` / `removePos` / `addObjectToPathfindMap` / `removeObjectFromPathfindMap` / `removeWallPiece`; turret and locomotor queries are tables |
| `ScriptEngine` (11), `GameLogic`, `BuildAssistant` | `notifyOfObjectCreationOrDestruction`, `notifyOfObjectDestruction`, `AppendDebugMessage`, `updateObjectsChangedTriggerAreas`, `deselectObject`, `sellObject` |
| the client | audio, radar, EVA, in-game UI, control bar, `notifyTerrainObjectMoved`, and every `m_drawable->...` call are `ObjectClientEvent`s (`ClientEvent::Object`) |
| helpers (9) | `ObjectSMCHelper` ... `TempWeaponBonusHelper` are stub modules (`NewModule::helper_stub`); `sleepUntil` is a `CutCall` |
| module factory | `NewObject::modules` (the caller makes the modules) |
| ~~**not translated**~~ **CORRECTED 2026-10-20 (`p1w8-sonnet`)** | ~~`updateObjValuesFromMapProperties` (the map `Dict`), `doCommandButton*`, `canProduceUpgrade` (GUI command buttons), the bone-position getters (client model data), the weapon forwarders, `adjustModelConditionForWeaponStatus`~~ translated in `logic::world::object_weapons` (generic over `ObjectWeaponHost` / `ObjectBoneHost`, implemented by the layer-12 adapter; tested against the unchanged text in the weapon differential): the 25 weapon forwarders, `adjustModelConditionForWeaponStatus`, `getSingleLogicalBonePosition`, `...OnTurret`, `getMultiLogicalBonePosition`, `getCommandSetString`, `canProduceUpgrade`. **Still not translated:** `updateObjValuesFromMapProperties` (the map `Dict`; its AI, locomotor, audio and upgrade calls are cut points that need the layer 9/10 units) and `doCommandButton*` (`CommandButton`, special powers, AI orders) |
| `TheActionManager` (7) | translated as `logic::action_manager` over the `ActionHost` trait (31 methods); the layer-12 adapter implements it for `World` from `Object`, `Player`, the modules and the partition; nothing in `World` calls it yet |

**Other deviations from the upstream text (all kept visible):**

- `new_object` puts the object in the id table *before* its modules exist
  (`ObjectArena::insert_unlisted`), as `setID` does in the constructor, and in
  the list at the end of construction (`registerObject`).
- `~Object` runs on an object already removed from the list and table;
  `delete_instance` puts it back for the duration (the destructor reads its
  own team and radar data) and removes it at the end.
- `Object::position` mirrors the thing's position (the scheduler reads it).
- `Object::new(id)` is a blank object with a default template: the shell the
  world-core tests use.
- The constructor tests `getTemplateInfo()` of the team before setting the AI
  attitude; upstream does not (a null dereference on a team with no template
  info).

**Verification.** `tests/differential_object.rs` compiles the text of
`Object.cpp` (cut in `build.rs`, every cut asserted once) on the stand-ins of
`differential/shim_object` and runs random programs on both sides; see its
header for the method and the results. It found three defects in the first
translation (the constructor set the AI attitude from the team asked for, not
the team the object ended on; `findAnyShortcutSpecialPowerModuleInterface` did
not skip a module with no template; `m_destinationLayer` started at
`LAYER_GROUND`, but the memory pool zero-fills and the constructor never sets
it).
<!-- --- end p1w6-object --- -->

<!-- --- p1l9-example --- -->
## Layer 9: how a concrete module is translated (lead's pattern, 2026-10-20)

`logic::modules::command_set_upgrade` (`CommandSetUpgrade`) is the worked
example; its module docs carry the checklist. Every layer-9 module adds the
same pieces, each in its translator's marked block:

1. **Data class** with the base by `Arc` (`upgrade: Arc<UpgradeModuleData>`;
   upstream's base and the module's base are the same object) and its own
   fields; the field table is the base table first (for upgrade modules,
   `upgrade_module_data_field_parse!`) then the class's own;
   `friend_new_module_data(ini)`.
2. **Module** holding its base(s) by value (`UpgradeModule`, with the
   `UpgradeMux` inside) and its own state.
3. **Behaviour**: the interface calls `Object` makes arrive as `ModuleCall`s
   through `World::module_call`. Upgrade modules implement `UpgradeBehavior`
   and answer with `upgrade_module_call` (`modules::upgrade_body`), which runs
   upstream's `attemptUpgrade` -> `giveSelfUpgrade` steps with the world in
   between. Other families get a shared helper of the same shape when their
   first module is translated.
4. **Statics**: the stores upstream reads through globals are
   `world.statics` (`GameStatics`, an `Arc`, read-only once the game runs:
   `upgrade_center` so far; a module that needs another store adds it there).
   `TheNameKeyGenerator` is `world.names` (lookups assign keys at run time).
5. **Re-entrancy**: a module lends itself (`modules::upgrade_body::lend`,
   `World::lend_body`) around any call into the world that can reach its own
   object's modules (`removeUpgrade` resets every upgrade module, this one
   included).
6. **Client**: `world.emit(ObjectClientEvent::..)`; a new notification is a
   new variant (`FxListDoFxObj` was added for `FXList::doFXObj`).
7. **Upstream throws during a game** (`INI_INVALID_DATA` from a mux name that
   is not an upgrade) are recorded in `world.cuts.module_errors` with the
   module; the call answers `FALSE`. Undefined behaviour (a mux list naming
   `None`, which upstream dereferences as a null template) is an error with
   `is_undefined()`, never sent to the C++.
8. **Dispatch**: a variant of `UpdateBody`, with arms in `update`,
   `module_call`, `crc`/`xfer`/`load_post_process`, and the `*_chain` methods.
9. **Save order**: a translated module saves its **whole base chain itself**,
   in upstream's order (`X::xfer` writes its version, then the base's `xfer`,
   then its fields). `UpdateBody::owns_base_chain` is true for every
   translated module, and the slot snapshot (`object_xfer.rs`) then hands the
   whole chain to the module, passing the scheduling half (`UpdateModule`)
   for an update module through `xfer_chain(update, ..)`; the slot no longer
   writes the bases first.
10. **Factory**: an arm in `module_factory::translated_module` (data
    constructor, `getInterfaceMask()`, a `NewModuleProc` that makes the base
    the factory's accessors answer for), a variant of `ModuleDataKind` with
    its arms, and an arm in `modules::new_module`, which makes the module for
    an object.
11. **Differential test**: the unchanged `.cpp` is `#include`d by one driver
    file per batch (`differential/l9_<batch>.inc`), itself `#include`d in its
    marked block at the end of `differential/wrap_module.cpp`, so it shares
    that unit's stand-ins (`differential/shim_module`: `Object`, `Thing`,
    `Player`, `ControlBar`, ...; add to them in marked blocks), its effect log
    and dump helpers, and the REAL `UpgradeCenter` and `INI`. The Rust side is
    the real `World`, not a twin.
<!-- --- end p1l9-example --- -->
## Open item (2026-10-20): Object still keeps team state in `ObjectCuts`

The translated Team unit (`team.rs`, `team_ops.rs`, `team_xfer.rs`) reaches objects only through `TeamHost`.
`Object::setTeam` and the object's own team fields are still the earlier shell state in `ObjectCuts`;
the layer-12 `World` adapter is what connects the two. Not verified end to end until then.
