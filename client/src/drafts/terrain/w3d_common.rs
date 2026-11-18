//! UNTESTED DRAFT (#202, #204, #214): compiles; not yet tested against the upstream C++.
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

    /// `SphereClass::SphereClass(const Vector3 *Position, const int VertCount)` (sphere.h:114): the bounding sphere of
    /// a point set by the algorithm of Graphics Gems I page 301 (within about 5% of the optimal one). `positions` must
    /// not be empty (upstream reads `Position[0]` unconditionally).
    pub fn from_points(positions: &[Vector3]) -> SphereClass {
        let vert_count = positions.len();
        let mut xmin = positions[0];
        let mut xmax = positions[0];
        let mut ymin = positions[0];
        let mut ymax = positions[0];
        let mut zmin = positions[0];
        let mut zmax = positions[0];

        // FIRST PASS: finding the 6 minima and maxima points
        for i in 1..vert_count {
            let p = positions[i];
            if p.x < xmin.x {
                xmin = p;
            }
            if p.x > xmax.x {
                xmax = p;
            }
            if p.y < ymin.y {
                ymin = p;
            }
            if p.y > ymax.y {
                ymax = p;
            }
            if p.z < zmin.z {
                zmin = p;
            }
            if p.z > zmax.z {
                zmax = p;
            }
        }

        // xspan = distance between the 2 points xmin and xmax squared. same goes for yspan and zspan.
        let span = |a: &Vector3, b: &Vector3| -> f64 {
            let dx = (b.x - a.x) as f64;
            let dy = (b.y - a.y) as f64;
            let dz = (b.z - a.z) as f64;
            dx * dx + dy * dy + dz * dz
        };
        let xspan = span(&xmin, &xmax);
        let yspan = span(&ymin, &ymax);
        let zspan = span(&zmin, &zmax);

        // Set points dia1 and dia2 to the maximally separated pair. This will be the diameter of the initial sphere
        let mut dia1 = xmin;
        let mut dia2 = xmax;
        let mut maxspan = xspan;

        if yspan > maxspan {
            maxspan = yspan;
            dia1 = ymin;
            dia2 = ymax;
        }
        if zspan > maxspan {
            dia1 = zmin;
            dia2 = zmax;
        }

        // Compute initial center and radius and radius squared
        let mut center = Vector3::default();
        center.x = (dia1.x + dia2.x) / 2.0;
        center.y = (dia1.y + dia2.y) / 2.0;
        center.z = (dia1.z + dia2.z) / 2.0;

        let dx = (dia2.x - center.x) as f64;
        let dy = (dia2.y - center.y) as f64;
        let dz = (dia2.z - center.z) as f64;

        let mut radsqr = dx * dx + dy * dy + dz * dz;
        let mut radius = radsqr.sqrt();

        // SECOND PASS: increment current sphere if any points fall outside of it.
        for i in 0..vert_count {
            let dx = (positions[i].x - center.x) as f64;
            let dy = (positions[i].y - center.y) as f64;
            let dz = (positions[i].z - center.z) as f64;

            let testrad2 = dx * dx + dy * dy + dz * dz;

            if testrad2 > radsqr {
                // this point was outside the old sphere, compute a new center point and radius which contains this point
                let testrad = testrad2.sqrt();

                // adjust center and radius
                radius = (radius + testrad) / 2.0;
                radsqr = radius * radius;

                let oldtonew = testrad - radius;
                center.x = ((radius * center.x as f64 + oldtonew * positions[i].x as f64) / testrad) as f32;
                center.y = ((radius * center.y as f64 + oldtonew * positions[i].y as f64) / testrad) as f32;
                center.z = ((radius * center.z as f64 + oldtonew * positions[i].z as f64) / testrad) as f32;
            }
        }

        SphereClass { center, radius: radius as f32 }
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

/// `struct VertexFormatXYZDUV2` (dx8fvf.h): position, diffuse, two UV sets. `DX8_FVF_XYZDUV2`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexFormatXYZDUV2 {
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
    /// `u2`.
    pub u2: f32,
    /// `v2`.
    pub v2: f32,
}

/// `struct VertexFormatXYZNDUV1` (dx8fvf.h): position, normal, diffuse, one UV set. `DX8_FVF_XYZNDUV1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexFormatXYZNDUV1 {
    /// `x`.
    pub x: f32,
    /// `y`.
    pub y: f32,
    /// `z`.
    pub z: f32,
    /// `nx`.
    pub nx: f32,
    /// `ny`.
    pub ny: f32,
    /// `nz`.
    pub nz: f32,
    /// `diffuse`: packed ARGB.
    pub diffuse: u32,
    /// `u1`.
    pub u1: f32,
    /// `v1`.
    pub v1: f32,
}

/// A `TextureClass *` of the device side (`WW3DAssetManager::Get_Texture`), as an opaque id the renderer assigns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

/// The mesh data `MeshClass::Peek_Model()` hands the terrain buffers (`Get_Vertex_Count`, `Get_Vertex_Array`,
/// `Get_Vertex_Normal_Array`, `Get_UV_Array_By_Index(0)`, `Get_Polygon_Count`, `Get_Polygon_Array`).
#[derive(Clone, Debug, Default)]
pub struct MeshModelData {
    /// `Get_Vertex_Array`.
    pub vertices: Vec<Vector3>,
    /// `Get_Vertex_Normal_Array`.
    pub normals: Vec<Vector3>,
    /// `Get_UV_Array_By_Index(0)`: `(U, V)` per vertex.
    pub uvs: Vec<(f32, f32)>,
    /// `Get_Polygon_Array`: `TriIndex` `(I, J, K)` per polygon.
    pub polygons: Vec<(u32, u32, u32)>,
}

/// `class LineSegClass` (lineseg.h, lineseg.cpp): a line segment with its precomputed direction and length. Only the
/// members the terrain buffers use.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineSegClass {
    /// `P0`: start point.
    pub p0: Vector3,
    /// `P1`: end point.
    pub p1: Vector3,
    /// `DP`: difference of the two points.
    pub dp: Vector3,
    /// `Dir`: normalized direction.
    pub dir: Vector3,
    /// `Length`: length of the segment.
    pub length: f32,
}

impl LineSegClass {
    /// `LineSegClass(const Vector3 &p0, const Vector3 &p1)` (lineseg.h:63).
    pub fn new(p0: Vector3, p1: Vector3) -> Self {
        let mut l = LineSegClass { p0, p1, dp: Vector3::default(), dir: Vector3::default(), length: 0.0 };
        l.recalculate();
        l
    }

    /// `LineSegClass::recalculate` (lineseg.h:27).
    fn recalculate(&mut self) {
        self.dp = self.p1 - self.p0;
        self.dir = self.dp;
        self.dir.normalize();
        self.length = self.dp.length();
    }

    /// `LineSegClass::Set(p0, p1)` (lineseg.h).
    pub fn set(&mut self, p0: Vector3, p1: Vector3) {
        self.p0 = p0;
        self.p1 = p1;
        self.recalculate();
    }

    /// `LineSegClass::Get_Dir` (lineseg.h).
    pub fn get_dir(&self) -> Vector3 {
        self.dir
    }

    /// `LineSegClass::Find_Point_Closest_To` (lineseg.cpp:111).
    pub fn find_point_closest_to(&self, pos: &Vector3) -> Vector3 {
        // Get a vector from one line endpoint to point in question.
        let v_0_pos = *pos - self.p0;
        let dotprod = Vector3::dot_product(&self.dir, &v_0_pos);

        // Check to see if point is past either of the endpoints.
        if dotprod <= 0.0 {
            self.p0
        } else if dotprod >= self.length {
            self.p1
        } else {
            // Find point on line seg that is closest to pos passed in.
            self.p0 + (dotprod * self.dir)
        }
    }

    /// `LineSegClass::Find_Intersection` (lineseg.cpp:140): the closest points on the two (infinite) lines, and the
    /// fractions of `Length`. `None` when the lines are parallel (the `ALLOW_TEMPORARIES` branch is not compiled).
    pub fn find_intersection(&self, other_line: &LineSegClass) -> Option<(Vector3, f32, Vector3, f32)> {
        let cross1 = Vector3::cross_product(&self.dir, &other_line.dir);
        let cross2 = Vector3::cross_product(&(other_line.p0 - self.p0), &other_line.dir);
        let top1 = Vector3::dot_product(&cross2, &cross1);
        let bottom1 = Vector3::dot_product(&cross1, &cross1);
        let cross3 = Vector3::cross_product(&other_line.dir, &self.dir);
        let cross4 = Vector3::cross_product(&(self.p0 - other_line.p0), &self.dir);
        let top2 = Vector3::dot_product(&cross4, &cross3);
        let bottom2 = Vector3::dot_product(&cross3, &cross3);

        // If either of the divisors are 0, then the lines are parallel
        if bottom1 != 0.0 && bottom2 != 0.0 {
            let length1 = top1 / bottom1;
            let length2 = top2 / bottom2;

            // Calculate the closest points on both lines. Note: If the points are the same, then the lines intersect.
            let p1 = self.p0 + (self.dir * length1);
            let p2 = other_line.p0 + (other_line.dir * length2);
            return Some((p1, length1 / self.length, p2, length2 / self.length));
        }
        None
    }
}
