//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DSupplyDraw.cpp
//!
//! Sweep 2 (#213): `W3DSupplyDraw::updateDrawModuleSupplyStatus`.

use super::w3d_model_draw::{update_draw_module_supply_status as base_supply_status, HideShowSubObjInfo, SupplyCondition};

/// The members of `W3DSupplyDraw`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct W3DSupplyDrawState {
    /// `m_totalBones` (-1 until counted).
    pub total_bones: i32,
    /// `m_lastNumberShown`.
    pub last_number_shown: i32,
}

impl Default for W3DSupplyDrawState {
    fn default() -> Self {
        W3DSupplyDrawState { total_bones: -1, last_number_shown: 0 }
    }
}

impl W3DSupplyDrawState {
    /// `W3DSupplyDraw::updateDrawModuleSupplyStatus` (W3DSupplyDraw.cpp:71): the base class's CARRYING condition, then the
    /// supply piles shown in proportion to `currentSupply / maxSupply`: bones named `<prefix>NN` between the old and new
    /// shown count are hidden (fewer) or shown (more), in index order. `count_bones(prefix)` is
    /// `getDrawable()->getPristineBonePositions(prefix, 1, NULL, NULL, INT_MAX)`.
    pub fn update_draw_module_supply_status(
        &mut self,
        max_supply: i32,
        current_supply: i32,
        bone_prefix: &str,
        count_bones: &impl Fn(&str) -> i32,
    ) -> (SupplyCondition, Vec<HideShowSubObjInfo>) {
        let condition = base_supply_status(max_supply, current_supply);
        if self.total_bones == -1 {
            self.total_bones = count_bones(bone_prefix);
            self.last_number_shown = self.total_bones;
        }
        let mut bones_to_show = ((self.total_bones as f64) * ((current_supply as f32 / max_supply as f32) as f64)).ceil() as i32;
        bones_to_show = bones_to_show.min(self.total_bones);
        let mut bone_vector = Vec::new();
        if bones_to_show != self.last_number_shown {
            let low_index = self.last_number_shown.min(bones_to_show);
            let high_index = self.last_number_shown.max(bones_to_show);
            let hide = bones_to_show < self.last_number_shown;
            let mut current_index = low_index + 1;
            while current_index <= high_index {
                bone_vector.push(HideShowSubObjInfo { sub_obj_name: format!("{}{:02}", bone_prefix, current_index).into_bytes(), hide });
                current_index += 1;
            }
            self.last_number_shown = bones_to_show;
        }
        (condition, bone_vector)
    }
}
