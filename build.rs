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

        // The W3D readers, compiled first so that their references to
        // matrix3d.cpp (upstream_wwmath) and to the RAMFile of
        // wrap_w3d_file.cpp (upstream_big) resolve against libraries linked
        // after them. WWLib/chunkio.cpp, WW3D2/hrawanim.cpp, hcanim.cpp,
        // pivot.cpp and WWMath/quat.cpp compile unchanged; htree.cpp and
        // motchan.cpp are copies with mechanical edits (below). All with
        // shim_w3d/w3d_prelude.h force-included (see the file).
        //
        // clang++, not g++: WWLib's Vector.H templates use members of their
        // dependent base classes unqualified, which MSVC accepted and g++
        // rejects at definition time; clang's -fdelayed-template-parsing
        // (with -fms-extensions) accepts the code as written. Same libstdc++.
        //
        // The edits, each asserted to match once (MSVC 6/7's pre-standard
        // `for` scope, and one MSVC-only cast; all in code the loaders do
        // not run):
        // - motchan.cpp, Do_Data_Compression (dead code after `return;`):
        //   the loop variable `i` is reused after its loop, so its
        //   declaration moves before the loop; `unsigned short(ivalue)`, a
        //   functional cast g++/clang reject for a two-word type, becomes
        //   `(unsigned short)(ivalue)`.
        // - htree.cpp, HTreeClass::Combo_Update: `anim_num`, declared in the
        //   function's first loop, is used by a later loop; the declaration
        //   moves before the first loop.
        let shim_w3d = dif.join("shim_w3d");
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        let w3d = ww.join("WW3D2");
        let patch_copy = |src: &Path, name: &str, edits: &[(&str, &str, usize)]| -> Vec<u8> {
            let mut text = std::fs::read(src).expect("read upstream source");
            for (from, to, count) in edits {
                let f = from.as_bytes();
                let hits: Vec<usize> = (0..text.len().saturating_sub(f.len() - 1)).filter(|&i| &text[i..i + f.len()] == f).collect();
                assert_eq!(hits.len(), *count, "{name} patch site {from:?} must occur {count} time(s)");
                for &h in hits.iter().rev() {
                    text.splice(h..h + f.len(), to.bytes());
                }
            }
            text
        };
        let write = |name: &str, text: &[u8]| -> std::path::PathBuf {
            let dst = out_dir.join(name);
            std::fs::write(&dst, text).expect("write generated file");
            dst
        };

        // Generated headers (OUT_DIR/w3d_gen), on the include path after
        // shim_w3d: (1) for every WWLib/WWMath/WW3D2 file whose name is not
        // lower case, a forwarder under the lower-case name, as the
        // case-insensitive Windows file system resolves `#include
        // "vector3i.h"` to Vector3i.h; (2) vector_this.h, a copy of
        // WWLib/Vector.H whose DynamicVectorClass names five members of its
        // dependent base class through `this->` (MSVC found them without;
        // g++ and clang reject them when the template is instantiated), for
        // the mesh build only, which instantiates them (meshmdlio.cpp's
        // DynamicVectorClass::Add); (3) mesh_load_context.h,
        // MeshLoadContextClass as defined in meshmdlio.cpp (not in a
        // header), extracted verbatim so the mesh wrapper can read it.
        let gen = out_dir.join("w3d_gen");
        std::fs::create_dir_all(&gen).expect("create w3d_gen");
        for d in ["WWLib", "WWMath", "WW3D2"] {
            for e in std::fs::read_dir(ww.join(d)).expect("read upstream dir") {
                let name = e.expect("dir entry").file_name().into_string().expect("UTF-8 name");
                let lower = name.to_lowercase();
                if lower != name && !shim_w3d.join(&lower).exists() && !ww.join(d).join(&lower).exists() {
                    std::fs::write(gen.join(&lower), format!("/* generated by build.rs: {d}/{name} */\n#include \"{name}\"\n"))
                        .expect("write forwarder");
                }
            }
        }
        let vector = patch_copy(
            &ww.join("WWLib/Vector.H"),
            "Vector.H",
            &[
                ("if (Length() < ActiveCount) ActiveCount = Length();", "if (this->Length() < ActiveCount) ActiveCount = this->Length();", 1),
                ("if (ActiveCount >= Length()) {", "if (ActiveCount >= this->Length()) {", 4),
                ("if ((IsAllocated || !VectorMax) && GrowthStep > 0) {", "if ((this->IsAllocated || !this->VectorMax) && GrowthStep > 0) {", 4),
                ("if (!Resize(Length() + GrowthStep)) {", "if (!this->Resize(this->Length() + GrowthStep)) {", 4),
                ("int len = VectorMax;", "int len = this->VectorMax;", 1),
            ],
        );
        std::fs::write(gen.join("vector_this.h"), vector).expect("write vector_this.h");
        let meshmdlio_text = std::fs::read(w3d.join("meshmdlio.cpp")).expect("read meshmdlio.cpp");
        let ctx_start = b"class MeshLoadContextClass : public W3DMPO";
        let a = (0..meshmdlio_text.len()).find(|&i| meshmdlio_text[i..].starts_with(ctx_start)).expect("MeshLoadContextClass");
        let b = a + (0..meshmdlio_text.len() - a).find(|&i| meshmdlio_text[a + i..].starts_with(b"\n};")).expect("end of class") + 3;
        let mut ctx_h = b"/* generated by build.rs: MeshLoadContextClass, verbatim from WW3D2/meshmdlio.cpp */\n".to_vec();
        ctx_h.extend_from_slice(&meshmdlio_text[a..b]);
        ctx_h.push(b'\n');
        std::fs::write(gen.join("mesh_load_context.h"), ctx_h).expect("write mesh_load_context.h");

        let motchan = write(
            "motchan_msvc_scope.cpp",
            &patch_copy(
                &w3d.join("motchan.cpp"),
                "motchan.cpp",
                &[
                    ("\tfor (int i=0;i<count;i++) {", "\tint i; for (i=0;i<count;i++) {", 1),
                    ("CompressedData[i]=unsigned short(ivalue);", "CompressedData[i]=(unsigned short)(ivalue);", 1),
                ],
            ),
        );
        let htree = write(
            "htree_msvc_scope.cpp",
            &patch_copy(
                &w3d.join("htree.cpp"),
                "htree.cpp",
                &[(
                    "\n\tfor ( int anim_num = 0; anim_num < anim->Get_Num_Anims(); anim_num++ ) {",
                    "\n\tint anim_num; for ( anim_num = 0; anim_num < anim->Get_Num_Anims(); anim_num++ ) {",
                    1,
                )],
            ),
        );
        // multilist.cpp (the list base classes meshmdl.h's members derive
        // from): DEFINE_AUTO_POOL defines a member of a class template
        // specialisation without `template<>`, which MSVC accepts.
        let multilist = write(
            "multilist_template.cpp",
            &patch_copy(
                &ww.join("WWLib/multilist.cpp"),
                "multilist.cpp",
                &[(
                    "DEFINE_AUTO_POOL(MultiListNodeClass, 256);",
                    "template<> ObjectPoolClass<MultiListNodeClass, 256> AutoPoolClass<MultiListNodeClass, 256>::Allocator{};",
                    1,
                )],
            ),
        );
        // meshmdlio.cpp, MeshLoadContextClass::Add_Legacy_Material: si, vi
        // and ti are used after the loops that declare them.
        let meshmdlio = write(
            "meshmdlio_msvc_scope.cpp",
            &patch_copy(
                &w3d.join("meshmdlio.cpp"),
                "meshmdlio.cpp",
                &[
                    ("\tfor (int si=0; si<Shaders.Count(); si++) {", "\tint si; for (si=0; si<Shaders.Count(); si++) {", 1),
                    ("\t\tfor (int vi=0; vi<VertexMaterialCrcs.Count(); vi++) {", "\t\tint vi; for (vi=0; vi<VertexMaterialCrcs.Count(); vi++) {", 1),
                    ("\t\tfor (int ti=0; ti<Textures.Count(); ti++) {", "\t\tint ti; for (ti=0; ti<Textures.Count(); ti++) {", 1),
                ],
            ),
        );
        // meshmatdesc.cpp, MeshMatDescClass::Post_Load_Process: `pass` is
        // used after the loop that declares it. The copy also starts with
        // lines that rename MeshMatDescClass::Is_Empty (its declaration in
        // meshmatdesc.h and its definition here, not MultiListClass's in
        // meshmdl.h) to Is_Empty_Upstream, so that the wrapper can define
        // the one meshmdlio.cpp calls: the point at which the wrapper
        // compares (see differential/wrap_w3d_mesh.cpp). The file's own
        // includes of the two headers are then no-ops (include guards).
        let mut meshmatdesc_text = b"/* differential tests: see build.rs */\n#define Is_Empty Is_Empty_Upstream\n\
            #include \"meshmatdesc.h\"\n#undef Is_Empty\n#include \"meshmdl.h\"\n#define Is_Empty Is_Empty_Upstream\n"
            .to_vec();
        meshmatdesc_text.extend(patch_copy(
            &w3d.join("meshmatdesc.cpp"),
            "meshmatdesc.cpp",
            &[(
                "\tbool set_lighting_to_false=true;\n\tfor (int pass=0; pass<PassCount; pass++) {",
                "\tbool set_lighting_to_false=true;\n\tint pass; for (pass=0; pass<PassCount; pass++) {",
                1,
            )],
        ));
        let meshmatdesc = write("meshmatdesc_msvc_scope.cpp", &meshmatdesc_text);
        // Textures. ddsfile.cpp: `level` is used after the loop that
        // declares it, and `unsigned short(x)` is MSVC's functional cast of
        // a two-word type name. ddsfile.h (as a generated header found
        // before WW3D2's): LegacyDDSURFACEDESC2's `void* Surface` is 4 bytes
        // on Win32, so it is spelled `unsigned` to keep the 124-byte layout
        // the file is read into. formconv.cpp: `i` used after its loop.
        let ddsfile = write(
            "ddsfile_msvc.cpp",
            &patch_copy(
                &w3d.join("ddsfile.cpp"),
                "ddsfile.cpp",
                &[
                    ("\tfor (unsigned level=0;level<ReductionFactor;++level) ", "\tunsigned level; for (level=0;level<ReductionFactor;++level) ", 1),
                    ("unsigned short(cols>>16)", "(unsigned short)(cols>>16)", 9),
                    ("unsigned short(cols&0xffff)", "(unsigned short)(cols&0xffff)", 9),
                ],
            ),
        );
        let ddsfile_h = patch_copy(
            &w3d.join("ddsfile.h"),
            "ddsfile.h",
            &[("\tvoid* Surface;", "\tunsigned Surface; /* void* on Win32 (differential tests: see build.rs) */", 1)],
        );
        std::fs::write(gen.join("ddsfile.h"), ddsfile_h).expect("write ddsfile.h");
        let formconv = write(
            "formconv_msvc_scope.cpp",
            &patch_copy(
                &w3d.join("formconv.cpp"),
                "formconv.cpp",
                &[("\tfor (int i=0;i<HIGHEST_SUPPORTED_D3DFORMAT;++i) {", "\tint i; for (i=0;i<HIGHEST_SUPPORTED_D3DFORMAT;++i) {", 1)],
            ),
        );
        // TARGA.CPP: `long` is 32 bits on Win32 (its structs and size
        // arithmetic depend on it): the copy includes what TARGA.CPP
        // includes, then shim_w3d/targa.h, then defines `long` as `int` for
        // its own text. YFlip's _swapBytes is MSVC x86 inline assembly with
        // 32-bit pointers; the copy keeps it under `#if 0` and adds a
        // GNU-asm transcription of the same instructions on 64-bit
        // registers (the same reads and writes, including its byte loop,
        // which writes back the byte it took instead of swapping it).
        let mut targa_text = b"/* differential tests: see build.rs */\n#include <stdlib.h>\n#include <malloc.h>\n\
            #include <memory.h>\n#include <string.h>\n#include \"wwfile.h\"\n#include \"ffactory.h\"\n#include \"targa.h\"\n\
            #define long int\n"
            .to_vec();
        targa_text.extend(patch_copy(
            &ww.join("WWLib/TARGA.CPP"),
            "TARGA.CPP",
            &[
                ("{\n  _asm\n  {\n    mov esi,[p1]\n", "{\n#if 0\n  _asm\n  {\n    mov esi,[p1]\n", 1),
                (
                    "  done:\n  }\n}\n",
                    concat!(
                        "  done:\n  }\n#else\n",
                        "  /* GNU-asm transcription of the block above (differential tests) */\n",
                        "  __asm__ __volatile__(\n",
                        "    \"movl %%ebx, %%ecx\\n\\t\" \"shrl $4, %%ecx\\n\\t\" \"jz 2f\\n\"\n",
                        "    \"1:\\n\\t\"\n",
                        "    \"movl (%%rsi), %%eax\\n\\t\" \"movl 4(%%rsi), %%edx\\n\\t\"\n",
                        "    \"xchgl %%eax, (%%rdi)\\n\\t\" \"xchgl %%edx, 4(%%rdi)\\n\\t\"\n",
                        "    \"movl %%eax, (%%rsi)\\n\\t\" \"movl %%edx, 4(%%rsi)\\n\\t\"\n",
                        "    \"movl 8(%%rsi), %%eax\\n\\t\" \"movl 12(%%rsi), %%edx\\n\\t\"\n",
                        "    \"xchgl %%eax, 8(%%rdi)\\n\\t\" \"xchgl %%edx, 12(%%rdi)\\n\\t\"\n",
                        "    \"movl %%eax, 8(%%rsi)\\n\\t\" \"movl %%edx, 12(%%rsi)\\n\\t\"\n",
                        "    \"addq $16, %%rsi\\n\\t\" \"addq $16, %%rdi\\n\\t\" \"decl %%ecx\\n\\t\" \"jnz 1b\\n\"\n",
                        "    \"2:\\n\\t\" \"andl $15, %%ebx\\n\"\n",
                        "    \"3:\\n\\t\" \"jz 4f\\n\\t\"\n",
                        "    \"movb (%%rsi), %%al\\n\\t\" \"xchgb %%al, (%%rdi)\\n\\t\" \"movb %%al, (%%rdi)\\n\\t\"\n",
                        "    \"incq %%rsi\\n\\t\" \"incq %%rdi\\n\\t\" \"decl %%ebx\\n\\t\" \"jmp 3b\\n\"\n",
                        "    \"4:\\n\"\n",
                        "    : \"+S\"(p1), \"+D\"(p2), \"+b\"(count)\n",
                        "    :\n",
                        "    : \"eax\", \"ecx\", \"edx\", \"memory\", \"cc\");\n",
                        "#endif\n}\n"
                    ),
                    1,
                ),
            ],
        ));
        let targa = write("targa_win32_long.cpp", &targa_text);
        let w3d_build = |cfg: &mut cc::Build, mesh: bool| {
            base(cfg);
            cfg.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&shim_w3d)
                .include(&gen)
                .include(ww.join("WWLib"))
                .include(ww.join("WWMath"))
                .include(&w3d)
                .include(ww.join("WWDebug"))
                .flag("-include")
                .flag(shim_w3d.join("w3d_prelude.h").to_str().expect("UTF-8 path"));
            if mesh {
                cfg.flag("-include").flag(shim_w3d.join("mesh_prelude.h").to_str().expect("UTF-8 path"));
            }
        };
        // The mesh loader (see differential/wrap_w3d_mesh.cpp) and the texture
        // readers (differential/wrap_texture.cpp), linked before
        // upstream_w3d, whose chunk reader and pool functions it uses.
        let mut mb = cc::Build::new();
        w3d_build(&mut mb, true);
        mb.file(dif.join("wrap_w3d_mesh.cpp"))
            .file(&meshmdlio)
            .file(&meshmatdesc)
            .file(w3d.join("w3d_util.cpp"))
            .file(ww.join("WWLib/realcrc.cpp"))
            .file(&multilist)
            .file(dif.join("wrap_texture.cpp"))
            .file(&ddsfile)
            .file(&formconv)
            .file(w3d.join("ww3dformat.cpp"))
            .file(&targa);
        mb.compile("upstream_w3d_mesh");
        let mut wb = cc::Build::new();
        w3d_build(&mut wb, false);
        wb.file(dif.join("wrap_w3d.cpp"))
            .file(ww.join("WWLib/chunkio.cpp"))
            .file(&motchan)
            .file(&htree)
            .file(w3d.join("hrawanim.cpp"))
            .file(w3d.join("hcanim.cpp"))
            .file(w3d.join("pivot.cpp"))
            .file(ww.join("WWMath/quat.cpp"));
        wb.compile("upstream_w3d");

        // BaseType.h's value types and templates (differential/
        // wrap_value_types.cpp): two segments of the real header's text,
        // copied verbatim into OUT_DIR/value_types_gen/basetype_derived.h,
        // each located by its first and last line, which must occur exactly
        // once. g++ cannot compile the rest of the header (MSVC __int64,
        // __forceinline, __asm). Compiled before upstream_gameengine, whose
        // Trig.cpp supplies the ACos that Coord2D::toAngle calls.
        let basetype = std::fs::read_to_string(up.join("Libraries/Include/Lib/BaseType.h")).expect("read BaseType.h");
        let segment = |first: &str, last: &str| -> String {
            assert_eq!(basetype.matches(first).count(), 1, "BaseType.h segment start {first:?} must occur once");
            let a = basetype.find(first).expect("segment start");
            let rel = basetype[a..].find(last).expect("segment end");
            assert_eq!(basetype[a..].matches(last).count(), 1, "BaseType.h segment end {last:?} must occur once");
            basetype[a..a + rel + last.len()].to_string()
        };
        let templates = segment(
            "template <typename NUM>\ninline NUM sqr(NUM x)",
            "inline Real deg2rad(Real rad) { return rad * (PI/180); }\n",
        );
        let derived = segment("struct RealRange \n", "struct RGBAColorInt\n{\n\n\tUnsignedInt red, green, blue, alpha;  // range between 0 and 255\n\n};\n");
        let vt_gen = out_dir.join("value_types_gen");
        std::fs::create_dir_all(&vt_gen).expect("create value_types_gen");
        std::fs::write(
            vt_gen.join("basetype_derived.h"),
            format!(
                "/* generated by build.rs: verbatim segments of Libraries/Include/Lib/BaseType.h */\n{templates}\n{derived}"
            ),
        )
        .expect("write basetype_derived.h");
        let mut vt = cc::Build::new();
        base(&mut vt);
        vt.include(&vt_gen)
            .include(&shim) // math.h only: this file includes no shim BaseType.h
            .include(up.join("Libraries/Include"))
            .file(dif.join("wrap_value_types.cpp"));
        vt.compile("upstream_value_types");

        // --- p1w2-globaldata ---
        // GlobalData (differential/wrap_globaldata.cpp): GlobalData.cpp from a
        // copy, INIGameData.cpp unchanged (the wrapper #includes it), and the
        // WeaponBonusSet functions, on the real headers and the stand-ins of
        // differential/shim_globaldata. Linked here, ahead of upstream_money,
        // the INI libraries and upstream_xfer, which it calls.
        //
        // The one edit of GlobalData.cpp: the tail of parseGameDataDefinition,
        // from the comment "override INI values with user preferences" to the
        // function's closing brace (the last text of the file), applies the
        // player's OptionPreferences (Options.ini in the user data directory)
        // to twelve members; it is cut, and the brace put back. Asserted to
        // match once and to be the end of the file.
        //
        // Verbatim extractions into OUT_DIR/globaldata_gen, each from its
        // first line, which must occur once, to the first occurrence of its
        // last line after it (the rest of those headers needs most of the
        // engine): AIDebugOptions (AI.h), BodyDamageType and its names
        // (BodyModule.h), TerrainLOD and its names (TerrainVisual.h), and
        // WeaponBonusConditionType, WeaponBonus and WeaponBonusSet with their
        // names (Weapon.h) and their member functions (Weapon.cpp). The one
        // value taken from FirewallHelper.h (FIREWALL_TYPE_UNKNOWN = 0) is
        // asserted to be there once, and written in the stand-in header.
        {
            let gd_shim = dif.join("shim_globaldata");
            let shim_ini = dif.join("shim_ini");
            let engine = up.join("GameEngine");
            let gen = out_dir.join("globaldata_gen");
            for d in ["GameLogic/Module", "GameClient", "GameNetwork"] {
                std::fs::create_dir_all(gen.join(d)).expect("create globaldata_gen dir");
            }
            let extract = |file: &str, first: &str, last: &str| -> String {
                let text = std::fs::read_to_string(engine.join(file)).expect("read upstream file");
                assert_eq!(text.matches(first).count(), 1, "{file}: {first:?} must occur once");
                let a = text.find(first).expect("segment start");
                let rel = text[a..].find(last).unwrap_or_else(|| panic!("{file}: {last:?} must follow {first:?}"));
                text[a..a + rel + last.len()].to_string()
            };
            let header = |name: &str, from: &str, body: &str| {
                let guard: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
                std::fs::write(
                    gen.join(name),
                    format!("/* generated by build.rs: verbatim from {from} */\n#ifndef GLOBALDATA_GEN_{guard}\n#define GLOBALDATA_GEN_{guard}\n{body}\n#endif\n"),
                )
                .expect("write generated header");
            };
            header("GameLogic/AI.h", "Include/GameLogic/AI.h", &extract("Include/GameLogic/AI.h", "enum AIDebugOptions", "AI_DEBUG_END\n};"));
            header(
                "GameLogic/Module/BodyModule.h",
                "Include/GameLogic/Module/BodyModule.h",
                &extract("Include/GameLogic/Module/BodyModule.h", "enum BodyDamageType", "\tNULL\n};\n#endif"),
            );
            header(
                "GameClient/TerrainVisual.h",
                "Include/GameClient/TerrainVisual.h",
                &extract("Include/GameClient/TerrainVisual.h", "typedef enum _TerrainLOD", "#endif  // end DEFINE_TERRAIN_LOD_NAMES"),
            );
            header(
                "GameLogic/Weapon.h",
                "Include/GameLogic/Weapon.h",
                &format!(
                    "#include \"Common/INI.h\"\n#include \"Common/GameMemory.h\"\n{}\n",
                    extract("Include/GameLogic/Weapon.h", "enum WeaponBonusConditionType", "EMPTY_DTOR(WeaponBonusSet)")
                ),
            );
            let fw = std::fs::read_to_string(engine.join("Include/GameNetwork/FirewallHelper.h")).expect("read FirewallHelper.h");
            assert_eq!(fw.matches("FIREWALL_TYPE_UNKNOWN = 0,").count(), 1, "FirewallHelper.h: FIREWALL_TYPE_UNKNOWN = 0 must occur once");
            header(
                "GameNetwork/FirewallHelper.h",
                "Include/GameNetwork/FirewallHelper.h (FIREWALL_TYPE_UNKNOWN only)",
                "class FirewallHelperClass { public: enum { FIREWALL_TYPE_UNKNOWN = 0 }; };",
            );
            let weapon_cpp = extract(
                "Source/GameLogic/Object/Weapon.cpp",
                "void WeaponBonus::appendBonuses(WeaponBonus& bonus) const",
                "\t\tthis->m_bonus[i].appendBonuses(bonus);\n\t}\n}",
            );
            let weapon_bonus_cpp = write(
                "globaldata_weapon_bonus.cpp",
                format!(
                    "/* generated by build.rs: verbatim from Source/GameLogic/Object/Weapon.cpp */\n#include \"PreRTS.h\"\n\
                     #define DEFINE_WEAPONBONUSCONDITION_NAMES\n#define DEFINE_WEAPONBONUSFIELD_NAMES\n\
                     #include \"Common/INI.h\"\n#include \"GameLogic/Weapon.h\"\n{weapon_cpp}\n"
                )
                .as_bytes(),
            );
            let gd_src = std::fs::read_to_string(engine.join("Source/Common/GlobalData.cpp")).expect("read GlobalData.cpp");
            let marker = "\t// override INI values with user preferences\n";
            assert_eq!(gd_src.matches(marker).count(), 1, "GlobalData.cpp: the user preferences comment must occur once");
            let cut = gd_src.find(marker).expect("marker");
            assert!(
                gd_src[cut..].trim_end().ends_with('}') && gd_src[cut..].matches("\n}").count() == 1,
                "GlobalData.cpp: the preferences must be the end of its last function"
            );
            let global_data_cpp = write("GlobalData_no_preferences.cpp", format!("{}}}\n", &gd_src[..cut]).as_bytes());

            let globaldata_build = |cfg: &mut cc::Build| {
                base(cfg);
                cfg.compiler("clang++")
                    .flag("-fms-extensions")
                    .flag("-fdelayed-template-parsing")
                    .include(&gen)
                    .include(&gd_shim)
                    .include(&shim_ini)
                    .include(&shim)
                    .include(engine.join("Include"))
                    .include(up.join("Libraries/Include"))
                    .include(ww.join("WWMath"))
                    .include(ww.join("WWLib"))
                    .include(&dif)
                    .flag("-include")
                    .flag(gd_shim.join("globaldata_prelude.h").to_str().expect("UTF-8 path"));
            };
            // the wrapper reads private members (GLOBALDATA_WRAPPER, see the
            // prelude); it is its own library, ahead of the one it calls
            let mut gdw = cc::Build::new();
            globaldata_build(&mut gdw);
            gdw.define("GLOBALDATA_WRAPPER", None).file(dif.join("wrap_globaldata.cpp"));
            gdw.compile("upstream_globaldata_wrap");
            let mut gd = cc::Build::new();
            globaldata_build(&mut gd);
            gd.file(&global_data_cpp).file(&weapon_bonus_cpp);
            gd.compile("upstream_globaldata");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/GlobalData.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INIGameData.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/Common/GlobalData.h",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/Weapon.h",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameNetwork/FirewallHelper.h",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w2-globaldata ---

        // --- p1w1-geometry ---
        // GeometryInfo: Geometry.cpp unchanged (differential/wrap_geometry.cpp),
        // with the INI build's headers and prelude. Compiled before the
        // libraries it calls (RandomValue, the Xfer family, INI), as the link
        // order requires (see the comment above the W3D group).
        {
            let shim_ini = dif.join("shim_ini");
            let mut geo = cc::Build::new();
            base(&mut geo);
            geo.include(&shim_ini)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .flag("-include")
                .flag(shim_ini.join("ini_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_geometry.cpp"));
            geo.compile("upstream_geometry");
        }
        // BezierSegment.cpp and BezFwdIterator.cpp, unchanged
        // (differential/wrap_bezier.cpp), on shim_bezier: Coord3D's methods
        // and the D3DX math transcription (see the shim headers).
        {
            let shim_bezier = dif.join("shim_bezier");
            let mut bez = cc::Build::new();
            base(&mut bez);
            bez.include(&shim_bezier)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_bezier.cpp"));
            bez.compile("upstream_bezier");
        }
        // Money.cpp and Handicap.cpp, unchanged (differential/wrap_money.cpp,
        // wrap_handicap.cpp), on shim_money: stand-ins for the layer 5 to 7
        // singletons they call, which log what logic asks of the client. The
        // money group also reads the INI field through the INI build's
        // headers (shim_ini's prelude).
        {
            let shim_money = dif.join("shim_money");
            let shim_ini = dif.join("shim_ini");
            let mut money = cc::Build::new();
            base(&mut money);
            money
                .include(&shim_money)
                .include(&shim_ini)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .flag("-include")
                .flag(shim_ini.join("ini_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_money.cpp"));
            money.compile("upstream_money");

            let mut handicap = cc::Build::new();
            base(&mut handicap);
            handicap
                .include(&shim_money)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .file(dif.join("wrap_handicap.cpp"));
            handicap.compile("upstream_handicap");
        }
        // --- end p1w1-geometry ---

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
            .file(dif.join("wrap_discrete_circle.cpp"))
            // --- p1w1-partition ---
            .file(dif.join("wrap_partition_solver.cpp"))
            // --- end p1w1-partition ---
            .file(up.join("GameEngine/Source/Common/crc.cpp"));
        ge.compile("upstream_gameengine");

        let sys = up.join("GameEngine/Source/Common/System");

        // BitFlags and its instantiations (differential/wrap_flags.cpp):
        // BitFlags.cpp, KindOf.cpp, ObjectStatusTypes.cpp and
        // DisabledTypes.cpp unchanged; the s_bitNameList definitions of
        // WeaponSet.cpp and SpecialPower.cpp (files that need most of the
        // engine) copied verbatim into OUT_DIR/flags_gen/bit_name_lists.h,
        // each from its first line to the `};` that ends it, asserted to
        // occur once. Upstream defines every s_bitNameList as a member of a
        // class template specialisation without `template<>`, which MSVC
        // accepted and g++ and clang reject: the copies (the four .cpp files
        // in OUT_DIR, and the two extracted lists) add `template<> ` before
        // each definition, asserted once per list. clang++ with
        // -fms-extensions -fdelayed-template-parsing (the MSVC dialect of
        // the headers; SparseMatchFinder.h, which the test instantiates,
        // gets its missing `typename`s in a copy, below). shim_flags comes
        // first: the real BitFlagsIO.h (after KindOf.h, for Xfer.h's
        // forward-declared `enum KindOfType;`, see the file) and
        // ModelState.h, and an STLTypedefs.h without the KindOfType stand-in
        // the other builds use. Linked before upstream_ini
        // (INI's tokenizer, scanIndexList) and upstream_xfer (XferSave,
        // XferLoad, XferCRC).
        let flags_gen = out_dir.join("flags_gen");
        std::fs::create_dir_all(&flags_gen).expect("create flags_gen");
        let mut lists = String::from("/* generated by build.rs: verbatim s_bitNameList definitions */\n");
        for (file, first) in [
            ("GameEngine/Source/GameLogic/Object/WeaponSet.cpp", "const char* WeaponSetFlags::s_bitNameList[] = \n"),
            ("GameEngine/Source/Common/RTS/SpecialPower.cpp", "const char* SpecialPowerMaskType::s_bitNameList[] = \n"),
        ] {
            let text = std::fs::read_to_string(up.join(file)).expect("read name-list source");
            assert_eq!(text.matches(first).count(), 1, "{file}: {first:?} must occur once");
            let a = text.find(first).expect("list start");
            let b = a + text[a..].find("\n};\n").expect("list end") + 4;
            lists.push_str(&format!("/* {file} */\ntemplate<> {}\n", &text[a..b]));
        }
        std::fs::write(flags_gen.join("bit_name_lists.h"), lists).expect("write bit_name_lists.h");
        // SparseMatchFinder.h, instantiated by the test: a copy in
        // OUT_DIR/flags_gen/Common (found before the upstream header) adds
        // the two `typename`s MSVC did not require on dependent types,
        // each asserted once.
        std::fs::create_dir_all(flags_gen.join("Common")).expect("create flags_gen/Common");
        std::fs::write(
            flags_gen.join("Common/SparseMatchFinder.h"),
            patch_copy(
                &up.join("GameEngine/Include/Common/SparseMatchFinder.h"),
                "SparseMatchFinder.h",
                &[
                    ("for (std::vector<MATCHABLE>::const_iterator it", "for (typename std::vector<MATCHABLE>::const_iterator it", 1),
                    ("\t\tMatchMap::const_iterator it = m_bestMatches.find(bits);", "\t\ttypename MatchMap::const_iterator it = m_bestMatches.find(bits);", 1),
                ],
            ),
        )
        .expect("write SparseMatchFinder.h copy");
        let mut fl = cc::Build::new();
        base(&mut fl);
        fl.compiler("clang++")
            .flag("-fms-extensions")
            .flag("-fdelayed-template-parsing")
            .include(&flags_gen)
            .include(dif.join("shim_flags"))
            .include(dif.join("shim_ini"))
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .include(ww.join("WWMath"))
            .include(ww.join("WWLib"))
            .include(&dif)
            .flag("-include")
            .flag(dif.join("shim_flags/flags_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_flags.cpp"));
        for (src, name, lists) in [
            (up.join("GameEngine/Source/Common/BitFlags.cpp"), "BitFlags.cpp", &["ModelConditionFlags", "ArmorSetFlags"][..]),
            (sys.join("KindOf.cpp"), "KindOf.cpp", &["KindOfMaskType"][..]),
            (sys.join("ObjectStatusTypes.cpp"), "ObjectStatusTypes.cpp", &["ObjectStatusMaskType"][..]),
            (sys.join("DisabledTypes.cpp"), "DisabledTypes.cpp", &["DisabledMaskType"][..]),
        ] {
            let edits: Vec<(String, String)> = lists
                .iter()
                .map(|t| (format!("const char* {t}::s_bitNameList[] = "), format!("template<> const char* {t}::s_bitNameList[] = ")))
                .collect();
            let edits: Vec<(&str, &str, usize)> = edits.iter().map(|(a, b)| (a.as_str(), b.as_str(), 1)).collect();
            let copy = write(&format!("template_spec_{name}"), &patch_copy(&src, name, &edits));
            fl.file(copy);
        }
        fl.compile("upstream_flags");

        // --- p1w1-armor ---
        // Armor, Damage and DamageFX (differential/wrap_armor.cpp): Armor.cpp,
        // DamageFX.cpp and INI/INIDamageFX.cpp unchanged, Damage.cpp from a
        // copy that adds the `template<> ` before its one s_bitNameList
        // definition (the g++/clang spelling, as for the flags build above;
        // asserted once). The BitFlags build's setup (clang++
        // -fms-extensions, the real BitFlagsIO.h through flags_prelude.h)
        // plus differential/shim_armor: ThingTemplate, ThingFactory, FXList,
        // Object and an ordered std::hash_map. Linked here, ahead of
        // upstream_xfer, upstream_ini and upstream_chunk, whose Xfer,
        // INI and NameKeyGenerator it uses; the weak stand-ins of
        // wrap_ini.cpp give way to the real definitions.
        let shim_armor = dif.join("shim_armor");
        let damage_copy = write(
            "Damage_template_spec.cpp",
            &patch_copy(
                &up.join("GameEngine/Source/GameLogic/System/Damage.cpp"),
                "Damage.cpp",
                &[("const char* DamageTypeFlags::s_bitNameList[] = ", "template<> const char* DamageTypeFlags::s_bitNameList[] = ", 1)],
            ),
        );
        let mut arm = cc::Build::new();
        base(&mut arm);
        arm.compiler("clang++")
            .flag("-fms-extensions")
            .flag("-fdelayed-template-parsing")
            .include(&shim_armor)
            .include(&flags_gen)
            .include(dif.join("shim_flags"))
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .include(ww.join("WWMath"))
            .include(ww.join("WWLib"))
            .include(&dif)
            .flag("-include")
            .flag(shim_armor.join("armor_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_armor.cpp"))
            .file(up.join("GameEngine/Source/GameLogic/Object/Armor.cpp"))
            .file(up.join("GameEngine/Source/Common/DamageFX.cpp"))
            .file(up.join("GameEngine/Source/Common/INI/INIDamageFX.cpp"))
            .file(damage_copy);
        arm.compile("upstream_armor");
        for p in [
            "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Armor.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/DamageFX.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INIDamageFX.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/System/Damage.cpp",
        ] {
            println!("cargo:rerun-if-changed={p}");
        }
        // --- end p1w1-armor ---
        // --- p1w2-terrain ---
        // TerrainTypes, MultiplayerSettings and CrateSystem with their INI
        // blocks (differential/wrap_terrain.cpp): TerrainTypes.cpp,
        // MultiplayerSettings.cpp, CrateSystem.cpp, and INI/INITerrain.cpp,
        // INI/INIMultiplayer.cpp, INI/INICrate.cpp, all unchanged, built as the
        // armor build is (clang++ -fms-extensions, the real BitFlagsIO.h through
        // flags_prelude.h, the bit-name lists of flags_gen), with shim_terrain
        // ahead of shim_flags for the client and game-info headers they name.
        // Linked ahead of upstream_ini, whose INI::initFromINI and the
        // INI reader they use.
        let shim_terrain = dif.join("shim_terrain");
        let mut ter = cc::Build::new();
        base(&mut ter);
        ter.compiler("clang++")
            .flag("-fms-extensions")
            .flag("-fdelayed-template-parsing")
            .include(&shim_terrain)
            .include(&flags_gen)
            .include(dif.join("shim_flags"))
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .include(ww.join("WWMath"))
            .include(ww.join("WWLib"))
            .include(&dif)
            .flag("-include")
            .flag(shim_terrain.join("terrain_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_terrain.cpp"))
            .file(up.join("GameEngine/Source/Common/TerrainTypes.cpp"))
            .file(up.join("GameEngine/Source/Common/MultiplayerSettings.cpp"))
            .file(up.join("GameEngine/Source/GameLogic/System/CrateSystem.cpp"))
            .file(up.join("GameEngine/Source/Common/INI/INITerrain.cpp"))
            .file(up.join("GameEngine/Source/Common/INI/INIMultiplayer.cpp"))
            .file(up.join("GameEngine/Source/Common/INI/INICrate.cpp"));
        ter.compile("upstream_terrain");
        for p in [
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/TerrainTypes.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/MultiplayerSettings.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/System/CrateSystem.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INITerrain.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INIMultiplayer.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INICrate.cpp",
        ] {
            println!("cargo:rerun-if-changed={p}");
        }
        // --- end p1w2-terrain ---

        // --- p1w2-science ---
        // Science and RankInfo (differential/wrap_science.cpp): Science.cpp
        // and RankInfo.cpp unchanged. The Armor build's setup (clang++
        // -fms-extensions, the real BitFlagsIO.h through flags_prelude.h)
        // plus differential/shim_science: the REAL Science.h (included, with
        // private made public, by science_prelude.h before the stand-in of
        // differential/shim could be found) and a Player with hasScience only.
        // Linked ahead of upstream_xfer, upstream_ini and upstream_chunk,
        // whose Xfer, INI and NameKeyGenerator it uses; the weak stand-ins of
        // wrap_ini.cpp and wrap_xfer.cpp give way to the real definitions.
        let shim_science = dif.join("shim_science");
        let mut sci = cc::Build::new();
        base(&mut sci);
        sci.compiler("clang++")
            .flag("-fms-extensions")
            .flag("-fdelayed-template-parsing")
            .include(&shim_science)
            .include(&shim_armor)
            .include(&flags_gen)
            .include(dif.join("shim_flags"))
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .include(ww.join("WWMath"))
            .include(ww.join("WWLib"))
            .include(&dif)
            // The real lookup keeps its own name in this archive, so that
            // wrap_ini.cpp's single dispatcher (shared by every test binary)
            // can call it through ref_science_lookup_hook.
            .define("friend_lookupScience", "real_friend_lookupScience")
            .flag("-include")
            .flag(shim_science.join("science_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_science.cpp"))
            .file(up.join("GameEngine/Source/Common/RTS/Science.cpp"))
            .file(up.join("GameEngine/Source/GameLogic/System/RankInfo.cpp"));
        sci.compile("upstream_science");
        for p in [
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Science.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/System/RankInfo.cpp",
        ] {
            println!("cargo:rerun-if-changed={p}");
        }
        // --- end p1w2-science ---
        // --- p1w2-upgrade ---
        // Upgrade and SpecialPower (differential/wrap_upgrade.cpp, which
        // #includes Upgrade.cpp, INIUpgrade.cpp, SpecialPower.cpp and
        // INISpecialPower.cpp under renaming macros, see the wrapper):
        // the Armor build's setup (clang++ -fms-extensions, the real
        // BitFlagsIO.h through the flags prelude) plus
        // differential/shim_upgrade first on the include path (a forwarder
        // to the REAL Upgrade.h, ahead of the Xfer build's stand-in in
        // shim/Common, and the stand-ins of the Player, Object, client and
        // audio headers). Two generated files, in OUT_DIR/up_gen:
        // SpecialPower_names_renamed.cpp, a copy of SpecialPower.cpp in which
        // the one definition of `SpecialPowerMaskType::s_bitNameList` is
        // renamed (asserted once; the BitFlags build defines that member and
        // keeps it, and the renamed array is dumped so the Rust list is
        // checked against the real file); academy_names.inc, the definition
        // of `TheAcademyClassificationTypeNames` cut verbatim out of
        // AcademyStats.cpp (asserted once; the rest of that file is layer 7).
        // Linked ahead of upstream_xfer, upstream_ini and upstream_chunk like
        // the Armor build.
        {
            let shim_up = dif.join("shim_upgrade");
            let up_gen = out_dir.join("up_gen");
            std::fs::create_dir_all(&up_gen).expect("create up_gen");
            let sp_copy = patch_copy(
                &up.join("GameEngine/Source/Common/RTS/SpecialPower.cpp"),
                "SpecialPower.cpp",
                &[("const char* SpecialPowerMaskType::s_bitNameList[] = ", "const char* UpSpecialPowerRealNames[] = ", 1)],
            );
            std::fs::write(up_gen.join("SpecialPower_names_renamed.cpp"), sp_copy).expect("write SpecialPower copy");
            let academy = std::fs::read_to_string(up.join("GameEngine/Source/Common/RTS/AcademyStats.cpp")).expect("read AcademyStats.cpp");
            let first = "const char *TheAcademyClassificationTypeNames[] = \n{";
            assert_eq!(academy.matches(first).count(), 1, "AcademyStats.cpp names definition must occur once");
            let a = academy.find(first).expect("names definition");
            let rel = academy[a..].find("};").expect("end of names definition");
            std::fs::write(
                up_gen.join("academy_names.inc"),
                format!("/* generated by build.rs: verbatim from AcademyStats.cpp */\n{}\n", &academy[a..a + rel + 2]),
            )
            .expect("write academy_names.inc");
            let mut upg = cc::Build::new();
            base(&mut upg);
            upg.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&shim_up)
                .include(&up_gen)
                .include(&flags_gen)
                .include(dif.join("shim_flags"))
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .include(&dif)
                .flag("-include")
                .flag(shim_up.join("upgrade_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_upgrade.cpp"));
            upg.compile("upstream_upgrade");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Upgrade.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INIUpgrade.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/SpecialPower.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/AcademyStats.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INISpecialPower.cpp",
                "differential/wrap_upgrade.cpp",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w2-upgrade ---

        // The Xfer family: Xfer.cpp (via wrap_xfer.cpp, after matrix3d.h),
        // XferCRC.cpp, XferSave.cpp, XferLoad.cpp and Snapshot.cpp unchanged,
        // against the shims in differential/shim/Common. `_DEBUG` again only
        // selects crc.h's C++ class (XferCRC.cpp includes crc.h, unused).
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
            .file(dif.join("wrap_w3d_file.cpp"))
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
        // The game-wide enums and name tables (differential/wrap_game_types.cpp)
        // and GameType.cpp, unchanged, next to GameCommon.cpp.
        iniw.file(dif.join("wrap_ini.cpp"))
            .file(sys.join("GameCommon.cpp"))
            .file(dif.join("wrap_game_types.cpp"))
            .file(sys.join("GameType.cpp"))
            .include(&dif);
        iniw.compile("upstream_ini_wrap");


        // Dict.cpp unchanged (differential/wrap_dict.cpp), with the
        // pointer-sized string shims and the zero-filling allocator of
        // differential/shim_dict ahead of differential/shim. The class
        // names are renamed by macro in this build: its AsciiString and
        // UnicodeString are not the std::string-based shims the other
        // builds use, and the chunk build has a stand-in Dict, so the same
        // mangled names would otherwise denote different classes (an ODR
        // violation the linker would resolve by picking one).
        let mut dict = cc::Build::new();
        base(&mut dict);
        dict.define("AsciiString", Some("DictShimAsciiString"))
            .define("UnicodeString", Some("DictShimUnicodeString"))
            .define("Dict", Some("UpstreamDict"))
            .include(dif.join("shim_dict"))
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .file(dif.join("wrap_dict.cpp"))
            .file(up.join("GameEngine/Source/Common/Dict.cpp"));
        dict.compile("upstream_dict");

        // AsciiString.cpp unchanged (differential/wrap_ascii_string.cpp),
        // with the REAL AsciiString.h (shim_ascii forwards to it, ahead of the
        // std::string stand-in of differential/shim) on shim_dict's
        // allocator and pointer-sized UnicodeString. Renamed by macro, as in
        // the Dict build: AsciiString (the other builds' stand-in has the same
        // name), UnicodeString (to the Dict build's name for the same shim)
        // and TheDynamicMemoryAllocator (the Dict build defines one too).
        let mut ascii = cc::Build::new();
        base(&mut ascii);
        ascii
            .define("AsciiString", Some("UpstreamAsciiString"))
            .define("UnicodeString", Some("DictShimUnicodeString"))
            .define("TheDynamicMemoryAllocator", Some("AsciiBuildDynamicMemoryAllocator"))
            .include(dif.join("shim_ascii"))
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .flag("-include")
            .flag(dif.join("shim_ascii/ascii_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_ascii_string.cpp"))
            .file(sys.join("AsciiString.cpp"));
        ascii.compile("upstream_ascii_string");

        // --- p1w1-unicode ---
        // UnicodeString.cpp unchanged (differential/wrap_unicode_string.cpp),
        // with the REAL UnicodeString.h (shim_unicode forwards to it) on
        // shim_dict's allocator, a stand-in AsciiString (translate reads only
        // getLength/getCharAt) and a no-op critical section. -fshort-wchar
        // makes wchar_t (hence WideChar and the L"..." literals) 16-bit
        // unsigned as in MSVC; the wide C runtime functions are replaced by
        // 16-bit versions in shim_unicode/unicode_prelude.h (each documented
        // there). Classes renamed by macro, as in the Dict and AsciiString
        // builds, so that the same mangled names cannot denote different
        // classes in different builds.
        let mut unicode = cc::Build::new();
        base(&mut unicode);
        unicode
            .flag("-fshort-wchar")
            .define("UnicodeString", Some("UpstreamUnicodeString"))
            .define("AsciiString", Some("UnicodeBuildAsciiString"))
            .define("TheDynamicMemoryAllocator", Some("UnicodeBuildDynamicMemoryAllocator"))
            .include(dif.join("shim_unicode"))
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .flag("-include")
            .flag(dif.join("shim_unicode/unicode_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_unicode_string.cpp"))
            .file(sys.join("UnicodeString.cpp"));
        unicode.compile("upstream_unicode_string");
        // --- end p1w1-unicode ---

        // The CSF string loader: GameText.cpp unchanged, with csf_prelude.h
        // force-included (windows.h names, SubsystemInterface; see the file)
        // and the stand-ins of differential/shim_csf.
        let shim_csf = dif.join("shim_csf");
        let mut csf = cc::Build::new();
        base(&mut csf);
        csf.include(&shim_csf)
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .flag("-include")
            .flag(shim_csf.join("csf_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_csf.cpp"))
            .file(up.join("GameEngine/Source/GameClient/GameText.cpp"));
        csf.compile("upstream_csf");

        // The chunk file reader: DataChunk.cpp and NameKeyGenerator.cpp
        // unchanged, with chunk_prelude.h force-included and the stand-in
        // Dict of differential/shim_chunk (see the file). Compression.h comes
        // from Libraries/Source/Compression (declarations only; the codecs
        // are not compiled).
        let shim_chunk = dif.join("shim_chunk");
        let mut chunk = cc::Build::new();
        base(&mut chunk);
        chunk
            .include(&shim_chunk)
            .include(&dif) // well_known_keys_list.h
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .include(up.join("Libraries/Source/Compression"))
            .flag("-include")
            .flag(shim_chunk.join("chunk_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_chunk.cpp"))
            .file(dif.join("wrap_namekey.cpp"))
            .file(sys.join("DataChunk.cpp"))
            .file(up.join("GameEngine/Source/Common/NameKeyGenerator.cpp"));
        chunk.compile("upstream_chunk");

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

        // --- p1w1-small ---
        // LatchRestore.h, Overridable.h and Override.h, unchanged, through
        // differential/wrap_latch_override.cpp. The only stand-in is
        // shim/Common/GameMemory.h (MemoryPoolObject, as in the other builds).
        let mut lo = cc::Build::new();
        base(&mut lo);
        lo.include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .file(dif.join("wrap_latch_override.cpp"));
        lo.compile("upstream_latch_override");

        // STLTypedefs.h, unchanged, through differential/wrap_stl_typedefs.cpp,
        // with AsciiString.cpp unchanged in the same build (its out-of-line
        // functions, e.g. removeLastChar). The real AsciiString.h is built as
        // in the AsciiString build (the classes renamed by macro, so this
        // build's AsciiString cannot be linked against another's). shim_stl supplies the BaseType and Trig
        // forwarders (the real BaseType.h is MSVC-only), an opaque declaration
        // of the three enums STLTypedefs.h forward-declares without a base, and
        // a UnicodeString with compareNoCase (which only aborts if called).
        let shim_stl = dif.join("shim_stl");
        let mut stl = cc::Build::new();
        base(&mut stl);
        stl.define("AsciiString", Some("StlTypedefsAsciiString"))
            .define("UnicodeString", Some("StlTypedefsUnicodeString"))
            .define("TheDynamicMemoryAllocator", Some("StlTypedefsAllocator"))
            .include(&shim_stl)
            .include(dif.join("shim_ascii"))
            .include(dif.join("shim_dict"))
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .flag("-include")
            .flag(shim_stl.join("stl_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_stl_typedefs.cpp"))
            .file(sys.join("AsciiString.cpp"));
        stl.compile("upstream_stl_typedefs");

        // --- p1w2-dictmove ---
        // STLTypedefs.h, unchanged, through differential/wrap_stl_typedefs_unicode.cpp,
        // for rts::less_than_nocase<UnicodeString>. This is a build of its own
        // because the stl build above holds a UnicodeString stand-in whose
        // compareNoCase aborts. Here the REAL UnicodeString.h and
        // UnicodeString.cpp are compiled unchanged (-fshort-wchar, with the
        // 16-bit wide C runtime of shim_unicode/unicode_prelude.h, forced in
        // by shim_stl_unicode/stl_unicode_prelude.h), with the REAL AsciiString.h
        // (shim_ascii), which STLTypedefs.h declares its functors against.
        // AsciiString.cpp is not needed: nothing here calls its out-of-line
        // functions. Classes renamed by macro, as in the other builds.
        let mut stlu = cc::Build::new();
        base(&mut stlu);
        stlu.flag("-fshort-wchar")
            .define("AsciiString", Some("StlUnicodeAsciiString"))
            .define("UnicodeString", Some("StlUnicodeUnicodeString"))
            .define("TheDynamicMemoryAllocator", Some("StlUnicodeAllocator"))
            .include(dif.join("shim_stl_unicode"))
            .include(dif.join("shim_ascii"))
            .include(dif.join("shim_stl"))
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .flag("-include")
            .flag(dif.join("shim_stl_unicode/stl_unicode_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_stl_typedefs_unicode.cpp"))
            .file(sys.join("UnicodeString.cpp"));
        stlu.compile("upstream_stl_unicode");
        // --- end p1w2-dictmove ---

        // SubsystemInterface.cpp unchanged, through differential/wrap_subsystem_interface.cpp.
        // shim_subsys holds the PreRTS.h of this unit, stand-ins for INI.h and
        // Xfer.h (their bodies abort; this build passes no INI paths), and
        // forwarders to the shim tree for every header the real
        // SubsystemInterface.h chain includes except SubsystemInterface.h itself,
        // which must be the real one.
        let mut sub = cc::Build::new();
        base(&mut sub);
        sub.include(dif.join("shim_subsys"))
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .file(dif.join("wrap_subsystem_interface.cpp"))
            .file(sys.join("SubsystemInterface.cpp"));
        sub.compile("upstream_subsystem_interface");
        // --- end p1w1-small ---

        for p in [
            "differential",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/RandomValue.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/crc.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Include/Common/crc.h",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/System",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/GameClient/GameText.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/NameKeyGenerator.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/Dict.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Include/Common/AsciiString.h",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/DiscreteCircle.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/BitFlags.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/WeaponSet.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/SpecialPower.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic",
            "upstream/GeneralsMD/Code/GameEngine/Include/GameClient",
            "upstream/GeneralsMD/Code/GameEngine/Include/Common/QuickTrig.h",
            "upstream/GeneralsMD/Code/Libraries/Include/Lib/trig.h",
            "upstream/GeneralsMD/Code/Libraries/Include/Lib/BaseType.h",
            "upstream/GeneralsMD/Code/GameEngine/Include/Common",
            "upstream/GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common",
            "upstream/GeneralsMD/Code/GameEngineDevice/Include/Win32Device/Common",
            "upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWMath",
            "upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib",
            "upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2",
        ] {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}
