//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 scene sequences (about 5,000 to 21,000 operations per function) (test: tests/differential_draft_smallw3d.rs);
//! not yet wired into the game.
//! Note: the iterator walks a COPY of the list (upstream walks the live list); the two differ only if the list changes while an iterator is live, which is not tested.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/scene.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/scene.h
//!
//! `SimpleSceneClass` members no earlier pass translated: `Remove_All_Render_Objects`, `Create_Iterator`,
//! `Destroy_Iterator`, `SimpleSceneIterator::Current_Item`. The `RenderList` is [`SimpleScene::render_list`] (host
//! handles in list order, head first); `RefRenderObjListClass::Remove_Head` returns the object with a reference that the
//! caller releases, which is the host's `Release_Ref`.

use crate::logic::modules::plant;

/// What the scene calls on the objects.
pub trait SceneHost {
    /// `SceneClass::Remove_Render_Object(obj)` (scene.cpp:195): `obj->Notify_Removed(this)`. Note
    /// `Remove_All_Render_Objects` calls the BASE version (qualified call), not the virtual one, so the list is not
    /// touched a second time.
    fn scene_remove_render_object_base(&mut self, obj: usize);
    /// `obj->Release_Ref()`.
    fn release_ref(&mut self, obj: usize);
}

/// `SimpleSceneClass`'s `RenderList`.
#[derive(Clone, Debug, Default)]
pub struct SimpleScene {
    pub render_list: Vec<usize>,
}

/// `SimpleSceneIterator` (scene.cpp:675): an iterator over a copy of the list, `OnlyVis` kept but unused ("TODO: make
/// SimpleSceneIterator able to iterate through only the visible nodes").
#[derive(Clone, Debug)]
pub struct SimpleSceneIterator {
    pub objects: Vec<usize>,
    pub position: usize,
    pub only_vis: bool,
}

impl SimpleScene {
    /// `SimpleSceneClass::Remove_All_Render_Objects()` (scene.cpp:361): `while ((obj = RenderList.Remove_Head()) != NULL)`
    /// notify the object it was removed (base `Remove_Render_Object`) and release the reference `Remove_Head` handed over.
    pub fn remove_all_render_objects<H: SceneHost>(&mut self, host: &mut H) {
        while !self.render_list.is_empty() {
            let obj = if plant(13921) { self.render_list.pop().unwrap_or(0) } else { self.render_list.remove(0) };
            if plant(13924) && !plant(13922) {
                host.release_ref(obj);
            }
            if !plant(13923) {
                host.scene_remove_render_object_base(obj);
            }
            if !plant(13922) && !plant(13924) {
                host.release_ref(obj); // remove head gets a ref
            }
        }
    }

    /// `SimpleSceneClass::Create_Iterator(onlyvisible)` (scene.cpp:652): a new `SimpleSceneIterator` over `RenderList`.
    pub fn create_iterator(&self, onlyvisible: bool) -> SimpleSceneIterator {
        if plant(13930) {
            let rev: Vec<usize> = self.render_list.iter().rev().copied().collect();
            return SimpleSceneIterator::new(&rev, onlyvisible);
        }
        SimpleSceneIterator::new(&self.render_list, onlyvisible)
    }

    /// `SimpleSceneClass::Destroy_Iterator(it)` (scene.cpp:671): `delete it`; dropping the value.
    pub fn destroy_iterator(&self, it: SimpleSceneIterator) {
        drop(it);
    }
}

impl SimpleSceneIterator {
    /// `SimpleSceneIterator::SimpleSceneIterator(list, onlyvis)` (scene.cpp:677).
    pub fn new(list: &[usize], onlyvis: bool) -> Self {
        SimpleSceneIterator { objects: list.to_vec(), position: 0, only_vis: onlyvis }
    }

    /// `SimpleSceneIterator::First()`.
    pub fn first(&mut self) {
        if !plant(13928) {
            self.position = 0;
        }
    }

    /// `SimpleSceneIterator::Next()`.
    pub fn next(&mut self) {
        if plant(13926) {
            self.position += 1;
        } else {
            self.position = (self.position + 1) % (self.objects.len() + 1);
        }
    }

    /// `SimpleSceneIterator::Is_Done()`.
    pub fn is_done(&self) -> bool {
        if plant(13927) {
            return self.position + 1 >= self.objects.len();
        }
        if plant(13926) {
            return self.position >= self.objects.len();
        }
        self.position == self.objects.len()
    }

    /// `SimpleSceneIterator::Current_Item()` (scene.cpp:699): `RobjIterator.Peek_Obj()`, the object without a reference
    /// (NULL past the end).
    pub fn current_item(&self) -> Option<usize> {
        if plant(13929) {
            return self.objects.last().copied();
        }
        self.objects.get(self.position).copied()
    }
}
