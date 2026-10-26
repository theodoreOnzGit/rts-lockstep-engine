// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ProductionUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ProductionUpdate`: a factory's production queue. Units and upgrades are queued (paid for at once: a unit if the build assistant says it
//! can be made, an upgrade if its player can afford it and does not have it or have it in production, an object's upgrade if the object
//! would act on it and can produce it), built one at a time over their build time, and cancelled with a refund. A unit, finished, is made
//! through the object's exit interface, as many at once as a `QuantityModifier` names (redguards come four for one), each through a door it
//! reserves (a parking place behaviour may reserve it when the unit is queued); a building with door animations opens the door, holds it,
//! and makes the unit only once the door is open. A finished upgrade goes to the player or the object. The model conditions (actively
//! constructing, construction complete, the doors) are set in one go each update. The factory's death cancels and refunds everything.
//!
//! # Upstream facts kept
//!
//! - `update()` is always awake; it handles only the head of the queue, and a sold factory is frozen ("leave things in the queue").
//! - A unit the player may no longer build (`allowedToBuild`) is cancelled, unless it is a dozer.
//! - The percent complete is `frames / totalFrames * 100` in `Real`s; a unit finished but unable to exit is "overbuilt" and tried again
//!   each frame.
//! - `cancelUnitCreate(productionID)` matches any entry by id, a unit's or an upgrade's (an upgrade's id is `PRODUCTIONID_INVALID`, 0);
//!   `cancelAndRefundAllProduction` gives up after 100 passes.
//! - The model condition `ACTIVELY_CONSTRUCTING` is set or cleared by what the drawable shows now (the client's flags).
//! - The xfer saves the entries by template or upgrade name and finds them again on load (an unknown name throws `SC_INVALID_DATA`, as does a
//!   load into a non-empty queue); the doors are saved as raw bytes (each door twelve bytes and its hold flag, with three bytes of padding,
//!   zero); `m_specialPowerConstructionCommandButton` is not saved ("NO NEED TO SAVE DATA ON THIS FIELD").
//! - Unchecked upstream, so undefined here: a queue or a cancel by a factory with no controlling player (`Undefined::NullControllingPlayer`),
//!   `cancelUnitCreate` matching an upgrade's entry (it reads the upgrade as a thing template: `Undefined::TypeConfusion`), an exit door beyond
//!   the four doors' flags (`Undefined::ArrayIndexOutOfRange`), and a build time that is no `Int` (`Undefined::FloatToIntOutOfRange`).
//!
//! The player's money, upgrades and permissions, the thing templates' costs and build times, the build assistant's `canMakeUnit`, the
//! object's `canProduceUpgrade` and the client's sounds, messages and radar are the world's cut points: the player and the client are later
//! layers'.
//!
//! `p1w13-sonnet`, batch R76, 2026-10-26.

use std::sync::Arc;

use crate::common::disabled_types::{DisabledMaskType, DISABLED_HELD};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_DOZER;
use crate::common::model_state::{ModelConditionFlags, MODELCONDITION_ACTIVELY_CONSTRUCTING, MODELCONDITION_CONSTRUCTION_COMPLETE, MODELCONDITION_DOOR_1_OPENING};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::name_key_generator::NameKeyType;
use crate::common::object_status_types::OBJECT_STATUS_SOLD;
use crate::common::thing_template::ThingTemplateAudioType;
use crate::common::upgrade::{UpgradeType, UPGRADE_STATUS_COMPLETE, UPGRADE_STATUS_IN_PRODUCTION};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::module::update_module::{
    ExitDoorType, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, DOOR_COUNT_MAX, DOOR_NONE_AVAILABLE, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::PlayerIndex;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{EvaMessage, ObjectClientEvent};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `ProductionType`.
pub const PRODUCTION_INVALID: i32 = 0;
pub const PRODUCTION_UNIT: i32 = 1;
pub const PRODUCTION_UPGRADE: i32 = 2;
/// `PRODUCTIONID_INVALID`.
pub const PRODUCTIONID_INVALID: i32 = 0;
/// `CanMakeType`: `CANMAKE_OK`, `CANMAKE_QUEUE_FULL`, `CANMAKE_PARKING_PLACES_FULL`.
pub const CANMAKE_OK: i32 = 0;
pub const CANMAKE_QUEUE_FULL: i32 = 4;
pub const CANMAKE_PARKING_PLACES_FULL: i32 = 5;
/// `RADAR_EVENT_UPGRADE`.
const RADAR_EVENT_UPGRADE: i32 = 2;
/// `MAX_UPGRADE_CAMEO_UPGRADES`.
const MAX_UPGRADE_CAMEO_UPGRADES: usize = 5;

/// The door's model conditions (`theOpeningFlags`, `theClosingFlags`, `theWaitingOpenFlags`): door `i`'s opening is `DOOR_1_OPENING + 4i`,
/// then closing, waiting open, waiting to close.
fn opening_flag(i: usize) -> i32 {
    MODELCONDITION_DOOR_1_OPENING + 4 * i as i32
}
fn closing_flag(i: usize) -> i32 {
    opening_flag(i) + 1
}
fn waiting_open_flag(i: usize) -> i32 {
    opening_flag(i) + 2
}

/// `struct QuantityModifier`.
#[derive(Clone, Debug, PartialEq)]
pub struct QuantityModifier {
    pub template_name: Vec<u8>,
    pub quantity: i32,
}

/// `class ProductionUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductionUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_numDoorAnimations`.
    pub num_door_animations: i32,
    /// `m_doorOpeningTime`, `m_doorWaitOpenTime`, `m_doorClosingTime`, `m_constructionCompleteDuration`.
    pub door_opening_time: u32,
    pub door_wait_open_time: u32,
    pub door_closing_time: u32,
    pub construction_complete_duration: u32,
    /// `m_quantityModifiers`.
    pub quantity_modifiers: Vec<QuantityModifier>,
    /// `m_maxQueueEntries`.
    pub max_queue_entries: i32,
    /// `m_disabledTypesToProcess`.
    pub disabled_types_to_process: DisabledMaskType,
}

impl Default for ProductionUpdateModuleData {
    /// `ProductionUpdateModuleData()`.
    fn default() -> Self {
        let mut disabled = DisabledMaskType::default();
        disabled.set(DISABLED_HELD, 1);
        ProductionUpdateModuleData {
            update: UpdateModuleData::new(),
            num_door_animations: 0,
            door_opening_time: 0,
            door_wait_open_time: 0,
            door_closing_time: 0,
            construction_complete_duration: 0,
            quantity_modifiers: Vec::new(),
            max_queue_entries: 9,
            disabled_types_to_process: disabled,
        }
    }
}

type D = ProductionUpdateModuleData;

/// `parseAppendQuantityModifier`: a template name and an optional count (1).
fn parse_append_quantity_modifier(ini: &mut Ini<'_>, d: &mut D, _: &UserData) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let count = match ini.get_next_token_or_null(None) {
        Some(s) => Ini::scan_int(&s)?,
        None => 1,
    };
    d.quantity_modifiers.push(QuantityModifier { template_name: name, quantity: count });
    Ok(())
}

static FIELD_PARSE: [FieldParse<D>; 9] = [
    FieldParse::new("MaxQueueEntries", |i, d: &mut D, _| { d.max_queue_entries = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("NumDoorAnimations", |i, d: &mut D, _| { d.num_door_animations = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DoorOpeningTime", |i, d: &mut D, _| { d.door_opening_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DoorWaitOpenTime", |i, d: &mut D, _| { d.door_wait_open_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DoorCloseTime", |i, d: &mut D, _| { d.door_closing_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new(
        "ConstructionCompleteDuration",
        |i, d: &mut D, _| { d.construction_complete_duration = i.parse_duration_unsigned_int()?; Ok(()) },
        UserData::None,
    ),
    FieldParse::new("QuantityModifier", parse_append_quantity_modifier, UserData::None),
    FieldParse::new("DisabledTypesToProcess", |i, d: &mut D, _| d.disabled_types_to_process.parse_from_ini(i), UserData::None),
    FieldParse::end(),
];

impl ProductionUpdateModuleData {
    /// `buildFieldParse(p)`: `UpdateModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, D>) -> IniResult<()> {
        crate::logic::module::behavior_module::BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<D> {
        let mut data = D::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, D::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `ProductionUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ProductionUpdate(Arc::new(D::friend_new_module_data(ini)?)))
}

/// `class ProductionEntry`: what is produced (a unit's thing template or an upgrade, by its name key), its id, its progress, how many it
/// makes and has made, and its reserved door.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductionEntry {
    /// `m_type`.
    pub production_type: i32,
    /// `m_objectToProduce` (a thing template's id), `m_upgradeToResearch` (an upgrade's name key): one of them, as the type says.
    pub object_to_produce: Option<u16>,
    pub upgrade_to_research: Option<NameKeyType>,
    /// `m_productionID`, `m_percentComplete`, `m_framesUnderConstruction`.
    pub production_id: i32,
    pub percent_complete: f32,
    pub frames_under_construction: i32,
    /// `m_productionQuantityTotal`, `m_productionQuantityProduced`, `m_exitDoor`.
    pub quantity_total: i32,
    pub quantity_produced: i32,
    pub exit_door: ExitDoorType,
    /// The template's or upgrade's name, which the xfer saves (`getName()`, `getUpgradeName()`).
    pub name: Vec<u8>,
}

impl Default for ProductionEntry {
    /// `ProductionEntry()` (the door is the allocator's zero: the constructor leaves it).
    fn default() -> Self {
        ProductionEntry {
            production_type: PRODUCTION_INVALID,
            object_to_produce: None,
            upgrade_to_research: None,
            production_id: 1,
            percent_complete: 0.0,
            frames_under_construction: 0,
            quantity_total: 0,
            quantity_produced: 0,
            exit_door: 0,
            name: Vec::new(),
        }
    }
}

impl ProductionEntry {
    /// `getProductionQuantityRemaining()`.
    pub fn quantity_remaining(&self) -> i32 {
        self.quantity_total.wrapping_sub(self.quantity_produced)
    }
}

/// `struct DoorInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct DoorInfo {
    door_opened_frame: u32,
    door_wait_open_frame: u32,
    door_closed_frame: u32,
    hold_open: bool,
}

/// `class ProductionUpdate : public UpdateModule, public ProductionUpdateInterface, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct ProductionUpdate {
    base: UpdateModule,
    data: Arc<D>,
    /// `m_specialPowerConstructionCommandButton` (a command button's name; not saved).
    special_power_construction_command_button: Option<Vec<u8>>,
    /// `m_productionQueue`..`m_productionQueueTail`, in order.
    queue: Vec<ProductionEntry>,
    /// `m_uniqueID`, `m_productionCount`, `m_constructionCompleteFrame`.
    unique_id: i32,
    production_count: u32,
    construction_complete_frame: u32,
    /// `m_doors`.
    doors: [DoorInfo; DOOR_COUNT_MAX as usize],
    /// `m_clearFlags`, `m_setFlags`, `m_flagsDirty`.
    clear_flags: ModelConditionFlags,
    set_flags: ModelConditionFlags,
    flags_dirty: bool,
    /// The stores' names, staged by the world before a load (the xfer finds each entry's template or upgrade by name).
    load_names: Option<LoadNames>,
}

/// The thing factory's template names and the upgrade center's upgrade names, for a load.
#[derive(Clone, Debug, Default)]
pub struct LoadNames {
    templates: Vec<(Vec<u8>, u16)>,
    upgrades: Vec<(Vec<u8>, NameKeyType)>,
}

impl PartialEq for ProductionUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base.behavior().object() == other.base.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl ProductionUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s and the die interface.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The constructor (it touches no world): an empty queue, the ids from 1, the doors closed, no flags (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<D>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let mut base = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        base.set_disabled_types_to_process(data.disabled_types_to_process);
        Ok(ProductionUpdate {
            base,
            data,
            special_power_construction_command_button: None,
            queue: Vec::new(),
            unique_id: 1,
            production_count: 0,
            construction_complete_frame: 0,
            doors: [DoorInfo::default(); DOOR_COUNT_MAX as usize],
            clear_flags: ModelConditionFlags::default(),
            set_flags: ModelConditionFlags::default(),
            flags_dirty: false,
            load_names: None,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.base
    }

    pub fn module_data(&self) -> &Arc<D> {
        &self.data
    }

    /// `firstProduction()` .. `nextProduction()`: the queue, in order.
    pub fn queue(&self) -> &[ProductionEntry] {
        &self.queue
    }

    /// The counters, the doors, the flags and each entry.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = vec![self.unique_id as u32, self.production_count, self.construction_complete_frame, u32::from(self.flags_dirty)];
        for d in &self.doors {
            v.extend([d.door_opened_frame, d.door_wait_open_frame, d.door_closed_frame, u32::from(d.hold_open)]);
        }
        for f in [&self.clear_flags, &self.set_flags] {
            v.extend((0..crate::common::model_state::MODELCONDITION_COUNT).filter(|&i| f.test(i)).map(|i| i as u32));
            v.push(u32::MAX);
        }
        v.push(self.queue.len() as u32);
        for e in &self.queue {
            v.extend([
                e.production_type as u32,
                e.object_to_produce.map_or(u32::MAX, u32::from),
                e.upgrade_to_research.map_or(u32::MAX, |k| k.0),
                e.production_id as u32,
                e.percent_complete.to_bits(),
                e.frames_under_construction as u32,
                e.quantity_total as u32,
                e.quantity_produced as u32,
                e.exit_door as u32,
            ]);
        }
        v
    }

    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The world's load, before the xfer: the stores' names (`TheThingFactory->findTemplate(name)`, `TheUpgradeCenter->findUpgrade(name)`).
    pub fn stage_load_names(&mut self, world: &World) {
        let templates = world.statics.thing_factory.templates().map(|t| (t.data.get_name().to_vec(), t.data.get_template_id())).collect();
        let upgrades = world.statics.upgrade_center.templates().map(|u| (u.upgrade_name().to_vec(), u.upgrade_name_key())).collect();
        self.load_names = Some(LoadNames { templates, upgrades });
    }

    fn set_flag(f: &mut ModelConditionFlags, flag: i32, on: bool) {
        f.set(flag, i32::from(on));
    }

    fn controlling_player(world: &World, us: ObjectId) -> Result<PlayerIndex, IniError> {
        world.get_controlling_player(us).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))
    }

    /// The upgrade's template (`None`: no upgrade of that key).
    fn upgrade(world: &World, key: NameKeyType) -> Option<&crate::common::upgrade::UpgradeTemplate> {
        world.statics.upgrade_center.find_upgrade_by_key(key)
    }

    /// `canQueueUpgrade(upgrade)`.
    fn can_queue_upgrade(&self) -> i32 {
        if self.production_count >= self.data.max_queue_entries as u32 && !crate::logic::modules::plant(2480) {
            return CANMAKE_QUEUE_FULL;
        }
        CANMAKE_OK
    }

    /// `canQueueCreateUnit(unitType)`: a parking place behaviour that reserves a door when queued and has no space says the parking places
    /// are full; else the queue's size.
    fn can_queue_create_unit(&self, world: &mut World, us: ObjectId, template: u16) -> i32 {
        for idx in world.all_modules(us) {
            if world.module_call(us, idx, ModuleCall::HasParkingPlaceInterface).as_bool()
                && world.module_call(us, idx, ModuleCall::ParkingPlaceShouldReserveDoorWhenQueued { template }).as_bool()
                && !world.module_call(us, idx, ModuleCall::ParkingPlaceHasAvailableSpaceFor { template }).as_bool()
            {
                return CANMAKE_PARKING_PLACES_FULL;
            }
        }
        if self.production_count >= self.data.max_queue_entries as u32 {
            return CANMAKE_QUEUE_FULL;
        }
        CANMAKE_OK
    }

    /// `queueUpgrade(upgrade)`: paid for and queued, unless the player cannot afford it, the object has it or would not act on it, it is
    /// queued here already, the object cannot produce it, the player has it or is producing it, or the queue is full.
    fn queue_upgrade(&mut self, world: &mut World, us: ObjectId, key: Option<NameKeyType>) -> Result<bool, IniError> {
        let Some(key) = key else { return Ok(false) };
        let Some(u) = Self::upgrade(world, key) else { return Ok(false) };
        let (utype, mask, cost) = (u.upgrade_type(), u.upgrade_mask(), u.cost_to_build());
        let player = world.get_controlling_player(us);
        if utype == UpgradeType::Player && !world.upgrade_center_can_afford_upgrade(player, cost) {
            return Ok(false);
        }
        if utype == UpgradeType::Object && (world.has_upgrade(us, &mask) || !world.affected_by_upgrade(us, &mask)) {
            return Ok(false);
        }
        // "you cannot queue the production of an upgrade twice in this queue"
        if self.is_upgrade_in_queue(key) {
            return Ok(false);
        }
        // "STOP cheaters by making sure they can actually build this"
        if !world.object_can_produce_upgrade(us, key.0) {
            return Ok(false);
        }
        if utype == UpgradeType::Player {
            let p = player.ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
            if world.player_has_upgrade_complete(p, &mask) || world.player_has_upgrade_in_production(p, &mask) {
                return Ok(false);
            }
        }
        if self.production_count >= self.data.max_queue_entries as u32 {
            return Ok(false);
        }
        let p = player.ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        // "take the cost for the build away from the player"
        world.cut(CutCall::PlayerMoneyWithdraw { player: p, amount: cost as u32 });
        let name = Self::upgrade(world, key).map(|u| u.upgrade_name().to_vec()).unwrap_or_default();
        let entry = ProductionEntry {
            production_type: PRODUCTION_UPGRADE,
            upgrade_to_research: Some(key),
            production_id: PRODUCTIONID_INVALID,
            name,
            ..Default::default()
        };
        self.add_to_production_queue(world, us, entry);
        // "add this upgrade as in progress in the player"
        world.player_add_upgrade(p, key.0, &mask, UPGRADE_STATUS_IN_PRODUCTION);
        Ok(true)
    }

    /// `cancelUpgrade(upgrade)`: the entry refunded and removed, and a player's upgrade no longer in production.
    fn cancel_upgrade(&mut self, world: &mut World, us: ObjectId, key: Option<NameKeyType>) -> Result<(), IniError> {
        let Some(key) = key else { return Ok(()) };
        let Some(u) = Self::upgrade(world, key) else { return Ok(()) };
        let (utype, mask, cost) = (u.upgrade_type(), u.upgrade_mask(), u.cost_to_build());
        let player = world.get_controlling_player(us);
        if utype == UpgradeType::Player {
            let p = player.ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
            if !world.player_has_upgrade_in_production(p, &mask) {
                return Ok(());
            }
        }
        let Some(i) = self.queue.iter().position(|e| e.production_type == PRODUCTION_UPGRADE && e.upgrade_to_research == Some(key)) else {
            return Ok(());
        };
        let p = player.ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        world.cut(CutCall::PlayerMoneyDeposit { player: p, amount: cost as u32 });
        self.remove_from_production_queue(world, us, i);
        if utype == UpgradeType::Player && !crate::logic::modules::plant(2481) {
            world.player_remove_upgrade(p, key.0, &mask);
        }
        Ok(())
    }

    /// `queueCreateUnit(unitType, productionID)`: paid for and queued (with a reserved door, for a parking place behaviour that wants one),
    /// unless the build assistant says it cannot be made, no door could be reserved, or the queue is full.
    fn queue_create_unit(&mut self, world: &mut World, us: ObjectId, template: Option<u16>, production_id: i32) -> Result<bool, IniError> {
        let Some(template) = template else { return Err(IniError::undefined(Undefined::NullObject)) };
        if world.build_assistant_can_make_unit(us, template) != CANMAKE_OK {
            return Ok(false);
        }
        let mut exit_door = DOOR_NONE_AVAILABLE;
        for idx in world.all_modules(us) {
            if world.module_call(us, idx, ModuleCall::HasParkingPlaceInterface).as_bool()
                && world.module_call(us, idx, ModuleCall::ParkingPlaceShouldReserveDoorWhenQueued { template }).as_bool()
            {
                if let Some(exit) = world.get_object_exit_interface(us) {
                    exit_door = world.module_call(us, exit, ModuleCall::ReserveDoorForExit { template: Some(template), specific: 0 }).as_int(DOOR_NONE_AVAILABLE);
                }
                if exit_door == DOOR_NONE_AVAILABLE {
                    return Ok(false);
                }
                break;
            }
        }
        if self.production_count >= self.data.max_queue_entries as u32 {
            return Ok(false);
        }
        let p = Self::controlling_player(world, us)?;
        let cost = world.template_calc_cost_to_build(template, p);
        world.cut(CutCall::PlayerMoneyWithdraw { player: p, amount: cost as u32 });
        // "Check to see if there is a quantity modifier entry ... they are built 4 at a time"
        let mut quantity_total = 1;
        for m in self.data.quantity_modifiers.clone() {
            let found = world.statics.thing_factory.find_template(&m.template_name).map(|t| t.data.get_template_id());
            if found.is_some_and(|t| world.thing_template_is_equivalent_to(t, template)) {
                quantity_total = m.quantity;
                break;
            }
        }
        let name = world.statics.thing_factory.find_by_template_id(template).map(|t| t.data.get_name().to_vec()).unwrap_or_default();
        let entry = ProductionEntry {
            production_type: PRODUCTION_UNIT,
            object_to_produce: Some(template),
            production_id,
            quantity_total,
            quantity_produced: 0,
            exit_door,
            name,
            ..Default::default()
        };
        self.add_to_production_queue(world, us, entry);
        Ok(true)
    }

    /// `cancelUnitCreate(productionID)`: the first entry of that id refunded and removed.
    fn cancel_unit_create(&mut self, world: &mut World, us: ObjectId, production_id: i32) -> Result<(), IniError> {
        let Some(i) = self.queue.iter().position(|e| e.production_id == production_id) else { return Ok(()) };
        let Some(template) = self.queue[i].object_to_produce else {
            // (an upgrade's entry, read as a thing template)
            return Err(IniError::undefined(Undefined::TypeConfusion));
        };
        let p = Self::controlling_player(world, us)?;
        let cost = world.template_calc_cost_to_build(template, p);
        world.cut(CutCall::PlayerMoneyDeposit { player: p, amount: cost as u32 });
        self.remove_from_production_queue(world, us, i);
        Ok(())
    }

    /// `cancelAllUnitsOfType(unitType)`. (`cancelUnitCreate` removes the first entry of the id, this one or one before it: the walk goes on
    /// from the entry after this one, which has moved into its place.)
    fn cancel_all_units_of_type(&mut self, world: &mut World, us: ObjectId, template: Option<u16>) -> Result<(), IniError> {
        let mut i = 0;
        while i < self.queue.len() {
            let e = &self.queue[i];
            if e.production_type == PRODUCTION_UNIT && e.object_to_produce == template && !crate::logic::modules::plant(2486) {
                let id = e.production_id;
                self.cancel_unit_create(world, us, id)?;
            } else {
                i += 1;
            }
        }
        Ok(())
    }

    /// `updateDoors()`: an opened door waits open, a waiting door closes (unless held), a closing door closes, each after its time.
    fn update_doors(&mut self, world: &World) {
        let d = self.data.clone();
        let now = world.frame();
        for i in 0..DOOR_COUNT_MAX as usize {
            let door = &mut self.doors[i];
            if door.door_opened_frame != 0 {
                if now.wrapping_sub(door.door_opened_frame) > d.door_opening_time {
                    door.door_opened_frame = 0;
                    door.door_wait_open_frame = now;
                    Self::set_flag(&mut self.clear_flags, opening_flag(i), true);
                    Self::set_flag(&mut self.set_flags, opening_flag(i), false);
                    Self::set_flag(&mut self.set_flags, waiting_open_flag(i), true);
                    self.flags_dirty = true;
                }
            } else if door.door_wait_open_frame != 0 {
                if now.wrapping_sub(door.door_wait_open_frame) > d.door_wait_open_time && !door.hold_open {
                    door.door_wait_open_frame = 0;
                    door.door_closed_frame = now;
                    Self::set_flag(&mut self.clear_flags, waiting_open_flag(i), true);
                    Self::set_flag(&mut self.set_flags, waiting_open_flag(i), false);
                    Self::set_flag(&mut self.set_flags, closing_flag(i), true);
                    self.flags_dirty = true;
                }
            } else if door.door_closed_frame != 0 && !door.hold_open && now.wrapping_sub(door.door_closed_frame) > d.door_closing_time {
                door.door_closed_frame = 0;
                Self::set_flag(&mut self.clear_flags, closing_flag(i), true);
                Self::set_flag(&mut self.set_flags, closing_flag(i), false);
                self.flags_dirty = true;
            }
        }
    }

    /// The flags of a mask, in index order.
    fn flag_list(f: &ModelConditionFlags) -> Vec<i32> {
        (0..crate::common::model_state::MODELCONDITION_COUNT).filter(|&i| f.test(i)).collect()
    }

    /// `update()`'s body; an error is undefined upstream.
    fn do_update(&mut self, world: &mut World, us: ObjectId) -> Result<UpdateSleepTime, IniError> {
        let d = self.data.clone();
        let now = world.frame();
        if d.num_door_animations > 0 {
            self.update_doors(world);
        }
        // "if we're in the construction complete state ... get out of it when the time is up"
        if self.construction_complete_frame != 0 && now.wrapping_sub(self.construction_complete_frame) > d.construction_complete_duration {
            self.construction_complete_frame = 0;
            Self::set_flag(&mut self.clear_flags, MODELCONDITION_CONSTRUCTION_COMPLETE, true);
            Self::set_flag(&mut self.set_flags, MODELCONDITION_CONSTRUCTION_COMPLETE, false);
            self.flags_dirty = true;
        }
        // "if we have dirty bits that need to be set and cleared, do it here all at once"
        if self.flags_dirty {
            if world.has_drawable(us) {
                let (clear, set) = (Self::flag_list(&self.clear_flags), Self::flag_list(&self.set_flags));
                world.clear_and_set_model_condition_flags(us, &clear, &set);
            }
            self.clear_flags = ModelConditionFlags::default();
            self.set_flags = ModelConditionFlags::default();
            self.flags_dirty = false;
        }
        // "if nothing in the queue get outta here"
        if self.queue.is_empty() {
            return Ok(UPDATE_SLEEP_NONE);
        }
        // "if we've become OBJECT_STATUS_SOLD, halt all production"
        if world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_SOLD)) {
            return Ok(UPDATE_SLEEP_NONE);
        }
        let Some(player) = world.get_controlling_player(us) else {
            self.remove_from_production_queue(world, us, 0);
            return Ok(UPDATE_SLEEP_NONE);
        };
        // "you can disallow types of units on the fly in scripts"
        let head = self.queue[0].clone();
        if head.production_type == PRODUCTION_UNIT {
            let t = head.object_to_produce.ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
            if !world.player_allowed_to_build(player, t) {
                // "Don't cancel dozers in the queue."
                let dozer = world.statics.thing_factory.find_by_template_id(t).is_some_and(|x| x.data.is_kind_of(KINDOF_DOZER));
                if !dozer {
                    self.cancel_unit_create(world, us, head.production_id)?;
                    return Ok(UPDATE_SLEEP_NONE);
                }
            }
        }
        self.queue[0].frames_under_construction = self.queue[0].frames_under_construction.wrapping_add(1);
        let total_production_frames = if head.production_type == PRODUCTION_UNIT {
            world.template_calc_time_to_build(head.object_to_produce.ok_or_else(|| IniError::undefined(Undefined::NullObject))?, player)
        } else {
            let u = head.upgrade_to_research.and_then(|k| Self::upgrade(world, k)).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
            u.time_to_build_frames()?
        };
        let e = &mut self.queue[0];
        e.percent_complete = e.frames_under_construction as f32 / total_production_frames as f32 * 100.0;
        if e.percent_complete < 100.0 && !crate::logic::modules::plant(2482) {
            return Ok(UPDATE_SLEEP_NONE);
        }
        if head.production_type == PRODUCTION_UNIT {
            self.finish_unit(world, us, player, now)?;
        } else if head.production_type == PRODUCTION_UPGRADE {
            self.finish_upgrade(world, us, player)?;
        }
        Ok(UPDATE_SLEEP_NONE)
    }

    /// A finished unit's entry: as many made as remain and can exit (each through a reserved door, opened first where the doors are
    /// animated), the entry removed once none remain (or at once without an exit interface).
    fn finish_unit(&mut self, world: &mut World, us: ObjectId, player: PlayerIndex, now: u32) -> Result<(), IniError> {
        let d = self.data.clone();
        let Some(exit) = world.get_object_exit_interface(us) else {
            // "there is no exit interface, this is an error"
            self.remove_from_production_queue(world, us, 0);
            return Ok(());
        };
        let template = self.queue[0].object_to_produce.ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        // "this will change midloop, so it can't be the For clause"
        let number_to_try = self.queue[0].quantity_remaining();
        for _ in 0..number_to_try {
            let mut exit_door = self.queue[0].exit_door;
            if exit_door == DOOR_NONE_AVAILABLE {
                exit_door = world.module_call(us, exit, ModuleCall::ReserveDoorForExit { template: Some(template), specific: 0 }).as_int(DOOR_NONE_AVAILABLE);
                self.queue[0].exit_door = exit_door;
            }
            if exit_door == DOOR_NONE_AVAILABLE {
                continue;
            }
            // "note, could be DOOR_NONE_NEEDED! so door could be null."
            let door = (0..DOOR_COUNT_MAX).contains(&exit_door).then_some(exit_door as usize);
            if d.num_door_animations > 0 {
                if let Some(i) = door {
                    let dr = &mut self.doors[i];
                    if dr.door_opened_frame == 0 && dr.door_wait_open_frame == 0 && dr.door_closed_frame == 0 {
                        // "if the door is closed, open it"
                        dr.door_opened_frame = now;
                        Self::set_flag(&mut self.set_flags, opening_flag(i), true);
                        self.flags_dirty = true;
                    } else if dr.door_wait_open_frame != 0 {
                        // "if the door is waiting-open, keep it there"
                        dr.door_wait_open_frame = now;
                    } else if dr.door_closed_frame != 0 {
                        // "if the door is closing, for now, pop it to waiting open"
                        dr.door_wait_open_frame = now;
                        Self::set_flag(&mut self.clear_flags, opening_flag(i), true);
                        Self::set_flag(&mut self.clear_flags, closing_flag(i), true);
                        Self::set_flag(&mut self.set_flags, opening_flag(i), false);
                        Self::set_flag(&mut self.set_flags, closing_flag(i), false);
                        Self::set_flag(&mut self.set_flags, waiting_open_flag(i), true);
                        self.flags_dirty = true;
                    }
                }
            }
            // "we now go into the construction complete condition for a while"
            if self.construction_complete_frame == 0 {
                self.construction_complete_frame = now;
                Self::set_flag(&mut self.set_flags, MODELCONDITION_CONSTRUCTION_COMPLETE, true);
                self.flags_dirty = true;
            }
            let door_open = door.is_none_or(|i| self.doors[i].door_wait_open_frame != 0);
            if d.num_door_animations == 0 || door_open {
                self.make_unit(world, us, player, exit, template, exit_door)?;
            }
        }
        if self.queue[0].quantity_remaining() == 0 {
            // "remove this production entry so we can go on to the next if we are totally finished"
            self.remove_from_production_queue(world, us, 0);
        }
        Ok(())
    }

    /// One unit made: on the player's default team, its producer the factory, out through the door; its voice, the player's
    /// `onUnitCreated`, its `onBuildComplete`s, the first one's `VoiceCreate`, the academy's record, and one more produced.
    fn make_unit(&mut self, world: &mut World, us: ObjectId, player: PlayerIndex, exit: u16, template: u16, exit_door: ExitDoorType) -> Result<(), IniError> {
        let team = world.cuts.player_default_team(player);
        // (`newObject` throws on failure; the world's errors are not upstream's)
        let new_obj = world.thing_factory_new_object(template, Some(team), Default::default()).map_err(|_| IniError::new(IniErrorKind::BadArg))?;
        world.set_producer(new_obj, us);
        // "call the exit interface to do the rally point and position stuff"
        world.module_call(us, exit, ModuleCall::ExitObjectViaDoor { obj: new_obj, door: exit_door });
        // "since we successfully exited via this door, we should NOT call unreserveDoorForExit"
        self.queue[0].exit_door = DOOR_NONE_AVAILABLE;
        // "Now, play the sound associated with the new object's production."
        let voice = world.objects.find(new_obj).map(|o| o.template().get_audio(ThingTemplateAudioType::VoiceCreated).event_name.clone()).unwrap_or_default();
        world.audio_add_event(voice, new_obj);
        world.player_on_unit_created(player, us, new_obj);
        for idx in world.all_modules(new_obj) {
            world.module_call(new_obj, idx, ModuleCall::OnBuildComplete);
        }
        let e = &self.queue[0];
        if e.quantity_total == e.quantity_remaining() && !crate::logic::modules::plant(2483) {
            // "Call the voice created for the 1st object"
            let sound = world.objects.find(new_obj).map(|o| o.template().get_per_unit_sound(b"VoiceCreate").event_name.clone()).unwrap_or_default();
            world.audio_add_event(sound, new_obj);
        }
        world.cut(CutCall::PlayerAcademyRecordProduction { player, object: new_obj, constructor: us });
        // "We created one guy, but we may want to do more so we should stay in this node of production."
        let e = &mut self.queue[0];
        e.quantity_produced = e.quantity_produced.wrapping_add(1);
        e.exit_door = DOOR_NONE_AVAILABLE;
        Ok(())
    }

    /// A finished upgrade's entry: the money spent scored, the script engine told, the local player's message, radar event and sounds, the
    /// upgrade given to the player or the object, the academy's record, the control bar marked for a selected object that shows it, and the
    /// entry removed.
    fn finish_upgrade(&mut self, world: &mut World, us: ObjectId, player: PlayerIndex) -> Result<(), IniError> {
        let key = self.queue[0].upgrade_to_research.ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        let u = Self::upgrade(world, key).ok_or_else(|| IniError::undefined(Undefined::NullObject))?.clone();
        world.cut(CutCall::PlayerScoreAddMoneySpent { player, amount: u.cost_to_build() });
        world.cut(CutCall::ScriptNotifyOfCompletedUpgrade { player, name: u.upgrade_name().to_vec(), source: us });
        if world.is_locally_controlled(us) && !u.display_name_label().is_empty() {
            world.emit(ObjectClientEvent::UiMessageFormatted { format: b"UPGRADE:UpgradeComplete".to_vec(), arg: u.display_name_label().to_vec() });
            // "upgrades are a more rare event, play a nifty radar event thingie"
            let pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
            world.emit(ObjectClientEvent::RadarCreateEvent { position: pos, event_type: RADAR_EVENT_UPGRADE });
            let sound = u.research_complete_sound().event_name.clone();
            if world.audio_is_valid_event(&sound) {
                // "We have a custom upgrade complete sound."
                world.audio_add_event(sound, us);
            } else {
                // "Use a generic EVA event."
                world.emit(ObjectClientEvent::Eva { message: EvaMessage::UpgradeComplete });
            }
            // "Play any available unit specific sound for upgrade."
            world.audio_add_event(u.unit_specific_sound().event_name.clone(), us);
        }
        // "update the upgrade status in the player or give the upgrade to the object"
        if u.upgrade_type() == UpgradeType::Player {
            world.player_add_upgrade(player, key.0, &u.upgrade_mask(), UPGRADE_STATUS_COMPLETE);
        } else if !crate::logic::modules::plant(2484) {
            world.give_upgrade(us, Some(&u.upgrade_mask()));
        }
        world.cut(CutCall::PlayerAcademyRecordUpgrade { player, upgrade: key.0, granted: false });
        // "Also mark the UI dirty -- incase object with upgrade cameo is selected."
        if let Some(selected) = world.reads.first_selected() {
            let names: Vec<Vec<u8>> = match world.objects.find(selected) {
                Some(o) => (0..MAX_UPGRADE_CAMEO_UPGRADES).map(|i| o.template().get_upgrade_cameo_name(i).to_vec()).collect(),
                None => Vec::new(),
            };
            for name in names {
                let found = world.statics.upgrade_center.templates().find(|t| t.upgrade_name() == &name[..]).map(|t| t.upgrade_name_key());
                if found == Some(key) {
                    world.emit(ObjectClientEvent::ControlBarMarkUiDirty);
                    break;
                }
            }
        }
        self.remove_from_production_queue(world, us, 0);
        Ok(())
    }

    /// `addToProductionQueue(production)`: at the end; the actively constructing condition set, if the drawable does not show it.
    fn add_to_production_queue(&mut self, world: &World, us: ObjectId, entry: ProductionEntry) {
        self.queue.push(entry);
        self.production_count = self.production_count.wrapping_add(1);
        if world.has_drawable(us) && !world.reads.drawable_model_condition_flags(us).test(MODELCONDITION_ACTIVELY_CONSTRUCTING) {
            Self::set_flag(&mut self.set_flags, MODELCONDITION_ACTIVELY_CONSTRUCTING, true);
            self.flags_dirty = true;
        }
    }

    /// `removeFromProductionQueue(production)`: a unit's reserved door given back, the entry unlinked, and the actively constructing
    /// condition cleared when the queue is empty and the drawable shows it.
    fn remove_from_production_queue(&mut self, world: &mut World, us: ObjectId, i: usize) {
        let e = &self.queue[i];
        if e.production_type == PRODUCTION_UNIT && e.exit_door != DOOR_NONE_AVAILABLE {
            if let Some(exit) = world.get_object_exit_interface(us) {
                world.module_call(us, exit, ModuleCall::UnreserveDoorForExit(e.exit_door));
            }
        }
        self.queue.remove(i);
        self.production_count = self.production_count.wrapping_sub(1);
        if world.has_drawable(us)
            && self.production_count == 0
            && world.reads.drawable_model_condition_flags(us).test(MODELCONDITION_ACTIVELY_CONSTRUCTING)
            && !crate::logic::modules::plant(2485)
        {
            Self::set_flag(&mut self.clear_flags, MODELCONDITION_ACTIVELY_CONSTRUCTING, true);
            Self::set_flag(&mut self.set_flags, MODELCONDITION_ACTIVELY_CONSTRUCTING, false);
            self.flags_dirty = true;
        }
    }

    /// `isUpgradeInQueue(upgrade)`.
    fn is_upgrade_in_queue(&self, key: NameKeyType) -> bool {
        self.queue.iter().any(|e| e.production_type == PRODUCTION_UPGRADE && e.upgrade_to_research == Some(key))
    }

    /// `countUnitTypeInQueue(unitType)`.
    fn count_unit_type_in_queue(&self, template: Option<u16>) -> u32 {
        self.queue.iter().filter(|e| e.production_type == PRODUCTION_UNIT && e.object_to_produce == template).count() as u32
    }

    /// `cancelAndRefundAllProduction()`: the head cancelled, 100 times at most ("Empirically, in release the code can loop forever").
    fn cancel_and_refund_all_production(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        for _ in 0..100 {
            let Some(head) = self.queue.first().cloned() else { continue };
            match head.production_type {
                PRODUCTION_UNIT => self.cancel_unit_create(world, us, head.production_id)?,
                PRODUCTION_UPGRADE => self.cancel_upgrade(world, us, head.upgrade_to_research)?,
                // "unknown production type"
                _ => return Ok(()),
            }
        }
        Ok(())
    }

    /// `setHoldDoorOpen(exitDoor, holdIt)`: a door held opens if it is closed.
    fn set_hold_door_open(&mut self, world: &World, exit_door: ExitDoorType, hold: bool) {
        if !(0..DOOR_COUNT_MAX).contains(&exit_door) {
            return;
        }
        let i = exit_door as usize;
        let door = &mut self.doors[i];
        door.hold_open = hold;
        if hold && door.door_opened_frame == 0 && door.door_wait_open_frame == 0 && door.door_closed_frame == 0 {
            door.door_opened_frame = world.frame();
            Self::set_flag(&mut self.set_flags, opening_flag(i), true);
            self.flags_dirty = true;
        }
    }

    fn record<T>(world: &mut World, me: ModuleHandle, r: Result<T, IniError>) -> Option<T> {
        match r {
            Ok(t) => Some(t),
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                None
            }
        }
    }

    /// The doors' raw bytes (`xferUser(&m_doors, sizeof(DoorInfo) * DOOR_COUNT_MAX)`): three words, the hold flag and three bytes of zero
    /// padding each.
    fn doors_bytes(&self) -> [u8; 16 * DOOR_COUNT_MAX as usize] {
        let mut b = [0u8; 16 * DOOR_COUNT_MAX as usize];
        for (i, d) in self.doors.iter().enumerate() {
            b[16 * i..16 * i + 4].copy_from_slice(&d.door_opened_frame.to_le_bytes());
            b[16 * i + 4..16 * i + 8].copy_from_slice(&d.door_wait_open_frame.to_le_bytes());
            b[16 * i + 8..16 * i + 12].copy_from_slice(&d.door_closed_frame.to_le_bytes());
            b[16 * i + 12] = u8::from(d.hold_open);
        }
        b
    }
}

impl UpdateBehavior for ProductionUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.base
    }
    /// `update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let r = self.do_update(world, me.object);
        Self::record(world, me, r).unwrap_or(UPDATE_SLEEP_NONE)
    }
    /// The entries (each its type, its template's or upgrade's name, its id, progress, quantities and door), the unique id, the count, the
    /// construction complete frame, the doors' bytes, the two flag masks and the dirty flag. A load needs an empty queue, and finds each
    /// name again (the store's lookup: the world's xfer cannot reach it, so the names are kept and checked by `check_loaded_names`).
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        let mut count = self.queue.len() as u16;
        xfer.xfer_unsigned_short(&mut count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for e in self.queue.iter_mut() {
                let mut t = e.production_type.to_le_bytes();
                xfer.xfer_user(&mut t)?;
                let mut name = e.name.clone();
                xfer.xfer_ascii_string(&mut name)?;
                let mut id = e.production_id.to_le_bytes();
                xfer.xfer_user(&mut id)?;
                xfer.xfer_real(&mut e.percent_complete)?;
                xfer.xfer_int(&mut e.frames_under_construction)?;
                xfer.xfer_int(&mut e.quantity_total)?;
                xfer.xfer_int(&mut e.quantity_produced)?;
                xfer.xfer_int(&mut e.exit_door)?;
            }
        } else {
            // "the queue should be emtpy now" (`SC_INVALID_DATA`, reported as `XferStatus::ListNotEmpty`)
            if !self.queue.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            let names = self.load_names.take().unwrap_or_default();
            for _ in 0..count {
                // "tie to list at end", then read the entry in place (a failure leaves it there, as far as it was read)
                self.queue.push(ProductionEntry::default());
                let e = self.queue.last_mut().expect("just pushed");
                let mut t = [0u8; 4];
                xfer.xfer_user(&mut t)?;
                e.production_type = i32::from_le_bytes(t);
                let mut name = Vec::new();
                xfer.xfer_ascii_string(&mut name)?;
                if e.production_type == PRODUCTION_UNIT {
                    e.object_to_produce = names.templates.iter().find(|(n, _)| *n == name).map(|&(_, id)| id);
                } else {
                    e.upgrade_to_research = names.upgrades.iter().find(|(n, _)| *n == name).map(|&(_, k)| k);
                }
                let found = e.object_to_produce.is_some() || e.upgrade_to_research.is_some();
                e.name = name;
                if !found {
                    // "Cannot find template" / "Cannot find upgrade": `SC_INVALID_DATA`
                    return Err(XferStatus::ListNotEmpty);
                }
                let mut id = [0u8; 4];
                xfer.xfer_user(&mut id)?;
                e.production_id = i32::from_le_bytes(id);
                xfer.xfer_real(&mut e.percent_complete)?;
                xfer.xfer_int(&mut e.frames_under_construction)?;
                xfer.xfer_int(&mut e.quantity_total)?;
                xfer.xfer_int(&mut e.quantity_produced)?;
                xfer.xfer_int(&mut e.exit_door)?;
            }
        }
        let mut uid = self.unique_id.to_le_bytes();
        xfer.xfer_user(&mut uid)?;
        self.unique_id = i32::from_le_bytes(uid);
        xfer.xfer_unsigned_int(&mut self.production_count)?;
        xfer.xfer_unsigned_int(&mut self.construction_complete_frame)?;
        let mut b = self.doors_bytes();
        xfer.xfer_user(&mut b)?;
        for (i, d) in self.doors.iter_mut().enumerate() {
            let w = |k: usize| u32::from_le_bytes(b[16 * i + 4 * k..16 * i + 4 * k + 4].try_into().expect("four bytes"));
            *d = DoorInfo { door_opened_frame: w(0), door_wait_open_frame: w(1), door_closed_frame: w(2), hold_open: b[16 * i + 12] != 0 };
        }
        self.clear_flags.xfer(xfer)?;
        self.set_flags.xfer(xfer)?;
        xfer.xfer_bool(&mut self.flags_dirty)?;
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ProductionUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ProductionUpdate(m) => Some(m),
            _ => None,
        }
    }
    /// The death, and the `ProductionUpdateInterface`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let us = me.object;
        match call {
            ModuleCall::HasProductionUpdateInterface => ModuleReply::Bool(true),
            ModuleCall::OnDie(_) | ModuleCall::CancelAndRefundAllProduction => {
                // "we need to cancel all of our production on death"
                let r = self.cancel_and_refund_all_production(world, us);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::ProductionCanQueueCreateUnit { template } => ModuleReply::Int(self.can_queue_create_unit(world, us, template)),
            ModuleCall::ProductionCanQueueUpgrade { .. } => ModuleReply::Int(self.can_queue_upgrade()),
            ModuleCall::ProductionRequestUniqueUnitId => {
                let id = self.unique_id;
                self.unique_id = self.unique_id.wrapping_add(1);
                ModuleReply::Int(id)
            }
            ModuleCall::ProductionQueueUpgrade { upgrade } => {
                let r = self.queue_upgrade(world, us, upgrade.map(NameKeyType));
                ModuleReply::Bool(Self::record(world, me, r).unwrap_or(false))
            }
            ModuleCall::ProductionCancelUpgrade { upgrade } => {
                let r = self.cancel_upgrade(world, us, upgrade.map(NameKeyType));
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::ProductionIsUpgradeInQueue { upgrade } => ModuleReply::Bool(upgrade.is_some_and(|k| self.is_upgrade_in_queue(NameKeyType(k)))),
            ModuleCall::ProductionCountUnitTypeInQueue { template } => ModuleReply::Int(self.count_unit_type_in_queue(template) as i32),
            ModuleCall::ProductionQueueCreateUnit { template, production_id } => {
                let r = self.queue_create_unit(world, us, template, production_id);
                ModuleReply::Bool(Self::record(world, me, r).unwrap_or(false))
            }
            ModuleCall::ProductionCancelUnitCreate { production_id } => {
                let r = self.cancel_unit_create(world, us, production_id);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::ProductionCancelAllUnitsOfType { template } => {
                let r = self.cancel_all_units_of_type(world, us, template);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::ProductionGetCount => ModuleReply::Int(self.production_count as i32),
            ModuleCall::SetHoldDoorOpen { door, hold } => {
                self.set_hold_door_open(world, door, hold);
                ModuleReply::Done
            }
            ModuleCall::ProductionSetSpecialPowerConstructionCommandButton(button) => {
                self.special_power_construction_command_button = button;
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for ProductionUpdate {
    /// `crc`: `UpdateModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    /// `xfer` (version 1): the version, `UpdateModule::xfer`, the fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.base.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.base = base;
        r
    }
    fn load_post_process(&mut self) {
        self.base.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = D::default();
        assert_eq!((d.max_queue_entries, d.num_door_animations), (9, 0));
        assert!(d.disabled_types_to_process.test(DISABLED_HELD));
        let m = ProductionUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!((m.unique_id, m.production_count, m.queue.len()), (1, 0, 0));
        assert_eq!(m.doors_bytes(), [0u8; 64]);
    }

    #[test]
    fn the_door_flags_follow_the_model_state_numbering() {
        use crate::common::model_state::*;
        assert_eq!((opening_flag(1), closing_flag(1), waiting_open_flag(1)), (MODELCONDITION_DOOR_2_OPENING, MODELCONDITION_DOOR_2_CLOSING, MODELCONDITION_DOOR_2_WAITING_OPEN));
        assert_eq!(opening_flag(3), MODELCONDITION_DOOR_4_OPENING);
    }
}
