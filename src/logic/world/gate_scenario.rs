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
    w.cuts.live_special_powers = true;
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
            options: 0,
            max_shots_to_fire: 0,
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
    pub(super) const TRUCK: &str = "\
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

#[cfg(test)]
mod force_fire_tests {
    use super::*;
    use crate::logic::world::module_call::{ModuleCall, ModuleReply};
    use crate::logic::world::script_actions::ScriptActionsHost;

    /// The batch-I14 rocket, fired by a gun capable of following waypoints, and a launcher carrying it.
    const FOLLOWER: &str = "\
Locomotor RocketThrust\n Surfaces = AIR\n Speed = 200\n Acceleration = 1000\n Braking = 1000\n TurnRate = 720\n Appearance = THRUST\nEnd\n\
Weapon FollowGun\n PrimaryDamage = 30\n PrimaryDamageRadius = 5\n AttackRange = 150\n DamageType = EXPLOSION\n DeathType = NORMAL\n WeaponSpeed = 200\n DelayBetweenShots = 5000\n ProjectileObject = Rocket\n CapableOfFollowingWaypoints = Yes\nEnd\n\
Object Rocket\n KindOf = PROJECTILE\n Geometry = CYLINDER\n GeometryMajorRadius = 1\n GeometryHeight = 1\n\
 Behavior = MissileAIUpdate ModuleTag_AI\n  TryToFollowTarget = Yes\n  FuelLifetime = 5000\n  InitialVelocity = 100\n  IgnitionDelay = 0\n  DistanceToTravelBeforeTurning = 0\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 1\n End\n Locomotor = SET_NORMAL RocketThrust\nEnd\n\
Object Follower\n KindOf = VEHICLE\n VisionRange = 200\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 WeaponSet\n  Conditions = None\n  Weapon = PRIMARY FollowGun\n End\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n";

    /// Batch I22, the script's `doNamedFireWeaponFollowingWaypointPath` weapon calls live. Predicted before the run: the follower has a
    /// waypoint-following weapon and the soldier none (the host answered false for every unit before, so the action never fired); the
    /// follower's force-fire at its own position makes a rocket whose launcher is the follower and which has an AI (the action then
    /// sends it along the path); ~~a second force-fire at once makes a second rocket (upstream's `forceFireWeapon` checks no reload)~~
    /// **missed:** the second makes nothing. `forceFireWeapon` itself checks nothing, but the `privateFireWeapon` it calls returns
    /// early when `getStatus() != READY_TO_FIRE` (Weapon.cpp:2594), and the first shot left the gun `BETWEEN_FIRING_SHOTS` for its
    /// 5000 ms; the prediction read only `forceFireWeapon`'s body. The soldier's makes nothing.
    #[test]
    fn a_waypoint_following_weapon_is_force_fired() {
        let mut w = gate_map_world_with(FOLLOWER);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let f = make(&mut w, b"Follower", team, Coord3D { x: 100.0, y: 200.0, z: 0.0 });
        let s = make(&mut w, b"Soldier", team, Coord3D { x: 150.0, y: 200.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let has = (w.sa_obj_has_waypoint_following_weapon(f), w.sa_obj_has_waypoint_following_weapon(s));
        let pos = Coord3D { x: 100.0, y: 200.0, z: 0.0 };
        let first = w.sa_weapon_force_fire(f, &pos);
        let second = w.sa_weapon_force_fire(f, &pos);
        let none = w.sa_weapon_force_fire(s, &Coord3D { x: 150.0, y: 200.0, z: 0.0 });
        let r = first.expect("a rocket");
        let ai = w.find_object(r).and_then(|o| o.ai);
        let launched_by = ai.map(|b| w.module_call(r, b, ModuleCall::ProjectileGetLauncherId));
        let name = w.find_object(r).map(|o| o.template().get_name().to_vec());
        eprintln!("{has:?}; {first:?} {second:?} {none:?}; launcher {launched_by:?}; errors {:?}", w.cuts.team_live_errors);
        assert_eq!(has, (true, false));
        assert_eq!(name.as_deref(), Some(&b"Rocket"[..]));
        assert_eq!(launched_by, Some(ModuleReply::Id(f)));
        assert_eq!(second, None);
        assert_eq!(none, None);
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}

#[cfg(test)]
mod script_query_tests {
    use super::*;
    use crate::logic::scripts::{action_type as at, Parameter, ScriptAction};
    use crate::logic::world::module_call::ModuleCall;
    use crate::logic::world::script_actions::ScriptActionsHost;

    /// Batch I22, the script actions' object, contain and AI queries live (each answered `Default::default()` before, which closed the
    /// actions they gate). A truck `Truck1` (two slots) holding a rider, and a second rider outside. Predicted before the run: the
    /// truck's contain max 2, count 1, contained [rider], valid for the second rider with capacity checked; the rider's transport
    /// slot count 1 and a current locomotor; the truck has no railroad, cave, warehouse, sticky bomb or hunt module; an unknown attack
    /// info is the default's empty name; `"hidden"` is availability 2 (case-insensitive) and `"x"` none; ~~the truck's transform at
    /// orientation 0 leaves (1, 2, 3) as it is~~ **missed:** (101, 62, 3). WW3D's `Matrix3D::Transform_Vector` adds the translation
    /// (`A[i][3]`, matrix3d.h:1459), as the sticky-bomb action needs (an offset on the perimeter made a world position); the
    /// prediction took it for a rotation only (`Rotate_Vector`). Then `UNIT_DESTROY_ALL_CONTAINED Truck1` kills the rider inside (before this batch
    /// the count 0 returned at once) and not the one outside.
    #[test]
    fn the_script_queries_reach_the_objects() {
        let mut w = gate_map_world_with(super::contain_tests::TRUCK);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let truck = make(&mut w, b"Truck", team, Coord3D { x: 100.0, y: 60.0, z: 0.0 });
        let rider = make(&mut w, b"Rider", team, Coord3D { x: 60.0, y: 60.0, z: 0.0 });
        let outside = make(&mut w, b"Rider", team, Coord3D { x: 60.0, y: 90.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let c = w.find_object(truck).and_then(|o| o.contain).expect("the truck's contain");
        w.module_call(truck, c, ModuleCall::AddToContain(rider));
        let contain = (w.sa_contain_max(truck), w.sa_contain_count(truck), w.sa_contained(truck), w.sa_contain_is_valid_container_for(truck, outside, true));
        let rider_q = (w.sa_obj_transport_slot_count(rider), w.sa_ai_has_cur_locomotor(rider));
        let modules = [w.sa_obj_has_railroad(truck), w.sa_obj_has_cave(truck), w.sa_obj_has_warehouse(truck), w.sa_obj_has_sticky_bomb_update(truck), w.sa_obj_has_command_button_hunt(truck)];
        let names = (w.sa_attack_info(b"NoSuchList"), w.sa_player_science_availability_from_string(1, b"hidden"), w.sa_player_science_availability_from_string(1, b"x"));
        let v = w.sa_obj_transform_vector(truck, &Coord3D { x: 1.0, y: 2.0, z: 3.0 });
        let mut engine = std::mem::take(&mut w.script_engine);
        engine.named_objects.push((b"Truck1".to_vec(), Some(truck)));
        let mut a = ScriptAction::new_for_read();
        a.action_type = at::UNIT_DESTROY_ALL_CONTAINED;
        a.num_parms = 1;
        let mut parm = Parameter::new(0, 0);
        parm.string = b"Truck1".to_vec();
        a.parms[0] = Some(parm);
        w.execute_action_with(&mut engine, &a).unwrap();
        w.script_engine = engine;
        let dead = |w: &World, o| w.find_object(o).is_none_or(|x| x.is_effectively_dead());
        let killed = (dead(&w, rider), dead(&w, outside));
        eprintln!("{contain:?}; {rider_q:?}; {modules:?}; {names:?}; {v:?}; killed {killed:?}");
        assert_eq!(contain, (2, 1, vec![rider], true));
        assert_eq!(rider_q, (1, true));
        assert_eq!(modules, [false; 5]);
        assert_eq!(names, (Vec::new(), Some(2), None));
        assert_eq!((v.x, v.y, v.z), (101.0, 62.0, 3.0));
        assert_eq!(killed, (true, false));
    }
}

#[cfg(test)]
mod command_button_object_tests {
    use super::*;
    use crate::logic::world::script_actions::ScriptActionsHost;

    /// A caster with an `OCLSpecialPower` (its OCL drops a pup at the target location) and an AI; its command set holds the power's
    /// button and a stop button.
    pub(super) const CASTER: &str = "\
ObjectCreationList OCL_Drop\n CreateObject\n  ObjectNames = Pup\n  Count = 1\n End\nEnd\n\
SpecialPower DropPower\n Enum = SPECIAL_DAISY_CUTTER\n ReloadTime = 0\n RadiusCursorRadius = 40\nEnd\n\
Object Pup\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 2\n GeometryHeight = 2\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 10\n  InitialHealth = 10\n End\nEnd\n\
Object Caster\n KindOf = VEHICLE\n CommandSet = CasterSet\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = OCLSpecialPower ModuleTag_Drop\n  SpecialPowerTemplate = DropPower\n  OCL = OCL_Drop\n  CreateLocation = CREATE_AT_LOCATION\n  ReferenceObject = Pup\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n\
CommandButton Command_Drop\n Command = SPECIAL_POWER\n SpecialPower = DropPower\n Options = NEED_TARGET_POS\nEnd\n\
CommandButton Command_Halt\n Command = STOP\nEnd\n\
CommandSet CasterSet\n 1 = Command_Drop\n 2 = Command_Halt\nEnd\n";

    /// Batch I23, an object carrying out command buttons and the special power store's check, live. Predicted before the run: the
    /// caster can use `DropPower` (it has the module; the power needs no science) and the soldier cannot (no module); the caster's module
    /// is ready (reload 0), its reference template is the pup's, the power's cursor radius 40; `Command_Drop` at (200, 200), as a script
    /// orders it, makes one pup there (`CREATE_AT_LOCATION`; before this batch `doCommandButtonAtPosition` was a cut and nothing was
    /// made); a group of the caster finds it as the source of the power and of a stop button (its command set); the caster ordered to
    /// move and then `Command_Halt` by the group is idle after a frame. Ablated, the table's check allows the soldier.
    #[test]
    fn an_object_carries_out_its_command_buttons() {
        let mut w = gate_map_world_with(CASTER);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let caster = make(&mut w, b"Caster", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let soldier = make(&mut w, b"Soldier", team, Coord3D { x: 60.0, y: 100.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let power = w.statics.special_power_store.find_special_power_template(b"DropPower").map(|p| p.id()).expect("the power");
        let button = |w: &World, n: &[u8]| w.cuts.command_buttons.iter().position(|b| b.name == n).expect("the button") as i32;
        let (drop, halt) = (button(&w, b"Command_Drop"), button(&w, b"Command_Halt"));
        let pup = World::thing_factory_find_template(&w, b"Pup").expect("the pup's template");
        let can = (w.can_use_special_power(caster, Some(power)), w.can_use_special_power(soldier, Some(power)));
        let module = (w.sa_spm_is_ready(caster, power as i32), w.sa_spm_reference_template(caster, power as i32), w.sa_special_power_radius_cursor(power as i32));
        let pups = |w: &World| w.objects.ids_in_order().into_iter().filter(|&id| w.objects.find(id).is_some_and(|o| o.template().get_name() == b"Pup")).collect::<Vec<_>>();
        w.sa_obj_do_command_button_at_position(caster, drop, &Coord3D { x: 200.0, y: 200.0, z: 0.0 }, CMD_FROM_SCRIPT_I32);
        let made = pups(&w);
        let at = made.first().and_then(|&p| w.find_object(p)).map(|o| o.get_position());
        let g = w.ai_create_group();
        w.with_group(g, |gr, w| gr.add(w, caster));
        let sources = (w.sa_group_special_power_source(g as i32, power), w.sa_group_command_button_source(g as i32, 12));
        w.with_ai_self(soldier, |w| crate::logic::ai::ai_states::AiStateHost::member_ai_command(w, caster, crate::logic::ai::assault_transport_ai_update::MemberCommand::MoveToPosition { pos: Coord3D { x: 100.0, y: 300.0, z: 0.0 }, src: 2 }));
        w.logic_update().unwrap();
        let moving = w.with_ai_self(soldier, |w| crate::logic::ai::ai_states::AiStateHost::member_ai_state_type(w, caster));
        w.sa_group_do_command_button(g as i32, halt, CMD_FROM_SCRIPT_I32);
        w.logic_update().unwrap();
        let after = w.with_ai_self(soldier, |w| crate::logic::ai::ai_states::AiStateHost::member_ai_state_type(w, caster));
        eprintln!("can {can:?}; module {module:?}; pups {made:?} at {at:?}; sources {sources:?}; state {moving} then {after}; errors {:?} {:?}", w.cuts.team_live_errors, w.cuts.module_errors);
        assert_eq!(can, (true, false));
        assert_eq!(module, (true, Some(i32::from(pup)), 40.0));
        assert_eq!(made.len(), 1);
        assert_eq!(at.map(|p| (p.x, p.y)), Some((200.0, 200.0)));
        assert_eq!(sources, (Some(caster), Some(caster)));
        assert_ne!(moving, crate::logic::ai::ai_update_types::AI_IDLE);
        assert_eq!(after, crate::logic::ai::ai_update_types::AI_IDLE);
        // ablation: without `live_special_powers` the store's check is the table, whose empty deny list allows the soldier too
        w.cuts.live_special_powers = false;
        assert!(w.can_use_special_power(soldier, Some(power)));
    }

    const CMD_FROM_SCRIPT_I32: i32 = 1;
}

#[cfg(test)]
mod group_host_tests {
    use super::*;
    use crate::logic::ai::ai_group::AiGroupHost;
    use crate::logic::world::module_call::ModuleCall;

    /// The batch-I9h truck and rider, and an object upgrade.
    fn fixture() -> String {
        format!("{}Upgrade Upgrade_Armor\n Type = OBJECT\n BuildCost = 500\nEnd\n", super::contain_tests::TRUCK)
    }

    /// Batch I24, the group host's remaining logic calls live (each was a cut answering a default). A truck holding a rider, a soldier and
    /// a depot of team 1. Predicted before the run: the soldier pristine (damage state 0); the truck's contained [rider], its passengers
    /// not allowed to fire (`TransportContain` without `PassengersAllowedToFire`); no spawn behaviour on the soldier; the soldier's
    /// template has a weapon set for the veteran flag (`findWeaponTemplateSet` finds the best match, here its only set, never `NULL` while
    /// there is one); `Upgrade_Armor` an object upgrade the soldier does not have; the soldier cannot produce it (it has no command set,
    /// which upstream dereferences: an error recorded); no stealth delay (no stealth module); cheering for 90 frames holds the special
    /// condition until the frame plus 90; the depot sold is on the build assistant's sell list; ordered out, the rider leaves within 30
    /// frames.
    #[test]
    fn the_group_host_answers_from_the_objects() {
        let mut w = gate_map_world_with(&fixture());
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let truck = make(&mut w, b"Truck", team, Coord3D { x: 100.0, y: 60.0, z: 0.0 });
        let rider = make(&mut w, b"Rider", team, Coord3D { x: 60.0, y: 60.0, z: 0.0 });
        let soldier = make(&mut w, b"Soldier", team, Coord3D { x: 60.0, y: 120.0, z: 0.0 });
        let depot = make(&mut w, b"Depot", team, Coord3D { x: 250.0, y: 250.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let c = w.find_object(truck).and_then(|o| o.contain).expect("the truck's contain");
        w.module_call(truck, c, ModuleCall::AddToContain(rider));
        let armor = w.statics.upgrade_center.templates().find(|u| u.upgrade_name() == b"Upgrade_Armor").map(|u| u.upgrade_name_key().0).expect("the upgrade");
        let objects = (AiGroupHost::damage_state(&mut w, soldier), AiGroupHost::contained(&mut w, truck), w.contain_passengers_allowed_to_fire(truck), w.has_spawn(soldier));
        let templates = (w.template_has_weapon_set(soldier, 0 /* WEAPONSET_VETERAN */), w.upgrade_is_object_type(armor), AiGroupHost::has_upgrade(&mut w, soldier, armor));
        let produce = AiGroupHost::can_produce_upgrade(&mut w, soldier, armor);
        let stealth = w.stealth_delay(soldier);
        let now = w.frame();
        w.set_cheering(soldier, 90);
        let cheer = w.find_object(soldier).map(|o| o.smc_until);
        AiGroupHost::sell(&mut w, depot);
        let selling = w.build_assistant.sell_list.len();
        w.contain_order_all_passengers_to_exit(truck, 1);
        let mut out = None;
        for f in 0..30 {
            w.logic_update().unwrap();
            if AiGroupHost::contained(&mut w, truck).is_empty() {
                out = Some(f);
                break;
            }
        }
        eprintln!("{objects:?}; {templates:?}; produce {produce}; stealth {stealth:?}; cheer {cheer:?} (now {now}); selling {selling}; out {out:?}; errors {:?}", w.cuts.team_live_errors);
        assert_eq!(objects, (0, vec![rider], false, false));
        assert_eq!(templates, (true, true, false));
        assert!(!produce);
        assert_eq!(w.cuts.team_live_errors.iter().filter(|e| e.contains("canProduceUpgrade")).count(), 1);
        assert_eq!(stealth, None);
        assert_eq!(cheer, Some(now + 90));
        assert_eq!(selling, 1);
        assert!(out.is_some());
    }
}

#[cfg(test)]
mod sequential_script_tests {
    use super::*;
    use crate::logic::scripts::{action_type as at, Parameter, Script, ScriptAction, ScriptList};
    use crate::logic::world::script_actions::ScriptActionsHost;

    /// A subroutine `KillMe` whose one action kills the object running it (`NAMED_KILL <This Object>`), in side 0's list (a side added
    /// for it).
    fn add_kill_me(w: &mut World) {
        let mut a = ScriptAction::new_for_read();
        a.action_type = at::NAMED_KILL;
        a.num_parms = 1;
        let mut p = Parameter::new(0, 0);
        p.string = crate::logic::world::script_engine::names::THIS_OBJECT.to_vec();
        a.parms[0] = Some(p);
        let mut s = Script::new();
        s.script_name = b"KillMe".to_vec();
        s.is_active = true;
        s.is_subroutine = true;
        s.actions = vec![a];
        // (the gate's map has no sides: one, holding only the script list)
        if w.sides.get_num_sides() == 0 {
            w.sides.add_side(&Dict::default());
        }
        w.sides.get_side_info_mut(0).expect("side 0").scripts = Some(ScriptList { groups: Vec::new(), scripts: vec![s] });
    }

    /// Batch I25, the sequential script actions live (each was a cut). Predicted before the run: appending `KillMe` for a soldier makes
    /// one chain headed by it; its timer set to 7 is the head's wait; removing the object's scripts leaves none; a team's chain is
    /// removed by `removeAllSequentialScripts(team)`; an unknown script appends nothing; appended again, the soldier is dead within three
    /// frames (the engine progresses the chain in its update and runs the action on the soldier; before this batch it lived).
    /// **Missed on the first run, the test's setup:** the soldier lived, because the gate fixture never runs the engine's `newMap`, so
    /// its update returned at once (`m_endGameTimer >= 0`, as upstream's constructed engine); the test now calls it first. Then it still
    /// lived: **port defect**, the engine asked `se_ai_is_idle`, which read the `cuts.ai_idle` table (no live unit is in it), so no
    /// unit's sequential script ever started; with `live_ai` it is now the AI's own `isIdle()`, and the soldier dies on the first frame.
    #[test]
    fn a_unit_runs_a_sequential_script() {
        let mut w = gate_map_world();
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let s = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        add_kill_me(&mut w);
        // the engine's `newMap` (`startNewGame`'s, which the gate's fixture does not run): until then its update returns at once
        // (`m_endGameTimer >= 0`), so no sequential script progresses
        w.script_engine_new_map().unwrap();
        let chains = |w: &World| w.script_engine.sequential_scripts.iter().map(|c| (c[0].object_id, c[0].team_to_exec_on, c[0].frames_to_wait)).collect::<Vec<_>>();
        w.sa_append_sequential_script(None, s, b"KillMe", 0);
        let appended = chains(&w);
        w.sa_set_sequential_timer_obj(s, 7);
        let timed = chains(&w);
        w.sa_remove_all_sequential_scripts_obj(s);
        let removed = chains(&w);
        w.sa_append_sequential_script(Some(team as i32), 0, b"KillMe", 0);
        let team_chain = chains(&w).len();
        w.sa_remove_all_sequential_scripts_team(team as i32);
        w.sa_append_sequential_script(None, s, b"NoSuchScript", 0);
        let after_team = chains(&w);
        w.sa_append_sequential_script(None, s, b"KillMe", 0);
        let mut dead = None;
        for f in 0..3 {
            w.logic_update().unwrap();
            if w.find_object(s).is_none_or(|o| o.is_effectively_dead()) {
                dead = Some(f);
                break;
            }
        }
        eprintln!("{appended:?} {timed:?} {removed:?} team {team_chain} {after_team:?}; dead {dead:?}; errors {:?}", w.cuts.team_live_errors);
        assert_eq!(appended, vec![(s, None, -1)]);
        assert_eq!(timed, vec![(s, None, 7)]);
        assert!(removed.is_empty());
        assert_eq!(team_chain, 1);
        assert!(after_team.is_empty());
        assert!(dead.is_some());
    }
}

#[cfg(test)]
mod script_partition_tests {
    use super::*;
    use crate::common::ascii_string::AsciiString;
    use crate::common::base_type::ICoord3D;
    use crate::common::game_common::CellShroudStatus as S;
    use crate::logic::terrain_logic::TriggerArea;
    use crate::logic::world::script_actions::ScriptActionsHost;

    /// Batch I26, the script actions' partition, shroud and list calls live (each was a cut or a default). Two soldiers of team 1 at
    /// (100, 100) and (300, 100), a depot at (200, 300); trigger areas `All` (x 0 to 400) and `East` (x 200 to 400). Predicted before the
    /// run: player 2 (no objects) sees cell (0, 0) shrouded at first, fogged after `revealMapForPlayer` (a look added and removed),
    /// shrouded after `shroudMapForPlayer`, clear after `revealMapForPlayerPermanently`, fogged after its undo; the closest soldier to
    /// (90, 100) within `All` is the first, 10 away, and within `East` the second, 210 away; a soldier's structure candidates (not an
    /// internet centre) are [depot], its leader's garrisonable ones none (the depot has no garrison contain); `Soldier` maintained into
    /// a new list `MyList` is its one entry; player 1 (human, no AI) has no construction zone or superweapon target.
    #[test]
    fn the_script_partition_calls_reach_the_partition() {
        let mut w = gate_map_world();
        let area = |name: &[u8], id, x0, x1| {
            let corners = [(x0, 0), (x1, 0), (x1, 400), (x0, 400)].map(|(x, y)| ICoord3D { x, y, z: 0 });
            TriggerArea::new(AsciiString::try_from_bytes(name).expect("a name"), id, false, corners.to_vec()).expect("the area")
        };
        w.terrain.set_triggers(vec![area(b"All", 1, 0, 400), area(b"East", 2, 200, 400)]);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let a = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let b = make(&mut w, b"Soldier", team, Coord3D { x: 300.0, y: 100.0, z: 0.0 });
        let depot = make(&mut w, b"Depot", team, Coord3D { x: 200.0, y: 300.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let cell = |w: &World| w.partition.get_shroud_status_for_player_at_cell(2, 0, 0);
        let mut shroud = vec![cell(&w)];
        w.sa_partition_reveal_map(2);
        shroud.push(cell(&w));
        w.sa_partition_shroud_map(2);
        shroud.push(cell(&w));
        w.sa_partition_reveal_map_permanently(2, false);
        shroud.push(cell(&w));
        w.sa_partition_reveal_map_permanently(2, true);
        shroud.push(cell(&w));
        let soldier = World::thing_factory_find_template(&w, b"Soldier").expect("the template") as i32;
        let trig = |w: &World, n: &[u8]| w.terrain.trigger_area_by_name(&AsciiString::try_from_bytes(n).expect("a name")).expect("the area") as i32;
        let (all, east) = (trig(&w, b"All"), trig(&w, b"East"));
        let from = Coord3D { x: 90.0, y: 100.0, z: 0.0 };
        let closest = (w.sa_partition_closest_of_template(&from, soldier, all, a), w.sa_partition_closest_of_template(&from, soldier, east, a));
        let garrison = (w.sa_partition_unit_garrison_candidates(a, false), w.sa_partition_garrison_candidates(a, false));
        w.sa_object_type_list_maintenance(b"MyList", b"Soldier", true);
        let list = w.sa_object_types(b"MyList");
        let player = (w.sa_player_closest_construction_zone(1, soldier), w.sa_player_compute_superweapon_target(1, 1, 2, 0.0));
        eprintln!("{shroud:?}; {closest:?}; {garrison:?}; {list:?}; {player:?}");
        assert_eq!(shroud, vec![S::Shrouded, S::Fogged, S::Shrouded, S::Clear, S::Fogged]);
        assert_eq!(closest.0.map(|(o, d)| (o, d.round())), Some((a, 10.0)));
        assert_eq!(closest.1.map(|(o, d)| (o, d.round())), Some((b, 210.0)));
        assert_eq!(garrison, (vec![depot], Vec::new()));
        assert_eq!(list, Some(vec![b"Soldier".to_vec()]));
        assert_eq!(player, (None, None));
    }
}

#[cfg(test)]
mod button_search_tests {
    use super::*;
    use crate::logic::world::script_actions::{ButtonFrom, ButtonSearch, ScriptActionsHost};

    /// The batch-I23 caster, given a second power `ZapPower` (`SPECIAL_BATTLESHIP_BOMBARDMENT`: valid at any object not allied) with a
    /// button needing an enemy object, and a costly enemy `Prize`.
    fn fixture() -> String {
        format!(
            "{}SpecialPower ZapPower\n Enum = SPECIAL_BATTLESHIP_BOMBARDMENT\n ReloadTime = 0\nEnd\n\
CommandButton Command_Zap\n Command = SPECIAL_POWER\n SpecialPower = ZapPower\n Options = NEED_TARGET_ENEMY_OBJECT\nEnd\n\
Object Zapper\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = OCLSpecialPower ModuleTag_Zap\n  SpecialPowerTemplate = ZapPower\n  OCL = OCL_Drop\n  CreateLocation = CREATE_AT_LOCATION\n End\nEnd\n\
Object Prize\n KindOf = VEHICLE\n BuildCost = 900\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n",
            super::command_button_object_tests::CASTER
        )
    }

    /// Batch I27, `CommandButton::isValidToUseOn` and the button searches with their player, source and button. A zapper of team 1 at
    /// (100, 100), a friend at (120, 100), an enemy soldier at (160, 100) and an enemy prize (cost 900) at (300, 100). Predicted before
    /// the run: `Command_Zap` from the zapper is valid on the enemy soldier and not on the friend (the bombardment is refused on allies),
    /// nor with no target (it needs an object); `Command_Halt` (no special power) is valid on nothing; `Command_Drop` (`NEED_TARGET_POS`)
    /// from the caster is not valid with neither target nor position. The nearest valid enemy to (100, 100) is the soldier, 60 away (the
    /// friend is no enemy); the most valuable within 500 is the prize (sorted expensive to cheap); the unmanned search finds nothing.
    /// **Missed on the first run:** every answer was false or none, because `Command_Zap` had no special power: **port defect**, the
    /// INI loader made the buttons' name lookups at the first `CommandButton` block and kept them, so `ZapPower`, read after
    /// `Command_Drop`, was unknown to `Command_Zap` (upstream looks each name up as it parses); the lookups are now remade when the stores
    /// have grown. Then every prediction held.
    #[test]
    fn button_searches_find_valid_targets() {
        let mut w = gate_map_world_with(&fixture());
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let zapper = make(&mut w, b"Zapper", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let friend = make(&mut w, b"Soldier", team, Coord3D { x: 120.0, y: 100.0, z: 0.0 });
        let foe = make(&mut w, b"Soldier", enemy, Coord3D { x: 160.0, y: 100.0, z: 0.0 });
        let prize = make(&mut w, b"Prize", enemy, Coord3D { x: 300.0, y: 100.0, z: 0.0 });
        let caster = make(&mut w, b"Caster", team, Coord3D { x: 60.0, y: 60.0, z: 0.0 });
        w.pathfinder_op(PfOp::NewMap);
        let button = |w: &World, n: &[u8]| w.cuts.command_buttons.iter().position(|b| b.name == n).expect("the button") as i32;
        let (zap, halt, drop) = (button(&w, b"Command_Zap"), button(&w, b"Command_Halt"), button(&w, b"Command_Drop"));
        let valid = (
            w.sa_button_is_valid_to_use_on(zap, zapper, Some(foe), 1),
            w.sa_button_is_valid_to_use_on(zap, zapper, Some(friend), 1),
            w.sa_button_is_valid_to_use_on(zap, zapper, None, 1),
            w.sa_button_is_valid_to_use_on(halt, zapper, Some(foe), 1),
            w.sa_button_is_valid_to_use_on(drop, caster, None, 1),
        );
        let player = w.get_controlling_player(zapper).expect("the zapper's player");
        let from = ButtonFrom { player: Some(player), src: zapper, button: zap };
        let at = Coord3D { x: 100.0, y: 100.0, z: 0.0 };
        let nearest = w.sa_partition_closest_for_button(&at, ButtonSearch::Enemy, from).map(|(o, d)| (o, d.round()));
        let valuable = w.sa_partition_most_valuable_target(&at, 500.0, from);
        let unmanned = w.sa_partition_closest_for_button(&at, ButtonSearch::Unmanned, from);
        eprintln!("{valid:?}; nearest {nearest:?}; valuable {valuable:?}; unmanned {unmanned:?}; errors {:?}", w.cuts.team_live_errors);
        assert_eq!(valid, (true, false, false, false, false));
        assert_eq!(nearest, Some((foe, 60.0)));
        assert_eq!(valuable, Some(prize));
        assert_eq!(unmanned, None);
    }
}

#[cfg(test)]
mod dispatch_rest_tests {
    use super::*;
    use crate::logic::world::game_logic_dispatch::DispatchHost;
    use crate::logic::world::script_actions::ScriptActionsHost;

    /// Batch I28, the dispatch's waypoint queue, weapon lock and rally path, and the scripts' team prototype info and track name, live.
    /// Predicted before the run: a soldier with two queued waypoints is not executing its queue, and is after the dispatch's
    /// `executeWaypointQueue`; its weapon locked permanently is unlocked by `releaseWeaponLock(LOCKED_PERMANENTLY)`; the rally path from
    /// it to (200, 100) on the flat open map exists with a `BasicHumanLocomotor` of ground surfaces (the gate's INI has none: then no
    /// surface, and no path); team1's prototype, given one unit line (`Soldier`, at most 3), a transport `Truck`, the waypoint `Start`
    /// and starts-full, reads so (before this batch, a default with no units); the track name set by a script is the engine's.
    #[test]
    fn the_dispatch_and_script_rest_are_live() {
        let human = "Locomotor BasicHumanLocomotor\n Surfaces = GROUND\n Speed = 20\nEnd\n";
        let mut with = gate_map_world_with(human);
        let mut without = gate_map_world();
        let mut paths = Vec::new();
        for w in [&mut without, &mut with] {
            let team = factory_find_team(w, b"team1").expect("the team").expect("team1");
            let s = make(w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
            w.pathfinder_op(PfOp::NewMap);
            paths.push(w.d_rally_path_exists(s as u32, &Coord3D { x: 200.0, y: 100.0, z: 0.0 }));
        }
        let w = &mut with;
        let team = factory_find_team(w, b"team1").expect("the team").expect("team1");
        let s = make(w, b"Soldier", team, Coord3D { x: 60.0, y: 60.0, z: 0.0 });
        w.with_ai_lent(s, |w| crate::logic::ai::ai_states::with_ai(w, |a, _| {
            a.queue_waypoint(&Coord3D { x: 80.0, y: 60.0, z: 0.0 });
            a.queue_waypoint(&Coord3D { x: 80.0, y: 90.0, z: 0.0 });
        }));
        let executing = |w: &World| w.ai_peek(s).map(|a| a.executing_waypoint_queue);
        let before = executing(w);
        w.d_ai_execute_waypoint_queue(s as u32);
        let after = executing(w);
        crate::logic::world::object_weapons::set_weapon_lock(w, s, 0, crate::logic::weapon_set_state::LOCKED_PERMANENTLY);
        let locked = crate::logic::world::object_weapons::is_cur_weapon_locked(w, s);
        w.d_release_weapon_lock(s as u32, crate::logic::weapon_set_state::LOCKED_PERMANENTLY);
        let released = !crate::logic::world::object_weapons::is_cur_weapon_locked(w, s);
        let proto = w.teams.prototypes().find(|p| p.name == b"team1").map(|p| p.id).expect("team1's prototype");
        {
            let t = &mut w.teams.prototype_mut(proto).expect("the prototype").team_template;
            t.units_info[0].unit_thing_name = b"Soldier".to_vec();
            t.units_info[0].max_units = 3;
            t.num_units_info = 1;
            t.transport_unit_type = b"Truck".to_vec();
            t.start_reinforce_waypoint = b"Start".to_vec();
            t.team_starts_full = true;
        }
        let info = w.sa_proto_template_info(proto as i32);
        w.sa_set_current_track_name(b"Track_Battle");
        eprintln!("paths {paths:?}; waypoints {before:?} -> {after:?}; lock {locked} released {released}; {info:?}; track {:?}", w.script_engine.current_track_name);
        assert_eq!(paths, vec![false, true]);
        assert_eq!((before, after), (Some(false), Some(true)));
        assert!(locked && released);
        assert_eq!(info.units_info, vec![(b"Soldier".to_vec(), 3)]);
        assert_eq!((info.transport_unit_type, info.start_reinforce_waypoint, info.team_starts_full, info.transports_exit), (b"Truck".to_vec(), b"Start".to_vec(), true, false));
        assert_eq!(w.script_engine.current_track_name, b"Track_Battle");
    }
}

#[cfg(test)]
mod script_note_tests {
    use super::*;
    use crate::logic::world::script_actions::ScriptActionsHost;

    /// Batch I29, the logic's notifications reach the world's script engine (they were records only). Predicted before the run: a soldier
    /// named `Bob` in the engine's cache and destroyed is `didUnitExist` and no longer `getUnitNamed` after the next engine update
    /// (before, the cache kept the dead object); the engine's object count frame is the frame of the soldier's creation (frame 2, after
    /// two updates); a team's sequential chain is removed when the team is deleted (`notifyOfTeamDestruction`); a science added to player
    /// 1 is in the engine's acquired sciences of player 1. **Missed on the first run:** right after the frame in which `Bob` was
    /// destroyed, the cache still held him: the object is deleted at the end of that frame, after the engine's update, so its note is
    /// taken at the next engine update (the logic reads the cache only inside the engine's update, after that, so nothing it sees
    /// differs from upstream's immediate call; the test had read the field directly). Read after the next update, as below.
    ///
    /// **Batch I35, re-run with the notes given at once while the engine is in the world.** Predicted before the run: right after the
    /// frame in which `Bob` was destroyed the cache no longer holds him (`named_raw` is now `(None, true)`, not `(Some(bob), false)`):
    /// the I29 miss above was the queue's delay, and upstream's immediate call is what the world now does.
    #[test]
    fn the_logics_notifications_reach_the_engine() {
        let mut w = gate_map_world_with("Science SCIENCE_Test\nEnd\n");
        w.script_engine_new_map().unwrap();
        w.logic_update().unwrap();
        w.logic_update().unwrap();
        let made_on = w.frame();
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let bob = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        w.script_engine.named_objects.push((b"Bob".to_vec(), Some(bob)));
        let other = crate::logic::world::team_ops::factory_create_inactive_team(&mut w, b"team1").expect("a second team1 instance");
        w.script_engine.sequential_scripts.push(vec![crate::logic::world::script_engine::SequentialScript {
            team_to_exec_on: Some(other as i32),
            ..crate::logic::world::script_engine::SequentialScript::new(crate::logic::world::script_engine::ScriptRef { side: 0, group: None, index: 0 })
        }]);
        w.destroy_object(bob);
        w.logic_update().unwrap();
        let count_frame = w.script_engine.frame_object_count_changed;
        let named_raw = (w.script_engine.get_unit_named(b"Bob"), w.script_engine.did_unit_exist(b"Bob"));
        crate::logic::world::team_ops::delete_team_instance(&mut w, other).expect("the team deleted");
        let sci = { let s = w.statics.clone(); s.science_store.get_science_from_internal_name(&mut w.names, b"SCIENCE_Test").expect("the science") };
        crate::logic::world::player_ops::player_add_science(&mut w, 1, sci).expect("the science added");
        w.logic_update().unwrap();
        let named = (w.script_engine.get_unit_named(b"Bob"), w.script_engine.did_unit_exist(b"Bob"));
        let chains = w.script_engine.sequential_scripts.len();
        let acquired = w.script_engine.acquired_sciences[1].clone();
        eprintln!("made on {made_on}, count frame {count_frame}; named {named_raw:?} then {named:?}; chains {chains}; acquired {acquired:?} (science {})", sci.0);
        assert_eq!(count_frame, made_on);
        assert_eq!(named_raw, (None, true));
        assert_eq!(named, (None, true));
        assert_eq!(chains, 0);
        assert_eq!(acquired, vec![sci.0]);
        let _ = w.sa_unit_named(b"Bob");
    }
}

#[cfg(test)]
mod script_power_note_tests {
    use super::*;
    use crate::logic::modules::create_host::{CreateHost, UpgradeRef};
    use crate::logic::world::object_cuts::CutCall;

    /// Batch I35, the special powers', the upgrades' and the name transfers' notifications reach the world's engine (they were records
    /// only, so `TheScriptEngine`'s lists the special power and upgrade conditions read stayed empty in a real game), and
    /// `GrantUpgradeCreate`'s `player->addUpgrade` is the player's (it set a table the live player never read). Predicted before the run:
    /// with the engine in the world, a triggered power, a completed power and a completed upgrade of player 1 are in the engine's lists
    /// at once (each with its name and source); with the engine out of the world (as during its update), a triggered power is queued and
    /// the engine's list is unchanged until it is put back and updated, after which the list has it; a name transfer from `Alpha` (in the
    /// cache) to a second soldier names the second soldier `Alpha` and points the cache at it; an upgrade granted to player 1 through the
    /// create host is complete in the live player's list.
    #[test]
    fn power_and_name_notes_reach_the_engine() {
        let mut w = gate_map_world_with("Upgrade Upgrade_I35\n  Type = PLAYER\nEnd\n");
        w.script_engine_new_map().unwrap();
        w.logic_update().unwrap();
        w.cut(CutCall::ScriptNotifyOfTriggeredSpecialPower { player: 1, name: b"PowerT".to_vec(), source: 7 });
        w.cut(CutCall::ScriptNotifyOfCompletedSpecialPower { player: 1, name: b"PowerC".to_vec(), creator: 8 });
        w.cut(CutCall::ScriptNotifyOfCompletedUpgrade { player: 1, name: b"UpgradeU".to_vec(), source: 9 });
        let at_once = (
            w.script_engine.triggered_special_powers[1].clone(),
            w.script_engine.finished_special_powers[1].clone(),
            w.script_engine.completed_upgrades[1].clone(),
        );
        // the engine out of the world
        let mut se = std::mem::take(&mut w.script_engine);
        w.cut(CutCall::ScriptNotifyOfTriggeredSpecialPower { player: 1, name: b"PowerLate".to_vec(), source: 10 });
        let queued = w.cuts.pending_script_notes.len();
        let unchanged = se.triggered_special_powers[1].len();
        std::mem::swap(&mut w.script_engine, &mut se);
        w.script_engine_update().unwrap();
        let after_update = w.script_engine.triggered_special_powers[1].clone();
        // a name transfer
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let alpha = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let second = make(&mut w, b"Soldier", team, Coord3D { x: 120.0, y: 100.0, z: 0.0 });
        w.find_object_mut(alpha).unwrap().name = b"Alpha".to_vec();
        w.script_engine.named_objects.push((b"Alpha".to_vec(), Some(alpha)));
        w.cut(CutCall::ScriptEngineTransferObjectName { name: b"Alpha".to_vec(), object: second });
        let transferred = (w.find_object(second).unwrap().name.clone(), w.script_engine.get_unit_named(b"Alpha"));
        // an upgrade granted to the player
        let statics = w.statics.clone();
        let t = statics.upgrade_center.find_upgrade(&mut w.names, b"Upgrade_I35").unwrap().expect("the upgrade");
        let (key, mask) = (t.upgrade_name_key().0, t.upgrade_mask());
        w.player_add_upgrade_complete(1, UpgradeRef { id: key, player_type: true });
        let granted = w.player_has_upgrade_complete(1, &mask);
        eprintln!("at once {at_once:?}; queued {queued}, unchanged {unchanged}, after update {after_update:?}; transferred {transferred:?}; granted {granted}");
        assert_eq!(at_once.0, vec![(b"PowerT".to_vec(), 7)]);
        assert_eq!(at_once.1, vec![(b"PowerC".to_vec(), 8)]);
        assert_eq!(at_once.2, vec![(b"UpgradeU".to_vec(), 9)]);
        assert_eq!((queued, unchanged), (1, 1));
        assert_eq!(after_update, vec![(b"PowerT".to_vec(), 7), (b"PowerLate".to_vec(), 10)]);
        assert_eq!(transferred, (b"Alpha".to_vec(), Some(second)));
        assert!(granted);
    }
}

#[cfg(test)]
mod attack_search_tests {
    use super::*;
    use crate::logic::ai::ai_states::AiStateHost;

    /// Batch I38, the attack path asks the attacker's weapon (`findAttackPath(.., weapon)`: `isGoalPosWithinAttackRange` and
    /// `getAttackDistance` were tables, empty in a game, and `isIdle` of the units in the way the `ai_idle` table). A soldier (GateGun,
    /// range 100) at (100, 105) and an enemy soldier at (400, 105), on the fixture's map after `newMap`. Predicted before the run: a
    /// path is found; its goal is within the weapon's attack range of the enemy (`isGoalPosWithinAttackRange`, the test the AI makes
    /// after the search); the goal is short of the enemy: between 50 and 100 from it (the range undersized by a quarter cell, the
    /// cells 10 wide); no search's facts remain after it. **Missed on the first run, in the prediction's measure:** the goal was in
    /// range, at (295, 105), but 105 from the enemy's centre: the weapon's distance is between the two bounding circles (radius 5
    /// each), 95, inside the 97.5 of the undersized range. The distance is checked between the circles below.
    #[test]
    fn an_attack_path_ends_in_the_weapons_range() {
        let mut w = gate_map_world();
        assert!(w.pathfinder_op(PfOp::NewMap));
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy_team = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let me = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let enemy = make(&mut w, b"Soldier", enemy_team, Coord3D { x: 400.0, y: 105.0, z: 0.0 });
        let enemy_pos = w.objects.find(enemy).unwrap().get_position();
        w.ai_self = me;
        let path = w.pf_find_attack_path(enemy, &enemy_pos).expect("an attack path");
        let goal = *path.node(path.last_node().expect("a node")).position();
        let weapon = w.objects.find(me).unwrap().weapon_set.get_cur_weapon().cloned().expect("the soldier's gun");
        let in_range = weapon.is_goal_pos_within_attack_range(&mut w, me, &goal, Some(enemy), &enemy_pos);
        let dist = ((goal.x - enemy_pos.x).powi(2) + (goal.y - enemy_pos.y).powi(2)).sqrt() - 10.0;
        eprintln!("goal {goal:?}, {dist} from the enemy's circle, in range {in_range}; facts left {:?}", w.cuts.pathfinder.attack_search.is_some());
        assert!(in_range);
        assert!((50.0..=97.5).contains(&dist), "the goal is {dist} from the enemy's circle");
        assert!(w.cuts.pathfinder.attack_search.is_none());
    }

    /// Batch I38, `Weapon::computeApproachTarget`'s `adjustTargetDestination(source, target, pos, this, &approach)` (an aircraft's
    /// approach "not stacked atop another aircraft") on the world's pathfinder; it was a record that left the approach as it was.
    /// Predicted before the run: an approach put at (330, 103), about 70 from the enemy at (400, 105), moves to a cell's centre (both
    /// coordinates 5 modulo 10, `adjustCoordToCell`) that the weapon's `isGoalPosWithinAttackRange` accepts, and no record is made.
    #[test]
    fn an_approach_is_adjusted_to_a_cell_in_range() {
        use crate::logic::weapon_host::WeaponHost;
        let mut w = gate_map_world();
        assert!(w.pathfinder_op(PfOp::NewMap));
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy_team = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let me = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let enemy = make(&mut w, b"Soldier", enemy_team, Coord3D { x: 400.0, y: 105.0, z: 0.0 });
        let enemy_pos = w.objects.find(enemy).unwrap().get_position();
        let weapon = w.objects.find(me).unwrap().weapon_set.get_cur_weapon().cloned().expect("the soldier's gun");
        w.cuts.log = Some(Vec::new());
        let mut approach = Coord3D { x: 330.0, y: 103.0, z: 0.0 };
        w.adjust_target_destination(me, Some(enemy), None, weapon.template(), &mut approach);
        let in_range = weapon.is_goal_pos_within_attack_range(&mut w, me, &approach, Some(enemy), &enemy_pos);
        let records = w.cuts.log.as_ref().unwrap().iter().filter(|c| matches!(c, crate::logic::world::object_cuts::CutCall::PathfinderAdjustTargetDestination { .. })).count();
        eprintln!("approach {approach:?}, in range {in_range}, records {records}");
        assert_eq!((approach.x.rem_euclid(10.0), approach.y.rem_euclid(10.0)), (5.0, 5.0));
        assert!(in_range);
        assert_eq!(records, 0);
    }
}

#[cfg(test)]
mod footprint_wall_tests {
    use super::*;
    use crate::common::game_type::LAYER_GROUND;
    use crate::logic::ai::cell::{CELL_CLEAR, CELL_OBSTACLE};
    use crate::logic::world::object_cuts::CutCall;

    /// Batch I37, `createAWallFromMyFootprint` and `removeWallFromMyFootprint` live (the tensile formation's and the railroad's "temporarily
    /// treat this object as an obstacle"; they were records only). Predicted before the run: the ground cell under a soldier at
    /// (205, 205) is clear (a mobile unit is no obstacle), an obstacle after the wall is made from its footprint, and clear again after
    /// the wall is removed; no record of either call (the world's pathfinder answered). **Missed on the first run:** every cell `None`:
    /// the fixture's pathfinder has had no `newMap`, so its extent is empty (the test's, not the wall's); `newMap` is run first, as the
    /// other pathfinder tests do.
    #[test]
    fn an_objects_footprint_becomes_a_wall_and_back() {
        let mut w = gate_map_world();
        assert!(w.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::NewMap));
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let soldier = make(&mut w, b"Soldier", team, Coord3D { x: 205.0, y: 205.0, z: 0.0 });
        let cell = |w: &World| {
            let pf = w.pathfinder.as_ref().expect("the world's pathfinder");
            pf.get_cell(LAYER_GROUND, 20, 20).map(|c| pf.store.cells[c as usize].get_type())
        };
        w.cuts.log = Some(Vec::new());
        let before = cell(&w);
        w.pathfinder_create_wall_from_footprint(soldier);
        let walled = cell(&w);
        w.pathfinder_remove_wall_from_footprint(soldier);
        let after = cell(&w);
        let records = w.cuts.log.as_ref().unwrap().iter().filter(|c| matches!(c, CutCall::PathfinderCreateWallFromFootprint { .. } | CutCall::PathfinderRemoveWallFromFootprint { .. })).count();
        eprintln!("cell (20, 20): {before:?}, walled {walled:?}, after {after:?}; records {records}");
        assert_eq!((before, walled, after), (Some(CELL_CLEAR), Some(CELL_OBSTACLE), Some(CELL_CLEAR)));
        assert_eq!(records, 0);
    }
}

#[cfg(test)]
mod energy_tests {
    use super::*;

    /// A power plant making 5 and a consumer using 3, structures with a body and nothing else.
    const GRID: &str = "\
Object Plant\n KindOf = STRUCTURE IMMOBILE\n EnergyProduction = 5\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n\
Object Consumer\n KindOf = STRUCTURE IMMOBILE\n EnergyProduction = -3\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n";

    /// Batch I36, the players' power grid live (`Energy::objectEnteringInfluence`, `objectLeavingInfluence` and `adjustPower` were
    /// records only, so every player's production and consumption stayed 0). Predicted before the run: a plant made on team1's player
    /// gives that player production 5, consumption 0; a consumer then gives consumption 3, with sufficient power; the plant disabled
    /// (EMP) takes its 5 away (`adjustPower(5, false)`), production 0, so the player is browned out; the plant enabled again gives 5
    /// back; the consumer destroyed (and deleted at the end of the frame) takes its 3 away, consumption 0.
    #[test]
    fn a_players_power_grid_counts_its_plants_and_consumers() {
        let mut w = gate_map_world_with(GRID);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let plant = make(&mut w, b"Plant", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let p = w.get_controlling_player(plant).expect("the plant's player");
        let grid = |w: &World| {
            let f = w.frame();
            let e = &w.players.get_nth(p).unwrap().energy;
            (e.production(f), e.consumption(), e.has_sufficient_power(f))
        };
        let with_plant = grid(&w);
        let consumer = make(&mut w, b"Consumer", team, Coord3D { x: 200.0, y: 100.0, z: 0.0 });
        let with_consumer = grid(&w);
        w.set_disabled(plant, crate::common::disabled_types::DISABLED_EMP);
        let disabled = grid(&w);
        w.clear_disabled(plant, crate::common::disabled_types::DISABLED_EMP);
        let enabled = grid(&w);
        w.destroy_object(consumer);
        w.logic_update().unwrap();
        let destroyed = grid(&w);
        eprintln!("player {p}: plant {with_plant:?}, consumer {with_consumer:?}, disabled {disabled:?}, enabled {enabled:?}, consumer destroyed {destroyed:?}; errors {:?}", w.cuts.team_live_errors);
        assert_eq!(with_plant, (5, 0, true));
        assert_eq!(with_consumer, (5, 3, true));
        assert_eq!(disabled, (0, 3, false));
        assert_eq!(enabled, (5, 3, true));
        assert_eq!(destroyed, (5, 0, true));
    }
}

#[cfg(test)]
mod team_script_tests {
    use super::*;
    use crate::logic::scripts::{action_type as at, condition_type as ct, Condition, OrCondition, Parameter, Script, ScriptAction, ScriptList};
    use crate::logic::world::team_ops::TeamHost;

    /// A script `name` whose one action sets the flag `flag` (and whose condition is `CONDITION_TRUE`), for every difficulty.
    fn flag_script(name: &[u8], flag: &[u8], subroutine: bool) -> Script {
        let mut a = ScriptAction::new_for_read();
        a.action_type = at::SET_FLAG;
        a.num_parms = 2;
        let mut p = Parameter::new(0, 0);
        p.string = flag.to_vec();
        a.parms[0] = Some(p);
        a.parms[1] = Some(Parameter::new(0, 1));
        let mut c = Condition::new_for_read();
        c.condition_type = ct::CONDITION_TRUE;
        let mut s = Script::new();
        s.script_name = name.to_vec();
        s.is_active = true;
        s.is_subroutine = subroutine;
        s.easy = true;
        s.normal = true;
        s.hard = true;
        s.conditions = vec![OrCondition { conditions: vec![c] }];
        s.actions = vec![a];
        s
    }

    /// Batch I30, the team code's scripts through the world's engine (they were a table, empty in a real game, so every prototype's
    /// production condition was "always false" and no team script ran). Predicted before the run: team1's prototype, whose production
    /// condition is `BuildIt` (a true condition), evaluates true, keeping a copy the world owns; that copy's actions run for the team
    /// set the flag `Built`; team2's, naming `Nope` (no such script), evaluates false and is marked always false; `runScript("OnCreate",
    /// team)` runs the subroutine, setting `Created`.
    #[test]
    fn a_prototypes_production_condition_is_the_engines() {
        let mut w = gate_map_world();
        if w.sides.get_num_sides() == 0 {
            w.sides.add_side(&Dict::default());
        }
        w.sides.get_side_info_mut(0).expect("side 0").scripts =
            Some(ScriptList { groups: Vec::new(), scripts: vec![flag_script(b"BuildIt", b"Built", false), flag_script(b"OnCreate", b"Created", true)] });
        w.script_engine_new_map().unwrap();
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let proto = |w: &World, n: &[u8]| w.teams.prototypes().find(|p| p.name == n).map(|p| p.id).expect("the prototype");
        let (p1, p2) = (proto(&w, b"team1"), proto(&w, b"team2"));
        w.teams.prototype_mut(p1).expect("p1").team_template.production_condition = b"BuildIt".to_vec();
        w.teams.prototype_mut(p2).expect("p2").team_template.production_condition = b"Nope".to_vec();
        let cond = (
            crate::logic::world::team_ops::proto_evaluate_production_condition(&mut w, p1).expect("evaluated"),
            crate::logic::world::team_ops::proto_evaluate_production_condition(&mut w, p2).expect("evaluated"),
        );
        let kept = w.teams.prototype(p1).and_then(|p| p.production_condition_script);
        let owned = kept.and_then(|h| w.team_scripts.refs.get(h as usize - 1)).map(|r| r.side == crate::logic::world::script_engine::OWNED_SIDE);
        if let Some(h) = kept {
            w.script_execute_action(h, Some(team));
        }
        w.script_run(b"OnCreate", team);
        let flag = |w: &World, n: &[u8]| w.script_engine.flags.iter().find(|f| f.name == n).map(|f| f.value);
        let flags = (flag(&w, b"Built"), flag(&w, b"Created"));
        let always_false = w.teams.prototype(p2).map(|p| p.production_condition_always_false);
        eprintln!("{cond:?}; kept {kept:?} owned {owned:?}; flags {flags:?}; p2 always false {always_false:?}; errors {:?}", w.cuts.team_live_errors);
        assert_eq!(cond, (true, false));
        assert_eq!(owned, Some(true));
        assert_eq!(flags, (Some(true), Some(true)));
        assert_eq!(always_false, Some(true));
    }
}

#[cfg(test)]
mod player_rest_tests {
    use super::*;
    use crate::common::game_common::{GameDifficulty, PlayerType};
    use crate::logic::world::player_ops::{player_set_player_type, PlayerHost};
    use crate::logic::world::script_engine::ScriptEngineHost;

    /// Batch I31, three more of the logic's calls live. Predicted before the run: player 2 made a computer player has an AI player of
    /// the game's difficulty (normal), and after `setAIDifficulty(HARD)` reads hard (before, the call set a table the live AI never
    /// read); `MSG_ENABLE_RETALIATION_MODE` posted for player 1 turns its logical retaliation mode on at the next frame (before, only a
    /// record); the engine's `doEnableOrDisableObjectDifficultyBonuses(false)` turns two soldiers' difficulty bonus off. Then (found by
    /// the save and load test, see the batch's docs) a soldier of the hard computer player made outside a single-player game has no
    /// `SOLO_AI_HARD` weapon bonus condition, and one made in a single-player game has it. **Missed on the first run of that part:**
    /// both had none, because the test had just turned the engine's bonus flag off (above), which a new object reads; it is turned on
    /// again first.
    #[test]
    fn the_players_remaining_calls_are_live() {
        let mut w = gate_world();
        player_set_player_type(&mut w, 2, PlayerType::Computer, false).expect("Player2 a computer player");
        let before = w.ai_difficulty(2);
        crate::logic::world::player_init::PlayerInitHost::ai_set_difficulty(&mut w, 2, GameDifficulty::Hard);
        let after = w.ai_difficulty(2);
        let retaliation = w.players.get_nth(1).map(|p| p.logical_retaliation_mode_enabled);
        w.message_enable_retaliation_mode(1, true);
        w.logic_update().unwrap();
        let retaliating = w.players.get_nth(1).map(|p| p.logical_retaliation_mode_enabled);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let a = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let b = make(&mut w, b"Soldier", team, Coord3D { x: 120.0, y: 100.0, z: 0.0 });
        let bonus = |w: &World| [a, b].map(|o| w.find_object(o).map(|x| x.is_receiving_difficulty_bonus));
        w.set_receiving_difficulty_bonus(a, true);
        w.set_receiving_difficulty_bonus(b, true);
        let on = bonus(&w);
        w.se_actions_enable_difficulty_bonuses(false);
        let off = bonus(&w);
        eprintln!("difficulty {before:?} -> {after:?}; retaliation {retaliation:?} -> {retaliating:?}; bonus {on:?} -> {off:?}");
        assert_eq!((before, after), (GameDifficulty::Normal, GameDifficulty::Hard));
        assert_eq!((retaliation, retaliating), (Some(false), Some(true)));
        assert_eq!(on, [Some(true); 2]);
        assert_eq!(off, [Some(false); 2]);
        // the single-player difficulty bonus (the game mode the world's; it was a table flag, never set): a hard computer player's new
        // soldier in a single-player game gets the `SOLO_AI_HARD` weapon bonus condition; none outside one
        // (the bonuses back on: the action above turned the engine's flag off, and a new object takes it from the engine; the first
        // run of this part missed that and saw no bonus inside either)
        w.se_actions_enable_difficulty_bonuses(true);
        let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let outside = make(&mut w, b"Soldier", enemy, Coord3D { x: 200.0, y: 100.0, z: 0.0 });
        w.cuts.game_loop.game_mode = crate::common::recorder::game_mode::SINGLE_PLAYER;
        let inside = make(&mut w, b"Soldier", enemy, Coord3D { x: 220.0, y: 100.0, z: 0.0 });
        let solo = |w: &World, o| w.find_object(o).map(|x| x.weapon_bonus_condition & (1 << crate::logic::weapon_bonus::condition::SOLO_AI_HARD) != 0);
        let (o, i) = (solo(&w, outside), solo(&w, inside));
        eprintln!("solo AI hard: outside {o:?}, inside {i:?}");
        assert_eq!((o, i), (Some(false), Some(true)));
    }
}

#[cfg(test)]
mod map_load_tests {
    use super::*;
    use crate::formats::data_chunk_output::DataChunkOutput;

    /// A map file of the logic's chunks: a 20 by 20 height map (height 2 everywhere but 10 at sample (5, 5)), two objects (a
    /// `Soldier` at (100, 120) and a waypoint `Rally`, id 1, at (200, 200)), a polygon trigger `Zone` (id 3) and two sides (the
    /// neutral one and `ThePlayer`).
    fn map_file(w: &mut World) -> Vec<u8> {
        let mut out = DataChunkOutput::new();
        out.open_data_chunk(b"HeightMapData", crate::logic::terrain_visual::K_HEIGHT_MAP_VERSION_4);
        out.write_int(20);
        out.write_int(20);
        out.write_int(0);
        out.write_int(1);
        out.write_int(20);
        out.write_int(20);
        out.write_int(400);
        let mut data = vec![2u8; 400];
        data[5 * 20 + 5] = 10;
        out.write_array_of_bytes(&data);
        out.close_data_chunk();
        out.open_data_chunk(b"ObjectsList", crate::logic::map_object::K_OBJECTS_VERSION_3);
        let mut soldier = Dict::new();
        soldier.set_ascii_string(w.names.name_to_key(b"originalOwner"), b"team1").unwrap();
        let mut rally = Dict::new();
        rally.set_int(w.names.name_to_key(b"waypointID"), 1).unwrap();
        rally.set_ascii_string(w.names.name_to_key(b"waypointName"), b"Rally").unwrap();
        for (x, y, flags, name, d) in [(100.0, 120.0, 1, &b"Soldier"[..], &soldier), (200.0, 200.0, 0, &b"*Waypoints/Waypoint"[..], &rally)] {
            out.open_data_chunk(b"Object", crate::logic::map_object::K_OBJECTS_VERSION_3);
            out.write_real(x);
            out.write_real(y);
            out.write_real(0.0);
            out.write_real(0.0);
            out.write_int(flags);
            out.write_ascii_string(name);
            out.write_dict(d, &w.names);
            out.close_data_chunk();
        }
        out.close_data_chunk();
        out.open_data_chunk(b"PolygonTriggers", crate::logic::polygon_trigger::K_TRIGGERS_VERSION_4);
        out.write_int(1);
        out.write_ascii_string(b"Zone");
        out.write_ascii_string(b"");
        out.write_int(3);
        out.write_byte(0);
        out.write_byte(0);
        out.write_int(0);
        out.write_int(4);
        for (x, y) in [(0, 0), (100, 0), (100, 100), (0, 100)] {
            out.write_int(x);
            out.write_int(y);
            out.write_int(0);
        }
        out.close_data_chunk();
        let templates = crate::logic::script_templates::script_templates(&mut w.names);
        let mut sc = crate::logic::sides_list::SidesCtx::new(w.names.clone(), templates);
        for name in [&b""[..], b"ThePlayer"] {
            let mut d = Dict::new();
            d.set_ascii_string(sc.names.generator.name_to_key(b"playerName"), name).unwrap();
            sc.sides.add_side(&d);
        }
        crate::logic::sides_list::SidesList::write_sides_data_chunk(&mut out, &mut sc).expect("the sides written");
        out.finish()
    }

    /// Batch I32, a map file read into a world (`TheTerrainLogic->loadMap` was a cut: whoever made the world put the map in it).
    /// Predicted before the run: the terrain's extent is the file's 20 by 20 samples and its ground 2 high (in height units, times
    /// `MAP_HEIGHT_SCALE`) away from sample (5, 5) and 10 there; the waypoint `Rally` is the terrain's, at (200, 200); the trigger
    /// `Zone` (id 3) holds (50, 50) and not (150, 50); the new game's map objects are the two, in file order, the soldier with its
    /// template, its owner `team1` and its mirror flag, the waypoint with no template; the sides are the two; a file that is not a
    /// map (the gate's INI text) changes nothing and answers false.
    #[test]
    fn a_map_file_is_read_into_the_world() {
        let mut w = gate_map_world();
        let file = map_file(&mut w);
        w.save_side.disk.insert(b"maps\\test\\test.map".to_vec(), file);
        let not_a_map = w.load_map_file(b"Maps\\Gate\\Gate.map");
        let read = w.load_map_file(b"Maps\\Test\\Test.map");
        let scale = crate::logic::terrain_visual::MAP_HEIGHT_SCALE;
        let ground = |w: &World, x: f32, y: f32| w.terrain.ground_height(x, y, false).0;
        let heights = (ground(&w, 15.0 * 10.0, 15.0 * 10.0), ground(&w, 5.0 * 10.0, 5.0 * 10.0));
        let rally = w.terrain.waypoint_by_name(&crate::common::ascii_string::AsciiString::try_from_bytes(b"Rally").expect("a name")).and_then(|h| w.terrain.waypoint(h).map(|p| *p.location()));
        let zone = w.terrain.triggers().iter().position(|t| t.name().str() == b"Zone");
        let inside = zone.map(|z| (w.terrain.triggers()[z].point_in_trigger(&crate::common::base_type::ICoord3D { x: 50, y: 50, z: 0 }), w.terrain.triggers()[z].point_in_trigger(&crate::common::base_type::ICoord3D { x: 150, y: 50, z: 0 })));
        let soldier = World::thing_factory_find_template(&w, b"Soldier").map(u32::from);
        let objects: Vec<_> = w.cuts.new_game.map_objects.iter().map(|(m, _)| (m.name.clone(), m.template, m.original_owner.clone(), m.draws_in_mirror)).collect();
        let sides: Vec<Vec<u8>> = (0..w.sides.get_num_sides()).map(|i| {
            let d = &w.sides.get_side_info(i).expect("a side").dict;
            d.get_ascii_string(w.names.name_to_key(b"playerName")).0
        }).collect();
        eprintln!("not a map {not_a_map}, read {read}; heights {heights:?} (scale {scale}); rally {rally:?}; zone {zone:?} {inside:?}; objects {objects:?}; sides {sides:?}; errors {:?}", w.cuts.team_live_errors);
        assert!(!not_a_map && read);
        assert_eq!(heights, (2.0 * scale, 10.0 * scale));
        assert_eq!(rally.map(|p| (p.x, p.y)), Some((200.0, 200.0)));
        assert_eq!(inside, Some((true, false)));
        assert_eq!(objects, vec![(b"Soldier".to_vec(), soldier, b"team1".to_vec(), true), (b"*Waypoints/Waypoint".to_vec(), None, Vec::new(), false)]);
        assert_eq!(sides, vec![Vec::new(), b"ThePlayer".to_vec()]);
    }
}

#[cfg(test)]
mod map_ini_tests {
    use super::*;

    /// Batch I33, a map's `map.ini` read over the world's stores as overrides (`loadMapINI`'s `INI::load(.., INI_LOAD_CREATE_OVERRIDES)`
    /// was a cut, and its `doesFileExist` a table). Predicted before the run: with `Maps\Test\map.ini` (`Object Soldier`, vision 300)
    /// on the world's disk, `loadMapINI("Maps\Test\Test.map")` finds and reads it; a soldier made before it sees 150 (the gate's
    /// INI) and one made after sees 300 (the template's override); a map with no `map.ini` loads nothing and changes nothing.
    #[test]
    fn a_maps_ini_overrides_the_games() {
        let mut w = gate_map_world();
        w.cuts.new_game.has_map_cache = true;
        w.save_side.disk.insert(b"maps\\test\\map.ini".to_vec(), b"Object Soldier\n VisionRange = 300\nEnd\n".to_vec());
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let before = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        crate::logic::world::game_logic_new_game::load_map_ini(b"Maps\\Other\\Other.map", &mut w).expect("no map.ini there");
        let unchanged = make(&mut w, b"Soldier", team, Coord3D { x: 110.0, y: 100.0, z: 0.0 });
        crate::logic::world::game_logic_new_game::load_map_ini(b"Maps\\Test\\Test.map", &mut w).expect("the map's ini");
        let after = make(&mut w, b"Soldier", team, Coord3D { x: 120.0, y: 100.0, z: 0.0 });
        let vision = [before, unchanged, after].map(|o| w.get_vision_range(o));
        eprintln!("vision {vision:?}; errors {:?}", w.cuts.team_live_errors);
        assert_eq!(vision, [150.0, 150.0, 300.0]);
    }
}

#[cfg(test)]
mod new_game_sides_tests {
    use super::*;
    use crate::formats::data_chunk_output::DataChunkOutput;
    use crate::logic::scripts::{Script, ScriptList};
    use crate::logic::world::game_logic_new_game::NewGameHost;

    /// Batch I33, the new game's sides: `SidesList::prepareForMP_or_Skirmish` and the multiplayer scripts (a cut and a table). Two
    /// sides (the neutral one and `ThePlayer`, neither `FactionCivilian`, each with an empty script list); a `MultiplayerScripts.scb`
    /// of one list holding `MP_A` then `MP_B` on the world's disk. Predicted before the run: the preparation copies both sides to the
    /// skirmish sides and leaves the real list with the last one, `ThePlayer` (the neutral side removed; the last side is never
    /// removed); the multiplayer scripts are read (`Some(true)`) and side 0's list is `MP_B`, `MP_A` (each added at index 0); without
    /// the file the table's answer stands (none).
    #[test]
    fn a_new_games_sides_are_prepared() {
        let mut w = gate_map_world();
        let without = w.sn_add_multiplayer_scripts();
        for name in [&b""[..], b"ThePlayer"] {
            let mut d = Dict::new();
            d.set_ascii_string(w.names.name_to_key(b"playerName"), name).unwrap();
            w.sides.add_side(&d);
            let n = w.sides.get_num_sides() - 1;
            w.sides.get_side_info_mut(n).expect("the side").scripts = Some(ScriptList::default());
        }
        let templates = crate::logic::script_templates::script_templates(&mut w.names);
        let mut list = ScriptList::default();
        for n in [&b"MP_A"[..], b"MP_B"] {
            let mut s = Script::new();
            s.script_name = n.to_vec();
            list.scripts.push(s);
        }
        let mut out = DataChunkOutput::new();
        ScriptList::write_scripts_data_chunk(&mut out, &mut [Some(&mut list)], &templates, &w.names).expect("the scripts written");
        w.save_side.disk.insert(b"data\\scripts\\multiplayerscripts.scb".to_vec(), out.finish());
        w.sn_sides_prepare_for_mp();
        let pn = w.names.name_to_key(b"playerName");
        let real: Vec<Vec<u8>> = (0..w.sides.get_num_sides()).map(|i| w.sides.get_side_info(i).expect("a side").dict.get_ascii_string(pn).0).collect();
        let skirmish = w.sides.get_num_skirmish_sides();
        let read = w.sn_add_multiplayer_scripts();
        let side0: Vec<Vec<u8>> = w.sides.get_side_info(0).ok().and_then(|s| s.scripts.as_ref()).map(|l| l.scripts.iter().map(|s| s.script_name.clone()).collect()).unwrap_or_default();
        eprintln!("without {without:?}; real {real:?}, skirmish {skirmish}; read {read:?}; side 0 {side0:?}; errors {:?}", w.cuts.team_live_errors);
        assert_eq!(without, None);
        assert_eq!((real, skirmish), (vec![b"ThePlayer".to_vec()], 2));
        assert_eq!(read, Some(true));
        assert_eq!(side0, vec![b"MP_B".to_vec(), b"MP_A".to_vec()]);
    }
}

#[cfg(test)]
mod ai_record_tests {
    use super::*;
    use crate::logic::ai::ai_update::LocoGoalType;
    use crate::logic::ai::ai_update_types::{AI_AGGRESSIVE, AI_PASSIVE};
    use crate::logic::weapon_types::WeaponStatus;
    use crate::logic::world::module_call::ModuleReply;
    use crate::logic::world::object_cuts::CutCall;

    fn attitude_and_set(w: &World, o: ObjectId) -> (i32, Option<usize>) {
        let a = w.ai_peek(o).expect("the AI");
        (a.attitude, a.attack_info)
    }

    /// Batch I39, `Object`'s constructor and `setTeam` tell the AI its team's attitude and attack priority set (Object.cpp:465, :888;
    /// they were records only). Team 1's prototype aggressive with the set "Tanks First" (the script engine's index 1), team 2's
    /// passive with a set the engine does not have. Predicted before the run: a soldier made on team 1 is (aggressive, Some(1)); moved
    /// to team 2 it is (passive, Some(1)): `getAttackInfo` of an unknown name is the default, whose name is empty, so the set is not
    /// changed; no record of either call.
    #[test]
    fn a_unit_takes_its_teams_attitude_and_attack_set() {
        let mut w = gate_map_world();
        assert!(!w.script_engine.is_stand_in());
        let team1 = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let team2 = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        w.script_engine.num_attack_info = 2;
        w.script_engine.attack_priority_info[1].name = b"Tanks First".to_vec();
        for (team, attitude, set) in [(team1, AI_AGGRESSIVE, &b"Tanks First"[..]), (team2, AI_PASSIVE, &b"No Such Set"[..])] {
            let proto = w.teams.team(team).expect("the team").proto;
            let p = w.teams.prototype_mut(proto).expect("the prototype");
            p.team_template.initial_team_attitude = attitude;
            p.attack_priority_name = set.to_vec();
        }
        w.cuts.log = Some(Vec::new());
        let me = make(&mut w, b"Soldier", team1, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let made = attitude_and_set(&w, me);
        w.set_team(me, Some(team2));
        let moved = attitude_and_set(&w, me);
        let records = w.cuts.log.as_ref().unwrap().iter().filter(|c| matches!(c, CutCall::AiSetAttitude { .. } | CutCall::AiSetAttackInfo { .. })).count();
        eprintln!("made {made:?}, moved {moved:?}, records {records}");
        assert_eq!(made, (AI_AGGRESSIVE, Some(1)));
        assert_eq!(moved, (AI_PASSIVE, Some(1)));
        assert_eq!(records, 0);
    }

    /// Batch I39, `transferAttack(fromID, toID)` (AIUpdate.cpp:4166), which the rebuild hole's die and behavior and `CreateObjectDie`
    /// send to an AI: it reached no arm of the world's AI module, so it did nothing. Predicted before the run: a soldier ordered to
    /// attack enemy 1 has it as its goal object; after the transfer from enemy 1 to enemy 2, enemy 2; a second transfer from enemy 1
    /// (no longer the goal) changes nothing.
    #[test]
    fn an_attack_is_transferred_to_the_new_victim() {
        let mut w = gate_map_world();
        assert!(w.pathfinder_op(PfOp::NewMap));
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy_team = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let me = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let e1 = make(&mut w, b"Soldier", enemy_team, Coord3D { x: 400.0, y: 105.0, z: 0.0 });
        let e2 = make(&mut w, b"Soldier", enemy_team, Coord3D { x: 400.0, y: 205.0, z: 0.0 });
        let ai = w.find_object(me).and_then(|o| o.ai).expect("the soldier's AI");
        let goal = |w: &mut World| match w.module_call(me, ai, ModuleCall::AiGetGoalObject) {
            ModuleReply::Id(id) => id,
            r => panic!("{r:?}"),
        };
        w.module_call(me, ai, ModuleCall::AiAttackObject { victim: e1, max_shots: 100, command_source: 0 });
        let before = goal(&mut w);
        w.module_call(me, ai, ModuleCall::AiTransferAttack { from: e1, to: e2 });
        let after = goal(&mut w);
        w.module_call(me, ai, ModuleCall::AiTransferAttack { from: e1, to: me });
        let again = goal(&mut w);
        eprintln!("goal {before} -> {after} -> {again} (enemies {e1}, {e2})");
        assert_eq!((before, after, again), (e1, e2, e2));
    }

    /// Batch I39, `destroyObject`'s `setLocomotorGoalNone(); destroyPath()` on the AI (a record only). Predicted before the run: a
    /// soldier 30 frames into a move across the map has a path and a goal on it; after the call, neither; destroying another soldier
    /// makes no record of the call.
    #[test]
    fn a_destroyed_units_ai_stops() {
        let mut w = gate_map_world();
        assert!(w.pathfinder_op(PfOp::NewMap));
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let me = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let other = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 205.0, z: 0.0 });
        let ai = w.find_object(me).and_then(|o| o.ai).expect("the soldier's AI");
        w.module_call(me, ai, ModuleCall::AiMoveToPosition { pos: Coord3D { x: 350.0, y: 305.0, z: 0.0 }, command_source: 0 });
        for _ in 0..30 {
            w.logic_update().unwrap();
        }
        let state = |w: &World| {
            let a = w.ai_peek(me).expect("the AI");
            (a.path.is_some(), a.locomotor_goal_type)
        };
        let moving = state(&w);
        w.module_call(me, ai, ModuleCall::AiStopForDestroy);
        let stopped = state(&w);
        w.cuts.log = Some(Vec::new());
        w.destroy_object(other);
        w.logic_update().unwrap();
        let records = w.cuts.log.as_ref().unwrap().iter().filter(|c| matches!(c, CutCall::AiStopForDestroy { .. })).count();
        eprintln!("moving {moving:?}, stopped {stopped:?}, records {records}");
        assert_eq!(moving, (true, LocoGoalType::PositionOnPath));
        assert_eq!(stopped, (false, LocoGoalType::None));
        assert_eq!(records, 0);
    }

    /// Batch I39, `setWeaponBonusCondition` and `clearWeaponBonusCondition` tell the weapon set (`weaponSetOnWeaponBonusChange`, a
    /// record only): a weapon between shots restarts its delay from now at the new bonus (Weapon.cpp `onWeaponBonusChange`). A
    /// soldier shoots an enemy 50 away; 5 frames after its shot (GateGun: 500 ms between shots, 15 frames) the soldier is garrisoned
    /// (a condition the fixture gives no bonus). Predicted before the run: the reload restarts on that frame and the next shot is 15
    /// frames after it (5 frames later than before); no record. **Missed on the first run, by one frame, in the prediction's count:**
    /// the reload restarted on that frame (8) and the next shot moved to 23 = 8 + 15, as predicted, but from 17, 6 frames later, not 5:
    /// the shot was fired on frame 2 (17 = 2 + 15), and the loop saw it after the next update (frame 3). The move is checked as the
    /// frames since the shot.
    #[test]
    fn a_weapon_bonus_change_restarts_the_reload() {
        let mut w = gate_map_world();
        assert!(w.pathfinder_op(PfOp::NewMap));
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy_team = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let me = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let enemy = make(&mut w, b"Soldier", enemy_team, Coord3D { x: 150.0, y: 105.0, z: 0.0 });
        let ai = w.find_object(me).and_then(|o| o.ai).expect("the soldier's AI");
        w.module_call(me, ai, ModuleCall::AiAttackObject { victim: enemy, max_shots: 100, command_source: 0 });
        let gun = |w: &World| w.find_object(me).and_then(|o| o.weapon_set.get_cur_weapon().cloned()).expect("the gun");
        let mut shot = None;
        for _ in 0..90 {
            w.logic_update().unwrap();
            if gun(&w).get_status(w.frame()) == WeaponStatus::BetweenFiringShots {
                shot = Some(w.frame());
                break;
            }
        }
        let shot = shot.expect("a shot within 90 frames");
        for _ in 0..5 {
            w.logic_update().unwrap();
        }
        let before = gun(&w).possible_next_shot_frame();
        let now = w.frame();
        w.cuts.log = Some(Vec::new());
        w.set_weapon_bonus_condition(me, crate::logic::weapon_bonus::condition::GARRISONED);
        let g = gun(&w);
        let records = w.cuts.log.as_ref().unwrap().iter().filter(|c| matches!(c, CutCall::WeaponSetOnWeaponBonusChange { .. })).count();
        eprintln!("shot on {shot}, now {now}, next shot {before} -> {}, reload started {}, records {records}", g.possible_next_shot_frame(), g.last_reload_started_frame());
        assert_eq!(g.last_reload_started_frame(), now);
        assert_eq!(g.possible_next_shot_frame(), now + 15);
        assert_eq!(g.possible_next_shot_frame(), before + (now - (before - 15)));
        assert_eq!(records, 0);
    }
}

#[cfg(test)]
mod live_effect_tests {
    use super::*;
    use crate::common::disabled_types::DISABLED_EMP;
    use crate::logic::ai::ai_commands::AICMD_REPAIR;
    use crate::logic::ai::ai_update::AiSubclass;
    use crate::logic::ai::ai_update_types::AI_IDLE;
    use crate::logic::ai::dozer_ai_update::{DOZER_TASK_INVALID, DOZER_TASK_REPAIR};
    use crate::logic::world::module_call::ModuleReply;
    use crate::logic::world::module_probe::damage_of;
    use crate::common::game_type::INVALID_ID;
    use crate::logic::world::object_cuts::CutCall;

    fn records(w: &World, f: impl Fn(&CutCall) -> bool) -> usize {
        w.cuts.log.as_ref().unwrap().iter().filter(|c| f(c)).count()
    }

    /// Batch I40, `onDisabledEdge(true)`'s `dozerAI->cancelTask(dozerAI->getCurrentTask())` (Object.cpp:3820, a record only, and only
    /// for an id in a test table), and the dozer interface's calls to the world's AI module, which answered none of them (so a dozer's
    /// current task read as 0, `DOZER_TASK_BUILD`). A dozer ordered to repair a damaged depot of its team. Predicted before the run:
    /// 30 frames on, its current task is the repair (1) on the depot, and the module answers `getCurrentTask` with 1; disabled by EMP,
    /// the task is cancelled: current task -1, the repair task's target 0, no record of the call. **Missed on the first run, by the
    /// test's setup:** ordered on frame 0, the task stayed pending (the depot its target) and the current task -1, the dozer machine
    /// idle: `getMostRecentCommand` takes only an order frame after 0 (DozerAIUpdate.cpp, as the port); the order is now given after
    /// one frame.
    #[test]
    fn a_disabled_dozers_task_is_cancelled() {
        let mut w = gate_map_world();
        assert!(w.pathfinder_op(PfOp::NewMap));
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let dozer = make(&mut w, b"Dozer", team, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let depot = make(&mut w, b"Depot", team, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        w.attempt_damage(depot, &mut damage_of(500.0, INVALID_ID));
        let ai = w.find_object(dozer).and_then(|o| o.ai).expect("the dozer's AI");
        // (a task ordered on frame 0 is never taken: `getMostRecentCommand` wants an order frame after 0, as upstream's)
        w.logic_update().unwrap();
        w.module_call(dozer, ai, ModuleCall::AiDoCommand { cmd: AICMD_REPAIR, command_source: 0, obj: depot, pos: Coord3D::default() });
        for _ in 0..30 {
            w.logic_update().unwrap();
        }
        let task = |w: &World| match &w.ai_peek(dozer).expect("the AI").subclass {
            AiSubclass::Dozer(z) => (z.m.current_task, z.m.task[DOZER_TASK_REPAIR as usize].target_object_id),
            s => panic!("{s:?}"),
        };
        let working = task(&w);
        let asked = w.module_call(dozer, ai, ModuleCall::DozerGetCurrentTask);
        w.cuts.log = Some(Vec::new());
        let until = w.frame() + 100;
        w.set_disabled_until(dozer, DISABLED_EMP, until);
        let cancelled = task(&w);
        let n = records(&w, |c| matches!(c, CutCall::DozerCancelTask { .. }));
        eprintln!("working {working:?} (asked {asked:?}), cancelled {cancelled:?}, records {n}, errors {:?}", w.cuts.team_live_errors);
        assert_eq!(working, (DOZER_TASK_REPAIR, depot));
        assert!(matches!(asked, ModuleReply::Int(DOZER_TASK_REPAIR)));
        assert_eq!(cancelled, (DOZER_TASK_INVALID, INVALID_ID));
        assert_eq!(n, 0);
    }

    /// Batch I40, `onCapture`'s `aiIdle(CMD_FROM_AI)` (Object.cpp:4537, a record only). Predicted before the run: a soldier 30 frames
    /// into a move is not idle (its machine in a move state); captured by team 2's player it is idle (`AI_IDLE`), with no record.
    #[test]
    fn a_captured_unit_goes_idle() {
        let mut w = gate_map_world();
        assert!(w.pathfinder_op(PfOp::NewMap));
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy_team = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let me = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let other = make(&mut w, b"Soldier", enemy_team, Coord3D { x: 100.0, y: 305.0, z: 0.0 });
        let ai = w.find_object(me).and_then(|o| o.ai).expect("the soldier's AI");
        w.module_call(me, ai, ModuleCall::AiMoveToPosition { pos: Coord3D { x: 350.0, y: 105.0, z: 0.0 }, command_source: 0 });
        for _ in 0..30 {
            w.logic_update().unwrap();
        }
        let state = |w: &mut World| w.module_call(me, ai, ModuleCall::AiGetAiStateType).as_int(-1);
        let moving = state(&mut w);
        let old = w.get_controlling_player(me).expect("the soldier's player");
        let new = w.get_controlling_player(other).expect("the enemy's player");
        w.cuts.log = Some(Vec::new());
        w.on_capture(me, old, new);
        let captured = state(&mut w);
        let n = records(&w, |c| matches!(c, CutCall::AiIdle { .. }));
        eprintln!("moving {moving}, captured {captured} (players {old} -> {new}), records {n}");
        assert_ne!(moving, AI_IDLE as i32);
        assert_eq!(captured, AI_IDLE as i32);
        assert_eq!(n, 0);
    }

    /// Batch I40, the destructor's `m_group->remove(this)` (Object.cpp:650, a record only): a destroyed member stayed in its group.
    /// Predicted before the run: a group of two soldiers; the first destroyed and the frame run, the group holds only the second; no
    /// record.
    #[test]
    fn a_destroyed_unit_leaves_its_group() {
        let mut w = gate_map_world();
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let a = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let b = make(&mut w, b"Soldier", team, Coord3D { x: 100.0, y: 205.0, z: 0.0 });
        let g = w.ai_create_group();
        w.with_group(g, |gr, w| {
            gr.add(w, a);
            gr.add(w, b);
        });
        let before = w.ai_group_members(g);
        w.cuts.log = Some(Vec::new());
        w.destroy_object(a);
        w.logic_update().unwrap();
        let after = w.ai_group_members(g);
        let n = records(&w, |c| matches!(c, CutCall::AiGroupRemove { .. }));
        eprintln!("members {before:?} -> {after:?}, records {n}");
        assert_eq!(before.len(), 2);
        assert_eq!(after, vec![b]);
        assert_eq!(n, 0);
    }

    /// Batch I40, `LocomotorSetUpgrade`'s `ai->setLocomotorUpgrade(true)` and `PassengersFireUpgrade`'s
    /// `contain->setPassengerAllowedToFire(TRUE)` (both records only). A truck with both, triggered by `Upgrade_Fire`. Predicted before
    /// the run: before the upgrade, its locomotors not upgraded and its passengers not allowed to fire; given the upgrade, both; no
    /// record of either.
    #[test]
    fn a_trucks_upgrades_reach_its_ai_and_contain() {
        let extra = "\
Upgrade Upgrade_Fire\n Type = OBJECT\n BuildCost = 100\nEnd\n\
Object FireTruck\n KindOf = VEHICLE TRANSPORT\n Geometry = CYLINDER\n GeometryMajorRadius = 8\n GeometryHeight = 6\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 200\n  InitialHealth = 200\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = TransportContain ModuleTag_Contain\n  Slots = 2\n  AllowInsideKindOf = VEHICLE\n  AllowAlliesInside = Yes\n  ExitDelay = 0\n End\n\
 Behavior = LocomotorSetUpgrade ModuleTag_Loco\n  TriggeredBy = Upgrade_Fire\n End\n\
 Behavior = PassengersFireUpgrade ModuleTag_Fire\n  TriggeredBy = Upgrade_Fire\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 100\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n";
        let mut w = gate_map_world_with(extra);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let truck = make(&mut w, b"FireTruck", team, Coord3D { x: 100.0, y: 105.0, z: 0.0 });
        let mask = w.statics.upgrade_center.templates().find(|u| u.upgrade_name() == b"Upgrade_Fire").map(|u| u.upgrade_mask()).expect("the upgrade");
        let state = |w: &mut World| (w.ai_peek(truck).expect("the AI").upgraded_locomotors, crate::logic::ai::ai_group::AiGroupHost::contain_passengers_allowed_to_fire(w, truck));
        let before = state(&mut w);
        w.cuts.log = Some(Vec::new());
        w.give_upgrade(truck, Some(&mask));
        let after = state(&mut w);
        let n = records(&w, |c| matches!(c, CutCall::AiSetLocomotorUpgrade { .. } | CutCall::ContainSetPassengerAllowedToFire { .. }));
        eprintln!("before {before:?}, after {after:?}, records {n}, errors {:?}", w.cuts.module_errors);
        assert_eq!(before, (false, false));
        assert_eq!(after, (true, true));
        assert_eq!(n, 0);
    }
}

#[cfg(test)]
mod temp_weapon_tests {
    use super::*;
    use crate::common::game_type::INVALID_ID;
    use crate::logic::world::module_probe::damage_of;
    use crate::logic::world::object_cuts::CutCall;

    const EXTRA: &str = "\
Weapon Blast\n PrimaryDamage = 30\n PrimaryDamageRadius = 20\n AttackRange = 20\n DamageType = EXPLOSION\n DeathType = NORMAL\n WeaponSpeed = 999999\n DelayBetweenShots = 500\nEnd\n\
Object Bomber\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = FireWeaponWhenDeadBehavior ModuleTag_Death\n  StartsActive = Yes\n  DeathWeapon = Blast\n End\nEnd\n";

    fn records(w: &World) -> usize {
        w.cuts.log.as_ref().unwrap().iter().filter(|c| matches!(c, CutCall::WeaponCreateAndFireTemp { .. })).count()
    }

    /// Batch I41, `FireWeaponWhenDeadBehavior::onDie`'s `TheWeaponStore->createAndFireTempWeapon(m_deathWeapon, obj, obj->getPosition())`
    /// (a record only). A bomber with the death weapon `Blast` (30 damage, radius 20, instant), an enemy soldier 10 east and an allied
    /// one 10 west. Predicted before the run: killed, the blast hits both at once (upstream's default `RadiusDamageAffects` is allies,
    /// enemies and neutrals): each 100 → 70, with no record and no error; a soldier 50 away untouched.
    #[test]
    fn a_dead_units_death_weapon_hits_its_neighbours() {
        let mut w = gate_map_world_with(EXTRA);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy_team = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let bomber = make(&mut w, b"Bomber", team, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        let enemy = make(&mut w, b"Soldier", enemy_team, Coord3D { x: 210.0, y: 200.0, z: 0.0 });
        let ally = make(&mut w, b"Soldier", team, Coord3D { x: 190.0, y: 200.0, z: 0.0 });
        let far = make(&mut w, b"Soldier", enemy_team, Coord3D { x: 250.0, y: 200.0, z: 0.0 });
        w.cuts.log = Some(Vec::new());
        w.attempt_damage(bomber, &mut damage_of(1000.0, INVALID_ID));
        let health: Vec<f32> = [enemy, ally, far].iter().map(|&s| w.body_health(s).map_or(-1.0, |h| h.0)).collect();
        let n = records(&w);
        eprintln!("health {health:?}, records {n}, errors {:?}", w.cuts.team_live_errors);
        assert_eq!(health, vec![70.0, 70.0, 100.0]);
        assert_eq!(n, 0);
        assert!(w.cuts.team_live_errors.is_empty());
    }

    /// Batch I41, `World::fire_temp_weapon`'s edges. Predicted before the run: a name the store lacks (upstream's `NULL` template)
    /// does nothing, with no error; no source (`NULL`, which upstream would dereference) does nothing and is reported; with the live AI
    /// off, the call is the record and nothing is hit. **Missed on the first run, by the prediction:** a live shot from the source
    /// (200, 200) at the soldier's (300, 300) was predicted to hit (100 → 70); it did nothing, because upstream's
    /// `WeaponTemplate::fireWeaponTemplate` returns without firing when the distance exceeds `AttackRange` (Weapon.cpp:874, 20 here
    /// against about 141), and the temporary weapon does not ignore ranges; the port does the same. Kept as the out-of-range case, with
    /// an in-range shot from a second source 10 from the soldier: 100 → 70.
    #[test]
    fn a_temp_weapon_without_a_template_or_source_does_nothing() {
        let mut w = gate_map_world_with(EXTRA);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy_team = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let source = make(&mut w, b"Bomber", team, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        let target = make(&mut w, b"Soldier", enemy_team, Coord3D { x: 300.0, y: 300.0, z: 0.0 });
        let at = Coord3D { x: 300.0, y: 300.0, z: 0.0 };
        let hp = |w: &mut World| w.body_health(target).map_or(-1.0, |h| h.0);
        w.fire_temp_weapon(b"NoSuchWeapon".to_vec(), source, at);
        let unknown = (hp(&mut w), w.cuts.team_live_errors.len());
        w.fire_temp_weapon(b"Blast".to_vec(), INVALID_ID, at);
        let sourceless = (hp(&mut w), w.cuts.team_live_errors.len());
        w.cuts.live_ai = false;
        w.cuts.log = Some(Vec::new());
        w.fire_temp_weapon(b"Blast".to_vec(), source, at);
        let recorded = (hp(&mut w), records(&w));
        w.cuts.live_ai = true;
        w.fire_temp_weapon(b"Blast".to_vec(), source, at);
        let out_of_range = hp(&mut w);
        let near = make(&mut w, b"Bomber", team, Coord3D { x: 290.0, y: 300.0, z: 0.0 });
        w.fire_temp_weapon(b"Blast".to_vec(), near, at);
        let fired = hp(&mut w);
        eprintln!("unknown {unknown:?}, sourceless {sourceless:?} {:?}, recorded {recorded:?}, out of range {out_of_range}, fired {fired}", w.cuts.team_live_errors);
        assert_eq!(unknown, (100.0, 0));
        assert_eq!(sourceless, (100.0, 1));
        assert_eq!(recorded, (100.0, 1));
        assert_eq!(out_of_range, 100.0);
        assert_eq!(fired, 70.0);
    }
}
