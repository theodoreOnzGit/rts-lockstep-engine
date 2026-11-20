//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_ww3d5_misc.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/static_sort_list.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/static_sort_list.h

use super::handles::RenderObjHandle;
use super::rinfo::RenderInfoClass;
use crate::formats::w3d::w3d_file::MAX_SORT_LEVEL;
use std::collections::VecDeque;

/// What `DefaultStaticSortListClass::Render_And_Clear` asks of a render object and its render hook
/// (`RenderObjClass::Get_Render_Hook`, `RenderHookClass::Pre_Render`/`Post_Render`, `Render`), as a compile-time
/// contract on the owner of the objects.
pub trait StaticSortRenderHost {
    /// `robj->Get_Render_Hook() != NULL`.
    fn has_render_hook(&self, robj: RenderObjHandle) -> bool;
    /// `robj->Get_Render_Hook()->Pre_Render(robj, rinfo)`.
    fn pre_render(&mut self, robj: RenderObjHandle, rinfo: &mut RenderInfoClass) -> bool;
    /// `robj->Get_Render_Hook()->Post_Render(robj, rinfo)`.
    fn post_render(&mut self, robj: RenderObjHandle, rinfo: &mut RenderInfoClass);
    /// `robj->Render(rinfo)`.
    fn render(&mut self, robj: RenderObjHandle, rinfo: &mut RenderInfoClass);
    /// `robj->Release_Ref()`.
    fn release_ref(&mut self, robj: RenderObjHandle);
    /// `TheDX8MeshRenderer.Flush()` — device-only, so the host decides what it is.
    fn flush_mesh_renderer(&mut self);
}

/// `class DefaultStaticSortListClass` (static_sort_list.h); `StaticSortListClass` is its only implementor, so the
/// abstract base is not a separate type. `SortLists` are `RefRenderObjListClass` (FIFO: `Add_Tail`/`Remove_Head`).
#[derive(Clone, Debug)]
pub struct DefaultStaticSortListClass {
    min_sort: u32,
    max_sort: u32,
    sort_lists: Vec<VecDeque<RenderObjHandle>>,
}

impl DefaultStaticSortListClass {
    /// `DefaultStaticSortListClass::DefaultStaticSortListClass` (static_sort_list.cpp:34): MinSort 1, MaxSort
    /// MAX_SORT_LEVEL, `MAX_SORT_LEVEL + 1` lists.
    pub fn new() -> Self {
        DefaultStaticSortListClass {
            min_sort: 1,
            max_sort: MAX_SORT_LEVEL as u32,
            sort_lists: vec![VecDeque::new(); MAX_SORT_LEVEL as usize + 1],
        }
    }

    /// `Get_Min_Sort` (static_sort_list.h).
    pub fn get_min_sort(&self) -> u32 {
        self.min_sort
    }

    /// `Get_Max_Sort` (static_sort_list.h).
    pub fn get_max_sort(&self) -> u32 {
        self.max_sort
    }

    /// `Set_Min_Sort` (static_sort_list.h): capped at MAX_SORT_LEVEL.
    pub fn set_min_sort(&mut self, value: u32) {
        self.min_sort = if value > MAX_SORT_LEVEL as u32 { MAX_SORT_LEVEL as u32 } else { value };
    }

    /// `Set_Max_Sort` (static_sort_list.h): capped at MAX_SORT_LEVEL.
    pub fn set_max_sort(&mut self, value: u32) {
        self.max_sort = if value > MAX_SORT_LEVEL as u32 { if crate::logic::modules::plant(18677) { MAX_SORT_LEVEL as u32 - 1 } else { MAX_SORT_LEVEL as u32 } } else { value };
    }

    /// `DefaultStaticSortListClass::Add_To_List` (static_sort_list.cpp:46): levels outside 1..=MAX_SORT_LEVEL are
    /// dropped (`WWASSERT(0)` in debug builds).
    pub fn add_to_list(&mut self, robj: RenderObjHandle, sort_level: u32) {
        if sort_level < 1 || sort_level > MAX_SORT_LEVEL as u32 || (crate::logic::modules::plant(18676) && sort_level == MAX_SORT_LEVEL as u32) {
            return;
        }
        // Add_Tail(robj, false): no extra Add_Ref
        self.sort_lists[sort_level as usize].push_back(robj);
    }

    /// `DefaultStaticSortListClass::Render_And_Clear` (static_sort_list.cpp:56): from MaxSort down to MinSort (a
    /// lower level is in front, so it renders later); empties each list head first; objects with a render hook are
    /// rendered only if `Pre_Render` says so, and `Post_Render` always runs; then the mesh renderer is flushed
    /// if anything rendered at that level.
    ///
    /// Upstream's loop variable is `unsigned`: `for (sort_level = MaxSort; sort_level >= MinSort; sort_level--)`
    /// with MinSort 0 would never end; that case is kept as an explicit stop at 0 after level 0 is done.
    pub fn render_and_clear<H: StaticSortRenderHost>(&mut self, host: &mut H, rinfo: &mut RenderInfoClass) {
        let mut sort_level = if crate::logic::modules::plant(18675) { self.min_sort } else { self.max_sort };
        loop {
            if crate::logic::modules::plant(18675) {
                if sort_level > self.max_sort {
                    break;
                }
            } else if sort_level < self.min_sort || (crate::logic::modules::plant(18680) && sort_level == self.min_sort) {
                break;
            }
            let mut render = false;
            while let Some(robj) = self.sort_lists[sort_level as usize].pop_front() {
                if host.has_render_hook(robj) {
                    if host.pre_render(robj, rinfo) {
                        host.render(robj, rinfo);
                        render = true;
                    } else if crate::logic::modules::plant(18679) {
                        render = true;
                    }
                    if !(crate::logic::modules::plant(18678) && !render) {
                        host.post_render(robj, rinfo);
                    }
                } else {
                    host.render(robj, rinfo);
                    render = true;
                }
                host.release_ref(robj);
            }
            if render {
                host.flush_mesh_renderer(); // TheDX8MeshRenderer.Flush()
            }
            if crate::logic::modules::plant(18675) {
                sort_level += 1;
                continue;
            }
            if sort_level == 0 {
                break;
            }
            sort_level -= 1;
        }
    }
}

impl Default for DefaultStaticSortListClass {
    fn default() -> Self {
        Self::new()
    }
}
