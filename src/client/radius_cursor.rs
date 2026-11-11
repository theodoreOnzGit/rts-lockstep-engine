// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/InGameUI.h (RadiusCursorType, TheRadiusCursorNames, m_radiusCursors,
//     m_curRadiusCursor, m_curRcType)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (TheInGameUIFieldParseTable, INI::parseInGameUIDefinition,
//     InGameUI::init's load of InGameUI.ini, setRadiusCursor, handleRadiusCursor)
//   GeneralsMD/Code/GameEngine/Source/GameClient/RadiusDecal.cpp (RadiusDecalTemplate::createRadiusDecal's drawing,
//     RadiusDecal::RadiusDecal, clear, update, setOpacity, setPosition; #130: ~RadiusDecal, and the same calls on the
//     logic's own decals, LogicRadiusDecals)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The in-game UI's radius cursor (GitHub #124): the circle a command with a radius (an attack's damage area, a guard
//! area, a special power's reach) shows on the ground under the cursor while it waits for a target.
//!
//! # As upstream reads it
//!
//! **The templates.** `InGameUI::init` loads `Data\INI\InGameUI.ini` (`INI_LOAD_OVERWRITE`); its `InGameUI` block is read
//! by `TheInGameUIFieldParseTable`, whose `...RadiusCursor` fields are `RadiusDecalTemplate` blocks
//! (`parseRadiusDecalTemplate`: `Texture`, `Style`, `OpacityMin`, `OpacityMax`, `OpacityThrobTime`, `Color`,
//! `OnlyVisibleToOwningPlayer`) into `m_radiusCursors[type]`. There is no field for `RADIUSCURSOR_NONE`.
//!
//! **`setRadiusCursor(type, specialPower, weaponSlot)`**: nothing if `type` is the current one. Otherwise the current
//! decal is cleared and the type is none; for none, that is all. The object it is for is, with a pending
//! `GUI_COMMAND_SPECIAL_POWER_FROM_SHORTCUT`, the local player's most ready shortcut special power of the power's type
//! (none without a power), else the first selected drawable's object (nothing with no selection); without an object, or
//! without its controlling player, nothing. The radius: the weapon in the slot's primary damage radius (attack damage
//! area), its scatter radius plus its scatter target scalar (scatter area), its continue attack range (continue area,
//! clear mines), `AIGuardMachine::getStdGuardRange` (guard area), the power's `RadiusCursorRadius` (all the others), 0
//! without a weapon or a power. A radius not above 0 makes nothing. Else the type's template makes the decal at
//! `(0, 0, 0)` for the controlling player, the type becomes current, and `handleRadiusCursor` places it.
//!
//! **`createRadiusDecal(pos, radius, owningPlayer, result)`**: `result` cleared; without a texture or with a radius not
//! above 0, nothing more; else the decal is not empty, and, when the template is visible to everyone or the player is
//! the local one, a projected decal (`addDecal`: the texture, the style, `2·radius` square, world aligned, not updated)
//! turned to 0, coloured the template's colour (the player's for colour 0) and placed at `pos`.
//!
//! **`handleRadiusCursor()`** (`preDraw`, every frame, and after `setRadiusCursor` made a decal): for a decal that is
//! not empty, the point under the mouse (the radar's world point when the radar is on, forced or not hidden with the
//! local player having a radar, and the mouse is over it; else the terrain under the mouse); then while
//! `m_doubleClickAttackMove` is on and the guard hint's timer is above 0, the decal's opacity is `timer · 0.1` and it
//! stays at the stashed position (`triggerDoubleClickAttackMoveGuardHint`'s terrain point under the mouse); else it is
//! at the mouse's point and `update`d.
//!
//! **`RadiusDecal::update()`**: with a projected decal, its opacity throbs: `θ = 2π · (frame % throb) / throb`,
//! `opacity = REAL_TO_INT((min + ½(sin θ + 1)(max − min)) · 255)`, or 0 while scripts hide the icon UI
//! (`getDrawIconUI`). **`setOpacity(o)`**: `REAL_TO_INT(255 · o)`.
//!
//! # Where this differs from upstream
//!
//! - **The projected decal** (`TheProjectedShadowManager->addDecal`'s `Shadow`) is [`DecalShadow`], what the client draws
//!   (`client/`'s terrain overlay); `addDecal` never fails here.
//! - The calls are made once a frame by the owner of the mouse, in the order the InGameUI queued them
//!   (`crate::client::in_game_ui_cursor::UiMouseCall::RadiusCursor`), not inside `setGUICommand` and the hints: the
//!   selection and the pending command are read when the queue is played, the same frame.
//! - The rest of `InGameUI.ini` is read into [`InGameUiIniSettings`] (so the file reads as upstream reads it); the
//!   constructor's values of those fields are not translated (zero here), nothing reads them yet.
//! - The questions on the World are [`RadiusCursorHost`]'s.
//! - **(#130)** The logic's own decals (delivery decals, the shroud's grid, the gunship's reticles) are
//!   [`LogicRadiusDecals`]: the same `RadiusDecal` calls, made from the logic's `GridDecal` events.

use crate::common::base_type::{Coord2D, Coord3D, ICoord2D, RGBAColorInt};
use crate::common::game_type::ObjectId;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::common::special_power_type::SpecialPowerType;
use crate::logic::modules::plant;
use crate::logic::radius_decal::RadiusDecalTemplate;

/// `enum RadiusCursorType`.
pub mod radius_cursor_type {
    pub const NONE: i32 = 0;
    pub const ATTACK_DAMAGE_AREA: i32 = 1;
    pub const ATTACK_SCATTER_AREA: i32 = 2;
    pub const ATTACK_CONTINUE_AREA: i32 = 3;
    pub const GUARD_AREA: i32 = 4;
    pub const EMERGENCY_REPAIR: i32 = 5;
    pub const FRIENDLY_SPECIALPOWER: i32 = 6;
    pub const OFFENSIVE_SPECIALPOWER: i32 = 7;
    pub const SUPERWEAPON_SCATTER_AREA: i32 = 8;
    pub const PARTICLECANNON: i32 = 9;
    pub const A10STRIKE: i32 = 10;
    pub const CARPETBOMB: i32 = 11;
    pub const DAISYCUTTER: i32 = 12;
    pub const PARADROP: i32 = 13;
    pub const SPYSATELLITE: i32 = 14;
    pub const SPECTREGUNSHIP: i32 = 15;
    pub const HELIX_NAPALM_BOMB: i32 = 16;
    pub const NUCLEARMISSILE: i32 = 17;
    pub const EMPPULSE: i32 = 18;
    pub const ARTILLERYBARRAGE: i32 = 19;
    pub const NAPALMSTRIKE: i32 = 20;
    pub const CLUSTERMINES: i32 = 21;
    pub const SCUDSTORM: i32 = 22;
    pub const ANTHRAXBOMB: i32 = 23;
    pub const AMBUSH: i32 = 24;
    pub const RADAR: i32 = 25;
    pub const SPYDRONE: i32 = 26;
    pub const FRENZY: i32 = 27;
    pub const CLEARMINES: i32 = 28;
    pub const AMBULANCE: i32 = 29;
    pub const COUNT: i32 = 30;
}

/// `GUI_COMMAND_SPECIAL_POWER_FROM_SHORTCUT`.
const GUI_COMMAND_SPECIAL_POWER_FROM_SHORTCUT: i32 = crate::client::control_bar::gui::SPECIAL_POWER_FROM_SHORTCUT;

/// The fields of `TheInGameUIFieldParseTable` other than the radius cursors (named after the members).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InGameUiIniSettings {
    /// `m_messageColor1`.
    pub message_color1: i32,
    /// `m_messageColor2`.
    pub message_color2: i32,
    /// `m_messagePosition`.
    pub message_position: ICoord2D,
    /// `m_messageFont`.
    pub message_font: Vec<u8>,
    /// `m_messagePointSize`.
    pub message_point_size: i32,
    /// `m_messageBold`.
    pub message_bold: bool,
    /// `m_messageDelayMS`.
    pub message_delay_ms: i32,
    /// `m_militaryCaptionColor`.
    pub military_caption_color: RGBAColorInt,
    /// `m_militaryCaptionPosition`.
    pub military_caption_position: ICoord2D,
    /// `m_militaryCaptionTitleFont`.
    pub military_caption_title_font: Vec<u8>,
    /// `m_militaryCaptionTitlePointSize`.
    pub military_caption_title_point_size: i32,
    /// `m_militaryCaptionTitleBold`.
    pub military_caption_title_bold: bool,
    /// `m_militaryCaptionFont`.
    pub military_caption_font: Vec<u8>,
    /// `m_militaryCaptionPointSize`.
    pub military_caption_point_size: i32,
    /// `m_militaryCaptionBold`.
    pub military_caption_bold: bool,
    /// `m_militaryCaptionRandomizeTyping`.
    pub military_caption_randomize_typing: bool,
    /// `m_militaryCaptionSpeed`.
    pub military_caption_speed: i32,
    /// `m_superweaponPosition`.
    pub superweapon_position: Coord2D,
    /// `m_superweaponFlashDuration`.
    pub superweapon_flash_duration: f32,
    /// `m_superweaponFlashColor`.
    pub superweapon_flash_color: i32,
    /// `m_superweaponNormalFont`.
    pub superweapon_normal_font: Vec<u8>,
    /// `m_superweaponNormalPointSize`.
    pub superweapon_normal_point_size: i32,
    /// `m_superweaponNormalBold`.
    pub superweapon_normal_bold: bool,
    /// `m_superweaponReadyFont`.
    pub superweapon_ready_font: Vec<u8>,
    /// `m_superweaponReadyPointSize`.
    pub superweapon_ready_point_size: i32,
    /// `m_superweaponReadyBold`.
    pub superweapon_ready_bold: bool,
    /// `m_namedTimerPosition`.
    pub named_timer_position: Coord2D,
    /// `m_namedTimerFlashDuration`.
    pub named_timer_flash_duration: f32,
    /// `m_namedTimerFlashColor`.
    pub named_timer_flash_color: i32,
    /// `m_namedTimerNormalFont`.
    pub named_timer_normal_font: Vec<u8>,
    /// `m_namedTimerNormalPointSize`.
    pub named_timer_normal_point_size: i32,
    /// `m_namedTimerNormalBold`.
    pub named_timer_normal_bold: bool,
    /// `m_namedTimerNormalColor`.
    pub named_timer_normal_color: i32,
    /// `m_namedTimerReadyFont`.
    pub named_timer_ready_font: Vec<u8>,
    /// `m_namedTimerReadyPointSize`.
    pub named_timer_ready_point_size: i32,
    /// `m_namedTimerReadyBold`.
    pub named_timer_ready_bold: bool,
    /// `m_namedTimerReadyColor`.
    pub named_timer_ready_color: i32,
    /// `m_floatingTextTimeOut`.
    pub floating_text_time_out: u32,
    /// `m_floatingTextMoveUpSpeed`.
    pub floating_text_move_up_speed: f32,
    /// `m_floatingTextMoveVanishRate`.
    pub floating_text_move_vanish_rate: f32,
    /// `m_popupMessageColor`.
    pub popup_message_color: i32,
    /// `m_drawableCaptionFont`.
    pub drawable_caption_font: Vec<u8>,
    /// `m_drawableCaptionPointSize`.
    pub drawable_caption_point_size: i32,
    /// `m_drawableCaptionBold`.
    pub drawable_caption_bold: bool,
    /// `m_drawableCaptionColor`.
    pub drawable_caption_color: i32,
    /// `m_drawRMBScrollAnchor`.
    pub draw_rmb_scroll_anchor: bool,
    /// `m_moveRMBScrollAnchor`.
    pub move_rmb_scroll_anchor: bool,
}

/// The projected decal (`Shadow` made by `TheProjectedShadowManager->addDecal`): what the client draws.
#[derive(Clone, Debug, PartialEq)]
pub struct DecalShadow {
    /// `decalInfo.m_ShadowName`: the texture.
    pub texture: Vec<u8>,
    /// `decalInfo.m_type` (a `ShadowType` bit set).
    pub shadow_type: u32,
    /// `m_sizeX`, `m_sizeY`: world units.
    pub size: (f32, f32),
    /// `setAngle`.
    pub angle: f32,
    /// `setColor`.
    pub color: i32,
    /// `setPosition`: its centre in the world.
    pub position: Coord3D,
    /// `setOpacity` (0..255; `None`: never set, the shadow's own, opaque).
    pub opacity: Option<i32>,
}

/// `class RadiusDecal` as the client keeps it: the template it was made from (its type), the projected decal, empty or not.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientRadiusDecal {
    /// `m_template` (the radius cursor type whose template it is; `None`: `NULL`).
    pub template: Option<i32>,
    /// `m_decal`.
    pub decal: Option<DecalShadow>,
    /// `m_empty`.
    pub empty: bool,
}

impl Default for ClientRadiusDecal {
    /// `RadiusDecal()`.
    fn default() -> Self {
        ClientRadiusDecal { template: None, decal: None, empty: true }
    }
}

impl ClientRadiusDecal {
    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.empty
    }

    /// `clear()`: the decal released.
    pub fn clear(&mut self) {
        self.template = None;
        self.decal = None;
        self.empty = true;
    }

    /// `update()` at logic frame `now`; `draw_icon_ui` is `TheGameLogic->getDrawIconUI()`.
    pub fn update(&mut self, templates: &[RadiusDecalTemplate], now: u32, draw_icon_ui: bool) {
        let (Some(decal), Some(t)) = (self.decal.as_mut(), self.template) else { return };
        let t = &templates[t as usize];
        let throb = if plant(9175) { t.opacity_throb_time.wrapping_add(1) } else { t.opacity_throb_time };
        // `(2*PI) * (Real)(now % m_opacityThrobTime) / (Real)m_opacityThrobTime` (a throb of 0 divides by zero upstream,
        // a crash; here the phase is 0)
        let phase = now.checked_rem(throb).unwrap_or(0);
        let theta = (2.0 * core::f32::consts::PI) * phase as f32 / throb as f32;
        let percent = 0.5 * (petir::Real::sin(theta) + 1.0);
        let opac = if draw_icon_ui && !plant(9176) {
            crate::common::base_type::real_to_int((t.min_opacity + percent * (t.max_opacity - t.min_opacity)) * 255.0)
        } else {
            // Scripts turned this off, so don't show them!
            0
        };
        decal.opacity = Some(opac);
    }

    /// `setOpacity(o)`.
    pub fn set_opacity(&mut self, o: f32) {
        if let Some(d) = self.decal.as_mut() {
            d.opacity = Some(crate::common::base_type::real_to_int(255.0 * o));
        }
    }

    /// `setPosition(pos)`: the decal's centre in the world.
    pub fn set_position(&mut self, pos: Coord3D) {
        if let Some(d) = self.decal.as_mut() {
            d.position = pos;
        }
    }
}

/// `RadiusDecalTemplate::createRadiusDecal(pos, radius, owningPlayer, result)` with the client's drawing: `kind` is the
/// template's radius cursor type, `owning` the owning player's index and colour (`None` is `NULL`: a `DEBUG_CRASH`, then
/// nothing), `local_player` the local player's index.
pub fn create_radius_decal(tmpl: &RadiusDecalTemplate, kind: i32, pos: Coord3D, radius: f32, owning: Option<(i32, i32)>, local_player: i32, result: &mut ClientRadiusDecal) {
    result.clear();
    let Some((index, color)) = owning else { return };
    if tmpl.name.is_empty() || radius <= 0.0 {
        return;
    }
    // it is now considered nonEmpty, regardless of the state of m_decal, etc
    result.empty = false;
    if !tmpl.only_visible_to_owning_player || index == local_player || plant(9177) {
        result.decal = Some(DecalShadow {
            texture: tmpl.name.clone(),
            shadow_type: tmpl.shadow_type,
            size: (radius * 2.0, radius * 2.0),
            angle: 0.0,
            color: if tmpl.color == 0 { color } else { tmpl.color },
            position: pos,
            opacity: None,
        });
        result.template = Some(kind);
    }
}

/// What `setRadiusCursor` and `handleRadiusCursor` ask of the game.
pub trait RadiusCursorHost {
    /// `draw->getObject()` of a selected drawable.
    fn radius_drawable_object(&self, draw: crate::client::in_game_ui::DrawableId) -> Option<ObjectId>;
    /// `ThePlayerList->getLocalPlayer()->findMostReadyShortcutSpecialPowerOfType(type)`.
    fn radius_most_ready_shortcut_special_power(&mut self, sp: SpecialPowerType) -> Option<ObjectId>;
    /// The special power template's `getSpecialPowerType()` and `getRadiusCursorRadius()`, by its id.
    fn special_power_radius(&self, sp: u32) -> Option<(SpecialPowerType, f32)>;
    /// `obj->getControllingPlayer()`: its index and `getPlayerColor()`.
    fn radius_controlling_player(&self, obj: ObjectId) -> Option<(i32, i32)>;
    /// `obj->getWeaponInWeaponSlot(slot)`'s `getPrimaryDamageRadius(obj)`, `getScatterRadius() + getScatterTargetScalar()`
    /// and `getContinueAttackRange()` (`None`: no weapon in the slot).
    fn weapon_primary_damage_radius(&mut self, obj: ObjectId, slot: i32) -> Option<f32>;
    fn weapon_scatter_radius(&mut self, obj: ObjectId, slot: i32) -> Option<f32>;
    fn weapon_continue_attack_range(&mut self, obj: ObjectId, slot: i32) -> Option<f32>;
    /// `AIGuardMachine::getStdGuardRange(obj)`.
    fn std_guard_range(&mut self, obj: ObjectId) -> f32;
    /// `ThePlayerList->getLocalPlayer()->getPlayerIndex()`.
    fn radius_local_player(&self) -> i32;
    /// `TheMouse->getMouseStatus()->pos`.
    fn radius_mouse_pos(&self) -> ICoord2D;
    /// `TheRadar->isRadarForced() || (!TheRadar->isRadarHidden() && local player && hasRadar())`.
    fn radius_radar_on(&self) -> bool;
    /// `TheRadar->screenPixelToWorld(pixel)` (`None`: not on the radar).
    fn radar_screen_pixel_to_world(&self, pixel: ICoord2D) -> Option<Coord3D>;
    /// `TheTacticalView->screenToTerrain(pixel)`.
    fn radius_screen_to_terrain(&mut self, pixel: ICoord2D) -> Coord3D;
    /// `TheGlobalData->m_doubleClickAttackMove`.
    fn radius_double_click_attack_move(&self) -> bool;
    /// `TheGameLogic->getFrame()`, `getDrawIconUI()`.
    fn radius_logic_frame(&self) -> u32;
    fn draw_icon_ui(&self) -> bool;
}

/// What `setRadiusCursor` and `handleRadiusCursor` read of the InGameUI's other members.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RadiusUiState {
    /// `m_pendingGUICommand && m_pendingGUICommand->getCommandType() == GUI_COMMAND_SPECIAL_POWER_FROM_SHORTCUT`.
    pub shortcut_pending: bool,
    /// `getSelectCount()`, `getFirstSelectedDrawable()`.
    pub select_count: i32,
    pub first_selected: Option<crate::client::in_game_ui::DrawableId>,
    /// `m_duringDoubleClickAttackMoveGuardHintTimer`, `m_duringDoubleClickAttackMoveGuardHintStashedPosition`.
    pub guard_hint_timer: i32,
    pub guard_hint_stash: Coord3D,
}

impl RadiusUiState {
    /// The members as `ui` holds them.
    pub fn of(ui: &crate::client::in_game_ui::InGameUiSelection) -> Self {
        RadiusUiState {
            shortcut_pending: ui.gui_command().is_some_and(|c| c.command_type == GUI_COMMAND_SPECIAL_POWER_FROM_SHORTCUT),
            select_count: ui.select_count(),
            first_selected: ui.first_selected_drawable(),
            guard_hint_timer: ui.double_click_attack_move_guard_hint_timer(),
            guard_hint_stash: ui.double_click_attack_move_guard_hint_stashed_position(),
        }
    }
}

/// The InGameUI's radius cursor members: `m_radiusCursors`, `m_curRadiusCursor`, `m_curRcType`, with the rest of
/// `InGameUI.ini`'s fields.
#[derive(Clone, Debug, PartialEq)]
pub struct InGameUiRadiusCursor {
    /// `m_radiusCursors[RADIUSCURSOR_COUNT]`.
    pub radius_cursors: Vec<RadiusDecalTemplate>,
    /// `m_curRadiusCursor`.
    pub cur_radius_cursor: ClientRadiusDecal,
    /// `m_curRcType`.
    pub cur_rc_type: i32,
    /// The other fields of `InGameUI.ini`.
    pub settings: InGameUiIniSettings,
}

impl Default for InGameUiRadiusCursor {
    /// `InGameUI()`: the templates' defaults, no decal, `RADIUSCURSOR_NONE`.
    fn default() -> Self {
        InGameUiRadiusCursor {
            radius_cursors: vec![RadiusDecalTemplate::default(); radius_cursor_type::COUNT as usize],
            cur_radius_cursor: ClientRadiusDecal::default(),
            cur_rc_type: radius_cursor_type::NONE,
            settings: InGameUiIniSettings::default(),
        }
    }
}

/// `TheInGameUIFieldParseTable` (`MilitaryCaptionPosition` twice, as upstream lists it).
const IN_GAME_UI_FIELDS: &[FieldParse<InGameUiRadiusCursor>] = &[
    FieldParse::new("MessageColor1", |ini, u, _| ini.parse_color_int().map(|v| u.settings.message_color1 = v), UserData::None),
    FieldParse::new("MessageColor2", |ini, u, _| ini.parse_color_int().map(|v| u.settings.message_color2 = v), UserData::None),
    FieldParse::new("MessagePosition", |ini, u, _| ini.parse_icoord2d().map(|v| u.settings.message_position = v), UserData::None),
    FieldParse::new("MessageFont", |ini, u, _| ini.parse_ascii_string().map(|v| u.settings.message_font = v), UserData::None),
    FieldParse::new("MessagePointSize", |ini, u, _| ini.parse_int().map(|v| u.settings.message_point_size = v), UserData::None),
    FieldParse::new("MessageBold", |ini, u, _| ini.parse_bool().map(|v| u.settings.message_bold = v), UserData::None),
    FieldParse::new("MessageDelayMS", |ini, u, _| ini.parse_int().map(|v| u.settings.message_delay_ms = v), UserData::None),
    FieldParse::new("MilitaryCaptionColor", |ini, u, _| ini.parse_rgba_color_int().map(|v| u.settings.military_caption_color = v), UserData::None),
    FieldParse::new("MilitaryCaptionPosition", |ini, u, _| ini.parse_icoord2d().map(|v| u.settings.military_caption_position = v), UserData::None),
    FieldParse::new("MilitaryCaptionTitleFont", |ini, u, _| ini.parse_ascii_string().map(|v| u.settings.military_caption_title_font = v), UserData::None),
    FieldParse::new("MilitaryCaptionTitlePointSize", |ini, u, _| ini.parse_int().map(|v| u.settings.military_caption_title_point_size = v), UserData::None),
    FieldParse::new("MilitaryCaptionTitleBold", |ini, u, _| ini.parse_bool().map(|v| u.settings.military_caption_title_bold = v), UserData::None),
    FieldParse::new("MilitaryCaptionFont", |ini, u, _| ini.parse_ascii_string().map(|v| u.settings.military_caption_font = v), UserData::None),
    FieldParse::new("MilitaryCaptionPointSize", |ini, u, _| ini.parse_int().map(|v| u.settings.military_caption_point_size = v), UserData::None),
    FieldParse::new("MilitaryCaptionBold", |ini, u, _| ini.parse_bool().map(|v| u.settings.military_caption_bold = v), UserData::None),
    FieldParse::new("MilitaryCaptionRandomizeTyping", |ini, u, _| ini.parse_bool().map(|v| u.settings.military_caption_randomize_typing = v), UserData::None),
    FieldParse::new("MilitaryCaptionSpeed", |ini, u, _| ini.parse_int().map(|v| u.settings.military_caption_speed = v), UserData::None),
    FieldParse::new("MilitaryCaptionPosition", |ini, u, _| ini.parse_icoord2d().map(|v| u.settings.military_caption_position = v), UserData::None),
    FieldParse::new("SuperweaponCountdownPosition", |ini, u, _| ini.parse_coord2d().map(|v| u.settings.superweapon_position = v), UserData::None),
    FieldParse::new("SuperweaponCountdownFlashDuration", |ini, u, _| ini.parse_duration_real().map(|v| u.settings.superweapon_flash_duration = v), UserData::None),
    FieldParse::new("SuperweaponCountdownFlashColor", |ini, u, _| ini.parse_color_int().map(|v| u.settings.superweapon_flash_color = v), UserData::None),
    FieldParse::new("SuperweaponCountdownNormalFont", |ini, u, _| ini.parse_ascii_string().map(|v| u.settings.superweapon_normal_font = v), UserData::None),
    FieldParse::new("SuperweaponCountdownNormalPointSize", |ini, u, _| ini.parse_int().map(|v| u.settings.superweapon_normal_point_size = v), UserData::None),
    FieldParse::new("SuperweaponCountdownNormalBold", |ini, u, _| ini.parse_bool().map(|v| u.settings.superweapon_normal_bold = v), UserData::None),
    FieldParse::new("SuperweaponCountdownReadyFont", |ini, u, _| ini.parse_ascii_string().map(|v| u.settings.superweapon_ready_font = v), UserData::None),
    FieldParse::new("SuperweaponCountdownReadyPointSize", |ini, u, _| ini.parse_int().map(|v| u.settings.superweapon_ready_point_size = v), UserData::None),
    FieldParse::new("SuperweaponCountdownReadyBold", |ini, u, _| ini.parse_bool().map(|v| u.settings.superweapon_ready_bold = v), UserData::None),
    FieldParse::new("NamedTimerCountdownPosition", |ini, u, _| ini.parse_coord2d().map(|v| u.settings.named_timer_position = v), UserData::None),
    FieldParse::new("NamedTimerCountdownFlashDuration", |ini, u, _| ini.parse_duration_real().map(|v| u.settings.named_timer_flash_duration = v), UserData::None),
    FieldParse::new("NamedTimerCountdownFlashColor", |ini, u, _| ini.parse_color_int().map(|v| u.settings.named_timer_flash_color = v), UserData::None),
    FieldParse::new("NamedTimerCountdownNormalFont", |ini, u, _| ini.parse_ascii_string().map(|v| u.settings.named_timer_normal_font = v), UserData::None),
    FieldParse::new("NamedTimerCountdownNormalPointSize", |ini, u, _| ini.parse_int().map(|v| u.settings.named_timer_normal_point_size = v), UserData::None),
    FieldParse::new("NamedTimerCountdownNormalBold", |ini, u, _| ini.parse_bool().map(|v| u.settings.named_timer_normal_bold = v), UserData::None),
    FieldParse::new("NamedTimerCountdownNormalColor", |ini, u, _| ini.parse_color_int().map(|v| u.settings.named_timer_normal_color = v), UserData::None),
    FieldParse::new("NamedTimerCountdownReadyFont", |ini, u, _| ini.parse_ascii_string().map(|v| u.settings.named_timer_ready_font = v), UserData::None),
    FieldParse::new("NamedTimerCountdownReadyPointSize", |ini, u, _| ini.parse_int().map(|v| u.settings.named_timer_ready_point_size = v), UserData::None),
    FieldParse::new("NamedTimerCountdownReadyBold", |ini, u, _| ini.parse_bool().map(|v| u.settings.named_timer_ready_bold = v), UserData::None),
    FieldParse::new("NamedTimerCountdownReadyColor", |ini, u, _| ini.parse_color_int().map(|v| u.settings.named_timer_ready_color = v), UserData::None),
    FieldParse::new("FloatingTextTimeOut", |ini, u, _| ini.parse_duration_unsigned_int().map(|v| u.settings.floating_text_time_out = v), UserData::None),
    FieldParse::new("FloatingTextMoveUpSpeed", |ini, u, _| ini.parse_velocity_real().map(|v| u.settings.floating_text_move_up_speed = v), UserData::None),
    FieldParse::new("FloatingTextVanishRate", |ini, u, _| ini.parse_velocity_real().map(|v| u.settings.floating_text_move_vanish_rate = v), UserData::None),
    FieldParse::new("PopupMessageColor", |ini, u, _| ini.parse_color_int().map(|v| u.settings.popup_message_color = v), UserData::None),
    FieldParse::new("DrawableCaptionFont", |ini, u, _| ini.parse_ascii_string().map(|v| u.settings.drawable_caption_font = v), UserData::None),
    FieldParse::new("DrawableCaptionPointSize", |ini, u, _| ini.parse_int().map(|v| u.settings.drawable_caption_point_size = v), UserData::None),
    FieldParse::new("DrawableCaptionBold", |ini, u, _| ini.parse_bool().map(|v| u.settings.drawable_caption_bold = v), UserData::None),
    FieldParse::new("DrawableCaptionColor", |ini, u, _| ini.parse_color_int().map(|v| u.settings.drawable_caption_color = v), UserData::None),
    FieldParse::new("DrawRMBScrollAnchor", |ini, u, _| ini.parse_bool().map(|v| u.settings.draw_rmb_scroll_anchor = v), UserData::None),
    FieldParse::new("MoveRMBScrollAnchor", |ini, u, _| ini.parse_bool().map(|v| u.settings.move_rmb_scroll_anchor = v), UserData::None),
    FieldParse::new("AttackDamageAreaRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::ATTACK_DAMAGE_AREA as usize]), UserData::None),
    FieldParse::new("AttackScatterAreaRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::ATTACK_SCATTER_AREA as usize]), UserData::None),
    FieldParse::new("AttackContinueAreaRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::ATTACK_CONTINUE_AREA as usize]), UserData::None),
    FieldParse::new("FriendlySpecialPowerRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::FRIENDLY_SPECIALPOWER as usize]), UserData::None),
    FieldParse::new("OffensiveSpecialPowerRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::OFFENSIVE_SPECIALPOWER as usize]), UserData::None),
    FieldParse::new("SuperweaponScatterAreaRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::SUPERWEAPON_SCATTER_AREA as usize]), UserData::None),
    FieldParse::new("GuardAreaRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::GUARD_AREA as usize]), UserData::None),
    FieldParse::new("EmergencyRepairRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::EMERGENCY_REPAIR as usize]), UserData::None),
    FieldParse::new("ParticleCannonRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::PARTICLECANNON as usize]), UserData::None),
    FieldParse::new("A10StrikeRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::A10STRIKE as usize]), UserData::None),
    FieldParse::new("CarpetBombRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::CARPETBOMB as usize]), UserData::None),
    FieldParse::new("DaisyCutterRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::DAISYCUTTER as usize]), UserData::None),
    FieldParse::new("ParadropRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::PARADROP as usize]), UserData::None),
    FieldParse::new("SpySatelliteRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::SPYSATELLITE as usize]), UserData::None),
    FieldParse::new("SpectreGunshipRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::SPECTREGUNSHIP as usize]), UserData::None),
    FieldParse::new("HelixNapalmBombRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::HELIX_NAPALM_BOMB as usize]), UserData::None),
    FieldParse::new("NuclearMissileRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::NUCLEARMISSILE as usize]), UserData::None),
    FieldParse::new("EMPPulseRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::EMPPULSE as usize]), UserData::None),
    FieldParse::new("ArtilleryRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::ARTILLERYBARRAGE as usize]), UserData::None),
    FieldParse::new("FrenzyRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::FRENZY as usize]), UserData::None),
    FieldParse::new("NapalmStrikeRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::NAPALMSTRIKE as usize]), UserData::None),
    FieldParse::new("ClusterMinesRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::CLUSTERMINES as usize]), UserData::None),
    FieldParse::new("ScudStormRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::SCUDSTORM as usize]), UserData::None),
    FieldParse::new("AnthraxBombRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::ANTHRAXBOMB as usize]), UserData::None),
    FieldParse::new("AmbushRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::AMBUSH as usize]), UserData::None),
    FieldParse::new("RadarRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::RADAR as usize]), UserData::None),
    FieldParse::new("SpyDroneRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::SPYDRONE as usize]), UserData::None),
    FieldParse::new("ClearMinesRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::CLEARMINES as usize]), UserData::None),
    FieldParse::new("AmbulanceRadiusCursor", |ini, u, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut u.radius_cursors[radius_cursor_type::AMBULANCE as usize]), UserData::None),
    FieldParse::end(),
];

/// `INI::parseInGameUIDefinition`.
fn parse_in_game_ui_definition(ini: &mut Ini<'_>, u: &mut InGameUiRadiusCursor) -> IniResult<()> {
    ini.init_from_ini(u, IN_GAME_UI_FIELDS)
}

/// The block `InGameUI.ini` holds (upstream's `theTypeTable` has it among the others).
pub const IN_GAME_UI_BLOCKS: &[BlockParse<InGameUiRadiusCursor>] = &[BlockParse { token: "InGameUI", parse: parse_in_game_ui_definition }];

impl InGameUiRadiusCursor {
    /// `InGameUI::init`'s `INI::load("Data\\INI\\InGameUI.ini", INI_LOAD_OVERWRITE)` of the file's bytes.
    pub fn parse_ini(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, IN_GAME_UI_BLOCKS, self)
    }

    /// `setRadiusCursor(cursorType, specPowTempl, weaponSlot)` (the power by its template's id), with the InGameUI's
    /// pending command and selection, and the guard hint's timer and stashed position for `handleRadiusCursor`.
    pub fn set_radius_cursor<H: RadiusCursorHost>(&mut self, ui: &RadiusUiState, cursor_type: i32, sp: Option<u32>, weapon_slot: i32, h: &mut H) {
        use radius_cursor_type as rc;
        if cursor_type == self.cur_rc_type && !plant(9178) {
            return;
        }
        self.cur_radius_cursor.clear();
        self.cur_rc_type = rc::NONE;
        if cursor_type == rc::NONE {
            return;
        }
        let spt = sp.and_then(|id| h.special_power_radius(id));
        let obj = if ui.shortcut_pending {
            // (`specPowTempl != NULL`; a template the store does not know is taken as none)
            match spt {
                Some((t, _)) => h.radius_most_ready_shortcut_special_power(t),
                None => None,
            }
        } else {
            if ui.select_count == 0 {
                return;
            }
            let Some(draw) = ui.first_selected else { return };
            h.radius_drawable_object(draw)
        };
        let Some(obj) = obj else { return };
        let Some(controller) = h.radius_controlling_player(obj) else { return };
        let radius = match cursor_type {
            rc::ATTACK_DAMAGE_AREA => h.weapon_primary_damage_radius(obj, weapon_slot).unwrap_or(0.0),
            rc::ATTACK_SCATTER_AREA => h.weapon_scatter_radius(obj, weapon_slot).unwrap_or(0.0),
            rc::ATTACK_CONTINUE_AREA | rc::CLEARMINES => h.weapon_continue_attack_range(obj, weapon_slot).unwrap_or(0.0),
            rc::GUARD_AREA => {
                if plant(9179) {
                    spt.map_or(0.0, |(_, r)| r)
                } else {
                    h.std_guard_range(obj)
                }
            }
            rc::EMERGENCY_REPAIR..=rc::AMBULANCE => spt.map_or(0.0, |(_, r)| r),
            // (a type past the enum reads past `m_radiusCursors` upstream; here it makes nothing)
            _ => return,
        };
        if radius <= 0.0 {
            return;
        }
        let pos = Coord3D { x: 0.0, y: 0.0, z: 0.0 }; // will be updated right away
        let local = h.radius_local_player();
        let t = &self.radius_cursors[cursor_type as usize];
        create_radius_decal(t, cursor_type, pos, radius, Some(controller), local, &mut self.cur_radius_cursor);
        self.cur_rc_type = cursor_type;
        self.handle_radius_cursor(ui, h);
    }

    /// `handleRadiusCursor()`: the decal follows the mouse (or the guard hint's stashed position, fading with its timer).
    pub fn handle_radius_cursor<H: RadiusCursorHost>(&mut self, ui: &RadiusUiState, h: &mut H) {
        if self.cur_radius_cursor.is_empty() {
            return;
        }
        let mouse = h.radius_mouse_pos();
        let radar_on = h.radius_radar_on();
        let pos = match radar_on.then(|| h.radar_screen_pixel_to_world(mouse)).flatten() {
            Some(p) if !plant(9180) => p,
            _ => h.radius_screen_to_terrain(mouse),
        };
        let timer = ui.guard_hint_timer;
        if h.radius_double_click_attack_move() && timer > 0 {
            self.cur_radius_cursor.set_opacity(timer as f32 * 0.1);
            self.cur_radius_cursor.set_position(ui.guard_hint_stash);
        } else {
            self.cur_radius_cursor.set_position(pos);
            let (now, icons) = (h.radius_logic_frame(), h.draw_icon_ui());
            self.cur_radius_cursor.update(&self.radius_cursors, now, icons);
        }
    }

    /// The decal to draw, if any: the projected decal of a radius cursor that is shown.
    pub fn shown(&self) -> Option<&DecalShadow> {
        self.cur_radius_cursor.decal.as_ref()
    }
}

// ---- #130 (batch #134): the logic's own radius decals ----

/// #130: what the client's half of the logic's `RadiusDecal`s asks of the game.
pub trait LogicRadiusDecalHost {
    /// `owningPlayer->getPlayerColor()` of the player with this index (`None`: no such player, read as `NULL`).
    fn decal_player_color(&self, player: i32) -> Option<i32>;
    /// `ThePlayerList->getLocalPlayer()->getPlayerIndex()`.
    fn decal_local_player(&self) -> i32;
    /// `TheGameLogic->getDrawIconUI()`.
    fn decal_draw_icon_ui(&self) -> bool;
}

/// #130: the client's halves of the logic's `RadiusDecal`s: the delivery decals of `DeliverPayloadAIUpdate`,
/// `NeutronMissileUpdate` and `RadiusDecalUpdate` (made by the OCL `Attack` nugget), `DynamicShroudClearingRangeUpdate`'s
/// grid decals and the Spectre gunship's attack area and targeting reticle. The logic keeps each decal's `m_empty` (game
/// state, the same on every machine) and tells the client every call on it as an
/// [`ObjectClientEvent::GridDecal`](crate::logic::world::object_events::ObjectClientEvent::GridDecal), addressed by the
/// owning object and the decal's index in its module. Here each address keeps what upstream's `RadiusDecal` keeps beyond
/// `m_empty`: `m_template` (the module data's template, which the event carries a copy of) and `m_decal` (the
/// projected decal, [`DecalShadow`]), and the calls are made on them as RadiusDecal.cpp makes them:
///
/// - `createRadiusDecal(pos, radius, owningPlayer, result)` ([`create_radius_decal`]): cleared, then, with an owning
///   player, a texture and a radius above 0, a projected decal when the template is visible to everyone or the owner is
///   the local player, coloured the template's colour or the owner's;
/// - `clear()`, `setPosition(pos)`, `setOpacity(o)` ([`ClientRadiusDecal`]);
/// - `update()`: the opacity throb at `TheGameLogic->getFrame()`, the frame the module ran in (the events are read after
///   the frame, so the caller passes the frame the logic was in before it advanced);
/// - `~RadiusDecal()`: the owning object's destruction (`ClientEvent::ObjectDestroyed`) clears all its decals, as each
///   module's member is destroyed with the object;
/// - `xferRadiusDecal` on a load clears the decal: the client starts a loaded game with none.
///
/// Nothing here reaches the logic: the client only reads the events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogicRadiusDecals {
    /// By `(object, index)`: the template the decal was made from and the decal.
    decals: std::collections::BTreeMap<(ObjectId, u32), (RadiusDecalTemplate, ClientRadiusDecal)>,
}

impl LogicRadiusDecals {
    /// One call on the decal at `(object, index)`, during logic frame `now`.
    pub fn apply(&mut self, object: ObjectId, index: u32, op: &crate::logic::world::object_events::GridDecalOp, now: u32, h: &impl LogicRadiusDecalHost) {
        use crate::logic::world::object_events::GridDecalOp;
        let index = if plant(9227) { 0 } else { index };
        let (tmpl, decal) = self.decals.entry((object, index)).or_insert_with(|| (RadiusDecalTemplate::default(), ClientRadiusDecal::default()));
        match op {
            GridDecalOp::Clear => {
                if !plant(9222) {
                    decal.clear();
                }
            }
            GridDecalOp::Create { template, radius, position, player } => {
                // `result.m_template = this`: the module data's template, kept here as the event's copy
                *tmpl = template.clone();
                let owning = player.and_then(|p| h.decal_player_color(p).map(|c| (p, c)));
                let local = if plant(9226) { owning.map_or(-1, |o| o.0) } else { h.decal_local_player() };
                create_radius_decal(tmpl, 0, *position, *radius, owning, local, decal);
            }
            GridDecalOp::SetPosition(p) => decal.set_position(*p),
            GridDecalOp::SetOpacity(o) => {
                if !plant(9225) {
                    decal.set_opacity(*o);
                }
            }
            GridDecalOp::Update => {
                let frame = if plant(9224) { now.wrapping_add(1) } else { now };
                decal.update(std::slice::from_ref(tmpl), frame, h.decal_draw_icon_ui());
            }
        }
    }

    /// `~RadiusDecal()` for every decal of `object`: the object is gone.
    pub fn object_destroyed(&mut self, object: ObjectId) {
        if plant(9223) {
            return;
        }
        self.decals.retain(|&(o, _), _| o != object);
    }

    /// The frame's client events, in order: the grid decal calls and the objects destroyed. `now` is the logic frame the
    /// events were made in.
    pub fn take_events(&mut self, events: &[crate::logic::client_event::ClientEvent], now: u32, h: &impl LogicRadiusDecalHost) {
        use crate::logic::client_event::ClientEvent;
        use crate::logic::world::object_events::ObjectClientEvent;
        for e in events {
            match e {
                ClientEvent::Object(ObjectClientEvent::GridDecal { object_id, index, op }) => self.apply(*object_id, *index, op, now, h),
                ClientEvent::ObjectDestroyed { object_id } => self.object_destroyed(*object_id),
                _ => {}
            }
        }
    }

    /// The decal at `(object, index)`, if the logic ever made a call on it.
    pub fn get(&self, object: ObjectId, index: u32) -> Option<&ClientRadiusDecal> {
        self.decals.get(&(object, index)).map(|(_, d)| d)
    }

    /// The projected decals to draw, in address order.
    pub fn shown(&self) -> impl Iterator<Item = &DecalShadow> {
        self.decals.values().filter_map(|(_, d)| d.decal.as_ref())
    }
}

#[cfg(test)]
mod logic_decal_tests {
    use super::*;
    use crate::logic::world::object_events::GridDecalOp;

    struct Host;
    impl LogicRadiusDecalHost for Host {
        fn decal_player_color(&self, p: i32) -> Option<i32> {
            (0..4).contains(&p).then_some(0x10 + p)
        }
        fn decal_local_player(&self) -> i32 {
            1
        }
        fn decal_draw_icon_ui(&self) -> bool {
            true
        }
    }

    /// A delivery decal for the local player (1) is drawn in its colour at the target, follows `setPosition`, throbs on
    /// `update` (frame 0 of a 30-frame throb from 0 % to 100 %: half of 255, 127.5, truncated by `REAL_TO_INT` to 127) and goes with the object; one for player 2
    /// visible only to its owner is not drawn but not empty; a clear empties it.
    #[test]
    fn the_logics_decals_are_made_moved_throbbed_and_cleared() {
        let t = RadiusDecalTemplate { name: b"SCCNukeDecal".to_vec(), min_opacity: 0.0, max_opacity: 1.0, opacity_throb_time: 30, ..Default::default() };
        let at = Coord3D { x: 50.0, y: 60.0, z: 2.0 };
        let mut d = LogicRadiusDecals::default();
        d.apply(7, 0, &GridDecalOp::Create { template: t.clone(), radius: 40.0, position: at, player: Some(1) }, 5, &Host);
        let s = d.get(7, 0).and_then(|x| x.decal.clone()).expect("drawn for the local player");
        assert_eq!((s.size, s.color, s.position), ((80.0, 80.0), 0x11, at));
        d.apply(7, 0, &GridDecalOp::SetPosition(Coord3D { x: 1.0, y: 2.0, z: 3.0 }), 5, &Host);
        d.apply(7, 0, &GridDecalOp::Update, 0, &Host);
        let s = d.get(7, 0).and_then(|x| x.decal.clone()).expect("still drawn");
        assert_eq!((s.position, s.opacity), (Coord3D { x: 1.0, y: 2.0, z: 3.0 }, Some(127)));
        d.apply(8, 3, &GridDecalOp::Create { template: t.clone(), radius: 40.0, position: at, player: Some(2) }, 5, &Host);
        assert!(d.get(8, 3).is_some_and(|x| !x.is_empty() && x.decal.is_none()), "made for another player: not empty, not drawn");
        assert_eq!(d.shown().count(), 1);
        d.object_destroyed(7);
        assert!(d.get(7, 0).is_none() && d.shown().count() == 0, "the object's decals go with it");
        d.apply(8, 3, &GridDecalOp::Clear, 5, &Host);
        assert!(d.get(8, 3).is_some_and(|x| x.is_empty()));
    }
}
