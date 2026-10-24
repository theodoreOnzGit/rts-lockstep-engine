// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/CrateCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CrateCollide`, the base of the crate pickup modules: `CrateCollideModuleData` ([`CrateCollideModuleData`], with the field table every
//! derived data class reuses through [`crate_collide_data_field_parse!`]) and the behaviour every crate has ([`CrateBehavior`],
//! [`crate_module_call`]): `onCollide(other)` runs the derived module's `executeCrateBehavior(other)` if `isValidToExecute(other)`, plays the
//! execute FX on `other`, destroys the crate and shows the execute animation.
//!
//! # What is kept as a name
//!
//! `PickupScience` is `INI::parseScience`, which needs `TheScienceStore`; a module's data proc has no store, so the science is kept as its name
//! and looked up in `world.statics.science_store` when `isValidToExecute` needs it (as `GrantScienceUpgrade` does). A name the store does not
//! know is an `INI_INVALID_DATA` throw upstream when the data is read; here it is an error when the crate is first tested
//! ([`Undefined::UnknownPickupScience`]). `ExecuteFX` is a name as everywhere else in this translation (`None` is `NULL`).
//!
//! # Deviation: `TheAnim2DCollection`
//!
//! The execute animation (`TheAnim2DCollection->findTemplate`, `TheInGameUI->addWorldAnimation`) is the client's: it is the cut call
//! [`CutCall::InGameUiAddWorldAnimation`], made when the name is not empty, `TheGameLogic->getDrawIconUI()` and the collection exists
//! (`cuts.globals.anim2d_collection_present`).
//!
//! `p1w11-sonnet`, 2026-10-21.

use crate::common::base_type::Coord3D;
use crate::common::damage_fx::FxListName;
use crate::common::game_common::PlayerType;
use crate::common::ini::{Ini, IniError, IniResult, Undefined};
use crate::common::kind_of::{KindOfMaskType, KINDOF_PARACHUTE, KINDOF_STRUCTURE};
use crate::common::module::ObjectId;
use crate::logic::module::collide_module::{CollideModule, CollideModuleData};
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::update_body::UpdateBody;

/// `class CrateCollideModuleData : public CollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CrateCollideModuleData {
    /// The `CollideModuleData` base.
    pub collide: CollideModuleData,
    /// `m_kindof`: "the kind(s) of units that can be collided with" (`RequiredKindOf`).
    pub kind_of: KindOfMaskType,
    /// `m_kindofnot`: "the kind(s) of units that CANNOT be collided with" (`ForbiddenKindOf`).
    pub kind_of_not: KindOfMaskType,
    /// `m_isForbidOwnerPlayer`: "this crate cannot be picked up by the player of the dead thing that made it".
    pub is_forbid_owner_player: bool,
    /// `m_isBuildingPickup`: "this crate can be picked up by a building (bypassing the AI requirement)".
    pub is_building_pickup: bool,
    /// `m_isHumanOnlyPickup`: "can this crate only be picked up by a human player? (mission thing)".
    pub is_human_only_pickup: bool,
    /// `m_pickupScience` as the science's name (empty for `SCIENCE_INVALID`); see the module docs.
    pub pickup_science: Vec<u8>,
    /// `m_executeFX`: the FX list played when activated.
    pub execute_fx: FxListName,
    /// `m_executionAnimationTemplate`: "Anim2D to play at crate location".
    pub execution_animation_template: Vec<u8>,
    /// `m_executeAnimationDisplayTimeInSeconds`.
    pub execute_animation_display_time_in_seconds: f32,
    /// `m_executeAnimationZRisePerSecond`.
    pub execute_animation_z_rise_per_second: f32,
    /// `m_executeAnimationFades` (`TRUE` by default).
    pub execute_animation_fades: bool,
}

impl CrateCollideModuleData {
    /// `CrateCollideModuleData()`.
    pub fn new() -> Self {
        CrateCollideModuleData {
            collide: CollideModuleData::new(),
            kind_of: KindOfMaskType::new(),
            kind_of_not: KindOfMaskType::new(),
            is_forbid_owner_player: false,
            is_building_pickup: false,
            is_human_only_pickup: false,
            pickup_science: Vec::new(),
            execute_fx: None,
            execution_animation_template: Vec::new(),
            execute_animation_display_time_in_seconds: 0.0,
            execute_animation_z_rise_per_second: 0.0,
            execute_animation_fades: true,
        }
    }

    /// `INI::parseFXList`, as far as the token goes: the next token names the list, `None` is `NULL`.
    pub fn parse_fx_list(ini: &mut Ini<'_>) -> IniResult<FxListName> {
        let token = ini.get_next_token(None)?;
        Ok(if token.eq_ignore_ascii_case(b"None") {
            None
        } else {
            Some(token)
        })
    }
}

/// `CrateCollideModuleData::buildFieldParse`'s own table (after `ModuleData::buildFieldParse`), for a derived data class `$t` whose
/// `CrateCollideModuleData` is the field `$base`.
#[macro_export]
macro_rules! crate_collide_data_field_parse {
    ($t:ty, $base:ident) => {
        [
            $crate::common::ini::FieldParse::<$t>::new("RequiredKindOf", |i, d, _| d.$base.kind_of.parse_from_ini(i), $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("ForbiddenKindOf", |i, d, _| d.$base.kind_of_not.parse_from_ini(i), $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("ForbidOwnerPlayer", |i, d, _| { d.$base.is_forbid_owner_player = i.parse_bool()?; Ok(()) }, $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("BuildingPickup", |i, d, _| { d.$base.is_building_pickup = i.parse_bool()?; Ok(()) }, $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("HumanOnly", |i, d, _| { d.$base.is_human_only_pickup = i.parse_bool()?; Ok(()) }, $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("PickupScience", |i, d, _| { d.$base.pickup_science = i.get_next_token(None)?; Ok(()) }, $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("ExecuteFX", |i, d, _| { d.$base.execute_fx = $crate::logic::modules::crate_collide::CrateCollideModuleData::parse_fx_list(i)?; Ok(()) }, $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("ExecuteAnimation", |i, d, _| { d.$base.execution_animation_template = i.parse_ascii_string()?; Ok(()) }, $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("ExecuteAnimationTime", |i, d, _| { d.$base.execute_animation_display_time_in_seconds = i.parse_real()?; Ok(()) }, $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("ExecuteAnimationZRise", |i, d, _| { d.$base.execute_animation_z_rise_per_second = i.parse_real()?; Ok(()) }, $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::new("ExecuteAnimationFades", |i, d, _| { d.$base.execute_animation_fades = i.parse_bool()?; Ok(()) }, $crate::common::ini::UserData::None),
            $crate::common::ini::FieldParse::<$t>::end(),
        ]
    };
}

/// A concrete crate module (`class X : public CrateCollide`). A compile-time contract, used as a generic bound by [`crate_module_call`].
pub trait CrateBehavior: Clone + PartialEq + Sized {
    /// The `CollideModule` base.
    fn collide(&self) -> &CollideModule;
    /// `getCrateCollideModuleData()`: the `CrateCollideModuleData` base of the module's data.
    fn crate_data(&self) -> &CrateCollideModuleData;
    /// `executeCrateBehavior(other)` (pure virtual): "did the crate act?". An error is upstream's undefined behaviour.
    fn execute_crate_behavior(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        other: ObjectId,
    ) -> Result<bool, IniError>;
    /// `isValidToExecute(other)` (virtual; `other` is `INVALID_ID` for `NULL`): the base's unless the module overrides it.
    fn is_valid_to_execute(
        &self,
        world: &mut World,
        me: ModuleHandle,
        other: ObjectId,
    ) -> Result<bool, IniError> {
        crate_is_valid_to_execute(self.crate_data(), world, me, other)
    }
    /// The module as the variant of [`UpdateBody`] it is.
    fn into_body(self) -> UpdateBody;
    /// The module back out of its variant; `None` for another variant.
    fn from_body(body: UpdateBody) -> Option<Self>;
    /// A call the base does not answer (a module's own interface, such as `isSalvageCrateCollide`): [`ModuleReply::Unhandled`] unless it
    /// overrides this.
    fn other_call(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        _call: ModuleCall,
    ) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

/// `TheGameLogic->destroyObject(obj)` for a crate (the crate itself, or `ConvertToHijackedVehicleCrateCollide`'s hijacker): a trace line for the tests
/// that compare the call, then the destruction.
pub fn crate_destroy_object(world: &mut World, id: ObjectId) {
    if let Some(t) = &mut world.cuts.trace {
        t.push(format!("destroy {id}"));
    }
    world.destroy_object(id);
}

/// `CrateCollide::isValidToExecute(other)`. An error is upstream's undefined behaviour (a pickup science the store does not know).
pub fn crate_is_valid_to_execute(
    md: &CrateCollideModuleData,
    world: &mut World,
    me: ModuleHandle,
    other: ObjectId,
) -> Result<bool, IniError> {
    // `other == NULL`
    let none_mask = KindOfMaskType::new();
    let (is_structure, has_ai, kind_ok, dead, parachute) = match world.objects.find(other) {
        Some(o) => (
            o.is_kind_of(KINDOF_STRUCTURE),
            o.ai.is_some(),
            o.template().is_kind_of_multi(
                &md.kind_of,
                if crate::logic::modules::plant(758) {
                    &none_mask
                } else {
                    &md.kind_of_not
                },
            ),
            o.is_effectively_dead(),
            o.is_kind_of(KINDOF_PARACHUTE),
        ),
        None => return Ok(false),
    };
    if world.is_neutral_controlled(other) && !crate::logic::modules::plant(754) {
        return Ok(false);
    }
    let valid_building_attempt =
        md.is_building_pickup && is_structure && !crate::logic::modules::plant(760);
    // "building exception flag for Drop Zone"
    if !has_ai && !valid_building_attempt {
        return Ok(false);
    }
    if !kind_ok {
        return Ok(false);
    }
    if dead {
        return Ok(false);
    }
    if world.is_above_terrain(me.object) && !valid_building_attempt {
        return Ok(false);
    }
    let (my_player, other_player) = (
        world.get_controlling_player(me.object),
        world.get_controlling_player(other),
    );
    if md.is_forbid_owner_player && (my_player == other_player) != crate::logic::modules::plant(755)
    {
        // "design has decreed this to not be picked up by the dead guy's team"
        return Ok(false);
    }
    if md.is_human_only_pickup {
        if let Some(p) = other_player {
            // "human only mission crate"
            if world.players.get_nth(p).is_some_and(|pl| {
                (pl.player_type != PlayerType::Human) != crate::logic::modules::plant(763)
            }) {
                return Ok(false);
            }
        }
    }
    if !md.pickup_science.is_empty() {
        if let Some(p) = other_player {
            // "science required to pick this up"
            let science = world
                .statics
                .science_store
                .friend_lookup_science(&mut world.names, &md.pickup_science)
                .map_err(|_| IniError::undefined(Undefined::UnknownPickupScience))?;
            if world
                .players
                .get_nth(p)
                .is_some_and(|pl| pl.sciences.contains(&science))
                == crate::logic::modules::plant(762)
            {
                return Ok(false);
            }
        }
    }
    if parachute && !crate::logic::modules::plant(761) {
        return Ok(false);
    }
    Ok(true)
}

/// `CrateCollide::onCollide(other, loc, normal)`: "note that when other is NULL it means 'collide with ground'".
pub fn crate_on_collide<M: CrateBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    other: ObjectId,
) {
    let md = m.crate_data().clone();
    match m.is_valid_to_execute(world, me, other) {
        Err(e) => world.cuts.module_errors.push((me, e)),
        Ok(false) => {}
        Ok(true) => {
            match m.execute_crate_behavior(world, me, other) {
                Err(e) => {
                    world.cuts.module_errors.push((me, e));
                    return;
                }
                Ok(true) => {
                    if let Some(fx) = md
                        .execute_fx
                        .as_ref()
                        .filter(|_| !crate::logic::modules::plant(756))
                    {
                        world.emit(ObjectClientEvent::FxListDoFxObj {
                            fx: fx.clone(),
                            object_id: other,
                        });
                    }
                    if !crate::logic::modules::plant(757) {
                        crate_destroy_object(world, me.object);
                    }
                }
                Ok(false) => {}
            }
            if world.cuts.globals.anim2d_collection_present
                && !md.execution_animation_template.is_empty()
                && (world.cuts.globals.draw_icon_ui || crate::logic::modules::plant(759))
            {
                let position = world
                    .objects
                    .find(me.object)
                    .map_or_else(Coord3D::default, |o| o.get_position());
                world.cut(CutCall::InGameUiAddWorldAnimation {
                    template: md.execution_animation_template.clone(),
                    position,
                    display_time_in_seconds: md.execute_animation_display_time_in_seconds,
                    z_rise_per_second: md.execute_animation_z_rise_per_second,
                });
            }
        }
    }
}

/// The answer of a crate module to one of `Object`'s calls.
pub fn crate_module_call<M: CrateBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    call: ModuleCall,
) -> ModuleReply {
    match call {
        ModuleCall::OnCollide { other, .. } => {
            lend_clone(world, me, m, M::into_body, M::from_body, |w, m| {
                crate_on_collide(m, w, me, other)
            });
            ModuleReply::Done
        }
        // `wouldLikeToCollideWith(other)`: `isValidToExecute(other)`
        ModuleCall::WouldLikeToCollideWith { other } => {
            match m.is_valid_to_execute(world, me, other) {
                Ok(b) => ModuleReply::Bool(b),
                Err(e) => {
                    world.cuts.module_errors.push((me, e));
                    ModuleReply::Bool(false)
                }
            }
        }
        ModuleCall::IsRailroad
        | ModuleCall::IsCarBombCrateCollide
        | ModuleCall::IsHijackedVehicleCrateCollide
        | ModuleCall::IsSabotageBuildingCrateCollide => match m.other_call(world, me, call) {
            ModuleReply::Unhandled => ModuleReply::Bool(false),
            r => r,
        },
        other => m.other_call(world, me, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_upstreams() {
        let d = CrateCollideModuleData::new();
        assert!(
            d.execute_animation_fades
                && !d.is_forbid_owner_player
                && d.pickup_science.is_empty()
                && d.execute_fx.is_none()
        );
    }
}
