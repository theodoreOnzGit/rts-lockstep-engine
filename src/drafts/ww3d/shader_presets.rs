//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.cpp (the `SC_*` preset definitions, lines 57-260)
//!
//! The verified port's `crate::formats::w3d::shader` carries only the presets the game has needed so far
//! (`PRESET_ADDITIVE_SPRITE_SHADER`, `PRESET_ADDITIVE_SHADER`, `PRESET_ALPHA_SPRITE_SHADER`) and keeps `shade_cnst`
//! private. The other presets the drafts use are defined here with the same field order. Constants not exported
//! by the port are written as upstream's literal values: `PASS_ALWAYS` = 7, `DEPTH_WRITE_ENABLE` = 1,
//! `COLOR_WRITE_ENABLE` = 1, `TEXTURING_ENABLE` = 1, `ALPHATEST_ENABLE` = 1, `CULL_MODE_ENABLE` = 1, the
//! `*_DISABLE` ones 0. `SRCBLEND_ONE_MINUS_SRC_ALPHA`-style enumerators come from the port.

use crate::formats::w3d::mesh::shader_bits::*;
use crate::formats::w3d::shader::*;

const PASS_ALWAYS: u32 = 7;

#[allow(clippy::too_many_arguments)]
const fn sc(
    depth_compare: u32,
    depth_mask: u32,
    src_blend: u32,
    dst_blend: u32,
    pri_grad: u32,
    texture: u32,
    alpha_test: u32,
) -> ShaderClass {
    // SHADE_CNST(depth_compare, depth_mask, COLOR_WRITE_ENABLE, src, dst, FOG_DISABLE, pri_grad,
    //            SECONDARY_GRADIENT_DISABLE, texture, alpha_test, CULL_MODE_ENABLE, DETAILCOLOR_DISABLE,
    //            DETAILALPHA_DISABLE)
    ShaderClass(
        depth_compare << SHIFT_DEPTHCOMPARE
            | depth_mask << SHIFT_DEPTHMASK
            | 1u32 << SHIFT_COLORMASK
            | dst_blend << SHIFT_DSTBLEND
            | FOG_DISABLE << SHIFT_FOG
            | pri_grad << SHIFT_PRIGRADIENT
            | 0u32 << SHIFT_SECGRADIENT
            | src_blend << SHIFT_SRCBLEND
            | texture << SHIFT_TEXTURING
            | alpha_test << SHIFT_ALPHATEST
            | 1u32 << SHIFT_CULLMODE
            | 0u32 << SHIFT_POSTDETAILCOLORFUNC
            | 0u32 << SHIFT_POSTDETAILALPHAFUNC,
    )
}

/// `SC_OPAQUE`: texturing, zbuffer, primary gradient, no blending.
pub const PRESET_OPAQUE_SHADER: ShaderClass = sc(PASS_LEQUAL, 1, SRCBLEND_ONE, DSTBLEND_ZERO, GRADIENT_MODULATE, 1, 0);
/// `SC_OP_2D`.
pub const PRESET_OPAQUE_2D_SHADER: ShaderClass = sc(PASS_ALWAYS, 0, SRCBLEND_ONE, DSTBLEND_ZERO, GRADIENT_DISABLE, 1, 0);
/// `SC_OP_SPRITE`.
pub const PRESET_OPAQUE_SPRITE_SHADER: ShaderClass = sc(PASS_LEQUAL, 0, SRCBLEND_ONE, DSTBLEND_ZERO, GRADIENT_DISABLE, 1, 0);
/// `SC_ADD_2D`.
pub const PRESET_ADDITIVE_2D_SHADER: ShaderClass = sc(PASS_ALWAYS, 0, SRCBLEND_ONE, DSTBLEND_ONE, GRADIENT_DISABLE, 1, 0);
/// `SC_ALPHA_2D`.
pub const PRESET_ALPHA_2D_SHADER: ShaderClass =
    sc(PASS_ALWAYS, 0, SRCBLEND_SRC_ALPHA, DSTBLEND_ONE_MINUS_SRC_ALPHA, GRADIENT_DISABLE, 1, 0);
/// `SC_ALPHA`: texturing, zbuffer read, no write, primary gradient, alpha blending.
pub const PRESET_ALPHA_SHADER: ShaderClass =
    sc(PASS_LEQUAL, 0, SRCBLEND_SRC_ALPHA, DSTBLEND_ONE_MINUS_SRC_ALPHA, GRADIENT_MODULATE, 1, 0);
/// `SC_OPAQUE_SOLID`: no texturing, zbuffer, primary gradient, no blending.
pub const PRESET_OPAQUE_SOLID_SHADER: ShaderClass = sc(PASS_LEQUAL, 1, SRCBLEND_ONE, DSTBLEND_ZERO, GRADIENT_MODULATE, 0, 0);
/// `SC_ADDITIVE_SOLID`.
pub const PRESET_ADDITIVE_SOLID_SHADER: ShaderClass = sc(PASS_LEQUAL, 0, SRCBLEND_ONE, DSTBLEND_ONE, GRADIENT_MODULATE, 0, 0);
/// `SC_ALPHA_SOLID`.
pub const PRESET_ALPHA_SOLID_SHADER: ShaderClass =
    sc(PASS_LEQUAL, 0, SRCBLEND_SRC_ALPHA, DSTBLEND_ONE_MINUS_SRC_ALPHA, GRADIENT_MODULATE, 0, 0);
/// `SC_ATEST_2D`.
pub const PRESET_ATEST_2D_SHADER: ShaderClass = sc(PASS_ALWAYS, 0, SRCBLEND_ONE, DSTBLEND_ZERO, GRADIENT_DISABLE, 1, 1);
/// `SC_ATEST_SPRITE` (depth write enabled; see shader.cpp:180).
pub const PRESET_ATEST_SPRITE_SHADER: ShaderClass = sc(PASS_LEQUAL, 1, SRCBLEND_ONE, DSTBLEND_ZERO, GRADIENT_DISABLE, 1, 1);
/// `SC_ATESTBLEND_2D`.
pub const PRESET_ATEST_BLEND_2D_SHADER: ShaderClass =
    sc(PASS_ALWAYS, 0, SRCBLEND_SRC_ALPHA, DSTBLEND_ONE_MINUS_SRC_ALPHA, GRADIENT_DISABLE, 1, 1);
/// `SC_ATESTBLEND_SPRITE` (depth write enabled; see shader.cpp:201).
pub const PRESET_ATEST_BLEND_SPRITE_SHADER: ShaderClass =
    sc(PASS_LEQUAL, 1, SRCBLEND_SRC_ALPHA, DSTBLEND_ONE_MINUS_SRC_ALPHA, GRADIENT_DISABLE, 1, 1);
/// `SC_SCREEN_2D`.
pub const PRESET_SCREEN_2D_SHADER: ShaderClass =
    sc(PASS_ALWAYS, 0, SRCBLEND_ONE, DSTBLEND_ONE_MINUS_SRC_COLOR, GRADIENT_DISABLE, 1, 0);
/// `SC_SCREEN_SPRITE`.
pub const PRESET_SCREEN_SPRITE_SHADER: ShaderClass =
    sc(PASS_LEQUAL, 0, SRCBLEND_ONE, DSTBLEND_ONE_MINUS_SRC_COLOR, GRADIENT_DISABLE, 1, 0);
/// `SC_MUL_2D`.
pub const PRESET_MULTIPLICATIVE_2D_SHADER: ShaderClass =
    sc(PASS_ALWAYS, 0, SRCBLEND_ZERO, DSTBLEND_SRC_COLOR, GRADIENT_DISABLE, 1, 0);
/// `SC_MUL_SPRITE`.
pub const PRESET_MULTIPLICATIVE_SPRITE_SHADER: ShaderClass =
    sc(PASS_LEQUAL, 0, SRCBLEND_ZERO, DSTBLEND_SRC_COLOR, GRADIENT_DISABLE, 1, 0);
/// `SC_MULTIPLICATIVE`.
pub const PRESET_MULTIPLICATIVE_SHADER: ShaderClass =
    sc(PASS_LEQUAL, 0, SRCBLEND_ZERO, DSTBLEND_SRC_COLOR, GRADIENT_MODULATE, 1, 0);
