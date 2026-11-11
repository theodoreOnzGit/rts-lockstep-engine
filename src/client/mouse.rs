// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/Mouse.h (MouseCursor, RedrawMode, CursorInfo, the members below)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Mouse.cpp (TheMouseCursorFieldParseTable,
//     TheMouseFieldParseTable, CursorInfo::CursorInfo, Mouse::Mouse, Mouse::parseIni, Mouse::init (its cursor
//     members), Mouse::setMouseText, Mouse::getCursorIndex, Mouse::setCursor, INI::parseMouseCursorDefinition,
//     INI::parseMouseDefinition)
//   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/GameClient/Win32Mouse.cpp (Win32Mouse::Win32Mouse (its cursor
//     members), setVisibility, initCursorResources, setCursor)
//   GeneralsMD/Code/GameEngineDevice/Include/Win32Device/GameClient/Win32Mouse.h (lostFocus)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DMouse.cpp (W3DMouse::W3DMouse (its cursor
//     members), init, setCursor and draw on their RM_WINDOWS branches, setCursorDirection)
//   GeneralsMD/Code/Main/WinMain.cpp (WM_SETCURSOR, WM_SETFOCUS and WM_KILLFOCUS on the mouse)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (setMouseCursor; setScrolling's cursor)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Mouse.cpp: Colin Day, June 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TheMouse`'s cursor (GitHub #117): which cursor the game shows and whether it shows one, the cursor definitions of
//! `Data\INI\Mouse.ini`, and the calls the rest of the client makes on them (`setCursor`, `setVisibility`,
//! `getMouseCursor`, `getVisibility`).
//!
//! # As upstream reads it
//!
//! **The mode.** `TheGlobalData->m_winCursors` is `TRUE` (GlobalData.cpp:638; the command line's `-winCursors` sets it
//! `TRUE` again and nothing sets it `FALSE`), so `Mouse::Mouse` and again `Mouse::init` put the cursor in
//! `RM_WINDOWS`, the Windows cursor, whatever `Mouse.ini`'s `CursorMode` says (it is read before `init`).
//! `W3DMouse::setRedrawMode`, which would change it, has no caller. The game therefore always shows a Windows cursor
//! loaded from `data\cursors\<Texture>[<direction>].ANI`, and the W3D model, polygon and DX8 cursors never show.
//!
//! **Start.** `GameClient::init` makes the mouse (`m_currentCursor = ARROW`, `m_visible = FALSE`, no cursor text
//! string), reads `Mouse.ini` (`parseIni`: the `MouseCursor` blocks into `m_cursorInfo` by `getCursorIndex` of the
//! block's name, the `Mouse` block into the mouse), loads the cursor resources (`initCursorResources`: for every
//! cursor from `FIRST_CURSOR` and every direction below its `Directions`, if it names a `Texture` and the resource is
//! not loaded yet, `LoadCursorFromFile` of the `.ANI`, with the direction's number before `.ANI` when it has more than
//! one), and later `init`s it (`W3DMouse::init`: `Mouse::init`'s `m_currentCursor = ARROW` and its cursor text
//! string made, then `setCursor(ARROW)`).
//!
//! **`setCursor(cursor)`** is `W3DMouse::setCursor`'s `RM_WINDOWS` branch: the direction frame back to 0, the D3D,
//! W3D and polygon cursors forgotten (`NONE`), `setCursorDirection(cursor)`; **only while the mouse is drawing**
//! (`m_drawing`, inside `draw`) `Win32Mouse::setCursor(cursor)`; then `m_currentCursor = cursor`.
//! `Win32Mouse::setCursor` first calls `Mouse::setCursor` (which, when the cursor changes and the text string exists,
//! sets the cursor text to the definition's `CursorText` fetched from the game's text with its two colours, or to
//! nothing with the colours kept); with the focus lost that is all; else `SetCursor(NULL)` for `NONE` or an invisible
//! mouse, else `SetCursor` of the cursor's resource for the direction frame; then both `m_currentWin32Cursor` and
//! `m_currentCursor` are the cursor.
//!
//! **`setCursorDirection(cursor)`**: for a cursor whose definition has more than one direction while
//! `TheInGameUI->isScrolling()`, the scroll amount (`getScrollAmount`), if it is not zero, normalized; its angle
//! `atan2(y, x)` brought into `[0, 2π)` with `fmod(theta + 2π, 2π)`; the frame is
//! `(Int)(theta / (2π / n) + 0.5)` with `n` the directions of the **current** cursor (`m_currentCursor`, not the new
//! one), 0 if that reaches `n`. Otherwise the frame is 0.
//!
//! **`setVisibility(visible)`** (`Win32Mouse`'s, not overridden by `W3DMouse`): `m_visible`, then
//! `Win32Mouse::setCursor(getMouseCursor())` called directly: the Windows cursor is set (or hidden) at once, not only
//! while drawing.
//!
//! **`draw()`** (`W3DDisplay::draw` every frame, and on the load screen): `m_drawing` set, `setCursor(m_currentCursor)`
//! (so the Windows cursor is set from the current cursor once a frame), nothing else for `RM_WINDOWS`, the cursor text
//! drawn at the mouse and (when visible) the tooltip, `m_drawing` cleared.
//!
//! **The window's messages** (WinMain.cpp): `WM_SETCURSOR` calls `TheWin32Mouse->setCursor(getMouseCursor())`, which
//! is virtual (`W3DMouse::setCursor`: not drawing, so no `SetCursor`); `WM_SETFOCUS` and `WM_KILLFOCUS` call
//! `lostFocus(FALSE)` and `lostFocus(TRUE)`.
//!
//! **`InGameUI::setMouseCursor(c)`**: `TheMouse->setCursor(c)`; in `MOUSEMODE_GUI_COMMAND`, a cursor other than the
//! arrow and the scroll one is remembered as `m_mouseModeCursor`. **`InGameUI::setScrolling(b)`** when it changes: on,
//! the mouse captured and `setMouseCursor(SCROLL)`; off, `setMouseCursor(ARROW)` and the capture released.
//!
//! # Upstream quirks that are kept
//!
//! - **Outside `draw` nothing reaches the Windows cursor but `setVisibility`**: `setCursor` only records the cursor;
//!   the next `draw` (or a `setVisibility`) shows it.
//! - **The cursor text never changes in `RM_WINDOWS` unless the cursor changes inside `draw`**: `W3DMouse::setCursor`
//!   stores `m_currentCursor` before the next `draw`'s `Win32Mouse::setCursor`, whose `Mouse::setCursor` then finds the
//!   cursor unchanged and returns. Only `setVisibility` (which calls `Win32Mouse::setCursor` directly, with the cursor
//!   already current) and `draw` reach it, both with the current cursor: in practice the text stays as `init` left it.
//!   (`tests/differential_mouse.rs`, 2026-11-11: over 20,000 random `Mouse.ini`s and 608,946 steps against the C++, 0
//!   mismatches, and no step set a cursor text; the planted bug 9068, which would keep the text, is not caught there
//!   for that reason, as predicted.)
//! - **The direction frame divides by the current cursor's directions**, not the new one's, and a cursor with fewer
//!   directions than the frame reached gets the frame anyway (its resource for that direction is `NULL`, so the cursor
//!   disappears: `SetCursor(NULL)`).
//! - With the focus lost, `Win32Mouse::setCursor` returns before it saves the cursor; `W3DMouse::setCursor` saves it
//!   after anyway.
//! - `Mouse::setCursor` keeps the colours when the definition has no text.
//! - `getCursorIndex` of an empty name is `INVALID_MOUSE_CURSOR` without the `DEBUG_CRASH`; an unknown name likewise
//!   (the release build has no `DEBUG_CRASH`). A `MouseCursor` block with an unknown name is not read: its field lines
//!   are then read as block names, and the file's load fails (`INI::load`'s unknown token).
//! - `CursorInfo`'s colours leave `alpha` unset (`0` here, as for an object in static storage).
//!
//! # Where this differs from upstream
//!
//! - **The Windows cursor is the client's system cursor icon.** `SetCursor(resource)` is recorded as
//!   [`OsCursor::Resource`] (the cursor and the direction frame it was loaded for), and the client (`client/`) shows a
//!   system cursor icon chosen for that cursor and direction; `SetCursor(NULL)` is [`OsCursor::Hidden`]. The game's
//!   animated `.ANI` cursor art is not drawn: it is EA's, the browser cannot show a Windows `.ANI` cursor, and a phone
//!   shows no cursor at all. Whether a resource exists is still decided as `initCursorResources` decides it (the
//!   definition names a `Texture`; the file is asked of the owner through `exists`), so a cursor the game would not
//!   show is not shown.
//! - **Only `RM_WINDOWS` is translated** (the one mode the game runs in, above): `W3DMouse::setCursor`'s and `draw`'s
//!   `RM_DX8`, `RM_POLYGON` and `RM_W3D` branches, their assets and `setRedrawMode` are not. [`Mouse::init`] sets
//!   `RM_WINDOWS` as upstream's does with `m_winCursors`.
//! - **`TheInGameUI->isScrolling()` and `getScrollAmount()`**, which `setCursorDirection` reads, are
//!   [`Mouse::ui_scrolling`] and [`Mouse::ui_scroll_amount`], which the owner keeps up to date (the camera's).
//! - **The cursor text** is kept as the definition's text label ([`CursorText::label`], `TheGameText->fetch` of it is the
//!   drawer's), with its colours; `drawCursorText` and `drawTooltip` are not here (the window manager keeps the
//!   tooltip).
//! - The mouse's input (`MouseIO`, the event buffer, `createStreamMessages`, `setPosition`, `setMouseLimits`,
//!   `capture`) is not here: the client's input is Bevy's (`client/src/input.rs`, `client/src/camera.rs`). The
//!   `Mouse` block's tooltip fields and drag tolerances are read into [`MouseIniSettings`].
//! - `MouseThreadClass` (`isThread`) only runs in `RM_DX8`: `draw` always draws its text here.
//!
//! # Representation
//!
//! [`Mouse`] holds `Mouse`'s, `Win32Mouse`'s and `W3DMouse`'s members that decide the cursor, `cursorResources` as
//! [`Mouse::resources`] (whether each cursor's direction loaded), and the last `SetCursor` as [`Mouse::os_cursor`].
//! Cursors are upstream's `MouseCursor` numbers ([`cursor`]); the release build defines neither `ALLOW_DEMORALIZE`
//! nor `ALLOW_SURRENDER`, so `DOCK` is 29 and there are 40.

use crate::common::base_type::{Coord2D, ICoord2D, RGBAColorInt};
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::logic::modules::plant;

/// `Mouse::MouseCursor` (release build: no `ALLOW_DEMORALIZE`, no `ALLOW_SURRENDER`).
pub mod cursor {
    pub const INVALID_MOUSE_CURSOR: i32 = -1;
    pub const NONE: i32 = 0;
    pub const FIRST_CURSOR: i32 = 1;
    pub const NORMAL: i32 = 1;
    pub const ARROW: i32 = 2;
    pub const SCROLL: i32 = 3;
    pub const CROSS: i32 = 4;
    pub const MOVETO: i32 = 5;
    pub const ATTACKMOVETO: i32 = 6;
    pub const ATTACK_OBJECT: i32 = 7;
    pub const FORCE_ATTACK_OBJECT: i32 = 8;
    pub const FORCE_ATTACK_GROUND: i32 = 9;
    pub const BUILD_PLACEMENT: i32 = 10;
    pub const INVALID_BUILD_PLACEMENT: i32 = 11;
    pub const GENERIC_INVALID: i32 = 12;
    pub const SELECTING: i32 = 13;
    pub const ENTER_FRIENDLY: i32 = 14;
    pub const ENTER_AGGRESSIVELY: i32 = 15;
    pub const SET_RALLY_POINT: i32 = 16;
    pub const GET_REPAIRED: i32 = 17;
    pub const GET_HEALED: i32 = 18;
    pub const DO_REPAIR: i32 = 19;
    pub const RESUME_CONSTRUCTION: i32 = 20;
    pub const CAPTUREBUILDING: i32 = 21;
    pub const SNIPE_VEHICLE: i32 = 22;
    pub const LASER_GUIDED_MISSILES: i32 = 23;
    pub const TANKHUNTER_TNT_ATTACK: i32 = 24;
    pub const STAB_ATTACK: i32 = 25;
    pub const PLACE_REMOTE_CHARGE: i32 = 26;
    pub const PLACE_TIMED_CHARGE: i32 = 27;
    pub const DEFECTOR: i32 = 28;
    pub const DOCK: i32 = 29;
    pub const FIRE_FLAME: i32 = 30;
    pub const FIRE_BOMB: i32 = 31;
    pub const PLACE_BEACON: i32 = 32;
    pub const DISGUISE_AS_VEHICLE: i32 = 33;
    pub const WAYPOINT: i32 = 34;
    pub const OUTRANGE: i32 = 35;
    pub const STAB_ATTACK_INVALID: i32 = 36;
    pub const PLACE_CHARGE_INVALID: i32 = 37;
    pub const HACK: i32 = 38;
    pub const PARTICLE_UPLINK_CANNON: i32 = 39;
    pub const NUM_MOUSE_CURSORS: i32 = 40;
}

/// `Mouse::RedrawMode`.
pub mod redraw_mode {
    pub const RM_WINDOWS: i32 = 0;
    pub const RM_W3D: i32 = 1;
    pub const RM_POLYGON: i32 = 2;
    pub const RM_DX8: i32 = 3;
}

/// `MAX_2D_CURSOR_DIRECTIONS`.
pub const MAX_2D_CURSOR_DIRECTIONS: usize = 8;

/// `getCursorIndex`'s `CursorININames[]`, by cursor.
pub const CURSOR_INI_NAMES: [&str; cursor::NUM_MOUSE_CURSORS as usize] = [
    "None",
    "Normal",
    "Arrow",
    "Scroll",
    "Target",
    "Move",
    "AttackMove",
    "AttackObj",
    "ForceAttackObj",
    "ForceAttackGround",
    "Build",
    "InvalidBuild",
    "GenericInvalid",
    "Select",
    "EnterFriendly",
    "EnterAggressive",
    "SetRallyPoint",
    "GetRepaired",
    "GetHealed",
    "DoRepair",
    "ResumeConstruction",
    "CaptureBuilding",
    "SnipeVehicle",
    "LaserGuidedMissiles",
    "TankHunterTNTAttack",
    "StabAttack",
    "PlaceRemoteCharge",
    "PlaceTimedCharge",
    "Defector",
    "Dock",
    "FireFlame",
    "FireBomb",
    "PlaceBeacon",
    "DisguiseAsVehicle",
    "Waypoint",
    "OutRange",
    "StabAttackInvalid",
    "PlaceChargeInvalid",
    "Hack",
    "ParticleUplinkCannon",
];

/// `Mouse::getCursorIndex(name)`: the cursor named `name` (without case), or `INVALID_MOUSE_CURSOR`.
pub fn get_cursor_index(name: &[u8]) -> i32 {
    if name.is_empty() {
        return cursor::INVALID_MOUSE_CURSOR;
    }
    for (i, n) in CURSOR_INI_NAMES.iter().enumerate() {
        let same = if plant(9066) { n.as_bytes() == name } else { n.as_bytes().eq_ignore_ascii_case(name) };
        if same {
            return i as i32;
        }
    }
    cursor::INVALID_MOUSE_CURSOR
}

/// `class CursorInfo`: one `MouseCursor` definition.
#[derive(Clone, Debug, PartialEq)]
pub struct CursorInfo {
    pub cursor_name: Vec<u8>,
    pub cursor_text: Vec<u8>,
    pub cursor_text_color: RGBAColorInt,
    pub cursor_text_drop_color: RGBAColorInt,
    pub texture_name: Vec<u8>,
    pub image_name: Vec<u8>,
    pub w3d_model_name: Vec<u8>,
    pub w3d_anim_name: Vec<u8>,
    pub w3d_scale: f32,
    pub loop_: bool,
    pub hot_spot_position: ICoord2D,
    pub num_frames: i32,
    pub fps: f32,
    pub num_directions: i32,
}

impl Default for CursorInfo {
    /// `CursorInfo::CursorInfo()`.
    fn default() -> Self {
        CursorInfo {
            cursor_name: Vec::new(),
            cursor_text: Vec::new(),
            cursor_text_color: RGBAColorInt::default(),
            cursor_text_drop_color: RGBAColorInt::default(),
            texture_name: Vec::new(),
            image_name: Vec::new(),
            w3d_model_name: Vec::new(),
            w3d_anim_name: Vec::new(),
            w3d_scale: 1.0,
            loop_: true,
            // Assume hotspot is at the center of a 32x32 image.
            hot_spot_position: ICoord2D { x: 16, y: 16 },
            num_frames: 1,
            fps: 20.0,
            num_directions: 1,
        }
    }
}

/// `TheMouseCursorFieldParseTable`.
const CURSOR_FIELDS: &[FieldParse<CursorInfo>] = &[
    FieldParse::new("CursorText", |ini, c, _| ini.parse_ascii_string().map(|v| c.cursor_text = v), UserData::None),
    FieldParse::new("CursorTextColor", |ini, c, _| ini.parse_rgba_color_int().map(|v| c.cursor_text_color = v), UserData::None),
    FieldParse::new("CursorTextDropColor", |ini, c, _| ini.parse_rgba_color_int().map(|v| c.cursor_text_drop_color = v), UserData::None),
    FieldParse::new("W3DModel", |ini, c, _| ini.parse_ascii_string().map(|v| c.w3d_model_name = v), UserData::None),
    FieldParse::new("W3DAnim", |ini, c, _| ini.parse_ascii_string().map(|v| c.w3d_anim_name = v), UserData::None),
    FieldParse::new("W3DScale", |ini, c, _| ini.parse_real().map(|v| c.w3d_scale = v), UserData::None),
    FieldParse::new("Loop", |ini, c, _| ini.parse_bool().map(|v| c.loop_ = v), UserData::None),
    FieldParse::new("Image", |ini, c, _| ini.parse_ascii_string().map(|v| c.image_name = v), UserData::None),
    FieldParse::new("Texture", |ini, c, _| ini.parse_ascii_string().map(|v| c.texture_name = v), UserData::None),
    FieldParse::new("HotSpot", |ini, c, _| ini.parse_icoord2d().map(|v| c.hot_spot_position = v), UserData::None),
    FieldParse::new("Frames", |ini, c, _| ini.parse_int().map(|v| c.num_frames = v), UserData::None),
    FieldParse::new("FPS", |ini, c, _| ini.parse_real().map(|v| c.fps = v), UserData::None),
    FieldParse::new("Directions", |ini, c, _| ini.parse_int().map(|v| c.num_directions = v), UserData::None),
    FieldParse::end(),
];

/// The `Mouse` block of `Mouse.ini` (`TheMouseFieldParseTable`'s members of `Mouse`, with `Mouse::Mouse`'s values).
#[derive(Clone, Debug, PartialEq)]
pub struct MouseIniSettings {
    pub tooltip_font_name: Vec<u8>,
    pub tooltip_font_size: i32,
    pub tooltip_font_is_bold: bool,
    pub tooltip_animate_background: bool,
    pub tooltip_fill_time: i32,
    pub tooltip_delay_time: i32,
    pub tooltip_color_text: RGBAColorInt,
    pub tooltip_color_highlight: RGBAColorInt,
    pub tooltip_color_shadow: RGBAColorInt,
    pub tooltip_color_background: RGBAColorInt,
    pub tooltip_color_border: RGBAColorInt,
    pub tooltip_width: f32,
    pub use_tooltip_alt_text_color: bool,
    pub use_tooltip_alt_back_color: bool,
    pub adjust_tooltip_alt_color: bool,
    pub ortho_camera: bool,
    pub ortho_zoom: f32,
    pub drag_tolerance: u32,
    pub drag_tolerance_3d: u32,
    pub drag_tolerance_ms: u32,
}

impl Default for MouseIniSettings {
    fn default() -> Self {
        let c = |red, green, blue, alpha| RGBAColorInt { red, green, blue, alpha };
        MouseIniSettings {
            tooltip_font_name: b"Times New Roman".to_vec(),
            tooltip_font_size: 12,
            tooltip_font_is_bold: false,
            tooltip_animate_background: true,
            tooltip_fill_time: 50,
            tooltip_delay_time: 50,
            tooltip_color_text: c(220, 220, 220, 255),
            tooltip_color_highlight: c(255, 255, 0, 255),
            tooltip_color_shadow: c(0, 0, 0, 255),
            tooltip_color_background: c(20, 20, 0, 127),
            tooltip_color_border: c(0, 0, 0, 255),
            tooltip_width: 15.0,
            use_tooltip_alt_text_color: false,
            use_tooltip_alt_back_color: false,
            adjust_tooltip_alt_color: false,
            ortho_camera: false,
            ortho_zoom: 1.0,
            drag_tolerance: 0,
            drag_tolerance_3d: 0,
            drag_tolerance_ms: 0,
        }
    }
}

/// `TheMouseFieldParseTable` (its `CursorMode` is the mouse's redraw mode).
const MOUSE_FIELDS: &[FieldParse<Mouse>] = &[
    FieldParse::new("TooltipFontName", |ini, m, _| ini.parse_ascii_string().map(|v| m.settings.tooltip_font_name = v), UserData::None),
    FieldParse::new("TooltipFontSize", |ini, m, _| ini.parse_int().map(|v| m.settings.tooltip_font_size = v), UserData::None),
    FieldParse::new("TooltipFontIsBold", |ini, m, _| ini.parse_bool().map(|v| m.settings.tooltip_font_is_bold = v), UserData::None),
    FieldParse::new("TooltipAnimateBackground", |ini, m, _| ini.parse_bool().map(|v| m.settings.tooltip_animate_background = v), UserData::None),
    FieldParse::new("TooltipFillTime", |ini, m, _| ini.parse_int().map(|v| m.settings.tooltip_fill_time = v), UserData::None),
    FieldParse::new("TooltipDelayTime", |ini, m, _| ini.parse_int().map(|v| m.settings.tooltip_delay_time = v), UserData::None),
    FieldParse::new("TooltipTextColor", |ini, m, _| ini.parse_rgba_color_int().map(|v| m.settings.tooltip_color_text = v), UserData::None),
    FieldParse::new("TooltipHighlightColor", |ini, m, _| ini.parse_rgba_color_int().map(|v| m.settings.tooltip_color_highlight = v), UserData::None),
    FieldParse::new("TooltipShadowColor", |ini, m, _| ini.parse_rgba_color_int().map(|v| m.settings.tooltip_color_shadow = v), UserData::None),
    FieldParse::new("TooltipBackgroundColor", |ini, m, _| ini.parse_rgba_color_int().map(|v| m.settings.tooltip_color_background = v), UserData::None),
    FieldParse::new("TooltipBorderColor", |ini, m, _| ini.parse_rgba_color_int().map(|v| m.settings.tooltip_color_border = v), UserData::None),
    FieldParse::new("TooltipWidth", |ini, m, _| ini.parse_percent_to_real().map(|v| m.settings.tooltip_width = v), UserData::None),
    FieldParse::new("CursorMode", |ini, m, _| ini.parse_int().map(|v| m.current_redraw_mode = v), UserData::None),
    FieldParse::new("UseTooltipAltTextColor", |ini, m, _| ini.parse_bool().map(|v| m.settings.use_tooltip_alt_text_color = v), UserData::None),
    FieldParse::new("UseTooltipAltBackColor", |ini, m, _| ini.parse_bool().map(|v| m.settings.use_tooltip_alt_back_color = v), UserData::None),
    FieldParse::new("AdjustTooltipAltColor", |ini, m, _| ini.parse_bool().map(|v| m.settings.adjust_tooltip_alt_color = v), UserData::None),
    FieldParse::new("OrthoCamera", |ini, m, _| ini.parse_bool().map(|v| m.settings.ortho_camera = v), UserData::None),
    FieldParse::new("OrthoZoom", |ini, m, _| ini.parse_real().map(|v| m.settings.ortho_zoom = v), UserData::None),
    FieldParse::new("DragTolerance", |ini, m, _| ini.parse_unsigned_int().map(|v| m.settings.drag_tolerance = v), UserData::None),
    FieldParse::new("DragTolerance3D", |ini, m, _| ini.parse_unsigned_int().map(|v| m.settings.drag_tolerance_3d = v), UserData::None),
    FieldParse::new("DragToleranceMS", |ini, m, _| ini.parse_unsigned_int().map(|v| m.settings.drag_tolerance_ms = v), UserData::None),
    FieldParse::end(),
];

/// `INI::parseMouseCursorDefinition`: the block's name; a known cursor's definition read (and named), an unknown one's
/// block left unread.
fn parse_mouse_cursor_definition(ini: &mut Ini<'_>, m: &mut Mouse) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let index = get_cursor_index(&name);
    if index != cursor::INVALID_MOUSE_CURSOR {
        let info = &mut m.cursor_info[index as usize];
        info.cursor_name = name;
        ini.init_from_ini(info, CURSOR_FIELDS)?;
    }
    Ok(())
}

/// `INI::parseMouseDefinition`.
fn parse_mouse_definition(ini: &mut Ini<'_>, m: &mut Mouse) -> IniResult<()> {
    ini.init_from_ini(m, MOUSE_FIELDS)
}

/// The block types `Mouse.ini` holds (upstream's `theTypeTable` has them among the others).
pub const MOUSE_BLOCKS: &[BlockParse<Mouse>] = &[
    BlockParse { token: "Mouse", parse: parse_mouse_definition },
    BlockParse { token: "MouseCursor", parse: parse_mouse_cursor_definition },
];

/// The cursor text (`m_cursorTextDisplayString`'s text and `m_cursorTextColor`, `m_cursorTextDropColor`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CursorText {
    /// The text's label in the game's text (`TheGameText->fetch(label)`); empty: no text (`UnicodeString(L"")`).
    pub label: Vec<u8>,
    pub color: RGBAColorInt,
    pub drop_color: RGBAColorInt,
}

/// What `SetCursor` was last given (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsCursor {
    /// `SetCursor(NULL)` (also a resource that did not load): no cursor.
    Hidden,
    /// `SetCursor(cursorResources[cursor][direction])`, a loaded resource.
    Resource { cursor: i32, direction: i32 },
}

/// `TheMouse`'s cursor (see the module docs).
#[derive(Clone, Debug)]
pub struct Mouse {
    /// `m_cursorInfo`, by cursor.
    pub cursor_info: Vec<CursorInfo>,
    /// The rest of the `Mouse` block.
    pub settings: MouseIniSettings,
    /// `m_currentRedrawMode`.
    pub current_redraw_mode: i32,
    /// `m_visible`.
    pub visible: bool,
    /// `m_currentCursor`.
    pub current_cursor: i32,
    /// The cursor text (`None`: `m_cursorTextDisplayString` is `NULL`, before `init`).
    pub cursor_text: Option<CursorText>,
    /// `Win32Mouse::m_lostFocus`.
    pub lost_focus: bool,
    /// `Win32Mouse::m_directionFrame`.
    pub direction_frame: i32,
    /// `Win32Mouse::m_currentWin32Cursor`.
    pub current_win32_cursor: i32,
    /// `cursorResources[cursor][direction] != NULL`, flat (`cursor * MAX_2D_CURSOR_DIRECTIONS + direction`, as the C
    /// array is laid out: a direction past the eighth is the next cursor's slot, as upstream indexes it).
    pub resources: Vec<bool>,
    /// The last `SetCursor` (`None`: never called: the window class's own arrow).
    pub os_cursor: Option<OsCursor>,
    /// `W3DMouse`'s `m_currentD3DCursor`, `m_currentW3DCursor`, `m_currentPolygonCursor`.
    pub current_d3d_cursor: i32,
    pub current_w3d_cursor: i32,
    pub current_polygon_cursor: i32,
    /// `W3DMouse::m_drawing`.
    pub drawing: bool,
    /// `TheInGameUI->isScrolling()` and `getScrollAmount()` as the owner last set them (`setCursorDirection` reads them).
    pub ui_scrolling: bool,
    pub ui_scroll_amount: Coord2D,
}

impl Default for Mouse {
    fn default() -> Self {
        Mouse::new()
    }
}

impl Mouse {
    /// `W3DMouse::W3DMouse()` (with `Mouse::Mouse` and `Win32Mouse::Win32Mouse`): the arrow, not visible, in `RM_WINDOWS`
    /// (`m_winCursors`), no cursor text string, no resources.
    pub fn new() -> Self {
        Mouse {
            cursor_info: vec![CursorInfo::default(); cursor::NUM_MOUSE_CURSORS as usize],
            settings: MouseIniSettings::default(),
            current_redraw_mode: redraw_mode::RM_WINDOWS,
            visible: false,
            current_cursor: cursor::ARROW,
            cursor_text: None,
            lost_focus: false,
            direction_frame: 0,
            current_win32_cursor: cursor::NONE,
            resources: vec![false; cursor::NUM_MOUSE_CURSORS as usize * MAX_2D_CURSOR_DIRECTIONS],
            os_cursor: None,
            current_d3d_cursor: cursor::NONE,
            current_w3d_cursor: cursor::NONE,
            current_polygon_cursor: cursor::NONE,
            drawing: false,
            ui_scrolling: false,
            ui_scroll_amount: Coord2D { x: 0.0, y: 0.0 },
        }
    }

    /// `parseIni()`: `INI::load("Data\\INI\\Mouse.ini", INI_LOAD_OVERWRITE)` of the file's bytes.
    pub fn parse_ini(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, MOUSE_BLOCKS, self)
    }

    /// `Win32Mouse::initCursorResources()`: each cursor's directions loaded from `data\cursors\<Texture>[n].ANI`, if it
    /// names a texture and `exists(path)` (`LoadCursorFromFile` succeeds).
    pub fn init_cursor_resources(&mut self, exists: impl Fn(&str) -> bool) {
        for c in cursor::FIRST_CURSOR..cursor::NUM_MOUSE_CURSORS {
            let info = &self.cursor_info[c as usize];
            let n = info.num_directions + i32::from(plant(9070));
            for direction in 0..n {
                let Some(slot) = self.resources.get_mut(c as usize * MAX_2D_CURSOR_DIRECTIONS + direction as usize) else { break };
                if !*slot && !info.texture_name.is_empty() {
                    let t = String::from_utf8_lossy(&info.texture_name);
                    let path = if info.num_directions > 1 { format!("data\\cursors\\{t}{direction}.ANI") } else { format!("data\\cursors\\{t}.ANI") };
                    *slot = exists(&path);
                }
            }
        }
    }

    /// `W3DMouse::init()`'s cursor: `Mouse::init` (`RM_WINDOWS` with `m_winCursors`, the arrow, the cursor text string
    /// made), then `setCursor(ARROW)`.
    pub fn init(&mut self) {
        self.current_redraw_mode = redraw_mode::RM_WINDOWS;
        self.current_cursor = cursor::ARROW;
        // (the colours are `Mouse::Mouse`'s white; the text string is `init`'s)
        let white = RGBAColorInt { red: 255, green: 255, blue: 255, alpha: 255 };
        self.cursor_text = Some(CursorText { label: Vec::new(), color: white, drop_color: white });
        self.set_cursor(cursor::ARROW);
    }

    /// `getMouseCursor()`.
    pub fn get_mouse_cursor(&self) -> i32 {
        self.current_cursor
    }

    /// `getVisibility()`.
    pub fn get_visibility(&self) -> bool {
        self.visible
    }

    /// `lostFocus(state)` (WinMain's `WM_KILLFOCUS`: true, `WM_SETFOCUS`: false).
    pub fn lost_focus(&mut self, state: bool) {
        self.lost_focus = state;
    }

    /// `Mouse::setMouseText(text, color, dropColor)` (the text by its label).
    fn set_mouse_text(&mut self, label: &[u8], color: Option<RGBAColorInt>, drop_color: Option<RGBAColorInt>) {
        let Some(t) = self.cursor_text.as_mut() else { return };
        t.label = label.to_vec();
        if let Some(c) = color {
            t.color = c;
        }
        if let Some(c) = drop_color {
            t.drop_color = c;
        }
    }

    /// `Mouse::setCursor(cursor)`: the cursor text, when the cursor changes.
    fn mouse_set_cursor(&mut self, cursor: i32) {
        if self.current_cursor == cursor {
            return;
        }
        if self.cursor_text.is_some() {
            let info = &self.cursor_info[cursor as usize];
            if !info.cursor_text.is_empty() {
                let (label, color, drop) = (info.cursor_text.clone(), info.cursor_text_color, info.cursor_text_drop_color);
                self.set_mouse_text(&label, Some(color), Some(drop));
            } else if !plant(9068) {
                self.set_mouse_text(b"", None, None);
            }
        }
    }

    /// `Win32Mouse::setCursor(cursor)`.
    fn win32_set_cursor(&mut self, cursor: i32) {
        self.mouse_set_cursor(cursor);
        if self.lost_focus && !plant(9063) {
            return; // stop messing with mouse cursor if we don't have focus.
        }
        if cursor == cursor::NONE || (!self.visible && !plant(9062)) {
            self.os_cursor = Some(OsCursor::Hidden);
        } else {
            let slot = cursor as usize * MAX_2D_CURSOR_DIRECTIONS + self.direction_frame as usize;
            let loaded = self.direction_frame >= 0 && self.resources.get(slot).copied().unwrap_or(false);
            self.os_cursor = Some(if loaded { OsCursor::Resource { cursor, direction: self.direction_frame } } else { OsCursor::Hidden });
        }
        self.current_win32_cursor = cursor;
        self.current_cursor = cursor;
    }

    /// `setCursor(cursor)`: `W3DMouse::setCursor`'s `RM_WINDOWS` branch.
    pub fn set_cursor(&mut self, cursor: i32) {
        self.direction_frame = 0;
        // Windows default cursor needs to refreshed whenever we get a WM_SETCURSOR
        self.current_d3d_cursor = cursor::NONE;
        self.current_w3d_cursor = cursor::NONE;
        self.current_polygon_cursor = cursor::NONE;
        self.set_cursor_direction(cursor);
        if self.drawing || plant(9064) {
            // only allow cursor to change when drawing the cursor (once per frame) to fix flickering.
            self.win32_set_cursor(cursor);
        }
        self.current_cursor = cursor;
    }

    /// `W3DMouse::setCursorDirection(cursor)`.
    fn set_cursor_direction(&mut self, cursor: i32) {
        let pi = core::f64::consts::PI;
        if self.cursor_info[cursor as usize].num_directions > 1 && self.ui_scrolling {
            let mut offset = self.ui_scroll_amount;
            if offset.x != 0.0 || offset.y != 0.0 {
                offset.normalize();
                // `atan2` on floats: C's double `atan2`, rounded back to `Real`
                let theta = petir::Real::atan2(f64::from(offset.y), f64::from(offset.x)) as f32;
                // `fmod` of two positive numbers (theta + 2π > 0): `rem_euclid` is `fmod` there
                let theta = petir::Real::rem_euclid(f64::from(theta) + pi * 2.0, pi * 2.0) as f32;
                let of = if plant(9061) { cursor } else { self.current_cursor };
                let num_directions = self.cursor_info[of as usize].num_directions;
                let half = if plant(9067) { 0.0 } else { f64::from(0.5f32) };
                // `(Int)(theta/(2.0f*M_PI/(Real)numDirections)+0.5f)`, in double
                self.direction_frame = (f64::from(theta) / (f64::from(2.0f32) * pi / f64::from(num_directions as f32)) + half) as i32;
                if self.direction_frame >= num_directions {
                    self.direction_frame = 0;
                }
            } else {
                self.direction_frame = 0;
            }
        } else {
            self.direction_frame = 0;
        }
    }

    /// `setVisibility(visible)` (`Win32Mouse`'s): the Windows cursor set (or hidden) at once.
    pub fn set_visibility(&mut self, visible: bool) {
        self.visible = visible;
        if !plant(9065) {
            // Maybe need to set cursor to force hiding of some cursors.
            self.win32_set_cursor(self.current_cursor);
        }
    }

    /// WinMain's `WM_SETCURSOR`: `TheWin32Mouse->setCursor(getMouseCursor())` (virtual: `W3DMouse::setCursor`).
    pub fn wm_set_cursor(&mut self) {
        self.set_cursor(self.current_cursor);
    }

    /// `W3DMouse::draw()` in `RM_WINDOWS`: the current cursor applied to the Windows cursor (the text and the tooltip are
    /// the drawer's).
    pub fn draw(&mut self) {
        self.drawing = true;
        // make sure the correct cursor image is selected
        self.set_cursor(self.current_cursor);
        if !plant(9071) {
            self.drawing = false;
        }
    }

    /// What the window shows: the cursor and direction of a loaded resource, `None` for no cursor (`SetCursor(NULL)`).
    /// Before any `SetCursor` the window class's arrow: the arrow.
    pub fn shown(&self) -> Option<(i32, i32)> {
        match self.os_cursor {
            None => Some((cursor::ARROW, 0)),
            Some(OsCursor::Hidden) => None,
            Some(OsCursor::Resource { cursor, direction }) => Some((cursor, direction)),
        }
    }
}

/// `InGameUI::setMouseCursor(c)`: `TheMouse->setCursor(c)`; in `MOUSEMODE_GUI_COMMAND` (`gui_command_mode`) a cursor
/// other than the arrow and the scroll one becomes `m_mouseModeCursor`.
pub fn in_game_ui_set_mouse_cursor(mouse: &mut Mouse, gui_command_mode: bool, mouse_mode_cursor: &mut i32, c: i32) {
    mouse.set_cursor(c);
    remember_mode_cursor(gui_command_mode, mouse_mode_cursor, c);
}

/// `InGameUI::setMouseCursor(c)`'s second half: in `MOUSEMODE_GUI_COMMAND` (`gui_command_mode`) a cursor other than the
/// arrow and the scroll one becomes `m_mouseModeCursor` (#122: shared with
/// [`crate::client::in_game_ui::InGameUiSelection::set_mouse_cursor`], whose `TheMouse->setCursor` is queued).
pub fn remember_mode_cursor(gui_command_mode: bool, mouse_mode_cursor: &mut i32, c: i32) {
    if gui_command_mode && (plant(9069) || (c != cursor::ARROW && c != cursor::SCROLL)) {
        *mouse_mode_cursor = c;
    }
}

/// `InGameUI::setScrolling(isScrolling)`'s cursor and flag (`m_isScrolling` is `is_scrolling`): on, `SCROLL`; off, the
/// arrow. (The mouse's capture and the tactical view's camera locks are not here.)
pub fn in_game_ui_set_scrolling(mouse: &mut Mouse, gui_command_mode: bool, mouse_mode_cursor: &mut i32, is_scrolling: &mut bool, scrolling: bool) {
    if *is_scrolling == scrolling {
        return;
    }
    if scrolling {
        in_game_ui_set_mouse_cursor(mouse, gui_command_mode, mouse_mode_cursor, cursor::SCROLL);
    } else {
        in_game_ui_set_mouse_cursor(mouse, gui_command_mode, mouse_mode_cursor, cursor::ARROW);
    }
    *is_scrolling = scrolling;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Mouse {
        let mut m = Mouse::new();
        m.cursor_info[cursor::SCROLL as usize].num_directions = 8;
        m.cursor_info[cursor::SCROLL as usize].texture_name = b"SCCScroll".to_vec();
        m.cursor_info[cursor::ARROW as usize].texture_name = b"SCCPointer".to_vec();
        m.cursor_info[cursor::ATTACK_OBJECT as usize].texture_name = b"SCCAttack".to_vec();
        m.cursor_info[cursor::ATTACK_OBJECT as usize].cursor_text = b"CURSOR:Attack".to_vec();
        m.init_cursor_resources(|_| true);
        m.init();
        m
    }

    /// #117, predicted from Mouse.cpp, Win32Mouse.cpp and W3DMouse.cpp before the run:
    /// - after `init` the mouse is the arrow, invisible, and no `SetCursor` was made (`setCursor(ARROW)` outside draw);
    /// - `setVisibility(TRUE)` shows the arrow at once; a `setCursor(ATTACK_OBJECT)` changes nothing on screen until the
    ///   next `draw`, which shows it; the cursor text stays empty (the quirk: the cursor was already current);
    /// - `setVisibility(FALSE)` hides it; a draw keeps it hidden; `NONE` is hidden even when visible;
    /// - the scroll cursor while scrolling right-down (`(1, 1)`): `atan2 = π/4`, frame `(Int)(π/4 / (π/4) + 0.5)` = 1, but
    ///   the frame divides by the **current** cursor's directions: from the arrow (1 direction) it is
    ///   `(Int)(π/4 / 2π + 0.5)` = 0; drawn again (now current) it is 1;
    /// - with the focus lost, a draw does not change the Windows cursor;
    /// - the resources: `SCCScroll0.ANI` .. `SCCScroll7.ANI` and `SCCPointer.ANI` asked for, nothing for a cursor
    ///   without a texture.
    ///
    /// Results (2026-11-11): all held.
    #[test]
    fn the_cursor_shows_as_the_windows_mouse_decides() {
        let mut asked = Vec::new();
        let mut m = Mouse::new();
        m.cursor_info[cursor::SCROLL as usize].num_directions = 8;
        m.cursor_info[cursor::SCROLL as usize].texture_name = b"SCCScroll".to_vec();
        m.cursor_info[cursor::ARROW as usize].texture_name = b"SCCPointer".to_vec();
        let cell = std::cell::RefCell::new(&mut asked);
        m.init_cursor_resources(|p| {
            cell.borrow_mut().push(p.to_string());
            true
        });
        assert_eq!(asked.len(), 9);
        assert!(asked.contains(&"data\\cursors\\SCCScroll7.ANI".to_string()) && asked.contains(&"data\\cursors\\SCCPointer.ANI".to_string()));

        let mut m = ready();
        assert_eq!((m.current_cursor, m.visible, m.os_cursor), (cursor::ARROW, false, None));
        m.set_visibility(true);
        assert_eq!(m.shown(), Some((cursor::ARROW, 0)));
        m.set_cursor(cursor::ATTACK_OBJECT);
        assert_eq!(m.shown(), Some((cursor::ARROW, 0)));
        m.draw();
        assert_eq!(m.shown(), Some((cursor::ATTACK_OBJECT, 0)));
        assert_eq!(m.cursor_text.as_ref().map(|t| t.label.clone()), Some(Vec::new()));
        m.set_visibility(false);
        assert_eq!(m.shown(), None);
        m.draw();
        assert_eq!(m.shown(), None);
        m.set_visibility(true);
        m.set_cursor(cursor::NONE);
        m.draw();
        assert_eq!(m.shown(), None);

        m.set_cursor(cursor::ARROW);
        m.draw();
        m.ui_scrolling = true;
        m.ui_scroll_amount = Coord2D { x: 1.0, y: 1.0 };
        m.set_cursor(cursor::SCROLL);
        m.draw();
        assert_eq!(m.shown(), Some((cursor::SCROLL, 1)));
        let mut m2 = ready();
        m2.set_visibility(true);
        m2.ui_scrolling = true;
        m2.ui_scroll_amount = Coord2D { x: 1.0, y: 1.0 };
        m2.set_cursor(cursor::SCROLL);
        assert_eq!(m2.direction_frame, 0);

        m.lost_focus(true);
        m.set_cursor(cursor::ARROW);
        m.draw();
        assert_eq!(m.shown(), Some((cursor::SCROLL, 1)));
        assert_eq!(m.current_cursor, cursor::ARROW);
        m.lost_focus(false);
        m.draw();
        assert_eq!(m.shown(), Some((cursor::ARROW, 0)));
    }

    #[test]
    fn mouse_ini_is_read_and_cursor_names_are_found_without_case() {
        assert_eq!(get_cursor_index(b"target"), cursor::CROSS);
        assert_eq!(get_cursor_index(b"ParticleUplinkCannon"), cursor::PARTICLE_UPLINK_CANNON);
        assert_eq!(get_cursor_index(b""), cursor::INVALID_MOUSE_CURSOR);
        assert_eq!(get_cursor_index(b"Nope"), cursor::INVALID_MOUSE_CURSOR);
        let mut m = Mouse::new();
        let ini = b"Mouse\n  CursorMode = 1\n  DragTolerance = 4\nEnd\nMouseCursor Scroll\n  Texture = SCCScroll\n  Directions = 8\n  CursorText = CURSOR:Scroll\nEnd\n";
        m.parse_ini(b"Data\\INI\\Mouse.ini", ini).expect("reads");
        assert_eq!((m.current_redraw_mode, m.settings.drag_tolerance), (redraw_mode::RM_W3D, 4));
        let s = &m.cursor_info[cursor::SCROLL as usize];
        assert_eq!((s.cursor_name.as_slice(), s.texture_name.as_slice(), s.num_directions), (&b"Scroll"[..], &b"SCCScroll"[..], 8));
        m.init();
        assert_eq!(m.current_redraw_mode, redraw_mode::RM_WINDOWS);
        // an unknown cursor's block is left unread: its fields are then block names
        assert!(Mouse::new().parse_ini(b"m", b"MouseCursor Nope\n  Texture = X\nEnd\n").is_err());
    }

    #[test]
    fn the_scroll_cursor_comes_and_goes_with_the_in_game_ui_scrolling() {
        let mut m = ready();
        m.set_visibility(true);
        let (mut mode_cursor, mut scrolling) = (cursor::NONE, false);
        in_game_ui_set_scrolling(&mut m, true, &mut mode_cursor, &mut scrolling, true);
        assert_eq!((m.current_cursor, scrolling, mode_cursor), (cursor::SCROLL, true, cursor::NONE));
        in_game_ui_set_mouse_cursor(&mut m, true, &mut mode_cursor, cursor::ATTACK_OBJECT);
        assert_eq!(mode_cursor, cursor::ATTACK_OBJECT);
        in_game_ui_set_scrolling(&mut m, true, &mut mode_cursor, &mut scrolling, false);
        assert_eq!((m.current_cursor, scrolling, mode_cursor), (cursor::ARROW, false, cursor::ATTACK_OBJECT));
    }
}
