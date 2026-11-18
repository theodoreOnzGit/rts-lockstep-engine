//! UNTESTED DRAFT (#202, #208): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DemoralizeSpecialPower.h
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/DemoralizeSpecialPower.cpp
//!
//! **Not compiled into the shipped game** (`#ifdef ALLOW_DEMORALIZE`). The special power whose range and duration grow with the number of
//! captured units inside the caster, and which demoralizes every enemy or neutral infantry unit with AI in range.
//! `SpecialPowerModule` (the base) and the partition scan are the world's: [`DemoralizeWorld`].

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};

/// `DemoralizeSpecialPowerModuleData` (after `SpecialPowerModuleData`).
#[derive(Clone, Debug, PartialEq)]
pub struct DemoralizeSpecialPowerModuleData {
    /// `m_baseRange`: "base range for this special power".
    pub base_range: f32,
    /// `m_bonusRangePerCaptured`: "additional range we get for each prisoner".
    pub bonus_range_per_captured: f32,
    /// `m_maxRange`: "no matter how many prisoners we have, this is max".
    pub max_range: f32,
    /// `m_baseDurationInFrames`.
    pub base_duration_in_frames: u32,
    /// `m_bonusDurationPerCapturedInFrames`.
    pub bonus_duration_per_captured_in_frames: u32,
    /// `m_maxDurationInFrames`.
    pub max_duration_in_frames: u32,
    /// `m_fxList` by name (`None` for NULL).
    pub fx_list: Option<Vec<u8>>,
}

impl Default for DemoralizeSpecialPowerModuleData {
    /// `DemoralizeSpecialPowerModuleData()`: everything 0 / NULL.
    fn default() -> Self {
        DemoralizeSpecialPowerModuleData {
            base_range: 0.0,
            bonus_range_per_captured: 0.0,
            max_range: 0.0,
            base_duration_in_frames: 0,
            bonus_duration_per_captured_in_frames: 0,
            max_duration_in_frames: 0,
            fx_list: None,
        }
    }
}

/// `DemoralizeSpecialPowerModuleData::buildFieldParse`'s keys, after `SpecialPowerModuleData::buildFieldParse`: `BaseRange`,
/// `BonusRangePerCaptured`, `MaxRange` (`parseReal`); `BaseDuration`, `BonusDurationPerCaptured`, `MaxDuration`
/// (`parseDurationUnsignedInt`); `FXList` (`parseFXList`).
pub fn demoralize_field_names() -> [&'static str; 7] {
    ["BaseRange", "BonusRangePerCaptured", "MaxRange", "BaseDuration", "BonusDurationPerCaptured", "MaxDuration", "FXList"]
}

/// The world calls the power makes.
pub trait DemoralizeWorld {
    /// `getObject()->isDisabled()`.
    fn is_disabled(&self, obj: ObjectId) -> bool;
    /// `SpecialPowerModule::doSpecialPowerAtLocation(loc, angle, commandOptions)` (the base).
    fn special_power_base_do_at_location(&mut self, obj: ObjectId, loc: &Coord3D, angle: f32, command_options: u32);
    /// `source->getContain()->getContainCount()`, `None` with no contain module.
    fn contain_count(&self, obj: ObjectId) -> Option<u32>;
    /// `ThePartitionManager->iterateObjectsInRange(loc, range, FROM_CENTER_2D, filters)` with `PartitionFilterRelationship(source,
    /// ALLOW_ENEMIES | ALLOW_NEUTRAL)`, `PartitionFilterAcceptByKindOf(KINDOF_INFANTRY, none)` and `PartitionFilterSameMapStatus(source)`.
    fn iterate_enemy_neutral_infantry_in_range(&mut self, source: ObjectId, loc: &Coord3D, range: f32) -> Vec<ObjectId>;
    /// `ai->setDemoralized(duration)` when the object has AI; false if it has none.
    fn set_demoralized(&mut self, obj: ObjectId, duration: u32) -> bool;
    /// `FXList::doFXPos(fxList, loc)`.
    fn do_fx_pos(&mut self, fx_list: &[u8], loc: &Coord3D);
    /// `obj->getPosition()`.
    fn position(&self, obj: ObjectId) -> Coord3D;
}

/// `class DemoralizeSpecialPower : public SpecialPowerModule`.
#[derive(Clone, Debug, PartialEq)]
pub struct DemoralizeSpecialPower {
    /// The module data.
    pub data: DemoralizeSpecialPowerModuleData,
}

impl DemoralizeSpecialPower {
    /// `DemoralizeSpecialPower::DemoralizeSpecialPower(thing, moduleData)`.
    pub fn new(data: DemoralizeSpecialPowerModuleData) -> DemoralizeSpecialPower {
        DemoralizeSpecialPower { data }
    }

    /// `DemoralizeSpecialPower::doSpecialPowerAtLocation(loc, angle, commandOptions)`: nothing when disabled or `loc` is NULL; the base
    /// first (extending); duration = base + `containCount * bonus` capped at the max, range likewise (both only with a contain module);
    /// every enemy/neutral infantry unit with AI in range (centre 2D) is demoralized for `duration`; then the FX plays at `loc`.
    /// The duration product is `UnsignedInt * UnsignedInt` (wrapping), the range product `UnsignedInt * Real` (the count converts to float).
    pub fn do_special_power_at_location<W: DemoralizeWorld>(&mut self, w: &mut W, source: ObjectId, loc: Option<&Coord3D>, angle: f32, command_options: u32) {
        if w.is_disabled(source) {
            return;
        }
        // sanity
        let loc = match loc {
            Some(l) => l,
            None => return,
        };
        // call the base class action cause we are *EXTENDING* functionality
        w.special_power_base_do_at_location(source, loc, angle, command_options);
        // the demoralize special power increases in range and duration of effect the more units we have captured within us
        let mut duration: u32 = self.data.base_duration_in_frames;
        let mut range: f32 = self.data.base_range;
        if let Some(count) = w.contain_count(source) {
            // for every captured unit we get a bonus
            duration = duration.wrapping_add(count.wrapping_mul(self.data.bonus_duration_per_captured_in_frames));
            // cap at the max
            if duration > self.data.max_duration_in_frames {
                duration = self.data.max_duration_in_frames;
            }
            // increase the range for every prisoner we got
            range += (count as f32) * self.data.bonus_range_per_captured;
            // cap at the max
            if range > self.data.max_range {
                range = self.data.max_range;
            }
        }
        // scan the objects in the area range and demoralize them
        for obj in w.iterate_enemy_neutral_infantry_in_range(source, loc, range) {
            // demoralize only affects with ai of course
            w.set_demoralized(obj, duration);
        }
        // play an effect at the destination location
        if let Some(fx) = self.data.fx_list.clone() {
            // execute FX
            w.do_fx_pos(&fx, loc);
        }
    }

    /// `DemoralizeSpecialPower::doSpecialPowerAtObject(obj, angle, commandOptions)`: nothing when disabled; with an object, the power at
    /// its position.
    pub fn do_special_power_at_object<W: DemoralizeWorld>(&mut self, w: &mut W, source: ObjectId, obj: Option<ObjectId>, angle: f32, command_options: u32) {
        if w.is_disabled(source) {
            return;
        }
        if let Some(o) = obj {
            let pos = w.position(o);
            self.do_special_power_at_location(w, source, Some(&pos), angle, command_options);
        }
    }
}

impl Snapshot for DemoralizeSpecialPower {
    /// `DemoralizeSpecialPower::crc`: the base's.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(()) // wire: SpecialPowerModule::crc
    }
    /// `DemoralizeSpecialPower::xfer`: version 1, then the base.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        Ok(()) // wire: SpecialPowerModule::xfer
    }
    /// `DemoralizeSpecialPower::loadPostProcess`: the base's.
    fn load_post_process(&mut self) {}
}
