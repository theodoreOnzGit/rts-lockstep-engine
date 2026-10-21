// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CommandSetUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/CommandSetUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   CommandSetUpgrade.cpp: Graham Smallwood, September 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CommandSetUpgrade`: "an upgrade that sets a new override string for
//! Command Set look ups". When it fires it gives the object
//! `CommandSetAlt` if the `TriggerAlt` upgrade is already complete (for the
//! controlling player, or else for the object itself), and `CommandSet`
//! otherwise, then tells the control bar to redraw.
//!
//! # The lead's reference layer-9 module
//!
//! This is the worked example every layer-9 module follows. What a module
//! needs, and where each piece is:
//!
//! | piece | here |
//! |---|---|
//! | the data class and its INI field table, base fields first | [`CommandSetUpgradeModuleData`], [`COMMAND_SET_UPGRADE_FIELD_PARSE`] |
//! | the module: its base(s) by value, its own state | [`CommandSetUpgrade`] |
//! | the interface calls `Object` makes | [`UpgradeBehavior`] + `upgrade_module_call` (`modules::upgrade_body`) |
//! | static stores (`TheUpgradeCenter`) | `world.statics`; the name-key generator is `world.names` |
//! | client notifications | `world.emit(ObjectClientEvent::..)` |
//! | its variant of `UpdateBody` and the arms of every dispatch `match` | `world::update_body`, block `p1l9-example` |
//! | `xfer`/`crc` of the whole base chain, in upstream's order | [`Snapshot`] here; `UpdateBody::owns_base_chain` |
//! | the factory: data constructor, interface mask, `ModuleDataKind` variant | `common::module_factory::translated_module`, `common::module` |
//! | building the module for an object | `modules::new_module_body` |
//! | a differential test of the unchanged C++ | `tests/differential_command_set_upgrade.rs` |
//!
//! No upstream defect: the module does what its comments say. Note that
//! `TriggerAlt` defaults to `"none"`, which is not an upgrade, so
//! `findUpgrade` answers `NULL` and the module gives `CommandSet`; the lookup
//! still assigns `"none"` a name key, as upstream's does.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// `class CommandSetUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct CommandSetUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the `UpgradeModule` base of
    /// every module made from this data (upstream: the same object).
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_newCommandSet` (`CommandSet`).
    pub new_command_set: Vec<u8>,
    /// `m_newCommandSetAlt` (`CommandSetAlt`).
    pub new_command_set_alt: Vec<u8>,
    /// `m_triggerAlt` (`TriggerAlt`).
    pub trigger_alt: Vec<u8>,
}

impl Default for CommandSetUpgradeModuleData {
    /// `CommandSetUpgradeModuleData()`: both sets empty, `m_triggerAlt = "none"`.
    fn default() -> Self {
        CommandSetUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            new_command_set: Vec::new(),
            new_command_set_alt: Vec::new(),
            trigger_alt: b"none".to_vec(),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<CommandSetUpgradeModuleData>; 6] =
    upgrade_module_data_field_parse!(CommandSetUpgradeModuleData, upgrade);

/// `CommandSetUpgradeModuleData::buildFieldParse`'s own table.
pub static COMMAND_SET_UPGRADE_FIELD_PARSE: [FieldParse<CommandSetUpgradeModuleData>; 4] = [
    FieldParse::new("CommandSet", |i, d, _| { d.new_command_set = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("CommandSetAlt", |i, d, _| { d.new_command_set_alt = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("TriggerAlt", |i, d, _| { d.trigger_alt = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl CommandSetUpgradeModuleData {
    /// `CommandSetUpgradeModuleData::buildFieldParse(p)`:
    /// `UpgradeModuleData::buildFieldParse(p)` (which adds the mux table after
    /// `ModuleData`'s, which adds none), then this class's table.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CommandSetUpgradeModuleData>) -> IniResult<()> {
        crate::logic::module::behavior_module::BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&COMMAND_SET_UPGRADE_FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)` (`MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA`):
    /// a default data object, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CommandSetUpgradeModuleData> {
        let mut data = CommandSetUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CommandSetUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class CommandSetUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct CommandSetUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getCommandSetUpgradeModuleData()`.
    data: Arc<CommandSetUpgradeModuleData>,
}

impl CommandSetUpgrade {
    /// `CommandSetUpgrade(thing, moduleData)`: the base, nothing of its own.
    /// `INI_INVALID_DATA` for no data, as the base's constructor.
    pub fn new(object: ObjectId, data: Option<Arc<CommandSetUpgradeModuleData>>) -> IniResult<CommandSetUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(CommandSetUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getCommandSetUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<CommandSetUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }

    /// `obj->setCommandSetStringOverride(set); TheControlBar->markUIDirty()`.
    fn give_command_set(world: &mut World, me: ModuleHandle, set: &[u8]) {
        if let Some(o) = world.objects.find_mut(me.object) {
            o.command_set_string_override = set.to_vec();
        }
        world.emit(ObjectClientEvent::ControlBarMarkUiDirty); // "Refresh the UI in case we are selected"
    }
}

impl PartialEq for CommandSetUpgrade {
    /// The same state: the same data object (upstream compares pointers) and
    /// the same `m_upgradeExecuted`. (`UpdateBody` is compared in tests.)
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl UpgradeBehavior for CommandSetUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }

    /// `upgradeImplementation()`.
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let d = self.data.clone();
        let statics = world.statics.clone();
        let upgrade_template = statics
            .upgrade_center
            .find_upgrade(&mut world.names, &d.trigger_alt)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        if let Some(upgrade_template) = upgrade_template {
            let upgrade_mask = upgrade_template.upgrade_mask();
            // "See if upgrade is found in the player completed upgrades"
            if let Some(player) = world.get_controlling_player(me.object) {
                let player_mask = world.cuts.player_completed_upgrade_mask(player);
                if player_mask.test_for_any(&upgrade_mask) {
                    CommandSetUpgrade::give_command_set(world, me, &d.new_command_set_alt);
                    return Ok(());
                }
            }
            // "See if upgrade is found in the object completed upgrades"
            let object_mask = world.objects.find(me.object).map(|o| o.get_object_completed_upgrade_mask()).unwrap_or_default();
            if object_mask.test_for_any(&upgrade_mask) {
                CommandSetUpgrade::give_command_set(world, me, &d.new_command_set_alt);
                return Ok(());
            }
        }
        CommandSetUpgrade::give_command_set(world, me, &d.new_command_set);
        Ok(())
    }

    /// `isSubObjectsUpgrade()`: `false`.
    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::CommandSetUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CommandSetUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for CommandSetUpgrade {
    /// `crc`: "extend base class", `UpgradeModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }

    /// `xfer` (version 1, "Initial version"): the version, then
    /// `UpgradeModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.base.xfer(xfer)
    }

    /// `loadPostProcess`: `UpgradeModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.base.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `CommandSetUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CommandSetUpgrade(Arc::new(CommandSetUpgradeModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::common::name_key_generator::NameKeyGenerator;
    use crate::common::thing_template::ThingTemplate;
    use crate::common::upgrade::{make_upgrade_mask, UpgradeCenter, UpgradeMaskType};
    use crate::logic::world::object_create::NewObject;
    use crate::logic::world::statics::GameStatics;

    fn block(ini: &mut Ini<'_>, out: &mut Vec<IniResult<CommandSetUpgradeModuleData>>) -> IniResult<()> {
        out.push(CommandSetUpgradeModuleData::friend_new_module_data(Some(ini)));
        Ok(())
    }
    static BLOCKS: [BlockParse<Vec<IniResult<CommandSetUpgradeModuleData>>>; 1] = [BlockParse { token: "D", parse: block }];

    fn data_of(text: &str) -> IniResult<CommandSetUpgradeModuleData> {
        let mut out = Vec::new();
        let _ = Ini::load(b"c.ini", format!("D\n{text}\nEnd\n").as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut out);
        out.remove(0)
    }

    fn mask(bits: &[i32]) -> UpgradeMaskType {
        make_upgrade_mask(bits).unwrap()
    }

    /// A world whose upgrade store has A..D (bits 0..3), team 1 of player 0,
    /// and one object carrying the module made from `text`.
    fn world_with(text: &str) -> (World, ObjectId) {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut center = UpgradeCenter::new();
        for n in ["A", "B", "C", "D"] {
            center.new_upgrade(&mut names, n.as_bytes()).unwrap();
        }
        let mut w = World::new();
        w.statics = Arc::new(GameStatics { upgrade_center: center, ..Default::default() });
        w.names = names;
        w.cuts.neutral_default_team = 1;
        w.teams.insert_team(crate::logic::world::team::Team::new(1, 0)).unwrap();
        w.cuts.team_players.insert(1, 0);
        let data = ModuleDataKind::CommandSetUpgrade(Arc::new(data_of(text).unwrap()));
        let m = crate::logic::modules::new_module(0, b"CommandSetUpgrade", b"ModuleTag_CS", &data).unwrap().unwrap();
        let tt = Arc::new(ThingTemplate::new(0));
        let id = w.new_object(NewObject { template: tt, status: Default::default(), team: None, modules: vec![m] }).unwrap();
        (w, id)
    }

    fn the_module(w: &World, id: ObjectId) -> u16 {
        w.find_module(id, b"CommandSetUpgrade").unwrap()
    }

    fn dirty_count(w: &World) -> usize {
        w.events
            .events()
            .iter()
            .filter(|e| matches!(e, crate::logic::client_event::ClientEvent::Object(ObjectClientEvent::ControlBarMarkUiDirty)))
            .count()
    }

    #[test]
    fn the_fields_and_defaults_are_read() {
        let d = data_of("").unwrap();
        assert_eq!((d.new_command_set.as_slice(), d.trigger_alt.as_slice()), (&b""[..], &b"none"[..]));
        let d = data_of("CommandSet = Set1\n CommandSetAlt = Set2\n TriggerAlt = B\n TriggeredBy = A").unwrap();
        assert_eq!(d.new_command_set, b"Set1");
        assert_eq!(d.new_command_set_alt, b"Set2");
        assert_eq!(d.trigger_alt, b"B");
        assert_eq!(d.upgrade.mux_data().activation_upgrade_names, vec![b"A".to_vec()]);
        // parseAsciiString takes an empty value as an empty name, as upstream's getNextAsciiString
        assert_eq!(data_of("CommandSet").unwrap().new_command_set, b"");
    }

    #[test]
    fn it_gives_the_command_set_once_and_marks_the_ui_dirty() {
        let (mut w, id) = world_with("CommandSet = Set1\n CommandSetAlt = Set2\n TriggerAlt = B\n TriggeredBy = A");
        let idx = the_module(&w, id);
        assert!(!w.module_call(id, idx, ModuleCall::AttemptUpgrade(mask(&[2]))).as_bool());
        assert!(w.module_call(id, idx, ModuleCall::AttemptUpgrade(mask(&[0]))).as_bool());
        assert_eq!(w.objects.find(id).unwrap().command_set_string_override, b"Set1");
        assert_eq!(dirty_count(&w), 1);
        // once only, until reset
        assert!(!w.module_call(id, idx, ModuleCall::AttemptUpgrade(mask(&[0]))).as_bool());
        assert!(w.module_call(id, idx, ModuleCall::IsAlreadyUpgraded).as_bool());
        assert!(w.module_call(id, idx, ModuleCall::ResetUpgrade(mask(&[0]))).as_bool());
        assert!(!w.module_call(id, idx, ModuleCall::IsAlreadyUpgraded).as_bool());
        assert!(!w.module_call(id, idx, ModuleCall::IsSubObjectsUpgrade).as_bool());
        assert!(w.cuts.module_errors.is_empty());
    }

    #[test]
    fn the_alternative_comes_from_the_player_or_else_the_object() {
        let text = "CommandSet = Set1\n CommandSetAlt = Set2\n TriggerAlt = B\n TriggeredBy = A";
        // the player has B
        let (mut w, id) = world_with(text);
        w.cuts.player_upgrades.insert(0, mask(&[1]));
        let idx = the_module(&w, id);
        assert!(w.module_call(id, idx, ModuleCall::AttemptUpgrade(mask(&[0]))).as_bool());
        assert_eq!(w.objects.find(id).unwrap().command_set_string_override, b"Set2");
        // the object has B
        let (mut w, id) = world_with(text);
        w.objects.find_mut(id).unwrap().object_upgrades_completed = mask(&[1]);
        let idx = the_module(&w, id);
        w.module_call(id, idx, ModuleCall::AttemptUpgrade(mask(&[0])));
        assert_eq!(w.objects.find(id).unwrap().command_set_string_override, b"Set2");
        // force refresh runs the implementation again, only once upgraded
        w.objects.find_mut(id).unwrap().object_upgrades_completed = UpgradeMaskType::default();
        w.module_call(id, idx, ModuleCall::ForceRefreshUpgrade);
        assert_eq!(w.objects.find(id).unwrap().command_set_string_override, b"Set1");
    }

    #[test]
    fn a_removal_reaches_this_module_at_once_and_the_fx_is_named() {
        // RemovesUpgrades = A resets this very module (removeUpgrade -> resetUpgrade on every upgrade module)
        let (mut w, id) = world_with("CommandSet = S\n TriggeredBy = A B\n RemovesUpgrades = A\n FXListUpgrade = FX_Up");
        w.cuts.trace = Some(Vec::new());
        w.objects.find_mut(id).unwrap().object_upgrades_completed = mask(&[0]);
        let idx = the_module(&w, id);
        assert!(w.module_call(id, idx, ModuleCall::AttemptUpgrade(mask(&[0]))).as_bool());
        assert!(w.cuts.reentrant_skips.is_empty(), "the reset was delivered, not skipped");
        assert_eq!(w.cuts.reentrant_delivered, 1);
        assert!(!w.objects.find(id).unwrap().object_upgrades_completed.any(), "A was removed from the object");
        assert!(w.module_call(id, idx, ModuleCall::IsAlreadyUpgraded).as_bool(), "executed is set after the removals");
        let trace = w.cuts.trace.clone().unwrap();
        assert!(trace.iter().any(|t| t == "remove A"));
        assert!(w.events.events().iter().any(|e| matches!(e,
            crate::logic::client_event::ClientEvent::Object(ObjectClientEvent::FxListDoFxObj { fx, .. }) if fx == b"FX_Up")));
    }

    #[test]
    fn a_bad_name_is_recorded_not_a_crash() {
        // initObject already asks the upgrade modules (updateUpgradeModules), which throws upstream
        let (mut w, id) = world_with("TriggeredBy = Nope");
        assert_eq!(w.cuts.module_errors.len(), 1);
        let idx = the_module(&w, id);
        assert!(!w.module_call(id, idx, ModuleCall::AttemptUpgrade(mask(&[0]))).as_bool());
        assert_eq!(w.cuts.module_errors.len(), 2);
    }

    #[test]
    fn the_factory_knows_it_and_the_save_is_the_whole_chain() {
        use crate::common::module_factory::{is_translated, translated_module};
        assert!(is_translated("CommandSetUpgrade"));
        let t = translated_module("CommandSetUpgrade").unwrap();
        assert_eq!(t.which_interfaces, Some(crate::common::module::MODULEINTERFACE_UPGRADE));
        let data = (t.create_data_proc.unwrap())(None).unwrap();
        assert!(matches!((t.create_proc.unwrap())(5, Some(data)).unwrap(), crate::common::module::Module::Upgrade(_)));
        // the module's xfer: its version (1), then UpgradeModule's version (1), BehaviorModule's chain, the mux
        let mut m = CommandSetUpgrade::new(5, Some(Arc::new(CommandSetUpgradeModuleData::default()))).unwrap();
        let mut base = UpgradeModule::new(5, Some(m.data.upgrade.clone())).unwrap();
        let mine = crate::logic::module::test_support::save_bytes(&mut m).unwrap();
        let theirs = crate::logic::module::test_support::save_bytes(&mut base).unwrap();
        assert_eq!(mine[0], 1);
        assert_eq!(&mine[1..], &theirs[..]);
        let mut body = UpdateBody::CommandSetUpgrade(m.clone());
        assert!(body.owns_base_chain());
        assert!(!UpdateBody::ModuleProbe(crate::logic::world::module_probe::ProbeModule::new(Default::default(), 0.0, Default::default())).owns_base_chain());
        let _ = &mut body;
    }
}
