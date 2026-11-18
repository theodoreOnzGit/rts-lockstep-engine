//! UNTESTED DRAFT (#202, #208, #214): compiles; not yet tested against the upstream C++.
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

/// `UnicodeString::format(fmt, ...)`'s substitution for `%d`, `%c`, `%s` and `%%` (the only conversions `version.cpp` uses).
pub fn format_wide(fmt: &[u16], args: &[FormatArg]) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    let mut next = 0;
    let mut i = 0;
    while i < fmt.len() {
        if fmt[i] == b'%' as u16 && i + 1 < fmt.len() {
            let c = fmt[i + 1];
            i += 2;
            if c == b'%' as u16 {
                out.push(c);
                continue;
            }
            match args.get(next) {
                Some(FormatArg::Int(v)) => out.extend(v.to_string().encode_utf16()),
                Some(FormatArg::Char(ch)) => out.push(*ch),
                Some(FormatArg::Wide(w)) => out.extend_from_slice(w),
                None => {}
            }
            next += 1;
        } else {
            out.push(fmt[i]);
            i += 1;
        }
    }
    out
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
    /// `Version::Version()`: 1.0, build 0, user "somebody", location "somewhere"; `m_showFullVersion` is TRUE in a debug build.
    pub fn new() -> Version {
        Version {
            major: 1,
            minor: 0,
            build_num: 0,
            local_build_num: 0,
            build_user: b"somebody".to_vec(),
            build_location: b"somewhere".to_vec(),
            build_time: Vec::new(),
            build_date: Vec::new(),
            show_full_version: false,
            debug_build: false,
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
        ((self.major as u32) << 16) | (self.minor as u32)
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
                v.push(Self::char_at(&self.build_user, 0));
                v.push(Self::char_at(&self.build_user, 1));
                v
            } else {
                format!("{}.{}.{}", self.major, self.minor, self.build_num).into_bytes()
            }
        } else {
            format!("{}.{}", self.major, self.minor).into_bytes()
        }
    }

    /// The `Format3`/`Format4` choice shared by `getUnicodeVersion` (debug) and `getFullUnicodeVersion`.
    fn format_full<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: &F) -> Vec<u16> {
        if self.local_build_num == 0 {
            format_wide(&fetch(b"Version:Format3"), &[FormatArg::Int(self.major), FormatArg::Int(self.minor), FormatArg::Int(self.build_num)])
        } else {
            format_wide(
                &fetch(b"Version:Format4"),
                &[
                    FormatArg::Int(self.major),
                    FormatArg::Int(self.minor),
                    FormatArg::Int(self.build_num),
                    FormatArg::Int(self.local_build_num),
                    FormatArg::Char(Self::char_at(&self.build_user, 0) as u16),
                    FormatArg::Char(Self::char_at(&self.build_user, 1) as u16),
                ],
            )
        }
    }

    /// `Version::getUnicodeVersion()`: release is `Version:Format2` with major and minor; a debug build uses the full form and appends `" Debug"`.
    pub fn get_unicode_version<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        if self.debug_build {
            let mut v = self.format_full(&fetch);
            v.extend(" Debug".encode_utf16());
            v
        } else {
            format_wide(&fetch(b"Version:Format2"), &[FormatArg::Int(self.major), FormatArg::Int(self.minor)])
        }
    }

    /// `Version::getFullUnicodeVersion()`: always the full form; `" Debug"` is appended only under `_DEBUG` (and `" Internal"` under `_INTERNAL`, not modelled).
    pub fn get_full_unicode_version<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        let mut v = self.format_full(&fetch);
        if self.debug_build {
            v.extend(" Debug".encode_utf16());
        }
        v
    }

    /// `Version::getAsciiBuildTime()`: `"%s %s"` of the build date and time.
    pub fn get_ascii_build_time(&self) -> Vec<u8> {
        let mut s = self.build_date.clone();
        s.push(b' ');
        s.extend_from_slice(&self.build_time);
        s
    }

    /// `Version::getUnicodeBuildTime()`: `Version:BuildTime` with the translated date and time.
    pub fn get_unicode_build_time<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        let date: Vec<u16> = self.build_date.iter().map(|&b| b as u16).collect();
        let time: Vec<u16> = self.build_time.iter().map(|&b| b as u16).collect();
        format_wide(&fetch(b"Version:BuildTime"), &[FormatArg::Wide(date), FormatArg::Wide(time)])
    }

    /// `Version::getAsciiBuildLocation()`.
    pub fn get_ascii_build_location(&self) -> Vec<u8> {
        self.build_location.clone()
    }

    /// `Version::getUnicodeBuildLocation()`: `Version:BuildMachine` with the translated location.
    pub fn get_unicode_build_location<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        let machine: Vec<u16> = self.build_location.iter().map(|&b| b as u16).collect();
        format_wide(&fetch(b"Version:BuildMachine"), &[FormatArg::Wide(machine)])
    }

    /// `Version::getAsciiBuildUser()`.
    pub fn get_ascii_build_user(&self) -> Vec<u8> {
        self.build_user.clone()
    }

    /// `Version::getUnicodeBuildUser()`: `Version:BuildUser` with the translated user.
    pub fn get_unicode_build_user<F: Fn(&[u8]) -> Vec<u16>>(&self, fetch: F) -> Vec<u16> {
        let user: Vec<u16> = self.build_user.iter().map(|&b| b as u16).collect();
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

