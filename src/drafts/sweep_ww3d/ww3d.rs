//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/ww3d.cpp
//!
//! The `WW3D` static functions that no earlier pass translated. Upstream's `WW3D` is a class of statics; here the statics
//! these functions touch are [`Ww3dState`], and everything that is a call into `DX8Wrapper`, `TheDX8MeshRenderer`, the
//! asset manager's texture hash, `TextureLoader`, `Debug_Statistics` or `BoxRenderObjClass` is the [`Ww3dHost`] contract.
//!
//! Excluded here: `Make_Screen_Shot` (not on the list; DX8 front-buffer capture) and the movie-capture pixel copy
//! (`Update_Movie_Capture` reads the DX8 front buffer, so only its guard is kept, the copy loop is the host's).
//! Not on the list because already translated: `Init`, `Shutdown`, `Begin_Render`, `End_Render`, `Sync`, `Pause_Movie`,
//! `Get_Pixel_Center`, `Enable_Texturing`, `Get_Texture_Reduction`, `Set_Texture_Reduction`, `Enable_Sorting`,
//! `Add_To_Static_Sort_List`, `Set_Gamma`, `Get_Window`.

/// `WW3DErrorType` (ww3d.h), the values these functions return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ww3dErrorType {
    Ok,
    InitializationFailed,
}

/// `WW3D::ScreenShotFormatEnum` (ww3d.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenShotFormat {
    Tga,
    Bmp,
}

/// `WW3D::NPatchesGapFillingModeEnum` (ww3d.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NPatchesGapFillingMode {
    Disabled,
    Enabled,
    Strong,
}

/// `TextureFilterClass::TEXTURE_FILTER_ANISOTROPIC` (texturefilter.h): the largest `TextureFilterMode`.
pub const TEXTURE_FILTER_ANISOTROPIC: i32 = 3;

/// `RenderDeviceDescClass` placeholder: the host owns the real description.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderDeviceDesc {
    pub device_name: String,
}

/// Everything `WW3D` calls out of its own translation unit.
pub trait Ww3dHost {
    // upstream: DX8Wrapper::* — device-only, not translated (the host decides)
    fn dx8_set_render_device_by_name(&mut self, dev_name: &str, width: i32, height: i32, bits: i32, windowed: i32, resize_window: bool) -> bool;
    fn dx8_set_any_render_device(&mut self) -> bool;
    fn dx8_set_render_device(&mut self, dev: i32, width: i32, height: i32, bits: i32, windowed: i32, resize_window: bool, reset_device: bool, restore_assets: bool) -> bool;
    fn dx8_set_next_render_device(&mut self) -> bool;
    fn dx8_is_windowed(&self) -> bool;
    fn dx8_toggle_windowed(&mut self) -> bool;
    fn dx8_get_render_device(&self) -> i32;
    fn dx8_get_render_device_desc(&self, deviceidx: i32) -> RenderDeviceDesc;
    fn dx8_get_render_device_count(&self) -> i32;
    fn dx8_get_render_device_name(&self, device_index: i32) -> String;
    fn dx8_registry_save_render_device(&mut self, sub_key: &str) -> bool;
    fn dx8_registry_save_render_device_with(&mut self, sub_key: &str, device: i32, width: i32, height: i32, depth: i32, windowed: bool, texture_depth: i32) -> bool;
    fn dx8_registry_load_render_device(&mut self, sub_key: &str, resize_window: bool) -> bool;
    /// Fills `(device, width, height, depth, windowed, texture_depth)`; `None` is `false`.
    fn dx8_registry_load_render_device_values(&mut self, sub_key: &str, device_len: i32) -> Option<(String, i32, i32, i32, i32, i32)>;
    fn dx8_flip_to_primary(&mut self);
    fn dx8_set_swap_interval(&mut self, swap: i32);
    fn dx8_get_swap_interval(&self) -> i32;
    fn dx8_device_resolution_width(&self) -> i32;
    fn dx8_device_resolution_height(&self) -> i32;
    fn dx8_set_texture_bitdepth(&mut self, bitdepth: i32);
    fn dx8_get_texture_bitdepth(&self) -> i32;
    /// `TheDX8MeshRenderer.Invalidate()`.
    fn mesh_renderer_invalidate(&mut self);
    /// `TextureLoader::Flush_Pending_Load_Tasks()`.
    fn texture_loader_flush_pending_load_tasks(&mut self);
    /// `WW3DAssetManager::Get_Instance()` is non-null.
    fn asset_manager_exists(&self) -> bool;
    /// Walks `Texture_Hash()` and calls `TextureClass::Invalidate` on each.
    fn asset_manager_invalidate_all_textures(&mut self);
    /// `TextureFilterClass::_Init_Filters(mode)`.
    fn texture_filter_init_filters(&mut self, mode: i32);
    /// `Debug_Statistics::Get_DX8_Polygons()`.
    fn debug_statistics_dx8_polygons(&self) -> u32;
    /// `Debug_Statistics::Get_DX8_Vertices()`.
    fn debug_statistics_dx8_vertices(&self) -> u32;
    /// `BoxRenderObjClass::Set_Box_Display_Mask`.
    fn box_set_display_mask(&mut self, mask: i32);
    /// `BoxRenderObjClass::Get_Box_Display_Mask`.
    fn box_get_display_mask(&self) -> i32;
    /// `new FrameGrabClass(filename_base, AVI, width, height, depth, frame_rate)`; the movie is then "ready".
    fn frame_grab_create(&mut self, filename_base: &str, width: i32, height: i32, depth: i32, frame_rate: f32);
    /// `delete Movie`.
    fn frame_grab_destroy(&mut self);
    /// `Movie->GetFrameRate()`.
    fn frame_grab_frame_rate(&self) -> f32;
    /// `GetWindowRect(_Hwnd, ..)` width and height.
    fn window_size(&self) -> (i32, i32);
    /// The front-buffer copy and `Movie->Grab(image)` of `Update_Movie_Capture`.
    fn grab_front_buffer_frame(&mut self);
    /// `StaticSortListClass::Render_And_Clear(rinfo)` on the current lists.
    fn static_sort_lists_render_and_clear(&mut self);
}

/// The `WW3D` statics these functions read and write.
#[derive(Clone, Debug, PartialEq)]
pub struct Ww3dState {
    pub n_patches_gap_filling_mode: NPatchesGapFillingMode,
    pub n_patches_level: u32,
    pub texture_filter: i32,
    pub is_capturing: bool,
    pub pause_record: bool,
    pub record_next_frame: bool,
    /// `Movie != NULL`.
    pub movie: bool,
    pub is_coloring_enabled: bool,
    pub large_texture_extra_reduction_enabled: bool,
    pub texture_reduction: i32,
    pub texture_min_dim: i32,
    pub pixel_center_x: f32,
    pub pixel_center_y: f32,
    pub are_static_sort_lists_enabled: bool,
    /// `CurrentStaticSortLists != DefaultStaticSortLists`.
    pub static_sort_lists_overridden: bool,
}

impl Default for Ww3dState {
    fn default() -> Self {
        Ww3dState {
            n_patches_gap_filling_mode: NPatchesGapFillingMode::Disabled,
            n_patches_level: 1,
            texture_filter: 0,
            is_capturing: false,
            pause_record: false,
            record_next_frame: false,
            movie: false,
            is_coloring_enabled: false,
            large_texture_extra_reduction_enabled: false,
            texture_reduction: 0,
            texture_min_dim: 0,
            pixel_center_x: 0.0,
            pixel_center_y: 0.0,
            are_static_sort_lists_enabled: true,
            static_sort_lists_overridden: false,
        }
    }
}

fn to_error(success: bool) -> Ww3dErrorType {
    if success {
        Ww3dErrorType::Ok
    } else {
        Ww3dErrorType::InitializationFailed
    }
}

impl Ww3dState {
    /// `WW3D::Set_NPatches_Gap_Filling_Mode` (ww3d.cpp:239).
    pub fn set_n_patches_gap_filling_mode<H: Ww3dHost>(&mut self, host: &mut H, mode: NPatchesGapFillingMode) {
        if self.n_patches_gap_filling_mode != mode {
            self.n_patches_gap_filling_mode = mode;
            host.mesh_renderer_invalidate();
        }
    }

    /// `WW3D::Set_NPatches_Level` (ww3d.cpp:247).
    pub fn set_n_patches_level<H: Ww3dHost>(&mut self, host: &mut H, mut level: u32) {
        if level > 8 {
            level = 8;
        }
        if level < 1 {
            level = 1;
        }
        if self.n_patches_level == 1 && level > 1 {
            host.mesh_renderer_invalidate();
        }
        if self.n_patches_level > 1 && level == 1 {
            host.mesh_renderer_invalidate();
        }
        self.n_patches_level = level;
    }

    /// `WW3D::Set_Render_Device(const char*, ...)` (ww3d.cpp:398).
    pub fn set_render_device_by_name<H: Ww3dHost>(&mut self, host: &mut H, dev_name: &str, width: i32, height: i32, bits: i32, windowed: i32, resize_window: bool) -> Ww3dErrorType {
        to_error(host.dx8_set_render_device_by_name(dev_name, width, height, bits, windowed, resize_window))
    }

    /// `WW3D::Set_Any_Render_Device` (ww3d.cpp:421).
    pub fn set_any_render_device<H: Ww3dHost>(&mut self, host: &mut H) -> Ww3dErrorType {
        to_error(host.dx8_set_any_render_device())
    }

    /// `WW3D::Set_Render_Device(int, ...)` (ww3d.cpp:444).
    pub fn set_render_device<H: Ww3dHost>(&mut self, host: &mut H, dev: i32, width: i32, height: i32, bits: i32, windowed: i32, resize_window: bool, reset_device: bool, restore_assets: bool) -> Ww3dErrorType {
        to_error(host.dx8_set_render_device(dev, width, height, bits, windowed, resize_window, reset_device, restore_assets))
    }

    /// `WW3D::Set_Next_Render_Device` (ww3d.cpp:467).
    pub fn set_next_render_device<H: Ww3dHost>(&mut self, host: &mut H) -> Ww3dErrorType {
        to_error(host.dx8_set_next_render_device())
    }

    /// `WW3D::Is_Windowed` (ww3d.cpp:506).
    pub fn is_windowed<H: Ww3dHost>(&self, host: &H) -> bool {
        host.dx8_is_windowed()
    }

    /// `WW3D::Toggle_Windowed` (ww3d.cpp:526).
    pub fn toggle_windowed<H: Ww3dHost>(&mut self, host: &mut H) -> Ww3dErrorType {
        to_error(host.dx8_toggle_windowed())
    }

    /// `WW3D::Get_Render_Device` (ww3d.cpp:550).
    pub fn get_render_device<H: Ww3dHost>(&self, host: &H) -> i32 {
        host.dx8_get_render_device()
    }

    /// `WW3D::Get_Render_Device_Desc` (ww3d.cpp:569).
    pub fn get_render_device_desc<H: Ww3dHost>(&self, host: &H, deviceidx: i32) -> RenderDeviceDesc {
        host.dx8_get_render_device_desc(deviceidx)
    }

    /// `WW3D::Get_Render_Device_Count` (ww3d.cpp:589).
    pub fn get_render_device_count<H: Ww3dHost>(&self, host: &H) -> i32 {
        host.dx8_get_render_device_count()
    }

    /// `WW3D::Get_Render_Device_Name` (ww3d.cpp:608).
    pub fn get_render_device_name<H: Ww3dHost>(&self, host: &H, device_index: i32) -> String {
        host.dx8_get_render_device_name(device_index)
    }

    /// `WW3D::Registry_Save_Render_Device(const char*)` (ww3d.cpp:689).
    pub fn registry_save_render_device<H: Ww3dHost>(&mut self, host: &mut H, sub_key: &str) -> Ww3dErrorType {
        to_error(host.dx8_registry_save_render_device(sub_key))
    }

    /// `WW3D::Registry_Save_Render_Device(sub_key, device, ...)` (ww3d.cpp:711).
    pub fn registry_save_render_device_with<H: Ww3dHost>(&mut self, host: &mut H, sub_key: &str, device: i32, width: i32, height: i32, depth: i32, windowed: bool, texture_depth: i32) -> Ww3dErrorType {
        to_error(host.dx8_registry_save_render_device_with(sub_key, device, width, height, depth, windowed, texture_depth))
    }

    /// `WW3D::Registry_Load_Render_Device(sub_key, resize_window)` (ww3d.cpp:734).
    pub fn registry_load_render_device<H: Ww3dHost>(&mut self, host: &mut H, sub_key: &str, resize_window: bool) -> Ww3dErrorType {
        to_error(host.dx8_registry_load_render_device(sub_key, resize_window))
    }

    /// `WW3D::Registry_Load_Render_Device(sub_key, device, device_len, ...)` (ww3d.cpp:744): the out parameters are the
    /// returned tuple, `None` is `false`.
    pub fn registry_load_render_device_values<H: Ww3dHost>(&mut self, host: &mut H, sub_key: &str, device_len: i32) -> Option<(String, i32, i32, i32, i32, i32)> {
        host.dx8_registry_load_render_device_values(sub_key, device_len)
    }

    /// `WW3D::_Invalidate_Mesh_Cache` (ww3d.cpp:749).
    pub fn invalidate_mesh_cache<H: Ww3dHost>(&mut self, host: &mut H) {
        host.mesh_renderer_invalidate();
    }

    /// `WW3D::_Invalidate_Textures` (ww3d.cpp:754): flush the loader, then invalidate every texture in the manager's hash.
    pub fn invalidate_textures<H: Ww3dHost>(&mut self, host: &mut H) {
        if !host.asset_manager_exists() {
            return;
        }
        host.texture_loader_flush_pending_load_tasks();
        host.asset_manager_invalidate_all_textures();
    }

    /// `WW3D::Set_Texture_Filter` (ww3d.cpp:770).
    pub fn set_texture_filter<H: Ww3dHost>(&mut self, host: &mut H, mut texture_filter: i32) {
        if texture_filter < 0 {
            texture_filter = 0;
        }
        if texture_filter > TEXTURE_FILTER_ANISOTROPIC {
            texture_filter = TEXTURE_FILTER_ANISOTROPIC;
        }
        self.texture_filter = texture_filter;
        host.texture_filter_init_filters(self.texture_filter);
    }

    /// `WW3D::Flip_To_Primary` (ww3d.cpp:1139).
    pub fn flip_to_primary<H: Ww3dHost>(&mut self, host: &mut H) {
        host.dx8_flip_to_primary();
    }

    /// `WW3D::Get_Last_Frame_Poly_Count` (ww3d.cpp:1157).
    pub fn get_last_frame_poly_count<H: Ww3dHost>(&self, host: &H) -> u32 {
        host.debug_statistics_dx8_polygons()
    }

    /// `WW3D::Get_Last_Frame_Vertex_Count` (ww3d.cpp:1162).
    pub fn get_last_frame_vertex_count<H: Ww3dHost>(&self, host: &H) -> u32 {
        host.debug_statistics_dx8_vertices()
    }

    /// `WW3D::Set_Ext_Swap_Interval` (ww3d.cpp:1199).
    pub fn set_ext_swap_interval<H: Ww3dHost>(&mut self, host: &mut H, swap: i32) {
        host.dx8_set_swap_interval(swap);
    }

    /// `WW3D::Get_Ext_Swap_Interval` (ww3d.cpp:1217).
    pub fn get_ext_swap_interval<H: Ww3dHost>(&self, host: &H) -> i32 {
        host.dx8_get_swap_interval()
    }

    /// `WW3D::Set_Collision_Box_Display_Mask` (ww3d.cpp:1235).
    pub fn set_collision_box_display_mask<H: Ww3dHost>(&mut self, host: &mut H, mask: i32) {
        host.box_set_display_mask(mask);
    }

    /// `WW3D::Get_Collision_Box_Display_Mask` (ww3d.cpp:1252).
    pub fn get_collision_box_display_mask<H: Ww3dHost>(&self, host: &H) -> i32 {
        host.box_get_display_mask()
    }

    /// `WW3D::Normalize_Coordinates` (ww3d.cpp:1273): clip back into the screen resolution (`Bound(x, 0, width)`), then
    /// divide by it. Returns `(fx, fy)`.
    pub fn normalize_coordinates<H: Ww3dHost>(&self, host: &H, x: i32, y: i32) -> (f32, f32) {
        let w = host.dx8_device_resolution_width();
        let h = host.dx8_device_resolution_height();
        // Bound(x, 0, w): Min(Max(x, 0), w)
        let x = x.max(0).min(w);
        let y = y.max(0).min(h);
        (x as f32 / w as f32, y as f32 / h as f32)
    }

    /// `WW3D::Start_Movie_Capture` (ww3d.cpp:1474), the `_WINDOWS` branch.
    pub fn start_movie_capture<H: Ww3dHost>(&mut self, host: &mut H, filename_base: &str, mut frame_rate: f32) {
        if self.is_capturing {
            self.stop_movie_capture(host);
        }
        self.is_capturing = true;
        let (width, height) = host.window_size();
        let depth = 24;
        if frame_rate == 0.0 {
            frame_rate = 1.0;
            self.pause_record = true;
        } else {
            self.pause_record = false;
        }
        host.frame_grab_create(filename_base, width, height, depth, frame_rate);
        self.movie = true;
    }

    /// `WW3D::Stop_Movie_Capture` (ww3d.cpp:1517).
    pub fn stop_movie_capture<H: Ww3dHost>(&mut self, host: &mut H) {
        if self.is_capturing {
            self.is_capturing = false;
            host.frame_grab_destroy();
            self.movie = false;
        }
    }

    /// `WW3D::Toggle_Movie_Capture` (ww3d.cpp:1544).
    pub fn toggle_movie_capture<H: Ww3dHost>(&mut self, host: &mut H, filename_base: &str, frame_rate: f32) {
        if self.is_capturing {
            self.stop_movie_capture(host);
        } else {
            self.start_movie_capture(host, filename_base, frame_rate);
        }
    }

    /// `WW3D::Start_Single_Frame_Movie_Capture` (ww3d.cpp:1566): a zero frame rate means "paused, one frame at a time".
    pub fn start_single_frame_movie_capture<H: Ww3dHost>(&mut self, host: &mut H, filename_base: &str) {
        self.start_movie_capture(host, filename_base, 0.0);
    }

    /// `WW3D::Capture_Next_Movie_Frame` (ww3d.cpp:1584).
    pub fn capture_next_movie_frame(&mut self) {
        self.record_next_frame = true;
    }

    /// `WW3D::Is_Movie_Paused` (ww3d.cpp:1620).
    pub fn is_movie_paused(&self) -> bool {
        self.pause_record
    }

    /// `WW3D::Is_Recording_Next_Frame` (ww3d.cpp:1638).
    pub fn is_recording_next_frame(&self) -> bool {
        self.movie && (!self.pause_record || self.record_next_frame)
    }

    /// `WW3D::Is_Movie_Ready` (ww3d.cpp:1656).
    pub fn is_movie_ready(&self) -> bool {
        self.movie
    }

    /// `WW3D::Update_Movie_Capture` (ww3d.cpp:1675): asserts `IsCapturing`; the front-buffer copy (BGRA to bottom-up BGR)
    /// and `Movie->Grab` are the host's.
    pub fn update_movie_capture<H: Ww3dHost>(&mut self, host: &mut H) {
        debug_assert!(self.is_capturing);
        // upstream: DX8Wrapper::_Get_DX8_Front_Buffer / LockRect — device-only, not translated
        host.grab_front_buffer_frame();
    }

    /// `WW3D::Enable_Coloring` (ww3d.cpp:1777).
    pub fn enable_coloring(&mut self, color: u32) {
        self.is_coloring_enabled = color != 0;
    }

    /// `WW3D::Enable_Large_Texture_Extra_Reduction` (ww3d.cpp:1816).
    pub fn enable_large_texture_extra_reduction<H: Ww3dHost>(&mut self, host: &mut H, onoff: bool) {
        if self.large_texture_extra_reduction_enabled != onoff {
            self.large_texture_extra_reduction_enabled = onoff;
            self.invalidate_textures(host);
        }
    }

    /// `WW3D::Is_Large_Texture_Extra_Reduction_Enabled` (ww3d.cpp:1824).
    pub fn is_large_texture_extra_reduction_enabled(&self) -> bool {
        self.large_texture_extra_reduction_enabled
    }

    /// `WW3D::Peek_Default_Debug_Material` (ww3d.cpp:1841): only exists under `WWDEBUG`; the release build returns NULL.
    pub fn peek_default_debug_material(&self) -> Option<()> {
        None
    }

    /// `WW3D::Allocate_Debug_Resources` (ww3d.cpp:1897): body is `#ifdef WWDEBUG`, empty in release.
    pub fn allocate_debug_resources(&mut self) {}

    /// `WW3D::Release_Debug_Resources` (ww3d.cpp:1923): body is `#ifdef WWDEBUG`, empty in release.
    pub fn release_debug_resources(&mut self) {}

    /// `WW3D::On_Deactivate_App` (ww3d.cpp:1932).
    pub fn on_deactivate_app<H: Ww3dHost>(&mut self, host: &mut H) -> Ww3dErrorType {
        self.invalidate_textures(host);
        self.invalidate_mesh_cache(host);
        Ww3dErrorType::Ok
    }

    /// `WW3D::On_Activate_App` (ww3d.cpp:1941).
    pub fn on_activate_app(&mut self) -> Ww3dErrorType {
        Ww3dErrorType::Ok
    }

    /// `WW3D::Update_Pixel_Center` (ww3d.cpp:1953), the `WW3D_DX8` branch, from the device's short name.
    pub fn update_pixel_center(&mut self, name: &str) {
        if name.contains("OpenGL") {
            self.pixel_center_x = 0.0;
            self.pixel_center_y = 0.0;
        } else if name.contains("Glide") {
            self.pixel_center_x = 0.0;
            self.pixel_center_y = 0.0;
        } else if name.contains("DirectX") {
            self.pixel_center_x = 0.5;
            self.pixel_center_y = 0.5;
        } else if name.contains("Software") {
            self.pixel_center_x = 0.0;
            self.pixel_center_y = 0.0;
        } else if name.contains("Null") {
            self.pixel_center_x = 0.0;
            self.pixel_center_y = 0.0;
        } else {
            self.pixel_center_x = 0.0;
            self.pixel_center_y = 0.0;
        }
    }

    /// `WW3D::Set_Texture_Bitdepth` (ww3d.cpp:1974).
    pub fn set_texture_bitdepth<H: Ww3dHost>(&mut self, host: &mut H, bitdepth: i32) {
        host.dx8_set_texture_bitdepth(bitdepth);
    }

    /// `WW3D::Get_Texture_Bitdepth` (ww3d.cpp:1979).
    pub fn get_texture_bitdepth<H: Ww3dHost>(&self, host: &H) -> i32 {
        host.dx8_get_texture_bitdepth()
    }

    /// `WW3D::Render_And_Clear_Static_Sort_Lists` (ww3d.cpp:1989): the lists are disabled while rendering from them,
    /// otherwise `Render()` puts the objects straight back on the same lists.
    pub fn render_and_clear_static_sort_lists<H: Ww3dHost>(&mut self, host: &mut H) {
        let old_enable = self.are_static_sort_lists_enabled;
        self.are_static_sort_lists_enabled = false;
        host.static_sort_lists_render_and_clear();
        self.are_static_sort_lists_enabled = old_enable;
    }

    /// `WW3D::Override_Current_Static_Sort_Lists` (ww3d.cpp:2007): a NULL list asserts and changes nothing.
    pub fn override_current_static_sort_lists(&mut self, has_list: bool) {
        if has_list {
            self.static_sort_lists_overridden = true;
        } else {
            debug_assert!(has_list);
        }
    }

    /// `WW3D::Reset_Current_Static_Sort_Lists_To_Default` (ww3d.cpp:2017).
    pub fn reset_current_static_sort_lists_to_default(&mut self) {
        self.static_sort_lists_overridden = false;
    }
}
