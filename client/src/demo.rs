// SPDX-License-Identifier: GPL-3.0-only
//! The demo game the client runs until the player's own Zero Hour files are loaded (milestone C2d, GitHub #6). It is the
//! gate scenario's world and test data (`rts_lockstep_engine::logic::world::gate_scenario`: this repository's own INI, no
//! EA data), with units the player can select and order: `SELECTABLE` copies of the gate's Runner and Soldier, a WorkDozer
//! that can build the gate's Depot, and I23's Caster with its drop power, all with sight, as shipped units have.
//!
//! Player 1 (the local player, set at the demo's game start as `PlayerList::newGame` sets it, #56) has two Runners, two Soldiers, the dozer and the Caster in the south-west; player 2 has two
//! Soldiers in the north-east, beyond the Soldiers' sight. Since #54 the Soldiers have guard and attack move buttons (and
//! voices for them, whose sounds the demo does not have), and player 1 has a Hut with a set rally point button and an
//! exit to rally. The game is in progress (`GAME_SKIRMISH`), so the dispatcher
//! makes groups for the player's commands. Since #35 the demo's own templates have a `RadarPriority` (units, the Hut a
//! structure), so they show on the radar.

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::recorder::game_mode;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::gate_scenario::{gate_map_world_with, make, INI};
use rts_lockstep_engine::logic::world::player_list::player_list_set_local_player;
use rts_lockstep_engine::logic::world::team_ops::factory_find_team;

/// The demo's objects, by role.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DemoObjects {
    pub runners: Vec<ObjectId>,
    pub soldiers: Vec<ObjectId>,
    pub dozer: ObjectId,
    pub caster: ObjectId,
    pub enemies: Vec<ObjectId>,
    /// #54: player 1's Hut, with a rally point.
    pub hut: ObjectId,
    /// The dozer's construct-Depot button, in the World's button list.
    pub build_depot: usize,
}

/// The gate INI's `Object name` block, renamed `Click<name>`, `SELECTABLE` and with a vision of 250.
fn selectable(name: &str) -> String {
    let start = INI.find(&format!("Object {name}\n")).expect("the template");
    let end = start + INI[start..].find("\nEnd\n").expect("its end") + 5;
    INI[start..end]
        .replacen(&format!("Object {name}"), &format!("Object Click{name}"), 1)
        .replacen("KindOf = VEHICLE", "KindOf = VEHICLE SELECTABLE\n VisionRange = 250\n RadarPriority = UNIT", 1)
}

/// The dozer, with its portrait (#35: the synthetic control bar's `ImgPortrait`) and its command set: construct a Depot, with
/// the demo's own button texts (this project's, not the game's).
const WORK_DOZER: &str = "\
Object WorkDozer\n KindOf = VEHICLE DOZER SELECTABLE\n RadarPriority = UNIT\n CommandSet = DemoDozerSet\n SelectPortrait = ImgPortrait\n VisionRange = 250\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = DozerAIUpdate ModuleTag_AI\n  RepairHealthPercentPerSecond = 2%\n  BoredTime = 5000\n  BoredRange = 150\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n\
CommandButton Command_BuildDepot\n Command = DOZER_CONSTRUCT\n Object = Depot\n TextLabel = DEMO:BuildDepot\n DescriptLabel = DEMO:BuildDepotDesc\nEnd\n\
CommandSet DemoDozerSet\n 1 = Command_BuildDepot\nEnd\n";

/// #54: the Soldiers' guard and attack move buttons and voices, and the Hut with its rally point button and exit (the
/// demo's own, not the game's). The guard button may target an object or the ground, as the shipped one does. A rally point
/// is only set where `BasicHumanLocomotor` can find a path (`doSetRallyPoint`'s one locomotor, by name), so the demo has one,
/// for the ground; without it every rally point is refused, as upstream's `clientSafeQuickDoesPathExist` does.
const GUARD_AND_RALLY: &str = "\
Locomotor BasicHumanLocomotor\n Surfaces = GROUND\n Speed = 20\n Acceleration = 100\n Braking = 100\n TurnRate = 180\n Appearance = TWO_LEGS\nEnd\n\
CommandButton Command_DemoGuard\n Command = GUARD\n Options = NEED_TARGET_ENEMY_OBJECT NEED_TARGET_NEUTRAL_OBJECT NEED_TARGET_ALLY_OBJECT NEED_TARGET_POS OK_FOR_MULTI_SELECT\n TextLabel = DEMO:Guard\nEnd\n\
CommandButton Command_DemoAttackMove\n Command = ATTACK_MOVE\n Options = NEED_TARGET_POS OK_FOR_MULTI_SELECT\n TextLabel = DEMO:AttackMove\nEnd\n\
CommandButton Command_DemoRally\n Command = SET_RALLY_POINT\n Options = NEED_TARGET_POS\n TextLabel = DEMO:Rally\nEnd\n\
CommandSet DemoSoldierSet\n 1 = Command_DemoGuard\n 2 = Command_DemoAttackMove\nEnd\n\
CommandSet DemoHutSet\n 1 = Command_DemoRally\nEnd\n\
Object DemoHut\n KindOf = STRUCTURE IMMOBILE SELECTABLE\n RadarPriority = STRUCTURE\n CommandSet = DemoHutSet\n VisionRange = 250\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 500\n  InitialHealth = 500\n End\n\
 Behavior = DefaultProductionExitUpdate ModuleTag_Exit\n  UnitCreatePoint = X:0 Y:0 Z:0\n  NaturalRallyPoint = X:20 Y:0 Z:0\n End\nEnd\n";

/// GitHub #56: the demo's local player, `Player1` (index 1; 0 is the neutral player).
pub const DEMO_LOCAL_PLAYER: i32 = 1;

/// The demo's own texts for its buttons (#35, #54), by label.
pub const DEMO_TEXT: [(&str, &str); 5] = [
    ("DEMO:BuildDepot", "Build a Depot"),
    ("DEMO:BuildDepotDesc", "The demo's one building: a depot. Hold or hover a button to read this."),
    ("DEMO:Guard", "Guard"),
    ("DEMO:AttackMove", "Attack move"),
    ("DEMO:Rally", "Set rally point"),
];

/// The demo game at frame 0, and its objects.
pub fn demo_world() -> (World, DemoObjects) {
    let caster = rts_lockstep_engine::logic::world::gate_scenario::CASTER_INI
        .replacen("Object Caster\n KindOf = VEHICLE", "Object Caster\n KindOf = VEHICLE SELECTABLE\n VisionRange = 250\n RadarPriority = UNIT", 1);
    // (#54: the Soldiers' command set and voices)
    let soldier = selectable("Soldier").replacen(
        "Object ClickSoldier\n",
        "Object ClickSoldier\n CommandSet = DemoSoldierSet\n VoiceGuard = DemoSoldierGuard\n VoiceMove = DemoSoldierMove\n",
        1,
    );
    let extra = [selectable("Runner"), soldier, WORK_DOZER.to_string(), caster, GUARD_AND_RALLY.to_string()].join("\n");
    let mut w = gate_map_world_with(&extra);
    // GitHub #56: the local player, as `PlayerList::newGame` (`GameLogic::startNewGame`) sets it before the map's objects
    // are made: with no network, the first side whose `playerIsHuman` is set, through `setLocalPlayer` (which tells the
    // player it became local: its team colour and the control bar). The gate's two sides are both human, so `Player1`.
    // ~~The demo never set it, so the logic's local feedback (`doSetRallyPoint`'s message and sound, the EVA) never fired.~~
    player_list_set_local_player(&mut w, Some(DEMO_LOCAL_PLAYER)).expect("the local player");
    let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
    let enemy = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
    let at = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
    let mut o = DemoObjects::default();
    for (x, y) in [(40.0, 60.0), (40.0, 90.0)] {
        o.runners.push(make(&mut w, b"ClickRunner", team, at(x, y)));
    }
    for (x, y) in [(80.0, 60.0), (80.0, 90.0)] {
        o.soldiers.push(make(&mut w, b"ClickSoldier", team, at(x, y)));
    }
    o.dozer = make(&mut w, b"WorkDozer", team, at(60.0, 130.0));
    o.caster = make(&mut w, b"Caster", team, at(100.0, 130.0));
    for (x, y) in [(360.0, 360.0), (370.0, 340.0)] {
        o.enemies.push(make(&mut w, b"ClickSoldier", enemy, at(x, y)));
    }
    // #54: made last, so the other objects keep their ids
    o.hut = make(&mut w, b"DemoHut", team, at(40.0, 200.0));
    // the dozer's command set: construct a Depot (~~the control bar's data, which the World keeps as a table~~ **since #35**
    // the demo's INI, so the button has its texts)
    o.build_depot = w.cuts.command_buttons.iter().position(|b| b.name == b"Command_BuildDepot").expect("the dozer's button");
    if let Some(statics) = std::sync::Arc::get_mut(&mut w.statics) {
        for (label, text) in DEMO_TEXT {
            let units: Vec<u16> = text.encode_utf16().collect();
            if let Ok(t) = rts_lockstep_engine::common::unicode_string::UnicodeString::try_from_chars(&units) {
                statics.game_text.strings.insert(label.as_bytes().to_vec(), t);
            }
        }
    }
    w.pathfinder_op(rts_lockstep_engine::logic::world::pathfinder_host_world::PfOp::NewMap);
    w.cuts.game_loop.game_mode = game_mode::SKIRMISH;
    (w, o)
}
