//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htreemgr.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htreemgr.h
//!
//! `HTreeManagerClass::Free_All_Trees_With_Exclusion_List` and `::Get_Tree_Name`. The verified
//! `formats::w3d::assetmgr::AssetManager` keeps the trees as `trees: Vec<(name, HTree)>` (private) and has `Free_All_Trees`
//! only as part of `free_assets`; neither of these two is there. This draft works on [`HTreeManagerState`], a list of
//! tree names in `TreePtr` order, so it can replace `trees` handling when merged. The hash (`TreeHash`, keyed by the
//! lower-cased name) is rebuilt by the caller from [`HTreeManagerState::trees`], as upstream re-inserts each survivor.

use crate::drafts::ww3d::w3dexclusionlist::W3DExclusionListClass;
use crate::formats::w3d::assetmgr::MAX_TREES;

/// `HTreeManagerClass`'s `TreePtr[]` (names only; `None` is NULL) and `NumTrees`.
#[derive(Clone, Debug, Default)]
pub struct HTreeManagerState {
    /// `TreePtr[0..MAX_TREES]`; only the first `num_trees` slots are normally non-NULL.
    pub tree_ptr: Vec<Option<String>>,
    /// `NumTrees`.
    pub num_trees: i32,
}

impl HTreeManagerState {
    /// An empty manager (`MAX_TREES` NULL slots, `NumTrees = 0`).
    pub fn new() -> Self {
        HTreeManagerState { tree_ptr: vec![None; MAX_TREES], num_trees: 0 }
    }

    /// `HTreeManagerClass::Free_All_Trees_With_Exclusion_List(exclusion_list)` (htreemgr.cpp:146): "we simply loop over
    /// the array either deleting the tree or copying it to the new tail index if it is excluded". Note upstream copies the
    /// survivor to `TreePtr[new_tail]` without clearing the slot it came from; a survivor that moves leaves its old
    /// slot pointing at the same tree (so the slots above `NumTrees` can hold stale pointers). Kept: the stale slot stays
    /// `Some` here too. Returns the survivors in order, for the `TreeHash` rebuild (lower-cased names).
    pub fn free_all_trees_with_exclusion_list(&mut self, exclusion_list: &W3DExclusionListClass) -> Vec<String> {
        let mut new_tail = 0usize;
        for treeidx in 0..MAX_TREES.min(self.tree_ptr.len()) {
            if let Some(name) = self.tree_ptr[treeidx].clone() {
                if exclusion_list.is_excluded_htree(&name) {
                    self.tree_ptr[new_tail] = Some(name);
                    new_tail += 1;
                } else {
                    self.tree_ptr[treeidx] = None;
                }
            }
        }
        self.num_trees = new_tail as i32;

        // Clear the hash table, then add back any trees that were not deleted
        let mut survivors = Vec::with_capacity(new_tail);
        for treeidx in 0..new_tail {
            // StringClass lower_case_name(TreePtr[treeidx]->Get_Name(),true); _strlwr(...)
            if let Some(name) = &self.tree_ptr[treeidx] {
                survivors.push(name.to_lowercase());
            }
        }
        survivors
    }

    /// `HTreeManagerClass::Get_Tree_Name(idx)` (htreemgr.cpp:271): `(idx < NumTrees) && TreePtr[idx]` gives the name, else
    /// NULL. A negative `idx` is `< NumTrees` and indexes before the array upstream; here it is NULL.
    pub fn get_tree_name(&self, idx: i32) -> Option<&str> {
        if idx < self.num_trees && idx >= 0 {
            if let Some(Some(name)) = self.tree_ptr.get(idx as usize) {
                return Some(name.as_str());
            }
        }
        None
    }
}
