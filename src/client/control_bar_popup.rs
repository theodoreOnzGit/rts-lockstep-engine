// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (ControlBar::setPortraitByObject,
//     setPortraitByImage's hidden case)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarCommand.cpp
//     (ControlBar::calculateVeterancyOverlayForObject)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarPopupDescription.cpp
//     (ControlBar::populateBuildTooltipLayout: what the popup says, for a command button and for the money and power windows;
//     #144: and the general's experience window, the comparisons that pick the window, the observer's player)
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/ProductionPrerequisite.cpp (ProductionPrerequisite::getRequiresList)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (MAX_RIGHT_HUD_UPGRADE_CAMEOS)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The selected object's portrait and the command buttons' popup description (GitHub #35).
//!
//! # As upstream does it
//!
//! `setPortraitByObject(obj)` shows `ControlBar.wnd:WinUnitSelected` with the object's template's `SelectPortrait` mapped
//! image in `ControlBar.wnd:CameoWindow`, the veterancy chevron (`SSChevron1L`..`3L`) drawn over it, and up to five upgrade
//! cameos (`ControlBar.wnd:UnitUpgrade1..5`): the template's `UpgradeCameo1..5` upgrades' button images, enabled when the
//! object or its player has the upgrade. No object, or a civilian vehicle (`KINDOF_SHOW_PORTRAIT_WHEN_CONTROLLED`) that is
//! not locally controlled, hides it all. An enemy disguised unit shows the disguise's template (and its player's upgrades).
//!
//! `populateBuildTooltipLayout(button)` fills `ControlBarPopupDescription.wnd`: the name (the button's `TextLabel`), the
//! cost (`TOOLTIP:Cost` of the thing's or upgrade's cost to build, hidden when 0) and the description (`DescriptLabel`, plus
//! the reason a unit cannot be bought, the overcharge state, the requirements, or the purchased/conflicting text of an
//! upgrade; for a purchase-science button the science's own name, description and `TOOLTIP:ScienceCost`). The money,
//! power and general's experience windows have popups of their own ([`tooltip_window_of`], [`populate_window_tooltip`],
//! #144).
//!
//! # Where this differs from upstream
//!
//! The windows are not changed in place: [`set_portrait_by_object`] and [`populate_build_tooltip_layout`] return what is
//! shown ([`Portrait`], [`Tooltip`]); the drawing applies it. ~~The popup's delay, animation, word wrap and placement
//! (`showBuildTooltipLayout`, the wrap and resize at the end of `populateBuildTooltipLayout`) are the drawing's, not
//! translated: on a phone a long press stands for the hover.~~ **Corrected 2026-11-11, #138:** the popup's delay
//! (`showBuildTooltipLayout`, `commandButtonTooltip`, `ControlBar::update`'s part) is translated in
//! `crate::client::build_tooltip` (its animation is dead code upstream); the word wrap and placement (the end of
//! `populateBuildTooltipLayout`) are still the drawing's. Upstream's popup has **no build time** line; none is added.
//! `UnicodeString::format` (`vswprintf`) is [`format_wide`], which knows the conversions the game's strings use.

use crate::client::control_bar::gui;
use crate::client::control_bar_command::ControlBarStateHost;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{KindOfType, KINDOF_SHOW_PORTRAIT_WHEN_CONTROLLED, KINDOF_STRUCTURE};
use crate::logic::command_button::CommandButton;
use crate::logic::world::build_assistant::CanMakeType;

/// `MAX_RIGHT_HUD_UPGRADE_CAMEOS` (and `MAX_UPGRADE_CAMEO_UPGRADES`).
pub const MAX_RIGHT_HUD_UPGRADE_CAMEOS: usize = 5;

/// `MAX_BUILD_QUEUE_BUTTONS`, as the popup compares the queue with it.
const MAX_BUILD_QUEUE_BUTTONS: i32 = crate::client::control_bar_command::MAX_BUILD_QUEUE_BUTTONS as i32;

/// The `getVeterancyLevel()` values the overlay reads (`LEVEL_VETERAN`, `LEVEL_ELITE`, `LEVEL_HEROIC`).
const LEVEL_VETERAN: i32 = 1;
const LEVEL_ELITE: i32 = 2;
const LEVEL_HEROIC: i32 = 3;

/// What the portrait windows show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Portrait {
    /// `WinUnitSelected` shown, with `CameoWindow`'s enabled image (`None` shown: a template without `SelectPortrait`).
    pub shown: bool,
    pub image: Option<Vec<u8>>,
    /// `GadgetButtonDrawOverlayImage(CameoWindow, calculateVeterancyOverlayForObject(obj))`.
    pub overlay: Option<Vec<u8>>,
    /// `UnitUpgrade1..5`: hidden (`None`), or the upgrade's button image and whether the window is enabled.
    pub upgrade_cameos: [Option<(Option<Vec<u8>>, bool)>; MAX_RIGHT_HUD_UPGRADE_CAMEOS],
}

/// What the portrait asks of the game.
pub trait PortraitHost {
    /// `obj->isKindOf(k)`, `isLocallyControlled()`.
    fn portrait_is_kind_of(&self, obj: ObjectId, k: KindOfType) -> bool;
    fn portrait_is_locally_controlled(&self, obj: ObjectId) -> bool;
    /// `obj->getTemplate()` (an id) and `getControllingPlayer()`.
    fn portrait_template(&self, obj: ObjectId) -> Option<u16>;
    fn portrait_controlling_player(&self, obj: ObjectId) -> Option<i32>;
    /// `draw->getStealthLook() == STEALTHLOOK_DISGUISED_ENEMY`: the drawable's template then, and (when the object's
    /// stealth update `isDisguised()`) the disguise's player index.
    fn portrait_disguised_enemy(&mut self, obj: ObjectId) -> Option<(u16, Option<i32>)>;
    /// `thing->isKindOf(k)`, `getSelectedPortraitImage()`'s name, `getUpgradeCameoName(i)`.
    fn template_is_kind_of(&self, template: u16, k: KindOfType) -> bool;
    fn template_selected_portrait(&self, template: u16) -> Vec<u8>;
    fn template_upgrade_cameo_name(&self, template: u16, i: usize) -> Vec<u8>;
    /// `TheUpgradeCenter->findUpgrade(name)`: its name key and button image's name.
    fn find_upgrade(&self, name: &[u8]) -> Option<(u32, Vec<u8>)>;
    /// `obj->hasUpgrade(ut)`, `player->hasUpgradeComplete(ut)`.
    fn portrait_obj_has_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool;
    fn portrait_player_has_upgrade_complete(&self, player: i32, upgrade: u32) -> bool;
    /// `getVeterancyLevel()`.
    fn veterancy_level(&self, obj: ObjectId) -> i32;
}

/// `ControlBar::calculateVeterancyOverlayForObject(obj)`: the chevron's mapped image name, `None` for a regular unit.
pub fn calculate_veterancy_overlay_for_object(level: i32) -> Option<&'static [u8]> {
    match level {
        LEVEL_VETERAN => Some(b"SSChevron1L"),
        LEVEL_ELITE => Some(b"SSChevron2L"),
        LEVEL_HEROIC => Some(b"SSChevron3L"),
        _ => None,
    }
}

/// `ControlBar::setPortraitByObject(obj)`.
pub fn set_portrait_by_object(obj: Option<ObjectId>, h: &mut impl PortraitHost) -> Portrait {
    // (the `else` branch: everything hidden, the overlay cleared)
    let Some(obj) = obj else { return Portrait::default() };
    if h.portrait_is_kind_of(obj, KINDOF_SHOW_PORTRAIT_WHEN_CONTROLLED) && !h.portrait_is_locally_controlled(obj) {
        // "Handles civ vehicles without terrorists in them"
        return set_portrait_by_object(None, h);
    }
    let Some(mut thing) = h.portrait_template(obj) else { return Portrait::default() };
    let mut player = h.portrait_controlling_player(obj);
    // "If we have an enemy stealth disguised unit, swap portraits!"
    if let Some((disguise, disguised_player)) = h.portrait_disguised_enemy(obj) {
        thing = disguise;
        if h.template_is_kind_of(thing, KINDOF_SHOW_PORTRAIT_WHEN_CONTROLLED) {
            // "If a bomb truck disguises as a civ vehicle, don't use it's portrait (or else you'll see the terrorist)."
            return set_portrait_by_object(None, h);
        }
        if let Some(p) = disguised_player {
            // "Fake player upgrades too!"
            player = Some(p);
        }
    }
    let image = Some(h.template_selected_portrait(thing)).filter(|n| !n.is_empty());
    let overlay = calculate_veterancy_overlay_for_object(h.veterancy_level(obj)).map(<[u8]>::to_vec);
    let mut upgrade_cameos: [Option<(Option<Vec<u8>>, bool)>; MAX_RIGHT_HUD_UPGRADE_CAMEOS] = Default::default();
    for (i, cameo) in upgrade_cameos.iter_mut().enumerate() {
        let name = h.template_upgrade_cameo_name(thing, i);
        if name.is_empty() {
            continue;
        }
        let Some((key, button_image)) = h.find_upgrade(&name) else { continue };
        // "Object level upgrades", then "Player level upgrades", else "Failure"
        let enabled = h.portrait_obj_has_upgrade(obj, key) || player.is_some_and(|p| h.portrait_player_has_upgrade_complete(p, key));
        *cameo = Some((Some(button_image).filter(|n| !n.is_empty()), enabled));
    }
    Portrait { shown: true, image, overlay, upgrade_cameos }
}

/// What the popup description shows: `StaticTextName`, `StaticTextCost` (`None`: hidden) and `StaticTextDescription`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tooltip {
    pub name: Vec<u16>,
    pub cost: Option<Vec<u16>>,
    pub description: Vec<u16>,
}

/// One unit of a prerequisite, as `getRequiresList` reads it: its display name, whether it is or-ed with the previous
/// unit (`UNIT_OR_WITH_PREV`), and how many the player owns (`calcNumPrereqUnitsOwned`).
#[derive(Clone, Debug, PartialEq)]
pub struct PrereqUnit {
    pub display_name: Vec<u16>,
    pub or_with_prev: bool,
    pub owned: i32,
}

/// What the popup asks of the game beyond the control bar's questions (upstream reads the local player and
/// `TheInGameUI`).
pub trait PopupHost: ControlBarStateHost {
    /// `TheInGameUI->getFirstSelectedDrawable()`'s object.
    fn first_selected_object(&self) -> Option<ObjectId>;
    /// The local player's `hasScience(st)`, `hasUpgradeInProduction(u)`, `hasUpgradeComplete(u)`, and
    /// `TheUpgradeCenter->canAffordUpgrade(localPlayer, u, FALSE)`.
    fn local_has_science(&mut self, st: i32) -> bool;
    fn local_has_upgrade_in_production(&mut self, upgrade: u32) -> bool;
    fn local_has_upgrade_complete(&mut self, upgrade: u32) -> bool;
    fn local_can_afford_upgrade(&mut self, upgrade: u32) -> bool;
    /// `thingTemplate->calcCostToBuild(localPlayer)`, and `isKindOf(KINDOF_STRUCTURE)`.
    fn thing_cost_to_build(&mut self, template: u16) -> i32;
    fn thing_is_kind_of(&self, template: u16, k: KindOfType) -> bool;
    /// The template's prerequisites (`getNthPrereq(i)`), each as its units for `getRequiresList(localPlayer)` and whether
    /// the local player has all its sciences.
    fn thing_prereqs(&mut self, template: u16) -> Vec<(Vec<PrereqUnit>, bool)>;
    /// `upgradeTemplate->calcCostToBuild(player)` (`m_cost`).
    fn upgrade_cost_to_build(&self, upgrade: u32) -> i32;
    /// `TheScienceStore->getNameAndDescription(st)`, `getSciencePurchaseCost(st)`.
    fn science_name_and_description(&self, st: i32) -> Option<(Vec<u16>, Vec<u16>)>;
    fn science_cost(&self, st: i32) -> i32;
    /// Each behavior module's overcharge interface's `isOverchargeActive()`, in module order.
    fn overcharge_interfaces(&mut self, obj: ObjectId) -> Vec<bool>;
    // ~~`local_energy`: the local player's energy~~ **Corrected 2026-11-11, #144:** moved to [`PopupWindowHost`], with the
    // observer's player, for the power window's popup.
}

/// GitHub #144: what `populateBuildTooltipLayout(NULL, tooltipWin)`'s money, power and general's experience branches read.
pub trait PopupWindowHost {
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `TheControlBar->isObserverControlBarOn()`.
    fn is_observer_control_bar_on(&self) -> bool;
    /// `TheControlBar->getObserverLookAtPlayer()`'s energy: `(getProduction(), getConsumption())`, `None` without one.
    fn observer_look_at_player_energy(&self) -> Option<(i32, i32)>;
    /// `ThePlayerList->getLocalPlayer()`'s energy, `None` without a player.
    fn local_energy(&self) -> Option<(i32, i32)>;
}

/// One argument of [`format_wide`].
#[derive(Clone, Debug, PartialEq)]
pub enum FormatArg {
    Int(i64),
    Str(Vec<u16>),
}

/// `UnicodeString::format(fmt, ...)` (upstream's `vswprintf`), for the conversions the game's strings use: `%d`, `%i`,
/// `%u` (an integer), `%s`, `%ls`, `%S`, `%hs` (a string), `%%`. Flags and widths are kept out of the copy (upstream would pad).
/// A conversion with no argument left writes nothing. New code: the formatting is the C library's upstream.
pub fn format_wide(fmt: &[u16], args: &[FormatArg]) -> Vec<u16> {
    let mut out = Vec::with_capacity(fmt.len());
    let mut next = args.iter();
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        if c != u16::from(b'%') {
            out.push(c);
            i += 1;
            continue;
        }
        // the conversion: skip flags, width, precision and length to the type letter
        let mut j = i + 1;
        while j < fmt.len() && b"-+ #0123456789.lhL".iter().any(|&b| u16::from(b) == fmt[j]) {
            j += 1;
        }
        let Some(&t) = fmt.get(j) else {
            out.extend_from_slice(&fmt[i..]);
            break;
        };
        match u8::try_from(t).unwrap_or(0) {
            b'%' => out.push(t),
            b'd' | b'i' | b'u' => {
                if let Some(a) = next.next() {
                    match a {
                        FormatArg::Int(n) => out.extend(n.to_string().encode_utf16()),
                        FormatArg::Str(s) => out.extend_from_slice(s),
                    }
                }
            }
            b's' | b'S' => {
                if let Some(a) = next.next() {
                    match a {
                        FormatArg::Str(s) => out.extend_from_slice(s),
                        FormatArg::Int(n) => out.extend(n.to_string().encode_utf16()),
                    }
                }
            }
            _ => out.extend_from_slice(&fmt[i..=j]),
        }
        i = j + 1;
    }
    out
}

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `ProductionPrerequisite::getRequiresList(player)`: the unfulfilled units' display names (an or-ed pair as
/// "A `CONTROLBAR:OrRequirement` B"), then `CONTROLBAR:GeneralsPromotion` when a science is missing.
///
/// **Upstream quirks kept:** the line break of a later unit is appended *after* its name (so "A" then "B\n"), and the one
/// before the promotion text is appended to the last unit name's copy, not to the list (it is lost).
pub fn get_requires_list(units: &[PrereqUnit], has_sciences: bool, fetch: &mut dyn FnMut(&[u8]) -> Vec<u16>) -> Vec<u16> {
    let mut requires_list = Vec::new();
    let cnt = units.len().min(crate::common::production_prerequisite::MAX_PREREQ);
    let mut own: Vec<i32> = units[..cnt].iter().map(|u| u.owned).collect();
    let mut or_requirements = vec![false; cnt];
    // account for the "or" unit cases, start for loop at 1
    for i in 1..cnt {
        if units[i].or_with_prev {
            or_requirements[i] = true;
            own[i] += own[i - 1];
            own[i - 1] = -1;
        }
    }
    let mut unit_name: Vec<u16> = Vec::new();
    let mut first_requirement = true;
    for i in 0..cnt {
        if own[i] == 0 {
            if or_requirements[i] {
                unit_name = units[i - 1].display_name.clone();
                unit_name.push(u16::from(b' '));
                unit_name.extend(fetch(b"CONTROLBAR:OrRequirement"));
                unit_name.push(u16::from(b' '));
                requires_list.extend_from_slice(&unit_name);
            }
            unit_name = units[i].display_name.clone();
            if first_requirement {
                first_requirement = false;
            } else {
                unit_name.push(u16::from(b'\n'));
            }
            requires_list.extend_from_slice(&unit_name);
        }
    }
    if !has_sciences {
        if !first_requirement {
            // (upstream concatenates the break to `unitName`, which is not used again)
            unit_name.push(u16::from(b'\n'));
        }
        requires_list.extend(fetch(b"CONTROLBAR:GeneralsPromotion"));
    }
    requires_list
}

/// The requirements of a thing's prerequisites, joined with ", ", formatted with `CONTROLBAR:Requirements` (empty when
/// nothing is missing): the loop `populateBuildTooltipLayout` makes twice.
fn requirements(h: &mut impl PopupHost, template: u16) -> Vec<u16> {
    let mut requires: Vec<u16> = Vec::new();
    let mut first_requirement = true;
    for (units, has_sciences) in h.thing_prereqs(template) {
        let requires_list = get_requires_list(&units, has_sciences, &mut |l| h.fetch_text(l));
        if !requires_list.is_empty() {
            if first_requirement {
                first_requirement = false;
            } else {
                requires.extend(w(", "));
            }
        }
        requires.extend(requires_list);
    }
    if requires.is_empty() {
        return requires;
    }
    let f = h.fetch_text(b"CONTROLBAR:Requirements");
    format_wide(&f, &[FormatArg::Str(requires)])
}

fn append_line(descrip: &mut Vec<u16>, extra: Vec<u16>) {
    if !descrip.is_empty() {
        descrip.push(u16::from(b'\n'));
    }
    descrip.extend(extra);
}

/// `ControlBar::populateBuildTooltipLayout(commandButton)`: the popup of a command button.
pub fn populate_build_tooltip_layout(button: &CommandButton, h: &mut impl PopupHost) -> Tooltip {
    let mut name: Vec<u16>;
    let mut cost: Vec<u16> = Vec::new();
    let mut descrip: Vec<u16> = Vec::new();
    let mut cost_to_build: i32 = 0;
    let mut fire_science_button = false;
    let thing = button.thing_template;
    let upgrade = button.upgrade.map(|k| k.0);
    let t = button.command;

    let mut st: Option<i32> = None;
    if t != gui::PLAYER_UPGRADE && t != gui::OBJECT_UPGRADE {
        let sv = &button.science;
        if sv.len() > 1 {
            for j in 0..sv.len() {
                st = Some(sv[j].0);
                if t != gui::PURCHASE_SCIENCE {
                    if !h.local_has_science(sv[j].0) && j > 0 {
                        // "we want to get the description for the previous science -- the one we can use, not purchase!"
                        st = Some(sv[j - 1].0);
                    }
                    fire_science_button = true;
                    break;
                } else if !h.local_has_science(sv[j].0) {
                    // "The first science we run into that we don't have, that's the one we'll want to show!"
                    break;
                }
            }
        } else if sv.len() == 1 {
            st = Some(sv[0].0);
            if t != gui::PURCHASE_SCIENCE {
                fire_science_button = true;
            }
        }
    }

    if !button.labels[1].is_empty() {
        descrip = h.fetch_text(&button.labels[1]);
        if let Some(selected) = h.first_selected_object() {
            if t == gui::TOGGLE_OVERCHARGE {
                // "Special case: Append status of overcharge on China power plant."
                for active in h.overcharge_interfaces(selected) {
                    descrip.push(u16::from(b'\n'));
                    let l: &[u8] = if active { b"TOOLTIP:TooltipNukeReactorOverChargeIsOn" } else { b"TOOLTIP:TooltipNukeReactorOverChargeIsOff" };
                    descrip.extend(h.fetch_text(l));
                }
            } else if let Some(tt) = thing {
                // "the CanMakeType determines reasons for not being able to buy stuff"
                let reason: Option<&[u8]> = match h.can_make_unit(selected, Some(tt)) {
                    CanMakeType::NoMoney => Some(b"TOOLTIP:TooltipNotEnoughMoneyToBuild"),
                    CanMakeType::QueueFull => Some(b"TOOLTIP:TooltipCannotPurchaseBecauseQueueFull"),
                    CanMakeType::ParkingPlacesFull => Some(b"TOOLTIP:TooltipCannotBuildUnitBecauseParkingFull"),
                    CanMakeType::MaxedOutForPlayer => Some(if h.thing_is_kind_of(tt, KINDOF_STRUCTURE) {
                        b"TOOLTIP:TooltipCannotBuildBuildingBecauseMaximumNumber"
                    } else {
                        b"TOOLTIP:TooltipCannotBuildUnitBecauseMaximumNumber"
                    }),
                    _ => None,
                };
                if let Some(l) = reason {
                    descrip.extend(w("\n\n"));
                    descrip.extend(h.fetch_text(l));
                }
            } else if let Some(u) = upgrade.filter(|&u| !h.local_has_upgrade_in_production(u)) {
                if t == gui::PLAYER_UPGRADE || t == gui::OBJECT_UPGRADE {
                    if h.production_count(selected) == Some(MAX_BUILD_QUEUE_BUTTONS) {
                        descrip.extend(w("\n\n"));
                        descrip.extend(h.fetch_text(b"TOOLTIP:TooltipCannotPurchaseBecauseQueueFull"));
                    } else if !h.local_can_afford_upgrade(u) {
                        descrip.extend(w("\n\n"));
                        descrip.extend(h.fetch_text(b"TOOLTIP:TooltipNotEnoughMoneyToBuild"));
                    }
                }
            }
        }
    }

    name = h.fetch_text(&button.labels[0]);

    if let Some(tt) = thing.filter(|_| t != gui::PURCHASE_SCIENCE) {
        // "Format the cost only when we have to pay for it."
        cost_to_build = h.thing_cost_to_build(tt);
        if cost_to_build > 0 {
            cost = format_wide(&h.fetch_text(b"TOOLTIP:Cost"), &[FormatArg::Int(i64::from(cost_to_build))]);
        }
        let requires = requirements(h, tt);
        if !requires.is_empty() {
            append_line(&mut descrip, requires);
        }
    } else if let Some(u) = upgrade {
        // "We are looking at an upgrade purchase icon. Maybe we already purchased it?"
        let mut has_upgrade_already = h.local_has_upgrade_complete(u);
        let mut has_conflicting_upgrade = false;
        let player_upgrade_button = t == gui::PLAYER_UPGRADE;
        let object_upgrade_button = t == gui::OBJECT_UPGRADE;
        if !has_upgrade_already {
            if let Some(object) = h.first_selected_object() {
                has_upgrade_already = h.obj_has_upgrade(object, u);
                if object_upgrade_button {
                    has_conflicting_upgrade = !h.affected_by_upgrade(object, u);
                }
            }
        }
        if has_conflicting_upgrade && !has_upgrade_already {
            let l: &[u8] = if button.labels[3].is_empty() { b"TOOLTIP:HasConflictingUpgradeDefault" } else { &button.labels[3] };
            descrip = h.fetch_text(l);
        } else if has_upgrade_already && (player_upgrade_button || object_upgrade_button) {
            let l: &[u8] = if button.labels[2].is_empty() { b"TOOLTIP:AlreadyUpgradedDefault" } else { &button.labels[2] };
            descrip = h.fetch_text(l);
        } else if !has_upgrade_already {
            let missing_science = button.science.iter().any(|s| !h.local_has_science(s.0));
            cost_to_build = h.upgrade_cost_to_build(u);
            if cost_to_build > 0 {
                cost = format_wide(&h.fetch_text(b"TOOLTIP:Cost"), &[FormatArg::Int(i64::from(cost_to_build))]);
            }
            if missing_science {
                let f = h.fetch_text(b"CONTROLBAR:Requirements");
                let promotion = h.fetch_text(b"CONTROLBAR:GeneralsPromotion");
                append_line(&mut descrip, format_wide(&f, &[FormatArg::Str(promotion)]));
            }
        }
    } else if let Some(st) = st.filter(|_| !fire_science_button) {
        if let Some((n, d)) = h.science_name_and_description(st) {
            name = n;
            descrip = d;
        }
        cost_to_build = h.science_cost(st);
        if cost_to_build > 0 {
            cost = format_wide(&h.fetch_text(b"TOOLTIP:ScienceCost"), &[FormatArg::Int(i64::from(cost_to_build))]);
        }
        // (the prerequisites' requirements of a thing: never reached, a thing took the first branch)
    }
    Tooltip { name, cost: (cost_to_build > 0).then_some(cost), description: descrip }
}

/// The windows `populateBuildTooltipLayout(NULL, tooltipWin)` knows besides the buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipWindow {
    /// `ControlBar.wnd:MoneyDisplay`.
    Money,
    /// `ControlBar.wnd:PowerWindow`.
    Power,
    /// `ControlBar.wnd:GeneralsExp`.
    GeneralsExp,
}

/// The names `populateBuildTooltipLayout` compares `tooltipWin` with, in its order (ControlBarPopupDescription.cpp:556).
pub const TOOLTIP_WINDOW_NAMES: [(&[u8], TooltipWindow); 3] = [
    (b"ControlBar.wnd:MoneyDisplay", TooltipWindow::Money),
    (b"ControlBar.wnd:PowerWindow", TooltipWindow::Power),
    (b"ControlBar.wnd:GeneralsExp", TooltipWindow::GeneralsExp),
];

/// GitHub #144: `populateBuildTooltipLayout(NULL, tooltipWin)`'s comparisons (ControlBarPopupDescription.cpp:556): which
/// of the money, power and general's experience windows `tooltip_win` is, each found with
/// `TheWindowManager->winGetWindowFromId(m_buildToolTipLayout->getFirstWindow(), nameToKey(name))` (`find`); `None` is
/// upstream's `DEBUG_ASSERTCRASH` and `return`: the popup is not filled (and `showBuildTooltipLayout` then shows it as it
/// was).
///
/// The search starts at the popup layout's first window and walks the window list from there (`m_next`) and every
/// child. `ControlBar::init` makes the popup's layout after `ControlBar.wnd` exists (`InGameUI::init`'s
/// `createControlBar()` comes before `TheControlBar->init()`), and `linkWindow` puts a new window at the list's head, so
/// every window of `ControlBar.wnd` is reached: `find` searches the bar's windows.
pub fn tooltip_window_of(tooltip_win: crate::client::window_script::WinId, find: impl Fn(&[u8]) -> Option<crate::client::window_script::WinId>) -> Option<TooltipWindow> {
    for (i, (name, win)) in TOOLTIP_WINDOW_NAMES.iter().enumerate() {
        if find(name) == Some(tooltip_win) {
            // (planted: the general's experience window read as the money's)
            if crate::logic::modules::plant(9358) && i == 2 {
                return Some(TooltipWindow::Money);
            }
            return Some(*win);
        }
    }
    if crate::logic::modules::plant(9355) {
        return Some(TooltipWindow::Money);
    }
    None
}

/// `populateBuildTooltipLayout(NULL, tooltipWin)`'s branches for the money, power and general's experience windows
/// (ControlBarPopupDescription.cpp:556-596; no cost). The power's description is formatted with the displayed player's
/// production and consumption (`descrip.format(descrip, ...)`): the observer's look-at player while the observer's
/// control bar is on, else the local player; 0, 0 without one. **Corrected 2026-11-11, #144:** ~~the observer's player
/// is not translated~~ it is ([`PopupWindowHost::is_observer_control_bar_on`]).
pub fn populate_window_tooltip(win: TooltipWindow, h: &impl PopupWindowHost) -> Tooltip {
    use crate::logic::modules::plant;
    let (n, d): (&[u8], &[u8]) = match win {
        TooltipWindow::Money => (b"CONTROLBAR:Money", b"CONTROLBAR:MoneyDescription"),
        TooltipWindow::Power => (b"CONTROLBAR:Power", b"CONTROLBAR:PowerDescription"),
        TooltipWindow::GeneralsExp => (b"CONTROLBAR:GeneralsExp", b"CONTROLBAR:GeneralsExpDescription"),
    };
    let name = h.fetch_text(n);
    let mut description = h.fetch_text(d);
    if win == TooltipWindow::Power {
        let energy = if h.is_observer_control_bar_on() && !plant(9356) { h.observer_look_at_player_energy() } else { h.local_energy() };
        match energy {
            Some((p, c)) if plant(9354) => description = format_wide(&description, &[FormatArg::Int(i64::from(c)), FormatArg::Int(i64::from(p))]),
            Some((p, c)) => description = format_wide(&description, &[FormatArg::Int(i64::from(p)), FormatArg::Int(i64::from(c))]),
            None if plant(9357) => {}
            None => description = format_wide(&description, &[FormatArg::Int(0), FormatArg::Int(0)]),
        }
    }
    Tooltip { name, cost: None, description }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> Vec<u16> {
        w(x)
    }

    /// #35, `UnicodeString::format`. Predicted: `Cost: $%d` with 300 is `Cost: $300`; `Requires: %s` with a list puts the
    /// list in; `%%` is one `%`; a missing argument writes nothing; `%5d` loses its width (not translated).
    #[test]
    fn the_game_strings_format_as_vswprintf() {
        assert_eq!(format_wide(&s("Cost: $%d"), &[FormatArg::Int(300)]), s("Cost: $300"));
        assert_eq!(format_wide(&s("Requires: %s"), &[FormatArg::Str(s("Barracks"))]), s("Requires: Barracks"));
        assert_eq!(format_wide(&s("Power %d/%d 100%%"), &[FormatArg::Int(5), FormatArg::Int(3)]), s("Power 5/3 100%"));
        assert_eq!(format_wide(&s("x%dy"), &[]), s("xy"));
        assert_eq!(format_wide(&s("%5d|%ls"), &[FormatArg::Int(7), FormatArg::Str(s("a"))]), s("7|a"));
    }

    /// #35, `getRequiresList`. Predicted from ProductionPrerequisite.cpp: units A (owned 0), B (owned 0, or-ed with A), C
    /// (owned 0) and D (owned 1): the or-pair gives "A or " then B, C follows with its break after it ("C\n": the quirk);
    /// with a science missing the promotion text follows (its break lost). All owned and the sciences had: empty.
    #[test]
    fn the_requirements_list_as_upstream_writes_it() {
        let u = |n: &str, or: bool, owned: i32| PrereqUnit { display_name: s(n), or_with_prev: or, owned };
        let mut fetch = |l: &[u8]| match l {
            b"CONTROLBAR:OrRequirement" => s("or"),
            b"CONTROLBAR:GeneralsPromotion" => s("Promotion"),
            _ => s("?"),
        };
        let units = [u("A", false, 0), u("B", true, 0), u("C", false, 0), u("D", false, 1)];
        assert_eq!(get_requires_list(&units, true, &mut fetch), s("A or BC\n"));
        assert_eq!(get_requires_list(&units, false, &mut fetch), s("A or BC\nPromotion"));
        assert_eq!(get_requires_list(&[u("A", false, 2)], true, &mut fetch), Vec::<u16>::new());
        assert_eq!(get_requires_list(&[u("A", false, 1), u("B", true, 0)], true, &mut fetch), Vec::<u16>::new(), "either owned satisfies");
    }

    /// #35, the veterancy overlay. Predicted: regular none; veteran, elite, heroic `SSChevron1L`, `2L`, `3L`.
    #[test]
    fn the_portrait_overlay_shows_the_rank() {
        assert_eq!(calculate_veterancy_overlay_for_object(0), None);
        assert_eq!(calculate_veterancy_overlay_for_object(1), Some(&b"SSChevron1L"[..]));
        assert_eq!(calculate_veterancy_overlay_for_object(2), Some(&b"SSChevron2L"[..]));
        assert_eq!(calculate_veterancy_overlay_for_object(3), Some(&b"SSChevron3L"[..]));
    }
}
