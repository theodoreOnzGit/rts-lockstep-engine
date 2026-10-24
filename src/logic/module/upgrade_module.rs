// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/UpgradeModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/UpgradeModule.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/Module.cpp (lines 255-334:
//   `UpgradeMuxData`'s member functions)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, September 2001 (UpgradeMux: Kris Morness)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `UpgradeModule`, `UpgradeMux` and `UpgradeMuxData`: how a module reacts
//! to an object getting an upgrade.
//!
//! # How an upgrade module decides
//!
//! Every upgrade owns one bit of the 128-bit `UpgradeMaskType` (see
//! `common::upgrade`). An object keeps the mask of the upgrades it has;
//! whenever that changes it asks each upgrade module whether it fires, with
//! that mask as the *key*. A module (its [`UpgradeMuxData`], read from INI)
//! names:
//!
//! - `TriggeredBy`: the upgrades that activate it. With `RequiresAllTriggers`
//!   all of them must be in the key, otherwise any one.
//! - `ConflictsWith`: upgrades that stop it; any one of them in the key
//!   blocks it.
//! - `RemovesUpgrades`: upgrades that are reset when it fires ("while the
//!   function is called `Object::removeUpgrade`, it is not removing anything
//!   in the sense of undoing the effects. It is just resetting the upgrade so
//!   it may be run again").
//! - `FXListUpgrade`: the effect played when it fires.
//!
//! and runs **once** (`m_upgradeExecuted`) until [`UpgradeMux::reset_upgrade`].
//! The names are turned into masks the **first time anyone asks**
//! ([`UpgradeMuxData::get_upgrade_activation_masks`]: "the first time someone
//! looks at my mask, I'll figure it out"), because the templates are read
//! from INI after the modules; the names are then cleared, and the trigger
//! names kept for [`UpgradeMuxData::is_triggered_by`]. That lookup assigns a
//! name key to every name it has not seen (`findUpgrade` calls `nameToKey`),
//! which is lockstep state, so the call takes the
//! [`NameKeyGenerator`] and happens at the same moment as upstream's.
//!
//! # Translation choices
//!
//! - **`mutable` members.** The mux data is shared (`const ModuleData*`) but
//!   its names, `m_fxListUpgrade` and the two masks are `mutable`: here
//!   [`UpgradeModuleData`] holds the [`UpgradeMuxData`] in an `RwLock`
//!   (shared mutable state).
//! - **The mix-in.** `UpgradeMux` is a base class with five pure virtuals;
//!   the Rust splits it into the state ([`UpgradeMux`]), the data
//!   ([`UpgradeMuxData`], passed in) and the effects the module performs
//!   ([`UpgradeEffects`]: `upgradeImplementation`, the effect list, and the
//!   `Object::removeUpgrade` call). `getUpgradeActivationMasks`,
//!   `requiresAllActivationUpgrades`, `performUpgradeFX` and
//!   `processUpgradeRemoval` of `UpgradeModule` all read `m_upgradeMuxData`,
//!   so they are [`UpgradeMuxData`]'s methods; a class that mixes `UpgradeMux`
//!   into something else (`AutoHealBehavior`) keeps an `UpgradeMuxData` in its
//!   data and passes it in the same way.
//! - **`FXList`.** A name ([`FxListName`]; `None` for `None`), as in
//!   `DamageFX`. `FXList::doFXObj(m_fxListUpgrade, obj)` is the client's:
//!   [`UpgradeEffects::do_fx_obj`].
//! - **`Object::removeUpgrade(template)`** is layer 8:
//!   [`UpgradeEffects::remove_upgrade`] gets the template.
//! - `UpgradeMuxData::requiresAllActivationUpgrades()` is declared but never
//!   defined upstream (nothing calls it; callers read `m_requiresAllTriggers`):
//!   not translated.
//!
//! # Where upstream is undefined
//!
//! An empty or `None` name in `TriggeredBy`, `ConflictsWith` or
//! `RemovesUpgrades` that finds no upgrade passes the `!theTemplate &&
//! !isEmpty() && !isNone()` check and then dereferences the null template:
//! [`Undefined::NullUpgradeTemplate`]. A name key past the last one:
//! [`Undefined::NameKeysExhausted`].
//!
//! # Verification
//!
//! `tests/differential_module.rs` compares the parsed data, the masks and
//! every decision of `UpgradeMux` on random sequences against the unchanged
//! `Module.cpp` and `UpgradeModule.cpp`, with the real `UpgradeCenter`.

use std::sync::{Arc, RwLock};

use crate::common::damage_fx::FxListName;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_UPGRADE};
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::upgrade::{UpgradeCenter, UpgradeMaskType, UpgradeTemplate};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};

/// What `UpgradeMux` asks of the module it is mixed into (its pure virtuals
/// and the object calls behind them). A compile-time contract, used as a
/// generic bound.
pub trait UpgradeEffects {
    /// `upgradeImplementation()`: "here's the actual work of upgrading".
    fn upgrade_implementation(&mut self);
    /// `FXList::doFXObj(m_fxListUpgrade, obj)`, for a non-null list.
    fn do_fx_obj(&mut self, fx: &[u8]);
    /// `obj->removeUpgrade(template)`.
    fn remove_upgrade(&mut self, template: &UpgradeTemplate);
}

/// `findUpgrade(name)` for one of the mux's names: the template, `INI_INVALID_DATA`
/// for a name that is neither empty, `None`, nor an upgrade, and
/// [`Undefined::NullUpgradeTemplate`] for an empty or `None` one that is not.
pub(crate) fn find_named_upgrade<'c>(
    center: &'c UpgradeCenter,
    names: &mut NameKeyGenerator,
    name: &[u8],
) -> IniResult<&'c UpgradeTemplate> {
    let found = center
        .find_upgrade(names, name)
        .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
    match found {
        Some(t) => Ok(t),
        None if !name.is_empty() && !name.eq_ignore_ascii_case(b"None") => {
            Err(IniError::new(IniErrorKind::InvalidData))
        }
        None => Err(IniError::undefined(Undefined::NullUpgradeTemplate)),
    }
}

/// `class UpgradeMuxData` ("does NOT inherit from ModuleData").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpgradeMuxData {
    /// `m_triggerUpgradeNames`: filled when the masks are first computed.
    pub trigger_upgrade_names: Vec<Vec<u8>>,
    /// `m_activationUpgradeNames` (`TriggeredBy`).
    pub activation_upgrade_names: Vec<Vec<u8>>,
    /// `m_conflictingUpgradeNames` (`ConflictsWith`).
    pub conflicting_upgrade_names: Vec<Vec<u8>>,
    /// `m_removalUpgradeNames` (`RemovesUpgrades`).
    pub removal_upgrade_names: Vec<Vec<u8>>,
    /// `m_fxListUpgrade` (`FXListUpgrade`), as a name.
    pub fx_list_upgrade: FxListName,
    /// `m_activationMask`: "activation only supports a single name currently".
    activation_mask: UpgradeMaskType,
    /// `m_conflictingMask`: "conflicts support multiple listings, and they are an OR".
    conflicting_mask: UpgradeMaskType,
    /// `m_requiresAllTriggers` (`RequiresAllTriggers`).
    pub requires_all_triggers: bool,
}

impl UpgradeMuxData {
    /// `UpgradeMuxData()`: no names, no list, empty masks, any trigger will do.
    pub fn new() -> Self {
        Self::default()
    }

    /// `m_activationMask`, as computed so far.
    pub fn activation_mask(&self) -> UpgradeMaskType {
        self.activation_mask
    }

    /// `m_conflictingMask`, as computed so far.
    pub fn conflicting_mask(&self) -> UpgradeMaskType {
        self.conflicting_mask
    }

    /// `getFieldParse()` as a table over a type that holds the mux data: the
    /// five fields. `T` reaches its mux data through `mux`.
    pub fn parse_triggered_by(
        ini: &mut Ini<'_>,
        d: &mut UpgradeMuxData,
        _: &UserData,
    ) -> IniResult<()> {
        ini.parse_ascii_string_vector(&mut d.activation_upgrade_names)
    }
    pub fn parse_conflicts_with(
        ini: &mut Ini<'_>,
        d: &mut UpgradeMuxData,
        _: &UserData,
    ) -> IniResult<()> {
        ini.parse_ascii_string_vector(&mut d.conflicting_upgrade_names)
    }
    pub fn parse_removes_upgrades(
        ini: &mut Ini<'_>,
        d: &mut UpgradeMuxData,
        _: &UserData,
    ) -> IniResult<()> {
        ini.parse_ascii_string_vector(&mut d.removal_upgrade_names)
    }
    pub fn parse_fx_list_upgrade(
        ini: &mut Ini<'_>,
        d: &mut UpgradeMuxData,
        _: &UserData,
    ) -> IniResult<()> {
        // INI::parseFXList, as far as the token goes (see common::damage_fx)
        let token = ini.get_next_token(None)?;
        d.fx_list_upgrade = if token.eq_ignore_ascii_case(b"None") {
            None
        } else {
            Some(token)
        };
        Ok(())
    }
    pub fn parse_requires_all_triggers(
        ini: &mut Ini<'_>,
        d: &mut UpgradeMuxData,
        _: &UserData,
    ) -> IniResult<()> {
        d.requires_all_triggers = ini.parse_bool()?;
        Ok(())
    }

    /// `getUpgradeActivationMasks(activation, conflicting)`: if any name is
    /// still to be resolved, resolve them all now (the masks are rebuilt from
    /// scratch, the activation names kept as the trigger names, the name
    /// lists cleared); either way return the two masks. `INI_INVALID_DATA`
    /// for a name that is not an upgrade (and not empty or `None`); the
    /// masks are then partly built and the names kept, as upstream leaves
    /// them.
    pub fn get_upgrade_activation_masks(
        &mut self,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
    ) -> IniResult<(UpgradeMaskType, UpgradeMaskType)> {
        // already computed.
        if !self.activation_upgrade_names.is_empty() || !self.conflicting_upgrade_names.is_empty() {
            self.activation_mask.clear();
            self.conflicting_mask.clear();

            for i in 0..self.activation_upgrade_names.len() {
                let t = find_named_upgrade(center, names, &self.activation_upgrade_names[i])?;
                self.activation_mask.set_mask(&t.upgrade_mask());
            }
            for i in 0..self.conflicting_upgrade_names.len() {
                let t = find_named_upgrade(center, names, &self.conflicting_upgrade_names[i])?;
                self.conflicting_mask.set_mask(&t.upgrade_mask());
            }

            // "We set the trigger upgrade names with the activationUpgradeNames entries to be
            // used later. We have to do this because the activationUpgradeNames are toasted
            // just below."
            self.trigger_upgrade_names = self.activation_upgrade_names.clone();

            // "Clear the names now that we've cached the values!"
            self.activation_upgrade_names.clear();
            self.conflicting_upgrade_names.clear();
        }
        Ok((self.activation_mask, self.conflicting_mask))
    }

    /// `muxDataProcessUpgradeRemoval(obj)`: reset each `RemovesUpgrades`
    /// upgrade on the object, in order, through `remove` (`obj->removeUpgrade`).
    /// `INI_INVALID_DATA` at the first name that is not an upgrade; the ones
    /// before it were already removed.
    pub fn mux_data_process_upgrade_removal(
        &self,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        mut remove: impl FnMut(&UpgradeTemplate),
    ) -> IniResult<()> {
        for name in &self.removal_upgrade_names {
            let t = find_named_upgrade(center, names, name)?;
            remove(t);
        }
        Ok(())
    }

    /// `isTriggeredBy(upgrade)`: whether `upgrade` is one of the trigger
    /// names (compared without regard to case). The trigger names are filled
    /// by the first [`UpgradeMuxData::get_upgrade_activation_masks`].
    pub fn is_triggered_by(&self, upgrade: &[u8]) -> bool {
        self.trigger_upgrade_names
            .iter()
            .any(|t| t.eq_ignore_ascii_case(upgrade))
    }
}

/// `UpgradeMuxData::getFieldParse()`: `TriggeredBy`, `ConflictsWith`,
/// `RemovesUpgrades`, `FXListUpgrade`, `RequiresAllTriggers`.
pub static UPGRADE_MUX_DATA_FIELD_PARSE: [FieldParse<UpgradeMuxData>; 6] = [
    FieldParse::new(
        "TriggeredBy",
        UpgradeMuxData::parse_triggered_by,
        UserData::None,
    ),
    FieldParse::new(
        "ConflictsWith",
        UpgradeMuxData::parse_conflicts_with,
        UserData::None,
    ),
    FieldParse::new(
        "RemovesUpgrades",
        UpgradeMuxData::parse_removes_upgrades,
        UserData::None,
    ),
    FieldParse::new(
        "FXListUpgrade",
        UpgradeMuxData::parse_fx_list_upgrade,
        UserData::None,
    ),
    FieldParse::new(
        "RequiresAllTriggers",
        UpgradeMuxData::parse_requires_all_triggers,
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class UpgradeMux`: the state of the mix-in, "upgrade only executes once".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UpgradeMux {
    /// `m_upgradeExecuted`.
    upgrade_executed: bool,
}

impl UpgradeMux {
    /// `UpgradeMux()`: not yet executed.
    pub fn new() -> Self {
        Self::default()
    }

    /// `setUpgradeExecuted(e)`.
    pub fn set_upgrade_executed(&mut self, e: bool) {
        self.upgrade_executed = e;
    }

    /// `isAlreadyUpgraded()`.
    pub fn is_already_upgraded(&self) -> bool {
        self.upgrade_executed
    }

    /// `forceRefreshUpgrade()`: "*** DANGER! DANGER! Don't use this, unless
    /// you are forcing an already made upgrade to refresh!!" Runs
    /// `upgradeImplementation` again, only if the upgrade was made.
    pub fn force_refresh_upgrade(&self, effects: &mut impl UpgradeEffects) {
        if self.upgrade_executed {
            effects.upgrade_implementation();
        }
    }

    /// `attemptUpgrade(keyMask)`: if [`UpgradeMux::would_upgrade`], give the
    /// upgrade ([`UpgradeMux::give_self_upgrade`]) and return true. An error
    /// (from the masks, or the removal) leaves the upgrade not executed.
    pub fn attempt_upgrade(
        &mut self,
        data: &mut UpgradeMuxData,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        effects: &mut impl UpgradeEffects,
        key_mask: UpgradeMaskType,
    ) -> IniResult<bool> {
        if self.would_upgrade(data, center, names, key_mask)? {
            // "If I have an activation condition, and I haven't activated, and this key matches my condition."
            self.give_self_upgrade(data, center, names, effects)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// `wouldUpgrade(keyMask)`: whether this module has an activation
    /// condition, has not executed, the key holds none of the conflicting
    /// upgrades, and holds all (`RequiresAllTriggers`) or any of the
    /// activating ones.
    pub fn would_upgrade(
        &self,
        data: &mut UpgradeMuxData,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        key_mask: UpgradeMaskType,
    ) -> IniResult<bool> {
        let (activation, conflicting) = data.get_upgrade_activation_masks(center, names)?;

        // "Make sure we have activation conditions and we haven't performed the upgrade already."
        if activation.any() && key_mask.any() && !self.upgrade_executed {
            // "Okay, make sure we don't have any conflicting upgrades"
            if !key_mask.test_for_any(&conflicting) {
                // "Finally check to see if our upgrade conditions match."
                if data.requires_all_triggers {
                    // "Make sure ALL triggers requirements are upgraded"
                    if key_mask.test_for_all(&activation) {
                        return Ok(true);
                    }
                } else if key_mask.test_for_any(&activation) {
                    // "Check if ANY trigger requirements are met."
                    return Ok(true);
                }
            }
        }
        // "We can't upgrade!"
        Ok(false)
    }

    /// `giveSelfUpgrade()`: the effect list, then the removals ("need to
    /// execute removals first, to prevent both being on for a moment"), then
    /// `upgradeImplementation`, then the upgrade is marked executed. A
    /// removal error stops it before `upgradeImplementation`.
    pub fn give_self_upgrade(
        &mut self,
        data: &mut UpgradeMuxData,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        effects: &mut impl UpgradeEffects,
    ) -> IniResult<()> {
        // performUpgradeFX()
        if let Some(fx) = &data.fx_list_upgrade {
            effects.do_fx_obj(fx);
        }
        // processUpgradeRemoval()
        data.mux_data_process_upgrade_removal(center, names, |t| effects.remove_upgrade(t))?;
        effects.upgrade_implementation();
        self.set_upgrade_executed(true);
        Ok(())
    }

    /// `testUpgradeConditions(keyMask)`: like `wouldUpgrade`, but ignoring
    /// whether the upgrade ran, and true for a module with no activation
    /// condition when the key has no conflicting upgrade ("this *upgrade* is
    /// relying only on not having conflicts").
    pub fn test_upgrade_conditions(
        &self,
        data: &mut UpgradeMuxData,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        key_mask: UpgradeMaskType,
    ) -> IniResult<bool> {
        let (activation, conflicting) = data.get_upgrade_activation_masks(center, names)?;

        // "Okay, make sure we don't have any conflicting upgrades"
        if !key_mask.any() || !key_mask.test_for_any(&conflicting) {
            // "Make sure we have activation conditions"
            if activation.any() {
                // "Finally check to see if our upgrade conditions match."
                if data.requires_all_triggers {
                    if key_mask.test_for_all(&activation) {
                        return Ok(true);
                    }
                } else if key_mask.test_for_any(&activation) {
                    return Ok(true);
                }
            } else {
                // "This *upgrade* is relying only on not having conflicts."
                return Ok(true);
            }
        }
        // "We can't upgrade!"
        Ok(false)
    }

    /// `resetUpgrade(keyMask)`: if the key holds any activating upgrade and
    /// the upgrade ran, mark it not executed (it may run again) and return
    /// true.
    pub fn reset_upgrade(
        &mut self,
        data: &mut UpgradeMuxData,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        key_mask: UpgradeMaskType,
    ) -> IniResult<bool> {
        let (activation, _conflicting) = data.get_upgrade_activation_masks(center, names)?;
        if key_mask.test_for_any(&activation) && self.upgrade_executed {
            self.upgrade_executed = false;
            return Ok(true);
        }
        Ok(false)
    }

    /// `upgradeMuxCRC`: "just call the regular xfer, it's simple".
    pub fn upgrade_mux_crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.upgrade_mux_xfer(xfer)
    }

    /// `upgradeMuxXfer` (version 1): the version, `m_upgradeExecuted`.
    pub fn upgrade_mux_xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_bool(&mut self.upgrade_executed)
    }

    /// `upgradeMuxLoadPostProcess`: nothing.
    pub fn upgrade_mux_load_post_process(&mut self) {}
}

/// `struct UpgradeModuleData : public BehaviorModuleData`.
#[derive(Debug, Default)]
pub struct UpgradeModuleData {
    /// The `BehaviorModuleData` base.
    pub behavior: BehaviorModuleData,
    /// `m_upgradeMuxData` (shared, with `mutable` members: see the module
    /// docs).
    upgrade_mux_data: RwLock<UpgradeMuxData>,
}

impl UpgradeModuleData {
    /// `UpgradeModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// The mux data, to read.
    pub fn mux_data(&self) -> std::sync::RwLockReadGuard<'_, UpgradeMuxData> {
        self.upgrade_mux_data
            .read()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// The mux data, to change (the `mutable` members).
    pub fn mux_data_mut(&self) -> std::sync::RwLockWriteGuard<'_, UpgradeMuxData> {
        self.upgrade_mux_data
            .write()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// A copy of the mux data (for comparing).
    pub fn mux_data_snapshot(&self) -> UpgradeMuxData {
        self.mux_data().clone()
    }

    /// `UpgradeModuleData::buildFieldParse(p)`: `ModuleData::buildFieldParse`,
    /// then the five mux fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, UpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_MODULE_DATA_FIELD_PARSE)
    }

    /// `UpgradeModule::friend_newModuleData(ini)`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<UpgradeModuleData> {
        let mut data = UpgradeModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, UpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `UpgradeMuxData::getFieldParse()` at `offsetof(UpgradeModuleData, m_upgradeMuxData)`.
pub static UPGRADE_MODULE_DATA_FIELD_PARSE: [FieldParse<UpgradeModuleData>; 6] = [
    FieldParse::new(
        "TriggeredBy",
        |i, d, u| UpgradeMuxData::parse_triggered_by(i, &mut d.mux_data_mut(), u),
        UserData::None,
    ),
    FieldParse::new(
        "ConflictsWith",
        |i, d, u| UpgradeMuxData::parse_conflicts_with(i, &mut d.mux_data_mut(), u),
        UserData::None,
    ),
    FieldParse::new(
        "RemovesUpgrades",
        |i, d, u| UpgradeMuxData::parse_removes_upgrades(i, &mut d.mux_data_mut(), u),
        UserData::None,
    ),
    FieldParse::new(
        "FXListUpgrade",
        |i, d, u| UpgradeMuxData::parse_fx_list_upgrade(i, &mut d.mux_data_mut(), u),
        UserData::None,
    ),
    FieldParse::new(
        "RequiresAllTriggers",
        |i, d, u| UpgradeMuxData::parse_requires_all_triggers(i, &mut d.mux_data_mut(), u),
        UserData::None,
    ),
    FieldParse::end(),
];

// --- p1l9-example ---
/// The field table of [`UPGRADE_MODULE_DATA_FIELD_PARSE`] for a concrete
/// module's data class `$t`, whose `UpgradeModuleData` base is the field
/// `$base` (an `Arc<UpgradeModuleData>`). Upstream adds the same table at the
/// base's offset; here each entry reaches the base through the field.
///
/// ```ignore
/// static BASE: [FieldParse<CommandSetUpgradeModuleData>; 6] =
///     upgrade_module_data_field_parse!(CommandSetUpgradeModuleData, upgrade);
/// ```
#[macro_export]
macro_rules! upgrade_module_data_field_parse {
    ($t:ty, $base:ident) => {
        [
            $crate::common::ini::FieldParse::<$t>::new(
                "TriggeredBy",
                |i, d, u| $crate::logic::module::upgrade_module::UpgradeMuxData::parse_triggered_by(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::new(
                "ConflictsWith",
                |i, d, u| $crate::logic::module::upgrade_module::UpgradeMuxData::parse_conflicts_with(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::new(
                "RemovesUpgrades",
                |i, d, u| $crate::logic::module::upgrade_module::UpgradeMuxData::parse_removes_upgrades(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::new(
                "FXListUpgrade",
                |i, d, u| $crate::logic::module::upgrade_module::UpgradeMuxData::parse_fx_list_upgrade(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::new(
                "RequiresAllTriggers",
                |i, d, u| $crate::logic::module::upgrade_module::UpgradeMuxData::parse_requires_all_triggers(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::end(),
        ]
    };
}
// --- end p1l9-example ---

/// `class UpgradeModule : public BehaviorModule, public UpgradeMux`.
#[derive(Clone, Debug)]
pub struct UpgradeModule {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// The typed module data (`getUpgradeModuleData()`).
    data: Arc<UpgradeModuleData>,
    /// The `UpgradeMux` base.
    mux: UpgradeMux,
}

impl UpgradeModule {
    /// `UpgradeModule(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<UpgradeModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let behavior = BehaviorModule::new(object, Some(ModuleDataKind::Upgrade(data.clone())))?;
        Ok(UpgradeModule {
            behavior,
            data,
            mux: UpgradeMux::new(),
        })
    }

    /// `UpgradeModule::getInterfaceMask()`: `MODULEINTERFACE_UPGRADE`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_UPGRADE
    }

    /// The `BehaviorModule` base.
    pub fn behavior(&self) -> &BehaviorModule {
        &self.behavior
    }

    /// The `UpgradeMux` base.
    pub fn mux(&self) -> &UpgradeMux {
        &self.mux
    }

    /// `getUpgradeModuleData()`.
    pub fn upgrade_module_data(&self) -> &Arc<UpgradeModuleData> {
        &self.data
    }

    /// `isTriggeredBy(upgrade)`.
    pub fn is_triggered_by(&self, upgrade: &[u8]) -> bool {
        self.data.mux_data().is_triggered_by(upgrade)
    }

    // --- p1l9-example ---
    /// The `UpgradeMux` base, to change. For a concrete module that runs the
    /// mux steps itself with the world in between (`logic::modules::upgrade_body`).
    pub fn mux_mut(&mut self) -> &mut UpgradeMux {
        &mut self.mux
    }
    // --- end p1l9-example ---

    /// `isAlreadyUpgraded()`.
    pub fn is_already_upgraded(&self) -> bool {
        self.mux.is_already_upgraded()
    }

    /// `forceRefreshUpgrade()`.
    pub fn force_refresh_upgrade(&self, effects: &mut impl UpgradeEffects) {
        self.mux.force_refresh_upgrade(effects);
    }

    /// `attemptUpgrade(keyMask)`.
    pub fn attempt_upgrade(
        &mut self,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        effects: &mut impl UpgradeEffects,
        key_mask: UpgradeMaskType,
    ) -> IniResult<bool> {
        let mut d = self.data.mux_data_mut();
        self.mux
            .attempt_upgrade(&mut d, center, names, effects, key_mask)
    }

    /// `wouldUpgrade(keyMask)`.
    pub fn would_upgrade(
        &self,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        key_mask: UpgradeMaskType,
    ) -> IniResult<bool> {
        let mut d = self.data.mux_data_mut();
        self.mux.would_upgrade(&mut d, center, names, key_mask)
    }

    /// `resetUpgrade(keyMask)`.
    pub fn reset_upgrade(
        &mut self,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        key_mask: UpgradeMaskType,
    ) -> IniResult<bool> {
        let mut d = self.data.mux_data_mut();
        self.mux.reset_upgrade(&mut d, center, names, key_mask)
    }

    /// `testUpgradeConditions(keyMask)`.
    pub fn test_upgrade_conditions(
        &self,
        center: &UpgradeCenter,
        names: &mut NameKeyGenerator,
        key_mask: UpgradeMaskType,
    ) -> IniResult<bool> {
        let mut d = self.data.mux_data_mut();
        self.mux
            .test_upgrade_conditions(&mut d, center, names, key_mask)
    }
}

impl Snapshot for UpgradeModule {
    /// `UpgradeModule::crc`: `BehaviorModule::crc`, then `upgradeMuxCRC`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)?;
        self.mux.upgrade_mux_crc(xfer)
    }

    /// `UpgradeModule::xfer` (version 1): the version, `BehaviorModule::xfer`,
    /// `upgradeMuxXfer` (its own version, `m_upgradeExecuted`).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)?;
        self.mux.upgrade_mux_xfer(xfer)
    }

    /// `UpgradeModule::loadPostProcess`: `BehaviorModule::loadPostProcess`,
    /// then `upgradeMuxLoadPostProcess`.
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
        self.mux.upgrade_mux_load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::common::name_key_generator::NameKeyType;
    use crate::common::upgrade::make_upgrade_mask;
    use crate::logic::module::test_support::{crc_of, load_bytes, save_bytes};

    /// Four upgrades A..D (mask bits 0..3), a name key generator.
    fn world() -> (UpgradeCenter, NameKeyGenerator) {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut center = UpgradeCenter::new();
        for n in ["A", "B", "C", "D"] {
            center.new_upgrade(&mut names, n.as_bytes()).unwrap();
        }
        (center, names)
    }

    fn mask(bits: &[i32]) -> UpgradeMaskType {
        make_upgrade_mask(bits).unwrap()
    }

    fn block(ini: &mut Ini<'_>, out: &mut Vec<IniResult<UpgradeModuleData>>) -> IniResult<()> {
        out.push(UpgradeModuleData::friend_new_module_data(Some(ini)));
        Ok(())
    }
    static BLOCKS: [BlockParse<Vec<IniResult<UpgradeModuleData>>>; 1] = [BlockParse {
        token: "D",
        parse: block,
    }];

    fn data_of(text: &str) -> IniResult<UpgradeModuleData> {
        let mut out = Vec::new();
        let _ = Ini::load(
            b"u.ini",
            format!("D\n{text}\nEnd\n").as_bytes(),
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut out,
        );
        out.remove(0)
    }

    fn module(text: &str) -> UpgradeModule {
        UpgradeModule::new(3, Some(Arc::new(data_of(text).unwrap()))).unwrap()
    }

    #[derive(Default)]
    struct Rec {
        log: Vec<String>,
    }
    impl UpgradeEffects for Rec {
        fn upgrade_implementation(&mut self) {
            self.log.push("impl".into());
        }
        fn do_fx_obj(&mut self, fx: &[u8]) {
            self.log.push(format!("fx {}", String::from_utf8_lossy(fx)));
        }
        fn remove_upgrade(&mut self, t: &UpgradeTemplate) {
            self.log.push(format!(
                "remove {}",
                String::from_utf8_lossy(t.upgrade_name())
            ));
        }
    }

    #[test]
    fn the_five_fields_are_read() {
        let d = data_of("TriggeredBy = A B\n ConflictsWith = C\n RemovesUpgrades = D A\n FXListUpgrade = FX_Up\n RequiresAllTriggers = Yes").unwrap();
        let m = d.mux_data_snapshot();
        let v = |s: &[&str]| s.iter().map(|x| x.as_bytes().to_vec()).collect::<Vec<_>>();
        assert_eq!(m.activation_upgrade_names, v(&["A", "B"]));
        assert_eq!(m.conflicting_upgrade_names, v(&["C"]));
        assert_eq!(m.removal_upgrade_names, v(&["D", "A"]));
        assert_eq!(m.fx_list_upgrade, Some(b"FX_Up".to_vec()));
        assert!(m.requires_all_triggers);
        assert!(
            m.trigger_upgrade_names.is_empty(),
            "the trigger names are filled when the masks are first computed"
        );
        assert!(!m.activation_mask().any() && !m.conflicting_mask().any());
        // the defaults, None, a repeated field replacing the list
        let d = data_of("FXListUpgrade = none\nTriggeredBy = A\nTriggeredBy = B C").unwrap();
        assert_eq!(d.mux_data().fx_list_upgrade, None);
        assert_eq!(d.mux_data().activation_upgrade_names.len(), 2);
        assert!(!d.mux_data().requires_all_triggers);
        // a bad value and an unknown field
        assert!(data_of("RequiresAllTriggers = maybe").is_err());
        assert_eq!(
            data_of("Nope = 1").unwrap_err().kind,
            IniErrorKind::UnknownToken
        );
    }

    #[test]
    fn masks_are_computed_on_first_use_and_the_names_cleared() {
        let (center, mut names) = world();
        let m = module("TriggeredBy = B D\n ConflictsWith = C A");
        let before = names.name_to_key(b"Fresh").0;
        let (act, con) = m
            .upgrade_module_data()
            .mux_data_mut()
            .get_upgrade_activation_masks(&center, &mut names)
            .unwrap();
        assert_eq!((act, con), (mask(&[1, 3]), mask(&[0, 2])));
        let d = m.upgrade_module_data().mux_data_snapshot();
        assert!(d.activation_upgrade_names.is_empty() && d.conflicting_upgrade_names.is_empty());
        assert_eq!(d.trigger_upgrade_names, vec![b"B".to_vec(), b"D".to_vec()]);
        assert_eq!((d.activation_mask(), d.conflicting_mask()), (act, con));
        // the lookup assigned no new key (the names already had them) and a second ask changes nothing
        assert_eq!(names.name_to_key(b"Fresh2").0, before + 1);
        let again = m
            .upgrade_module_data()
            .mux_data_mut()
            .get_upgrade_activation_masks(&center, &mut names)
            .unwrap();
        assert_eq!(again, (act, con));
        // is_triggered_by: any case, trigger names only (not the conflicting ones)
        assert!(
            m.is_triggered_by(b"b")
                && m.is_triggered_by(b"D")
                && !m.is_triggered_by(b"A")
                && !m.is_triggered_by(b"C")
        );
        // before the first ask there are no trigger names
        assert!(!module("TriggeredBy = B").is_triggered_by(b"B"));
    }

    #[test]
    fn the_first_ask_assigns_a_key_to_each_name_in_order() {
        let (center, mut names) = world();
        let m = module("TriggeredBy = A Zed\n ConflictsWith = Yak");
        let next = names.name_to_key(b"Probe").0;
        let e = m
            .upgrade_module_data()
            .mux_data_mut()
            .get_upgrade_activation_masks(&center, &mut names)
            .unwrap_err();
        // Zed is not an upgrade: INI_INVALID_DATA, after "A" (known) and "Zed" were looked up
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        assert_eq!(
            names.name_to_key(b"Zed").0,
            next + 1,
            "Zed got the key after Probe"
        );
        assert_eq!(
            names.name_to_key(b"Yak").0,
            next + 2,
            "Yak was not looked up (the ask stopped at Zed): this is its first key"
        );
        // the names stay, the mask is the part built so far
        let d = m.upgrade_module_data().mux_data_snapshot();
        assert_eq!(d.activation_upgrade_names.len(), 2);
        assert_eq!(d.activation_mask(), mask(&[0]));
        assert!(d.trigger_upgrade_names.is_empty());
    }

    #[test]
    fn an_empty_or_none_name_that_finds_no_upgrade_is_undefined() {
        let (center, mut names) = world();
        for text in [
            "TriggeredBy = None",
            "ConflictsWith = NONE",
            "TriggeredBy = A none",
        ] {
            let m = module(text);
            let e = m
                .upgrade_module_data()
                .mux_data_mut()
                .get_upgrade_activation_masks(&center, &mut names)
                .unwrap_err();
            assert_eq!(
                e.kind,
                IniErrorKind::Undefined(Undefined::NullUpgradeTemplate),
                "{text}"
            );
        }
        // ... unless an upgrade really has that name
        let mut c2 = center.clone();
        c2.new_upgrade(&mut names, b"None").unwrap();
        let m = module("TriggeredBy = None");
        let (act, _) = m
            .upgrade_module_data()
            .mux_data_mut()
            .get_upgrade_activation_masks(&c2, &mut names)
            .unwrap();
        assert_eq!(act, mask(&[4]));
        // an INI line with no names leaves the list empty: no conditions at all
        let (center, mut names) = world();
        let m = module("RequiresAllTriggers = No");
        let r = m
            .upgrade_module_data()
            .mux_data_mut()
            .get_upgrade_activation_masks(&center, &mut names)
            .unwrap();
        assert!(!r.0.any() && !r.1.any());
    }

    #[test]
    fn would_upgrade_needs_a_condition_a_key_no_conflict_and_a_fresh_module() {
        let (center, mut names) = world();
        let m = module("TriggeredBy = A B\n ConflictsWith = D");
        let w = |m: &UpgradeModule, names: &mut NameKeyGenerator, bits: &[i32]| {
            m.would_upgrade(&center, names, mask(bits)).unwrap()
        };
        assert!(w(&m, &mut names, &[0]), "any trigger");
        assert!(w(&m, &mut names, &[1, 2]));
        assert!(!w(&m, &mut names, &[2]), "no trigger in the key");
        assert!(!w(&m, &mut names, &[]), "an empty key");
        assert!(!w(&m, &mut names, &[0, 3]), "a conflict");
        let all = module("TriggeredBy = A B\n RequiresAllTriggers = Yes");
        assert!(
            !w(&all, &mut names, &[0])
                && w(&all, &mut names, &[0, 1])
                && w(&all, &mut names, &[0, 1, 2])
        );
        // no activation condition: never
        let none = module("ConflictsWith = D");
        assert!(!w(&none, &mut names, &[0]));
    }

    #[test]
    fn test_upgrade_conditions_ignores_whether_it_ran_and_accepts_no_condition() {
        let (center, mut names) = world();
        let t = |m: &UpgradeModule, names: &mut NameKeyGenerator, bits: &[i32]| {
            m.test_upgrade_conditions(&center, names, mask(bits))
                .unwrap()
        };
        let m = module("TriggeredBy = A B\n ConflictsWith = D");
        assert!(t(&m, &mut names, &[0]) && !t(&m, &mut names, &[2]) && !t(&m, &mut names, &[0, 3]));
        // an empty key has no conflicts, but then no trigger either
        assert!(!t(&m, &mut names, &[]));
        // only conflicts: true unless one is present
        let only = module("ConflictsWith = D");
        assert!(
            t(&only, &mut names, &[]) && t(&only, &mut names, &[1]) && !t(&only, &mut names, &[3])
        );
        // nothing at all: always true
        let nothing = module("RequiresAllTriggers = Yes");
        assert!(t(&nothing, &mut names, &[]) && t(&nothing, &mut names, &[0, 1, 2, 3]));
        let all = module("TriggeredBy = A B\n RequiresAllTriggers = Yes");
        assert!(!t(&all, &mut names, &[0]) && t(&all, &mut names, &[0, 1]));
        // it does not look at the executed flag
        let mut ran = module("TriggeredBy = A");
        ran.attempt_upgrade(&center, &mut names, &mut Rec::default(), mask(&[0]))
            .unwrap();
        assert!(ran.is_already_upgraded() && t(&ran, &mut names, &[0]));
    }

    #[test]
    fn attempt_runs_the_fx_the_removals_then_the_implementation_once() {
        let (center, mut names) = world();
        let mut m = module("TriggeredBy = A\n RemovesUpgrades = C B\n FXListUpgrade = FX_Go");
        let mut rec = Rec::default();
        assert!(!m
            .attempt_upgrade(&center, &mut names, &mut rec, mask(&[1]))
            .unwrap());
        assert!(rec.log.is_empty() && !m.is_already_upgraded());
        assert!(m
            .attempt_upgrade(&center, &mut names, &mut rec, mask(&[0, 1]))
            .unwrap());
        assert_eq!(rec.log, ["fx FX_Go", "remove C", "remove B", "impl"]);
        assert!(m.is_already_upgraded());
        // once
        assert!(!m
            .attempt_upgrade(&center, &mut names, &mut rec, mask(&[0]))
            .unwrap());
        assert_eq!(rec.log.len(), 4);
        // forceRefresh runs the implementation again, only because it ran
        m.force_refresh_upgrade(&mut rec);
        assert_eq!(rec.log.last().unwrap(), "impl");
        let fresh = module("TriggeredBy = A");
        let mut r2 = Rec::default();
        fresh.force_refresh_upgrade(&mut r2);
        assert!(r2.log.is_empty());
        // reset: only with a trigger in the key and only if it ran
        assert!(!m.reset_upgrade(&center, &mut names, mask(&[2])).unwrap());
        assert!(m.is_already_upgraded());
        assert!(m.reset_upgrade(&center, &mut names, mask(&[0])).unwrap());
        assert!(!m.is_already_upgraded());
        assert!(!m.reset_upgrade(&center, &mut names, mask(&[0])).unwrap());
        // ... and it can run again
        assert!(m
            .attempt_upgrade(&center, &mut names, &mut rec, mask(&[0]))
            .unwrap());
    }

    #[test]
    fn a_bad_removal_name_stops_the_upgrade_after_the_earlier_removals() {
        let (center, mut names) = world();
        let mut m = module("TriggeredBy = A\n RemovesUpgrades = B Nope C\n FXListUpgrade = FX_Go");
        let mut rec = Rec::default();
        let e = m
            .attempt_upgrade(&center, &mut names, &mut rec, mask(&[0]))
            .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        assert_eq!(
            rec.log,
            ["fx FX_Go", "remove B"],
            "the fx and B's removal happened, the implementation did not"
        );
        assert!(!m.is_already_upgraded());
        // a removal name of None finds no upgrade: undefined, before anything is removed for it
        let mut n = module("TriggeredBy = A\n RemovesUpgrades = B None");
        let mut rec = Rec::default();
        let e = n
            .attempt_upgrade(&center, &mut names, &mut rec, mask(&[0]))
            .unwrap_err();
        assert_eq!(
            e.kind,
            IniErrorKind::Undefined(Undefined::NullUpgradeTemplate)
        );
        assert_eq!(rec.log, ["remove B"]);
    }

    #[test]
    fn xfer_is_two_versions_and_the_flag() {
        let (center, mut names) = world();
        let mut m = module("TriggeredBy = A");
        assert_eq!(save_bytes(&mut m).unwrap(), [1, 1, 1, 1, 1, 0]);
        m.attempt_upgrade(&center, &mut names, &mut Rec::default(), mask(&[0]))
            .unwrap();
        let bytes = save_bytes(&mut m).unwrap();
        assert_eq!(bytes, [1, 1, 1, 1, 1, 1]);
        let mut other = module("TriggeredBy = A");
        load_bytes(&mut other, bytes).unwrap();
        assert!(other.is_already_upgraded());
        // the CRC is the same bytes as the xfer from UpgradeMux (version, flag), after BehaviorModule's nothing
        let mut crc_a = module("TriggeredBy = A");
        let mut crc_b = module("TriggeredBy = A");
        crc_b
            .attempt_upgrade(&center, &mut names, &mut Rec::default(), mask(&[0]))
            .unwrap();
        assert_ne!(crc_of(&mut crc_a).unwrap(), crc_of(&mut crc_b).unwrap());
        assert_eq!(UpgradeModule::get_interface_mask(), 0x80);
        assert!(UpgradeModule::new(1, None).is_err());
        let _ = NameKeyType::INVALID;
    }
}
