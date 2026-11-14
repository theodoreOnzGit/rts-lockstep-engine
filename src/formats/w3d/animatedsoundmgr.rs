// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/animatedsoundmgr.cpp, animatedsoundmgr.h
//   (AnimatedSoundMgrClass: Initialize, Shutdown, Get_Embedded_Sound_Name, Find_Sound_List, Trigger_Sound,
//   Set_Sound_Library; the file's static Build_List_From_String and Is_In_Param_List)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/soundlibrarybridge.h (SoundLibraryBridgeClass)
//   (strtrim, which StringClass::Trim calls, is in ww::trim; INIClass in ww::ini)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, animatedsoundmgr by Patrick Smith, MBL's CNC3 INCURSION update of 10.23.2002)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The animation-triggered sounds (`AnimatedSoundMgrClass`, GitHub #181 item 3, #163 item 2): a table, read from
//! `w3danimsound.ini`, of sounds keyed by animation name and frame, which `Animatable3DObjClass::
//! Update_Sub_Object_Transforms` fires as an animation's frame passes them (see
//! [`crate::client::w3d_model_draw::RenderObj::update_sub_object_transforms`]).
//!
//! # What Zero Hour does with it
//!
//! - `WW3D::Init` (ww3d.cpp:312, `lite` false from `W3DDisplay::init`) calls `Initialize()`, which reads
//!   `w3danimsound.ini` through `_TheFileFactory` (the game's `W3DFileSystem`). Whether Zero Hour's archives hold that
//!   file is **not checked**: no game file is in this repository or on the machine this was written on.
//! - **`Set_Sound_Library` is never called** anywhere in GeneralsMD/Code (a grep of GameEngine, GameEngineDevice and
//!   Libraries finds only its definition). So `SoundLibrary` stays `NULL`, and [`trigger_sound`] returns `old_frame`
//!   at its first line every time: no sound reaches the game's audio, and `PrevFrame = Trigger_Sound(..., PrevFrame,
//!   ...)` writes back the value it read. Its observable effect in Zero Hour is nil, whatever the table holds. The
//!   one thing the call still does is evaluate `HTree->Get_Transform(bone)` for its argument (a read).
//! - Nothing here draws a random value, logic or client.
//!
//! # As upstream keeps it
//!
//! The class is all statics; here one process-wide [`AnimatedSoundMgr`] behind a lock ([`initialize`],
//! [`shutdown`], [`get_embedded_sound_name`], [`trigger_sound`], [`set_sound_library`] reach it), so every render
//! object shares it as upstream's do. ~~`AnimationNameHash` (a `HashTemplateClass<StringClass, ANIM_SOUND_LIST *>`, whose
//! `Insert` puts a new entry first in its bucket and whose `Get` answers the first equal key) is a map in which a
//! later insert of an equal name replaces the earlier: the same answer, except after a `Re_Hash` reorders a bucket
//! (two sections with one upper-case name; not compared, see the gaps in docs/phase2-client.md).~~ **Corrected
//! 2026-11-14 (#181 item 3, INIClass):** `AnimationNameHash` is upstream's `HashTemplateClass<StringClass, ...>`
//! translated ([`NameHash`]: `Insert` first in its bucket, `Get` the first equal key, `Re_Hash` when the table is full,
//! which reverses a bucket's order, `Remove_All` keeping the table's size). The map that stood for it answered the later
//! of two sections with one upper-case name where upstream, after a `Re_Hash`, answers the earlier: found by
//! tests/differential_iniclass.rs (a port defect).
//!
//! ~~The INI file itself (`INIClass`, WWLib/ini.cpp: `Load`, `Find_Section` by CRC, the duplicate-entry check) is
//! **not translated**: [`initialize`] takes the sections as `INIClass` holds them after loading (`IniSection`).~~
//! **Corrected 2026-11-14 (#181 item 3, INIClass):** the INI file is read by WWLib's `INIClass`, translated in
//! [`crate::ww::ini`]: [`initialize_file`] is the whole of `Initialize` (the file through `_TheFileFactory`, i.e.
//! [`W3dFileFactory`] over the port's [`FileSystem`], as the W3D assets are read), and [`initialize`] the part after
//! `Get_INI`, on an [`IniClass`].
//! The bridge to the audio (`SoundLibraryBridgeClass`, which no Zero Hour code implements) is
//! [`SoundLibraryBridge`], whose one implementation records the calls.
//!
//! The strings are C strings throughout: upstream's `StringClass` keeps a stored length that `Trim()` and a written
//! terminator do not update, and the names reach the bridge through `Peek_Buffer()`, so only the bytes before the
//! first NUL matter; where the stored length is read (`Build_List_From_String`'s `Get_Length() > 0`) it is
//! reproduced.

use std::sync::RwLock;

use super::assetmgr::W3dFileFactory;
use super::{W3dError, W3dResult, W3dUndefined};
use crate::common::file_system::{FileSystem, LocalDevice};
use crate::logic::modules::plant;
use crate::ww::ini::IniClass;
use crate::ww::trim::c_str;
use crate::ww::matrix3d::Matrix3D;

/// `AnimatedSoundMgrClass::AnimSoundInfo` (`ANIM_SOUND_INFO`): a sound at a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimSoundInfo {
    /// `Frame`: `atoi` of the first parameter.
    pub frame: i32,
    /// `SoundName` (its C string): the second parameter, trimmed.
    pub sound_name: Vec<u8>,
    /// `Is2D`: a later parameter contains `"2D"`.
    pub is_2d: bool,
    /// `IsStop`: a later parameter contains `"STOP"`.
    pub is_stop: bool,
}

/// `AnimatedSoundMgrClass::AnimSoundList` (`ANIM_SOUND_LIST`): an animation's sounds, in the order of the INI's
/// entries, and the bone they play at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimSoundList {
    /// `List`.
    pub list: Vec<AnimSoundInfo>,
    /// `BoneName` (`"root"` unless a `BoneName` entry gives one).
    pub bone_name: Vec<u8>,
}

impl AnimSoundList {
    /// `AnimSoundList()`: no sounds, the bone `"root"`.
    fn new() -> Self {
        AnimSoundList { list: Vec::new(), bone_name: b"root".to_vec() }
    }
}

/// A call the manager made to the sound library (`SoundLibraryBridgeClass`'s three methods).
#[derive(Clone, Debug, PartialEq)]
pub enum SoundLibraryCall {
    /// `Play_3D_Audio(name, tm)`.
    Play3D { name: Vec<u8>, tm: Matrix3D },
    /// `Play_2D_Audio(name)`.
    Play2D { name: Vec<u8> },
    /// `Stop_Playing_Audio(name)`.
    Stop { name: Vec<u8> },
}

/// `SoundLibraryBridgeClass` ("Bridges E&B code with WW3D"), an interface no Zero Hour code implements (module
/// docs). New: the port's one bridge, which records each call (for the tests; nothing plays them).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum SoundLibraryBridge {
    #[default]
    Recorder,
}

/// `AnimatedSoundMgrClass`'s static members.
#[derive(Debug, Default)]
pub struct AnimatedSoundMgr {
    /// `AnimationNameHash`: an upper-case animation name to its list in [`AnimatedSoundMgr::anim_sound_lists`].
    animation_name_hash: NameHash,
    /// `AnimSoundLists`.
    anim_sound_lists: Vec<AnimSoundList>,
    /// `SoundLibrary` (`NULL`, upstream's only value in Zero Hour, is `None`).
    sound_library: Option<SoundLibraryBridge>,
    /// The calls [`SoundLibraryBridge::Recorder`] recorded, oldest first.
    recorded: Vec<SoundLibraryCall>,
}

impl AnimatedSoundMgr {
    const fn new() -> Self {
        AnimatedSoundMgr { animation_name_hash: NameHash::new(), anim_sound_lists: Vec::new(), sound_library: None, recorded: Vec::new() }
    }

    /// `Find_Sound_List(anim)`: the list of the animation's name made upper case (`strupr`), if any.
    fn find_sound_list(&self, anim_name: &[u8]) -> Option<&AnimSoundList> {
        let full_name: Vec<u8> = if plant(10063) { c_str(anim_name).to_vec() } else { c_str(anim_name).to_ascii_uppercase() };
        self.animation_name_hash.get(&full_name).and_then(|i| self.anim_sound_lists.get(i))
    }

    /// The bridge's method for one sound (`SoundLibrary->...`), recorded.
    fn call(&mut self, call: SoundLibraryCall) {
        match self.sound_library {
            Some(SoundLibraryBridge::Recorder) => self.recorded.push(call),
            None => {}
        }
    }
}

/// `HashTemplateClass<StringClass, ANIM_SOUND_LIST *>` (WWLib/hashtemplate.h), the type of `AnimationNameHash`, with
/// the list's position as its value: `Table` of entries chained by index (`Next`, `NIL` -1), `Hash` the first entry of
/// each bucket, `First` the first free entry, `Size` the table's size (a power of two, 0 before the first insert).
#[derive(Clone, Debug, Default)]
pub struct NameHash {
    /// `Hash`.
    hash: Vec<i32>,
    /// `Table`: (`Next`, `Key`, `Value`).
    table: Vec<(i32, Vec<u8>, usize)>,
    /// `First`.
    first: i32,
    /// `Size`.
    size: u32,
}

/// `HashTemplateClass::NIL`.
const NIL: i32 = -1;

impl NameHash {
    /// `HashTemplateClass()`: no table (`First` NIL, `Size` 0).
    pub const fn new() -> Self {
        NameHash { hash: Vec::new(), table: Vec::new(), first: NIL, size: 0 }
    }

    /// `HashTemplateKeyClass<StringClass>::Get_Hash_Value(s)`: for a name shorter than 8 bytes `hval += 37 * hval +
    /// byte` over its bytes; otherwise the 4 bytes 8 from its end read as a little-endian `unsigned int`, then `hval +
    /// (hval >> 5) + (hval >> 10) + (hval >> 20)` (all wrapping). The length is `Get_Length()`, the C string's here.
    pub fn get_hash_value(s: &[u8]) -> u32 {
        let len = s.len();
        if len < 8 {
            let mut hval: u32 = 0;
            for &b in s {
                hval = hval.wrapping_add(37u32.wrapping_mul(hval).wrapping_add(u32::from(b)));
            }
            return hval;
        }
        let hval = u32::from_le_bytes([s[len - 8], s[len - 7], s[len - 6], s[len - 5]]);
        hval.wrapping_add(hval >> 5).wrapping_add(hval >> 10).wrapping_add(hval >> 20)
    }

    /// `Get_Hash_Val(s, size)`: the hash value masked to the table.
    fn get_hash_val(s: &[u8], size: u32) -> usize {
        (Self::get_hash_value(s) & size.wrapping_sub(1)) as usize
    }

    /// `Re_Hash()`: a table of twice the size (at least 4); each old bucket in order, each chain from its head, is
    /// copied to the next new entry and put first in its new bucket; the rest is the free list.
    fn re_hash(&mut self) {
        let new_size = (self.size * 2).max(4);
        let mut new_table: Vec<(i32, Vec<u8>, usize)> = (0..new_size).map(|_| (NIL, Vec::new(), 0)).collect();
        let mut new_hash: Vec<i32> = vec![NIL; new_size as usize];
        let mut cnt = 0usize;
        if self.size != 0 {
            for i in 0..self.size as usize {
                let mut h = self.hash[i];
                while h != NIL {
                    let (next, key, value) = self.table[h as usize].clone();
                    let hval = Self::get_hash_val(&key, new_size);
                    new_table[cnt] = (new_hash[hval], key, value);
                    new_hash[hval] = cnt as i32;
                    cnt += 1;
                    h = next;
                }
            }
        }
        for i in cnt..new_size as usize {
            new_table[i].0 = i as i32 + 1;
        }
        new_table[new_size as usize - 1].0 = NIL;
        self.first = cnt as i32;
        self.hash = new_hash;
        self.table = new_table;
        self.size = new_size;
    }

    /// `Alloc_Entry()`: the first free entry, the table re-hashed first when there is none.
    fn alloc_entry(&mut self) -> usize {
        if self.first == NIL {
            self.re_hash();
        }
        let h = self.first as usize;
        self.first = self.table[h].0;
        h
    }

    /// `Insert(s, d)`: a new entry, first in its bucket (an equal key already there stays, behind it).
    pub fn insert(&mut self, s: &[u8], d: usize) {
        let h = self.alloc_entry();
        let hval = Self::get_hash_val(s, self.size);
        self.table[h] = (self.hash[hval], s.to_vec(), d);
        self.hash[hval] = h as i32;
    }

    /// `Get(s)`: the value of the first entry of its bucket whose key equals `s` (`ValueType(0)`, `NULL`: `None`).
    pub fn get(&self, s: &[u8]) -> Option<usize> {
        if self.hash.is_empty() {
            return None;
        }
        let mut h = self.hash[Self::get_hash_val(s, self.size)];
        while h != NIL {
            let (next, key, value) = &self.table[h as usize];
            if key == s {
                return Some(*value);
            }
            h = *next;
        }
        None
    }

    /// `Remove_All()`: each bucket's chain put at the head of the free list, the table's size kept.
    pub fn remove_all(&mut self) {
        for i in 0..self.size as usize {
            let f = self.hash[i];
            if f != NIL {
                let mut h = f;
                while self.table[h as usize].0 != NIL {
                    h = self.table[h as usize].0;
                }
                self.table[h as usize].0 = self.first;
                self.first = f;
                self.hash[i] = NIL;
            }
        }
    }

    /// New: every (key, value) in the table, bucket by bucket (the order of `HashTemplateIterator`).
    pub fn entries(&self) -> Vec<(Vec<u8>, usize)> {
        let mut v = Vec::new();
        for &head in &self.hash {
            let mut h = head;
            while h != NIL {
                let (next, key, value) = &self.table[h as usize];
                v.push((key.clone(), *value));
                h = *next;
            }
        }
        v
    }
}

/// The statics, shared by every render object of the process as upstream's are.
static MGR: RwLock<AnimatedSoundMgr> = RwLock::new(AnimatedSoundMgr::new());

fn mgr_read() -> std::sync::RwLockReadGuard<'static, AnimatedSoundMgr> {
    MGR.read().unwrap_or_else(|e| e.into_inner())
}

fn mgr_write() -> std::sync::RwLockWriteGuard<'static, AnimatedSoundMgr> {
    MGR.write().unwrap_or_else(|e| e.into_inner())
}

pub use crate::ww::trim::strtrim;

/// The CRT's `atoi` on a C string: white space (`isspace` in the C locale), a sign, decimal digits. A value outside
/// `int` is where the CRTs differ (MSVC's `atol` wraps, glibc's `strtol` saturates a `long`): not translated.
pub fn atoi(s: &[u8]) -> W3dResult<i32> {
    let s = c_str(s);
    let mut i = s.iter().take_while(|&&c| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')).count();
    let mut negative = false;
    if let Some(&c) = s.get(i) {
        if c == b'-' || c == b'+' {
            negative = c == b'-' && !plant(10073);
            i += 1;
        }
    }
    let mut v: i64 = 0;
    while let Some(&c) = s.get(i) {
        if !c.is_ascii_digit() {
            break;
        }
        v = v * 10 + i64::from(c - b'0');
        if v > i64::from(i32::MAX) + 1 {
            return Err(W3dError::Unsupported("atoi of a number outside int (the CRTs differ)"));
        }
        i += 1;
    }
    let v = if negative { -v } else { v };
    i32::try_from(v).map_err(|_| W3dError::Unsupported("atoi of a number outside int (the CRTs differ)"))
}

/// `strnicmp(&buffer[at..], delimiter, strlen(delimiter)) == 0` (ASCII case folded; the buffer's NUL ends it).
fn strnicmp_eq(buffer: &[u8], at: usize, delimiter: &[u8]) -> bool {
    delimiter.iter().enumerate().all(|(k, &d)| buffer.get(at + k).copied().unwrap_or(0).eq_ignore_ascii_case(&d))
}

/// `strstr(&buffer[from..], needle)`: the index of the first occurrence at or after `from`.
fn strstr(buffer: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(from);
    }
    (from..buffer.len()).find(|&i| buffer[i..].starts_with(needle))
}

/// `Build_List_From_String(buffer, delimiter, &string_list)` (animatedsoundmgr.cpp, a file `static`): the entries
/// between the delimiters, each a C string. Two passes over the buffer: the first counts (`(entry != NULL) &&
/// (entry[1] != 0)`, the delimiter at `entry` skipped after the first entry), the second copies each entry and cuts it
/// at its delimiter, keeping it when `Get_Length() > 0`: the stored length of the copy made before the cut, which is
/// the length of the whole rest of the buffer, so an empty entry is kept too. A buffer of one character has no entry
/// in the first pass and becomes a list of itself (`delim_len > 0`). An empty buffer reads `entry[1]` past its
/// terminator: undefined.
pub fn build_list_from_string(buffer: &[u8], delimiter: &[u8]) -> W3dResult<Vec<Vec<u8>>> {
    let buffer = c_str(buffer);
    let delim_len = delimiter.len();
    let at = |i: usize| buffer.get(i).copied().unwrap_or(0);
    let cond = |e: usize| -> W3dResult<bool> {
        if e >= buffer.len() {
            // `entry[1]` past the terminator (only an empty buffer reaches it)
            return Err(W3dUndefined::StringOverrun.into());
        }
        Ok(at(e + 1) != 0)
    };
    //
    // Determine how many entries there will be in the list
    //
    let mut count = 0usize;
    let mut entry = Some(0usize);
    while let Some(mut e) = entry {
        if !cond(e)? {
            break;
        }
        // Move past the current delimiter (if necessary)
        if strnicmp_eq(buffer, e, delimiter) && count > 0 {
            e += delim_len;
        }
        count += 1;
        entry = strstr(buffer, e, delimiter);
    }
    let mut string_list: Vec<Vec<u8>> = Vec::new();
    if count > 0 {
        //
        // Parse the string and pull out its entries.
        //
        let mut entry = Some(0usize);
        while let Some(mut e) = entry {
            if !cond(e)? {
                break;
            }
            if strnicmp_eq(buffer, e, delimiter) && string_list.len() > 0 {
                e += delim_len;
            }
            // Copy this entry into its own string
            let stored_length = buffer.len() - e;
            let mut entry_string = buffer[e..].to_vec();
            if let Some(d) = strstr(&entry_string, 0, delimiter) {
                entry_string.truncate(d);
            }
            // Add this entry to our list
            let length = if plant(10062) { entry_string.len() } else { stored_length };
            if length > 0 || string_list.is_empty() {
                string_list.push(entry_string);
            }
            entry = strstr(buffer, e, delimiter);
        }
    } else if delim_len > 0 {
        string_list.push(buffer.to_vec());
    }
    Ok(string_list)
}

/// `Is_In_Param_List(param_list, param_count, param_to_check)` (a file `static`): whether a parameter after the
/// first two (frame and name) contains `param_to_check` (`strstr`, case sensitive: "Breaks with whitespaces" says the
/// comment of the `stricmp` it replaced); `false` for fewer than two parameters.
pub fn is_in_param_list(param_list: &[Vec<u8>], param_to_check: &[u8]) -> bool {
    if param_list.len() < 2 {
        return false;
    }
    // Note: params 0 & 1 are fixed to frame and name...
    let first = if plant(10060) { 1 } else { 2 };
    param_list.iter().skip(first).any(|p| if plant(10075) { c_str(p) == param_to_check } else { strstr(c_str(p), 0, param_to_check).is_some() })
}

/// `Initialize`'s `DEFAULT_INI_FILENAME`.
pub const DEFAULT_INI_FILENAME: &[u8] = b"w3danimsound.ini";

/// `Get_INI(filename)` (animatedsoundmgr.cpp, a file `static`): the file from `_TheFileFactory` (the game's
/// `W3DFileSystem`, here [`W3dFileFactory`] over `fs`, as the W3D assets are read) and, when it is available, an
/// `INIClass` loaded from it ([`IniClass::from_file`]); `None` when it is not.
pub fn get_ini<D: LocalDevice>(factory: &W3dFileFactory, fs: &FileSystem<D>, filename: &[u8]) -> W3dResult<Option<IniClass>> {
    // FileClass *file = _TheFileFactory->Get_File (filename); file->Is_Available ()
    let Some(path) = factory.resolve(fs, filename)? else { return Ok(None) };
    // new INIClass (*file): Load(FileClass &) reads the file through a FileStraw, which opens it
    let data = fs.open_file(&path);
    let ini = IniClass::from_file(filename, data.as_deref()).map_err(|_| W3dError::Unsupported("INIClass::Load's merge branch"))?;
    Ok(Some(ini))
}

/// `AnimatedSoundMgrClass::Initialize(ini_filename)`, whole: unless lists are already loaded ("Don't
/// re-initialize..."), the file `ini_filename` (`None`: [`DEFAULT_INI_FILENAME`]) read through [`get_ini`], and its
/// sections made into the table ([`initialize`]). `WW3D::Init` calls it with no name.
pub fn initialize_file<D: LocalDevice>(factory: &W3dFileFactory, fs: &FileSystem<D>, ini_filename: Option<&[u8]>) -> W3dResult<()> {
    //
    //	Don't re-initialize...
    //
    if !mgr_read().anim_sound_lists.is_empty() && !plant(10064) {
        return Ok(());
    }
    //
    //	Determine which filename to use
    //
    let filename_to_use = ini_filename.unwrap_or(DEFAULT_INI_FILENAME);
    //
    //	Get the INI file which contains the data for this viewer
    //
    let ini = get_ini(factory, fs, filename_to_use)?;
    initialize(ini.as_ref())
}

/// `AnimatedSoundMgrClass::Initialize`'s work on the INI `Get_INI` gave (`None`: the file was not there): unless lists
/// are already loaded ("Don't re-initialize..."), each section of the INI's section list, in order, becomes the sound
/// list of the animation named by the section, upper case. Its entries are read **through the section's name**
/// (`Entry_Count(section->Section)`, `Get_Entry`, `Get_String`), so of two sections with one name both read the one
/// `Find_Section` finds ([`IniClass::find_section`]). Its `BoneName` entry (exactly that name) is the bone, each other
/// entry a sound `<frame>,<name>[,<more>...]`, with `2D` or `STOP` in a later parameter; a section with no sound is
/// dropped. A value of fewer than two parameters is read past its list (`param_list[1]`, the parameter-count test
/// is commented out upstream): undefined, as an empty value is ([`build_list_from_string`]). A section stops being
/// read at its first undefined entry, and nothing of it is kept.
pub fn initialize(ini: Option<&IniClass>) -> W3dResult<()> {
    let mut m = mgr_write();
    //
    //	Don't re-initialize...
    //
    if !m.anim_sound_lists.is_empty() && !plant(10064) {
        return Ok(());
    }
    let Some(ini) = ini else { return Ok(()) };
    //
    //	Loop over all the sections in the INI
    //
    for section in ini.get_section_list() {
        // Get the animation name from the section name
        let animation_name = c_str(&section.section).to_ascii_uppercase();
        let mut sound_list = AnimSoundList::new();
        //
        //	Loop over all the entries in this section
        //
        let entries: Vec<(Vec<u8>, Vec<u8>)> = if plant(10252) {
            // planted: the listed section's own entries, not the one found by its name
            section.entry_list.iter().map(|e| (e.entry.clone(), e.value.clone())).collect()
        } else {
            let entry_count = ini.entry_count(&section.section);
            let mut v = Vec::new();
            for entry_index in 0..entry_count {
                // strcmp(entry_name, ...) of a NULL name: undefined (not reached: the count is the list's)
                let entry_name = ini.get_entry(&section.section, entry_index).ok_or(W3dUndefined::NullPointer)?.to_vec();
                let value = ini.get_string(&section.section, &entry_name, b"");
                v.push((entry_name, value));
            }
            v
        };
        for (entry_name, value) in &entries {
            let value = c_str(value);
            let is_bone_name = if plant(10065) { c_str(entry_name).eq_ignore_ascii_case(b"BoneName") } else { c_str(entry_name) == b"BoneName" };
            if is_bone_name {
                sound_list.bone_name = value.to_vec();
            } else {
                // Separate the parameters into an easy-to-handle data structure
                let param_list = build_list_from_string(value, b",")?;
                let (Some(p0), Some(p1)) = (param_list.first(), param_list.get(1)) else {
                    return Err(W3dUndefined::IndexOutOfRange.into());
                };
                let action_frame = atoi(p0)?;
                let definition_name = strtrim(p1);
                sound_list.list.push(AnimSoundInfo {
                    frame: action_frame,
                    sound_name: definition_name,
                    is_2d: is_in_param_list(&param_list, b"2D"),
                    is_stop: is_in_param_list(&param_list, b"STOP"),
                });
            }
        }
        if !sound_list.list.is_empty() {
            // Add this sound list to our hash-table and vector-array
            let index = m.anim_sound_lists.len();
            m.anim_sound_lists.push(sound_list);
            m.animation_name_hash.insert(&animation_name, index);
        }
    }
    Ok(())
}

/// `AnimatedSoundMgrClass::Shutdown()`: the hash and the lists emptied (the sound library stays).
pub fn shutdown() {
    let mut m = mgr_write();
    if !plant(10074) {
        m.animation_name_hash.remove_all();
    }
    m.anim_sound_lists.clear();
}

/// `AnimatedSoundMgrClass::Get_Embedded_Sound_Name(anim)` (its comment heading says `Does_Animation_Have_Embedded_
/// Sounds`): the bone name of the animation's sound list (the C string of `BoneName.Peek_Buffer()`); `None` without an
/// animation (`anim == NULL`) or without a list.
pub fn get_embedded_sound_name(anim_name: Option<&[u8]>) -> Option<Vec<u8>> {
    let anim_name = anim_name?;
    let m = mgr_read();
    m.find_sound_list(anim_name).map(|l| c_str(&l.bone_name).to_vec())
}

/// `AnimatedSoundMgrClass::Trigger_Sound(anim, old_frame, new_frame, tm)`: without a sound library or an animation,
/// `old_frame`. Otherwise each sound of the animation's list, in order, whose frame the animation passed (`old_frame <
/// frame && new_frame >= frame`, the integer frame converted to `float`) is stopped (`STOP`) or played in 2D or at
/// `tm` in 3D, and the answer becomes that frame; the answer is `old_frame` when none was passed. There is no wrap and
/// no direction: an animation that loops back or plays backwards passes nothing (the frame's comparison only goes up),
/// and a frame skipped far ahead still fires (the `Fabs(new_frame - old_frame) < 3.0F` test is commented out). A NaN
/// frame passes nothing.
pub fn trigger_sound(anim_name: Option<&[u8]>, old_frame: f32, new_frame: f32, tm: &Matrix3D) -> f32 {
    let mut m = mgr_write();
    let Some(anim_name) = anim_name else { return old_frame };
    if m.sound_library.is_none() && !plant(10072) {
        return old_frame;
    }
    let mut retval = old_frame;
    //
    //	Lookup the sound list for this animation
    //
    let Some(sound_list) = m.find_sound_list(anim_name).cloned() else { return retval };
    for info in &sound_list.list {
        let frame = info.frame as f32;
        //
        //	Is the animation passing the frame we need?
        //
        let passing = if plant(10056) {
            old_frame <= frame && new_frame >= frame
        } else if plant(10057) {
            old_frame < frame && new_frame > frame
        } else {
            old_frame < frame && new_frame >= frame
        };
        if passing {
            let name = c_str(&info.sound_name).to_vec();
            if info.is_stop && !plant(10059) {
                // Stop the audio
                m.call(SoundLibraryCall::Stop { name });
            } else if info.is_2d {
                // Play the audio
                m.call(SoundLibraryCall::Play2D { name });
            } else {
                m.call(SoundLibraryCall::Play3D { name, tm: *tm });
            }
            if !plant(10058) {
                retval = frame;
            }
        }
    }
    retval
}

/// `AnimatedSoundMgrClass::Set_Sound_Library(library)` (`None`: `NULL`). Zero Hour never calls it (module docs).
pub fn set_sound_library(library: Option<SoundLibraryBridge>) {
    mgr_write().sound_library = library;
}

/// New: the calls the recording bridge received since the last call of this, oldest first (for the tests).
pub fn take_recorded_calls() -> Vec<SoundLibraryCall> {
    std::mem::take(&mut mgr_write().recorded)
}

/// New: the sound lists, in the order they were loaded, with the upper-case name each is found by (the inspection the
/// tests make; upstream's are private).
pub fn sound_lists() -> Vec<(Vec<u8>, AnimSoundList)> {
    let m = mgr_read();
    let mut v: Vec<(usize, Vec<u8>, AnimSoundList)> = m
        .animation_name_hash
        .entries()
        .into_iter()
        .filter(|(n, i)| m.animation_name_hash.get(n) == Some(*i))
        .filter_map(|(n, i)| m.anim_sound_lists.get(i).map(|l| (i, n, l.clone())))
        .collect();
    v.sort_by_key(|(i, _, _)| *i);
    v.dedup_by_key(|(i, _, _)| *i);
    v.into_iter().map(|(_, n, l)| (n, l)).collect()
}

/// New: the statics as at the process's start (no list, no hash table, no library, nothing recorded), for the tests
/// that compare a table from a known state; upstream has no such call (`Shutdown` keeps the hash table's size).
pub fn reset_statics() {
    *mgr_write() = AnimatedSoundMgr::new();
}

/// New: every sound list in the order it was loaded (`AnimSoundLists`), with any whose name a later list took (the
/// differential test's view of the table; upstream's is private).
pub fn anim_sound_lists() -> Vec<AnimSoundList> {
    mgr_read().anim_sound_lists.clone()
}

/// New: the position in [`anim_sound_lists`] of the list `AnimationNameHash.Get(name)` answers (`name` as given, not
/// made upper case), if any.
pub fn sound_list_index(name: &[u8]) -> Option<usize> {
    mgr_read().animation_name_hash.get(c_str(name))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The manager is process-wide: the tests that load it take turns (and use names no other test's animations have).
    pub(crate) static LOCK: Mutex<()> = Mutex::new(());

    /// An INI database loaded from `text` (the tests' INI files).
    pub(crate) fn ini(text: &[u8]) -> IniClass {
        IniClass::from_file(b"test.ini", Some(text)).unwrap()
    }

    /// `HashTemplateClass`: two inserts of one name answer the later until a `Re_Hash` (the fifth insert into the
    /// first table of 4) copies the bucket from its head, which reverses it, and then answer the earlier.
    #[test]
    fn the_name_hash_reverses_a_bucket_when_it_grows() {
        let mut h = NameHash::new();
        assert_eq!(h.get(b"A"), None);
        h.insert(b"A", 0);
        h.insert(b"A", 1);
        assert_eq!(h.get(b"A"), Some(1));
        h.insert(b"B", 2);
        h.insert(b"C", 3);
        assert_eq!(h.size, 4);
        h.insert(b"D", 4);
        assert_eq!(h.size, 8);
        assert_eq!(h.get(b"A"), Some(0));
        assert_eq!(h.get(b"D"), Some(4));
        assert_eq!(h.entries().len(), 5);
        h.remove_all();
        assert_eq!((h.get(b"A"), h.size), (None, 8));
        h.insert(b"LONGNAME_X", 7);
        assert_eq!(h.get(b"LONGNAME_X"), Some(7));
        // 8 bytes or more: the 4 bytes 8 from the end ("LONG"), little-endian, mixed
        let w = 0x474E_4F4Cu32;
        assert_eq!(NameHash::get_hash_value(b"LONGNAME"), w.wrapping_add(w >> 5).wrapping_add(w >> 10).wrapping_add(w >> 20));
        // fewer: hval += 37 * hval + byte ('A' 65, then 65 + 37 * 65 + 66)
        assert_eq!(NameHash::get_hash_value(b"AB"), 2536);
    }

    #[test]
    fn strtrim_keeps_trailing_space_when_the_copy_ends_at_source() {
        assert_eq!(strtrim(b"  ab  "), b"ab");
        assert_eq!(strtrim(b"  a "), b"a ");
        assert_eq!(strtrim(b" a "), b"a");
        assert_eq!(strtrim(b"ab \t"), b"ab");
        assert_eq!(strtrim(b"   "), b"");
        assert_eq!(strtrim(b""), b"");
    }

    #[test]
    fn the_list_keeps_empty_entries_and_a_lone_character() {
        let l = build_list_from_string(b"10,abc", b",").unwrap();
        assert_eq!(l, vec![b"10".to_vec(), b"abc".to_vec()]);
        let l = build_list_from_string(b"10,,x,", b",").unwrap();
        assert_eq!(l, vec![b"10".to_vec(), b"".to_vec(), b"x".to_vec()]);
        assert_eq!(build_list_from_string(b",abc", b",").unwrap(), vec![b"".to_vec(), b"abc".to_vec()]);
        assert_eq!(build_list_from_string(b"7", b",").unwrap(), vec![b"7".to_vec()]);
        assert!(build_list_from_string(b"", b",").is_err());
        assert!(is_in_param_list(&[b"1".to_vec(), b"n".to_vec(), b"x2Dy".to_vec()], b"2D"));
        assert!(!is_in_param_list(&[b"1".to_vec(), b"2D".to_vec()], b"2D"));
        assert_eq!(atoi(b" -12x"), Ok(-12));
        assert!(atoi(b"99999999999").is_err());
    }

    #[test]
    fn sounds_fire_as_their_frames_are_passed_and_only_with_a_library() {
        let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        shutdown();
        set_sound_library(None);
        let db = ini(b"[SndTest.Walk]\nBoneName=FOOT\nS1=5, step ,2D\nS2=10,boom\nS3=12,boom,STOP\n[SndTest.Empty]\nBoneName=X\n");
        initialize(Some(&db)).unwrap();
        assert_eq!(get_embedded_sound_name(Some(b"sndtest.walk")), Some(b"FOOT".to_vec()));
        assert_eq!(get_embedded_sound_name(Some(b"SNDTEST.EMPTY")), None);
        assert_eq!(get_embedded_sound_name(None), None);
        // no library (Zero Hour): nothing, and the old frame back
        assert_eq!(trigger_sound(Some(b"SNDTEST.WALK"), 0.0, 20.0, &Matrix3D::IDENTITY), 0.0);
        set_sound_library(Some(SoundLibraryBridge::Recorder));
        take_recorded_calls();
        assert_eq!(trigger_sound(Some(b"SNDTEST.WALK"), 4.5, 10.0, &Matrix3D::IDENTITY), 10.0);
        assert_eq!(
            take_recorded_calls(),
            vec![SoundLibraryCall::Play2D { name: b"step".to_vec() }, SoundLibraryCall::Play3D { name: b"boom".to_vec(), tm: Matrix3D::IDENTITY }]
        );
        // a loop back passes nothing
        assert_eq!(trigger_sound(Some(b"SNDTEST.WALK"), 14.0, 2.0, &Matrix3D::IDENTITY), 14.0);
        assert_eq!(trigger_sound(Some(b"SNDTEST.WALK"), 10.0, 12.0, &Matrix3D::IDENTITY), 12.0);
        assert_eq!(take_recorded_calls(), vec![SoundLibraryCall::Stop { name: b"boom".to_vec() }]);
        // not re-initialised while lists are loaded
        initialize(Some(&ini(b"[SndTest.Other]\nS=1,x\n"))).unwrap();
        assert_eq!(get_embedded_sound_name(Some(b"SNDTEST.OTHER")), None);
        assert_eq!(sound_lists().len(), 1);
        shutdown();
        assert_eq!(get_embedded_sound_name(Some(b"SNDTEST.WALK")), None);
        set_sound_library(None);
    }

    /// `Initialize()` whole: `w3danimsound.ini` read through the game's file factory from the port's file system (a
    /// memory device). Predicted: the file at the game folder's root is found (an `.ini` is not an art type, so its
    /// path is its name); two sections `[SndTest.Dup]` both read the entries of the one `Find_Section` finds, so the
    /// table holds that one's sounds; a second `initialize_file` changes nothing; without the file nothing loads.
    #[test]
    fn initialize_reads_w3danimsound_ini_through_the_file_factory() {
        use crate::common::file_system::{FileSystem, MemoryDevice};
        let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        shutdown();
        let factory = W3dFileFactory::default();
        let empty = FileSystem::new(MemoryDevice::new());
        initialize_file(&factory, &empty, None).unwrap();
        assert!(sound_lists().is_empty());
        let mut d = MemoryDevice::new();
        d.insert(b"W3DAnimSound.ini", b"; sounds\r\n[SndFile.Run]\r\nBoneName = B2 ; the bone\r\nS1 = 3 , thud , 2D\r\n[SndFile.Dup]\nA=1,one\n[SndFile.Dup]\nB=2,two\n".to_vec());
        let fs = FileSystem::new(d);
        assert!(get_ini(&factory, &fs, b"nope.ini").unwrap().is_none());
        let db = get_ini(&factory, &fs, DEFAULT_INI_FILENAME).unwrap().unwrap();
        assert_eq!(db.get_filename(), DEFAULT_INI_FILENAME);
        let dup = db.find_section(b"SndFile.Dup").unwrap().entry_list[0].value.clone();
        initialize_file(&factory, &fs, None).unwrap();
        assert_eq!(get_embedded_sound_name(Some(b"SNDFILE.RUN")), Some(b"B2".to_vec()));
        let lists = sound_lists();
        let run = &lists.iter().find(|(n, _)| n == b"SNDFILE.RUN").unwrap().1;
        assert_eq!(run.list, vec![AnimSoundInfo { frame: 3, sound_name: b"thud".to_vec(), is_2d: true, is_stop: false }]);
        let d = &lists.iter().find(|(n, _)| n == b"SNDFILE.DUP").unwrap().1;
        assert_eq!(build_list_from_string(&dup, b",").unwrap()[1], d.list[0].sound_name);
        initialize_file(&factory, &fs, Some(b"other.ini")).unwrap();
        assert_eq!(sound_lists().len(), lists.len());
        assert_eq!(anim_sound_lists().len(), 3);
        assert_eq!(sound_list_index(b"SNDFILE.DUP"), Some(2));
        assert_eq!(sound_list_index(b"sndfile.dup"), None);
        shutdown();
    }

    /// The owner's `w3danimsound.ini`, read from their install (`RTS_ZH_DIR=<Zero Hour folder>`, its archives) through
    /// the game's file search. Skipped without it (no EA file is in this repository, never faked). Reports how many
    /// sections and sounds load, or that the archives hold no such file. **Not run (2026-11-14):** no install is on
    /// this machine; the test skipped.
    #[test]
    fn the_owner_s_w3danimsound_ini_loads() {
        use crate::common::file_system::{FileSystem, NativeDevice};
        let Some(dir) = std::env::var_os("RTS_ZH_DIR") else {
            eprintln!("skipped: RTS_ZH_DIR is not set (the owner's Zero Hour folder)");
            return;
        };
        let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut fs = FileSystem::new(NativeDevice::new(std::path::Path::new(&dir)));
        fs.init(None);
        let factory = W3dFileFactory::default();
        let Some(db) = get_ini(&factory, &fs, DEFAULT_INI_FILENAME).expect("Get_INI") else {
            eprintln!("the install has no {} (Initialize loads nothing)", String::from_utf8_lossy(DEFAULT_INI_FILENAME));
            return;
        };
        shutdown();
        initialize_file(&factory, &fs, None).expect("Initialize");
        let lists = sound_lists();
        eprintln!(
            "{} sections ({} duplicate entries dropped), {} animations with sounds, {} sounds",
            db.section_count(),
            db.duplicate_crc_errors().len(),
            lists.len(),
            lists.iter().map(|(_, l)| l.list.len()).sum::<usize>()
        );
        shutdown();
    }
}
