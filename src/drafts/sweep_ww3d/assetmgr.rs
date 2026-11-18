//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/assetmgr.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/assetmgr.h
//!
//! The `WW3DAssetManager` members no earlier pass translated: releasing unused assets, freeing assets with an
//! exclusion list, the asset list, the asset iterators and the `Font3DData` registry. The verified
//! `formats::w3d::assetmgr::AssetManager` owns prototypes, trees and animations; this file works on a small
//! [`AssetState`] that holds what these functions touch (prototype names and class ids, the texture hash, the font list),
//! so it can be merged into the real manager when verified.
//!
//! Excluded: `WW3DAssetManager::Log_Texture_Statistics` (assetmgr.cpp:384) and `::Log_All_Textures` (:1235). Both only
//! print (`WWDEBUG_SAY`, compiled out of release builds) the sizes and names of textures and `CLASSID_HMODEL` render
//! objects, and change no state: debug-only. (Their statistics are in `sweep_ww3d::texture`.)

use std::collections::BTreeMap;

/// `RenderObjClass::CLASSID_*` value the asset list does not need: a prototype is its name and class id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrototypeEntry {
    pub name: String,
    pub class_id: i32,
}

/// A `TextureClass` in `TextureHash`, with its reference count (`Num_Refs()`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureEntry {
    pub num_refs: u32,
}

/// `Font3DDataClass` in the asset manager's `Font3DDatas` list: its name and reference count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Font3DData {
    pub name: String,
    pub num_refs: u32,
}

/// What `Free_Assets_With_Exclusion_List` asks of the exclusion list and the managers.
pub trait AssetHost {
    /// `TheDX8MeshRenderer.Invalidate()`.
    fn mesh_renderer_invalidate(&mut self);
    /// `W3DExclusionListClass::Is_Excluded(PrototypeClass*)`.
    fn exclusion_is_excluded(&self, proto: &PrototypeEntry) -> bool;
    /// `HAnimManager.Free_All_Anims_With_Exclusion_List(exclusion_list)`.
    fn hanim_free_all_anims_with_exclusion_list(&mut self);
    /// `HTreeManager.Free_All_Trees_With_Exclusion_List(exclusion_list)`.
    fn htree_free_all_trees_with_exclusion_list(&mut self);
    /// `HAnimManager.Create_Asset_List(model_list)`: the anims' w3d file names.
    fn hanim_create_asset_list(&self) -> Vec<String>;
    /// `HTreeManager.Get_Tree(index)->Get_Name()` and `Num_Trees()`.
    fn htree_count(&self) -> usize;
    fn htree_name(&self, index: usize) -> String;
}

/// The asset manager state these functions use.
#[derive(Clone, Debug, Default)]
pub struct AssetState {
    /// `Prototypes`: `None` is a NULL slot.
    pub prototypes: Vec<Option<PrototypeEntry>>,
    /// `TextureHash`, keyed by texture name.
    pub texture_hash: BTreeMap<String, TextureEntry>,
    /// `Font3DDatas` (`Add_Head`: newest first).
    pub font3d_datas: Vec<Font3DData>,
}

/// `WW3DAssetManager::Create_Asset_List`'s filter: sub objects have a '.' in their name and munged objects a '#'.
fn is_asset_name(name: &str) -> bool {
    !name.contains('#') && !name.contains('.')
}

impl AssetState {
    /// `WW3DAssetManager::Release_Unused_Assets` (assetmgr.cpp:503): textures and font data nobody else references.
    pub fn release_unused_assets(&mut self) {
        self.release_unused_textures();
        self.release_unused_font3d_datas();
    }

    /// `WW3DAssetManager::Release_Unused_Textures` (assetmgr.cpp:1181): drop every texture whose only reference is the
    /// hash's own (`Num_Refs() == 1`). Upstream collects them 256 at a time because the iterator cannot survive a
    /// removal; the end state is the same as removing them all.
    pub fn release_unused_textures(&mut self) {
        self.texture_hash.retain(|_, t| t.num_refs != 1);
    }

    /// `WW3DAssetManager::Release_Unused_Font3DDatas` (called by `Release_Unused_Assets`; defined with the font list, not
    /// on the sweep list): drop font data whose only reference is the list's.
    pub fn release_unused_font3d_datas(&mut self) {
        self.font3d_datas.retain(|f| f.num_refs != 1);
    }

    /// `WW3DAssetManager::Release_Texture` (assetmgr.cpp:1225): remove one texture from the hash and release it.
    pub fn release_texture(&mut self, texture_name: &str) {
        self.texture_hash.remove(texture_name);
    }

    /// `WW3DAssetManager::Free_Assets_With_Exclusion_List` (assetmgr.cpp:528): delete every prototype the exclusion list
    /// does not name, re-add the excluded ones, free the anims and trees, then release the unused textures.
    /// `add_prototype` is `Add_Prototype` (hash-table insertion), which the real manager owns.
    pub fn free_assets_with_exclusion_list<H: AssetHost>(&mut self, host: &mut H, mut add_prototype: impl FnMut(&mut Self, PrototypeEntry)) {
        host.mesh_renderer_invalidate();
        // DEFAULT_EXCLUDE_ARRAY_SIZE = 8000, only a pre-allocation
        let mut exclude_array: Vec<PrototypeEntry> = Vec::with_capacity(8000);
        for i in 0..self.prototypes.len() {
            if let Some(proto) = self.prototypes[i].take() {
                if host.exclusion_is_excluded(&proto) {
                    exclude_array.push(proto);
                }
                // else: proto->DeleteSelf()
            }
        }
        // Prototypes.Reset_Active(); the PrototypeHashTable is cleared by the real manager's add path
        self.prototypes.clear();
        for proto in exclude_array {
            add_prototype(self, proto);
        }
        host.hanim_free_all_anims_with_exclusion_list();
        host.htree_free_all_trees_with_exclusion_list();
        self.release_unused_textures();
    }

    /// `WW3DAssetManager::Create_Asset_List` (assetmgr.cpp:598): the top-level model names, then the animations' files.
    pub fn create_asset_list<H: AssetHost>(&self, host: &H) -> Vec<String> {
        let mut model_list = Vec::new();
        for proto in self.prototypes.iter().flatten() {
            if is_asset_name(&proto.name) {
                model_list.push(proto.name.clone());
            }
        }
        model_list.extend(host.hanim_create_asset_list());
        model_list
    }

    /// `WW3DAssetManager::Create_Render_Obj_Iterator` (assetmgr.cpp:880).
    pub fn create_render_obj_iterator(&self) -> RObjIterator {
        RObjIterator { index: 0 }
    }

    /// `WW3DAssetManager::Release_Render_Obj_Iterator` (assetmgr.cpp:898): `delete it`; dropping the value is the same.
    pub fn release_render_obj_iterator(&self, _it: RObjIterator) {}

    /// `WW3DAssetManager::Create_HAnim_Iterator` (assetmgr.cpp:920).
    pub fn create_hanim_iterator(&self) -> HAnimIterator {
        HAnimIterator { index: 0 }
    }

    /// `WW3DAssetManager::Create_HTree_Iterator` (assetmgr.cpp:938).
    pub fn create_htree_iterator(&self) -> HTreeIterator {
        HTreeIterator { index: 0 }
    }

    /// `WW3DAssetManager::Create_Font3DData_Iterator` (assetmgr.cpp:959).
    pub fn create_font3d_data_iterator(&self) -> Font3DDataIterator {
        Font3DDataIterator { index: 0 }
    }

    /// `WW3DAssetManager::Get_Font3DInstance` (assetmgr.cpp:1330): `NEW_REF(Font3DInstanceClass, (name))`; the instance
    /// itself is `Font3DInstanceClass` (font3d.cpp), here its name.
    pub fn get_font3d_instance(&self, name: &str) -> String {
        name.to_string()
    }

    /// `WW3DAssetManager::Add_Font3DData` (assetmgr.cpp:1383): takes a reference and adds at the head.
    pub fn add_font3d_data(&mut self, mut font: Font3DData) {
        font.num_refs += 1;
        self.font3d_datas.insert(0, font);
    }

    /// `WW3DAssetManager::Remove_Font3DData` (assetmgr.cpp:1389): releases a reference and removes it from the list.
    pub fn remove_font3d_data(&mut self, name: &str) {
        if let Some(pos) = self.font3d_datas.iter().position(|f| f.name == name) {
            self.font3d_datas.remove(pos);
        }
    }
}

/// `RObjIterator` (assetmgr.h): walks `Prototypes`. `First`/`Next`/`Is_Done` are already in the port's
/// `AssetManager::prototypes()` iteration; the two accessors below are the ones missing.
#[derive(Clone, Copy, Debug, Default)]
pub struct RObjIterator {
    pub index: usize,
}

impl RObjIterator {
    /// `RObjIterator::Is_Done`.
    pub fn is_done(&self, state: &AssetState) -> bool {
        !(self.index < state.prototypes.len())
    }

    /// `RObjIterator::Current_Item_Name` (assetmgr.cpp:1704): NULL (`None`) past the end.
    pub fn current_item_name<'a>(&self, state: &'a AssetState) -> Option<&'a str> {
        if self.index < state.prototypes.len() {
            state.prototypes[self.index].as_ref().map(|p| p.name.as_str())
        } else {
            None
        }
    }

    /// `RObjIterator::Current_Item_Class_ID` (assetmgr.cpp:1713): -1 past the end.
    pub fn current_item_class_id(&self, state: &AssetState) -> i32 {
        if self.index < state.prototypes.len() {
            state.prototypes[self.index].as_ref().map_or(-1, |p| p.class_id)
        } else {
            -1
        }
    }
}

/// `HAnimIterator`: index only (its accessors are in the port's `AssetManager::anim_names`).
#[derive(Clone, Copy, Debug, Default)]
pub struct HAnimIterator {
    pub index: usize,
}

/// `HTreeIterator` (assetmgr.h).
#[derive(Clone, Copy, Debug, Default)]
pub struct HTreeIterator {
    pub index: usize,
}

impl HTreeIterator {
    /// `HTreeIterator::Is_Done`.
    pub fn is_done<H: AssetHost>(&self, host: &H) -> bool {
        !(self.index < host.htree_count())
    }

    /// `HTreeIterator::Current_Item_Name` (assetmgr.cpp:1727): `HTreeManager.Get_Tree(Index)->Get_Name()`.
    pub fn current_item_name<H: AssetHost>(&self, host: &H) -> String {
        host.htree_name(self.index)
    }
}

/// `Font3DDataIterator`: index over `Font3DDatas`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Font3DDataIterator {
    pub index: usize,
}
