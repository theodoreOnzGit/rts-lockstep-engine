//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texfcach.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texfcach.h
//!
//! `TextureFileCache`: an on-disk cache of mip-mapped textures in one `.tfc` file made of named "tag blocks"
//! (`TagBlockFile`). Each texture's block is a [`TextureBlockHeader`], a table of [`OffsetTableType`] (one per mip level and a
//! final end-of-block entry), then the mip data (a stored copy: `Compressor::Compress`/`Decompress` are `memcpy`).
//!
//! **This file is compiled out upstream.** Like `texturefile.cpp`, the whole of texfcach.h / .cpp sits inside
//! `#ifdef WW3D_DX8`, which is defined nowhere in the source tree, and it is built on the Surrender library (`srColorSurface`,
//! `srTextureIFace`) and `TagBlockFile`, none of which is in the release. It is drafted as the data handling the brief asks for:
//! the file layout and the cache logic, with the tag-block file as the [`TagBlockStore`] contract (disk I/O is the host's)
//! and Surrender surfaces as [`CacheSurface`] byte buffers.
//!
//! **Findings kept as upstream has them.**
//! - `_Create_File_Name`: when the prefix is not relative to the current directory (or `_getcwd` fails), the `else`
//!   branch builds `prefix + ".tfc"` and then does `*_FileNamePtr = 0;`, so the file name passed on is the empty string.
//! - `Add_Cached_Surface` (cache full) releases `CachedSurfaces[idx]` with `idx` the loop variable used after the loop
//!   (equal to `NumCachedTextures`, past the end under VC6's for-scoping) instead of `smallidx`; the surface actually
//!   replaced is `smallidx`. The intended behaviour (release `smallidx`) is implemented.
//! - `Load_Texture` compares sizes in bytes to match mip levels of the cache to those requested; a request whose levels do
//!   not size-match any cached level is filled by copying (`surface->copy`) from the largest, as translated below.

/// `FILE_HEADER_NAME`.
pub const FILE_HEADER_NAME: &str = "Texture File Cache Header";
/// `FileHeader::TCF_VERSION`: "Put in date when format is changed."
pub const TCF_VERSION: i32 = 20000814;
/// `srTextureIFace::MAX_LOD` (= `MAX_CACHED_SURFACES`).
pub const MAX_LOD: usize = 16;
/// `MAX_CACHED_SURFACES`.
pub const MAX_CACHED_SURFACES: usize = MAX_LOD;

/// A `srColorSurface`: a pixel format (opaque), size and bytes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CacheSurface {
    /// `srColorSurfaceIFace::PixelFormat` (opaque bytes).
    pub pixel_format: Vec<u8>,
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
}

impl CacheSurface {
    /// `getDataSize()`.
    pub fn get_data_size(&self) -> i32 {
        self.data.len() as i32
    }
}

/// `struct FileHeader`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileHeader {
    pub version: i32,
}

/// `struct TextureBlockHeader`.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureBlockHeader {
    /// Time data stamp of file.
    pub file_time: u64,
    /// Number of mip maps in texture (including first one).
    pub num_mip_maps: i32,
    /// Dimensions of first mip map level saved.
    pub largest_width: i32,
    pub largest_height: i32,
    /// Dimensions of original surface/art work.
    pub source_width: i32,
    pub source_height: i32,
    /// The pixel format used to create the source's original surface / the format we saved in.
    pub source_pixel_format: Vec<u8>,
    pub pixel_format: Vec<u8>,
}

impl Default for TextureBlockHeader {
    /// `TextureBlockHeader()`: `NumMipMaps(-1), SourceWidth(-1), SourceHeight(-1), LargestWidth(-1), LargestHeight(-1)`.
    fn default() -> Self {
        TextureBlockHeader {
            file_time: 0,
            num_mip_maps: -1,
            largest_width: -1,
            largest_height: -1,
            source_width: -1,
            source_height: -1,
            source_pixel_format: Vec::new(),
            pixel_format: Vec::new(),
        }
    }
}

/// `struct OffsetTableType`: each mip level's offset into the block and its size when uncompressed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OffsetTableType {
    pub offset: i32,
    pub size: i32,
}

/// `TagBlockHandle` operations the cache uses on an open block.
pub trait TagBlockHandle {
    /// `Read(&Header, sizeof)` / `Write`.
    fn read_header(&mut self) -> TextureBlockHeader;
    fn write_header(&mut self, h: &TextureBlockHeader);
    /// `Read(Offsets, ..)` / `Write(Offsets, ..)` of `n` entries.
    fn read_offsets(&mut self, n: usize) -> Vec<OffsetTableType>;
    fn write_offsets(&mut self, offsets: &[OffsetTableType]);
    /// `Tell()`, `Seek(pos, SEEK_SET)`, `Seek(0, SEEK_END)`.
    fn tell(&mut self) -> i32;
    fn seek_set(&mut self, pos: i32);
    fn seek_end(&mut self) -> i32;
    /// `Read(buffer, n)` / `Write(buffer, n)`; the count read / written.
    fn read(&mut self, n: usize) -> Vec<u8>;
    fn write(&mut self, data: &[u8]) -> usize;
    /// `End_Write_Access()`.
    fn end_write_access(&mut self);
    /// The file-header tag's `Read(&fileheader)` / `Write`.
    fn read_file_header(&mut self) -> FileHeader;
    fn write_file_header(&mut self, h: &FileHeader);
}

/// `TagBlockFile`: the named blocks of the `.tfc` file.
pub trait TagBlockStore {
    type Handle: TagBlockHandle;
    /// `Open_Tag(name)`.
    fn open_tag(&mut self, name: &str) -> Option<Self::Handle>;
    /// `Create_Tag(name)`.
    fn create_tag(&mut self, name: &str) -> Option<Self::Handle>;
    /// `Does_Tag_Exist(name)`.
    fn does_tag_exist(&mut self, name: &str) -> bool;
    /// `Reset_File()`.
    fn reset_file(&mut self);
}

/// The file queries the cache makes of the asset's file (`_TheFileFactory->Get_File(name)`, `Get_Date_Time()`) and of the texture
/// loader.
pub trait TextureFileCacheHost {
    /// The asset file's `Get_Date_Time()`.
    fn asset_date_time(&mut self, texturename: &str) -> u64;
    /// `TextureLoader::Texture_File_Exists(name)`.
    fn texture_file_exists(&mut self, texturename: &str) -> bool;
    /// `::Load_Surface(name)`.
    fn load_surface(&mut self, texturename: &str) -> Option<CacheSurface>;
    /// `srColorSurface::copy(src)`: resamples `src` into `dst`'s size and format.
    fn copy_surface(&mut self, dst: &mut CacheSurface, src: &CacheSurface);
}

/// `Compressor::Compress` / `Decompress`: both a plain copy; `None` is `FALSE` (a NULL argument or an empty input).
pub fn compress(input: &[u8]) -> Option<Vec<u8>> {
    if input.is_empty() {
        return None;
    }
    Some(input.to_vec())
}

/// `_Create_File_Name(fileprefix)`: `cwd + "\\" + prefix + ".tfc"` for a relative prefix and a successful `_getcwd`; otherwise
/// the empty string (see the file header).
pub fn create_file_name(fileprefix: &str, cwd: Option<&str>) -> String {
    let bytes = fileprefix.as_bytes();
    let addpath = bytes.first() != Some(&b'\\') && bytes.get(1) != Some(&b':');
    match (addpath, cwd) {
        (true, Some(path)) => format!("{}\\{}.tfc", path, fileprefix),
        _ => String::new(), // strcpy + strcat, then `*_FileNamePtr = 0`
    }
}

/// `class TextureFileCache`.
pub struct TextureFileCache<S: TagBlockStore> {
    /// `File`.
    pub file: S,
    /// `CurrentTexture`.
    pub current_texture: Option<String>,
    /// `TextureHandle`.
    pub texture_handle: Option<S::Handle>,
    /// `Header`.
    pub header: TextureBlockHeader,
    /// `Offsets`.
    pub offsets: Vec<OffsetTableType>,
    /// `CachedSurfaces[MAX_CACHED_SURFACES]` / `NumCachedTextures` (a `Vec` of at most `MAX_CACHED_SURFACES`).
    pub cached_surfaces: Vec<CacheSurface>,
}

impl<S: TagBlockStore> TextureFileCache<S> {
    /// `TextureFileCache(fileprefix)`: opens the file; a missing or wrong-version header resets it. (`Instances` is a
    /// singleton guard: one cache at a time, the owner's.)
    pub fn new(mut file: S) -> Self {
        let mut reset = false;
        match file.open_tag(FILE_HEADER_NAME) {
            Some(mut handle) => {
                let fileheader = handle.read_file_header();
                if fileheader.version != TCF_VERSION {
                    reset = true;
                }
            }
            None => reset = true,
        }
        let mut c = TextureFileCache {
            file,
            current_texture: None,
            texture_handle: None,
            header: TextureBlockHeader::default(),
            offsets: Vec::new(),
            cached_surfaces: Vec::new(),
        };
        if reset {
            c.reset_file();
        }
        c
    }

    /// `~TextureFileCache()`: `Close_Texture_Handle()`.
    pub fn destroy(mut self) {
        self.close_texture_handle();
    }

    /// `Reset_File()`: empties the file and writes the version tag.
    pub fn reset_file(&mut self) {
        self.file.reset_file();
        match self.file.create_tag(FILE_HEADER_NAME) {
            Some(mut handle) => handle.write_file_header(&FileHeader { version: TCF_VERSION }),
            None => debug_assert!(false),
        }
    }

    /// `Texture_Size(lod)`: the uncompressed size of mip level `lod`.
    fn texture_size(&self, lod: usize) -> i32 {
        debug_assert!(!self.offsets.is_empty());
        debug_assert!((lod as i32) < self.header.num_mip_maps);
        self.offsets[lod].size
    }

    /// `Compressed_Texture_Size(lod)`: the next entry's offset minus this one's.
    fn compressed_texture_size(&self, lod: usize) -> i32 {
        debug_assert!(!self.offsets.is_empty());
        debug_assert!((lod as i32) < self.header.num_mip_maps);
        self.offsets[lod + 1].offset - self.offsets[lod].offset
    }

    /// `Texture_Exists(fname)`.
    pub fn texture_exists(&mut self, fname: &str) -> bool {
        self.file.does_tag_exist(fname)
    }

    /// `Open_Texture_Handle(fname)`: the block of `fname`, its header (rejected, and the whole file reset, when the asset's
    /// time stamp differs from the one saved) and offset table.
    pub fn open_texture_handle<H: TextureFileCacheHost>(&mut self, host: &mut H, fname: &str) -> bool {
        if self.texture_handle.is_some() {
            debug_assert!(self.current_texture.is_some());
            if self.current_texture.as_deref().map(|c| c.eq_ignore_ascii_case(fname)).unwrap_or(false) {
                return true;
            }
            self.close_texture_handle();
        }
        if self.current_texture.is_none() {
            self.current_texture = Some(fname.to_string());
        }
        // (an `#if 0` block compared the asset date with the file's)
        self.texture_handle = self.file.open_tag(fname);
        if let Some(handle) = self.texture_handle.as_mut() {
            self.header = handle.read_header();
            let asset_time = host.asset_date_time(fname);
            if self.header.file_time != asset_time {
                self.texture_handle = None;
                self.reset_file();
                return false;
            }
            let n = (self.header.num_mip_maps + 1) as usize;
            self.offsets = self.texture_handle.as_mut().unwrap().read_offsets(n);
            return true;
        }
        false
    }

    /// `Close_Texture_Handle()`: forgets the open block, the cached surfaces and the offset table.
    pub fn close_texture_handle(&mut self) {
        if self.current_texture.is_some() {
            self.current_texture = None;
            self.texture_handle = None;
            self.cached_surfaces.clear();
            self.offsets.clear();
        } else {
            debug_assert!(self.current_texture.is_none());
        }
    }

    /// `Find_Cached_Surface(size)`: the cached surface of exactly that data size.
    fn find_cached_surface(&self, size: i32) -> Option<usize> {
        self.cached_surfaces.iter().position(|s| s.get_data_size() == size)
    }

    /// `Add_Cached_Surface(surface)`: when full, the smallest cached surface is replaced (see the file header).
    fn add_cached_surface(&mut self, surface: &CacheSurface) {
        debug_assert!(self.find_cached_surface(surface.get_data_size()).is_none());
        if self.cached_surfaces.len() == MAX_CACHED_SURFACES {
            let mut smallsize = 0xfffffff;
            let mut smallidx: i32 = -1;
            for (idx, s) in self.cached_surfaces.iter().enumerate() {
                let size = s.get_data_size();
                if size < smallsize {
                    smallsize = size;
                    smallidx = idx as i32;
                }
            }
            debug_assert!(smallidx >= 0);
            self.cached_surfaces[smallidx as usize] = surface.clone();
        } else {
            self.cached_surfaces.push(surface.clone());
        }
    }

    /// `Read_Texture(offsetidx, surface)`: seeks to the level, reads its stored bytes and "decompresses" them into the surface.
    fn read_texture(&mut self, offsetidx: usize, surface: &mut CacheSurface) {
        let compsize = self.compressed_texture_size(offsetidx) as usize;
        let off = self.offsets[offsetidx].offset;
        let h = self.texture_handle.as_mut().unwrap();
        h.seek_set(off);
        let buf = h.read(compsize);
        debug_assert!(buf.len() == compsize);
        let decomp = compress(&buf); // Decompress: memcpy
        debug_assert!(decomp.is_some());
        let d = decomp.unwrap_or_default();
        debug_assert!(d.len() as i32 == self.texture_size(offsetidx));
        surface.data = d;
    }

    /// `Save_Texture(texturename, mreq, origsurface)`: writes the header, a placeholder offset table, each level of
    /// `levels[large_lod..=small_lod]`, then rewrites the table with the real offsets (the last entry is the end of the block,
    /// size 0). `levels` are indexed by LOD.
    pub fn save_texture<H: TextureFileCacheHost>(
        &mut self,
        host: &mut H,
        texturename: &str,
        levels: &[CacheSurface],
        large_lod: usize,
        small_lod: usize,
        origsurface: &CacheSurface,
    ) -> bool {
        self.open_texture_handle(host, texturename);
        self.texture_handle = self.file.create_tag(texturename);
        if self.texture_handle.is_none() {
            self.close_texture_handle();
            return false;
        }
        self.header.file_time = host.asset_date_time(texturename);
        self.header.num_mip_maps = (small_lod as i32 - large_lod as i32) + 1;
        self.header.largest_width = levels[large_lod].width;
        self.header.largest_height = levels[large_lod].height;
        self.header.source_width = origsurface.width;
        self.header.source_height = origsurface.height;
        self.header.pixel_format = levels[large_lod].pixel_format.clone();
        self.header.source_pixel_format = origsurface.pixel_format.clone();

        let n = (self.header.num_mip_maps + 1) as usize;
        self.offsets = vec![OffsetTableType::default(); n];
        let hdr = self.header.clone();
        let handle = self.texture_handle.as_mut().unwrap();
        handle.write_header(&hdr);
        let tableoffset = handle.tell();
        handle.write_offsets(&self.offsets);

        let mut idx = 0usize;
        let mut lod = large_lod;
        while lod <= small_lod {
            let surface = &levels[lod];
            debug_assert!(!surface.data.is_empty());
            let off = self.texture_handle.as_mut().unwrap().tell();
            self.offsets[idx].offset = off;
            self.offsets[idx].size = surface.get_data_size();
            self.add_cached_surface(surface);
            let comp = compress(&surface.data);
            debug_assert!(comp.is_some());
            let comp = comp.unwrap_or_default();
            let readin = self.texture_handle.as_mut().unwrap().write(&comp);
            debug_assert!(readin == comp.len());
            idx += 1;
            lod += 1;
        }
        let handle = self.texture_handle.as_mut().unwrap();
        let _pos = handle.seek_end();
        self.offsets[idx].offset = handle.tell();
        self.offsets[idx].size = 0;
        handle.seek_set(tableoffset);
        handle.write_offsets(&self.offsets);
        let _pos = handle.seek_end();
        handle.end_write_access();
        true
    }

    /// `Load_Original_Texture_Surface(texturename)`: a blank surface of the source's format and size.
    pub fn load_original_texture_surface<H: TextureFileCacheHost>(&mut self, host: &mut H, texturename: &str) -> Option<CacheSurface> {
        if self.open_texture_handle(host, texturename) {
            return Some(CacheSurface {
                pixel_format: self.header.source_pixel_format.clone(),
                width: self.header.source_width,
                height: self.header.source_height,
                data: Vec::new(),
            });
        }
        None
    }

    /// `Get_Surface(texturename, reduce_factor)`: the mip level `reduce_factor` (clamped to the last), from the cached
    /// surfaces when one has that size, else read from the file.
    pub fn get_surface<H: TextureFileCacheHost>(&mut self, host: &mut H, texturename: &str, mut reduce_factor: u32) -> Option<CacheSurface> {
        if !self.open_texture_handle(host, texturename) {
            return None;
        }
        if reduce_factor >= self.header.num_mip_maps as u32 {
            reduce_factor = (self.header.num_mip_maps - 1) as u32;
        }
        let mut width = self.header.largest_width >> reduce_factor;
        let mut height = self.header.largest_height >> reduce_factor;
        if width == 0 {
            width = 1;
        }
        if height == 0 {
            height = 1;
        }
        let mut surface = CacheSurface { pixel_format: self.header.pixel_format.clone(), width, height, data: Vec::new() };
        let size = self.texture_size(reduce_factor as usize);
        match self.find_cached_surface(size) {
            Some(ci) => {
                surface.data = self.cached_surfaces[ci].data.clone();
            }
            None => {
                self.read_texture(reduce_factor as usize, &mut surface);
                self.add_cached_surface(&surface);
            }
        }
        Some(surface)
    }

    /// `Create_First_Texture_As_Surface(surfacetype)`: the largest level, in `surfacetype`'s pixel format.
    fn create_first_texture_as_surface(&mut self, surfacetype: &CacheSurface) -> CacheSurface {
        let mut surface = CacheSurface {
            pixel_format: surfacetype.pixel_format.clone(),
            width: self.header.largest_width,
            height: self.header.largest_height,
            data: Vec::new(),
        };
        self.read_texture(0, &mut surface);
        surface
    }

    /// `Load_Texture(texturename, mreq)`: fills the requested levels from the cache where level sizes match, and the rest
    /// by resampling the largest cached level into them. `levels` are the request's surfaces indexed by LOD.
    pub fn load_texture<H: TextureFileCacheHost>(&mut self, host: &mut H, texturename: &str, levels: &mut [CacheSurface], large_lod: usize, small_lod: usize) -> bool {
        if !self.open_texture_handle(host, texturename) {
            return false;
        }
        // find the first requested level no larger than the cache's largest
        let mut idxsize = self.texture_size(0);
        let mut lodsize: i32 = -1;
        let mut lod = large_lod;
        while lod <= small_lod {
            lodsize = levels[lod].get_data_size();
            if lodsize <= idxsize {
                break;
            }
            lod += 1;
        }
        debug_assert!(lodsize != -1);
        // find the first cached level no larger than that
        let mut idx = 0usize;
        while (idx as i32) < self.header.num_mip_maps {
            idxsize = self.texture_size(idx);
            if idxsize <= lodsize {
                break;
            }
            idx += 1;
        }
        let firstlod = lod;
        if idxsize == lodsize {
            while lod <= small_lod && (idx as i32) < self.header.num_mip_maps {
                let size = levels[lod].get_data_size();
                debug_assert!(self.texture_size(idx) == size);
                match self.find_cached_surface(size) {
                    Some(ci) => {
                        let data = self.cached_surfaces[ci].data.clone();
                        levels[lod].data = data;
                    }
                    None => {
                        let mut s = levels[lod].clone();
                        self.read_texture(idx, &mut s);
                        levels[lod].data = s.data.clone();
                        self.add_cached_surface(&levels[lod].clone());
                    }
                }
                idx += 1;
                lod += 1;
            }
        }
        let lastlod = lod as i32 - 1;
        // `surface` is the first filled level when more than one was filled, else made on demand
        let mut surface: Option<CacheSurface> = None;
        if (firstlod as i32) < lastlod {
            surface = Some(levels[firstlod].clone());
        }
        // levels above the first filled one are copied down from the full-size texture
        if large_lod < firstlod {
            if surface.is_none() {
                let proto = levels[large_lod].clone();
                surface = Some(self.create_first_texture_as_surface(&proto));
            }
            let src = surface.clone().unwrap();
            for l in large_lod..=firstlod {
                host.copy_surface(&mut levels[l], &src);
            }
        }
        // levels below the last filled one
        if lastlod < small_lod as i32 {
            if surface.is_none() {
                let proto = levels[large_lod].clone();
                surface = Some(self.create_first_texture_as_surface(&proto));
            }
            let src = surface.clone().unwrap();
            let mut l = (lastlod + 1) as usize;
            while l <= small_lod {
                host.copy_surface(&mut levels[l], &src);
                l += 1;
            }
        }
        true
    }

    /// `Validate_Texture(FileName)`: when the cache has no block for the texture, loads the surface, builds a chain of
    /// square levels halving from its width down to 1 (`w` = the surface's width, each level `copy`ed from the original),
    /// and `Save_Texture`s it. False when the texture file does not exist or cannot be loaded.
    pub fn validate_texture<H: TextureFileCacheHost>(&mut self, host: &mut H, file_name: &str) -> bool {
        if !self.texture_exists(file_name) {
            if !host.texture_file_exists(file_name) {
                return false;
            }
            let temp = match host.load_surface(file_name) {
                Some(s) => s,
                None => return false,
            };
            let mut w = temp.width;
            let pf = temp.pixel_format.clone();
            let mut levels: Vec<CacheSurface> = Vec::new();
            let mut cnt = 0usize;
            while cnt < MAX_LOD {
                let mut l = CacheSurface { pixel_format: pf.clone(), width: w, height: w, data: vec![0u8; (w.max(0) * w.max(0)) as usize * 0] };
                host.copy_surface(&mut l, &temp);
                levels.push(l);
                w >>= 1;
                cnt += 1;
                if w == 0 {
                    break;
                }
            }
            let small_lod = cnt - 1;
            let large_lod = 0;
            self.save_texture(host, file_name, &levels, large_lod, small_lod, &temp);
        }
        true
    }
}
