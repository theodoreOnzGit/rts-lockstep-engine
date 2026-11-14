// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/animatedsoundmgr.cpp, animatedsoundmgr.h
//   (AnimatedSoundMgrClass: Initialize, Shutdown, Get_Embedded_Sound_Name, Find_Sound_List, Trigger_Sound,
//   Set_Sound_Library; the file's static Build_List_From_String and Is_In_Param_List)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/soundlibrarybridge.h (SoundLibraryBridgeClass)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/trim.cpp (strtrim, which StringClass::Trim calls)
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
//! object shares it as upstream's do. `AnimationNameHash` (a `HashTemplateClass<StringClass, ANIM_SOUND_LIST *>`, whose
//! `Insert` puts a new entry first in its bucket and whose `Get` answers the first equal key) is a map in which a
//! later insert of an equal name replaces the earlier: the same answer, except after a `Re_Hash` reorders a bucket
//! (two sections with one upper-case name; not compared, see the gaps in docs/phase2-client.md).
//!
//! The INI file itself (`INIClass`, WWLib/ini.cpp: `Load`, `Find_Section` by CRC, the duplicate-entry check) is
//! **not translated**: [`initialize`] takes the sections as `INIClass` holds them after loading ([`IniSection`]).
//! The bridge to the audio (`SoundLibraryBridgeClass`, which no Zero Hour code implements) is
//! [`SoundLibraryBridge`], whose one implementation records the calls.
//!
//! The strings are C strings throughout: upstream's `StringClass` keeps a stored length that `Trim()` and a written
//! terminator do not update, and the names reach the bridge through `Peek_Buffer()`, so only the bytes before the
//! first NUL matter; where the stored length is read (`Build_List_From_String`'s `Get_Length() > 0`) it is
//! reproduced.

use std::collections::BTreeMap;
use std::sync::RwLock;

use super::{W3dError, W3dResult, W3dUndefined};
use crate::logic::modules::plant;
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

/// One section of the INI file as `INIClass` holds it after `Load`: its name (`INISection::Section`) and its entries
/// in order (`Get_Entry(section, i)`, and `Get_String` of each). New: the parser is not translated (module docs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IniSection {
    pub section: Vec<u8>,
    pub entries: Vec<(Vec<u8>, Vec<u8>)>,
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
    animation_name_hash: BTreeMap<Vec<u8>, usize>,
    /// `AnimSoundLists`.
    anim_sound_lists: Vec<AnimSoundList>,
    /// `SoundLibrary` (`NULL`, upstream's only value in Zero Hour, is `None`).
    sound_library: Option<SoundLibraryBridge>,
    /// The calls [`SoundLibraryBridge::Recorder`] recorded, oldest first.
    recorded: Vec<SoundLibraryCall>,
}

impl AnimatedSoundMgr {
    const fn new() -> Self {
        AnimatedSoundMgr { animation_name_hash: BTreeMap::new(), anim_sound_lists: Vec::new(), sound_library: None, recorded: Vec::new() }
    }

    /// `Find_Sound_List(anim)`: the list of the animation's name made upper case (`strupr`), if any.
    fn find_sound_list(&self, anim_name: &[u8]) -> Option<&AnimSoundList> {
        let full_name: Vec<u8> = if plant(10063) { c_str(anim_name).to_vec() } else { c_str(anim_name).to_ascii_uppercase() };
        self.animation_name_hash.get(&full_name).and_then(|&i| self.anim_sound_lists.get(i))
    }

    /// The bridge's method for one sound (`SoundLibrary->...`), recorded.
    fn call(&mut self, call: SoundLibraryCall) {
        match self.sound_library {
            Some(SoundLibraryBridge::Recorder) => self.recorded.push(call),
            None => {}
        }
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

/// The bytes of a C string: up to its first NUL.
fn c_str(s: &[u8]) -> &[u8] {
    &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]
}

/// `strtrim(buffer)` (WWLib/trim.cpp) on a C string, as `StringClass::Trim` runs it: leading bytes up to 32 skipped
/// and the rest copied to the front (`strcpy` over itself, front to back), then trailing bytes up to 32 cut, **while
/// `*source` is not 0**, where `source` still points where the first kept byte was before the copy. So a string whose
/// kept part is as long as its leading white space ends the copy with a NUL at `source` and keeps its trailing white
/// space (`"  a "` gives `"a "`), and a cut that reaches `source` stops there.
pub fn strtrim(s: &[u8]) -> Vec<u8> {
    let s = c_str(s);
    let mut buffer: Vec<u8> = s.to_vec();
    buffer.push(0);
    // Strip leading white space from the string.
    let source = buffer.iter().take_while(|&&c| c != 0 && c <= 32).count();
    if source != 0 {
        let mut i = 0;
        loop {
            let c = buffer[source + i];
            buffer[i] = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
    }
    // Clip trailing white space from the string.
    let len = c_str(&buffer).len();
    for index in (0..len).rev() {
        if (buffer[source] != 0 || plant(10061)) && buffer[index] <= 32 {
            buffer[index] = 0;
        } else {
            break;
        }
    }
    c_str(&buffer).to_vec()
}

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

/// `AnimatedSoundMgrClass::Initialize(ini_filename)`: unless lists are already loaded ("Don't re-initialize..."),
/// each section of the INI (`ini`: `Get_INI(filename)`, `None` when the file is not there) becomes the sound list of
/// the animation named by the section, upper case: its `BoneName` entry (exactly that name) the bone, each other entry
/// a sound `<frame>,<name>[,<more>...]`, with `2D` or `STOP` in a later parameter; a section with no sound is
/// dropped. A value of fewer than two parameters is read past its list (`param_list[1]`, the parameter-count test
/// is commented out upstream): undefined, as an empty value is ([`build_list_from_string`]). A section stops being
/// read at its first undefined entry, and nothing of it is kept.
pub fn initialize(ini: Option<&[IniSection]>) -> W3dResult<()> {
    let mut m = mgr_write();
    //
    //	Don't re-initialize...
    //
    if !m.anim_sound_lists.is_empty() && !plant(10064) {
        return Ok(());
    }
    let Some(ini) = ini else { return Ok(()) };
    for section in ini {
        // Get the animation name from the section name
        let animation_name = c_str(&section.section).to_ascii_uppercase();
        let mut sound_list = AnimSoundList::new();
        for (entry_name, value) in &section.entries {
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
            m.animation_name_hash.insert(animation_name, index);
        }
    }
    Ok(())
}

/// `AnimatedSoundMgrClass::Shutdown()`: the hash and the lists emptied (the sound library stays).
pub fn shutdown() {
    let mut m = mgr_write();
    if !plant(10074) {
        m.animation_name_hash.clear();
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
    let mut v: Vec<(Vec<u8>, AnimSoundList)> =
        m.animation_name_hash.iter().filter_map(|(n, &i)| m.anim_sound_lists.get(i).map(|l| (n.clone(), l.clone()))).collect();
    v.sort_by_key(|(n, _)| m.animation_name_hash[n]);
    v
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The manager is process-wide: the tests that load it take turns (and use names no other test's animations have).
    pub(crate) static LOCK: Mutex<()> = Mutex::new(());

    fn section(name: &[u8], entries: &[(&[u8], &[u8])]) -> IniSection {
        IniSection { section: name.to_vec(), entries: entries.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect() }
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
        let ini = [
            section(b"SndTest.Walk", &[(b"BoneName", b"FOOT"), (b"S1", b"5, step ,2D"), (b"S2", b"10,boom"), (b"S3", b"12,boom,STOP")]),
            section(b"SndTest.Empty", &[(b"BoneName", b"X")]),
        ];
        initialize(Some(&ini)).unwrap();
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
        initialize(Some(&[section(b"SndTest.Other", &[(b"S", b"1,x")])])).unwrap();
        assert_eq!(get_embedded_sound_name(Some(b"SNDTEST.OTHER")), None);
        assert_eq!(sound_lists().len(), 1);
        shutdown();
        assert_eq!(get_embedded_sound_name(Some(b"SNDTEST.WALK")), None);
        set_sound_library(None);
    }
}
