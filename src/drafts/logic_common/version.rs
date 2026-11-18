//! UNTESTED DRAFT (#202, #208, #214): compiles; not yet tested against the upstream C++.
//! Differential-tested in part (#215, tests/differential_draft_version.rs): every Version method matches the upstream C++ on 100,000 cases each for the shipped (release) build and for `_DEBUG`. Not covered: game_main and GameEngineHost (the engine's own calls), `_INTERNAL` (" Internal", not modelled), and the Unicode results rest on a stand-in for MSVC's wide printf.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/Common/version.h
//!   GeneralsMD/Code/GameEngine/Source/Common/version.cpp
//!   GeneralsMD/Code/GameEngine/Source/Common/GameMain.cpp (see [`game_main`])
//!   GeneralsMD/Code/GameEngine/Source/Common/INI/INIModel.cpp (nothing: the file holds no code, only banner comments)
//!
//! The shipped game is the release configuration: `#if defined _DEBUG || defined _INTERNAL` takes its `#else` branch
//! (`getAsciiVersion` is `"%d.%d"`, `getUnicodeVersion` uses `Version:Format2`, `m_showFullVersion` starts FALSE). The
//! debug branches are translated beside them behind the `debug_build` field so a differential build can choose.
//! `TheGameText->fetch(label)` is passed in as `fetch`.

use crate::logic::modules::plant;

/// One `%d`, `%c` or `%s` argument of upstream's `UnicodeString::format`.
#[derive(Clone, Debug, PartialEq)]
pub enum FormatArg {
    /// `%d`.
    Int(i32),
    /// `%c`.
    Char(u16),
    /// `%s` (a wide string argument, `UnicodeString::str()`).
    Wide(Vec<u16>),
}

/// `UnicodeString::format(fmt, ...)`'s substitution for `%d`, `%c`, `%s` and `%%` (the only conversions `version.cpp` uses), as
/// `_vsnwprintf` reads its variadic arguments: `%d` and `%c` each take the NEXT argument as an `int` (a `char` is promoted to its
/// sign-extended `int`), whatever the conversion is for; `%c` writes the low 16 bits of it. `%s` takes a wide string (any other
/// argument there is undefined upstream and not produced). The result is then taken as a C string (`UnicodeString::set(buf)`), so it
/// ends at the first 0 unit (a `%c` of 0, or a `%c` of an `int` whose low 16 bits are 0, cuts it).
/// ~~(`%c` printed an `Int` argument as a number; the text kept a 0 unit)~~ **Corrected 2026-11-18 (#215):** found by the differential test.
pub fn format_wide(fmt: &[u16], args: &[FormatArg]) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    let mut next = 0;
    let mut i = 0;
    let as_int = |a: Option<&FormatArg>| -> i32 {
        match a {
            Some(FormatArg::Int(v)) => *v,
            Some(FormatArg::Char(ch)) => *ch as i16 as i32, // a promoted `char`: sign-extended
            _ => 0,
        }
    };
    while i < fmt.len() {
        if fmt[i] == b'%' as u16 && i + 1 < fmt.len() {
            let c = fmt[i + 1];
            i += 2;
            if c == b'%' as u16 {
                out.push(c);
                continue;
            }
            if c == b'd' as u16 {
                out.extend(as_int(args.get(next)).to_string().encode_utf16());
            } else if c == b'c' as u16 {
                out.push(as_int(args.get(next)) as u16);
            } else if c == b's' as u16 {
                if let Some(FormatArg::Wide(w)) = args.get(next) {
                    out.extend_from_slice(w);
                }
            }
            next += 1;
        } else {
            out.push(fmt[i]);
            i += 1;
        }
    }
    if let Some(end) = out.iter().position(|&u| u == 0) {
        out.truncate(end);
    }
    out
}

/// One byte of `UnicodeString::translate(AsciiString)`: `concat((WideChar)stringSrc.getCharAt(i))`, a signed `char` widened to the 16-bit
/// `WideChar`, so a byte of 0x80 or more becomes 0xFF80 to 0xFFFF (upstream's comment: "this will only work for 7-bit ascii").
/// ~~(zero-extended)~~ **Corrected 2026-11-18 (#215).**
fn translate_char(b: u8) -> u16 {
    b as i8 as i16 as u16
}

/// `class Version`: "formats the version number into integer and string values for different parts of the game."
#[derive(Clone, Debug, PartialEq)]
pub struct Version {
    /// `m_major`.
    pub major: i32,
    /// `m_minor`.
    pub minor: i32,
    /// `m_buildNum`.
    pub build_num: i32,
    /// `m_localBuildNum`.
    pub local_build_num: i32,
    /// `m_buildLocation`.
    pub build_location: Vec<u8>,
    /// `m_buildUser`.
    pub build_user: Vec<u8>,
    /// `m_buildTime`.
    pub build_time: Vec<u8>,
    /// `m_buildDate`.
    pub build_date: Vec<u8>,
    /// `m_showFullVersion`.
    pub show_full_version: bool,
    /// `_DEBUG || _INTERNAL` (false for the shipped game).
    pub debug_build: bool,
}

impl Version {
    /// `Version::Version()` of the shipped (release) game: see [`Version::with_debug_build`].
    pub fn new() -> Version {
        Version::with_debug_build(false)
    }

    /// `Version::Version()`: 1.0, build 0, user "somebody", location "somewhere"; `m_showFullVersion` is TRUE in a debug build
    /// (`_DEBUG || _INTERNAL`) and FALSE in the release build. ~~(`new()` left it FALSE for a debug build too)~~ **Corrected
    /// 2026-11-18 (#215):** set from `debug_build`, as the constructor's `#if` does.
    pub fn with_debug_build(debug_build: bool) -> Version {
        Version {
            major: 1,
            minor: 0,
            build_num: 0,
            local_build_num: 0,
            build_user: if plant(11500) { b"somewhere".to_vec() } else { b"somebody".to_vec() },
            build_location: if plant(11500) { b"somebody".to_vec() } else { b"somewhere".to_vec() },
            build_time: Vec::new(),
            build_date: Vec::new(),
            show_full_version: debug_build && !plant(11502),
            debug_build,
        }
    }

    /// `Version::setVersion(major, minor, buildNum, localBuildNum, user, location, buildTime, buildDate)`.
    pub fn set_version(&mut self, major: i32, minor: i32, build_num: i32, local_build_num: i32, user: Vec<u8>, location: Vec<u8>, build_time: Vec<u8>, build_date: Vec<u8>) {
        self.major = major;
        self.minor = minor;
        self.build_num = build_num;
        self.local_build_num = local_build_num;
        self.build_user = user;
        self.build_location = location;
        self.build_time = build_time;
        self.build_date = build_date;
    }

    /// `showFullVersion()`.
    pub fn show_full_version(&self) -> bool {
        self.show_full_version
    }

    /// `setShowFullVersion(val)`.
    pub fn set_show_full_version(&mut self, val: bool) {
        self.show_full_version = val;
    }

    /// `Version::getVersionNumber()`: "a 4-byte integer suitable for WOLAPI", `m_major << 16 | m_minor`.
    pub fn get_version_number(&self) -> u32 {
        ((self.major as u32) << (if plant(11490) { 8 } else { 16 })) | (self.minor as u32)
    }

    /// `AsciiString::getCharAt(i)` on a non-empty-enough name (0 past the end).
    fn char_at(s: &[u8], i: usize) -> u8 {
        s.get(i).copied().unwrap_or(0)
    }

    /// `Version::getAsciiVersion()`: release is `"%d.%d"`; a debug build is `"%d.%d.%d.%d%c%c"` (with a local build number) or `"%d.%d.%d"`.
    pub fn get_ascii_version(&self) -> Vec<u8> {
        if self.debug_build {
            if self.local_build_num != 0 {
                let mut v = format!("{}.{}.{}.{}", self.major, self.minor, self.build_num, self.local_build_num).into_bytes();
                if plant(11492) {
                    v.push(Self::char_at(&self.build_user, 1));
                    v.push(Self::char_at(&self.build_user, 0));
                } else {
                    v.push(Self::char_at(&self.build_user, 0));
                    v.push(Self::char_at(&self.build_user, 1));
                }
                // AsciiString::format -> set(buf): the text is a C string, a `%c` of 0 ends it
                if let Some(end) = v.iter().position(|&b| b == 0) {
                    v.truncate(end);
                }
                v
            } else {
                format!("{}.{}.{}", self.major, self.minor, self.build_num).into_bytes()
            }
        } else if plant(11491) {
            format!("{}.{}.{}", self.major, self.minor, self.build_num).into_bytes()
        } else {
            format!("{}.{}", self.major, self.minor).into_bytes()
        }
    }

    /// The `Format3`/`Format4` choice shared by `getUnicodeVersion` (debug) and `getFullUnicodeVersion`.
    fn format_full<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: &F) -> Vec<u16> {
        if (self.local_build_num == 0) != plant(11493) {
            format_wide(&fetch(b"Version:Format3"), &[FormatArg::Int(self.major), FormatArg::Int(self.minor), FormatArg::Int(self.build_num)])
        } else {
            format_wide(
                &fetch(b"Version:Format4"),
                &[
                    FormatArg::Int(self.major),
                    FormatArg::Int(self.minor),
                    FormatArg::Int(self.build_num),
                    FormatArg::Int(self.local_build_num),
                    FormatArg::Char(Self::char_at(&self.build_user, 0) as i8 as i16 as u16),
                    FormatArg::Char(Self::char_at(&self.build_user, 1) as i8 as i16 as u16),
                ],
            )
        }
    }

    /// `Version::getUnicodeVersion()`: release is `Version:Format2` with major and minor; a debug build uses the full form and appends `" Debug"`.
    pub fn get_unicode_version<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        if self.debug_build {
            let mut v = self.format_full(&fetch);
            if !plant(11501) {
                v.extend(" Debug".encode_utf16());
            }
            v
        } else {
            format_wide(&fetch(if plant(11494) { b"Version:Format3" } else { b"Version:Format2" }), &[FormatArg::Int(self.major), FormatArg::Int(self.minor)])
        }
    }

    /// `Version::getFullUnicodeVersion()`: always the full form; `" Debug"` is appended only under `_DEBUG` (and `" Internal"` under `_INTERNAL`, not modelled).
    pub fn get_full_unicode_version<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        let mut v = self.format_full(&fetch);
        if self.debug_build || plant(11497) {
            v.extend(" Debug".encode_utf16());
        }
        v
    }

    /// `Version::getAsciiBuildTime()`: `"%s %s"` of the build date and time.
    pub fn get_ascii_build_time(&self) -> Vec<u8> {
        let (first, second) = if plant(11495) { (&self.build_time, &self.build_date) } else { (&self.build_date, &self.build_time) };
        let mut s = first.clone();
        s.push(b' ');
        s.extend_from_slice(second);
        s
    }

    /// `Version::getUnicodeBuildTime()`: `Version:BuildTime` with the translated date and time.
    pub fn get_unicode_build_time<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        let date: Vec<u16> = self.build_date.iter().map(|&b| translate_char(b)).collect();
        let time: Vec<u16> = self.build_time.iter().map(|&b| translate_char(b)).collect();
        if plant(11496) {
            format_wide(&fetch(b"Version:BuildTime"), &[FormatArg::Wide(time), FormatArg::Wide(date)])
        } else {
            format_wide(&fetch(b"Version:BuildTime"), &[FormatArg::Wide(date), FormatArg::Wide(time)])
        }
    }

    /// `Version::getAsciiBuildLocation()`.
    pub fn get_ascii_build_location(&self) -> Vec<u8> {
        self.build_location.clone()
    }

    /// `Version::getUnicodeBuildLocation()`: `Version:BuildMachine` with the translated location.
    pub fn get_unicode_build_location<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        let machine: Vec<u16> = (if plant(11498) { &self.build_user } else { &self.build_location }).iter().map(|&b| translate_char(b)).collect();
        format_wide(&fetch(b"Version:BuildMachine"), &[FormatArg::Wide(machine)])
    }

    /// `Version::getAsciiBuildUser()`.
    pub fn get_ascii_build_user(&self) -> Vec<u8> {
        self.build_user.clone()
    }

    /// `Version::getUnicodeBuildUser()`: `Version:BuildUser` with the translated user.
    pub fn get_unicode_build_user<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        let user: Vec<u16> = self.build_user.iter().map(|&b| translate_char(b)).collect();
        format_wide(&fetch(b"Version:BuildUser"), &[FormatArg::Wide(user)])
    }
}

impl Default for Version {
    fn default() -> Self {
        Version::new()
    }
}

/// What `GameMain` drives: the engine's three calls.
pub trait GameEngineHost {
    /// `TheGameEngine = CreateGameEngine(); TheGameEngine->init(argc, argv)`.
    fn init(&mut self, argv: &[Vec<u8>]);
    /// `TheGameEngine->execute()`.
    fn execute(&mut self);
}

/// `GameMain(argc, argv)`, "the entry point for the game system": create and init the engine, run it, and when `execute()`
/// returns delete it (here: drop). `CreateGameEngine` is the host's factory (the Win32 engine).
pub fn game_main<E: GameEngineHost>(mut engine: E, argv: &[Vec<u8>]) {
    // initialize the game engine using factory function
    engine.init(argv);
    // run it
    engine.execute();
    // since execute() returned, we are exiting the game
    drop(engine);
}

