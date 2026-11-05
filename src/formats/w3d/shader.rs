// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.h
//   (ShaderClass's enumerations and Get_* accessors)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.cpp
//   (srcBlendLUT, dstBlendLUT, ShaderClass::Apply, Get_SS_Category,
//   Guess_Sort_Level, _PolygonCullMode)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, by Greg Hjelstrom, Jani Penttinen and others)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ShaderClass`: what a W3D shader's bits mean to the device. Upstream's
//! `ShaderClass::Apply` turns the bits into Direct3D 8 render states and
//! texture-stage states; [`apply`] computes the same states as plain data
//! ([`ShaderStates`]), which the client maps to its own renderer.
//!
//! # As upstream reads it
//!
//! The bits are the fields of shader.h (`MASK_*`, `SHIFT_*`, see
//! [`super::mesh::shader_bits`]). `Apply` runs with `ShaderDirty` set the
//! first time and after `Invalidate`, so every state is written; that full
//! write is what [`apply`] returns (later calls on a device that already
//! holds a shader only write the states whose fields changed, which ends in
//! the same device state):
//!
//! - **Colour mask, blending and alpha test.** With colour writes off the
//!   blend is (`ZERO`, `ONE`); otherwise the source and destination factors
//!   come from `srcBlendLUT` and `dstBlendLUT`. Blending is enabled unless the
//!   pair is (`ONE`, `ZERO`), and only then are `D3DRS_SRCBLEND` and
//!   `D3DRS_DESTBLEND` written. With alpha test on, the reference is `0x60`
//!   ("produces best results with mip-mapped textures") and the function
//!   `GREATEREQUAL`.
//! - **Fog.** Only when the device allows fog and fog is on: `FOG_ENABLE`
//!   fogs to the fog colour, `FOG_SCALE_FRAGMENT` to black, `FOG_WHITE` to
//!   white.
//! - **Texture stage 0 (the primary gradient).** With texturing on:
//!   decal (`GRADIENT_DISABLE`), modulate (`GRADIENT_MODULATE` and any value
//!   out of the enumeration), add, the bump-map ops (when the caps have them,
//!   otherwise the vertex colour), modulate 2x. With texturing off: the vertex
//!   colour, or the stage disabled for `GRADIENT_DISABLE`.
//! - **Texture stage 1 (the post-detail functions).** Only with texturing
//!   on; each colour function falls back as the caps require; a stage whose
//!   colour or alpha op alone is set selects the current value for the other.
//! - **The rest.** Specular on for the secondary gradient; the depth function
//!   is the depth compare plus one (`D3DCMP_*`); depth writes; culling with
//!   `_PolygonCullMode` (`D3DCULL_CW`) or none; N-patch segments (1, or the
//!   N-patch level); alpha test enable written again.
//!
//! # Upstream quirks that are kept
//!
//! - `srcBlendLUT[SRCBLEND_ONE_MINUS_SRC_ALPHA]` is `D3DBLEND_DESTCOLOR`,
//!   not `D3DBLEND_INVSRCALPHA`: a W3D "one minus source alpha" source blend
//!   multiplies by the destination colour.
//! - So the alpha test's `sf == D3DBLEND_INVSRCALPHA` branch (reference
//!   `0xFF - 0x60`, `LESSEQUAL`) is never taken.
//! - `GRADIENT_MODULATE2X` tests the caps with `D3DTOP_MODULATE2X` (5), an op
//!   code, instead of `D3DTEXOPCAPS_MODULATE2X`: it asks for the `DISABLE` or
//!   `SELECTARG2` cap bits.
//! - `GRADIENT_ADD` modulates the alpha; without the `ADD` cap it modulates
//!   the colour too.
//! - `Get_Depth_Compare` reads `ShaderBits&MASK_DEPTHCOMPARE>>SHIFT_DEPTHCOMPARE`
//!   (the shift binds first); with a shift of 0 it is the same field.
//!
//! # Where this differs from upstream
//!
//! - A destination blend of 6 or 7 indexes `dstBlendLUT` (6 entries) past its
//!   end: undefined ([`W3dUndefined::IndexOutOfRange`]). Upstream's loader
//!   can produce such bits (an out-of-range `DestBlend` in the file).
//! - The 3dfx Voodoo 3 path (stage 2 juggling) is not modelled: the device
//!   is never a Voodoo 3 here.
//! - The device is described by [`DeviceCaps`]; the client's is
//!   [`DeviceCaps::full`] (every texture op, fog off).
//!
//! # Representation
//!
//! The Direct3D 8 constants are the SDK's values ([`d3d`]); the states are
//! [`ShaderStates`], with the states `Apply` does not write left `None`.

use super::mesh::shader_bits::*;
use super::{W3dResult, W3dUndefined};

/// Direct3D 8's constants that `ShaderClass::Apply` writes (d3d8types.h and
/// d3d8caps.h values).
pub mod d3d {
    pub const D3DBLEND_ZERO: u32 = 1;
    pub const D3DBLEND_ONE: u32 = 2;
    pub const D3DBLEND_SRCCOLOR: u32 = 3;
    pub const D3DBLEND_INVSRCCOLOR: u32 = 4;
    pub const D3DBLEND_SRCALPHA: u32 = 5;
    pub const D3DBLEND_INVSRCALPHA: u32 = 6;
    pub const D3DBLEND_DESTCOLOR: u32 = 9;

    pub const D3DCMP_LESSEQUAL: u32 = 4;
    pub const D3DCMP_GREATEREQUAL: u32 = 7;

    pub const D3DCULL_NONE: u32 = 1;
    pub const D3DCULL_CW: u32 = 2;
    pub const D3DCULL_CCW: u32 = 3;

    pub const D3DTOP_DISABLE: u32 = 1;
    pub const D3DTOP_SELECTARG1: u32 = 2;
    pub const D3DTOP_SELECTARG2: u32 = 3;
    pub const D3DTOP_MODULATE: u32 = 4;
    pub const D3DTOP_MODULATE2X: u32 = 5;
    pub const D3DTOP_ADD: u32 = 7;
    pub const D3DTOP_ADDSIGNED: u32 = 8;
    pub const D3DTOP_ADDSIGNED2X: u32 = 9;
    pub const D3DTOP_SUBTRACT: u32 = 10;
    pub const D3DTOP_ADDSMOOTH: u32 = 11;
    pub const D3DTOP_BLENDTEXTUREALPHA: u32 = 13;
    pub const D3DTOP_BLENDCURRENTALPHA: u32 = 16;
    pub const D3DTOP_MODULATEALPHA_ADDCOLOR: u32 = 18;
    pub const D3DTOP_BUMPENVMAP: u32 = 22;
    pub const D3DTOP_BUMPENVMAPLUMINANCE: u32 = 23;

    pub const D3DTA_DIFFUSE: u32 = 0;
    pub const D3DTA_CURRENT: u32 = 1;
    pub const D3DTA_TEXTURE: u32 = 2;

    pub const D3DTEXOPCAPS_SELECTARG1: u32 = 0x0000_0002;
    pub const D3DTEXOPCAPS_MODULATE: u32 = 0x0000_0008;
    pub const D3DTEXOPCAPS_MODULATE2X: u32 = 0x0000_0010;
    pub const D3DTEXOPCAPS_ADD: u32 = 0x0000_0040;
    pub const D3DTEXOPCAPS_ADDSIGNED: u32 = 0x0000_0080;
    pub const D3DTEXOPCAPS_ADDSIGNED2X: u32 = 0x0000_0100;
    pub const D3DTEXOPCAPS_SUBTRACT: u32 = 0x0000_0200;
    pub const D3DTEXOPCAPS_ADDSMOOTH: u32 = 0x0000_0400;
    pub const D3DTEXOPCAPS_BLENDTEXTUREALPHA: u32 = 0x0000_1000;
    pub const D3DTEXOPCAPS_BLENDCURRENTALPHA: u32 = 0x0000_8000;
    pub const D3DTEXOPCAPS_BUMPENVMAP: u32 = 0x0020_0000;
    pub const D3DTEXOPCAPS_BUMPENVMAPLUMINANCE: u32 = 0x0040_0000;
    /// Every `D3DTEXOPCAPS_*` bit (`DISABLE` through `MULTIPLYADD`, `LERP`).
    pub const D3DTEXOPCAPS_ALL: u32 = 0x03FF_FFFF;
}
use d3d::*;

/// `ShaderClass::DepthCompareType`, `PASS_LEQUAL`.
pub const PASS_LEQUAL: u32 = 3;
/// `ShaderClass::DstBlendFuncType` values.
pub const DSTBLEND_ZERO: u32 = 0;
pub const DSTBLEND_ONE: u32 = 1;
pub const DSTBLEND_SRC_COLOR: u32 = 2;
pub const DSTBLEND_ONE_MINUS_SRC_COLOR: u32 = 3;
pub const DSTBLEND_SRC_ALPHA: u32 = 4;
pub const DSTBLEND_ONE_MINUS_SRC_ALPHA: u32 = 5;
/// `ShaderClass::SrcBlendFuncType` values.
pub const SRCBLEND_ZERO: u32 = 0;
pub const SRCBLEND_ONE: u32 = 1;
pub const SRCBLEND_SRC_ALPHA: u32 = 2;
pub const SRCBLEND_ONE_MINUS_SRC_ALPHA: u32 = 3;
/// `ShaderClass::PriGradientType` values.
pub const GRADIENT_DISABLE: u32 = 0;
pub const GRADIENT_MODULATE: u32 = 1;
pub const GRADIENT_ADD: u32 = 2;
pub const GRADIENT_BUMPENVMAP: u32 = 3;
pub const GRADIENT_BUMPENVMAPLUMINANCE: u32 = 4;
pub const GRADIENT_MODULATE2X: u32 = 5;
/// `ShaderClass::FogFuncType` values.
pub const FOG_DISABLE: u32 = 0;
pub const FOG_ENABLE: u32 = 1;
pub const FOG_SCALE_FRAGMENT: u32 = 2;
pub const FOG_WHITE: u32 = 3;
/// `SORT_LEVEL_*` (w3d_file.h).
pub const SORT_LEVEL_NONE: i32 = 0;
pub const SORT_LEVEL_BIN1: i32 = 20;
pub const SORT_LEVEL_BIN2: i32 = 15;
pub const SORT_LEVEL_BIN3: i32 = 10;

/// `class ShaderClass`: its bits, with shader.h's accessors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShaderClass(pub u32);

impl ShaderClass {
    fn field(self, mask: u32, shift: u32) -> u32 {
        (self.0 & mask) >> shift
    }
    /// `Get_Depth_Compare()` (`PASS_*`).
    pub fn get_depth_compare(self) -> u32 {
        // ShaderBits&MASK_DEPTHCOMPARE>>SHIFT_DEPTHCOMPARE: the shift binds first
        self.0 & (MASK_DEPTHCOMPARE >> SHIFT_DEPTHCOMPARE)
    }
    pub fn get_depth_mask(self) -> u32 {
        self.field(MASK_DEPTHMASK, SHIFT_DEPTHMASK)
    }
    pub fn get_color_mask(self) -> u32 {
        self.field(MASK_COLORMASK, SHIFT_COLORMASK)
    }
    pub fn get_post_detail_alpha_func(self) -> u32 {
        self.field(MASK_POSTDETAILALPHAFUNC, SHIFT_POSTDETAILALPHAFUNC)
    }
    pub fn get_post_detail_color_func(self) -> u32 {
        self.field(MASK_POSTDETAILCOLORFUNC, SHIFT_POSTDETAILCOLORFUNC)
    }
    pub fn get_alpha_test(self) -> u32 {
        self.field(MASK_ALPHATEST, SHIFT_ALPHATEST)
    }
    pub fn get_cull_mode(self) -> u32 {
        self.field(MASK_CULLMODE, SHIFT_CULLMODE)
    }
    pub fn get_dst_blend_func(self) -> u32 {
        self.field(MASK_DSTBLEND, SHIFT_DSTBLEND)
    }
    pub fn get_fog_func(self) -> u32 {
        self.field(MASK_FOG, SHIFT_FOG)
    }
    pub fn get_primary_gradient(self) -> u32 {
        self.field(MASK_PRIGRADIENT, SHIFT_PRIGRADIENT)
    }
    pub fn get_secondary_gradient(self) -> u32 {
        self.field(MASK_SECGRADIENT, SHIFT_SECGRADIENT)
    }
    pub fn get_src_blend_func(self) -> u32 {
        self.field(MASK_SRCBLEND, SHIFT_SRCBLEND)
    }
    pub fn get_texturing(self) -> u32 {
        self.field(MASK_TEXTURING, SHIFT_TEXTURING)
    }
    pub fn get_npatch_enable(self) -> u32 {
        self.field(MASK_NPATCHENABLE, SHIFT_NPATCHENABLE)
    }
    /// `Set_Cull_Mode(x)`.
    pub fn set_cull_mode(&mut self, x: u32) {
        self.0 &= !MASK_CULLMODE;
        self.0 |= x << SHIFT_CULLMODE;
    }

    /// `Get_SS_Category()`: the static sort category (`SSCAT_*`).
    pub fn get_ss_category(self) -> StaticSortCategory {
        let (at, src, dst) = (self.get_alpha_test(), self.get_src_blend_func(), self.get_dst_blend_func());
        if at == 0 && dst == DSTBLEND_ZERO {
            return StaticSortCategory::Opaque;
        }
        if at == 1 {
            if dst == DSTBLEND_ZERO {
                return StaticSortCategory::AlphaTest;
            }
            if src == SRCBLEND_SRC_ALPHA && dst == DSTBLEND_ONE_MINUS_SRC_ALPHA {
                return StaticSortCategory::AlphaTest;
            }
        }
        if src == SRCBLEND_ONE && dst == DSTBLEND_ONE {
            return StaticSortCategory::Additive;
        }
        if src == SRCBLEND_ONE && dst == DSTBLEND_ONE_MINUS_SRC_COLOR {
            return StaticSortCategory::Screen;
        }
        StaticSortCategory::Other
    }

    /// `Guess_Sort_Level()`.
    pub fn guess_sort_level(self) -> i32 {
        match self.get_ss_category() {
            StaticSortCategory::Opaque | StaticSortCategory::AlphaTest => SORT_LEVEL_NONE,
            StaticSortCategory::Screen => SORT_LEVEL_BIN2,
            StaticSortCategory::Additive => SORT_LEVEL_BIN3,
            StaticSortCategory::Other => SORT_LEVEL_BIN1,
        }
    }
}

/// `ShaderClass::StaticSortCategoryType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticSortCategory {
    Opaque = 0,
    AlphaTest = 1,
    Additive = 2,
    Screen = 3,
    Other = 4,
}

/// What `Apply` asks of the device (`DX8Wrapper::Get_Current_Caps()`, the
/// fog state, `_PolygonCullMode`, `WW3D::Get_NPatches_Level()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceCaps {
    /// `Get_DX8_Caps().TextureOpCaps`.
    pub texture_op_caps: u32,
    /// `Support_ModAlphaAddClr()`.
    pub mod_alpha_add_clr: bool,
    /// `Is_Fog_Allowed() && Get_Fog_Enable()`: the fog colour (`Get_Fog_Color()`)
    /// when both, else `None`.
    pub fog: Option<u32>,
    /// `_PolygonCullMode` (`D3DCULL_CW`; `Invert_Backface_Culling(true)` makes
    /// it `D3DCULL_CCW`).
    pub polygon_cull_mode: u32,
    /// `WW3D::Get_NPatches_Level()`.
    pub npatches_level: u32,
}

impl DeviceCaps {
    /// A device with every texture op, fog off, upstream's default cull mode
    /// and N-patch level 1: what the client draws with.
    pub fn full() -> Self {
        DeviceCaps { texture_op_caps: D3DTEXOPCAPS_ALL, mod_alpha_add_clr: true, fog: None, polygon_cull_mode: D3DCULL_CW, npatches_level: 1 }
    }
}

/// One texture stage's ops and arguments (`D3DTSS_COLOROP`, `COLORARG1`,
/// `COLORARG2`, `ALPHAOP`, `ALPHAARG1`, `ALPHAARG2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageOps {
    pub color_op: u32,
    pub color_arg1: u32,
    pub color_arg2: u32,
    pub alpha_op: u32,
    pub alpha_arg1: u32,
    pub alpha_arg2: u32,
}

/// The device states `ShaderClass::Apply` writes (with `ShaderDirty`).
/// `None`: not written.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderStates {
    /// `D3DRS_ALPHABLENDENABLE`.
    pub alpha_blend_enable: bool,
    /// `D3DRS_SRCBLEND` (written only when blending).
    pub src_blend: Option<u32>,
    /// `D3DRS_DESTBLEND` (written only when blending).
    pub dst_blend: Option<u32>,
    /// `D3DRS_ALPHATESTENABLE` (its last write).
    pub alpha_test_enable: bool,
    /// `D3DRS_ALPHAREF` (only with alpha test).
    pub alpha_ref: Option<u32>,
    /// `D3DRS_ALPHAFUNC` (only with alpha test).
    pub alpha_func: Option<u32>,
    /// `D3DRS_FOGENABLE`.
    pub fog_enable: bool,
    /// `D3DRS_FOGCOLOR` (only when fogging).
    pub fog_color: Option<u32>,
    /// Texture stages 0 and 1.
    pub stage: [StageOps; 2],
    /// `D3DRS_SPECULARENABLE`.
    pub specular_enable: bool,
    /// `D3DRS_ZFUNC` (`D3DCMP_*`).
    pub zfunc: u32,
    /// `D3DRS_ZWRITEENABLE`.
    pub zwrite_enable: bool,
    /// `D3DRS_CULLMODE` (`D3DCULL_*`).
    pub cull_mode: u32,
    /// `D3DRS_PATCHSEGMENTS`.
    pub patch_segments: f32,
}

/// `srcBlendLUT[i]`: the factor (the `useAlpha` flag is computed upstream
/// and never used).
const SRC_BLEND_LUT: [u32; 4] = [D3DBLEND_ZERO, D3DBLEND_ONE, D3DBLEND_SRCALPHA, D3DBLEND_DESTCOLOR];
/// `dstBlendLUT[i]`.
const DST_BLEND_LUT: [u32; 6] =
    [D3DBLEND_ZERO, D3DBLEND_ONE, D3DBLEND_SRCCOLOR, D3DBLEND_INVSRCCOLOR, D3DBLEND_SRCALPHA, D3DBLEND_INVSRCALPHA];

/// `ShaderClass::Apply()` with `ShaderDirty` set: every state it writes.
pub fn apply(shader: ShaderClass, caps: &DeviceCaps) -> W3dResult<ShaderStates> {
    let tcaps = caps.texture_op_caps;

    // COLOR MASK
    let plane_mask = shader.get_color_mask() == 1;
    let (sf, df) = if !plane_mask {
        (D3DBLEND_ZERO, D3DBLEND_ONE)
    } else {
        let s = SRC_BLEND_LUT[shader.get_src_blend_func() as usize];
        let d = *DST_BLEND_LUT.get(shader.get_dst_blend_func() as usize).ok_or(W3dUndefined::IndexOutOfRange)?;
        (s, d)
    };
    let blend_on = sf != D3DBLEND_ONE || df != D3DBLEND_ZERO;
    let (src_blend, dst_blend) = if blend_on { (Some(sf), Some(df)) } else { (None, None) };
    let (mut alpha_ref, mut alpha_func) = (None, None);
    if shader.get_alpha_test() == 1 {
        let alphareference = 0x60u32;
        if sf == D3DBLEND_INVSRCALPHA {
            alpha_ref = Some(0xff - alphareference);
            alpha_func = Some(D3DCMP_LESSEQUAL);
        } else {
            alpha_ref = Some(alphareference);
            alpha_func = Some(D3DCMP_GREATEREQUAL);
        }
    }

    // FOG
    let (fog_enable, fog_color) = match caps.fog {
        Some(fog_color) => match shader.get_fog_func() {
            FOG_ENABLE => (true, Some(fog_color)),
            FOG_SCALE_FRAGMENT => (true, Some(0)),
            FOG_WHITE => (true, Some(0xffffff)),
            _ => (false, None),
        },
        None => (false, None),
    };

    // Defaults
    let mut pri = StageOps {
        color_op: D3DTOP_SELECTARG1,
        color_arg1: D3DTA_DIFFUSE,
        color_arg2: D3DTA_DIFFUSE,
        alpha_op: D3DTOP_SELECTARG1,
        alpha_arg1: D3DTA_DIFFUSE,
        alpha_arg2: D3DTA_DIFFUSE,
    };
    let mut sec = StageOps {
        color_op: D3DTOP_DISABLE,
        color_arg1: D3DTA_TEXTURE,
        color_arg2: D3DTA_CURRENT,
        alpha_op: D3DTOP_DISABLE,
        alpha_arg1: D3DTA_TEXTURE,
        alpha_arg2: D3DTA_CURRENT,
    };
    let set = |s: &mut StageOps, cop: u32, c1: u32, c2: u32, aop: u32, a1: u32, a2: u32| {
        *s = StageOps { color_op: cop, color_arg1: c1, color_arg2: c2, alpha_op: aop, alpha_arg1: a1, alpha_arg2: a2 };
    };
    let texturing = shader.get_texturing() == 1;
    if texturing {
        match shader.get_primary_gradient() {
            GRADIENT_DISABLE => set(&mut pri, D3DTOP_SELECTARG1, D3DTA_TEXTURE, D3DTA_CURRENT, D3DTOP_SELECTARG1, D3DTA_TEXTURE, D3DTA_CURRENT),
            GRADIENT_ADD => {
                let op = if tcaps & D3DTEXOPCAPS_ADD == 0 { D3DTOP_MODULATE } else { D3DTOP_ADD };
                set(&mut pri, op, D3DTA_TEXTURE, D3DTA_DIFFUSE, D3DTOP_MODULATE, D3DTA_TEXTURE, D3DTA_DIFFUSE);
            }
            GRADIENT_BUMPENVMAP => {
                if tcaps & D3DTEXOPCAPS_BUMPENVMAP != 0 {
                    set(&mut pri, D3DTOP_BUMPENVMAP, D3DTA_TEXTURE, D3DTA_DIFFUSE, D3DTOP_DISABLE, D3DTA_TEXTURE, D3DTA_CURRENT);
                } else {
                    set(&mut pri, D3DTOP_SELECTARG1, D3DTA_DIFFUSE, D3DTA_DIFFUSE, D3DTOP_SELECTARG1, D3DTA_DIFFUSE, D3DTA_DIFFUSE);
                }
            }
            GRADIENT_BUMPENVMAPLUMINANCE => {
                if tcaps & D3DTEXOPCAPS_BUMPENVMAPLUMINANCE != 0 {
                    set(&mut pri, D3DTOP_BUMPENVMAPLUMINANCE, D3DTA_TEXTURE, D3DTA_DIFFUSE, D3DTOP_DISABLE, D3DTA_TEXTURE, D3DTA_CURRENT);
                } else {
                    set(&mut pri, D3DTOP_SELECTARG1, D3DTA_DIFFUSE, D3DTA_DIFFUSE, D3DTOP_SELECTARG1, D3DTA_DIFFUSE, D3DTA_DIFFUSE);
                }
            }
            GRADIENT_MODULATE2X => {
                // the caps tested with the op code D3DTOP_MODULATE2X (kept)
                let op = if tcaps & D3DTOP_MODULATE2X == 0 { D3DTOP_MODULATE } else { D3DTOP_MODULATE2X };
                set(&mut pri, op, D3DTA_TEXTURE, D3DTA_DIFFUSE, D3DTOP_MODULATE, D3DTA_TEXTURE, D3DTA_DIFFUSE);
            }
            // GRADIENT_MODULATE, and default
            _ => set(&mut pri, D3DTOP_MODULATE, D3DTA_TEXTURE, D3DTA_DIFFUSE, D3DTOP_MODULATE, D3DTA_TEXTURE, D3DTA_DIFFUSE),
        }
    } else {
        match shader.get_primary_gradient() {
            GRADIENT_DISABLE => set(&mut pri, D3DTOP_DISABLE, D3DTA_TEXTURE, D3DTA_CURRENT, D3DTOP_DISABLE, D3DTA_TEXTURE, D3DTA_CURRENT),
            _ => set(&mut pri, D3DTOP_SELECTARG2, D3DTA_TEXTURE, D3DTA_DIFFUSE, D3DTOP_SELECTARG2, D3DTA_TEXTURE, D3DTA_DIFFUSE),
        }
    }

    if texturing {
        let tc = |s: &mut StageOps, op: u32| {
            s.color_op = op;
            s.color_arg1 = D3DTA_TEXTURE;
            s.color_arg2 = D3DTA_CURRENT;
        };
        match shader.get_post_detail_color_func() {
            1 if tcaps & D3DTEXOPCAPS_SELECTARG1 != 0 => tc(&mut sec, D3DTOP_SELECTARG1), // DETAIL
            2 if tcaps & D3DTEXOPCAPS_MODULATE != 0 => tc(&mut sec, D3DTOP_MODULATE), // SCALE
            3 => {
                // INVSCALE
                if tcaps & D3DTEXOPCAPS_ADDSMOOTH != 0 {
                    tc(&mut sec, D3DTOP_ADDSMOOTH);
                } else if tcaps & D3DTEXOPCAPS_ADD != 0 {
                    tc(&mut sec, D3DTOP_ADD);
                }
            }
            4 if tcaps & D3DTEXOPCAPS_ADD != 0 => tc(&mut sec, D3DTOP_ADD), // ADD
            5 if tcaps & D3DTEXOPCAPS_SUBTRACT != 0 => tc(&mut sec, D3DTOP_SUBTRACT), // SUB
            6 if tcaps & D3DTEXOPCAPS_SUBTRACT != 0 => {
                // SUBR
                sec.color_op = D3DTOP_SUBTRACT;
                sec.color_arg1 = D3DTA_CURRENT;
                sec.color_arg2 = D3DTA_TEXTURE;
            }
            7 if tcaps & D3DTEXOPCAPS_BLENDTEXTUREALPHA != 0 => tc(&mut sec, D3DTOP_BLENDTEXTUREALPHA), // BLEND
            8 if tcaps & D3DTEXOPCAPS_BLENDCURRENTALPHA != 0 => tc(&mut sec, D3DTOP_BLENDCURRENTALPHA), // DETAILBLEND
            9 => {
                // ADDSIGNED
                if tcaps & D3DTEXOPCAPS_ADDSIGNED != 0 {
                    tc(&mut sec, D3DTOP_ADDSIGNED);
                } else if tcaps & D3DTEXOPCAPS_ADD != 0 {
                    tc(&mut sec, D3DTOP_ADD);
                }
            }
            10 => {
                // ADDSIGNED2X
                if tcaps & D3DTEXOPCAPS_ADDSIGNED2X != 0 {
                    tc(&mut sec, D3DTOP_ADDSIGNED2X);
                } else if tcaps & D3DTEXOPCAPS_ADDSIGNED != 0 {
                    tc(&mut sec, D3DTOP_ADDSIGNED);
                } else if tcaps & D3DTEXOPCAPS_ADD != 0 {
                    tc(&mut sec, D3DTOP_ADD);
                }
            }
            11 => {
                // SCALE2X
                if tcaps & D3DTEXOPCAPS_MODULATE2X != 0 {
                    tc(&mut sec, D3DTOP_MODULATE2X);
                } else if tcaps & D3DTEXOPCAPS_MODULATE != 0 {
                    tc(&mut sec, D3DTOP_MODULATE);
                }
            }
            12 => {
                // MODALPHAADDCOLOR
                if caps.mod_alpha_add_clr {
                    sec.color_op = D3DTOP_MODULATEALPHA_ADDCOLOR;
                    sec.color_arg1 = D3DTA_CURRENT;
                    sec.color_arg2 = D3DTA_TEXTURE;
                } else if tcaps & D3DTEXOPCAPS_ADD != 0 {
                    tc(&mut sec, D3DTOP_ADD);
                }
            }
            // DETAILCOLOR_DISABLE, default, and a function the caps lack
            _ => {}
        }
        let ta = |s: &mut StageOps, op: u32| {
            s.alpha_op = op;
            s.alpha_arg1 = D3DTA_TEXTURE;
            s.alpha_arg2 = D3DTA_CURRENT;
        };
        match shader.get_post_detail_alpha_func() {
            1 if tcaps & D3DTEXOPCAPS_SELECTARG1 != 0 => ta(&mut sec, D3DTOP_SELECTARG1),
            2 if tcaps & D3DTEXOPCAPS_MODULATE != 0 => ta(&mut sec, D3DTOP_MODULATE),
            3 if tcaps & D3DTEXOPCAPS_ADDSMOOTH != 0 => ta(&mut sec, D3DTOP_ADDSMOOTH),
            _ => {}
        }
        if sec.color_op != D3DTOP_DISABLE && sec.alpha_op == D3DTOP_DISABLE {
            sec.alpha_op = D3DTOP_SELECTARG2;
            sec.alpha_arg2 = D3DTA_CURRENT;
        } else if sec.color_op == D3DTOP_DISABLE && sec.alpha_op != D3DTOP_DISABLE {
            sec.color_op = D3DTOP_SELECTARG2;
            sec.color_arg2 = D3DTA_CURRENT;
        }
    }

    let patch_segments = if shader.get_npatch_enable() != 0 { caps.npatches_level as f32 } else { 1.0 };
    Ok(ShaderStates {
        alpha_blend_enable: blend_on,
        src_blend,
        dst_blend,
        // written as alphaTest, then again as BOOL(Get_Alpha_Test())
        alpha_test_enable: shader.get_alpha_test() != 0,
        alpha_ref,
        alpha_func,
        fog_enable,
        fog_color,
        stage: [pri, sec],
        specular_enable: shader.get_secondary_gradient() != 0,
        zfunc: shader.get_depth_compare() + 1,
        zwrite_enable: shader.get_depth_mask() != 0,
        cull_mode: if shader.get_cull_mode() != 0 { caps.polygon_cull_mode } else { D3DCULL_NONE },
        patch_segments,
    })
}

#[cfg(test)]
mod tests {
    use super::super::mesh::shader_reset;
    use super::*;

    /// Predictions, written before the first run:
    /// - the default shader (`ShaderClass()`: LEQUAL, depth write, colour
    ///   write, ONE/ZERO, modulate, no texturing, cull) blends nothing, tests
    ///   no alpha, takes the vertex colour (SELECTARG2 of DIFFUSE), disables
    ///   stage 1, LEQUAL is `D3DCMP_LESSEQUAL` (4), culls CW;
    /// - an alpha-blended textured shader (SRC_ALPHA, ONE_MINUS_SRC_ALPHA)
    ///   blends SRCALPHA/INVSRCALPHA and modulates texture by diffuse;
    /// - a source blend of ONE_MINUS_SRC_ALPHA gives DESTCOLOR (the kept LUT
    ///   quirk), and with alpha test the reference is still 0x60 GREATEREQUAL;
    /// - colour writes off force (ZERO, ONE), blending on;
    /// - destination blend 6 is undefined.
    #[test]
    fn apply_decodes_the_bits_as_upstream() {
        let caps = DeviceCaps::full();
        let def = apply(ShaderClass(shader_reset()), &caps).unwrap();
        assert!(!def.alpha_blend_enable);
        assert_eq!((def.src_blend, def.dst_blend), (None, None));
        assert!(!def.alpha_test_enable);
        assert_eq!(def.stage[0].color_op, D3DTOP_SELECTARG2);
        assert_eq!(def.stage[0].color_arg2, D3DTA_DIFFUSE);
        assert_eq!(def.stage[1].color_op, D3DTOP_DISABLE);
        assert_eq!(def.zfunc, D3DCMP_LESSEQUAL);
        assert!(def.zwrite_enable);
        assert_eq!(def.cull_mode, D3DCULL_CW);

        let mut alpha = ShaderClass(shader_reset());
        alpha.0 = (alpha.0 & !(MASK_SRCBLEND | MASK_DSTBLEND | MASK_TEXTURING))
            | SRCBLEND_SRC_ALPHA << SHIFT_SRCBLEND
            | DSTBLEND_ONE_MINUS_SRC_ALPHA << SHIFT_DSTBLEND
            | 1 << SHIFT_TEXTURING;
        let a = apply(alpha, &caps).unwrap();
        assert!(a.alpha_blend_enable);
        assert_eq!((a.src_blend, a.dst_blend), (Some(D3DBLEND_SRCALPHA), Some(D3DBLEND_INVSRCALPHA)));
        assert_eq!(a.stage[0], StageOps {
            color_op: D3DTOP_MODULATE,
            color_arg1: D3DTA_TEXTURE,
            color_arg2: D3DTA_DIFFUSE,
            alpha_op: D3DTOP_MODULATE,
            alpha_arg1: D3DTA_TEXTURE,
            alpha_arg2: D3DTA_DIFFUSE
        });
        assert_eq!(alpha.get_ss_category(), StaticSortCategory::Other);

        let mut quirk = ShaderClass(shader_reset());
        quirk.0 = (quirk.0 & !MASK_SRCBLEND) | SRCBLEND_ONE_MINUS_SRC_ALPHA << SHIFT_SRCBLEND | 1 << SHIFT_ALPHATEST;
        let q = apply(quirk, &caps).unwrap();
        assert_eq!(q.src_blend, Some(D3DBLEND_DESTCOLOR));
        assert_eq!((q.alpha_ref, q.alpha_func), (Some(0x60), Some(D3DCMP_GREATEREQUAL)));

        let mut nocolor = ShaderClass(shader_reset());
        nocolor.0 &= !MASK_COLORMASK;
        let n = apply(nocolor, &caps).unwrap();
        assert_eq!((n.alpha_blend_enable, n.src_blend, n.dst_blend), (true, Some(D3DBLEND_ZERO), Some(D3DBLEND_ONE)));

        let bad = ShaderClass(shader_reset() | 6 << SHIFT_DSTBLEND);
        assert_eq!(apply(bad, &caps), Err(W3dUndefined::IndexOutOfRange.into()));

        let mut add = ShaderClass(shader_reset());
        add.0 = (add.0 & !(MASK_SRCBLEND | MASK_DSTBLEND)) | SRCBLEND_ONE << SHIFT_SRCBLEND | DSTBLEND_ONE << SHIFT_DSTBLEND;
        assert_eq!(add.guess_sort_level(), SORT_LEVEL_BIN3);
    }
}
