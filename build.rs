// Build script for rts-lockstep-engine.
//
// By default it does nothing: the crate is pure Rust and needs no C++ compiler
// (so native and wasm32 builds are unaffected).
//
// With `--features differential` it compiles the ORIGINAL upstream C++ from the
// pinned `upstream/` git submodule (EA's release, commit
// 0a05454d8574207440a5fb15241b98ad0b435590), together with the minimal shim
// headers and `extern "C"` wrappers in `differential/`, into a static library
// that the differential tests call. Upstream files are never edited.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(feature = "differential")]
    differential::build();
}

#[cfg(feature = "differential")]
mod differential {
    use std::path::Path;

    const UPSTREAM_COMMIT: &str = "0a05454d8574207440a5fb15241b98ad0b435590";

    /// Floating-point flags. They make the C++ evaluate `float` arithmetic as
    /// single IEEE-754 binary32 operations, one rounding per operation, the way
    /// the Rust does: SSE (not x87), no FMA contraction, no fast-math.
    /// `-fwrapv`: signed overflow wraps (as on the MSVC x86 target), instead of
    /// being undefined behaviour the optimiser may exploit.
    /// `-fno-strict-aliasing`: upstream type-puns floats through `int*`.
    const FLAGS: &[&str] = &[
        "-msse2",
        "-mfpmath=sse",
        "-ffp-contract=off",
        "-fno-fast-math",
        "-fwrapv",
        "-fno-strict-aliasing",
        "-std=gnu++17",
        "-w",
    ];

    fn base(cfg: &mut cc::Build) {
        cfg.cpp(true).opt_level(2).debug(false);
        for f in FLAGS {
            cfg.flag(f);
        }
        // `__cdecl` appears in WWMath declarations; it means nothing on x86_64.
        cfg.define("__cdecl", Some(""));
        // Release build: upstream asserts compiled out (as in the shipped game).
        cfg.define("NDEBUG", None);
    }

    pub fn build() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let up = root.join("upstream/GeneralsMD/Code");
        if !up.join("GameEngine/Source/Common/RandomValue.cpp").exists() {
            panic!(
                "feature `differential` needs the upstream submodule: run \
                 `git submodule update --init` (expected commit {UPSTREAM_COMMIT})"
            );
        }
        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        if target_arch != "x86_64" {
            panic!("feature `differential` is supported on x86_64 hosts only (found {target_arch})");
        }
        let shim = root.join("differential/shim");
        let dif = root.join("differential");
        let ww = up.join("Libraries/Source/WWVegas");

        // GameEngine: RandomValue.cpp, Trig.cpp, QuickTrig.cpp (via their
        // wrappers) and crc.cpp. `_DEBUG` selects crc.h's C++ CRC class
        // instead of its MSVC `_asm` block; the asm path is transcribed
        // separately in wrap_crc.cpp.
        let mut ge = cc::Build::new();
        base(&mut ge);
        ge.define("_DEBUG", None)
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .file(dif.join("wrap_random_value.cpp"))
            .file(dif.join("wrap_crc.cpp"))
            .file(dif.join("wrap_trig.cpp"))
            .file(dif.join("wrap_quick_trig.cpp"))
            .file(up.join("GameEngine/Source/Common/crc.cpp"));
        ge.compile("upstream_gameengine");

        // The Xfer family: Xfer.cpp (via wrap_xfer.cpp, after matrix3d.h),
        // XferCRC.cpp, XferSave.cpp, XferLoad.cpp and Snapshot.cpp unchanged,
        // against the shims in differential/shim/Common. `_DEBUG` again only
        // selects crc.h's C++ class (XferCRC.cpp includes crc.h, unused).
        let sys = up.join("GameEngine/Source/Common/System");
        let mut xf = cc::Build::new();
        base(&mut xf);
        xf.define("_DEBUG", None)
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .include(ww.join("WWMath"))
            .include(ww.join("WWLib"))
            .file(dif.join("wrap_xfer.cpp"))
            .file(sys.join("XferCRC.cpp"))
            .file(sys.join("XferSave.cpp"))
            .file(sys.join("XferLoad.cpp"))
            .file(sys.join("Snapshot.cpp"));
        xf.compile("upstream_xfer");

        // The .big archive reader: ArchiveFile.cpp, ArchiveFileSystem.cpp,
        // File.cpp, RAMFile.cpp, StreamingArchiveFile.cpp, Win32BIGFile.cpp
        // and Win32BIGFileSystem.cpp unchanged, with archive_prelude.h
        // force-included (MSVC names, _MAX_PATH; see the file).
        let dev = up.join("GameEngineDevice");
        let mut ar = cc::Build::new();
        base(&mut ar);
        ar.include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(dev.join("Include"))
            .include(up.join("Libraries/Include"))
            .flag("-include")
            .flag(shim.join("archive_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_big.cpp"))
            .file(sys.join("ArchiveFile.cpp"))
            .file(sys.join("ArchiveFileSystem.cpp"))
            .file(sys.join("File.cpp"))
            .file(sys.join("RAMFile.cpp"))
            .file(sys.join("StreamingArchiveFile.cpp"))
            .file(dev.join("Source/Win32Device/Common/Win32BIGFile.cpp"))
            .file(dev.join("Source/Win32Device/Common/Win32BIGFileSystem.cpp"));
        ar.compile("upstream_big");

        // The INI reader: INI.cpp, from a copy with two mechanical edits for
        // the MSVC dialect it is written in, which g++ rejects and no shim or
        // flag can supply (the upstream file itself is never edited). Each
        // edit must match exactly once, so a different upstream fails here:
        // 1. findFieldParse uses its loop variable `parse` after the `for`
        //    loop that declares it (MSVC 6/7's pre-standard `for` scope); the
        //    declaration moves before the loop, which is what the scope rule
        //    meant when the game was compiled.
        // 2. initFromINIMulti passes a `void*` to findFieldParse's `const
        //    void*&` parameter, which MSVC accepted; the local becomes
        //    `const void*`, its only other use being a `const void*` argument.
        // `-fpermissive` downgrades the remaining MSVC-isms to warnings: the
        // cast of `userData` to `UnsignedInt` in parseBitInInt32 (a 64-bit
        // pointer truncated to its low 32 bits, the mask). GameCommon.cpp
        // (TheVeterancyNames) and the wrapper compile unchanged.
        let ini_src = up.join("GameEngine/Source/Common/INI/INI.cpp");
        let mut ini_text = std::fs::read_to_string(&ini_src).expect("read INI.cpp");
        for (from, to) in [
            (
                "\tfor (const FieldParse* parse = parseTable; parse->token; ++parse)",
                "\tconst FieldParse* parse; for (parse = parseTable; parse->token; ++parse)",
            ),
            ("\t\t\t\t\tvoid* userData = 0;", "\t\t\t\t\tconst void* userData = 0;"),
        ] {
            assert_eq!(ini_text.matches(from).count(), 1, "INI.cpp patch site {from:?} must occur once");
            ini_text = ini_text.replace(from, to);
        }
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        let ini_copy = out_dir.join("INI_msvc_scope.cpp");
        std::fs::write(&ini_copy, ini_text).expect("write INI.cpp copy");
        let shim_ini = dif.join("shim_ini");
        let ini_build = |cfg: &mut cc::Build| {
            base(cfg);
            cfg.include(&shim_ini)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .flag("-include")
                .flag(shim_ini.join("ini_prelude.h").to_str().expect("UTF-8 path"));
        };
        let mut ini = cc::Build::new();
        ini_build(&mut ini);
        ini.flag("-fpermissive").file(&ini_copy);
        ini.compile("upstream_ini");
        let mut iniw = cc::Build::new();
        ini_build(&mut iniw);
        iniw.file(dif.join("wrap_ini.cpp")).file(sys.join("GameCommon.cpp"));
        iniw.compile("upstream_ini_wrap");

        // Lib/BaseType.h asm helpers (hand transcriptions, see the file).
        let mut bt = cc::Build::new();
        base(&mut bt);
        bt.file(dif.join("wrap_base_type.cpp"));
        bt.compile("upstream_basetype");

        // WWMath: headers via the wrapper, matrix3d.cpp compiled unchanged.
        let mut wm = cc::Build::new();
        base(&mut wm);
        wm.include(&shim)
            .include(ww.join("WWMath"))
            .include(ww.join("WWLib"))
            .file(dif.join("wrap_wwmath.cpp"))
            .file(ww.join("WWMath/matrix3d.cpp"));
        wm.compile("upstream_wwmath");

        for p in [
            "differential",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/RandomValue.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/crc.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Include/Common/crc.h",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/System",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic",
            "upstream/GeneralsMD/Code/GameEngine/Include/GameClient",
            "upstream/GeneralsMD/Code/GameEngine/Include/Common/QuickTrig.h",
            "upstream/GeneralsMD/Code/Libraries/Include/Lib/trig.h",
            "upstream/GeneralsMD/Code/GameEngine/Include/Common",
            "upstream/GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common",
            "upstream/GeneralsMD/Code/GameEngineDevice/Include/Win32Device/Common",
            "upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWMath",
        ] {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}
