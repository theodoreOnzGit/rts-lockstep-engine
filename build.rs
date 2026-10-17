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
            "upstream/GeneralsMD/Code/GameEngine/Include/Common/QuickTrig.h",
            "upstream/GeneralsMD/Code/Libraries/Include/Lib/trig.h",
            "upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWMath",
        ] {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}
