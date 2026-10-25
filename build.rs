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

    // --- p1w6-object ---
    /// The text of Object.cpp and its neighbours for differential/wrap_object.cpp (see the block in `build`).
    /// Latin-1 sources are read lossily: only comments carry non-ASCII bytes.
    fn generate_object_includes(engine: &Path, out: &Path) {
        let rd = |rel: &str| -> String { String::from_utf8_lossy(&std::fs::read(engine.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))).into_owned() };
        let wr = |name: &str, text: String| std::fs::write(out.join(name), text).unwrap_or_else(|e| panic!("write {name}: {e}"));
        fn once(text: &str, marker: &str) -> usize {
            assert_eq!(text.matches(marker).count(), 1, "{marker:?} must occur exactly once");
            text.find(marker).expect("marker")
        }
        fn cut(text: &str, start: &str, end: &str) -> String {
            let a = once(text, start);
            let b = once(text, end);
            assert!(a < b, "{start:?} must come before {end:?}");
            format!("{}\n{}", &text[..a], &text[b..])
        }
        fn class_text(text: &str, start: &str, end: &str) -> String {
            let a = once(text, start);
            let b = text[a..].find(end).unwrap_or_else(|| panic!("end of {start:?}")) + a;
            format!("{}\n", &text[a..b + end.len()])
        }
        fn function_text(text: &str, sig: &str) -> String {
            let a = once(text, sig);
            let b = text[a..].find("\n}").unwrap_or_else(|| panic!("end of {sig:?}")) + a;
            format!("{}\n", &text[a..b + 2])
        }
        let dashes = "//-------------------------------------------------------------------------------------------------\n";

        // Object.cpp
        let mut src = rd("Source/GameLogic/Object/Object.cpp");
        let a = once(&src, "#include \"PreRTS.h\"");
        let endmark = "#include \"Common/DynamicAudioEventInfo.h\"\n";
        let b = once(&src, endmark) + endmark.len();
        src = format!("{}{}", &src[..a], &src[b..]);
        src = cut(&src, "static const ModelConditionFlags s_allWeaponFireFlags[WEAPONSLOT_COUNT] =", &format!("{dashes}extern void addIcon"));
        src = cut(
            &src,
            "extern void addIcon(const Coord3D *pos, Real width, Int numFramesDuration, RGBColor color);",
            &format!("{dashes}{dashes}#ifdef DEBUG_LOGGING"),
        );
        src = cut(&src, "void Object::reloadAllAmmo(Bool now)", "void Object::setModelConditionState( ModelConditionFlagType a )");
        src = cut(&src, "\nWeapon* Object::getCurrentWeapon(WeaponSlotType* wslot)\n{", "Relationship Object::getRelationship(const Object *that) const");
        src = cut(&src, "void Object::adjustModelConditionForWeaponStatus()", "void Object::onPartitionCellChange()");
        src = cut(&src, "void Object::updateObjValuesFromMapProperties(Dict* properties)", "void Object::friend_adjustPowerForPlayer( Bool incoming )");
        src = cut(
            &src,
            "void Object::doCommandButton( const CommandButton *commandButton, CommandSourceType cmdSource )",
            "ProductionUpdateInterface* Object::getProductionUpdateInterface( void )",
        );
        src = cut(&src, "Int Object::getNumConsecutiveShotsFiredAtTarget( const Object *victim ) const", "const AsciiString& Object::getCommandSetString() const");
        src = cut(&src, "Bool Object::canProduceUpgrade( const UpgradeTemplate *upgrade )", "void Object::defect( Team* newTeam, UnsignedInt detectionTime )");
        once(&src, "TheInGameUI->addWorldAnimation( animTemplate,");
        src = src.replace("TheInGameUI->addWorldAnimation( animTemplate,", "TheInGameUI->addWorldAnimation( this, animTemplate,");
        wr("object_cpp.inc", format!("/* generated: Object.cpp, includes and untranslated regions removed (build.rs) */\n{src}"));

        // the class body of Object.h
        let hdr = rd("Include/GameLogic/Object.h");
        let a = once(&hdr, "class Object : public Thing, public Snapshot");
        let endm = "};  // end class Object";
        let b = once(&hdr, endm) + endm.len();
        wr("object_class.inc", format!("/* generated: class Object from Object.h */\n{}\n", &hdr[a..b]));

        // Geometry
        let geo_h = rd("Include/Common/Geometry.h");
        let geo_c = rd("Source/Common/System/Geometry.cpp");
        wr(
            "geometry_class.inc",
            format!(
                "/* generated: GeometryType and class GeometryInfo from Geometry.h */\n{}class INI;\n{}",
                class_text(&geo_h, "enum GeometryType\n{", "\n};"),
                class_text(&geo_h, "class GeometryInfo : public Snapshot", "\n};")
            ),
        );
        let geo_sigs = [
            "void GeometryInfo::set(GeometryType type, Bool isSmall, Real height, Real majorRadius, Real minorRadius)",
            "void GeometryInfo::setMaxHeightAbovePosition(Real z)",
            "Real GeometryInfo::getMaxHeightAbovePosition() const",
            "void GeometryInfo::calcBoundingStuff()",
            "void GeometryInfo::crc( Xfer *xfer )",
            "void GeometryInfo::xfer( Xfer *xfer )",
            "void GeometryInfo::loadPostProcess( void )",
        ];
        wr(
            "geometry_cpp.inc",
            format!("/* generated: the GeometryInfo functions Object reaches, from Geometry.cpp */\n{}", geo_sigs.iter().map(|s| function_text(&geo_c, s)).collect::<String>()),
        );

        // Thing
        let th_h = rd("Include/Common/Thing.h");
        let th_c = rd("Source/Common/Thing/Thing.cpp");
        wr("thing_class.inc", format!("/* generated: class Thing from Thing.h */\n{}", class_text(&th_h, "class Thing : public MemoryPoolObject", "\n}; \n")));
        let a = once(&th_c, "//=============================================================================\n/** Constructor */");
        wr("thing_cpp.inc", format!("/* generated: Thing.cpp without its includes */\n{}", &th_c[a..]));

        // ExperienceTracker
        let ex_h = rd("Include/GameLogic/ExperienceTracker.h");
        let ex_c = rd("Source/GameLogic/Object/ExperienceTracker.cpp");
        wr(
            "experience_class.inc",
            format!("/* generated: class ExperienceTracker from ExperienceTracker.h */\n{}", class_text(&ex_h, "class ExperienceTracker : public MemoryPoolObject, public Snapshot", "\n};")),
        );
        let a = once(&ex_c, &format!("{dashes}ExperienceTracker::ExperienceTracker(Object *parent)"));
        wr("experience_cpp.inc", format!("/* generated: ExperienceTracker.cpp without its includes */\n{}", &ex_c[a..]));

        // Coord3D, ICoord3D, Region3D
        let bt = String::from_utf8_lossy(&std::fs::read(engine.join("../Libraries/Include/Lib/BaseType.h")).expect("read BaseType.h")).into_owned();
        let struct_text = |first: &str| -> String {
            let a = once(&bt, first);
            let b = bt[a..].find("\n};\n").unwrap_or_else(|| panic!("end of {first:?}")) + a;
            bt[a..b + 4].to_string()
        };
        wr(
            "basetype_coords.inc",
            format!(
                "/* generated: Coord3D, ICoord3D and Region3D verbatim from Lib/BaseType.h */\n{}\n{}\n{}",
                struct_text("struct Coord3D \n{"),
                struct_text("struct ICoord3D \n{"),
                struct_text("struct Region3D\n{")
            ),
        );

        // the enums, verbatim from the headers that define them: `enum Name` followed by `{` (a bare `enum Name;` is a declaration)
        let enums = [
            ("Common/ObjectStatusTypes.h", "ObjectStatusTypes"),
            ("Common/DisabledTypes.h", "DisabledType"),
            ("Common/KindOf.h", "KindOfType"),
            ("Common/ModelState.h", "ModelConditionFlagType"),
            ("GameLogic/WeaponSetType.h", "WeaponSetType"),
            ("GameLogic/Weapon.h", "WeaponBonusConditionType"),
            ("GameLogic/ArmorSet.h", "ArmorSetType"),
            ("Common/SpecialPowerType.h", "SpecialPowerType"),
            ("GameLogic/Damage.h", "DamageType"),
            ("GameLogic/Damage.h", "DeathType"),
            ("Common/GameCommon.h", "Relationship"),
            ("Common/GameCommon.h", "VeterancyLevel"),
            ("Common/GameCommon.h", "ObjectShroudStatus"),
            ("Common/GameCommon.h", "WhichTurretType"),
            ("Common/Radar.h", "RadarPriorityType"),
            ("Common/GameType.h", "WeaponSlotType"),
            ("Common/GameType.h", "PathfindLayerEnum"),
            ("GameClient/Drawable.h", "TintStatus"),
            ("GameLogic/ObjectScriptStatusBits.h", "ObjectScriptStatusBit"),
            ("GameLogic/Object.h", "CrushSquishTestType"),
            ("GameClient/Eva.h", "EvaMessage"),
            ("Common/Radar.h", "RadarEventType"),
        ];
        let mut all = String::from("/* generated: the enums, verbatim from the headers */\n");
        for (path, name) in enums {
            let t = rd(&format!("Include/{path}"));
            let needle = format!("enum {name}");
            let mut hits = Vec::new();
            let mut from = 0;
            while let Some(p) = t[from..].find(&needle) {
                let at = from + p;
                from = at + needle.len();
                let line_start = t[..at].rfind('\n').map_or(0, |i| i + 1);
                if !t[line_start..at].chars().all(|c| c == ' ' || c == '\t') {
                    continue;
                }
                if t[from..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                if t[from..].trim_start().starts_with('{') {
                    hits.push(at);
                }
            }
            assert_eq!(hits.len(), 1, "enum {name} in {path} must be defined once");
            let end = t[hits[0]..].find("};").unwrap_or_else(|| panic!("end of enum {name}")) + hits[0];
            all.push_str(&format!("/* {path}: {name} */\n{}}};\n\n", &t[hits[0]..end]));
        }
        wr("enums.inc", all);

        // the weapon-set to model-condition map (WeaponSet.h) and initKindOfMasks (KindOf.cpp)
        let ws_h = rd("Include/GameLogic/WeaponSet.h");
        let a = once(&ws_h, "static const ModelConditionFlagType TheWeaponSetTypeToModelConditionTypeMap[WEAPONSET_COUNT] =");
        let b = ws_h[a..].find("\n};").expect("end of the map") + a;
        wr("weaponset_map.inc", format!("/* generated: from WeaponSet.h */\n{}\n", &ws_h[a..b + 3]));
        let ko_c = rd("Source/Common/System/KindOf.cpp");
        wr("kindof_masks.inc", format!("/* generated: from KindOf.cpp */\n{}", function_text(&ko_c, "void initKindOfMasks()")));
    }
    // --- end p1w6-object ---

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

        // --- p1w4-thingtemplate ---
        // ThingTemplate and ThingFactory (differential/wrap_thingtemplate.cpp): ThingTemplate.cpp,
        // ThingFactory.cpp and INIObject.cpp compiled UNCHANGED, with the
        // headers unchanged where they can be (ThingTemplate.h, ThingFactory.h, ProductionPrerequisite.h,
        // ModuleFactory.h, Module.h, ArmorSet.h, WeaponSet.h, Armor.h, DamageFX.h, Science.h, KindOf.h,
        // Geometry.h, Overridable.h, Powers.h, ThingSort.h, Shadow.h, Color.h, ...) and stand-ins in
        // differential/shim_thingtemplate for the client, layer 7 and layer 8 classes they name (Player,
        // Object, GameLogic, GameClient, the FXList and image stores, the audio event, GlobalData, ...).
        // thingtemplate_prelude.h (forced in) renames every class this archive defines or stands in for
        // (Tt*), so that no symbol of it clashes with another fixture's, and the INI block handlers and the
        // two INI field parsers it reaches (the real INI::load gets to the handlers through the hook in
        // wrap_ini.cpp, ref_object_block_hook).
        //
        // Pieces of upstream that cannot be compiled whole are cut out verbatim into OUT_DIR/tt_gen,
        // each marker asserted to occur exactly once:
        // - Common/Radar.h: RadarPriorityType and RadarPriorityNames (the real header needs the display);
        //   GameClient/Shadow.h: ShadowType and TheShadowNames (its inline code needs BaseType.h's x87 helpers).
        // - ini_field_parsers.inc: INI::parseDynamicAudioEventRTS and INI::parseWeaponTemplate from INI.cpp
        //   (renamed by the prelude, since the INI build's versions reach its own stand-ins of the audio
        //   event and the weapon store), and AI.h's TheCommandSourceMaskNames.
        // - weapon_template_set.inc: WeaponTemplateSet::clear to testWeaponSetFlag, from WeaponSet.cpp (the
        //   rest of that file is WeaponSet, which needs Object).
        // - module_factory_methods.inc: every member of ModuleFactory.cpp but init() (which names the 205
        //   module classes), so the real findModuleInterfaceMask, newModuleDataFromINI, addModuleInternal,
        //   findModuleTemplate and makeDecoratedNameKey run against the test modules the wrapper registers.
        // - production_prerequisite.inc: the members of ProductionPrerequisite.cpp that need no Player.
        // Linked ahead of everything it uses (the order of the compile calls is the link order): the geometry
        // build (GeometryInfo), upstream_gameengine (NameKeyGenerator, the random values), upstream_flags (the
        // bit name lists), upstream_armor (the stores), upstream_science, upstream_module (ModuleData,
        // Module::friend_newModuleData), upstream_xfer and upstream_ini. So this block sits before them, and
        // makes its own copy of SparseMatchFinder.h instead of using the BitFlags build's.
        {
            let shim_tt = dif.join("shim_thingtemplate");
            let tt_gen = out_dir.join("tt_gen");
            std::fs::create_dir_all(tt_gen.join("Common")).expect("create tt_gen");
            let read = |p: &str| std::fs::read_to_string(up.join(p)).expect("read upstream source");
            let once = |text: &str, what: &str, marker: &str| -> usize {
                assert_eq!(text.matches(marker).count(), 1, "{what}: {marker:?} must occur once");
                text.find(marker).expect("marker")
            };
            // the span from `first` to the end of `last` (the first occurrence of `last` after `first`)
            let span = |text: &str, what: &str, first: &str, last: &str| -> String {
                let a = once(text, what, first);
                let b = a + text[a..].find(last).unwrap_or_else(|| panic!("{what}: {last:?} after {first:?}")) + last.len();
                text[a..b].to_string()
            };
            // the function that starts at `first`: up to the first line that starts with `}` (the end of a
            // function, at column 0, as these files indent everything inside), that line included
            let function = |text: &str, what: &str, first: &str| -> String {
                let a = once(text, what, first);
                let close = a + text[a..].find("\n}").unwrap_or_else(|| panic!("{what}: the end of {first:?}")) + 1;
                let end = close + text[close..].find('\n').expect("end of line");
                text[a..end].to_string()
            };
            // Radar.h
            let radar = read("GameEngine/Include/Common/Radar.h");
            let region = span(&radar, "Radar.h", "enum RadarPriorityType\n{", "#endif  // DEFINE_RADAR_PRIOTITY_NAMES");
            std::fs::write(
                tt_gen.join("Common/Radar.h"),
                format!(
                    "/* generated by build.rs: RadarPriorityType and RadarPriorityNames, verbatim from Radar.h */\n\
                     #ifndef TT_GEN_RADAR_H\n#define TT_GEN_RADAR_H\n#include \"Lib/BaseType.h\"\n{region}\n#endif\n"
                ),
            )
            .expect("write Radar.h");
            // SparseMatchFinder.h with the two `typename`s MSVC did not require (the same two edits as the
            // BitFlags build's copy, which is made further down: this block comes first, see below)
            std::fs::write(
                tt_gen.join("Common/SparseMatchFinder.h"),
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
            // GameClient/Shadow.h (its inline code needs the x87 helpers of BaseType.h)
            let shadow = read("GameEngine/Include/GameClient/Shadow.h");
            let region = span(&shadow, "Shadow.h", "enum ShadowType\n{", "#endif  // end DEFINE_SHADOW_NAMES");
            std::fs::create_dir_all(tt_gen.join("GameClient")).expect("create tt_gen/GameClient");
            std::fs::write(
                tt_gen.join("GameClient/Shadow.h"),
                format!(
                    "/* generated by build.rs: ShadowType and TheShadowNames, verbatim from Shadow.h */\n\
                     #ifndef TT_GEN_SHADOW_H\n#define TT_GEN_SHADOW_H\n{region}\n#endif\n"
                ),
            )
            .expect("write Shadow.h");
            // INI.cpp's two field parsers, AI.h's command source names
            let ini_cpp = read("GameEngine/Source/Common/INI/INI.cpp");
            let audio = function(
                &ini_cpp,
                "INI.cpp",
                "void INI::parseDynamicAudioEventRTS( INI *ini, void * /*instance*/, void *store, const void* userData )\n{",
            );
            let weapon = function(
                &ini_cpp,
                "INI.cpp",
                "void INI::parseWeaponTemplate( INI* ini, void * /*instance*/, void *store, const void* /*userData*/ )\n{",
            );
            let ai_h = read("GameEngine/Include/GameLogic/AI.h");
            let sources = span(&ai_h, "AI.h", "static const char *TheCommandSourceMaskNames[] = \n{", "\n};\n");
            std::fs::write(
                tt_gen.join("ini_field_parsers.inc"),
                format!(
                    "/* generated by build.rs: verbatim from INI.cpp (the two parsers) and AI.h (the names) */\n{audio}\n{weapon}\n{sources}\n"
                ),
            )
            .expect("write ini_field_parsers.inc");
            // WeaponSet.cpp
            let weapon_set = read("GameEngine/Source/GameLogic/Object/WeaponSet.cpp");
            let a = once(&weapon_set, "WeaponSet.cpp", "void WeaponTemplateSet::clear()\n{");
            let end = "//-------------------------------------------------------------------------------------------------\nWeaponSet::WeaponSet()\n{";
            let b = once(&weapon_set, "WeaponSet.cpp", end);
            assert!(a < b, "WeaponSet.cpp: WeaponTemplateSet before WeaponSet");
            std::fs::write(
                tt_gen.join("weapon_template_set.inc"),
                format!("/* generated by build.rs: WeaponTemplateSet's members, verbatim from WeaponSet.cpp */\n{}\n", &weapon_set[a..b]),
            )
            .expect("write weapon_template_set.inc");
            // ModuleFactory.cpp: everything but init()
            let factory = read("GameEngine/Source/Common/Thing/ModuleFactory.cpp");
            let a = once(&factory, "ModuleFactory.cpp", "ModuleFactory::ModuleFactory( void )\n{");
            let init_first = "void ModuleFactory::init( void )\n{\n";
            let i = once(&factory, "ModuleFactory.cpp", init_first);
            let init_last = "}  // end init";
            let j = once(&factory, "ModuleFactory.cpp", init_last);
            assert!(a < i && i < j, "ModuleFactory.cpp: constructor, init");
            std::fs::write(
                tt_gen.join("module_factory_methods.inc"),
                format!(
                    "/* generated by build.rs: the members of ModuleFactory.cpp but init(), verbatim */\n{}\n{}\n",
                    &factory[a..i],
                    &factory[j + init_last.len()..]
                ),
            )
            .expect("write module_factory_methods.inc");

            // ProductionPrerequisite.cpp: the members that need no Player (the others need Player, Object
            // and the UnicodeString of the game text)
            let prereq = read("GameEngine/Source/Common/RTS/ProductionPrerequisite.cpp");
            let between = |first: &str, last: &str| -> &str {
                let a = once(&prereq, "ProductionPrerequisite.cpp", first);
                let b = a + prereq[a..].find(last).unwrap_or_else(|| panic!("ProductionPrerequisite.cpp: {last:?} after {first:?}"));
                assert_eq!(prereq.matches(last).count(), 1, "ProductionPrerequisite.cpp: {last:?} must occur once");
                &prereq[a..b]
            };
            std::fs::write(
                tt_gen.join("production_prerequisite.inc"),
                format!(
                    "/* generated by build.rs: the Player-free members of ProductionPrerequisite.cpp, verbatim */\n{}\n{}\n{}\n",
                    between("ProductionPrerequisite::ProductionPrerequisite()", "Int ProductionPrerequisite::calcNumPrereqUnitsOwned"),
                    between(
                        "Int ProductionPrerequisite::getAllPossibleBuildFacilityTemplates",
                        "const ThingTemplate *ProductionPrerequisite::getExistingBuildFacilityTemplate"
                    ),
                    between(
                        "void ProductionPrerequisite::addUnitPrereq( AsciiString unit, Bool orUnitWithPrevious )",
                        "UnicodeString ProductionPrerequisite::getRequiresList"
                    ),
                ),
            )
            .expect("write production_prerequisite.inc");

            let mut ttb = cc::Build::new();
            base(&mut ttb);
            ttb.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&shim_tt)
                .include(&tt_gen)
                .include(dif.join("shim_science"))
                .include(dif.join("shim_flags"))
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .include(&dif)
                .flag("-include")
                .flag(shim_tt.join("thingtemplate_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_thingtemplate.cpp"))
                .file(up.join("GameEngine/Source/Common/Thing/ThingTemplate.cpp"))
                .file(up.join("GameEngine/Source/Common/Thing/ThingFactory.cpp"))
                .file(up.join("GameEngine/Source/Common/INI/INIObject.cpp"));
            ttb.compile("upstream_thingtemplate");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/ThingTemplate.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/ThingFactory.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/ModuleFactory.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INIObject.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/ProductionPrerequisite.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/WeaponSet.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/AI.h",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp",
                "differential/wrap_thingtemplate.cpp",
                "differential/shim_thingtemplate",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w4-thingtemplate ---

        // --- p1w6-partition ---
        // PartitionManager (differential/wrap_partition.cpp): PartitionManager.cpp, SimpleObjectIterator.cpp
        // and Line2D.cpp are #included by the wrapper on differential/shim_partition (the stand-ins for the
        // object world, players, terrain, pathfinder, client and ghost objects; each header says what it
        // stands in for), with clang++ -fms-extensions as the other real-BaseType builds. The real
        // PartitionManager.h, BitFlags (KindOf, ObjectStatusTypes, DisabledTypes), GameCommon.h,
        // Geometry.h, ObjectIter.h, Snapshot.h, Xfer.h, DiscreteCircle.h and the WWMath Vector3 are used.
        // Compiled HERE, ahead of the libraries it calls (GeometryInfo in upstream_geometry; Cos, Sin and
        // DiscreteCircle in upstream_gameengine; the BitFlags names in upstream_flags; XferCRC in
        // upstream_xfer; the BaseType asm transcriptions in upstream_basetype; Vector3 in upstream_wwmath),
        // as the link order requires.
        //
        // Generated into OUT_DIR/partition_gen, every piece a verbatim cut or a mechanical edit of an
        // upstream file, each edit asserted to match the stated number of times:
        // - GameLogic/PartitionManager.h, a copy with two edits: (1) PartitionData::wasSeenByAnyPlayers
        //   uses its `for` variable after the loop (MSVC 6 leaked it), so `Int i;` moves before the loop;
        //   (2) the eight `friend void hLine...(...)` declarations are removed (MSVC made a friend
        //   declaration visible to ordinary lookup, clang does not; they only gave access to protected
        //   members, which the wrapper's `#define protected public` gives anyway);
        // - PartitionManager_decl.cpp, a copy of PartitionManager.cpp with one edit: eight `static void
        //   hLine...(Int, Int, Int, void *);` declarations after `static int cellValueProc(...);`, so the
        //   uses above the (static) definitions find them;
        // - save_code.h: `enum SaveCode` of GameState.h, verbatim (PartitionManager::xfer throws SC_INVALID_DATA);
        // - map_xy_factor.h: the `#define MAP_XY_FACTOR` line of MapObject.h, verbatim.
        {
            let engine = up.join("GameEngine");
            let gen = out_dir.join("partition_gen");
            std::fs::create_dir_all(gen.join("GameLogic")).expect("create partition_gen");
            let read = |p: std::path::PathBuf| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            let friends: Vec<(String, String, usize)> = [
                ("hLineAddLooker", "playerIndex"),
                ("hLineRemoveLooker", "playerIndex"),
                ("hLineAddShrouder", "playerIndex"),
                ("hLineRemoveShrouder", "playerIndex"),
                ("hLineAddThreat", "threatValueParms"),
                ("hLineRemoveThreat", "threatValueParms"),
                ("hLineAddValue", "threatValueParms"),
                ("hLineRemoveValue", "threatValueParms"),
            ]
            .iter()
            .map(|(f, p)| (format!("\tfriend void {f}(Int x1, Int x2, Int y, void *{p});\n"), String::new(), 1))
            .collect();
            let mut header_edits: Vec<(&str, &str, usize)> = vec![(
                "\t\tfor (Int i=0; i<MAX_PLAYER_COUNT; i++)\n\t\t\tif (m_everSeenByPlayer[i] && m_shroudedness[i] == OBJECTSHROUD_FOGGED)\n\t\t\t\treturn i;\n\t\treturn i;",
                "\t\tInt i;\n\t\tfor (i=0; i<MAX_PLAYER_COUNT; i++)\n\t\t\tif (m_everSeenByPlayer[i] && m_shroudedness[i] == OBJECTSHROUD_FOGGED)\n\t\t\t\treturn i;\n\t\treturn i;",
                1,
            )];
            for (a, b, n) in &friends {
                header_edits.push((a.as_str(), b.as_str(), *n));
            }
            std::fs::write(
                gen.join("GameLogic/PartitionManager.h"),
                patch_copy(&engine.join("Include/GameLogic/PartitionManager.h"), "PartitionManager.h", &header_edits),
            )
            .expect("write PartitionManager.h copy");
            let hline_names = ["hLineAddLooker", "hLineRemoveLooker", "hLineAddShrouder", "hLineRemoveShrouder", "hLineAddThreat", "hLineRemoveThreat", "hLineAddValue", "hLineRemoveValue"];
            let decls: String = hline_names.iter().map(|n| format!("static void {n}(Int x1, Int x2, Int y, void *parms);\n")).collect();
            let anchor_line = "static int cellValueProc(PartitionCell* cell, void* userData);\n";
            let with_decls = format!("{anchor_line}{decls}");
            std::fs::write(
                gen.join("PartitionManager_decl.cpp"),
                patch_copy(
                    &engine.join("Source/GameLogic/Object/PartitionManager.cpp"),
                    "PartitionManager.cpp",
                    &[(anchor_line, with_decls.as_str(), 1)],
                ),
            )
            .expect("write PartitionManager.cpp copy");
            let game_state = read(engine.join("Include/Common/GameState.h"));
            assert_eq!(game_state.matches("enum SaveCode").count(), 1, "GameState.h: enum SaveCode must occur once");
            let a = game_state.find("enum SaveCode").expect("SaveCode start");
            let rel = game_state[a..].find("\n};").expect("SaveCode end");
            std::fs::write(
                gen.join("save_code.h"),
                format!(
                    "/* generated by build.rs: verbatim from Common/GameState.h */\n#ifndef PM_GEN_SAVE_CODE_H\n#define PM_GEN_SAVE_CODE_H\n{}\n#endif\n",
                    &game_state[a..a + rel + 3]
                ),
            )
            .expect("write save_code.h");
            let map_object = read(engine.join("Include/Common/MapObject.h"));
            let factor: Vec<&str> = map_object.lines().filter(|l| l.starts_with("#define MAP_XY_FACTOR")).collect();
            assert_eq!(factor.len(), 1, "MapObject.h: exactly one MAP_XY_FACTOR line");
            std::fs::write(gen.join("map_xy_factor.h"), format!("/* generated by build.rs: verbatim from Common/MapObject.h */\n{}\n", factor[0])).expect("write map_xy_factor.h");

            let mut pt = cc::Build::new();
            base(&mut pt);
            // -ftrivial-auto-var-init=zero: upstream leaves some automatic variables unset on some
            // paths (the CollideLocAndNormal passed to onCollide when two nearly coincident boxes or
            // cylinders collide has no location or normal in x and y); clang fills them with zero, the
            // value the Rust gives, so that the defined part of the log can be compared.
            pt.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .flag("-ftrivial-auto-var-init=zero")
                .include(&gen)
                .include(dif.join("shim_partition"))
                .include(dif.join("shim_flags"))
                .include(dif.join("shim_ini"))
                .include(dif.join("shim_terrain"))
                .include(&shim)
                .include(engine.join("Include"))
                .include(up.join("Libraries/Include"))
                .include(&ww)
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .include(&dif)
                .file(dif.join("wrap_partition.cpp"));
            pt.compile("upstream_partition");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/PartitionManager.h",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SimpleObjectIterator.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameClient/Line2D.cpp",
                "differential/wrap_partition.cpp",
                "differential/shim_partition",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w6-partition ---
        // --- p1w6-energy ---
        // Energy.cpp, unchanged (differential/wrap_energy.cpp), on shim_energy: stand-ins for
        // Player (the brown-out notification), ThePlayerList, the template's energy numbers and
        // TheGameLogic->getFrame(). Linked before upstream_xfer, which it calls (as the geometry
        // group below: see the comment there).
        {
            let shim_energy = dif.join("shim_energy");
            let mut en = cc::Build::new();
            base(&mut en);
            en.include(&shim_energy)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_energy.cpp"));
            en.compile("upstream_energy");

            // ScoreKeeper.cpp, unchanged (differential/wrap_score_keeper.cpp), on shim_score:
            // stand-ins for Player, ThingTemplate, ThingFactory, Object and TheGameLogic.
            let shim_score = dif.join("shim_score");
            let mut sk = cc::Build::new();
            base(&mut sk);
            sk.include(&shim_score)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_score_keeper.cpp"));
            sk.compile("upstream_score_keeper");

            // MissionStats.cpp, unchanged (differential/wrap_mission_stats.cpp), on shim_energy
            // (its Player.h is the stand-in; the file never calls a player).
            let mut ms = cc::Build::new();
            base(&mut ms);
            ms.include(&shim_energy)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_mission_stats.cpp"));
            ms.compile("upstream_mission_stats");

            // AcademyStats.cpp, unchanged (differential/wrap_academy_stats.cpp), on shim_academy:
            // stand-ins for the player and its neighbours, the templates, the client text and the
            // global data. Its Energy.h is a stand-in (the real Energy is the energy wrapper's).
            let shim_academy = dif.join("shim_academy");
            let mut ac = cc::Build::new();
            base(&mut ac);
            ac.include(&shim_academy)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_academy_stats.cpp"));
            ac.compile("upstream_academy_stats");
        }
        // --- end p1w6-energy ---
        // --- p1w7-sonnet ---
        // TunnelTracker.cpp and CaveSystem.cpp, unchanged (differential/wrap_tunnel_tracker.cpp), on
        // shim_tunnel: stand-ins for the objects of a test world, TheGameLogic, the partition manager, the
        // pathfinder, the drawable and the body module. Linked before upstream_xfer, which they call.
        {
            let shim_tunnel = dif.join("shim_tunnel");
            let mut tt = cc::Build::new();
            base(&mut tt);
            tt.include(&shim_tunnel)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_tunnel_tracker.cpp"));
            tt.compile("upstream_tunnel_tracker");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/TunnelTracker.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/System/CaveSystem.cpp",
                "differential/wrap_tunnel_tracker.cpp",
                "differential/shim_tunnel",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // ResourceGatheringManager.cpp, unchanged (differential/wrap_resource_gathering.cpp), on shim_resource:
        // stand-ins for the objects of a test world (AI, supply-truck interface, dock interface, modules by
        // name), TheGameLogic, ActionManager::canTransferSuppliesAt and the partition manager's distance.
        {
            let shim_resource = dif.join("shim_resource");
            let mut rg = cc::Build::new();
            base(&mut rg);
            rg.include(&shim_resource)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_resource_gathering.cpp"));
            rg.compile("upstream_resource_gathering");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/ResourceGatheringManager.cpp",
                "differential/wrap_resource_gathering.cpp",
                "differential/shim_resource",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w7-sonnet ---
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
        // --- p1w3-thing ---
        // Thing and ObjectIter (differential/wrap_thing.cpp,
        // wrap_object_iter.cpp): Coord3D is copied VERBATIM from the real
        // Libraries/Include/Lib/BaseType.h (its struct, located by its first
        // line and the first "};" after it, asserted once) into
        // OUT_DIR/thing_gen/basetype_coord3d.h, which shim_thing/Lib/BaseType.h
        // includes: Thing.cpp calls Coord3D::crossProduct, which the root
        // shim's Coord3D does not have.
        let thing_gen = out_dir.join("thing_gen");
        std::fs::create_dir_all(&thing_gen).expect("create thing_gen");
        let coord3d = {
            let first = "struct Coord3D \n{";
            assert_eq!(basetype.matches(first).count(), 1, "Coord3D must occur once in BaseType.h");
            let a = basetype.find(first).expect("Coord3D start");
            let rel = basetype[a..].find("\n};\n").expect("Coord3D end");
            basetype[a..a + rel + "\n};\n".len()].to_string()
        };
        std::fs::write(
            thing_gen.join("basetype_coord3d.h"),
            format!("/* generated by build.rs: verbatim from Libraries/Include/Lib/BaseType.h */\n{coord3d}"),
        )
        .expect("write basetype_coord3d.h");
        // Thing (differential/wrap_thing.cpp): Thing.cpp unchanged (included
        // by the wrapper), on
        // differential/shim_thing (the ThingTemplate, GlobalData, TerrainLogic
        // and Object stand-ins; the real KindOfType enum, copied verbatim).
        // Linked ahead of the libraries that define what it calls (Cos and
        // Sin in upstream_gameengine, normalizeAngle in upstream_ini_wrap and
        // the Matrix3D methods in upstream_wwmath), as the other fixtures are.
        let shim_thing = dif.join("shim_thing");
        let mut th = cc::Build::new();
        base(&mut th);
        th.include(&shim_thing)
            .include(&thing_gen)
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .include(ww.join("WWMath"))
            .include(ww.join("WWLib"))
            .include(&dif)
            .file(dif.join("wrap_thing.cpp"));
        th.compile("upstream_thing");
        // SimpleObjectIterator.cpp unchanged (included by
        // differential/wrap_object_iter.cpp), on the same stand-ins for
        // ThingTemplate and Object. It calls no other library.
        let mut oi = cc::Build::new();
        base(&mut oi);
        oi.include(&shim_thing)
            .include(&thing_gen)
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .include(ww.join("WWMath"))
            .include(ww.join("WWLib"))
            .include(&dif)
            .file(dif.join("wrap_object_iter.cpp"));
        oi.compile("upstream_object_iter");
        for p in [
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SimpleObjectIterator.cpp",
            "differential/wrap_thing.cpp",
            "differential/wrap_object_iter.cpp",
            "differential/shim_thing",
        ] {
            println!("cargo:rerun-if-changed={p}");
        }
        // --- end p1w3-thing ---
        // --- p1w6-object ---
        // Object (differential/wrap_object.cpp): the text of Object.cpp and of the class Object, compiled with
        // the stand-ins of differential/shim_object. Upstream is never edited; the text is cut in memory and
        // written to OUT_DIR/object_gen, every marker asserted to occur exactly once:
        // - Object.cpp: the includes; the weapon-flag table and the `addIcon` declaration (client types); the
        //   region of Weapon forwarders (reloadAllAmmo .. setModelConditionState), the weapon getters (getCurrentWeapon ..
        //   getRelationship), adjustModelConditionForWeaponStatus (the weapon-status model conditions),
        //   updateObjValuesFromMapProperties, the doCommandButton* .. clearLeechRangeModeForAllWeapons region
        //   (GUI command buttons), getNumConsecutiveShotsFiredAtTarget .. getCommandSetString (bone positions) and
        //   canProduceUpgrade; and one edit, `addWorldAnimation( animTemplate,` becomes
        //   `addWorldAnimation( this, animTemplate,` (the stand-in names the object in its record).
        // - Object.h: the class body. Thing.h/Thing.cpp, ExperienceTracker.h/.cpp, Geometry.h and the seven functions of
        //   Geometry.cpp that Object reaches, the enums, Coord3D/ICoord3D/Region3D (BaseType.h), the weapon-set
        //   model-condition map and initKindOfMasks: verbatim.
        {
            let obj_gen = out_dir.join("object_gen");
            std::fs::create_dir_all(&obj_gen).expect("create object_gen");
            generate_object_includes(&up.join("GameEngine"), &obj_gen);
            let shim_obj = dif.join("shim_object");
            let mut ob = cc::Build::new();
            base(&mut ob);
            ob.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&obj_gen)
                .include(&dif)
                .flag("-include")
                .flag(shim_obj.join("object_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_object.cpp"));
            ob.compile("upstream_object");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/Object.h",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ExperienceTracker.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Geometry.cpp",
                "differential/wrap_object.cpp",
                "differential/shim_object",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w6-object ---
        // --- p1w4-terrain ---
        // TerrainLogic (differential/wrap_terrain_logic.cpp): TerrainLogic.cpp is
        // #included by the wrapper UNCHANGED, on differential/shim_terrain_logic (the
        // stand-ins for the object world, pathfinder, partition manager, polygon
        // triggers, map objects, global data and terrain visual; each header says what
        // it stands in for) ahead of shim_terrain (the real BaseType.h, GameMemory),
        // shim_chunk (the Dict the real DataChunk.h includes) and the shared shim, with
        // clang++ -fms-extensions as the other real-BaseType builds. Compiled HERE,
        // ahead of the libraries it calls, which come later in the link line: Cos and
        // Sin (upstream_gameengine), the chunk reader and name keys (upstream_chunk),
        // the Xfer base (upstream_xfer), Matrix3D (upstream_wwmath), the BaseType
        // asm transcriptions (upstream_basetype) and AsciiString::TheEmptyString
        // (upstream_big).
        //
        // Generated into OUT_DIR/terrain_logic_gen, every piece a verbatim cut of an
        // upstream file, its first line asserted to occur once:
        // - terrain_logic_gen_defs.h: the enums GeometryType (Geometry.h), SaveCode
        //   (GameState.h), DistanceCalculationType (PartitionManager.h),
        //   BodyDamageType (BodyModule.h), DamageType and DeathType (Damage.h),
        //   BridgeTowerType (TerrainRoads.h), each from its `enum` line to the first
        //   `};`; and the lines HUGE_DAMAGE_AMOUNT (Damage.h), MAP_XY_FACTOR and
        //   MAP_HEIGHT_SCALE (MapObject.h), LAYER_Z_CLOSE_ENOUGH_F, PATHFIND_CELL_SIZE
        //   and PATHFIND_CELL_SIZE_F (AIPathfind.h), WATER_GRID (Scripts.h), and the
        //   enum holding LOGICFRAMES_PER_SECOND (GameCommon.h);
        // - w3d_terrain_logic_methods.inc: W3DTerrainLogic::getExtent through
        //   getLayerHeight (W3DTerrainLogic.cpp), the device subclass' overrides;
        // - height_map_height.inc: BaseHeightMapRenderObjClass::getHeightMapHeight
        //   (BaseHeightMap.cpp) and clip_height.inc: its getClipHeight (BaseHeightMap.h);
        // - polygon_trigger_methods.inc: PolygonTrigger::updateBounds and pointInTrigger;
        // - geometry_set.inc: GeometryInfo::set (Geometry.cpp), the stand-in's constructor.
        // The one EDIT, to a copy of TerrainLogic.cpp (TerrainLogic_nostatic.cpp, which the
        // wrapper includes): `static const ThingTemplate* genericBridgeTemplate` loses its
        // `static`, asserted to occur once; see the header of the wrapper for why.
        {
            let tl_shim = dif.join("shim_terrain_logic");
            let engine = up.join("GameEngine");
            let device = up.join("GameEngineDevice");
            let gen = out_dir.join("terrain_logic_gen");
            std::fs::create_dir_all(&gen).expect("create terrain_logic_gen");
            let read = |p: std::path::PathBuf| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            // from `first` (which must occur once) to the end of `last` after it
            let seg = |text: &str, what: &str, first: &str, last: &str| -> String {
                assert_eq!(text.matches(first).count(), 1, "{what}: {first:?} must occur once");
                let a = text.find(first).expect("segment start");
                let rel = text[a..].find(last).unwrap_or_else(|| panic!("{what}: {last:?} must follow {first:?}"));
                text[a..a + rel + last.len()].to_string()
            };
            let enum_of = |text: &str, what: &str, first: &str| seg(text, what, first, "\n};");
            let line_of = |text: &str, what: &str, prefix: &str| -> String {
                let lines: Vec<&str> = text.lines().filter(|l| l.starts_with(prefix)).collect();
                assert_eq!(lines.len(), 1, "{what}: exactly one line must start with {prefix:?}");
                lines[0].to_string()
            };
            let geometry = read(engine.join("Include/Common/Geometry.h"));
            let game_state = read(engine.join("Include/Common/GameState.h"));
            let partition = read(engine.join("Include/GameLogic/PartitionManager.h"));
            let body = read(engine.join("Include/GameLogic/Module/BodyModule.h"));
            let damage = read(engine.join("Include/GameLogic/Damage.h"));
            let roads = read(engine.join("Include/GameClient/TerrainRoads.h"));
            let map_object = read(engine.join("Include/Common/MapObject.h"));
            let pathfind = read(engine.join("Include/GameLogic/AIPathfind.h"));
            let scripts = read(engine.join("Include/GameLogic/Scripts.h"));
            let game_common = read(engine.join("Include/Common/GameCommon.h"));
            let defs = [
                enum_of(&geometry, "Geometry.h", "enum GeometryType"),
                enum_of(&game_state, "GameState.h", "enum SaveCode"),
                enum_of(&partition, "PartitionManager.h", "enum DistanceCalculationType"),
                enum_of(&body, "BodyModule.h", "enum BodyDamageType"),
                enum_of(&damage, "Damage.h", "enum DamageType"),
                enum_of(&damage, "Damage.h", "enum DeathType"),
                line_of(&damage, "Damage.h", "const Real HUGE_DAMAGE_AMOUNT"),
                enum_of(&roads, "TerrainRoads.h", "enum BridgeTowerType"),
                line_of(&map_object, "MapObject.h", "#define MAP_XY_FACTOR"),
                line_of(&map_object, "MapObject.h", "#define MAP_HEIGHT_SCALE"),
                line_of(&pathfind, "AIPathfind.h", "#define LAYER_Z_CLOSE_ENOUGH_F"),
                line_of(&pathfind, "AIPathfind.h", "#define PATHFIND_CELL_SIZE\t"),
                line_of(&pathfind, "AIPathfind.h", "#define PATHFIND_CELL_SIZE_F"),
                line_of(&scripts, "Scripts.h", "#define WATER_GRID "),
                seg(&game_common, "GameCommon.h", "enum\n{\n\tLOGICFRAMES_PER_SECOND = 30,", "MSEC_PER_SECOND = 1000\n};"),
            ];
            std::fs::write(
                gen.join("terrain_logic_gen_defs.h"),
                format!(
                    "/* generated by build.rs: verbatim from the upstream headers */\n#ifndef TL_GEN_DEFS_H\n#define TL_GEN_DEFS_H\n{}\n#endif\n",
                    defs.join("\n")
                ),
            )
            .expect("write terrain_logic_gen_defs.h");
            let w3d = read(device.join("Source/W3DDevice/GameLogic/W3DTerrainLogic.cpp"));
            std::fs::write(
                gen.join("w3d_terrain_logic_methods.inc"),
                format!(
                    "/* generated by build.rs: verbatim from W3DTerrainLogic.cpp (getExtent .. getLayerHeight) */\n{}",
                    seg(
                        &w3d,
                        "W3DTerrainLogic.cpp",
                        "//-------------------------------------------------------------------------------------------------\n/** Get the 3D extent of the terrain in world coordinates */",
                        "#endif\n}  // end getLayerHeight\n"
                    )
                ),
            )
            .expect("write w3d_terrain_logic_methods.inc");
            let height_map = read(device.join("Source/W3DDevice/GameClient/BaseHeightMap.cpp"));
            std::fs::write(
                gen.join("height_map_height.inc"),
                format!(
                    "/* generated by build.rs: verbatim from BaseHeightMap.cpp */\n{}",
                    seg(
                        &height_map,
                        "BaseHeightMap.cpp",
                        "Real BaseHeightMapRenderObjClass::getHeightMapHeight(Real x, Real y, Coord3D* normal) const\n{",
                        "\n\treturn height;\n}\n"
                    )
                ),
            )
            .expect("write height_map_height.inc");
            let height_map_h = read(device.join("Include/W3DDevice/GameClient/BaseHeightMap.h"));
            std::fs::write(
                gen.join("clip_height.inc"),
                format!(
                    "/* generated by build.rs: verbatim from BaseHeightMap.h */\n{}",
                    seg(&height_map_h, "BaseHeightMap.h", "\tinline UnsignedByte getClipHeight(Int x, Int y) const\n\t{", "\n\t}\n")
                ),
            )
            .expect("write clip_height.inc");
            let polygon = read(engine.join("Source/GameLogic/Map/PolygonTrigger.cpp"));
            std::fs::write(
                gen.join("polygon_trigger_methods.inc"),
                format!(
                    "/* generated by build.rs: verbatim from PolygonTrigger.cpp */\n{}\n{}",
                    seg(&polygon, "PolygonTrigger.cpp", "void PolygonTrigger::updateBounds(void)\tconst\n{", "\n}\n"),
                    seg(&polygon, "PolygonTrigger.cpp", "Bool PolygonTrigger::pointInTrigger(ICoord3D &point) const\n{", "\treturn inside;\n}\n"),
                ),
            )
            .expect("write polygon_trigger_methods.inc");

            let geometry_cpp = read(engine.join("Source/Common/System/Geometry.cpp"));
            std::fs::write(
                gen.join("geometry_set.inc"),
                format!(
                    "/* generated by build.rs: verbatim from Geometry.cpp */\n{}",
                    seg(
                        &geometry_cpp,
                        "Geometry.cpp",
                        "void GeometryInfo::set(GeometryType type, Bool isSmall, Real height, Real majorRadius, Real minorRadius)\n{",
                        "\n}\n"
                    )
                ),
            )
            .expect("write geometry_set.inc");
            let terrain_logic = read(engine.join("Source/GameLogic/Map/TerrainLogic.cpp"));
            let static_line = "static const ThingTemplate* genericBridgeTemplate = TheThingFactory->findTemplate(\"GenericBridge\");";
            assert_eq!(terrain_logic.matches(static_line).count(), 1, "TerrainLogic.cpp: the genericBridgeTemplate static must occur once");
            std::fs::write(
                gen.join("TerrainLogic_nostatic.cpp"),
                terrain_logic.replace(static_line, "const ThingTemplate* genericBridgeTemplate = TheThingFactory->findTemplate(\"GenericBridge\");"),
            )
            .expect("write TerrainLogic_nostatic.cpp");

            let mut tl = cc::Build::new();
            base(&mut tl);
            tl.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&gen)
                .include(&tl_shim)
                .include(dif.join("shim_terrain"))
                .include(dif.join("shim_chunk"))
                .include(&shim)
                .include(engine.join("Include"))
                .include(up.join("Libraries/Include"))
                .include(up.join("Libraries/Source/Compression"))
                .include(&ww)
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .include(&dif)
                .file(dif.join("wrap_terrain_logic.cpp"));
            tl.compile("upstream_terrain_logic");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Map/TerrainLogic.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/TerrainLogic.h",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Map/PolygonTrigger.cpp",
                "upstream/GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameLogic/W3DTerrainLogic.cpp",
                "upstream/GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/BaseHeightMap.cpp",
                "upstream/GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/BaseHeightMap.h",
                "differential/wrap_terrain_logic.cpp",
                "differential/shim_terrain_logic",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w4-terrain ---
        // --- p1w6-locomotor ---
        // Locomotor and LocomotorSet (differential/wrap_locomotor.cpp): Locomotor.cpp is
        // #included by the wrapper UNCHANGED, with the real Locomotor.h, LocomotorSet.h,
        // INI.h, NameKeyGenerator.h, Override.h, Snapshot.h, the Xfer headers and BaseType.h, on
        // differential/shim_locomotor (the stand-ins for the object, its physics behaviour and
        // body module, the terrain, the partition manager, the pathfinder, the global data and
        // the game logic; locomotor_world.h says what each stands in for) ahead of shim_terrain
        // (GameMemory, SubsystemInterface, STLTypedefs for the real BaseType.h) and the shared
        // shim, with clang++ -fms-extensions as the other real-BaseType builds.
        // Compiled HERE, ahead of the libraries it calls, which come later in the link line:
        // INI (upstream_ini), name keys (upstream_chunk), Xfer (upstream_xfer), Matrix3D
        // (upstream_wwmath), Cos/Sin/ACos and the logic random values (upstream_gameengine),
        // normalizeAngle (upstream_ini_wrap). Its INI::parseLocomotorTemplateDefinition is the
        // real one; the weak STUB of wrap_ini.cpp gives way to it.
        //
        // Generated into OUT_DIR/loco_gen, every piece a verbatim cut of an upstream file, its
        // first marker asserted to occur once:
        // - locomotor_gen_defs.h: the enums ModelConditionFlagType (ModelState.h), KindOfType
        //   (KindOf.h), ObjectStatusTypes (ObjectStatusTypes.h), DisabledType
        //   (DisabledTypes.h), BodyDamageType (BodyModule.h), PhysicsTurningType
        //   (PhysicsUpdate.h), each from its `enum` line to the first `};`; and the lines
        //   IS_CONDITION_WORSE and IS_CONDITION_BETTER (BodyModule.h), PATHFIND_CELL_SIZE and
        //   PATHFIND_CELL_SIZE_F (AIPathfind.h);
        // - thing_methods.inc: Thing::getUnitDirectionVector2D .. Thing::setTransformMatrix
        //   (Thing.cpp), and thing_heights.inc: Thing::calculateHeightAboveTerrain ..
        //   getHeightAboveTerrainOrWater, with the ONE mechanical edit that `Thing::` reads
        //   `Object::` (the stand-in Object of locomotor_world.h is what the Locomotor drives;
        //   the edit is asserted to apply and to leave no `Thing::` behind).
        {
            let engine = up.join("GameEngine");
            let gen = out_dir.join("loco_gen");
            std::fs::create_dir_all(&gen).expect("create loco_gen");
            let read = |p: std::path::PathBuf| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            // from `first` (which must occur once) to the end of `last` after it
            let seg = |text: &str, what: &str, first: &str, last: &str| -> String {
                assert_eq!(text.matches(first).count(), 1, "{what}: {first:?} must occur once");
                let a = text.find(first).expect("segment start");
                let rel = text[a..].find(last).unwrap_or_else(|| panic!("{what}: {last:?} must follow {first:?}"));
                text[a..a + rel + last.len()].to_string()
            };
            // from `first` (which must occur once) to just before `last`
            let seg_before = |text: &str, what: &str, first: &str, last: &str| -> String {
                assert_eq!(text.matches(first).count(), 1, "{what}: {first:?} must occur once");
                let a = text.find(first).expect("segment start");
                let rel = text[a..].find(last).unwrap_or_else(|| panic!("{what}: {last:?} must follow {first:?}"));
                text[a..a + rel].to_string()
            };
            let enum_of = |text: &str, what: &str, first: &str| seg(text, what, first, "\n};");
            let line_of = |text: &str, what: &str, prefix: &str| -> String {
                let lines: Vec<&str> = text.lines().filter(|l| l.starts_with(prefix)).collect();
                assert_eq!(lines.len(), 1, "{what}: exactly one line must start with {prefix:?}");
                lines[0].to_string()
            };
            let model_state = read(engine.join("Include/Common/ModelState.h"));
            let kind_of = read(engine.join("Include/Common/KindOf.h"));
            let status = read(engine.join("Include/Common/ObjectStatusTypes.h"));
            let disabled = read(engine.join("Include/Common/DisabledTypes.h"));
            let body = read(engine.join("Include/GameLogic/Module/BodyModule.h"));
            let physics = read(engine.join("Include/GameLogic/Module/PhysicsUpdate.h"));
            let pathfind = read(engine.join("Include/GameLogic/AIPathfind.h"));
            let defs = [
                enum_of(&model_state, "ModelState.h", "enum ModelConditionFlagType"),
                enum_of(&kind_of, "KindOf.h", "enum KindOfType"),
                enum_of(&status, "ObjectStatusTypes.h", "enum ObjectStatusTypes"),
                enum_of(&disabled, "DisabledTypes.h", "enum DisabledType"),
                enum_of(&body, "BodyModule.h", "enum BodyDamageType"),
                line_of(&body, "BodyModule.h", "#define IS_CONDITION_WORSE"),
                line_of(&body, "BodyModule.h", "#define IS_CONDITION_BETTER"),
                enum_of(&physics, "PhysicsUpdate.h", "enum PhysicsTurningType"),
                line_of(&pathfind, "AIPathfind.h", "#define PATHFIND_CELL_SIZE\t"),
                line_of(&pathfind, "AIPathfind.h", "#define PATHFIND_CELL_SIZE_F"),
            ];
            std::fs::write(
                gen.join("locomotor_gen_defs.h"),
                format!("/* generated by build.rs: verbatim from the upstream headers */\n#ifndef LOCO_GEN_DEFS_H\n#define LOCO_GEN_DEFS_H\n{}\n#endif\n", defs.join("\n")),
            )
            .expect("write locomotor_gen_defs.h");
            let thing = read(engine.join("Source/Common/Thing/Thing.cpp"));
            let as_object = |what: &str, text: String| -> String {
                assert!(text.contains("Thing::"), "{what}: the cut must name Thing::");
                let edited = text.replace("Thing::", "Object::");
                assert!(!edited.contains("Thing::"), "{what}: no Thing:: may remain");
                edited
            };
            let methods = seg_before(
                &thing,
                "Thing.cpp",
                "const Coord3D* Thing::getUnitDirectionVector2D() const",
                "\n//-------------------------------------------------------------------------------------------------\nBool Thing::isKindOf(KindOfType t) const",
            );
            std::fs::write(
                gen.join("thing_methods.inc"),
                format!("/* generated by build.rs: Thing.cpp getUnitDirectionVector2D .. setTransformMatrix, `Thing::` read as `Object::` */\n{}", as_object("thing_methods", methods)),
            )
            .expect("write thing_methods.inc");
            let heights = seg_before(
                &thing,
                "Thing.cpp",
                "Real Thing::calculateHeightAboveTerrain() const",
                "\n//=============================================================================\n/** If we treat this as airborne",
            );
            std::fs::write(
                gen.join("thing_heights.inc"),
                format!("/* generated by build.rs: Thing.cpp calculateHeightAboveTerrain .. getHeightAboveTerrainOrWater, `Thing::` read as `Object::` */\n{}", as_object("thing_heights", heights)),
            )
            .expect("write thing_heights.inc");

            let mut lo = cc::Build::new();
            base(&mut lo);
            lo.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&gen)
                .include(dif.join("shim_locomotor"))
                .include(dif.join("shim_terrain"))
                .include(&shim)
                .include(engine.join("Include"))
                .include(up.join("Libraries/Include"))
                .include(up.join("Libraries/Source/Compression"))
                .include(&ww)
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .include(&dif)
                .file(dif.join("wrap_locomotor.cpp"));
            lo.compile("upstream_locomotor");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Locomotor.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/Locomotor.h",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/LocomotorSet.h",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp",
                "differential/wrap_locomotor.cpp",
                "differential/shim_locomotor",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w6-locomotor ---

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
        // --- p1w7-sonnet ---
        // WeaponTemplate and WeaponStore (differential/wrap_weapon.cpp): Weapon.cpp with the regions that need an
        // Object, the partition manager, the drawable or the damage application cut out, in memory, into
        // OUT_DIR/weapon_gen/Weapon_cut.cpp (upstream is never edited; every marker asserted to occur once):
        // - the includes (replaced by the ones the kept code needs) and the two DistanceCalculationType constants;
        // - estimateWeaponTemplateDamage, shouldProjectileCollideWith, trimOldHistoricDamage, dealDamageInternal and
        //   fireWeaponTemplate (up to WeaponStore::WeaponStore());
        // - handleProjectileDetonation and the two createAndFireTempWeapon (up to findWeaponTemplate);
        // - WeaponStore::update (up to deleteAllDelayedDamage);
        // - the class Weapon, from its constructor to the WeaponBonus code.
        // Built as the armor build is (clang++ -fms-extensions, the real BitFlagsIO.h through flags_prelude.h), with
        // differential/shim_weapon ahead of shim_armor, and linked ahead of upstream_ini and upstream_xfer.
        {
            fn once_at(text: &str, marker: &str) -> usize {
                assert_eq!(text.matches(marker).count(), 1, "{marker:?} must occur once");
                text.find(marker).expect("marker")
            }
            fn cut_between(text: &str, start: &str, end: &str) -> String {
                let (a, b) = (once_at(text, start), once_at(text, end));
                assert!(a < b, "{start:?} must come before {end:?}");
                format!("{}\n{}", &text[..a], &text[b..])
            }
            let weapon_gen = out_dir.join("weapon_gen");
            std::fs::create_dir_all(&weapon_gen).expect("create weapon_gen");
            // Coord3D verbatim from the real BaseType.h (shim_weapon/Lib/BaseType.h includes it), as the Thing build does
            std::fs::write(
                weapon_gen.join("basetype_coord3d.h"),
                format!("/* generated by build.rs: verbatim from Libraries/Include/Lib/BaseType.h */\n{coord3d}"),
            )
            .expect("write basetype_coord3d.h");
            let mut src = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/Weapon.cpp")).expect("read Weapon.cpp");
            // the includes become the ones the stand-ins need (fire_standins.h is included by the wrapper before this text);
            // PATHFIND_CELL_SIZE_F is AIPathfind.h's. Everything else of Weapon.cpp is kept, unchanged.
            let (a, b) = (once_at(&src, "#include \"Common/CRC.h\""), once_at(&src, "#define RATIONALIZE_ATTACK_RANGE"));
            assert!(a < b);
            src = format!(
                "{}#include \"Common/INI.h\"\n#include \"Common/NameKeyGenerator.h\"\n#include \"Common/ThingFactory.h\"\n#include \"GameLogic/ObjectCreationList.h\"\n#include \"GameLogic/Weapon.h\"\n#define PATHFIND_CELL_SIZE_F 10.0f\n\n{}",
                &src[..a],
                &src[b..]
            );
            // the one edit of the text: WeaponStore::update has `const isProjectileDetonation = false;`, an implicit int that only
            // MSVC accepts; it gets its type (`Bool`, which the parameter it is passed as has)
            let marker = "const isProjectileDetonation = false;";
            once_at(&src, marker);
            src = src.replace(marker, "const Bool isProjectileDetonation = false;");
            std::fs::write(weapon_gen.join("Weapon_cut.cpp"), &src).expect("write Weapon_cut.cpp");
            // WeaponSet.cpp: the includes, the bit-name list of WeaponSetFlags (another fixture defines it) and the four INI
            // parsers of WeaponTemplateSet (the sets are built without INI here) are cut; the rest is unchanged.
            let mut ws = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/WeaponSet.cpp")).expect("read WeaponSet.cpp");
            let (a, b) = (once_at(&ws, "#include \"PreRTS.h\""), once_at(&ws, "#include \"GameLogic/Weapon.h\""));
            assert!(a < b);
            ws = format!("{}#include \"GameLogic/WeaponSet.h\"\n{}", &ws[..a], &ws[b..]);
            ws = cut_between(&ws, "const char* WeaponSetFlags::s_bitNameList[] = ", "void WeaponTemplateSet::clear()");
            ws = cut_between(&ws, "void WeaponTemplateSet::parseWeapon(INI* ini", "Bool WeaponTemplateSet::testWeaponSetFlag(");
            std::fs::write(weapon_gen.join("WeaponSet_cut.cpp"), ws).expect("write WeaponSet_cut.cpp");
            // Object.cpp: the weapon forwarders and the two model-condition functions (Object_weapons.cpp), each function
            // taken whole, unchanged, from its signature line to its closing brace (every signature asserted to occur once),
            // with the static array they share. The rest of Object.cpp is the Object differential test's (wrap_object.cpp).
            let oc = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/Object.cpp")).expect("read Object.cpp");
            fn extract(text: &str, header: &str) -> String {
                assert_eq!(text.matches(header).count(), 1, "{header:?} must occur once");
                let a = text.find(header).expect("header");
                let open = a + text[a..].find('{').expect("open brace");
                let mut depth = 0usize;
                for (i, c) in text[open..].char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                return format!("{}\n\n", &text[a..open + i + 1]);
                            }
                        }
                        _ => {}
                    }
                }
                panic!("{header:?} is not closed");
            }
            let mut ow = String::from("/* generated by build.rs: functions of Object.cpp, unchanged */\n");
            let a = once_at(&oc, "static const ModelConditionFlags s_allWeaponFireFlags[WEAPONSLOT_COUNT] =");
            let b = a + oc[a..].find("\n};").expect("end of the array") + 4;
            ow.push_str(&oc[a..b]);
            ow.push_str("\n\n");
            for h in [
                "void Object::reloadAllAmmo(Bool now)",
                "Bool Object::isOutOfAmmo() const",
                "Bool Object::hasAnyWeapon() const",
                "Bool Object::hasAnyDamageWeapon() const",
                "UnsignedInt Object::getMostPercentReadyToFireAnyWeapon() const",
                "Bool Object::hasWeaponToDealDamageType(DamageType typeToDeal) const",
                "Real Object::getLargestWeaponRange() const",
                "void Object::setFiringConditionForCurrentWeapon() const",
                "Weapon* Object::getCurrentWeapon(WeaponSlotType* wslot)\n",
                "const Weapon* Object::getCurrentWeapon(WeaponSlotType* wslot) const",
                "Weapon* Object::findWaypointFollowingCapableWeapon()",
                "Bool Object::getAmmoPipShowingInfo(Int& numTotal, Int& numFull) const",
                "CanAttackResult Object::getAbleToAttackSpecificObject(",
                "CanAttackResult Object::getAbleToUseWeaponAgainstTarget(",
                "Bool Object::chooseBestWeaponForTarget(",
                "void Object::fireCurrentWeapon(Object *target)",
                "void Object::fireCurrentWeapon(const Coord3D* pos)",
                "void Object::notifyFiringTrackerShotFired(",
                "void Object::preFireCurrentWeapon(",
                "UnsignedInt Object::getLastShotFiredFrame() const",
                "void Object::adjustModelConditionForWeaponStatus()",
                "void Object::clearLeechRangeModeForAllWeapons()",
                "Bool Object::getSingleLogicalBonePosition(const char* boneName, Coord3D* position, Matrix3D* transform) const",
                "Bool Object::getSingleLogicalBonePositionOnTurret(",
                "Int Object::getMultiLogicalBonePosition(",
                "const AsciiString& Object::getCommandSetString() const",
                "Bool Object::canProduceUpgrade(",
            ] {
                ow.push_str(&extract(&oc, h));
            }
            std::fs::write(weapon_gen.join("Object_weapons.cpp"), ow).expect("write Object_weapons.cpp");
            // ActionManager.cpp: only its include block is replaced (the stand-ins and the real enums are in
            // wrap_weapon.cpp and fire_standins.h); everything else is unchanged.
            let mut am = std::fs::read_to_string(up.join("GameEngine/Source/Common/RTS/ActionManager.cpp")).expect("read ActionManager.cpp");
            let (a, b) = (once_at(&am, "#include \"PreRTS.h\""), once_at(&am, "#ifdef _INTERNAL"));
            assert!(a < b);
            am = format!("{}#include \"Common/ActionManager.h\"\n\n{}", &am[..a], &am[b..]);
            std::fs::write(weapon_gen.join("ActionManager_cut.cpp"), am).expect("write ActionManager_cut.cpp");
            let shim_weapon = dif.join("shim_weapon");
            let shim_armor = dif.join("shim_armor");
            let mut wp = cc::Build::new();
            base(&mut wp);
            wp.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&weapon_gen)
                .include(&shim_weapon)
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
                .flag(shim_weapon.join("weapon_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_weapon.cpp"));
            wp.compile("upstream_weapon");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/WeaponSet.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/ActionManager.cpp",
                "differential/wrap_weapon.cpp",
                "differential/shim_weapon",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w7-sonnet ---
        // --- p1w9-sonnet ---
        // Team.cpp (differential/wrap_team.cpp): TeamRelationMap, TeamFactory, TeamTemplateInfo, TeamPrototype and Team,
        // every function, with ONE mechanical edit made in memory into OUT_DIR/team_gen/Team_cut.cpp (upstream is never
        // edited; the markers are asserted to occur once): the include block (PreRTS.h .. ScriptEngine.h) is replaced
        // by a comment, the wrapper including what the stand-ins need before it. Team.h is unchanged. PlayerRelationMap,
        // which Team.cpp uses and Player.cpp defines, is Player.h's class text and Player.cpp's five functions, each
        // taken whole and unchanged (team_gen/player_relation_map_*.inc). The world Team.cpp reaches (objects,
        // players, scripts, the AI, the terrain, the partition manager, the game logic, the sides list) is stand-ins
        // in differential/shim_team, every class renamed by wrap_team.cpp. Built as the weapon build is.
        {
            fn once_at(text: &str, marker: &str) -> usize {
                assert_eq!(text.matches(marker).count(), 1, "{marker:?} must occur once");
                text.find(marker).expect("marker")
            }
            fn extract_fn(text: &str, header: &str) -> String {
                let a = once_at(text, header);
                let open = a + text[a..].find('{').expect("open brace");
                let mut depth = 0usize;
                for (i, c) in text[open..].char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                return format!("{}\n\n", &text[a..open + i + 1]);
                            }
                        }
                        _ => {}
                    }
                }
                panic!("{header:?} is not closed");
            }
            let team_gen = out_dir.join("team_gen");
            std::fs::create_dir_all(&team_gen).expect("create team_gen");
            std::fs::write(
                team_gen.join("basetype_coord3d.h"),
                format!("/* generated by build.rs: verbatim from Libraries/Include/Lib/BaseType.h */\n{coord3d}"),
            )
            .expect("write basetype_coord3d.h");
            let mut src = std::fs::read_to_string(up.join("GameEngine/Source/Common/RTS/Team.cpp")).expect("read Team.cpp");
            let a = once_at(&src, "#include \"PreRTS.h\"");
            let end = "#include \"GameLogic/ScriptEngine.h\"";
            let b = once_at(&src, end) + end.len();
            assert!(a < b);
            src = format!("{}/* the include block of Team.cpp is replaced by the includes of differential/wrap_team.cpp (build.rs) */{}", &src[..a], &src[b..]);
            std::fs::write(team_gen.join("Team_cut.cpp"), src).expect("write Team_cut.cpp");
            // GameCommon.h, with ONE mechanical edit (a portability fix; clang rejects what MSVC accepts): the DLINK
            // iterator names the pointer to the next-link member as `OBJCLASS::dlink_next_<list>`, which only MSVC reads
            // as `&OBJCLASS::dlink_next_<list>`. Found before the real header (team_gen is first on the include path).
            {
                let gc = std::fs::read_to_string(up.join("GameEngine/Include/Common/GameCommon.h")).expect("read GameCommon.h");
                let marker = "return DLINK_ITERATOR<OBJCLASS>(m_dlinkhead_##LISTNAME.m_head, OBJCLASS::dlink_next_##LISTNAME);";
                once_at(&gc, marker);
                let gc = gc.replace(marker, "return DLINK_ITERATOR<OBJCLASS>(m_dlinkhead_##LISTNAME.m_head, &OBJCLASS::dlink_next_##LISTNAME);");
                std::fs::create_dir_all(team_gen.join("Common")).expect("create team_gen/Common");
                std::fs::write(team_gen.join("Common/GameCommon.h"), gc).expect("write GameCommon.h");
            }
            let ph = std::fs::read_to_string(up.join("GameEngine/Include/Common/Player.h")).expect("read Player.h");
            let a = once_at(&ph, "typedef std::hash_map< PlayerIndex, Relationship, std::hash<PlayerIndex>, std::equal_to<PlayerIndex> > PlayerRelationMapType;");
            let c = once_at(&ph, "class PlayerRelationMap : public MemoryPoolObject,");
            let e = c + ph[c..].find("\n};").expect("end of PlayerRelationMap") + 3;
            assert!(a < c);
            std::fs::write(
                team_gen.join("player_relation_map_class.inc"),
                format!("/* generated by build.rs: PlayerRelationMapType and class PlayerRelationMap, verbatim from Player.h */\n{}\n", &ph[a..e]),
            )
            .expect("write player_relation_map_class.inc");
            let pc = std::fs::read_to_string(up.join("GameEngine/Source/Common/RTS/Player.cpp")).expect("read Player.cpp");
            let mut fns = String::from("/* generated by build.rs: the functions of PlayerRelationMap, verbatim from Player.cpp */\n");
            for h in [
                "PlayerRelationMap::PlayerRelationMap( void )",
                "PlayerRelationMap::~PlayerRelationMap( void )",
                "void PlayerRelationMap::crc( Xfer *xfer )",
                "void PlayerRelationMap::xfer( Xfer *xfer )",
                "void PlayerRelationMap::loadPostProcess( void )",
            ] {
                fns.push_str(&extract_fn(&pc, h));
            }
            std::fs::write(team_gen.join("player_relation_map_cpp.inc"), fns).expect("write player_relation_map_cpp.inc");
            let shim_team = dif.join("shim_team");
            let shim_weapon = dif.join("shim_weapon");
            let shim_armor = dif.join("shim_armor");
            let mut tmb = cc::Build::new();
            base(&mut tmb);
            tmb.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&team_gen)
                .include(&shim_team)
                .include(&shim_weapon)
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
                .flag(shim_weapon.join("weapon_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_team.cpp"));
            tmb.compile("upstream_team");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/Common/Team.h",
                "upstream/GeneralsMD/Code/GameEngine/Include/Common/Player.h",
                "upstream/GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h",
                "differential/wrap_team.cpp",
                "differential/shim_team",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w9-sonnet ---
        // --- p1w10-sonnet ---
        // Player.cpp (differential/wrap_player.cpp), slice 1: the functions of Player.h's class that slice translates
        // (the list below), each taken whole and UNCHANGED from Player.cpp into OUT_DIR/player_gen/player_fns.inc by
        // build.rs (asserted to occur once, and to be closed), with the file-level statics they use and the five
        // functions of PlayerRelationMap; Energy.cpp, Money.cpp, Handicap.cpp and MissionStats.cpp contribute the
        // functions slice 1 calls, the same way. Player.h, Team.h and every other header are unchanged. Team.cpp is the
        // p1w9-sonnet block's team_gen/Team_cut.cpp (one mechanical edit of its include block). The rest of the world is
        // stand-ins in differential/shim_player, every class renamed by wrap_player.cpp.
        {
            fn once_at(text: &str, marker: &str) -> usize {
                assert_eq!(text.matches(marker).count(), 1, "{marker:?} must occur once");
                text.find(marker).expect("marker")
            }
            // the text of the function, class or struct that starts at `header`: through the closing brace, and
            // through the `;` that follows a class or struct
            fn extract_block(text: &str, header: &str) -> String {
                let a = once_at(text, header);
                let open = a + text[a..].find('{').expect("open brace");
                let mut depth = 0usize;
                for (i, c) in text[open..].char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                let mut end = open + i + 1;
                                if text[end..].starts_with(';') {
                                    end += 1;
                                }
                                return format!("{}\n\n", &text[a..end]);
                            }
                        }
                        _ => {}
                    }
                }
                panic!("{header:?} is not closed");
            }
            let player_gen = out_dir.join("player_gen");
            std::fs::create_dir_all(&player_gen).expect("create player_gen");
            let rts = up.join("GameEngine/Source/Common/RTS");
            let read = |n: &str| std::fs::read_to_string(rts.join(n)).unwrap_or_else(|_| panic!("read {n}"));
            let pc = read("Player.cpp");
            let mut fns = String::from("/* generated by build.rs (p1w10-sonnet): the functions of Player.cpp, Energy.cpp, Money.cpp, Handicap.cpp and MissionStats.cpp that\n** slice 1 translates, each verbatim */\n");
            let neutral = "#define NEUTRAL_PLAYER_COLOR 0xffffffff";
            once_at(&pc, neutral);
            fns.push_str(neutral);
            fns.push_str("\n\n");
            for h in [
                "class ClosestKindOfData",
                "ClosestKindOfData::ClosestKindOfData( void )",
                "static void findClosestKindOf( Object *obj, void *userData )",
                "static void doPowerDisable( Object *obj, void *userData )",
                "void PlayerRelationMap::crc( Xfer *xfer )",
                "void PlayerRelationMap::xfer( Xfer *xfer )",
                "void PlayerRelationMap::loadPostProcess( void )",
                "PlayerRelationMap::PlayerRelationMap( void )",
                "PlayerRelationMap::~PlayerRelationMap( void )",
            ] {
                fns.push_str(&extract_block(&pc, h));
            }
            for h in [
                "Player::Player( Int playerIndex )",
                "Player::~Player()",
                "void Player::friend_setSkillset(Int skillSet)",
                "void Player::init(const PlayerTemplate* pt)",
                "Relationship Player::getRelationship(const Team *that) const",
                "void Player::setPlayerRelationship(const Player *that, Relationship r)",
                "Bool Player::removePlayerRelationship(const Player *that)",
                "void Player::setTeamRelationship(const Team *that, Relationship r)",
                "Bool Player::removeTeamRelationship(const Team *that)",
                "void Player::setBuildList(BuildListInfo *pBuildList)",
                "void Player::addToBuildList(Object *obj)",
                "void Player::addToPriorityBuildList(AsciiString templateName, Coord3D *pos, Real angle)",
                "void Player::update()",
                "void Player::newMap()",
                "void Player::setPlayerType(PlayerType t, Bool skirmish)",
                "void Player::setDefaultTeam(void) {",
                "Bool Player::isSkirmishAIPlayer( void )",
                "Bool Player::computeSuperweaponTarget(const SpecialPowerTemplate *power, Coord3D *retPos, Int playerNdx, Real weaponRadius)",
                "Player  *Player::getCurrentEnemy( void )",
                "GameDifficulty Player::getPlayerDifficulty(void) const",
                "Bool Player::checkBridges(Object *unit, Waypoint *way)",
                "Bool Player::getAiBaseCenter(Coord3D *pos)",
                "void Player::repairStructure(ObjectID structureID)",
                "void Player::onUnitCreated( Object *factory, Object *unit )",
                "Bool Player::isSupplySourceSafe( Int minSupplies )",
                "Bool Player::isSupplySourceAttacked( void )",
                "void Player::setTeamDelaySeconds(Int delay  )",
                "void Player::guardSupplyCenter( Team *team, Int minSupplies  )",
                "void Player::preTeamDestroy( const Team *team )",
                "void Player::onStructureCreated( Object *builder, Object *structure )",
                "void Player::onStructureUndone(Object *structure)",
                "void Player::addTeamToList(TeamPrototype* team)",
                "void Player::removeTeamFromList(TeamPrototype* team)",
                "void Player::healAllObjects()",
                "void Player::iterateObjects( ObjectIterateFunc func, void *userData ) const",
                "void Player::countObjectsByThingTemplate(Int numTmplates, const ThingTemplate* const * things, Bool ignoreDead, Int *counts, Bool ignoreUnderConstruction ) const",
                "Int Player::countBuildings(void)",
                "Int Player::countObjects(KindOfMaskType setMask, KindOfMaskType clearMask)",
                "Object *Player::findClosestByKindOf( Object *queryObject, KindOfMaskType setMask, KindOfMaskType clearMask )",
                "Bool Player::hasAnyBuildings(void) const",
                "Bool Player::hasAnyBuildings(KindOfMaskType kindOf) const",
                "Bool Player::hasAnyUnits(void) const",
                "Bool Player::hasAnyObjects(void) const",
                "Bool Player::hasAnyBuildFacility(void) const",
                "void Player::updateTeamStates(void)",
                "Bool Player::isLocalPlayer() const",
                "void Player::setListInScoreScreen(Bool listInScoreScreen)",
                "Bool Player::getListInScoreScreen()",
                "UnsignedInt Player::getSupplyBoxValue()",
                "Real Player::getProductionCostChangePercent( AsciiString buildTemplateName ) const",
                "Real Player::getProductionTimeChangePercent( AsciiString buildTemplateName ) const",
                "VeterancyLevel Player::getProductionVeterancyLevel( AsciiString buildTemplateName ) const",
                "void Player::setObjectsEnabled(AsciiString templateTypeToAffect, Bool enable)",
                "Bool Player::allowedToBuild(const ThingTemplate *tmplate) const",
                "void Player::buildSpecificTeam( TeamPrototype *teamProto)",
                "void Player::buildBaseDefense(Bool flank)",
                "void Player::buildBaseDefenseStructure(const AsciiString &thingName, Bool flank)",
                "void Player::buildSpecificBuilding(const AsciiString &thingName)",
                "void Player::buildBySupplies(Int minimumCash, const AsciiString &thingName)",
                "void Player::buildSpecificBuildingNearestTeam( const AsciiString &thingName, const Team *team )",
                "void Player::buildUpgrade( const AsciiString &upgrade)",
                "void Player::recruitSpecificTeam( TeamPrototype *teamProto, Real recruitRadius)",
                "Bool Player::calcClosestConstructionZoneLocation( const ThingTemplate *constructTemplate, Coord3D *location )",
                "Bool Player::hasPrereqsForScience(ScienceType t) const",
                "Bool Player::addSkillPoints(Int delta)",
                "Bool Player::addSkillPointsForKill(const Object* killer, const Object* victim)",
                "void Player::resetSciences()",
                "Bool Player::addScience(ScienceType science)",
                "void Player::addSciencePurchasePoints(Int delta)",
                "Bool Player::attemptToPurchaseScience(ScienceType science)",
                "Bool Player::grantScience(ScienceType science)",
                "Bool Player::isCapableOfPurchasingScience(ScienceType science) const",
                "void Player::resetRank()",
                "Bool Player::setRankLevel(Int newLevel)",
                "Bool Player::hasScience(ScienceType t) const",
                "Bool Player::isScienceDisabled( ScienceType t ) const",
                "Bool Player::isScienceHidden( ScienceType t ) const",
                "void Player::setScienceAvailability( ScienceType science, ScienceAvailabilityType type )",
                "ScienceAvailabilityType Player::getScienceAvailabilityTypeFromString( const AsciiString& name )",
                "void Player::deleteUpgradeList( void )",
                "Bool Player::okToPlayRadarEdgeSound( void )",
                "void Player::addRadar( Bool disableProof )",
                "void Player::removeRadar( Bool disableProof )",
                "void Player::disableRadar()",
                "void Player::enableRadar()",
                "Bool Player::hasRadar() const",
                "void Player::onPowerBrownOutChange( Bool brownOut )",
                "void Player::addNewSharedSpecialPowerTimer( const SpecialPowerTemplate *temp, UnsignedInt frame )",
                "void Player::resetOrStartSpecialPowerReadyFrame( const SpecialPowerTemplate *temp )",
                "void Player::expressSpecialPowerReadyFrame( const SpecialPowerTemplate *temp, UnsignedInt frame )",
                "UnsignedInt Player::getOrStartSpecialPowerReadyFrame( const SpecialPowerTemplate *temp)",
                "void Player::addKindOfProductionCostChange(	KindOfMaskType kindOf, Real percent )",
                "void Player::removeKindOfProductionCostChange(	KindOfMaskType kindOf, Real percent )",
                "Real Player::getProductionCostChangeBasedOnKindOf( KindOfMaskType kindOf ) const",
                "void Player::setAttackedBy( Int playerNdx )",
                "Bool Player::getAttackedBy( Int playerNdx ) const",
                "Bool Player::isPlayerObserver(void) const",
                "Bool Player::isPlayerDead(void) const",
                "Bool Player::isPlayerActive(void) const",
                "Bool Player::isPlayableSide( void ) const",
            ] {
                fns.push_str(&extract_block(&pc, h));
            }
            // slice 2: the half of Player that works through objects, and the helpers and structs they use
            for h in [
                "struct PlayerObjectFindInfo",
                "static void doFindCommandCenter(Object* obj, void* userData)",
                "static void doFindSpecialPowerSourceObject( Object *obj, void *userData )",
                "static void doCountSpecialPowersReady( Object *obj, void *userData )",
                "static void doFindMostReadyWeaponForThing( Object *obj, void *userData )",
                "static void doFindMostReadySpecialPowerForThing( Object *obj, void *userData )",
                "static void doFindExistingObjectWithThingTemplate( Object *obj, void *userData )",
                "struct TypeCountData",
                "static void countExisting( Object *obj, void *userData )",
                "struct VisionSpiedStruct",
                "static void iterator_setUnitsVisionSpied( Object *obj, void * voidData)",
                "void sellBuildings( Object *obj, void *userData )",
                "static void localApplyBattlePlanBonusesToObject( Object *obj, void *userData )",
                "void Player::becomingTeamMember(Object *obj, Bool yes)",
                "void Player::becomingLocalPlayer(Bool yes)",
                "Object* Player::findNaturalCommandCenter()",
                "Object* Player::findMostReadyShortcutSpecialPowerOfType( SpecialPowerType spType )",
                "Object* Player::findMostReadyShortcutWeaponForThing( const ThingTemplate *thing, UnsignedInt &mostReadyPercentage )",
                "Object* Player::findMostReadyShortcutSpecialPowerForThing( const ThingTemplate *thing, UnsignedInt &mostReadyPercentage )",
                "Object* Player::findAnyExistingObjectWithThingTemplate( const ThingTemplate *thing )",
                "Bool Player::hasAnyShortcutSpecialPower()",
                "Int Player::countReadyShortcutSpecialPowersOfType( SpecialPowerType spType )",
                "void Player::onStructureConstructionComplete( Object *builder, Object *structure, Bool isRebuild )",
                "void Player::setUnitsShouldHunt(Bool unitsShouldHunt, CommandSourceType source)",
                "void Player::killPlayer(void)",
                "void Player::transferAssetsFromThat(Player *that)",
                "void Player::garrisonAllUnits(CommandSourceType source)",
                "void Player::ungarrisonAllUnits(CommandSourceType source)",
                "void Player::setUnitsShouldIdleOrResume(Bool idle)",
                "void Player::sellEverythingUnderTheSun()",
                "void Player::doBountyForKill(const Object* killer, const Object* victim)",
                "Bool Player::canBuildMoreOfType( const ThingTemplate *whatToBuild ) const",
                "Bool Player::canBuild(const ThingTemplate *tmplate) const",
                "Bool Player::canAffordBuild( const ThingTemplate *whatToBuild ) const",
                "void Player::friend_applyDifficultyBonusesForObject(Object* obj, Bool apply) const",
                "Bool Player::doesObjectQualifyForBattlePlan( Object *obj ) const",
                "void Player::changeBattlePlan( BattlePlanStatus plan, Int delta, BattlePlanBonuses *bonus )",
                "Int Player::getBattlePlansActiveSpecific( BattlePlanStatus plan ) const",
                "void Player::applyBattlePlanBonusesForObject( Object *obj ) const",
                "void Player::removeBattlePlanBonusesForObject( Object *obj ) const",
                "void Player::applyBattlePlanBonusesForPlayerObjects( const BattlePlanBonuses *bonus )",
                "void Player::setUnitsVisionSpied( Bool setting, KindOfMaskType whichUnits, PlayerIndex byWhom )",
            ] {
                fns.push_str(&extract_block(&pc, h));
            }
            // slice 2b and 3: the upgrades, the hotkey squads, and the save and load
            for h in [
                "Upgrade *Player::findUpgrade( const UpgradeTemplate *upgradeTemplate )",
                "Bool Player::hasUpgradeComplete( const UpgradeTemplate *upgradeTemplate )",
                "Bool Player::hasUpgradeComplete( UpgradeMaskType testMask )",
                "Bool Player::hasUpgradeInProduction( const UpgradeTemplate *upgradeTemplate )",
                "Upgrade *Player::addUpgrade( const UpgradeTemplate *upgradeTemplate, UpgradeStatusType status )",
                "void Player::onUpgradeCompleted( const UpgradeTemplate *upgradeTemplate )",
                "void Player::removeUpgrade( const UpgradeTemplate *upgradeTemplate )",
                "void Player::processCreateTeamGameMessage(Int hotkeyNum, GameMessage *msg) {",
                "void Player::processSelectTeamGameMessage(Int hotkeyNum, GameMessage *msg) {",
                "void Player::processAddTeamGameMessage(Int hotkeyNum, GameMessage *msg) {",
                "void Player::getCurrentSelectionAsAIGroup(AIGroup *group) {",
                "void Player::setCurrentlySelectedAIGroup(AIGroup *group) {",
                "Squad *Player::getHotkeySquad(Int squadNumber)",
                "Int Player::getSquadNumberForObject(const Object *objToFind) const",
                "void Player::removeObjectFromHotkeySquad(Object *objToRemove)",
                "void Player::addAIGroupToCurrentSelection(AIGroup *group) {",
                "void Player::crc( Xfer *xfer )",
                "void Player::xfer( Xfer *xfer )",
                "void Player::loadPostProcess( void )",
                "void Player::initFromDict(const Dict* d)",
            ] {
                fns.push_str(&extract_block(&pc, h));
            }
            let gsrc = up.join("GameEngine/Source");
            let rds = |n: &str| std::fs::read_to_string(gsrc.join(n)).unwrap_or_else(|_| panic!("read {n}"));
            let sq = rds("GameLogic/AI/Squad.cpp");
            for h in [
                "void Squad::addObject(Object *objectToAdd)",
                "void Squad::addObjectID(ObjectID objectID) {",
                "void Squad::removeObject(Object *objectToRemove)",
                "void Squad::clearSquad() {",
                "const VecObjectPtr& Squad::getAllObjects(void)",
                "const VecObjectPtr& Squad::getLiveObjects(void)",
                "Int Squad::getSizeOfGroup(void) const",
                "Bool Squad::isOnSquad(const Object *objToTest) const",
                "void Squad::squadFromTeam(const Team* fromTeam, Bool clearSquadFirst)",
                "void Squad::squadFromAIGroup(const AIGroup* fromAIGroup, Bool clearSquadFirst)",
                "void Squad::aiGroupFromSquad(AIGroup* aiGroupToFill)",
                "void Squad::crc( Xfer *xfer )",
                "void Squad::xfer( Xfer *xfer )",
                "void Squad::loadPostProcess( void )",
            ] {
                fns.push_str(&extract_block(&sq, h));
            }
            let ug = rds("Common/System/Upgrade.cpp");
            for h in [
                "Upgrade::Upgrade( const UpgradeTemplate *upgradeTemplate )",
                "Upgrade::~Upgrade( void )",
                "void Upgrade::crc( Xfer *xfer )",
                "void Upgrade::xfer( Xfer *xfer )",
                "void Upgrade::loadPostProcess( void )",
                "UpgradeTemplate::UpgradeTemplate( void )",
                "UpgradeTemplate::~UpgradeTemplate( void )",
                "UpgradeCenter::UpgradeCenter( void )",
                "UpgradeCenter::~UpgradeCenter( void )",
                "UpgradeTemplate *UpgradeCenter::findNonConstUpgradeByKey( NameKeyType key )",
                "UpgradeTemplate *UpgradeCenter::firstUpgradeTemplate( void )",
                "const UpgradeTemplate *UpgradeCenter::findUpgradeByKey( NameKeyType key ) const",
                "const UpgradeTemplate *UpgradeCenter::findUpgrade( const AsciiString& name ) const",
                "UpgradeTemplate *UpgradeCenter::newUpgrade( const AsciiString& name )",
                "void UpgradeCenter::linkUpgrade( UpgradeTemplate *upgrade )",
            ] {
                fns.push_str(&extract_block(&ug, h));
            }
            let sl = rds("GameLogic/Map/SidesList.cpp");
            for h in [
                "BuildListInfo::BuildListInfo(void) :",
                "void BuildListInfo::crc( Xfer *xfer )",
                "void BuildListInfo::xfer( Xfer *xfer )",
                "void BuildListInfo::loadPostProcess( void )",
            ] {
                fns.push_str(&extract_block(&sl, h));
            }
            let sci = read("Science.cpp");
            for h in [
                "ScienceType ScienceStore::getScienceFromInternalName(const AsciiString& name) const",
                "AsciiString ScienceStore::getInternalNameForScience(ScienceType science) const",
            ] {
                fns.push_str(&extract_block(&sci, h));
            }
            let rg = read("ResourceGatheringManager.cpp");
            for h in [
                "void ResourceGatheringManager::crc( Xfer *xfer )",
                "void ResourceGatheringManager::xfer( Xfer *xfer )",
            ] {
                fns.push_str(&extract_block(&rg, h));
            }
            let tn = read("TunnelTracker.cpp");
            for h in ["void TunnelTracker::crc( Xfer *xfer )", "void TunnelTracker::xfer( Xfer *xfer )"] {
                fns.push_str(&extract_block(&tn, h));
            }
            // MSVC 6 scoped a `for` loop's declaration to the enclosing block, and `Player::killPlayer` relies on it:
            // its second loop reuses the `it` of the first. Every later compiler rejects that; the edit gives the second
            // loop its own declaration, with the same type and the same initial value, and changes nothing else.
            {
                let old = "\tfor (it = m_playerTeamPrototypes.begin(); it != m_playerTeamPrototypes.end(); ++it) {";
                assert_eq!(fns.matches(old).count(), 1, "killPlayer's second loop");
                fns = fns.replace(old, "\tfor (PlayerTeamList::iterator it = m_playerTeamPrototypes.begin(); it != m_playerTeamPrototypes.end(); ++it) {");
            }
            let tt = std::fs::read_to_string(up.join("GameEngine/Source/Common/Thing/ThingTemplate.cpp")).expect("read ThingTemplate.cpp");
            fns.push_str(&extract_block(&tt, "Int ThingTemplate::calcCostToBuild( const Player* player) const"));
            let en = read("Energy.cpp");
            for h in [
                "Energy::Energy()",
                "Int Energy::getProduction() const",
                "Real Energy::getEnergySupplyRatio() const",
                "Bool Energy::hasSufficientPower(void) const",
                "void Energy::adjustPower(Int powerDelta, Bool adding)",
                "void Energy::addProduction(Int amt)",
                "void Energy::addConsumption(Int amt)",
                "void Energy::crc( Xfer *xfer )",
                "void Energy::xfer( Xfer *xfer )",
                "void Energy::loadPostProcess( void )",
            ] {
                fns.push_str(&extract_block(&en, h));
            }
            let mo = read("Money.cpp");
            for h in [
                "UnsignedInt Money::withdraw(UnsignedInt amountToWithdraw, Bool playSound)",
                "void Money::deposit(UnsignedInt amountToDeposit, Bool playSound)",
                "void Money::crc( Xfer *xfer )",
                "void Money::xfer( Xfer *xfer )",
                "void Money::loadPostProcess( void )",
            ] {
                fns.push_str(&extract_block(&mo, h));
            }
            let ha = read("Handicap.cpp");
            for h in [
                "Handicap::Handicap()",
                "void Handicap::init()",
                "void Handicap::readFromDict(const Dict* d)",
                "/*static*/ Handicap::ThingType Handicap::getBestThingType(const ThingTemplate *tmpl)",
                "Real Handicap::getHandicap(HandicapType ht, const ThingTemplate *tmpl) const",
            ] {
                fns.push_str(&extract_block(&ha, h));
            }
            let pln = read("PlayerList.cpp");
            for h in [
                "void PlayerList::teamAboutToBeDeleted(Team* team)",
                "Player *PlayerList::getNthPlayer(Int i)",
                "void PlayerList::reset()",
                "void PlayerList::newGame()",
                "void PlayerList::init()",
                "void PlayerList::update()",
                "void PlayerList::newMap()",
                "void PlayerList::updateTeamStates(void)",
                "Team *PlayerList::validateTeam( AsciiString owner )",
                "void PlayerList::setLocalPlayer(Player *player)",
                "Player *PlayerList::getPlayerFromMask( PlayerMaskType mask )",
                "Player *PlayerList::getEachPlayerFromMask( PlayerMaskType& maskToAdjust )",
                "PlayerMaskType PlayerList::getPlayersWithRelationship( Int srcPlayerIndex, UnsignedInt allowedRelationships )",
                "void PlayerList::crc( Xfer *xfer )",
                "void PlayerList::xfer( Xfer *xfer )",
                "void PlayerList::loadPostProcess( void )",
            ] {
                fns.push_str(&extract_block(&pln, h));
            }
            let ms = read("MissionStats.cpp");
            for h in ["MissionStats::MissionStats()", "void MissionStats::init()"] {
                fns.push_str(&extract_block(&ms, h));
            }
            std::fs::write(player_gen.join("player_fns.inc"), fns).expect("write player_fns.inc");
            // `BitFlags::xfer` in CRC mode hashes `sizeof( this )` bytes: the size of a POINTER, 4 in the shipped Win32 build and 8 in this
            // 64-bit reference (see common/bit_flags.rs). Player::crc writes two such masks, so the reference is made to hash the 4 bytes
            // the shipped build hashes: the one edit of this copy of BitFlagsIO.h (found before the upstream header), asserted once.
            std::fs::create_dir_all(player_gen.join("Common")).expect("create player_gen/Common");
            let bf = std::fs::read_to_string(up.join("GameEngine/Include/Common/BitFlagsIO.h")).expect("read BitFlagsIO.h");
            let bf_old = "xfer->xferUser( this, sizeof( this ) );";
            assert_eq!(bf.matches(bf_old).count(), 1, "BitFlags::xfer's CRC branch");
            // (it also does what differential/shim_flags/Common/BitFlagsIO.h does, which it replaces: the real KindOf.h first, see that file)
            let bf_text = format!(
                "#ifndef SHIM_FLAGS_BITFLAGSIO_H\n#define SHIM_FLAGS_BITFLAGSIO_H\n#include \"Common/KindOf.h\"\n{}\n#endif\n",
                bf.replace(bf_old, "xfer->xferUser( this, 4 ); /* build.rs: sizeof( this ) of the shipped Win32 build */")
            );
            std::fs::write(player_gen.join("Common/BitFlagsIO.h"), bf_text).expect("write BitFlagsIO.h");
            // Player.h forward-declares `enum BattlePlanStatus;` and `class BattlePlanBonuses;`, which only MSVC accepts
            // for an enum; the definitions come from BattlePlanUpdate.h, whole and unchanged
            let bp = std::fs::read_to_string(up.join("GameEngine/Include/GameLogic/Module/BattlePlanUpdate.h")).expect("read BattlePlanUpdate.h");
            let mut enums = String::from("/* generated by build.rs (p1w10-sonnet): from BattlePlanUpdate.h, verbatim */\n");
            enums.push_str(&extract_block(&bp, "enum BattlePlanStatus"));
            enums.push_str(&extract_block(&bp, "class BattlePlanBonuses : public MemoryPoolObject"));
            let all_plans = "#define ALL_PLANS\t1000000";
            once_at(&bp, all_plans);
            // slice 2: two enums the real headers declare only as opaque `enum X;` (Object.h) or inside classes the
            // stand-ins replace
            let wpn = std::fs::read_to_string(up.join("GameEngine/Include/GameLogic/Weapon.h")).expect("read Weapon.h");
            enums.push_str(&extract_block(&wpn, "enum WeaponBonusConditionType"));
            let tth = std::fs::read_to_string(up.join("GameEngine/Include/Common/ThingTemplate.h")).expect("read ThingTemplate.h");
            enums.push_str(&extract_block(&tth, "enum BuildableStatus"));
            enums.push_str("EMPTY_DTOR(BattlePlanBonuses)\n");
            enums.push_str(all_plans);
            enums.push('\n');
            std::fs::write(player_gen.join("enums_gen.inc"), enums).expect("write enums_gen.inc");
            let team_gen = out_dir.join("team_gen");
            let shim_player = dif.join("shim_player");
            let shim_weapon = dif.join("shim_weapon");
            let shim_armor = dif.join("shim_armor");
            let mut plb = cc::Build::new();
            base(&mut plb);
            plb.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&team_gen)
                .include(&player_gen)
                .include(&shim_player)
                .include(&shim_weapon)
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
                .flag(shim_player.join("player_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_player.cpp"));
            plb.compile("upstream_player");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Energy.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Money.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Handicap.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/MissionStats.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerList.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/ResourceGatheringManager.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/TunnelTracker.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Upgrade.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Xfer.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/AI/Squad.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Map/SidesList.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/Common/Player.h",
                "differential/wrap_player.cpp",
                "differential/shim_player",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // BuildAssistant.cpp (differential/wrap_build_assistant.cpp), p1w10-sonnet: the whole file, with ONE mechanical edit made in memory into
        // OUT_DIR/build_assistant_gen/BuildAssistant_cut.cpp (upstream is never edited; the markers are asserted to occur once): the include
        // block (PreRTS.h .. ParkingPlaceBehavior.h) is replaced by a comment, the wrapper including what the stand-ins need before it.
        // BuildAssistant.h is unchanged. The world it reaches is stand-ins in differential/shim_buildassist, every class renamed by the wrapper.
        {
            fn once_at(text: &str, marker: &str) -> usize {
                assert_eq!(text.matches(marker).count(), 1, "{marker:?} must occur once");
                text.find(marker).expect("marker")
            }
            let ba_gen = out_dir.join("build_assistant_gen");
            std::fs::create_dir_all(&ba_gen).expect("create build_assistant_gen");
            let mut src = std::fs::read_to_string(up.join("GameEngine/Source/Common/System/BuildAssistant.cpp")).expect("read BuildAssistant.cpp");
            let a = once_at(&src, "#include \"PreRTS.h\"");
            let end = "#include \"GameLogic/Module/ParkingPlaceBehavior.h\"";
            let b = once_at(&src, end) + end.len();
            assert!(a < b);
            src = format!("{}/* the include block of BuildAssistant.cpp is replaced by the includes of differential/wrap_build_assistant.cpp (build.rs) */{}", &src[..a], &src[b..]);
            std::fs::write(ba_gen.join("BuildAssistant_cut.cpp"), src).expect("write BuildAssistant_cut.cpp");
            let shim_ba = dif.join("shim_buildassist");
            let shim_weapon = dif.join("shim_weapon");
            let shim_armor = dif.join("shim_armor");
            let mut bab = cc::Build::new();
            base(&mut bab);
            bab.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&ba_gen)
                .include(&shim_ba)
                .include(out_dir.join("team_gen"))
                .include(&shim_weapon)
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
                .flag(shim_weapon.join("weapon_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_build_assistant.cpp"));
            bab.compile("upstream_build_assistant");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/System/BuildAssistant.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/Common/BuildAssistant.h",
                "differential/wrap_build_assistant.cpp",
                "differential/shim_buildassist",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // ObjectCreationList.cpp (differential/wrap_ocl.cpp), p1w10-sonnet: the whole file, with TWO mechanical edits made in memory into
        // OUT_DIR/ocl_gen/ObjectCreationList_cut.cpp (upstream is never edited; every marker is asserted to occur once): `#include "PreRTS.h"` and
        // the include block (Common/AudioEventRTS.h .. Common/CRCDebug.h) are each replaced by a comment, the wrapper including what the
        // stand-ins need before it. ObjectCreationList.h and RadiusDecal.h are unchanged. Three pieces of OTHER units' files, which the file
        // reaches and which are verbatim text, are cut out into .inc files: RadiusDecalTemplate's constructor and parser (RadiusDecal.cpp), the
        // DeliverPayloadData class with its field parse (DeliverPayloadAIUpdate.h/.cpp) and INI::parseStaticGameLODLevel with its name list
        // (GameLOD.cpp). The world the file reaches is stand-ins in differential/shim_ocl, every class renamed by the wrapper.
        {
            fn once_at(text: &str, marker: &str) -> usize {
                assert_eq!(text.matches(marker).count(), 1, "{marker:?} must occur once");
                text.find(marker).expect("marker")
            }
            // the text from `start` to the end of the first `end` after it, both asserted to occur once (`end` after `start`)
            fn extract(text: &str, start: &str, end: &str) -> String {
                let a = once_at(text, start);
                let rest = &text[a..];
                let b = rest.find(end).unwrap_or_else(|| panic!("{end:?} must follow {start:?}"));
                format!("{}\n", &rest[..b + end.len()])
            }
            let ocl_gen = out_dir.join("ocl_gen");
            std::fs::create_dir_all(&ocl_gen).expect("create ocl_gen");
            let mut src = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/ObjectCreationList.cpp")).expect("read ObjectCreationList.cpp");
            let pre = "#include \"PreRTS.h\"";
            let a = once_at(&src, pre);
            let eol = a + src[a..].find('\n').expect("end of the PreRTS line");
            src = format!("{}/* PreRTS.h is replaced by the includes of differential/wrap_ocl.cpp (build.rs) */{}", &src[..a], &src[eol..]);
            let first = "#include \"Common/AudioEventRTS.h\"";
            let last = "#include \"Common/CRCDebug.h\"";
            let a = once_at(&src, first);
            let b = once_at(&src, last) + last.len();
            assert!(a < b);
            src = format!("{}/* the include block of ObjectCreationList.cpp is replaced by the includes of differential/wrap_ocl.cpp (build.rs) */{}", &src[..a], &src[b..]);
            std::fs::write(ocl_gen.join("ObjectCreationList_cut.cpp"), src).expect("write ObjectCreationList_cut.cpp");

            let rd = std::fs::read_to_string(up.join("GameEngine/Source/GameClient/RadiusDecal.cpp")).expect("read RadiusDecal.cpp");
            let mut inc = extract(&rd, "RadiusDecalTemplate::RadiusDecalTemplate() : ", "\n}\n");
            inc += &extract(&rd, "/*static*/ void RadiusDecalTemplate::parseRadiusDecalTemplate(", "\tini->initFromINI(store, dataFieldParse);\n}");
            std::fs::write(ocl_gen.join("ocl_radius_decal_template.inc"), inc).expect("write ocl_radius_decal_template.inc");

            let dh = std::fs::read_to_string(up.join("GameEngine/Include/GameLogic/Module/DeliverPayloadAIUpdate.h")).expect("read DeliverPayloadAIUpdate.h");
            let dc = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/Update/AIUpdate/DeliverPayloadAIUpdate.cpp")).expect("read DeliverPayloadAIUpdate.cpp");
            let mut inc = extract(&dh, "class DeliverPayloadData\n{", "\tstatic const FieldParse* getFieldParse();\n};");
            inc += &extract(&dc, "const FieldParse* DeliverPayloadData::getFieldParse()", "\treturn dataFieldParse;\n}");
            std::fs::write(ocl_gen.join("ocl_deliver_payload_data.inc"), inc).expect("write ocl_deliver_payload_data.inc");

            let lod = std::fs::read_to_string(up.join("GameEngine/Source/Common/GameLOD.cpp")).expect("read GameLOD.cpp");
            let mut inc = extract(&lod, "static const char *StaticGameLODNames[]=", "\n};");
            inc += &extract(&lod, "void INI::parseStaticGameLODLevel( INI* ini, void * , void *store, const void*)", "\tthrow INI_INVALID_DATA;\n}");
            std::fs::write(ocl_gen.join("ocl_static_lod_level.inc"), inc).expect("write ocl_static_lod_level.inc");

            // differential/shim_weapon has a stand-in GameLogic/ObjectCreationList.h that comes first on the include path; the real header
            // is reached by its absolute path through this generated file
            let real_header = up.join("GameEngine/Include/GameLogic/ObjectCreationList.h");
            std::fs::write(
                ocl_gen.join("ocl_real_header.h"),
                format!("/* generated by build.rs: the real ObjectCreationList.h, unchanged */\n#include \"{}\"\n", real_header.display()),
            )
            .expect("write ocl_real_header.h");
            let shim_ocl = dif.join("shim_ocl");
            let shim_weapon = dif.join("shim_weapon");
            let shim_armor = dif.join("shim_armor");
            let mut oclb = cc::Build::new();
            base(&mut oclb);
            // The undefined inputs of the file (a method called on a `NULL` object, see ocl_driver.h) must crash the C++ the way they crash the
            // shipped game, so that the driver can catch the SIGSEGV; by default the optimiser may assume a pointer that is dereferenced is not null
            // and compile the crash away.
            oclb.compiler("clang++")
                .flag("-fno-delete-null-pointer-checks")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&ocl_gen)
                .include(&shim_ocl)
                .include(out_dir.join("team_gen"))
                .include(&shim_weapon)
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
                .flag(shim_weapon.join("weapon_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_ocl.cpp"));
            oclb.compile("upstream_ocl");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectCreationList.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/ObjectCreationList.h",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameClient/RadiusDecal.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameClient/RadiusDecal.h",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DeliverPayloadAIUpdate.h",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/DeliverPayloadAIUpdate.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/GameLOD.cpp",
                "differential/wrap_ocl.cpp",
                "differential/shim_ocl",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w10-sonnet ---
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

        // --- p1w3-playertemplate ---
        // PlayerTemplate (differential/wrap_playertemplate.cpp):
        // PlayerTemplate.cpp unchanged, in the Science build's setup (clang++
        // -fms-extensions, the real BitFlagsIO.h through flags_prelude.h, the
        // real Science.h through science_prelude.h) plus
        // differential/shim_playertemplate (a prelude adding three typedefs of
        // the real STLTypedefs.h, and the one-lookup Image.h of the INI
        // build). It defines ThePlayerTemplateStore and
        // INI::parsePlayerTemplateDefinition (WEAK in wrap_ini.cpp) and no
        // other symbol another fixture defines: Money::deposit comes from
        // upstream_money, Handicap's constructor from upstream_handicap,
        // ScienceStore and Player::hasScience from upstream_science (whose
        // wrap_science.o is pulled in for the latter). Placed ahead of
        // upstream_science, upstream_money, upstream_handicap, upstream_xfer,
        // upstream_ini and upstream_chunk, which it uses. The Science lookup
        // is renamed as in the Science build, so that the wrapper can call
        // the real one from the hook it installs in ref_science_lookup_hook.
        {
            let shim_pt = dif.join("shim_playertemplate");
            let shim_science = dif.join("shim_science");
            let mut ptb = cc::Build::new();
            base(&mut ptb);
            ptb.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&shim_pt)
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
                .define("friend_lookupScience", "real_friend_lookupScience")
                .flag("-include")
                .flag(shim_pt.join("playertemplate_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_playertemplate.cpp"))
                .file(up.join("GameEngine/Source/Common/RTS/PlayerTemplate.cpp"));
            ptb.compile("upstream_playertemplate");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerTemplate.cpp",
                "differential/wrap_playertemplate.cpp",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w3-playertemplate ---
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
        // --- p1w3-statemachine ---
        // StateMachine (differential/wrap_statemachine.cpp): StateMachine.cpp
        // compiled unchanged, as its own translation unit, with StateMachine.h
        // and the Xfer headers unchanged, against differential/shim_statemachine
        // (GameLogic.h with the frame and findObjectByID, Object.h, ThingTemplate.h,
        // GlobalData.h, and a GameState.h that adds the SaveCode enum to the Xfer
        // build's). The engine classes the stand-ins define are renamed Sm* by
        // shim_statemachine/statemachine_prelude.h (forced in), so that no symbol
        // of this archive clashes with another fixture's. One generated file, in
        // OUT_DIR/sm_gen: save_code.inc, `enum SaveCode` cut verbatim out of the
        // real GameState.h (asserted once; StateMachine.cpp throws SC_INVALID_DATA).
        // Linked ahead of upstream_xfer, whose Xfer classes it uses.
        {
            let shim_sm = dif.join("shim_statemachine");
            let sm_gen = out_dir.join("sm_gen");
            std::fs::create_dir_all(&sm_gen).expect("create sm_gen");
            let game_state = std::fs::read_to_string(up.join("GameEngine/Include/Common/GameState.h")).expect("read GameState.h");
            let first = "enum SaveCode\n{";
            assert_eq!(game_state.matches(first).count(), 1, "GameState.h SaveCode definition must occur once");
            let a = game_state.find(first).expect("SaveCode definition");
            let rel = game_state[a..].find("};").expect("end of SaveCode definition");
            std::fs::write(
                sm_gen.join("save_code.inc"),
                format!("/* generated by build.rs: verbatim from GameState.h */\n{}\n", &game_state[a..a + rel + 2]),
            )
            .expect("write save_code.inc");
            let mut smb = cc::Build::new();
            base(&mut smb);
            smb.include(&shim_sm)
                .include(&sm_gen)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .flag("-include")
                .flag(shim_sm.join("statemachine_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_statemachine.cpp"))
                .file(up.join("GameEngine/Source/Common/StateMachine.cpp"));
            smb.compile("upstream_statemachine");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/StateMachine.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/Common/StateMachine.h",
                "upstream/GeneralsMD/Code/GameEngine/Include/Common/GameState.h",
                "differential/wrap_statemachine.cpp",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w3-statemachine ---

        // --- p1l9-updates ---
        // LifetimeUpdate.cpp and DeletionUpdate.cpp, unchanged (differential/wrap_l9updates.cpp), built with the
        // module harness's setup (clang++ -fms-extensions, module_prelude.h forced in, the same renames) and with
        // differential/shim_updates first on the include path: its GameLogic/Object.h and GameLogic.h are the module
        // build's with three declarations added and no member changed. Linked before upstream_module (the base
        // classes UpdateModule, BehaviorModule, ModuleData and the wake recorder come from there).
        {
            let shim_mod = dif.join("shim_module");
            let shim_upd = dif.join("shim_updates");
            let mut lu = cc::Build::new();
            base(&mut lu);
            lu.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&shim_upd)
                .include(&shim_mod)
                .include(dif.join("shim_upgrade"))
                .include(&flags_gen)
                .include(dif.join("shim_flags"))
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .include(&dif)
                // BaseType first, forced: PreRTS.h (shim/) includes its own Lib/BaseType.h by a relative path, which
                // would win otherwise. The two files share the include guard, so the shim's copy is then skipped.
                .flag("-include")
                .flag(shim_upd.join("Lib/BaseType.h").to_str().expect("UTF-8 path"))
                .flag("-include")
                .flag(shim_mod.join("module_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_l9updates.cpp"));
            lu.compile("upstream_l9updates");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/LifetimeUpdate.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DeletionUpdate.cpp",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1l9-updates ---
        // --- p1w3-modules ---
        // The module base classes (differential/wrap_module.cpp, which #includes Module.cpp,
        // BehaviorModule.cpp, UpdateModule.cpp, UpgradeModule.cpp, DieModule.cpp, CreateModule.cpp,
        // DamageModule.cpp, CollideModule.cpp, DestroyModule.cpp and BodyModule.cpp unchanged, see the
        // wrapper): the Armor build's setup (clang++ -fms-extensions, the real BitFlagsIO.h through the
        // flags prelude) with differential/shim_module first on the include path (module_prelude.h
        // forced in: the renames, the stand-ins of Thing, Object, GameLogic, Drawable and FXList) and
        // differential/shim_upgrade after it (the forwarder to the REAL Upgrade.h, so that this unit
        // reaches the UpgradeCenter that wrap_upgrade.cpp compiles). One generated file, in
        // OUT_DIR/mod_gen: module_registrations.inc, the body of `ModuleFactory::init` cut verbatim
        // out of ModuleFactory.cpp (both markers asserted once; the rest of that file includes 205
        // module headers), which the wrapper preprocesses with `addModule` defined as a recorder.
        {
            let shim_mod = dif.join("shim_module");
            let mod_gen = out_dir.join("mod_gen");
            std::fs::create_dir_all(&mod_gen).expect("create mod_gen");
            let factory = std::fs::read_to_string(up.join("GameEngine/Source/Common/Thing/ModuleFactory.cpp")).expect("read ModuleFactory.cpp");
            let (first, last) = ("void ModuleFactory::init( void )\n{\n", "}  // end init");
            assert_eq!(factory.matches(first).count(), 1, "ModuleFactory.cpp: init() must occur once");
            assert_eq!(factory.matches(last).count(), 1, "ModuleFactory.cpp: its end marker must occur once");
            let a = factory.find(first).expect("init start") + first.len();
            let b = factory.find(last).expect("init end");
            assert!(a < b, "ModuleFactory.cpp: init() body");
            std::fs::write(
                mod_gen.join("module_registrations.inc"),
                format!("/* generated by build.rs: the body of ModuleFactory::init, verbatim from ModuleFactory.cpp */\n{}", &factory[a..b]),
            )
            .expect("write module_registrations.inc");
            // --- p1w11-sonnet ---
            // Crate batch F (differential/l9_crates_f.inc) and update batch L (differential/l9_updates_l.inc): two mechanical edits of upstream text, each
            // asserted to apply once.
            // (1) MoneyCrateCollide.cpp -> mod_gen/MoneyCrateCollide_static.cpp: the function-local `static` inside the loop of
            //     getUpgradedSupplyBoost (initialised once per process) becomes a global and its flag, which the driver resets for each case, so
            //     that the Rust side's per-world cache can be compared run by run. The initialiser is upstream's, the init-once control flow is the
            //     `static`'s own.
            let money_path = up.join("GameEngine/Source/GameLogic/Object/Collide/CrateCollide/MoneyCrateCollide.cpp");
            let money = std::fs::read_to_string(&money_path).expect("read MoneyCrateCollide.cpp");
            let static_line = "static const UpgradeTemplate *upgradeTemplate = TheUpgradeCenter->findUpgrade( info.type.c_str() );";
            assert_eq!(money.matches(static_line).count(), 1, "MoneyCrateCollide.cpp: the static declaration must occur once");
            let money_cut = money.replace(
                static_line,
                "if (!g_moneyStaticInit) { g_moneyStatic = TheUpgradeCenter->findUpgrade( info.type.c_str() ); g_moneyStaticInit = true; } const UpgradeTemplate *upgradeTemplate = g_moneyStatic;",
            );
            std::fs::write(mod_gen.join("MoneyCrateCollide_static.cpp"), money_cut).expect("write MoneyCrateCollide_static.cpp");
            // (2) AutoDepositUpdate.cpp -> mod_gen/AutoDepositUpdate_static.cpp: the same `static` inside the loop of its
            //     getUpgradedSupplyBoost (a defect of its own), made a global and a flag the driver resets. The file also defines parseUpgradePair,
            //     which MoneyCrateCollide's data reads (crate batch F had it cut out alone before the update module was translated).
            let deposit = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/Update/AutoDepositUpdate.cpp")).expect("read AutoDepositUpdate.cpp");
            assert_eq!(deposit.matches(static_line).count(), 1, "AutoDepositUpdate.cpp: the static declaration must occur once");
            let deposit_cut = deposit.replace(
                static_line,
                "if (!g_adStaticInit) { g_adStatic = TheUpgradeCenter->findUpgrade( info.type.c_str() ); g_adStaticInit = true; } const UpgradeTemplate *upgradeTemplate = g_adStatic;",
            );
            std::fs::write(mod_gen.join("AutoDepositUpdate_static.cpp"), deposit_cut).expect("write AutoDepositUpdate_static.cpp");
            println!("cargo:rerun-if-changed=upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/MoneyCrateCollide.cpp");
            println!("cargo:rerun-if-changed=upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AutoDepositUpdate.cpp");
            // Special power batch H (differential/l9_powers_h.inc): three function-local `static NameKeyType key_X = NAMEKEY("X")` (in
            // SpecialPowerModule.cpp, CleanupAreaPower.cpp, SpyVisionSpecialPower.cpp) lose the `static`. In the game the name key generator is never
            // reset, so the cached key is the key; the test resets it for every case, and a key cached by an earlier case would name another
            // string. Each edit is asserted to apply once; the edited copies are in mod_gen/ (…_nokey.cpp) and the driver includes those.
            for (file, decl) in [
                ("SpecialPowerModule.cpp", "static NameKeyType key_DeletionUpdate = NAMEKEY(\"DeletionUpdate\");"),
                ("CleanupAreaPower.cpp", "static NameKeyType key_CleanupHazardUpdate = NAMEKEY( \"CleanupHazardUpdate\" );"),
                ("SpyVisionSpecialPower.cpp", "static const NameKeyType key_SpyVisionUpdate = NAMEKEY( \"SpyVisionUpdate\" );"),
            ] {
                let path = up.join("GameEngine/Source/GameLogic/Object/SpecialPower").join(file);
                let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {file}: {e}"));
                assert_eq!(text.matches(decl).count(), 1, "{file}: the static name key must occur once");
                let cut = text.replace(decl, &decl.replacen("static ", "", 1));
                std::fs::write(mod_gen.join(file.replace(".cpp", "_nokey.cpp")), cut).expect("write nokey copy");
                println!("cargo:rerun-if-changed={}", path.display());
            }
            // --- end p1w11-sonnet ---
            // --- p1w12-sonnet ---
            // Update batch M (differential/l9_auto_heal_m.inc): `Object::attemptHealing`, `getSoleHealingBenefactor` and `attemptHealingFromSoleBenefactor`
            // are cut out of Object.cpp verbatim, from the first line of the first to the last line of the last (both markers asserted once), and
            // compiled on the module build's stand-in Object (its two sole-benefactor members, its body and its game logic).
            {
                let object_cpp = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/Object.cpp")).expect("read Object.cpp");
                let first = "void Object::attemptHealing(Real amount, const Object* source)\n{";
                let last = "\t\treturn TRUE;\n\t}\n\n\treturn FALSE;\n\n}\n";
                assert_eq!(object_cpp.matches(first).count(), 1, "Object.cpp: attemptHealing must occur once");
                assert_eq!(object_cpp.matches(last).count(), 1, "Object.cpp: the end of attemptHealingFromSoleBenefactor must occur once");
                let a = object_cpp.find(first).expect("attemptHealing start");
                let b = object_cpp.find(last).expect("sole benefactor end") + last.len();
                assert!(a < b, "Object.cpp: the healing functions");
                std::fs::write(
                    mod_gen.join("ObjectHealing.inc"),
                    format!("/* generated by build.rs: the healing functions of Object, verbatim from Object.cpp */\n{}", &object_cpp[a..b]),
                )
                .expect("write ObjectHealing.inc");
                println!("cargo:rerun-if-changed=upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp");
            }
            // Update batch M (differential/l9_auto_heal_m.inc): AutoHealBehavior.h's field table names `INI::parseParticleSystemTemplate`, which lives in the
            // INI archive and calls `ParticleSystemManager::findTemplate` (a symbol the weapon fixture defines for itself); the two names are replaced by the
            // driver's own copy of that function (`l9ahParsePsTemplate`, on this unit's renamed manager). Each edit is asserted to apply the stated number of
            // times; the edited header is mod_gen/GameLogic/Module/AutoHealBehavior.h, which the module build finds before the upstream one.
            {
                let path = up.join("GameEngine/Include/GameLogic/Module/AutoHealBehavior.h");
                let text = std::fs::read_to_string(&path).expect("read AutoHealBehavior.h");
                let (parse, include) = ("INI::parseParticleSystemTemplate", "#include \"Common/BitFlagsIO.h\"\n");
                assert_eq!(text.matches(parse).count(), 2, "AutoHealBehavior.h: the particle system parser must occur twice");
                assert_eq!(text.matches(include).count(), 1, "AutoHealBehavior.h: the BitFlagsIO include must occur once");
                let cut = text.replace(parse, "l9ahParsePsTemplate").replace(
                    include,
                    &format!("{include}class INI;\nvoid l9ahParsePsTemplate( INI *ini, void *instance, void *store, const void *userData );\n"),
                );
                let dir = mod_gen.join("GameLogic/Module");
                std::fs::create_dir_all(&dir).expect("create mod_gen/GameLogic/Module");
                std::fs::write(dir.join("AutoHealBehavior.h"), cut).expect("write AutoHealBehavior.h copy");
                println!("cargo:rerun-if-changed={}", path.display());
            }
            // --- p1w13-sonnet --- (batch R23, differential/l9_geom_stealth_r23.inc): GrantStealthBehavior.h's field table names
            // `INI::parseParticleSystemTemplate` too; the same replacement (asserted once), its declaration after the header's include guard line
            // (asserted once). The edited header is mod_gen/GameLogic/Module/GrantStealthBehavior.h.
            {
                let path = up.join("GameEngine/Include/GameLogic/Module/GrantStealthBehavior.h");
                let text = std::fs::read_to_string(&path).expect("read GrantStealthBehavior.h");
                let (parse, guard) = ("INI::parseParticleSystemTemplate", "#define __GrantStealthBehavior_H_\n");
                assert_eq!(text.matches(parse).count(), 1, "GrantStealthBehavior.h: the particle system parser must occur once");
                assert_eq!(text.matches(guard).count(), 1, "GrantStealthBehavior.h: the include guard must occur once");
                let cut = text.replace(parse, "l9ahParsePsTemplate").replace(
                    guard,
                    &format!("{guard}class INI;\nvoid l9ahParsePsTemplate( INI *ini, void *instance, void *store, const void *userData );\n"),
                );
                let dir = mod_gen.join("GameLogic/Module");
                std::fs::create_dir_all(&dir).expect("create mod_gen/GameLogic/Module");
                std::fs::write(dir.join("GrantStealthBehavior.h"), cut).expect("write GrantStealthBehavior.h copy");
                println!("cargo:rerun-if-changed={}", path.display());
            }
            // --- end p1w13-sonnet ---
            // Update batch N (differential/l9_flammable_n.inc): the `static` of a function-local name key loses its `static` (the test resets the name key
            // generator for every case, and a key cached by an earlier case would name another string, as in special power batch H). Asserted to apply once;
            // the edited copy is in mod_gen/ (…_nokey.cpp) and the driver includes that.
            {
                let (dir, file, decl) = ("Update", "FlammableUpdate.cpp", "static const NameKeyType key_FireSpreadUpdate = NAMEKEY(\"FireSpreadUpdate\");");
                let path = up.join("GameEngine/Source/GameLogic/Object").join(dir).join(file);
                let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {file}: {e}"));
                assert_eq!(text.matches(decl).count(), 1, "{file}: the static name key must occur once");
                let cut = text.replace(decl, &decl.replacen("static const ", "const ", 1));
                std::fs::write(mod_gen.join(file.replace(".cpp", "_nokey.cpp")), cut).expect("write nokey copy");
                println!("cargo:rerun-if-changed={}", path.display());
            }
            // --- end p1w12-sonnet ---
            // --- p1w13-sonnet ---
            // Update batch O: `Object::kill` (differential/l9_height_die_o.inc) and `Object::getRelationship` (differential/l9_squish_collide_o.inc) are cut out of Object.cpp verbatim, from its first line to the comment that
            // closes it (both asserted once), into mod_gen/ObjectKill.inc; `RadiusDecalTemplate`'s constructor and field parser (differential/
            // l9_dynamic_shroud_o.inc) are cut out of RadiusDecal.cpp verbatim, as the ObjectCreationList fixture does, into mod_gen/radius_decal_template.inc.
            {
                fn cut(text: &str, start: &str, end: &str) -> String {
                    assert_eq!(text.matches(start).count(), 1, "{start:?} must occur once");
                    let a = text.find(start).expect("start");
                    let rest = &text[a..];
                    let b = rest.find(end).unwrap_or_else(|| panic!("{end:?} must follow {start:?}"));
                    format!("{}\n", &rest[..b + end.len()])
                }
                let object_cpp = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/Object.cpp")).expect("read Object.cpp");
                let kill = cut(&object_cpp, "void Object::kill( DamageType damageType, DeathType deathType )\n{", "}  // end kill\n");
                std::fs::write(mod_gen.join("ObjectKill.inc"), format!("/* generated by build.rs: Object::kill, verbatim from Object.cpp */\n{kill}")).expect("write ObjectKill.inc");
                let relationship = cut(&object_cpp, "Relationship Object::getRelationship(const Object *that) const", "\treturn NEUTRAL;\n\n}\n");
                std::fs::write(mod_gen.join("ObjectRelationship.inc"), format!("/* generated by build.rs: Object::getRelationship, verbatim from Object.cpp */\n{relationship}")).expect("write ObjectRelationship.inc");
                let rd = std::fs::read_to_string(up.join("GameEngine/Source/GameClient/RadiusDecal.cpp")).expect("read RadiusDecal.cpp");
                let mut inc = cut(&rd, "RadiusDecalTemplate::RadiusDecalTemplate() : ", "\n}\n");
                inc += &cut(&rd, "/*static*/ void RadiusDecalTemplate::parseRadiusDecalTemplate(", "\tini->initFromINI(store, dataFieldParse);\n}");
                std::fs::write(mod_gen.join("radius_decal_template.inc"), inc).expect("write radius_decal_template.inc");
                println!("cargo:rerun-if-changed=upstream/GeneralsMD/Code/GameEngine/Source/GameClient/RadiusDecal.cpp");
                // SquishCollide.cpp (differential/l9_squish_collide_o.inc): the function-local `static` of its name key loses its `static` (the test resets
                // the name key generator for every case), as FlammableUpdate.cpp's does; asserted once, the edited copy is mod_gen/SquishCollide_nokey.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Collide/SquishCollide.cpp");
                let text = std::fs::read_to_string(&path).expect("read SquishCollide.cpp");
                let decl = "static NameKeyType key_HijackerUpdate = NAMEKEY( \"HijackerUpdate\" );";
                assert_eq!(text.matches(decl).count(), 1, "SquishCollide.cpp: the static name key must occur once");
                std::fs::write(mod_gen.join("SquishCollide_nokey.cpp"), text.replace(decl, &decl.replacen("static ", "", 1))).expect("write SquishCollide_nokey.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // FireSpreadUpdate.cpp (differential/l9_fire_spread_o.inc): the two function-local `static` name keys lose their `static` (asserted twice), and its two
                // calls on another object's FlammableUpdate, `fu->wouldIgnite()` and `fu->tryToIgnite()` (asserted once each), name the driver's functions of the
                // same arguments, which answer for the test's objects (the real FlammableUpdate needs a body, a drawable and a sound manager of its own, and has its
                // own test); the edited copy is mod_gen/FireSpreadUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/FireSpreadUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read FireSpreadUpdate.cpp");
                let decl = "static NameKeyType key_FlammableUpdate = NAMEKEY(\"FlammableUpdate\");";
                assert_eq!(text.matches(decl).count(), 2, "FireSpreadUpdate.cpp: the static name key must occur twice");
                let (would, tryto) = ("fu->wouldIgnite()", "fu->tryToIgnite()");
                assert_eq!(text.matches(would).count(), 1, "FireSpreadUpdate.cpp: wouldIgnite must occur once");
                assert_eq!(text.matches(tryto).count(), 1, "FireSpreadUpdate.cpp: tryToIgnite must occur once");
                let edited = text.replace(decl, &decl.replacen("static ", "", 1)).replace(would, "l9fsWouldIgnite( fu )").replace(tryto, "l9fsTryToIgnite( fu )");
                std::fs::write(mod_gen.join("FireSpreadUpdate_edit.cpp"), edited).expect("write FireSpreadUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // Batch P (differential/l9_active_body_p.inc). ActiveBody.cpp: the seven function-local `static` particle system templates of
                // updateBodyParticleSystems lose their `static` (the test resets the particle system manager for every case, and a template found by an earlier case
                // would be another case's; asserted seven times), into mod_gen/ActiveBody_edit.cpp. DamageFX.cpp: the five functions that take an Object (the
                // constructor, clear, getDamageFXThrottleTime, doDamageFX, getDamageFXList), verbatim, into mod_gen/DamageFX_cut.inc (the Armor build compiles the
                // whole file against its own Object; this unit's class is ModTestDamageFX). BridgeBehavior.cpp: getBridgeBehaviorInterfaceFromObject, verbatim,
                // into mod_gen/BridgeBehaviorFromObject.inc.
                let path = up.join("GameEngine/Source/GameLogic/Object/Body/ActiveBody.cpp");
                let text = std::fs::read_to_string(&path).expect("read ActiveBody.cpp");
                let decl = "static const ParticleSystemTemplate *";
                assert_eq!(text.matches(decl).count(), 7, "ActiveBody.cpp: the static particle system templates must occur seven times");
                std::fs::write(mod_gen.join("ActiveBody_edit.cpp"), text.replace(decl, "const ParticleSystemTemplate *")).expect("write ActiveBody_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                let path = up.join("GameEngine/Source/Common/DamageFX.cpp");
                let text = std::fs::read_to_string(&path).expect("read DamageFX.cpp");
                let fx = cut(&text, "DamageFX::DamageFX()\n{", "\treturn fx;\n}\n");
                std::fs::write(mod_gen.join("DamageFX_cut.inc"), format!("/* generated by build.rs: the Object functions of DamageFX, verbatim from DamageFX.cpp */\n{fx}")).expect("write DamageFX_cut.inc");
                println!("cargo:rerun-if-changed={}", path.display());
                let path = up.join("GameEngine/Source/GameLogic/Object/Behavior/BridgeBehavior.cpp");
                let text = std::fs::read_to_string(&path).expect("read BridgeBehavior.cpp");
                let bridge = cut(&text, "/*static */BridgeBehaviorInterface *BridgeBehavior::getBridgeBehaviorInterfaceFromObject( Object *obj )\n{", "}  // end getBridgeBehaviorInterfaceFromObject\n");
                std::fs::write(mod_gen.join("BridgeBehaviorFromObject.inc"), format!("/* generated by build.rs: BridgeBehavior::getBridgeBehaviorInterfaceFromObject, verbatim from BridgeBehavior.cpp */\n{bridge}")).expect("write BridgeBehaviorFromObject.inc");
                println!("cargo:rerun-if-changed={}", path.display());
                // SlowDeathBehavior.cpp (differential/l9_slow_death_q.inc): the function-local `static` name key of SlavedUpdate loses its `static` (the test resets the
                // name key generator for every case), the SlavedUpdate it finds is an UpdateModule whose onSlaverDie is the driver's l9sdSlaverDie (the real
                // SlavedUpdate needs the whole slaved layer), and its second loop declares its own `update`; each asserted once, into mod_gen/SlowDeathBehavior_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Behavior/SlowDeathBehavior.cpp");
                let text = std::fs::read_to_string(&path).expect("read SlowDeathBehavior.cpp");
                let decl = "static NameKeyType key_SlavedUpdate = NAMEKEY( \"SlavedUpdate\" );";
                assert_eq!(text.matches(decl).count(), 1, "SlowDeathBehavior.cpp: the static name key must occur once");
                let (find, die) = ("SlavedUpdate* slave = (SlavedUpdate*)obj->findUpdateModule( key_SlavedUpdate );", "slave->onSlaverDie( NULL );");
                assert_eq!(text.matches(find).count(), 1, "SlowDeathBehavior.cpp: the SlavedUpdate lookup must occur once");
                assert_eq!(text.matches(die).count(), 1, "SlowDeathBehavior.cpp: onSlaverDie must occur once");
                // The second loop of onDie names the first loop's `update` (Visual C++ 6 let a `for` variable outlive its loop; clang does not): it declares its own.
                let (loop2, loop2_fixed) = ("for (/* UpdateModuleInterface** */ update = obj->getBehaviorModules(); *update; ++update)", "for (BehaviorModule** update = obj->getBehaviorModules(); *update; ++update)");
                assert_eq!(text.matches(loop2).count(), 1, "SlowDeathBehavior.cpp: the second loop must occur once");
                let edited = text.replace(decl, &decl.replacen("static ", "", 1)).replace(find, "UpdateModule* slave = obj->findUpdateModule( key_SlavedUpdate );").replace(die, "l9sdSlaverDie( slave );").replace(loop2, loop2_fixed);
                std::fs::write(mod_gen.join("SlowDeathBehavior_edit.cpp"), edited).expect("write SlowDeathBehavior_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // UndeadBody.cpp (differential/l9_active_body_p.inc): the second loop of startSecondLife names the first loop's `update` (Visual C++ 6 let a `for`
                // variable outlive its loop; clang does not): it declares its own, asserted once, into mod_gen/UndeadBody_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Body/UndeadBody.cpp");
                let text = std::fs::read_to_string(&path).expect("read UndeadBody.cpp");
                let (loop2, loop2_fixed) = ("for( update = getObject()->getBehaviorModules(); *update; ++update)", "for( BehaviorModule** update = getObject()->getBehaviorModules(); *update; ++update)");
                assert_eq!(text.matches(loop2).count(), 1, "UndeadBody.cpp: the second loop must occur once");
                std::fs::write(mod_gen.join("UndeadBody_edit.cpp"), text.replace(loop2, loop2_fixed)).expect("write UndeadBody_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // OpenContain.cpp (differential/l9_open_contain_r.inc): processDamageToContained loops `while( *it )` and so dereferences list::end() once the last rider is
                // done (undefined: the sentinel holds no value). The edit reads that as the loop ending, which is what the shipped game does, asserted once, into
                // mod_gen/OpenContain_edit.cpp; the Rust side records the undefined read (`Undefined::EndIteratorDereferenced`) and the test accepts that one.
                let path = up.join("GameEngine/Source/GameLogic/Object/Contain/OpenContain.cpp");
                let text = std::fs::read_to_string(&path).expect("read OpenContain.cpp");
                let (wh, wh_fixed) = ("while( *it )", "while( it != items->end() )");
                assert_eq!(text.matches(wh).count(), 1, "OpenContain.cpp: the processDamageToContained loop must occur once");
                std::fs::write(mod_gen.join("OpenContain_edit.cpp"), text.replace(wh, wh_fixed)).expect("write OpenContain_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R2: TransportContain.cpp loops `while( *it )` twice (letRidersUpgradeWeaponSet over riderList, update over items), with the same read of
                // list::end() after the last rider; both end there, asserted once each and in that order, into mod_gen/TransportContain_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Contain/TransportContain.cpp");
                let text = std::fs::read_to_string(&path).expect("read TransportContain.cpp");
                assert_eq!(text.matches(wh).count(), 2, "TransportContain.cpp: the two `while( *it )` loops must occur twice");
                let first = text.find(wh).expect("first loop");
                let second = first + wh.len() + text[first + wh.len()..].find(wh).expect("second loop");
                assert!(text[..first].contains("riderList = getContainedItemsList()") && text[first..second].contains("items = contain->getContainedItemsList()"),
                    "TransportContain.cpp: the loops are letRidersUpgradeWeaponSet's then update's");
                let mut edited = String::new();
                edited.push_str(&text[..first]);
                edited.push_str("while( it != riderList->end() )");
                edited.push_str(&text[first + wh.len()..second]);
                edited.push_str("while( it != items->end() )");
                edited.push_str(&text[second + wh.len()..]);
                // and `hisLocomotor == FALSE` (isSpecificRiderFreeToExit) compares a pointer with MSVC's FALSE (0); clang's FALSE is a bool, so the test is spelled
                // with the value it has under MSVC
                let (cmp, cmp_fixed) = ("if( hisLocomotor == FALSE )", "if( hisLocomotor == 0 )");
                assert_eq!(edited.matches(cmp).count(), 1, "TransportContain.cpp: the locomotor test must occur once");
                let edited = edited.replace(cmp, cmp_fixed);
                std::fs::write(mod_gen.join("TransportContain_edit.cpp"), edited).expect("write TransportContain_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R5: RiderChangeContain.h forward-declares three enums (an MSVC extension clang refuses). WeaponSetType and LocomotorSetType are
                // defined before it here (WeaponSetType.h, the AIUpdate.h stand-in); ObjectStatusType is defined nowhere upstream (the real enum is
                // ObjectStatusTypes), so it becomes an empty enum of int, the size MSVC gives the forward declaration. Into mod_gen/RiderChangeContain_edit.h,
                // which the driver includes first (the same include guard).
                let path = up.join("GameEngine/Include/GameLogic/Module/RiderChangeContain.h");
                let text = std::fs::read_to_string(&path).expect("read RiderChangeContain.h");
                let fwd = "enum WeaponSetType;\r\nenum ObjectStatusType;\r\nenum LocomotorSetType;";
                let fwd_lf = "enum WeaponSetType;\nenum ObjectStatusType;\nenum LocomotorSetType;";
                let fixed = "enum ObjectStatusType : int {};   /* build.rs: see the batch R5 note */";
                let edited = if text.matches(fwd).count() == 1 { text.replace(fwd, fixed) } else {
                    assert_eq!(text.matches(fwd_lf).count(), 1, "RiderChangeContain.h: the three forward enum declarations must occur once");
                    text.replace(fwd_lf, fixed)
                };
                std::fs::write(mod_gen.join("RiderChangeContain_edit.h"), edited).expect("write RiderChangeContain_edit.h");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R6: OverlordContain.cpp's clientVisibleContainedFlashAsSelected loops `while( *it )` over the list (the read of end() after the
                // last one); it ends there, asserted once, into mod_gen/OverlordContain_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Contain/OverlordContain.cpp");
                let text = std::fs::read_to_string(&path).expect("read OverlordContain.cpp");
                assert_eq!(text.matches(wh).count(), 1, "OverlordContain.cpp: the `while( *it )` loop must occur once");
                assert!(text.contains("const ContainedItemsList* items = TransportContain::getContainedItemsList();"), "OverlordContain.cpp: the loop is over items");
                std::fs::write(mod_gen.join("OverlordContain_edit.cpp"), text.replace(wh, "while( it != items->end() )")).expect("write OverlordContain_edit.cpp");
                // batch R9: MobNexusContain.cpp reads `end()` twice: `if( *it )` of `begin()` of an empty special zero-slot rider's list
                // (isValidContainerFor) and update's `while( *it )` after the last rider; both stop at `end()` (the Rust side records
                // `Undefined::EndIteratorDereferenced`), asserted once each, into mod_gen/MobNexusContain_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Contain/MobNexusContain.cpp");
                let text = std::fs::read_to_string(&path).expect("read MobNexusContain.cpp");
                let iff = "if( *it )";
                assert_eq!(text.matches(iff).count(), 1, "MobNexusContain.cpp: `if( *it )` must occur once");
                assert_eq!(text.matches(wh).count(), 1, "MobNexusContain.cpp: `while( *it )` must occur once");
                let edited = text.replace(iff, "if( it != items->end() && *it )").replace(wh, "while( it != items->end() )");
                std::fs::write(mod_gen.join("MobNexusContain_edit.cpp"), edited).expect("write MobNexusContain_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R12: GarrisonContain.cpp's loadStationGarrisonPoints names the first loop's `t` in its second loop (Visual C++ 6 let a `for`
                // variable outlive its loop; clang does not): it declares its own, asserted once, into mod_gen/GarrisonContain_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Contain/GarrisonContain.cpp");
                let text = std::fs::read_to_string(&path).expect("read GarrisonContain.cpp");
                let (loop2, loop2_fixed) = ("for( t = 0; t < count; ++t )", "for( int t = 0; t < count; ++t )");
                assert_eq!(text.matches(loop2).count(), 1, "GarrisonContain.cpp: the second loop must occur once");
                std::fs::write(mod_gen.join("GarrisonContain_edit.cpp"), text.replace(loop2, loop2_fixed)).expect("write GarrisonContain_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R27 (differential/l9_stealth_detector_r27.inc): StealthDetectorUpdate.cpp's field table names `INI::parseParticleSystemTemplate`
                // four times, as AutoHealBehavior.h does; the same replacement (asserted four times), its declaration after the last include (asserted
                // once), into mod_gen/StealthDetectorUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/StealthDetectorUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read StealthDetectorUpdate.cpp");
                let (parse, include) = ("INI::parseParticleSystemTemplate", "#include \"Common/Player.h\"\n");
                assert_eq!(text.matches(parse).count(), 4, "StealthDetectorUpdate.cpp: the particle system parser must occur four times");
                assert_eq!(text.matches(include).count(), 1, "StealthDetectorUpdate.cpp: the Player.h include must occur once");
                let cut = text.replace(parse, "l9ahParsePsTemplate").replace(
                    include,
                    &format!("{include}class INI;\nvoid l9ahParsePsTemplate( INI *ini, void *instance, void *store, const void *userData );\n"),
                );
                std::fs::write(mod_gen.join("StealthDetectorUpdate_edit.cpp"), cut).expect("write StealthDetectorUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R30 (differential/l9_bonefx_r30.inc): BoneFXUpdate.cpp parses its particle systems with `ini->parseParticleSystemTemplate`; the same
                // replacement (asserted once), its declaration after the BoneFXDamage.h include (asserted once), into mod_gen/BoneFXUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/BoneFXUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read BoneFXUpdate.cpp");
                let (parse, include) = ("ini->parseParticleSystemTemplate", "#include \"GameLogic/Module/BoneFXDamage.h\"\n");
                assert_eq!(text.matches(parse).count(), 1, "BoneFXUpdate.cpp: the particle system parser must occur once");
                assert_eq!(text.matches(include).count(), 1, "BoneFXUpdate.cpp: the BoneFXDamage.h include must occur once");
                let cut = text.replace(parse, "l9ahParsePsTemplate").replace(
                    include,
                    &format!("{include}class INI;\nvoid l9ahParsePsTemplate( INI *ini, void *instance, void *store, const void *userData );\n"),
                );
                // StructureCollapseUpdate's driver has a stand-in BoneFXUpdate, so the driver compiles these classes under R30 names: the header once more,
                // as mod_gen/BoneFXUpdate_r30.h (its `#pragma once` and include guard removed, each asserted once), which the edited .cpp includes.
                let header = "#include \"GameLogic/Module/BoneFXUpdate.h\"\n";
                assert_eq!(cut.matches(header).count(), 1, "BoneFXUpdate.cpp: the BoneFXUpdate.h include must occur once");
                let cut = cut.replace(header, "#include \"BoneFXUpdate_r30.h\"\n");
                // and the function-local `static` name key loses its `static` (asserted once), as in FlammableUpdate.cpp: the test resets the name key
                // generator for every case. BoneFXDamage.cpp's two the same (asserted twice), into mod_gen/BoneFXDamage_edit.cpp.
                let key = "static NameKeyType key_BoneFXDamage = NAMEKEY(\"BoneFXDamage\");";
                assert_eq!(cut.matches(key).count(), 1, "BoneFXUpdate.cpp: the BoneFXDamage name key must occur once");
                let cut = cut.replace(key, &key["static ".len()..]);
                let dpath = up.join("GameEngine/Source/GameLogic/Object/Damage/BoneFXDamage.cpp");
                let dtext = std::fs::read_to_string(&dpath).expect("read BoneFXDamage.cpp");
                let dkey = "static NameKeyType key_BoneFXUpdate = NAMEKEY(\"BoneFXUpdate\");";
                assert_eq!(dtext.matches(dkey).count(), 2, "BoneFXDamage.cpp: the BoneFXUpdate name key must occur twice");
                std::fs::write(mod_gen.join("BoneFXDamage_edit.cpp"), dtext.replace(dkey, &dkey["static ".len()..])).expect("write BoneFXDamage_edit.cpp");
                println!("cargo:rerun-if-changed={}", dpath.display());
                std::fs::write(mod_gen.join("BoneFXUpdate_edit.cpp"), cut).expect("write BoneFXUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                let hpath = up.join("GameEngine/Include/GameLogic/Module/BoneFXUpdate.h");
                let htext = std::fs::read_to_string(&hpath).expect("read BoneFXUpdate.h");
                let (once, guard) = ("#pragma once\n", "#ifndef __BONEFXUPDATE_H_\n#define __BONEFXUPDATE_H_\n");
                assert_eq!(htext.matches(once).count(), 1, "BoneFXUpdate.h: #pragma once must occur once");
                assert_eq!(htext.matches(guard).count(), 1, "BoneFXUpdate.h: the include guard must occur once");
                let hcut = htext.replace(once, "").replace(guard, "#ifndef __BONEFXUPDATE_R30_H_\n#define __BONEFXUPDATE_R30_H_\n");
                std::fs::write(mod_gen.join("BoneFXUpdate_r30.h"), hcut).expect("write BoneFXUpdate_r30.h");
                println!("cargo:rerun-if-changed={}", hpath.display());
                // batch R31 (differential/l9_transition_r31.inc): TransitionDamageFX.cpp parses its particle systems with
                // `ini->parseParticleSystemTemplate`; the same replacement (asserted once), its declaration after the module header's include (asserted
                // once), into mod_gen/TransitionDamageFX_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Damage/TransitionDamageFX.cpp");
                let text = std::fs::read_to_string(&path).expect("read TransitionDamageFX.cpp");
                let (parse, include) = ("ini->parseParticleSystemTemplate", "#include \"GameLogic/Module/TransitionDamageFX.h\"\n");
                assert_eq!(text.matches(parse).count(), 1, "TransitionDamageFX.cpp: the particle system parser must occur once");
                assert_eq!(text.matches(include).count(), 1, "TransitionDamageFX.cpp: the module header include must occur once");
                let cut = text.replace(parse, "l9ahParsePsTemplate").replace(
                    include,
                    &format!("{include}class INI;\nvoid l9ahParsePsTemplate( INI *ini, void *instance, void *store, const void *userData );\n"),
                );
                std::fs::write(mod_gen.join("TransitionDamageFX_edit.cpp"), cut).expect("write TransitionDamageFX_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R32 (differential/l9_firestorm_r32.inc): FirestormDynamicGeometryInfoUpdate.cpp's field table names
                // `INI::parseParticleSystemTemplate` sixteen times; the same replacement (asserted sixteen times), its declaration after the module header's
                // include (asserted once), into mod_gen/FirestormDynamicGeometryInfoUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/FirestormDynamicGeometryInfoUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read FirestormDynamicGeometryInfoUpdate.cpp");
                let (parse, include) = ("INI::parseParticleSystemTemplate", "#include \"GameLogic/Module/FirestormDynamicGeometryInfoUpdate.h\"\n");
                assert_eq!(text.matches(parse).count(), 16, "FirestormDynamicGeometryInfoUpdate.cpp: the particle system parser must occur sixteen times");
                assert_eq!(text.matches(include).count(), 1, "FirestormDynamicGeometryInfoUpdate.cpp: the module header include must occur once");
                let cut = text.replace(parse, "l9ahParsePsTemplate").replace(
                    include,
                    &format!("{include}class INI;\nvoid l9ahParsePsTemplate( INI *ini, void *instance, void *store, const void *userData );\n"),
                );
                std::fs::write(mod_gen.join("FirestormDynamicGeometryInfoUpdate_edit.cpp"), cut).expect("write FirestormDynamicGeometryInfoUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R33 (differential/l9_emp_r33.inc): EMPUpdate.h's field tables name `INI::parseParticleSystemTemplate` twice; the same
                // replacement (asserted twice), its declaration after the include guard line (asserted once), into mod_gen/GameLogic/Module/EMPUpdate.h.
                let path = up.join("GameEngine/Include/GameLogic/Module/EMPUpdate.h");
                let text = std::fs::read_to_string(&path).expect("read EMPUpdate.h");
                let (parse, guard) = ("INI::parseParticleSystemTemplate", "#define __EMPUPDATE_H_\n");
                assert_eq!(text.matches(parse).count(), 2, "EMPUpdate.h: the particle system parser must occur twice");
                assert_eq!(text.matches(guard).count(), 1, "EMPUpdate.h: the include guard must occur once");
                let cut = text
                    .replace(parse, "l9ahParsePsTemplate")
                    .replace(guard, &format!("{guard}class INI;\nvoid l9ahParsePsTemplate( INI *ini, void *instance, void *store, const void *userData );\n"));
                let dir = mod_gen.join("GameLogic/Module");
                std::fs::create_dir_all(&dir).expect("create mod_gen/GameLogic/Module");
                std::fs::write(dir.join("EMPUpdate.h"), cut).expect("write EMPUpdate.h copy");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R34 (differential/l9_tensile_r34.inc): TensileFormationUpdate.cpp's getTFU keeps its name key in a function-local `static`,
                // which the test's per-case reset of the name key generator would leave stale; the `static` dropped (asserted once), into
                // mod_gen/TensileFormationUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/TensileFormationUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read TensileFormationUpdate.cpp");
                let key = "static NameKeyType theTFUKey";
                assert_eq!(text.matches(key).count(), 1, "TensileFormationUpdate.cpp: getTFU's static name key must occur once");
                let cut = text.replace(key, "NameKeyType theTFUKey");
                std::fs::write(mod_gen.join("TensileFormationUpdate_edit.cpp"), cut).expect("write TensileFormationUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R36 (differential/l9_structure_topple_r36.inc): StructureToppleUpdate.cpp's applyCrushingDamage reads its loop variable `j`
                // after the loop (MSVC's old `for` scope); the declaration hoisted before the loop (asserted once), which changes nothing else,
                // into mod_gen/StructureToppleUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/StructureToppleUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read StructureToppleUpdate.cpp");
                let loop_head = "for (Real j = m_lastCrushedLocation;";
                assert_eq!(text.matches(loop_head).count(), 1, "StructureToppleUpdate.cpp: the crushing loop must occur once");
                let cut = text.replace(loop_head, "Real j; for (j = m_lastCrushedLocation;");
                std::fs::write(mod_gen.join("StructureToppleUpdate_edit.cpp"), cut).expect("write StructureToppleUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R39 (differential/l9_sticky_bomb_r39.inc): StickyBombUpdate.cpp's initStickyBomb keeps LifetimeUpdate's name key in a
                // function-local `static`, which the test's per-case reset of the name key generator would leave stale; the `static` dropped
                // (asserted once), into mod_gen/StickyBombUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/StickyBombUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read StickyBombUpdate.cpp");
                let key = "static NameKeyType key_LifetimeUpdate";
                assert_eq!(text.matches(key).count(), 1, "StickyBombUpdate.cpp: initStickyBomb's static name key must occur once");
                let cut = text.replace(key, "NameKeyType key_LifetimeUpdate");
                std::fs::write(mod_gen.join("StickyBombUpdate_edit.cpp"), cut).expect("write StickyBombUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // The header is already in the unit (RebuildHoleBehavior's driver, batch R15, stands in for two of the bomb's methods), so the
                // driver compiles the class under another name from a copy of the header whose include guard is renamed (asserted twice).
                let path = up.join("GameEngine/Include/GameLogic/Module/StickyBombUpdate.h");
                let text = std::fs::read_to_string(&path).expect("read StickyBombUpdate.h");
                let guard = "__STICK_BOMB_UPDATE_H";
                assert_eq!(text.matches(guard).count(), 3, "StickyBombUpdate.h: the include guard must occur three times (two lines and the end comment)");
                std::fs::write(mod_gen.join("StickyBombUpdate_r39.h"), text.replace(guard, "__STICK_BOMB_UPDATE_H_R39")).expect("write StickyBombUpdate_r39.h");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R40 (differential/l9_minefield_r40.inc): MinefieldBehavior.cpp's update keeps AutoHealBehavior's name key in a
                // function-local `static`, which the test's per-case reset of the name key generator would leave stale; the `static` dropped
                // (asserted once), into mod_gen/MinefieldBehavior_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Behavior/MinefieldBehavior.cpp");
                let text = std::fs::read_to_string(&path).expect("read MinefieldBehavior.cpp");
                let key = "static const NameKeyType key_AutoHealBehavior";
                assert_eq!(text.matches(key).count(), 1, "MinefieldBehavior.cpp: update's static name key must occur once");
                let cut = text.replace(key, "const NameKeyType key_AutoHealBehavior");
                std::fs::write(mod_gen.join("MinefieldBehavior_edit.cpp"), cut).expect("write MinefieldBehavior_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R43 (differential/l9_repair_dock_r43.inc): DockUpdate.cpp is written for MSVC 6's for-loop scoping, under which a
                // variable declared in a `for` lives on after it; clang's standard scoping rejects the three later uses. Each use gets its own
                // declaration (asserted once each, the two xfer loops together), into mod_gen/DockUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/DockUpdate/DockUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read DockUpdate.cpp");
                let (a, b) = ("\t\tpositionIndex = m_approachPositionOwners.size() - 1;", "\tfor( vectorIndex = 0; vectorIndex < vectorSize; ++vectorIndex )");
                assert_eq!((text.matches(a).count(), text.matches(b).count()), (1, 2), "DockUpdate.cpp: the MSVC for-scope uses must occur once and twice");
                let cut = text.replace(a, "\t\tInt positionIndex = m_approachPositionOwners.size() - 1;").replace(b, "\tfor( Int vectorIndex = 0; vectorIndex < vectorSize; ++vectorIndex )");
                std::fs::write(mod_gen.join("DockUpdate_edit.cpp"), cut).expect("write DockUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R45 (differential/l9_exits_r45.inc): SpawnPointProductionExitUpdate.cpp has the same MSVC 6 scoping: exitObjectViaDoor
                // reads its loop's index after the loop and initializeBonePositions reuses its first loop's. Both are declared before their loops
                // (asserted once each), into mod_gen/SpawnPointProductionExitUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/SpawnPointProductionExitUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read SpawnPointProductionExitUpdate.cpp");
                let (a, b) = ("\t\tfor( Int positionIndex = 0; positionIndex < m_spawnPointCount; positionIndex++ )", "\tfor( Int matrixIndex = 0; matrixIndex < MAX_SPAWN_POINTS; matrixIndex++ )");
                assert_eq!((text.matches(a).count(), text.matches(b).count()), (1, 1), "SpawnPointProductionExitUpdate.cpp: the MSVC for-scope loops must occur once each");
                let cut = text
                    .replace(a, "\t\tInt positionIndex;\n\t\tfor( positionIndex = 0; positionIndex < m_spawnPointCount; positionIndex++ )")
                    .replace(b, "\tInt matrixIndex;\n\tfor( matrixIndex = 0; matrixIndex < MAX_SPAWN_POINTS; matrixIndex++ )");
                std::fs::write(mod_gen.join("SpawnPointProductionExitUpdate_edit.cpp"), cut).expect("write SpawnPointProductionExitUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R48 (differential/l9_heli_death_r48.inc): HelicopterSlowDeathUpdate.cpp's field table names `INI::parseParticleSystemTemplate`
                // once, as AutoHealBehavior.h does; the same replacement (asserted once), its declaration after the PhysicsUpdate.h include (asserted
                // once), into mod_gen/HelicopterSlowDeathUpdate_edit.cpp.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/HelicopterSlowDeathUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read HelicopterSlowDeathUpdate.cpp");
                let (parse, include) = ("INI::parseParticleSystemTemplate", "#include \"GameLogic/Module/PhysicsUpdate.h\"\n");
                assert_eq!(text.matches(parse).count(), 1, "HelicopterSlowDeathUpdate.cpp: the particle system parser must occur once");
                assert_eq!(text.matches(include).count(), 1, "HelicopterSlowDeathUpdate.cpp: the PhysicsUpdate.h include must occur once");
                let cut = text.replace(parse, "l9ahParsePsTemplate").replace(
                    include,
                    &format!("{include}class INI;\nvoid l9ahParsePsTemplate( INI *ini, void *instance, void *store, const void *userData );\n"),
                );
                std::fs::write(mod_gen.join("HelicopterSlowDeathUpdate_edit.cpp"), cut).expect("write HelicopterSlowDeathUpdate_edit.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R50 (differential/l9_physics_r50.inc): PhysicsUpdate.cpp, compiled a second time in the unit (whose PhysicsBehavior is the
                // module build's stand-in, a block of memory other drivers define methods of) under R50 names, by macros in the driver: the header once
                // more, as mod_gen/PhysicsUpdate_r50.h (its `#pragma once` and include guard renamed, each asserted once); the .cpp, into
                // mod_gen/PhysicsUpdate_r50.cpp, with its include of the header and the AI's `processCollision(this, other)` (the AI was declared
                // with the stand-in's type for the argument) edited, each asserted once.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/PhysicsUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read PhysicsUpdate.cpp");
                let edits = [
                    ("#include \"GameLogic/Module/PhysicsUpdate.h\"", "#include \"PhysicsUpdate_r50.h\""),
                    ("ai->processCollision(this, other)", "ai->processCollision((R50OrigPhysics *)(void *)this, other)"),
                ];
                let mut cut = text.clone();
                for (from, to) in edits {
                    assert_eq!(cut.matches(from).count(), 1, "PhysicsUpdate.cpp: `{from}` must occur once");
                    cut = cut.replace(from, to);
                }
                std::fs::write(mod_gen.join("PhysicsUpdate_r50.cpp"), cut).expect("write PhysicsUpdate_r50.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                let hpath = up.join("GameEngine/Include/GameLogic/Module/PhysicsUpdate.h");
                let htext = std::fs::read_to_string(&hpath).expect("read PhysicsUpdate.h");
                let (once, guard) = ("#pragma once\n", "#ifndef _PHYSICSUPDATE_H_\n#define _PHYSICSUPDATE_H_\n");
                assert_eq!(htext.matches(once).count(), 1, "PhysicsUpdate.h: #pragma once must occur once");
                assert_eq!(htext.matches(guard).count(), 1, "PhysicsUpdate.h: the include guard must occur once");
                let hcut = htext.replace(once, "").replace(guard, "#ifndef _PHYSICSUPDATE_R50_H_\n#define _PHYSICSUPDATE_R50_H_\n");
                std::fs::write(mod_gen.join("PhysicsUpdate_r50.h"), hcut).expect("write PhysicsUpdate_r50.h");
                println!("cargo:rerun-if-changed={}", hpath.display());
                // and `Object::canCrushOrSquish`, cut out of Object.cpp verbatim (from its first line to the end of the function, asserted once), into
                // mod_gen/ObjectCrush.inc, as batch O cuts `Object::kill`.
                let object_cpp = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/Object.cpp")).expect("read Object.cpp");
                let start = "Bool Object::canCrushOrSquish(Object *otherObj, CrushSquishTestType testType ) const\n{";
                let end = "\t\t\treturn true;\n\t\t}\n\t}\n\t\t\n\treturn false;\n}\n";
                assert_eq!(object_cpp.matches(start).count(), 1, "Object.cpp: canCrushOrSquish must occur once");
                let at = object_cpp.find(start).expect("canCrushOrSquish");
                let rest = &object_cpp[at..];
                let stop = rest.find(end).expect("the end of canCrushOrSquish") + end.len();
                std::fs::write(mod_gen.join("ObjectCrush.inc"), format!("/* generated by build.rs: Object::canCrushOrSquish, verbatim from Object.cpp */\n{}", &rest[..stop]))
                    .expect("write ObjectCrush.inc");
                // batch R52 (differential/l9_countermeasures_r52.inc): CountermeasuresBehavior.cpp, into mod_gen/CountermeasuresBehavior_r52.cpp, with
                // the test of the end iterator as a pointer (`if( it )`: the shipped std::vector's iterator is one, null while the vector has no
                // storage, which no other library's is) made the test of the storage it stands for, asserted once.
                let path = up.join("GameEngine/Source/GameLogic/Object/Behavior/CountermeasuresBehavior.cpp");
                let text = std::fs::read_to_string(&path).expect("read CountermeasuresBehavior.cpp");
                let from = "\tif( it )\n";
                assert_eq!(text.matches(from).count(), 1, "CountermeasuresBehavior.cpp: `if( it )` must occur once");
                let cut = text.replace(from, "\tif( m_counterMeasures.capacity() != 0 )   /* build.rs: was `if( it )` */\n");
                std::fs::write(mod_gen.join("CountermeasuresBehavior_r52.cpp"), cut).expect("write CountermeasuresBehavior_r52.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
                // batch R56 (differential/l9_cleanup_hazard_r56.inc): CleanupHazardUpdate.cpp, into mod_gen/CleanupHazardUpdate_r56.cpp, with
                // setCleanupAreaParameters defined as a member of the driver's subclass R56Cleanup (the class's own name is special power batch H's
                // stand-in, the line CleanupAreaPower's test reads), its body unchanged; asserted once.
                let path = up.join("GameEngine/Source/GameLogic/Object/Update/CleanupHazardUpdate.cpp");
                let text = std::fs::read_to_string(&path).expect("read CleanupHazardUpdate.cpp");
                let from = "void CleanupHazardUpdate::setCleanupAreaParameters( const Coord3D *pos, Real range )\n";
                assert_eq!(text.matches(from).count(), 1, "CleanupHazardUpdate.cpp: setCleanupAreaParameters must occur once");
                let cut = text.replace(from, "void R56Cleanup::r56SetCleanupAreaParameters( const Coord3D *pos, Real range )   /* build.rs: was CleanupHazardUpdate:: */\n");
                std::fs::write(mod_gen.join("CleanupHazardUpdate_r56.cpp"), cut).expect("write CleanupHazardUpdate_r56.cpp");
                println!("cargo:rerun-if-changed={}", path.display());
            }
            // --- end p1w13-sonnet ---
            let mut md = cc::Build::new();
            base(&mut md);
            md.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&shim_mod)
                .include(dif.join("shim_upgrade"))
                .include(&mod_gen)
                .include(&flags_gen)
                .include(dif.join("shim_flags"))
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .include(&dif)
                .flag("-include")
                .flag(shim_mod.join("module_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_module.cpp"));
            md.compile("upstream_module");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/Module.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/ModuleFactory.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w3-modules ---
        // --- p1l9-creates ---
        // The layer-9 create modules (differential/wrap_create_modules.cpp): the seven upstream files of
        // GameLogic/Object/Create, unchanged, #included by the wrapper inside the forced prelude
        // differential/shim_creates/create_prelude.h (the renames), with the layer-5 chain of the module
        // build on the same include path. CreateModule.cpp is not compiled here: it is linked from
        // upstream_module (wrap_module.cpp), whose symbols this build reuses.
        {
            let shim_cr = dif.join("shim_creates");
            let mut cr = cc::Build::new();
            base(&mut cr);
            cr.compiler("clang++")
                .flag("-fms-extensions")
                .flag("-fdelayed-template-parsing")
                .include(&shim_cr)
                .include(dif.join("shim_module"))
                .include(dif.join("shim_upgrade"))
                .include(&flags_gen)
                .include(dif.join("shim_flags"))
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .include(&dif)
                .flag("-include")
                .flag(shim_cr.join("create_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_create_modules.cpp"));
            cr.compile("upstream_create_modules");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create",
                "differential/wrap_create_modules.cpp",
                "differential/shim_creates",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1l9-creates ---
        // --- p1w4-polygon ---
        // PolygonTrigger.cpp unchanged (differential/wrap_polygon_trigger.cpp), on
        // differential/shim_polygon (the precompiled header, GlobalData's water
        // extents, MapObject's MAP_XY_FACTOR, TerrainLogic's ground height), the
        // Dict of shim_chunk (with the accessors this block adds) and the real
        // headers; DataChunkInput, NameKeyGenerator and the Xfer classes are the
        // ones of the chunk and Xfer builds, linked after this archive.
        // The object chunk reader: ParseObjectDataChunk is `static` in
        // GameClient/MapUtil.cpp, a GUI file that needs the whole client, so its
        // text is cut out verbatim (both markers asserted once) into
        // OUT_DIR/map_gen/map_object_parse.inc, which differential/wrap_map_object.cpp
        // includes between its stand-ins.
        {
            let map_gen = out_dir.join("map_gen");
            std::fs::create_dir_all(&map_gen).expect("create map_gen");
            let mu = std::fs::read_to_string(up.join("GameEngine/Source/GameClient/MapUtil.cpp")).expect("read MapUtil.cpp");
            let (first, last) = (
                "static Bool ParseObjectDataChunk(DataChunkInput &file, DataChunkInfo *info, void *userData)\n",
                "static Bool ParseObjectsDataChunk(",
            );
            assert_eq!(mu.matches(first).count(), 1, "MapUtil.cpp: ParseObjectDataChunk must occur once");
            assert_eq!(mu.matches(last).count(), 1, "MapUtil.cpp: ParseObjectsDataChunk must occur once");
            let a = mu.find(first).expect("parser start");
            let b = mu.find(last).expect("parser end");
            assert!(a < b, "MapUtil.cpp: ParseObjectDataChunk before ParseObjectsDataChunk");
            std::fs::write(
                map_gen.join("map_object_parse.inc"),
                format!("/* generated by build.rs: ParseObjectDataChunk, verbatim from MapUtil.cpp */\n{}", &mu[a..b]),
            )
            .expect("write map_object_parse.inc");

            // PolygonTrigger.cpp: removePolygonTrigger declares its loop variable in
            // the for, and uses it after the loop (MSVC's pre-standard scope, which
            // g++ rejects; the use is in DEBUG_ASSERTCRASH and `if (pTrig)`). A copy
            // with that declaration moved before the loop, as INI.cpp's copy below
            // (the upstream file is never edited). The text must occur once.
            let pt_src = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Map/PolygonTrigger.cpp"))
                .expect("read PolygonTrigger.cpp");
            let pt_old = "\tfor (PolygonTrigger *pTrig=getFirstPolygonTrigger(); pTrig; pTrig = pTrig->getNext()) {\n\t\tif (pTrig==pTrigger) break;\n";
            let pt_new = "\tPolygonTrigger *pTrig;\n\tfor (pTrig=getFirstPolygonTrigger(); pTrig; pTrig = pTrig->getNext()) {\n\t\tif (pTrig==pTrigger) break;\n";
            assert_eq!(pt_src.matches(pt_old).count(), 1, "PolygonTrigger.cpp: removePolygonTrigger's loop must occur once");
            let poly_gen = out_dir.join("poly_gen");
            std::fs::create_dir_all(&poly_gen).expect("create poly_gen");
            std::fs::write(poly_gen.join("PolygonTrigger.cpp"), pt_src.replacen(pt_old, pt_new, 1)).expect("write PolygonTrigger.cpp");

            let shim_poly = dif.join("shim_polygon");
            let shim_chunk = dif.join("shim_chunk");
            let mut pt = cc::Build::new();
            base(&mut pt);
            // `this` is not assumed non-null: MemoryPoolObject::deleteInstance's
            // `if (this)` (GameMemory.h, as MSVC compiles it) is kept, so that
            // deleteTriggers on an empty chain is a no-op
            pt.flag("-fno-delete-null-pointer-checks");
            pt.include(&poly_gen)
                .include(&shim_poly)
                .include(&shim_chunk)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_polygon_trigger.cpp"));
            pt.compile("upstream_polygon_trigger");

            let mut mo = cc::Build::new();
            base(&mut mo);
            mo.include(&shim_chunk)
                .include(&dif) // well_known_keys_list.h
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(up.join("Libraries/Source/Compression"))
                .include(&map_gen)
                .flag("-include")
                .flag(shim_chunk.join("chunk_prelude.h").to_str().expect("UTF-8 path"))
                .file(dif.join("wrap_map_object.cpp"));
            mo.compile("upstream_map_object");

            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Map/PolygonTrigger.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w4-polygon ---

        // --- p1w6-trackers ---
        // ObjectTypes (differential/wrap_object_types.cpp): ObjectTypes.cpp
        // #included by the wrapper, unchanged, on differential/shim_object_types
        // (Player::canBuild, ThingFactory::findTemplate and ThingTemplate::getName,
        // renamed ObjTypes*). Linked ahead of upstream_xfer, whose XferSave and
        // XferLoad it calls.
        {
            let shim_ot = dif.join("shim_object_types");
            let mut ot = cc::Build::new();
            base(&mut ot);
            ot.include(&shim_ot)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_object_types.cpp"));
            ot.compile("upstream_object_types");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectTypes.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/ObjectTypes.h",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // ExperienceTracker (differential/wrap_experience_tracker.cpp):
        // ExperienceTracker.cpp #included by the wrapper, unchanged, on
        // differential/shim_experience (Object, ThingTemplate, GameLogic,
        // renamed ExpTrack*). Linked ahead of upstream_xfer, whose XferSave
        // and XferLoad it calls.
        {
            let shim_ex = dif.join("shim_experience");
            let mut ex = cc::Build::new();
            base(&mut ex);
            ex.include(&shim_ex)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_experience_tracker.cpp"));
            ex.compile("upstream_experience_tracker");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ExperienceTracker.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/ExperienceTracker.h",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // FiringTracker (differential/wrap_firing_tracker.cpp): FiringTracker.cpp
        // #included by the wrapper, unchanged, on differential/shim_firing (the
        // module base UpdateModule, Object, Weapon, ThingTemplate, GameLogic,
        // AudioManager, AudioEventRTS, ModelConditionFlags, DisabledMaskType,
        // renamed FireTest*). Linked ahead of upstream_xfer.
        {
            let shim_fi = dif.join("shim_firing");
            let mut fi = cc::Build::new();
            base(&mut fi);
            fi.include(&shim_fi)
                .include(&shim)
                .include(up.join("GameEngine/Include"))
                .include(up.join("Libraries/Include"))
                .include(ww.join("WWMath"))
                .include(ww.join("WWLib"))
                .file(dif.join("wrap_firing_tracker.cpp"));
            fi.compile("upstream_firing_tracker");
            for p in [
                "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/FiringTracker.cpp",
                "upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/FiringTracker.h",
            ] {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        // --- end p1w6-trackers ---

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

        // --- p1w4-sides ---
        // The map's sides and scripts: Scripts.cpp unchanged and SidesList.cpp with ONE
        // mechanical edit (below), through differential/wrap_sides.cpp, with the chunk
        // build's headers and prelude (shim_sides/sides_prelude.h force-includes
        // shim_chunk/chunk_prelude.h and DataChunk.h) and the stand-ins of
        // differential/shim_sides (each says what it stands in for). Compiled ahead of
        // upstream_chunk, upstream_xfer, upstream_ini and upstream_flags, whose
        // DataChunk, Xfer, INI and KindOf/ObjectStatus name-table symbols it uses.
        // The edit, asserted to match once, in the copy under OUT_DIR: the call that
        // opens the skirmish scripts file in SidesList::prepareForMP_or_Skirmish,
        // `theInputStream.open(path)`, goes through the file system and
        // CompressionManager (which aborts in the chunk build), and becomes a call to
        // sides_open_skirmish_stream (wrap_sides.cpp), which serves the bytes of the
        // case; the rest of the function, the reading of the file included, is unchanged.
        // -fno-delete-null-pointer-checks: upstream calls deleteInstance() on NULL
        // pointers (a no-op under MSVC), which g++ would otherwise compile as
        // "this is never NULL".
        let shim_sides = dif.join("shim_sides");
        let sides_cpp = patch_copy(
            &up.join("GameEngine/Source/GameLogic/Map/SidesList.cpp"),
            "SidesList.cpp",
            &[("if (theInputStream.open(path)) {", "if (sides_open_skirmish_stream(theInputStream, path)) {", 1)],
        );
        let sides_copy = write("SidesList.cpp", &sides_cpp);
        let mut sides = cc::Build::new();
        base(&mut sides);
        sides
            .flag("-fno-delete-null-pointer-checks")
            .include(&shim_sides)
            .include(dif.join("shim_chunk"))
            .include(&dif) // well_known_keys_list.h
            .include(&shim)
            .include(up.join("GameEngine/Include"))
            .include(up.join("Libraries/Include"))
            .include(up.join("Libraries/Source/Compression"))
            .flag("-include")
            .flag(shim_sides.join("sides_prelude.h").to_str().expect("UTF-8 path"))
            .file(dif.join("wrap_sides.cpp"))
            .file(&sides_copy)
            .file(up.join("GameEngine/Source/GameLogic/ScriptEngine/Scripts.cpp"));
        sides.compile("upstream_sides");
        // --- end p1w4-sides ---

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
        // --- p1w5-world ---
        // The object-lifetime part of GameLogic (differential/wrap_game_logic.cpp): seven
        // functions or blocks of upstream text, each cut VERBATIM (first and last marker
        // asserted to occur exactly once) into OUT_DIR/world_gen/*.inc, which the wrapper
        // includes between its stand-ins for Object, UpdateModule and the GameLogic members
        // (GameLogic.cpp itself needs the whole engine). The cuts are: the sleepy-update heap
        // (validateSleepyUpdate .. friend_awakenUpdateModule), processDestroyList, the object
        // ids and registration (getFirstObject .. registerObject), the module loop of
        // update(), destroyAllObjectsImmediate, loadPostProcess, findObjectByID and the
        // three list functions of Object.cpp. Standalone: it includes no upstream header.
        {
            let world_gen = out_dir.join("world_gen");
            std::fs::create_dir_all(&world_gen).expect("create world_gen");
            let gl = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/System/GameLogic.cpp")).expect("read GameLogic.cpp");
            let glh = std::fs::read_to_string(up.join("GameEngine/Include/GameLogic/GameLogic.h")).expect("read GameLogic.h");
            let oc = std::fs::read_to_string(up.join("GameEngine/Source/GameLogic/Object/Object.cpp")).expect("read Object.cpp");
            // (file name, source, first marker (kept), last marker (cut before it))
            let cuts: [(&str, &String, &str, &str); 9] = [
                ("sleepy_heap.inc", &gl, "inline void GameLogic::validateSleepyUpdate() const", "#ifdef DO_UNIT_TIMINGS\n\nvoid drawGraph"),
                ("process_destroy_list.inc", &gl, "void GameLogic::processDestroyList( void )\n{", "/** Process the command list passed to the logic from the network */"),
                ("register_object.inc", &gl, "Object *GameLogic::getFirstObject( void )", "/** create an object based on the thing template"),
                (
                    "sleepy_loop.inc",
                    &gl,
                    "\t{\n\t\twhile (!m_sleepyUpdates.empty())\n",
                    "\n\t// update the Artificial Intelligence system",
                ),
                ("destroy_all.inc", &gl, "void GameLogic::destroyAllObjectsImmediate()\n{", "//-------------------------------------------------------------------------------------------------\n/**GameLogic class destructor"),
                ("load_post_process.inc", &gl, "void GameLogic::loadPostProcess( void )\n{", "}  // end loadPostProcess"),
                ("find_object.inc", &glh, "inline Object* GameLogic::findObjectByID( ObjectID id )", "// the singleton"),
                ("object_list_a.inc", &oc, "Bool Object::isInList(Object **pListHead) const", "void Object::setLayer(PathfindLayerEnum layer)"),
                ("object_list_b.inc", &oc, "void Object::removeFromList(Object **pListHead)", "void Object::friend_prepareForMapBoundaryAdjust"),
            ];
            for (name, text, first, last) in cuts {
                assert_eq!(text.matches(first).count(), 1, "world cut {name}: start marker {first:?} must occur once");
                let a = text.find(first).expect("cut start");
                assert!(text[a..].contains(last), "world cut {name}: end marker {last:?} must follow");
                let rel = text[a..].find(last).expect("cut end");
                let mut body = text[a..a + rel].to_string();
                if name == "load_post_process.inc" {
                    body.push_str("}  // end loadPostProcess\n");
                }
                std::fs::write(world_gen.join(name), format!("/* generated by build.rs: verbatim from upstream, see build.rs (p1w5-world) */\n{body}"))
                    .expect("write world cut");
            }
            let mut wl = cc::Build::new();
            base(&mut wl);
            wl.include(&world_gen).file(dif.join("wrap_game_logic.cpp"));
            wl.compile("upstream_game_logic");
        }
        // --- end p1w5-world ---

        for p in [
            "differential",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/RandomValue.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/crc.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Include/Common/crc.h",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/System",
            "upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/GameClient/GameText.cpp",
            "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Map/SidesList.cpp", // p1w4-sides
            "upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/Scripts.cpp", // p1w4-sides
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
