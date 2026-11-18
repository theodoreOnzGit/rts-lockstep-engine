//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/dazzle.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/dazzle.h
//!
//! The dazzle (sun / light flare) system functions no earlier pass translated: the INI vector readers, the lensflare
//! vertex generation, the intensity maths, the type tables, the visibility lists and the two render paths. Drawing is
//! client-side, so `f32` methods are used as in the client's other drawing code (no `petir` here). The DX8 pieces
//! (`DynamicVBAccessClass`, `Set_Shader`, `Set_Texture`, `Draw_Triangles`, transforms) are [`DazzleHost`]; the
//! vertex and index buffers it would fill are built here and handed to the host.
//!
//! Not on the list because already translated or not a definition: the constructors, `Render`, `Init_Shaders`,
//! `Deinit`, `Set_Transform`, `Get_Type_ID` (names appear elsewhere in the port).

/// `VertexFormatXYZNDUV2` as the dazzle code fills it: position, first texture coordinate, diffuse.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexXyzNduv2 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub u1: f32,
    pub v1: f32,
    pub diffuse: u32,
}

/// The `DazzleInitClass` / `LensflareInitClass` fields used here (`ic` / `lic`).
#[derive(Clone, Debug, Default)]
pub struct DazzleInit {
    pub dazzle_area: f32,
    pub dazzle_direction_area: f32,
    pub dazzle_size_pow: f32,
    pub dazzle_intensity_pow: f32,
    pub dazzle_intensity: f32,
    pub halo_intensity_pow: f32,
    pub halo_intensity: f32,
    pub use_camera_translation: bool,
    pub fadeout_start: f32,
    pub fadeout_end: f32,
    pub halo_scale_x: f32,
    pub halo_scale_y: f32,
    pub dazzle_scale_x: f32,
    pub dazzle_scale_y: f32,
    pub dazzle_color: [f32; 3],
    pub halo_color: [f32; 3],
    pub primary_texture_name: String,
    pub secondary_texture_name: String,
}

/// `LensflareInitClass`.
#[derive(Clone, Debug, Default)]
pub struct LensflareInit {
    pub flare_count: usize,
    pub flare_locations: Vec<f32>,
    pub flare_sizes: Vec<f32>,
    pub flare_colors: Vec<[f32; 3]>,
    pub flare_uv: Vec<[f32; 4]>,
    pub texture_name: String,
}

/// `WWMATH_EPSILON`.
const WWMATH_EPSILON: f32 = 0.0001;

/// `DazzleINIClass::Get_Vector2` (dazzle.cpp:302): `sscanf("%f,%f")` must give exactly 2, otherwise the default.
/// `value` is the entry's text (`None` when the section / entry is missing or has no value).
pub fn get_vector2(value: Option<&str>, defvalue: [f32; 2]) -> [f32; 2] {
    match scan_floats(value, 2) {
        Some(v) => [v[0], v[1]],
        None => defvalue,
    }
}

/// `DazzleINIClass::Get_Vector3` (dazzle.cpp:316): `sscanf("%f,%f,%f") == 3`.
pub fn get_vector3(value: Option<&str>, defvalue: [f32; 3]) -> [f32; 3] {
    match scan_floats(value, 3) {
        Some(v) => [v[0], v[1], v[2]],
        None => defvalue,
    }
}

/// `DazzleINIClass::Get_Vector4` (dazzle.cpp:330): `sscanf("%f,%f,%f,%f") == 4`.
pub fn get_vector4(value: Option<&str>, defvalue: [f32; 4]) -> [f32; 4] {
    match scan_floats(value, 4) {
        Some(v) => [v[0], v[1], v[2], v[3]],
        None => defvalue,
    }
}

/// `sscanf` of `n` comma-separated floats: succeeds only when all `n` convert (upstream compares the count to `n`).
fn scan_floats(value: Option<&str>, n: usize) -> Option<Vec<f32>> {
    let text = value?;
    let mut out = Vec::with_capacity(n);
    let mut parts = text.split(',');
    for _ in 0..n {
        let p = parts.next()?;
        out.push(p.trim().parse::<f32>().ok()?);
    }
    Some(out)
}

/// `DX8Wrapper::Convert_Color(Vector3, alpha)`: the host's packed colour.
pub type ConvertColor = fn([f32; 3], f32) -> u32;

/// `LensflareTypeClass::Generate_Vertex_Buffers` (dazzle.cpp:367): four vertices per flare on the 2D line through the
/// light and the screen centre. Appends to `vertex` and returns the vertex count it added (`vertex_count` upstream).
pub fn lensflare_generate_vertex_buffers(
    lic: &LensflareInit,
    vertex: &mut Vec<VertexXyzNduv2>,
    screen_x_scale: f32,
    screen_y_scale: f32,
    dazzle_intensity: f32,
    transformed_location: [f32; 4],
    convert_color: ConvertColor,
) -> i32 {
    let z = transformed_location[2];
    let distance_multiplier = (transformed_location[0] * transformed_location[0] + transformed_location[1] * transformed_location[1]).sqrt() + 1.0;
    let start = vertex.len();
    for a in 0..lic.flare_count {
        let x = lic.flare_locations[a] * transformed_location[0];
        let y = lic.flare_locations[a] * transformed_location[1];
        let size = lic.flare_sizes[a] * distance_multiplier;
        let ix = size * screen_x_scale;
        let iy = size * screen_y_scale;
        let mut col = [lic.flare_colors[a][0] * dazzle_intensity, lic.flare_colors[a][1] * dazzle_intensity, lic.flare_colors[a][2] * dazzle_intensity];
        for c in col.iter_mut() {
            if *c > 1.0 {
                *c = 1.0;
            }
        }
        let color = convert_color(col, 1.0);
        let uv = lic.flare_uv[a];
        vertex.push(VertexXyzNduv2 { x: x + ix, y: y - iy, z, u1: uv[0], v1: uv[1], diffuse: color });
        vertex.push(VertexXyzNduv2 { x: x + ix, y: y + iy, z, u1: uv[2], v1: uv[1], diffuse: color });
        vertex.push(VertexXyzNduv2 { x: x - ix, y: y + iy, z, u1: uv[2], v1: uv[3], diffuse: color });
        vertex.push(VertexXyzNduv2 { x: x - ix, y: y - iy, z, u1: uv[0], v1: uv[3], diffuse: color });
    }
    (vertex.len() - start) as i32
}

/// `DazzleTypeClass`: its init block, name, shaders, textures (host handles) and fade distances squared.
#[derive(Clone, Debug, Default)]
pub struct DazzleTypeClass {
    pub name: String,
    pub ic: DazzleInit,
    pub lensflare_id: u32,
    pub dazzle_shader: u32,
    pub halo_shader: u32,
    pub fadeout_start_sqr: f32,
    pub fadeout_end_sqr: f32,
    pub primary_texture: Option<usize>,
    pub secondary_texture: Option<usize>,
}

/// What the texture lookups call out to: `WW3DAssetManager::Get_Texture(name)`.
pub trait DazzleTextures {
    fn get_texture(&mut self, name: &str) -> Option<usize>;
}

impl DazzleTypeClass {
    /// `DazzleTypeClass::Set_Dazzle_Shader` (dazzle.cpp:461).
    pub fn set_dazzle_shader(&mut self, s: u32) {
        self.dazzle_shader = s;
    }

    /// `DazzleTypeClass::Set_Halo_Shader` (dazzle.cpp:468).
    pub fn set_halo_shader(&mut self, s: u32) {
        self.halo_shader = s;
    }

    /// `DazzleTypeClass::Get_Dazzle_Texture` (dazzle.cpp:475): loaded on first use.
    pub fn get_dazzle_texture<T: DazzleTextures>(&mut self, assets: &mut T) -> Option<usize> {
        if self.primary_texture.is_none() {
            self.primary_texture = assets.get_texture(&self.ic.primary_texture_name);
        }
        self.primary_texture
    }

    /// `DazzleTypeClass::Get_Halo_Texture` (dazzle.cpp:488).
    pub fn get_halo_texture<T: DazzleTextures>(&mut self, assets: &mut T) -> Option<usize> {
        if self.secondary_texture.is_none() {
            self.secondary_texture = assets.get_texture(&self.ic.secondary_texture_name);
        }
        self.secondary_texture
    }

    /// `DazzleTypeClass::Calculate_Intensities` (dazzle.cpp:501). `halo_intensity` is in/out (it comes in as the object's
    /// halo intensity), `dazzle_size` is out. Returns `(dazzle_intensity, dazzle_size, halo_intensity)`;
    /// `distance` is the squared distance to the camera (compared against `fadeout_*_sqr`).
    pub fn calculate_intensities(&self, mut halo_intensity: f32, camera_dir: [f32; 3], dazzle_dir: [f32; 3], dir_to_dazzle: [f32; 3], mut distance: f32) -> (f32, f32, f32) {
        let ic = &self.ic;
        let mut dazzle_size = 0.0f32;
        let dot = -dot3(dir_to_dazzle, camera_dir);
        let mut dazzle_intensity = dot;
        if ic.use_camera_translation && distance > self.fadeout_end_sqr {
            return (0.0, dazzle_size, halo_intensity);
        }
        dazzle_intensity -= 1.0 - ic.dazzle_area;
        dazzle_intensity /= ic.dazzle_area;
        dazzle_intensity = clamp01(dazzle_intensity);
        if ic.dazzle_direction_area > 0.0 {
            let mut angle = -dot3(camera_dir, dazzle_dir);
            angle -= 1.0 - ic.dazzle_direction_area;
            angle /= ic.dazzle_direction_area;
            angle = clamp01(angle);
            dazzle_intensity *= angle;
        }
        if dazzle_intensity > 0.0 {
            dazzle_size = dazzle_intensity.powf(ic.dazzle_size_pow);
            dazzle_intensity = dazzle_intensity.powf(ic.dazzle_intensity_pow);
        } else {
            dazzle_intensity = 0.0;
        }
        if ic.halo_intensity_pow > WWMATH_EPSILON {
            if dot > 0.0 {
                let scale = dot.powf(ic.halo_intensity_pow);
                halo_intensity *= scale;
            } else {
                halo_intensity = 0.0;
            }
        }
        dazzle_intensity *= ic.dazzle_intensity;
        halo_intensity *= ic.halo_intensity;
        // Flare fadeout distance - fade it out...
        if ic.use_camera_translation && distance > self.fadeout_start_sqr {
            distance = distance.sqrt();
            distance -= ic.fadeout_start;
            distance /= ic.fadeout_end - ic.fadeout_start;
            dazzle_intensity *= 1.0 - distance;
            halo_intensity *= 1.0 - distance;
        }
        (dazzle_intensity, dazzle_size, halo_intensity)
    }
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `WWMath::Clamp(val)` with its default 0..1 bounds.
fn clamp01(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

/// `DazzleRenderObjClass`' static tables (`types`, `lensflares`).
#[derive(Clone, Debug, Default)]
pub struct DazzleTables {
    pub types: Vec<Option<DazzleTypeClass>>,
    pub lensflares: Vec<Option<LensflareTypeClassData>>,
    /// `_VisibilityHandler`: `None` is `&_DefaultVisibilityHandler`.
    pub custom_visibility_handler: bool,
}

/// `LensflareTypeClass` data.
#[derive(Clone, Debug, Default)]
pub struct LensflareTypeClassData {
    pub name: String,
    pub lic: LensflareInit,
}

impl DazzleTables {
    /// `DazzleRenderObjClass::Init_Type` (dazzle.cpp:680): grow the table to `type+1`, replace any previous entry.
    /// `init_shaders` is the (already translated) `Init_Shaders`.
    pub fn init_type(&mut self, ty: usize, new_type: DazzleTypeClass, init_shaders: impl FnOnce()) {
        init_shaders();
        if ty >= self.types.len() {
            self.types.resize(ty + 1, None);
        }
        self.types[ty] = Some(new_type);
    }

    /// `DazzleRenderObjClass::Init_Lensflare` (dazzle.cpp:703).
    pub fn init_lensflare(&mut self, ty: usize, new_lensflare: LensflareTypeClassData, init_shaders: impl FnOnce()) {
        init_shaders();
        if ty >= self.lensflares.len() {
            self.lensflares.resize(ty + 1, None);
        }
        self.lensflares[ty] = Some(new_lensflare);
    }

    /// `DazzleRenderObjClass::Install_Dazzle_Visibility_Handler` (dazzle.cpp:750): NULL restores the default handler.
    pub fn install_dazzle_visibility_handler(&mut self, has_handler: bool) {
        self.custom_visibility_handler = has_handler;
    }

    /// `DazzleRenderObjClass::Get_Type_Name` (dazzle.cpp:1293): "DEFAULT" (id 0 by convention) when out of range.
    pub fn get_type_name(&self, id: u32) -> &str {
        if (id as usize) < self.types.len() {
            if let Some(t) = &self.types[id as usize] {
                return &t.name;
            }
        }
        "DEFAULT"
    }

    /// `DazzleRenderObjClass::Get_Type_Class` (dazzle.cpp:1310): `None` when out of range (usually `UINT_MAX`).
    pub fn get_type_class(&self, id: u32) -> Option<&DazzleTypeClass> {
        if id as usize >= self.types.len() {
            return None;
        }
        self.types[id as usize].as_ref()
    }

    /// `DazzleRenderObjClass::Get_Lensflare_ID` (dazzle.cpp:1323): `UINT_MAX` when not found.
    pub fn get_lensflare_id(&self, name: &str) -> u32 {
        for (a, l) in self.lensflares.iter().enumerate() {
            if let Some(l) = l {
                if l.name == name {
                    return a as u32;
                }
            }
        }
        u32::MAX
    }

    /// `DazzleRenderObjClass::Get_Lensflare_Class` (dazzle.cpp:1339).
    pub fn get_lensflare_class(&self, id: u32) -> Option<&LensflareTypeClassData> {
        if id as usize >= self.lensflares.len() {
            return None;
        }
        self.lensflares[id as usize].as_ref()
    }
}

/// `DazzleLayerClass`: one visible list per dazzle type, as the ids of the dazzles on it (head first).
#[derive(Clone, Debug, Default)]
pub struct DazzleLayerClass {
    pub visible_lists: Vec<Vec<usize>>,
}

impl DazzleLayerClass {
    /// `DazzleLayerClass::Get_Visible_Item_Count` (dazzle.cpp:1610): 0 (asserting) for an out-of-range type.
    pub fn get_visible_item_count(&self, ty: usize) -> i32 {
        if ty >= self.visible_lists.len() {
            debug_assert!(false);
            return 0;
        }
        self.visible_lists[ty].len() as i32
    }

    /// `DazzleLayerClass::Clear_Visible_List` (dazzle.cpp:1629): each dazzle releases its reference and leaves the list
    /// (`on_list = false`, done by `release` here); returns the ids released.
    pub fn clear_visible_list(&mut self, ty: usize) -> Vec<usize> {
        if ty >= self.visible_lists.len() {
            debug_assert!(false);
            return Vec::new();
        }
        std::mem::take(&mut self.visible_lists[ty])
    }

    /// `DazzleRenderObjClass::Set_Current_Dazzle_Layer` is a static setter of `current_dazzle_layer`; see
    /// [`set_current_dazzle_layer`].
    pub fn index_buffer_for(poly_count: usize) -> Vec<u16> {
        dazzle_index_buffer(poly_count)
    }
}

/// `DazzleRenderObjClass::Set_Current_Dazzle_Layer` (dazzle.cpp:892): `current_dazzle_layer = layer`.
pub fn set_current_dazzle_layer(current: &mut Option<usize>, layer: Option<usize>) {
    *current = layer;
}

/// The quad index fill of `Render_Dazzle`: `for a in 0..poly_count/2`, indices `4a, 4a+1, 4a+2, 4a, 4a+2, 4a+3`.
pub fn dazzle_index_buffer(poly_count: usize) -> Vec<u16> {
    let mut inds = Vec::with_capacity(poly_count * 3);
    for a in 0..poly_count / 2 {
        let a = a as u16;
        inds.push(4 * a);
        inds.push(4 * a + 1);
        inds.push(4 * a + 2);
        inds.push(4 * a);
        inds.push(4 * a + 2);
        inds.push(4 * a + 3);
    }
    inds
}

/// The draw calls `Render_Dazzle` makes.
pub trait DazzleHost {
    /// `DX8Wrapper::Get_Transform(D3DTS_VIEW)` as a row-major 4x4.
    fn get_view_transform(&self) -> [[f32; 4]; 4];
    fn get_world_transform(&self) -> [[f32; 4]; 4];
    fn get_projection_transform(&self) -> [[f32; 4]; 4];
    fn set_view_transform(&mut self, m: [[f32; 4]; 4]);
    fn set_world_transform(&mut self, m: [[f32; 4]; 4]);
    fn set_projection_transform(&mut self, m: [[f32; 4]; 4]);
    /// `Set_World_Identity`, `Set_View_Identity`, `Set_Transform(PROJECTION, Matrix4x4(true))`.
    fn set_identity_transforms(&mut self);
    /// `WW3D::Get_Device_Resolution`: `(width, height)`.
    fn device_resolution(&self) -> (i32, i32);
    fn set_vertex_buffer(&mut self, verts: &[VertexXyzNduv2]);
    fn set_index_buffer(&mut self, inds: &[u16], index_base_offset: i32);
    fn set_shader(&mut self, shader: u32);
    fn set_texture(&mut self, stage: u32, texture: Option<usize>);
    fn draw_triangles(&mut self, start_index: i32, polygon_count: i32, min_vertex_index: i32, vertex_count: i32);
    fn convert_color(&self, color: [f32; 3], alpha: f32) -> u32;
}

/// The per-dazzle state `Render_Dazzle` reads.
#[derive(Clone, Debug, Default)]
pub struct DazzleRenderState {
    pub ty: usize,
    pub current_scale: f32,
    pub current_vloc: [f32; 3],
    pub transformed_loc: [f32; 4],
    pub current_dazzle_intensity: f32,
    pub current_dazzle_size: f32,
    pub current_halo_intensity: f32,
    pub dazzle_color: [f32; 3],
    pub halo_color: [f32; 3],
    pub lensflare_intensity: f32,
    pub default_halo_shader: u32,
    pub default_dazzle_shader: u32,
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn clamp_color(mut col: [f32; 3]) -> [f32; 3] {
    for c in col.iter_mut() {
        if *c > 1.0 {
            *c = 1.0;
        }
    }
    col
}

fn quad(verts: &mut Vec<VertexXyzNduv2>, p: [[f32; 3]; 4], color: u32) {
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    for i in 0..4 {
        verts.push(VertexXyzNduv2 { x: p[i][0], y: p[i][1], z: p[i][2], u1: uvs[i][0], v1: uvs[i][1], diffuse: color });
    }
}

/// `DazzleRenderObjClass::Render_Dazzle` (dazzle.cpp:1015): build the dazzle quad, the halo quad and the lensflare quads
/// in one vertex buffer (dazzle, then halo, then lensflare), then draw halo, dazzle and lensflare in that order with the
/// identity transforms and restore the old ones. `halo_texture` / `dazzle_texture` are the type's textures, and
/// `lensflare_texture` the lensflare's.
pub fn render_dazzle<H: DazzleHost>(
    host: &mut H,
    st: &DazzleRenderState,
    params: &DazzleTypeClass,
    lensflare: Option<&LensflareTypeClassData>,
    halo_texture: Option<usize>,
    dazzle_texture: Option<usize>,
    lensflare_texture: Option<usize>,
) {
    // WWPROFILE("Dazzle::Render")
    let old_view = host.get_view_transform();
    let old_world = host.get_world_transform();
    let old_projection = host.get_projection_transform();

    let (display_width, display_height) = host.device_resolution();
    let w = display_width as f32;
    let h = display_height as f32;
    let mut screen_x_scale = 1.0f32;
    let mut screen_y_scale = 1.0f32;
    if w > h {
        screen_y_scale = w / h;
    } else {
        screen_x_scale = h / w;
    }
    let halo_scale_x = params.ic.halo_scale_x;
    let halo_scale_y = params.ic.halo_scale_y;
    let dazzle_scale_x = params.ic.dazzle_scale_x * st.current_scale;
    let dazzle_scale_y = params.ic.dazzle_scale_y * st.current_scale;

    let mut vertex_count: i32 = 4;
    let mut halo_vertex_count: i32 = 0;
    let mut dazzle_vertex_count: i32 = 0;
    let mut lensflare_vertex_count: i32 = 0;

    let mut verts: Vec<VertexXyzNduv2> = Vec::new();
    let dazzle_dxt = [screen_x_scale, 0.0, 0.0];
    let halo_dxt = mul3(dazzle_dxt, halo_scale_x);
    let dazzle_dxt = mul3(dazzle_dxt, dazzle_scale_x);
    let dazzle_dyt = [0.0, screen_y_scale, 0.0];
    let halo_dyt = mul3(dazzle_dyt, halo_scale_y);
    let dazzle_dyt = mul3(dazzle_dyt, dazzle_scale_y);
    let halo_size = 1.0f32;

    if st.current_dazzle_intensity > 0.0 {
        dazzle_vertex_count += 4;
        let col = mul3(
            [st.dazzle_color[0] * params.ic.dazzle_color[0], st.dazzle_color[1] * params.ic.dazzle_color[1], st.dazzle_color[2] * params.ic.dazzle_color[2]],
            st.current_dazzle_intensity,
        );
        let color = host.convert_color(clamp_color(col), 1.0);
        let s = st.current_dazzle_size;
        let sum = add3(dazzle_dxt, dazzle_dyt);
        let dif = sub3(dazzle_dxt, dazzle_dyt);
        quad(
            &mut verts,
            [
                add3(st.current_vloc, mul3(dif, s)),
                add3(st.current_vloc, mul3(sum, s)),
                sub3(st.current_vloc, mul3(dif, s)),
                sub3(st.current_vloc, mul3(sum, s)),
            ],
            color,
        );
    }
    if st.current_halo_intensity != 0.0 {
        halo_vertex_count += 4;
        let col = mul3(
            [st.halo_color[0] * params.ic.halo_color[0], st.halo_color[1] * params.ic.halo_color[1], st.halo_color[2] * params.ic.halo_color[2]],
            st.current_halo_intensity,
        );
        let color = host.convert_color(clamp_color(col), 1.0);
        let dif = mul3(sub3(halo_dxt, halo_dyt), halo_size);
        let sum = mul3(add3(halo_dxt, halo_dyt), halo_size);
        quad(
            &mut verts,
            [
                add3(st.current_vloc, dif),
                add3(st.current_vloc, sum),
                add3(st.current_vloc, mul3(dif, -1.0)),
                add3(st.current_vloc, mul3(sum, -1.0)),
            ],
            color,
        );
    }
    if let Some(lf) = lensflare {
        if st.current_dazzle_intensity > 0.0 {
            let convert = |c: [f32; 3], a: f32| host.convert_color(c, a);
            let mut tmp: Vec<VertexXyzNduv2> = Vec::new();
            // Generate_Vertex_Buffers with the host's colour conversion
            let z = st.transformed_loc[2];
            let dm = (st.transformed_loc[0] * st.transformed_loc[0] + st.transformed_loc[1] * st.transformed_loc[1]).sqrt() + 1.0;
            let intensity = st.current_dazzle_intensity * st.lensflare_intensity;
            for a in 0..lf.lic.flare_count {
                let x = lf.lic.flare_locations[a] * st.transformed_loc[0];
                let y = lf.lic.flare_locations[a] * st.transformed_loc[1];
                let size = lf.lic.flare_sizes[a] * dm;
                let ix = size * screen_x_scale;
                let iy = size * screen_y_scale;
                let col = clamp_color(mul3(lf.lic.flare_colors[a], intensity));
                let color = convert(col, 1.0);
                let uv = lf.lic.flare_uv[a];
                tmp.push(VertexXyzNduv2 { x: x + ix, y: y - iy, z, u1: uv[0], v1: uv[1], diffuse: color });
                tmp.push(VertexXyzNduv2 { x: x + ix, y: y + iy, z, u1: uv[2], v1: uv[1], diffuse: color });
                tmp.push(VertexXyzNduv2 { x: x - ix, y: y + iy, z, u1: uv[2], v1: uv[3], diffuse: color });
                tmp.push(VertexXyzNduv2 { x: x - ix, y: y - iy, z, u1: uv[0], v1: uv[3], diffuse: color });
            }
            lensflare_vertex_count = tmp.len() as i32;
            verts.extend(tmp);
            vertex_count += lensflare_vertex_count;
        }
    }

    let dazzle_poly_count = dazzle_vertex_count >> 1;
    let halo_poly_count = halo_vertex_count >> 1;
    let lensflare_poly_count = lensflare_vertex_count >> 1;
    let mut poly_count = if halo_poly_count > dazzle_poly_count { halo_poly_count } else { dazzle_poly_count };
    if lensflare_poly_count > poly_count {
        poly_count = lensflare_poly_count;
    }
    if poly_count == 0 {
        return;
    }
    host.set_vertex_buffer(&verts);
    let inds = dazzle_index_buffer(poly_count as usize);

    host.set_identity_transforms();
    if halo_poly_count != 0 {
        host.set_index_buffer(&inds, dazzle_vertex_count);
        host.set_shader(st.default_halo_shader);
        host.set_texture(0, halo_texture);
        host.draw_triangles(0, halo_poly_count, 0, vertex_count);
    }
    if dazzle_poly_count != 0 {
        host.set_index_buffer(&inds, 0);
        host.set_shader(st.default_dazzle_shader);
        host.set_texture(0, dazzle_texture);
        host.draw_triangles(0, dazzle_poly_count, 0, vertex_count);
    }
    if lensflare_poly_count != 0 {
        host.set_index_buffer(&inds, dazzle_vertex_count + halo_vertex_count);
        host.set_shader(st.default_dazzle_shader);
        host.set_texture(0, lensflare_texture);
        host.draw_triangles(0, lensflare_poly_count, 0, vertex_count);
    }
    host.set_projection_transform(old_projection);
    host.set_view_transform(old_view);
    host.set_world_transform(old_world);
}

/// The VIS-rasterizer calls `vis_render_dazzle` makes.
pub trait VisRasterizerHost {
    fn enable_two_sided_rendering(&mut self, on: bool);
    fn camera_translation(&self) -> [f32; 3];
    /// `Peek_Camera()->Get_View_Plane(min, max)`.
    fn camera_view_plane(&self) -> ([f32; 2], [f32; 2]);
    /// `Set_Model_Transform(tm)` for `tm.Look_At(daz_point, cam_point, 0)`.
    fn set_model_transform_look_at(&mut self, from: [f32; 3], to: [f32; 3]);
    /// `Render_Triangles(verts, 4, polys, 2, bounds)`.
    fn render_triangles(&mut self, verts: &[[f32; 3]; 4], polys: &[[u16; 3]; 2], center: [f32; 3], extent: [f32; 3]);
}

/// `DazzleRenderObjClass::vis_render_dazzle` (dazzle.cpp:1352): a camera-facing quad at the dazzle, as big as the larger
/// of its halo and dazzle scales, so static dazzles get VIS information.
pub fn vis_render_dazzle<R: VisRasterizerHost>(rast: &mut R, params: &DazzleTypeClass, daz_point: [f32; 3]) {
    rast.enable_two_sided_rendering(true);
    let cam_point = rast.camera_translation();
    rast.set_model_transform_look_at(daz_point, cam_point);
    let polys = [[0u16, 1, 2], [0, 2, 3]];
    let (view_min, view_max) = rast.camera_view_plane();
    let mut scale_x = params.ic.halo_scale_x;
    if scale_x < 0.001 && params.ic.dazzle_scale_x > scale_x {
        scale_x = params.ic.dazzle_scale_x;
    }
    let mut scale_y = params.ic.halo_scale_y;
    if scale_y < 0.001 && params.ic.dazzle_scale_y > scale_y {
        scale_y = params.ic.dazzle_scale_y;
    }
    let d = sub3(daz_point, cam_point);
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let dxt = [dist * scale_x / (view_max[0] - view_min[0]), 0.0, 0.0];
    let dyt = [0.0, dist * scale_y / (view_max[1] - view_min[1]), 0.0];
    let verts = [add3(dxt, dyt), sub3(dxt, dyt), mul3(add3(dxt, dyt), -1.0), add3(mul3(dxt, -1.0), dyt)];
    let s = add3(dxt, dyt);
    let extent = 1.1 * (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
    rast.enable_two_sided_rendering(true);
    rast.render_triangles(&verts, &polys, daz_point, [extent, extent, extent]);
    rast.enable_two_sided_rendering(false);
}
