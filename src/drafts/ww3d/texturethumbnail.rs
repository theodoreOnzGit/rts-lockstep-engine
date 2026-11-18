//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texturethumbnail.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texturethumbnail.h
//!
//! Texture thumbnails: a 32 x 32 (at most) A4R4G4B4 version of each texture, kept so a texture can be shown at once
//! while its real data loads. A `ThumbnailManagerClass` is a name -> thumbnail hash (one per mix file, plus a global one);
//! the managers upstream hang on statics (`ThumbnailManagerList`, `GlobalThumbnailManager`,
//! `CreateThumbnailIfNotFound`, `message_box_displayed`), which are the fields of [`ThumbnailManagerRegistry`] here.
//!
//! **What upstream really does.** `ThumbnailManagerClass::Load()` and `Save()` are `#if 0` ("don't do thumbnail file",
//! "don't write thumbnails. jba."): the `.th6` file format is never read or written by the game, so thumbnails exist
//! only in memory, made by `ThumbnailClass(manager, filename)` from the DDS or TGA file. The disabled file code is not
//! translated (`Load`/`Save` are the no-ops they compile to, keeping `Changed` handling: `Save` clears it).
//!
//! Disk I/O, the mix-file factories, the DDS / Targa loaders and the Win32 calls are the host's ([`ThumbnailHost`]
//! and [`ThumbnailFiles`]). Reused from the port: `Ww3dFormat`, and `bitmaphandler::copy_image`
//! (`BitmapHandlerClass::Copy_Image`) for the TGA path.
//!
//! Device / OS only, not translated: `::MessageBox` ("Updating texture thumbnails") in `Update_Thumbnail_File`,
//! `GetCurrentDirectory` / `SetCurrentDirectory` / `FindFirstFile` in `Pre_Init` (the host lists the `*.mix` names).

use super::bitmaphandler::copy_image;
use crate::formats::texture::format::Ww3dFormat;
use std::collections::HashMap;

/// `GLOBAL_THUMBNAIL_MANAGER_FILENAME`.
pub const GLOBAL_THUMBNAIL_MANAGER_FILENAME: &str = "global.th6";
/// `ThumbFileHeader`.
pub const THUMB_FILE_HEADER: &[u8; 4] = b"THU6";

/// A DDS file as `ThumbnailClass` reads it (`DDSFileClass(filename, reduction_factor)`, `Load()`).
#[derive(Clone, Debug)]
pub struct DdsThumbSource {
    /// `Get_Date_Time()`.
    pub date_time: u64,
    /// `Get_Width(level)`, `Get_Height(level)` per mip level (with the reduction applied).
    pub level_dims: Vec<(u32, u32)>,
    /// `Get_Full_Width()`, `Get_Full_Height()`.
    pub full_width: u32,
    pub full_height: u32,
    /// `Get_Format()`.
    pub format: Ww3dFormat,
    /// `Get_Mip_Level_Count()`.
    pub mip_level_count: u32,
    /// `Copy_Level_To_Surface(0, WW3D_FORMAT_A4R4G4B4, Width, Height, Bitmap, Width*2, Vector3(0,0,0))`: level 0 as
    /// A4R4G4B4 rows of `Width * 2` bytes ("we don't want to HSV-shift here").
    pub level0_a4r4g4b4: Vec<u8>,
}

/// A Targa file as `ThumbnailClass` reads it.
#[derive(Clone, Debug)]
pub struct TgaThumbSource {
    /// `targa.Header.Width`, `Height` (the host has applied the `ImageDescriptor ^= TGAIDF_YORIGIN` flip: "DX8 uses image
    /// upside down compared to TGA").
    pub width: u32,
    pub height: u32,
    /// `Get_WW3D_Format(src_format, src_bpp, targa)`.
    pub src_format: Ww3dFormat,
    pub src_bpp: u32,
    /// `targa.Header.CMapDepth`.
    pub cmap_depth: u32,
    /// `targa.Load(filename, TGAF_IMAGE, false)` succeeded, with `GetImage()` and `GetPalette()`.
    pub image: Option<Vec<u8>>,
    pub palette: Vec<u8>,
    /// The file's `Get_Date_Time()`.
    pub date_time: u64,
}

/// The image loaders `ThumbnailClass(manager, filename)` calls.
pub trait ThumbnailHost {
    /// `DDSFileClass dds_file(filename, reduction_factor); dds_file.Is_Available() && dds_file.Load()`; `None` otherwise.
    fn load_dds(&mut self, filename: &str, reduction_factor: u32) -> Option<DdsThumbSource>;
    /// `targa.Open(filename, TGA_READMODE)` (`None` when the open fails, `TARGA_ERROR_HANDLER`).
    fn open_tga(&mut self, filename: &str) -> Option<TgaThumbSource>;
}

/// The file operations `Update_Thumbnail_File` and `Create_Thumbnails` make through `SimpleFileFactoryClass` /
/// `MixFileFactoryClass`.
pub trait ThumbnailFiles {
    /// `mix_file->Open(READ)` then `Is_Available()`; its `Get_Date_Time()`. `None` when the mix file is not available.
    fn mix_file_date_time(&mut self, mix_file_name: &str) -> Option<u64>;
    /// `thumb_file->Is_Available()` and its `Get_Date_Time()`; `None` when not available.
    fn thumb_file_date_time(&mut self, thumb_file_name: &str) -> Option<u64>;
    /// `thumb_file->Read(tmp, 4)`.
    fn thumb_file_header(&mut self, thumb_file_name: &str) -> [u8; 4];
    /// `thumb_file->Delete()`.
    fn delete_thumb_file(&mut self, thumb_file_name: &str);
    /// `MixFileFactoryClass mix(MixFileName, &ff); mix.Is_Valid()` then `mix.Build_Filename_List(list)`; `None` when
    /// the mix is not valid. (The texture loads `ThumbnailClass` then makes go through this mix: [`ThumbnailHost`].)
    fn mix_filename_list(&mut self, mix_file_name: &str) -> Option<Vec<String>>;
    /// The `*.mix` files in the `Data` directory (`FindFirstFile("*.mix")`, files only).
    fn list_mix_files(&mut self) -> Vec<String>;
    /// `::MessageBox(NULL, "Some or all texture thumbnails need to be updated. ...")` — device-only; the host shows it.
    fn display_update_message(&mut self);
}

/// `Create_Hash_Name(name, thumb_name)`: the name without its 4-character extension, lowercased. (Upstream asserts the
/// extension is `.tga` or `.dds`.)
pub fn create_hash_name(thumb_name: &str) -> String {
    let len = thumb_name.len();
    debug_assert!(len > 4);
    let ext = &thumb_name[len - 4..];
    debug_assert!(ext.eq_ignore_ascii_case(".tga") || ext.eq_ignore_ascii_case(".dds"));
    thumb_name[..len - 4].to_ascii_lowercase()
}

/// `class ThumbnailClass`.
#[derive(Clone, Debug)]
pub struct ThumbnailClass {
    /// `Name` (with the extension of the file actually found).
    pub name: String,
    /// `Bitmap`: A4R4G4B4, `Width * Height * 2` bytes; empty when no image could be made.
    pub bitmap: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub original_texture_width: u32,
    pub original_texture_height: u32,
    pub original_texture_mip_level_count: u32,
    pub original_texture_format: Ww3dFormat,
    pub date_time: u64,
}

impl ThumbnailClass {
    /// `ThumbnailClass(manager, name, bitmap, w, h, original_w, original_h, mip_count, format, allocated, date_time)`:
    /// the constructor `Load()` used (disabled upstream); inserting into the manager's hash is
    /// [`ThumbnailManagerClass::insert_to_hash`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        bitmap: Vec<u8>,
        w: u32,
        h: u32,
        original_w: u32,
        original_h: u32,
        original_mip_level_count: u32,
        original_format: Ww3dFormat,
        date_time: u64,
    ) -> Self {
        ThumbnailClass {
            name: name.to_string(),
            bitmap,
            width: w,
            height: h,
            original_texture_width: original_w,
            original_texture_height: original_h,
            original_texture_mip_level_count: original_mip_level_count,
            original_texture_format: original_format,
            date_time,
        }
    }

    /// `ThumbnailClass(manager, filename)`: loads the texture from its DDS, else its TGA, and makes the thumbnail. The
    /// returned thumbnail has an empty `bitmap` when nothing could be loaded, and the second value says whether it is to
    /// be inserted into the manager's hash (`Manager->Insert_To_Hash(this)` is the last statement, reached only on
    /// success; the early `return`s of the failures skip it, leaving a half-made object upstream).
    pub fn from_file<H: ThumbnailHost>(host: &mut H, filename: &str) -> (Self, bool) {
        // WWPROFILE(("ThumbnailClass::ThumbnailClass"));
        let mut t = ThumbnailClass {
            name: filename.to_string(),
            bitmap: Vec::new(),
            width: 0,
            height: 0,
            original_texture_width: 0,
            original_texture_height: 0,
            original_texture_mip_level_count: 0,
            original_texture_format: Ww3dFormat::Unknown,
            date_time: 0,
        };
        let mut reduction_factor: u32 = 3;

        // First, try loading image from a DDS file
        if let Some(dds) = host.load_dds(filename, reduction_factor) {
            t.date_time = dds.date_time;
            let len = t.name.len();
            debug_assert!(len > 4);
            t.name.replace_range(len - 3.., "dds");

            // (the result of this search is not used upstream either)
            let mut level = 0u32;
            let dim = |l: u32| dds.level_dims.get(l as usize).copied().unwrap_or((0, 0));
            while dim(level).0 > 32 || dim(level).1 > 32 {
                if level >= dds.mip_level_count {
                    break;
                }
                level += 1;
            }

            t.original_texture_width = dds.full_width;
            t.original_texture_height = dds.full_height;
            t.original_texture_format = dds.format;
            t.original_texture_mip_level_count = dds.mip_level_count;
            t.width = dim(0).0;
            t.height = dim(0).1;
            t.bitmap = dds.level0_a4r4g4b4.clone(); // Copy_Level_To_Surface(0, A4R4G4B4, ...)
            return (t, true);
        }

        // If DDS file can't be used try loading from TGA
        // Make sure the file can be opened. If not, return missing texture.
        let tga = match host.open_tga(filename) {
            Some(tga) => tga,
            None => return (t, false),
        };
        if tga.src_format == Ww3dFormat::Unknown {
            // WWDEBUG_SAY(("Unknown texture format for %s\n", filename));
            return (t, false);
        }

        // Destination size will be the next power of two square from the larger width and height...
        t.original_texture_width = tga.width;
        t.original_texture_height = tga.height;
        t.original_texture_format = tga.src_format;
        t.width = tga.width >> reduction_factor;
        t.height = tga.height >> reduction_factor;
        t.original_texture_mip_level_count = 1;
        let mut iw: u32 = 1;
        let mut ih: u32 = 1;
        while iw < t.original_texture_width && ih < t.original_texture_height {
            iw += iw;
            ih += ih;
            t.original_texture_mip_level_count += 1;
        }
        while t.width > 32 || t.height > 32 {
            reduction_factor += 1;
            t.width >>= 2;
            t.height >>= 2;
        }
        let mut poweroftwowidth: u32 = 1;
        while poweroftwowidth < t.width {
            poweroftwowidth <<= 1;
        }
        let mut poweroftwoheight: u32 = 1;
        while poweroftwoheight < t.height {
            poweroftwoheight <<= 1;
        }
        t.width = poweroftwowidth;
        t.height = poweroftwoheight;
        let src_width = tga.width;
        let src_height = tga.height;

        // NOTE: We load the palette but we do not yet support paletted textures!
        let mut src_surface = match tga.image {
            Some(img) => img,
            None => return (t, false), // TARGA_ERROR_HANDLER(targa.Load(..))
        };

        // Get time stamp from the tga file
        t.date_time = tga.date_time;

        let len = t.name.len();
        debug_assert!(len > 4);
        t.name.replace_range(len - 3.., "tga");

        t.bitmap = vec![0u8; (t.width * t.height * 2) as usize];
        // dest_format = WW3D_FORMAT_A8R8G8B8 (assigned, never used upstream)
        copy_image(
            &mut t.bitmap,
            t.width,
            t.height,
            t.width * 2,
            Ww3dFormat::A4R4G4B4,
            &mut src_surface,
            src_width,
            src_height,
            src_width * tga.src_bpp,
            tga.src_format,
            &tga.palette,
            tga.cmap_depth >> 3,
            false,
            [0.0, 0.0, 0.0],
        );
        (t, true)
    }

    /// `Peek_Bitmap()`.
    pub fn peek_bitmap(&self) -> &[u8] {
        &self.bitmap
    }

    /// `Get_Format()`: always `WW3D_FORMAT_A4R4G4B4`.
    pub fn get_format(&self) -> Ww3dFormat {
        Ww3dFormat::A4R4G4B4
    }
}

/// `class ThumbnailManagerClass`.
#[derive(Clone, Debug)]
pub struct ThumbnailManagerClass {
    /// `PerTextureTimeStampUsed`.
    pub per_texture_time_stamp_used: bool,
    /// `ThumbnailFileName`.
    pub thumbnail_file_name: String,
    /// `MixFileName` (NULL for the global manager).
    pub mix_file_name: Option<String>,
    /// `ThumbnailHash`: hash name -> thumbnail.
    pub thumbnail_hash: HashMap<String, ThumbnailClass>,
    /// `Changed`.
    pub changed: bool,
    /// `DateTime`.
    pub date_time: u64,
}

impl ThumbnailManagerClass {
    /// `ThumbnailManagerClass(thumbnail_filename, mix_file_name)`; its `Load()` is disabled upstream (`#if 0`).
    pub fn new(thumbnail_filename: &str, mix_file_name: Option<&str>) -> Self {
        ThumbnailManagerClass {
            per_texture_time_stamp_used: false,
            thumbnail_file_name: thumbnail_filename.to_string(),
            mix_file_name: mix_file_name.map(|s| s.to_string()),
            thumbnail_hash: HashMap::new(),
            changed: false,
            date_time: 0,
        }
    }

    /// `Save(force)`: "Save only if changed"; the file writing is `#if 0`, so this only clears `Changed`.
    pub fn save(&mut self, force: bool) {
        if !self.changed && !force {
            return;
        }
        self.changed = false;
    }

    /// `Insert_To_Hash(thumb)`: `Changed = true`, keyed by the hash name.
    pub fn insert_to_hash(&mut self, thumb: ThumbnailClass) {
        self.changed = true;
        let hash_name = create_hash_name(&thumb.name);
        self.thumbnail_hash.insert(hash_name, thumb);
    }

    /// `Get_From_Hash(name)`.
    pub fn get_from_hash(&self, name: &str) -> Option<&ThumbnailClass> {
        let hash_name = create_hash_name(name);
        self.thumbnail_hash.get(&hash_name)
    }

    /// `Remove_From_Hash(thumb)` (`~ThumbnailClass`): `Changed = true`, removes the entry named like `thumb_name`.
    pub fn remove_from_hash(&mut self, thumb_name: &str) {
        self.changed = true;
        let hash_name = create_hash_name(thumb_name);
        self.thumbnail_hash.remove(&hash_name);
    }

    /// `Peek_Thumbnail_Instance(name)`.
    pub fn peek_thumbnail_instance(&self, name: &str) -> Option<&ThumbnailClass> {
        self.get_from_hash(name)
    }

    /// `Is_Per_Texture_Time_Stamp_Used()`.
    pub fn is_per_texture_time_stamp_used(&self) -> bool {
        self.per_texture_time_stamp_used
    }

    /// `Enable_Per_Texture_Time_Stamp(enable)`.
    pub fn enable_per_texture_time_stamp(&mut self, enable: bool) {
        self.per_texture_time_stamp_used = enable;
    }

    /// `ThumbnailManagerClass::~ThumbnailManagerClass()`: `Save()`, then every thumbnail is deleted.
    pub fn destroy(mut self) {
        self.save(false);
        self.thumbnail_hash.clear();
    }

    /// `Create_Thumbnails()`: a thumbnail for every `.tga` / `.dds` in the mix file that has none; each is removed from
    /// the global manager when it holds one for the same texture. (Upstream swaps `_TheFileFactory` to the mix for the
    /// duration; the host's `ThumbnailHost` reads through that mix.)
    pub fn create_thumbnails<F: ThumbnailFiles, H: ThumbnailHost>(&mut self, files: &mut F, host: &mut H, mut global: Option<&mut ThumbnailManagerClass>) {
        let mix_name = self.mix_file_name.clone().unwrap_or_default();
        if let Some(list) = files.mix_filename_list(&mix_name) {
            for tex_name in list.iter() {
                let len = tex_name.len();
                if len < 4 {
                    continue;
                }
                let ext = &tex_name[len - 4..];
                if ext.eq_ignore_ascii_case(".tga") || ext.eq_ignore_ascii_case(".dds") {
                    if self.peek_thumbnail_instance(tex_name).is_none() {
                        let (thumb, insert) = ThumbnailClass::from_file(host, tex_name);
                        if insert {
                            self.insert_to_hash(thumb);
                        }
                    }
                    // Remove from Global manager if texture exists in it
                    if let Some(g) = global.as_mut() {
                        let g_thumb_name = g.peek_thumbnail_instance(tex_name).map(|t| t.name.clone());
                        if let Some(n) = g_thumb_name {
                            g.remove_from_hash(&n); // delete g_thumb
                        }
                    }
                }
            }
        }
    }
}

/// The statics of texturethumbnail.cpp: `ThumbnailManagerList`, `GlobalThumbnailManager`,
/// `ThumbnailManagerClass::CreateThumbnailIfNotFound`, `message_box_displayed`.
#[derive(Clone, Debug, Default)]
pub struct ThumbnailManagerRegistry {
    /// `ThumbnailManagerList`.
    pub managers: Vec<ThumbnailManagerClass>,
    /// `GlobalThumbnailManager`.
    pub global_thumbnail_manager: Option<ThumbnailManagerClass>,
    /// `CreateThumbnailIfNotFound`: false by default.
    pub create_thumbnail_if_not_found: bool,
    /// `message_box_displayed`.
    pub message_box_displayed: bool,
}

impl ThumbnailManagerRegistry {
    /// `Is_Thumbnail_Created_If_Not_Found()`.
    pub fn is_thumbnail_created_if_not_found(&self) -> bool {
        self.create_thumbnail_if_not_found
    }

    /// `Create_Thumbnail_If_Not_Found(create)`.
    pub fn create_thumbnail_if_not_found_set(&mut self, create: bool) {
        self.create_thumbnail_if_not_found = create;
    }

    /// `Peek_Thumbnail_Manager(thumbnail_filename)`: the list first, then the global manager. Returns the index into
    /// [`Self::managers`], or `None` with the global flag.
    pub fn peek_thumbnail_manager(&self, thumbnail_filename: &str) -> Option<ManagerRef> {
        for (i, man) in self.managers.iter().enumerate() {
            if man.thumbnail_file_name == thumbnail_filename {
                return Some(ManagerRef::List(i));
            }
        }
        if let Some(g) = &self.global_thumbnail_manager {
            if g.thumbnail_file_name == thumbnail_filename {
                return Some(ManagerRef::Global);
            }
        }
        None
    }

    /// `Add_Thumbnail_Manager(thumbnail_filename, mix_file_name)`: "Must NOT add global manager with this function".
    /// An existing manager of that name is left as it is; otherwise the thumbnail file is brought up to date
    /// ([`Self::update_thumbnail_file`]) and a manager appended.
    pub fn add_thumbnail_manager<F: ThumbnailFiles, H: ThumbnailHost>(&mut self, files: &mut F, host: &mut H, thumbnail_filename: &str, mix_file_name: &str) {
        debug_assert!(!thumbnail_filename.eq_ignore_ascii_case(GLOBAL_THUMBNAIL_MANAGER_FILENAME));
        if self.peek_thumbnail_manager(thumbnail_filename).is_some() {
            return;
        }
        // Always update thumbnail files when they're out of date
        self.update_thumbnail_file(files, host, Some(mix_file_name), false);
        // Not found, create and add to the list.
        self.managers.push(ThumbnailManagerClass::new(thumbnail_filename, Some(mix_file_name)));
    }

    /// `Remove_Thumbnail_Manager(thumbnail_filename)`: deletes the first list manager of that name, else the global one if
    /// it matches.
    pub fn remove_thumbnail_manager(&mut self, thumbnail_filename: &str) {
        if let Some(i) = self.managers.iter().position(|m| m.thumbnail_file_name == thumbnail_filename) {
            self.managers.remove(i).destroy();
            return;
        }
        if let Some(g) = &self.global_thumbnail_manager {
            if g.thumbnail_file_name == thumbnail_filename {
                self.global_thumbnail_manager.take().unwrap().destroy();
            }
        }
    }

    /// `Peek_Thumbnail_Instance_From_Any_Manager(filename)`: the list, then the global manager; failing that, when
    /// thumbnails are created if not found, one is made in the global manager (a texture outside any mix file) and
    /// returned if it has a bitmap.
    pub fn peek_thumbnail_instance_from_any_manager<H: ThumbnailHost>(&mut self, host: &mut H, filename: &str) -> Option<&ThumbnailClass> {
        // WWPROFILE(("Peek_Thumbnail_Instance_From_Any_Manager"));
        let mut found: Option<ManagerRef> = None;
        for (i, man) in self.managers.iter().enumerate() {
            if man.peek_thumbnail_instance(filename).is_some() {
                found = Some(ManagerRef::List(i));
                break;
            }
        }
        if found.is_none() {
            if let Some(g) = &self.global_thumbnail_manager {
                if g.peek_thumbnail_instance(filename).is_some() {
                    found = Some(ManagerRef::Global);
                }
            }
        }
        match found {
            Some(ManagerRef::List(i)) => return self.managers[i].peek_thumbnail_instance(filename),
            Some(ManagerRef::Global) => return self.global_thumbnail_manager.as_ref().unwrap().peek_thumbnail_instance(filename),
            None => {}
        }

        // If thumbnail is not found, see if we can find a texture. It is possible that the texture is outside of a mix file and
        // didn't get included in any thumbnail database based on a mixfile. If so, we'll add it to our global thumbnail database.
        if self.is_thumbnail_created_if_not_found() {
            if let Some(g) = self.global_thumbnail_manager.as_mut() {
                let (thumb, insert) = ThumbnailClass::from_file(host, filename);
                if thumb.peek_bitmap().is_empty() {
                    // delete thumb: it was never inserted; ~ThumbnailClass still calls Remove_From_Hash (Changed = true)
                    g.remove_from_hash(&thumb.name);
                    return None;
                }
                if insert {
                    let key = create_hash_name(&thumb.name);
                    g.insert_to_hash(thumb);
                    return g.thumbnail_hash.get(&key);
                }
                return None;
            }
        }
        None
    }

    /// `Update_Thumbnail_File(mix_file_name, display_message_box)`: makes sure a `.th6` exists for the mix file and is
    /// current: a thumbnail file whose date differs from the mix's, or whose header is not `THU6`, is deleted; when there
    /// is none afterwards a manager is run to create the thumbnails and `Save(true)`d (which writes nothing; `#if 0`).
    pub fn update_thumbnail_file<F: ThumbnailFiles, H: ThumbnailHost>(&mut self, files: &mut F, host: &mut H, mix_file_name: Option<&str>, display_message_box: bool) {
        let mix_file_name = match mix_file_name {
            Some(n) => n,
            None => return,
        };
        let mut thumb_file_name = mix_file_name.to_string();
        let len = thumb_file_name.len();
        debug_assert!(len > 4);
        thumb_file_name.replace_range(len - 3.., "th6");

        // If mix file isn't found but thumb file is, delete the obsolete thumb file
        let mix_date_time = match files.mix_file_date_time(mix_file_name) {
            Some(d) => d,
            None => {
                if files.thumb_file_date_time(&thumb_file_name).is_some() {
                    files.delete_thumb_file(&thumb_file_name);
                }
                return;
            }
        };

        let mut thumb_available = files.thumb_file_date_time(&thumb_file_name);
        if let Some(thumb_date_time) = thumb_available {
            if mix_date_time != thumb_date_time {
                files.delete_thumb_file(&thumb_file_name);
                thumb_available = None;
            }
            // Read header to make sure the thumb file is of correct version.
            let tmp = files.thumb_file_header(&thumb_file_name);
            if &tmp != THUMB_FILE_HEADER {
                files.delete_thumb_file(&thumb_file_name);
                thumb_available = None;
            }
        }
        if thumb_available.is_some() {
            return;
        }

        if display_message_box && !self.message_box_displayed {
            self.message_box_displayed = true;
            files.display_update_message(); // ::MessageBox(...) — device-only, not translated
        }

        // we don't currently have a thumbnail file (either we just deleted it or it never existed, we don't care) so we
        // must create one now.
        let mut manager = ThumbnailManagerClass::new(&thumb_file_name, Some(mix_file_name));
        manager.date_time = mix_date_time; // Set the datetime to mixfile's datetime.
        manager.create_thumbnails(files, host, self.global_thumbnail_manager.as_mut());
        manager.save(true);
        manager.destroy();
    }

    /// `Pre_Init(display_message_box)`: "Verify that up-to-date thumbnails exist for all textures": `always.dat` first, then
    /// each `*.mix` in the `Data` directory. (`WWASSERT(!ThumbnailManagerList.Head())`.)
    pub fn pre_init<F: ThumbnailFiles, H: ThumbnailHost>(&mut self, files: &mut F, host: &mut H, display_message_box: bool) {
        debug_assert!(self.managers.is_empty());
        // Collect all mix file names
        let mix_names = files.list_mix_files();
        // First generate thumbnails for always.dat
        self.update_thumbnail_file(files, host, Some("always.dat"), display_message_box);
        // Then loop over all .mix files
        for n in mix_names.iter() {
            self.update_thumbnail_file(files, host, Some(n), display_message_box);
        }
    }

    /// `Init()`: the global manager, with per-texture time stamps on.
    pub fn init(&mut self) {
        debug_assert!(self.global_thumbnail_manager.is_none());
        let mut g = ThumbnailManagerClass::new(GLOBAL_THUMBNAIL_MANAGER_FILENAME, None);
        g.enable_per_texture_time_stamp(true);
        self.global_thumbnail_manager = Some(g);
    }

    /// `Deinit()`: deletes every manager in the list, then the global one.
    pub fn deinit(&mut self) {
        while !self.managers.is_empty() {
            self.managers.remove(0).destroy();
        }
        if let Some(g) = self.global_thumbnail_manager.take() {
            g.destroy();
        }
    }
}

/// Where a manager is: the list, or the global slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagerRef {
    List(usize),
    Global,
}
