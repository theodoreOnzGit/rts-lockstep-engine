// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ActiveBody.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/ActiveBody.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ActiveBody`: the body that gives an object its health, its armor, its damage states and its death. Every unit, structure and bridge with health has
//! one (or one of the classes derived from it: `StructureBody`, `HighlanderBody`, `ImmortalBody`, `UndeadBody`, `HiveStructureBody`).
//!
//! # Virtual calls
//!
//! `ActiveBody` calls two of its own functions that the derived classes override: `attemptDamage` (`HighlanderBody`, `UndeadBody`, `HiveStructureBody`) and
//! `internalChangeHealth` (`ImmortalBody`). They are the two methods of [`ActiveBodyVirtuals`], and every function of the base that reaches them is a generic
//! function over the derived type, so that the call goes to the override, as the vtable sends it. [`ActiveBody`] implements the trait with no overrides.
//!
//! # Re-entrancy
//!
//! Every call out to the world that can reach this body again (a damage module that kills its object, a die module, the kill of a rider, the veterancy a
//! score brings) runs through [`out`], which lends the body's current state to its slot around the call, so that the nested call runs at once, on the live
//! state, in upstream's order, and the changes it makes are the state afterwards. (`docs/world-design.md`, "Re-entrancy".)
//!
//! # The cut points
//!
//! `TheGlobalData` and `TheAI->getAiData()` are [`BodyGlobals`](crate::logic::world::body_cuts::BodyGlobals); the particle system manager, the bones of the
//! drawable, the pathfinder, the partition manager and `getAbleToAttackSpecificObject` are the methods of [`crate::logic::world::body_cuts`]; the Object-level
//! calls that cascade (`setDisabled`, `scoreTheKill`, `onDie`, ...) are `body_*` there too, run for real unless a test ablates them. The armor store and the
//! damage FX store are `world.statics`. `TheInGameUI->getFirstSelectedDrawable()` and `getSelectCount()` are `world.reads.local_selection`;
//! `TheControlBar->markUIDirty()` is [`ObjectClientEvent::ControlBarMarkUiDirty`]; `reactToBodyDamageStateChange` is
//! [`ObjectClientEvent::DrawableReactToBodyDamageStateChange`].
//!
//! # Upstream facts kept
//!
//! - `m_lastDamageInfo` takes the damage of the first hit of a frame, and of the next frame; a later hit of the same frame replaces it only if its source is a
//!   vehicle, infantry or a faction structure (or the old one has no source object).
//! - The damage scalar is not applied to `DAMAGE_UNRESISTABLE` and not to the damage types that are already handled (the pilot kill, the garrison kill,
//!   status damage, subdual).
//! - A subdual hit on a body that cannot be subdued returns at once: no damage FX, no retaliation.
//! - `doDamageFX` runs for every hit, killed or not, and `DAMAGE_RADIATION` no longer tints (the Generals version did).
//! - The garrison kill iterates the contain module's list as it is when the kill begins (upstream walks the live list, which is undefined if a kill erases
//!   the node the iterator stands on).
//! - A loaded particle system list is the saved one reversed ("the list will be reversed, but we don't care").
//! - `ThePlayerList->getNeutralPlayer()->getAcademyStats()->recordVehicleSniped()` runs for every vehicle hit by the pilot kill, rider-change or not.
//! - The function-local `static` particle system templates of `updateBodyParticleSystems` are looked up at every call here (the stores do not change while a
//!   game runs, so the result is the same).
//!
//! # Where upstream is undefined
//!
//! A controlling player that is `NULL` where the body calls through it ([`Undefined::NullControllingPlayer`]); `getAI()` of a rider-change vehicle that has no
//! AI ([`Undefined::NullAi`]); the first of an empty contained list ([`Undefined::EmptyContainedList`]); a veterancy level outside the four indexing
//! `m_healthBonus` ([`Undefined::ArrayIndexOutOfRange`]); a damage type outside the armor and damage FX tables ([`Undefined::DamageTypeOutOfRange`]); a float
//! converted to an `Int` outside its range; the `Int` product of a particle count. Each is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::base_type::{max, min, real_to_int_ceil, real_to_int_floor, sqr, Coord3D};
use crate::common::game_common::{
    convert_duration_from_msecs_to_frames, PlayerType, Relationship, VeterancyLevel,
};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::kind_of::{
    KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER, KINDOF_CANNOT_RETALIATE, KINDOF_CAN_BE_REPULSED,
    KINDOF_DRONE, KINDOF_FS_INTERNET_CENTER, KINDOF_IMMOBILE, KINDOF_INFANTRY, KINDOF_PROJECTILE,
    KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use crate::common::module::{ModuleDataKind, MODULEINTERFACE_DAMAGE};
use crate::common::name_key_generator::NameKeyType;
use crate::common::object_status_types::{
    OBJECT_STATUS_AFLAME, OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_DETECTED,
    OBJECT_STATUS_IS_USING_ABILITY, OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_REPULSOR,
    OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::sparse_match_finder::SparseMatchFinder;
use crate::common::thing_template::ThingTemplateAudioType;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::armor::Armor;
use crate::logic::armor_set::{
    ArmorSetFlags, ArmorSetNames, ArmorSetType, ARMORSET_ELITE, ARMORSET_HERO, ARMORSET_VETERAN,
};
use crate::logic::damage::{
    is_subdual_damage, DamageInfo, DamageInfoInput, DamageType, DAMAGE_HEALING, DAMAGE_KILLPILOT,
    DAMAGE_KILL_GARRISONED, DAMAGE_SNIPER, DAMAGE_STATUS, DAMAGE_UNRESISTABLE, DEATH_NORMAL,
};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::{
    BodyDamageType, BodyModule, BodyModuleData, MaxHealthChangeType, ADD_CURRENT_HEALTH_TOO,
    BODY_DAMAGED, BODY_PRISTINE, BODY_REALLYDAMAGED, BODY_RUBBLE, FULLY_HEAL, PRESERVE_RATIO,
    SAME_CURRENTHEALTH,
};
use crate::logic::modules::lend_clone;
use crate::logic::weapon_host::CanAttackResult;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::object_state::mask_of;
use crate::logic::world::update_body::UpdateBody;

/// `YELLOW_DAMAGE_PERCENT`.
pub const YELLOW_DAMAGE_PERCENT: f32 = 0.25;

/// `enum { MAX_BONES = 16 }` of `createParticleSystems`.
const MAX_BONES: usize = 16;

/// `BRIDGE_MAX_TOWERS` (`BridgeBehavior.h`).
pub const BRIDGE_MAX_TOWERS: i32 = 4;

/// `class ActiveBodyModuleData : public BodyModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActiveBodyModuleData {
    /// The `BodyModuleData` base.
    pub body: BodyModuleData,
    /// `m_maxHealth`.
    pub max_health: f32,
    /// `m_initialHealth`.
    pub initial_health: f32,
    /// `m_subdualDamageCap`: "subdual damage will never accumulate past this".
    pub subdual_damage_cap: f32,
    /// `m_subdualDamageHealRate`: "every this often, we drop subdual damage..." (frames).
    pub subdual_damage_heal_rate: u32,
    /// `m_subdualDamageHealAmount`: "by this much".
    pub subdual_damage_heal_amount: f32,
}

/// `ActiveBodyModuleData::buildFieldParse`'s own table.
pub(crate) static FIELD_PARSE: [FieldParse<ActiveBodyModuleData>; 6] = [
    FieldParse::new(
        "MaxHealth",
        |ini, d, _| {
            d.max_health = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "InitialHealth",
        |ini, d, _| {
            d.initial_health = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SubdualDamageCap",
        |ini, d, _| {
            d.subdual_damage_cap = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SubdualDamageHealRate",
        |ini, d, _| {
            d.subdual_damage_heal_rate = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SubdualDamageHealAmount",
        |ini, d, _| {
            d.subdual_damage_heal_amount = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl ActiveBodyModuleData {
    /// `ActiveBodyModuleData::buildFieldParse(p)`: `ModuleData::buildFieldParse`, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, ActiveBodyModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ActiveBodyModuleData> {
        let mut data = ActiveBodyModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ActiveBodyModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class ActiveBody : public BodyModule`.
#[derive(Clone, Debug)]
pub struct ActiveBody {
    /// The `BodyModule` base.
    body: BodyModule,
    /// `getActiveBodyModuleData()`.
    data: Arc<ActiveBodyModuleData>,
    /// `m_currentHealth`.
    current_health: f32,
    /// `m_prevHealth`: "previous health value before current health change op".
    prev_health: f32,
    /// `m_maxHealth`.
    max_health: f32,
    /// `m_initialHealth`.
    initial_health: f32,
    /// `m_currentSubdualDamage`: "starts at zero and goes up".
    current_subdual_damage: f32,
    /// `m_curDamageState`.
    cur_damage_state: BodyDamageType,
    /// `m_nextDamageFXTime`.
    next_damage_fx_time: u32,
    /// `m_lastDamageFXDone`.
    last_damage_fx_done: DamageType,
    /// `m_lastDamageInfo`.
    last_damage_info: DamageInfo,
    /// `m_lastDamageTimestamp`: "frame of last damage dealt".
    last_damage_timestamp: u32,
    /// `m_lastHealingTimestamp`.
    last_healing_timestamp: u32,
    /// `m_frontCrushed`.
    front_crushed: bool,
    /// `m_backCrushed`.
    back_crushed: bool,
    /// `m_lastDamageCleared`.
    last_damage_cleared: bool,
    /// `m_indestructible`.
    indestructible: bool,
    /// `m_particleSystems`: the ids of the body particle systems, head of the list first.
    particle_systems: Vec<u32>,
    /// `m_curArmorSetFlags`.
    cur_armor_set_flags: ArmorSetFlags,
    /// `m_curArmorSet`: the armor template set of the object's template that the armor and damage FX were made from (its position in the template's sets;
    /// `None` is `NULL`).
    cur_armor_set: Option<usize>,
    /// `m_curArmor`.
    cur_armor: Armor,
    /// `m_curDamageFX`: the key of the damage FX in the store (`None` is `NULL`).
    cur_damage_fx: Option<NameKeyType>,
}

impl PartialEq for ActiveBody {
    /// The same object and the same data object (upstream compares pointers): the lent copy of [`out`] is always the state afterwards.
    fn eq(&self, other: &Self) -> bool {
        self.body.behavior().object() == other.body.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

/// The virtual calls `ActiveBody` makes on itself that a derived class overrides. See the module docs.
pub trait ActiveBodyVirtuals: Clone + PartialEq + Sized {
    /// The `ActiveBody` this is (or is the base of).
    fn ab(&self) -> &ActiveBody;
    /// The `ActiveBody` this is (or is the base of).
    fn ab_mut(&mut self) -> &mut ActiveBody;
    /// This module as the body of its slot.
    fn into_body(self) -> UpdateBody;
    /// The module of a body of its slot.
    fn from_body(b: UpdateBody) -> Option<Self>;

    /// `attemptDamage(damageInfo)`: `ActiveBody::attemptDamage` unless overridden.
    fn attempt_damage(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
        attempt_damage(self, world, me, info);
    }

    /// `internalChangeHealth(delta)`: `ActiveBody::internalChangeHealth` unless overridden.
    fn internal_change_health(&mut self, world: &mut World, me: ModuleHandle, delta: f32) {
        internal_change_health(self, world, me, delta);
    }
}

impl ActiveBodyVirtuals for ActiveBody {
    fn ab(&self) -> &ActiveBody {
        self
    }
    fn ab_mut(&mut self) -> &mut ActiveBody {
        self
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ActiveBody(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::ActiveBody(m) => Some(m),
            _ => None,
        }
    }
}

/// Runs a call out to the world with the body's current state lent to its slot (see the module docs). `f` must not use the body.
pub fn out<T: ActiveBodyVirtuals, R>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    f: impl FnOnce(&mut World) -> R,
) -> R {
    lend_clone(world, me, t, T::into_body, T::from_body, |w, _m| f(w))
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

impl ActiveBody {
    /// `setArmorSetFlag(ast)`: "m_curArmorSetFlags.set(ast, 1)" (`UndeadBody::startSecondLife`).
    pub(crate) fn set_armor_set_flag(&mut self, ast: ArmorSetType) {
        self.cur_armor_set_flags.set(ast, 1);
    }

    /// `getInterfaceMask()`: `BodyModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        BodyModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base and the fields (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<ActiveBodyModuleData>>,
    ) -> IniResult<ActiveBody> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let body = BodyModule::new(object, Some(Arc::new(data.body.clone())))?;
        Ok(ActiveBody {
            body,
            current_health: data.initial_health,
            prev_health: data.initial_health,
            max_health: data.max_health,
            initial_health: data.initial_health,
            current_subdual_damage: 0.0,
            cur_damage_state: BODY_PRISTINE,
            next_damage_fx_time: 0,
            last_damage_fx_done: -1,
            last_damage_info: DamageInfo::default(),
            last_damage_timestamp: 0xffff_ffff, // so we don't think we just got damaged on the first frame
            last_healing_timestamp: 0xffff_ffff, // so we don't think we just got healed on the first frame
            front_crushed: false,
            back_crushed: false,
            last_damage_cleared: false,
            indestructible: false,
            particle_systems: Vec::new(),
            cur_armor_set_flags: ArmorSetFlags::new(),
            cur_armor_set: None,
            cur_armor: Armor::default(),
            cur_damage_fx: None,
            data,
        })
    }

    /// `ActiveBody(thing, moduleData)` for a caller that has a world: the first half, then the second.
    pub fn new(
        object: ObjectId,
        module_data: Option<Arc<ActiveBodyModuleData>>,
    ) -> IniResult<ActiveBody> {
        ActiveBody::unconstructed(object, module_data)
    }

    /// The constructor's second half, run by the world once the module is part of its object: "force an initially-valid armor setup", "start us in the right
    /// state".
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.validate_armor_and_damage_fx(world, me.object);
        set_correct_damage_state_of(self, world, me);
    }

    /// The module data (`getActiveBodyModuleData()`).
    pub fn module_data(&self) -> &Arc<ActiveBodyModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        body_module_call(self, world, me, call)
    }

    /// `validateArmorAndDamageFX()`: "you MUST call this before accessing" the armor and damage FX; they are made again when the template's best armor set
    /// for the flags is not the one they came from.
    fn validate_armor_and_damage_fx(&mut self, world: &World, id: ObjectId) {
        let Some(o) = world.objects.find(id) else {
            return;
        };
        let sets = o.template().armor_template_sets();
        // DEBUG_ASSERTCRASH(set, ("findArmorSet should never return null")): a release build goes on with the armor it has
        let Some(i) = SparseMatchFinder::find_best_info_slow::<ArmorSetNames, _>(
            sets,
            &self.cur_armor_set_flags,
        ) else {
            return;
        };
        if Some(i) != self.cur_armor_set {
            let set = &sets[i];
            if let Some(template) = set.get_armor_template() {
                self.cur_armor = world.statics.armor_store.make_armor(Some(template));
            } else if !crate::logic::modules::plant(1212) {
                self.cur_armor.clear();
            }
            if !crate::logic::modules::plant(1213) {
                self.cur_damage_fx = set.get_damage_fx();
            }
            self.cur_armor_set = Some(i);
        }
    }

    /// `m_curArmor.adjustDamage(type, amount)`: an error is a damage type the armor table does not have (undefined upstream).
    fn adjust_damage(&self, world: &World, t: DamageType, amount: f32) -> Result<f32, ()> {
        self.cur_armor
            .adjust_damage(&world.statics.armor_store, t, amount)
            .map_err(|_| ())
    }

    /// `getHealth()`.
    pub fn get_health(&self) -> f32 {
        self.current_health
    }

    /// `getMaxHealth()`.
    pub fn get_max_health(&self) -> f32 {
        self.max_health
    }

    /// `getInitialHealth()`.
    pub fn get_initial_health(&self) -> f32 {
        self.initial_health
    }

    /// `getPreviousHealth()`.
    pub fn get_previous_health(&self) -> f32 {
        self.prev_health
    }

    /// `getDamageState()`.
    pub fn get_damage_state(&self) -> BodyDamageType {
        self.cur_damage_state
    }

    /// `getCurrentSubdualDamageAmount()`.
    pub fn get_current_subdual_damage_amount(&self) -> f32 {
        self.current_subdual_damage
    }

    /// `getLastDamageInfo()`.
    pub fn get_last_damage_info(&self) -> &DamageInfo {
        &self.last_damage_info
    }

    /// `getLastDamageTimestamp()`.
    pub fn get_last_damage_timestamp(&self) -> u32 {
        self.last_damage_timestamp
    }

    /// `getLastHealingTimestamp()`.
    pub fn get_last_healing_timestamp(&self) -> u32 {
        self.last_healing_timestamp
    }

    /// `getClearableLastAttacker()`: "the source of the last damage, unless it has been cleared".
    pub fn get_clearable_last_attacker(&self) -> ObjectId {
        if self.last_damage_cleared {
            INVALID_ID
        } else {
            self.last_damage_info.input.source_id
        }
    }

    /// `isIndestructible()`.
    pub fn is_indestructible(&self) -> bool {
        self.indestructible
    }

    /// `getDamageScalar()` (the base's).
    pub fn get_damage_scalar(&self) -> f32 {
        self.body.get_damage_scalar()
    }

    /// `canBeSubdued()`: "any body with subdue listings can be subdued".
    pub fn can_be_subdued(&self) -> bool {
        self.data.subdual_damage_cap > 0.0
    }

    /// `isSubdued()`.
    pub fn is_subdued(&self) -> bool {
        self.max_health <= self.current_subdual_damage
    }

    /// `internalAddSubdualDamage(delta)`: `min(current + delta, cap)`.
    fn internal_add_subdual_damage(&mut self, delta: f32) {
        self.current_subdual_damage += delta;
        self.current_subdual_damage =
            min(self.current_subdual_damage, self.data.subdual_damage_cap);
    }

    /// The state of the body as words, for a test to compare: health, previous, maximum, initial, subdual, damage state, next damage FX time, last damage FX
    /// done, the two timestamps, the four flags, the armor set flags, the damage scalar, the armor set (a position, or all ones), the number of particle
    /// systems and their ids.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = vec![
            self.current_health.to_bits(),
            self.prev_health.to_bits(),
            self.max_health.to_bits(),
            self.initial_health.to_bits(),
            self.current_subdual_damage.to_bits(),
            self.cur_damage_state as u32,
            self.next_damage_fx_time,
            self.last_damage_fx_done as u32,
            self.last_damage_timestamp,
            self.last_healing_timestamp,
            u32::from(self.front_crushed),
            u32::from(self.back_crushed),
            u32::from(self.last_damage_cleared),
            u32::from(self.indestructible),
            (0..crate::logic::armor_set::ARMORSET_COUNT).fold(0u32, |a, i| {
                a | (u32::from(self.cur_armor_set_flags.test(i)) << i)
            }),
            self.body.get_damage_scalar().to_bits(),
            self.cur_armor_set.map_or(u32::MAX, |i| i as u32),
            self.particle_systems.len() as u32,
        ];
        w.extend(self.particle_systems.iter().copied());
        w
    }
}

// ---- the functions of the base that reach a virtual, generic over the derived class ----------------------------------------------------------------------

/// `calcDamageState(health, maxHealth)`.
fn calc_damage_state(world: &World, health: f32, max_health: f32) -> BodyDamageType {
    let ratio = health / max_health;
    let g = &world.cuts.body.globals;
    if ratio > g.unit_damaged_thresh {
        BODY_PRISTINE
    } else if ratio > g.unit_really_damaged_thresh {
        BODY_DAMAGED
    } else if ratio > 0.0 {
        BODY_REALLYDAMAGED
    } else {
        BODY_RUBBLE
    }
}

/// `setCorrectDamageState()`.
fn set_correct_damage_state_of(ab: &mut ActiveBody, world: &mut World, me: ModuleHandle) {
    ab.cur_damage_state = calc_damage_state(world, ab.current_health, ab.max_health);
    let id = me.object;

    // /// @todo srj -- bleah, this is an icky way to do it. oh well.
    if ab.cur_damage_state == BODY_RUBBLE
        && world
            .objects
            .find(id)
            .is_some_and(|o| o.is_kind_of(KINDOF_STRUCTURE))
    {
        let mut rubble_height = world
            .objects
            .find(id)
            .map_or(0.0, |o| o.template().get_structure_rubble_height());
        if rubble_height <= 0.0 {
            rubble_height = world.cuts.body.globals.default_structure_rubble_height;
        }
        // I had to change this to a Z only version to keep it from disappearing from the PartitionManager for a frame.
        world.set_geometry_info_z(id, rubble_height);

        // Have to tell pathfind as well, as rubble pathfinds differently.
        world.cut(CutCall::PathfinderRemoveObjectFromPathfindMap { object: id });
        world.cut(CutCall::PathfinderAddObjectToPathfindMap { object: id });

        // here we make sure nobody collides with us, ever again...
        // This allows projectiles shot from infantry that are inside rubble to get out of said rubble safely
        if !crate::logic::modules::plant(1214) {
            world.body_set_status(id, mask_of(OBJECT_STATUS_NO_COLLISIONS), true);
        }
    }
}

/// `ActiveBody::internalChangeHealth(delta)`: "simple changing of the health value, it does *NOT* track any transition states for the event of 'damage' or
/// the event of 'death'. ... No game logic should go in here."
pub fn internal_change_health<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    delta: f32,
) {
    let ab = t.ab_mut();
    // save the current health as the previous health
    ab.prev_health = ab.current_health;

    // change the health by the delta, it can be positive or negative
    ab.current_health += delta;

    // high end cap
    let max_health = ab.max_health;
    if ab.current_health > max_health {
        ab.current_health = max_health;
    }

    // low end cap
    let low_end_cap = 0.0f32; // low end cap for health, don't go below this
    if ab.current_health < low_end_cap {
        ab.current_health = low_end_cap;
    }

    // recalc the damage state
    let old_state = ab.cur_damage_state;
    set_correct_damage_state_of(ab, world, me);

    // if our state has changed
    if ab.cur_damage_state != old_state {
        // show a visual change in the model for the damage state, we do not show visual changes for damage states when things are under construction because
        // we just don't have all the art states for that during buildup animation
        let under_construction = world
            .objects
            .find(me.object)
            .is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION));
        if !under_construction {
            evaluate_visual_condition(t, world, me);
        }
    }

    // mark the bit according to our health. (if our AI is dead but our health improves, it will still re-flag this bit in the AIDeadState every frame.)
    let dead = t.ab().current_health <= 0.0;
    world.set_effectively_dead(me.object, dead);
}

/// `evaluateVisualCondition()`: "given the current damage state of the object, evaluate the visual model conditions that have a visual impact on the object".
fn evaluate_visual_condition<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
) {
    let has_drawable = world
        .objects
        .find(me.object)
        .is_some_and(|o| o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID);
    if has_drawable {
        world.emit(ObjectClientEvent::DrawableReactToBodyDamageStateChange {
            object_id: me.object,
            state: t.ab().cur_damage_state,
        });
    }

    // destroy any particle systems that were attached to our body for the old state and create new particle systems for the new state
    update_body_particle_systems(t, world, me);
}

/// `deleteAllParticleSystems()`.
fn delete_all_particle_systems(ab: &mut ActiveBody, world: &mut World) {
    while !ab.particle_systems.is_empty() {
        // get this particle system, and destroy it if the manager still has it
        let id = ab.particle_systems[0];
        world.body_particle_destroy(id);
        // set the body systems head to the next
        ab.particle_systems.remove(0);
    }
}

/// `createParticleSystems(boneBaseName, systemTemplate, maxSystems)`: "create up to maxSystems particle systems of type particleSystemName and attach to
/// bones specified by the bone base name. If there are more bones than maxSystems then the bones will be randomly selected".
fn create_particle_systems(
    ab: &mut ActiveBody,
    world: &mut World,
    me: ModuleHandle,
    bone_base_name: &[u8],
    system_template: Option<&[u8]>,
    mut max_systems: i32,
) {
    let us = me.object;

    // sanity
    let Some(system_template) = system_template else {
        return;
    };

    // get the bones
    let bone_positions =
        world.body_get_multi_logical_bone_position(us, bone_base_name, MAX_BONES as i32);
    let num_bones = bone_positions.len() as i32;

    // if no bones found nothing else to do
    if num_bones == 0 {
        return;
    }

    // if we don't have enough bones to go up to maxSystems, we will change maxSystems to be the number of bones we actually have (we don't want systems
    // doubling up on bones)
    if num_bones < max_systems {
        max_systems = num_bones;
    }

    // create an array that we'll use to mark which bone positions have already been used, this is necessary when we have more bones than particle systems
    // we're going to create, in which case we place the particle systems at random bone locations but don't want to repeat any
    let mut used_bone_indices = [false; MAX_BONES];

    // create the particle systems
    for i in 0..max_systems {
        // pick a bone index to place this particle system at
        let bone_index = world
            .random
            .game_client_random_value(0, max_systems - i - 1);

        // find the actual bone location to use and mark that bone index as used
        let mut count = 0;
        let mut pos: Option<Coord3D> = None;
        for j in 0..num_bones as usize {
            // ignore bone positions that have already been used
            if used_bone_indices[j] {
                continue;
            }

            // this spot is available, if count == boneIndex then use this index
            if count == bone_index {
                pos = Some(bone_positions[j]);
                used_bone_indices[j] = true;
                break; // exit for j
            } else {
                // we won't use this index, increment count until we find a suitable index to use
                count += 1;
            }
        }

        // `pos` is read unassigned if no index was chosen (DEBUG_ASSERTCRASH(j != numBones))
        let Some(pos) = pos else {
            undefined(world, me, Undefined::UninitialisedLocal);
            return;
        };

        // create particle system here; set the position of the particle system in local object space
        let id = world.create_particle_system(system_template.to_vec(), pos);
        // attach particle system to object
        world.body_particle_attach_to_object(id, us);

        // create a new body particle system entry and keep this particle system in it
        ab.particle_systems.insert(0, id);
    }
}

/// `updateBodyParticleSystems()`: "this function is called on state changes only. Body Type or Aflameness."
fn update_body_particle_systems<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
) {
    let g = world.cuts.body.globals.clone();
    let find = |world: &mut World, name: &[u8]| -> Option<Vec<u8>> {
        world
            .body_particle_find_template(name)
            .then(|| name.to_vec())
    };
    let fire_small_template = find(world, &g.auto_fire[0].system);
    let fire_medium_template = find(world, &g.auto_fire[1].system);
    let fire_large_template = find(world, &g.auto_fire[2].system);
    let smoke_small_template = find(world, &g.auto_smoke[0].system);
    let smoke_medium_template = find(world, &g.auto_smoke[1].system);
    let smoke_large_template = find(world, &g.auto_smoke[2].system);
    let aflame_template = find(world, &g.auto_aflame.system);

    let aflame = world
        .objects
        .find(me.object)
        .is_some_and(|o| o.test_status(OBJECT_STATUS_AFLAME));
    // when we're aflame, we use a slightly different set of particle systems that are auto created that lends itself to more fire and bigger fire
    let (
        fire_small,
        fire_medium,
        fire_large,
        smoke_small,
        smoke_medium,
        smoke_large,
        count_modifier,
    );
    if aflame {
        fire_small = fire_medium_template.clone(); // small fire becomes medium fire
        fire_medium = fire_large_template.clone(); // medium fire becomes large fire
        fire_large = fire_large_template.clone(); // large fire stays large
        smoke_small = fire_small_template.clone(); // small smoke becomes small fire
        smoke_medium = fire_small_template.clone(); // medium smoke becomes small fire
        smoke_large = fire_small_template.clone(); // large smoke becomes small fire

        // we get to make more of them all too
        count_modifier = 2;
    } else {
        // use regular templates
        fire_small = fire_small_template.clone();
        fire_medium = fire_medium_template.clone();
        fire_large = fire_large_template.clone();
        smoke_small = smoke_small_template.clone();
        smoke_medium = smoke_medium_template.clone();
        smoke_large = smoke_large_template.clone();

        // we make just the normal amount of these
        count_modifier = 1;
    }

    // remove any particle systems we have currently in the list in favor of any new ones that we're going to autopopulate ourselves with
    delete_all_particle_systems(t.ab_mut(), world);

    // create particle systems for the new body state
    let classes: [(
        &crate::logic::world::body_cuts::AutoParticle,
        &Option<Vec<u8>>,
    ); 6] = [
        (&g.auto_fire[0], &fire_small),
        (&g.auto_fire[1], &fire_medium),
        (&g.auto_fire[2], &fire_large),
        (&g.auto_smoke[0], &smoke_small),
        (&g.auto_smoke[1], &smoke_medium),
        (&g.auto_smoke[2], &smoke_large),
    ];
    for (class, template) in classes {
        let Some(max) = class.max.checked_mul(count_modifier) else {
            undefined(world, me, Undefined::SignedIntOverflow);
            return;
        };
        create_particle_systems(
            t.ab_mut(),
            world,
            me,
            &class.prefix,
            template.as_deref(),
            max,
        );
    }

    // actively on fire
    if aflame {
        let Some(max) = g.auto_aflame.max.checked_mul(count_modifier) else {
            undefined(world, me, Undefined::SignedIntOverflow);
            return;
        };
        create_particle_systems(
            t.ab_mut(),
            world,
            me,
            &g.auto_aflame.prefix,
            aflame_template.as_deref(),
            max,
        );
    }
}

/// `doDamageFX(damageInfo)`.
fn do_damage_fx<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    info: &DamageInfo,
) {
    let mut damage_type_to_use = info.input.damage_type;
    if info.input.damage_fx_override != DAMAGE_UNRESISTABLE && !crate::logic::modules::plant(1215) {
        // Just the visual aspect of damage can be overridden in some cases. Unresistable is the default to mean no override, as we are out of bits.
        damage_type_to_use = info.input.damage_fx_override;
    }

    if let Some(key) = t.ab().cur_damage_fx {
        let now = world.frame();
        let ab = t.ab_mut();
        if damage_type_to_use == ab.last_damage_fx_done
            && (ab.next_damage_fx_time > now) != crate::logic::modules::plant(1216)
        {
            return;
        }
        let source = world
            .objects
            .find(info.input.source_id)
            .map(|o| (info.input.source_id, o.get_veterancy_level())); // might be null, I guess
        ab.last_damage_fx_done = damage_type_to_use;
        let level = source.map_or(VeterancyLevel::Regular, |(_, l)| l);
        let Some(dfx) = world.statics.damage_fx_store.get(key) else {
            return;
        };
        let Ok(throttle) = dfx.get_damage_fx_throttle_time(damage_type_to_use, level) else {
            undefined(world, me, Undefined::DamageTypeOutOfRange);
            return;
        };
        ab.next_damage_fx_time = now.wrapping_add(throttle);
        // DamageFX::doDamageFX: since the victim is receiving the damage, it's the "primary" object. The source is the "secondary" object.
        let fx =
            match dfx.get_damage_fx_list(damage_type_to_use, info.out.actual_damage_dealt, level) {
                Ok(fx) => fx.map(|f| f.to_vec()),
                Err(_) => {
                    undefined(world, me, Undefined::DamageTypeOutOfRange);
                    return;
                }
            };
        if let Some(fx) = fx {
            world.emit(ObjectClientEvent::FxListDoFxObjSecondary {
                fx,
                object_id: me.object,
                secondary: source.map_or(INVALID_ID, |(id, _)| id),
            });
        }
    }
}

/// `shouldRetaliateAgainstAggressor(obj, damager)`: "this considers whether obj should invoke his friends to retaliate against damager. Note that obj could be
/// a structure, so we don't actually check whether obj will retaliate, as in many cases he wouldn't."
fn should_retaliate_against_aggressor(
    world: &mut World,
    obj: ObjectId,
    damager: Option<ObjectId>,
) -> bool {
    let Some(damager) = damager else { return false };
    if world
        .objects
        .find(damager)
        .is_some_and(|d| d.test_status(OBJECT_STATUS_AIRBORNE_TARGET))
    {
        return false; // Don't retaliate against aircraft.
    }
    if world.get_relationship(damager, obj) != Relationship::Enemies {
        return false; // only retaliate against enemies.
    }
    let dist_sqr = world.body_get_distance_squared(obj, damager);
    if dist_sqr > sqr(world.cuts.body.globals.max_retaliate_distance) {
        return false;
    }
    // Only human players retaliate.
    let human = world
        .get_controlling_player(obj)
        .and_then(|p| world.players.get_nth(p))
        .is_some_and(|p| p.player_type == PlayerType::Human);
    if !human {
        return false;
    }
    // Drones never retaliate.
    if world
        .objects
        .find(obj)
        .is_some_and(|o| o.is_kind_of(KINDOF_DRONE))
    {
        return false;
    }
    true
}

/// `shouldRetaliate(obj)`.
fn should_retaliate(world: &mut World, obj: ObjectId) -> bool {
    let Some(o) = world.objects.find(obj) else {
        return false;
    };
    // Cannot retaliate objects dont.
    if o.is_kind_of(KINDOF_CANNOT_RETALIATE) {
        return false;
    }
    if o.is_kind_of(KINDOF_IMMOBILE) {
        return false;
    }
    // Drones never retaliate.
    if o.is_kind_of(KINDOF_DRONE) {
        return false;
    }
    // Any unit that isn't idle won't retaliate.
    let ai = o.ai;
    let (stealthed, detected, using_ability) = (
        o.status.test(OBJECT_STATUS_STEALTHED),
        o.status.test(OBJECT_STATUS_DETECTED),
        o.test_status(OBJECT_STATUS_IS_USING_ABILITY),
    );
    match ai {
        Some(ai) => {
            if !world.module_call(obj, ai, ModuleCall::AiIsIdle).as_bool() {
                return false;
            }
        }
        None => return false, // Non-ai can't retaliate.
    }
    // Stealthed units don't retaliate unless they're detected.
    if stealthed && !detected {
        return false;
    }
    // If we're using an ability, don't stop.
    if using_ability {
        return false;
    }
    true
}

/// `attemptDamage(damageInfo)`: "try to damage this object".
pub fn attempt_damage<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    info: &mut DamageInfo,
) {
    let obj = me.object;
    t.ab_mut().validate_armor_and_damage_fx(world, obj);

    if t.ab().indestructible && !crate::logic::modules::plant(1217) {
        return;
    }

    // initialize these, just in case we bail out early
    info.out.actual_damage_dealt = 0.0;
    info.out.actual_damage_clipped = 0.0;

    // we cannot damage again objects that are already dead
    match world.objects.find(obj) {
        Some(o) if o.is_effectively_dead() => return,
        None => return,
        _ => {}
    }

    let damager: Option<ObjectId> = world
        .objects
        .find(info.input.source_id)
        .map(|_| info.input.source_id);
    if let Some(d) = damager {
        if !crate::logic::modules::plant(1218) {
            // Store the template so later if the attacking object dies, we use script conditions to look at the damager's template inside
            // evaluateTeamAttackedByType or evaluateNameAttackedByType.
            info.input.source_template = world
                .objects
                .find(d)
                .map_or_else(Vec::new, |o| o.template().get_name().to_vec());
        }
    }

    let mut already_handled = false;
    let mut allow_modifier = true;
    let Ok(mut amount) = t
        .ab()
        .adjust_damage(world, info.input.damage_type, info.input.amount)
    else {
        undefined(world, me, Undefined::DamageTypeOutOfRange);
        return;
    };

    match info.input.damage_type {
        DAMAGE_HEALING => {
            if !info.input.kill || crate::logic::modules::plant(1219) {
                // Healing and Damage are separate, so this shouldn't happen
                attempt_healing(t, world, me, info);
            }
            return;
        }

        DAMAGE_KILLPILOT => {
            // This type of damage doesn't actually damage the unit, but it does kill it's pilot, in the case of a vehicle.
            if world
                .objects
                .find(obj)
                .is_some_and(|o| o.is_kind_of(KINDOF_VEHICLE))
            {
                // Handle special case for combat bike. We actually will kill the bike by forcing the rider to leave the bike. That way the bike will
                // automatically scuttle and be unusable.
                let contain = world.get_contain(obj);
                let rider_change = match contain {
                    Some(c) => out(t, world, me, |w| {
                        w.module_call(obj, c, ModuleCall::IsRiderChangeContain)
                    })
                    .as_bool(),
                    None => false,
                };
                if rider_change {
                    let contain = contain.expect("rider_change implies a contain module");
                    let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) else {
                        undefined(world, me, Undefined::NullAi);
                        return;
                    };

                    let moving = out(t, world, me, |w| {
                        w.module_call(obj, ai, ModuleCall::AiIsMoving)
                    })
                    .as_bool();
                    if moving != crate::logic::modules::plant(1220) {
                        // Bike is moving, so just blow it up instead.
                        if let Some(d) = damager {
                            out(t, world, me, |w| w.body_score_the_kill(d, obj));
                        }
                        out(t, world, me, |w| {
                            w.kill(obj, DAMAGE_UNRESISTABLE, DEATH_NORMAL)
                        });
                    } else {
                        // Removing the rider will scuttle the bike.
                        let items = match out(t, world, me, |w| {
                            w.module_call(obj, contain, ModuleCall::GetContainedItems)
                        }) {
                            ModuleReply::Ids(ids) => ids,
                            _ => Vec::new(),
                        };
                        let Some(&rider) = items.first() else {
                            undefined(world, me, Undefined::EmptyContainedList);
                            return;
                        };
                        out(t, world, me, |w| {
                            w.module_call(
                                obj,
                                ai,
                                ModuleCall::AiEvacuateInstantly {
                                    expose_stealth_units: true,
                                },
                            )
                        });

                        // Kill the rider.
                        if let Some(d) = damager {
                            out(t, world, me, |w| w.body_score_the_kill(d, rider));
                        }
                        out(t, world, me, |w| {
                            w.kill(rider, DAMAGE_UNRESISTABLE, DEATH_NORMAL)
                        });
                    }
                } else {
                    // Make it unmanned, so units can easily check the ability to "take control of it"
                    out(t, world, me, |w| {
                        w.body_set_disabled(obj, crate::common::disabled_types::DISABLED_UNMANNED)
                    });
                    world.cut(CutCall::GameLogicDeselectObject {
                        object: obj,
                        player_mask: crate::common::game_common::PLAYERMASK_ALL,
                        affect_client: true,
                    });

                    if let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) {
                        out(t, world, me, |w| w.module_call(obj, ai, ModuleCall::AiIdle));
                    }

                    // Convert it to the neutral team so it renders gray giving visual representation that it is unmanned.
                    out(t, world, me, |w| w.body_set_team_to_neutral_default(obj));
                }

                // We don't care which team sniped the vehicle... we use this information to flag whether or not we captured a vehicle.
                if !crate::logic::modules::plant(1221) {
                    out(t, world, me, |w| w.body_academy_vehicle_sniped());
                }
            }
            already_handled = true;
            allow_modifier = false;
        }

        DAMAGE_KILL_GARRISONED => {
            // KRIS: READ THIS!!!
            // This code is very misleading (but in a good way). One would think this is an excellent place to add the hook to kill garrisoned troops. And that
            // is a correct assumption. Unfortunately, the vast majority of garrison slayings are performed in DumbProjectileBehavior::projectileHandleCollision(),
            // so my hope is that this message will save you some research time!
            let amount_in = info.input.amount;
            if !amount_in.is_finite()
                || amount_in.floor() < i32::MIN as f32
                || amount_in.floor() >= i32::MAX as f32
            {
                undefined(world, me, Undefined::FloatToIntOutOfRange);
                return;
            }
            let kills_to_make = real_to_int_floor(amount_in);
            if let Some(contain) = world.get_contain(obj) {
                let count = out(t, world, me, |w| {
                    w.module_call(obj, contain, ModuleCall::GetContainCount)
                });
                let count = match count {
                    ModuleReply::Int(n) => n,
                    _ => 0,
                };
                let garrisonable = count > 0
                    && out(t, world, me, |w| {
                        w.module_call(obj, contain, ModuleCall::IsGarrisonable)
                    })
                    .as_bool();
                let may_kill = garrisonable
                    && !out(t, world, me, |w| {
                        w.module_call(obj, contain, ModuleCall::IsImmuneToClearBuildingAttacks)
                    })
                    .as_bool();
                if may_kill {
                    let mut num_killed = 0;

                    // garrisonable buildings subvert the normal process here.
                    let items = match out(t, world, me, |w| {
                        w.module_call(obj, contain, ModuleCall::GetContainedItems)
                    }) {
                        ModuleReply::Ids(ids) => ids,
                        _ => Vec::new(),
                    };
                    for thing_to_kill in items {
                        if num_killed >= kills_to_make && !crate::logic::modules::plant(1222) {
                            break;
                        }
                        let dead = world
                            .objects
                            .find(thing_to_kill)
                            .is_none_or(|o| o.is_effectively_dead());
                        if !dead || crate::logic::modules::plant(1223) {
                            if let Some(d) = damager {
                                out(t, world, me, |w| w.body_score_the_kill(d, thing_to_kill));
                            }
                            out(t, world, me, |w| {
                                w.kill(thing_to_kill, DAMAGE_UNRESISTABLE, DEATH_NORMAL)
                            });
                            num_killed += 1;
                            let Some(player) = world.get_controlling_player(thing_to_kill) else {
                                undefined(world, me, Undefined::NullControllingPlayer);
                                return;
                            };
                            out(t, world, me, |w| {
                                w.body_academy_cleared_garrisoned_building(player)
                            });
                        }
                    }
                }
            }
            already_handled = true;
            allow_modifier = false;
        }

        DAMAGE_STATUS => {
            // Damage amount is msec time we set the status given in damageStatusType
            let real_frames_to_status_for = convert_duration_from_msecs_to_frames(amount);
            if !real_frames_to_status_for.is_finite()
                || real_frames_to_status_for.abs() >= i32::MAX as f32
            {
                undefined(world, me, Undefined::FloatToIntOutOfRange);
                return;
            }
            let frames = if crate::logic::modules::plant(1224) {
                real_to_int_floor(real_frames_to_status_for)
            } else {
                real_to_int_ceil(real_frames_to_status_for)
            };
            let status = info.input.damage_status_type;
            out(t, world, me, |w| {
                w.body_do_status_damage(obj, status, frames)
            });
            already_handled = true;
            allow_modifier = false;
        }

        _ => {}
    }

    if is_subdual_damage(info.input.damage_type) {
        if !can_be_subdued_of(t.ab()) && !crate::logic::modules::plant(1225) {
            return;
        }

        let was_subdued = t.ab().is_subdued();
        t.ab_mut().internal_add_subdual_damage(amount);
        let now_subdued = t.ab().is_subdued();
        already_handled = true;
        allow_modifier = false;

        if was_subdued != now_subdued {
            on_subdual_change(t, world, me, now_subdued);
        }

        if !crate::logic::modules::plant(1226) {
            out(t, world, me, |w| w.body_notify_subdual_damage(obj, amount));
        }
    }

    if allow_modifier && info.input.damage_type != DAMAGE_UNRESISTABLE {
        // Apply the damage scalar (extra bonuses -- like strategy center defensive battle plan)
        // And remember not to adjust unresistable damage, just like the armor code can't.
        amount *= t.ab().body.get_damage_scalar();
    } else if crate::logic::modules::plant(1227) && info.input.damage_type == DAMAGE_UNRESISTABLE {
        amount *= t.ab().body.get_damage_scalar();
    }

    // sanity check the damage value -- can't apply negative damage
    if amount > 0.0 || info.input.kill {
        let old_state = t.ab().cur_damage_state;

        // If the object is going to die, make sure we damage all remaining health.
        if info.input.kill && !crate::logic::modules::plant(1228) {
            amount = t.ab().current_health;
        }

        if !already_handled {
            // do the damage simplistic damage subtraction
            t.internal_change_health(world, me, -amount);
        }

        // record the actual damage done from this, and when it happened
        info.out.actual_damage_dealt = amount;
        info.out.actual_damage_clipped = if crate::logic::modules::plant(1229) {
            amount
        } else {
            t.ab().prev_health - t.ab().current_health
        };

        // then copy the whole DamageInfo struct for easy lookup (object pointer loses scope as soon as atteptdamage's caller ends)
        // m_lastDamageTimestamp is initialized to FFFFFFFFFF, so doing a < compare is problematic. jba.
        let frame = world.frame();
        let ab = t.ab_mut();
        if (ab.last_damage_timestamp != frame && ab.last_damage_timestamp != frame.wrapping_sub(1))
            || (crate::logic::modules::plant(1230) && ab.last_damage_timestamp != frame)
        {
            ab.last_damage_info = info.clone();
            ab.last_damage_cleared = false;
            ab.last_damage_timestamp = frame;
        } else {
            // Multiple damages applied in one/next frame. We prefer the one that tells who the attacker is.
            let src_obj1 = world
                .objects
                .find(ab.last_damage_info.input.source_id)
                .is_some();
            let src_obj2 = world.objects.find(info.input.source_id).is_some();
            if src_obj2 {
                if src_obj1 {
                    let s = info.input.source_id;
                    let preferred = world.objects.find(s).is_some_and(|o| {
                        o.is_kind_of(KINDOF_VEHICLE) || o.is_kind_of(KINDOF_INFANTRY)
                    }) || (world.is_faction_structure(s)
                        && !crate::logic::modules::plant(1231));
                    if preferred {
                        ab.last_damage_info = info.clone();
                        ab.last_damage_cleared = false;
                        ab.last_damage_timestamp = frame;
                    }
                } else {
                    ab.last_damage_info = info.clone();
                    ab.last_damage_cleared = false;
                    ab.last_damage_timestamp = frame;
                }
            } else {
                // no change.
            }
        }

        // Notify the player that they have been attacked by this player
        let last_source = t.ab().last_damage_info.input.source_id;
        if last_source != INVALID_ID
            && !crate::logic::modules::plant(1232)
            && world.objects.find(last_source).is_some()
        {
            {
                let src_player = world.get_controlling_player(last_source);
                let our_player = world.get_controlling_player(obj);
                let (Some(src_player), Some(our_player)) = (src_player, our_player) else {
                    undefined(world, me, Undefined::NullControllingPlayer);
                    return;
                };
                let frame = world.frame();
                if let Some(p) = world.players.get_nth_mut(our_player) {
                    if let Some(b) = p.attacked_by.get_mut(src_player as usize) {
                        *b = true;
                    }
                    p.attacked_frame = frame;
                }
            }
        }

        // if our health has gone down then do run the damage module callback
        if t.ab().current_health < t.ab().prev_health || crate::logic::modules::plant(1233) {
            for idx in world.modules_with(obj, MODULEINTERFACE_DAMAGE) {
                let i = info.clone();
                out(t, world, me, |w| {
                    w.module_call(obj, idx, ModuleCall::OnDamage(i))
                });
            }
        }

        if t.ab().cur_damage_state != old_state {
            let new_state = t.ab().cur_damage_state;
            if !crate::logic::modules::plant(1234) {
                for idx in world.modules_with(obj, MODULEINTERFACE_DAMAGE) {
                    let i = info.clone();
                    out(t, world, me, |w| {
                        w.module_call(
                            obj,
                            idx,
                            ModuleCall::OnBodyDamageStateChange {
                                info: i,
                                old_state,
                                new_state,
                            },
                        )
                    });
                }
            }

            // @todo: This really feels like it should be in the TransitionFX lists.
            if new_state == BODY_DAMAGED {
                let name = world.objects.find(obj).map_or_else(Vec::new, |o| {
                    o.template()
                        .get_audio(ThingTemplateAudioType::SoundOnDamaged)
                        .event_name
                        .clone()
                });
                if !crate::logic::modules::plant(1235) {
                    world.emit(ObjectClientEvent::AudioAddEvent {
                        name,
                        object_id: obj,
                        position: None,
                        player_index: None,
                    });
                }
            } else if new_state == BODY_REALLYDAMAGED {
                let name = world.objects.find(obj).map_or_else(Vec::new, |o| {
                    o.template()
                        .get_audio(ThingTemplateAudioType::SoundOnReallyDamaged)
                        .event_name
                        .clone()
                });
                world.emit(ObjectClientEvent::AudioAddEvent {
                    name,
                    object_id: obj,
                    position: None,
                    player_index: None,
                });
            }
        }

        // Should we play our fear sound?
        let (prev_health, current_health, max_health) =
            (t.ab().prev_health, t.ab().current_health, t.ab().max_health);
        if (prev_health / max_health) > YELLOW_DAMAGE_PERCENT
            && (current_health / max_health) < YELLOW_DAMAGE_PERCENT
            && (current_health > 0.0)
        {
            // 25% chance to play
            let limit = if crate::logic::modules::plant(1236) {
                26
            } else {
                25
            };
            if world.random.game_logic_random_value(0, 99) < limit {
                let (name, position) = world.objects.find(obj).map_or_else(
                    || {
                        (
                            Vec::new(),
                            Coord3D {
                                x: 0.0,
                                y: 0.0,
                                z: 0.0,
                            },
                        )
                    },
                    |o| {
                        (
                            o.template()
                                .get_audio(ThingTemplateAudioType::VoiceFear)
                                .event_name
                                .clone(),
                            o.get_position(),
                        )
                    },
                );
                let Some(player) = world.get_controlling_player(obj) else {
                    undefined(world, me, Undefined::NullControllingPlayer);
                    return;
                };
                world.emit(ObjectClientEvent::AudioAddEvent {
                    name,
                    object_id: INVALID_ID,
                    position: Some(position),
                    player_index: Some(player),
                });
            }
        }

        // check to see if we died
        let died = if crate::logic::modules::plant(1237) {
            current_health <= 0.0
        } else {
            current_health <= 0.0 && prev_health > 0.0
        };
        if died {
            // Give our killer credit for killing us, if there is one.
            if let Some(d) = damager {
                if !crate::logic::modules::plant(1238) {
                    out(t, world, me, |w| w.body_score_the_kill(d, obj));
                }
            }

            let i = info.clone();
            out(t, world, me, |w| w.body_on_die(obj, &i));
        }
    }

    do_damage_fx(t, world, me, info);

    // Damaged repulsable civilians scare (repulse) other civs. jba.
    if world.cuts.globals.enable_repulsors
        && world
            .objects
            .find(obj)
            .is_some_and(|o| o.is_kind_of(KINDOF_CAN_BE_REPULSED))
    {
        out(t, world, me, |w| {
            w.body_set_status(obj, mask_of(OBJECT_STATUS_REPULSOR), true)
        });
    }

    // Retaliate, even if I'm dead -- we'll still get my nearby friends to get revenge!!!
    // Also only retaliate if we're controlled by a human player and the thing that attacked me is an enemy.
    if let Some(controlling_player) = world.get_controlling_player(obj) {
        let human_retaliating = world.players.get_nth(controlling_player).is_some_and(|p| {
            p.logical_retaliation_mode_enabled && p.player_type == PlayerType::Human
        });
        if human_retaliating && should_retaliate_against_aggressor(world, obj, damager) {
            let (position, radius) = {
                let o = world.objects.find(obj).expect("checked above");
                (
                    o.get_position(),
                    o.geometry_info.get_bounding_circle_radius(),
                )
            };
            let distance = world.cuts.body.globals.retaliate_friends_radius + radius;
            let friends =
                world.body_iterate_retaliation_friends(position, distance, controlling_player);
            let damager =
                damager.expect("shouldRetaliateAgainstAggressor holds only with a damager");
            for them in friends {
                if !should_retaliate(world, them) {
                    continue;
                }
                let Some(ai) = world.objects.find(them).and_then(|o| o.ai) else {
                    continue;
                };
                // If we have AI and we're mobile, then assist!
                if !world
                    .objects
                    .find(them)
                    .is_some_and(|o| o.is_kind_of(KINDOF_IMMOBILE))
                {
                    // But only if we can attack it!
                    let result = world.body_get_able_to_attack_specific_object(them, damager);
                    if result == CanAttackResult::PossibleAfterMoving as i32
                        || result == CanAttackResult::Possible as i32
                    {
                        let pos = world.objects.find(them).map_or(
                            Coord3D {
                                x: 0.0,
                                y: 0.0,
                                z: 0.0,
                            },
                            |o| o.get_position(),
                        );
                        out(t, world, me, |w| {
                            w.module_call(
                                them,
                                ai,
                                ModuleCall::AiGuardRetaliate {
                                    victim: damager,
                                    pos,
                                },
                            )
                        });
                    }
                }
            }
        }
    }
}

/// `canBeSubdued()` for the free functions.
fn can_be_subdued_of(ab: &ActiveBody) -> bool {
    ab.can_be_subdued()
}

/// `onSubdualChange(isNowSubdued)`: "override this if you want a totally different effect than DISABLED_SUBDUED".
fn on_subdual_change<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    is_now_subdued: bool,
) {
    let obj = me.object;
    let projectile = world
        .objects
        .find(obj)
        .is_some_and(|o| o.is_kind_of(KINDOF_PROJECTILE));
    if !projectile {
        if is_now_subdued {
            out(t, world, me, |w| {
                w.body_set_disabled(obj, crate::common::disabled_types::DISABLED_SUBDUED)
            });

            if let Some(contain) = world.get_contain(obj) {
                out(t, world, me, |w| {
                    w.module_call(obj, contain, ModuleCall::OrderAllPassengersToIdle)
                });
            }
        } else {
            out(t, world, me, |w| {
                w.body_clear_disabled(obj, crate::common::disabled_types::DISABLED_SUBDUED)
            });

            if world
                .objects
                .find(obj)
                .is_some_and(|o| o.is_kind_of(KINDOF_FS_INTERNET_CENTER))
            {
                // Kris: October 20, 2003 - Patch 1.01
                // Any unit inside an internet center is a hacker! Order them to start hacking again.
                if let Some(contain) = world.get_contain(obj) {
                    out(t, world, me, |w| {
                        w.module_call(obj, contain, ModuleCall::OrderAllPassengersToHackInternet)
                    });
                }
            }
        }
    } else if is_now_subdued {
        // There is no coming back from being jammed, and projectiles can't even heal, but this makes it clear.
        if let Some(pui) = world.get_projectile_update_interface(obj) {
            out(t, world, me, |w| {
                w.module_call(obj, pui, ModuleCall::ProjectileNowJammed)
            });
        }
    }
}

/// `attemptHealing(damageInfo)`: "try to heal this object".
pub fn attempt_healing<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    info: &mut DamageInfo,
) {
    let obj = me.object;
    t.ab_mut().validate_armor_and_damage_fx(world, obj);

    if info.input.damage_type != DAMAGE_HEALING {
        // Healing and Damage are separate, so this shouldn't happen
        t.attempt_damage(world, me, info);
        return;
    }

    // srj sez: sorry, once yer dead, yer dead.
    // Special case for bridges, cause the system now things they're dead
    // @todo we need to figure out what has changed so we don't have to hack this (CBD 11-1-2002)
    let Some(o) = world.objects.find(obj) else {
        return;
    };
    if !o.is_kind_of(KINDOF_BRIDGE) && !o.is_kind_of(KINDOF_BRIDGE_TOWER) && o.is_effectively_dead()
    {
        return;
    }

    // initialize these, just in case we bail out early
    info.out.actual_damage_dealt = 0.0;
    info.out.actual_damage_clipped = 0.0;

    let Ok(amount) = t
        .ab()
        .adjust_damage(world, info.input.damage_type, info.input.amount)
    else {
        undefined(world, me, Undefined::DamageTypeOutOfRange);
        return;
    };

    // sanity check the damage value -- can't apply negative healing
    if amount > 0.0 {
        let old_state = t.ab().cur_damage_state;

        // do the damage simplistic damage ADDITION
        t.internal_change_health(world, me, amount);

        // record the actual damage done from this, and when it happened
        info.out.actual_damage_dealt = amount;
        info.out.actual_damage_clipped = t.ab().prev_health - t.ab().current_health;

        // then copy the whole DamageInfo struct for easy lookup (object pointer loses scope as soon as atteptdamage's caller ends)
        let frame = world.frame();
        let ab = t.ab_mut();
        ab.last_damage_info = info.clone();
        ab.last_damage_cleared = false;
        ab.last_damage_timestamp = frame;
        ab.last_healing_timestamp = frame;

        // if our health has gone UP then do run the damage module callback
        if t.ab().current_health > t.ab().prev_health {
            for idx in world.modules_with(obj, MODULEINTERFACE_DAMAGE) {
                let i = info.clone();
                out(t, world, me, |w| {
                    w.module_call(obj, idx, ModuleCall::OnHealing(i))
                });
            }
        }

        if t.ab().cur_damage_state != old_state {
            let new_state = t.ab().cur_damage_state;
            for idx in world.modules_with(obj, MODULEINTERFACE_DAMAGE) {
                let i = info.clone();
                out(t, world, me, |w| {
                    w.module_call(
                        obj,
                        idx,
                        ModuleCall::OnBodyDamageStateChange {
                            info: i,
                            old_state,
                            new_state,
                        },
                    )
                });
            }
        }
    }

    do_damage_fx(t, world, me, info);
}

/// `estimateDamage(damageInfo)`.
fn estimate_damage<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    input: &DamageInfoInput,
) -> f32 {
    let obj = me.object;
    t.ab_mut().validate_armor_and_damage_fx(world, obj);

    // Subdual damage can't affect you if you can't be subdued
    if is_subdual_damage(input.damage_type)
        && !t.ab().can_be_subdued()
        && !crate::logic::modules::plant(1239)
    {
        return 0.0;
    }

    if input.damage_type == DAMAGE_KILL_GARRISONED {
        if let Some(contain) = world.get_contain(obj) {
            let count = match out(t, world, me, |w| {
                w.module_call(obj, contain, ModuleCall::GetContainCount)
            }) {
                ModuleReply::Int(n) => n,
                _ => 0,
            };
            if count > 0
                && out(t, world, me, |w| {
                    w.module_call(obj, contain, ModuleCall::IsGarrisonable)
                })
                .as_bool()
                && (!out(t, world, me, |w| {
                    w.module_call(obj, contain, ModuleCall::IsImmuneToClearBuildingAttacks)
                })
                .as_bool()
                    || crate::logic::modules::plant(1240))
            {
                return 1.0;
            }
            return 0.0;
        }
        return 0.0;
    }

    if input.damage_type == DAMAGE_SNIPER && !crate::logic::modules::plant(1241) {
        let under_construction_structure = world.objects.find(obj).is_some_and(|o| {
            o.is_kind_of(KINDOF_STRUCTURE) && o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION)
        });
        if under_construction_structure {
            // If we're a pathfinder shooting a stinger site under construction... don't. Special case code.
            return 0.0;
        }
    }

    match t.ab().adjust_damage(world, input.damage_type, input.amount) {
        Ok(amount) => amount,
        Err(()) => {
            undefined(world, me, Undefined::DamageTypeOutOfRange);
            0.0
        }
    }
}

/// `setDamageState(newState)`: "control damage state directly. Will adjust hitpoints."
fn set_damage_state<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    new_state: BodyDamageType,
) {
    let g = &world.cuts.body.globals;
    let ratio = match new_state {
        BODY_PRISTINE => 1.0,
        BODY_DAMAGED => g.unit_damaged_thresh,
        BODY_REALLYDAMAGED => {
            if crate::logic::modules::plant(1242) {
                g.unit_damaged_thresh
            } else {
                g.unit_really_damaged_thresh
            }
        }
        BODY_RUBBLE => 0.0,
        _ => 1.0,
    };
    let ab = t.ab();
    let mut desired_health = ab.max_health * ratio - 1.0; // -1 because < not <= in calcState
    desired_health = max(desired_health, 0.0);
    let delta = desired_health - ab.current_health;
    t.internal_change_health(world, me, delta);
    set_correct_damage_state_of(t.ab_mut(), world, me);
}

/// `setInitialHealth(initialPercent)`: "simple setting of the health value, it does *NOT* track any transition states for the event of 'damage' or the event
/// of 'death'".
fn set_initial_health<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    initial_percent: i32,
) {
    // save the current health as the previous health
    let ab = t.ab_mut();
    ab.prev_health = ab.current_health;

    let factor = initial_percent as f32 / 100.0;
    let new_health = factor
        * if crate::logic::modules::plant(1244) {
            ab.max_health
        } else {
            ab.initial_health
        };

    // change the health to the requested percentage.
    let delta = new_health - ab.current_health;
    t.internal_change_health(world, me, delta);
}

/// `setMaxHealth(maxHealth, healthChangeType)`.
pub(crate) fn set_max_health<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    max_health: f32,
    health_change_type: MaxHealthChangeType,
) {
    let ab = t.ab_mut();
    let prev_max_health = ab.max_health;
    ab.max_health = max_health;
    ab.initial_health = max_health;

    match health_change_type {
        PRESERVE_RATIO => {
            // 400/500 (80%) + 100 becomes 480/600 (80%)
            // 200/500 (40%) - 100 becomes 160/400 (40%)
            let ratio = ab.current_health
                / if crate::logic::modules::plant(1245) {
                    max_health
                } else {
                    prev_max_health
                };
            let new_health = max_health * ratio;
            let delta = new_health - ab.current_health;
            t.internal_change_health(world, me, delta);
        }
        ADD_CURRENT_HEALTH_TOO => {
            // Add the same amount that we are adding to the max health.
            // This could kill you if max health is reduced (if we ever have that ability to add buffer health like in D&D)
            // 400/500 (80%) + 100 becomes 500/600 (83%)
            // 200/500 (40%) - 100 becomes 100/400 (25%)
            t.internal_change_health(
                world,
                me,
                if crate::logic::modules::plant(1246) {
                    max_health
                } else {
                    max_health - prev_max_health
                },
            );
        }
        SAME_CURRENTHEALTH => {
            // do nothing
        }
        FULLY_HEAL => {
            // Set current to the new Max.
            // 400/500 (80%) + 100 becomes 600/600 (100%)
            // 200/500 (40%) - 100 becomes 400/400 (100%)
            let delta = if crate::logic::modules::plant(1247) {
                0.0
            } else {
                ab.max_health - ab.current_health
            };
            t.internal_change_health(world, me, delta);
        }
        _ => {}
    }

    // when max health is getting clipped to a lower value, if our current health value is now outside of the max health range we will set it back down to
    // the new cap. Note that we are *NOT* going through any healing or damage methods here and are doing a direct set
    if t.ab().current_health > max_health && !crate::logic::modules::plant(1248) {
        let delta = max_health - t.ab().current_health;
        t.internal_change_health(world, me, delta);
    }
}

/// `setIndestructible(indestructible)`: "set or unset the overridable indestructible flag in the body".
fn set_indestructible<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    indestructible: bool,
) {
    t.ab_mut().indestructible = indestructible;

    // for bridges, we mirror this state on its towers
    let us = me.object;
    if world
        .objects
        .find(us)
        .is_some_and(|o| o.is_kind_of(KINDOF_BRIDGE))
    {
        // BridgeBehavior::getBridgeBehaviorInterfaceFromObject: the first module that is a bridge behaviour
        let mut bbi = None;
        for idx in world.all_modules(us) {
            if out(t, world, me, |w| {
                w.module_call(us, idx, ModuleCall::HasBridgeBehaviorInterface)
            })
            .as_bool()
            {
                bbi = Some(idx);
                break;
            }
        }
        if let Some(bbi) = bbi {
            // get tower
            for i in 0..BRIDGE_MAX_TOWERS {
                let tower_id = out(t, world, me, |w| {
                    w.module_call(us, bbi, ModuleCall::GetBridgeTowerId { tower: i })
                })
                .as_id();
                if world.objects.find(tower_id).is_some() {
                    if let Some(body) = world.get_body_module(tower_id) {
                        out(t, world, me, |w| {
                            w.module_call(
                                tower_id,
                                body,
                                ModuleCall::SetIndestructible(indestructible),
                            )
                        });
                    }
                }
            }
        }
    }
}

/// `onVeterancyLevelChanged(oldLevel, newLevel, provideFeedback)`.
fn on_veterancy_level_changed<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    old_level: i32,
    new_level: i32,
    provide_feedback: bool,
) {
    if old_level == new_level {
        return;
    }
    let obj = me.object;

    if old_level < new_level {
        if provide_feedback && !crate::logic::modules::plant(1253) {
            let slot = match new_level {
                1 => Some(ThingTemplateAudioType::SoundPromotedVeteran),
                2 => Some(ThingTemplateAudioType::SoundPromotedElite),
                3 => {
                    if crate::logic::modules::plant(1250) {
                        Some(ThingTemplateAudioType::SoundPromotedElite)
                    } else {
                        Some(ThingTemplateAudioType::SoundPromotedHero)
                    }
                }
                _ => None,
            };
            let name = match (slot, world.objects.find(obj)) {
                (Some(slot), Some(o)) => o.template().get_audio(slot).event_name.clone(),
                _ => Vec::new(), // `AudioEventRTS veterancyChanged;` stays an event with no name
            };
            world.emit(ObjectClientEvent::AudioAddEvent {
                name,
                object_id: obj,
                position: None,
                player_index: None,
            });
        }

        // Also mark the UI dirty -- incase the object is selected or contained.
        if let Some(selected) = world.reads.first_selected() {
            if selected == obj || crate::logic::modules::plant(1251) {
                // Our selected object has been promoted!
                world.emit(ObjectClientEvent::ControlBarMarkUiDirty);
            } else {
                let contained_by = world
                    .objects
                    .find(obj)
                    .map_or(INVALID_ID, |o| o.contained_by);
                if world.objects.find(contained_by).is_some()
                    && world.reads.local_selection.len() == 1
                    && (selected == contained_by || crate::logic::modules::plant(1252))
                {
                    // But only if the contained by object is containing me!
                    world.emit(ObjectClientEvent::ControlBarMarkUiDirty);
                }
            }
        }
    }

    let bonus = &world.cuts.body.globals.health_bonus;
    let (Some(&old_bonus), Some(&new_bonus)) = (
        usize::try_from(old_level).ok().and_then(|i| bonus.get(i)),
        usize::try_from(new_level).ok().and_then(|i| bonus.get(i)),
    ) else {
        undefined(world, me, Undefined::ArrayIndexOutOfRange);
        return;
    };
    let mult = if crate::logic::modules::plant(1249) {
        old_bonus / new_bonus
    } else {
        new_bonus / old_bonus
    };

    // change the max
    let max = t.ab().max_health * mult;
    set_max_health(t, world, me, max, PRESERVE_RATIO);

    let ab = t.ab_mut();
    let set = |ab: &mut ActiveBody, ast: ArmorSetType, v: i32| ab.cur_armor_set_flags.set(ast, v);
    match new_level {
        0 => {
            set(ab, ARMORSET_VETERAN, 0);
            set(ab, ARMORSET_ELITE, 0);
            set(ab, ARMORSET_HERO, 0);
        }
        1 => {
            set(ab, ARMORSET_VETERAN, 1);
            set(ab, ARMORSET_ELITE, 0);
            set(ab, ARMORSET_HERO, 0);
        }
        2 => {
            set(ab, ARMORSET_VETERAN, 0);
            set(ab, ARMORSET_ELITE, 1);
            set(ab, ARMORSET_HERO, 0);
        }
        3 => {
            set(ab, ARMORSET_VETERAN, 0);
            set(ab, ARMORSET_ELITE, 0);
            set(ab, ARMORSET_HERO, 1);
        }
        _ => {}
    }
}

/// The answers of the body to the calls `Object` and the other modules make, for any class derived from `ActiveBody`.
pub fn body_module_call<T: ActiveBodyVirtuals>(
    t: &mut T,
    world: &mut World,
    me: ModuleHandle,
    call: ModuleCall,
) -> ModuleReply {
    match call {
        ModuleCall::AttemptDamage(mut info) => {
            t.attempt_damage(world, me, &mut info);
            ModuleReply::Damage(info)
        }
        ModuleCall::AttemptHealing(mut info) => {
            attempt_healing(t, world, me, &mut info);
            ModuleReply::Damage(info)
        }
        ModuleCall::EstimateDamage(input) => {
            ModuleReply::Real(estimate_damage(t, world, me, &input))
        }
        ModuleCall::GetMaxHealth => ModuleReply::Real(t.ab().max_health),
        ModuleCall::GetHealth => ModuleReply::Real(t.ab().current_health),
        ModuleCall::GetInitialHealth => ModuleReply::Real(t.ab().initial_health),
        ModuleCall::GetPreviousHealth => ModuleReply::Real(t.ab().prev_health),
        ModuleCall::GetDamageState => ModuleReply::Int(t.ab().cur_damage_state),
        ModuleCall::GetDamageScalar => ModuleReply::Real(t.ab().body.get_damage_scalar()),
        ModuleCall::ApplyDamageScalar(s) => {
            t.ab_mut().body.apply_damage_scalar(s);
            ModuleReply::Done
        }
        ModuleCall::GetLastDamageSource => ModuleReply::Id(t.ab().last_damage_info.input.source_id),
        ModuleCall::GetLastDamageInfo => ModuleReply::Damage(t.ab().last_damage_info.clone()),
        ModuleCall::GetLastDamageTimestamp => ModuleReply::Int(t.ab().last_damage_timestamp as i32),
        ModuleCall::GetLastHealingTimestamp => {
            ModuleReply::Int(t.ab().last_healing_timestamp as i32)
        }
        ModuleCall::GetClearableLastAttacker => {
            ModuleReply::Id(t.ab().get_clearable_last_attacker())
        }
        ModuleCall::ClearLastAttacker => {
            t.ab_mut().last_damage_cleared = true;
            ModuleReply::Done
        }
        ModuleCall::SetArmorSetFlag(ast) => {
            t.ab_mut().cur_armor_set_flags.set(ast, 1);
            ModuleReply::Done
        }
        ModuleCall::ClearArmorSetFlag(ast) => {
            t.ab_mut().cur_armor_set_flags.set(ast, 0);
            ModuleReply::Done
        }
        ModuleCall::TestArmorSetFlag(ast) => {
            ModuleReply::Bool(t.ab().cur_armor_set_flags.test(ast))
        }
        ModuleCall::OnVeterancyLevelChanged {
            old_level,
            new_level,
            provide_feedback,
        } => {
            on_veterancy_level_changed(t, world, me, old_level, new_level, provide_feedback);
            ModuleReply::Done
        }
        ModuleCall::SetMaxHealth(m) => {
            set_max_health(t, world, me, m, SAME_CURRENTHEALTH);
            ModuleReply::Done
        }
        ModuleCall::SetMaxHealthWithChange {
            max_health,
            change_type,
        } => {
            set_max_health(t, world, me, max_health, change_type);
            ModuleReply::Done
        }
        ModuleCall::SetInitialHealth(p) => {
            set_initial_health(t, world, me, p);
            ModuleReply::Done
        }
        ModuleCall::SetIndestructible(b) => {
            set_indestructible(t, world, me, b);
            ModuleReply::Done
        }
        ModuleCall::IsIndestructible => ModuleReply::Bool(t.ab().indestructible),
        ModuleCall::SetDamageState(s) => {
            set_damage_state(t, world, me, s);
            ModuleReply::Done
        }
        ModuleCall::InternalChangeHealth(d) => {
            t.internal_change_health(world, me, d);
            ModuleReply::Done
        }
        ModuleCall::EvaluateVisualCondition => {
            evaluate_visual_condition(t, world, me);
            ModuleReply::Done
        }
        ModuleCall::UpdateBodyParticleSystems => {
            update_body_particle_systems(t, world, me);
            ModuleReply::Done
        }
        ModuleCall::SetAflame(_) => {
            // All this does now is act like a major body state change. It is called after Aflame has been set or cleared as an Object Status
            update_body_particle_systems(t, world, me);
            ModuleReply::Done
        }
        ModuleCall::IsSubdued => ModuleReply::Bool(t.ab().is_subdued()),
        ModuleCall::CanBeSubdued => ModuleReply::Bool(t.ab().can_be_subdued()),
        ModuleCall::OnSubdualChange(b) => {
            on_subdual_change(t, world, me, b);
            ModuleReply::Done
        }
        ModuleCall::GetSubdualDamageHealRate => {
            ModuleReply::Int(t.ab().data.subdual_damage_heal_rate as i32)
        }
        ModuleCall::GetSubdualDamageHealAmount => {
            ModuleReply::Real(t.ab().data.subdual_damage_heal_amount)
        }
        ModuleCall::HasAnySubdualDamage => ModuleReply::Bool(t.ab().current_subdual_damage > 0.0),
        ModuleCall::GetCurrentSubdualDamageAmount => {
            ModuleReply::Real(t.ab().current_subdual_damage)
        }
        ModuleCall::GetFrontCrushed => ModuleReply::Bool(t.ab().front_crushed),
        ModuleCall::GetBackCrushed => ModuleReply::Bool(t.ab().back_crushed),
        ModuleCall::SetFrontCrushed(b) => {
            t.ab_mut().front_crushed = b;
            ModuleReply::Done
        }
        ModuleCall::SetBackCrushed(b) => {
            t.ab_mut().back_crushed = b;
            ModuleReply::Done
        }
        ModuleCall::OnDelete => {
            // delete all particle systems
            delete_all_particle_systems(t.ab_mut(), world);
            ModuleReply::Done
        }
        _ => ModuleReply::Unhandled,
    }
}

impl Snapshot for ActiveBody {
    /// `crc`: `BodyModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.body.crc(xfer)
    }

    /// `xfer` (version 1, "Initial version"): the version, `BodyModule::xfer`, the health fields, the damage state, the damage FX throttle, the last damage,
    /// the timestamps, the four flags, the particle system ids (a count of `UnsignedShort`, then the ids) and the armor set flags.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;

        // base class
        self.body.xfer(xfer)?;

        // current health
        xfer.xfer_real(&mut self.current_health)?;

        xfer.xfer_real(&mut self.current_subdual_damage)?;

        // previous health
        xfer.xfer_real(&mut self.prev_health)?;

        // max health
        xfer.xfer_real(&mut self.max_health)?;

        // initial health
        xfer.xfer_real(&mut self.initial_health)?;

        // current damage state
        xfer_enum(xfer, &mut self.cur_damage_state)?;

        // next damage fx time
        xfer.xfer_unsigned_int(&mut self.next_damage_fx_time)?;

        // last damage fx done
        xfer_enum(xfer, &mut self.last_damage_fx_done)?;

        // last damage info
        xfer.xfer_snapshot(&mut self.last_damage_info)?;

        // last damage timestamp
        xfer.xfer_unsigned_int(&mut self.last_damage_timestamp)?;

        // last damage timestamp
        xfer.xfer_unsigned_int(&mut self.last_healing_timestamp)?;

        // front crushed
        xfer.xfer_bool(&mut self.front_crushed)?;

        // back crushed
        xfer.xfer_bool(&mut self.back_crushed)?;

        // last damaged cleared
        xfer.xfer_bool(&mut self.last_damage_cleared)?;

        // indestructible
        xfer.xfer_bool(&mut self.indestructible)?;

        // particle system count
        let mut particle_system_count = self.particle_systems.len() as u16;
        xfer.xfer_unsigned_short(&mut particle_system_count)?;

        // particle systems
        if xfer.get_xfer_mode() == XferMode::Save {
            // walk the particle systems
            for system in &self.particle_systems {
                // write particle system ID
                let mut b = system.to_le_bytes();
                xfer.xfer_user(&mut b)?;
            }
        } else {
            // the list should be empty at this time
            if !self.particle_systems.is_empty() {
                // DEBUG_CRASH(("ActiveBody::xfer - m_particleSystems should be empty, but is not\n")); throw SC_INVALID_DATA;
                return Err(XferStatus::ListNotEmpty);
            }

            // read all data elements
            for _ in 0..particle_system_count {
                // read particle system ID
                let mut b = [0u8; 4];
                xfer.xfer_user(&mut b)?;
                // allocate entry and add to list; the list will be reversed, but we don't care
                self.particle_systems.insert(0, u32::from_le_bytes(b));
            }
        }

        // armor set flags
        self.cur_armor_set_flags.xfer(xfer)
    }

    /// `loadPostProcess`: `BodyModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.body.load_post_process();
    }
}

/// `xferUser(&e, sizeof(e))` of a four-byte enum.
fn xfer_enum(xfer: &mut dyn Xfer, v: &mut i32) -> XferResult<()> {
    let mut b = v.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    *v = i32::from_le_bytes(b);
    Ok(())
}

/// The factory's `NewModuleDataProc` for `ActiveBody`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ActiveBody(Arc::new(
        ActiveBodyModuleData::friend_new_module_data(ini)?,
    )))
}
