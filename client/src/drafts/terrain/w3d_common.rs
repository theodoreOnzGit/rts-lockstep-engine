//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d): no file of its own. The small WW3D2 types the terrain
//! buffers share and the port does not have yet, under upstream's names:
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/sphere.h (`SphereClass`)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aabox.h (`AABoxClass`)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.h (`RenderObjClass`, as an opaque handle)

use rts_lockstep_engine::ww::vector3::Vector3;

/// `class SphereClass` (sphere.h): `Center`, `Radius`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SphereClass {
    /// `Center`.
    pub center: Vector3,
    /// `Radius`.
    pub radius: f32,
}

impl SphereClass {
    /// `SphereClass::SphereClass(const Vector3 &center, float radius)`.
    pub fn new(center: Vector3, radius: f32) -> Self {
        SphereClass { center, radius }
    }
}

/// `class AABoxClass` (aabox.h): `Center`, `Extent`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AABoxClass {
    /// `Center`.
    pub center: Vector3,
    /// `Extent`: half-sizes.
    pub extent: Vector3,
}

/// A `RenderObjClass *` of the device side (`WW3DAssetManager::Create_Render_Obj` / `Clone`), as an opaque id the
/// renderer assigns. `None` where upstream holds NULL. Releasing one (`REF_PTR_RELEASE`) is the caller's, device-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RenderObjHandle(pub u32);

/// `struct VertexFormatXYZDUV1` (dx8fvf.h): position, diffuse, one UV set. `DX8_FVF_XYZDUV1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexFormatXYZDUV1 {
    /// `x`.
    pub x: f32,
    /// `y`.
    pub y: f32,
    /// `z`.
    pub z: f32,
    /// `diffuse`: packed ARGB.
    pub diffuse: u32,
    /// `u1`.
    pub u1: f32,
    /// `v1`.
    pub v1: f32,
}

/// One `DX8Wrapper::Draw_Triangles(start_index, polygon_count, min_vertex_index, vertex_count)` call (device-only, not
/// translated): the index range a buffer's draw would use, recorded so a Bevy mesh can be cut from the same ranges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawTriangles {
    /// `start_index`.
    pub start_index: i32,
    /// `polygon_count`.
    pub polygon_count: i32,
    /// `min_vertex_index`.
    pub min_vertex_index: i32,
    /// `vertex_count`.
    pub vertex_count: i32,
}

/// `TextureLoader::Validate_Texture_Size` (textureloader.cpp:360): rounds each dimension up to a power of two, caps them
/// at the device's maximums (`D3DCAPS8::MaxTextureWidth`, `MaxTextureHeight`, `MaxVolumeExtent`, passed in) and widens
/// the smaller side until the aspect ratio is at most 8:1. Returns `(width, height, depth)`.
pub fn validate_texture_size(width: u32, height: u32, depth: u32, max_width: u32, max_height: u32, max_volume_extent: u32) -> (u32, u32, u32) {
    let mut poweroftwowidth: u32 = 1;
    while poweroftwowidth < width {
        poweroftwowidth <<= 1;
    }

    let mut poweroftwoheight: u32 = 1;
    while poweroftwoheight < height {
        poweroftwoheight <<= 1;
    }

    let mut poweroftwodepth: u32 = 1;
    while poweroftwodepth < depth {
        poweroftwodepth <<= 1;
    }

    if poweroftwowidth > max_width {
        poweroftwowidth = max_width;
    }
    if poweroftwoheight > max_height {
        poweroftwoheight = max_height;
    }
    if poweroftwodepth > max_volume_extent {
        poweroftwodepth = max_volume_extent;
    }

    if poweroftwowidth > poweroftwoheight {
        while poweroftwowidth / poweroftwoheight > 8 {
            poweroftwoheight *= 2;
        }
    } else {
        while poweroftwoheight / poweroftwowidth > 8 {
            poweroftwowidth *= 2;
        }
    }

    (poweroftwowidth, poweroftwoheight, poweroftwodepth)
}
