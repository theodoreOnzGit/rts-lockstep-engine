// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/LobbyUtils.cpp (playerTemplateComboBoxTooltip,
//     playerTemplateListBoxTooltip)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The faction combo boxes' tooltips (GitHub #94, #157): LobbyUtils.cpp's two tooltip callbacks, which the skirmish
//! screen (and upstream's LAN and online screens) sets on each slot's faction combo box and its list. The rest of
//! LobbyUtils.cpp is the online lobby (GameSpy), not part of the client.
//!
//! # As upstream reads it
//!
//! **`playerTemplateComboBoxTooltip`**: the combo box's selected entry's item data is the template's index (-1 for
//! *Random*); the tooltip is `TOOLTIP:BioStrategyLong_Random`, or the template's `getTooltip()` label fetched (an index
//! the store lacks: an empty tooltip). **`playerTemplateListBoxTooltip`**: the same for the list's cell under the mouse
//! (nothing over no cell), shown at once (delay 0).
//!
//! # Upstream quirks that are kept
//!
//! - The combo box's `GadgetComboBoxGetSelectedPos` leaves `index` at 0 when nothing answers.
//! - A cell whose item data is 0 is the store's first template (0 is a valid index), not *Random*.

use crate::client::gadget_combo_box as cb;
use crate::client::gadget_list_box as lb;
use crate::client::skirmish_game_options::SkirmishHost;
use crate::client::window_manager::GameWindowManager;
use crate::client::window_script::WinId;

/// The tooltip's label for a template index (`templateNum`): `None` for an index the store lacks.
pub fn player_template_tooltip_label<H: SkirmishHost>(h: &H, template_num: i32) -> Option<Vec<u8>> {
    if template_num == -1 {
        return Some(b"TOOLTIP:BioStrategyLong_Random".to_vec());
    }
    let t = usize::try_from(template_num).ok().and_then(|i| h.util_data().templates.get(i))?;
    Some(t.tooltip.clone())
}

/// `playerTemplateComboBoxTooltip(wndComboBox, instData, mouse)`.
pub fn player_template_combo_box_tooltip<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, combo: WinId, _mouse: u32) {
    let index = cb::get_selected_pos(wm, h, combo, 0);
    let template_num = cb::get_item_data(wm, h, combo, index) as i32;
    let t = player_template_tooltip_label(h, template_num).map_or_else(Vec::new, |l| h.fetch_text(&l));
    wm.mouse.set_cursor_tooltip(&t, -1);
}

/// `playerTemplateListBoxTooltip(wndListBox, instData, mouse)`.
pub fn player_template_list_box_tooltip<H: SkirmishHost>(wm: &mut GameWindowManager, h: &mut H, list: WinId, mouse: u32) {
    // (`LOLONGTOSHORT(mouse)`, `HILONGTOSHORT(mouse)`: masked, not sign-extended)
    let x = (mouse & 0x0000_FFFF) as i32;
    let y = ((mouse & 0xFFFF_0000) >> 16) as i32;
    let (row, col) = lb::get_entry_based_on_xy(wm, h, list, x, y).unwrap_or((-1, -1));
    if row == -1 || col == -1 {
        return;
    }
    let template_num = lb::get_item_data(wm, h, list, row, col) as i32;
    let t = player_template_tooltip_label(h, template_num).map_or_else(Vec::new, |l| h.fetch_text(&l));
    // "use no tooltip delay here"
    wm.mouse.set_cursor_tooltip(&t, 0);
}
