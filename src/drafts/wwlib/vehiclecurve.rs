//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vehiclecurve.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vehiclecurve.cpp
//!
//! `VehicleCurveClass : Curve3DClass`: the path a vehicle takes through a series of points, each corner replaced by a
//! turn arc of the vehicle's turn radius. The base class is the `curve.rs` draft's [`Curve3DClass`], held as `curve`.
//! `PersistFactoryClass` (`_VehicleCurveFactory`, `WWMATH_CHUNKID_VEHICLECURVE`) is [`VehicleCurveClass::get_factory_id`].
//! Upstream notes kept: `Initialize_Arc(radius)` is declared in vehiclecurve.h and defined nowhere (not translated);
//! `Evaluate` reads `Keys[0]` / `Keys[count-1]` unchecked (an empty curve is an out-of-range read upstream, a panic
//! here); `DEG_TO_RADF(x)` is `(float)x * WWMATH_PI / 180.0f`; `Load` appends to `m_ArcList` without clearing it;
//! the arc records are saved as the raw 52-byte struct (3 vectors then 4 floats, no padding).

use super::chunk_save::ChunkSaveClass;
use super::curve::Curve3DClass;
use super::saveload_ids::WWMATH_CHUNKID_VEHICLECURVE;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// Save/Load constants (vehiclecurve.cpp:45): `CHUNKID_PARENT = 0x11071217`, `CHUNKID_ARC_INFO`, `CHUNKID_VARIABLES`;
/// `VARID_IS_DIRTY = 1`, `VARID_RADIUS`.
const CHUNKID_PARENT: u32 = 0x11071217;
const CHUNKID_ARC_INFO: u32 = 0x11071218;
const CHUNKID_VARIABLES: u32 = 0x11071219;
const VARID_IS_DIRTY: u32 = 1;
const VARID_RADIUS: u32 = 2;

/// `DEG_TO_RADF(x)` (wwmath.h:87): `((float)x * WWMATH_PI / 180.0f)`.
fn deg_to_radf(x: f32) -> f32 {
    x * wwmath::WWMATH_PI / 180.0f32
}

/// `struct _ArcInfoStruct` / `ArcInfoStruct`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArcInfoStruct {
    pub center: Vector3,
    pub point_in: Vector3,
    pub point_out: Vector3,
    pub point_angle: f32,
    pub radius: f32,
    pub angle_in_delta: f32,
    pub angle_out_delta: f32,
}

impl ArcInfoStruct {
    /// The raw struct image (`csave.Write(&arc_info, sizeof(arc_info))`): 13 little-endian floats.
    pub fn to_bytes(&self) -> [u8; 52] {
        let f = [
            self.center.x, self.center.y, self.center.z, self.point_in.x, self.point_in.y, self.point_in.z, self.point_out.x, self.point_out.y, self.point_out.z, self.point_angle, self.radius, self.angle_in_delta,
            self.angle_out_delta,
        ];
        let mut b = [0u8; 52];
        for (i, v) in f.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }

    /// Inverse of [`ArcInfoStruct::to_bytes`] (`cload.Read(&arc_info, sizeof(arc_info))`).
    pub fn from_bytes(b: &[u8; 52]) -> Self {
        let f = |i: usize| f32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        ArcInfoStruct {
            center: Vector3::new(f(0), f(1), f(2)),
            point_in: Vector3::new(f(3), f(4), f(5)),
            point_out: Vector3::new(f(6), f(7), f(8)),
            point_angle: f(9),
            radius: f(10),
            angle_in_delta: f(11),
            angle_out_delta: f(12),
        }
    }
}

/// `Find_Tangent(center, radius, point, clockwise, &result)` (vehiclecurve.cpp:70): the angle of the tangent point on
/// the circle seen from `point`, or `None` when the point is inside the circle.
fn find_tangent(center: &Vector3, radius: f32, point: &Vector3, clockwise: bool) -> Option<f32> {
    // Calculate the distance from the point to the center of the circle (`::sqrt`, the double version)
    let delta_x = point.x - center.x;
    let delta_y = point.y - center.y;
    let dist = ((delta_x * delta_x + delta_y * delta_y) as f64).sqrt() as f32;

    if dist >= radius {
        // Determine the offset angle (from the line between the point and center) where the 2 tangent points lie.
        let angle_offset = wwmath::acos(radius / dist);
        let mut base_angle = wwmath::atan2(delta_x, -delta_y);
        base_angle = wwmath::wrap(base_angle, 0.0, deg_to_radf(360.0));

        // Determine which tangent angle we would come across first, depending on our orientation
        let mut angle = if clockwise { base_angle - angle_offset } else { base_angle + angle_offset };
        angle = wwmath::wrap(angle, 0.0, deg_to_radf(360.0));
        return Some(angle);
    }
    None
}

/// `Get_Angle_Delta(angle1, angle2, clockwise)`: the delta wrapped differently depending on the orientation.
fn get_angle_delta(angle1: f32, angle2: f32, clockwise: bool) -> f32 {
    let mut result = angle1 - angle2;
    if clockwise {
        if angle1 < angle2 {
            result = angle1 - (angle2 - deg_to_radf(360.0));
        }
    } else if angle1 > angle2 {
        result = (angle1 - deg_to_radf(360.0)) - angle2;
    }
    result
}

/// `Find_Turn_Arc(transform, radius, prev_pt, curr_pt, next_pt, &arc_center, &is_right_turn)`: the arc centre lies on
/// the circle of `radius` about the current point, halfway (the shorter way) between the directions to the previous and
/// next points.
fn find_turn_arc(transform: &Matrix3D, radius: f32, prev_pt: &Vector3, curr_pt: &Vector3, next_pt: &Vector3) -> (Vector3, bool) {
    let mut angle1 = wwmath::atan2(prev_pt.y - curr_pt.y, prev_pt.x - curr_pt.x);
    angle1 = wwmath::wrap(angle1, 0.0, deg_to_radf(360.0));

    let mut angle2 = wwmath::atan2(next_pt.y - curr_pt.y, next_pt.x - curr_pt.x);
    angle2 = wwmath::wrap(angle2, 0.0, deg_to_radf(360.0));

    // (upstream first sets `avg_angle = (angle1 + angle2) * 0.5F`, then overwrites it below)

    // Find the shortest delta between the two angles (either clockwise or counterclockwise).
    let delta1 = wwmath::fabs(get_angle_delta(angle1, angle2, true));
    let delta2 = wwmath::fabs(get_angle_delta(angle1, angle2, false));
    let avg_angle = if delta1 < delta2 { angle1 - (delta1 * 0.5f32) } else { angle1 + (delta2 * 0.5f32) };

    // Find the point on the circle at this angle
    let arc_center = Vector3::new(curr_pt.x + (radius * wwmath::cos(avg_angle)), curr_pt.y + (radius * wwmath::sin(avg_angle)), curr_pt.z);

    // Will we be making a right turn or a left turn?
    let rel_center = Matrix3D::inverse_transform_vector(transform, &arc_center);
    (arc_center, rel_center.y > 0.0)
}

/// `Find_Tangents(radius, prev_pt, curr_pt, next_pt, arc_center, is_right_turn, &point_angle, &angle_in_delta,
/// &angle_out_delta)`.
fn find_tangents(radius: f32, prev_pt: &Vector3, curr_pt: &Vector3, next_pt: &Vector3, arc_center: &Vector3, is_right_turn: bool) -> (f32, f32, f32) {
    // Find the 'in' and 'out' tangent angles
    let angle_in = find_tangent(arc_center, radius, prev_pt, is_right_turn);
    let angle_out = find_tangent(arc_center, radius, next_pt, !is_right_turn);

    // Find the angle where the current position lies on the turn arc
    let mut point_angle = wwmath::atan2(curr_pt.x - arc_center.x, -(curr_pt.y - arc_center.y));
    point_angle = wwmath::wrap(point_angle, 0.0, deg_to_radf(360.0));

    // If the tangent-in is valid, find its delta from the 'point angle.
    let angle_in_delta = match angle_in {
        Some(a) => get_angle_delta(a, point_angle, is_right_turn),
        None => 0.0,
    };

    // If the tangent-out is valid, find its delta from the 'point angle.
    let angle_out_delta = match angle_out {
        Some(a) => get_angle_delta(a, point_angle, !is_right_turn),
        None => 0.0,
    };

    (point_angle, angle_in_delta, angle_out_delta)
}

/// `class VehicleCurveClass : public Curve3DClass`.
#[derive(Clone, Default)]
pub struct VehicleCurveClass {
    /// The `Curve3DClass` base.
    pub curve: Curve3DClass,
    is_dirty: bool,
    radius: f32,
    arc_list: Vec<ArcInfoStruct>,
    last_time: f32,
    sharpness: f32,
    sharpness_pos: Vector3,
}

impl VehicleCurveClass {
    /// `VehicleCurveClass()`: `m_IsDirty(true)`, radius 0.
    pub fn new() -> Self {
        VehicleCurveClass { curve: Curve3DClass::new(), is_dirty: true, radius: 0.0, arc_list: Vec::new(), last_time: 0.0, sharpness: 0.0, sharpness_pos: Vector3::new(0.0, 0.0, 0.0) }
    }

    /// `VehicleCurveClass(radius)`.
    pub fn with_radius(radius: f32) -> Self {
        let mut v = VehicleCurveClass::new();
        v.radius = radius;
        v
    }

    /// `Set_Key(i, point)`: marks the arcs dirty, then the base.
    pub fn set_key(&mut self, i: i32, point: &Vector3) {
        self.is_dirty = true;
        self.curve.set_key(i, point);
    }

    /// `Add_Key(point, t)`.
    pub fn add_key(&mut self, point: &Vector3, t: f32) -> i32 {
        self.is_dirty = true;
        self.curve.add_key(point, t)
    }

    /// `Remove_Key(i)`.
    pub fn remove_key(&mut self, i: i32) {
        self.is_dirty = true;
        self.curve.remove_key(i);
    }

    /// `Clear_Keys()`.
    pub fn clear_keys(&mut self) {
        self.is_dirty = true;
        self.curve.clear_keys();
    }

    /// `Get_Current_Sharpness(&position)`: the sharpness and its position.
    pub fn get_current_sharpness(&self) -> (f32, Vector3) {
        (self.sharpness, self.sharpness_pos)
    }

    /// `Get_Last_Eval_Time()`.
    pub fn get_last_eval_time(&self) -> f32 {
        self.last_time
    }

    /// `Get_Factory()`: the persist factory's chunk id (`WWMATH_CHUNKID_VEHICLECURVE`).
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_VEHICLECURVE
    }

    /// `VehicleCurveClass::Update_Arc_List` (vehiclecurve.cpp:245): one arc record per key; the first and last are
    /// degenerate (radius 0), each interior key gets a turn arc (or a degenerate record when a tangent delta exceeds
    /// 200 degrees: "Sanity check to ensure the vehicle doesn't try to go the long way around the turn arc").
    fn update_arc_list(&mut self) {
        self.arc_list.clear();

        // Bail out if there is nothing to do
        let count = self.curve.key_count();
        if count == 0 {
            return;
        }

        // Add a record for the starting point of the arc...
        let k0 = self.curve.keys[0].point;
        self.arc_list.push(ArcInfoStruct { point_in: k0, point_out: k0, center: k0, point_angle: 0.0, radius: 0.0, angle_in_delta: 0.0, angle_out_delta: 0.0 });

        // Loop over each 'interior' point and generate arc information for each.
        let mut index = 1;
        while index < count - 1 {
            // Get information about the previous, next, and current points.
            let (prev_pt, _) = self.curve.get_key(index - 1);
            let (curr_pt, _) = self.curve.get_key(index);
            let (next_pt, _) = self.curve.get_key(index + 1);

            // Determine the last known point on the path
            let last_path_pt = self.arc_list[(index - 1) as usize].point_out;

            // Create a transformation matrix to simulate the vehicle's position and orientation at the last point...
            let mut x_vector = curr_pt - last_path_pt;
            let z_vector = Vector3::new(0.0, 0.0, 1.0);
            x_vector.normalize();
            let y_vector = Vector3::cross_product(&x_vector, &z_vector);
            let tm = Matrix3D::from_axes(&x_vector, &y_vector, &z_vector, &last_path_pt);

            // Find where the turn arc should be centered and whether we should make a right-turn or a left turn...
            let (arc_center, is_right_turn) = find_turn_arc(&tm, self.radius, &last_path_pt, &curr_pt, &next_pt);

            // Determine where the vehicle should enter and exit the turn
            let (point_angle, angle_in_delta, angle_out_delta) = find_tangents(self.radius, &last_path_pt, &curr_pt, &next_pt, &arc_center, is_right_turn);

            // Determine at what points these angles intersect the arc
            let mut point_in = Vector3::new(0.0, 0.0, 0.0);
            point_in.x = arc_center.x + (self.radius * wwmath::sin(point_angle + angle_in_delta));
            point_in.y = arc_center.y + (self.radius * -wwmath::cos(point_angle + angle_in_delta));

            let mut point_out = Vector3::new(0.0, 0.0, 0.0);
            point_out.x = arc_center.x + (self.radius * wwmath::sin(point_angle + angle_out_delta));
            point_out.y = arc_center.y + (self.radius * -wwmath::cos(point_angle + angle_out_delta));

            // Sanity check to ensure the vehicle doesn't try to go the long way around the turn arc...
            if angle_in_delta > deg_to_radf(200.0) || angle_out_delta > deg_to_radf(200.0) || angle_in_delta < -deg_to_radf(200.0) || angle_out_delta < -deg_to_radf(200.0) {
                // Record information about this arc
                self.arc_list.push(ArcInfoStruct { center: curr_pt, point_angle: 0.0, point_in: curr_pt, point_out: curr_pt, radius: 0.0, angle_in_delta: 0.0, angle_out_delta: 0.0 });
            } else {
                self.arc_list.push(ArcInfoStruct {
                    center: arc_center,
                    point_angle,
                    point_in,
                    point_out,
                    radius: self.radius,
                    angle_in_delta,
                    angle_out_delta,
                });
            }
            index += 1;
        }

        // Add a record for the ending point of the arc...
        if count > 1 {
            let kl = self.curve.keys[(count - 1) as usize].point;
            self.arc_list.push(ArcInfoStruct { point_in: kl, point_out: kl, center: kl, point_angle: 0.0, radius: 0.0, angle_in_delta: 0.0, angle_out_delta: 0.0 });
        }

        self.is_dirty = false;
    }

    /// `VehicleCurveClass::Evaluate(time, &set_val)` (vehiclecurve.cpp:378). `set_val` is read-modify-write as upstream
    /// writes `X`, `Y` and `Z` separately (a fresh value is returned from the same fields).
    pub fn evaluate(&mut self, time: f32) -> Vector3 {
        let mut set_val = Vector3::new(0.0, 0.0, 0.0);
        let count = self.curve.keys.len();
        self.sharpness = 0.0;

        if time < self.curve.keys[0].time {
            set_val = self.curve.keys[0].point;
            self.last_time = self.curve.keys[0].time;
            return set_val;
        }

        if time >= self.curve.keys[count - 1].time {
            set_val = self.curve.keys[count - 1].point;
            self.last_time = self.curve.keys[count - 1].time;
            return set_val;
        }

        // Update the arc information if any of the keys have changed...
        if self.is_dirty {
            self.update_arc_list();
        }

        // Determine which segment we are on
        let (index0, index1, seg_time) = self.curve.find_interval(time);
        let arc_info0 = self.arc_list[index0 as usize];
        let arc_info1 = self.arc_list[index1 as usize];
        let (i0, i1) = (index0 as usize, index1 as usize);

        // Determine the lengths of each segment of this curve: exit curve from prev point, straight line from exit of
        // last curve to enter of this curve, enter curve for the current point
        let arc_length0 = arc_info0.radius * wwmath::fabs(arc_info0.angle_out_delta);
        let arc_length1 = arc_info1.radius * wwmath::fabs(arc_info1.angle_in_delta);
        let other_length = (arc_info1.point_in - arc_info0.point_out).length() / 2.0;
        let total_length = arc_length0 + arc_length1 + other_length;

        // Determine at what times we should switch between parts of the segment
        let time1 = arc_length0 / total_length;
        let time2 = (arc_length0 + other_length) / total_length;

        // Determine which part of the segment we are on
        if seg_time < time1 {
            // We are on the initial curve of the segment (the interpolation along the arc is commented out upstream:
            // the point is the arc's exit)
            let angle = arc_info0.point_angle + arc_info0.angle_out_delta;
            set_val.x = arc_info0.center.x + (arc_info0.radius * wwmath::sin(angle));
            set_val.y = arc_info0.center.y + (arc_info0.radius * -wwmath::cos(angle));

            self.sharpness = wwmath::clamp(wwmath::fabs(arc_info0.angle_out_delta) / deg_to_radf(15.0), 0.0, 1.0f32);
            self.sharpness_pos.x = set_val.x;
            self.sharpness_pos.y = set_val.y;
            self.sharpness_pos.z = self.curve.keys[i0].point.z + (self.curve.keys[i1].point.z - self.curve.keys[i0].point.z) * seg_time;

            self.last_time = self.curve.keys[i0].time + (self.curve.keys[i1].time - self.curve.keys[i0].time) * time1;
        } else if seg_time < time2 {
            // We are on the line between the two curves (the linear interpolation is commented out upstream)
            let percent = (seg_time - time1) / (time2 - time1);
            if percent == 0.0 {
                set_val.x = arc_info0.point_out.x;
                set_val.y = arc_info0.point_out.y;
            } else {
                set_val.x = arc_info1.point_in.x;
                set_val.y = arc_info1.point_in.y;
            }

            self.sharpness = wwmath::clamp(wwmath::fabs(arc_info1.angle_out_delta) / deg_to_radf(15.0), 0.0, 1.0f32);
            self.sharpness_pos = arc_info1.point_in;

            self.last_time = self.curve.keys[i0].time + (self.curve.keys[i1].time - self.curve.keys[i0].time) * time2;
        } else {
            // We are on the ending curve of the segment (the interpolation is commented out upstream)
            let angle = arc_info1.point_angle + arc_info1.angle_out_delta;
            set_val.x = arc_info1.center.x + (arc_info1.radius * wwmath::sin(angle));
            set_val.y = arc_info1.center.y + (arc_info1.radius * -wwmath::cos(angle));

            self.sharpness = wwmath::clamp(wwmath::fabs(arc_info1.angle_out_delta) / deg_to_radf(15.0), 0.0, 1.0f32);
            self.sharpness_pos.x = set_val.x;
            self.sharpness_pos.y = set_val.y;
            self.sharpness_pos.z = self.curve.keys[i0].point.z + (self.curve.keys[i1].point.z - self.curve.keys[i0].point.z) * seg_time;

            self.last_time = self.curve.keys[i1].time;
        }

        // Our Z value is just a linear interpolation
        set_val.z = self.curve.keys[i0].point.z + (self.curve.keys[i1].point.z - self.curve.keys[i0].point.z) * seg_time;
        set_val
    }

    /// `VehicleCurveClass::Save(csave)`: the parent chunk, the variables (micro chunks `m_IsDirty` as a 1-byte bool and
    /// `m_Radius`), then one `CHUNKID_ARC_INFO` chunk per arc.
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.begin_chunk(CHUNKID_PARENT);
        self.curve.save(csave);
        csave.end_chunk();

        csave.begin_chunk(CHUNKID_VARIABLES);
        // Save each variable to its own microchunk
        csave.write_micro_chunk(VARID_IS_DIRTY, &[self.is_dirty as u8]);
        csave.write_micro_chunk(VARID_RADIUS, &self.radius.to_le_bytes());
        csave.end_chunk();

        // Save each arc info struct to its own chunk
        for arc_info in self.arc_list.iter() {
            csave.begin_chunk(CHUNKID_ARC_INFO);
            csave.write(&arc_info.to_bytes());
            csave.end_chunk();
        }
        true
    }

    /// `VehicleCurveClass::Load(cload)`.
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                CHUNKID_PARENT => {
                    self.curve.load(cload);
                }
                CHUNKID_ARC_INFO => {
                    let mut b = [0u8; 52];
                    let _ = cload.read_into(&mut b);
                    self.arc_list.push(ArcInfoStruct::from_bytes(&b));
                }
                CHUNKID_VARIABLES => self.load_variables(cload),
                _ => {}
            }
            let _ = cload.close_chunk();
        }
        true
    }

    /// `VehicleCurveClass::Load_Variables(cload)`: "Loop through all the microchunks that define the variables".
    fn load_variables(&mut self, cload: &mut ChunkLoad) {
        while cload.open_micro_chunk().unwrap_or(false) {
            match cload.cur_micro_chunk_id() {
                VARID_IS_DIRTY => {
                    let mut b = [0u8; 1];
                    let _ = cload.read_into(&mut b);
                    self.is_dirty = b[0] != 0;
                }
                VARID_RADIUS => {
                    let mut b = [0u8; 4];
                    let _ = cload.read_into(&mut b);
                    self.radius = f32::from_le_bytes(b);
                }
                _ => {}
            }
            let _ = cload.close_micro_chunk();
        }
    }
}
