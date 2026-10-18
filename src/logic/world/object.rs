// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Object.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//   (the data of `Object`; its operations are in `object_ops.rs`, its
//   save/CRC in `object_xfer.rs`)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   (`m_objList`, `m_objVector`, `addObjectToLookupTable`,
//   `removeObjectFromLookupTable`; GameLogic.h `findObjectByID`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Object.h / Object.cpp: Michael S. Booth, October 2000
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Object` (layer 8) and the object arena: the object list (`m_objList`) and
//! the id lookup table (`m_objVector`) of `GameLogic`.
//!
//! # Where the translation of `Object` lives
//!
//! | file | holds |
//! |---|---|
//! | `object.rs` (this file) | the data of `Object` (`Object.h`), the arena, the module list, the accessors that touch no other object |
//! | `object_ops.rs` | `Object`'s operations as `impl World` (they reach other objects, the teams, the client and the cut points) |
//! | `object_xfer.rs` | `crc`, `xfer`, `loadPostProcess` |
//! | `object_cuts.rs` | the cut points: every call into a layer that is not translated yet, listed |
//! | `object_events.rs` | the client notifications |
//!
//! # The two structures upstream keeps, and the one here
//!
//! Upstream has two views of the same objects:
//!
//! - **`m_objList`**, an intrusive doubly linked list threaded through
//!   `Object::m_prev`/`m_next`. `registerObject` *prepends*, so the list runs
//!   **newest first**. Everything that walks "all objects"
//!   (`getFirstObject()`/`getNextObject()`, 28 calls in the logic, the CRC,
//!   the disabled-status pass at the end of a frame, `TerrainLogic`, script
//!   conditions) sees this order, and so does any result built from it.
//! - **`m_objVector`**, `std::vector<Object*>` indexed by `ObjectID`, 8192
//!   entries to begin with, doubled until the id fits. `findObjectByID` is an
//!   index into it.
//!
//! Here the objects live once, in a slab ([`ObjectArena`]); the list is the
//! same intrusive list, but its links are **ids**, kept next to the object in
//! its slab entry (so [`Object`] need not know it is in a list, and the
//! Object translator cannot forget to maintain the links); and the lookup
//! table is a `Vec<u32>` indexed by id, as upstream's, holding the slab
//! position. Slab positions are internal: nothing observable depends on them.
//!
//! # Semantics kept from upstream (each is relied on by callers)
//!
//! - **A destroyed object is still found.** `destroyObject` only marks the
//!   object and queues it; it stays in the list and the table until
//!   `processDestroyList` at the end of the frame. [`ObjectArena::find`]
//!   therefore returns destroyed objects, and callers check
//!   [`Object::is_destroyed`] (upstream's pattern at the 373 call sites).
//! - **`INVALID_ID` is never found**, and so is an id beyond the table.
//! - **The table grows by doubling**; ids are never reused during a game
//!   (the allocator is a counter).
//!
//! # Where upstream is undefined (Rust: an error, never a crash)
//!
//! - `findObjectByID` with a negative id indexes before the vector: here
//!   `None`.
//! - `addObjectToLookupTable` with a negative id (an `Int` compared with a
//!   `size_t`, so it doubles forever) or with `INVALID_ID`: [`ArenaError::BadId`].
//!   An id of 2^28 or more would need a table of 2 GiB: [`ArenaError::BadId`]
//!   too (a game allocates one id per object; 2^28 is unreachable).
//! - Registering an id that is already registered silently overwrites the
//!   table entry upstream (and corrupts the list): [`ArenaError::DuplicateId`].
//! - `prependToList` of an object already in a list asserts in debug and
//!   corrupts the list in release; same error.
//!
//! # `Object` (the data)
//!
//! Every member of `Object.h` is here, with the pointer members turned into
//! ids (the pointer table is in the field docs):
//!
//! | upstream pointer | here |
//! |---|---|
//! | `Thing` base (`m_template`, transform, caches) | [`Object::thing`] |
//! | `m_drawable` | [`Object::drawable`], the client's `DrawableId` |
//! | `m_next`, `m_prev` | the arena's (not on the object) |
//! | `m_group` | [`Object::group`], an `AIGroup` id |
//! | `m_team` | [`Object::team`], a `TeamId` (`0` = none) |
//! | `m_containedBy` | [`Object::contained_by`], an `ObjectId` |
//! | `m_behaviors`, `m_body`, `m_contain`, `m_stealth`, `m_ai`, `m_physics` and the eight helpers | [`Object::modules`] and indices into it |
//! | `m_partitionData`, `m_radarData` | flags: the partition manager and the radar keep the data |
//! | `m_partitionLast*` (`SightingInfo*`) | [`Sighting`] by value |
//! | `m_experienceTracker` | [`ExperienceRecord`], the tracker's state (its behaviours are cut points) |
//! | `m_weaponSet` | [`WeaponSetRecord`], the weapon set's state with no weapon in any slot (weapons are a cut point) |
//! | `m_triggerInfo[].pTrigger` | the trigger area's id |
//!
//! # Modules
//!
//! `m_behaviors` is [`Object::modules`], in module order: the helpers first
//! (`Object::Object` makes them before the template's modules), then the
//! template's. Two views of the same list:
//!
//! - [`Object::behaviors`] is the **update-capable** modules, by update
//!   interface, in the same order: the scheduler's view (`UpdateHandle.index`
//!   is a position in it). This is the contract of `docs/world-design.md`
//!   ("What the Object translator must keep"). A [`ModuleSlot`] that has an
//!   update interface names its [`BehaviorSlot`] by `update_index`.
//! - Every interface call upstream makes through `BehaviorModule::getX()` is a
//!   [`ModuleCall`](crate::logic::world::update_body::ModuleCall) sent to the
//!   module's body (an [`UpdateBody`]); which modules have which interface is
//!   the slot's `interfaces` mask.

use std::sync::Arc;

use crate::common::base_type::{Coord2D, Coord3D, ICoord3D};
use crate::common::disabled_types::{DisabledMaskType, DISABLED_COUNT};
use crate::common::game_common::{PlayerMaskType, VeterancyLevel, MAX_PLAYER_COUNT, NEVER};
use crate::common::game_type::{
    DrawableId, FormationId, ObjectId, PathfindLayer, INVALID_DRAWABLE_ID, INVALID_ID, LAYER_GROUND, LAYER_INVALID, NO_FORMATION_ID,
    WEAPONSLOT_COUNT,
};
use crate::common::geometry::GeometryInfo;
use crate::common::kind_of::{KindOfMaskType, KindOfType};
use crate::common::module::{Module, ModuleInterfaceType};
use crate::common::object_status_types::{ObjectStatusMaskType, ObjectStatusTypes, OBJECT_STATUS_DESTROYED};
use crate::common::special_power_mask_type::SpecialPowerMaskType;
use crate::common::thing_template::ThingTemplate;
use crate::common::upgrade::UpgradeMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DamageTypeFlags};
use crate::logic::module::update_module::UpdateModule;
use crate::logic::thing::{Thing, ThingTemplateView};
use crate::logic::weapon_set_flags::WeaponSetFlags;
use crate::logic::world::ids::TeamId;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `OBJ_HASH_SIZE` of `GameLogic.cpp`: the initial size of the lookup table.
pub const OBJ_HASH_SIZE: usize = 8192;

/// Ids of 2^28 and above are refused (see the module docs).
pub const MAX_LOOKUP_ID: ObjectId = 1 << 28;

/// `MAX_TRIGGER_AREA_INFOS` (`Object.h`).
pub const MAX_TRIGGER_AREA_INFOS: usize = 5;

/// `CONSTRUCTION_COMPLETE` (`BuildAssistant.h`): `m_constructionPercent` of an
/// object that is not being built.
pub const CONSTRUCTION_COMPLETE: f32 = -1.0;

/// `ObjectPrivateStatusBits`: `EFFECTIVELY_DEAD`.
pub const PRIVATE_EFFECTIVELY_DEAD: u8 = 1 << 0;
/// `ObjectPrivateStatusBits`: `UNDETECTED_DEFECTOR`.
pub const PRIVATE_UNDETECTED_DEFECTOR: u8 = 1 << 1;
/// `ObjectPrivateStatusBits`: `CAPTURED`.
pub const PRIVATE_CAPTURED: u8 = 1 << 2;
/// `ObjectPrivateStatusBits`: `OFF_MAP`.
pub const PRIVATE_OFF_MAP: u8 = 1 << 3;

/// `ThingTemplateView` for the real template: the three `isKindOf` queries
/// `Thing` forwards (cut point 1 of `logic::thing`, filled now that the
/// template is translated).
impl ThingTemplateView for ThingTemplate {
    fn is_kind_of(&self, t: KindOfType) -> bool {
        ThingTemplate::is_kind_of(self, t)
    }
    fn is_kind_of_multi(&self, must_be_set: &KindOfMaskType, must_be_clear: &KindOfMaskType) -> bool {
        ThingTemplate::is_kind_of_multi(self, must_be_set, must_be_clear)
    }
    fn is_any_kind_of(&self, any_kind_of: &KindOfMaskType) -> bool {
        ThingTemplate::is_any_kind_of(self, any_kind_of)
    }
}

/// A position in [`Object::modules`].
pub type ModuleIndex = u16;

/// The eight sleepy helper modules `Object::Object` makes (`NUM_SLEEP_HELPERS`),
/// in the order it makes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HelperKind {
    /// `ObjectSMCHelper` (`m_smcHelper`): always.
    Smc,
    /// `StatusDamageHelper`: unless the body is an `InactiveBody`.
    StatusDamage,
    /// `SubdualDamageHelper`: unless the body is an `InactiveBody`.
    SubdualDamage,
    /// `ObjectRepulsorHelper`: if `KINDOF_CAN_BE_REPULSED` and the AI data enables repulsors.
    Repulsor,
    /// `ObjectDefectionHelper`: unless `KINDOF_SHRUBBERY`.
    Defection,
    /// `ObjectWeaponStatusHelper`: if the template can possibly have a weapon.
    WeaponStatus,
    /// `FiringTracker`: if the template can possibly have a weapon.
    FiringTracker,
    /// `TempWeaponBonusHelper`: if the template can possibly have a weapon.
    TempWeaponBonus,
}

/// The cached helper pointers of `Object` (`m_smcHelper`, ...), as indices
/// into [`Object::modules`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectHelpers {
    /// `m_repulsorHelper`.
    pub repulsor: Option<ModuleIndex>,
    /// `m_smcHelper`.
    pub smc: Option<ModuleIndex>,
    /// `m_wsHelper`.
    pub weapon_status: Option<ModuleIndex>,
    /// `m_defectionHelper`.
    pub defection: Option<ModuleIndex>,
    /// `m_statusDamageHelper`.
    pub status_damage: Option<ModuleIndex>,
    /// `m_subdualDamageHelper`.
    pub subdual_damage: Option<ModuleIndex>,
    /// `m_tempWeaponBonusHelper`.
    pub temp_weapon_bonus: Option<ModuleIndex>,
    /// `m_firingTracker`.
    pub firing_tracker: Option<ModuleIndex>,
}

/// One behaviour module of an object that has an update interface, as the
/// scheduler sees it.
///
/// Upstream's `BehaviorModule*` is a heap object whose `getUpdate()` yields the
/// `UpdateModuleInterface` of the same object. Here the **scheduling half**
/// (`UpdateModule`: the next-call word and the heap index, which the scheduler
/// reads while another module of the same object may be running) stays in the
/// arena at all times, and the **behaviour half** (the concrete module's data)
/// is an [`UpdateBody`] that is *checked out* while it runs (see
/// `docs/world-design.md`, "Re-entrancy").
#[derive(Clone, Debug)]
pub struct BehaviorSlot {
    /// The `UpdateModule` base class.
    pub update: UpdateModule,
    /// The concrete module; `None` while it is running.
    pub body: Option<UpdateBody>,
}

/// One entry of `Object::m_behaviors`. See the module docs.
#[derive(Clone, Debug)]
pub struct ModuleSlot {
    /// `getModuleNameKey()`, as the class name (`"PhysicsBehavior"`).
    pub name: Vec<u8>,
    /// `TheNameKeyGenerator->keyToName(getModuleTagNameKey())` (`"ModuleTag_01"`),
    /// which `Object::xfer` writes and matches on load.
    pub tag: Vec<u8>,
    /// Which `BehaviorModule::getX()` answer non-null (`MODULEINTERFACE_*`).
    pub interfaces: ModuleInterfaceType,
    /// The position in [`Object::behaviors`] if the module has an update
    /// interface; its body lives there.
    pub update_index: Option<u16>,
    /// The concrete module of a module with no update interface; `None`
    /// while it is running, or for a helper no layer has made yet.
    pub body: Option<UpdateBody>,
    /// The base-class state the module factory made (`Module`), for the
    /// `xfer` chain of the bases.
    pub module: Option<Module>,
    /// Which helper this is, if it is one of the eight `Object::Object` makes.
    pub helper: Option<HelperKind>,
    /// The module was given a body when it was made. A module with a body
    /// whose slot is empty is running (its body is checked out), which is how
    /// a call that cannot be delivered is told from a module with no behaviour.
    pub has_body: bool,
}

impl ModuleSlot {
    /// Whether the module has the interface `mask` (`MODULEINTERFACE_*`).
    pub fn has_interface(&self, mask: ModuleInterfaceType) -> bool {
        self.interfaces & mask != 0
    }
}

/// `SightingInfo` (`PartitionManager.h`): where and for whom an object last
/// looked, shrouded or dealt with threat or value, so that it can undo it.
/// Held by value (upstream allocates five per object and keeps them for its
/// life). The partition manager's own copy of the type replaces this when it
/// lands; the layout and `xfer` are the same.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sighting {
    /// `m_where`.
    pub where_: Coord3D,
    /// `m_howFar`.
    pub how_far: f32,
    /// `m_forWhom`.
    pub for_whom: PlayerMaskType,
    /// `m_data`.
    pub data: u32,
}

impl Sighting {
    /// `SightingInfo::reset()`.
    pub fn reset(&mut self) {
        *self = Sighting::default();
    }

    /// `SightingInfo::isInvalid()`: `m_howFar == 0.0f`.
    pub fn is_invalid(&self) -> bool {
        self.how_far == 0.0
    }
}

impl Snapshot for Sighting {
    /// `SightingInfo::crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// `SightingInfo::xfer` (version 1).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_coord3d(&mut self.where_)?;
        xfer.xfer_real(&mut self.how_far)?;
        let mut for_whom = self.for_whom.to_le_bytes();
        xfer.xfer_user(&mut for_whom)?;
        self.for_whom = PlayerMaskType::from_le_bytes(for_whom);
        xfer.xfer_unsigned_int(&mut self.data)?;
        Ok(())
    }
    fn load_post_process(&mut self) {}
}

/// `ExperienceTracker`'s state (`m_experienceTracker`). The tracker is its own
/// layer-8 unit; `Object` owns one for its whole life, and the calls into it
/// are cut points (`object_cuts.rs`), whose default implementation here is the
/// tracker's logic. Same `crc` and `xfer`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExperienceRecord {
    /// `m_currentLevel`.
    pub level: VeterancyLevel,
    /// `m_currentExperience`.
    pub points: i32,
    /// `m_experienceSink`.
    pub sink: ObjectId,
    /// `m_experienceScalar`.
    pub scalar: f32,
}

impl Default for ExperienceRecord {
    /// `ExperienceTracker::ExperienceTracker`.
    fn default() -> Self {
        ExperienceRecord { level: VeterancyLevel::Regular, points: 0, sink: INVALID_ID, scalar: 1.0 }
    }
}

impl Snapshot for ExperienceRecord {
    /// `ExperienceTracker::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_int(&mut self.points)?;
        xfer.xfer_user(&mut (self.level as i32).to_le_bytes())
    }
    /// `ExperienceTracker::xfer` (version 1).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut level = (self.level as i32).to_le_bytes();
        xfer.xfer_user(&mut level)?;
        self.level = veterancy_from_i32(i32::from_le_bytes(level));
        xfer.xfer_int(&mut self.points)?;
        xfer.xfer_object_id(&mut self.sink)?;
        xfer.xfer_real(&mut self.scalar)
    }
    fn load_post_process(&mut self) {}
}

/// `(VeterancyLevel)i`, for a value read from a file: out of range stays at
/// the nearest level (upstream stores any int).
pub fn veterancy_from_i32(i: i32) -> VeterancyLevel {
    match i {
        i32::MIN..=0 => VeterancyLevel::Regular,
        1 => VeterancyLevel::Veteran,
        2 => VeterancyLevel::Elite,
        _ => VeterancyLevel::Heroic,
    }
}

/// `WeaponSet`'s state with every weapon slot empty. `Weapon` and the weapon
/// set's logic are not translated (a cut point); this is what `Object::xfer`
/// and `Object::crc` need of it, laid out as `WeaponSet::xfer` writes it.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponSetRecord {
    /// The template name of `m_curWeaponTemplateSet` (empty = none).
    pub template_name: Vec<u8>,
    /// The flags of `m_curWeaponTemplateSet`.
    pub flags: WeaponSetFlags,
    /// `m_curWeapon` (a `WeaponSlotType`).
    pub cur_weapon: i32,
    /// `m_curWeaponLockedStatus` (a `WeaponLockType`).
    pub cur_weapon_locked_status: i32,
    /// `m_filledWeaponSlotMask`.
    pub filled_weapon_slot_mask: u32,
    /// `m_totalAntiMask`.
    pub total_anti_mask: i32,
    /// `m_hasDamageWeapon`.
    pub has_damage_weapon: bool,
    /// `m_totalDamageTypeMask`.
    pub total_damage_type_mask: DamageTypeFlags,
}

impl Default for WeaponSetRecord {
    /// `WeaponSet::WeaponSet()`: no template set, `PRIMARY_WEAPON`, not locked.
    fn default() -> Self {
        WeaponSetRecord {
            template_name: Vec::new(),
            flags: WeaponSetFlags::new(),
            cur_weapon: 0,
            cur_weapon_locked_status: 0,
            filled_weapon_slot_mask: 0,
            total_anti_mask: 0,
            has_damage_weapon: false,
            total_damage_type_mask: DamageTypeFlags::new(),
        }
    }
}

impl Snapshot for WeaponSetRecord {
    /// `WeaponSet::crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// `WeaponSet::xfer` (version 1), with no weapon in any slot.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_ascii_string(&mut self.template_name)?;
        self.flags.xfer(xfer)?;
        for _ in 0..WEAPONSLOT_COUNT {
            // xferBool(&hasWeaponInSlot): the weapons themselves are a cut point
            let mut has = false;
            xfer.xfer_bool(&mut has)?;
        }
        let mut cur = self.cur_weapon.to_le_bytes();
        xfer.xfer_user(&mut cur)?;
        self.cur_weapon = i32::from_le_bytes(cur);
        let mut locked = self.cur_weapon_locked_status.to_le_bytes();
        xfer.xfer_user(&mut locked)?;
        self.cur_weapon_locked_status = i32::from_le_bytes(locked);
        xfer.xfer_unsigned_int(&mut self.filled_weapon_slot_mask)?;
        xfer.xfer_int(&mut self.total_anti_mask)?;
        xfer.xfer_bool(&mut self.has_damage_weapon)?;
        // upstream writes m_hasDamageWeapon twice (the second xfer is a copy-paste of the first)
        xfer.xfer_bool(&mut self.has_damage_weapon)?;
        self.total_damage_type_mask.xfer(xfer)
    }
    fn load_post_process(&mut self) {}
}

/// `TTriggerInfo` (`Object.h`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TriggerInfo {
    /// `pTrigger`, by the trigger area's id.
    pub trigger: Option<i32>,
    /// `entered`.
    pub entered: bool,
    /// `exited`.
    pub exited: bool,
    /// `isInside`.
    pub is_inside: bool,
}

/// The shape of `Object`. See the module docs.
#[derive(Clone, Debug)]
pub struct Object {
    /// The `Thing` base: template, transform and the caches derived from it.
    pub thing: Thing<ThingTemplate>,
    /// `getPosition()`, a read-only mirror of `thing.position()` that every
    /// setter of this module keeps current (it was the one position field of
    /// the shell, which other code reads). Write through
    /// [`Object::set_position`], never to this field.
    pub position: Coord3D,
    /// `m_id`, set by `allocateObjectID` before the modules are made.
    pub id: ObjectId,
    /// `m_producerID`.
    pub producer_id: ObjectId,
    /// `m_builderID`.
    pub builder_id: ObjectId,
    /// `m_drawable`: the client's id of the drawable bound to this object
    /// (`INVALID_DRAWABLE_ID` = none).
    pub drawable: DrawableId,
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_status`.
    pub status: ObjectStatusMaskType,
    /// `m_geometryInfo`.
    pub geometry_info: GeometryInfo,
    /// `m_group`: the `AIGroup` this object is in (0 = none).
    pub group: u32,
    /// `m_partitionLastLook`.
    pub partition_last_look: Sighting,
    /// `m_partitionRevealAllLastLook`.
    pub partition_reveal_all_last_look: Sighting,
    /// `m_visionSpiedBy`.
    pub vision_spied_by: [i32; MAX_PLAYER_COUNT as usize],
    /// `m_visionSpiedMask`.
    pub vision_spied_mask: PlayerMaskType,
    /// `m_partitionLastShroud`.
    pub partition_last_shroud: Sighting,
    /// `m_partitionLastThreat`.
    pub partition_last_threat: Sighting,
    /// `m_partitionLastValue`.
    pub partition_last_value: Sighting,
    /// `m_visionRange`.
    pub vision_range: f32,
    /// `m_shroudClearingRange`.
    pub shroud_clearing_range: f32,
    /// `m_shroudRange`.
    pub shroud_range: f32,
    /// `m_disabledMask`.
    pub disabled: DisabledMaskType,
    /// `m_disabledTillFrame`.
    pub disabled_till_frame: [u32; DISABLED_COUNT as usize],
    /// `m_smcUntil`.
    pub smc_until: u32,
    /// `m_repulsorHelper`, `m_smcHelper`, ... as indices into [`Object::modules`].
    pub helpers: ObjectHelpers,
    /// `m_behaviors`: every module, in module order.
    pub modules: Vec<ModuleSlot>,
    /// The modules with an update interface, in module order (the scheduler's
    /// view, see the module docs).
    pub behaviors: Vec<BehaviorSlot>,
    /// `m_contain`.
    pub contain: Option<ModuleIndex>,
    /// `m_body`.
    pub body: Option<ModuleIndex>,
    /// `m_stealth`.
    pub stealth: Option<ModuleIndex>,
    /// `m_ai`.
    pub ai: Option<ModuleIndex>,
    /// `m_physics`.
    pub physics: Option<ModuleIndex>,
    /// `m_partitionData != NULL`: the object is registered with the partition manager.
    pub in_partition: bool,
    /// `m_radarData != NULL`: the object is on the radar.
    pub on_radar: bool,
    /// `m_experienceTracker`'s state.
    pub experience: ExperienceRecord,
    /// `m_containedBy`.
    pub contained_by: ObjectId,
    /// `m_xferContainedByID`.
    pub xfer_contained_by_id: ObjectId,
    /// `m_containedByFrame`.
    pub contained_by_frame: u32,
    /// `m_constructionPercent`.
    pub construction_percent: f32,
    /// `m_objectUpgradesCompleted`.
    pub object_upgrades_completed: UpgradeMaskType,
    /// `m_team`, by id (0 = none).
    pub team: TeamId,
    /// `m_originalTeamName`.
    pub original_team_name: Vec<u8>,
    /// `m_indicatorColor`.
    pub indicator_color: i32,
    /// `m_healthBoxOffset`.
    pub health_box_offset: Coord3D,
    /// `m_weaponSet`.
    pub weapon_set: WeaponSetRecord,
    /// `m_curWeaponSetFlags`.
    pub cur_weapon_set_flags: WeaponSetFlags,
    /// `m_weaponBonusCondition`.
    pub weapon_bonus_condition: u32,
    /// `m_lastWeaponCondition`.
    pub last_weapon_condition: [i8; WEAPONSLOT_COUNT as usize],
    /// `m_specialPowerBits`.
    pub special_power_bits: SpecialPowerMaskType,
    /// `m_soleHealingBenefactorID`.
    pub sole_healing_benefactor_id: ObjectId,
    /// `m_soleHealingBenefactorExpirationFrame`.
    pub sole_healing_benefactor_expiration_frame: u32,
    /// `m_triggerInfo`.
    pub trigger_info: [TriggerInfo; MAX_TRIGGER_AREA_INFOS],
    /// `m_enteredOrExitedFrame`.
    pub entered_or_exited_frame: u32,
    /// `m_iPos`.
    pub i_pos: ICoord3D,
    /// `m_layer`.
    pub layer: PathfindLayer,
    /// `m_destinationLayer`.
    pub destination_layer: PathfindLayer,
    /// `m_formationID`.
    pub formation_id: FormationId,
    /// `m_formationOffset`.
    pub formation_offset: Coord2D,
    /// `m_commandSetStringOverride`.
    pub command_set_string_override: Vec<u8>,
    /// `m_safeOcclusionFrame`.
    pub safe_occlusion_frame: u32,
    /// `m_isSelectable`.
    pub is_selectable: bool,
    /// `m_modulesReady`.
    pub modules_ready: bool,
    /// `m_scriptStatus`.
    pub script_status: u8,
    /// `m_privateStatus`.
    pub private_status: u8,
    /// `m_numTriggerAreasActive`.
    pub num_trigger_areas_active: i8,
    /// `m_singleUseCommandUsed`.
    pub single_use_command_used: bool,
    /// `m_isReceivingDifficultyBonus`.
    pub is_receiving_difficulty_bonus: bool,
    /// The damage handed to this object through `World::attempt_damage`,
    /// oldest first. A record of the cut point (every call is appended before
    /// the body module is asked), kept for tests and for the probes of
    /// `docs/world-design.md`; nothing reads it in the simulation.
    pub damage_received: Vec<DamageInfo>,
}

impl Object {
    /// A bare object with the given id and a blank template: nothing set, no
    /// modules. The shell of the world core (its tests and probes make these);
    /// a real object is made by `World::new_object`, which takes a template.
    pub fn new(id: ObjectId) -> Object {
        Object::with_template(id, Arc::new(ThingTemplate::new(0)))
    }

    /// The members `Object::Object` initialises in its initializer list and
    /// first lines, for an object of `template` with id `id`. The modules, the
    /// team and the rest of the constructor are `World::new_object`'s.
    pub fn with_template(id: ObjectId, template: Arc<ThingTemplate>) -> Object {
        let geometry_info = template.get_template_geometry_info().clone();
        let vision_range = template.friend_calc_vision_range();
        let mut shroud_clearing_range = template.friend_calc_shroud_clearing_range();
        if shroud_clearing_range == -1.0 {
            // Backwards compatible, and perfectly logical default to assign
            shroud_clearing_range = vision_range;
        }
        let is_selectable = template.is_kind_of(crate::common::kind_of::KINDOF_SELECTABLE);
        let thing = Thing::new(Some(template)).expect("a template is present");
        Object {
            thing,
            position: Coord3D::default(),
            id,
            producer_id: INVALID_ID,
            builder_id: INVALID_ID,
            drawable: INVALID_DRAWABLE_ID,
            name: Vec::new(),
            status: ObjectStatusMaskType::new(),
            geometry_info,
            group: 0,
            partition_last_look: Sighting::default(),
            partition_reveal_all_last_look: Sighting::default(),
            vision_spied_by: [0; MAX_PLAYER_COUNT as usize],
            vision_spied_mask: 0,
            partition_last_shroud: Sighting::default(),
            partition_last_threat: Sighting::default(),
            partition_last_value: Sighting::default(),
            vision_range,
            shroud_clearing_range,
            shroud_range: 0.0,
            disabled: DisabledMaskType::new(),
            disabled_till_frame: [NEVER; DISABLED_COUNT as usize],
            smc_until: NEVER,
            helpers: ObjectHelpers::default(),
            modules: Vec::new(),
            behaviors: Vec::new(),
            contain: None,
            body: None,
            stealth: None,
            ai: None,
            physics: None,
            in_partition: false,
            on_radar: false,
            experience: ExperienceRecord::default(),
            contained_by: INVALID_ID,
            xfer_contained_by_id: INVALID_ID,
            contained_by_frame: 0,
            construction_percent: CONSTRUCTION_COMPLETE,
            object_upgrades_completed: UpgradeMaskType::new(),
            team: 0,
            original_team_name: Vec::new(),
            indicator_color: 0,
            health_box_offset: Coord3D::default(),
            weapon_set: WeaponSetRecord::default(),
            cur_weapon_set_flags: WeaponSetFlags::new(),
            weapon_bonus_condition: 0,
            last_weapon_condition: [0; WEAPONSLOT_COUNT as usize],
            special_power_bits: SpecialPowerMaskType::new(),
            sole_healing_benefactor_id: INVALID_ID,
            sole_healing_benefactor_expiration_frame: 0,
            trigger_info: [TriggerInfo::default(); MAX_TRIGGER_AREA_INFOS],
            entered_or_exited_frame: 0,
            i_pos: ICoord3D::default(),
            layer: LAYER_GROUND,
            destination_layer: LAYER_INVALID, // not set by the constructor; the memory pool zero-fills (found by the differential test)
            formation_id: NO_FORMATION_ID,
            formation_offset: Coord2D::default(),
            command_set_string_override: Vec::new(),
            safe_occlusion_frame: 0,
            is_selectable,
            modules_ready: false,
            script_status: 0,
            private_status: 0,
            num_trigger_areas_active: 0,
            single_use_command_used: false,
            is_receiving_difficulty_bonus: false,
            damage_received: Vec::new(),
        }
    }

    /// `getID()`.
    pub fn get_id(&self) -> ObjectId {
        self.id
    }

    /// `getTemplate()`.
    pub fn template(&self) -> &Arc<ThingTemplate> {
        self.thing.template()
    }

    /// `isKindOf(t)`.
    pub fn is_kind_of(&self, t: KindOfType) -> bool {
        self.thing.is_kind_of(t)
    }

    /// `isAnyKindOf(mask)`.
    pub fn is_any_kind_of(&self, mask: &KindOfMaskType) -> bool {
        self.thing.is_any_kind_of(mask)
    }

    /// `getPosition()`.
    pub fn get_position(&self) -> Coord3D {
        *self.thing.position()
    }

    /// `isDestroyed()`: `m_status.test(OBJECT_STATUS_DESTROYED)`.
    pub fn is_destroyed(&self) -> bool {
        self.status.test(OBJECT_STATUS_DESTROYED)
    }

    /// `isDisabled()`: any bit of `m_disabledMask`.
    pub fn is_disabled(&self) -> bool {
        self.disabled.any()
    }

    /// `isDisabledByType(type)`.
    pub fn is_disabled_by_type(&self, t: crate::common::disabled_types::DisabledType) -> bool {
        self.disabled.test(t)
    }

    /// `testStatus(bit)`.
    pub fn test_status(&self, bit: ObjectStatusTypes) -> bool {
        self.status.test(bit)
    }

    /// `setStatus(MAKE_OBJECT_STATUS_MASK(bit))`, bare: set that one bit
    /// without any of `Object::setStatus`'s reactions (see `World::set_status`
    /// for the translation). The world core's tests use it.
    pub fn set_status_bit(&mut self, bit: i32) {
        self.status.set(bit, 1);
    }

    /// `Object::isEffectivelyDead()`.
    pub fn is_effectively_dead(&self) -> bool {
        self.private_status & PRIVATE_EFFECTIVELY_DEAD != 0
    }

    /// `getIsUndetectedDefector()`.
    pub fn is_undetected_defector(&self) -> bool {
        self.private_status & PRIVATE_UNDETECTED_DEFECTOR != 0
    }

    /// `isOffMap()`.
    pub fn is_off_map(&self) -> bool {
        self.private_status & PRIVATE_OFF_MAP != 0
    }

    /// `isCaptured()`.
    pub fn is_captured(&self) -> bool {
        self.private_status & PRIVATE_CAPTURED != 0
    }

    /// `getVeterancyLevel()`: `m_experienceTracker ? .. : LEVEL_REGULAR` (the
    /// record always exists once the object is made).
    pub fn get_veterancy_level(&self) -> VeterancyLevel {
        self.experience.level
    }

    /// `testWeaponSetFlag(wst)`.
    pub fn test_weapon_set_flag(&self, wst: i32) -> bool {
        self.cur_weapon_set_flags.test(wst)
    }

    /// `testWeaponBonusCondition(wst)`: `(m_weaponBonusCondition & (1 << wst)) != 0`.
    pub fn test_weapon_bonus_condition(&self, wst: i32) -> bool {
        (self.weapon_bonus_condition & (1u32.wrapping_shl(wst as u32))) != 0
    }

    /// `testScriptStatusBit(b)`.
    pub fn test_script_status_bit(&self, b: u8) -> bool {
        self.script_status & b != 0
    }

    /// `hasSpecialPower(type)`.
    pub fn has_special_power(&self, t: crate::common::special_power_type::SpecialPowerType) -> bool {
        self.special_power_bits.test(t)
    }

    /// `hasAnySpecialPower()`.
    pub fn has_any_special_power(&self) -> bool {
        self.special_power_bits.any()
    }

    /// `getObjectCompletedUpgradeMask()`.
    pub fn get_object_completed_upgrade_mask(&self) -> UpgradeMaskType {
        self.object_upgrades_completed
    }

    /// `findModule(key)` by class name: the first module in module order with
    /// that name (`getModuleNameKey() == key`), or `None`. Upstream's debug
    /// build also complains about duplicates; the shipped game returns the first.
    pub fn find_module(&self, class_name: &[u8]) -> Option<ModuleIndex> {
        self.modules.iter().position(|m| m.name == class_name).map(|i| i as ModuleIndex)
    }

    /// The first module in module order that has the interface (`MODULEINTERFACE_*`).
    pub fn first_module_with(&self, mask: ModuleInterfaceType) -> Option<ModuleIndex> {
        self.modules.iter().position(|m| m.has_interface(mask)).map(|i| i as ModuleIndex)
    }
}

/// What can go wrong with the arena (each is undefined upstream).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArenaError {
    /// `INVALID_ID`, a negative id or one of 2^28 and above.
    BadId(ObjectId),
    /// The id is already registered.
    DuplicateId(ObjectId),
}

#[derive(Clone, Debug)]
struct Entry {
    object: Object,
    /// `m_prev`: toward the head (newer). `INVALID_ID` for none.
    prev: ObjectId,
    /// `m_next`: toward the tail (older). `INVALID_ID` for none.
    next: ObjectId,
    /// In the object list (`prependToList` has run). An object under
    /// construction is in the table before it is in the list.
    listed: bool,
}

/// `m_objList` + `m_objVector`. See the module docs.
#[derive(Clone, Debug)]
pub struct ObjectArena {
    /// `m_objVector`: slab position + 1 per id, 0 for `NULL`.
    lookup: Vec<u32>,
    /// The objects, in no meaningful order (see the module docs).
    slab: Vec<Option<Entry>>,
    /// Free slab positions, most recently freed last (deterministic).
    free: Vec<u32>,
    /// `m_objList`: the head (newest), `INVALID_ID` when empty.
    head: ObjectId,
    count: usize,
}

impl Default for ObjectArena {
    fn default() -> Self {
        ObjectArena::new()
    }
}

impl ObjectArena {
    /// `GameLogic::reset()`'s `m_objVector.resize(OBJ_HASH_SIZE, NULL)` and an
    /// empty list.
    pub fn new() -> ObjectArena {
        ObjectArena { lookup: vec![0; OBJ_HASH_SIZE], slab: Vec::new(), free: Vec::new(), head: INVALID_ID, count: 0 }
    }

    /// The number of objects in the list.
    pub fn len(&self) -> usize {
        self.count
    }

    /// No objects.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The size of the lookup table (`m_objVector.size()`).
    pub fn lookup_len(&self) -> usize {
        self.lookup.len()
    }

    fn entry(&self, id: ObjectId) -> Option<&Entry> {
        if id <= INVALID_ID {
            return None;
        }
        match self.lookup.get(id as usize) {
            Some(&p) if p != 0 => self.slab[(p - 1) as usize].as_ref(),
            _ => None,
        }
    }

    fn entry_mut(&mut self, id: ObjectId) -> Option<&mut Entry> {
        if id <= INVALID_ID {
            return None;
        }
        match self.lookup.get(id as usize) {
            Some(&p) if p != 0 => self.slab[(p - 1) as usize].as_mut(),
            _ => None,
        }
    }

    /// `findObjectByID(id)`: `None` for `INVALID_ID`, an unregistered id or
    /// one beyond the table. Finds destroyed objects (see the module docs).
    pub fn find(&self, id: ObjectId) -> Option<&Object> {
        self.entry(id).map(|e| &e.object)
    }

    /// `findObjectByID(id)` for a caller that changes the object.
    pub fn find_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.entry_mut(id).map(|e| &mut e.object)
    }

    /// The scheduling half of the module `h` names: `None` if the object is
    /// not registered or has no such behaviour.
    pub fn update_module(&self, h: UpdateHandle) -> Option<&UpdateModule> {
        self.find(h.object)?.behaviors.get(h.index as usize).map(|b| &b.update)
    }

    /// [`ObjectArena::update_module`] for a caller that changes it.
    pub fn update_module_mut(&mut self, h: UpdateHandle) -> Option<&mut UpdateModule> {
        self.find_mut(h.object)?.behaviors.get_mut(h.index as usize).map(|b| &mut b.update)
    }

    /// Checks the behaviour half of module `h` out of its slot. `None` if
    /// there is no such module or it is already checked out (it is running:
    /// see `docs/world-design.md`, "Re-entrancy").
    pub fn take_body(&mut self, h: UpdateHandle) -> Option<UpdateBody> {
        self.find_mut(h.object)?.behaviors.get_mut(h.index as usize)?.body.take()
    }

    /// Puts a checked-out behaviour back. Dropped if the module is gone.
    pub fn put_body(&mut self, h: UpdateHandle, body: UpdateBody) {
        if let Some(slot) = self.find_mut(h.object).and_then(|o| o.behaviors.get_mut(h.index as usize)) {
            slot.body = Some(body);
        }
    }

    /// `getFirstObject()`: the newest object, `INVALID_ID` if none.
    pub fn first_id(&self) -> ObjectId {
        self.head
    }

    /// `obj->getNextObject()`: the next older object; `INVALID_ID` at the end
    /// or for an object that is not in the list.
    pub fn next_id(&self, id: ObjectId) -> ObjectId {
        self.entry(id).map_or(INVALID_ID, |e| e.next)
    }

    /// `obj->getPrevObject()` (toward the newest).
    pub fn prev_id(&self, id: ObjectId) -> ObjectId {
        self.entry(id).map_or(INVALID_ID, |e| e.prev)
    }

    /// The ids in list order (newest first): the snapshot a caller takes when
    /// upstream's loop caches `getNextObject()` before it acts on the current
    /// object (`destroyAllObjectsImmediate`).
    pub fn ids_in_order(&self) -> Vec<ObjectId> {
        let mut v = Vec::with_capacity(self.count);
        let mut id = self.head;
        while id != INVALID_ID {
            v.push(id);
            id = self.next_id(id);
        }
        v
    }

    /// `obj->isInList(&m_objList)`: `m_prev || m_next || head == obj`.
    pub fn is_in_list(&self, id: ObjectId) -> bool {
        match self.entry(id) {
            Some(e) => e.prev != INVALID_ID || e.next != INVALID_ID || self.head == id,
            None => false,
        }
    }

    /// `addObjectToLookupTable` + `prependToList`, in `registerObject`'s order:
    /// the object becomes the new head of the list and findable by id.
    pub fn insert_at_head(&mut self, object: Object) -> Result<(), ArenaError> {
        let id = object.id;
        self.insert_unlisted(object)?;
        self.link_at_head(id)
    }

    /// `addObjectToLookupTable` alone, as `Object::setID` does it in the
    /// constructor: the object is findable by id, in no list (`isInList` is
    /// false, `len` does not count it, `ids_in_order` does not show it).
    pub fn insert_unlisted(&mut self, object: Object) -> Result<(), ArenaError> {
        let id = object.id;
        if id <= INVALID_ID || id >= MAX_LOOKUP_ID {
            return Err(ArenaError::BadId(id));
        }
        if self.entry(id).is_some() {
            return Err(ArenaError::DuplicateId(id));
        }
        let entry = Entry { object, prev: INVALID_ID, next: INVALID_ID, listed: false };
        let pos = match self.free.pop() {
            Some(p) => {
                self.slab[p as usize] = Some(entry);
                p
            }
            None => {
                self.slab.push(Some(entry));
                (self.slab.len() - 1) as u32
            }
        };
        // addObjectToLookupTable: "while (newID >= size) resize(size * 2)"
        while (id as usize) >= self.lookup.len() {
            let n = self.lookup.len() * 2;
            self.lookup.resize(n, 0);
        }
        self.lookup[id as usize] = pos + 1;
        Ok(())
    }

    /// `prependToList(&m_objList)` for an object already in the table: it
    /// becomes the head. [`ArenaError::DuplicateId`] if it is in the list
    /// already (upstream asserts "obj is already in a list" and corrupts the
    /// list), [`ArenaError::BadId`] if it is not in the table.
    pub fn link_at_head(&mut self, id: ObjectId) -> Result<(), ArenaError> {
        let old_head = self.head;
        match self.entry_mut(id) {
            None => return Err(ArenaError::BadId(id)),
            Some(e) if e.listed => return Err(ArenaError::DuplicateId(id)),
            Some(e) => {
                e.prev = INVALID_ID;
                e.next = old_head;
                e.listed = true;
            }
        }
        if old_head != INVALID_ID {
            if let Some(h) = self.entry_mut(old_head) {
                h.prev = id;
            }
        }
        self.head = id;
        self.count += 1;
        Ok(())
    }

    /// `setID(newId)` for an object that is in the table: the object keeps
    /// its place in the list and its slab entry and answers to `new`
    /// (`removeObjectFromLookupTable`, the assignment, `addObjectToLookupTable`).
    /// The neighbours' links and the list head are renamed with it (upstream's
    /// are pointers). [`ArenaError::DuplicateId`] if `new` is taken,
    /// [`ArenaError::BadId`] if `old` is not in the table or `new` is no id.
    pub fn rekey(&mut self, old: ObjectId, new: ObjectId) -> Result<(), ArenaError> {
        if new <= INVALID_ID || new >= MAX_LOOKUP_ID {
            return Err(ArenaError::BadId(new));
        }
        if old == new {
            return Ok(());
        }
        if self.entry(new).is_some() {
            return Err(ArenaError::DuplicateId(new));
        }
        let (prev, next, listed) = match self.entry(old) {
            Some(e) => (e.prev, e.next, e.listed),
            None => return Err(ArenaError::BadId(old)),
        };
        let pos = self.lookup[old as usize];
        self.lookup[old as usize] = 0;
        while (new as usize) >= self.lookup.len() {
            let n = self.lookup.len() * 2;
            self.lookup.resize(n, 0);
        }
        self.lookup[new as usize] = pos;
        if let Some(e) = self.slab[(pos - 1) as usize].as_mut() {
            e.object.id = new;
        }
        if listed {
            if prev != INVALID_ID {
                if let Some(p) = self.entry_mut(prev) {
                    p.next = new;
                }
            } else {
                self.head = new;
            }
            if next != INVALID_ID {
                if let Some(n) = self.entry_mut(next) {
                    n.prev = new;
                }
            }
        }
        Ok(())
    }

    /// `removeFromList(&m_objList)` + `removeObjectFromLookupTable`, in
    /// `processDestroyList`'s order. Returns the object (upstream then calls
    /// `friend_deleteInstance`), or `None` if it is not registered.
    pub fn remove(&mut self, id: ObjectId) -> Option<Object> {
        let (prev, next, listed) = {
            let e = self.entry(id)?;
            (e.prev, e.next, e.listed)
        };
        if listed {
            // removeFromList: if (m_next) m_next->m_prev = m_prev; if (m_prev) m_prev->m_next = m_next; else head = m_next
            if next != INVALID_ID {
                if let Some(n) = self.entry_mut(next) {
                    n.prev = prev;
                }
            }
            if prev != INVALID_ID {
                if let Some(p) = self.entry_mut(prev) {
                    p.next = next;
                }
            } else {
                self.head = next;
            }
            self.count -= 1;
        }
        // removeObjectFromLookupTable
        let pos = self.lookup[id as usize] - 1;
        self.lookup[id as usize] = 0;
        let entry = self.slab[pos as usize].take();
        self.free.push(pos);
        entry.map(|e| e.object)
    }

    /// `GameLogic::reset`'s `m_objVector.clear(); resize(OBJ_HASH_SIZE)` and
    /// `m_objList = NULL`, after the objects are destroyed.
    pub fn clear(&mut self) {
        *self = ObjectArena::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(id: ObjectId) -> Object {
        Object::new(id)
    }

    #[test]
    fn the_list_is_newest_first_and_find_uses_the_id() {
        let mut a = ObjectArena::new();
        for id in [1, 2, 3] {
            a.insert_at_head(obj(id)).unwrap();
        }
        assert_eq!(a.ids_in_order(), [3, 2, 1]);
        assert_eq!(a.first_id(), 3);
        assert_eq!(a.next_id(3), 2);
        assert_eq!(a.prev_id(2), 3);
        assert_eq!(a.next_id(1), INVALID_ID);
        assert_eq!(a.find(2).map(|o| o.id), Some(2));
        assert!(a.find(4).is_none());
        assert!(a.find(INVALID_ID).is_none());
        assert!(a.find(-5).is_none(), "undefined upstream: None here");
        assert!(a.find(i32::MAX).is_none());
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn remove_relinks_like_remove_from_list() {
        let mut a = ObjectArena::new();
        for id in 1..=5 {
            a.insert_at_head(obj(id)).unwrap();
        }
        // head, middle, tail
        assert_eq!(a.remove(5).unwrap().id, 5);
        assert_eq!(a.first_id(), 4);
        assert_eq!(a.prev_id(4), INVALID_ID);
        assert_eq!(a.remove(3).unwrap().id, 3);
        assert_eq!(a.ids_in_order(), [4, 2, 1]);
        assert_eq!(a.remove(1).unwrap().id, 1);
        assert_eq!(a.ids_in_order(), [4, 2]);
        assert!(a.remove(1).is_none());
        assert!(a.find(3).is_none());
        assert_eq!(a.remove(4).unwrap().id, 4);
        assert_eq!(a.remove(2).unwrap().id, 2);
        assert!(a.is_empty());
        assert_eq!(a.first_id(), INVALID_ID);
    }

    #[test]
    fn is_in_list_is_prev_or_next_or_head() {
        let mut a = ObjectArena::new();
        assert!(!a.is_in_list(1));
        a.insert_at_head(obj(1)).unwrap();
        assert!(a.is_in_list(1), "a sole member is the head");
        a.insert_at_head(obj(2)).unwrap();
        assert!(a.is_in_list(1) && a.is_in_list(2));
        a.remove(1);
        assert!(!a.is_in_list(1));
    }

    #[test]
    fn the_table_doubles_until_the_id_fits() {
        let mut a = ObjectArena::new();
        assert_eq!(a.lookup_len(), OBJ_HASH_SIZE);
        a.insert_at_head(obj(OBJ_HASH_SIZE as i32 - 1)).unwrap();
        assert_eq!(a.lookup_len(), OBJ_HASH_SIZE);
        a.insert_at_head(obj(OBJ_HASH_SIZE as i32)).unwrap();
        assert_eq!(a.lookup_len(), 2 * OBJ_HASH_SIZE);
        a.insert_at_head(obj(5 * OBJ_HASH_SIZE as i32 + 3)).unwrap();
        assert_eq!(a.lookup_len(), 8 * OBJ_HASH_SIZE);
        assert!(a.find(5 * OBJ_HASH_SIZE as i32 + 3).is_some());
    }

    #[test]
    fn undefined_ids_and_duplicates_are_errors() {
        let mut a = ObjectArena::new();
        assert_eq!(a.insert_at_head(obj(0)), Err(ArenaError::BadId(0)));
        assert_eq!(a.insert_at_head(obj(-3)), Err(ArenaError::BadId(-3)));
        assert_eq!(a.insert_at_head(obj(MAX_LOOKUP_ID)), Err(ArenaError::BadId(MAX_LOOKUP_ID)));
        a.insert_at_head(obj(7)).unwrap();
        assert_eq!(a.insert_at_head(obj(7)), Err(ArenaError::DuplicateId(7)));
        assert_eq!(a.len(), 1);
        a.clear();
        assert!(a.is_empty() && a.find(7).is_none());
    }

    #[test]
    fn destroyed_objects_are_still_found() {
        let mut a = ObjectArena::new();
        a.insert_at_head(obj(1)).unwrap();
        a.find_mut(1).unwrap().set_status_bit(OBJECT_STATUS_DESTROYED);
        assert!(a.find(1).unwrap().is_destroyed());
        assert!(!a.find(1).unwrap().is_disabled());
    }
}
