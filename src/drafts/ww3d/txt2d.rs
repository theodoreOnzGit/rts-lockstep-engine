//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/txt2d.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/txt2d.h
//!
//! The whole implementation is under `#ifdef WW3D_DX8` upstream and uses the legacy `FontClass`/`srTextureIFace`
//! text texture ([`TextTextureClass`], txt.rs). Translated: the quad layout of `Set_Text` over a
//! [`DynamicMeshClass`] screen mesh. The `vsprintf` formatting is the caller's (it passes the final string), and
//! the legacy texture is a handle.

use super::dynamesh::DynamicMeshClass;
use super::handles::TextureHandle;
use super::shader_presets::PRESET_OPAQUE_2D_SHADER;
use super::txt::TextTextureClass;
use crate::ww::vector3::Vector3;

/// `class Text2DObjClass : public DynamicScreenMeshClass` (txt2d.h).
#[derive(Clone, Debug)]
pub struct Text2DObjClass {
    pub mesh: DynamicMeshClass,
    pub text_texture: TextTextureClass,
    /// `_LastWidth` / `_LastHeight` (class statics upstream, kept per object here).
    pub last_width: f32,
    pub last_height: f32,
}

impl Text2DObjClass {
    pub fn new() -> Self {
        Text2DObjClass { mesh: DynamicMeshClass::new_screen(2, 4, 1.0), text_texture: TextTextureClass::new(), last_width: 0.0, last_height: 0.0 }
    }

    /// `Text2DObjClass::Set_Text` (txt2d.cpp:80). `fw`/`fh` are `String_Pixel_Width(text) + 1` and `Get_Height() + 1`;
    /// `(resw, resh)` the device resolution; `pixel_center` is `WW3D::Get_Pixel_Center`. `texture` is the text
    /// texture made from the strips `Build_Texture` returned. Returns early (no change) when the texture was still
    /// valid, as upstream.
    #[allow(clippy::too_many_arguments)]
    pub fn set_text(
        &mut self,
        font: usize,
        text: &str,
        mut screen_x: f32,
        mut screen_y: f32,
        fore: i32,
        back: i32,
        conv: usize,
        center: bool,
        clamp: bool,
        fw: i32,
        fh: i32,
        resw: i32,
        resh: i32,
        pixel_center: (f32, f32),
        texture: TextureHandle,
    ) {
        if self.text_texture.build_texture(font, text, fore, back, conv, fw, fh).is_none() {
            return;
        }
        let tsize = self.text_texture.get_texture_size();
        let mw = if fw & (tsize - 1) != 0 { fw / tsize + 1 } else { fw / tsize };

        let mut x_delta = 0.0f32;
        let mut y_delta = 0.0f32;
        if center {
            x_delta = -(fw as f32 / resw as f32) / 2.0;
            y_delta = -(fh as f32 / resh as f32) / 2.0;
        }
        self.last_width = fw as f32 / resw as f32;
        self.last_height = fh as f32 / resh as f32;

        if clamp {
            if screen_x < 0.0 {
                screen_x = 0.0;
            }
            if screen_x + self.last_width > 1.0 {
                screen_x = 1.0 - self.last_width;
                debug_assert!(screen_x >= 0.0);
            }
            if screen_y < 0.0 {
                screen_y = 0.0;
            }
            if screen_y + self.last_height > 1.0 {
                screen_y = 1.0 - self.last_height;
                debug_assert!(screen_y >= 0.0);
            }
        }

        self.mesh.resize((mw * 2) as usize, (mw * 4) as usize);
        self.mesh.set_texture(texture, false, 0);
        self.mesh.set_shader(PRESET_OPAQUE_2D_SHADER, 0);
        self.mesh.enable_sort();

        for lp in 0..mw {
            let pw = (fw - tsize * lp).min(tsize);
            let ph = fh;
            let tw = pw as f32 / tsize as f32;
            let th = ph as f32 / tsize as f32;
            let ty = (fh as f32 * lp as f32) / tsize as f32;
            let mut x = (lp as f32 * tsize as f32) / resw as f32;
            let mut y = 0.0f32;
            x += x_delta;
            y += y_delta;
            let vw = pw as f32 / resw as f32;
            let vh = ph as f32 / resh as f32;

            self.mesh.begin_tri_strip();
            self.mesh.vertex(x, y, 0.0, 0.0, ty);
            self.mesh.vertex(x + vw, y, 0.0, tw, ty);
            self.mesh.vertex(x, y + vh, 0.0, 0.0, ty + th);
            self.mesh.vertex(x + vw, y + vh, 0.0, tw, ty + th);
            self.mesh.end_tri_strip();
        }

        // Snap the position to a pixel centre.
        screen_x *= resw as f32;
        screen_x = screen_x.floor() + pixel_center.0;
        screen_x /= resw as f32;
        screen_y *= resh as f32;
        screen_y = screen_y.floor() + pixel_center.1;
        screen_y /= resh as f32;
        self.mesh.set_screen_position(&Vector3::new(screen_x, screen_y, 0.0));
        self.mesh.set_dirty();
    }
}
