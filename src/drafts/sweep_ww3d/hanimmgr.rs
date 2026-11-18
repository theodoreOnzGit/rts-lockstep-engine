//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hanimmgr.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hanimmgr.h
//!
//! `HAnimManagerClass` members no earlier pass drafted. Checked against the verified
//! `formats::w3d::assetmgr::AssetManager` (which owns the animation table):
//!   - `Load_Morph_Anim`, `Load_Raw_Anim`, `Load_Compressed_Anim` (hanimmgr.cpp:154, :197, :240): translated:
//!     `formats::w3d::assetmgr::AssetManager::load_anim` (assetmgr.rs:985) does the three in one `match` on the chunk id,
//!     with the same "load failed" / "duplicate exists" returns of 1 and `Add_Anim` + `Release_Ref` on success.
//!   - `Get_Anim` (hanimmgr.cpp:300): translated: `AssetManager::peek_anim_shared` (assetmgr.rs:919), `Add_Ref` being the
//!     `Arc` clone.
//!   - `HAnimManagerIterator::Get_Current_Anim` (hanimmgr.cpp:454): translated: the cast of `Get_Current()`; the iteration
//!     order is `AssetManager::anim_names` (assetmgr.rs:949).
//! Drafted here: `Free_All_Anims_With_Exclusion_List` and `Create_Asset_List`, over [`AnimEntry`] (name and `Num_Refs()`)
//! in iterator order.

use crate::drafts::ww3d::w3dexclusionlist::W3DExclusionListClass;

/// An `HAnimClass` in `AnimPtrTable`: its `Get_Name()` (`"<hierarchy>.<anim>"`) and `Num_Refs()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimEntry {
    pub name: String,
    pub num_refs: u32,
}

/// `HAnimManagerClass::AnimPtrTable`'s contents, in `HAnimManagerIterator` order.
#[derive(Clone, Debug, Default)]
pub struct HAnimManagerState {
    pub anims: Vec<AnimEntry>,
}

impl HAnimManagerState {
    /// `HAnimManagerClass::Free_All_Anims_With_Exclusion_List(exclusion_list)` (hanimmgr.cpp:347): "Remove and
    /// Release_Ref any animation not in the exclusion list." Only an animation whose sole reference is the table's
    /// (`Num_Refs() == 1`) and that is not excluded (`W3DExclusionListClass::Is_Excluded(HAnimClass*)`, the name after
    /// the '.') is removed (`AnimPtrTable->Remove(anim)` then `anim->Release_Ref()`); the others stay.
    pub fn free_all_anims_with_exclusion_list(&mut self, exclusion_list: &W3DExclusionListClass) {
        self.anims.retain(|anim| {
            if anim.num_refs == 1 && !exclusion_list.is_excluded_hanim(&anim.name) {
                false
            } else {
                true
            }
        });
    }

    /// `HAnimManagerClass::Create_Asset_List(exclusion_list)` (hanimmgr.cpp:379): for each animation, "File that this anim
    /// came from should be the name after the '.'. Anims are named in the format: <skeleton>.<animname>"; the part after
    /// the first '.' is added (an animation without a '.' adds nothing).
    pub fn create_asset_list(&self, exclusion_list: &mut Vec<String>) {
        for anim in &self.anims {
            if let Some(p) = anim.name.find('.') {
                exclusion_list.push(anim.name[p + 1..].to_string());
            }
        }
    }
}
