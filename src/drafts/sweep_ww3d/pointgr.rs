//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/pointgr.cpp
//!
//! `PointGroupClass`' default-value setters and getters and `_Shutdown`. The statics they touch are [`PointGroupDefaults`].
//! **Left to do:** `PointGroupClass::Update_Arrays` (pointgr.cpp:1015-1595), the twelve-way vertex-generation switch
//! over (mode, size, orientation) plus the UV-frame and diffuse loops. It needs the `_Tri/_Quad/_Screenspace` vertex
//! tables built by `_Init`, which this pass did not reach.

/// `PointGroupClass::PointModeEnum`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointModeEnum {
    Tris = 0,
    Quads = 1,
    ScreenSpace = 2,
}

/// The `Default*` statics of `PointGroupClass`.
#[derive(Clone, Debug, PartialEq)]
pub struct PointGroupDefaults {
    pub default_point_size: f32,
    pub default_point_color: [f32; 3],
    pub default_point_alpha: f32,
    pub default_point_orientation: u8,
    pub default_point_frame: u8,
    pub point_mode: PointModeEnum,
    pub billboard: bool,
    pub frame_row_column_count_log2: u8,
}

impl PointGroupDefaults {
    /// `PointGroupClass::Set_Point_Size` (pointgr.cpp:319).
    pub fn set_point_size(&mut self, size: f32) {
        self.default_point_size = size;
    }
    /// `PointGroupClass::Get_Point_Size` (pointgr.cpp:337).
    pub fn get_point_size(&self) -> f32 {
        self.default_point_size
    }
    /// `PointGroupClass::Set_Point_Color`.
    pub fn set_point_color(&mut self, color: [f32; 3]) {
        self.default_point_color = color;
    }
    /// `PointGroupClass::Get_Point_Color` (pointgr.cpp:372).
    pub fn get_point_color(&self) -> [f32; 3] {
        self.default_point_color
    }
    /// `PointGroupClass::Set_Point_Alpha`.
    pub fn set_point_alpha(&mut self, alpha: f32) {
        self.default_point_alpha = alpha;
    }
    /// `PointGroupClass::Get_Point_Alpha` (pointgr.cpp:407).
    pub fn get_point_alpha(&self) -> f32 {
        self.default_point_alpha
    }
    /// `PointGroupClass::Set_Point_Orientation` (pointgr.cpp:427).
    pub fn set_point_orientation(&mut self, orientation: u8) {
        self.default_point_orientation = orientation;
    }
    /// `PointGroupClass::Get_Point_Orientation` (pointgr.cpp:445).
    pub fn get_point_orientation(&self) -> u8 {
        self.default_point_orientation
    }
    /// `PointGroupClass::Set_Point_Frame` (pointgr.cpp:465).
    pub fn set_point_frame(&mut self, frame: u8) {
        self.default_point_frame = frame;
    }
    /// `PointGroupClass::Get_Point_Frame` (pointgr.cpp:483).
    pub fn get_point_frame(&self) -> u8 {
        self.default_point_frame
    }
    /// `PointGroupClass::Set_Point_Mode` (pointgr.cpp:501).
    pub fn set_point_mode(&mut self, mode: PointModeEnum) {
        self.point_mode = mode;
    }
    /// `PointGroupClass::Get_Point_Mode` (pointgr.cpp:519).
    pub fn get_point_mode(&self) -> PointModeEnum {
        self.point_mode
    }
    /// `PointGroupClass::Set_Billboard` (pointgr.cpp:674).
    pub fn set_billboard(&mut self, should_billboard: bool) {
        self.billboard = should_billboard;
    }
    /// `PointGroupClass::Get_Billboard` (pointgr.cpp:691).
    pub fn get_billboard(&self) -> bool {
        self.billboard
    }
    /// `PointGroupClass::Get_Frame_Row_Column_Count_Log2` (pointgr.cpp:709).
    pub fn get_frame_row_column_count_log2(&self) -> u8 {
        self.frame_row_column_count_log2
    }
    /// `PointGroupClass::Set_Frame_Row_Column_Count_Log2` (already translated; shown for its clamp): `MIN(frccl2, 4)`.
    pub fn set_frame_row_column_count_log2(&mut self, frccl2: u8) {
        self.frame_row_column_count_log2 = frccl2.min(4);
    }
}

/// `PointGroupClass::_Shutdown` (pointgr.cpp:1602): frees the five UV frame tables of each kind, releases the material,
/// the sorting / non-sorting quad and triangle index buffers and clears the working arrays. In Rust the tables and
/// buffers are owned values, so shutting down is dropping them; this clears the working vectors as upstream's `Clear()`.
pub fn shutdown(transformed_loc: &mut Vec<[f32; 3]>, vertex_loc: &mut Vec<[f32; 3]>, vertex_diffuse: &mut Vec<u32>, vertex_uv: &mut Vec<[f32; 2]>) {
    transformed_loc.clear();
    vertex_loc.clear();
    vertex_diffuse.clear();
    vertex_uv.clear();
}
