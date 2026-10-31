// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp (Object::updateObjValuesFromMapProperties)
//   GeneralsMD/Code/GameEngine/Include/Common/AudioEventInfo.h (AudioEventInfo::isPermanentSound)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/DynamicAudioEventInfo.cpp (the override* functions)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Object::updateObjValuesFromMapProperties` (layer 8, batch O1): what a map object's properties (its `Dict`) change on the
//! object made from it: the name, health, veterancy, the AI's mood, recruitability and stopping distance, selectability,
//! the script status bits, vision and shroud-clearing ranges, the upgrades it is granted, and, when it has a drawable,
//! the time of day, the weather and the ambient sound.
//!
//! The logic half is [`update_obj_values_from_map_properties`] on a [`MapPropsHost`] ([`World`] is one). The drawable half is the client's: the world sends
//! [`ObjectClientEvent::DrawableTimeOrWeather`] and [`ObjectClientEvent::DrawableAmbientSoundProperties`], and the client
//! runs [`apply_ambient_sound_properties`], the translation of upstream's ambient-sound block, against its own audio data
//! ([`AmbientSoundClient`]).
//!
//! # Verification
//!
//! `tests/differential_object_map_props.rs` compiles the function cut verbatim from `Object.cpp` on logging stand-ins
//! (`differential/wrap_object_map_props.cpp`) and compares random property dictionaries; its header has the results.

use crate::common::dict::Dict;
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::upgrade::UpgradeMaskType;
use crate::common::game_common::{VeterancyLevel, LEVEL_COUNT};
use crate::common::well_known_keys::{self as wk, WellKnownKeys};
use crate::logic::object_script_status_bits::{
    OBJECT_STATUS_SCRIPT_DISABLED, OBJECT_STATUS_SCRIPT_TARGETABLE, OBJECT_STATUS_SCRIPT_UNPOWERED, OBJECT_STATUS_SCRIPT_UNSELLABLE,
};
use crate::logic::modules::plant;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::ObjectId;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_events::{AmbientSoundProperties, ObjectClientEvent};

/// What upstream leaves undefined in `updateObjValuesFromMapProperties`; the call stops there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapPropsUndefined {
    /// `objectVeterancy` outside `0..LEVEL_COUNT` on a trainable object: `setVeterancyLevel((VeterancyLevel)valInt)`
    /// indexes the template's experience table out of bounds.
    VeterancyOutOfRange(i32),
}

/// `INT_TO_REAL`.
fn int_to_real(i: i32) -> f32 {
    i as f32
}

/// What `updateObjValuesFromMapProperties` asks of the object and the game, for object `id` (its `this`).
pub trait MapPropsHost {
    /// `TheNameKeyGenerator` (the well-known keys and the `objectGrantUpgrade<n>` keys are looked up in it).
    fn mp_names(&mut self) -> &mut NameKeyGenerator;
    /// `setName(name)`.
    fn mp_set_name(&mut self, id: ObjectId, name: Vec<u8>);
    /// `getBodyModule() != NULL`.
    fn mp_has_body(&self, id: ObjectId) -> bool;
    /// `body->setMaxHealth(maxHealth)` (the default change type).
    fn mp_body_set_max_health(&mut self, id: ObjectId, max_health: f32);
    /// `body->setInitialHealth(percent)`.
    fn mp_body_set_initial_health(&mut self, id: ObjectId, percent: i32);
    /// `body->setIndestructible(on)`.
    fn mp_body_set_indestructible(&mut self, id: ObjectId, on: bool);
    /// `m_experienceTracker && m_experienceTracker->isTrainable()`.
    fn mp_is_trainable(&self, id: ObjectId) -> bool;
    /// `m_experienceTracker->setVeterancyLevel(level)` (feedback on, the default).
    fn mp_set_veterancy_level(&mut self, id: ObjectId, level: VeterancyLevel);
    /// `getAIUpdateInterface() != NULL`.
    fn mp_has_ai(&self, id: ObjectId) -> bool;
    /// `ai->setAttitude(attitude)`.
    fn mp_ai_set_attitude(&mut self, id: ObjectId, attitude: i32);
    /// `ai->setIsRecruitable(on)`.
    fn mp_ai_set_is_recruitable(&mut self, id: ObjectId, on: bool);
    /// `ai->getCurLocomotor() != NULL`.
    fn mp_ai_has_cur_locomotor(&mut self, id: ObjectId) -> bool;
    /// `ai->getCurLocomotor()->setCloseEnoughDist(dist)`.
    fn mp_ai_set_close_enough_dist(&mut self, id: ObjectId, dist: f32);
    /// `isSelectable()`.
    fn mp_is_selectable(&self, id: ObjectId) -> bool;
    /// `setSelectable(on)`.
    fn mp_set_selectable(&mut self, id: ObjectId, on: bool);
    /// `setScriptStatus(bit, on)`.
    fn mp_set_script_status(&mut self, id: ObjectId, bit: u8, on: bool);
    /// `m_visionRange = range` (the field, not `setVisionRange`).
    fn mp_set_vision_range_field(&mut self, id: ObjectId, range: f32);
    /// `m_shroudClearingRange = range` (the field: no relook, unlike `setShroudClearingRange`).
    fn mp_set_shroud_clearing_range_field(&mut self, id: ObjectId, range: f32);
    /// `TheUpgradeCenter->findUpgrade(name)`'s upgrade mask; `None` for `NULL`.
    fn mp_find_upgrade(&mut self, name: &[u8]) -> Option<UpgradeMaskType>;
    /// `giveUpgrade(upgrade)` with the upgrade's mask.
    fn mp_give_upgrade(&mut self, id: ObjectId, mask: &UpgradeMaskType);
    /// `getDrawable() != NULL`.
    fn mp_has_drawable(&self, id: ObjectId) -> bool;
    /// The drawable's `MODELCONDITION_NIGHT` and `MODELCONDITION_SNOW` (`Some(true)` set, `Some(false)` cleared, in that
    /// order), when either property asks for a change.
    fn mp_drawable_time_or_weather(&mut self, id: ObjectId, night: Option<bool>, snow: Option<bool>);
    /// The ambient-sound properties, for the client's [`apply_ambient_sound_properties`] on the drawable.
    fn mp_drawable_ambient_sound(&mut self, id: ObjectId, properties: AmbientSoundProperties);
}

/// `Object::updateObjValuesFromMapProperties(properties)` on object `id`. See the module docs.
pub fn update_obj_values_from_map_properties<H: MapPropsHost>(h: &mut H, id: ObjectId, properties: &Dict) -> Result<(), MapPropsUndefined> {
    let keys = WellKnownKeys::new();
    let key = |h: &mut H, which: usize| keys.key(which, h.mp_names());

    let (val_str, exists) = properties.get_ascii_string(key(h, wk::OBJECT_NAME));
    if exists {
        h.mp_set_name(id, val_str);
    }

    let (val_int, exists) = properties.get_int(key(h, wk::OBJECT_MAX_HPS));
    if exists && (val_int >= 0 || plant(4042)) && h.mp_has_body(id) {
        h.mp_body_set_max_health(id, int_to_real(val_int));
    }

    let (val_int, exists) = properties.get_int(key(h, wk::OBJECT_INITIAL_HEALTH));
    if exists && h.mp_has_body(id) {
        h.mp_body_set_initial_health(id, val_int);
    }

    // set the veterancy level
    let (val_int, exists) = properties.get_int(key(h, wk::OBJECT_VETERANCY));
    if exists && (h.mp_is_trainable(id) || plant(4041)) {
        let level = match val_int {
            0 => VeterancyLevel::Regular,
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            3 => VeterancyLevel::Heroic,
            v => {
                debug_assert!(!(0..LEVEL_COUNT).contains(&v));
                return Err(MapPropsUndefined::VeterancyOutOfRange(v));
            }
        };
        h.mp_set_veterancy_level(id, level);
    }

    // set the aggressiveness/mood
    let (val_int, exists) = properties.get_int(key(h, wk::OBJECT_AGGRESSIVENESS));
    if exists && h.mp_has_ai(id) {
        h.mp_ai_set_attitude(id, val_int);
    }

    // set recruitable
    let (val_bool, exists) = properties.get_bool(key(h, wk::OBJECT_RECRUITABLE_AI));
    if exists && h.mp_has_ai(id) {
        h.mp_ai_set_is_recruitable(id, val_bool);
    }

    // set selectable
    let (val_bool, exists) = properties.get_bool(key(h, wk::OBJECT_SELECTABLE));
    if exists && (val_bool != h.mp_is_selectable(id) || plant(4043)) {
        h.mp_set_selectable(id, val_bool);
    }

    // set the stopping distance
    let (val_real, exists) = properties.get_real(key(h, wk::OBJECT_STOPPING_DISTANCE));
    if exists && val_real >= 0.5 && h.mp_has_ai(id) && h.mp_ai_has_cur_locomotor(id) {
        h.mp_ai_set_close_enough_dist(id, val_real);
    }

    // set the disabledness of this object
    let (val_bool, exists) = properties.get_bool(key(h, wk::OBJECT_ENABLED));
    if exists {
        h.mp_set_script_status(id, OBJECT_STATUS_SCRIPT_DISABLED, !val_bool);
    }

    // set the disabledness of this object
    let (val_bool, exists) = properties.get_bool(key(h, wk::OBJECT_POWERED));
    if exists {
        h.mp_set_script_status(id, OBJECT_STATUS_SCRIPT_UNPOWERED, !val_bool);
    }

    // set the invulnerability of the object
    let (val_bool, exists) = properties.get_bool(key(h, wk::OBJECT_INDESTRUCTIBLE));
    if exists && h.mp_has_body(id) {
        h.mp_body_set_indestructible(id, val_bool);
    }

    // set the sellability of the object
    let (val_bool, exists) = properties.get_bool(key(h, wk::OBJECT_UNSELLABLE));
    if exists {
        h.mp_set_script_status(id, OBJECT_STATUS_SCRIPT_UNSELLABLE, val_bool);
    }

    //Set the player targetable setting of the object
    let (val_bool, exists) = properties.get_bool(key(h, wk::OBJECT_TARGETABLE));
    if exists {
        h.mp_set_script_status(id, OBJECT_STATUS_SCRIPT_TARGETABLE, val_bool);
    }

    // adjust the vision distance of this object, overriding its default vision distance
    let (val_int, exists) = properties.get_int(key(h, wk::OBJECT_VISUAL_RANGE));
    if exists {
        h.mp_set_vision_range_field(id, int_to_real(val_int.max(0)));
    }

    // adjust the shroud clearing distance of this object, overriding its default distance
    let (val_int, exists) = properties.get_int(key(h, wk::OBJECT_SHROUD_CLEARING_DISTANCE));
    if exists {
        h.mp_set_shroud_clearing_range_field(id, int_to_real(if plant(4044) { val_int } else { val_int.max(0) }));
    }

    let grant = key(h, wk::OBJECT_GRANT_UPGRADE);
    let grant_name = h.mp_names().key_to_name(grant).to_vec();
    let mut upgrade_num = 0;
    loop {
        let mut key_name = grant_name.clone();
        key_name.extend_from_slice(upgrade_num.to_string().as_bytes());
        let k = h.mp_names().name_to_key(&key_name);
        let (val_str, exists) = properties.get_ascii_string(k);
        let val_str = if exists {
            match h.mp_find_upgrade(&val_str) {
                Some(mask) => h.mp_give_upgrade(id, &mask),
                None if plant(4045) => break,
                None => {}
            }
            val_str
        } else {
            Vec::new()
        };
        upgrade_num += 1;
        if val_str.is_empty() {
            break;
        }
    }

    if h.mp_has_drawable(id) {
        let (time, time_exists) = properties.get_int(key(h, wk::OBJECT_TIME));
        let (weather, weather_exists) = properties.get_int(key(h, wk::OBJECT_WEATHER));
        let pick = |exists: bool, v: i32| match (exists, v) {
            (true, 1) => Some(false), // clearModelConditionState
            (true, 2) => Some(true),  // setModelConditionState
            _ => None,
        };
        let (night, snow) = (pick(time_exists, time), pick(weather_exists, weather));
        if night.is_some() || snow.is_some() {
            h.mp_drawable_time_or_weather(id, night, snow);
        }
        let mut k = |which: usize| keys.key(which, h.mp_names());
        let opt = |(v, e): (bool, bool)| e.then_some(v);
        let opt_i = |(v, e): (i32, bool)| e.then_some(v);
        let opt_r = |(v, e): (f32, bool)| e.then_some(v);
        let (name, name_exists) = properties.get_ascii_string(k(wk::OBJECT_SOUND_AMBIENT));
        let ambient = AmbientSoundProperties {
            name: name_exists.then_some(name),
            customized: opt(properties.get_bool(k(wk::OBJECT_SOUND_AMBIENT_CUSTOMIZED))),
            enabled: opt(properties.get_bool(k(wk::OBJECT_SOUND_AMBIENT_ENABLED))),
            looping: opt(properties.get_bool(k(wk::OBJECT_SOUND_AMBIENT_LOOPING))),
            loop_count: opt_i(properties.get_int(k(wk::OBJECT_SOUND_AMBIENT_LOOP_COUNT))),
            min_volume: opt_r(properties.get_real(k(wk::OBJECT_SOUND_AMBIENT_MIN_VOLUME))),
            volume: opt_r(properties.get_real(k(wk::OBJECT_SOUND_AMBIENT_VOLUME))),
            min_range: opt_r(properties.get_real(k(wk::OBJECT_SOUND_AMBIENT_MIN_RANGE))),
            max_range: opt_r(properties.get_real(k(wk::OBJECT_SOUND_AMBIENT_MAX_RANGE))),
            priority: opt_i(properties.get_int(k(wk::OBJECT_SOUND_AMBIENT_PRIORITY))),
        };
        h.mp_drawable_ambient_sound(id, ambient);
    }
    Ok(())
}

impl World {
    /// `obj->updateObjValuesFromMapProperties(properties)`: [`update_obj_values_from_map_properties`] on this world.
    pub fn update_obj_values_from_map_properties(&mut self, id: ObjectId, properties: &Dict) -> Result<(), MapPropsUndefined> {
        update_obj_values_from_map_properties(self, id, properties)
    }
}

impl MapPropsHost for World {
    fn mp_names(&mut self) -> &mut NameKeyGenerator {
        &mut self.names
    }
    fn mp_set_name(&mut self, id: ObjectId, name: Vec<u8>) {
        if let Some(o) = self.objects.find_mut(id) {
            o.name = name;
        }
    }
    fn mp_has_body(&self, id: ObjectId) -> bool {
        self.get_body_module(id).is_some()
    }
    fn mp_body_set_max_health(&mut self, id: ObjectId, max_health: f32) {
        if let Some(body) = self.get_body_module(id) {
            self.module_call(id, body, ModuleCall::SetMaxHealth(max_health));
        }
    }
    fn mp_body_set_initial_health(&mut self, id: ObjectId, percent: i32) {
        if let Some(body) = self.get_body_module(id) {
            self.module_call(id, body, ModuleCall::SetInitialHealth(percent));
        }
    }
    fn mp_body_set_indestructible(&mut self, id: ObjectId, on: bool) {
        if let Some(body) = self.get_body_module(id) {
            self.module_call(id, body, ModuleCall::SetIndestructible(on));
        }
    }
    fn mp_is_trainable(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.template().is_trainable())
    }
    fn mp_set_veterancy_level(&mut self, id: ObjectId, level: VeterancyLevel) {
        self.cut_experience_set_veterancy_level(id, level, true);
    }
    fn mp_has_ai(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.ai.is_some())
    }
    fn mp_ai_set_attitude(&mut self, id: ObjectId, attitude: i32) {
        if let Some(ai) = self.objects.find(id).and_then(|o| o.ai) {
            self.module_call(id, ai, ModuleCall::AiSetAttitude(attitude));
        }
    }
    fn mp_ai_set_is_recruitable(&mut self, id: ObjectId, on: bool) {
        if let Some(ai) = self.objects.find(id).and_then(|o| o.ai) {
            self.module_call(id, ai, ModuleCall::AiSetIsRecruitable(on));
        }
    }
    fn mp_ai_has_cur_locomotor(&mut self, id: ObjectId) -> bool {
        match self.objects.find(id).and_then(|o| o.ai) {
            Some(ai) => self.module_call(id, ai, ModuleCall::AiHasCurLocomotor).as_bool(),
            None => false,
        }
    }
    fn mp_ai_set_close_enough_dist(&mut self, id: ObjectId, dist: f32) {
        if let Some(ai) = self.objects.find(id).and_then(|o| o.ai) {
            self.module_call(id, ai, ModuleCall::AiLocomotorSetCloseEnoughDist(dist));
        }
    }
    fn mp_is_selectable(&self, id: ObjectId) -> bool {
        self.is_selectable(id)
    }
    fn mp_set_selectable(&mut self, id: ObjectId, on: bool) {
        self.set_selectable(id, on);
    }
    fn mp_set_script_status(&mut self, id: ObjectId, bit: u8, on: bool) {
        self.set_script_status(id, bit, on);
    }
    fn mp_set_vision_range_field(&mut self, id: ObjectId, range: f32) {
        if let Some(o) = self.objects.find_mut(id) {
            o.vision_range = range;
        }
    }
    fn mp_set_shroud_clearing_range_field(&mut self, id: ObjectId, range: f32) {
        if let Some(o) = self.objects.find_mut(id) {
            o.shroud_clearing_range = range;
        }
    }
    fn mp_find_upgrade(&mut self, name: &[u8]) -> Option<UpgradeMaskType> {
        let statics = self.statics.clone();
        statics.upgrade_center.find_upgrade(&mut self.names, name).ok().flatten().map(|ut| ut.upgrade_mask())
    }
    fn mp_give_upgrade(&mut self, id: ObjectId, mask: &UpgradeMaskType) {
        self.give_upgrade(id, Some(mask));
    }
    fn mp_has_drawable(&self, id: ObjectId) -> bool {
        self.has_drawable(id)
    }
    fn mp_drawable_time_or_weather(&mut self, id: ObjectId, night: Option<bool>, snow: Option<bool>) {
        self.emit(ObjectClientEvent::DrawableTimeOrWeather { object_id: id, night, snow });
    }
    fn mp_drawable_ambient_sound(&mut self, id: ObjectId, properties: AmbientSoundProperties) {
        self.emit(ObjectClientEvent::DrawableAmbientSoundProperties { object_id: id, properties });
    }
}

/// `AC_LOOP` (`AudioEventInfo.h`).
pub const AC_LOOP: u32 = 0x0001;

/// The `DynamicAudioEventInfo::OVERRIDE_*` bits, in upstream's order.
pub mod override_bits {
    /// `OVERRIDE_NAME`.
    pub const NAME: u32 = 1 << 0;
    /// `OVERRIDE_LOOP_FLAG`.
    pub const LOOP_FLAG: u32 = 1 << 1;
    /// `OVERRIDE_LOOP_COUNT`.
    pub const LOOP_COUNT: u32 = 1 << 2;
    /// `OVERRIDE_VOLUME`.
    pub const VOLUME: u32 = 1 << 3;
    /// `OVERRIDE_MIN_VOLUME`.
    pub const MIN_VOLUME: u32 = 1 << 4;
    /// `OVERRIDE_MIN_RANGE`.
    pub const MIN_RANGE: u32 = 1 << 5;
    /// `OVERRIDE_MAX_RANGE`.
    pub const MAX_RANGE: u32 = 1 << 6;
    /// `OVERRIDE_PRIORITY`.
    pub const PRIORITY: u32 = 1 << 7;
}

/// The fields of an `AudioEventInfo` the ambient-sound customisation reads or overrides, and, once copied into a
/// `DynamicAudioEventInfo`, which of them were overridden.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AmbientAudioInfo {
    /// `m_audioName`.
    pub name: Vec<u8>,
    /// `m_control`.
    pub control: u32,
    /// `m_loopCount`.
    pub loop_count: i32,
    /// `m_volume`.
    pub volume: f32,
    /// `m_minVolume`.
    pub min_volume: f32,
    /// `m_minDistance`.
    pub min_distance: f32,
    /// `m_maxDistance`.
    pub max_distance: f32,
    /// `m_priority`.
    pub priority: i32,
    /// `m_overriddenFields` ([`override_bits`]).
    pub overridden: u32,
}

impl AmbientAudioInfo {
    /// `isPermanentSound()`: looping, for ever.
    pub fn is_permanent_sound(&self) -> bool {
        self.control & AC_LOOP != 0 && self.loop_count == 0
    }
}

/// The drawable and the audio system the ambient-sound customisation talks to (the client's).
pub trait AmbientSoundClient {
    /// `drawable->setCustomSoundAmbientOff()`.
    fn set_custom_sound_ambient_off(&mut self);
    /// `TheAudio->findAudioEventInfo(name)`; `None` for `NULL`.
    fn find_audio_event_info(&mut self, name: &[u8]) -> Option<AmbientAudioInfo>;
    /// `drawable->getBaseSoundAmbientInfo()`; `None` for `NULL`.
    fn base_sound_ambient_info(&mut self) -> Option<AmbientAudioInfo>;
    /// `drawable->enableAmbientSoundFromScript(on)`.
    fn enable_ambient_sound_from_script(&mut self, on: bool);
    /// `drawable->getAmbientSoundEnabledFromScript()`.
    fn ambient_sound_enabled_from_script(&mut self) -> bool;
    /// `drawable->mangleCustomAudioName(info)`, `TheAudio->addAudioEventInfo(info)` and
    /// `drawable->setCustomSoundAmbientInfo(info)`: the customised sound becomes the drawable's.
    fn install_custom_sound_ambient(&mut self, info: AmbientAudioInfo);
}

/// The ambient-sound block of `updateObjValuesFromMapProperties`, run by the client on the drawable of the object the
/// properties came from (see the module docs).
pub fn apply_ambient_sound_properties<C: AmbientSoundClient>(p: &AmbientSoundProperties, client: &mut C) {
    // See if we are supposed to playing the ambient sound
    let mut sound_enabled_exists = p.enabled.is_some();
    let mut sound_enabled = p.enabled.unwrap_or(false);

    let mut audio_to_modify: Option<AmbientAudioInfo> = None;
    let mut info_modified = false;
    let forced_off = p.name.as_ref().is_some_and(|n| n.is_empty());
    if let Some(name) = &p.name {
        if name.is_empty() {
            client.set_custom_sound_ambient_off();
            sound_enabled_exists = true;
            sound_enabled = false; // Don't bother trying to enable later
        } else if let Some(base_info) = client.find_audio_event_info(name) {
            audio_to_modify = Some(base_info);
            info_modified = true;
        }
    }

    // Don't do anything more to audio if we forced the ambient sound off
    if !forced_off && p.customized == Some(true) {
        if audio_to_modify.is_none() {
            audio_to_modify = client.base_sound_ambient_info();
        }
        if let Some(a) = &mut audio_to_modify {
            if let Some(looping) = p.looping {
                a.overridden |= override_bits::LOOP_FLAG;
                if looping {
                    a.control |= AC_LOOP;
                } else {
                    a.control &= !AC_LOOP;
                }
                info_modified = true;
            }
            if let Some(n) = p.loop_count {
                if a.control & AC_LOOP != 0 || plant(4046) {
                    a.overridden |= override_bits::LOOP_COUNT;
                    a.loop_count = n;
                    info_modified = true;
                }
            }
            if let Some(v) = p.min_volume {
                a.overridden |= override_bits::MIN_VOLUME;
                a.min_volume = v;
                info_modified = true;
            }
            if let Some(v) = p.volume {
                a.overridden |= override_bits::VOLUME;
                a.volume = v;
                info_modified = true;
            }
            if let Some(v) = p.min_range {
                a.overridden |= override_bits::MIN_RANGE;
                a.min_distance = v;
                info_modified = true;
            }
            if let Some(v) = p.max_range {
                a.overridden |= override_bits::MAX_RANGE;
                a.max_distance = v;
                info_modified = true;
            }
            if let Some(v) = p.priority {
                a.overridden |= override_bits::PRIORITY;
                a.priority = v;
                info_modified = true;
            }
        }
    }

    if !sound_enabled_exists {
        // Decide if the sound should start enabled or not, since the map maker didn't record
        // a preference. Enable permanently looping sounds, disable one-shot sounds by default
        if let Some(a) = &audio_to_modify {
            sound_enabled = a.is_permanent_sound();
            sound_enabled_exists = true; // To get into enableAmbientSoundFromScript() call.
        } else if let Some(base_info) = client.base_sound_ambient_info() {
            // Use default audio
            sound_enabled = base_info.is_permanent_sound();
            sound_enabled_exists = true; // To get into enableAmbientSoundFromScript() call.
        }
    }

    if sound_enabled_exists && !sound_enabled {
        // Make sure sound doesn't start playing when we set it
        client.enable_ambient_sound_from_script(false);
    }

    if info_modified {
        if let Some(a) = audio_to_modify.take() {
            client.install_custom_sound_ambient(a);
        }
    }

    if sound_enabled_exists && sound_enabled {
        // Play sound now that it is set up, if needed. Don't call if already enabled because that
        // can cause sound to play twice
        if !client.ambient_sound_enabled_from_script() {
            client.enable_ambient_sound_from_script(true);
        }
    }
}
