//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.h
//!
//! `RenderObjClass` members no earlier pass translated. Render objects are opaque host handles (`usize`); every virtual
//! call the base class makes (`Get_Num_Sub_Objects`, `Get_Sub_Object`, `Remove_Sub_Object`, ...) is the
//! [`RenderObjHost`] contract. `Release_Ref` of a returned object is the host's concern.
//!
//! Excluded: `RenderObjClass::Calculate_Texture_Reduction_Factor` (rendobj.cpp:260): inside a `/* ... */` block (dead code;
//! its caller `Prepare_LOD` has the call commented out as well: "Jani: Don't set tex reduction, it's broken!").

/// What the base-class members ask of the object.
pub trait RenderObjHost {
    /// `Get_Num_Sub_Objects_On_Bone(boneindex)`.
    fn get_num_sub_objects_on_bone(&self, boneindex: i32) -> i32;
    /// `Get_Sub_Object_On_Bone(index, boneindex)`: `None` is NULL.
    fn get_sub_object_on_bone(&self, index: i32, boneindex: i32) -> Option<usize>;
    /// `Remove_Sub_Object(robj)`: the number removed.
    fn remove_sub_object(&mut self, robj: usize) -> i32;
    /// `Get_Bone_Index(bname)`.
    fn get_bone_index(&self, bname: &str) -> i32;
    /// `Get_Num_Sub_Objects()`.
    fn get_num_sub_objects(&self) -> i32;
    /// `Get_Sub_Object(index)`: `None` is NULL.
    fn get_sub_object(&self, index: i32) -> Option<usize>;
    /// `psub_obj->Build_Dependency_List(file_list)` (its `recursive` defaults to true).
    fn sub_object_build_dependency_list(&mut self, sub_obj: usize, file_list: &mut Vec<String>) -> bool;
    /// `Add_Dependencies_To_List(file_list, textures_only = false)`.
    fn add_dependencies_to_list(&mut self, file_list: &mut Vec<String>);
}

/// The container / scene links `RenderObjClass` keeps (`Container`, `Scene`): handles, `None` is NULL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderObjLinks {
    pub container: Option<usize>,
    pub scene: Option<usize>,
}

impl RenderObjLinks {
    /// `RenderObjClass::Get_Scene()` (rendobj.cpp:361): the scene, `Add_Ref`'d when not NULL (the caller owns the new
    /// reference; reference counting is the host's).
    pub fn get_scene(&self) -> Option<usize> {
        self.scene
    }

    /// `RenderObjClass::Set_Container(con)` (rendobj.cpp:382): "Either we arent currently in a container or we are
    /// clearing our container, otherwise Houston, there is a problem!" (`WWASSERT((con == NULL) || (Container ==
    /// NULL))`, a debug assertion), then `Container = con`.
    pub fn set_container(&mut self, con: Option<usize>) {
        debug_assert!(con.is_none() || self.container.is_none());
        self.container = con;
    }
}

/// `RenderObjClass::Remove_Sub_Objects_From_Bone(boneindex)` (rendobj.cpp:602): removes every sub-object on the bone,
/// last first (`for (i = count-1; i >= 0; i--)`), returning how many `Remove_Sub_Object` removed.
pub fn remove_sub_objects_from_bone<H: RenderObjHost>(host: &mut H, boneindex: i32) -> i32 {
    let count = host.get_num_sub_objects_on_bone(boneindex);
    let mut remove_count = 0;
    let mut i = count - 1;
    while i >= 0 {
        if let Some(robj) = host.get_sub_object_on_bone(i, boneindex) {
            remove_count += host.remove_sub_object(robj);
            // robj->Release_Ref(): the host's reference
        }
        i -= 1;
    }
    remove_count
}

/// `RenderObjClass::Remove_Sub_Objects_From_Bone(bname)` (rendobj.cpp:629): by bone name.
pub fn remove_sub_objects_from_bone_by_name<H: RenderObjHost>(host: &mut H, bname: &str) -> i32 {
    let boneindex = host.get_bone_index(bname);
    remove_sub_objects_from_bone(host, boneindex)
}

/// `RenderObjClass::Build_Dependency_List(file_list, recursive)` (rendobj.cpp:1052): "Generates a list of files this obj
/// depends on." With `recursive` (the header's default is true) each sub-object adds its own first; then this object's
/// dependencies. True when the list is not empty.
pub fn build_dependency_list<H: RenderObjHost>(host: &mut H, file_list: &mut Vec<String>, recursive: bool) -> bool {
    if recursive {
        // Loop through all this object's subobj's
        let subobj_count = host.get_num_sub_objects();
        for index in 0..subobj_count {
            // Ask this subobj to add all of its file dependencies to the list
            if let Some(psub_obj) = host.get_sub_object(index) {
                host.sub_object_build_dependency_list(psub_obj, file_list);
                // psub_obj->Release_Ref(): the host's reference
            }
        }
    }

    // Now add all of this object's dependencies to the list
    host.add_dependencies_to_list(file_list);

    // Return the true/false result code
    file_list.len() > 0
}
