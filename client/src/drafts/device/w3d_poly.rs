//! UNTESTED DRAFT (#202, #205, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DPoly.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DPoly.h (`ClipPolyClass`)
//!
//! `PlaneClass::In_Front(point)` is `Dot_Product(point, N) > D` (plane.h:183); the port's `PlaneClass` has no such
//! method, so it is written out in [`in_front`].

use rts_lockstep_engine::ww::plane::PlaneClass;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `PlaneClass::In_Front(const Vector3&)` (plane.h:183).
pub fn in_front(plane: &PlaneClass, point: &Vector3) -> bool {
    let dist = Vector3::dot_product(point, &plane.n);
    dist > plane.d
}

/// `class ClipPolyClass`: a polygon (`Verts`) clipped against planes.
#[derive(Clone, Debug, Default)]
pub struct ClipPolyClass {
    /// `Verts`.
    pub verts: Vec<Vector3>,
}

impl ClipPolyClass {
    /// `ClipPolyClass::Reset` (W3DPoly.cpp:36): delete all vertices.
    pub fn reset(&mut self) {
        self.verts.clear();
    }

    /// `ClipPolyClass::Add_Vertex` (W3DPoly.cpp:44): add a new vertex.
    pub fn add_vertex(&mut self, point: &Vector3) {
        self.verts.push(*point);
    }

    /// `ClipPolyClass::Clip` (W3DPoly.cpp:52): clip to `plane`, the new polygon in `dest`.
    pub fn clip(&self, plane: &PlaneClass, dest: &mut ClipPolyClass) {
        dest.reset();

        // temporary variables used in clipping
        let mut i: usize = 0;
        let vcount = self.verts.len();
        if vcount <= 2 {
            return;
        }
        let mut iprev = vcount - 1;

        // perform clipping
        let mut prev_point_in_front = !in_front(plane, &self.verts[iprev]); // Note, plane normal is outward so we invert this test
        for _j in 0..vcount {
            let cur_point_in_front = !in_front(plane, &self.verts[i]); // Note, plane nomral is out so we invert this test
            if prev_point_in_front {
                if cur_point_in_front {
                    // Previous vertex was in front of plane and this vertex is in
                    // front of the plane so we emit this vertex.
                    dest.add_vertex(&self.verts[i]);
                } else {
                    // Previous vert was in front, this vert is behind, compute
                    // the intersection and emit the point.
                    let (_hit, alpha) = plane.compute_intersection(&self.verts[iprev], &self.verts[i]);
                    let int_point = Vector3::lerp(&self.verts[iprev], &self.verts[i], alpha.unwrap_or(0.0));
                    dest.add_vertex(&int_point);
                }
            } else if cur_point_in_front {
                // segment is going from the back halfspace to the front halfspace
                // compute the intersection and emit it, then continue
                // the edge into the front halfspace and emit the end point.
                let (_hit, alpha) = plane.compute_intersection(&self.verts[iprev], &self.verts[i]);
                let int_point = Vector3::lerp(&self.verts[iprev], &self.verts[i], alpha.unwrap_or(0.0));
                dest.add_vertex(&int_point);
                dest.add_vertex(&self.verts[i]);
            }

            prev_point_in_front = cur_point_in_front;
            iprev = i;

            //i = (i+1)%(Verts.Count());
            i += 1;
            if i >= vcount {
                i = 0;
            }
        }
    }
}
