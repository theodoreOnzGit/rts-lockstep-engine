// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (assignDefaultGadgetLook, winFindImage)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetListBox.cpp (GadgetListBoxSetColors)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetComboBox.cpp (GadgetComboBoxSetColors)
//   GeneralsMD/Code/GameEngine/Include/GameClient/{GadgetPushButton.h, GadgetCheckBox.h, GadgetRadioButton.h,
//     GadgetSlider.h, GadgetListBox.h, GadgetComboBox.h, GadgetTextEntry.h, GadgetStaticText.h,
//     GadgetProgressBar.h} (the inline setters: which state and draw data index each one writes)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `assignDefaultGadgetLook` (GitHub #72): the font every gadget gets when it is made, and, when the maker asks for
//! the **default visual**, the colours, images and text colours a gadget made by code has.
//!
//! # As upstream reads it
//!
//! The font: the one given, else the language's default window font, else Times New Roman 14 (`winSetFont`, which
//! passes it on to a list box's cells, a combo box's list and edit box, a text entry's strings). With
//! `defaultVisual` TRUE the gadget then gets, by its style (push button, check box, radio button, horizontal slider,
//! vertical slider, list box, combo box, progress bar, static text, entry field, in that order of tests; a tab
//! control gets nothing): fixed colours (red, light red, yellow, white, ...) in its draw data, images by name
//! (`PushButtonEnabled`, `ListBoxEnabledSelectedItemLeftEnd`, ...; `NULL` where the image collection has none), the
//! text colours white on dark grey (enabled), dark grey on black (disabled) and light blue on blue (hilite), and its
//! inner windows' (a slider's thumb, a list box's slider and buttons, a combo box's three children) theirs.
//!
//! The scripts never ask for it (`createGadget` passes FALSE); the inner windows the gadgets make do
//! (`gogoGadgetPushButton(..., TRUE)` for the list box's scroll buttons and the sliders' thumbs, `gogoGadgetSlider(...,
//! TRUE)` for the list box's slider, the combo box's drop-down button). A script's part draw data then replace all
//! nine draw data of each state of those, but not their text colours.
//!
//! # Upstream quirks that are kept
//!
//! - The push button's image setters also clear two images of the **enabled** state, whatever state they set:
//!   `GadgetButtonSet*Image` clears enabled 5 and 6, `GadgetButtonSet*SelectedImage` enabled 2 and 3.
//! - The slider's thumb colours: the selected ones are the slider's colours swapped (border as colour).
//! - The combo box's colours give its **edit box** push button colours (`GadgetButtonSet*Color`: indices 0 and 1).
//! - A list box's slider and buttons get the list box's colours (`GadgetListBoxSetColors`), the buttons' selected
//!   colours from the slider's thumb's.
//! - Text colours set on a combo box are also set on its list box and edit box (`winSet*TextColors`).
//!
//! # Where this differs from upstream
//!
//! The IME composite text colours (`winSetIMECompositeTextColors`) are not modelled: nothing draws them without an
//! IME (and `WinInstanceData::init` leaves them unset). The colours are made once (`static Color`), as here: they
//! are constants. An image is its name, kept when the host has it ([`GuiHost::has_image`]).

use crate::client::gadget_list_box::listbox_rt;
use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::client::window_script::{gws, GameFont, WinId, WIN_COLOR_UNDEFINED};

/// `winMakeColor(r, g, b, 255)`.
const fn rgb(r: u32, g: u32, b: u32) -> i32 {
    (0xFF00_0000 | (r << 16) | (g << 8) | b) as i32
}

const RED: i32 = rgb(255, 0, 0);
const DARK_RED: i32 = rgb(128, 0, 0);
const LIGHT_RED: i32 = rgb(255, 128, 128);
const GREEN: i32 = rgb(0, 255, 0);
const DARK_GREEN: i32 = rgb(0, 128, 0);
const LIGHT_GREEN: i32 = rgb(128, 255, 128);
const BLUE: i32 = rgb(0, 0, 255);
const LIGHT_BLUE: i32 = rgb(128, 128, 255);
const YELLOW: i32 = rgb(255, 255, 0);
const GRAY: i32 = rgb(128, 128, 128);
const DARK_GRAY: i32 = rgb(64, 64, 64);
const LIGHT_GRAY: i32 = rgb(192, 192, 192);
const BLACK: i32 = rgb(0, 0, 0);
const WHITE: i32 = rgb(254, 254, 254);

/// The states of the draw data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum St {
    E,
    D,
    H,
}

fn dd(wm: &mut GameWindowManager, w: WinId, st: St, i: usize) -> &mut crate::client::window_script::WinDrawData {
    let inst = &mut wm.tree.windows[w].inst;
    match st {
        St::E => &mut inst.enabled_draw_data[i],
        St::D => &mut inst.disabled_draw_data[i],
        St::H => &mut inst.hilite_draw_data[i],
    }
}

/// `winSet{Enabled,Disabled,Hilite}Image(i, winFindImage(name))`.
fn image<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId, st: St, i: usize, name: &str) {
    let found = h.has_image(name.as_bytes()).then(|| name.as_bytes().to_vec());
    dd(wm, w, st, i).image = found;
}
fn color(wm: &mut GameWindowManager, w: WinId, st: St, i: usize, c: i32) {
    dd(wm, w, st, i).color = c;
}
fn border(wm: &mut GameWindowManager, w: WinId, st: St, i: usize, c: i32) {
    dd(wm, w, st, i).border_color = c;
}
fn get_color(wm: &GameWindowManager, w: WinId, st: St, i: usize) -> i32 {
    let inst = &wm.tree.windows[w].inst;
    match st {
        St::E => inst.enabled_draw_data[i].color,
        St::D => inst.disabled_draw_data[i].color,
        St::H => inst.hilite_draw_data[i].color,
    }
}
fn get_border(wm: &GameWindowManager, w: WinId, st: St, i: usize) -> i32 {
    let inst = &wm.tree.windows[w].inst;
    match st {
        St::E => inst.enabled_draw_data[i].border_color,
        St::D => inst.disabled_draw_data[i].border_color,
        St::H => inst.hilite_draw_data[i].border_color,
    }
}

/// `GadgetButtonSet{state}Image` (`selected` false) / `Set{state}SelectedImage` (true): the image, and two
/// **enabled** images cleared.
fn button_image<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId, st: St, selected: bool, name: &str) {
    image(wm, h, w, st, usize::from(selected), name);
    let (a, b) = if selected { (2, 3) } else { (5, 6) };
    dd(wm, w, St::E, a).image = None;
    dd(wm, w, St::E, b).image = None;
}

/// `GadgetButtonSet{state}{,Selected}{Color,BorderColor}`.
fn button_colors(wm: &mut GameWindowManager, w: WinId, st: St, selected: bool, c: i32, b: i32) {
    color(wm, w, st, usize::from(selected), c);
    border(wm, w, st, usize::from(selected), b);
}

/// The slider's thumb (`winGetChild`), if any.
fn thumb(wm: &GameWindowManager, s: WinId) -> Option<WinId> {
    wm.tree.windows[s].children.first().copied()
}

/// The text colours every visual gives: enabled white / dark grey, disabled dark grey / black, hilite light blue /
/// blue (and the IME's, not modelled).
fn text_colors(wm: &mut GameWindowManager, w: WinId) {
    let hilite_border = if crate::logic::modules::plant(4708) { rgb(0, 0, 128) } else { BLUE };
    wm.win_set_text_colors(w, 0, WHITE, DARK_GRAY);
    wm.win_set_text_colors(w, 1, DARK_GRAY, BLACK);
    wm.win_set_text_colors(w, 2, LIGHT_BLUE, hilite_border);
}

/// `assignDefaultGadgetLook(gadget, defaultFont, assignVisual)`.
pub fn assign_default_gadget_look<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, font: Option<GameFont>, visual: bool) {
    // the font: the one given, else ~~(no language data here) Times New Roman 14~~ (corrected 2026-11-12, #151) the
    // language's `m_defaultWindowFont`, or Times New Roman 14 without one (`GameWindowManager::win_default_font`)
    let font = font.unwrap_or_else(|| wm.win_default_font(9422));
    wm.win_set_font(w, Some(font));
    if !visual {
        return;
    }
    let style = wm.tree.windows[w].inst.style;
    if style & gws::PUSH_BUTTON != 0 {
        push_button(wm, h, w);
    } else if style & gws::CHECK_BOX != 0 {
        check_like(wm, h, w, "CheckBox", [(St::E, RED, LIGHT_RED, WIN_COLOR_UNDEFINED, LIGHT_BLUE, BLUE, LIGHT_BLUE), (St::D, GRAY, LIGHT_GRAY, WIN_COLOR_UNDEFINED, LIGHT_GRAY, DARK_GRAY, WHITE), (St::H, GREEN, LIGHT_GREEN, WIN_COLOR_UNDEFINED, LIGHT_BLUE, YELLOW, WHITE)]);
    } else if style & gws::RADIO_BUTTON != 0 {
        check_like(wm, h, w, "RadioButton", [(St::E, RED, LIGHT_RED, DARK_RED, BLACK, BLUE, LIGHT_BLUE), (St::D, GRAY, LIGHT_GRAY, GRAY, LIGHT_GRAY, DARK_GRAY, WHITE), (St::H, GREEN, LIGHT_GREEN, DARK_GREEN, LIGHT_GREEN, YELLOW, WHITE)]);
    } else if style & gws::HORZ_SLIDER != 0 {
        slider(wm, h, w, "HSlider", ["LeftEnd", "RightEnd"]);
    } else if style & gws::VERT_SLIDER != 0 {
        slider(wm, h, w, "VSlider", ["TopEnd", "BottomEnd"]);
    } else if style & gws::SCROLL_LISTBOX != 0 {
        list_box_set_colors(wm, w);
        list_box_images(wm, h, w);
        if let Some(l) = listbox_rt(wm, w).cloned() {
            if let Some(s) = l.slider {
                scroll_bar_images(wm, h, s, l.up_button, l.down_button);
            }
        }
    } else if style & gws::COMBO_BOX != 0 {
        combo_box(wm, h, w);
    } else if style & gws::PROGRESS_BAR != 0 {
        progress_bar(wm, h, w);
    } else if style & gws::STATIC_TEXT != 0 {
        for (st, n, c, b) in [(St::E, "Enabled", RED, LIGHT_RED), (St::D, "Disabled", DARK_GRAY, LIGHT_GRAY), (St::H, "Hilite", DARK_GREEN, LIGHT_GREEN)] {
            image(wm, h, w, st, 0, &format!("StaticText{n}"));
            color(wm, w, st, 0, c);
            border(wm, w, st, 0, b);
        }
        text_colors(wm, w);
    } else if style & gws::ENTRY_FIELD != 0 {
        for (st, n, c, b) in [(St::E, "Enabled", RED, LIGHT_RED), (St::D, "Disabled", GRAY, BLACK), (St::H, "Hilite", GREEN, DARK_GREEN)] {
            text_entry_images(wm, h, w, st, n);
            color(wm, w, st, 0, c);
            border(wm, w, st, 0, b);
        }
        text_colors(wm, w);
    }
}

fn state_name(st: St) -> &'static str {
    match st {
        St::E => "Enabled",
        St::D => "Disabled",
        St::H => "Hilite",
    }
}

/// The push button's branch.
fn push_button<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId) {
    for (st, c, b, sc, sb) in [(St::E, RED, LIGHT_RED, YELLOW, WHITE), (St::D, GRAY, LIGHT_GRAY, LIGHT_GRAY, GRAY), (St::H, GREEN, DARK_GREEN, YELLOW, WHITE)] {
        let n = state_name(st);
        button_image(wm, h, w, st, false, &format!("PushButton{n}"));
        button_colors(wm, w, st, false, c, b);
        button_image(wm, h, w, st, true, &format!("PushButton{n}Selected"));
        button_colors(wm, w, st, true, sc, sb);
    }
    text_colors(wm, w);
}

/// The check box's and radio button's branch: per state, the background (0), the unchecked box (1), the checked box
/// (2): `(state, colour, border, unchecked colour, unchecked border, checked colour, checked border)`.
fn check_like<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId, kind: &str, states: [(St, i32, i32, i32, i32, i32, i32); 3]) {
    for (st, c, b, uc, ub, cc, cbord) in states {
        let n = state_name(st);
        image(wm, h, w, st, 0, &format!("{kind}{n}"));
        color(wm, w, st, 0, c);
        border(wm, w, st, 0, b);
        image(wm, h, w, st, 1, &format!("{kind}{n}BoxUnselected"));
        color(wm, w, st, 1, uc);
        border(wm, w, st, 1, ub);
        image(wm, h, w, st, 2, &format!("{kind}{n}BoxSelected"));
        color(wm, w, st, 2, cc);
        border(wm, w, st, 2, cbord);
    }
    text_colors(wm, w);
}

/// The sliders' branch: four images and colours per state, the text colours, then the thumb's.
fn slider<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId, kind: &str, ends: [&str; 2]) {
    for st in [St::E, St::D, St::H] {
        let n = state_name(st);
        image(wm, h, w, st, 0, &format!("{kind}{n}{}", ends[0]));
        image(wm, h, w, st, 1, &format!("{kind}{n}{}", ends[1]));
        image(wm, h, w, st, 2, &format!("{kind}{n}RepeatingCenter"));
        image(wm, h, w, st, 3, &format!("{kind}{n}SmallRepeatingCenter"));
        color(wm, w, st, 0, RED);
        border(wm, w, st, 0, LIGHT_RED);
    }
    text_colors(wm, w);
    for (st, img, sel) in [(St::E, "ThumbEnabled", "ThumbEnabled"), (St::D, "ThumbDisabled", "ThumbDisabled"), (St::H, "ThumbHilite", "ThumbHiliteSelected")] {
        let (c, b) = (get_color(wm, w, st, 0), get_border(wm, w, st, 0));
        if let Some(t) = thumb(wm, w) {
            button_image(wm, h, t, st, false, &format!("{kind}{img}"));
            button_colors(wm, t, st, false, c, b);
            button_image(wm, h, t, st, true, &format!("{kind}{sel}"));
            button_colors(wm, t, st, true, b, c);
        }
    }
}

/// `GadgetListBoxSetColors(listbox, red, lightRed, yellow, white, gray, lightGray, lightGray, white, green, darkGreen,
/// white, darkGreen)` (the values `assignDefaultGadgetLook` gives; the combo box gives the same).
fn list_box_set_colors(wm: &mut GameWindowManager, l: WinId) {
    const COLORS: [(St, i32, i32, i32, i32); 3] = [(St::E, RED, LIGHT_RED, YELLOW, WHITE), (St::D, GRAY, LIGHT_GRAY, LIGHT_GRAY, WHITE), (St::H, GREEN, DARK_GREEN, WHITE, DARK_GREEN)];
    for (st, c, b, sc, sb) in COLORS {
        color(wm, l, st, 0, c);
        border(wm, l, st, 0, b);
        color(wm, l, st, 1, sc);
        border(wm, l, st, 1, sb);
    }
    let Some(d) = listbox_rt(wm, l).cloned() else { return };
    let Some(s) = d.slider else { return };
    for st in [St::E, St::D, St::H] {
        let (c, b) = (get_color(wm, l, st, 0), get_border(wm, l, st, 0));
        color(wm, s, st, 0, c);
        border(wm, s, st, 0, b);
    }
    for button in [d.up_button, d.down_button] {
        let Some(bt) = button else { continue };
        for st in [St::E, St::D, St::H] {
            let (c, b) = (get_color(wm, s, st, 0), get_border(wm, s, st, 0));
            let (tc, tb) = thumb(wm, s).map_or((WIN_COLOR_UNDEFINED, WIN_COLOR_UNDEFINED), |t| (get_color(wm, t, st, 1), get_border(wm, t, st, 1)));
            button_colors(wm, bt, st, false, c, b);
            button_colors(wm, bt, st, true, tc, tb);
        }
    }
}

/// The list box's images: `ListBox{state}` and its selected item's four.
fn list_box_images<H: GuiHost>(wm: &mut GameWindowManager, h: &H, l: WinId) {
    for st in [St::E, St::D, St::H] {
        let n = state_name(st);
        image(wm, h, l, st, 0, &format!("ListBox{n}"));
        image(wm, h, l, st, 1, &format!("ListBox{n}SelectedItemLeftEnd"));
        image(wm, h, l, st, 2, &format!("ListBox{n}SelectedItemRightEnd"));
        image(wm, h, l, st, 3, &format!("ListBox{n}SelectedItemRepeatingCenter"));
        image(wm, h, l, st, 4, &format!("ListBox{n}SelectedItemSmallRepeatingCenter"));
    }
}

/// A list box's scroll bar images: the large vertical slider, its thumb, the up and down buttons.
fn scroll_bar_images<H: GuiHost>(wm: &mut GameWindowManager, h: &H, s: WinId, up: Option<WinId>, down: Option<WinId>) {
    for (st, thumb_img, thumb_sel) in [(St::E, "Enabled", "Enabled"), (St::D, "Disabled", "Disabled"), (St::H, "Hilite", "Hilite")] {
        let n = state_name(st);
        image(wm, h, s, st, 0, &format!("VSliderLarge{n}TopEnd"));
        image(wm, h, s, st, 1, &format!("VSliderLarge{n}BottomEnd"));
        image(wm, h, s, st, 2, &format!("VSliderLarge{n}RepeatingCenter"));
        image(wm, h, s, st, 3, &format!("VSliderLarge{n}SmallRepeatingCenter"));
        if let Some(t) = thumb(wm, s) {
            button_image(wm, h, t, st, false, &format!("VSliderLargeThumb{thumb_img}"));
            button_image(wm, h, t, st, true, &format!("VSliderLargeThumb{thumb_sel}"));
        }
    }
    for (b, dir) in [(up, "Up"), (down, "Down")] {
        let Some(b) = b else { continue };
        for (st, sel) in [(St::E, "Enabled"), (St::D, "Disabled"), (St::H, "HiliteSelected")] {
            let n = state_name(st);
            button_image(wm, h, b, st, false, &format!("VSliderLarge{dir}Button{n}"));
            button_image(wm, h, b, st, true, &format!("VSliderLarge{dir}Button{sel}"));
        }
    }
}

/// `GadgetTextEntrySet{state}Image{Left,Right,Center,SmallCenter}`.
fn text_entry_images<H: GuiHost>(wm: &mut GameWindowManager, h: &H, e: WinId, st: St, n: &str) {
    image(wm, h, e, st, 0, &format!("TextEntry{n}LeftEnd"));
    image(wm, h, e, st, 1, &format!("TextEntry{n}RightEnd"));
    image(wm, h, e, st, 2, &format!("TextEntry{n}RepeatingCenter"));
    image(wm, h, e, st, 3, &format!("TextEntry{n}SmallRepeatingCenter"));
}

/// The combo box's branch: `GadgetComboBoxSetColors`, its images, the text colours, and its children's images.
fn combo_box<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId) {
    const COLORS: [(St, i32, i32, i32, i32); 3] = [(St::E, RED, LIGHT_RED, YELLOW, WHITE), (St::D, GRAY, LIGHT_GRAY, LIGHT_GRAY, WHITE), (St::H, GREEN, DARK_GREEN, WHITE, DARK_GREEN)];
    let c = crate::client::gadget_combo_box::combo_rt(wm, w).cloned();
    // GadgetComboBoxSetColors: the combo box's, the edit box's and drop-down button's (push button indices), the list's
    for (st, col, b, sc, sb) in COLORS {
        color(wm, w, st, 0, col);
        border(wm, w, st, 0, b);
        color(wm, w, st, 1, sc);
        border(wm, w, st, 1, sb);
    }
    if let Some(c) = &c {
        for x in [c.edit_box, c.drop_down_button].into_iter().flatten() {
            for (st, col, b, sc, sb) in COLORS {
                button_colors(wm, x, st, false, col, b);
                button_colors(wm, x, st, true, sc, sb);
            }
        }
        if let Some(l) = c.list_box {
            list_box_set_colors(wm, l);
        }
    }
    list_box_images(wm, h, w);
    text_colors(wm, w);
    let Some(c) = c else { return };
    if let Some(d) = c.drop_down_button {
        for st in [St::E, St::D, St::H] {
            let n = state_name(st);
            button_image(wm, h, d, st, false, &format!("PushButton{n}"));
            button_image(wm, h, d, st, true, &format!("PushButton{n}Selected"));
        }
    }
    if let Some(e) = c.edit_box {
        for st in [St::E, St::D, St::H] {
            text_entry_images(wm, h, e, st, state_name(st));
        }
    }
    if let Some(l) = c.list_box {
        list_box_images(wm, h, l);
        if let Some(d) = listbox_rt(wm, l).cloned() {
            if let Some(s) = d.slider {
                scroll_bar_images(wm, h, s, d.up_button, d.down_button);
            }
        }
    }
}

/// The progress bar's branch: the back (0..3) and the bar (4..7) of each state.
fn progress_bar<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId) {
    for (st, c, b, bc, bb) in [(St::E, RED, LIGHT_RED, YELLOW, WHITE), (St::D, DARK_GRAY, LIGHT_GRAY, LIGHT_GRAY, WHITE), (St::H, GREEN, DARK_GREEN, YELLOW, WHITE)] {
        let n = state_name(st);
        color(wm, w, st, 0, c);
        border(wm, w, st, 0, b);
        for (i, piece) in ["LeftEnd", "RightEnd", "RepeatingCenter", "SmallRepeatingCenter"].iter().enumerate() {
            image(wm, h, w, st, i, &format!("ProgressBar{n}{piece}"));
        }
        color(wm, w, st, 4, bc);
        border(wm, w, st, 4, bb);
        for (i, piece) in ["LeftEnd", "RightEnd", "RepeatingCenter", "SmallRepeatingCenter"].iter().enumerate() {
            image(wm, h, w, st, 4 + i, &format!("ProgressBar{n}Bar{piece}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::window_manager::tests::Rec;
    use crate::client::window_script::{win_status, GadgetData, WinInstanceData};

    /// #72: the default visual of a push button. Predicted from assignDefaultGadgetLook: enabled 0 = `PushButtonEnabled`,
    /// red / light red; enabled 1 = `PushButtonEnabledSelected`, yellow / white; disabled 0 grey / light grey, 1 light
    /// grey / grey; hilite 0 green / dark green, 1 yellow / white; the enabled images 2, 3, 5, 6 cleared; the text
    /// colours white / dark grey, dark grey / black, light blue / blue; the font Times New Roman 14.
    #[test]
    fn a_push_button_gets_upstreams_default_visual() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let inst = WinInstanceData { style: gws::PUSH_BUTTON, ..WinInstanceData::new() };
        let b = wm.gogo_gadget_look(&mut h, None, win_status::ENABLED, 0, 0, 30, 20, inst, GadgetData::None, true).expect("a button");
        let i = &wm.win(b).inst;
        assert_eq!((i.enabled_draw_data[0].image.as_deref(), i.enabled_draw_data[0].color, i.enabled_draw_data[0].border_color), (Some(&b"PushButtonEnabled"[..]), RED, LIGHT_RED));
        assert_eq!((i.enabled_draw_data[1].image.as_deref(), i.enabled_draw_data[1].color, i.enabled_draw_data[1].border_color), (Some(&b"PushButtonEnabledSelected"[..]), YELLOW, WHITE));
        assert_eq!((i.disabled_draw_data[0].color, i.disabled_draw_data[1].color, i.disabled_draw_data[1].border_color), (GRAY, LIGHT_GRAY, GRAY));
        assert_eq!((i.hilite_draw_data[0].color, i.hilite_draw_data[0].border_color, i.hilite_draw_data[1].color), (GREEN, DARK_GREEN, YELLOW));
        assert!([2, 3, 5, 6].iter().all(|&k| i.enabled_draw_data[k].image.is_none()));
        assert_eq!((i.enabled_text.color, i.enabled_text.border_color), (WHITE, DARK_GRAY));
        assert_eq!((i.disabled_text.color, i.disabled_text.border_color), (DARK_GRAY, BLACK));
        assert_eq!((i.hilite_text.color, i.hilite_text.border_color), (LIGHT_BLUE, BLUE));
        assert_eq!(i.font.as_ref().map(|f| (f.name.clone(), f.point_size)), Some((b"Times New Roman".to_vec(), 14)));
        assert_eq!(RED, 0xFFFF_0000_u32 as i32);
        assert_eq!(WHITE, 0xFFFE_FEFE_u32 as i32);
    }

    /// #72: a list box made by code with the default visual, and its scroll bar. Predicted: the list's colours (red /
    /// light red, the selected item yellow / white), images `ListBoxEnabled...`; its slider gets the list's colours 0
    /// and the `VSliderLarge...` images; the slider's thumb its images `VSliderLargeThumb...`; the up button the
    /// slider's colours and, as its selected colours, the thumb's selected ones (the thumb's are the slider's own
    /// default visual's, swapped: light red / red), and `VSliderLargeUpButton...` images.
    #[test]
    fn a_list_box_and_its_scroll_bar_get_upstreams_default_visual() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let inst = WinInstanceData { style: gws::SCROLL_LISTBOX, ..WinInstanceData::new() };
        let d = crate::client::window_script::ListboxData { list_length: 5, columns: 1, scroll_bar: true, ..Default::default() };
        let l = wm.gogo_gadget_look(&mut h, None, win_status::ENABLED, 0, 0, 100, 100, inst, GadgetData::Listbox(d), true).expect("a list box");
        let lr = listbox_rt(&wm, l).expect("data").clone();
        let (s, up) = (lr.slider.expect("slider"), lr.up_button.expect("up"));
        let t = thumb(&wm, s).expect("thumb");
        let li = &wm.win(l).inst;
        assert_eq!((li.enabled_draw_data[0].color, li.enabled_draw_data[1].color, li.enabled_draw_data[1].border_color), (RED, YELLOW, WHITE));
        assert_eq!(li.enabled_draw_data[3].image.as_deref(), Some(&b"ListBoxEnabledSelectedItemRepeatingCenter"[..]));
        assert_eq!((wm.win(s).inst.hilite_draw_data[0].color, wm.win(s).inst.hilite_draw_data[0].border_color), (GREEN, DARK_GREEN));
        assert_eq!(wm.win(s).inst.disabled_draw_data[1].image.as_deref(), Some(&b"VSliderLargeDisabledBottomEnd"[..]));
        assert_eq!(wm.win(t).inst.hilite_draw_data[1].image.as_deref(), Some(&b"VSliderLargeThumbHilite"[..]));
        assert_eq!((wm.win(up).inst.enabled_draw_data[0].color, wm.win(up).inst.enabled_draw_data[1].color), (RED, LIGHT_RED));
        assert_eq!(wm.win(up).inst.hilite_draw_data[1].image.as_deref(), Some(&b"VSliderLargeUpButtonHiliteSelected"[..]));
    }

    /// #72: the other gadgets. Predicted: a vertical slider's thumb gets swapped colours when selected (enabled 1:
    /// light red / red); a check box's enabled unchecked box has no colour and a light blue border; a radio button's
    /// hilite unchecked box dark green / light green; a progress bar's disabled bar light grey / white and image
    /// `ProgressBarDisabledBarSmallRepeatingCenter` at 7; a static text's hilite dark green; an entry field's disabled
    /// grey / black; a host without images gets none (`NULL`).
    #[test]
    fn the_other_gadgets_get_upstreams_default_visual() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let mk = |wm: &mut GameWindowManager, h: &mut Rec, style: u32, data: GadgetData| wm.gogo_gadget_look(h, None, win_status::ENABLED, 0, 0, 30, 100, WinInstanceData { style, ..WinInstanceData::new() }, data, true).expect("a gadget");
        let s = mk(&mut wm, &mut h, gws::VERT_SLIDER, GadgetData::Slider(crate::client::window_script::SliderData { min_val: 0, max_val: 10 }));
        let t = thumb(&wm, s).expect("thumb");
        assert_eq!((wm.win(t).inst.enabled_draw_data[1].color, wm.win(t).inst.enabled_draw_data[1].border_color), (LIGHT_RED, RED));
        let c = mk(&mut wm, &mut h, gws::CHECK_BOX, GadgetData::None);
        assert_eq!((wm.win(c).inst.enabled_draw_data[1].color, wm.win(c).inst.enabled_draw_data[1].border_color), (WIN_COLOR_UNDEFINED, LIGHT_BLUE));
        let r = mk(&mut wm, &mut h, gws::RADIO_BUTTON, GadgetData::Radio(Default::default()));
        assert_eq!((wm.win(r).inst.hilite_draw_data[1].color, wm.win(r).inst.hilite_draw_data[1].border_color), (DARK_GREEN, LIGHT_GREEN));
        let p = mk(&mut wm, &mut h, gws::PROGRESS_BAR, GadgetData::None);
        assert_eq!((wm.win(p).inst.disabled_draw_data[4].color, wm.win(p).inst.disabled_draw_data[4].border_color), (LIGHT_GRAY, WHITE));
        assert_eq!(wm.win(p).inst.disabled_draw_data[7].image.as_deref(), Some(&b"ProgressBarDisabledBarSmallRepeatingCenter"[..]));
        let st = mk(&mut wm, &mut h, gws::STATIC_TEXT, GadgetData::Text(Default::default()));
        assert_eq!(wm.win(st).inst.hilite_draw_data[0].color, DARK_GREEN);
        let e = mk(&mut wm, &mut h, gws::ENTRY_FIELD, GadgetData::Entry(Default::default()));
        assert_eq!((wm.win(e).inst.disabled_draw_data[0].color, wm.win(e).inst.disabled_draw_data[0].border_color), (GRAY, BLACK));
        assert_eq!(wm.win(e).inst.enabled_draw_data[2].image.as_deref(), Some(&b"TextEntryEnabledRepeatingCenter"[..]));
        let mut none = crate::client::window_manager::NoGuiHost;
        let b = wm.gogo_gadget_look(&mut none, None, win_status::ENABLED, 0, 0, 30, 20, WinInstanceData { style: gws::PUSH_BUTTON, ..WinInstanceData::new() }, GadgetData::None, true).expect("a button");
        assert_eq!((wm.win(b).inst.enabled_draw_data[0].image.clone(), wm.win(b).inst.enabled_draw_data[0].color), (None, RED));
    }

    /// #72: a combo box made by code with the default visual. Predicted: its own colours as a list box's; its edit
    /// box's as a push button's (enabled 1: yellow / white) and its images `TextEntry...`; its drop-down button's
    /// images `PushButton...` (its colours were the push button's default visual and are then the combo box's);
    /// its list's colours and `ListBox...` images; the text colours on the combo box, its list and its edit box.
    #[test]
    fn a_combo_box_gets_upstreams_default_visual() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let inst = WinInstanceData { style: gws::COMBO_BOX, ..WinInstanceData::new() };
        let c = wm.gogo_gadget_look(&mut h, None, win_status::ENABLED, 0, 0, 100, 20, inst, GadgetData::ComboBox(Default::default()), true).expect("a combo box");
        let d = crate::client::gadget_combo_box::combo_rt(&wm, c).expect("data").clone();
        let (e, dd, l) = (d.edit_box.expect("e"), d.drop_down_button.expect("dd"), d.list_box.expect("l"));
        assert_eq!((wm.win(c).inst.hilite_draw_data[1].color, wm.win(c).inst.hilite_draw_data[1].border_color), (WHITE, DARK_GREEN));
        assert_eq!((wm.win(e).inst.enabled_draw_data[1].color, wm.win(e).inst.enabled_draw_data[1].border_color), (YELLOW, WHITE));
        assert_eq!(wm.win(e).inst.hilite_draw_data[3].image.as_deref(), Some(&b"TextEntryHiliteSmallRepeatingCenter"[..]));
        assert_eq!(wm.win(dd).inst.disabled_draw_data[1].image.as_deref(), Some(&b"PushButtonDisabledSelected"[..]));
        assert_eq!(wm.win(dd).inst.disabled_draw_data[1].border_color, WHITE);
        assert_eq!(wm.win(l).inst.enabled_draw_data[0].image.as_deref(), Some(&b"ListBoxEnabled"[..]));
        for x in [c, e, l] {
            assert_eq!((wm.win(x).inst.hilite_text.color, wm.win(x).inst.hilite_text.border_color), (LIGHT_BLUE, BLUE));
        }
    }
}
