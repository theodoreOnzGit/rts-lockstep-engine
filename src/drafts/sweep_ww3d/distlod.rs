//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/distlod.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/distlod.h
//!
//! `DistLODDefClass::read_node` and the `DistLODClass` members no earlier pass translated: `Get_Num_Snap_Points`,
//! `Get_Snap_Point`, `Update_Lod` (with the private `Increment_Lod` and `Decrement_Lod` it calls, which the sweep list did
//! not carry). The verified `formats::w3d::distlod` loads the definition; [`DistLodState`] is the runtime `Lods` /
//! `CurLod` / `LodCount`. Models are opaque host handles; their virtuals are the [`DistLodHost`] contract.

use crate::ww::vector3::Vector3;

/// `DistLODClass::LODNodeClass`.
#[derive(Clone, Copy, Debug, Default)]
pub struct LodNode {
    /// `Model` (a host handle).
    pub model: usize,
    /// `ResUpDist`.
    pub res_up_dist: f32,
    /// `ResDownDist`.
    pub res_down_dist: f32,
}

/// `DistLODClass`'s level list and current level.
#[derive(Clone, Debug, Default)]
pub struct DistLodState {
    /// `Lods[0..LodCount]`.
    pub lods: Vec<LodNode>,
    /// `CurLod`.
    pub cur_lod: i32,
}

/// What the members call on the models and the scene.
pub trait DistLodHost {
    /// `Lods[0].Model->Get_Num_Snap_Points()`.
    fn model_get_num_snap_points(&self, model: usize) -> i32;
    /// `Lods[0].Model->Get_Snap_Point(index, set)`.
    fn model_get_snap_point(&self, model: usize, index: i32, set: &mut Vector3);
    /// `Lods[CurLod].Model->Get_Bounding_Sphere().Center`.
    fn model_bounding_sphere_center(&self, model: usize) -> Vector3;
    /// `Is_In_Scene()`.
    fn is_in_scene(&self) -> bool;
    /// `model->Notify_Removed(Scene)`.
    fn model_notify_removed(&mut self, model: usize);
    /// `model->Notify_Added(Scene)`.
    fn model_notify_added(&mut self, model: usize);
}

/// `DistLODDefClass::read_node(cload, node)` (distlod.cpp:371): the body is `return true;` - it reads nothing (the
/// nodes are read by `Load_W3D` itself; this helper is an unused leftover). Kept as the same no-op.
pub fn read_node() -> bool {
    true
}

impl DistLodState {
    /// `DistLODClass::Get_Num_Snap_Points()` (distlod.cpp:1019): the highest-detail model's count.
    pub fn get_num_snap_points<H: DistLodHost>(&self, host: &H) -> i32 {
        host.model_get_num_snap_points(self.lods[0].model)
    }

    /// `DistLODClass::Get_Snap_Point(index, set)` (distlod.cpp:1037): the highest-detail model's point.
    pub fn get_snap_point<H: DistLodHost>(&self, host: &H, index: i32, set: &mut Vector3) {
        host.model_get_snap_point(self.lods[0].model, index, set);
    }

    /// `DistLODClass::Update_Lod(camera)` (distlod.cpp:1097): "evaluate the distance from the camera and select an LOD
    /// based on it": `Quick_Length` of the camera-to-model-sphere-centre vector; closer than the current level's
    /// `ResUpDist` goes up one level of detail, farther than its `ResDownDist` down one.
    pub fn update_lod<H: DistLodHost>(&mut self, host: &mut H, camera_position: &Vector3) {
        let center = host.model_bounding_sphere_center(self.lods[self.cur_lod as usize].model);
        let dist = (*camera_position - center).quick_length();

        if dist < self.lods[self.cur_lod as usize].res_up_dist {
            self.increment_lod(host);
        } else if dist > self.lods[self.cur_lod as usize].res_down_dist {
            self.decrement_lod(host);
        }
    }

    /// `DistLODClass::Increment_Lod()` (distlod.cpp:1125): "TODO: change the order in which models are stored"; one level
    /// toward 0, with the scene notifications around the switch.
    pub fn increment_lod<H: DistLodHost>(&mut self, host: &mut H) {
        if self.cur_lod > 0 {
            if host.is_in_scene() {
                host.model_notify_removed(self.lods[self.cur_lod as usize].model);
            }
            self.cur_lod -= 1;
            if host.is_in_scene() {
                host.model_notify_added(self.lods[self.cur_lod as usize].model);
            }
        }
    }

    /// `DistLODClass::Decrement_Lod()` (distlod.cpp:1152): one level away from 0, below `LodCount - 1`.
    pub fn decrement_lod<H: DistLodHost>(&mut self, host: &mut H) {
        if self.cur_lod < self.lods.len() as i32 - 1 {
            if host.is_in_scene() {
                host.model_notify_removed(self.lods[self.cur_lod as usize].model);
            }
            self.cur_lod += 1;
            if host.is_in_scene() {
                host.model_notify_added(self.lods[self.cur_lod as usize].model);
            }
        }
    }
}
