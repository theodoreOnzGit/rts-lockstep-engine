// SPDX-License-Identifier: GPL-3.0-only
//! Layer-9 concrete modules, one file each (`Object/Upgrade/` first).
//!
//! Upstream's `UpgradeModuleData` subclasses add fields to the mux data; the
//! five mux fields (`TriggeredBy`, `ConflictsWith`, `RemovesUpgrades`,
//! `FXListUpgrade`, `RequiresAllTriggers`) are parsed here by
//! [`upgrade_mux_field_table!`] into a table over the subclass, because the
//! layer-5 parse functions are private to `upgrade_module.rs`. The bodies
//! are copies of those functions (same INI calls, same order).

use crate::common::ini::{Ini, IniResult, UserData};
use crate::logic::module::upgrade_module::UpgradeMuxData;

/// `TriggeredBy`: `parseAsciiStringVector` into `m_activationUpgradeNames`.
pub(crate) fn parse_triggered_by(ini: &mut Ini<'_>, d: &mut UpgradeMuxData, _: &UserData) -> IniResult<()> {
    ini.parse_ascii_string_vector(&mut d.activation_upgrade_names)
}

/// `ConflictsWith`: into `m_conflictingUpgradeNames`.
pub(crate) fn parse_conflicts_with(ini: &mut Ini<'_>, d: &mut UpgradeMuxData, _: &UserData) -> IniResult<()> {
    ini.parse_ascii_string_vector(&mut d.conflicting_upgrade_names)
}

/// `RemovesUpgrades`: into `m_removalUpgradeNames`.
pub(crate) fn parse_removes_upgrades(ini: &mut Ini<'_>, d: &mut UpgradeMuxData, _: &UserData) -> IniResult<()> {
    ini.parse_ascii_string_vector(&mut d.removal_upgrade_names)
}

/// `FXListUpgrade`: `INI::parseFXList`, as far as the token goes.
pub(crate) fn parse_fx_list_upgrade(ini: &mut Ini<'_>, d: &mut UpgradeMuxData, _: &UserData) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    d.fx_list_upgrade = if token.eq_ignore_ascii_case(b"None") { None } else { Some(token) };
    Ok(())
}

/// `RequiresAllTriggers`: a bool.
pub(crate) fn parse_requires_all_triggers(ini: &mut Ini<'_>, d: &mut UpgradeMuxData, _: &UserData) -> IniResult<()> {
    d.requires_all_triggers = ini.parse_bool()?;
    Ok(())
}

/// The five mux fields plus the terminator, over a type `$t` whose
/// `UpgradeModuleData` base is the field `$base` (an `Arc<UpgradeModuleData>`
/// or the data itself: anything with `mux_data_mut()` after `Deref`).
#[macro_export]
macro_rules! upgrade_mux_field_table {
    ($t:ty, $base:ident) => {
        [
            $crate::common::ini::FieldParse::new(
                "TriggeredBy",
                |i, d: &mut $t, u| $crate::logic::modules::parse_triggered_by(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::new(
                "ConflictsWith",
                |i, d: &mut $t, u| $crate::logic::modules::parse_conflicts_with(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::new(
                "RemovesUpgrades",
                |i, d: &mut $t, u| $crate::logic::modules::parse_removes_upgrades(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::new(
                "FXListUpgrade",
                |i, d: &mut $t, u| $crate::logic::modules::parse_fx_list_upgrade(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::new(
                "RequiresAllTriggers",
                |i, d: &mut $t, u| $crate::logic::modules::parse_requires_all_triggers(i, &mut d.$base.mux_data_mut(), u),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::end(),
        ]
    };
}

// --- p1l9-upgrades ---
pub mod weapon_set_upgrade;
pub mod weapon_bonus_upgrade;
pub mod armor_upgrade;
pub mod model_condition_upgrade;
pub mod experience_scalar_upgrade;
pub mod active_shroud_upgrade;
pub mod max_health_upgrade;
pub mod grant_science_upgrade;
// --- end p1l9-upgrades ---
