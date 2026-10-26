// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/RadiusDecal.h   (class RadiusDecalTemplate)
//   GeneralsMD/Code/GameEngine/Source/GameClient/RadiusDecal.cpp  (RadiusDecalTemplate::RadiusDecalTemplate, ::parseRadiusDecalTemplate)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RadiusDecalTemplate`, wave 7 (`p1w10-sonnet`): the INI-parsed description of the circle a superweapon or an attack draws on the ground
//! (a texture, a style, the opacity throb, a colour).
//!
//! The data half of `RadiusDecal.h/.cpp` is here: the template, its constructor, and its `DeliveryDecal` block parser, which
//! `ObjectCreationList.cpp` (`AttackNugget`, `DeliverPayloadNugget`) and `DeliverPayloadData` embed. ~~`RadiusDecal` itself (the instance
//! that owns a client `Shadow`, `createRadiusDecal`, `update`, `xfer`) draws through the projected shadow manager, which is the client's,
//! and is not translated~~ **CORRECTED 2026-10-23 (batch R20)**: the logic half of `RadiusDecal` (its `m_empty`, which `RadiusDecalUpdate`
//! sleeps by, `clear`, `update`, `xferRadiusDecal` and the template's `createRadiusDecal`) is [`RadiusDecal`]; the drawing through the
//! projected shadow manager stays the client's. `xferRadiusDecalTemplate` is not translated: nothing in the logic xfers a template.
//!
//! Verified through `differential_object_creation_list.rs`, which parses the block with the real `RadiusDecalTemplate` constructor and
//! parser (verbatim text of `RadiusDecal.cpp`, cut out by `build.rs`) inside `Attack` and `DeliverPayload` blocks.

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;
use crate::common::ini::{FieldParse, Ini, IniResult, UserData};
use crate::common::thing_template::SHADOW_NAMES;
use crate::logic::world::game_logic::World;
use crate::logic::world::object_events::{GridDecalOp, ObjectClientEvent};

/// `SHADOW_ALPHA_DECAL` (`Shadow.h`): bit 5 of `ShadowType`, named `SHADOW_NAMES[5]`.
pub const SHADOW_ALPHA_DECAL: u32 = 0x20;

/// `LOGICFRAMES_PER_SECOND` as the `UnsignedInt` the template keeps.
const LOGICFRAMES_PER_SECOND_U: u32 = crate::common::game_common::LOGICFRAMES_PER_SECOND as u32;

/// `class RadiusDecalTemplate`.
#[derive(Clone, Debug, PartialEq)]
pub struct RadiusDecalTemplate {
    /// `m_name`: the texture; empty for "no decal".
    pub name: Vec<u8>,
    /// `m_shadowType` (a `ShadowType` bit set).
    pub shadow_type: u32,
    /// `m_minOpacity`.
    pub min_opacity: f32,
    /// `m_maxOpacity`.
    pub max_opacity: f32,
    /// `m_opacityThrobTime`, in logic frames.
    pub opacity_throb_time: u32,
    /// `m_color` (`Color`, an `Int`; 0 means the owning player's colour).
    pub color: i32,
    /// `m_onlyVisibleToOwningPlayer`.
    pub only_visible_to_owning_player: bool,
}

impl Default for RadiusDecalTemplate {
    /// `RadiusDecalTemplate()`.
    fn default() -> Self {
        RadiusDecalTemplate {
            name: Vec::new(),
            shadow_type: SHADOW_ALPHA_DECAL,
            min_opacity: 1.0,
            max_opacity: 1.0,
            opacity_throb_time: LOGICFRAMES_PER_SECOND_U,
            color: 0,
            only_visible_to_owning_player: true,
        }
    }
}

/// `dataFieldParse` of `parseRadiusDecalTemplate`.
static RADIUS_DECAL_FIELDS: [FieldParse<RadiusDecalTemplate>; 8] = [
    FieldParse::new("Texture", |ini, t, _| { t.name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("Style", |ini, t, _| ini.parse_bit_string32(&mut t.shadow_type, &SHADOW_NAMES), UserData::None),
    FieldParse::new("OpacityMin", |ini, t, _| { t.min_opacity = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("OpacityMax", |ini, t, _| { t.max_opacity = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("OpacityThrobTime", |ini, t, _| { t.opacity_throb_time = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("Color", |ini, t, _| { t.color = ini.parse_color_int()?; Ok(()) }, UserData::None),
    FieldParse::new("OnlyVisibleToOwningPlayer", |ini, t, _| { t.only_visible_to_owning_player = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl RadiusDecalTemplate {
    /// `valid()`: it has a texture.
    pub fn valid(&self) -> bool {
        !self.name.is_empty()
    }

    /// `parseRadiusDecalTemplate(ini, instance, store, userData)`: the block's field lines up to `End`, into `store` (the template that is
    /// `self`; an earlier value is kept for any field the block leaves out).
    pub fn parse_radius_decal_template(ini: &mut Ini<'_>, store: &mut RadiusDecalTemplate) -> IniResult<()> {
        ini.init_from_ini(store, &RADIUS_DECAL_FIELDS)
    }
}

/// `class RadiusDecal`, its logic half (batch R20, `RadiusDecalUpdate`): whether the decal is empty. The rest (the client's `Shadow`, the template
/// it was drawn from) is the client's: each call is an [`ObjectClientEvent::GridDecal`] of the owning object and the decal's index in its module.
/// "It is very important, for game/net sync reasons, to ensure that isEmpty() returns the same value, regardless of whether this decal will be
/// visible to the local player or not": it is set by the logic alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadiusDecal {
    /// `m_empty`.
    pub empty: bool,
}

impl Default for RadiusDecal {
    /// `RadiusDecal()`: empty.
    fn default() -> Self {
        RadiusDecal { empty: true }
    }
}

impl RadiusDecal {
    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.empty
    }

    /// `clear()`: empty, and the client lets its shadow go.
    pub fn clear(&mut self, world: &mut World, owner: ObjectId, index: u32) {
        self.empty = true;
        world.emit(ObjectClientEvent::GridDecal { object_id: owner, index, op: GridDecalOp::Clear });
    }

    /// `update()`: the client's opacity throb.
    pub fn update(&self, world: &mut World, owner: ObjectId, index: u32) {
        world.emit(ObjectClientEvent::GridDecal { object_id: owner, index, op: GridDecalOp::Update });
    }

    /// `setPosition(pos)` (batch R72, `SpectreGunshipUpdate`): the client moves its shadow, if it drew one.
    pub fn set_position(&self, world: &mut World, owner: ObjectId, index: u32, pos: Coord3D) {
        world.emit(ObjectClientEvent::GridDecal { object_id: owner, index, op: GridDecalOp::SetPosition(pos) });
    }

    /// `xferRadiusDecal(xfer)`: nothing saved ("@todo implement me"); a load clears the decal. The load has no world, so the client is not told
    /// (it rebuilds its drawing after a load anyway).
    pub fn xfer_radius_decal(&mut self, loading: bool) {
        if loading {
            self.empty = true;
        }
    }
}

impl RadiusDecalTemplate {
    /// `createRadiusDecal(pos, radius, owningPlayer, result)`: `result` is cleared, then not empty if there is an owning player, a texture
    /// and a positive radius ("it is now considered nonEmpty, regardless of the state of m_decal"). Whether the client draws it (only for the
    /// local player, its colour) is the client's, told by one [`GridDecalOp::Create`] (which stands for the whole call, the clear included).
    /// `decal` is the owning object and the decal's index in its module (the client event's address).
    pub fn create_radius_decal(&self, world: &mut World, decal: (ObjectId, u32), pos: Coord3D, radius: f32, owning_player: Option<i32>, result: &mut RadiusDecal) {
        let (owner, index) = decal;
        result.empty = true;
        world.emit(ObjectClientEvent::GridDecal {
            object_id: owner,
            index,
            op: GridDecalOp::Create { template: self.clone(), radius, position: pos, player: owning_player },
        });
        if owning_player.is_none() || self.name.is_empty() || radius <= 0.0 || crate::logic::modules::plant(1871) {
            return;
        }
        result.empty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};

    fn block(ini: &mut Ini<'_>, t: &mut RadiusDecalTemplate) -> IniResult<()> {
        RadiusDecalTemplate::parse_radius_decal_template(ini, t)
    }

    #[test]
    fn defaults_and_parse() {
        let d = RadiusDecalTemplate::default();
        assert!(!d.valid());
        assert_eq!((d.shadow_type, d.opacity_throb_time, d.color, d.only_visible_to_owning_player), (0x20, 30, 0, true));
        assert_eq!(SHADOW_NAMES[5], "SHADOW_ALPHA_DECAL");
        let text = b"Decal\n Texture = Ring\n Style = SHADOW_ADDITIVE_DECAL\n OpacityMin = 25%\n OpacityThrobTime = 2000\n Color = R:1 G:2 B:3\n OnlyVisibleToOwningPlayer = No\nEnd\n";
        let blocks = [BlockParse { token: "Decal", parse: block }];
        let mut t = RadiusDecalTemplate::default();
        Ini::load(b"t.ini", text, IniLoadType::Overwrite, None, &blocks, &mut t).unwrap();
        assert_eq!(t.name, b"Ring");
        assert_eq!(t.shadow_type, 0x40);
        assert_eq!(t.min_opacity, 0.25);
        assert_eq!(t.opacity_throb_time, 60);
        assert!(!t.only_visible_to_owning_player);
        assert_eq!(t.color, crate::common::ini::game_make_color(1, 2, 3, 255));
        assert!(t.valid());
    }
}
