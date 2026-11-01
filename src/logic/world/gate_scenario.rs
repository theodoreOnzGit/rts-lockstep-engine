// SPDX-License-Identifier: GPL-3.0-only
//
// Part of a modified Rust translation of EA's GPLv3 release of Command & Conquer Generals Zero Hour
// (https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590), with the
// additional terms in NOTICE.md. Not upstream code: the scripted game of Phase 1 gate (a).
//! Phase 1 gate (a), integration batch I9d: one scripted game, the same on every build, whose per-frame logic CRCs a native
//! run and a wasm run must give identically (`SCOPE.md`, section 5).
//!
//! The game: a flat 400 x 400 map (40 x 40 cells) with the pathfinder, the partition manager, the teams and the AI live; an
//! immobile depot in the middle (an obstacle on the pathfinder's map); six runners (a ground locomotor, physics) on the west
//! side told by their player to cross to the east side along paths that cross around the depot, the first three each by its
//! own order and the last three as an AI group (batch I9c-2: `groupMoveToPosition`); a gun whose AI has a turret;
//! an idle dozer. Every frame is `GameLogic::update`, then `getCRC(CRC_RECALC)`.
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
Object Depot\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 20\n GeometryMinorRadius = 20\n GeometryHeight = 10\n\
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
    };
    static BLOCKS: [BlockParse<Ctx>; 2] =
        [BlockParse { token: "Object", parse: object_block }, BlockParse { token: "Locomotor", parse: locomotor_block }];
    Ini::load(b"Data\\INI\\Gate.ini", INI.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut c).expect("the game's INI");
    c.factory.post_process_load();

    let mut w = World::new();
    w.names = c.names;
    let mut st = GameStatics { thing_factory: c.factory, locomotor_store: c.locomotors, ..Default::default() };
    st.global_data.create_original().partition_cell_size = 40.0;
    w.statics = Arc::new(st);
    let mut t = TerrainLogic::new(GroundSource::LogicHeightMap);
    let size = ICoord2D { x: 40, y: 40 };
    t.set_logic_height_map(Some(LogicHeightMap::new(40, 40, 0, vec![size], vec![0; 1600]).expect("the height map")));
    t.set_boundaries(vec![size], 0);
    w.terrain = t;
    w.players.set_player_count(2);
    let key = w.names.name_to_key(b"Player1");
    w.players.get_nth_mut(1).expect("the player").name_key = key.0 as i32;
    factory_init_team(&mut w, b"team1", b"Player1", true, &Dict::new()).expect("the team");
    let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
    // (the player allied to itself, as `Player::initFromDict` and `PlayerList::newGame` leave it)
    crate::logic::world::player_ops::player_set_player_relationship(&mut w, 1, Some(1), crate::common::game_common::Relationship::Allies)
        .expect("the player's relationship");
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
