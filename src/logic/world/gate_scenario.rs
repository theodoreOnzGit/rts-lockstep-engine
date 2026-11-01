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
use crate::common::damage_fx::DamageFxStore;
use crate::common::dict::Dict;
use crate::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use crate::common::module_factory::ModuleFactory;
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::science::{GameTextFetch, ScienceStore};
use crate::common::thing_factory::ThingFactory;
use crate::common::thing_template::ThingParseEnv;
use crate::common::unicode_string::UnicodeString;
use crate::logic::armor::ArmorStore;
use crate::logic::locomotor::LocomotorStore;
use crate::logic::terrain_logic::{GroundSource, TerrainLogic};
use crate::logic::terrain_visual::LogicHeightMap;
use crate::logic::weapon_set::WeaponTemplateTable;
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_update::CRC_RECALC;
use crate::logic::world::ids::{ObjectId, TeamId};
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::pathfinder_host_world::PfOp;
use crate::logic::world::statics::GameStatics;
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

/// What the game's INI is read into.
struct Ctx {
    factory: ThingFactory,
    names: NameKeyGenerator,
    modules: ModuleFactory,
    armors: ArmorStore,
    dfx: DamageFxStore,
    weapons: WeaponTemplateTable,
    sciences: ScienceStore,
    locomotors: LocomotorStore,
    weapon_store: crate::logic::weapon::WeaponStore,
    ai_data: crate::logic::ai::ai_data::AiDataChain,
}

/// No game text (the game names none).
struct NoText;
impl GameTextFetch for NoText {
    fn fetch(&self, _label: &[u8]) -> UnicodeString {
        UnicodeString::new()
    }
}

fn object_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    let mut env = ThingParseEnv {
        names: &mut c.names,
        modules: &mut c.modules,
        armors: &c.armors,
        damage_fx: &c.dfx,
        weapons: &c.weapons,
        sciences: &c.sciences,
        text: &NoText,
        locomotors: &c.locomotors,
    };
    c.factory.parse_object_block(ini, &mut env, 0)
}

fn ai_data_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    crate::logic::ai::ai_data::parse_ai_data_definition(ini, &mut c.ai_data, &c.sciences, &mut c.names)
        .unwrap_or_else(|_| Err(crate::common::ini::IniError::new(crate::common::ini::IniErrorKind::Bug)))
}

fn weapon_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.weapon_store.parse_weapon_template_definition(ini, &mut c.names)?;
    // (the thing templates' weapon sets name the weapons by key)
    for t in c.weapon_store.templates() {
        c.weapons.add(t.name_key);
    }
    Ok(())
}

fn locomotor_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.locomotors.parse_locomotor_template_definition(ini, &mut c.names)
}

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
    id
}

/// The game at frame 0: the world, its map, its player's team and its objects, the runners' orders given.
pub fn gate_world() -> World {
    let mut names = NameKeyGenerator::new();
    names.reset();
    let mut modules = ModuleFactory::new();
    modules.init(&mut names).expect("the module factory");
    let mut c = Ctx {
        factory: ThingFactory::new(),
        names,
        modules,
        armors: ArmorStore::new(),
        dfx: DamageFxStore::new(),
        weapons: WeaponTemplateTable::new(),
        sciences: ScienceStore::new(),
        locomotors: LocomotorStore::new(),
        weapon_store: crate::logic::weapon::WeaponStore::new(),
        ai_data: Default::default(),
    };
    static BLOCKS: [BlockParse<Ctx>; 4] = [
        BlockParse { token: "AIData", parse: ai_data_block },
        BlockParse { token: "Object", parse: object_block },
        BlockParse { token: "Locomotor", parse: locomotor_block },
        BlockParse { token: "Weapon", parse: weapon_block },
    ];
    Ini::load(b"Data\\INI\\Gate.ini", INI.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut c).expect("the game's INI");
    c.factory.post_process_load();

    let mut w = World::new();
    w.names = c.names;
    let mut st = GameStatics { thing_factory: c.factory, locomotor_store: c.locomotors, weapon_store: c.weapon_store, ai_data: c.ai_data.current().clone(), ..Default::default() };
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
    let mut t = TerrainLogic::new(GroundSource::LogicHeightMap);
    let size = ICoord2D { x: 40, y: 40 };
    t.set_logic_height_map(Some(LogicHeightMap::new(40, 40, 0, vec![size], vec![0; 1600]).expect("the height map")));
    t.set_boundaries(vec![size], 0);
    w.terrain = t;
    w.players.set_player_count(3);
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
    w.cuts.live_partition = true;
    w.cuts.live_teams = true;
    w.cuts.live_ai = true;
    w.cuts.live_pathfinder = true;

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
    /// **Result** (2026-11-01, native, release): health after 300 frames `[60, 0, 0, 100]`: one soldier of each side killed,
    /// the first soldier hit twice. Before this batch the soldiers took their mood targets every frame but never attacked:
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
        assert_eq!(calls, ["GameClient::setFrame", "Recorder::update", "in_game_ui_idle_worker"]);
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
}
