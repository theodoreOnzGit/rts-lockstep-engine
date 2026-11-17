// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp (the field parse table, AnimTypeNames,
//     ControlBarScheme::reset, init, addAnimation, addImage, drawForeground, drawBackground; ControlBarSchemeManager's
//     parseImagePart, parseAnimatingPartImage, parseAnimatingPart, newControlBarScheme, findControlBarScheme, init,
//     setControlBarSchemeByPlayerTemplate; #146: setControlBarSchemeByPlayer; #145: the constructor's colours and
//     init's colour lines)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (#145: the constructor's colours,
//     updateBuildUpClockColor, updateCommanBarBorderColors, setControlBarSchemeByPlayer, setControlBarSchemeByPlayerTemplate)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (#145: updateBorderColor, getBorderColor, ControlBarContext)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBarScheme.h
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIControlBarScheme.cpp (INI::parseControlBarSchemeDefinition)
//   GeneralsMD/Code/GameEngine/Include/GameClient/GadgetPushButton.h (GadgetButtonSet*Image)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The control bar's look per side (GitHub #12): `ControlBarScheme.ini`'s schemes, which give the control bar its
//! background images (drawn in layers behind and in front of `ControlBar.wnd`'s windows), the positions of its fixed
//! buttons and displays (money, power, options, idle worker, ...) and their images, and its colours.
//!
//! # As upstream reads it
//!
//! `ControlBarSchemeManager::init` loads `Data\INI\Default\ControlBarScheme.ini` then `Data\INI\ControlBarScheme.ini`.
//! Each `ControlBarScheme name` block makes a scheme (or clears the one of that name) and reads its fields:
//! `ScreenCreationRes = X:800 Y:600`, `Side = America`, the many `...UL`/`...LR` corners (`X:n Y:n`), the mapped images
//! of the fixed buttons, the border colours (`R:r G:g B:b [A:a]`), and any number of
//! `ImagePart` (`Position`, `Size`, `ImageName`, `Layer`, `End`) and `AnimatingPart` (`Name`, `Animation = SLIDE_RIGHT`,
//! `Duration`, `FinalPos`, `ImagePart ... End`, `End`) blocks.
//!
//! `setControlBarSchemeByPlayerTemplate(pt)` takes the scheme of the player's side (`Side` compared without case;
//! among several, the one with the widest `ScreenCreationRes`), or `Default`, and sets the multiplier to the display's
//! size over its creation resolution. `ControlBarScheme::init` then moves and sizes the fixed windows of
//! `ControlBar.wnd` to the scheme's corners and gives the buttons their images. The background is drawn from layer 5 to
//! layer 3, the foreground from 2 to 0, each image stretched to its `Position` and `Size` times the multiplier.
//!
//! # Upstream quirks that are kept
//!
//! - A second block of the same name resets the first (its images, animations, name, side and creation resolution;
//!   **not** its colours or corners, which `reset` does not touch) and is read into it.
//! - An image on a layer outside 0..=5 goes to layer 0 (on top).
//! - An `AnimatingPart` without an `ImagePart` adds a null image, which `addImage` refuses.
//! - In `init`, a window is placed at `corner * multiplier - parent's screen position`, truncated toward zero after
//!   the subtraction (in `Real`), and sized `(LR - UL) * multiplier`, truncated.
//! - `setControlBarSchemeByPlayerTemplate` with the current scheme's side (exact case) only runs `init` again, without
//!   recomputing the multiplier.
//! - **#146:** `setControlBarSchemeByPlayer(p)` ([`ControlBarSchemeManager::set_control_bar_scheme_by_player`]) first
//!   gives the communicator button `NonCommand_BriefingHistory` outside a multiplayer game and `NonCommand_Communicator`
//!   in one ([`set_control_bar_scheme_by_player_communicator`]), even with no player; then it is the template version
//!   with the player's side and no "Small". Differential-tested (`tests/differential_scheme_by_player.rs`).
//! - **#145:** the scheme's colours: `init` gives the bar its clock, command bar border and four button border colours
//!   ([`ControlBarScheme::init_colors`] on [`ControlBarColors`]); a colour the block does not set is the scheme
//!   constructor's (the command bar border **0**, not the bar constructor's `GameMakeColor(0,0,0,100)`), so a scheme
//!   always overwrites all six. `updateCommanBarBorderColors` takes build, action, upgrade, system, in that order.
//! - **#145:** `ControlBar::setControlBarSchemeByPlayer(p)` ([`control_bar_set_control_bar_scheme_by_player`]) and
//!   `...ByPlayerTemplate(pt)` ([`control_bar_set_control_bar_scheme_by_player_template`]) after the manager's: the
//!   observer's bar for an inactive player or the `FactionObserver` template (compared as pointers: a `NULL` template is
//!   the observer's when the store has no `FactionObserver`), else the player's, with the beacon hidden outside a LAN or
//!   internet multiplayer game; then the default stage, and for the template version `hidePurchaseScience`.
//!   Differential-tested with the manager's `setControlBarSchemeByPlayerTemplate(pt, useSmall)`
//!   (`tests/differential_scheme_bar.rs`).
//!
//! # Where this differs from upstream
//!
//! - The mapped image fields keep the image's **name**, looked up in the [`ImageCollection`] when used: upstream looks
//!   it up while parsing (`INI::parseMappedImage`), which gives the same image as long as the collection is loaded
//!   first (it is: `GameClient::init` loads it before the control bar). An unknown name is `None` either way.
//! - The named images and corners are kept in maps by their field name, not as forty members.
//! - The animation (`update`, `animSlideRight`) is not translated: the parts are drawn where they start.
//!
//! # Representation
//!
//! [`ControlBarSchemeManager`] holds the schemes in file order and the current one; [`ControlBarScheme::init`] works on
//! a [`WindowTree`] read from `ControlBar.wnd`. The client's drawing (Bevy) takes [`ControlBarScheme::draw_list`].

use crate::client::image::ImageCollection;
use crate::client::window_script::{WindowTree, WinId};
use crate::common::base_type::ICoord2D;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, LookupListRec, UserData};
use crate::logic::modules::plant;
use std::collections::BTreeMap;

/// `MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS`.
pub const MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS: usize = 6;
/// `CONTROL_BAR_SCHEME_FOREGROUND_IMAGE_LAYERS`.
pub const CONTROL_BAR_SCHEME_FOREGROUND_IMAGE_LAYERS: usize = 3;
/// `COMMAND_BAR_SIZE_OFFSET`.
const COMMAND_BAR_SIZE_OFFSET: f32 = 0.0;

/// `class ControlBarSchemeImage`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControlBarSchemeImage {
    pub position: ICoord2D,
    pub size: ICoord2D,
    /// `m_image`, by name (see the module docs).
    pub image: Option<Vec<u8>>,
    pub layer: i32,
}

/// `class ControlBarSchemeAnimation`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControlBarSchemeAnimation {
    pub name: Vec<u8>,
    pub anim_type: i32,
    pub anim_image: Option<ControlBarSchemeImage>,
    pub anim_duration: u32,
    pub final_pos: ICoord2D,
}

/// `class ControlBarScheme`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControlBarScheme {
    /// `m_name`, lowercased.
    pub name: Vec<u8>,
    pub screen_creation_res: ICoord2D,
    pub side: Vec<u8>,
    /// `m_layer[0..6]`.
    pub layers: [Vec<ControlBarSchemeImage>; MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS],
    pub animations: Vec<ControlBarSchemeAnimation>,
    /// The mapped image members (`m_buttonQueueImage`, `m_optionsButtonEnable`, ...), by field name.
    pub images: BTreeMap<&'static str, Vec<u8>>,
    /// The corners (`m_moneyUL`, ...), by field name.
    pub coords: BTreeMap<&'static str, ICoord2D>,
    /// The colours (`m_buildUpClockColor`, `m_borderBuildColor`, ...), by field name.
    pub colors: BTreeMap<&'static str, i32>,
}

/// `AnimTypeNames`.
pub const ANIM_TYPE_NAMES: &[LookupListRec] = &[LookupListRec { name: "SLIDE_RIGHT", value: 0 }];

fn image_field(ini: &mut Ini<'_>, s: &mut ControlBarScheme, ud: &UserData) -> IniResult<()> {
    // INI::parseMappedImage: the token (its image looked up when used)
    let t = ini.get_next_token(None)?;
    s.images.insert(ud.names()[0], t);
    Ok(())
}
fn coord_field(ini: &mut Ini<'_>, s: &mut ControlBarScheme, ud: &UserData) -> IniResult<()> {
    let c = ini.parse_icoord2d()?;
    s.coords.insert(ud.names()[0], c);
    Ok(())
}
fn color_field(ini: &mut Ini<'_>, s: &mut ControlBarScheme, ud: &UserData) -> IniResult<()> {
    let c = ini.parse_color_int()?;
    s.colors.insert(ud.names()[0], c);
    Ok(())
}

macro_rules! named {
    ($f:ident, $($n:literal),*) => { [$(FieldParse::new($n, $f, UserData::Names(&[$n]))),*] };
}

/// `ControlBarSchemeManager::m_controlBarSchemeFieldParseTable` (in upstream's order; the images, corners and colours
/// by name).
pub fn control_bar_scheme_field_parse_table() -> Vec<FieldParse<ControlBarScheme>> {
    let mut t = vec![
        FieldParse::new("ImagePart", parse_image_part, UserData::None),
        FieldParse::new("AnimatingPart", parse_animating_part, UserData::None),
        FieldParse::new("ScreenCreationRes", |ini, s, _| ini.parse_icoord2d().map(|c| s.screen_creation_res = c), UserData::None),
        FieldParse::new("Side", |ini, s, _| ini.parse_ascii_string().map(|v| s.side = v), UserData::None),
    ];
    t.extend(named!(image_field, "QueueButtonImage", "RightHUDImage"));
    t.extend(named!(color_field, "BuildUpClockColor", "ButtonBorderBuildColor", "CommandBarBorderColor", "ButtonBorderActionColor",
        "ButtonBorderUpgradeColor", "ButtonBorderSystemColor"));
    t.extend(named!(image_field, "OptionsButtonEnable", "OptionsButtonHightlited", "OptionsButtonPushed", "OptionsButtonDisabled",
        "IdleWorkerButtonEnable", "IdleWorkerButtonHightlited", "IdleWorkerButtonPushed", "IdleWorkerButtonDisabled", "BuddyButtonEnable",
        "BuddyButtonHightlited", "BuddyButtonPushed", "BuddyButtonDisabled", "BeaconButtonEnable", "BeaconButtonHightlited",
        "BeaconButtonPushed", "BeaconButtonDisabled", "GenBarButtonIn", "GenBarButtonOn", "ToggleButtonUpIn", "ToggleButtonUpOn",
        "ToggleButtonUpPushed", "ToggleButtonDownIn", "ToggleButtonDownOn", "ToggleButtonDownPushed", "GeneralButtonEnable",
        "GeneralButtonHightlited", "GeneralButtonPushed", "GeneralButtonDisabled", "UAttackButtonEnable", "UAttackButtonHightlited",
        "UAttackButtonPushed", "GenArrow", "MinMaxButtonEnable", "MinMaxButtonHightlited", "MinMaxButtonPushed"));
    t.extend(named!(coord_field, "MinMaxUL", "MinMaxLR", "GeneralUL", "GeneralLR", "UAttackUL", "UAttackLR", "OptionsUL", "OptionsLR",
        "WorkerUL", "WorkerLR", "ChatUL", "ChatLR", "BeaconUL", "BeaconLR", "PowerBarUL", "PowerBarLR", "MoneyUL", "MoneyLR"));
    t.extend(named!(image_field, "CommandMarkerImage", "ExpBarForegroundImage", "PowerPurchaseImage"));
    t.push(FieldParse::end());
    t
}

/// The fields of an image part (`parseImagePart`'s and `parseAnimatingPartImage`'s `myFieldParse`).
const IMAGE_PART_FIELDS: &[FieldParse<ControlBarSchemeImage>] = &[
    FieldParse::new("Position", |ini, i, _| ini.parse_icoord2d().map(|c| i.position = c), UserData::None),
    FieldParse::new("Size", |ini, i, _| ini.parse_icoord2d().map(|c| i.size = c), UserData::None),
    FieldParse::new("ImageName", |ini, i, _| ini.get_next_token(None).map(|t| i.image = Some(t)), UserData::None),
    FieldParse::new("Layer", |ini, i, _| ini.parse_int().map(|v| i.layer = v), UserData::None),
    FieldParse::end(),
];

/// `ControlBarSchemeManager::parseImagePart`.
fn parse_image_part(ini: &mut Ini<'_>, s: &mut ControlBarScheme, _: &UserData) -> IniResult<()> {
    let mut image = ControlBarSchemeImage::default();
    ini.init_from_ini(&mut image, IMAGE_PART_FIELDS)?;
    s.add_image(Some(image));
    Ok(())
}

/// `parseAnimatingPart`'s `myFieldParse`.
const ANIMATING_PART_FIELDS: &[FieldParse<ControlBarSchemeAnimation>] = &[
    FieldParse::new("Name", |ini, a, _| ini.parse_ascii_string().map(|v| a.name = v), UserData::None),
    FieldParse::new("Animation", |ini, a, ud| ini.parse_lookup_list(ud.lookup()).map(|v| a.anim_type = v), UserData::Lookup(ANIM_TYPE_NAMES)),
    FieldParse::new("Duration", |ini, a, _| ini.parse_duration_unsigned_int().map(|v| a.anim_duration = v), UserData::None),
    FieldParse::new("FinalPos", |ini, a, _| ini.parse_icoord2d().map(|c| a.final_pos = c), UserData::None),
    // parseAnimatingPartImage
    FieldParse::new(
        "ImagePart",
        |ini, a, _| {
            let mut image = ControlBarSchemeImage::default();
            ini.init_from_ini(&mut image, IMAGE_PART_FIELDS)?;
            a.anim_image = Some(image);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `ControlBarSchemeManager::parseAnimatingPart`.
fn parse_animating_part(ini: &mut Ini<'_>, s: &mut ControlBarScheme, _: &UserData) -> IniResult<()> {
    let mut anim = ControlBarSchemeAnimation::default();
    ini.init_from_ini(&mut anim, ANIMATING_PART_FIELDS)?;
    let image = anim.anim_image.clone();
    s.add_animation(anim);
    s.add_image(image);
    Ok(())
}

/// One fixed window of `init`: its name, its four button images' fields (enabled, hilite, hilite selected, disabled),
/// and its corners' field prefix.
type FixedWindow = (&'static [u8], Option<[&'static str; 4]>, Option<&'static str>);

/// A placed draw: the image's name and its screen rectangle (`TheDisplay->drawImage`'s corners).
#[derive(Clone, Debug, PartialEq)]
pub struct SchemeDraw {
    pub image: Vec<u8>,
    pub layer: usize,
    pub start: (f32, f32),
    pub end: (f32, f32),
}

impl ControlBarScheme {
    /// `ControlBarScheme::reset()` (the colours and corners are not reset upstream).
    pub fn reset(&mut self) {
        self.layers = Default::default();
        self.animations.clear();
        self.name.clear();
        self.screen_creation_res = ICoord2D::default();
        self.side.clear();
        self.images.clear();
    }

    /// `addAnimation(anim)`.
    pub fn add_animation(&mut self, anim: ControlBarSchemeAnimation) {
        self.animations.push(anim);
    }

    /// `addImage(image)`: a null image is refused; a layer outside 0..6 becomes 0.
    pub fn add_image(&mut self, image: Option<ControlBarSchemeImage>) {
        let Some(mut image) = image else { return };
        if image.layer < 0 || image.layer >= MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS as i32 {
            image.layer = 0;
        }
        let l = image.layer as usize;
        self.layers[l].push(image);
    }

    /// GitHub #145: a colour member (`m_buildUpClockColor`, `m_borderBuildColor`, ...) by its field name: the INI's value,
    /// or the one `ControlBarScheme::ControlBarScheme` (ControlBarScheme.cpp:307-321) gave it: `BuildUpClockColor`
    /// `GameMakeColor(0,0,0,100)`, the four `ButtonBorder*Color`s `GAME_COLOR_UNDEFINED`, `CommandBarBorderColor` 0.
    /// (`reset` does not touch them, so a second block of the same name keeps the first's.)
    pub fn color(&self, field: &str) -> i32 {
        if let Some(c) = self.colors.get(field) {
            return *c;
        }
        match field {
            "BuildUpClockColor" => crate::client::window_transitions::game_make_color(0, 0, 0, 100),
            "CommandBarBorderColor" if plant(10403) => crate::client::window_transitions::game_make_color(0, 0, 0, 100),
            "CommandBarBorderColor" => 0,
            _ if plant(10404) => 0,
            _ => crate::client::build_tooltip::GAME_COLOR_UNDEFINED,
        }
    }

    /// GitHub #145: the colour lines of `ControlBarScheme::init()` (ControlBarScheme.cpp:432-434, inside
    /// `if(TheControlBar)`): `updateBuildUpClockColor(m_buildUpClockColor)`,
    /// `updateCommanBarBorderColors(m_borderBuildColor, m_borderActionColor, m_borderUpgradeColor, m_borderSystemColor)`
    /// and `updateBorderColor(m_commandBarBorderColor)` on the control bar's colours. (The other lines of that block, the
    /// stage, the queue's disabled images, the right HUD, the slot exit, up-down and arrow images, are not here: the stage
    /// is `crate::client::control_bar_stage`'s; the images are #145's remaining gaps.)
    pub fn init_colors(&self, bar: &mut ControlBarColors) {
        bar.update_build_up_clock_color(self.color("BuildUpClockColor"));
        bar.update_comman_bar_border_colors(self.color("ButtonBorderBuildColor"), self.color("ButtonBorderActionColor"), self.color("ButtonBorderUpgradeColor"), self.color("ButtonBorderSystemColor"));
        let border = if plant(10402) { self.color("BuildUpClockColor") } else { self.color("CommandBarBorderColor") };
        bar.update_border_color(border);
    }

    /// The image of a mapped image field, found in the collection.
    pub fn image<'c>(&self, field: &str, images: &'c ImageCollection) -> Option<&'c crate::client::image::Image> {
        self.images.get(field).and_then(|n| images.find_image_by_name(n))
    }

    /// The corner of a field (0, 0 when the file has none: the members are zero-initialised).
    pub fn coord(&self, field: &str) -> ICoord2D {
        self.coords.get(field).copied().unwrap_or_default()
    }

    /// `drawBackground(multi, offset)` then `drawForeground(multi, offset)`, as the draws they make, in order: layers 5 to
    /// 3, then 2 to 0, each layer's images in file order; images not in the collection are skipped.
    pub fn draw_list(&self, multi: (f32, f32), offset: ICoord2D, images: &ImageCollection) -> Vec<SchemeDraw> {
        let mut out = Vec::new();
        for i in (0..MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS).rev() {
            for si in &self.layers[i] {
                let Some(img) = si.image.as_ref().and_then(|n| images.find_image_by_name(n)) else { continue };
                out.push(SchemeDraw {
                    image: img.name.clone(),
                    layer: i,
                    start: (si.position.x as f32 * multi.0 + offset.x as f32, si.position.y as f32 * multi.1 + offset.y as f32),
                    end: (
                        (si.position.x + si.size.x) as f32 * multi.0 + offset.x as f32,
                        (si.position.y + si.size.y) as f32 * multi.1 + offset.y as f32,
                    ),
                });
            }
        }
        out
    }

    /// `ControlBarScheme::init()`'s placement of the fixed windows of `ControlBar.wnd` (and of
    /// `GeneralsExpPoints.wnd:GenExpParent` if it is in the tree), and the images it gives them. The control bar's own
    /// updates (`updateBuildQueueDisabledImages` and the rest, the first lines of `init`) are the control bar's.
    pub fn init(&self, tree: &mut WindowTree, display: (i32, i32), images: &ImageCollection) {
        let res = (display.0 as f32 / self.screen_creation_res.x as f32, display.1 as f32 / self.screen_creation_res.y as f32);
        let img = |f: &str| self.images.get(f).and_then(|n| images.find_image_by_name(n)).map(|i| i.name.clone());
        // (window, [enabled, hilite, hilite selected, disabled] fields, corner field prefix)
        let buttons: [FixedWindow; 8] = [
            (b"ControlBar.wnd:PopupCommunicator", Some(["BuddyButtonEnable", "BuddyButtonHightlited", "BuddyButtonPushed", "BuddyButtonDisabled"]), Some("Chat")),
            (b"ControlBar.wnd:ButtonIdleWorker", Some(["IdleWorkerButtonEnable", "IdleWorkerButtonHightlited", "IdleWorkerButtonPushed", "IdleWorkerButtonDisabled"]), Some("Worker")),
            (b"ControlBar.wnd:ButtonOptions", Some(["OptionsButtonEnable", "OptionsButtonHightlited", "OptionsButtonPushed", "OptionsButtonDisabled"]), Some("Options")),
            (b"ControlBar.wnd:ButtonPlaceBeacon", Some(["BeaconButtonEnable", "BeaconButtonHightlited", "BeaconButtonPushed", "BeaconButtonDisabled"]), Some("Beacon")),
            (b"ControlBar.wnd:MoneyDisplay", None, Some("Money")),
            (b"ControlBar.wnd:PowerWindow", None, Some("PowerBar")),
            (b"ControlBar.wnd:ButtonGeneral", Some(["GeneralButtonEnable", "GeneralButtonHightlited", "GeneralButtonPushed", "GeneralButtonDisabled"]), Some("General")),
            (b"ControlBar.wnd:ButtonLarge", None, Some("MinMax")),
        ];
        for (name, imgs, corner) in buttons {
            let Some(w) = tree.find_by_name(name) else { continue };
            if let Some([e, h, hs, d]) = imgs {
                gadget_button_set_enabled_image(tree, w, img(e));
                gadget_button_set_hilite_image(tree, w, img(h));
                gadget_button_set_hilite_selected_image(tree, w, img(hs));
                gadget_button_set_disabled_image(tree, w, img(d));
            }
            if let Some(c) = corner {
                self.place(tree, w, res, &format!("{c}UL"), &format!("{c}LR"));
            }
            // (ExpBarForeground, between ButtonIdleWorker and ButtonOptions upstream, only gets an image; no window is both)
            if name == b"ControlBar.wnd:ButtonOptions" {
                if let Some(x) = tree.find_by_name(b"ControlBar.wnd:ExpBarForeground") {
                    tree.windows[x].inst.enabled_draw_data[0].image = img("ExpBarForegroundImage");
                }
            }
        }
        if let Some(w) = tree.find_by_name(b"ControlBar.wnd:WinUAttack") {
            tree.windows[w].inst.enabled_draw_data[0].image = img("UAttackButtonEnable");
            tree.windows[w].inst.disabled_draw_data[0].image = img("UAttackButtonHightlited");
            self.place(tree, w, res, "UAttackUL", "UAttackLR");
        }
        if let Some(w) = tree.find_by_name(b"GeneralsExpPoints.wnd:GenExpParent") {
            tree.windows[w].inst.enabled_draw_data[0].image = img("PowerPurchaseImage");
            if let Some(i) = self.image("PowerPurchaseImage", images) {
                win_set_size(tree, w, (i.width() as f32 * res.0) as i32, (i.height() as f32 * res.1) as i32);
            }
        }
    }

    /// One window of `init`: `winSetPosition(UL * res - parent)` and `winSetSize((LR - UL) * res + COMMAND_BAR_SIZE_OFFSET)`.
    fn place(&self, tree: &mut WindowTree, w: WinId, res: (f32, f32), ul: &str, lr: &str) {
        let (ul, lr) = (self.coord(ul), self.coord(lr));
        let (px, py) = match tree.windows[w].parent {
            Some(p) => tree.screen_position(p),
            None => (0, 0),
        };
        let x = (ul.x as f32 * res.0 - px as f32) as i32;
        let y = (ul.y as f32 * res.1 - py as f32) as i32;
        win_set_position(tree, w, x, y);
        let wd = ((lr.x - ul.x) as f32 * res.0 + COMMAND_BAR_SIZE_OFFSET) as i32;
        let ht = ((lr.y - ul.y) as f32 * res.1 + COMMAND_BAR_SIZE_OFFSET) as i32;
        win_set_size(tree, w, wd, ht);
    }
}

/// `GameWindow::winSetPosition(x, y)`: the region from the position and the size, normalised.
pub fn win_set_position(tree: &mut WindowTree, w: WinId, x: i32, y: i32) {
    let win = &mut tree.windows[w];
    let (mut lx, mut hx, mut ly, mut hy) = (x, x.wrapping_add(win.size.0), y, y.wrapping_add(win.size.1));
    if lx > hx {
        std::mem::swap(&mut lx, &mut hx);
    }
    if ly > hy {
        std::mem::swap(&mut ly, &mut hy);
    }
    win.region = (lx, ly, hx, hy);
}

/// `GameWindow::winSetSize(width, height)` (the resize message to the window is not modelled).
pub fn win_set_size(tree: &mut WindowTree, w: WinId, width: i32, height: i32) {
    let win = &mut tree.windows[w];
    win.size = (width, height);
    win.region.2 = win.region.0.wrapping_add(width);
    win.region.3 = win.region.1.wrapping_add(height);
}

/// `GadgetButtonSetEnabledImage`: enabled 0, and clears enabled 5 and 6.
pub fn gadget_button_set_enabled_image(tree: &mut WindowTree, w: WinId, image: Option<Vec<u8>>) {
    let d = &mut tree.windows[w].inst.enabled_draw_data;
    d[0].image = image;
    d[5].image = None;
    d[6].image = None;
}
/// `GadgetButtonSetHiliteImage`: hilite 0, and clears enabled 5 and 6.
pub fn gadget_button_set_hilite_image(tree: &mut WindowTree, w: WinId, image: Option<Vec<u8>>) {
    let win = &mut tree.windows[w].inst;
    win.hilite_draw_data[0].image = image;
    win.enabled_draw_data[5].image = None;
    win.enabled_draw_data[6].image = None;
}
/// `GadgetButtonSetHiliteSelectedImage`: hilite 1, and clears enabled 2 and 3.
pub fn gadget_button_set_hilite_selected_image(tree: &mut WindowTree, w: WinId, image: Option<Vec<u8>>) {
    let win = &mut tree.windows[w].inst;
    win.hilite_draw_data[1].image = image;
    win.enabled_draw_data[2].image = None;
    win.enabled_draw_data[3].image = None;
}
/// `GadgetButtonSetDisabledImage`: disabled 0, and clears enabled 5 and 6.
pub fn gadget_button_set_disabled_image(tree: &mut WindowTree, w: WinId, image: Option<Vec<u8>>) {
    let win = &mut tree.windows[w].inst;
    win.disabled_draw_data[0].image = image;
    win.enabled_draw_data[5].image = None;
    win.enabled_draw_data[6].image = None;
}

/// `class ControlBarSchemeManager`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControlBarSchemeManager {
    /// `m_schemeList`.
    pub schemes: Vec<ControlBarScheme>,
    /// `m_currentScheme`.
    pub current: Option<usize>,
    /// `m_multiplyer`.
    pub multiplier: (f32, f32),
}

/// GitHub #146: `ControlBarSchemeManager::setControlBarSchemeByPlayer`'s first lines (ControlBarScheme.cpp:1165): the
/// window `ControlBar.wnd:PopupCommunicator`, when the layout has it (and `TheControlBar` exists, as it does in the
/// client), gets `NonCommand_Communicator` in a multiplayer game (`TheRecorder->isMultiplayer()`) and
/// `NonCommand_BriefingHistory` outside one, through `ControlBar::setControlCommand` with `findCommandButton`'s button
/// (`crate::client::build_tooltip::set_control_command`). It overrides `ControlBar::init`'s `NonCommand_Communicator`.
pub fn set_control_bar_scheme_by_player_communicator(multiplayer: bool, h: &mut impl crate::client::build_tooltip::ControlBarInitHost) {
    let Some(communicator_button) = h.win_get_window_from_id(b"ControlBar.wnd:PopupCommunicator") else { return };
    let multiplayer = multiplayer != plant(9378);
    let name: &[u8] = if multiplayer { b"NonCommand_Communicator" } else { b"NonCommand_BriefingHistory" };
    let c = h.find_command_button(name);
    crate::client::build_tooltip::set_control_command(communicator_button, c, h);
}

/// GitHub #145: `TheControlBar`'s colour members that a scheme sets (ControlBar.h:965-988): `m_buildUpClockColor` (the
/// inverse clock over a building button), `m_commandBarBorderColor` (`getBorderColor`, the command grid's lines in
/// `W3DCommandBarGridDraw`) and the four command button border colours (`setCommandBarBorder`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlBarColors {
    /// `m_buildUpClockColor`.
    pub build_up_clock: i32,
    /// `m_commandBarBorderColor`.
    pub command_bar_border: i32,
    /// `m_commandButtonBorderBuildColor`.
    pub border_build: i32,
    /// `m_commandButtonBorderActionColor`.
    pub border_action: i32,
    /// `m_commandButtonBorderUpgradeColor`.
    pub border_upgrade: i32,
    /// `m_commandButtonBorderSystemColor`.
    pub border_system: i32,
}

impl Default for ControlBarColors {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlBarColors {
    /// `ControlBar::ControlBar()`'s colours (ControlBar.cpp:899-900, 958): the clock and the command bar's border
    /// `GameMakeColor(0,0,0,100)`, then `updateCommanBarBorderColors(GAME_COLOR_UNDEFINED, ...)` for the four borders.
    pub fn new() -> Self {
        let undefined = crate::client::build_tooltip::GAME_COLOR_UNDEFINED;
        let clock = crate::client::window_transitions::game_make_color(0, 0, 0, 100);
        let mut c = ControlBarColors { build_up_clock: clock, command_bar_border: if plant(10405) { 0 } else { clock }, border_build: 0, border_action: 0, border_upgrade: 0, border_system: 0 };
        c.update_comman_bar_border_colors(undefined, undefined, undefined, undefined);
        c
    }

    /// `ControlBar::updateBuildUpClockColor(color)` (ControlBar.cpp:2904).
    pub fn update_build_up_clock_color(&mut self, color: i32) {
        self.build_up_clock = color;
    }

    /// `ControlBar::updateCommanBarBorderColors(build, action, upgrade, system)` (ControlBar.cpp:2911; upstream's
    /// spelling). The order is build, **action, upgrade**, system, unlike `setCommandBarBorder`'s switch.
    pub fn update_comman_bar_border_colors(&mut self, build: i32, action: i32, upgrade: i32, system: i32) {
        let (action, upgrade) = if plant(10401) { (upgrade, action) } else { (action, upgrade) };
        self.border_build = build;
        self.border_action = action;
        self.border_upgrade = upgrade;
        self.border_system = system;
    }

    /// `ControlBar::updateBorderColor(color)` (ControlBar.h:755).
    pub fn update_border_color(&mut self, color: i32) {
        self.command_bar_border = color;
    }

    /// `ControlBar::getBorderColor()` (ControlBar.h:754).
    pub fn get_border_color(&self) -> i32 {
        self.command_bar_border
    }

    /// The four border colours as [`crate::client::build_tooltip::ControlBarInitHost::border_colors`] gives them to
    /// `setCommandBarBorder`: build, upgrade, action, system (new glue).
    pub fn border_colors(&self) -> [i32; 4] {
        if plant(10406) {
            return [self.border_build, self.border_action, self.border_upgrade, self.border_system];
        }
        [self.border_build, self.border_upgrade, self.border_action, self.border_system]
    }
}

/// `ControlBarContext` (ControlBar.h:579): the two `setControlBarSchemeByPlayer` switches to.
pub mod context {
    /// `CB_CONTEXT_NONE`.
    pub const NONE: i32 = 0;
    /// `CB_CONTEXT_OBSERVER_LIST`.
    pub const OBSERVER_LIST: i32 = 7;
}

/// GitHub #145: what `ControlBar::setControlBarSchemeByPlayer` and `setControlBarSchemeByPlayerTemplate` reach.
pub trait SchemeByPlayerBarHost {
    /// `if(m_controlBarSchemeManager) m_controlBarSchemeManager->setControlBarSchemeByPlayer(p)`.
    fn scheme_manager_by_player(&mut self);
    /// `if(m_controlBarSchemeManager) m_controlBarSchemeManager->setControlBarSchemeByPlayerTemplate(pt)`.
    fn scheme_manager_by_player_template(&mut self);
    /// `TheWindowManager->winGetWindowFromId(NULL, NAMEKEY(name))`.
    fn win_get_window_from_id(&self, name: &[u8]) -> Option<WinId>;
    /// `window->winHide(hide)`.
    fn win_hide(&mut self, w: WinId, hide: bool);
    /// `window->winEnable(enable)`.
    fn win_enable(&mut self, w: WinId, enable: bool);
    /// `switchToContext(context, NULL)` ([`context`]).
    fn switch_to_context(&mut self, context: i32);
    /// `m_isObserverCommandBar = observer`.
    fn set_observer_command_bar(&mut self, observer: bool);
    /// `TheGameLogic->getGameMode()` (`crate::common::recorder::game_mode`).
    fn game_mode(&self) -> i32;
    /// `TheGameInfo->isMultiPlayer()`.
    fn game_info_is_multi_player(&self) -> bool;
    /// `switchControlBarStage(CONTROL_BAR_STAGE_DEFAULT)`.
    fn switch_control_bar_stage_default(&mut self);
    /// `hidePurchaseScience()`.
    fn hide_purchase_science(&mut self);
}

/// The part `ControlBar::setControlBarSchemeByPlayer` and `setControlBarSchemeByPlayerTemplate` share after the
/// manager's call (ControlBar.cpp:2755-2788 and 2800-2833): the beacon, idle worker and general's buttons looked up;
/// the observer's bar (`CB_CONTEXT_OBSERVER_LIST`, `m_isObserverCommandBar` set **before** the switch; the beacon and
/// idle worker hidden, the general's button disabled, not hidden) or the player's (`CB_CONTEXT_NONE`, the flag cleared
/// **after** the switch; the beacon hidden outside a LAN or internet game with `TheGameInfo->isMultiPlayer()`, the idle
/// worker shown, the general's button shown and enabled); then the default stage.
fn control_bar_scheme_buttons(observer: bool, h: &mut impl SchemeByPlayerBarHost) {
    use crate::common::recorder::game_mode;
    let beacon = h.win_get_window_from_id(b"ControlBar.wnd:ButtonPlaceBeacon");
    let idle_worker = h.win_get_window_from_id(b"ControlBar.wnd:ButtonIdleWorker");
    let general = h.win_get_window_from_id(b"ControlBar.wnd:ButtonGeneral");
    if observer {
        if plant(10407) {
            h.switch_to_context(context::OBSERVER_LIST);
            h.set_observer_command_bar(true);
        } else {
            h.set_observer_command_bar(true);
            h.switch_to_context(context::OBSERVER_LIST);
        }
        if let Some(w) = beacon {
            h.win_hide(w, true);
        }
        if let Some(w) = idle_worker {
            h.win_hide(w, true);
        }
        if let Some(w) = general {
            h.win_enable(w, false);
        }
    } else {
        h.switch_to_context(context::NONE);
        h.set_observer_command_bar(false);
        if let Some(w) = beacon {
            let mode = h.game_mode();
            let not_network = mode != game_mode::LAN && mode != game_mode::INTERNET;
            // (`||` short-circuits: `TheGameInfo` is not asked outside a LAN or internet game)
            let hide = if plant(10408) { not_network && !h.game_info_is_multi_player() } else if plant(10415) { not_network } else { not_network || !h.game_info_is_multi_player() };
            h.win_hide(w, hide);
        }
        if let Some(w) = idle_worker {
            h.win_hide(w, false);
        }
        if let Some(w) = general {
            if !plant(10409) {
                h.win_hide(w, false);
            }
            h.win_enable(w, true);
        }
    }
    h.switch_control_bar_stage_default();
}

/// GitHub #145: `ControlBar::setControlBarSchemeByPlayer(p)` (ControlBar.cpp:2750): the manager's
/// ([`set_control_bar_scheme_by_player_communicator`] and [`ControlBarSchemeManager::set_control_bar_scheme_by_player`],
/// which the host runs), then the observer's bar when `!p->isPlayerActive()` (`player_active`: an observer or a dead
/// player), else the player's ([`control_bar_scheme_buttons`]). Upstream dereferences `p` unchecked after the manager's
/// call, so a `NULL` player crashes there; every caller passes the local player.
pub fn control_bar_set_control_bar_scheme_by_player(player_active: bool, h: &mut impl SchemeByPlayerBarHost) {
    h.scheme_manager_by_player();
    control_bar_scheme_buttons(!player_active, h);
}

/// GitHub #145: `ControlBar::setControlBarSchemeByPlayerTemplate(pt)` (ControlBar.cpp:2795): the manager's
/// ([`ControlBarSchemeManager::set_control_bar_scheme_by_player_template`], which the host runs), then the observer's
/// bar when `pt` is `ThePlayerTemplateStore->findPlayerTemplate(NAMEKEY("FactionObserver"))` (`observer_template`;
/// templates are compared as pointers, so a `NULL` `pt` with no `FactionObserver` template is the observer's bar),
/// else the player's, then `hidePurchaseScience()`.
pub fn control_bar_set_control_bar_scheme_by_player_template(template: Option<usize>, observer_template: Option<usize>, h: &mut impl SchemeByPlayerBarHost) {
    h.scheme_manager_by_player_template();
    let observer = if plant(10412) { template.is_some() && template == observer_template } else { template == observer_template };
    control_bar_scheme_buttons(observer, h);
    if !plant(10410) {
        h.hide_purchase_science();
    }
}

/// The `ControlBarScheme` block of the INI.
fn parse_control_bar_scheme_definition(ini: &mut Ini<'_>, m: &mut ControlBarSchemeManager) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let Some(i) = m.new_control_bar_scheme(&name) else { return Ok(()) };
    let table = control_bar_scheme_field_parse_table();
    ini.init_from_ini(&mut m.schemes[i], &table)
}

/// The block table of the scheme files.
pub const CONTROL_BAR_SCHEME_BLOCKS: &[BlockParse<ControlBarSchemeManager>] = &[BlockParse { token: "ControlBarScheme", parse: parse_control_bar_scheme_definition }];

impl ControlBarSchemeManager {
    /// `ControlBarSchemeManager()`.
    pub fn new() -> Self {
        ControlBarSchemeManager { schemes: Vec::new(), current: None, multiplier: (1.0, 1.0) }
    }

    /// `newControlBarScheme(name)`: the scheme of that name reset, or a new one (`None` for an empty name).
    pub fn new_control_bar_scheme(&mut self, name: &[u8]) -> Option<usize> {
        if let Some(i) = self.find_control_bar_scheme(name) {
            let s = &mut self.schemes[i];
            s.reset();
            s.name = name.to_ascii_lowercase();
            return Some(i);
        }
        if name.is_empty() {
            return None;
        }
        self.schemes.push(ControlBarScheme { name: name.to_ascii_lowercase(), ..Default::default() });
        Some(self.schemes.len() - 1)
    }

    /// `findControlBarScheme(name)`, without case.
    pub fn find_control_bar_scheme(&self, name: &[u8]) -> Option<usize> {
        self.schemes.iter().position(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// `INI::load` of one scheme file.
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, CONTROL_BAR_SCHEME_BLOCKS, self)
    }

    /// `init()`: the two scheme files, each if present.
    pub fn init(&mut self, open: impl Fn(&[u8]) -> Option<Vec<u8>>) -> IniResult<()> {
        for p in [b"Data\\INI\\Default\\ControlBarScheme.ini".as_slice(), b"Data\\INI\\ControlBarScheme.ini"] {
            if let Some(t) = open(p) {
                self.load_text(p, &t)?;
            }
        }
        Ok(())
    }

    /// `setControlBarSchemeByPlayerTemplate(pt, useSmall)` with the template's side: the scheme chosen and its `init`
    /// on `tree`.
    pub fn set_control_bar_scheme_by_side(&mut self, side: &[u8], use_small: bool, tree: &mut WindowTree, display: (i32, i32), images: &ImageCollection) {
        if let Some(c) = self.set_control_bar_scheme_by_player_template(Some(side), use_small, display) {
            self.schemes[c].init(tree, display, images);
        }
    }

    /// GitHub #145: `ControlBarSchemeManager::setControlBarSchemeByPlayerTemplate(pt, useSmall)` (ControlBarScheme.cpp:1102)
    /// up to its `init`: no template (`template_side` `None`), nothing; else the template's side (`pt->getSide()`) with
    /// "Small" appended when `use_small`, as [`Self::choose_scheme_by_side`]. The scheme whose `init` runs next.
    pub fn set_control_bar_scheme_by_player_template(&mut self, template_side: Option<&[u8]>, use_small: bool, display: (i32, i32)) -> Option<usize> {
        let mut side = match template_side {
            Some(s) => s.to_vec(),
            None if plant(10411) => Vec::new(),
            None => return None,
        };
        if use_small && !plant(10414) {
            side.extend_from_slice(if plant(10413) { b"small" } else { b"Small" });
        }
        self.choose_scheme_by_side(side, display)
    }

    /// The part `setControlBarSchemeByPlayerTemplate` and (GitHub #146) `setControlBarSchemeByPlayer` share, from the
    /// side on: the current scheme kept when its side is the same (exact case), else the scheme of that side (compared
    /// without case; an empty side is `Observer`; among several, the one with the widest `ScreenCreationRes`, the first of
    /// equals) with the multiplier set to the display over its creation resolution, else `Default`. The scheme whose
    /// `init` runs next (`None`: no current scheme).
    pub fn choose_scheme_by_side(&mut self, mut side: Vec<u8>, display: (i32, i32)) -> Option<usize> {
        if let Some(c) = self.current {
            let same = if plant(9380) { self.schemes[c].side.eq_ignore_ascii_case(&side) } else { self.schemes[c].side == side };
            if same {
                return Some(c);
            }
        }
        // "if we don't have a side, set it to Observer shell"
        if side.is_empty() && !plant(9381) {
            side = b"Observer".to_vec();
        }
        let mut temp: Option<usize> = None;
        for (i, s) in self.schemes.iter().enumerate() {
            let wider = |t: usize| if plant(9382) { self.schemes[t].screen_creation_res.x <= s.screen_creation_res.x } else { self.schemes[t].screen_creation_res.x < s.screen_creation_res.x };
            if s.side.eq_ignore_ascii_case(&side) && temp.is_none_or(wider) {
                temp = Some(i);
            }
        }
        match temp {
            Some(t) => {
                // "setup the multiplyer value"
                let r = self.schemes[t].screen_creation_res;
                let y = if plant(9383) { display.0 } else { display.1 };
                self.multiplier = (display.0 as f32 / r.x as f32, y as f32 / r.y as f32);
                self.current = Some(t);
            }
            None => {
                // "well, we couldn't find"
                self.current = if plant(9384) { None } else { self.find_control_bar_scheme(b"Default") };
            }
        }
        self.current
    }

    /// GitHub #146: `ControlBarSchemeManager::setControlBarSchemeByPlayer(p)` (ControlBarScheme.cpp:1163) after its
    /// communicator switch ([`set_control_bar_scheme_by_player_communicator`], which needs the bar's windows): no player,
    /// nothing; else the scheme of the player's side (`p->getSide()`, never "Small": `useSmall` is the template
    /// version's), as [`Self::choose_scheme_by_side`]. The scheme whose `init` runs next.
    pub fn set_control_bar_scheme_by_player(&mut self, player_side: Option<&[u8]>, display: (i32, i32)) -> Option<usize> {
        let side = match player_side {
            Some(s) => s.to_vec(),
            None if plant(9379) => Vec::new(),
            None => return None,
        };
        self.choose_scheme_by_side(side, display)
    }

    /// The current scheme.
    pub fn current_scheme(&self) -> Option<&ControlBarScheme> {
        self.current.map(|c| &self.schemes[c])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::image::Image;
    use crate::client::window_script::{tests::Host, WindowScript};

    /// #12: a scheme. Predicted from ControlBarScheme.cpp before the run:
    /// - two schemes `America8x6` (800x600) and `America16x12` (1600x1200), side `America`: at a 1600x1200 display the
    ///   wider one is chosen, multiplier (1, 1); a `Default` scheme is chosen for side `China` (none);
    /// - `America16x12`'s three image parts: layer 4 (`ImgBack`), layer 9 (to layer 0, `ImgTop`), layer 1 with no such
    ///   image (skipped when drawn); an animating part's image on layer 5 (`ImgAnim`); the draw list: layer 5's `ImgAnim`,
    ///   layer 4's `ImgBack`, layer 0's `ImgTop`, in that order, `ImgBack` from (10, 900) to (110, 950);
    /// - init on the control-bar-like layout (`MoneyDisplay` is a child of the parent at screen (0, 896)): `MoneyUL`
    ///   X:100 Y:1000, `MoneyLR` X:200 Y:1030 gives the money display region (100, 104)..(200, 134), size (100, 30);
    /// - the colour `BuildUpClockColor = R:1 G:2 B:3` is alpha 255: 0xFF010203.
    /// #145: the colours. Predicted from ControlBar.cpp and ControlBarScheme.cpp before the run: the bar's constructor
    /// gives the clock and the command bar border 0x64000000 and the four borders 0x00FFFFFF; a scheme with only
    /// `ButtonBorderUpgradeColor = R:1 G:2 B:3` gives the bar upgrade 0xFF010203, the other borders 0x00FFFFFF, the clock
    /// 0x64000000 and the command bar border **0** (the scheme's constructor, not the bar's); `border_colors` is build,
    /// upgrade, action, system. The template version with `useSmall` takes `AmericaSmall`; with no template, nothing.
    #[test]
    fn the_scheme_gives_the_bar_its_colours() {
        let mut bar = ControlBarColors::new();
        assert_eq!(bar, ControlBarColors { build_up_clock: 0x6400_0000, command_bar_border: 0x6400_0000, border_build: 0x00FF_FFFF, border_action: 0x00FF_FFFF, border_upgrade: 0x00FF_FFFF, border_system: 0x00FF_FFFF });
        let ini = b"ControlBarScheme America\n  ScreenCreationRes = X:800 Y:600\n  Side = America\n  ButtonBorderUpgradeColor = R:1 G:2 B:3\nEnd\n\
ControlBarScheme AmericaSmall\n  ScreenCreationRes = X:800 Y:600\n  Side = AmericaSmall\nEnd\n";
        let mut m = ControlBarSchemeManager::new();
        m.load_text(b"ControlBarScheme.ini", ini).expect("parse");
        assert_eq!(m.set_control_bar_scheme_by_player_template(None, false, (800, 600)), None);
        let c = m.set_control_bar_scheme_by_player_template(Some(b"America"), false, (800, 600)).expect("America");
        m.schemes[c].init_colors(&mut bar);
        assert_eq!((bar.build_up_clock, bar.get_border_color()), (0x6400_0000, 0));
        assert_eq!(bar.border_colors(), [0x00FF_FFFF, 0xFF01_0203u32 as i32, 0x00FF_FFFF, 0x00FF_FFFF]);
        assert_eq!(m.set_control_bar_scheme_by_player_template(Some(b"America"), true, (800, 600)), Some(1));
    }

    #[test]
    fn a_scheme_reads_and_places_as_upstream() {
        let ini = b"ControlBarScheme America8x6\n  ScreenCreationRes = X:800 Y:600\n  Side = America\nEnd\n\
ControlBarScheme America16x12\n  ScreenCreationRes = X:1600 Y:1200\n  Side = America\n  BuildUpClockColor = R:1 G:2 B:3\n\
  MoneyUL = X:100 Y:1000\n  MoneyLR = X:200 Y:1030\n\
  ImagePart\n    Position = X:10 Y:900\n    Size = X:100 Y:50\n    ImageName = ImgBack\n    Layer = 4\n  End\n\
  ImagePart\n    Position = X:0 Y:0\n    Size = X:1 Y:1\n    ImageName = ImgTop\n    Layer = 9\n  End\n\
  ImagePart\n    ImageName = Missing\n    Layer = 1\n  End\n\
  AnimatingPart\n    Name = Slide\n    Animation = SLIDE_RIGHT\n    Duration = 1000\n    FinalPos = X:5 Y:5\n\
    ImagePart\n      ImageName = ImgAnim\n      Layer = 5\n    End\n  End\nEnd\n\
ControlBarScheme Default\n  ScreenCreationRes = X:800 Y:600\n  Side = Observer\nEnd\n";
        let mut m = ControlBarSchemeManager::new();
        m.load_text(b"ControlBarScheme.ini", ini).expect("parse");
        let mut images = ImageCollection::new();
        for n in ["ImgBack", "ImgTop", "ImgAnim"] {
            images.add_image(Image::new(n.as_bytes()));
        }
        let mut tree = WindowTree::default();
        WindowScript::new()
            .win_create_from_script(crate::client::window_script::tests::CONTROL_BAR_LIKE, &mut tree, &mut Host::default())
            .expect("layout");
        m.set_control_bar_scheme_by_side(b"America", false, &mut tree, (1600, 1200), &images);
        assert_eq!((m.current, m.multiplier), (Some(1), (1.0, 1.0)));
        let s = m.current_scheme().unwrap();
        assert_eq!(s.colors.get("BuildUpClockColor"), Some(&(0xFF010203u32 as i32)));
        let d = s.draw_list(m.multiplier, ICoord2D::default(), &images);
        let names: Vec<&[u8]> = d.iter().map(|x| x.image.as_slice()).collect();
        assert_eq!(names, vec![&b"ImgAnim"[..], b"ImgBack", b"ImgTop"]);
        assert_eq!((d[1].start, d[1].end), ((10.0, 900.0), (110.0, 950.0)));
        let money = tree.find_by_name(b"ControlBar.wnd:MoneyDisplay").unwrap();
        assert_eq!((tree.windows[money].region, tree.windows[money].size), ((100, 104, 200, 134), (100, 30)));
        m.set_control_bar_scheme_by_side(b"China", false, &mut tree, (1600, 1200), &images);
        assert_eq!(m.current, Some(2));
    }
}
