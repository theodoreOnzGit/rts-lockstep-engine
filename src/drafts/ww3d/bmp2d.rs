//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/bmp2d.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/bmp2d.h
//!
//! `Bitmap2DObjClass` is a `DynamicScreenMeshClass` (the draft [`DynamicMeshClass`] with `new_screen`) that tiles
//! a bitmap into power-of-two pieces. Reused from the port: `part_buf::find_pot` (`Find_POT`, pot.cpp). Asset
//! manager / texture-loader / vertex-material calls are the owner's: the constructors take the loaded surface (or
//! texture size and format) and closures that turn a piece surface into a texture handle and that provide the
//! colourizable vertex material. `Bitmap2DObjClass::Clone` is `Clone`.

use super::dynamesh::DynamicMeshClass;
use super::handles::{TextureHandle, VertexMaterialHandle};
use super::shader_presets::{PRESET_ADDITIVE_2D_SHADER, PRESET_ALPHA_2D_SHADER, PRESET_OPAQUE_2D_SHADER};
use super::surfaceclass::{has_alpha, SurfaceClass};
use crate::formats::texture::format::Ww3dFormat;
use crate::formats::w3d::mesh::shader_bits::{MASK_PRIGRADIENT, SHIFT_PRIGRADIENT};
use crate::formats::w3d::part_buf::find_pot;
use crate::formats::w3d::shader::{ShaderClass, GRADIENT_MODULATE};

/// `class Bitmap2DObjClass : public DynamicScreenMeshClass`.
#[derive(Clone, Debug)]
pub struct Bitmap2DObjClass {
    pub mesh: DynamicMeshClass,
}

fn pick_shader(additive: bool, ignore_alpha: bool, format: Ww3dFormat) -> ShaderClass {
    if additive {
        PRESET_ADDITIVE_2D_SHADER
    } else if !ignore_alpha && has_alpha(format) {
        PRESET_ALPHA_2D_SHADER
    } else {
        PRESET_OPAQUE_2D_SHADER
    }
}

fn set_gradient_modulate(shader: &mut ShaderClass) {
    shader.0 = (shader.0 & !MASK_PRIGRADIENT) | (GRADIENT_MODULATE << SHIFT_PRIGRADIENT);
}

impl Bitmap2DObjClass {
    /// `Bitmap2DObjClass::Bitmap2DObjClass(filename, screen_x, screen_y, center, additive, colorizable,
    /// usable_width, usable_height, ignore_alpha)` (bmp2d.cpp:42), with the texture's level-0 `surface` supplied
    /// (`None` = the texture had none: upstream makes a 32x32 R8G8B8-class surface; `fallback_format` is
    /// `Get_Valid_Texture_Format(WW3D_FORMAT_R8G8B8, true)`). `(resw, resh)` is the device resolution.
    /// `piece_texture` makes the clamp-addressed texture from a piece surface; `colorize_material` supplies the
    /// vertex material (ambient/diffuse/specular 0, emissive 1) when `colorizable`.
    pub fn from_surface<T: FnMut(&SurfaceClass) -> TextureHandle>(
        surface: Option<&SurfaceClass>,
        fallback_format: Ww3dFormat,
        mut screen_x: f32,
        mut screen_y: f32,
        center: bool,
        additive: bool,
        colorizable: bool,
        mut usable_width: i32,
        mut usable_height: i32,
        ignore_alpha: bool,
        resw: i32,
        resh: i32,
        mut piece_texture: T,
        colorize_material: Option<VertexMaterialHandle>,
    ) -> Self {
        let mut mesh = DynamicMeshClass::new_screen(2, 4, 1.0);
        let fallback;
        let surface = match surface {
            Some(s) => s,
            None => {
                fallback = SurfaceClass::new(32, 32, fallback_format);
                &fallback
            }
        };
        let sd = surface.get_description();
        if usable_width == -1 {
            usable_width = sd.width as i32;
        }
        if usable_height == -1 {
            usable_height = sd.height as i32;
        }
        if center {
            screen_x -= (usable_width as f32 / resw as f32) / 2.0;
            screen_y -= (usable_height as f32 / resh as f32) / 2.0;
        }

        let surf_w = usable_width;
        let surf_h = usable_height;
        let mut piece = find_pot(surf_w.min(surf_h));
        piece = piece.min(256);
        let mw = if surf_w & (piece - 1) != 0 { surf_w / piece + 1 } else { surf_w / piece };
        let mh = if surf_h & (piece - 1) != 0 { surf_h / piece + 1 } else { surf_h / piece };

        mesh.resize((mw * mh * 2) as usize, (mw * mh * 4) as usize);

        let mut shader = pick_shader(additive, ignore_alpha, sd.format);
        mesh.enable_sort();
        if colorizable {
            set_gradient_modulate(&mut shader);
            if let Some(m) = colorize_material {
                mesh.set_vertex_material(m, true, 0);
            }
        }
        mesh.set_shader(shader, 0);

        let mut tlpy = 0;
        for _lpy in 0..mh {
            let mut tlpx = 0;
            for _lpx in 0..mw {
                let iw = piece.min(usable_width - tlpx);
                let ih = piece.min(usable_height - tlpy);
                let pot = find_pot(iw).max(find_pot(ih));

                let mut piece_surface = SurfaceClass::new(pot as u32, pot as u32, sd.format);
                piece_surface.copy_from_surface(0, 0, tlpx as u32, tlpy as u32, pot as u32, pot as u32, surface);
                // TextureClass(piece_surface, MIP_LEVELS_1) with U/V address mode CLAMP: the owner's closure.
                let tex = piece_texture(&piece_surface);

                let tw = iw as f32 / pot as f32;
                let th = ih as f32 / pot as f32;
                let vw = iw as f32 / resw as f32;
                let vh = ih as f32 / resh as f32;
                let x = screen_x + tlpx as f32 / resw as f32;
                let y = screen_y + tlpy as f32 / resh as f32;

                mesh.set_texture(tex, false, 0);
                mesh.begin_tri_strip();
                mesh.vertex(x, y, 0.0, 0.0, 0.0);
                mesh.vertex(x + vw, y, 0.0, tw, 0.0);
                mesh.vertex(x, y + vh, 0.0, 0.0, th);
                mesh.vertex(x + vw, y + vh, 0.0, tw, th);
                mesh.end_tri_strip();

                tlpx += piece;
            }
            tlpy += piece;
        }
        mesh.set_dirty();
        Bitmap2DObjClass { mesh }
    }

    /// `Bitmap2DObjClass::Bitmap2DObjClass(texture, screen_x, screen_y, center, additive, colorizable,
    /// ignore_alpha)` (bmp2d.cpp:156): one quad sized by the texture. `texture_width/height/format` are
    /// `TextureClass::Get_Width/Get_Height/Get_Texture_Format`.
    pub fn from_texture(
        texture: TextureHandle,
        texture_width: u32,
        texture_height: u32,
        texture_format: Ww3dFormat,
        mut screen_x: f32,
        mut screen_y: f32,
        center: bool,
        additive: bool,
        colorizable: bool,
        ignore_alpha: bool,
        resw: i32,
        resh: i32,
        colorize_material: Option<VertexMaterialHandle>,
    ) -> Self {
        let mut mesh = DynamicMeshClass::new_screen(2, 4, 1.0);
        let vw = texture_width as f32 / resw as f32;
        let vh = texture_height as f32 / resh as f32;
        if center {
            screen_x -= vw / 2.0;
            screen_y -= vh / 2.0;
        }
        let mut shader = pick_shader(additive, ignore_alpha, texture_format);
        mesh.disable_sort();
        if colorizable {
            set_gradient_modulate(&mut shader);
            if let Some(m) = colorize_material {
                mesh.set_vertex_material(m, true, 0);
            }
        }
        mesh.set_shader(shader, 0);
        mesh.set_texture(texture, false, 0);
        mesh.begin_tri_strip();
        mesh.vertex(screen_x, screen_y, 0.0, 0.0, 0.0);
        mesh.vertex(screen_x + vw, screen_y, 0.0, 1.0, 0.0);
        mesh.vertex(screen_x, screen_y + vh, 0.0, 0.0, 1.0);
        mesh.vertex(screen_x + vw, screen_y + vh, 0.0, 1.0, 1.0);
        mesh.end_tri_strip();
        mesh.set_dirty();
        Bitmap2DObjClass { mesh }
    }
}
