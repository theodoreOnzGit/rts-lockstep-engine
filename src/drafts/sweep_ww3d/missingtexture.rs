//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/missingtexture.cpp
//!
//! `MissingTexture::_Get_Missing_Texture`, `::_Create_Missing_Surface` and `::_Deinit`: the placeholder texture
//! (`_MissingTexture`, an `IDirect3DTexture8`) the texture loader substitutes for a file it cannot find. All three are
//! Direct3D 8 calls on that one object, so the draft keeps their structure over the [`MissingTextureHost`] contract
//! (device-only, not translated); the texture is an opaque handle. (`_Init` is the first pass's or the verified port's: it
//! fills the 128x128 texture with the constant `0x7FFF00FF`, the palette lookup being commented out.)

/// The device calls.
pub trait MissingTextureHost {
    /// `_MissingTexture->AddRef()`.
    fn add_ref(&mut self, texture: usize);
    /// `_MissingTexture->GetSurfaceLevel(0, ...)`, `GetDesc`, `CreateImageSurface(w, h, format)`, `CopyRects(texture_surface,
    /// NULL, 0, surface, NULL)` and `texture_surface->Release()`: returns the new image surface handle.
    fn create_image_surface_from_level0(&mut self, texture: usize) -> usize;
    /// `_MissingTexture->Release()`.
    fn release(&mut self, texture: usize);
}

/// `static IDirect3DTexture8 * _MissingTexture` (`None` is NULL).
#[derive(Clone, Copy, Debug, Default)]
pub struct MissingTexture {
    pub texture: Option<usize>,
}

impl MissingTexture {
    /// `MissingTexture::_Get_Missing_Texture()` (missingtexture.cpp:34): `WWASSERT(_MissingTexture)`, then `AddRef()` and
    /// return it.
    pub fn get_missing_texture<H: MissingTextureHost>(&self, host: &mut H) -> usize {
        let t = self.texture.expect("WWASSERT(_MissingTexture)");
        host.add_ref(t);
        t
    }

    /// `MissingTexture::_Create_Missing_Surface()` (missingtexture.cpp:41): a system-memory copy of the texture's top level.
    pub fn create_missing_surface<H: MissingTextureHost>(&self, host: &mut H) -> usize {
        let t = self.texture.expect("_MissingTexture");
        host.create_image_surface_from_level0(t)
    }

    /// `MissingTexture::_Deinit()` (missingtexture.cpp:159): `_MissingTexture->Release(); _MissingTexture = 0;`.
    pub fn deinit<H: MissingTextureHost>(&mut self, host: &mut H) {
        if let Some(t) = self.texture {
            host.release(t);
        }
        self.texture = None;
    }
}
