// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp (the field parse table, AnimTypeNames,
//     ControlBarScheme::reset, init, addAnimation, addImage, drawForeground, drawBackground; ControlBarSchemeManager's
//     parseImagePart, parseAnimatingPartImage, parseAnimatingPart, newControlBarScheme, findControlBarScheme, init,
//     setControlBarSchemeByPlayerTemplate)
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
        let mut side = side.to_vec();
        if use_small {
            side.extend_from_slice(b"Small");
        }
        if let Some(c) = self.current {
            if self.schemes[c].side == side {
                self.schemes[c].init(tree, display, images);
                return;
            }
        }
        if side.is_empty() {
            side = b"Observer".to_vec();
        }
        let mut temp: Option<usize> = None;
        for (i, s) in self.schemes.iter().enumerate() {
            if s.side.eq_ignore_ascii_case(&side) && temp.is_none_or(|t| self.schemes[t].screen_creation_res.x < s.screen_creation_res.x) {
                temp = Some(i);
            }
        }
        match temp {
            Some(t) => {
                let r = self.schemes[t].screen_creation_res;
                self.multiplier = (display.0 as f32 / r.x as f32, display.1 as f32 / r.y as f32);
                self.current = Some(t);
            }
            None => self.current = self.find_control_bar_scheme(b"Default"),
        }
        if let Some(c) = self.current {
            self.schemes[c].init(tree, display, images);
        }
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
