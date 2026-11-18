//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/mapper.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/mapper.h
//!
//! `GridTextureMapperClass`: `Set_Frame_Per_Second`, `update_temporal_state`, `calculate_uv_offset` (and `initialize`,
//! which `Set_Frame_Per_Second` calls and which the sweep list did not carry). The mapper's sync time is passed in
//! (`WW3D::Get_Sync_Time()`).
//!
//! Quirk kept: in `update_temporal_state`, `new_frame % LastFrame` mixes an `int` with an `unsigned int`, so C++ converts
//! `new_frame` to unsigned first; a negative frame (a reversed animation stepping below 0) is taken modulo `LastFrame` as a
//! huge unsigned number, not as a mathematical remainder, and the `new_frame < 0` branch below it is never taken. The draft
//! does the same with `as u32`.

/// `GridTextureMapperClass`'s state.
#[derive(Clone, Copy, Debug, Default)]
pub struct GridTextureMapperClass {
    /// `Sign`: +1 if frame rate positive, -1 if negative, 0 for a still frame.
    pub sign: i32,
    /// `MSPerFrame`.
    pub ms_per_frame: u32,
    /// `OOGridWidth`: 1 / the side of the grid.
    pub oo_grid_width: f32,
    /// `GridWidthLog2`.
    pub grid_width_log2: u32,
    /// `LastFrame`.
    pub last_frame: u32,
    /// `Offset`.
    pub offset: u32,
    /// `Remainder`.
    pub remainder: u32,
    /// `CurrentFrame`.
    pub current_frame: u32,
    /// `LastUsedSyncTime`.
    pub last_used_sync_time: u32,
}

impl GridTextureMapperClass {
    /// `GridTextureMapperClass::Set_Frame_Per_Second(fps)` (mapper.cpp:267): `initialize(fps, GridWidthLog2)`.
    pub fn set_frame_per_second(&mut self, fps: f32, sync_time: u32) {
        let log2 = self.grid_width_log2;
        self.initialize(fps, log2, sync_time);
    }

    /// `GridTextureMapperClass::initialize(fps, gridwidth_log2)` (mapper.cpp:271). A zero frame rate gives `Sign = 0` and a
    /// nonzero `MSPerFrame` so the division in `update_temporal_state` is defined.
    pub fn initialize(&mut self, fps: f32, gridwidth_log2: u32, sync_time: u32) {
        let grid_width: u32 = 1u32 << gridwidth_log2;

        if self.last_frame == 0 {
            self.last_frame = grid_width.wrapping_mul(grid_width);
        }
        self.offset = self.offset % self.last_frame;
        self.last_used_sync_time = sync_time;
        self.grid_width_log2 = gridwidth_log2;
        self.oo_grid_width = 1.0 / (grid_width as f32);
        if fps == 0.0 {
            // Value of MSPerFrame does not matter as long as it is not 0 - sign will multiply results, zeroing them out.
            self.sign = 0;
            self.ms_per_frame = 1;
            self.current_frame = self.offset;
        } else if fps < 0.0 {
            self.sign = -1;
            self.ms_per_frame = (1000.0 / fps.abs()) as u32;
            self.current_frame = (self.last_frame - 1).wrapping_sub(self.offset);
        } else {
            self.sign = 1;
            self.ms_per_frame = (1000.0 / fps.abs()) as u32;
            self.current_frame = self.offset;
        }
        self.remainder = 0;
    }

    /// `GridTextureMapperClass::update_temporal_state()` (mapper.cpp:299): advance the frame by the whole frame periods
    /// elapsed since the last use; the leftover time carries in `Remainder`.
    pub fn update_temporal_state(&mut self, now: u32) {
        let delta = now.wrapping_sub(self.last_used_sync_time);
        self.remainder = self.remainder.wrapping_add(delta);
        self.last_used_sync_time = now;

        let step = ((self.remainder / self.ms_per_frame) as i32).wrapping_mul(self.sign);
        let mut new_frame = (self.current_frame as i32).wrapping_add(step);
        // `new_frame % LastFrame` is done in unsigned arithmetic (see the module note)
        new_frame = ((new_frame as u32) % self.last_frame) as i32;

        if new_frame < 0 {
            self.current_frame = self.last_frame.wrapping_add(new_frame as u32);
        } else {
            self.current_frame = new_frame as u32;
        }
        self.remainder = self.remainder % self.ms_per_frame;
    }

    /// `GridTextureMapperClass::calculate_uv_offset(&u, &v)` (mapper.cpp:317): the frame's column and row in the grid, each
    /// times `OOGridWidth`.
    pub fn calculate_uv_offset(&self) -> (f32, f32) {
        let row_mask: u32 = !(0xFFFF_FFFFu32.wrapping_shl(self.grid_width_log2));
        let col_mask: u32 = row_mask.wrapping_shl(self.grid_width_log2);
        let x = self.current_frame & row_mask;
        let y = (self.current_frame & col_mask) >> self.grid_width_log2;
        (x as f32 * self.oo_grid_width, y as f32 * self.oo_grid_width)
    }
}
