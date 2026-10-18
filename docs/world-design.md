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

- `Object`'s real fields and `Thing` base; `BehaviorModule` lists beyond the
  update-capable ones (destroy/die/damage/create interfaces dispatch).
- The `hook_*` bodies (layers 7-10), `attempt_damage` -> `BodyModule`.
- ~~`PartitionManager` (a `World` field)~~ **CORRECTED 2026-10-18** (wave 6,
  `p1w6-partition`): translated, see "The partition manager". Still left: the
  pathfinder, the AI, `ScriptEngine`, `GameLogic::update`'s remaining steps and
  `GameLogic::xfer`/CRC (layer 12), and the `World` implementation of the
  partition host traits (needs the real `Object`).
- `Player`/`Team` contents; the `TeamFactory` creation functions.
- The `GameSetup` struct (network slots), and the audio-length table loader.
- `popSleepyUpdate` is translated but unused (upstream never calls it).

## Open questions for the lead

1. **Same-module re-entrancy.** `with_body` on a module that is running
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
3. **Keep `world.terrain`** as a field, checked out for its own update.
