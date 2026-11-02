// SPDX-License-Identifier: GPL-3.0-only
//
// Part of a modified Rust translation of EA's GPLv3 release of Command & Conquer Generals Zero Hour
// (https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590), with the
// additional terms in NOTICE.md. Not upstream code: the scripted game of Phase 1 gate (a).
//! Phase 1 gate (a), integration batch I9d: one scripted game, the same on every build, whose per-frame logic CRCs a native
//! run and a wasm run must give identically (`SCOPE.md`, section 5).
//!
//! The game's AI data is its own `AIData` block (the guard modifiers 1.0, alert 1.1, aggressive 1.5: test values, not the
//! shipped game's `AIData.ini`, which the source release does not include). The game: a flat 400 x 400 map (40 x 40 cells) with the pathfinder, the partition manager, the teams and the AI live; an
//! immobile depot in the middle (an obstacle on the pathfinder's map); six runners (a ground locomotor, physics) on the west
//! side told by their player to cross to the east side along paths that cross around the depot, the first three each by its
//! own order and the last three as an AI group (batch I9c-2: `groupMoveToPosition`); a gun whose AI has a turret;
//! an idle dozer; and (batch I9c-5) a skirmish on the north edge, two soldiers of `Player1`'s against two of `Player2`'s (the
//! players enemies of each other, each its own ally, with default teams), which acquire each other when idle and fire. Every
//! frame is `GameLogic::update`, then `getCRC(CRC_RECALC)`.
//!
//! [`run_gate_scenario`] gives the CRCs; `examples/gate_native.rs` prints them, `examples/gate_wasm.rs` exports them from a
//! `wasm32-unknown-unknown` module, and `scripts/gate_a.sh` runs both (the wasm under Node) and compares.
//!
//! `p1w13-sonnet`, integration batch I9d, 2026-11-01.

use std::sync::Arc;

use crate::common::base_type::{Coord3D, ICoord2D};
use crate::common::dict::Dict;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::logic::terrain_logic::{GroundSource, TerrainLogic};
use crate::logic::terrain_visual::LogicHeightMap;
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_update::CRC_RECALC;
use crate::logic::world::ids::{ObjectId, TeamId};
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::pathfinder_host_world::PfOp;
use crate::logic::world::game_data::{load_game_data, MemoryFiles, MissingFiles, TextTable};
use crate::logic::world::team_ops::{factory_find_team, factory_init_team};

/// The game's INI: a tracked locomotor, the runner, the gun, the dozer and the depot.
const INI: &str = "\
AIData\n GuardInnerModifierHuman = 1.0\n GuardOuterModifierHuman = 1.0\n GuardInnerModifierAI = 1.0\n GuardOuterModifierAI = 1.0\n AlertRangeModifier = 1.1\n AggressiveRangeModifier = 1.5\nEnd\n\
Weapon GateGun\n PrimaryDamage = 20\n PrimaryDamageRadius = 0\n AttackRange = 100\n DamageType = ARMOR_PIERCING\n DeathType = NORMAL\n WeaponSpeed = 999999\n DelayBetweenShots = 500\nEnd\n\
Locomotor GateTreads\n Surfaces = GROUND\n Speed = 30\n Acceleration = 100\n Braking = 100\n TurnRate = 180\n Appearance = TREADS\nEnd\n\
Object Runner\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n\
Object Gun\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n  Turret\n   TurretTurnRate = 60\n   ControlledWeaponSlots = PRIMARY\n   NaturalTurretAngle = 0\n  End\n End\nEnd\n\
Object Dozer\n KindOf = VEHICLE DOZER\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = DozerAIUpdate ModuleTag_AI\n  RepairHealthPercentPerSecond = 2%\n  BoredTime = 5000\n  BoredRange = 150\n End\nEnd\n\
Object Soldier\n KindOf = VEHICLE\n VisionRange = 150\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 WeaponSet\n  Conditions = None\n  Weapon = PRIMARY GateGun\n End\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n  AutoAcquireEnemiesWhenIdle = Yes\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n\
Object Depot\n KindOf = STRUCTURE IMMOBILE\n BuildTime = 2.0\n Geometry = BOX\n GeometryMajorRadius = 20\n GeometryMinorRadius = 20\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\nEnd\n";

/// A new object of the template `name` on `team` at `pos`, in its team and the partition manager.
fn make(w: &mut World, name: &[u8], team: TeamId, pos: Coord3D) -> ObjectId {
    let t = World::thing_factory_find_template(w, name).expect("a template of the game");
    let id = w.thing_factory_new_object(t, Some(team), ObjectStatusMaskType::default()).expect("an object of the game");
    w.set_position(id, &pos);
    if !w.teams.is_in_member_list(team, id) {
        w.teams.prepend_member(team, id);
    }
    w.partition_register_object(id);
    w.partition_update();
    // (batch I9i: "This magically created thing needs to be considered as Built", as `startNewGame` does for a map's objects,
    // GameLogic.cpp:1902: its create modules' `onBuildComplete`, which registers a supply center with the players)
    crate::logic::world::start_host_world::build_complete(w, id);
    id
}

/// The game at frame 0: the world, its map, its player's team and its objects, the runners' orders given.
pub fn gate_world() -> World {
    let mut w = gate_map_world();
    let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
    let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");

    make(&mut w, b"Depot", team, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
    let mut runners = Vec::new();
    for i in 0..6 {
        let y = 50.0 + 60.0 * i as f32;
        runners.push((make(&mut w, b"Runner", team, Coord3D { x: 40.0, y, z: 0.0 }), Coord3D { x: 360.0, y: 350.0 - 60.0 * i as f32, z: 0.0 }));
    }
    make(&mut w, b"Gun", team, Coord3D { x: 100.0, y: 300.0, z: 0.0 });
    make(&mut w, b"Dozer", team, Coord3D { x: 300.0, y: 100.0, z: 0.0 });
    // (batch I9c-5: a skirmish on the north edge, two soldiers of each player 90 apart: within sight and range; a human
    // player's idle units take a mood target only within weapon range and out of the fog)
    for (t, x) in [(team, 150.0), (team, 160.0), (enemy, 250.0), (enemy, 260.0)] {
        make(&mut w, b"Soldier", t, Coord3D { x, y: 380.0, z: 0.0 });
    }
    w.pathfinder_op(PfOp::NewMap);
    // (the first three each by its own order; batch I9c-2: the last three as a group, by the player's group order)
    for &(r, goal) in &runners[..3] {
        if let Some(index) = w.find_object(r).and_then(|o| o.ai) {
            w.module_call(r, index, ModuleCall::AiMoveToPosition { pos: goal, command_source: 0 });
        }
    }
    let g = w.ai_create_group();
    for &(r, _) in &runners[3..] {
        w.with_group(g, |gr, w| gr.add(w, r));
    }
    w.live_group_call(g, crate::logic::world::game_logic_dispatch::GroupCall::MoveTo { pos: Coord3D { x: 360.0, y: 100.0, z: 0.0 }, add_waypoint: false });
    w
}

/// The gate's game before its units: the map, the players and teams, the subsystems live, no object (batch I10: a saved
/// game loads into a World made from the same map, as upstream's `loadGame` loads into the map it has just started).
pub fn gate_map_world() -> World {
    gate_map_world_with("")
}

/// [`gate_map_world`] with `extra` read after the game's INI (batch I9h: a test's own templates; the game's are unchanged).
pub fn gate_map_world_with(extra: &str) -> World {
    // (batch I12: read as the game reads its INI, `game_data::load_game_data`: the gate's blocks are one file of the thing
    // factory's directory, a test's another after it; the other subsystems' files are absent and skipped; the post-process runs)
    let mut files = MemoryFiles::new();
    files.insert(b"Data\\INI\\Object\\Gate.ini", INI.as_bytes().to_vec());
    if !extra.is_empty() {
        files.insert(b"Data\\INI\\Object\\GateTest.ini", extra.as_bytes().to_vec());
    }
    let load = load_game_data(&files, MissingFiles::Skip, TextTable::default()).expect("the game's INI");
    let (mut st, names) = load.into_statics();

    let mut w = World::new();
    w.names = names;
    {
        let g = st.global_data.create_original();
        g.partition_cell_size = 40.0;
        // (batch I9e: the low-energy production penalty, test values: the shipped `GameData.ini` is not in the source release;
        // all 0, an unpowered player's build times are divided by upstream's floor of 0.01)
        g.low_energy_penalty_modifier = 0.4;
        g.min_low_energy_production_speed = 0.5;
        g.max_low_energy_production_speed = 0.8;
    }
    w.statics = Arc::new(st);
    // (batch I19: the control bar's buttons and sets the INI held, into the world's tables)
    w.install_command_buttons();
    let mut t = TerrainLogic::new(GroundSource::LogicHeightMap);
    let size = ICoord2D { x: 40, y: 40 };
    t.set_logic_height_map(Some(LogicHeightMap::new(40, 40, 0, vec![size], vec![0; 1600]).expect("the height map")));
    t.set_boundaries(vec![size], 0);
    w.terrain = t;
    w.players.set_player_count(3);
    // (batch I10: each player initialised, as `PlayerList::init` does before the map's sides are read: its hotkey squads,
    // which a save writes, are made here)
    for p in 0..3 {
        crate::logic::world::player_ops::player_init(&mut w, p, None).expect("the player");
    }
    for (i, name) in [(1, &b"Player1"[..]), (2, &b"Player2"[..])] {
        let key = w.names.name_to_key(name);
        w.players.get_nth_mut(i).expect("the player").name_key = key.0 as i32;
    }
    factory_init_team(&mut w, b"team1", b"Player1", true, &Dict::new()).expect("the team");
    factory_init_team(&mut w, b"team2", b"Player2", true, &Dict::new()).expect("the team");
    let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
    let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
    // (each player's default team, as `Player::initFromDict` makes one: the look's mask asks it)
    w.players.get_nth_mut(1).expect("the player").default_team = Some(team);
    w.players.get_nth_mut(2).expect("the player").default_team = Some(enemy);
    // (batch I8c: and active, as `Player::setDefaultTeam` leaves it: a script finds a singleton team only when active)
    for t in [team, enemy] {
        crate::logic::world::team_ops::team_set_active(&mut w, t).expect("the team active");
    }
    // (batch I9i: each player's resource gathering manager, which `Player::initFromDict` makes, Player.cpp:1005: its supply
    // trucks ask it for a warehouse and a center)
    // (batch I17: and its tunnel system, made in the same place, Player.cpp:1012: a tunnel's guard asks for it)
    for p in 0..3 {
        let pl = w.players.get_nth_mut(p).expect("the player");
        pl.resource_gathering_manager = Some(Default::default());
        pl.tunnel_system = Some(Default::default());
    }
    // (the player allied to itself, as `Player::initFromDict` and `PlayerList::newGame` leave it)
    // (and the two players enemies, as a skirmish's `PlayerList::newGame` leaves them)
    {
        use crate::common::game_common::Relationship;
        use crate::logic::world::player_ops::player_set_player_relationship as rel;
        for (p, q, r) in [(1, 1, Relationship::Allies), (2, 2, Relationship::Allies), (1, 2, Relationship::Enemies), (2, 1, Relationship::Enemies)] {
            rel(&mut w, p, Some(q), r).expect("the players' relationships");
        }
    }
    w.partition_init();
    // (batch I10: the map's name and file, which a save embeds: the gate World is made from its INI and this code, and its
    // map file is that INI's text)
    w.save_side.map_name = b"Maps\\Gate\\Gate.map".to_vec();
    w.save_side.disk.insert(w.save_side.map_name.to_ascii_lowercase(), INI.as_bytes().to_vec());
    w.cuts.live_partition = true;
    w.cuts.live_teams = true;
    w.cuts.live_ai = true;
    w.cuts.live_pathfinder = true;
    w.cuts.live_ocl = true;
    w
}

/// The game's logic CRC after each of `frames` frames (`GameLogic::update`, then `getCRC(CRC_RECALC)`). A frame that fails,
/// or a CRC that cannot be taken, ends the list early (the caller compares lengths too).
pub fn run_gate_scenario(frames: u32) -> Vec<u32> {
    let mut w = gate_world();
    let mut crcs = Vec::with_capacity(frames as usize);
    for _ in 0..frames {
        if w.logic_update().is_err() {
            break;
        }
        match w.logic_crc(CRC_RECALC, None) {
            Ok(c) => crcs.push(c),
            Err(_) => break,
        }
    }
    crcs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game runs its frames with no live error and gives the same CRCs twice; its runners move.
    #[test]
    fn the_gate_game_runs_and_repeats() {
        let a = run_gate_scenario(60);
        assert_eq!(a.len(), 60);
        assert_eq!(a, run_gate_scenario(60));
        let mut w = gate_world();
        let before: Vec<Coord3D> = w.objects.ids_in_order().into_iter().map(|id| w.find_object(id).unwrap().get_position()).collect();
        for _ in 0..60 {
            w.logic_update().unwrap();
        }
        let after: Vec<Coord3D> = w.objects.ids_in_order().into_iter().map(|id| w.find_object(id).unwrap().get_position()).collect();
        assert!(before.iter().zip(&after).filter(|(b, a)| b != a).count() >= 6, "the runners move");
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}

#[cfg(test)]
mod combat_tests {
    use super::*;

    /// batch I9c-5: the skirmish. The soldiers see their enemies (90 apart, vision 150, range 100) and fire (20 a shot):
    /// within 300 frames some soldier has lost health, with no live error; and the whole gate scenario reaches no cut of the
    /// logic, only the client's and the UI's (the frame given to the client, the recorder, the idle-worker button).
    ///
    /// **Result** (2026-11-01, native, release): ~~health after 300 frames `[60, 0, 0, 100]`: one soldier of each side killed,
    /// the first soldier hit twice~~ **CORRECTED, batch I10:** `[0, 0, 20, 100]`: both of `Player1`'s soldiers killed,
    /// `Player2`'s first hit four times. The gate world's players are now initialised (`Player::init`, which upstream's
    /// `PlayerList::init` runs before the map's sides are read), which changed the fight; the move sounds are now reached
    /// too (`startMoveSound`, `removeAudioEvent`: the client's audio, cut points). Before this batch the soldiers took their mood targets every frame but never attacked:
    /// the attack state's `chooseBestWeaponForTarget` was a cut answering false, so each attack failed back to idle at once.
    #[test]
    fn the_soldiers_fight() {
        let mut w = gate_world();
        let soldiers: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| w.find_object(id).is_some_and(|o| o.template().get_name() == b"Soldier")).collect();
        assert_eq!(soldiers.len(), 4);
        w.cuts.trace = Some(Vec::new());
        for _ in 0..300 {
            w.logic_update().unwrap();
        }
        let mut calls: Vec<String> = w.cuts.trace.take().unwrap().into_iter().filter_map(|t| t.split("call: \"").nth(1).map(|c| c.split('"').next().unwrap().to_string())).collect();
        calls.sort();
        calls.dedup();
        assert_eq!(calls, ["GameClient::setFrame", "Recorder::update", "in_game_ui_idle_worker", "remove_audio_event", "start_move_sound"]);
        let health: Vec<f32> = soldiers.iter().map(|&s| w.body_health(s).map_or(0.0, |h| h.0)).collect();
        assert!(health.iter().any(|&h| h < 100.0), "health {health:?}");
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}

#[cfg(test)]
mod computer_player_tests {
    use super::*;
    use crate::common::game_common::PlayerType;
    use crate::logic::sides_list::BuildListInfo;
    use crate::logic::world::player_ops::{player_new_map, player_set_build_list, player_set_player_type};

    /// The objects of `template` controlled by player `p`, with their positions.
    fn owned(w: &World, template: &[u8], p: i32) -> Vec<(ObjectId, Coord3D)> {
        w.objects
            .ids_in_order()
            .into_iter()
            .filter_map(|id| w.find_object(id).map(|o| (id, o)))
            .filter(|(id, o)| o.template().get_name() == template && w.get_controlling_player(*id) == Some(p))
            .map(|(id, o)| (id, o.get_position()))
            .collect()
    }

    /// batch I9e: a computer player builds its base. `Player2` of the gate world becomes a computer player (`AIPlayer`, not the
    /// skirmish AI) with a dozer and a build list of two depots: one built initially at (300, 60), one to be built at (340, 120).
    /// Predicted: `newMap` builds the first at once (`buildStructureNow`); within 600 frames the AI player's update has its dozer
    /// start the second (`processBaseBuilding`, `buildStructureWithDozer`) and finish it; no live error.
    ///
    /// **Result** (2026-11-01, native, release): the first depot stands after `newMap`; the dozer starts the second on frame 1
    /// (the structure timer's two frames), teleported to the site (it has no locomotor: `buildStructureWithDozer` teleports a
    /// dozer with no path). The depot's build time is 99 frames (2 s, the unpowered player's rate 0.6 of the gate's test
    /// low-energy values; `calcTimeToBuild` truncates 100.0's float error to 99, as upstream's `Int` does); it is complete on
    /// frame 232. **Not first time:** the first runs found the World's AI module did not route `construct` to the dozer, the
    /// test's dozer had no command set (`isPossibleToMakeUnit` refuses a dozer whose command set cannot build the depot: the
    /// control bar's table now gives it one), the dozer's host answered a zero-cost withdraw as no money, and `construct` ran
    /// without the AI's machine lent (the dozer's states ask whether the AI is idle).
    #[test]
    fn a_computer_player_builds_its_base() {
        let mut w = gate_world();
        let team2 = w.players.get_nth(2).and_then(|p| p.default_team).expect("Player2's team");
        let dozer = make(&mut w, b"Dozer", team2, Coord3D { x: 320.0, y: 40.0, z: 0.0 });
        // (its command set, with a button to construct a depot: the control bar's data, which the World takes as a table;
        // `buildObjectNow` with a dozer requires the dozer can make the building, `isPossibleToMakeUnit`)
        let depot = w.statics.thing_factory.find_template(b"Depot").map(|t| t.data.get_template_id());
        w.cuts.command_buttons.push(crate::logic::world::object_cuts::CommandButtonFacts {
            name: b"GateDozerBuildDepot".to_vec(),
            command_type: crate::logic::world::build_assistant::GUI_COMMAND_DOZER_CONSTRUCT,
            weapon_slot: 0,
            special_power: None,
            thing_template: depot,
            upgrade: None,
            ready_for: Default::default(),
        });
        w.cuts.command_sets.push((b"GateDozerSet".to_vec(), vec![Some(w.cuts.command_buttons.len() - 1)]));
        w.objects.find_mut(dozer).expect("the dozer").command_set_string_override = b"GateDozerSet".to_vec();
        player_set_player_type(&mut w, 2, PlayerType::Computer, false).expect("Player2 a computer player");
        assert!(w.ai_players.by_player.contains_key(&2), "the World holds Player2's AI player");
        let entry = |x: f32, y: f32, initially: bool| BuildListInfo {
            template_name: b"Depot".to_vec(),
            location: Coord3D { x, y, z: 0.0 },
            is_initially_built: initially,
            ..BuildListInfo::new()
        };
        player_set_build_list(&mut w, 2, vec![entry(300.0, 60.0, true), entry(340.0, 120.0, false)]).expect("the build list");
        player_new_map(&mut w, 2).expect("the new map");
        let built = owned(&w, b"Depot", 2);
        assert_eq!(built.len(), 1, "newMap builds the initial depot: {built:?}");
        assert_eq!((built[0].1.x, built[0].1.y), (300.0, 60.0));
        let mut started = None;
        for f in 0..600 {
            w.logic_update().unwrap();
            if let Some(&(_, pos)) = owned(&w, b"Depot", 2).iter().find(|(_, p)| (p.x, p.y) != (300.0, 60.0)) {
                started = Some((f, pos));
                break;
            }
        }
        let (frame, pos) = started.expect("the AI player's dozer starts the second depot");
        assert!((pos.x - 340.0).abs() < 1.0 && (pos.y - 120.0).abs() < 1.0, "at the list's place: {pos:?} (frame {frame})");
        // (the dozer builds it: under construction until done)
        let site = owned(&w, b"Depot", 2).into_iter().find(|(_, p)| (p.x, p.y) != (300.0, 60.0)).map(|(id, _)| id).expect("the site");
        let under = |w: &World| w.find_object(site).is_some_and(|o| o.test_status(crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION));
        assert!(under(&w), "the new depot starts under construction");
        let mut done = None;
        for f in frame..600 {
            w.logic_update().unwrap();
            if !under(&w) {
                done = Some(f);
                break;
            }
        }
        let done = done.expect("the dozer finishes the depot");
        assert_eq!(w.find_object(site).map(|o| o.construction_percent), Some(crate::logic::world::object::CONSTRUCTION_COMPLETE));
        assert!(done < 600, "finished on frame {done}");
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }

    /// batch I9g: the AI players' remaining questions of the World, on the gate world. Predicted: the templates enumerate
    /// as the factory lists them (newest first, every one once: the gate's five; ~~six~~ **corrected after the first run**, a
    /// miscount of the INI); around `Player2`'s soldier at (250, 380) the threat query
    /// finds `Player1`'s nearest soldier at (160, 380) within 100 (90 away) and nothing within 50; neither player has a cash
    /// generator or a supply source near it; a soldier has no exit interface, a runner no rebuild hole; `joinTeam` of a
    /// `Player2` soldier runs with no live error.
    #[test]
    fn the_ai_players_questions_are_answered() {
        use crate::logic::ai::ai_player::{AiPlayerHost, ClosestQuery};
        let mut w = gate_world();
        let mut names = Vec::new();
        let mut t = AiPlayerHost::first_template(&mut w);
        while let Some(x) = t {
            names.push(AiPlayerHost::template_name(&mut w, x));
            t = AiPlayerHost::next_template(&mut w, x);
        }
        let expected: Vec<Vec<u8>> = w.statics.thing_factory.templates().map(|x| x.data.get_name().to_vec()).collect();
        assert_eq!(names, expected);
        assert_eq!(names.len(), 5, "{names:?}");
        let soldiers = |w: &World, p: i32| -> Vec<ObjectId> {
            w.objects.ids_in_order().into_iter().filter(|&id| w.find_object(id).is_some_and(|o| o.template().get_name() == b"Soldier") && w.get_controlling_player(id) == Some(p)).collect()
        };
        let here = Coord3D { x: 250.0, y: 380.0, z: 0.0 };
        let threat = AiPlayerHost::closest_object(&mut w, ClosestQuery::Threat, 2, &here, 100.0).expect("a threat within 100");
        assert_eq!(w.find_object(threat).map(|o| o.get_position().x), Some(160.0));
        assert!(soldiers(&w, 1).contains(&threat));
        assert_eq!(AiPlayerHost::closest_object(&mut w, ClosestQuery::Threat, 2, &here, 50.0), None);
        assert_eq!(AiPlayerHost::closest_object(&mut w, ClosestQuery::MyCashGenerator, 2, &here, 500.0), None);
        assert_eq!(AiPlayerHost::closest_object(&mut w, ClosestQuery::SupplySourceNotMine, 2, &here, 500.0), None);
        let s2 = soldiers(&w, 2)[0];
        assert_eq!(AiPlayerHost::exit_natural_rally_point(&mut w, s2), None);
        assert_eq!(AiPlayerHost::rebuild_hole(&mut w, s2), None);
        AiPlayerHost::ai_command(&mut w, s2, crate::logic::ai::ai_player::PlayerAiCommand::JoinTeam);
        for _ in 0..30 {
            w.logic_update().unwrap();
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }

    /// `PartitionFilterPlayerAffiliation` takes `AllowPlayerRelationship` flags (0x02 allies, 0x08 neutral), not the
    /// `PartitionFilterRelationship` ones (`1 << Relationship`); the script conditions' supply-structure scan
    /// (`ScriptConditions.cpp:2228`, `ALLOW_NEUTRAL, true`) once passed the latter, which reads as "allies". The Depot
    /// counts for player `p` exactly when `p` owns it or is neutral to its team; at least one player must be neutral to it,
    /// or the test could not tell the two flag sets apart.
    #[test]
    fn the_supply_structure_scan_keeps_neutral_structures() {
        use crate::common::game_common::Relationship;
        use crate::logic::world::partition::host::PartitionFilterHost;
        use crate::logic::world::script_conditions::ScriptConditionsHost;
        let mut w = gate_world();
        let depot = w.objects.ids_in_order().into_iter().find(|&id| w.find_object(id).is_some_and(|o| o.template().get_name() == b"Depot")).expect("the Depot");
        let at = Coord3D { x: 200.0, y: 200.0, z: 0.0 };
        let mut neutral_seen = false;
        for p in 0..w.players.player_count() {
            let rel = PartitionFilterHost::player_relationship_to_team_of(&w, p, depot);
            neutral_seen |= rel == Relationship::Neutral;
            let expected = rel == Relationship::Neutral || w.get_controlling_player(depot) == Some(p);
            assert_eq!(w.sc_partition_supply_structures(at, 50.0, p).contains(&depot), expected, "player {p}, {rel:?}");
            // `ScriptConditions.cpp:2277` (`ALLOW_ALLIES, false`) wants tech buildings; the Depot is not one.
            assert!(!w.sc_partition_tech_building_near(at, 50.0, p));
        }
        assert!(neutral_seen);
    }
}

#[cfg(test)]
mod script_order_tests {
    use super::*;
    use crate::logic::scripts::{action_type as at, Parameter, ScriptAction};

    fn move_team_to(team: &[u8], waypoint: &[u8]) -> ScriptAction {
        let mut a = ScriptAction::new_for_read();
        a.action_type = at::MOVE_TEAM_TO;
        a.num_parms = 2;
        for (i, s) in [team, waypoint].into_iter().enumerate() {
            let mut p = Parameter::new(0, 0);
            p.string = s.to_vec();
            a.parms[i] = Some(p);
        }
        a
    }

    /// batch I8c: a script moves a team. `MOVE_TEAM_TO` (`team2`, `GateWP`): the team becomes a group (`getTeamAsAIGroup`)
    /// whose members are moved (`groupMoveToPosition`, from the script). `team2`'s two soldiers start at (250, 380) and
    /// (260, 380); the waypoint is at (250, 200). Predicted: after 240 frames both are within 40 of it (they move about one
    /// unit a frame; the group's formation spreads them), with no live error. Before this batch the order was a cut.
    ///
    /// **Result** (2026-11-01, native, release): after 240 frames the soldiers are at (255.0, 205.9) and ~~(258.8, 205.2)~~
    /// (258.3, 205.2), 7.7 and ~~10.3~~ 9.8 from the waypoint; no live error (**re-measured, batch I9f:** the AI now updates
    /// in its upstream phase, before physics). **Not first time:** the first run found no team: the gate world's singleton
    /// teams were never made active (`Player::setDefaultTeam` does it), and `getTeamNamed` skips an inactive singleton.
    #[test]
    fn a_script_moves_a_team() {
        let mut w = gate_world();
        w.terrain.friend_push_waypoint(900, b"GateWP", Coord3D { x: 250.0, y: 200.0, z: 0.0 });
        let team2 = w.players.get_nth(2).and_then(|p| p.default_team).expect("team2");
        let soldiers: Vec<ObjectId> = w.teams.members(team2).into_iter().filter(|&id| w.find_object(id).is_some_and(|o| o.template().get_name() == b"Soldier")).collect();
        assert_eq!(soldiers.len(), 2);
        let mut engine = std::mem::take(&mut w.script_engine);
        w.execute_action_with(&mut engine, &move_team_to(b"team2", b"GateWP")).expect("the action");
        w.script_engine = engine;
        for _ in 0..240 {
            w.logic_update().unwrap();
        }
        for &s in &soldiers {
            let p = w.find_object(s).map(|o| o.get_position()).expect("the soldier lives");
            let d = ((p.x - 250.0).powi(2) + (p.y - 200.0).powi(2)).sqrt();
            assert!(d < 40.0, "soldier {s} at ({:.0}, {:.0}), {d:.0} from the waypoint", p.x, p.y);
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}

#[cfg(test)]
mod save_load_tests {
    use super::*;

    /// A save file's blocks by name (`xferSaveData`'s layout: each block's name as an ASCII string, its size, its data;
    /// `SG_EOF` last).
    fn blocks(bytes: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
        let mut out = std::collections::BTreeMap::new();
        let mut i = 0;
        while i < bytes.len() {
            let n = bytes[i] as usize;
            let name = String::from_utf8_lossy(&bytes[i + 1..i + 1 + n]).to_string();
            i += 1 + n;
            if name == "SG_EOF" {
                break;
            }
            let size = u32::from_le_bytes(bytes[i..i + 4].try_into().expect("a block size")) as usize;
            i += 4;
            out.insert(name, bytes[i..i + size].to_vec());
            i += size;
        }
        out
    }

    fn step(w: &mut World) -> u32 {
        w.logic_update().expect("the frame");
        w.logic_crc(CRC_RECALC, None).expect("the CRC")
    }

    /// batch I10, gate (b): the whole game saved and loaded. The gate game is run to frame 150 and saved
    /// (`GameState::saveGame`'s blocks); the save is loaded into a World made from the same map (`gate_map_world`: the
    /// map, players and teams, no units), and that game saved and loaded again.
    ///
    /// **What upstream guarantees, and so what is checked** (decided before the final runs; see the record below):
    /// (1) the save carries the durable logic state: after two loads (which restore the object list's order: each load
    /// prepends the objects it makes, as upstream's `registerObject` does) the twice-loaded game saves the same terrain,
    /// players, game logic (every object and module) and sides list as the original does at the save frame, once the
    /// original is given the load's re-look (below: each object re-sighted where it stands); (2) a save continues one way only: two Worlds loaded from the same bytes give the same CRC at every one of
    /// the next 150 frames; (3) no load or frame records an error.
    ///
    /// **Not checked, because upstream does not have it:** that a loaded game continues as the uninterrupted one would.
    /// Upstream's save leaves out transient state its load recomputes or resets: the closest-point-on-path cache
    /// (`Path::xfer` sets `m_cpopValid = FALSE` on a load), the logic's random state, the pathfinder (rebuilt by `newMap`),
    /// the script engine's screen fade (`ScriptEngine::newMap`'s fade in, not saved) and the team id counter
    /// (`TeamFactory::loadPostProcess` sets it past the highest id, one past what the running game holds), so the team
    /// factory and script engine blocks differ; and a load re-registers each object with the partition, whose re-look of a
    /// seeing object adds its sighting at once while the matching unlook waits in the pending-undo queue, so the partition
    /// block, and a whole-state CRC taken at the load, carry one extra look per seeing object until the queue runs.
    /// Measured here: a moving unit's first frame after a load steers from a recomputed closest point, and the CRCs part
    /// on that frame.
    ///
    /// **Result** (2026-11-01, native, release): (1) after two loads the four blocks equal the original's at frame 150;
    /// (2) the two loaded games agree at all 150 frames; (3) no error. **Not first time:** the first runs found the gate
    /// world's players uninitialised (`Player::init` makes the hotkey squads a save writes), the object id counter not saved
    /// (it is `CHUNK_GameStateMap`'s, now registered with an in-memory map file), and two id-keyed tables that `setID` did
    /// not follow (the turrets' AIs, the partition modules: a load makes each object under a temporary id, then gives it its
    /// saved one). ~~Predicted: the loaded game CRCs as the uninterrupted one from the load on~~ **WITHDRAWN, same batch**:
    /// upstream's own load does not give that (above); the criterion is now (1)-(3). ~~(1) as the deep logic CRC~~
    /// **WITHDRAWN, same batch**: once the partition rename registered the loaded objects, the re-look above made the CRC
    /// at the load the wrong instrument; (1) compares the durable blocks instead.
    /// Batch I17: a unit's locomotors survive a save. The AI's load clears its locomotor set and then xfers it
    /// (`xferSelfAndCurLocoPtr`), which was a cut that xfered nothing: a loaded unit had no locomotor and stood still, and the
    /// gate (b) blocks could not show it (both sides of the comparison had lost it the same way). Predicted before the run: a soldier
    /// ordered 300 north, saved on frame 10 and loaded, has a current locomotor, and moves at least 15 in the next 20 frames (30 a
    /// second); with the cut, no locomotor and no movement.
    #[test]
    fn a_loaded_unit_keeps_its_locomotors() {
        let mut a = gate_map_world();
        let team = factory_find_team(&mut a, b"team1").expect("the team").expect("team1");
        let s = make(&mut a, b"Soldier", team, Coord3D { x: 60.0, y: 40.0, z: 0.0 });
        a.pathfinder_op(PfOp::NewMap);
        let ai = a.find_object(s).and_then(|o| o.ai).expect("the AI");
        a.module_call(s, ai, ModuleCall::AiMoveToPosition { pos: Coord3D { x: 60.0, y: 340.0, z: 0.0 }, command_source: 0 });
        for _ in 0..10 {
            step(&mut a);
        }
        let bytes = a.save_game().expect("the save");
        let mut b = gate_map_world();
        b.load_game(bytes).expect("the load");
        let has_loco = |w: &World| {
            let ai = w.find_object(s).and_then(|o| o.ai).expect("the AI");
            match w.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: s, index: ai }) {
                Some(crate::logic::world::update_body::UpdateBody::AiUpdate(m)) => m.module.as_ref().and_then(|m| m.ai.as_ref()).is_some_and(|x| x.cur_locomotor.is_some()),
                _ => false,
            }
        };
        let y0 = b.find_object(s).map(|o| o.get_position().y).expect("the soldier");
        let loco = has_loco(&b);
        for _ in 0..20 {
            step(&mut b);
        }
        let y1 = b.find_object(s).map(|o| o.get_position().y).expect("the soldier");
        eprintln!("loaded with a locomotor {loco}; y {y0} -> {y1}");
        assert!(loco);
        assert!(y1 - y0 >= 15.0, "{y0} -> {y1}");
        assert!(b.cuts.team_live_errors.is_empty(), "{:?}", b.cuts.team_live_errors);
    }

    #[test]
    fn a_saved_game_loads_and_runs_on() {
        let mut a = gate_world();
        for _ in 0..150 {
            step(&mut a);
        }
        let bytes = a.save_game().unwrap_or_else(|e| panic!("the save: {e:?} {:?}", a.cuts.team_live_errors));
        let mut b = gate_map_world();
        b.load_game(bytes.clone()).unwrap_or_else(|e| panic!("the load: {e:?} {:?}", b.cuts.team_live_errors));
        let mut c = gate_map_world();
        c.load_game(b.save_game().expect("the second save")).expect("the second load");
        // (the load's re-look, applied to the original: each object re-sighted where it stands, see above)
        for id in a.objects.ids_in_order() {
            a.handle_partition_cell_maintenance(id);
        }
        let (sa, sc) = (blocks(&a.save_game().expect("the save")), blocks(&c.save_game().expect("the save")));
        for name in ["CHUNK_TerrainLogic", "CHUNK_Players", "CHUNK_GameLogic", "CHUNK_SidesList"] {
            assert!(sa.contains_key(name), "{name} saved");
            assert!(sa[name] == sc[name], "{name}: the twice-loaded game's is the original's");
        }
        let mut d = gate_map_world();
        d.load_game(bytes).expect("the load again");
        for f in 150..300 {
            let (cb, cd) = (step(&mut b), step(&mut d));
            assert_eq!(cb, cd, "two games loaded from one save part at frame {f}");
        }
        for w in [&a, &b, &c, &d] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
    }
}

#[cfg(test)]
mod contain_tests {
    use super::*;
    use crate::common::game_type::INVALID_ID;
    use crate::logic::world::module_call::ModuleCall;

    /// A truck that carries two vehicles, with an AI and the game's treads; and a rider, the game's soldier with a transport slot
    /// (`TransportContain::isValidContainerFor` refuses a rider whose `TransportSlotCount` is 0, the default, as the soldier's is).
    const TRUCK: &str = "\
Object Rider\n KindOf = VEHICLE\n TransportSlotCount = 1\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 WeaponSet\n  Conditions = None\n  Weapon = PRIMARY GateGun\n End\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n\
Object Truck\n KindOf = VEHICLE TRANSPORT\n Geometry = CYLINDER\n GeometryMajorRadius = 8\n GeometryHeight = 6\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 200\n  InitialHealth = 200\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = TransportContain ModuleTag_Contain\n  Slots = 2\n  AllowInsideKindOf = VEHICLE\n  AllowAlliesInside = Yes\n  ExitDelay = 0\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 100\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n";

    /// Batch I9h, the AI's contain and exit calls live. A soldier ordered into a truck 40 away walks to it and is contained
    /// (the enter state asks the truck's contain whether it is valid, says it wants to enter, and adds itself); ordered out,
    /// it leaves through the truck's exit (busy, reserve a door, exit via the door). Predicted before the run: in within 90
    /// frames (40 at 30 a second is 40 frames, plus turning and the approach), out within 30 (`ExitDelay = 0`); the
    /// contain's list is the soldier while it rides; no AI call reaches a cut point and no call is skipped as re-entrant.
    #[test]
    fn a_soldier_rides_a_truck() {
        let mut w = gate_map_world_with(TRUCK);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let soldier = make(&mut w, b"Rider", team, Coord3D { x: 60.0, y: 60.0, z: 0.0 });
        let truck = make(&mut w, b"Truck", team, Coord3D { x: 100.0, y: 60.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        w.cuts.trace = Some(Vec::new());
        let ai = w.find_object(soldier).and_then(|o| o.ai).expect("the soldier's AI");
        w.module_call(soldier, ai, ModuleCall::AiEnter { target: truck, command_source: 0 });
        let mut entered = None;
        for f in 0..90 {
            w.logic_update().unwrap();
            if w.find_object(soldier).is_some_and(|o| o.contained_by == truck) {
                entered = Some(f);
                break;
            }
        }
        let entered = entered.expect("the soldier is in the truck");
        let c = w.get_contain(truck).expect("the truck's contain");
        let items = match w.module_call(truck, c, ModuleCall::GetContainedItems) {
            crate::logic::world::module_call::ModuleReply::Ids(ids) => ids,
            r => panic!("{r:?}"),
        };
        assert_eq!(items, [soldier]);
        let ai = w.find_object(soldier).and_then(|o| o.ai).expect("the soldier's AI");
        w.module_call(soldier, ai, ModuleCall::AiExit { container: truck, command_source: 0 });
        let mut left = None;
        for f in 0..30 {
            w.logic_update().unwrap();
            if w.find_object(soldier).is_some_and(|o| o.contained_by == INVALID_ID) {
                left = Some(f);
                break;
            }
        }
        let left = left.expect("the soldier is out of the truck");
        // (out: no longer held, and its AI past the exit state: the exit's follow-path order reached it while it ran)
        assert!(w.find_object(soldier).is_some_and(|o| !o.disabled.any()));
        let state = match w.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: soldier, index: ai }) {
            Some(crate::logic::world::update_body::UpdateBody::AiUpdate(m)) => m.module.as_ref().and_then(|m| m.machine.as_ref()).map(|sm| sm.get_current_state_id()),
            _ => None,
        };
        assert_ne!(state, Some(crate::logic::ai::ai_update_types::AI_EXIT));
        let mut calls: Vec<String> = w.cuts.trace.take().unwrap().into_iter().filter_map(|t| t.split("call: \"").nth(1).map(|c| c.split('"').next().unwrap().to_string())).collect();
        calls.sort();
        calls.dedup();
        eprintln!("entered on frame {entered}, out {left} frames after the order; calls cut: {calls:?}");
        assert!(calls.iter().all(|c| !c.starts_with("contain_") && !c.starts_with("exit_") && !c.starts_with("own_exit") && c != "object_ai_free_to_exit"), "{calls:?}");
        assert!(w.cuts.reentrant_skips.is_empty(), "{:?}", w.cuts.reentrant_skips);
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}

#[cfg(test)]
mod supply_tests {
    use super::*;

    /// A warehouse of four boxes, a supply center, and a supply truck that carries two, at the game's treads; no bones, so the docks
    /// use the docker's own position ("If I don't have a bone, you are fine where you are") and their approach positions are dynamic.
    const SUPPLY: &str = "\
Object Warehouse\n KindOf = STRUCTURE IMMOBILE SUPPLY_SOURCE\n Geometry = BOX\n GeometryMajorRadius = 15\n GeometryMinorRadius = 15\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\n\
 Behavior = SupplyWarehouseCreate ModuleTag_Create\n End\n\
 Behavior = SupplyWarehouseDockUpdate ModuleTag_Dock\n  NumberApproachPositions = -1\n  AllowsPassthrough = No\n  StartingBoxes = 4\n  DeleteWhenEmpty = No\n End\nEnd\n\
Object Center\n KindOf = STRUCTURE IMMOBILE CASH_GENERATOR\n Geometry = BOX\n GeometryMajorRadius = 15\n GeometryMinorRadius = 15\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\n\
 Behavior = SupplyCenterCreate ModuleTag_Create\n End\n\
 Behavior = SupplyCenterDockUpdate ModuleTag_Dock\n  NumberApproachPositions = -1\n  AllowsPassthrough = No\n End\nEnd\n\
Object Harvester\n KindOf = VEHICLE HARVESTER\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = SupplyTruckAIUpdate ModuleTag_AI\n  MaxBoxes = 2\n  SupplyCenterActionDelay = 100\n  SupplyWarehouseActionDelay = 100\n  SupplyWarehouseScanDistance = 500\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n";

    /// Batch I9i, the supply chain live: the truck's supply machine, the docks, the resource gathering manager, the action manager's
    /// `canTransferSuppliesAt`. An idle truck wants supplies; with none it docks at the best warehouse, takes two boxes, docks at the
    /// best center and is paid for them. Predicted before the run: player 1's money up by 200 (two boxes at the default 100, no
    /// boost) within 600 frames (150 to the warehouse and 150 back at 30 a second, 300 frames, plus the docking); the warehouse down
    /// to two boxes; no cut point of the docks or the supply calls reached; no re-entrant skip.
    #[test]
    fn a_supply_truck_brings_money() {
        let mut w = gate_map_world_with(SUPPLY);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let warehouse = make(&mut w, b"Warehouse", team, Coord3D { x: 100.0, y: 250.0, z: 0.0 });
        let center = make(&mut w, b"Center", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let truck = make(&mut w, b"Harvester", team, Coord3D { x: 130.0, y: 100.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        // (as the center's `SupplyCenterProductionExitUpdate` does for a truck it makes, SupplyCenterProductionExitUpdate.cpp:135:
        // an idle truck wants supplies only when forced to)
        let ai = w.find_object(truck).and_then(|o| o.ai).expect("the truck's AI");
        w.module_call(truck, ai, crate::logic::world::module_call::ModuleCall::SupplyTruckSetForceWantingState(true));
        let p = w.get_controlling_player(truck).expect("the truck's player");
        let money = |w: &World| w.players.get_nth(p).map_or(0, |pl| pl.money.count_money());
        let start = money(&w);
        w.cuts.trace = Some(Vec::new());
        let mut paid = None;
        for f in 0..600 {
            w.logic_update().unwrap();
            if money(&w) != start {
                paid = Some(f);
                break;
            }
        }
        let boxes = w.find_module(warehouse, b"SupplyWarehouseDockUpdate").map(|d| w.module_call(warehouse, d, crate::logic::world::module_call::ModuleCall::SupplyWarehouseGetBoxesStored).as_int(-1));
        let mut calls: Vec<String> = w.cuts.trace.take().unwrap().into_iter().filter_map(|t| t.split("call: \"").nth(1).map(|c| c.split('"').next().unwrap().to_string())).collect();
        calls.sort();
        calls.dedup();
        eprintln!("paid on frame {paid:?}: {} -> {}; warehouse boxes {boxes:?}; center {center}; calls cut: {calls:?}", start, money(&w));
        assert!(paid.is_some(), "no money within 600 frames");
        assert_eq!(money(&w) - start, 200);
        assert_eq!(boxes, Some(2));
        assert!(calls.iter().all(|c| !c.starts_with("dock_") && !c.starts_with("supply_")), "{calls:?}");
        assert!(w.cuts.reentrant_skips.is_empty(), "{:?}", w.cuts.reentrant_skips);
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}

#[cfg(test)]
mod position_tests {
    use super::*;
    use crate::logic::ai::ai_states::AiStateHost;
    use crate::logic::terrain_visual::{LogicHeightMap, MAP_HEIGHT_SCALE};

    /// A pad of 50 by 50, a structure whose footprint holds 5 by 5 of the height map's grid points (10 apart) when centred on one.
    const PAD: &str = "\
Object Pad\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 25\n GeometryMinorRadius = 25\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\nEnd\nEnd\n";

    /// Batch I9j, the AI's position search live (`ThePartitionManager->findPositionAround`, no flags, as at every AI site). Predicted
    /// before the run: around a clear spot the spot itself (the search tries distance 0 first); around the middle of the gate's Depot
    /// (a box of 20 by 20 at (200, 200)) a point within 100 and outside the Depot's box; with the owner as the source to path from,
    /// the same kind of point; with the partition not live, none, and the call logged as a cut.
    #[test]
    fn a_position_is_found_clear_of_a_structure() {
        let mut w = gate_map_world();
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let depot = make(&mut w, b"Depot", team, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        let soldier = make(&mut w, b"Soldier", team, Coord3D { x: 60.0, y: 60.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let clear = Coord3D { x: 300.0, y: 300.0, z: 0.0 };
        let centre = Coord3D { x: 200.0, y: 200.0, z: 0.0 };
        let (at_clear, around_depot, with_source) = w.with_ai_self(soldier, |w| {
            (w.find_position_around_no_source(&clear, 0.0, 100.0), w.find_position_around_no_source(&centre, 0.0, 100.0), w.find_position_around(&centre, 0.0, 100.0))
        });
        eprintln!("clear {at_clear:?}; around the depot {around_depot:?}; with the soldier as source {with_source:?}");
        let at_clear = at_clear.expect("a position at a clear spot");
        assert_eq!((at_clear.x, at_clear.y), (clear.x, clear.y));
        for p in [around_depot.expect("a position around the depot"), with_source.expect("a position the soldier can reach")] {
            let (dx, dy) = (p.x - centre.x, p.y - centre.y);
            assert!(dx * dx + dy * dy <= 100.0 * 100.0 + 1e-3, "{p:?} is farther than 100");
            assert!(dx.abs() > 20.0 || dy.abs() > 20.0, "{p:?} is inside the depot {depot}");
        }
        w.cuts.live_partition = false;
        w.cuts.trace = Some(Vec::new());
        assert_eq!(w.with_ai_self(soldier, |w| w.find_position_around_no_source(&centre, 0.0, 100.0)), None);
        assert!(w.cuts.trace.take().unwrap().iter().any(|t| t.contains("\"find_position_around_no_source\"")));
    }

    /// Batch I9j, `TheTerrainLogic->flattenTerrain(obj)` live (a dozer's or a worker's new structure). The height map is flat at raw
    /// height 10 but for one point of 30 under the pad's centre. Predicted before the run from `TerrainLogic::flattenTerrain`: the 25
    /// grid points under the pad average 10.8, rounded to 11; the centre's height (30) does not cap it; `setRawMapHeight` only lowers,
    /// so the raised point drops to 11 and every other point stays 10. A pad that is never flattened keeps its 30.
    #[test]
    fn a_new_structure_flattens_the_ground_under_it() {
        let mut w = gate_map_world_with(PAD);
        let size = ICoord2D { x: 40, y: 40 };
        let mut data = vec![10u8; 1600];
        data[20 * 40 + 20] = 30;
        w.terrain.set_logic_height_map(Some(LogicHeightMap::new(40, 40, 0, vec![size], data).expect("the height map")));
        let raw = |w: &World, i: i32, j: i32| w.terrain.ground_height(i as f32 * 10.0, j as f32 * 10.0, false).0 / MAP_HEIGHT_SCALE;
        assert_eq!(raw(&w, 20, 20), 30.0);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let pad = make(&mut w, b"Pad", team, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        // (not live: the cut, and the ground as it was)
        w.with_ai_self(pad, |w| w.terrain_flatten(pad));
        assert_eq!(raw(&w, 20, 20), 30.0);
        w.cuts.live_terrain = true;
        w.with_ai_self(pad, |w| w.terrain_flatten(pad));
        let mut changed = Vec::new();
        for i in 0..40 {
            for j in 0..40 {
                let h = raw(&w, i, j);
                let before = if (i, j) == (20, 20) { 30.0 } else { 10.0 };
                if h != before {
                    changed.push((i, j, before, h));
                }
            }
        }
        eprintln!("changed: {changed:?}");
        assert_eq!(changed, [(20, 20, 30.0, 11.0)]);
    }
}

#[cfg(test)]
mod ocl_tests {
    use super::*;
    use crate::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
    use crate::logic::world::module_call::ModuleCall;

    /// A pair of pups as an object creation list; a crate that makes it when it dies; a gun that makes it when it fires, and a
    /// soldier that carries the gun.
    const OCL: &str = "\
ObjectCreationList OCL_Pair\n CreateObject\n  ObjectNames = Pup\n  Count = 2\n End\nEnd\n\
Weapon PopGun\n PrimaryDamage = 20\n PrimaryDamageRadius = 0\n AttackRange = 100\n DamageType = ARMOR_PIERCING\n DeathType = NORMAL\n WeaponSpeed = 999999\n DelayBetweenShots = 500\n FireOCL = OCL_Pair\nEnd\n\
Object Pup\n KindOf = INFANTRY\n Geometry = CYLINDER\n GeometryMajorRadius = 3\n GeometryHeight = 3\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 10\n  InitialHealth = 10\n End\nEnd\n\
Object Crate\n KindOf = STRUCTURE\n Geometry = BOX\n GeometryMajorRadius = 5\n GeometryMinorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 10\n  InitialHealth = 10\n End\n\
 Behavior = CreateObjectDie ModuleTag_Die\n  CreationList = OCL_Pair\n End\nEnd\n\
Object Popper\n KindOf = VEHICLE\n VisionRange = 150\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 WeaponSet\n  Conditions = None\n  Weapon = PRIMARY PopGun\n End\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n";

    /// The pups in the world: (id, position, team).
    fn pups(w: &World) -> Vec<(ObjectId, Coord3D, TeamId)> {
        w.objects.ids_in_order().into_iter().filter_map(|id| w.objects.find(id)).filter(|o| o.template().get_name() == b"Pup").map(|o| (o.id, o.get_position(), o.team)).collect()
    }

    /// Batch I11, the object creation lists run by the world. A crate of team 1 at (120, 140) with a `CreateObjectDie` of `OCL_Pair` is
    /// killed. Predicted before the run from `GenericObjectCreationNugget` (`Count = 2`, no offset, the default disposition
    /// `ON_GROUND_ALIGNED`): two pups, made in the kill, each at the crate's x and y on the flat ground (z 0), on player 1's default
    /// team (the crate's player); no error. With `live_ocl` off, none (the call only logged).
    #[test]
    fn a_dying_crate_makes_its_creation_list() {
        for live in [false, true] {
            let mut w = gate_map_world_with(OCL);
            w.cuts.live_ocl = live;
            let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
            let crate_id = make(&mut w, b"Crate", team, Coord3D { x: 120.0, y: 140.0, z: 0.0 });
            w.pathfinder_op(PfOp::NewMap);
            w.kill(crate_id, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
            let made = pups(&w);
            eprintln!("live {live}: {made:?}");
            if !live {
                assert!(made.is_empty());
                continue;
            }
            assert_eq!(made.len(), 2, "{made:?}");
            for (_, p, t) in &made {
                assert_eq!((p.x, p.y, p.z), (120.0, 140.0, 0.0));
                assert_eq!(*t, team);
            }
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
    }

    /// Batch I11, a weapon's `FireOCL` (found by `TheWeaponStore->postProcessLoad()`, which the world did not run before this batch).
    /// A popper of team 1 at (100, 200) is ordered to attack an enemy soldier 50 east. Predicted before the run: by its first shot,
    /// within 60 frames (in range already; the turn and the aim), two pups at the popper's x and y (the list's primary object is the
    /// shooter, `ObjectCreationList::create(ocl, source, NULL)`), on team 1.
    #[test]
    fn a_weapons_fire_ocl_makes_its_list() {
        let mut w = gate_map_world_with(OCL);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let popper = make(&mut w, b"Popper", team, Coord3D { x: 100.0, y: 200.0, z: 0.0 });
        let target = make(&mut w, b"Soldier", enemy, Coord3D { x: 150.0, y: 200.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let ai = w.find_object(popper).and_then(|o| o.ai).expect("the popper's AI");
        w.module_call(popper, ai, ModuleCall::AiAttackObject { victim: target, max_shots: 1, command_source: 0 });
        let mut frame = None;
        for f in 0..60 {
            w.logic_update().unwrap();
            if !pups(&w).is_empty() {
                frame = Some(f);
                break;
            }
        }
        let made = pups(&w);
        let at = w.find_object(popper).map(|o| o.get_position()).expect("the popper");
        eprintln!("pups on frame {frame:?}: {made:?}; popper at {at:?}");
        assert!(frame.is_some(), "no pup within 60 frames");
        assert_eq!(made.len(), 2, "{made:?}");
        for (_, p, t) in &made {
            assert_eq!((p.x, p.y), (at.x, at.y));
            assert_eq!(*t, team);
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}

#[cfg(test)]
mod parking_tests {
    use super::*;
    use crate::logic::ai::ai_states::AiStateHost;

    /// An airfield of one row of two spaces with runways, and a plane to park there.
    const AIRFIELD: &str = "\
Object Airfield\n KindOf = STRUCTURE IMMOBILE FS_AIRFIELD\n Geometry = BOX\n GeometryMajorRadius = 30\n GeometryMinorRadius = 30\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\n\
 Behavior = ParkingPlaceBehavior ModuleTag_Parking\n  NumRows = 1\n  NumCols = 2\n  HasRunways = Yes\n  ApproachHeight = 50\n End\nEnd\n\
Object Plane\n KindOf = AIRCRAFT\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n";

    /// Batch I13, the AI's parking place calls live. With no art the bones fall back to the airfield's own position. Predicted
    /// before the run: the airfield has a parking place and the depot none; ~~two runways~~ (**missed:** `getRunwayCount()` is
    /// `m_runways.size()` with no `buildInfo()`, ParkingPlaceBehavior.h:145, and the runways are built at the first reservation:
    /// 0 before it, as upstream, and 2 after, two columns with runways); two planes get
    /// spaces, each at the airfield's position, and a third is refused (one row of two); once the first releases its space it has
    /// none and the third gets one; the plain parking place answers no better assignment and no taxi or creation locations (the base
    /// class's `FALSE` and `NULL`); the approach height is the data's 50.
    #[test]
    fn planes_reserve_and_release_parking_spaces() {
        let mut w = gate_map_world_with(AIRFIELD);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let field_pos = Coord3D { x: 200.0, y: 100.0, z: 0.0 };
        let field = make(&mut w, b"Airfield", team, field_pos);
        let depot = make(&mut w, b"Depot", team, Coord3D { x: 100.0, y: 300.0, z: 0.0 });
        let planes: Vec<ObjectId> = (0..3).map(|i| make(&mut w, b"Plane", team, Coord3D { x: 60.0 + 20.0 * i as f32, y: 60.0, z: 0.0 })).collect();
        let a = planes[0];
        let r = w.with_ai_self(a, |w| {
            let exists = (w.parking_place_exists(field), w.parking_place_exists(depot));
            let runways_before = w.pp_runway_count(field);
            let approach = w.pp_approach_height(field);
            let first: Vec<_> = planes.iter().map(|&p| w.pp_reserve_space(field, p, 0.0, true)).collect();
            let runways = (runways_before, w.pp_runway_count(field));
            let held = planes.iter().map(|&p| w.pp_has_reserved_space(field, p)).collect::<Vec<_>>();
            w.pp_release_space(field, a);
            let after_release = (w.pp_has_reserved_space(field, a), w.pp_reserve_space(field, planes[2], 0.0, false).is_some());
            let base = (w.pp_calc_best_parking_assignment(field, a), w.pp_taxi_locations(field, a), w.pp_creation_locations(field, a));
            (exists, runways, approach, first, held, after_release, base)
        });
        eprintln!("{r:?}");
        let (exists, runways, approach, first, held, after_release, base) = r;
        assert_eq!(exists, (true, false));
        assert_eq!(runways, (0, 2));
        assert_eq!(approach, 50.0);
        for info in &first[..2] {
            let p = info.expect("a space").parking_space;
            assert_eq!((p.x, p.y), (field_pos.x, field_pos.y));
        }
        assert!(first[2].is_none(), "a third space: {:?}", first[2]);
        assert_eq!(held, [true, true, false]);
        assert_eq!(after_release, (false, true));
        assert_eq!(base, (None, None, None));
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}

#[cfg(test)]
mod missile_tests {
    use super::*;
    use crate::logic::world::module_call::ModuleCall;

    /// A rocket (a `MissileAIUpdate` projectile at a thrust locomotor) and a launcher whose gun fires it.
    const ROCKET: &str = "\
Locomotor RocketThrust\n Surfaces = AIR\n Speed = 200\n Acceleration = 1000\n Braking = 1000\n TurnRate = 720\n Appearance = THRUST\nEnd\n\
Weapon RocketGun\n PrimaryDamage = 30\n PrimaryDamageRadius = 5\n AttackRange = 150\n DamageType = EXPLOSION\n DeathType = NORMAL\n WeaponSpeed = 200\n DelayBetweenShots = 5000\n ProjectileObject = Rocket\nEnd\n\
Object Rocket\n KindOf = PROJECTILE\n Geometry = CYLINDER\n GeometryMajorRadius = 1\n GeometryHeight = 1\n\
 Behavior = MissileAIUpdate ModuleTag_AI\n  TryToFollowTarget = Yes\n  FuelLifetime = 5000\n  InitialVelocity = 100\n  IgnitionDelay = 0\n  DistanceToTravelBeforeTurning = 0\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 1\n End\n Locomotor = SET_NORMAL RocketThrust\nEnd\n\
Object Launcher\n KindOf = VEHICLE\n VisionRange = 200\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 WeaponSet\n  Conditions = None\n  Weapon = PRIMARY RocketGun\n End\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n";

    /// Batch I14: `MissileAIUpdate` as the world's projectile interface (none of its `ProjectileUpdateInterface` calls reached it, so a
    /// fired missile was never launched). A launcher of team 1 at (100, 200) is ordered to attack an enemy soldier 100 east. Predicted
    /// before the run: within 60 frames a rocket whose launcher is the launcher; ten frames after it appears it is east of the
    /// launcher; within 150 frames the soldier has lost 30 health (the gun's damage).
    #[test]
    fn a_launched_missile_flies_to_its_target() {
        let mut w = gate_map_world_with(ROCKET);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let launcher = make(&mut w, b"Launcher", team, Coord3D { x: 100.0, y: 200.0, z: 0.0 });
        let target = make(&mut w, b"Soldier", enemy, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let ai = w.find_object(launcher).and_then(|o| o.ai).expect("the launcher's AI");
        w.module_call(launcher, ai, ModuleCall::AiAttackObject { victim: target, max_shots: 1, command_source: 0 });
        let rocket = |w: &World| w.objects.ids_in_order().into_iter().find(|&id| w.objects.find(id).is_some_and(|o| o.template().get_name() == b"Rocket"));
        let health = |w: &World| w.find_object(target).map(|o| o.body).and_then(|b| b).map_or(-1.0, |_| 0.0);
        let _ = health;
        let mut fired = None;
        for f in 0..60 {
            w.logic_update().unwrap();
            if let Some(r) = rocket(&w) {
                fired = Some((f, r));
                break;
            }
        }
        let (f0, r) = fired.expect("a rocket within 60 frames");
        let b = w.find_object(r).and_then(|o| o.ai).expect("the rocket's AI");
        let launched_by = w.module_call(r, b, ModuleCall::ProjectileGetLauncherId);
        for _ in 0..10 {
            w.logic_update().unwrap();
        }
        let at = w.find_object(r).map(|o| o.get_position());
        let body = w.find_object(target).and_then(|o| o.body).expect("the soldier's body");
        let mut hit = None;
        for f in 0..150 {
            let h = w.module_call(target, body, ModuleCall::GetHealth).as_real(-1.0);
            if h < 100.0 {
                hit = Some((f, h));
                break;
            }
            w.logic_update().unwrap();
        }
        eprintln!("rocket {r} on frame {f0}, launcher {launched_by:?}, 10 frames later at {at:?}; hit {hit:?}; errors {:?}", w.cuts.team_live_errors);
        assert_eq!(launched_by, crate::logic::world::module_call::ModuleReply::Id(launcher));
        assert!(at.is_some_and(|p| p.x > 105.0), "{at:?}");
        assert_eq!(hit.map(|(_, h)| h), Some(70.0));
    }
}

#[cfg(test)]
mod ai_world_tests {
    use super::*;
    use crate::common::ascii_string::AsciiString;
    use crate::common::base_type::ICoord3D;
    use crate::logic::ai::ai_states::AiStateHost;
    use crate::logic::terrain_logic::TriggerArea;

    /// Batch I15, the AI's world questions live. Predicted before the run: a trigger area with corners (100, 100), (200, 100),
    /// (200, 300), (100, 300) has its centre at (150, 200) on the flat ground (z 0), its name, and the radius `PolygonTrigger::updateBounds`
    /// computes, whose half height is upstream's `(hi.y + lo.y) / 2` (a sum, kept): sqrt(50^2 + 200^2) = 206.155; the frame objects
    /// last changed trigger areas reads 0, then the frame on which an object is made after five frames; the script's
    /// `setChooseVictimAlwaysUsesNormal(TRUE)` reaches the AI's question; a soldier has no countermeasures.
    #[test]
    fn the_ais_world_questions_are_answered() {
        let mut w = gate_map_world();
        let corners = [(100, 100), (200, 100), (200, 300), (100, 300)].map(|(x, y)| ICoord3D { x, y, z: 0 });
        let zone = TriggerArea::new(AsciiString::try_from_bytes(b"Zone").expect("a name"), 7, false, corners.to_vec()).expect("the area");
        w.terrain.set_triggers(vec![zone]);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let soldier = make(&mut w, b"Soldier", team, Coord3D { x: 60.0, y: 60.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let before = w.with_ai_self(soldier, |w| w.frame_objects_changed_trigger_areas());
        for _ in 0..5 {
            w.logic_update().unwrap();
        }
        let made_on = w.frame();
        make(&mut w, b"Soldier", team, Coord3D { x: 80.0, y: 60.0, z: 0.0 });
        crate::logic::world::script_actions::ScriptActionsHost::sa_set_choose_victim_always_uses_normal(&mut w, true);
        let r = w.with_ai_self(soldier, |w| {
            let poly = w.polygon_trigger_by_name(b"Zone").expect("the zone");
            (w.polygon_center(poly), w.polygon_radius(poly), w.polygon_trigger_name(poly), w.frame_objects_changed_trigger_areas(), AiStateHost::choose_victim_always_uses_normal(w), AiStateHost::has_countermeasures(w))
        });
        eprintln!("before {before}; made on {made_on}; {r:?}");
        let (center, radius, name, after, choose, counter) = r;
        assert_eq!((center.x, center.y, center.z), (150.0, 200.0, 0.0));
        assert!((radius - 206.155).abs() < 1e-3, "{radius}");
        assert_eq!(name, b"Zone");
        assert_eq!((before, after), (0, made_on));
        assert!(choose);
        assert!(!counter);
    }
}

#[cfg(test)]
mod ai_members_tests {
    use super::*;
    use crate::common::game_type::INVALID_ID;
    use crate::common::ascii_string::AsciiString;
    use crate::common::base_type::ICoord3D;
    use crate::logic::ai::ai_guard::{GUARD_FILTER_ENEMIES, GUARD_FILTER_POLYGON, GUARD_FILTER_POSSIBLE_TO_ATTACK, GUARD_FILTER_SAME_MAP_STATUS};
    use crate::logic::ai::ai_states::AiStateHost;
    use crate::logic::ai::assault_transport_ai_update::MemberCommand;
    use crate::logic::terrain_logic::TriggerArea;

    /// Batch I16, the AI's member, group, team and guard-search questions live. A soldier A of team 1 at (100, 100), a friend F at
    /// (120, 100), an enemy E at (150, 100). Predicted before the run: the guard's search for enemies it can attack, of its map status,
    /// within 100 finds E, within 30 nothing, and limited to a polygon of x 0 to 130 nothing; the closest of the squad {E, F} to A is
    /// F (20 against 50); a group of A and F is centred at (110, 100); A is idle, and after a move order and a frame is moving and no
    /// longer idle; its chase flag set is read back; `team2` is the enemy team.
    #[test]
    fn the_ais_member_and_guard_questions_are_answered() {
        let mut w = gate_map_world();
        let corners = [(0, 0), (130, 0), (130, 400), (0, 400)].map(|(x, y)| ICoord3D { x, y, z: 0 });
        let west = TriggerArea::new(AsciiString::try_from_bytes(b"West").expect("a name"), 1, false, corners.to_vec()).expect("the area");
        w.terrain.set_triggers(vec![west]);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let a = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let f = make(&mut w, b"Soldier", team, Coord3D { x: 120.0, y: 100.0, z: 0.0 });
        let e = make(&mut w, b"Soldier", enemy, Coord3D { x: 150.0, y: 100.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let at = Coord3D { x: 100.0, y: 100.0, z: 0.0 };
        let filt = GUARD_FILTER_ENEMIES | GUARD_FILTER_POSSIBLE_TO_ATTACK | GUARD_FILTER_SAME_MAP_STATUS;
        let g = w.ai_create_group();
        for o in [a, f] {
            w.with_group(g, |gr, w| gr.add(w, o));
        }
        let r = w.with_ai_self(a, |w| {
            let west = w.polygon_trigger_by_name(b"West");
            let guard = (w.closest_guard_target(&at, 100.0, filt, None), w.closest_guard_target(&at, 30.0, filt, None), w.closest_guard_target(&at, 100.0, filt | GUARD_FILTER_POLYGON, west));
            let squad = w.closest_object_on_squad(&[e, f]);
            let center = w.group_center();
            let idle_before = w.member_ai_state_type(a);
            w.member_ai_set_allowed_to_chase(a, true);
            let team2 = w.team_by_name(b"team2");
            (guard, squad, center, idle_before, team2)
        });
        let chase = w.find_object(a).and_then(|o| o.ai).map(|ai| match w.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: a, index: ai }) {
            Some(crate::logic::world::update_body::UpdateBody::AiUpdate(m)) => m.module.as_ref().and_then(|m| m.ai.as_ref()).is_some_and(|x| x.allowed_to_chase),
            _ => false,
        });
        w.with_ai_self(f, |w| w.member_ai_command(a, MemberCommand::MoveToPosition { pos: Coord3D { x: 100.0, y: 300.0, z: 0.0 }, src: 2 }));
        w.logic_update().unwrap();
        let after = w.with_ai_self(f, |w| (w.member_ai_state_type(a), w.member_ai_is_moving(a)));
        eprintln!("{r:?}; chase {chase:?}; after {after:?}");
        let (guard, squad, center, idle_before, team2) = r;
        assert_eq!(guard, (e, INVALID_ID, INVALID_ID));
        assert_eq!(squad, f);
        assert_eq!((center.x, center.y), (110.0, 100.0));
        assert_eq!(idle_before, crate::logic::ai::ai_update_types::AI_IDLE);
        assert_eq!(chase, Some(true));
        assert_ne!(after.0, crate::logic::ai::ai_update_types::AI_IDLE);
        assert!(after.1);
        assert_eq!(team2, Some(enemy));
    }
}

#[cfg(test)]
mod ai_structures_tests {
    use super::*;
    use crate::logic::ai::ai_states::AiStateHost;

    /// Batch I17, the AI's tunnel network, railed dock and bridge questions live. Predicted before the run: player 1 has a tunnel
    /// system (`Player::init` makes one) with no tunnels; its nemesis set to an enemy soldier is its current nemesis; a soldier has no
    /// railed dock; a depot has no bridge interface (no towers, no scaffold); its bridge attack points with the terrain live are both
    /// its own position (`getBridgeAttackPoints` for an object that is no bridge), and from the table otherwise (none: the origin).
    #[test]
    fn the_ais_structure_questions_are_answered() {
        let mut w = gate_map_world();
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let a = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let e = make(&mut w, b"Soldier", enemy, Coord3D { x: 150.0, y: 100.0, z: 0.0 });
        let depot_pos = Coord3D { x: 200.0, y: 200.0, z: 0.0 };
        let depot = make(&mut w, b"Depot", team, depot_pos);
        w.pathfinder_op(PfOp::NewMap);
        let r = w.with_ai_self(a, |w| {
            let tunnels = (w.tunnel_system(), w.tunnel_container_list());
            w.tunnel_update_nemesis(e);
            let nemesis = w.tunnel_cur_nemesis();
            let railed = w.railed_dock_present();
            let bridge = (w.bridge_tower_ids(depot), w.bridge_scaffold_in_motion(depot));
            let table = w.bridge_attack_points(depot);
            w.cuts.live_terrain = true;
            let live = w.bridge_attack_points(depot);
            (tunnels, nemesis, railed, bridge, table, live)
        });
        eprintln!("{r:?}");
        let (tunnels, nemesis, railed, bridge, table, live) = r;
        assert_eq!(tunnels, (true, Vec::new()));
        assert_eq!(nemesis, e);
        assert!(!railed);
        assert_eq!(bridge, (None, None));
        assert_eq!(table, (Coord3D::default(), Coord3D::default()));
        assert_eq!(live, (depot_pos, depot_pos));
    }
}

#[cfg(test)]
mod airfield_search_tests {
    use super::*;
    use crate::logic::ai::ai_states::AiStateHost;

    /// An airfield of one space, and a plane.
    const FIELD: &str = "\
Object Field\n KindOf = STRUCTURE IMMOBILE FS_AIRFIELD\n Geometry = BOX\n GeometryMajorRadius = 20\n GeometryMinorRadius = 20\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\n\
 Behavior = ParkingPlaceBehavior ModuleTag_Parking\n  NumRows = 1\n  NumCols = 1\n  HasRunways = Yes\n End\nEnd\n\
Object Plane\n KindOf = AIRCRAFT\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n";

    /// Batch I18, `findSuitableAirfield`'s search with its parking place filter inside it. Two airfields of one space, the near one 50
    /// from the jet already holding plane P, the far one 150 away. Predicted before the run: the far airfield (upstream filters each
    /// candidate by `PartitionFilterHasParkingPlace` inside the search; the old host took the closest and tested it alone, and answered
    /// none); the far one then holds the jet's space (the filter's reservation) and the near one still P's; the attack info of an
    /// unknown name is index 0, the default, with an empty name.
    #[test]
    fn a_jet_finds_the_nearest_airfield_with_room() {
        let mut w = gate_map_world_with(FIELD);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let near = make(&mut w, b"Field", team, Coord3D { x: 150.0, y: 100.0, z: 0.0 });
        let far = make(&mut w, b"Field", team, Coord3D { x: 250.0, y: 100.0, z: 0.0 });
        let p = make(&mut w, b"Plane", team, Coord3D { x: 150.0, y: 150.0, z: 0.0 });
        let jet = make(&mut w, b"Plane", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let r = w.with_ai_self(jet, |w| {
            let held = w.pp_reserve_space(near, p, 0.0, false).is_some();
            let found = w.jet_airfield_candidate();
            let spaces = (w.pp_has_reserved_space(near, p), w.pp_has_reserved_space(near, jet), w.pp_has_reserved_space(far, jet));
            let info = (w.attack_info_by_name(b"NoSuchList"), w.attack_info_name(0));
            (held, found, spaces, info)
        });
        eprintln!("{r:?}");
        let (held, found, spaces, info) = r;
        assert!(held);
        assert_eq!(found, Some(far));
        assert_eq!(spaces, (true, false, true));
        assert_eq!(info, (Some(0), Vec::new()));
    }
}

#[cfg(test)]
mod command_button_tests {
    use super::*;
    use crate::logic::ai::ai_states::AiStateHost;

    /// A tank with a command set, the set and its buttons, all in the test's INI (any block may be in any file, as upstream's table
    /// is one); the buttons after the tank, whose template a button names.
    const BUTTONS: &str = "\
Object Tank\n KindOf = VEHICLE\n CommandSet = TankSet\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\nEnd\n\
CommandButton Command_Stop\n Command = STOP\nEnd\n\
CommandButton Command_Hijack\n Command = HIJACK_VEHICLE\n Object = Tank\nEnd\n\
CommandSet TankSet\n 1 = Command_Stop\n 2 = Command_Hijack\n 4 = Command_Stop\nEnd\n";

    /// Batch I19, the command buttons and sets read from the INI and installed in the world. Predicted before the run: the tank's AI
    /// finds `Command_Stop` by name; its set (`TankSet`, by the template's `CommandSet`) holds it twice, and its type is `STOP` (12);
    /// `Command_Hijack` once, `HIJACK_VEHICLE` (27), naming the tank's template; an unknown button is not found.
    #[test]
    fn a_units_command_set_is_read_from_the_ini() {
        let mut w = gate_map_world_with(BUTTONS);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let tank = make(&mut w, b"Tank", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let tank_template = w.find_object(tank).map(|o| o.template().get_template_id());
        let r = w.with_ai_self(tank, |w| {
            let stop = w.command_button_by_name(b"Command_Stop").expect("the stop button");
            let hijack = w.command_button_by_name(b"Command_Hijack").expect("the hijack button");
            (w.command_set_button_matches(stop), w.command_set_button_matches(hijack), w.command_button_by_name(b"NoSuchButton"), w.cuts.command_buttons[hijack as usize].thing_template)
        });
        eprintln!("{r:?}");
        assert_eq!(r, ((2, 12), (1, 27), None, tank_template));
    }
}

#[cfg(test)]
mod script_action_tests {
    use super::*;
    use crate::logic::world::module_call::ModuleCall;
    use crate::logic::world::script_actions::ScriptActionsHost;

    /// Batch I20, the script actions' logic live. Predicted before the run: a `GameLogic` member set while a frame holds `GameLogic` (as
    /// a script's is: scripts run inside the frame) is not in `game_logic` at once, and is after the frame (the hulk lifetime override,
    /// 300; before this batch it was written to the frame's placeholder and lost); set outside a frame it is there at once (the icon
    /// UI off; a control bar override found for its set and slot); a soldier made indestructible keeps its 100 health under 1000 damage;
    /// a team made not recruitable reads so.
    #[test]
    fn script_actions_reach_the_logic() {
        let mut w = gate_map_world();
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let s = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        w.in_logic_frame = true;
        w.sa_logic_set_hulk_max_lifetime_override(300);
        let during = w.game_logic.script_hulk_max_lifetime_override;
        w.in_logic_frame = false;
        w.logic_update().unwrap();
        let after = w.game_logic.script_hulk_max_lifetime_override;
        w.sa_logic_set_option(1, false);
        w.sa_logic_set_control_bar_override(b"TankSet", 3, Some(5));
        let found = crate::logic::world::game_logic_update::find_control_bar_override(&w.game_logic, b"TankSet", 3).ok().flatten();
        w.sa_body_set_indestructible(s, true);
        w.sa_obj_attempt_damage(s, crate::common::game_type::INVALID_ID, 0, 0, 1000.0);
        let body = w.find_object(s).and_then(|o| o.body).expect("the body");
        let health = w.module_call(s, body, ModuleCall::GetHealth).as_real(-1.0);
        w.sa_team_set_recruitable(team as i32, false);
        let recruitable = w.teams.team(team).map(|t| t.is_recruitable);
        eprintln!("hulk during {during} after {after}; icon {}; override {found:?}; health {health}; recruitable {recruitable:?}", w.game_logic.draw_icon_ui);
        assert_ne!(during, 300);
        assert_eq!(after, 300);
        assert!(!w.game_logic.draw_icon_ui);
        assert_eq!(found, Some(Some(5)));
        assert_eq!(health, 100.0);
        assert_eq!(recruitable, Some(false));
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}

#[cfg(test)]
mod build_assistant_ai_tests {
    use super::*;
    use crate::common::ascii_string::AsciiString;
    use crate::common::base_type::ICoord3D;
    use crate::common::game_type::INVALID_ID;
    use crate::logic::ai::ai_update::AiHost;
    use crate::logic::world::build_assistant::BuildAssistantHost;
    use crate::logic::terrain_logic::TriggerArea;

    /// Batch I21, the area-limited enemy search and the build assistant's two AI calls live. A soldier A of team 1 at (100, 100), enemy
    /// E1 at (150, 100) and E2 at (100, 180), a trigger area `West` of x 0 to 130 and y 0 to 400, and `East` of x 300 to 400. Predicted
    /// before the run: the closest enemy within 200 is E1 (50 against 80); within `West` it is E2 (upstream's optional
    /// `PartitionFilterPolygonTrigger`; before I21 the query with an area was the cut table, none); within `East` none; a quick path to
    /// (100, 300) on the flat open map is available; after `aiMoveToPositionEvenIfSleeping` to (100, 300) and a frame, A is moving.
    #[test]
    fn the_build_assistants_ai_calls_and_the_area_enemy_search_are_live() {
        let mut w = gate_map_world();
        let area = |name: &[u8], id, x0, x1| {
            let corners = [(x0, 0), (x1, 0), (x1, 400), (x0, 400)].map(|(x, y)| ICoord3D { x, y, z: 0 });
            TriggerArea::new(AsciiString::try_from_bytes(name).expect("a name"), id, false, corners.to_vec()).expect("the area")
        };
        w.terrain.set_triggers(vec![area(b"West", 1, 0, 130), area(b"East", 2, 300, 400)]);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let a = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let e1 = make(&mut w, b"Soldier", enemy, Coord3D { x: 150.0, y: 100.0, z: 0.0 });
        let e2 = make(&mut w, b"Soldier", enemy, Coord3D { x: 100.0, y: 180.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let r = w.with_ai_self(a, |w| {
            let west = w.terrain.trigger_area_by_name(&AsciiString::try_from_bytes(b"West").expect("a name")).map(|i| i as i32);
            let east = w.terrain.trigger_area_by_name(&AsciiString::try_from_bytes(b"East").expect("a name")).map(|i| i as i32);
            (AiHost::find_closest_enemy(w, 200.0, 0, None, None), AiHost::find_closest_enemy(w, 200.0, 0, None, west), AiHost::find_closest_enemy(w, 200.0, 0, None, east))
        });
        let dest = Coord3D { x: 100.0, y: 300.0, z: 0.0 };
        let quick = w.ai_is_quick_path_available(a, dest);
        w.ai_move_to_position_even_if_sleeping(a, dest);
        w.logic_update().unwrap();
        let moving = w.with_ai_self(e1, |w| crate::logic::ai::ai_states::AiStateHost::member_ai_is_moving(w, a));
        eprintln!("{r:?}; quick {quick}; moving {moving}");
        assert_eq!(r, (e1, e2, INVALID_ID));
        assert!(quick);
        assert!(moving);
    }
}
