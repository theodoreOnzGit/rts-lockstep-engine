// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DeliverPayloadAIUpdate.h   (class DeliverPayloadData)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/DeliverPayloadAIUpdate.cpp   (DeliverPayloadData::getFieldParse)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DeliverPayloadData` (wave 7, `p1w10-sonnet`): "all the necessary information for `DeliverPayloadAIUpdate` to perform its necessary
//! functions", set only by `DeliverPayloadNugget` in `ObjectCreationList.ini`.
//!
//! This is the data class of `DeliverPayloadAIUpdate.h`, its constructor and `getFieldParse()`, which the nugget composes into its own
//! parse. `DeliverPayloadAIUpdate` itself (the AI update, its state machine) is another unit and is not translated.
//!
//! `getFieldParse()` is a static table of offsets into the data; here it is [`DeliverPayloadData::field_parse`], generic over whatever
//! struct embeds the data, through [`DeliverPayloadDataParse`]: the nugget's parse target, today, and the module data of the AI update
//! when that unit is translated. The two lookups the table makes (`INI::parseWeaponTemplate`, `INI::parseFXList`) are the target's,
//! because the weapon and FX list stores are not here yet.
//!
//! Verified by `differential_object_creation_list.rs` through the nugget's `DeliverPayload` block, against the real class and table
//! (verbatim text, cut out by `build.rs`).

use crate::common::base_type::Coord3D;
use crate::common::damage_fx::FxListName;
use crate::common::ini::{FieldParse, Ini, IniResult, LookupListRec, UserData};
use crate::logic::radius_decal::RadiusDecalTemplate;

/// What `INI::parseWeaponTemplate` stores: the weapon template found by name (`None` for `NULL`). The id is the weapon store's.
pub type WeaponTemplateRef = u32;

/// `TheWeaponSlotTypeNamesLookupList` (`WeaponSet.h`), without the terminator.
pub static WEAPON_SLOT_TYPE_NAMES_LOOKUP_LIST: [LookupListRec; 3] = [
    LookupListRec {
        name: "PRIMARY",
        value: 0,
    },
    LookupListRec {
        name: "SECONDARY",
        value: 1,
    },
    LookupListRec {
        name: "TERTIARY",
        value: 2,
    },
];

/// `class DeliverPayloadData`.
#[derive(Clone, Debug, PartialEq)]
pub struct DeliverPayloadData {
    /// `m_visibleDropBoneName`: "where the payload is created (offset by current bone number 01-xx)".
    pub visible_drop_bone_name: Vec<u8>,
    /// `m_visibleSubObjectName`: "visible subobject to show or hide (offset by current drop number 01-xx)".
    pub visible_sub_object_name: Vec<u8>,
    /// `m_visiblePayloadTemplateName`.
    pub visible_payload_template_name: Vec<u8>,
    /// `m_distToTarget`.
    pub dist_to_target: f32,
    /// `m_preOpenDistance`.
    pub pre_open_distance: f32,
    /// `m_maxAttempts`.
    pub max_attempts: i32,
    /// `m_dropOffset`.
    pub drop_offset: Coord3D,
    /// `m_dropVariance`.
    pub drop_variance: Coord3D,
    /// `m_dropDelay`, in logic frames.
    pub drop_delay: u32,
    /// `m_fireWeapon`.
    pub fire_weapon: bool,
    /// `m_selfDestructObject`.
    pub self_destruct_object: bool,
    /// `m_visibleNumBones`: "the number of visible bones to process".
    pub visible_num_bones: i32,
    /// `m_diveStartDistance`.
    pub dive_start_distance: f32,
    /// `m_diveEndDistance`.
    pub dive_end_distance: f32,
    /// `m_strafingWeaponSlot`, a `WeaponSlotType` (-1 for none).
    pub strafing_weapon_slot: i32,
    /// `m_visibleItemsDroppedPerInterval`.
    pub visible_items_dropped_per_interval: i32,
    /// `m_inheritTransportVelocity`.
    pub inherit_transport_velocity: bool,
    /// `m_isParachuteDirectly`: "instead of parachuting to below the point of exit, go ahead and bunch up on the target".
    pub is_parachute_directly: bool,
    /// `m_exitPitchRate`.
    pub exit_pitch_rate: f32,
    /// `m_strafeFX`.
    pub strafe_fx: FxListName,
    /// `m_strafeLength`.
    pub strafe_length: f32,
    /// `m_visiblePayloadWeaponTemplate`.
    pub visible_payload_weapon_template: Option<WeaponTemplateRef>,
    /// `m_deliveryDecalTemplate`.
    pub delivery_decal_template: RadiusDecalTemplate,
    /// `m_deliveryDecalRadius`.
    pub delivery_decal_radius: f32,
}

impl Default for DeliverPayloadData {
    /// `DeliverPayloadData()`.
    fn default() -> Self {
        DeliverPayloadData {
            visible_drop_bone_name: Vec::new(),
            visible_sub_object_name: Vec::new(),
            visible_payload_template_name: Vec::new(),
            dist_to_target: 0.0,
            pre_open_distance: 0.0,
            max_attempts: 1,
            drop_offset: Coord3D::default(),
            drop_variance: Coord3D::default(),
            drop_delay: 0,
            fire_weapon: false,
            self_destruct_object: false,
            visible_num_bones: 0,
            dive_start_distance: 0.0,
            dive_end_distance: 0.0,
            strafing_weapon_slot: -1,
            visible_items_dropped_per_interval: 0,
            inherit_transport_velocity: false,
            is_parachute_directly: false,
            exit_pitch_rate: 0.0,
            strafe_fx: None,
            strafe_length: 0.0,
            visible_payload_weapon_template: None,
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
        }
    }
}

/// What embeds a [`DeliverPayloadData`] and parses it: the data to fill and the two stores the table looks names up in.
pub trait DeliverPayloadDataParse {
    /// The embedded data (`offsetof(..., m_data)` upstream).
    fn deliver_payload_data_mut(&mut self) -> &mut DeliverPayloadData;
    /// `TheWeaponStore->findWeaponTemplate(name)` (`None` for `NULL`).
    fn find_weapon_template(&self, name: &[u8]) -> Option<WeaponTemplateRef>;
    /// `TheFXListStore->findFXList(name)`, as an FX list is kept in this translation: its name, `None` for `NULL`.
    fn find_fx_list(&self, name: &[u8]) -> FxListName;
}

impl DeliverPayloadData {
    /// `getFieldParse()`: the table of the data's fields, for a struct that embeds the data.
    pub fn field_parse<T: DeliverPayloadDataParse>() -> Vec<FieldParse<T>> {
        vec![
            FieldParse::new(
                "DeliveryDistance",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().dist_to_target = ini.parse_real()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "PreOpenDistance",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().pre_open_distance = ini.parse_real()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "MaxAttempts",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().max_attempts = ini.parse_int()?;
                    Ok(())
                },
                UserData::None,
            ),
            // drop information
            FieldParse::new(
                "DropDelay",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().drop_delay = ini.parse_duration_unsigned_int()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "DropOffset",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().drop_offset = ini.parse_coord3d()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "DropVariance",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().drop_variance = ini.parse_coord3d()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "InheritTransportVelocity",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().inherit_transport_velocity = ini.parse_bool()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "ExitPitchRate",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().exit_pitch_rate =
                        ini.parse_angular_velocity_real()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "ParachuteDirectly",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().is_parachute_directly = ini.parse_bool()?;
                    Ok(())
                },
                UserData::None,
            ),
            // visible payload information (the payload is shown visibly and created only when dropped)
            FieldParse::new(
                "VisibleItemsDroppedPerInterval",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut()
                        .visible_items_dropped_per_interval = ini.parse_int()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "VisibleDropBoneBaseName",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().visible_drop_bone_name =
                        ini.parse_ascii_string()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "VisibleSubObjectBaseName",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().visible_sub_object_name =
                        ini.parse_ascii_string()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "VisibleNumBones",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().visible_num_bones = ini.parse_int()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "VisiblePayloadTemplateName",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().visible_payload_template_name =
                        ini.parse_ascii_string()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "VisiblePayloadWeaponTemplate",
                parse_visible_payload_weapon_template::<T>,
                UserData::None,
            ),
            FieldParse::new(
                "SelfDestructObject",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().self_destruct_object = ini.parse_bool()?;
                    Ok(())
                },
                UserData::None,
            ),
            // weapon based payload
            FieldParse::new(
                "FireWeapon",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().fire_weapon = ini.parse_bool()?;
                    Ok(())
                },
                UserData::None,
            ),
            // an additional weapon slot to be fired while strafing
            FieldParse::new(
                "DiveStartDistance",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().dive_start_distance = ini.parse_real()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "DiveEndDistance",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().dive_end_distance = ini.parse_real()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "StrafingWeaponSlot",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().strafing_weapon_slot =
                        ini.parse_lookup_list(&WEAPON_SLOT_TYPE_NAMES_LOOKUP_LIST)?;
                    Ok(())
                },
                UserData::Lookup(&WEAPON_SLOT_TYPE_NAMES_LOOKUP_LIST),
            ),
            FieldParse::new("StrafeWeaponFX", parse_strafe_fx::<T>, UserData::None),
            FieldParse::new(
                "StrafeLength",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().strafe_length = ini.parse_real()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "DeliveryDecal",
                |ini, t: &mut T, _| {
                    RadiusDecalTemplate::parse_radius_decal_template(
                        ini,
                        &mut t.deliver_payload_data_mut().delivery_decal_template,
                    )
                },
                UserData::None,
            ),
            FieldParse::new(
                "DeliveryDecalRadius",
                |ini, t: &mut T, _| {
                    t.deliver_payload_data_mut().delivery_decal_radius = ini.parse_real()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::end(),
        ]
    }
}

/// `INI::parseWeaponTemplate` on `m_visiblePayloadWeaponTemplate`: the next token names the template; an unknown name is `NULL`.
fn parse_visible_payload_weapon_template<T: DeliverPayloadDataParse>(
    ini: &mut Ini<'_>,
    t: &mut T,
    _: &UserData,
) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    let found = t.find_weapon_template(&token);
    t.deliver_payload_data_mut().visible_payload_weapon_template = found;
    Ok(())
}

/// `INI::parseFXList` on `m_strafeFX`.
fn parse_strafe_fx<T: DeliverPayloadDataParse>(
    ini: &mut Ini<'_>,
    t: &mut T,
    _: &UserData,
) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    let found = t.find_fx_list(&token);
    t.deliver_payload_data_mut().strafe_fx = found;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};

    struct Target(DeliverPayloadData);
    impl DeliverPayloadDataParse for Target {
        fn deliver_payload_data_mut(&mut self) -> &mut DeliverPayloadData {
            &mut self.0
        }
        fn find_weapon_template(&self, name: &[u8]) -> Option<WeaponTemplateRef> {
            (name == b"W1").then_some(7)
        }
        fn find_fx_list(&self, name: &[u8]) -> FxListName {
            (!name.eq_ignore_ascii_case(b"None")).then(|| name.to_vec())
        }
    }

    fn block(ini: &mut Ini<'_>, t: &mut Target) -> IniResult<()> {
        let table = DeliverPayloadData::field_parse::<Target>();
        ini.init_from_ini(t, &table)
    }

    #[test]
    fn defaults_and_every_field_form() {
        let d = DeliverPayloadData::default();
        assert_eq!(
            (
                d.max_attempts,
                d.strafing_weapon_slot,
                d.delivery_decal_radius
            ),
            (1, -1, 0.0)
        );
        let text = b"D\n DeliveryDistance = 5.5\n MaxAttempts = 3\n DropOffset = X:1 Y:2 Z:3\n DropDelay = 1000\n StrafingWeaponSlot = SECONDARY\n \
                     VisiblePayloadWeaponTemplate = W1\n StrafeWeaponFX = FX_A\n DeliveryDecal\n Texture = T\n End\n DeliveryDecalRadius = 9\n \
                     VisibleDropBoneBaseName = \"Bone Name\"\n FireWeapon = Yes\nEnd\n";
        let blocks = [BlockParse {
            token: "D",
            parse: block,
        }];
        let mut t = Target(DeliverPayloadData::default());
        Ini::load(
            b"t.ini",
            text,
            IniLoadType::Overwrite,
            None,
            &blocks,
            &mut t,
        )
        .unwrap();
        let d = t.0;
        assert_eq!(
            (
                d.dist_to_target,
                d.max_attempts,
                d.drop_delay,
                d.strafing_weapon_slot
            ),
            (5.5, 3, 30, 1)
        );
        assert_eq!(
            (d.drop_offset.x, d.drop_offset.y, d.drop_offset.z),
            (1.0, 2.0, 3.0)
        );
        assert_eq!(d.visible_payload_weapon_template, Some(7));
        assert_eq!(d.strafe_fx.as_deref(), Some(&b"FX_A"[..]));
        assert_eq!(d.delivery_decal_template.name, b"T");
        assert_eq!(d.delivery_decal_radius, 9.0);
        assert!(d.fire_weapon);
        assert_eq!(d.visible_drop_bone_name, b"Bone Name");
    }
}
