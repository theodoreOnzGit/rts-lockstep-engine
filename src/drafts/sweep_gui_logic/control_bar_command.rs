//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases per function (test: tests/differential_draft_cbcmd.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarCommand.cpp (ControlBar::populateInvDataCallback,
//!   resetBuildQueueData, calculateVeterancyOverlayForThing)

use crate::common::game_type::ObjectId;
use crate::logic::modules::plant;

/// `MAX_BUILD_QUEUE_BUTTONS` (ControlBar.h:414).
pub const MAX_BUILD_QUEUE_BUTTONS: usize = 9;

/// `ControlBar::ContainEntry`: a transport inventory button and the object it shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContainEntry {
    /// `control`.
    pub control: Option<u32>,
    /// `objectID`.
    pub object_id: ObjectId,
}

/// `ProductionType` of a build queue entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProductionType {
    /// `PRODUCTION_INVALID`.
    #[default]
    Invalid,
    Unit,
    Upgrade,
}

/// `ControlBar::QueueEntry`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueueEntry {
    /// `control`.
    pub control: Option<u32>,
    /// `type`.
    pub ty: ProductionType,
    /// `productionID` (`PRODUCTIONID_INVALID`: 0) of the union with `upgradeToResearch` (`NULL`: `None`).
    pub production_id: u32,
    pub upgrade_to_research: Option<u32>,
}

/// `PopulateInvButtonData` (ControlBarCommand.cpp): the buttons the transport's inventory fills.
pub struct PopulateInvButtonData {
    /// `controls`: the inventory button windows.
    pub controls: Vec<u32>,
    /// `currIndex`, `maxIndex`.
    pub curr_index: i32,
    pub max_index: i32,
}

/// What the callback asks of the world and the buttons.
pub trait PopulateInvHost {
    /// `obj->getTemplate()->getButtonImage()`: the image's name.
    fn object_button_image(&self, obj: ObjectId) -> Option<Vec<u8>>;
    /// `calculateVeterancyOverlayForObject(obj)`.
    fn veterancy_overlay_for_object(&self, obj: ObjectId) -> Option<Vec<u8>>;
    /// `GadgetButtonSetEnabledImage(control, image)`.
    fn button_set_enabled_image(&mut self, control: u32, image: Option<Vec<u8>>);
    /// `GadgetButtonDrawOverlayImage(control, image)`.
    fn button_draw_overlay_image(&mut self, control: u32, image: Option<Vec<u8>>);
    /// `control->winEnable(enable)`.
    fn win_enable(&mut self, control: u32, enable: bool);
}

/// `ControlBar::populateInvDataCallback(obj, userData)` (ControlBarCommand.cpp:91): puts one contained object on the next
/// inventory button: the button remembers the object, shows its image with its veterancy overlay, and is enabled. Too
/// many objects for the buttons is a `DEBUG_ASSERTCRASH` and the rest are ignored.
pub fn populate_inv_data_callback<H: PopulateInvHost>(h: &mut H, contain_data: &mut [ContainEntry], obj: ObjectId, data: &mut PopulateInvButtonData) {
    if (if plant(15683) { data.curr_index >= data.max_index } else { data.curr_index > data.max_index }) {
        // DEBUG_ASSERTCRASH( 0, ("There is not enough GUI slots to hold the # of items inside a '%s'\n", ...) );
        return;
    }
    let control = data.controls.get(data.curr_index as usize).copied();
    // DEBUG_ASSERTCRASH( control, ("populateInvDataCallback: Control not found\n") );
    contain_data[data.curr_index as usize].control = control;
    if !plant(15687) {
        contain_data[data.curr_index as usize].object_id = obj;
    }
    if !plant(15684) {
        data.curr_index += 1;
    }
    let Some(control) = control else { return };
    let image = h.object_button_image(obj);
    if plant(15688) {
        let overlay = h.veterancy_overlay_for_object(obj);
        h.button_set_enabled_image(control, overlay);
    } else {
        h.button_set_enabled_image(control, image);
    }
    let overlay = h.veterancy_overlay_for_object(obj);
    if !plant(15685) {
        h.button_draw_overlay_image(control, overlay);
    }
    h.win_enable(control, !plant(15686));
}

/// `ControlBar::resetBuildQueueData()` (ControlBarCommand.cpp:537): every queue entry has no control, an invalid type,
/// an invalid production id and no upgrade.
pub fn reset_build_queue_data(queue_data: &mut [QueueEntry; MAX_BUILD_QUEUE_BUTTONS]) {
    for q in queue_data.iter_mut() {
        q.control = None;
        if !plant(15691) {
            q.ty = ProductionType::Invalid;
        }
        if !plant(15690) {
            q.production_id = 0; // PRODUCTIONID_INVALID
        }
        if !plant(15689) {
            q.upgrade_to_research = None;
        }
    }
}

/// `VeterancyLevel` as the overlay choice reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VeterancyLevelRank {
    Regular,
    Veteran,
    Elite,
    Heroic,
}

/// `ControlBar::calculateVeterancyOverlayForThing(thingTemplate)` (ControlBarCommand.cpp:917): the rank icon a unit
/// built here starts with: the highest `m_startingLevel` of the template's `VeterancyGainCreate` modules whose required
/// science the local player has (or that need none). `modules` gives each such module's (`m_scienceRequired`, `m_startingLevel`),
/// `None` science for `SCIENCE_INVALID`; `has_science` is `player->hasScience`. `None` for no template, no local player or
/// `LEVEL_REGULAR`. (The loop goes on over later modules after one is found, as upstream's does.)
pub fn calculate_veterancy_overlay_for_thing(
    have_template: bool,
    have_local_player: bool,
    modules: &[(Option<i32>, VeterancyLevelRank)],
    has_science: impl Fn(i32) -> bool,
    rank_veteran_icon: Option<Vec<u8>>,
    rank_elite_icon: Option<Vec<u8>>,
    rank_heroic_icon: Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    let mut level = VeterancyLevelRank::Regular;
    if !have_template {
        return None;
    }
    if !have_local_player && !plant(15695) {
        return None;
    }
    for (science_required, starting_level) in modules {
        let science_ok = if plant(15692) {
            true
        } else if plant(15697) {
            science_required.map_or(false, |s| has_science(s))
        } else {
            science_required.map_or(true, |s| has_science(s))
        };
        if science_ok && (*starting_level > level || plant(15693)) {
            level = *starting_level;
            if plant(15696) {
                break;
            }
        }
    }
    match level {
        VeterancyLevelRank::Veteran => rank_veteran_icon,
        VeterancyLevelRank::Elite if plant(15694) => rank_heroic_icon,
        VeterancyLevelRank::Elite => rank_elite_icon,
        VeterancyLevelRank::Heroic => rank_heroic_icon,
        VeterancyLevelRank::Regular => None,
    }
}
