//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texturefile.cpp
//!
//! `TextureFileClass::getTextureFrameHandle`, `Set_Texture_Flash`, `Get_Texture_Flash`, `Update_Texture_Flash`.
//! **Compiled out upstream**: the whole of texturefile.cpp is inside `#ifdef WW3D_DX8`, defined nowhere (see the first
//! pass, `drafts::ww3d::texturefile`, which holds the rest of the class). The flash functions are marked "Warning! Slow,
//! for debug only!!!" and are drafted over the same host-contract style for completeness. `Update_Texture_Flash` is called
//! from `WW3D::Sync` (ww3d.cpp:830), and so runs every frame in a build with the texture class on; with it off, never.

/// The Surrender surface handle (`srColorSurfaceIFace*`) is a host handle; `None` is NULL.
pub type SurfaceHandle = Option<usize>;

/// The members the flash functions touch.
#[derive(Clone, Debug, Default)]
pub struct TextureFlashState {
    /// `flags.test(GENERATESURFACE_FAILURE)`.
    pub generate_surface_failure: bool,
    /// `TextureFrameHandle` (`srTextureIFace::FrameHandle`; 0 is none).
    pub texture_frame_handle: usize,
    /// `flash`.
    pub flash: bool,
    /// `flash_store_surface`.
    pub flash_store_surface: SurfaceHandle,
    /// `TempSurfacePtr`.
    pub temp_surface_ptr: SurfaceHandle,
}

/// The Surrender / loader calls.
pub trait TextureFlashHost {
    /// `Load_Temp_Surface()`: sets `TempSurfacePtr` (returned).
    fn load_temp_surface(&mut self) -> SurfaceHandle;
    /// `surface->addReference()`.
    fn surface_add_reference(&mut self, surface: usize);
    /// `invalidateFrameHandle(TextureFrameHandle)`.
    fn invalidate_frame_handle(&mut self, handle: usize);
    /// `Release_Temp_Surface()`.
    fn release_temp_surface(&mut self);
}

impl TextureFlashState {
    /// `TextureFileClass::getTextureFrameHandle()` (texturefile.cpp:251): 0 when surface generation failed, else the
    /// frame handle.
    pub fn get_texture_frame_handle(&self) -> usize {
        if self.generate_surface_failure {
            return 0;
        }
        self.texture_frame_handle
    }

    /// `TextureFileClass::Set_Texture_Flash(b)` (texturefile.cpp:504): "Make texture flash (Warning! Slow, for debug
    /// only!!!)". On: load the temp surface and keep a reference to it as the flash store (upstream dereferences
    /// `TempSurfacePtr` without a NULL test). Off: invalidate the frame handle, release the temp surface and put the
    /// stored original back.
    pub fn set_texture_flash<H: TextureFlashHost>(&mut self, host: &mut H, b: bool) {
        self.flash = b;
        if self.flash {
            self.temp_surface_ptr = host.load_temp_surface();
            self.flash_store_surface = self.temp_surface_ptr;
            if let Some(s) = self.flash_store_surface {
                host.surface_add_reference(s);
            }
        } else {
            // Restore the original surface... the surface will be released after applying to use
            host.invalidate_frame_handle(self.texture_frame_handle);
            host.release_temp_surface();
            self.temp_surface_ptr = self.flash_store_surface;
            self.flash_store_surface = None;
        }
    }

    /// `TextureFileClass::Get_Texture_Flash()` (texturefile.cpp:522): the flash state.
    pub fn get_texture_flash(&self) -> bool {
        self.flash
    }
}

/// `flash_time` (texturefile.cpp): the flash half-period in ms.
pub const FLASH_TIME: u32 = 500;

/// The file statics `latest_time`, `flash_state`, `flash_counter`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlashClock {
    pub latest_time: u32,
    pub flash_state: bool,
    pub flash_counter: u32,
}

/// The call out of `Update_Texture_Flash` over the registry (`head` list of all textures).
pub trait FlashRegistryHost {
    /// `srCore.getSurface()` copied into a new surface `s` (`W3DNEW srColorSurface(pf, w, w); s->copy(...)`): returns the
    /// new surface handle.
    fn make_flash_surface(&mut self) -> usize;
    /// For every texture with `flash` set: `invalidateFrameHandle`, `Release_Temp_Surface`, and `TempSurfacePtr` = `s`
    /// (`addReference`) when `s` is `Some`, else its own `flash_store_surface` (`addReference`).
    fn apply_to_flashing_textures(&mut self, s: Option<usize>);
    /// `s->release()`.
    fn release_surface(&mut self, s: usize);
}

impl FlashClock {
    /// `TextureFileClass::Update_Texture_Flash()` (texturefile.cpp:542): `cur_time` is `WW3D::Get_Sync_Time()`. The counter
    /// advances by the elapsed time; when it passes 500 ms it wraps (to 0 if still past), flips `flash_state`, and (on the
    /// "off" half) builds a copy of the core surface; every flashing texture then gets that copy or its stored original.
    /// Below the threshold it returns at once (`else return;`).
    ///
    /// Finding: upstream ends with `s->release()` unconditionally, though `s` is NULL in the "on" half (`flash_state ==
    /// true`): a null dereference on every second flip. Kept as is - here the release is skipped for `None`.
    pub fn update_texture_flash<H: FlashRegistryHost>(&mut self, host: &mut H, cur_time: u32) {
        let delta = cur_time.wrapping_sub(self.latest_time) as i32;
        self.latest_time = cur_time;
        self.flash_counter = self.flash_counter.wrapping_add(delta as u32);
        if self.flash_counter > FLASH_TIME {
            self.flash_counter -= FLASH_TIME;
            if self.flash_counter > FLASH_TIME {
                self.flash_counter = 0;
            }
            self.flash_state = !self.flash_state;
        } else {
            return;
        }

        let mut s: Option<usize> = None;
        if !self.flash_state {
            s = Some(host.make_flash_surface());
        }

        host.apply_to_flashing_textures(s);
        if let Some(s) = s {
            host.release_surface(s);
        }
    }
}
