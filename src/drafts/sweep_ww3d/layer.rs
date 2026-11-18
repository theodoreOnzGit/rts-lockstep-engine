//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/layer.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/layer.h
//!
//! `LayerClass` accessors no earlier pass translated: `Set_Scene`, `Get_Scene`, `Peek_Camera`. Scenes and cameras are
//! opaque host handles (`usize`); the `Add_Ref` / `Release_Ref` pairs upstream are reference counting, reported to the
//! host through [`LayerRefHost`].

/// Reference counting of the scene (`Scene->Add_Ref()` / `Release_Ref()`).
pub trait LayerRefHost {
    /// `Scene->Add_Ref()`.
    fn scene_add_ref(&mut self, scene: usize);
    /// `Scene->Release_Ref()`.
    fn scene_release_ref(&mut self, scene: usize);
}

/// `LayerClass`'s `Scene` and `Camera` (`None` is NULL).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerClass {
    pub scene: Option<usize>,
    pub camera: Option<usize>,
}

impl LayerClass {
    /// `LayerClass::Set_Scene(scene)` (layer.cpp:156): releases the old scene, takes the new one and adds a reference to
    /// it. (Setting the scene it already has releases then re-adds, as upstream.)
    pub fn set_scene<H: LayerRefHost>(&mut self, host: &mut H, scene: Option<usize>) {
        if let Some(old) = self.scene {
            host.scene_release_ref(old);
        }
        self.scene = scene;
        if let Some(new) = self.scene {
            host.scene_add_ref(new);
        }
    }

    /// `LayerClass::Get_Scene()` (layer.cpp:180): the scene with an `Add_Ref` when not NULL.
    pub fn get_scene<H: LayerRefHost>(&self, host: &mut H) -> Option<usize> {
        if let Some(s) = self.scene {
            host.scene_add_ref(s);
        }
        self.scene
    }

    /// `LayerClass::Peek_Camera()` (layer.cpp:264): the camera without a reference.
    pub fn peek_camera(&self) -> Option<usize> {
        self.camera
    }
}
