//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texture.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texture.h
//!
//! The `TextureBaseClass` bookkeeping, the asset-manager-wide texture statistics and the `TextureClass` /
//! `ZTextureClass` memory queries that no earlier pass translated. The D3D texture is a host handle (`usize`); the
//! walk over `WW3DAssetManager::Texture_Hash()` is a slice of [`TextureBase`] the caller passes in.
//! `Texture_Hash()` is a `HashTemplateIterator`, whose order is unspecified, and every statistic here is a sum or a
//! count, so the slice order does not matter.

/// The fields of `TextureBaseClass` these functions read and write.
#[derive(Clone, Debug, Default)]
pub struct TextureBase {
    pub mip_level_count: u32,
    pub width: u32,
    pub height: u32,
    /// `D3DTexture` (host handle); `None` is NULL.
    pub d3d_texture: Option<usize>,
    pub initialized: bool,
    pub name: String,
    pub is_lightmap: bool,
    pub is_procedural: bool,
    pub inactivation_time: u32,
    pub extended_inactivation_time: u32,
    pub last_inactivation_sync_time: u32,
    /// `LastAccessed` (mutable in upstream's const accessors).
    pub last_accessed: u32,
    pub texture_load_task: bool,
    pub thumbnail_load_task: bool,
    pub hsv_shift: [f32; 3],
    /// `Get_Texture_Memory_Usage()`, which each subclass computes from its D3D levels.
    pub memory_usage: u32,
}

/// `MIP_LEVELS_1`.
const MIP_LEVELS_1: u32 = 1;

/// What the texture code calls outside texture.cpp.
pub trait TextureHost {
    /// `WW3D::Get_Thumbnail_Enabled()`.
    fn ww3d_thumbnail_enabled(&self) -> bool;
    /// `WW3D::Get_Sync_Time()`.
    fn ww3d_sync_time(&self) -> u32;
    /// `WW3D::Get_Texture_Reduction()`.
    fn ww3d_texture_reduction(&self) -> i32;
    /// `WW3D::Is_Large_Texture_Extra_Reduction_Enabled()`.
    fn ww3d_large_texture_extra_reduction_enabled(&self) -> bool;
    /// `D3DTexture->Release()` / `AddRef()`.
    fn d3d_release(&mut self, d3d_texture: usize);
    fn d3d_add_ref(&mut self, d3d_texture: usize);
    /// `MissingTexture::_Get_Missing_Texture()` (the call AddRef's it; `Is_Missing_Texture` releases it again).
    fn missing_texture(&self) -> Option<usize>;
    /// `DX8Wrapper::Set_DX8_Texture(stage, NULL)`.
    fn dx8_set_texture_null(&mut self, stage: u32);
    /// `GetLevelCount()` / `GetLevelDesc(i).Size` of the D3D texture; `GetSurfaceLevel`.
    fn d3d_level_sizes(&self, d3d_texture: usize) -> Vec<u32>;
    fn d3d_get_surface_level(&mut self, d3d_texture: usize, level: u32) -> usize;
}

/// Static `TexturesAppliedPerFrame`.
#[derive(Clone, Debug, Default)]
pub struct TextureStatics {
    pub textures_applied_per_frame: u32,
}

impl TextureBase {
    /// `TextureBaseClass::Invalidate` (texture.cpp:~190): not while a load task is pending, never a procedural
    /// texture; otherwise release the D3D texture and clear `Initialized`.
    pub fn invalidate<H: TextureHost>(&mut self, host: &mut H) {
        if self.texture_load_task {
            return;
        }
        if self.thumbnail_load_task {
            return;
        }
        if self.is_procedural {
            return;
        }
        if let Some(t) = self.d3d_texture.take() {
            host.d3d_release(t);
        }
        self.initialized = false;
    }

    /// `TextureBaseClass::Invalidate_Old_Unused_Textures` (texture.cpp:142): each frame, invalidate initialised textures
    /// with an inactivation time that were not used for it (or for `invalidation_time_override` when nonzero).
    pub fn invalidate_old_unused_textures<H: TextureHost>(host: &mut H, statics: &mut TextureStatics, textures: &mut [TextureBase], invalidation_time_override: u32) {
        // (gth) If thumbnails are not enabled, then we don't run this code.
        if !host.ww3d_thumbnail_enabled() {
            return;
        }
        statics.textures_applied_per_frame = 0;
        let synctime = host.ww3d_sync_time();
        for tex in textures.iter_mut() {
            if tex.initialized && tex.inactivation_time != 0 {
                let age = synctime.wrapping_sub(tex.last_accessed);
                if invalidation_time_override != 0 {
                    if age > invalidation_time_override {
                        tex.invalidate(host);
                        tex.last_inactivation_sync_time = synctime;
                    }
                } else if age > tex.inactivation_time.wrapping_add(tex.extended_inactivation_time) {
                    tex.invalidate(host);
                    tex.last_inactivation_sync_time = synctime;
                }
            }
        }
    }

    /// `TextureBaseClass::Peek_D3D_Base_Texture` (texture.cpp:258): stamps `LastAccessed`.
    pub fn peek_d3d_base_texture<H: TextureHost>(&mut self, host: &H) -> Option<usize> {
        self.last_accessed = host.ww3d_sync_time();
        self.d3d_texture
    }

    /// `TextureBaseClass::Set_D3D_Base_Texture` (texture.cpp:268): resets the access timer, releases the old texture,
    /// takes a reference on the new one.
    pub fn set_d3d_base_texture<H: TextureHost>(&mut self, host: &mut H, tex: Option<usize>) {
        self.last_accessed = host.ww3d_sync_time();
        if let Some(old) = self.d3d_texture {
            host.d3d_release(old);
        }
        self.d3d_texture = tex;
        if let Some(new) = self.d3d_texture {
            host.d3d_add_ref(new);
        }
    }

    /// `TextureBaseClass::Is_Missing_Texture` (texture.cpp:302): the D3D texture is the shared missing texture. Upstream
    /// `_Get_Missing_Texture` adds a reference and this function releases it, a net zero.
    pub fn is_missing_texture<H: TextureHost>(&self, host: &H) -> bool {
        let missing = host.missing_texture();
        self.d3d_texture == missing
    }

    /// `TextureBaseClass::Set_Texture_Name` (texture.cpp:323).
    pub fn set_texture_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// `TextureBaseClass::Get_Reduction` (texture.cpp:375): none for a one-level or small (<= 32) texture; the global
    /// reduction, plus one for textures above 256 when the "large texture extra reduction" option is on, capped at the
    /// mip level count.
    pub fn get_reduction<H: TextureHost>(&self, host: &H) -> u32 {
        if self.mip_level_count == MIP_LEVELS_1 {
            return 0;
        }
        if self.width <= 32 || self.height <= 32 {
            return 0;
        }
        let mut reduction = host.ww3d_texture_reduction();
        if host.ww3d_large_texture_extra_reduction_enabled() && (self.width > 256 || self.height > 256) {
            reduction += 1;
        }
        if self.mip_level_count != 0 && reduction > self.mip_level_count as i32 {
            reduction = self.mip_level_count as i32;
        }
        reduction as u32
    }

    /// `TextureBaseClass::Apply_Null` (texture.cpp:400): render states for a "NULL" texture.
    pub fn apply_null<H: TextureHost>(host: &mut H, stage: u32) {
        host.dx8_set_texture_null(stage);
    }

    /// `TextureBaseClass::Set_HSV_Shift` (texture.cpp:411): relative to the original texture, so invalidate and reload.
    pub fn set_hsv_shift<H: TextureHost>(&mut self, host: &mut H, hsv_shift: [f32; 3]) {
        self.invalidate(host);
        self.hsv_shift = hsv_shift;
    }

    /// `TextureClass::Get_D3D_Surface_Level` (texture.cpp:1019) and `ZTextureClass::Get_D3D_Surface_Level` (:1311):
    /// `None` (asserting) without a D3D texture.
    pub fn get_d3d_surface_level<H: TextureHost>(&self, host: &mut H, level: u32) -> Option<usize> {
        match self.d3d_texture {
            None => {
                debug_assert!(false, "Get_D3D_Surface_Level: D3DTexture is NULL!");
                None
            }
            Some(t) => Some(host.d3d_get_surface_level(t, level)),
        }
    }

    /// `TextureClass::Get_Texture_Memory_Usage` (texture.cpp:1036) and `ZTextureClass::Get_Texture_Memory_Usage`
    /// (:1328): the sum of every level's `D3DSURFACE_DESC::Size`; 0 without a D3D texture.
    pub fn get_texture_memory_usage<H: TextureHost>(&self, host: &H) -> u32 {
        match self.d3d_texture {
            None => 0,
            Some(t) => host.d3d_level_sizes(t).iter().sum(),
        }
    }
}

/// `TextureBaseClass::_Get_Total_Locked_Surface_Size` (texture.cpp:421): the memory of every uninitialised texture.
pub fn get_total_locked_surface_size(textures: &[TextureBase]) -> i32 {
    textures.iter().filter(|t| !t.initialized).map(|t| t.memory_usage as i32).sum()
}

/// `TextureBaseClass::_Get_Total_Texture_Size` (texture.cpp:443).
pub fn get_total_texture_size(textures: &[TextureBase]) -> i32 {
    textures.iter().map(|t| t.memory_usage as i32).sum()
}

/// `TextureBaseClass::_Get_Total_Lightmap_Texture_Size` (texture.cpp:465).
pub fn get_total_lightmap_texture_size(textures: &[TextureBase]) -> i32 {
    textures.iter().filter(|t| t.is_lightmap).map(|t| t.memory_usage as i32).sum()
}

/// `TextureBaseClass::_Get_Total_Procedural_Texture_Size` (texture.cpp:488).
pub fn get_total_procedural_texture_size(textures: &[TextureBase]) -> i32 {
    textures.iter().filter(|t| t.is_procedural).map(|t| t.memory_usage as i32).sum()
}

/// `TextureBaseClass::_Get_Total_Texture_Count` (texture.cpp:510).
pub fn get_total_texture_count(textures: &[TextureBase]) -> i32 {
    textures.len() as i32
}

/// `TextureBaseClass::_Get_Total_Lightmap_Texture_Count` (texture.cpp:531).
pub fn get_total_lightmap_texture_count(textures: &[TextureBase]) -> i32 {
    textures.iter().filter(|t| t.is_lightmap).count() as i32
}

/// `TextureBaseClass::_Get_Total_Procedural_Texture_Count` (texture.cpp:552).
pub fn get_total_procedural_texture_count(textures: &[TextureBase]) -> i32 {
    textures.iter().filter(|t| t.is_procedural).count() as i32
}

/// `TextureBaseClass::_Get_Total_Locked_Surface_Count` (texture.cpp:574).
pub fn get_total_locked_surface_count(textures: &[TextureBase]) -> i32 {
    textures.iter().filter(|t| !t.initialized).count() as i32
}
