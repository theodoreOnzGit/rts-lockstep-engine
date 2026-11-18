//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3d_dep.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3d_dep.h
//!
//! Scans a `.w3d` file and lists the other files it needs (the container mesh's file, textures, the hierarchy an
//! animation / HModel / HLOD refers to, an emitter's texture, an aggregate's base model and subobjects). The file
//! factory (`_TheFileFactory->Get_File`) is the caller's: the function takes the bytes. Struct fields are read by
//! the byte offsets of `w3d_file.h` (checked against `formats::w3d::w3d_file`'s `FIELDS` tables for the sizes used:
//! `W3dMeshHeader3Struct.ContainerName` @24, `W3dAnimHeaderStruct.HierarchyName` @20, `W3dHModelHeaderStruct`
//! @20, `W3dHLodHeaderStruct` @24, `W3dAggregateInfoStruct` 32-byte name + count @32 and 64-byte subobject records).
//!
//! Upstream quirks kept: the `StringList::sort()/unique()` at the end (byte order of the generated names); a
//! texture name chunk is read with `Read(texture, Cur_Chunk_Length())` into a `_MAX_PATH` buffer with no length
//! guard and no NUL termination guarantee (names longer than 259 bytes overflow; here they are cut at the buffer).

use crate::formats::w3d::w3d_file::*;
use crate::ww::chunkio::{ChunkLoad, GameFile};

/// `W3D_NAME_LEN` (w3d_file.h): 16.
const W3D_NAME_LEN: usize = 16;
/// `_MAX_PATH`.
const MAX_PATH: usize = 260;

/// `Get_W3D_Name(filename, w3d_name)` (w3d_dep.cpp:340): the file name after the last backslash, without its
/// extension (the last dot), cut to `W3D_NAME_LEN - 1` characters, upper-cased.
pub fn get_w3d_name(filename: &str) -> String {
    let bytes = filename.as_bytes();
    let start = match bytes.iter().rposition(|&c| c == b'\\') {
        Some(p) => p + 1,
        None => 0,
    };
    let end = match bytes[start..].iter().rposition(|&c| c == b'.') {
        Some(p) => start + p,
        None => bytes.len(),
    };
    let num_chars = end - start;
    let n = if num_chars < W3D_NAME_LEN { num_chars } else { W3D_NAME_LEN - 1 };
    bytes[start..start + n].iter().map(|c| c.to_ascii_uppercase() as char).collect()
}

/// `Make_W3D_Filename(w3d_name)` (w3d_dep.cpp:362): empty stays empty; otherwise cut at the first dot, lower-cased,
/// `.w3d` appended. (The result lives in a 64-byte static buffer upstream.)
pub fn make_w3d_filename(w3d_name: &str) -> String {
    if w3d_name.is_empty() {
        return String::new();
    }
    debug_assert!(w3d_name.len() < W3D_NAME_LEN);
    let base = match w3d_name.find('.') {
        Some(d) => &w3d_name[..d],
        None => w3d_name,
    };
    let mut s = base.to_ascii_lowercase();
    s.push_str(".w3d");
    s
}

/// A NUL-terminated name inside a struct's byte image.
fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn read_chunk_body(cload: &mut ChunkLoad<'_>, len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    let _ = cload.read(&mut buf, len as u32);
    buf
}

/// Pushes `Make_W3D_Filename(name)` unless `name == w3d_name` or the filename is empty.
fn push_dependency(files: &mut Vec<String>, name: &str, w3d_name: &str) {
    if name != w3d_name {
        let filename = make_w3d_filename(name);
        if !filename.is_empty() {
            files.push(filename);
        }
    }
}

/// `Scan_Mesh_Textures` (w3d_dep.cpp:151).
fn scan_mesh_textures(cload: &mut ChunkLoad<'_>, files: &mut Vec<String>) {
    while cload.open_chunk().unwrap_or(false) {
        if cload.cur_chunk_id().unwrap_or(0) == W3D_CHUNK_TEXTURE {
            while cload.open_chunk().unwrap_or(false) {
                if cload.cur_chunk_id().unwrap_or(0) == W3D_CHUNK_TEXTURE_NAME {
                    let len = (cload.cur_chunk_length().unwrap_or(0) as usize).min(MAX_PATH);
                    let buf = read_chunk_body(cload, len);
                    let texture = cstr(&buf);
                    if !texture.is_empty() {
                        files.push(texture);
                    }
                }
                let _ = cload.close_chunk();
            }
        }
        let _ = cload.close_chunk();
    }
}

/// A `Scan_*` of the form "find chunk `header_id`, read `size` bytes, take the name at `offset`".
fn scan_hierarchy(cload: &mut ChunkLoad<'_>, files: &mut Vec<String>, w3d_name: &str, header_id: u32, size: usize, offset: usize) {
    while cload.open_chunk().unwrap_or(false) {
        if cload.cur_chunk_id().unwrap_or(0) == header_id {
            let buf = read_chunk_body(cload, size);
            let name = cstr(&buf[offset..offset + W3D_NAME_LEN]);
            push_dependency(files, &name, w3d_name);
        }
        let _ = cload.close_chunk();
    }
}

/// `Scan_Mesh` / `Scan_Mesh_Header` (w3d_dep.cpp:111): `W3dMeshHeader3Struct` (116 bytes), `ContainerName` @24.
fn scan_mesh(cload: &mut ChunkLoad<'_>, files: &mut Vec<String>, w3d_name: &str) {
    while cload.open_chunk().unwrap_or(false) {
        match cload.cur_chunk_id().unwrap_or(0) {
            W3D_CHUNK_MESH_HEADER3 => {
                let buf = read_chunk_body(cload, 116);
                let container = cstr(&buf[24..24 + W3D_NAME_LEN]);
                push_dependency(files, &container, w3d_name);
            }
            W3D_CHUNK_TEXTURES => scan_mesh_textures(cload, files),
            _ => {}
        }
        let _ = cload.close_chunk();
    }
}

/// `Scan_Emitter` (w3d_dep.cpp:223): `W3dEmitterInfoStruct` (332 bytes), `TextureFilename` @0 (260).
fn scan_emitter(cload: &mut ChunkLoad<'_>, files: &mut Vec<String>) {
    while cload.open_chunk().unwrap_or(false) {
        if cload.cur_chunk_id().unwrap_or(0) == W3D_CHUNK_EMITTER_INFO {
            let buf = read_chunk_body(cload, 332);
            if buf[0] != 0 {
                files.push(cstr(&buf[..MAX_PATH]));
            }
        }
        let _ = cload.close_chunk();
    }
}

/// `Scan_Aggregate` (w3d_dep.cpp:242): the base model name, then each subobject name.
fn scan_aggregate(cload: &mut ChunkLoad<'_>, files: &mut Vec<String>, w3d_name: &str) {
    while cload.open_chunk().unwrap_or(false) {
        if cload.cur_chunk_id().unwrap_or(0) == W3D_CHUNK_AGGREGATE_INFO {
            let buf = read_chunk_body(cload, 36);
            let base_model = cstr(&buf[..32]);
            let count = u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);
            if base_model != w3d_name {
                let f = make_w3d_filename(&base_model);
                if !f.is_empty() {
                    files.push(f);
                }
            }
            for _ in 0..count {
                let sub = read_chunk_body(cload, 64);
                let subobject = cstr(&sub[..32]);
                if subobject != w3d_name {
                    let f = make_w3d_filename(&subobject);
                    if !f.is_empty() {
                        files.push(f);
                    }
                }
            }
        }
        let _ = cload.close_chunk();
    }
}

/// `Get_W3D_Dependencies(w3d_filename, files)` (w3d_dep.cpp:63) over the file's bytes: scans each top-level chunk and
/// returns the sorted, de-duplicated list (`files.sort(); files.unique()`). Returns `None` if the file could not be
/// opened (here: the bytes do not form a chunk stream).
pub fn get_w3d_dependencies(w3d_filename: &str, data: &[u8]) -> Option<Vec<String>> {
    let file = GameFile::new(data)?;
    let w3d_name = get_w3d_name(w3d_filename);
    let mut cload = ChunkLoad::new(file);
    let mut files: Vec<String> = Vec::new();
    while cload.open_chunk().unwrap_or(false) {
        // Scan_Chunk
        match cload.cur_chunk_id().unwrap_or(0) {
            W3D_CHUNK_MESH => scan_mesh(&mut cload, &mut files, &w3d_name),
            W3D_CHUNK_ANIMATION => {
                scan_hierarchy(&mut cload, &mut files, &w3d_name, W3D_CHUNK_ANIMATION_HEADER, 44, 20)
            }
            W3D_CHUNK_COMPRESSED_ANIMATION => {
                scan_hierarchy(&mut cload, &mut files, &w3d_name, W3D_CHUNK_ANIMATION_HEADER, 44, 20)
            }
            W3D_CHUNK_HMODEL => scan_hierarchy(&mut cload, &mut files, &w3d_name, W3D_CHUNK_HMODEL_HEADER, 40, 20),
            W3D_CHUNK_EMITTER => scan_emitter(&mut cload, &mut files),
            W3D_CHUNK_AGGREGATE => scan_aggregate(&mut cload, &mut files, &w3d_name),
            W3D_CHUNK_HLOD => scan_hierarchy(&mut cload, &mut files, &w3d_name, W3D_CHUNK_HLOD_HEADER, 40, 24),
            _ => {}
        }
        let _ = cload.close_chunk();
    }
    files.sort();
    files.dedup();
    Some(files)
}
