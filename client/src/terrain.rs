// SPDX-License-Identifier: GPL-3.0-only
//! The terrain as the game draws it (#11): what to draw is decided here from the `World` and the game's files, in plain
//! Rust ([`terrain_look`], tested without Bevy); the meshes and the atlas image are made from it with Bevy
//! ([`spawn_terrain`], feature `app`). The translation (`WorldHeightMap`'s tiles, atlas and UVs, `updateVB`'s vertices)
//! is the engine's `client::terrain_height_map`; this module only reads and draws.
//!
//! **New, not translated:** upstream draws the base layer and the blend layer as two passes over the same vertex buffer
//! (stage 0 the base UVs, stage 1 the blend UVs with the vertex alpha), and the third layer as its own pass
//! (`renderExtraBlendTiles`). Here they are three meshes over the same positions: the base, opaque; the blend cells, with
//! the blend UVs and the alpha as a vertex colour, alpha-blended; the third-layer cells likewise. Each later layer has a
//! larger depth bias so it wins the depth test over the identical surface below, as upstream's later passes draw with
//! `ZFUNC` less-or-equal. The vertex lighting `doTheLight` bakes in is Bevy's lighting of the vertex normals.
//!
//! Three looks, from most to least:
//! - **textured**: a height map with its `BlendTileData` and at least one tile read from `Art/Terrain/`;
//! - **shaded**: a height map without textures (no art, no `BlendTileData`, or no tile found): the same vertices, one
//!   colour;
//! - **flat**: no height map (no map loaded): a plane at height 0.
//!
//! The logic's ground height is untouched: objects stand where the logic puts them (`view::snapshot` reads their
//! positions), on the logic's own height map. Upstream's drawn surface and its logic height differ on a cell whose
//! diagonal is flipped for a blend (see the engine module's quirks); that is kept.

use rts_lockstep_engine::client::terrain_height_map::{ExtraBlendCell, TerrainAtlas, TerrainColors, TerrainHeightMap, TerrainVertex};
use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice};
use rts_lockstep_engine::logic::world::game_logic::World;

/// What the terrain is drawn as.
pub enum TerrainLook {
    /// No height map: a flat plane.
    Flat,
    /// A height map without textures: `updateVB`'s vertices, untextured.
    Shaded { vertices: Vec<TerrainVertex> },
    /// The game's look: the vertices with both UV sets, the atlas, and the third-layer cells; and (#35) the tiles' colours
    /// for the radar (`getTerrainColorAt`).
    Textured { vertices: Vec<TerrainVertex>, atlas: TerrainAtlas, extra: Vec<ExtraBlendCell>, colors: TerrainColors },
}

impl TerrainLook {
    /// (#35) The radar's terrain colours: the tiles' when the terrain is textured; none otherwise (the radar's terrain is
    /// then black shaded by height, as upstream's colour of a missing tile).
    pub fn radar_colors(&self) -> Option<&TerrainColors> {
        match self {
            TerrainLook::Textured { colors, .. } => Some(colors),
            _ => None,
        }
    }

    /// "flat", "shaded" or "textured", for the HUD and the tests.
    pub fn name(&self) -> &'static str {
        match self {
            TerrainLook::Flat => "flat",
            TerrainLook::Shaded { .. } => "shaded",
            TerrainLook::Textured { .. } => "textured",
        }
    }
}

/// [`terrain_look`]'s answer: the look, or that a texture's bytes are still being fetched (the web's ranged reads, #24):
/// ask again later.
pub enum TerrainBuild {
    Ready(TerrainLook),
    Waiting,
}

/// What the terrain of `world` is drawn as, reading the texture classes' Targas from `fs` (`Art/Terrain/<texture>`,
/// the names from the world's `Terrain.ini`). Reads only.
pub fn terrain_look<D: LocalDevice>(world: &World, fs: Option<&FileSystem<D>>) -> TerrainBuild {
    let Some(map) = world.terrain.logic_height_map() else { return TerrainBuild::Ready(TerrainLook::Flat) };
    let gd = world.statics.global_data.the_global_data();
    let use_3way = gd.map_or(1, |g| g.use_3way_terrain_blends);
    let adjust = gd.is_some_and(|g| g.adjust_cliff_textures);
    let shaded = || TerrainBuild::Ready(TerrainLook::Shaded { vertices: TerrainHeightMap::heights_only(map).update_vb() });
    let Ok(mut hm) = TerrainHeightMap::from_logic(map, use_3way, adjust) else { return shaded() };
    let Some(fs) = fs else { return shaded() };
    let mut waiting = false;
    hm.read_tex_classes(&world.statics.terrain_types, &mut |path| {
        let before = fs.local.device.unserved_reads();
        let r = fs.open_file(path);
        // (an unserved read of a loose file opens as empty, of an archive member as `None`: either way the device
        // counted it)
        if fs.local.device.unserved_reads() != before {
            waiting = true;
            return None;
        }
        r
    });
    if waiting {
        return TerrainBuild::Waiting;
    }
    if !hm.has_tiles() {
        return shaded();
    }
    let atlas = hm.build_terrain_texture();
    TerrainBuild::Ready(TerrainLook::Textured { vertices: hm.update_vb(), atlas, extra: hm.extra_blend_cells(), colors: hm.terrain_colors() })
}

#[cfg(feature = "app")]
pub use bevy_side::*;

#[cfg(feature = "app")]
mod bevy_side {
    use super::*;
    use crate::view::to_bevy;
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::mesh::{Indices, PrimitiveTopology};
    use bevy::prelude::*;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    use rts_lockstep_engine::client::terrain_height_map::TERRAIN_CELL_INDICES;

    /// Marks the terrain's entities.
    #[derive(Component)]
    pub struct Ground;

    /// The atlas (`TerrainTextureClass`, A1R5G5B5 widened to RGBA8) as an sRGB image with 3 mip levels
    /// (`MIP_LEVELS_3`, box filtered as `D3DXFilterTexture`'s `D3DX_FILTER_BOX`), clamped.
    pub fn atlas_image(atlas: &TerrainAtlas) -> Image {
        let (w, h) = (atlas.width as u32, atlas.height as u32);
        let mut data = atlas.to_rgba8();
        let mut level = data.clone();
        let (mut lw, mut lh) = (w, h);
        let mut levels = 1;
        while levels < 3 && lw > 1 && lh > 1 {
            let (nw, nh) = (lw / 2, lh / 2);
            let mut next = vec![0u8; (nw * nh * 4) as usize];
            for y in 0..nh {
                for x in 0..nw {
                    for c in 0..4 {
                        let p = |xx: u32, yy: u32| u32::from(level[((yy * lw + xx) * 4 + c) as usize]);
                        let s = p(2 * x, 2 * y) + p(2 * x + 1, 2 * y) + p(2 * x, 2 * y + 1) + p(2 * x + 1, 2 * y + 1);
                        next[((y * nw + x) * 4 + c) as usize] = ((s + 2) / 4) as u8;
                    }
                }
            }
            data.extend_from_slice(&next);
            level = next;
            (lw, lh) = (nw, nh);
            levels += 1;
        }
        let mut image = Image::new(
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            TextureDimension::D2,
            atlas.to_rgba8(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.data = Some(data);
        image.texture_descriptor.mip_level_count = levels;
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            ..default()
        });
        image
    }

    /// A triangle list over `cells` cells of four vertices each, drawn with `indices(cell)` (each cell's own pattern).
    fn cell_mesh(positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, uvs: Option<Vec<[f32; 2]>>, colors: Option<Vec<[f32; 4]>>, indices: Vec<u32>) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        if let Some(uv) = uvs {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        }
        if let Some(c) = colors {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, c);
        }
        mesh.insert_indices(Indices::U32(indices));
        mesh
    }

    fn pos(v: &TerrainVertex) -> [f32; 3] {
        to_bevy([v.x, v.y, v.z])
    }

    fn nrm(v: &TerrainVertex) -> [f32; 3] {
        to_bevy(v.normal)
    }

    /// The base layer: every cell, the base UVs.
    pub fn base_mesh(vertices: &[TerrainVertex], textured: bool) -> Mesh {
        let indices = (0..vertices.len() as u32 / 4).flat_map(|c| TERRAIN_CELL_INDICES.map(|i| i + 4 * c)).collect();
        let uvs = textured.then(|| vertices.iter().map(|v| [v.u1, v.v1]).collect());
        cell_mesh(vertices.iter().map(pos).collect(), vertices.iter().map(nrm).collect(), uvs, None, indices)
    }

    /// The blend layer: the cells with some alpha, the blend UVs, the alpha as the vertex colour's.
    pub fn blend_mesh(vertices: &[TerrainVertex]) -> Option<Mesh> {
        let cells: Vec<&[TerrainVertex]> = vertices.chunks_exact(4).filter(|c| c.iter().any(|v| v.alpha != 0)).collect();
        if cells.is_empty() {
            return None;
        }
        let flat: Vec<&TerrainVertex> = cells.iter().flat_map(|c| c.iter()).collect();
        let indices = (0..cells.len() as u32).flat_map(|c| TERRAIN_CELL_INDICES.map(|i| i + 4 * c)).collect();
        Some(cell_mesh(
            flat.iter().map(|v| pos(v)).collect(),
            flat.iter().map(|v| nrm(v)).collect(),
            Some(flat.iter().map(|v| [v.u2, v.v2]).collect()),
            Some(flat.iter().map(|v| [1.0, 1.0, 1.0, f32::from(v.alpha) / 255.0]).collect()),
            indices,
        ))
    }

    /// The third layer (`renderExtraBlendTiles`): its cells, each with its own diagonal; the normals are the base
    /// vertices' at the same corners (upstream's are zero: its pass is lit by the static diffuse).
    pub fn extra_mesh(extra: &[ExtraBlendCell], vertices: &[TerrainVertex], width: i32) -> Option<Mesh> {
        if extra.is_empty() {
            return None;
        }
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        let mut indices = Vec::new();
        for (k, c) in extra.iter().enumerate() {
            let base = vertices.get(((c.y * (width - 1) + c.x) * 4) as usize..((c.y * (width - 1) + c.x) * 4 + 4) as usize);
            for q in 0..4 {
                let p = c.positions[q];
                positions.push(to_bevy(p));
                let n = base.and_then(|b| b.iter().find(|v| v.x == p[0] && v.y == p[1])).map_or([0.0, 0.0, 1.0], |v| v.normal);
                normals.push(to_bevy(n));
                uvs.push([c.u[q], c.v[q]]);
                colors.push([1.0, 1.0, 1.0, f32::from(c.alpha[q]) / 255.0]);
            }
            indices.extend(c.indices().map(|i| i + 4 * k as u32));
        }
        Some(cell_mesh(positions, normals, Some(uvs), Some(colors), indices))
    }

    /// Spawns the terrain (marked [`Ground`]) as `look` says; `width` is the height map's (the extra layer's lookup).
    pub fn spawn_terrain(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>, look: &TerrainLook, width: i32) {
        match look {
            TerrainLook::Flat => {
                commands.spawn((
                    Ground,
                    Mesh3d(meshes.add(Plane3d::default().mesh().size(400.0, 400.0))),
                    MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.42, 0.26), perceptual_roughness: 1.0, ..default() })),
                    Transform::from_xyz(200.0, 0.0, -200.0),
                ));
            }
            TerrainLook::Shaded { vertices } => {
                commands.spawn((
                    Ground,
                    Mesh3d(meshes.add(base_mesh(vertices, false))),
                    MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.42, 0.26), perceptual_roughness: 1.0, ..default() })),
                ));
            }
            TerrainLook::Textured { vertices, atlas, extra, .. } => {
                let image = images.add(atlas_image(atlas));
                let layer = |bias: f32, blend: bool| StandardMaterial {
                    base_color_texture: Some(image.clone()),
                    perceptual_roughness: 1.0,
                    reflectance: 0.0,
                    alpha_mode: if blend { AlphaMode::Blend } else { AlphaMode::Opaque },
                    depth_bias: bias,
                    ..default()
                };
                commands.spawn((Ground, Mesh3d(meshes.add(base_mesh(vertices, true))), MeshMaterial3d(materials.add(layer(0.0, false)))));
                if let Some(m) = blend_mesh(vertices) {
                    commands.spawn((Ground, Mesh3d(meshes.add(m)), MeshMaterial3d(materials.add(layer(1.0, true)))));
                }
                if let Some(m) = extra_mesh(extra, vertices, width) {
                    commands.spawn((Ground, Mesh3d(meshes.add(m)), MeshMaterial3d(materials.add(layer(2.0, true)))));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_lockstep_engine::common::file_system::MemoryDevice;

    /// Prediction: the gate map (the demo) has a height map and a `BlendTileData` whose classes name no terrain the
    /// demo's INI defines, so with no files it is "shaded", with `(w-1)*(h-1)*4` vertices whose heights are the logic's
    /// samples; with an empty file system likewise; and a world with no map loaded would be "flat" (not built here).
    #[test]
    fn the_demo_terrain_is_shaded_without_textures() {
        let (w, _) = crate::demo::demo_world();
        let map = w.terrain.logic_height_map().expect("the gate map has a height map");
        let (mw, mh) = (map.x_extent(), map.y_extent());
        let TerrainBuild::Ready(look) = terrain_look::<MemoryDevice>(&w, None) else { panic!("waiting without files") };
        assert_eq!(look.name(), "shaded");
        let TerrainLook::Shaded { vertices } = look else { unreachable!() };
        assert_eq!(vertices.len(), ((mw - 1) * (mh - 1) * 4) as usize);
        let b = map.border_size();
        let v = &vertices[0];
        assert_eq!((v.x, v.y), ((-b) as f32 * 10.0, (-b) as f32 * 10.0));
        assert_eq!(v.z, f32::from(map.data()[0]) * 0.625);
        let fs = FileSystem::new(MemoryDevice::new());
        let TerrainBuild::Ready(look) = terrain_look(&w, Some(&fs)) else { panic!("waiting") };
        assert_eq!(look.name(), "shaded");
    }

    /// A 128x128 24-bit Targa (2 x 2 tiles).
    fn grass_tga() -> Vec<u8> {
        let mut b = vec![0u8; 18];
        b[2] = 2;
        b[12] = 128;
        b[14] = 128;
        b[16] = 24;
        for row in 0..128u32 {
            for col in 0..128u32 {
                b.extend_from_slice(&[40, (100 + row / 4) as u8, (col / 2) as u8]);
            }
        }
        b
    }

    /// The gate world given a `BlendTileData` (one 2x2 class "TestGrass", every sample on it, a horizontal blend on
    /// every fourth cell) and a `Terrain.ini` entry naming `TGrass.tga`.
    fn textured_world() -> World {
        use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
        use rts_lockstep_engine::common::terrain_types::TerrainTypeCollection;
        use rts_lockstep_engine::logic::terrain_visual::{BlendTiles, TextureClass};
        let (mut w, _) = crate::demo::demo_world();
        let mut map = w.terrain.logic_height_map().expect("a height map").clone();
        let (mw, mh) = (map.x_extent(), map.y_extent());
        let n = (mw * mh) as usize;
        let mut rest = Vec::new();
        rest.extend_from_slice(&0i32.to_le_bytes());
        rest.extend_from_slice(&0i32.to_le_bytes());
        rest.extend_from_slice(&4i32.to_le_bytes()); // blend tile 1: tile 1's quarter 0
        rest.extend_from_slice(&[1, 0, 0, 0, 0, 0]);
        rest.extend_from_slice(&(-1i32).to_le_bytes());
        rest.extend_from_slice(&rts_lockstep_engine::client::terrain_height_map::FLAG_VAL.to_le_bytes());
        let tiles = (0..n).map(|k| (((k as i32 % mw) % 2 + 2 * ((k as i32 / mw) % 2)) << 2) as i16).collect();
        map.set_blend_tiles(Some(BlendTiles {
            version: 7,
            blend_tile_ndxes: (0..n).map(|k| i16::from(k % 4 == 0)).collect(),
            extra_blend_tile_ndxes: vec![0; n],
            cliff_info_ndxes: vec![0; n],
            num_bitmap_tiles: 4,
            num_blended_tiles: 2,
            num_cliff_info: 1,
            rest,
            tile_ndxes: tiles,
            cell_cliff_state: vec![0; ((mw + 7) / 8 * mh) as usize],
            flip_state_width: (mw + 7) / 8,
            texture_classes: vec![TextureClass { first_tile: 0, num_tiles: 4, width: 2, name: b"TestGrass".to_vec() }],
        }));
        w.terrain.set_logic_height_map(Some(map));
        fn block(ini: &mut Ini<'_>, c: &mut TerrainTypeCollection) -> IniResult<()> {
            c.parse_terrain_definition(ini)
        }
        let blocks = [BlockParse { token: "Terrain", parse: block }];
        let statics = std::sync::Arc::make_mut(&mut w.statics);
        Ini::load(b"Terrain.ini", b"Terrain TestGrass\n  Texture = TGrass.tga\nEnd\n", IniLoadType::Overwrite, None, &blocks, &mut statics.terrain_types).expect("Terrain.ini");
        w
    }

    /// Prediction: with the class's Targa at `Art/Terrain/TGrass.tga` the look is "textured": the class is placed at
    /// (4, 4) in a 2048 x 256 atlas, every cell's base UVs are inside the class's square (u in [4, 132] / 2048, v in [4,
    /// 132] / 256), a quarter of the cells blend (alpha 255 on corners 1 and 2), and there is no third layer. Without the
    /// file it is "shaded"; through a device that has not yet read it, "waiting".
    #[test]
    fn a_map_with_tiles_and_their_textures_is_textured() {
        let w = textured_world();
        let mut dev = MemoryDevice::new();
        dev.insert(b"Art/Terrain/TGrass.tga", grass_tga());
        let fs = FileSystem::new(dev);
        let TerrainBuild::Ready(look) = terrain_look(&w, Some(&fs)) else { panic!("waiting") };
        let TerrainLook::Textured { vertices, atlas, extra, .. } = look else { panic!("{}", look.name()) };
        assert_eq!((atlas.width, atlas.height), (2048, 256));
        assert!(extra.is_empty());
        for v in &vertices {
            assert!(v.u1 >= 4.0 / 2048.0 && v.u1 <= 132.0 / 2048.0 && v.v1 >= 4.0 / 256.0 && v.v1 <= 132.0 / 256.0, "{v:?}");
        }
        let blended = vertices.chunks_exact(4).filter(|c| c.iter().any(|v| v.alpha != 0)).count();
        let cells = vertices.len() / 4;
        let map = w.terrain.logic_height_map().unwrap();
        let expected = (0..map.y_extent() - 1).flat_map(|y| (0..map.x_extent() - 1).map(move |x| (y, x))).filter(|&(y, x)| (y * map.x_extent() + x) % 4 == 0).count();
        assert_eq!(blended, expected, "of {cells}");
        let TerrainBuild::Ready(look) = terrain_look(&w, Some(&FileSystem::new(MemoryDevice::new()))) else { panic!("waiting") };
        assert_eq!(look.name(), "shaded");
        // the web's ranged reads (#24): known by length, its bytes not yet arrived
        use rts_lockstep_engine::common::file_system::RangedDevice;
        let tga = grass_tga();
        let mut dev = RangedDevice::new();
        dev.insert_ranged(b"Art/Terrain/TGrass.tga", tga.len(), 7, Vec::new());
        let mut fs = FileSystem::new(dev);
        assert!(matches!(terrain_look(&w, Some(&fs)), TerrainBuild::Waiting));
        for (key, o, l) in fs.local.device.take_wanted() {
            assert_eq!(key, 7);
            fs.local.device.provide(key, o, tga[o..o + l].to_vec());
        }
        let TerrainBuild::Ready(look) = terrain_look(&w, Some(&fs)) else { panic!("still waiting") };
        assert_eq!(look.name(), "textured");
    }

    /// The client never changes the simulation: the gate game with the terrain's look built after every frame gives
    /// exactly the CRCs of the gate game without (as `view`'s snapshot test).
    #[test]
    fn building_the_terrain_leaves_the_crcs_alone() {
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        use rts_lockstep_engine::logic::world::gate_scenario::{gate_world, run_gate_scenario};
        let frames = 60;
        let plain = run_gate_scenario(frames);
        let mut w = gate_world();
        let fs = FileSystem::new(MemoryDevice::new());
        let mut crcs = Vec::new();
        for _ in 0..frames {
            w.logic_update().expect("a frame");
            let TerrainBuild::Ready(_) = terrain_look(&w, Some(&fs)) else { panic!("waiting") };
            crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        assert_eq!(crcs, plain);
    }

    /// A 128x128 24-bit Targa of one colour with a light grid every 16 pixels.
    fn tile_tga(rgb: [u8; 3]) -> Vec<u8> {
        let mut b = vec![0u8; 18];
        b[2] = 2;
        b[12] = 128;
        b[14] = 128;
        b[16] = 24;
        for row in 0..128u32 {
            for col in 0..128u32 {
                let k = if row % 16 == 0 || col % 16 == 0 { 30 } else { 0 };
                b.extend_from_slice(&[rgb[2].saturating_add(k), rgb[1].saturating_add(k), rgb[0].saturating_add(k)]);
            }
        }
        b
    }

    /// `map` (a chunk file) with a `BlendTileData` chunk (version 8) added: its label appended to the table of contents and
    /// the chunk to the body. Two classes, `TestGrass` (tiles 0..3) and `TestRock` (4..7), each 2 tiles wide; every
    /// sample on the grass, rock on the samples with `x >= w/2`, and a horizontal blend of rock onto grass along `x =
    /// w/2 - 1`.
    fn with_blend_tile_data(map: &[u8], w: i32, h: i32) -> Vec<u8> {
        assert_eq!(&map[..4], b"CkMp");
        let count = i32::from_le_bytes(map[4..8].try_into().unwrap());
        let mut p = 8;
        let mut max_id = 0;
        for _ in 0..count {
            let len = usize::from(map[p]);
            p += 1 + len;
            max_id = max_id.max(i32::from_le_bytes(map[p..p + 4].try_into().unwrap()));
            p += 4;
        }
        let id = max_id + 1;
        let mut d = Vec::new();
        let int = |d: &mut Vec<u8>, v: i32| d.extend_from_slice(&v.to_le_bytes());
        let n = (w * h) as usize;
        int(&mut d, w * h);
        let tile = |x: i32, y: i32| -> i16 {
            let base = if x >= w / 2 { 4 } else { 0 };
            ((base + (x % 2) + 2 * (y % 2)) << 2) as i16
        };
        let shorts = |d: &mut Vec<u8>, f: &dyn Fn(i32, i32) -> i16| {
            for k in 0..n as i32 {
                d.extend_from_slice(&f(k % w, k / w).to_le_bytes());
            }
        };
        shorts(&mut d, &|x, y| tile(x, y));
        shorts(&mut d, &|x, _| i16::from(x == w / 2 - 1));
        shorts(&mut d, &|_, _| 0);
        shorts(&mut d, &|_, _| 0);
        d.extend(std::iter::repeat_n(0u8, (h * ((w + 7) / 8)) as usize));
        int(&mut d, 8); // numBitmapTiles
        int(&mut d, 2); // numBlendedTiles
        int(&mut d, 1); // numCliffInfo
        int(&mut d, 2); // classes
        for (first, name) in [(0, &b"TestGrass"[..]), (4, b"TestRock")] {
            for v in [first, 4, 2, 0] {
                int(&mut d, v);
            }
            d.extend_from_slice(&(name.len() as u16).to_le_bytes());
            d.extend_from_slice(name);
        }
        int(&mut d, 0); // edge tiles
        int(&mut d, 0); // edge classes
        int(&mut d, 4 << 2); // blend tile 1: rock, horizontal
        d.extend_from_slice(&[1, 0, 0, 0, 0, 0]);
        int(&mut d, -1);
        int(&mut d, rts_lockstep_engine::client::terrain_height_map::FLAG_VAL);
        let mut out = b"CkMp".to_vec();
        out.extend_from_slice(&(count + 1).to_le_bytes());
        out.extend_from_slice(&map[8..p]);
        out.push(13);
        out.extend_from_slice(b"BlendTileData");
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&map[p..]);
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&8u16.to_le_bytes());
        out.extend_from_slice(&(d.len() as i32).to_le_bytes());
        out.extend_from_slice(&d);
        out
    }

    /// #11, not a check unless asked: with `RTS_TERRAIN_INSTALL_FROM=<an install>` (the test install, with or without
    /// art) and `RTS_WRITE_TERRAIN_INSTALL=<dir>`, writes that install with terrain: its map given a `BlendTileData`
    /// ([`with_blend_tile_data`]), `Terrain.ini` naming `TGrass.tga` and `TRock.tga`, and `TerrainZH.big` holding them.
    /// Then starts it from its files as the page does (`TerrainZH.big` by range) and builds the terrain.
    ///
    /// *Predictions:* the game starts on Desert Duel; the first build waits (the Targas' bytes are not there yet); once
    /// the wanted ranges are given, the look is "textured", with 29 x 29 cells, 29 of them blended (the column `x = 14`),
    /// and the atlas 2048 x 256 (two 2-wide classes in one row of the grid).
    ///
    /// *Results (2026-11-05, from the art test install):* as predicted: waited once, then 841 cells, 29 blended, the
    /// atlas 2048 x 256.
    #[test]
    fn write_the_terrain_install_when_asked() {
        use crate::import::{start_from_files, Imported};
        use rts_lockstep_engine::formats::big::{build_big, ArchiveFile, FilenameList};
        let (Ok(from), Ok(to)) = (std::env::var("RTS_TERRAIN_INSTALL_FROM"), std::env::var("RTS_WRITE_TERRAIN_INSTALL")) else { return };
        let (from, to) = (std::path::Path::new(&from), std::path::Path::new(&to));
        std::fs::create_dir_all(to).expect("the folder");
        let rebuild = |name: &str, f: &dyn Fn(&[u8], Vec<u8>) -> Vec<u8>| -> Vec<u8> {
            let a = ArchiveFile::open_archive_file(name.as_bytes(), std::fs::read(from.join(name)).expect(name)).expect("an archive");
            let mut list = FilenameList::new();
            a.get_file_list_in_directory(b"", b"", b"*", &mut list, true);
            let members: Vec<(Vec<u8>, Vec<u8>)> = list.iter().map(|n| (n.to_vec(), f(n, a.open_file(n).expect("a member").to_vec()))).collect();
            let refs: Vec<(&[u8], &[u8])> = members.iter().map(|(p, b)| (p.as_slice(), b.as_slice())).collect();
            build_big(&refs)
        };
        let ini = rebuild("INIZH.big", &|n, b| {
            if n.eq_ignore_ascii_case(b"data\\ini\\terrain.ini") {
                let mut t = b;
                t.extend_from_slice(b"\nTerrain TestGrass\n  Texture = TGrass.tga\nEnd\nTerrain TestRock\n  Texture = TRock.tga\nEnd\n");
                t
            } else {
                b
            }
        });
        let maps = rebuild("MapsZH.big", &|n, b| if n.to_ascii_lowercase().ends_with(b".map") { with_blend_tile_data(&b, 30, 30) } else { b });
        let terrain = build_big(&[(b"Art\\Terrain\\TGrass.tga", &tile_tga([70, 120, 50])), (b"Art\\Terrain\\TRock.tga", &tile_tga([120, 100, 80]))]);
        let mut archives: Vec<(String, Vec<u8>)> = vec![("INIZH.big".into(), ini), ("MapsZH.big".into(), maps), ("TerrainZH.big".into(), terrain)];
        for e in std::fs::read_dir(from).expect("the install") {
            let n = e.expect("an entry").file_name().into_string().expect("a name");
            if n.ends_with(".big") && !archives.iter().any(|(a, _)| *a == n) {
                archives.push((n.clone(), std::fs::read(from.join(&n)).expect("read")));
            }
        }
        for (n, b) in &archives {
            std::fs::write(to.join(n), b).expect("written");
        }
        let files = archives
            .iter()
            .enumerate()
            .map(|(key, (n, b))| {
                // the page's prefix: as much as the directory read asks for (`page_prefix` in `import`'s tests)
                let mut q = b.len().min(64 * 1024);
                while ArchiveFile::directory_prefix_needed(b.len(), &b[..q]) > q {
                    q = ArchiveFile::directory_prefix_needed(b.len(), &b[..q]);
                }
                let f = if n.starts_with("Terrain") { Imported::Ranged { key: key as u32, len: b.len(), prefix: b[..q].to_vec() } } else { Imported::Whole { key: key as u32, bytes: b.clone() } };
                (n.as_bytes().to_vec(), f)
            })
            .collect();
        let (s, mut fs) = start_from_files(files, None).expect("the game starts");
        assert!(s.report.contains("Desert Duel"), "{}", s.report);
        let mut waits = 0;
        let look = loop {
            match terrain_look(&s.world, Some(&fs)) {
                TerrainBuild::Ready(l) => break l,
                TerrainBuild::Waiting => {
                    waits += 1;
                    assert!(waits < 10, "still waiting");
                    for (key, o, l) in fs.local.device.take_wanted() {
                        let b = &archives[key as usize].1;
                        fs.local.device.provide(key, o, b[o..o + l].to_vec());
                    }
                }
            }
        };
        let TerrainLook::Textured { vertices, atlas, .. } = look else { panic!("{}", look.name()) };
        let blended = vertices.chunks_exact(4).filter(|c| c.iter().any(|v| v.alpha != 0)).count();
        println!("terrain: waited {waits} times; {} cells, {blended} blended; atlas {}x{}", vertices.len() / 4, atlas.width, atlas.height);
        assert_eq!((vertices.len() / 4, blended, atlas.height), (29 * 29, 29, 256));
    }
}
