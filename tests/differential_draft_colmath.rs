// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215) of the WWMath collision maths drafts (`src/drafts/wwlib/colmath*.rs` and the shapes
//! they use) against upstream's own `colmath*.cpp`, `aabox.cpp`, `obbox.cpp`, `lineseg.cpp` and `tri.cpp`, compiled whole
//! and unchanged (build.rs `drafts_colmath`, differential/wrap_draft_colmath.cpp), commit 0a05454d.
//!
//! # Methodology
//!
//! Per function (one `CollisionMath` overload), `CASES` random cases (default 20,000) from a fixed seed. The same
//! operands go to the Rust draft and to the C++ through one word stream (floats as bits); every output word is compared
//! bit for bit: the `OverlapType` value, the `bool`, and for each `Collide` all ten fields of the `CastResultStruct`
//! (`StartBad`, `Fraction`, `Normal`, `SurfaceType`, `ComputeContactPoint`, `ContactPoint`) after the call, which the test
//! pre-fills with non-default values so that a field the function leaves alone is compared too. `planes_passed` is
//! compared after the call. Both sides NaN with different payload counts as `NaN-payload-only` (not a mismatch).
//!
//! Inputs are not only generic: a share of every operand is degenerate or on a boundary: zero extents, zero radii,
//! coincident and collinear triangle points, zero-length segments, axis-aligned and axis-parallel segments, planes with an
//! axis-unit normal, boxes touching (integer grid coordinates), the identity and 90 degree bases, and non-orthonormal
//! bases. The OBB bases are mostly rotations, as the game builds them.
//!
//! The elementary functions are the test binary's interposers (tests/support), as in the other differential tests.
//!
//! # Excluded inputs (upstream undefined behaviour)
//!
//! * `Collide(AABox, move, AABox)`: when no axis raised `MaxFrac` above 0 the C++ writes `Normal[-1]` (the float before
//!   `Normal` in `CastResultStruct`, i.e. `Fraction`) with `-Side`, and `Side` is uninitialised. Found by running the C++:
//!   such a call returns true with `Normal` left at (0,0,0) (the pre-fill's normal is never zero, and a valid call sets a
//!   unit normal or leaves the pre-fill). These inputs are counted and not compared (the Rust draft would index out of
//!   range).
//!
//! * `Collide(OBBox|AABox, move, OBBox|AABox, move2)`: when no axis raised `MaxFrac` above 0 (a hit with `Fraction` 0 that
//!   is not the `StartBad` exit), `AxisId` stays `INTERSECTION`, and upstream reads the never-set `Side` (`Normal *=
//!   -Side`) and the never-set `x[3]` of `compute_contact_point`. The draft has `Side` 0 and `x` 0. Found by running the
//!   C++ (the answer's `Normal` differs from the pre-filled one and `Fraction` is 0); counted and not compared.
//!
//!   The same cause with `Fraction` above 0 (the false-collision axes set `AxisId` back to `INTERSECTION` with a `Side`):
//!   the normal is defined, but `compute_contact_point` reads the uninitialised `x[3]`. Those results (recognised by the
//!   draft's contact point being exactly `center0 + Fraction * move0`) are compared except for the three contact-point
//!   words, and counted as `masked`.
//!
//! # Results
//!
//! `cargo +1.97.1 test --release --features "differential untested-drafts" --test differential_draft_colmath -- --nocapture`
//! prints one `DIFF` line per function. The planted bugs (`L9_PLANT=<id>`, ids 10461..) are run one at a time.

mod support;

use rts_lockstep_engine::drafts::wwlib::aabox::{AABoxClass, MinMaxAABoxClass};
use rts_lockstep_engine::drafts::wwlib::obbox as obx;
use rts_lockstep_engine::drafts::wwlib::sphere as sph_mod;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector4::Vector4;
use rts_lockstep_engine::drafts::wwlib::aaplane::{AAPlaneClass, AxisEnum};
use rts_lockstep_engine::drafts::wwlib::colmath::{CastResultStruct, OverlapType};
use rts_lockstep_engine::drafts::wwlib::colmathaabox as cab;
use rts_lockstep_engine::drafts::wwlib::colmathaabtri as catri;
use rts_lockstep_engine::drafts::wwlib::colmathfrustum as cfr;
use rts_lockstep_engine::drafts::wwlib::colmathline as cli;
use rts_lockstep_engine::drafts::wwlib::colmathobbobb as cbb;
use rts_lockstep_engine::drafts::wwlib::colmathobbox as cob;
use rts_lockstep_engine::drafts::wwlib::colmathobbtri as cotri;
use rts_lockstep_engine::drafts::wwlib::colmathplane as cpl;
use rts_lockstep_engine::drafts::wwlib::colmathsphere as csp;
use rts_lockstep_engine::drafts::wwlib::frustum::FrustumClass;
use rts_lockstep_engine::drafts::wwlib::lineseg::LineSegClass;
use rts_lockstep_engine::drafts::wwlib::matrix3::Matrix3x3;
use rts_lockstep_engine::drafts::wwlib::obbox::OBBoxClass;
use rts_lockstep_engine::drafts::wwlib::sphere::SphereClass;
use rts_lockstep_engine::ww::plane::PlaneClass;
use rts_lockstep_engine::ww::tri::TriClass;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

extern "C" {
    fn ref_dcm_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

fn cpp(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 64];
    // SAFETY: both buffers are valid for the lengths given.
    let n = unsafe { ref_dcm_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0 && n as usize <= out.len(), "ref_dcm_run op {} returned {n}", input[0]);
    out.truncate(n as usize);
    out
}

fn cases() -> u64 {
    std::env::var("CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn bits(f: f32) -> u32 {
    f.to_bits()
}

/// Builds one case: the word stream for the C++ and the operands for the Rust.
struct B<'a> {
    g: &'a mut Gen,
    w: Vec<u32>,
}

impl<'a> B<'a> {
    fn new(g: &'a mut Gen, op: u32) -> Self {
        B { g, w: vec![op] }
    }
    fn f(&mut self, x: f32) -> f32 {
        self.w.push(bits(x));
        x
    }
    fn u(&mut self, x: u32) -> u32 {
        self.w.push(x);
        x
    }
    /// A coordinate: grid integers (touching, coincident), halves, uniform, tiny, signed zero.
    fn coord_value(&mut self) -> f32 {
        match self.g.below(20) {
            0..=7 => (self.g.below(9) as i32 - 4) as f32,
            8..=9 => (self.g.below(17) as i32 - 8) as f32 * 0.5,
            10..=16 => self.g.uniform(-5.0, 5.0),
            17 => self.g.uniform(-1.0e-4, 1.0e-4),
            18 => 0.0,
            _ => -0.0,
        }
    }
    /// A coordinate, written to the stream.
    fn coord_f(&mut self) -> f32 {
        let v = self.coord_value();
        self.f(v)
    }
    /// Writes an already-built OBB to the stream.
    fn copy_obb(&mut self, o: &OBBoxClass) {
        for i in 0..3 {
            self.vec_with(o.basis.row[i]);
        }
        self.vec_with(o.center);
        self.vec_with(o.extent);
    }
    fn extent_value(&mut self) -> f32 {
        match self.g.below(20) {
            0..=2 => 0.0,
            3..=9 => self.g.below(5) as f32,
            10..=11 => self.g.below(9) as f32 * 0.5,
            12..=18 => self.g.uniform(0.0, 4.0),
            _ => self.g.uniform(0.0, 1.0e-3),
        }
    }
    fn vec_with(&mut self, v: Vector3) -> Vector3 {
        self.f(v.x);
        self.f(v.y);
        self.f(v.z);
        v
    }
    fn raw_point(&mut self) -> Vector3 {
        Vector3::new(self.coord_value(), self.coord_value(), self.coord_value())
    }
    fn point(&mut self) -> Vector3 {
        let v = self.raw_point();
        self.vec_with(v)
    }
    fn unit_dir(&mut self) -> Vector3 {
        match self.g.below(10) {
            0..=2 => {
                let mut v = Vector3::new(0.0, 0.0, 0.0);
                let a = self.g.below(3) as usize;
                v[a] = if self.g.below(2) == 0 { 1.0 } else { -1.0 };
                v
            }
            _ => {
                let mut v = Vector3::new(self.g.uniform(-1.0, 1.0), self.g.uniform(-1.0, 1.0), self.g.uniform(-1.0, 1.0));
                v.normalize();
                v
            }
        }
    }
    /// A motion vector: zero, axis, random.
    fn move_vec(&mut self) -> Vector3 {
        let v = match self.g.below(10) {
            0 => Vector3::new(0.0, 0.0, 0.0),
            1..=2 => {
                let mut v = Vector3::new(0.0, 0.0, 0.0);
                let a = self.g.below(3) as usize;
                v[a] = self.g.below(9) as f32 - 4.0;
                v
            }
            3..=4 => Vector3::new(self.g.below(9) as f32 - 4.0, self.g.below(9) as f32 - 4.0, self.g.below(9) as f32 - 4.0),
            _ => Vector3::new(self.g.uniform(-6.0, 6.0), self.g.uniform(-6.0, 6.0), self.g.uniform(-6.0, 6.0)),
        };
        self.vec_with(v)
    }
    fn aab(&mut self) -> AABoxClass {
        let c = self.point();
        let e = Vector3::new(self.extent_value(), self.extent_value(), self.extent_value());
        let e = self.vec_with(e);
        AABoxClass::new(c, e)
    }
    fn basis(&mut self) -> Matrix3x3 {
        let mut m = Matrix3x3::new(true);
        match self.g.below(20) {
            0..=5 => {}
            6..=8 => {
                // a signed permutation: 90 degree turns
                let perm = [[0usize, 1, 2], [1, 2, 0], [2, 0, 1], [0, 2, 1], [1, 0, 2], [2, 1, 0]][self.g.below(6) as usize];
                for r in 0..3 {
                    m.row[r] = Vector3::new(0.0, 0.0, 0.0);
                    m.row[r][perm[r]] = if self.g.below(2) == 0 { 1.0 } else { -1.0 };
                }
            }
            9..=10 => {
                // non-orthonormal
                for r in 0..3 {
                    m.row[r] = Vector3::new(self.g.uniform(-1.5, 1.5), self.g.uniform(-1.5, 1.5), self.g.uniform(-1.5, 1.5));
                }
            }
            _ => {
                // an orthonormal basis from two random directions (Gram-Schmidt in f32)
                let a = self.unit_dir();
                let mut b = self.unit_dir();
                let d = Vector3::dot_product(&a, &b);
                b = Vector3::new(b.x - d * a.x, b.y - d * a.y, b.z - d * a.z);
                b.normalize();
                if b.length2() < 0.5 {
                    b = if a.x.abs() < 0.9 { Vector3::new(1.0, 0.0, 0.0) } else { Vector3::new(0.0, 1.0, 0.0) };
                    let d = Vector3::dot_product(&a, &b);
                    b = Vector3::new(b.x - d * a.x, b.y - d * a.y, b.z - d * a.z);
                    b.normalize();
                }
                let c = Vector3::cross_product(&a, &b);
                m.row[0] = a;
                m.row[1] = b;
                m.row[2] = c;
            }
        }
        for r in 0..3 {
            self.vec_with(m.row[r]);
        }
        m
    }
    fn obb(&mut self) -> OBBoxClass {
        let basis = self.basis();
        let c = self.point();
        let e = Vector3::new(self.extent_value(), self.extent_value(), self.extent_value());
        let e = self.vec_with(e);
        OBBoxClass::with_basis(&c, &e, &basis)
    }
    fn sph(&mut self) -> SphereClass {
        let c = self.point();
        let r = if self.g.below(10) == 0 { 0.0 } else { self.g.uniform(0.0, 4.0) };
        let r = self.f(r);
        SphereClass::new(&c, r)
    }
    fn line(&mut self) -> LineSegClass {
        let p0 = self.raw_point();
        let p1 = match self.g.below(10) {
            0 => p0,
            1..=2 => {
                // parallel to an axis
                let mut p = p0;
                let a = self.g.below(3) as usize;
                p[a] = self.coord_value();
                p
            }
            _ => self.raw_point(),
        };
        self.vec_with(p0);
        self.vec_with(p1);
        LineSegClass::new(&p0, &p1)
    }
    fn plane(&mut self) -> PlaneClass {
        let n = match self.g.below(10) {
            0..=3 => {
                let mut v = Vector3::new(0.0, 0.0, 0.0);
                let a = self.g.below(3) as usize;
                v[a] = if self.g.below(2) == 0 { 1.0 } else { -1.0 };
                v
            }
            4..=8 => self.unit_dir(),
            _ => Vector3::new(self.g.uniform(-2.0, 2.0), self.g.uniform(-2.0, 2.0), self.g.uniform(-2.0, 2.0)),
        };
        let d = self.coord_value();
        self.vec_with(n);
        self.f(d);
        PlaneClass { n, d }
    }
    fn aap(&mut self) -> AAPlaneClass {
        let a = self.g.below(3);
        let d = self.coord_value();
        self.u(a);
        self.f(d);
        AAPlaneClass::new([AxisEnum::XNormal, AxisEnum::YNormal, AxisEnum::ZNormal][a as usize], d)
    }
    fn tri(&mut self) -> TriClass {
        let v0 = self.raw_point();
        let (v1, v2) = match self.g.below(10) {
            0 => (v0, self.raw_point()),
            1 => {
                // collinear
                let v1 = self.raw_point();
                let t = (self.g.below(7) as f32 - 2.0) * 0.5;
                (v1, Vector3::new(v0.x + t * (v1.x - v0.x), v0.y + t * (v1.y - v0.y), v0.z + t * (v1.z - v0.z)))
            }
            2 => {
                // in an axis plane
                let mut a = self.raw_point();
                let mut b = self.raw_point();
                let ax = self.g.below(3) as usize;
                a[ax] = v0[ax];
                b[ax] = v0[ax];
                (a, b)
            }
            _ => (self.raw_point(), self.raw_point()),
        };
        let mut n = Vector3::cross_product(&Vector3::subtract_vectors(&v1, &v0), &Vector3::subtract_vectors(&v2, &v0));
        n.normalize();
        self.vec_with(n);
        self.vec_with(v0);
        self.vec_with(v1);
        self.vec_with(v2);
        TriClass { n, v: [v0, v1, v2] }
    }
    fn frustum(&mut self) -> FrustumClass {
        let mut fr = FrustumClass::default();
        for i in 0..6 {
            fr.planes[i] = self.plane();
        }
        fr
    }
    /// A transform: a rotation with a translation, the identity, or arbitrary entries.
    fn tm(&mut self) -> Matrix3D {
        let mut m = Matrix3D::default();
        match self.g.below(20) {
            0..=1 => {
                for i in 0..3 {
                    m.row[i] = Vector4::new(0.0, 0.0, 0.0, 0.0);
                }
                m.row[0].x = 1.0;
                m.row[1].y = 1.0;
                m.row[2].z = 1.0;
                let t = Vector3::new(self.coord_value(), self.coord_value(), self.coord_value());
                m.row[0].w = t.x;
                m.row[1].w = t.y;
                m.row[2].w = t.z;
            }
            2..=7 => {
                for i in 0..3 {
                    m.row[i] = Vector4::new(self.g.uniform(-2.0, 2.0), self.g.uniform(-2.0, 2.0), self.g.uniform(-2.0, 2.0), self.coord_value());
                }
            }
            _ => {
                let mut tmp = B { g: &mut *self.g, w: Vec::new() };
                let b = tmp.basis();
                let t = Vector3::new(tmp.coord_value(), tmp.coord_value(), tmp.coord_value());
                for i in 0..3 {
                    m.row[i] = Vector4::new(b.row[i].x, b.row[i].y, b.row[i].z, t[i]);
                }
            }
        }
        for i in 0..3 {
            self.f(m.row[i].x);
            self.f(m.row[i].y);
            self.f(m.row[i].z);
            self.f(m.row[i].w);
        }
        m
    }
    fn mm(&mut self) -> MinMaxAABoxClass {
        let a = self.raw_point();
        let b = if self.g.below(4) == 0 {
            // a proper box
            Vector3::new(a.x + self.extent_value(), a.y + self.extent_value(), a.z + self.extent_value())
        } else {
            self.raw_point()
        };
        self.vec_with(a);
        self.vec_with(b);
        MinMaxAABoxClass::new(a, b)
    }
    /// A count word (1 to 5) and that many points.
    fn pts(&mut self) -> Vec<Vector3> {
        let n = 1 + self.g.below(5);
        self.u(n);
        (0..n).map(|_| self.point()).collect()
    }
    /// The result pre-fill (never a zero normal, see the module docs).
    fn cr(&mut self) -> CastResultStruct {
        let fraction = match self.g.below(10) {
            0..=6 => 1.0,
            7 => 0.0,
            8 => 0.5,
            _ => self.g.uniform(0.0, 1.0),
        };
        let n = Vector3::new(self.g.uniform(0.1, 1.0), self.g.uniform(-1.0, 1.0), self.g.uniform(-1.0, 1.0));
        let contact = Vector3::new(self.g.uniform(-9.0, 9.0), self.g.uniform(-9.0, 9.0), self.g.uniform(-9.0, 9.0));
        let r = CastResultStruct {
            start_bad: self.g.below(8) == 0,
            fraction,
            normal: n,
            surface_type: self.g.u32(),
            compute_contact_point: self.g.below(2) == 0,
            contact_point: contact,
        };
        self.u(r.start_bad as u32);
        self.f(r.fraction);
        self.vec_with(r.normal);
        self.u(r.surface_type);
        self.u(r.compute_contact_point as u32);
        self.vec_with(r.contact_point);
        r
    }
}

/// The contact point of an OBB-OBB collide when `x[3]` was never set: `AxisId` stayed `INTERSECTION` (the false-collision
/// axes can set `MaxFrac` and `Side` with `TestAxisId == INTERSECTION`), so upstream's `switch` has no case and reads the
/// uninitialised `x[]`. The draft has `x` = 0, whose contact point is `center0 + fraction * move0`; those results (with
/// `ComputeContactPoint` on) have the contact words masked on both sides (see the module docs).
fn unset_x_contact(center0: &Vector3, move0: &Vector3, r: &CastResultStruct) -> bool {
    if !r.compute_contact_point {
        return false;
    }
    let expected = Vector3::add_vectors(center0, &(r.fraction * *move0));
    expected.x.to_bits() == r.contact_point.x.to_bits() && expected.y.to_bits() == r.contact_point.y.to_bits() && expected.z.to_bits() == r.contact_point.z.to_bits()
}

fn ov(o: OverlapType) -> Vec<u32> {
    vec![o.bits() as u32]
}

fn wv(v: &Vector3) -> Vec<u32> {
    vec![bits(v.x), bits(v.y), bits(v.z)]
}
fn waab(a: &AABoxClass) -> Vec<u32> {
    let mut o = wv(&a.center);
    o.extend(wv(&a.extent));
    o
}
fn wmm(a: &MinMaxAABoxClass) -> Vec<u32> {
    let mut o = wv(&a.min_corner);
    o.extend(wv(&a.max_corner));
    o
}
fn wobb(a: &OBBoxClass) -> Vec<u32> {
    let mut o = Vec::new();
    for i in 0..3 {
        o.extend(wv(&a.basis.row[i]));
    }
    o.extend(wv(&a.center));
    o.extend(wv(&a.extent));
    o
}
fn wsph(s: &SphereClass) -> Vec<u32> {
    let mut o = wv(&s.center);
    o.push(bits(s.radius));
    o
}
fn wline(l: &LineSegClass) -> Vec<u32> {
    let mut o = wv(l.get_p0());
    o.extend(wv(l.get_p1()));
    o.extend(wv(l.get_dp()));
    o.extend(wv(l.get_dir()));
    o.push(bits(l.get_length()));
    o
}

fn boolw(b: bool) -> Vec<u32> {
    vec![b as u32]
}

fn crw(hit: bool, r: &CastResultStruct) -> Vec<u32> {
    vec![
        hit as u32,
        r.start_bad as u32,
        bits(r.fraction),
        bits(r.normal.x),
        bits(r.normal.y),
        bits(r.normal.z),
        r.surface_type,
        r.compute_contact_point as u32,
        bits(r.contact_point.x),
        bits(r.contact_point.y),
        bits(r.contact_point.z),
    ]
}

/// One case of `op`: (the words for the C++, the Rust's output words).
fn make(op: u32, g: &mut Gen) -> (Vec<u32>, Vec<u32>, bool) {
    let mut b = B::new(g, op);
    let mut mask_contact = false;
    let out = match op {
        0 => { let a = b.aap(); let p = b.point(); ov(cpl::overlap_test_aaplane_point(&a, &p)) }
        1 => { let a = b.aap(); let l = b.line(); ov(cpl::overlap_test_aaplane_line(&a, &l)) }
        2 => { let a = b.aap(); let t = b.tri(); ov(cpl::overlap_test_aaplane_tri(&a, &t)) }
        3 => { let a = b.aap(); let s = b.sph(); ov(cpl::overlap_test_aaplane_sphere(&a, &s)) }
        4 => { let a = b.aap(); let x = b.aab(); ov(cpl::overlap_test_aaplane_aabox(&a, &x)) }
        5 => { let a = b.aap(); let x = b.obb(); ov(cpl::overlap_test_aaplane_obbox(&a, &x)) }
        6 => { let a = b.plane(); let p = b.point(); ov(cpl::overlap_test_plane_point(&a, &p)) }
        7 => { let a = b.plane(); let l = b.line(); ov(cpl::overlap_test_plane_line(&a, &l)) }
        8 => { let a = b.plane(); let t = b.tri(); ov(cpl::overlap_test_plane_tri(&a, &t)) }
        9 => { let a = b.plane(); let s = b.sph(); ov(cpl::overlap_test_plane_sphere(&a, &s)) }
        11 => { let a = b.plane(); let x = b.aab(); ov(cpl::overlap_test_plane_aabox(&a, &x)) }
        12 => { let a = b.plane(); let x = b.obb(); ov(cpl::overlap_test_plane_obbox(&a, &x)) }
        13 => { let a = b.sph(); let p = b.point(); ov(csp::overlap_test_sphere_point(&a, &p)) }
        14 => { let a = b.sph(); let l = b.line(); ov(csp::overlap_test_sphere_line(&a, &l)) }
        15 => { let a = b.sph(); let t = b.tri(); ov(csp::overlap_test_sphere_tri(&a, &t)) }
        16 => { let a = b.sph(); let s = b.sph(); ov(csp::overlap_test_sphere_sphere(&a, &s)) }
        17 => { let a = b.sph(); let x = b.aab(); ov(csp::overlap_test_sphere_aabox(&a, &x)) }
        18 => { let a = b.sph(); let x = b.obb(); ov(csp::overlap_test_sphere_obbox(&a, &x)) }
        19 => { let a = b.aab(); let p = b.point(); ov(cab::overlap_test_aabox_point(&a, &p)) }
        20 => { let a = b.aab(); let l = b.line(); ov(cab::overlap_test_aabox_line(&a, &l)) }
        21 => { let a = b.aab(); let t = b.tri(); ov(cab::overlap_test_aabox_tri(&a, &t)) }
        22 => { let a = b.aab(); let x = b.aab(); ov(cab::overlap_test_aabox_aabox(&a, &x)) }
        23 => { let a = b.aab(); let x = b.obb(); ov(cob::overlap_test_aabox_obbox(&a, &x)) }
        24 => { let a = b.aab(); let s = b.sph(); ov(cab::overlap_test_aabox_sphere(&a, &s)) }
        25 => { let a = b.obb(); let p = b.point(); ov(cob::overlap_test_obbox_point(&a, &p)) }
        26 => { let a = b.obb(); let l = b.line(); ov(cob::overlap_test_obbox_line(&a, &l)) }
        27 => { let a = b.obb(); let t = b.tri(); ov(cob::overlap_test_obbox_tri(&a, &t)) }
        28 => { let a = b.obb(); let x = b.aab(); ov(cob::overlap_test_obbox_aabox(&a, &x)) }
        29 => { let a = b.obb(); let x = b.obb(); ov(cob::overlap_test_obbox_obbox(&a, &x)) }
        30 => { let f = b.frustum(); let p = b.point(); ov(cfr::overlap_test_frustum_point(&f, &p)) }
        31 => { let f = b.frustum(); let t = b.tri(); ov(cfr::overlap_test_frustum_tri(&f, &t)) }
        32 => { let f = b.frustum(); let s = b.sph(); ov(cfr::overlap_test_frustum_sphere(&f, &s)) }
        33 => { let f = b.frustum(); let x = b.aab(); ov(cfr::overlap_test_frustum_aabox(&f, &x)) }
        34 => { let f = b.frustum(); let x = b.obb(); ov(cfr::overlap_test_frustum_obbox(&f, &x)) }
        35 => {
            let f = b.frustum();
            let x = b.aab();
            let mut pp = b.g.below(64) as i32;
            b.u(pp as u32);
            let r = cfr::overlap_test_frustum_aabox_planes_passed(&f, &x, &mut pp);
            vec![r.bits() as u32, pp as u32]
        }
        36 => {
            let f = b.frustum();
            let x = b.obb();
            let mut pp = b.g.below(64) as i32;
            b.u(pp as u32);
            let r = cfr::overlap_test_frustum_obbox_planes_passed(&f, &x, &mut pp);
            vec![r.bits() as u32, pp as u32]
        }
        37 => {
            let mn = b.point();
            let mx = b.point();
            let l = b.line();
            ov(cab::overlap_test_min_max_line(&mn, &mx, &l))
        }
        40 => { let a = b.aab(); let t = b.tri(); boolw(catri::intersection_test_aabox_tri(&a, &t)) }
        41 => { let a = b.aab(); let x = b.aab(); boolw(cab::intersection_test_aabox_aabox(&a, &x)) }
        42 => { let a = b.aab(); let x = b.obb(); boolw(cbb::intersection_test_aabox_obbox(&a, &x)) }
        43 => { let a = b.obb(); let t = b.tri(); boolw(cotri::intersection_test_obbox_tri(&a, &t)) }
        44 => { let a = b.obb(); let x = b.aab(); boolw(cbb::intersection_test_obbox_aabox(&a, &x)) }
        45 => { let a = b.obb(); let x = b.obb(); boolw(cbb::intersection_test_obbox_obbox(&a, &x)) }
        46 => { let a = b.sph(); let x = b.aab(); boolw(csp::intersection_test_sphere_aabox(&a, &x)) }
        47 => { let a = b.sph(); let x = b.obb(); boolw(csp::intersection_test_sphere_obbox(&a, &x)) }
        60 => { let a = b.line(); let p = b.aap(); let mut r = b.cr(); let k = cli::collide_line_aaplane(&a, &p, &mut r); crw(k, &r) }
        61 => { let a = b.line(); let p = b.plane(); let mut r = b.cr(); let k = cli::collide_line_plane(&a, &p, &mut r); crw(k, &r) }
        62 => { let a = b.line(); let t = b.tri(); let mut r = b.cr(); let k = cli::collide_line_tri(&a, &t, &mut r); crw(k, &r) }
        63 => { let a = b.line(); let s = b.sph(); let mut r = b.cr(); let k = cli::collide_line_sphere(&a, &s, &mut r); crw(k, &r) }
        64 => { let a = b.line(); let x = b.aab(); let mut r = b.cr(); let k = cli::collide_line_aabox(&a, &x, &mut r); crw(k, &r) }
        65 => { let a = b.line(); let x = b.obb(); let mut r = b.cr(); let k = cli::collide_line_obbox(&a, &x, &mut r); crw(k, &r) }
        66 => { let a = b.aab(); let m = b.move_vec(); let p = b.plane(); let mut r = b.cr(); let k = cab::collide_aabox_plane(&a, &m, &p, &mut r); crw(k, &r) }
        67 => { let a = b.aab(); let m = b.move_vec(); let t = b.tri(); let mut r = b.cr(); let k = catri::collide_aabox_tri(&a, &m, &t, &mut r); crw(k, &r) }
        68 => { let a = b.aab(); let m = b.move_vec(); let x = b.aab(); let mut r = b.cr(); let k = cab::collide_aabox_aabox(&a, &m, &x, &mut r); crw(k, &r) }
        69 => { let a = b.aab(); let m = b.move_vec(); let x = b.obb(); let m2 = b.move_vec(); let mut r = b.cr(); let k = cbb::collide_aabox_obbox(&a, &m, &x, &m2, &mut r); mask_contact = k && unset_x_contact(&a.center, &m, &r); crw(k, &r) }
        70 => { let a = b.obb(); let m = b.move_vec(); let p = b.plane(); let mut r = b.cr(); let k = cob::collide_obbox_plane(&a, &m, &p, &mut r); crw(k, &r) }
        71 => { let a = b.obb(); let m = b.move_vec(); let t = b.tri(); let m2 = b.move_vec(); let mut r = b.cr(); let k = cotri::collide_obbox_tri(&a, &m, &t, &m2, &mut r); crw(k, &r) }
        72 => { let a = b.obb(); let m = b.move_vec(); let x = b.aab(); let m2 = b.move_vec(); let mut r = b.cr(); let k = cbb::collide_obbox_aabox(&a, &m, &x, &m2, &mut r); mask_contact = k && unset_x_contact(&a.center, &m, &r); crw(k, &r) }
        73 => { let a = b.obb(); let m = b.move_vec(); let x = b.obb(); let m2 = b.move_vec(); let mut r = b.cr(); let k = cbb::collide_obbox_obbox(&a, &m, &x, &m2, &mut r); mask_contact = k && unset_x_contact(&a.center, &m, &r); crw(k, &r) }
        100 => { let a = b.aab(); let p = b.point(); vec![bits(a.project_to_axis(&p))] }
        101 => { let a = b.aab(); vec![bits(a.volume())] }
        102 => { let a = b.aab(); let p = b.point(); boolw(a.contains_point(&p)) }
        103 => { let a = b.aab(); let x = b.aab(); boolw(a.contains_box(&x)) }
        104 => { let a = b.aab(); let x = b.mm(); boolw(a.contains_min_max_box(&x)) }
        105 => { let mut a = b.aab(); let m = b.tm(); a.transform(&m); waab(&a) }
        106 => { let m = b.tm(); let a = b.aab(); waab(&AABoxClass::transform_into(&m, &a)) }
        107 => { let mut a = b.aab(); let p = b.point(); a.add_point(&p); waab(&a) }
        108 => { let mut a = b.aab(); let x = b.aab(); a.add_box(&x); waab(&a) }
        109 => { let mut a = b.aab(); let x = b.mm(); a.add_min_max_box(&x); waab(&a) }
        110 => { let p = b.pts(); waab(&AABoxClass::from_points(&p)) }
        111 => { let x = b.mm(); waab(&AABoxClass::from_min_max_box(&x)) }
        112 => { let l = b.line(); let mut a = AABoxClass::default(); a.init_line(&l); waab(&a) }
        113 => { let mn = b.point(); let mx = b.point(); let mut a = AABoxClass::default(); a.init_min_max(&mn, &mx); waab(&a) }
        114 => { let mut a = b.aab(); let p = b.point(); a.translate(&p); waab(&a) }
        115 => { let p = b.pts(); wmm(&MinMaxAABoxClass::from_points(&p)) }
        116 => { let a = b.aab(); wmm(&MinMaxAABoxClass::from_aabox(&a)) }
        117 => { let mut x = b.mm(); let p = b.point(); x.add_point(&p); wmm(&x) }
        118 => { let mut x = b.mm(); let y = b.mm(); x.add_min_max_box(&y); wmm(&x) }
        119 => { let mut x = b.mm(); let a = b.aab(); x.add_box(&a); wmm(&x) }
        120 => { let mut x = b.mm(); let mn = b.point(); let mx = b.point(); x.add_corners(&mn, &mx); wmm(&x) }
        121 => { let mut x = b.mm(); let m = b.tm(); x.transform(&m); wmm(&x) }
        122 => { let mut x = b.mm(); let p = b.point(); x.translate(&p); wmm(&x) }
        123 => { let x = b.mm(); vec![bits(x.volume())] }
        124 => { let mut x = b.mm(); x.init_empty(); wmm(&x) }
        130 => { let a = b.obb(); let p = b.point(); vec![bits(a.project_to_axis(&p))] }
        131 => { let a = b.obb(); vec![bits(a.volume())] }
        132 => { let a = b.obb(); let prm = [b.coord_f(), b.coord_f(), b.coord_f()]; wv(&a.compute_point(&prm)) }
        133 => { let a = b.obb(); wv(&a.compute_axis_aligned_extent()) }
        134 => { let m = b.tm(); let a = b.obb(); wobb(&OBBoxClass::transform(&m, &a)) }
        135 => { let a = b.obb(); let x = if b.g.below(3) == 0 { let c = a; b.copy_obb(&c); c } else { b.obb() }; boolw(a.eq_box(&x)) }
        137 => { let a = b.obb(); let x = b.obb(); boolw(obx::oriented_boxes_intersect(&a, &x)) }
        138 => { let a = b.obb(); let v0 = b.move_vec(); let x = b.obb(); let v1 = b.move_vec(); let dt = b.coord_f(); boolw(obx::oriented_boxes_collide(&a, &v0, &x, &v1, dt)) }
        139 => { let a = b.obb(); let t = b.tri(); boolw(obx::oriented_box_intersects_tri(&a, &t)) }
        140 => { let m = b.tm(); let p = b.point(); let r = b.coord_f(); wsph(&SphereClass::from_matrix(&m, &p, r)) }
        141 => { let mut s = b.sph(); let p = b.point(); s.re_center(&p); wsph(&s) }
        142 => { let mut s = b.sph(); let t = b.sph(); s.add_sphere(&t); wsph(&s) }
        143 => { let mut s = b.sph(); let m = b.tm(); s.transform(&m); wsph(&s) }
        144 => { let s = b.sph(); vec![bits(s.volume())] }
        145 => { let p = b.pts(); wsph(&SphereClass::from_points(&p)) }
        146 => { let p = b.point(); let t = b.sph(); wsph(&SphereClass::from_center_and_sphere(&p, &t)) }
        147 => { let mut s = b.sph(); let m = b.tm(); s.mul_assign_matrix(&m); wsph(&s) }
        148 => { let s = b.sph(); let t = b.sph(); boolw(sph_mod::spheres_intersect(&s, &t)) }
        149 => { let s = b.sph(); let t = b.sph(); wsph(&sph_mod::add_spheres(&s, &t)) }
        150 => { let m = b.tm(); let s = b.sph(); wsph(&sph_mod::transform_sphere(&m, &s)) }
        160 => { let l = b.line(); let m = b.tm(); wline(&LineSegClass::from_transformed(&l, &m)) }
        161 => { let l = b.line(); let t = b.coord_f(); wv(&l.compute_point(t)) }
        162 => { let l = b.line(); let p = b.point(); wv(&l.find_point_closest_to(&p)) }
        163 => {
            let l = b.line();
            let m = b.line();
            // the outputs' pre-fill: only written when the lines are not parallel
            let (p1, f1, p2, f2) = (b.point(), b.coord_f(), b.point(), b.coord_f());
            match l.find_intersection(&m) {
                Some((q1, g1, q2, g2)) => { let mut o = vec![1]; o.extend(wv(&q1)); o.push(bits(g1)); o.extend(wv(&q2)); o.push(bits(g2)); o }
                None => { let mut o = vec![0]; o.extend(wv(&p1)); o.push(bits(f1)); o.extend(wv(&p2)); o.push(bits(f2)); o }
            }
        }
        170 => { let a = b.aap(); wv(&a.get_normal()) }
        _ => panic!("unknown op {op}"),
    };
    (b.w, out, mask_contact)
}

/// `true` when the C++ answer to an op-68 case is the `Normal[-1]` write (see the module docs).
fn is_normal_minus_one(cpp_out: &[u32]) -> bool {
    // [hit, start_bad, fraction, nx, ny, nz, ...]
    cpp_out[0] == 1 && cpp_out[3] == 0 && cpp_out[4] == 0 && cpp_out[5] == 0
}

/// `true` when the C++ answer to an OBB-OBB collide is the uninitialised-`Side` / uninitialised `x[3]` case (see the
/// module docs): a hit with `Fraction` 0 that is not the `StartBad` exit (that exit leaves the pre-filled normal alone,
/// the other overwrites it with `A[0] * -Side`, `Side` never set because no axis raised `MaxFrac` above 0).
fn is_no_axis_hit(words: &[u32], c: &[u32]) -> bool {
    let n = words.len();
    let pre_normal = &words[n - 8..n - 5];
    c[0] == 1 && c[2] == 0 && c[3..6] != *pre_normal
}

fn canon(w: u32) -> u32 {
    let f = f32::from_bits(w);
    if f.is_nan() {
        0x7FC0_0000
    } else {
        w
    }
}

/// Run `n` cases of `op`; returns (compared, mismatches, excluded) and prints the DIFF line when `report`.
fn run_op(name: &str, op: u32, seed: u64) -> (u64, u64) {
    let n = cases();
    let mut g = Gen::new(seed ^ (u64::from(op) << 32));
    let mut chk = Check::new(name);
    let mut excluded = 0u64;
    let mut hits = 0u64;
    let mut masked = 0u64;
    let mut missed_cases = 0u64;
    let mut outcomes: std::collections::BTreeMap<u32, u64> = std::collections::BTreeMap::new();
    for i in 0..n {
        let mut g2 = Gen::new(g.u64());
        let (words, mut r, mask) = make(op, &mut g2);
        let c = cpp(&words);
        if (op == 68 && is_normal_minus_one(&c)) || (matches!(op, 69 | 72 | 73) && is_no_axis_hit(&words, &c)) {
            excluded += 1;
            continue;
        }
        let mut c = c;
        if mask {
            masked += 1;
            for k in 8..11 {
                r[k] = 0;
                c[k] = 0;
            }
        }
        if compare(&mut chk, i, &r, &c) {
            missed_cases += 1;
        }
        *outcomes.entry(c[0]).or_insert(0u64) += 1;
        if c[0] != 0 {
            hits += 1;
        }
    }
    println!("    {name}: {n} cases, {excluded} excluded, {masked} with the contact point masked, first word histogram {outcomes:?}");
    if planted() {
        println!("PLANTCASES {name}: {missed_cases} of {n} cases differ");
    }
    let _ = hits;
    let (compared, mism) = (chk.compared, chk.mismatches);
    chk.finish_or_report();
    (compared, mism)
}

fn compare(chk: &mut Check, i: u64, r: &[u32], c: &[u32]) -> bool {
    assert_eq!(r.len(), c.len(), "case {i}: output length rust {r:?} c++ {c:?}");
    let before = chk.mismatches;
    for (k, (&a, &b)) in r.iter().zip(c).enumerate() {
        chk.eq(canon(a), canon(b), || format!("case {i} word {k}: rust {r:x?} c++ {c:x?}"));
    }
    chk.mismatches != before
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

trait Finish {
    fn finish_or_report(self);
}

impl Finish for Check {
    fn finish_or_report(self) {
        if planted() || std::env::var("KEEP_GOING").is_ok() {
            self.report();
        } else {
            self.finish();
        }
    }
}

const OVERLAPS: &[(&str, u32)] = &[
    ("Overlap_Test(AAPlane, Vector3)", 0),
    ("Overlap_Test(AAPlane, LineSeg)", 1),
    ("Overlap_Test(AAPlane, Tri)", 2),
    ("Overlap_Test(AAPlane, Sphere)", 3),
    ("Overlap_Test(AAPlane, AABox)", 4),
    ("Overlap_Test(AAPlane, OBBox)", 5),
    ("Overlap_Test(Plane, Vector3)", 6),
    ("Overlap_Test(Plane, LineSeg)", 7),
    ("Overlap_Test(Plane, Tri)", 8),
    ("Overlap_Test(Plane, Sphere)", 9),
    ("Overlap_Test(Plane, AABox)", 11),
    ("Overlap_Test(Plane, OBBox)", 12),
    ("Overlap_Test(Sphere, Vector3)", 13),
    ("Overlap_Test(Sphere, LineSeg)", 14),
    ("Overlap_Test(Sphere, Tri)", 15),
    ("Overlap_Test(Sphere, Sphere)", 16),
    ("Overlap_Test(Sphere, AABox)", 17),
    ("Overlap_Test(Sphere, OBBox)", 18),
    ("Overlap_Test(AABox, Vector3)", 19),
    ("Overlap_Test(AABox, LineSeg)", 20),
    ("Overlap_Test(AABox, Tri)", 21),
    ("Overlap_Test(AABox, AABox)", 22),
    ("Overlap_Test(AABox, OBBox)", 23),
    ("Overlap_Test(AABox, Sphere)", 24),
    ("Overlap_Test(OBBox, Vector3)", 25),
    ("Overlap_Test(OBBox, LineSeg)", 26),
    ("Overlap_Test(OBBox, Tri)", 27),
    ("Overlap_Test(OBBox, AABox)", 28),
    ("Overlap_Test(OBBox, OBBox)", 29),
    ("Overlap_Test(Frustum, Vector3)", 30),
    ("Overlap_Test(Frustum, Tri)", 31),
    ("Overlap_Test(Frustum, Sphere)", 32),
    ("Overlap_Test(Frustum, AABox)", 33),
    ("Overlap_Test(Frustum, OBBox)", 34),
    ("Overlap_Test(Frustum, AABox, planes_passed)", 35),
    ("Overlap_Test(Frustum, OBBox, planes_passed)", 36),
    ("Overlap_Test(min, max, LineSeg)", 37),
];

const INTERSECTIONS: &[(&str, u32)] = &[
    ("Intersection_Test(AABox, Tri)", 40),
    ("Intersection_Test(AABox, AABox)", 41),
    ("Intersection_Test(AABox, OBBox)", 42),
    ("Intersection_Test(OBBox, Tri)", 43),
    ("Intersection_Test(OBBox, AABox)", 44),
    ("Intersection_Test(OBBox, OBBox)", 45),
    ("Intersection_Test(Sphere, AABox)", 46),
    ("Intersection_Test(Sphere, OBBox)", 47),
];

const COLLIDES: &[(&str, u32)] = &[
    ("Collide(LineSeg, AAPlane)", 60),
    ("Collide(LineSeg, Plane)", 61),
    ("Collide(LineSeg, Tri)", 62),
    ("Collide(LineSeg, Sphere)", 63),
    ("Collide(LineSeg, AABox)", 64),
    ("Collide(LineSeg, OBBox)", 65),
    ("Collide(AABox, move, Plane)", 66),
    ("Collide(AABox, move, Tri)", 67),
    ("Collide(AABox, move, AABox)", 68),
    ("Collide(AABox, move, OBBox, move2)", 69),
    ("Collide(OBBox, move, Plane)", 70),
    ("Collide(OBBox, move, Tri, move2)", 71),
    ("Collide(OBBox, move, AABox, move2)", 72),
    ("Collide(OBBox, move, OBBox, move2)", 73),
];

fn run_group(list: &[(&str, u32)]) {
    let only: Option<u32> = std::env::var("OP").ok().and_then(|v| v.parse().ok());
    for (name, op) in list {
        if only.is_some() && only != Some(*op) {
            continue;
        }
        run_op(name, *op, 0x215_C011);
    }
}

const SHAPES: &[(&str, u32)] = &[
    ("AABoxClass::Project_To_Axis", 100),
    ("AABoxClass::Volume", 101),
    ("AABoxClass::Contains(Vector3)", 102),
    ("AABoxClass::Contains(AABox)", 103),
    ("AABoxClass::Contains(MinMax)", 104),
    ("AABoxClass::Transform(tm)", 105),
    ("AABoxClass::Transform(tm, in, out)", 106),
    ("AABoxClass::Add_Point", 107),
    ("AABoxClass::Add_Box(AABox)", 108),
    ("AABoxClass::Add_Box(MinMax)", 109),
    ("AABoxClass::Init(points, num)", 110),
    ("AABoxClass::Init(MinMax)", 111),
    ("AABoxClass::Init(LineSeg)", 112),
    ("AABoxClass::Init_Min_Max", 113),
    ("AABoxClass::Translate", 114),
    ("MinMaxAABoxClass::Init(points, num)", 115),
    ("MinMaxAABoxClass::Init(AABox)", 116),
    ("MinMaxAABoxClass::Add_Point", 117),
    ("MinMaxAABoxClass::Add_Box(MinMax)", 118),
    ("MinMaxAABoxClass::Add_Box(AABox)", 119),
    ("MinMaxAABoxClass::Add_Box(min, max)", 120),
    ("MinMaxAABoxClass::Transform", 121),
    ("MinMaxAABoxClass::Translate", 122),
    ("MinMaxAABoxClass::Volume", 123),
    ("MinMaxAABoxClass::Init_Empty", 124),
    ("OBBoxClass::Project_To_Axis", 130),
    ("OBBoxClass::Volume", 131),
    ("OBBoxClass::Compute_Point", 132),
    ("OBBoxClass::Compute_Axis_Aligned_Extent", 133),
    ("OBBoxClass::Transform", 134),
    ("OBBoxClass::operator==", 135),
    ("Oriented_Boxes_Intersect", 137),
    ("Oriented_Boxes_Collide", 138),
    ("Oriented_Box_Intersects_Tri", 139),
    ("SphereClass(Matrix3D, center, radius)", 140),
    ("SphereClass::Re_Center", 141),
    ("SphereClass::Add_Sphere", 142),
    ("SphereClass::Transform", 143),
    ("SphereClass::Volume", 144),
    ("SphereClass(points, count)", 145),
    ("SphereClass(center, sphere)", 146),
    ("SphereClass::operator*=", 147),
    ("Spheres_Intersect", 148),
    ("Add_Spheres", 149),
    ("Transform_Sphere", 150),
    ("LineSegClass(that, tm)", 160),
    ("LineSegClass::Compute_Point", 161),
    ("LineSegClass::Find_Point_Closest_To", 162),
    ("LineSegClass::Find_Intersection", 163),
    ("AAPlaneClass::Get_Normal", 170),
];

#[test]
fn shapes_match_upstream() {
    run_group(SHAPES);
}

#[test]
fn overlap_tests_match_upstream() {
    run_group(OVERLAPS);
}

#[test]
fn intersection_tests_match_upstream() {
    run_group(INTERSECTIONS);
}

#[test]
fn collide_functions_match_upstream() {
    run_group(COLLIDES);
}
