//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texturefile.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texturefile.h
//!
//! `TextureFileClass`: the Surrender-library (`srTexture`) texture class that loads from a file, keeps an optional
//! "locked surface" at a reduced size, and switches between reduction levels (powers of two by which a texture is
//! shrunk) as the screen size of the object changes, with hysteresis.
//!
//! **This file is compiled out upstream.** The whole of `texturefile.h` / `.cpp` sits inside `#ifdef WW3D_DX8`, a macro
//! that is defined nowhere in the source tree, and it derives from `srTexture`, which comes from the old Surrender library
//! (`srTexture.hpp`, `srCore.hpp`, ...) that is not in the release either. The game's textures are `TextureClass`
//! (texture.cpp). It is drafted for completeness: the reduction state machine, the size statistics and the registry
//! (`head` list) are translated as data and plain logic; the Surrender surface objects are [`SrSurface`] byte buffers, and the
//! texture loader, the file cache and the frame handle are the [`TextureFileHost`] contract.
//!
//! **Finding.** `List_Missing_Files` never increments `missing_count`, so it always returns "No missing textures" (the
//! per-file lines are built and then overwritten).

use std::collections::BTreeMap;

/// A `srColorSurfaceIFace`: pitch, height and the bytes (what `Calculate_Size` reads).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SrSurface {
    pub pitch: i32,
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
}

/// `Calculate_Size(surface, red_factor)`: the bytes of the surface and its mip chain, summed as `pitch * height` while the
/// height halves to zero, starting from `pitch << red_factor` by `height << red_factor`.
pub fn calculate_size(surface: &SrSurface, red_factor: u32) -> i32 {
    let mut p = surface.pitch;
    let mut h = surface.height;
    let mut size = 0i32;
    p <<= red_factor;
    h <<= red_factor;
    while h != 0 {
        size += p * h;
        p >>= 1;
        h >>= 1;
    }
    size
}

/// What the file class asks of the texture loader, the file cache and the frame handle.
pub trait TextureFileHost {
    /// `TextureLoader::Texture_File_Exists(name)`.
    fn texture_file_exists(&mut self, name: &str) -> bool;
    /// `TextureLoader::Load_Locked_Surface_Immediate(name, surface, reduction_factor)`.
    fn load_locked_surface_immediate(&mut self, name: &str) -> Option<(SrSurface, u32)>;
    /// `TextureLoader::Add_Load_Task(this)`: load the surface at `texture_loader_info.reduction_factor` in the background.
    fn add_load_task(&mut self, texture_id: i32, reduction_factor: u32);
    /// `WW3DAssetManager::Texture_File_Cache()` is non-NULL: `Validate_Texture(name)` then `Get_Surface(name, 0)`; else
    /// `::Load_Surface(name)`.
    fn load_surface(&mut self, name: &str) -> Option<SrSurface>;
    /// `WW3D::Get_Texture_Thumbnail_Mode() == TEXTURE_THUMBNAIL_MODE_RESIZING`.
    fn thumbnail_mode_is_resizing(&self) -> bool;
}

/// `_SwitchThreshold` and `_CurrentTimeStamp` (class statics) and the `head` list and counters (file statics).
#[derive(Clone, Debug)]
pub struct TextureFileStatics {
    /// `_CurrentTimeStamp`.
    pub current_time_stamp: u32,
    /// `_SwitchThreshold`: default 0.75.
    pub switch_threshold: f32,
    /// `texture_count`, `locked_surface_count`, `texture_id`, `mipmaps`.
    pub texture_count: i32,
    pub locked_surface_count: i32,
    pub texture_id: i32,
    pub mipmaps: bool,
}

impl Default for TextureFileStatics {
    fn default() -> Self {
        TextureFileStatics { current_time_stamp: 0, switch_threshold: 0.75, texture_count: 0, locked_surface_count: 0, texture_id: 0, mipmaps: true }
    }
}

impl TextureFileStatics {
    /// `_Reset_Time_Stamp()`: `_CurrentTimeStamp++`.
    pub fn reset_time_stamp(&mut self) {
        self.current_time_stamp = self.current_time_stamp.wrapping_add(1);
    }

    /// `_Set_Switch_Threshold(t)`: clamped to `0.5 ..= 0.999` ("no hysteresis" to "maximum hysteresis").
    pub fn set_switch_threshold(&mut self, switch_threshold: f32) {
        self.switch_threshold = f32::min(switch_threshold, 0.999f64 as f32);
        self.switch_threshold = f32::max(self.switch_threshold, 0.5f64 as f32);
    }

    /// `_Get_Switch_Threshold()`.
    pub fn get_switch_threshold(&self) -> f32 {
        self.switch_threshold
    }
}

/// `class TextureFileClass`.
#[derive(Clone, Debug)]
pub struct TextureFileClass {
    /// `LockedSurface` / `LockedSurfaceReductionFactor`.
    pub locked_surface: Option<SrSurface>,
    pub locked_surface_reduction_factor: u32,
    /// For performance statistics.
    pub locked_surface_size: i32,
    pub texture_size: i32,
    pub non_reduced_texture_size: i32,
    /// `ReductionEnabled`.
    pub reduction_enabled: bool,
    /// "The current reduction factor reflects the texture size currently used in rendering."
    pub current_reduction_factor: u32,
    /// "fractional non-negative value ... needs to be fractional for proper implementation of hysteresis".
    pub desired_reduction_factor: f32,
    pub time_stamp_of_last_request_reduction_call: u32,
    pub time_stamp_of_last_process_reduction_call: u32,
    /// `FileName`.
    pub file_name: Option<String>,
    /// `TempSurfacePtr`: passes a surface from `Apply_New_Surface()` to `setupDefaultValues()` / `getMipmapData()`.
    pub temp_surface: Option<SrSurface>,
    /// `file_error`, `id`.
    pub file_error: bool,
    pub id: i32,
    /// `texture_loader_info`: `new_surface`, `loading`, `reduction_factor`.
    pub loader_new_surface: Option<SrSurface>,
    pub loader_loading: bool,
    pub loader_reduction_factor: u32,
    /// The `srTexture` default dimensions `setupDefaultValues` edits.
    pub default_width: i32,
    pub default_height: i32,
    /// `DIRTY_DEFAULTS` / `GENERATESURFACE_FAILURE` flags.
    pub dirty_defaults: bool,
    pub generate_surface_failure: bool,
}

impl TextureFileClass {
    /// `TextureFileClass::TextureFileClass(filename)`: an empty name is NULL; a file the loader cannot find sets
    /// `file_error`; the locked surface is loaded; reduction is turned off unless the thumbnail mode is resizing.
    pub fn new<H: TextureFileHost>(host: &mut H, st: &mut TextureFileStatics, filename: Option<&str>) -> Self {
        let mut t = TextureFileClass {
            locked_surface: None,
            locked_surface_reduction_factor: 0,
            locked_surface_size: 0,
            texture_size: 0,
            non_reduced_texture_size: 0,
            reduction_enabled: true, // setupDefaultValues of srTexture: reduction defaults on (the ctor tests the mode below)
            current_reduction_factor: 0,
            desired_reduction_factor: 0.0,
            time_stamp_of_last_request_reduction_call: st.current_time_stamp.wrapping_sub(1),
            time_stamp_of_last_process_reduction_call: st.current_time_stamp.wrapping_sub(1),
            file_name: None,
            temp_surface: None,
            file_error: false,
            id: st.texture_id,
            loader_new_surface: None,
            loader_loading: false,
            loader_reduction_factor: 0,
            default_width: 0,
            default_height: 0,
            dirty_defaults: false,
            generate_surface_failure: false,
        };
        st.texture_id += 1;
        if let Some(f) = filename {
            if !f.is_empty() {
                t.file_name = Some(f.to_string());
            }
        }
        if let Some(name) = t.file_name.clone() {
            // setName(FileName);
            if !host.texture_file_exists(&name) {
                t.file_error = true;
            }
        }
        t.load_locked_surface(host, st);
        st.texture_count += 1;
        // Succ = head; head = this (the registry is the owner's list)
        t.generate_surface_failure = false;
        t.dirty_defaults = true;
        if !host.thumbnail_mode_is_resizing() {
            t.reduction_enabled = false;
        }
        t
    }

    /// `~TextureFileClass()`: the counters (the list unlinking is the owner's).
    pub fn destroy(self, st: &mut TextureFileStatics) {
        st.texture_count -= 1;
        if self.locked_surface.is_some() {
            st.locked_surface_count -= 1;
        }
    }

    /// `invalidate()`: drops the temp surface, invalidates the frame handle (the owner's), clears the failure flag.
    pub fn invalidate(&mut self) {
        self.release_temp_surface();
        self.generate_surface_failure = false;
    }

    /// `Release_Temp_Surface()`.
    fn release_temp_surface(&mut self) {
        self.temp_surface = None;
    }

    /// `Load_Temp_Surface()`: `Validate_Texture` + `Get_Surface(name, 0)` through the file cache, else `Load_Surface`.
    fn load_temp_surface<H: TextureFileHost>(&mut self, host: &mut H) {
        if self.temp_surface.is_some() {
            self.invalidate();
        }
        let name = match &self.file_name {
            Some(n) => n.clone(),
            None => return,
        };
        self.temp_surface = host.load_surface(&name);
    }

    /// `Load_Locked_Surface()`.
    fn load_locked_surface<H: TextureFileHost>(&mut self, host: &mut H, st: &mut TextureFileStatics) {
        self.locked_surface = None;
        self.locked_surface_reduction_factor = 0;
        if let Some(name) = self.file_name.clone() {
            if let Some((s, rf)) = host.load_locked_surface_immediate(&name) {
                self.locked_surface = Some(s);
                self.locked_surface_reduction_factor = rf;
            }
            if let Some(ls) = &self.locked_surface {
                self.current_reduction_factor = self.locked_surface_reduction_factor;
                self.locked_surface_size = calculate_size(ls, 0);
                st.locked_surface_count += 1;
            }
        }
    }

    /// `Apply_New_Surface()`: the loader's finished surface becomes the temp surface; the defaults are dirty again.
    pub fn apply_new_surface(&mut self) {
        self.invalidate();
        self.temp_surface = self.loader_new_surface.take();
        self.dirty_defaults = true;
        self.current_reduction_factor = self.loader_reduction_factor;
    }

    /// `Request_Reduction(reduction_factor)`: "Only the smallest reduction factor requested in a given frame is preserved."
    pub fn request_reduction(&mut self, st: &TextureFileStatics, mut reduction_factor: f32) {
        if !self.reduction_enabled {
            return;
        }
        if reduction_factor < 0.0 {
            reduction_factor = 0.0;
        }
        if self.time_stamp_of_last_request_reduction_call != st.current_time_stamp {
            self.desired_reduction_factor = reduction_factor;
            self.time_stamp_of_last_request_reduction_call = st.current_time_stamp;
        } else {
            self.desired_reduction_factor = f32::min(reduction_factor, self.desired_reduction_factor);
        }
    }

    /// `Process_Reduction()`: once per frame, if the current and desired factors differ by more than the switch threshold
    /// and there is a locked surface, switch (from the locked surface when the desired factor is at least its factor) or
    /// ask the loader for a new one in the background.
    pub fn process_reduction<H: TextureFileHost>(&mut self, host: &mut H, st: &TextureFileStatics) {
        if self.time_stamp_of_last_process_reduction_call != st.current_time_stamp {
            if self.locked_surface.is_some() && (self.current_reduction_factor as f32 - self.desired_reduction_factor).abs() > st.switch_threshold {
                let des_red_factor = petir::Real::floor(self.desired_reduction_factor + 0.5);
                debug_assert!(des_red_factor >= 0.0);
                let int_desired_reduction_factor = des_red_factor as u32;
                if int_desired_reduction_factor >= self.locked_surface_reduction_factor {
                    self.current_reduction_factor = int_desired_reduction_factor;
                    self.invalidate();
                } else {
                    self.loader_reduction_factor = int_desired_reduction_factor;
                    host.add_load_task(self.id, self.loader_reduction_factor);
                }
            }
            self.time_stamp_of_last_process_reduction_call = st.current_time_stamp;
        }
    }

    /// `Enable_Reduction(b)`: turning it on reloads the locked surface; turning it off drops it and loads the full-size
    /// surface.
    pub fn enable_reduction<H: TextureFileHost>(&mut self, host: &mut H, st: &mut TextureFileStatics, b: bool) {
        if self.reduction_enabled == b {
            return;
        }
        self.reduction_enabled = b;
        if b {
            self.load_locked_surface(host, st);
        } else {
            self.locked_surface = None;
            self.locked_surface_reduction_factor = 0;
            self.current_reduction_factor = 0;
            self.desired_reduction_factor = 0.0;
            self.load_temp_surface(host);
            if let Some(ts) = &self.temp_surface {
                // setupDefaultValuesFromSurface(TempSurfacePtr)
                self.default_width = ts.width;
                self.default_height = ts.height;
                self.texture_size = calculate_size(ts, 0);
                self.non_reduced_texture_size = self.texture_size;
            }
        }
    }

    /// `setupDefaultValues()`: when the defaults are dirty, takes them from the temp surface, else from the locked surface
    /// (shifted down by the difference of the factors, never below 1), else loads the surface blocking.
    pub fn setup_default_values<H: TextureFileHost>(&mut self, host: &mut H) {
        if !self.dirty_defaults {
            return;
        }
        self.dirty_defaults = false;
        if let Some(ts) = &self.temp_surface {
            self.default_width = ts.width; // setupDefaultValuesFromSurface
            self.default_height = ts.height;
            self.texture_size = calculate_size(ts, 0);
            self.non_reduced_texture_size = calculate_size(ts, self.current_reduction_factor);
        } else if let Some(ls) = self.locked_surface.clone() {
            self.default_width = ls.width;
            self.default_height = ls.height;
            if self.current_reduction_factor >= self.locked_surface_reduction_factor {
                let diff = self.current_reduction_factor - self.locked_surface_reduction_factor;
                self.default_width >>= diff;
                if self.default_width == 0 {
                    self.default_width = 1;
                }
                self.default_height >>= diff;
                if self.default_height == 0 {
                    self.default_height = 1;
                }
            } else {
                self.current_reduction_factor = self.locked_surface_reduction_factor;
            }
            self.texture_size = 0;
            self.non_reduced_texture_size = 0;
        } else {
            self.load_temp_surface(host);
            if let Some(ts) = &self.temp_surface {
                self.default_width = ts.width;
                self.default_height = ts.height;
                self.texture_size = calculate_size(ts, 0);
                self.non_reduced_texture_size = self.texture_size;
            }
        }
    }

    /// `Fill_Multi_Request_From_Surface(m, surface)`: level `large_lod` is a copy of the surface and each smaller level a
    /// copy (resize) of the one before. `copy(dst, src)` is `srColorSurface::copy`.
    pub fn fill_multi_request_from_surface<F: FnMut(usize, Option<usize>)>(large_lod: usize, small_lod: usize, mut copy: F) {
        copy(large_lod, None); // m.levels[m.largeLOD]->copy(*surface)
        let mut i = large_lod + 1;
        while i <= small_lod {
            copy(i, Some(i - 1)); // m.levels[i]->copy(*m.levels[i-1])
            i += 1;
        }
    }

    /// `Get_File_Name()`.
    pub fn get_file_name(&self) -> Option<&str> {
        self.file_name.as_deref()
    }

    /// `Get_Texture_Size()` / `Get_Non_Reduced_Texture_Size()` / `Get_Locked_Surface_Size()` /
    /// `Get_Current_Reduction_Factor()` / `Get_File_Error()` / `ID()`.
    pub fn get_texture_size(&self) -> i32 {
        self.texture_size
    }
    pub fn get_non_reduced_texture_size(&self) -> i32 {
        self.non_reduced_texture_size
    }
    pub fn get_locked_surface_size(&self) -> i32 {
        self.locked_surface_size
    }
    pub fn get_current_reduction_factor(&self) -> u32 {
        self.current_reduction_factor
    }
    pub fn get_file_error(&self) -> bool {
        self.file_error
    }
    pub fn id(&self) -> i32 {
        self.id
    }
}

/// The `head` list with its static functions (`Get_Total_*`, `Get_Texture(id)`, `List_Missing_Files`,
/// `Switch_Mipmaping_Debug`), keyed by texture id (list order is newest first upstream: ids descend).
#[derive(Clone, Debug, Default)]
pub struct TextureFileRegistry {
    pub textures: BTreeMap<i32, TextureFileClass>,
    pub statics: TextureFileStatics,
}

impl TextureFileRegistry {
    /// `Get_Total_Locked_Surface_Size()`.
    pub fn get_total_locked_surface_size(&self) -> i32 {
        self.textures.values().map(|t| t.get_locked_surface_size()).sum()
    }

    /// `Get_Total_Texture_Size()`.
    pub fn get_total_texture_size(&self) -> i32 {
        self.textures.values().map(|t| t.get_texture_size()).sum()
    }

    /// `Get_Total_Non_Reduced_Texture_Size()`.
    pub fn get_total_non_reduced_texture_size(&self) -> i32 {
        self.textures.values().map(|t| t.get_non_reduced_texture_size()).sum()
    }

    /// `Get_Total_Texture_Count()`.
    pub fn get_total_texture_count(&self) -> i32 {
        self.statics.texture_count
    }

    /// `Get_Total_Locked_Surface_Count()`.
    pub fn get_total_locked_surface_count(&self) -> i32 {
        self.statics.locked_surface_count
    }

    /// `Get_Texture(id)`.
    pub fn get_texture(&self, id: i32) -> Option<&TextureFileClass> {
        self.textures.get(&id)
    }

    /// `Switch_Mipmaping_Debug()`: toggles the global flag (the per-texture `params.setMipmap` is the Surrender object's).
    pub fn switch_mipmaping_debug(&mut self) {
        self.statics.mipmaps = !self.statics.mipmaps;
    }

    /// `List_Missing_Files()` (see the file header: the count is never incremented, so the result is always the
    /// "no missing textures" text).
    pub fn list_missing_files(&self) -> String {
        let mut s = String::from("Missing textures\n\nid      name\n------------------------\n");
        let missing_count = 0;
        for n in self.textures.values().rev() {
            if n.get_file_error() {
                s += &format!("{:04}  {}\n", n.id(), n.get_file_name().unwrap_or(""));
            }
        }
        if missing_count == 0 {
            s = "No missing textures\n\n".to_string();
        } else {
            s += &format!("\nTotal missing textures: {}", missing_count);
        }
        s
    }
}
