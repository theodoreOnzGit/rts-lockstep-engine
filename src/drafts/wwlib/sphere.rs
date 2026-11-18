//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/sphere.h
//!
//! (The port's `crate::formats::w3d::part_buf::Sphere` is a trimmed copy for the particle buffer.)

use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `class SphereClass`: "Center - center of the sphere. Radius - radius of the sphere".
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SphereClass {
    pub center: Vector3,
    pub radius: f32,
}

impl SphereClass {
    /// `SphereClass(center, radius)`.
    pub fn new(center: &Vector3, radius: f32) -> Self {
        SphereClass { center: *center, radius }
    }

    /// `SphereClass(mtx, center, radius)` -> `Init(mtx, pos, radius)`.
    pub fn from_matrix(mtx: &Matrix3D, center: &Vector3, radius: f32) -> Self {
        SphereClass { center: mtx.mul_vector3(center), radius }
    }

    /// `SphereClass(center, s0)`: "Radius = s0.Radius + dist".
    pub fn from_center_and_sphere(center: &Vector3, s0: &SphereClass) -> Self {
        let dist = (s0.center - *center).length();
        SphereClass { center: *center, radius: s0.radius + dist }
    }

    /// `SphereClass(const Vector3 * Position, const int VertCount)`: the Graphics Gems I (p. 301) bounding sphere.
    /// Upstream reads `Position[0]` unchecked; an empty slice panics here. The intermediates are `double`.
    pub fn from_points(position: &[Vector3]) -> Self {
        let vert_count = position.len();
        let mut xmin = position[0];
        let mut xmax = position[0];
        let mut ymin = position[0];
        let mut ymax = position[0];
        let mut zmin = position[0];
        let mut zmax = position[0];

        // FIRST PASS: finding the 6 minima and maxima points
        for i in 1..vert_count {
            if position[i].x < xmin.x {
                xmin = position[i];
            }
            if position[i].x > xmax.x {
                xmax = position[i];
            }
            if position[i].y < ymin.y {
                ymin = position[i];
            }
            if position[i].y > ymax.y {
                ymax = position[i];
            }
            if position[i].z < zmin.z {
                zmin = position[i];
            }
            if position[i].z > zmax.z {
                zmax = position[i];
            }
        }

        // xspan = distance between the 2 points xmin and xmax squared; same for yspan and zspan.
        let mut dx = (xmax.x - xmin.x) as f64;
        let mut dy = (xmax.y - xmin.y) as f64;
        let mut dz = (xmax.z - xmin.z) as f64;
        let xspan = dx * dx + dy * dy + dz * dz;

        dx = (ymax.x - ymin.x) as f64;
        dy = (ymax.y - ymin.y) as f64;
        dz = (ymax.z - ymin.z) as f64;
        let yspan = dx * dx + dy * dy + dz * dz;

        dx = (zmax.x - zmin.x) as f64;
        dy = (zmax.y - zmin.y) as f64;
        dz = (zmax.z - zmin.z) as f64;
        let zspan = dx * dx + dy * dy + dz * dz;

        // Set points dia1 and dia2 to the maximally separated pair
        let mut dia1 = xmin;
        let mut dia2 = xmax;
        let mut maxspan = xspan;
        if yspan > maxspan {
            maxspan = yspan;
            dia1 = ymin;
            dia2 = ymax;
        }
        if zspan > maxspan {
            // upstream: maxspan = zspan;
            let _ = maxspan;
            dia1 = zmin;
            dia2 = zmax;
        }

        // Compute initial center and radius and radius squared
        let mut center = Vector3::new(0.0, 0.0, 0.0);
        center.x = (dia1.x + dia2.x) / 2.0f32;
        center.y = (dia1.y + dia2.y) / 2.0f32;
        center.z = (dia1.z + dia2.z) / 2.0f32;

        dx = (dia2.x - center.x) as f64;
        dy = (dia2.y - center.y) as f64;
        dz = (dia2.z - center.z) as f64;
        let mut radsqr = dx * dx + dy * dy + dz * dz;
        // upstream: double radius = sqrt(radsqr)  (the C library's double sqrt)
        let mut radius = radsqr.sqrt();

        // SECOND PASS: increment current sphere if any points fall outside of it.
        for i in 0..vert_count {
            dx = (position[i].x - center.x) as f64;
            dy = (position[i].y - center.y) as f64;
            dz = (position[i].z - center.z) as f64;
            let testrad2 = dx * dx + dy * dy + dz * dz;
            if testrad2 > radsqr {
                // this point was outside the old sphere, compute a new center point and radius which contains it
                let testrad = testrad2.sqrt();

                // adjust center and radius
                radius = (radius + testrad) / 2.0;
                radsqr = radius * radius;
                let oldtonew = testrad - radius;

                center.x = ((radius * center.x as f64 + oldtonew * position[i].x as f64) / testrad) as f32;
                center.y = ((radius * center.y as f64 + oldtonew * position[i].y as f64) / testrad) as f32;
                center.z = ((radius * center.z as f64 + oldtonew * position[i].z as f64) / testrad) as f32;
            }
        }

        SphereClass { center, radius: radius as f32 }
    }

    /// `Init(pos, radius)`.
    pub fn init(&mut self, pos: &Vector3, radius: f32) {
        self.center = *pos;
        self.radius = radius;
    }

    /// `Init(mtx, pos, radius)`: `Center = mtx * pos`.
    pub fn init_matrix(&mut self, mtx: &Matrix3D, pos: &Vector3, radius: f32) {
        self.center = mtx.mul_vector3(pos);
        self.radius = radius;
    }

    /// `Re_Center(center)`.
    pub fn re_center(&mut self, center: &Vector3) {
        let dist = (self.center - *center).length();
        self.center = *center;
        self.radius += dist;
    }

    /// `Add_Sphere(s)`.
    pub fn add_sphere(&mut self, s: &SphereClass) {
        if s.radius == 0.0 {
            return;
        }
        let dist = (s.center - self.center).length();
        if dist == 0.0 {
            self.radius = if self.radius > s.radius { self.radius } else { s.radius };
            return;
        }

        let rnew = (dist + self.radius + s.radius) / 2.0f32;

        // If rnew is smaller than either of the two sphere radii, the smaller sphere is inside the larger one.
        if rnew < self.radius {
            // The existing sphere is the result - do nothing.
        } else if rnew < s.radius {
            // The new sphere is the result:
            self.init(&s.center, s.radius);
        } else {
            // Neither sphere is completely inside the other, so rnew is the new radius
            let lerp = (rnew - self.radius) / dist;
            let center = (s.center - self.center) * lerp + self.center;
            self.init(&center, rnew);
        }
    }

    /// `Transform(tm)`: "warning, assumes Orthogonal matrix".
    pub fn transform(&mut self, tm: &Matrix3D) {
        self.center = tm.mul_vector3(&self.center);
    }

    /// `Volume()`: `(4.0 / 3.0) * WWMATH_PI * (Radius * Radius * Radius)` (double arithmetic, returned as float).
    pub fn volume(&self) -> f32 {
        ((4.0f64 / 3.0f64) * wwmath::WWMATH_PI as f64 * (self.radius * self.radius * self.radius) as f64) as f32
    }

    /// `operator *= (const Matrix3D &)`: `Init(m, Center, Radius)`.
    pub fn mul_assign_matrix(&mut self, m: &Matrix3D) {
        let c = self.center;
        let r = self.radius;
        self.init_matrix(m, &c, r);
    }
}

/// `Spheres_Intersect(s0, s1)`.
pub fn spheres_intersect(s0: &SphereClass, s1: &SphereClass) -> bool {
    let delta = s0.center - s1.center;
    let dist2 = Vector3::dot_product(&delta, &delta);
    dist2 < (s0.radius + s1.radius) * (s0.radius + s1.radius)
}

/// `Add_Spheres(s0, s1)` / `operator +`.
pub fn add_spheres(s0: &SphereClass, s1: &SphereClass) -> SphereClass {
    if s0.radius == 0.0 {
        *s1
    } else {
        let mut result = *s0;
        result.add_sphere(s1);
        result
    }
}

/// `Transform_Sphere(m, s)` / `operator * (Matrix3D, SphereClass)`: "Warning, assumes Orthogonal matrix".
pub fn transform_sphere(m: &Matrix3D, s: &SphereClass) -> SphereClass {
    SphereClass::from_matrix(m, &s.center, s.radius)
}
