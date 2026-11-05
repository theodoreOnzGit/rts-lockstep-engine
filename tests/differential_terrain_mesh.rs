// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of the terrain's client side (#11, `client::terrain_height_map`) against upstream's own
//! `WorldHeightMap.cpp`, `TerrainTex.cpp` and `HeightMap.cpp` (commit 0a05454d), cut verbatim by `build.rs` and driven
//! by `differential/wrap_terrain_mesh.cpp`.
//!
//! No game files are used: every map is **synthetic**. Each case makes a height map (3 to 70 samples a side, so the
//! vertex buffers' 32-cell tiles are crossed; a border of 0 to 3; flat, smooth, terraced or random heights), one to six
//! texture classes (1 to 4 tiles wide, sometimes 5 to 7; a tile count that is the square, or fewer, or more), or in one
//! case of ten 20 to 60 classes (1 to 4 wide), which crowd the atlas's grid until some find no room, and up to two
//! edge classes, a Targa per class (24 or 32 bits, raw or run-length, sometimes a half tile, sometimes smaller or larger
//! than the class, sometimes cut short, sometimes no file, sometimes no such terrain), the tile of every sample (a quarter
//! of a tile of a class, or of no tile), up to 24 blend tiles (every flag, inverted 0 to 3, a custom edge class or not),
//! the blend and extra blend index of every sample (some past the table, which the parse patches to 0), up to 12 cliff
//! infos and their indices, and `m_adjustCliffTextures` on in three cases of four.
//!
//! Both sides get the same map: the Rust as a `BlendTileData` chunk's parse (`BlendTiles`, with the edge classes, blend
//! tiles and cliff infos as the chunk's remaining bytes) through `TerrainHeightMap::from_logic`; the C++ as
//! `WorldHeightMap`'s members after `ParseBlendTileData` and the constructor's patch (done here, as the test's own
//! transcription of that loop). Then on both: `readTexClass` for every class, `getTerrainTexture`, and compared:
//! `m_terrainTexHeight`, `m_alphaEdgeHeight`, the number of cell flip bits set, every class's position, every source and
//! edge tile's presence and location, **the atlas word for word**, and for every cell `getUVData`'s result and UVs,
//! `getAlphaUVData`'s UVs, alphas and flip, `getExtraAlphaUVData`'s result, UVs, alphas, flip and cliff, and
//! **`updateVB`'s four vertices** (position, normal, both UV sets, as bit patterns, and the alpha).
//!
//! Inputs undefined upstream are not generated (a tile index outside `m_sourceTiles`, counts past upstream's arrays).
//!
//! # Predictions (written before the first run)
//!
//! 0 mismatches. Most likely to differ, if anything: the cliff UV stretch (`DO_OLD_UV`: float order of operations and
//! the integer `vFactor`) and the vertex normals (`Normalized_Cross_Product`); least likely: the placement and the
//! atlas, which are integer.
//!
//! # Results (2026-11-05, `cargo test --release --features differential --test differential_terrain_mesh`)
//!
//! - **First run: 235,104 mismatches**, every one a cell flip: the wrapper had not defined `FLIP_TRIANGLES`, which
//!   upstream's `WorldHeightMap.cpp` gets from `HeightMap.h`, so the C++ never set `*flip` (and `getTerrainTexture`
//!   read an uninitialised `flipForBlend`). A wrapper defect, not a translation one; with the define (the wrapper's
//!   header says so):
//! - 400 cases (the default): 224,976 cells, 899,904 vertices, 424,411,136 atlas words, 12,831 tiles read, 31,593
//!   cells stretched for cliffs, 49,476 flipped, 23,534 with a third layer: **0 mismatches**.
//! - `TERRAIN_CASES=3000`: 1,701,051 cells, 6,804,204 vertices, 3,138,387,968 atlas words, 90,478 tiles, 238,699
//!   stretched, 373,053 flipped, 175,149 with a third layer: **0 mismatches** (27 s).
//! - Planted bugs (each put in the Rust by hand, run on the 400 cases, then removed): the last column not wrapping in
//!   `getXWithOrigin` (8,811 mismatches); `updateTileTexturePositions` looking past a row's first free cell (152, once
//!   crowded cases were added: before them the generator never filled the grid, and the mutant passed); cliff info
//!   applied whatever its tile's class (`tilesMatch = true`, 102,537); the atlas's left border copied from the class's
//!   near side (351). Two more were not caught and are equivalent on every input upstream can have: `vFactor` in
//!   float (the atlas height is a power of two up to 2048, so the integer division is exact), and the stretch limit
//!   as `<=` (`sqrt(1 + d*d)` is never exactly 1.5 for `d` a multiple of 1/16).

#![cfg(feature = "differential")]

use rts_lockstep_engine::client::terrain_height_map::*;
use rts_lockstep_engine::common::base_type::ICoord2D;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::terrain_visual::{BlendTiles, LogicHeightMap, TextureClass};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1) as u32) as i32
    }
    fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }
}

/// A Targa of `tw` x `th` pixels: 24 or 32 bits, raw or run-length (random packets), random colours with runs.
fn targa(r: &mut Rng, tw: i32, th: i32) -> Vec<u8> {
    let bpp = if r.chance(1, 2) { 3 } else { 4 };
    let rle = r.chance(1, 3);
    let mut b = vec![0u8; 18];
    b[2] = if rle { 0xA } else { 2 };
    b[12..14].copy_from_slice(&(tw as i16).to_le_bytes());
    b[14..16].copy_from_slice(&(th as i16).to_le_bytes());
    b[16] = (bpp * 8) as u8;
    let n = (tw * th) as usize;
    let mut px = Vec::with_capacity(n);
    let mut cur = [0u8; 4];
    for _ in 0..n {
        if r.chance(1, 4) {
            cur = [r.next() as u8, r.next() as u8, r.next() as u8, r.next() as u8];
        }
        px.push(cur);
    }
    if !rle {
        for p in &px {
            b.extend_from_slice(&p[..bpp]);
        }
    } else {
        let mut i = 0;
        while i < n {
            let len = (r.range(1, 128) as usize).min(n - i);
            if r.chance(1, 2) {
                b.push(0x80 | (len - 1) as u8);
                b.extend_from_slice(&px[i][..bpp]);
            } else {
                b.push((len - 1) as u8);
                for p in &px[i..i + len] {
                    b.extend_from_slice(&p[..bpp]);
                }
            }
            i += len;
        }
    }
    // cut short (raw files only, after the first pixel: upstream's `flag` and `buf` are read uninitialised otherwise)
    if !rle && r.chance(1, 10) {
        let cut = 18 + bpp + r.below((b.len() - 18 - bpp) as u32) as usize;
        b.truncate(cut);
    }
    b
}

struct Case {
    w: i32,
    h: i32,
    border: i32,
    data: Vec<u8>,
    tiles: Vec<i16>,
    blend: Vec<i16>,
    extra: Vec<i16>,
    cliff_ndx: Vec<i16>,
    classes: Vec<(i32, i32, i32)>,
    edge_classes: Vec<(i32, i32, i32)>,
    blends: Vec<BlendTileInfo>,
    cliffs: Vec<CliffInfo>,
    /// Per class then edge class: `Some(file)`, or `None` with -1 (no terrain) or -2 (no file).
    files: Vec<(i32, Vec<u8>)>,
    num_bitmap_tiles: i32,
    adjust: bool,
}

fn make_case(r: &mut Rng) -> Case {
    let big = r.chance(1, 5);
    let w = if big { r.range(30, 70) } else { r.range(3, 24) };
    let h = if big { r.range(30, 70) } else { r.range(3, 24) };
    let n = (w * h) as usize;
    let border = r.range(0, 3);
    let mode = r.below(4);
    let base = r.below(200) as i32;
    let data: Vec<u8> = (0..n)
        .map(|k| {
            let (x, y) = ((k as i32) % w, (k as i32) / w);
            match mode {
                0 => base as u8,
                1 => (base + ((x * 7 + y * 3) % 40)).clamp(0, 255) as u8,
                2 => (if (x / 3 + y / 4) % 2 == 0 { base } else { base + 40 }).clamp(0, 255) as u8,
                _ => r.next() as u8,
            }
        })
        .collect();
    let mut classes = Vec::new();
    let mut first = 0;
    // one case in ten is crowded: 20 to 60 classes, so the atlas's grid fills and some classes find no room
    let nclasses = if r.chance(1, 10) { r.range(20, 60) } else { r.range(1, 6) };
    for _ in 0..nclasses {
        let width = if nclasses <= 6 && r.chance(1, 8) { r.range(5, 7) } else { r.range(1, 4) };
        let num = match r.below(6) {
            0 => (width * width - r.range(1, 2)).max(1),
            1 => width * width + r.range(1, 3),
            _ => width * width,
        };
        if first + num.max(width * width) > 1000 {
            break; // (m_sourceTiles holds 1024)
        }
        classes.push((first, num, width));
        first += num;
    }
    let num_bitmap_tiles = first;
    let mut edge_classes = Vec::new();
    let mut efirst = 0;
    for _ in 0..r.below(3) {
        let width = r.range(1, 2);
        edge_classes.push((efirst, width * width, width));
        efirst += width * width;
    }
    let mut files = Vec::new();
    for &(_, num, width) in classes.iter().chain(edge_classes.iter()) {
        let _ = num;
        match r.below(12) {
            0 => files.push((-1, Vec::new())),
            1 => files.push((-2, Vec::new())),
            2 => {
                let f = targa(r, 32, 32);
                files.push((f.len() as i32, f))
            }
            _ => {
                let side = (width + r.range(-1, 1)).clamp(1, 10) * TILE_PIXEL_EXTENT;
                let th = if r.chance(1, 6) { side + 64 } else { side };
                let f = targa(r, side, th);
                files.push((f.len() as i32, f))
            }
        }
    }
    let rand_tile = |r: &mut Rng| -> i16 {
        let t = if r.chance(1, 20) { r.range(0, 1023) } else { r.range(0, num_bitmap_tiles - 1) };
        ((t << 2) | r.range(0, 3)) as i16
    };
    let tiles: Vec<i16> = (0..n).map(|_| rand_tile(r)).collect();
    let nb = r.range(1, 24);
    let mut blends = vec![BlendTileInfo::default()];
    for _ in 1..nb {
        let mut t = BlendTileInfo { blend_ndx: i32::from(rand_tile(r)), custom_blend_edge_class: -1, ..Default::default() };
        match r.below(5) {
            0 => t.horiz = 1,
            1 => t.vert = 1,
            2 => t.right_diagonal = 1,
            3 => t.left_diagonal = 1,
            _ => {
                t.horiz = r.below(2) as u8;
                t.vert = r.below(2) as u8;
                t.right_diagonal = r.below(2) as u8;
                t.left_diagonal = r.below(2) as u8;
            }
        }
        t.inverted = r.below(4) as u8;
        t.long_diagonal = r.below(2) as u8;
        if r.chance(1, 8) {
            t.custom_blend_edge_class = r.range(0, 2);
        }
        blends.push(t);
    }
    let nc = r.range(1, 12);
    let mut cliffs = vec![CliffInfo::default()];
    for _ in 1..nc {
        let f = |r: &mut Rng| (r.below(1000) as f32) / 8000.0 - 0.02;
        cliffs.push(CliffInfo {
            u0: f(r),
            v0: f(r),
            u1: f(r),
            v1: f(r),
            u2: f(r),
            v2: f(r),
            u3: f(r),
            v3: f(r),
            flip: r.chance(1, 2),
            mutant: r.chance(1, 2),
            tile_index: rand_tile(r),
        });
    }
    let idx = |r: &mut Rng, m: i32| -> i16 {
        if r.chance(1, 2) {
            0
        } else if r.chance(1, 20) {
            r.range(-3, m + 3) as i16
        } else {
            r.range(0, m - 1) as i16
        }
    };
    let blend = (0..n).map(|_| idx(r, nb)).collect();
    let extra = (0..n).map(|_| if r.chance(3, 4) { 0 } else { idx(r, nb) }).collect();
    let cliff_ndx = (0..n).map(|_| idx(r, nc)).collect();
    Case { w, h, border, data, tiles, blend, extra, cliff_ndx, classes, edge_classes, blends, cliffs, files, num_bitmap_tiles, adjust: r.chance(3, 4) }
}

/// The chunk's bytes after the texture classes (version 7): the edge classes, the blend tiles, the cliff infos.
fn rest(c: &Case) -> Vec<u8> {
    let mut b = Vec::new();
    let int = |b: &mut Vec<u8>, v: i32| b.extend_from_slice(&v.to_le_bytes());
    let edge_tiles: i32 = c.edge_classes.iter().map(|e| e.1).sum();
    int(&mut b, edge_tiles);
    int(&mut b, c.edge_classes.len() as i32);
    for (k, &(first, num, width)) in c.edge_classes.iter().enumerate() {
        int(&mut b, first);
        int(&mut b, num);
        int(&mut b, width);
        let name = format!("E{k}");
        b.extend_from_slice(&(name.len() as u16).to_le_bytes());
        b.extend_from_slice(name.as_bytes());
    }
    for t in &c.blends[1..] {
        int(&mut b, t.blend_ndx);
        b.extend_from_slice(&[t.horiz, t.vert, t.right_diagonal, t.left_diagonal, t.inverted, t.long_diagonal]);
        int(&mut b, t.custom_blend_edge_class);
        int(&mut b, FLAG_VAL);
    }
    for ci in &c.cliffs[1..] {
        int(&mut b, i32::from(ci.tile_index));
        for f in [ci.u0, ci.v0, ci.u1, ci.v1, ci.u2, ci.v2, ci.u3, ci.v3] {
            b.extend_from_slice(&f.to_le_bytes());
        }
        b.push(u8::from(ci.flip));
        b.push(u8::from(ci.mutant));
    }
    b
}

fn rust_side(c: &Case) -> TerrainHeightMap {
    let n = (c.w * c.h) as usize;
    let mut m = LogicHeightMap::new(c.w, c.h, c.border, vec![], c.data.clone()).unwrap();
    m.set_blend_tiles(Some(BlendTiles {
        version: 7,
        blend_tile_ndxes: c.blend.clone(),
        extra_blend_tile_ndxes: c.extra.clone(),
        cliff_info_ndxes: c.cliff_ndx.clone(),
        num_bitmap_tiles: c.num_bitmap_tiles,
        num_blended_tiles: c.blends.len() as i32,
        num_cliff_info: c.cliffs.len() as i32,
        rest: rest(c),
        tile_ndxes: c.tiles.clone(),
        cell_cliff_state: vec![0; ((c.w + 7) / 8 * c.h) as usize],
        flip_state_width: (c.w + 7) / 8,
        texture_classes: c
            .classes
            .iter()
            .enumerate()
            .map(|(k, &(first_tile, num_tiles, width))| TextureClass { first_tile, num_tiles, width, name: format!("C{k}").into_bytes() })
            .collect(),
    }));
    let _ = n;
    let mut t = TerrainHeightMap::from_logic(&m, 1, c.adjust).expect("from_logic");
    let nclasses = c.classes.len();
    let texture_of = |name: &[u8]| -> Option<Vec<u8>> {
        let s = std::str::from_utf8(name).ok()?;
        let (edge, k) = (s.starts_with('E'), s[1..].parse::<usize>().ok()?);
        let slot = if edge { nclasses + k } else { k };
        (c.files[slot].0 != -1).then(|| format!("{}{k}.tga", if edge { "e" } else { "c" }).into_bytes())
    };
    let mut open = |path: &[u8]| -> Option<Vec<u8>> {
        let s = std::str::from_utf8(path).ok()?.strip_prefix("Art/Terrain/")?;
        let edge = s.starts_with('e');
        let k: usize = s[1..s.len() - 4].parse().ok()?;
        let slot = if edge { nclasses + k } else { k };
        (c.files[slot].0 >= 0).then(|| c.files[slot].1.clone())
    };
    t.read_tex_classes_with(&texture_of, &mut open);
    t
}

#[derive(Default)]
struct Tally {
    cases: usize,
    cells: usize,
    vertices: usize,
    atlas_words: usize,
    stretched: usize,
    flipped: usize,
    extra: usize,
    tiles: usize,
    mismatches: usize,
}

fn run_case(c: &Case, t: &mut Tally) {
    let mut rs = rust_side(c);
    let atlas = rs.build_terrain_texture();
    let n = (c.w * c.h) as usize;
    // the constructor's patch, for the C++ side
    let nb = c.blends.len() as i32;
    let nc = c.cliffs.len() as i32;
    let patch = |v: &[i16], m: i32| -> Vec<i16> { v.iter().map(|&x| if x < 0 || i32::from(x) >= m { 0 } else { x }).collect() };
    let (blend, extra, cliff_ndx) = (patch(&c.blend, nb), patch(&c.extra, nb), patch(&c.cliff_ndx, nc));
    let ints = [
        c.w,
        c.h,
        c.border,
        c.num_bitmap_tiles,
        nb,
        nc,
        c.classes.len() as i32,
        c.edge_classes.len() as i32,
        i32::from(c.adjust),
        c.edge_classes.iter().map(|e| e.1).sum(),
    ];
    let blends: Vec<i32> = c
        .blends
        .iter()
        .flat_map(|b| {
            [b.blend_ndx, i32::from(b.horiz), i32::from(b.vert), i32::from(b.right_diagonal), i32::from(b.left_diagonal), i32::from(b.inverted), i32::from(b.long_diagonal), b.custom_blend_edge_class]
        })
        .collect();
    let cliffs: Vec<f32> = c.cliffs.iter().flat_map(|ci| [ci.u0, ci.v0, ci.u1, ci.v1, ci.u2, ci.v2, ci.u3, ci.v3]).collect();
    let cliff_ints: Vec<i32> = c.cliffs.iter().flat_map(|ci| [i32::from(ci.flip), i32::from(ci.mutant), i32::from(ci.tile_index)]).collect();
    let classes: Vec<i32> = c.classes.iter().chain(c.edge_classes.iter()).flat_map(|&(a, b, w)| [a, b, w]).collect();
    let tga: Vec<u8> = c.files.iter().flat_map(|f| f.1.clone()).collect();
    let tga_lens: Vec<i32> = c.files.iter().map(|f| f.0).collect();
    let cells = ((c.w - 1) * (c.h - 1)) as usize;
    let mut uv = vec![0f32; cells * 24];
    let mut cell = vec![0i32; cells * 13];
    let mut vtx = vec![0f32; cells * 40];
    let mut vtx_alpha = vec![0i32; cells * 4];
    let ninfo = 3 + 2 * (c.classes.len() + c.edge_classes.len()) + 6 * NUM_SOURCE_TILES;
    let mut info = vec![0i32; ninfo];
    let cap = 2048 * 4096;
    let mut catlas = vec![0u16; cap];
    let ret = unsafe {
        cpp::ref_terrain_mesh(
            ints.as_ptr(),
            c.data.as_ptr(),
            c.tiles.as_ptr(),
            blend.as_ptr(),
            extra.as_ptr(),
            cliff_ndx.as_ptr(),
            blends.as_ptr(),
            cliffs.as_ptr(),
            cliff_ints.as_ptr(),
            classes.as_ptr(),
            tga.as_ptr(),
            tga_lens.as_ptr(),
            uv.as_mut_ptr(),
            cell.as_mut_ptr(),
            vtx.as_mut_ptr(),
            vtx_alpha.as_mut_ptr(),
            info.as_mut_ptr(),
            catlas.as_mut_ptr(),
            cap as i32,
        )
    };
    assert_eq!(ret, 0, "atlas did not fit");
    let _ = n;
    t.cases += 1;
    let mut miss = |what: &str, detail: String| {
        if t.mismatches < 20 {
            eprintln!("MISMATCH case {} {what}: {detail}", t.cases);
        }
        t.mismatches += 1;
    };
    let rust_bits: u32 = rs.cell_flip_state.iter().map(|b| b.count_ones()).sum();
    if [info[0], info[1], info[2]] != [rs.terrain_tex_height, rs.alpha_edge_height, rust_bits as i32] {
        miss("heights/flips", format!("{:?} vs {:?}", &info[..3], [rs.terrain_tex_height, rs.alpha_edge_height, rust_bits as i32]));
    }
    let mut p = 3;
    for tc in rs.texture_classes.iter().chain(rs.edge_texture_classes.iter()) {
        if (info[p], info[p + 1]) != (tc.position_in_texture.x, tc.position_in_texture.y) {
            miss("class position", format!("{:?} vs {:?}", (info[p], info[p + 1]), tc.position_in_texture));
        }
        p += 2;
    }
    for tiles in [&rs.source_tiles, &rs.edge_tiles] {
        for tile in tiles.iter() {
            let r = tile.as_ref().map_or((0, 0, 0), |x| (1, x.tile_location_in_texture.x, x.tile_location_in_texture.y));
            t.tiles += r.0 as usize;
            if (info[p], info[p + 1], info[p + 2]) != r {
                miss("tile", format!("{:?} vs {:?}", (info[p], info[p + 1], info[p + 2]), r));
            }
            p += 3;
        }
    }
    let words = (2048 * rs.terrain_tex_height) as usize;
    t.atlas_words += words;
    if catlas[..words] != atlas.pixels[..] {
        let k = (0..words).find(|&k| catlas[k] != atlas.pixels[k]).unwrap();
        miss("atlas", format!("first difference at word {k}: {:04x} vs {:04x}", catlas[k], atlas.pixels[k]));
    }
    let verts = rs.update_vb();
    let bits = |v: &[f32]| v.iter().map(|f| f.to_bits()).collect::<Vec<_>>();
    let mut k = 0;
    for y in 0..c.h - 1 {
        for x in 0..c.w - 1 {
            t.cells += 1;
            let o = &uv[24 * k..24 * k + 24];
            let ci = &cell[13 * k..13 * k + 13];
            let (u, v, s) = rs.get_uv_data(x, y, false).unwrap();
            t.stretched += usize::from(s);
            if ci[0] != i32::from(s) || bits(&o[..4]) != bits(&u) || bits(&o[4..8]) != bits(&v) {
                miss("getUVData", format!("({x},{y}) {:?} {:?} vs {s} {u:?} {v:?}", ci[0], &o[..8]));
            }
            let (ua, va, a, f) = rs.get_alpha_uv_data(x, y, false);
            t.flipped += usize::from(f);
            let ca: Vec<u8> = ci[1..5].iter().map(|&q| q as u8).collect();
            if bits(&o[8..12]) != bits(&ua) || bits(&o[12..16]) != bits(&va) || ca != a || ci[5] != i32::from(f) {
                miss("getAlphaUVData", format!("({x},{y}) {:?} {:?} vs {ua:?} {va:?} {a:?} {f}", &o[8..16], &ci[1..6]));
            }
            let e = rs.get_extra_alpha_uv_data(x, y);
            t.extra += usize::from(e.is_some());
            let (ue, ve, ae, fe, ce) = e.unwrap_or(([0.0; 4], [0.0; 4], [0; 4], false, false));
            let cae: Vec<u8> = ci[7..11].iter().map(|&q| q as u8).collect();
            if ci[6] != i32::from(e.is_some()) || bits(&o[16..20]) != bits(&ue) || bits(&o[20..24]) != bits(&ve) || cae != ae || ci[11] != i32::from(fe) || ci[12] != i32::from(ce) {
                miss("getExtraAlphaUVData", format!("({x},{y}) {:?} {:?}", &o[16..24], &ci[6..13]));
            }
            for q in 0..4 {
                let rv = &verts[k * 4 + q];
                let cv = &vtx[(k * 4 + q) * 10..(k * 4 + q) * 10 + 10];
                let rr = [rv.x, rv.y, rv.z, rv.normal[0], rv.normal[1], rv.normal[2], rv.u1, rv.v1, rv.u2, rv.v2];
                t.vertices += 1;
                if bits(cv) != bits(&rr) || vtx_alpha[k * 4 + q] != i32::from(rv.alpha) {
                    miss("updateVB", format!("({x},{y}) vertex {q}: {cv:?} {} vs {rr:?} {}", vtx_alpha[k * 4 + q], rv.alpha));
                }
            }
            k += 1;
        }
    }
    let _ = ICoord2D { x: 0, y: 0 };
}

/// Every case compared as the module docs say; the results are there.
#[test]
fn cases_match_upstream() {
    let cases: usize = std::env::var("TERRAIN_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(400);
    let mut r = Rng(0x2545_F491_4F6C_DD1D);
    let mut t = Tally::default();
    for _ in 0..cases {
        let c = make_case(&mut r);
        run_case(&c, &mut t);
    }
    eprintln!(
        "terrain: {} cases, {} cells, {} vertices, {} atlas words, {} tiles read, {} cells stretched for cliffs, {} flipped, {} with a third layer; {} mismatches",
        t.cases, t.cells, t.vertices, t.atlas_words, t.tiles, t.stretched, t.flipped, t.extra, t.mismatches
    );
    assert_eq!(t.mismatches, 0);
}
