//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/metalmap.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/metalmap.h
//!   (VectorProcessorClass::DotProduct / ClampMin / Power from WWMath/vp.cpp, which it calls)
//!
//! `MetalMapManagerClass`: updates the procedural environment maps ("metal maps": 16 x 16 textures shaded with a Blinn
//! model from the global lighting) each frame. Each metal's texture is data here: the pixel bytes of its one mip level
//! (`TextureClass` + `Get_Surface_Level(0)` + `Lock`/`Unlock` are the device's), which the owner uploads after
//! [`MetalMapManagerClass::update_textures`]. The INI file is the [`MetalMapIni`] contract (the port's
//! `ww::ini::IniClass` has `Find_Section` but not yet `Get_Point` / `Get_Float`).
//!
//! **Findings kept as upstream has them.** The constructor clamps `AmbientColor` three times after the three colour reads
//! (the second and third clamps were meant for `DiffuseColor` and `SpecularColor`), so those two are not clamped; the
//! `initialize_metal_params` asserts on them (0..=1) are therefore the only check (`debug_assert` here), and
//! `Shininess > 0.0` is asserted but `Get_Float`'s default is 0.0.

use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `METALMAP_SIZE`: "If this is increased too much the metal map updates will be too expensive".
pub const METALMAP_SIZE: usize = 16;
/// `METALMAP_SIZE_2`.
pub const METALMAP_SIZE_2: usize = METALMAP_SIZE * METALMAP_SIZE;

/// `WW3D_FORMAT_A4R4G4B4` / `WW3D_FORMAT_A8R8G8B8`: the two formats the maps use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetalMapFormat {
    A4R4G4B4,
    A8R8G8B8,
}

/// The INI reads the constructor makes (`INIClass`).
pub trait MetalMapIni {
    /// `Find_Section(section)`.
    fn find_section(&self, section: &str) -> bool;
    /// `Get_Point(section, entry, default)`: a `TPoint3D<float>` as `(X, Y, Z)`.
    fn get_point(&self, section: &str, entry: &str, default: (f32, f32, f32)) -> (f32, f32, f32);
    /// `Get_Float(section, entry, default)`.
    fn get_float(&self, section: &str, entry: &str, default: f32) -> f32;
}

/// `struct MetalMapManagerClass::MetalParams`: a simple Phong reflectance model.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MetalParams {
    pub ambient_color: Vector3,
    pub diffuse_color: Vector3,
    pub specular_color: Vector3,
    pub shininess: f32,
}

/// One metal map's texture: its name (`"!m%02d.tga"`), format, and the pixel bytes of level 0 (`METALMAP_SIZE`
/// rows of `pitch` bytes; the pitch of an unpadded surface).
#[derive(Clone, Debug, PartialEq)]
pub struct MetalMapTexture {
    pub name: String,
    pub format: MetalMapFormat,
    /// `TEXTURE_ADDRESS_CLAMP` on both axes.
    pub clamp: bool,
    pub pitch: usize,
    pub pixels: Vec<u8>,
}

/// `_NormalTable` (the class static): 16 x 16 camera-space normals for the environment maps.
pub fn initialize_normal_table() -> [Vector3; METALMAP_SIZE_2] {
    // Calculate vectors (area outside sphere should be filled with a radial fill of the vectors at the sphere's edge to
    // avoid aliasing artifacts)
    let mut table = [Vector3::new(0.0, 0.0, 0.0); METALMAP_SIZE_2];
    let step = 2.0f32 / METALMAP_SIZE as f32;
    let mut idx = 0;
    for y in 0..METALMAP_SIZE {
        for x in 0..METALMAP_SIZE {
            let normal = &mut table[idx];
            // Set vector to point to surface of unit sphere
            normal.set((step * x as f32) - 1.0, (step * y as f32) - 1.0, 0.0);
            let mut z2 = 1.0 - ((normal.x * normal.x) + (normal.y * normal.y));
            if z2 < 0.0 {
                z2 = 0.0; // MAX(z2, 0.0f): if outside the sphere, treat as if on its edge
            }
            normal.z = petir::Real::sqrt(z2);
            normal.normalize(); // Needed for "outside sphere" case and for safety's sake
            idx += 1;
        }
    }
    table
}

/// `class MetalMapManagerClass`.
#[derive(Clone, Debug)]
pub struct MetalMapManagerClass {
    /// `_NormalTable` (a static upstream, built once; here per manager, same values).
    normal_table: [Vector3; METALMAP_SIZE_2],
    /// `MapCount`.
    pub map_count: i32,
    /// `Textures`.
    pub textures: Vec<MetalMapTexture>,
    /// `MetalParameters`.
    pub metal_parameters: Vec<MetalParams>,
    /// `CurrentAmbient`, `CurrentMainLightColor`, `CurrentMainLightDir`, `CurrentCameraDir`.
    pub current_ambient: Vector3,
    pub current_main_light_color: Vector3,
    pub current_main_light_dir: Vector3,
    pub current_camera_dir: Vector3,
    /// `Use16Bit`.
    pub use_16_bit: bool,
}

impl MetalMapManagerClass {
    /// `MetalMapManagerClass::MetalMapManagerClass(ini)`: counts the `Metal00`, `Metal01`, ... sections, reads each's colours
    /// (0..255 scaled by 1/255) and shininess (clamped 0..127), then creates the textures. `bits` is the device's
    /// `WW3D::Get_Device_Resolution` bit depth (16-bit maps when `bits <= 16`).
    pub fn new<I: MetalMapIni>(ini: &I, bits: i32) -> Self {
        let mut m = MetalMapManagerClass {
            normal_table: initialize_normal_table(),
            map_count: 0,
            textures: Vec::new(),
            metal_parameters: Vec::new(),
            current_ambient: Vector3::new(0.0, 0.0, 0.0),
            current_main_light_color: Vector3::new(0.0, 0.0, 0.0),
            current_main_light_dir: Vector3::new(1.0, 0.0, 0.0),
            current_camera_dir: Vector3::new(1.0, 0.0, 0.0),
            use_16_bit: false,
        };

        // Determine how many metals are in this file
        let mut lp = 0usize;
        loop {
            let section = format!("Metal{:02}", lp);
            if !ini.find_section(&section) {
                break; // NAK - Mar 8, 2000: changed to a break to fix off by one error in lp
            }
            lp += 1;
        }

        if lp > 0 {
            // Create metal params structs and fill from INI:
            let mut metal_params = vec![MetalParams::default(); lp];
            let white_tpoint = (255.0f32, 255.0f32, 255.0f32);
            let white = Vector3::new(1.0, 1.0, 1.0);
            let black = Vector3::new(0.0, 0.0, 0.0);
            for i in 0..lp {
                let section = format!("Metal{:02}", i);
                let cf = 0.003921568627451f32; // 1 / 255
                let p = &mut metal_params[i];

                let color = ini.get_point(&section, "AmbientColor", white_tpoint);
                p.ambient_color.set(color.0 * cf, color.1 * cf, color.2 * cf);
                update_min(&mut p.ambient_color, &white);
                update_max(&mut p.ambient_color, &black);

                let color = ini.get_point(&section, "DiffuseColor", white_tpoint);
                p.diffuse_color.set(color.0 * cf, color.1 * cf, color.2 * cf);
                // upstream clamps AmbientColor again here (see the file header)
                update_min(&mut p.ambient_color, &white);
                update_max(&mut p.ambient_color, &black);

                let color = ini.get_point(&section, "SpecularColor", white_tpoint);
                p.specular_color.set(color.0 * cf, color.1 * cf, color.2 * cf);
                update_min(&mut p.ambient_color, &white);
                update_max(&mut p.ambient_color, &black);

                let shininess = ini.get_float(&section, "Shininess", 0.0);
                p.shininess = wwmath::clamp(shininess, 0.0, 127.0);
            }
            m.initialize_metal_params(lp as i32, &metal_params);
        } else {
            debug_assert!(false); // assert(0)
        }

        // WW3D::Get_Device_Resolution(w, h, bits, windowed)
        m.use_16_bit = bits <= 16;
        let format = if m.use_16_bit { MetalMapFormat::A4R4G4B4 } else { MetalMapFormat::A8R8G8B8 };
        let bpp = if m.use_16_bit { 2 } else { 4 };
        for i in 0..lp {
            // NEW_REF(TextureClass, (METALMAP_SIZE, METALMAP_SIZE, format, MIP_LEVELS_1)) with clamp addressing
            m.textures.push(MetalMapTexture {
                name: format!("!m{:02}.tga", i),
                format,
                clamp: true,
                pitch: METALMAP_SIZE * bpp,
                pixels: vec![0u8; METALMAP_SIZE * METALMAP_SIZE * bpp],
            });
        }
        m
    }

    /// `initialize_metal_params(map_count, metal_params)`: copies the parameters (asserting the colours are in 0..=1 and
    /// the shininess positive).
    fn initialize_metal_params(&mut self, map_count: i32, metal_params: &[MetalParams]) {
        self.map_count = map_count;
        if self.map_count > 0 {
            self.metal_parameters = Vec::with_capacity(map_count as usize);
            for i in 0..map_count as usize {
                let p = metal_params[i];
                for c in [p.ambient_color, p.diffuse_color, p.specular_color] {
                    debug_assert!(c.x >= 0.0 && c.x <= 1.0);
                    debug_assert!(c.y >= 0.0 && c.y <= 1.0);
                    debug_assert!(c.z >= 0.0 && c.z <= 1.0);
                }
                debug_assert!(p.shininess > 0.0);
                self.metal_parameters.push(p);
            }
        }
    }

    /// `Get_Metal_Map(id)`: the texture, `None` (NULL) for an id out of range (`Add_Ref` is not modelled).
    pub fn get_metal_map(&self, id: i32) -> Option<&MetalMapTexture> {
        if id < 0 || id >= self.map_count {
            return None;
        }
        Some(&self.textures[id as usize])
    }

    /// `Metal_Map_Count()`.
    pub fn metal_map_count(&self) -> i32 {
        self.map_count
    }

    /// `Update_Lighting(ambient, main_light_color, main_light_dir, camera_dir)`.
    pub fn update_lighting(&mut self, ambient: &Vector3, main_light_color: &Vector3, main_light_dir: &Vector3, camera_dir: &Vector3) {
        self.current_ambient = *ambient;
        self.current_main_light_color = *main_light_color;
        self.current_main_light_dir = *main_light_dir;
        self.current_camera_dir = *camera_dir;
    }

    /// `Update_Textures()`: "Currently the lighting is done using a simple Phong (actually Blinn) model." Fills each
    /// map's pixel bytes (B, G, R, A for 32-bit; A4R4G4B4 for 16-bit).
    pub fn update_textures(&mut self) {
        let l = self.current_main_light_dir;
        let v = self.current_camera_dir;

        // Calculate halfway vector
        let mut h = l + v;
        h.normalize();

        // Calculate quantities which are the same for all metal maps
        let mut n_dot_l = [0.0f32; METALMAP_SIZE_2];
        let mut n_dot_h = [0.0f32; METALMAP_SIZE_2];
        // VectorProcessorClass::DotProduct, then ClampMin(.., 0.0f)
        for i in 0..METALMAP_SIZE_2 {
            n_dot_l[i] = Vector3::dot_product(&l, &self.normal_table[i]);
        }
        for i in 0..METALMAP_SIZE_2 {
            n_dot_l[i] = if n_dot_l[i] > 0.0 { n_dot_l[i] } else { 0.0 };
        }
        for i in 0..METALMAP_SIZE_2 {
            n_dot_h[i] = Vector3::dot_product(&h, &self.normal_table[i]);
        }
        for i in 0..METALMAP_SIZE_2 {
            n_dot_h[i] = if n_dot_h[i] > 0.0 { n_dot_h[i] } else { 0.0 };
        }

        // Loop over each metal map and update it
        for i in 0..self.map_count as usize {
            let cur_params = self.metal_parameters[i];

            // If shinyness > 1, apply it to specular value array
            let mut temp_specular = [0.0f32; METALMAP_SIZE_2];
            let shinyness = cur_params.shininess;
            let specular: &[f32; METALMAP_SIZE_2] = if shinyness > 1.0 {
                // VectorProcessorClass::Power: powf
                for k in 0..METALMAP_SIZE_2 {
                    temp_specular[k] = petir::Real::powf(n_dot_h[k], shinyness);
                }
                &temp_specular
            } else {
                &n_dot_h
            };

            // Generate metal map row by row
            let c = self.current_main_light_color;
            let specular_color = Vector3::new(cur_params.specular_color.x * c.x, cur_params.specular_color.y * c.y, cur_params.specular_color.z * c.z);
            let diffuse_color = Vector3::new(cur_params.diffuse_color.x * c.x, cur_params.diffuse_color.y * c.y, cur_params.diffuse_color.z * c.z);
            let a = self.current_ambient;
            let ambient_color = Vector3::new(cur_params.ambient_color.x * a.x, cur_params.ambient_color.y * a.y, cur_params.ambient_color.z * a.z);
            let white = Vector3::new(1.0, 1.0, 1.0);

            let use16 = self.use_16_bit;
            let tex = &mut self.textures[i];
            let pitch = tex.pitch;
            let mut idx = 0usize;
            for y in 0..METALMAP_SIZE {
                let map = &mut tex.pixels[y * pitch..(y + 1) * pitch];
                for x in 0..METALMAP_SIZE {
                    let mut result = ambient_color + (diffuse_color * n_dot_l[idx]) + (specular_color * specular[idx]);
                    update_min(&mut result, &white); // Clamp to white
                    let b = wwmath::floor(result.z * 255.99) as u8; // B
                    let g = wwmath::floor(result.y * 255.99) as u8; // G
                    let r = wwmath::floor(result.x * 255.99) as u8; // R
                    let al = 0xFFu8; // A
                    if use16 {
                        let mut tmp: u16 = (u16::from(al) & 0xf0) << 8;
                        tmp |= (u16::from(r) & 0xf0) << 4;
                        tmp |= u16::from(g) & 0xf0;
                        tmp |= (u16::from(b) & 0xf0) >> 4;
                        map[2 * x..2 * x + 2].copy_from_slice(&tmp.to_le_bytes());
                    } else {
                        map[4 * x] = b;
                        map[4 * x + 1] = g;
                        map[4 * x + 2] = r;
                        map[4 * x + 3] = al;
                    }
                    idx += 1;
                }
            }
        }
    }
}

/// `Vector3::Update_Min(a)`: each component the smaller of the two.
fn update_min(v: &mut Vector3, a: &Vector3) {
    if a.x < v.x {
        v.x = a.x;
    }
    if a.y < v.y {
        v.y = a.y;
    }
    if a.z < v.z {
        v.z = a.z;
    }
}

/// `Vector3::Update_Max(a)`: each component the larger of the two.
fn update_max(v: &mut Vector3, a: &Vector3) {
    if a.x > v.x {
        v.x = a.x;
    }
    if a.y > v.y {
        v.y = a.y;
    }
    if a.z > v.z {
        v.z = a.z;
    }
}
