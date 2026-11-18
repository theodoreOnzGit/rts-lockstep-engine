// SPDX-License-Identifier: GPL-3.0-only
//! Byte builders for the group "anim" differential tests (#215): clean W3D hierarchy and animation chunks, built in memory
//! (nothing of the game's files). They are read by BOTH the C++ loaders and the verified Rust ones, so only well-formed
//! input is needed here; the loaders' handling of malformed input is `tests/differential_w3d.rs`'s.
#![allow(dead_code)]

use rts_lockstep_engine::formats::w3d::w3d_file::*;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector4::Vector4;

pub fn w32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
pub fn w16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
pub fn wf(v: &mut Vec<u8>, x: f32) {
    v.extend_from_slice(&x.to_bits().to_le_bytes());
}

/// A chunk: id, size (with the sub-chunk flag when it holds chunks), body.
pub fn chunk(id: u32, sub: bool, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    w32(&mut v, id);
    w32(&mut v, body.len() as u32 | if sub { 0x8000_0000 } else { 0 });
    v.extend_from_slice(body);
    v
}

pub fn name16(s: &[u8]) -> [u8; 16] {
    let mut n = [0u8; 16];
    n[..s.len()].copy_from_slice(s);
    n
}

#[derive(Clone, Debug)]
pub struct PivotSpec {
    pub name: Vec<u8>,
    /// `0xFFFFFFFF` for the root
    pub parent: u32,
    pub t: [f32; 3],
    pub e: [f32; 3],
    pub q: [f32; 4],
}

/// `W3D_CHUNK_HIERARCHY`: version 4.1, the header and the pivots.
pub fn tree_chunk(name: &[u8], pivots: &[PivotSpec]) -> Vec<u8> {
    let mut h = Vec::new();
    w32(&mut h, w3d_make_version(4, 1));
    h.extend_from_slice(&name16(name));
    w32(&mut h, pivots.len() as u32);
    for _ in 0..3 {
        wf(&mut h, 0.0);
    }
    let mut body = chunk(W3D_CHUNK_HIERARCHY_HEADER, false, &h);
    let mut pb = Vec::new();
    for p in pivots {
        pb.extend_from_slice(&name16(&p.name));
        w32(&mut pb, p.parent);
        for v in p.t.iter().chain(p.e.iter()).chain(p.q.iter()) {
            wf(&mut pb, *v);
        }
    }
    body.extend(chunk(W3D_CHUNK_PIVOTS, false, &pb));
    chunk(W3D_CHUNK_HIERARCHY, true, &body)
}

/// One float channel of a raw animation: `flags` 0..=6 (X, Y, Z, XR, YR, ZR, Q), the node, the frames and the data.
pub struct RawChan {
    pub flags: u16,
    pub pivot: u16,
    pub frames: u16,
}

/// `W3D_CHUNK_ANIMATION`: header, float channels (data from `val`), bit channels for the nodes in `vis`.
pub fn raw_anim_chunk(name: &[u8], hier: &[u8], frames: u32, chans: &[RawChan], vis: &[u16], mut val: impl FnMut() -> f32) -> Vec<u8> {
    let mut h = Vec::new();
    w32(&mut h, w3d_make_version(4, 1));
    h.extend_from_slice(&name16(name));
    h.extend_from_slice(&name16(hier));
    w32(&mut h, frames);
    w32(&mut h, 30);
    let mut body = chunk(W3D_CHUNK_ANIMATION_HEADER, false, &h);
    for c in chans {
        let vlen: u16 = if c.flags == 6 { 4 } else { 1 };
        let mut b = Vec::new();
        for v in [0u16, c.frames - 1, vlen, c.flags, c.pivot, 0] {
            w16(&mut b, v);
        }
        for _ in 0..(c.frames as usize * vlen as usize) {
            wf(&mut b, val());
        }
        body.extend(chunk(W3D_CHUNK_ANIMATION_CHANNEL, false, &b));
    }
    for &p in vis {
        let mut b = Vec::new();
        for v in [0u16, 15, 0, p] {
            w16(&mut b, v);
        }
        b.push(1);
        b.extend_from_slice(&[0xA5, 0x5A]);
        body.extend(chunk(W3D_CHUNK_BIT_CHANNEL, false, &b));
    }
    chunk(W3D_CHUNK_ANIMATION, true, &body)
}

/// `W3D_CHUNK_COMPRESSED_ANIMATION` (time coded): header, channels `(flags 0, 1, 2 or 6, pivot, time codes)`, bit channels.
pub fn comp_anim_chunk(name: &[u8], hier: &[u8], frames: u32, chans: &[(u8, u16, u32)], vis: &[u16], mut val: impl FnMut() -> f32) -> Vec<u8> {
    let mut h = Vec::new();
    w32(&mut h, w3d_make_version(0, 1));
    h.extend_from_slice(&name16(name));
    h.extend_from_slice(&name16(hier));
    w32(&mut h, frames);
    w16(&mut h, 30);
    w16(&mut h, 0);
    let mut body = chunk(W3D_CHUNK_COMPRESSED_ANIMATION_HEADER, false, &h);
    for &(flags, pivot, nt) in chans {
        let vlen: u8 = if flags == 6 { 4 } else { 1 };
        let mut b = Vec::new();
        w32(&mut b, nt);
        w16(&mut b, pivot);
        b.push(vlen);
        b.push(flags);
        for k in 0..nt {
            w32(&mut b, k * 2);
            for _ in 0..vlen {
                wf(&mut b, val());
            }
        }
        body.extend(chunk(W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL, false, &b));
    }
    for &p in vis {
        let mut b = Vec::new();
        w32(&mut b, 2);
        w16(&mut b, p);
        b.push(1);
        b.push(1);
        w32(&mut b, 0);
        w32(&mut b, 0x8000_0003);
        body.extend(chunk(W3D_CHUNK_COMPRESSED_BIT_CHANNEL, false, &b));
    }
    chunk(W3D_CHUNK_COMPRESSED_ANIMATION, true, &body)
}

/// A matrix from 12 floats, row-major 3x4.
pub fn mat(a: [f32; 12]) -> Matrix3D {
    Matrix3D {
        row: [Vector4::new(a[0], a[1], a[2], a[3]), Vector4::new(a[4], a[5], a[6], a[7]), Vector4::new(a[8], a[9], a[10], a[11])],
    }
}

pub fn words_blob(b: &[u8], out: &mut Vec<u32>) {
    out.push(b.len() as u32);
    out.extend(b.iter().map(|&x| u32::from(x)));
}

/// The end of a test: a planted run (`L9_PLANT` set) prints its MEASURED line and fails on purpose; a normal run must
/// have 0 mismatches.
pub fn finish(check: super::support::Check) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    if planted {
        check.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        check.finish();
    }
}

pub fn cases() -> u64 {
    std::env::var("DRAFT_ANIM_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}
