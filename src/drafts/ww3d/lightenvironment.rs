//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 19,290 environment cases and 19,577 equality pairs (test: tests/differential_draft_light.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/lightenvironment.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/lightenvironment.h
//!
//! Reused from the port: `RGB_To_HSV` / `HSV_To_RGB` are `crate::formats::texture::colorspace`; `LightClass` is
//! the draft [`super::light`].

use super::light::{LightClass, LightType, FAR_ATTENUATION};
use crate::formats::texture::colorspace::{hsv_to_rgb, rgb_to_hsv};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `LightEnvironmentClass::MAX_LIGHTS` (lightenvironment.h).
pub const MAX_LIGHTS: usize = 4;

/// `DIFFUSE_TO_AMBIENT_FRACTION` (lightenvironment.cpp:46).
const DIFFUSE_TO_AMBIENT_FRACTION: f32 = 1.0;

/// `_LightingLODCutoff` and `_LightingLODCutoff2` (lightenvironment.cpp:55). Upstream declares them
/// `static _LightingLODCutoff = 0.5f;`, with no type: implicit `int` (MSVC 6 accepted it). So both are `int`: the
/// initialisers `0.5f` and `0.5f * 0.5f` truncate to 0 and 0, and `Set_Lighting_LOD_Cutoff(float)` truncates its argument
/// and multiplies as ints. ~~The draft held them as float 0.5 and 0.25~~ **Corrected 2026-11-20 (#215, tests/differential_draft_light.rs):**
/// the differential test measured the initial `Get_Lighting_LOD_Cutoff` as 0 against the draft's 0.5. A diffuse light is therefore
/// rejected only when its squared length is not above `cutoff2` (0 until set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightingLodCutoff {
    pub cutoff: i32,
    pub cutoff2: i32,
}

impl Default for LightingLodCutoff {
    fn default() -> Self {
        LightingLodCutoff { cutoff: 0, cutoff2: 0 }
    }
}

impl LightingLodCutoff {
    /// `LightEnvironmentClass::Set_Lighting_LOD_Cutoff` (lightenvironment.cpp:325): float to int (truncation), then the int square.
    pub fn set_lighting_lod_cutoff(&mut self, inten: f32) {
        self.cutoff = if crate::logic::modules::plant(18673) { (inten + 0.5) as i32 } else { inten as i32 };
        self.cutoff2 = self.cutoff.wrapping_mul(self.cutoff);
    }

    /// `LightEnvironmentClass::Get_Lighting_LOD_Cutoff` (lightenvironment.cpp:331): the int, as a float.
    pub fn get_lighting_lod_cutoff(&self) -> f32 {
        self.cutoff as f32
    }
}

/// `LightEnvironmentClass::InputLightStruct` (lightenvironment.h).
#[derive(Clone, Copy, Debug, Default)]
pub struct InputLightStruct {
    pub direction: Vector3,
    pub ambient: Vector3,
    pub diffuse: Vector3,
    pub diffuse_rejected: bool,
    pub m_point: bool,
    pub m_center: Vector3,
    pub m_inner_radius: f32,
    pub m_outer_radius: f32,
    pub m_ambient: Vector3,
    pub m_diffuse: Vector3,
}

impl InputLightStruct {
    /// `InputLightStruct::Init` (lightenvironment.cpp:62): `m_point` false, then by light type.
    pub fn init(&mut self, light: &LightClass, object_center: &Vector3, lod: &LightingLodCutoff) {
        self.m_point = false;
        match light.get_type() {
            LightType::Point | LightType::Spot => self.init_from_point_or_spot_light(light, object_center, lod),
            LightType::Directional => self.init_from_directional_light(light, object_center),
        }
    }

    /// `InputLightStruct::Init_From_Point_Or_Spot_Light` (lightenvironment.cpp:80): direction to the light,
    /// distance attenuation (a step when start and end are within `WWMATH_EPSILON`), spot-cone attenuation, then
    /// the diffuse is rejected into the ambient when its squared length is not above the LOD cutoff squared.
    /// The range is read as doubles (the `double&` overload), as upstream.
    pub fn init_from_point_or_spot_light(&mut self, light: &LightClass, object_center: &Vector3, lod: &LightingLodCutoff) {
        // Compute the direction vector and the distance to the light
        self.direction = light.transform.get_translation() - *object_center;
        let dist = self.direction.length();
        if dist > 0.0 {
            self.direction /= dist;
        }

        // Compute the attenuation factor
        let mut atten: f32 = 1.0;
        let (atten_start, atten_end) = light.get_far_attenuation_range();

        if light.get_flag(FAR_ATTENUATION) {
            if wwmath::fabs((atten_end - atten_start) as f32) < wwmath::WWMATH_EPSILON {
                // Start and end are equal, attenuation is a "step" function
                if f64::from(dist) > atten_start {
                    atten = 0.0;
                }
            } else {
                // Compute the attenuation
                atten = if crate::logic::modules::plant(18661) { 1.0 - (dist - atten_start as f32) / (atten_end as f32 - atten_start as f32) } else { (1.0 - (f64::from(dist) - atten_start) / (atten_end - atten_start)) as f32 };
                atten = wwmath::clamp(atten, 0.0, 1.0);
            }
        }

        if light.get_type() == LightType::Spot {
            let mut spot_dir = Matrix3D::rotate_vector_by(&light.transform, &light.get_spot_direction());
            if crate::logic::modules::plant(18662) {
                spot_dir = -spot_dir;
            }
            let spot_angle_cos = light.get_spot_angle_cos();
            atten *= (Vector3::dot_product(&(-spot_dir), &self.direction) - spot_angle_cos) / (1.0 - spot_angle_cos);
            atten = wwmath::clamp(atten, 0.0, 1.0);
        }

        // Compute the ambient and diffuse values. Rejecting the diffuse component and folding it into the
        // ambient component if it is below the LOD cutoff
        self.ambient = light.get_ambient();
        self.diffuse = light.get_diffuse();
        self.ambient *= light.get_intensity(); // (gth) CNC3 obey the intensity parameter
        self.diffuse *= light.get_intensity();

        self.m_point = light.get_type() == LightType::Point;
        self.m_center = light.transform.get_translation();
        self.m_inner_radius = atten_start as f32;
        self.m_outer_radius = atten_end as f32;
        self.m_ambient = self.ambient;
        self.m_diffuse = self.diffuse;

        if (if crate::logic::modules::plant(18663) { self.diffuse.length2() >= lod.cutoff2 as f32 } else { self.diffuse.length2() > lod.cutoff2 as f32 }) {
            self.diffuse_rejected = false;
            self.ambient *= atten;
            self.diffuse *= atten;
        } else {
            self.diffuse_rejected = true;
            self.ambient *= atten;
            self.ambient += (atten * DIFFUSE_TO_AMBIENT_FRACTION) * self.diffuse;
            self.diffuse.set(0.0, 0.0, 0.0);
        }
    }

    /// `InputLightStruct::Init_From_Directional_Light` (lightenvironment.cpp:158): direction is -Z of the light's
    /// transform; ambient and diffuse as set. (`Direction = -light.Get_Transform().Get_Z_Vector()`.)
    pub fn init_from_directional_light(&mut self, light: &LightClass, _object_center: &Vector3) {
        self.direction = -light.transform.get_z_vector();
        self.diffuse_rejected = false;
        self.ambient = light.get_ambient();
        self.diffuse = light.get_diffuse();
    }

    /// `InputLightStruct::Contribution` (lightenvironment.cpp:171): the squared length of the diffuse.
    pub fn contribution(&self) -> f32 {
        self.diffuse.length2()
    }
}

/// `LightEnvironmentClass::OutputLightStruct` (lightenvironment.h).
#[derive(Clone, Copy, Debug, Default)]
pub struct OutputLightStruct {
    /// direction to the light, in camera space.
    pub direction: Vector3,
    /// diffuse color * attenuation.
    pub diffuse: Vector3,
}

impl OutputLightStruct {
    /// `OutputLightStruct::Init` (lightenvironment.cpp:185): diffuse copied, direction taken into camera space
    /// (`Inverse_Rotate_Vector`); a zero direction is replaced by (1,0,0).
    pub fn init(&mut self, input: &InputLightStruct, camera_tm: &Matrix3D) {
        self.diffuse = input.diffuse;
        self.direction = camera_tm.inverse_rotate_vector(&input.direction);

        // Guard against a direction that is invalid
        if self.direction.length2() == 0.0 {
            if crate::logic::modules::plant(18667) {
                self.direction.y = 1.0;
            } else {
                self.direction.x = 1.0;
            }
        }
    }
}

/// `class LightEnvironmentClass` (lightenvironment.h): the local lighting of a point, as up to
/// [`MAX_LIGHTS`] directional lights plus an ambient.
#[derive(Clone, Debug)]
pub struct LightEnvironmentClass {
    light_count: usize,
    object_center: Vector3,
    input_lights: [InputLightStruct; MAX_LIGHTS],
    output_ambient: Vector3,
    output_lights: [OutputLightStruct; MAX_LIGHTS],
    fill_light: InputLightStruct,
    fill_intensity: f32,
}

impl LightEnvironmentClass {
    /// `LightEnvironmentClass::LightEnvironmentClass` (lightenvironment.cpp:205).
    pub fn new() -> Self {
        LightEnvironmentClass {
            light_count: 0,
            object_center: Vector3::new(0.0, 0.0, 0.0),
            input_lights: [InputLightStruct::default(); MAX_LIGHTS],
            output_ambient: Vector3::new(0.0, 0.0, 0.0),
            output_lights: [OutputLightStruct::default(); MAX_LIGHTS],
            fill_light: InputLightStruct::default(),
            fill_intensity: 0.0,
        }
    }

    /// `LightEnvironmentClass::Reset` (lightenvironment.cpp:221).
    pub fn reset(&mut self, object_center: &Vector3, ambient: &Vector3) {
        self.light_count = 0;
        self.object_center = *object_center;
        self.output_ambient = *ambient;
    }

    /// `LightEnvironmentClass::Add_Light` (lightenvironment.cpp:229): rejects almost-black lights, folds the
    /// ambient in, and inserts a directional or point light into the list sorted by contribution, dropping the
    /// weakest when full.
    pub fn add_light(&mut self, light: &LightClass, lod: &LightingLodCutoff) {
        // Jani: Don't accept lights that are almost black
        let diff = light.get_diffuse();
        if (diff[0] < 0.05 && diff[1] < 0.05 && diff[2] < 0.05) || (crate::logic::modules::plant(18664) && diff[0] <= 0.05 && diff[1] <= 0.05 && diff[2] <= 0.05) {
            return;
        }

        // Compute the equivalent directional + ambient light
        let mut new_light = InputLightStruct::default();
        new_light.init(light, &self.object_center, lod);

        // If we have the fill light set, we also want to the diffuse light to be modified by the intensity of the light source
        if self.fill_intensity != 0.0 {
            new_light.diffuse *= light.get_intensity();
        }

        // Add in the ambient component
        self.output_ambient += new_light.ambient;

        // If not rejected, add the directional component to the active lights
        if !new_light.diffuse_rejected || new_light.m_point {
            // Insert the light into the sorted list of InputLights if it's contribution is greater than any of
            // the current number of lights
            for light_index in 0..self.light_count {
                if (if crate::logic::modules::plant(18665) { new_light.contribution() >= self.input_lights[light_index].contribution() } else { new_light.contribution() > self.input_lights[light_index].contribution() }) {
                    // Move back the lights to make space; the last light might be discarded if it moves off the array
                    let mut i = self.light_count;
                    while i > light_index {
                        if i < MAX_LIGHTS {
                            self.input_lights[i] = self.input_lights[i - 1];
                        }
                        i -= 1;
                    }

                    // Add the new light into the InputLights List where it belongs
                    self.input_lights[light_index] = new_light;

                    // Increment the light count if we have not reach the maximum lights limit yet
                    self.light_count = std::cmp::min(self.light_count + 1, MAX_LIGHTS);

                    // Since we have inserted a new light, we are done for this function
                    return;
                }
            }

            // If the light was not inserted but there are still spots empty, insert at the end of the list
            if self.light_count < MAX_LIGHTS {
                self.input_lights[self.light_count] = new_light;
                self.light_count += 1;
            }
        }
    }

    /// `LightEnvironmentClass::Pre_Render_Update` (lightenvironment.cpp:290): fill light, lights into camera
    /// space, ambient clamped to [0, 1].
    pub fn pre_render_update(&mut self, camera_tm: &Matrix3D) {
        // Calculate and set up the fill light for the object
        self.calculate_fill_light();

        // Transform each light into camera space
        for light_index in 0..self.light_count {
            let input = self.input_lights[light_index];
            self.output_lights[light_index].init(&input, camera_tm);
        }

        // Clamp ambient.
        self.output_ambient.x = wwmath::clamp(self.output_ambient.x, 0.0, 1.0);
        self.output_ambient.y = wwmath::clamp(self.output_ambient.y, 0.0, 1.0);
        if !crate::logic::modules::plant(18668) {
            self.output_ambient.z = wwmath::clamp(self.output_ambient.z, 0.0, 1.0);
        }
    }

    /// `LightEnvironmentClass::Add_Fill_Light` (lightenvironment.cpp:337): a black fill light only adds its
    /// ambient; otherwise it takes the first empty slot, or the last slot when full (displacing the weakest).
    pub fn add_fill_light(&mut self) {
        // Don't add black (or almost black) lights!
        if self.fill_light.diffuse[0] < 0.05 && self.fill_light.diffuse[1] < 0.05 && self.fill_light.diffuse[2] < 0.05 {
            self.output_ambient += self.fill_light.ambient;
            return;
        }

        // Get the 1st empty light slot or the very last slot regardless of whether the last slot is empty
        let slot;
        if self.light_count == MAX_LIGHTS {
            slot = MAX_LIGHTS - 1;
        } else {
            slot = self.light_count;
            self.light_count += 1;
        }

        // Add in the ambient component
        self.output_ambient += self.fill_light.ambient;

        // Insert the fill light into the calculated slot of the InputLights
        self.input_lights[slot] = self.fill_light;
    }

    /// `LightEnvironmentClass::Calculate_Fill_Light` (lightenvironment.cpp:373): averages up to the top
    /// `MAX_LIGHTS - 1` lights weighted by their share of the primary light's contribution, then derives a fill
    /// light of the opposite hue, value scaled by `FillIntensity`, pointing the other way.
    pub fn calculate_fill_light(&mut self) {
        // Early exit if we have no lights at all or if the fill light intensity is zero
        if self.light_count == 0 || self.fill_intensity == 0.0 {
            return;
        }

        // Initialize the averaged light to the primary light source (light with the most contribution)
        let primary_contribution = self.input_lights[0].contribution();
        let mut average_light = self.input_lights[0];

        // Loop through the remaining lights on the list (up to 2) and add their contributions
        let num_lights = std::cmp::min(self.light_count, if crate::logic::modules::plant(18666) { MAX_LIGHTS } else { MAX_LIGHTS - 1 });
        for i in 1..num_lights {
            let ratio = self.input_lights[i].contribution() / primary_contribution;
            average_light.direction += self.input_lights[i].direction * ratio;
            average_light.ambient += self.input_lights[i].ambient * ratio;
            average_light.diffuse += self.input_lights[i].diffuse * ratio;
        }

        // Normalize the averaged light direction
        average_light.direction.normalize();

        // Convert from RGB to HSV to get the reverse hue and with the value modified by the fill intensity
        let mut temp = rgb_to_hsv([average_light.diffuse.x, average_light.diffuse.y, average_light.diffuse.z]);
        temp[0] += 180.0; // Get the opposite hue from the averaged light
        if temp[0] > 360.0 {
            temp[0] -= 360.0;
        }
        temp[2] *= self.fill_intensity; // fraction of the intensity
        let mut rgb = [self.fill_light.diffuse.x, self.fill_light.diffuse.y, self.fill_light.diffuse.z];
        // HSV_To_RGB leaves the colour as it was for a hue outside its sectors (the port returns Err then)
        let _ = hsv_to_rgb(&mut rgb, temp);
        self.fill_light.diffuse = Vector3::new(rgb[0], rgb[1], rgb[2]);

        // Zero out the fill ambient
        self.fill_light.ambient.set(0.0, 0.0, 0.0);

        // now we set the fill light direction to be opposite the average light
        self.fill_light.direction = average_light.direction * -1.0f32;
        self.fill_light.diffuse_rejected = false;

        // Add the fill light into the InputLights list
        self.add_fill_light();
    }

    /// `Set_Fill_Intensity` (lightenvironment.h).
    pub fn set_fill_intensity(&mut self, intensity: f32) {
        self.fill_intensity = intensity;
    }

    /// `Get_Equivalent_Ambient` (lightenvironment.h).
    pub fn get_equivalent_ambient(&self) -> &Vector3 {
        &self.output_ambient
    }

    /// `Set_Output_Ambient` (lightenvironment.h).
    pub fn set_output_ambient(&mut self, oa: &Vector3) {
        self.output_ambient = *oa;
    }

    /// `Get_Light_Count` (lightenvironment.h).
    pub fn get_light_count(&self) -> usize {
        self.light_count
    }

    /// `Get_Light_Direction` (lightenvironment.h): the *input* light's direction (world space).
    pub fn get_light_direction(&self, i: usize) -> &Vector3 {
        &self.input_lights[i].direction
    }

    /// `Get_Light_Diffuse` (lightenvironment.h).
    pub fn get_light_diffuse(&self, i: usize) -> &Vector3 {
        &self.input_lights[i].diffuse
    }

    /// `isPointLight` (lightenvironment.h).
    pub fn is_point_light(&self, i: usize) -> bool {
        self.input_lights[i].m_point
    }

    /// `getPointIrad` (lightenvironment.h).
    pub fn get_point_irad(&self, i: usize) -> f32 {
        self.input_lights[i].m_inner_radius
    }

    /// `getPointOrad` (lightenvironment.h).
    pub fn get_point_orad(&self, i: usize) -> f32 {
        self.input_lights[i].m_outer_radius
    }

    /// `getPointDiffuse` (lightenvironment.h).
    pub fn get_point_diffuse(&self, i: usize) -> &Vector3 {
        &self.input_lights[i].m_diffuse
    }

    /// `getPointAmbient` (lightenvironment.h).
    pub fn get_point_ambient(&self, i: usize) -> &Vector3 {
        &self.input_lights[i].m_ambient
    }

    /// `getPointCenter` (lightenvironment.h).
    pub fn get_point_center(&self, i: usize) -> &Vector3 {
        &self.input_lights[i].m_center
    }

    /// `Get_Max_Lights` (lightenvironment.h).
    pub fn get_max_lights() -> usize {
        MAX_LIGHTS
    }

    /// `LightEnvironmentClass::operator==` (lightenvironment.h): equal light counts, centres and ambients, and
    /// the output lights' diffuse and direction.
    pub fn equals(&self, that: &LightEnvironmentClass) -> bool {
        if self.light_count != that.light_count {
            return false;
        }
        let mut dif = self.object_center != that.object_center;
        dif |= self.output_ambient != that.output_ambient;
        for i in 0..self.light_count {
            dif |= self.output_lights[i].diffuse != that.output_lights[i].diffuse;
            if !crate::logic::modules::plant(18674) {
                dif |= self.output_lights[i].direction != that.output_lights[i].direction;
            }
            if dif {
                return false;
            }
        }
        true
    }
}

impl Default for LightEnvironmentClass {
    fn default() -> Self {
        Self::new()
    }
}

impl LightEnvironmentClass {
    /// The output light `i` (upstream's `OutputLights[i]`, protected): read by the differential test, which compares what
    /// `Pre_Render_Update` produced.
    pub fn output_light(&self, i: usize) -> &OutputLightStruct {
        &self.output_lights[i]
    }
}
