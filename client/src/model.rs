// SPDX-License-Identifier: GPL-3.0-only
//! W3D models drawn with Bevy (GitHub #10, step 4): the device side of WW3D, which is the one part of the renderer that
//! is new (upstream's is Direct3D 8).
//!
//! What is drawn is decided in the engine crate, translated from upstream: [`MeshModel`] (a mesh after
//! `install_materials` and `post_process`, split into the DX8 renderer's polygon groups), [`RenderModel`] (an HLOD's
//! meshes bound to its hierarchy's bones, posed by an animation, skins deformed on the CPU as upstream does), the
//! shader's device states ([`shader::apply`]) and the texture images ([`load_texture_image`]). This module only maps
//! them to Bevy:
//!
//! - **One Bevy mesh per polygon group**, with every vertex of the W3D mesh (positions and normals in Bevy's axes,
//!   [`crate::view::to_bevy`]), the UV arrays the group's vertex material reads on stages 0 and 1, the vertex colours
//!   (DCG) when the material takes its diffuse from them, and the group's triangles as indices. A skin's positions and
//!   normals are rewritten every frame from the deformed vertices.
//! - **A `StandardMaterial` per group** ([`MaterialDesc`]): the blend pair Direct3D would use mapped to Bevy's alpha modes
//!   (`ONE/ZERO` opaque, alpha test a mask at upstream's reference `0x60`, `SRCALPHA/INVSRCALPHA` blend, `ONE/ONE` add,
//!   `ZERO/SRCCOLOR` multiply); culling from the shader's cull mode; lighting from the vertex material (`unlit` when its
//!   lighting is off); the base colour from the material's diffuse and opacity (white when the vertex colours are the
//!   diffuse source); its emissive colour; the stage-0 texture when the shader textures.
//! - **Sub-object transforms**: the bone's posed `Matrix3D`, conjugated into Bevy's axes ([`to_bevy_matrix`]).
//!
//! **Approximations, named** ([`MaterialDesc::approximations`]): blend pairs Bevy's alpha modes do not have (screen
//! `ONE/INVSRCCOLOR` is drawn additive; the rest as alpha blend); depth compare functions other than `LEQUAL` and
//! depth writes off on opaque groups; the secondary gradient (specular) and stage 1 (detail textures); fog; the
//! texture mappers (UV animation, environment maps); the gradient modes other than modulate (`GRADIENT_ADD` and
//! `MODULATE2X` are drawn as modulate); fixed-function lighting (Bevy's PBR with full roughness stands in for
//! Direct3D's Gouraud lighting). The front face is taken to be counter-clockwise in the model's right-handed, z-up
//! space (Bevy's default after the axis change); [`MeshModel::winding_agrees_with_normals`] reports how far a real
//! model agrees.

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, Face, TextureDimension, TextureFormat};

use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice, MemoryDevice};
use rts_lockstep_engine::formats::w3d::assetmgr::{AssetManager, HAnim, W3dFileFactory};
use rts_lockstep_engine::formats::w3d::meshmdl::{MeshModel, PolygonGroup};
use rts_lockstep_engine::formats::w3d::render::{
    compute_current_frame, get_frame_rate, load_texture_image, AnimMode, RenderModel, TextureImage, TextureLoadError,
};
use rts_lockstep_engine::formats::w3d::shader::{self, d3d, DeviceCaps};
use rts_lockstep_engine::formats::w3d::synth::{self, ChannelSpec, MeshSpec, PivotSpec};
use rts_lockstep_engine::formats::w3d::vertmaterial::{VertexMaterialState, D3DMCS_COLOR1};
use rts_lockstep_engine::formats::w3d::w3d_file::{ANIM_CHANNEL_Q, W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;

use crate::view::to_bevy;

/// A `Matrix3D` (game axes, z up) as a Bevy transform matrix (y up): `P M P^-1` with `P (x, y, z) = (x, z, -y)`.
pub fn to_bevy_matrix(m: &Matrix3D) -> Mat4 {
    let r = &m.row;
    let rot = Mat3::from_cols(Vec3::new(r[0].x, r[1].x, r[2].x), Vec3::new(r[0].y, r[1].y, r[2].y), Vec3::new(r[0].z, r[1].z, r[2].z));
    let p = Mat3::from_cols(Vec3::X, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
    let rb = p * rot * p.transpose();
    let t = p * Vec3::new(r[0].w, r[1].w, r[2].w);
    Mat4::from_cols(rb.x_axis.extend(0.0), rb.y_axis.extend(0.0), rb.z_axis.extend(0.0), t.extend(1.0))
}

/// How a group's shader and vertex material are drawn: the plain data a `StandardMaterial` is made from.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialDesc {
    pub alpha_mode: AlphaMode,
    /// `None`: both faces (the shader's cull mode off).
    pub cull_mode: Option<Face>,
    pub unlit: bool,
    /// Linear RGBA.
    pub base_color: [f32; 4],
    pub emissive: [f32; 3],
    /// The stage-0 texture (an index into [`MeshModel::textures`]) when the shader textures.
    pub texture: Option<usize>,
    /// Colour writes are off (`D3DBLEND_ZERO`/`ONE`): nothing visible is drawn.
    pub invisible: bool,
    /// What Bevy's material cannot reproduce for this group.
    pub approximations: Vec<&'static str>,
}

/// The material of a polygon group (see the module docs).
pub fn material_desc(model: &MeshModel, group: &PolygonGroup) -> MaterialDesc {
    let st = shader::apply(group.shader, &DeviceCaps::full());
    let default_vm = VertexMaterialState::default();
    let vm = group.material.and_then(|m| model.vertex_materials.get(m)).unwrap_or(&default_vm);
    let mut approximations = Vec::new();
    let Ok(st) = st else {
        // a destination blend past the table: undefined upstream
        approximations.push("an undefined destination blend, drawn opaque");
        return MaterialDesc {
            alpha_mode: AlphaMode::Opaque,
            cull_mode: Some(Face::Back),
            unlit: !vm.use_lighting,
            base_color: [1.0; 4],
            emissive: [0.0; 3],
            texture: None,
            invisible: false,
            approximations,
        };
    };
    let pair = (st.src_blend.unwrap_or(d3d::D3DBLEND_ONE), st.dst_blend.unwrap_or(d3d::D3DBLEND_ZERO));
    let mut invisible = false;
    let mut alpha_mode = match pair {
        (d3d::D3DBLEND_ONE, d3d::D3DBLEND_ZERO) => AlphaMode::Opaque,
        (d3d::D3DBLEND_SRCALPHA, d3d::D3DBLEND_INVSRCALPHA) => AlphaMode::Blend,
        (d3d::D3DBLEND_ONE, d3d::D3DBLEND_ONE) => AlphaMode::Add,
        (d3d::D3DBLEND_ZERO, d3d::D3DBLEND_SRCCOLOR) | (d3d::D3DBLEND_DESTCOLOR, d3d::D3DBLEND_ZERO) => AlphaMode::Multiply,
        (d3d::D3DBLEND_ZERO, d3d::D3DBLEND_ONE) => {
            invisible = true;
            AlphaMode::Blend
        }
        (d3d::D3DBLEND_ONE, d3d::D3DBLEND_INVSRCCOLOR) => {
            approximations.push("screen blending drawn as additive");
            AlphaMode::Add
        }
        _ => {
            approximations.push("a blend pair Bevy has no mode for, drawn as alpha blend");
            AlphaMode::Blend
        }
    };
    if let (true, Some(r)) = (st.alpha_test_enable, st.alpha_ref) {
        if alpha_mode == AlphaMode::Opaque || alpha_mode == AlphaMode::Blend {
            alpha_mode = AlphaMode::Mask(r as f32 / 255.0);
        } else {
            approximations.push("alpha test on a blended group");
        }
    }
    if st.zfunc != 4 {
        approximations.push("a depth compare other than LEQUAL");
    }
    if !st.zwrite_enable && alpha_mode == AlphaMode::Opaque {
        approximations.push("depth writes off on an opaque group");
    }
    if st.specular_enable {
        approximations.push("the secondary gradient (specular)");
    }
    if st.stage[1].color_op != d3d::D3DTOP_DISABLE {
        approximations.push("texture stage 1 (detail texture)");
    }
    if !matches!(group.shader.get_primary_gradient(), shader::GRADIENT_MODULATE | shader::GRADIENT_DISABLE) {
        approximations.push("a primary gradient other than modulate");
    }
    if vm.mapper.iter().any(|m| *m != rts_lockstep_engine::formats::w3d::vertmaterial::MapperKind::None) {
        approximations.push("a texture mapper (UVs drawn unmapped)");
    }
    let texturing = group.shader.get_texturing() == 1;
    let texture = if texturing { group.textures[0] } else { None };
    // GRADIENT_DISABLE with texturing is a decal: the texture alone
    let decal = texturing && group.shader.get_primary_gradient() == shader::GRADIENT_DISABLE;
    let vertex_diffuse = vm.diffuse_color_source == D3DMCS_COLOR1 && model.color_array[0].is_some();
    let base_rgb = if decal || vertex_diffuse { [1.0; 3] } else { vm.diffuse };
    let alpha = if decal || vertex_diffuse { 1.0 } else { vm.opacity };
    MaterialDesc {
        alpha_mode,
        cull_mode: if st.cull_mode == d3d::D3DCULL_NONE { None } else { Some(Face::Back) },
        unlit: !vm.use_lighting || decal,
        base_color: [base_rgb[0], base_rgb[1], base_rgb[2], alpha],
        emissive: vm.emissive,
        texture,
        invisible,
        approximations,
    }
}

impl MaterialDesc {
    /// The Bevy material, with the textures already made (by [`MeshModel::textures`] index).
    pub fn to_standard_material(&self, textures: &[Option<Handle<Image>>]) -> StandardMaterial {
        StandardMaterial {
            base_color: Color::linear_rgba(self.base_color[0], self.base_color[1], self.base_color[2], self.base_color[3]),
            base_color_texture: self.texture.and_then(|t| textures.get(t).cloned().flatten()),
            emissive: LinearRgba::rgb(self.emissive[0], self.emissive[1], self.emissive[2]),
            alpha_mode: self.alpha_mode,
            cull_mode: self.cull_mode,
            double_sided: self.cull_mode.is_none(),
            unlit: self.unlit,
            perceptual_roughness: 1.0,
            metallic: 0.0,
            reflectance: 0.1,
            ..default()
        }
    }
}

/// The Bevy mesh of one polygon group, over the given (posed or deformed) vertices and normals, game axes.
pub fn group_mesh(model: &MeshModel, group: &PolygonGroup, positions: &[[f32; 3]], normals: &[[f32; 3]]) -> Mesh {
    let n = positions.len();
    let pos: Vec<[f32; 3]> = positions.iter().map(|p| to_bevy(*p)).collect();
    let nrm: Vec<[f32; 3]> = (0..n).map(|i| to_bevy(normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]))).collect();
    let default_vm = VertexMaterialState::default();
    let vm = group.material.and_then(|m| model.vertex_materials.get(m)).unwrap_or(&default_vm);
    let uv = |stage: usize| -> Vec<[f32; 2]> {
        match model.uv_array(vm.uv_source[stage].max(0) as usize) {
            Some(a) if a.len() >= n => a[..n].to_vec(),
            _ => vec![[0.0, 0.0]; n],
        }
    };
    let mut indices = Vec::with_capacity(group.polygons.len() * 3);
    for &p in &group.polygons {
        let t = model.polys[p as usize];
        if t.iter().all(|&i| usize::from(i) < n) {
            indices.extend(t.iter().map(|&i| u32::from(i)));
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv(0))
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uv(1))
        .with_inserted_indices(Indices::U32(indices));
    if let Some(colors) = &model.color_array[0] {
        if vm.diffuse_color_source == D3DMCS_COLOR1 && colors.len() >= n {
            // packed ARGB (DX8Wrapper::Convert_Color), sRGB bytes
            let c: Vec<[f32; 4]> = colors[..n]
                .iter()
                .map(|&argb| {
                    let lin = |b: u32| srgb_to_linear((b & 0xff) as f32 / 255.0);
                    [lin(argb >> 16), lin(argb >> 8), lin(argb), ((argb >> 24) & 0xff) as f32 / 255.0]
                })
                .collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, c);
        }
    }
    mesh
}

fn srgb_to_linear(c: f32) -> f32 {
    Color::srgb(c, c, c).to_linear().red
}

/// A decoded W3D texture as a Bevy image (sRGB RGBA8), wrapping unless the texture asks to clamp.
pub fn texture_image(img: &TextureImage, clamp: Option<(bool, bool)>) -> Image {
    let mut image = Image::new(
        Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        img.rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let (cu, cv) = clamp.unwrap_or((false, false));
    let mode = |c: bool| if c { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: mode(cu), address_mode_v: mode(cv), ..ImageSamplerDescriptor::linear() });
    image
}

/// A model to view: the render object, the asset manager it came from (the morph animations' poses), its animation,
/// and its textures, decoded.
pub struct ModelScene {
    pub model: RenderModel,
    pub assets: AssetManager,
    pub anim: Option<HAnim>,
    /// The animation's current frame.
    pub frame: f32,
    /// Per sub-object, per texture: the image or why there is none.
    pub textures: Vec<Vec<Result<TextureImage, TextureLoadError>>>,
    /// What was loaded, for the status line.
    pub report: String,
}

impl ModelScene {
    /// Loads `name` (`Create_Render_Obj`) and, if given, the animation `anim` (`"HIERARCHY.ANIM"`) through the game's
    /// file search over `fs`, then poses it at frame 0.
    pub fn load<D: LocalDevice>(fs: &FileSystem<D>, name: &[u8], anim: Option<&[u8]>) -> Result<ModelScene, String> {
        let show = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        let mut assets = AssetManager::new_game(W3dFileFactory::default());
        let model = RenderModel::create(&mut assets, fs, name)
            .map_err(|e| format!("{}: {e}", show(name)))?
            .ok_or_else(|| format!("no render object {}", show(name)))?;
        let anim = match anim {
            Some(a) => Some(assets.get_hanim(fs, a).map_err(|e| format!("{}: {e}", show(a)))?.cloned().ok_or_else(|| format!("no animation {}", show(a)))?),
            None => None,
        };
        let textures = model_textures(&assets, fs, &model)?;
        let total: usize = textures.iter().map(|t| t.len()).sum();
        let found: usize = textures.iter().flatten().filter(|r| r.is_ok()).count();
        let report = format!(
            "{}: {} meshes ({} skins), {} not drawn, {found} of {total} textures{}",
            show(name),
            model.sub_objects.len(),
            model.sub_objects.iter().filter(|s| s.is_skin()).count(),
            model.not_drawn.len(),
            anim.as_ref().map(|a| format!(", animation {}", show(a.name()))).unwrap_or_default()
        );
        let mut scene = ModelScene { model, assets, anim, frame: 0.0, textures, report };
        scene.advance(0.0).map_err(|e| format!("posing: {e}"))?;
        Ok(scene)
    }

    /// `Compute_Current_Frame` (looping) after `dt_ms`, then the hierarchy's update at the new frame.
    pub fn advance(&mut self, dt_ms: f32) -> rts_lockstep_engine::formats::w3d::W3dResult<()> {
        let root = Matrix3D::IDENTITY;
        match &self.anim {
            Some(a) => {
                self.frame = compute_current_frame(self.frame, a.get_num_frames(), get_frame_rate(a), 1.0, 1.0, dt_ms, AnimMode::Loop).0;
                self.model.update(&root, Some((a, &self.assets, self.frame)))
            }
            None => self.model.update::<AssetManager>(&root, None),
        }
    }
}

/// Placeholder art made here (no game files): a robot of a hierarchy (root, body, arm), a rigid box body, a skinned arm
/// (its far vertices on the arm bone, the near on the body) and a hidden helper mesh, a 16x16 checker texture, and a
/// looping animation that swings the arm. The files the viewer reads when no Zero Hour folder is given; built by
/// [`synth`], read back by the translated loaders.
pub fn demo_art_files() -> Vec<(&'static [u8], Vec<u8>)> {
    let ident = [0.0, 0.0, 0.0, 1.0];
    let mut f = synth::htree_w3d(b"ROBOT", &[
        PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident },
        PivotSpec { name: b"BODY".to_vec(), parent: 0, translation: [0.0, 0.0, 10.0], rotation: ident },
        PivotSpec { name: b"ARM".to_vec(), parent: 1, translation: [10.0, 0.0, 5.0], rotation: ident },
    ]);
    let mut body = MeshSpec::cuboid(b"ROBOT", b"BODY", [-10.0, -6.0, -10.0], [10.0, 6.0, 10.0]);
    body.stage0.texture = Some(b"RobotChecker.tga".to_vec());
    // opaque, textured, modulated: DepthCompare LEQUAL, DepthMask, ColorMask, DestBlend ZERO, Fog, PriGradient
    // MODULATE, SecGradient, SrcBlend ONE, Texturing
    body.shader = [3, 1, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0];
    body.diffuse = [255, 255, 255];
    f.extend(body.to_w3d());
    // the arm: a box from the shoulder (bone BODY, x = 10) to the hand (bone ARM), vertices in their bone's space
    let mut arm = MeshSpec::cuboid(b"ROBOT", b"ARM", [0.0, -2.0, -2.0], [16.0, 2.0, 2.0]);
    arm.attributes = W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN;
    arm.stage0.texture = None;
    arm.shader = [3, 1, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
    arm.diffuse = [255, 160, 40];
    let mut bones = Vec::new();
    for v in arm.vertices.iter_mut() {
        if v[0] > 8.0 {
            // in the arm bone's space: the arm's pivot is the shoulder, so x stays
            bones.push(2);
        } else {
            bones.push(1);
            v[0] += 10.0; // in the body's space: the shoulder is at x = 10
            v[2] += 5.0;
        }
    }
    arm.bones = Some(bones);
    f.extend(arm.to_w3d());
    let mut helper = MeshSpec::quad(b"ROBOT", b"HELPER");
    helper.attributes = rts_lockstep_engine::formats::w3d::w3d_file::W3D_MESH_FLAG_HIDDEN;
    helper.stage0.texture = None;
    f.extend(helper.to_w3d());
    f.extend(synth::hlod_w3d(b"ROBOT", b"ROBOT", &[(b"ROBOT.BODY", 1), (b"ROBOT.ARM", 0), (b"ROBOT.HELPER", 0)]));
    // the arm swings about the body's y axis: 0, -45, 0, +45, 0 degrees over 5 keys (31 frames at 15 per second would
    // need more keys; the raw channel holds one value per frame, so 5 frames at 4 frames a second)
    let q = |deg: f32| {
        let h = deg.to_radians() / 2.0;
        [0.0, h.sin(), 0.0, h.cos()]
    };
    let data: Vec<f32> = [0.0, -45.0, 0.0, 45.0, 0.0].iter().flat_map(|&d| q(d)).collect();
    f.extend(synth::raw_anim_w3d(b"ROBOT", b"WAVE", 5, 4, &[ChannelSpec { pivot: 2, kind: ANIM_CHANNEL_Q, vector_len: 4, data }]));
    vec![(&b"Art/W3D/robot.w3d"[..], f), (&b"Art/Textures/robotchecker.tga"[..], checker_tga(16))]
}

/// The generated robot's files on a memory device (see [`demo_art_files`]).
pub fn demo_files() -> FileSystem<MemoryDevice> {
    let mut d = MemoryDevice::new();
    for (path, bytes) in demo_art_files() {
        d.insert(path, bytes);
    }
    FileSystem::new(d)
}

/// A `size` x `size` 32-bit Targa checker (bottom-up rows, as most Targas are).
fn checker_tga(size: u16) -> Vec<u8> {
    let mut t = vec![0u8, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    t.extend(size.to_le_bytes());
    t.extend(size.to_le_bytes());
    t.extend([32, 8]);
    for y in 0..size {
        for x in 0..size {
            let on = ((x / 4) + (y / 4)) % 2 == 0;
            t.extend(if on { [200, 200, 200, 255] } else { [60, 60, 160, 255] });
        }
    }
    t
}

/// One drawn piece of a [`ModelScene`]: sub-object `sub`'s polygon group `group`.
#[derive(Clone, Debug)]
pub struct Part {
    pub sub: usize,
    pub group: PolygonGroup,
    pub material: MaterialDesc,
}

/// Per sub-object, per texture of a render model: the image or why there is none (`Get_Texture` through the game's
/// file search over `fs`).
pub fn model_textures<D: LocalDevice>(assets: &AssetManager, fs: &FileSystem<D>, model: &RenderModel) -> Result<Vec<Vec<Result<TextureImage, TextureLoadError>>>, String> {
    let mut textures = Vec::new();
    for s in &model.sub_objects {
        let mut t = Vec::new();
        for tex in &s.model.textures {
            let r = load_texture_image(&assets.file_factory, fs, &tex.name).map_err(|e| format!("texture {}: {e}", String::from_utf8_lossy(&tex.name)))?;
            t.push(r);
        }
        textures.push(t);
    }
    Ok(textures)
}

/// The pieces to draw (hidden sub-objects and invisible groups left out).
pub fn parts(scene: &ModelScene) -> Result<Vec<Part>, String> {
    model_parts(&scene.model)
}

/// The pieces of a render model to draw (hidden sub-objects and invisible groups left out).
pub fn model_parts(model: &RenderModel) -> Result<Vec<Part>, String> {
    let mut out = Vec::new();
    for (i, s) in model.sub_objects.iter().enumerate() {
        if s.hidden {
            continue;
        }
        let groups = s.model.polygon_groups().map_err(|e| format!("{}: {e}", String::from_utf8_lossy(&s.name)))?;
        for group in groups {
            let material = material_desc(&s.model, &group);
            if !material.invisible {
                out.push(Part { sub: i, group, material });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    /// Predictions, written before the first run, for the generated robot:
    /// - the scene loads the HLOD with 3 meshes, 1 skin, 0 not drawn, 1 of 1 texture (the checker, from
    ///   `Art/Textures/robotchecker.tga`), and the animation `ROBOT.WAVE`;
    /// - two parts are drawn (the body and the arm; the helper is hidden), each one group of 12 triangles;
    /// - the body is opaque, textured, lit, culled; its mesh has 24 vertices with both UV sets and 36 indices, and its
    ///   texture is a 16x16 sRGB image that wraps;
    /// - the body sits on bone 1, 10 units up: in Bevy's axes its transform's translation is (0, 10, 0);
    /// - the arm's deformed hand vertex moves when the animation runs (the swing), the shoulder ones do not;
    /// - 125 ms at 4 frames a second is half a frame.
    ///
    /// **Prediction missed (2026-11-05), first run:** "1 of 2 textures": the hidden helper was `MeshSpec::quad`, which
    /// names `quad.tga`, and the scene loads every sub-object's textures (hidden ones too, as upstream's meshes do). The
    /// helper now names none.
    #[test]
    fn the_demo_robot_builds_bevy_assets() {
        let fs = demo_files();
        let mut scene = ModelScene::load(&fs, b"ROBOT", Some(b"ROBOT.WAVE")).unwrap();
        assert_eq!(scene.report, "ROBOT: 3 meshes (1 skins), 0 not drawn, 1 of 1 textures, animation ROBOT.WAVE");
        let tex = scene.textures[0][0].as_ref().unwrap();
        assert_eq!(tex.path, b"Art/Textures/robotchecker.tga");
        let parts = parts(&scene).unwrap();
        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|p| p.group.polygons.len() == 12));

        let body = &parts[0];
        assert_eq!(body.material.alpha_mode, AlphaMode::Opaque);
        assert_eq!((body.material.cull_mode, body.material.unlit, body.material.texture), (Some(Face::Back), false, Some(0)));
        assert!(body.material.approximations.is_empty(), "{:?}", body.material.approximations);
        let sub = &scene.model.sub_objects[body.sub];
        let mesh = group_mesh(&sub.model, &body.group, &sub.model.vertices, &sub.model.normals);
        assert_eq!(mesh.count_vertices(), 24);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some() && mesh.attribute(Mesh::ATTRIBUTE_UV_1).is_some());
        assert_eq!(mesh.indices().map(|i| i.len()), Some(36));
        let img = texture_image(tex, sub.model.textures[0].clamp);
        assert_eq!((img.width(), img.height(), img.texture_descriptor.format), (16, 16, TextureFormat::Rgba8UnormSrgb));
        let handles = vec![Some(Handle::<Image>::default())];
        let mat = body.material.to_standard_material(&handles);
        assert!(mat.base_color_texture.is_some());
        let t = to_bevy_matrix(&scene.model.sub_object_transform(body.sub).unwrap());
        assert_eq!(t.w_axis.truncate(), Vec3::new(0.0, 10.0, 0.0));

        let arm = &parts[1];
        assert!(scene.model.sub_objects[arm.sub].is_skin());
        let (v0, _) = scene.model.deformed(arm.sub).unwrap();
        scene.advance(250.0).unwrap(); // one frame: -45 degrees
        let (v1, _) = scene.model.deformed(arm.sub).unwrap();
        let moved = |i: usize| (0..3).any(|k| (v0[i][k] - v1[i][k]).abs() > 1e-3);
        let bones = scene.model.sub_objects[arm.sub].model.bone_links.clone().unwrap();
        for (i, bone) in bones.iter().enumerate().take(v0.len()) {
            assert_eq!(moved(i), *bone == 2, "vertex {i} on bone {bone}");
        }
        let mesh = group_mesh(&scene.model.sub_objects[arm.sub].model, &arm.group, &v1, &v1);
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(p)) => assert_eq!(p[0], to_bevy(v1[0])),
            other => panic!("{other:?}"),
        }
        scene.frame = 0.0;
        scene.advance(125.0).unwrap();
        assert!((scene.frame - 0.5).abs() < 1e-6);
    }

    /// Predictions: the quad of `MeshSpec::quad` (SRCALPHA/INVSRCALPHA, textured) is an alpha-blended material whose
    /// base colour is its diffuse (1, 128/255, 0) at opacity 1; with alpha test on it becomes a mask at 0x60 / 255;
    /// two-sided, it is not culled; additive ONE/ONE is `Add`.
    #[test]
    fn shaders_map_to_alpha_modes() {
        use rts_lockstep_engine::formats::w3d::mesh::{self as m, MeshLoadOptions};
        use rts_lockstep_engine::ww::chunkio::{ChunkLoad, GameFile};
        let model_of = |spec: &MeshSpec| {
            let bytes = spec.to_w3d();
            let mut c = ChunkLoad::new(GameFile::new(&bytes).unwrap());
            assert!(c.open_chunk().unwrap());
            MeshModel::from_mesh(&m::load_w3d(&mut c, &MeshLoadOptions::default()).unwrap().unwrap()).unwrap()
        };
        let desc = |spec: &MeshSpec| {
            let model = model_of(spec);
            let g = model.polygon_groups().unwrap();
            material_desc(&model, &g[0])
        };
        let mut spec = MeshSpec::quad(b"Q", b"Q");
        let d = desc(&spec);
        assert_eq!(d.alpha_mode, AlphaMode::Blend);
        assert_eq!(d.base_color, [1.0, 128.0 / 255.0, 0.0, 1.0]);
        spec.shader[12] = 1; // AlphaTest
        assert_eq!(desc(&spec).alpha_mode, AlphaMode::Mask(96.0 / 255.0));
        spec.shader[12] = 0;
        spec.attributes = rts_lockstep_engine::formats::w3d::w3d_file::W3D_MESH_FLAG_TWO_SIDED;
        assert_eq!(desc(&spec).cull_mode, None);
        spec.shader[3] = 1; // DestBlend ONE
        spec.shader[7] = 1; // SrcBlend ONE
        assert_eq!(desc(&spec).alpha_mode, AlphaMode::Add);
    }
}
