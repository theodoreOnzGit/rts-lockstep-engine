// SPDX-License-Identifier: GPL-3.0-only
//! The demo game the client runs until the player's own Zero Hour files are loaded (milestone C2d, GitHub #6). It is the
//! gate scenario's world and test data (`rts_lockstep_engine::logic::world::gate_scenario`: this repository's own INI, no
//! EA data), with units the player can select and order: `SELECTABLE` copies of the gate's Runner and Soldier, a WorkDozer
//! that can build the gate's Depot, and I23's Caster with its drop power, all with sight, as shipped units have.
//!
//! Player 1 (the local player) has two Runners, two Soldiers, the dozer and the Caster in the south-west; player 2 has two
//! Soldiers in the north-east, beyond the Soldiers' sight. The game is in progress (`GAME_SKIRMISH`), so the dispatcher
//! makes groups for the player's commands.

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::recorder::game_mode;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::gate_scenario::{gate_map_world_with, make, INI};
use rts_lockstep_engine::logic::world::team_ops::factory_find_team;

/// The demo's objects, by role.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DemoObjects {
    pub runners: Vec<ObjectId>,
    pub soldiers: Vec<ObjectId>,
    pub dozer: ObjectId,
    pub caster: ObjectId,
    pub enemies: Vec<ObjectId>,
    /// The dozer's construct-Depot button, in the World's button list.
    pub build_depot: usize,
}

/// The gate INI's `Object name` block, renamed `Click<name>`, `SELECTABLE` and with a vision of 250.
fn selectable(name: &str) -> String {
    let start = INI.find(&format!("Object {name}\n")).expect("the template");
    let end = start + INI[start..].find("\nEnd\n").expect("its end") + 5;
    INI[start..end]
        .replacen(&format!("Object {name}"), &format!("Object Click{name}"), 1)
        .replacen("KindOf = VEHICLE", "KindOf = VEHICLE SELECTABLE\n VisionRange = 250", 1)
}

/// The dozer, with its portrait (#35: the synthetic control bar's `ImgPortrait`) and its command set: construct a Depot, with
/// the demo's own button texts (this project's, not the game's).
const WORK_DOZER: &str = "\
Object WorkDozer\n KindOf = VEHICLE DOZER SELECTABLE\n CommandSet = DemoDozerSet\n SelectPortrait = ImgPortrait\n VisionRange = 250\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = DozerAIUpdate ModuleTag_AI\n  RepairHealthPercentPerSecond = 2%\n  BoredTime = 5000\n  BoredRange = 150\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n\
CommandButton Command_BuildDepot\n Command = DOZER_CONSTRUCT\n Object = Depot\n TextLabel = DEMO:BuildDepot\n DescriptLabel = DEMO:BuildDepotDesc\nEnd\n\
CommandSet DemoDozerSet\n 1 = Command_BuildDepot\nEnd\n";

/// The demo's own texts for its button (#35), by label.
pub const DEMO_TEXT: [(&str, &str); 2] = [("DEMO:BuildDepot", "Build a Depot"), ("DEMO:BuildDepotDesc", "The demo's one building: a depot. Hold or hover a button to read this.")];

/// The demo game at frame 0, and its objects.
pub fn demo_world() -> (World, DemoObjects) {
    let caster = rts_lockstep_engine::logic::world::gate_scenario::CASTER_INI
        .replacen("Object Caster\n KindOf = VEHICLE", "Object Caster\n KindOf = VEHICLE SELECTABLE\n VisionRange = 250", 1);
    let extra = [selectable("Runner"), selectable("Soldier"), WORK_DOZER.to_string(), caster].join("\n");
    let mut w = gate_map_world_with(&extra);
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
