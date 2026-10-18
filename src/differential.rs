// SPDX-License-Identifier: GPL-3.0-only
//! FFI declarations for the original upstream C++ compiled by `build.rs`
//! under the `differential` feature (test support only, not public API).
//!
//! Each `ref_*` function is an `extern "C"` wrapper in `differential/*.cpp`
//! around the unchanged upstream code (or, where upstream is MSVC inline asm
//! only, a labelled GNU-asm transcription). Vectors cross as `f32` arrays,
//! `Matrix3D` as 12 `f32` in row-major order. The wrappers document the
//! meaning of every `op` code.
//!
//! The RNG wrappers drive upstream's process-global seed arrays, so tests that
//! use them must serialise (see [`GLOBAL_RNG_LOCK`]).

use std::os::raw::{c_char, c_double, c_float, c_int, c_longlong, c_schar, c_short, c_uchar, c_uint, c_ushort};

/// Serialises tests that touch the C++ RNG globals.
pub static GLOBAL_RNG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

extern "C" {
    // RandomValue.cpp
    pub fn ref_seed_random(seed: c_uint, state6: *mut c_uint);
    pub fn ref_random_value(state6: *mut c_uint) -> c_uint;
    pub fn ref_mult_factor() -> c_float;
    pub fn ref_init_random(seed: c_uint);
    pub fn ref_init_game_logic_random(seed: c_uint);
    pub fn ref_get_game_logic_random_seed() -> c_uint;
    pub fn ref_get_game_logic_random_seed_crc() -> c_uint;
    pub fn ref_get_state(which: c_int, out6: *mut c_uint);
    pub fn ref_set_state(which: c_int, in6: *const c_uint);
    pub fn ref_logic_value(lo: c_int, hi: c_int) -> c_int;
    pub fn ref_client_value(lo: c_int, hi: c_int) -> c_int;
    pub fn ref_audio_value(lo: c_int, hi: c_int) -> c_int;
    pub fn ref_logic_value_real(lo: c_float, hi: c_float) -> c_float;
    pub fn ref_client_value_real(lo: c_float, hi: c_float) -> c_float;
    pub fn ref_audio_value_real(lo: c_float, hi: c_float) -> c_float;
    pub fn ref_logic_variable_get_value(kind: c_int, lo: c_float, hi: c_float) -> c_float;
    pub fn ref_client_variable_get_value(kind: c_int, lo: c_float, hi: c_float) -> c_float;

    // crc.h / crc.cpp
    pub fn ref_crc_debug_buffer(buf: *const c_uchar, len: c_int) -> c_uint;
    pub fn ref_crc_debug_chunks(buf: *const c_uchar, lens: *const c_int, n: c_int) -> c_uint;
    pub fn ref_crc_asm_update(crc: c_uint, buf: *const c_uchar, len: c_int) -> c_uint;

    // Trig.cpp / trig.h
    pub fn ref_trig_default(op: c_int, x: c_float) -> c_float;
    pub fn ref_trig_table(op: c_int, x: c_float) -> c_float;
    pub fn ref_int_trig(op: c_int, v: c_int) -> c_int;
    pub fn ref_trig_lookup(which: c_int, i: c_int) -> c_int;

    // DiscreteCircle.cpp (wrap_discrete_circle.cpp)
    pub fn ref_discrete_circle(
        x: c_int,
        y: c_int,
        r: c_int,
        edges: *mut c_int,
        edge_cap: c_int,
        calls: *mut c_int,
        call_cap: c_int,
        call_count: *mut c_int,
    ) -> c_int;

    // QuickTrig.h / QuickTrig.cpp
    pub fn ref_qtrig(op: c_int, x: c_float) -> c_float;
    pub fn ref_qmag(x: c_float, y: c_float, z: c_float) -> c_float;
    pub fn ref_qtrig_table(which: c_int, i: c_int) -> c_float;
    pub fn ref_qtrig_count(which: c_int) -> c_float;

    // Xfer.cpp, XferCRC.cpp, XferSave.cpp, XferLoad.cpp (script interpreter)
    pub fn ref_xfer_run(
        kind: c_int,
        path: *const c_char,
        script: *const c_uint,
        n: c_int,
        log: *mut c_uint,
        cap: c_int,
    ) -> c_int;

    // Xfer::xferKindOf, with the real KindOfMaskType lookups (wrap_xfer.cpp, wrap_flags.cpp)
    pub fn ref_xfer_kind_of(kind: c_int, path: *const c_char, value: c_int, out: *mut c_int) -> c_int;

    // ArchiveFile.cpp, ArchiveFileSystem.cpp, Win32BIGFile(System).cpp
    pub fn ref_big_reset();
    pub fn ref_big_add_local(name: *const c_char, data: *const c_char, len: c_int);
    pub fn ref_big_load(name: *const c_char, overwrite: c_int) -> c_int;
    pub fn ref_big_exists(name: *const c_char) -> c_int;
    pub fn ref_big_archive_for(name: *const c_char, out: *mut c_char, cap: c_int) -> c_int;
    pub fn ref_big_open(name: *const c_char, out: *mut c_char, cap: c_int, streaming: c_int, size_out: *mut c_int) -> c_int;
    pub fn ref_big_file_info(name: *const c_char) -> c_longlong;
    pub fn ref_big_list(
        current: *const c_char,
        original: *const c_char,
        search: *const c_char,
        out: *mut c_char,
        cap: c_int,
    ) -> c_int;

    // INI.cpp (a copy with two MSVC-scope edits, see build.rs), GameCommon.cpp
    pub fn ref_ini_reset();
    pub fn ref_ini_add_file(name: *const c_char, data: *const c_char, len: c_int);
    pub fn ref_ini_add_dir_entry(name: *const c_char);
    pub fn ref_ini_load(
        filename: *const c_char,
        with_xfer: c_int,
        log: *mut c_uchar,
        cap: c_int,
        log_len: *mut c_int,
        crc: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    pub fn ref_ini_load_dir(
        dir: *const c_char,
        with_xfer: c_int,
        log: *mut c_uchar,
        cap: c_int,
        log_len: *mut c_int,
        crc: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    pub fn ref_ini_scan(op: c_int, token: *const c_char, out: *mut c_uint) -> c_int;

    // GameText.cpp (the CSF loader)
    pub fn ref_csf_load(data: *const c_char, len: c_int) -> c_int;
    pub fn ref_csf_fetch(label: *const c_char, out: *mut c_ushort, cap: c_int, exists: *mut c_int) -> c_int;
    pub fn ref_csf_prefix(prefix: *const c_char, out: *mut c_char, cap: c_int) -> c_int;

    // DataChunk.cpp, NameKeyGenerator.cpp (script interpreter)
    pub fn ref_chunk_run(data: *const c_char, len: c_int, log: *mut c_uchar, cap: c_int, log_len: *mut c_int, code: *mut c_uint) -> c_int;

    // NameKeyGenerator.cpp (wrap_namekey.cpp, script interpreter)
    pub fn ref_namekey_run(script: *const c_uchar, len: c_int, log: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_well_known_key_count() -> c_int;
    pub fn ref_well_known_key_name(i: c_int, out: *mut c_char, cap: c_int, key: *mut c_uint) -> c_int;

    // AsciiString.cpp (wrap_ascii_string.cpp, script interpreter)
    pub fn ref_ascii_run(script: *const c_uchar, len: c_int, log: *mut c_uchar, cap: c_int) -> c_int;

    // Dict.cpp (wrap_dict.cpp, script interpreter)
    pub fn ref_dict_run(script: *const c_uchar, len: c_int, log: *mut c_uchar, cap: c_int) -> c_int;

    // WWLib/chunkio.cpp and the WW3D2 loaders (wrap_w3d.cpp)
    pub fn ref_chunkio_run(
        data: *const c_char,
        len: c_int,
        script: *const c_uchar,
        slen: c_int,
        log: *mut c_uchar,
        cap: c_int,
        log_len: *mut c_int,
    ) -> c_int;
    pub fn ref_w3d_layouts(log: *mut c_uchar, cap: c_int, log_len: *mut c_int) -> c_int;
    pub fn ref_w3d_parse(index: c_int, bytes: *const c_uchar, log: *mut c_uchar, cap: c_int, log_len: *mut c_int) -> c_int;
    pub fn ref_w3d_reset_htrees();
    pub fn ref_w3d_add_htree(name16: *const c_char, num_pivots: c_uint) -> c_int;
    pub fn ref_w3d_load(data: *const c_char, len: c_int, kind: c_int, log: *mut c_uchar, cap: c_int, log_len: *mut c_int) -> c_int;
    pub fn ref_w3d_load_mesh(
        data: *const c_char,
        len: c_int,
        prelit_mode: c_int,
        log: *mut c_uchar,
        cap: c_int,
        log_len: *mut c_int,
    ) -> c_int;
    pub fn ref_crc_memory(data: *const c_uchar, len: c_int, crc: c_uint) -> c_uint;
    pub fn ref_dx8_convert_color(rgba: *const c_float) -> c_uint;

    // WW3D2/ddsfile.cpp, WWLib/TARGA.CPP (wrap_texture.cpp)
    pub fn ref_d3d_to_ww3d(code: c_uint) -> c_uint;
    pub fn ref_bytes_per_pixel(format: c_uint) -> c_uint;
    pub fn ref_write_b8g8r8a8(format: c_uint, argb: c_uint, out8: *mut c_uchar);
    pub fn ref_recolor(rgba: c_uint, hsv: *const c_float) -> c_uint;
    pub fn ref_dds_surface_desc_size() -> c_int;
    pub fn ref_dds_run(
        data: *const c_char,
        len: c_int,
        reduction: c_uint,
        do_load: c_int,
        max_levels: c_uint,
        ops: *const c_uint,
        hsv: *const c_float,
        n_ops: c_int,
        log: *mut c_uchar,
        cap: c_int,
        log_len: *mut c_int,
    ) -> c_int;
    pub fn ref_tga_run(data: *const c_char, len: c_int, flow: c_int, log: *mut c_uchar, cap: c_int, log_len: *mut c_int) -> c_int;

    // BaseType.h value types and templates (wrap_value_types.cpp)
    pub fn ref_vt_coord2d(op: c_int, x: c_float, y: c_float, out: *mut c_float) -> c_float;
    pub fn ref_vt_coord3d(op: c_int, a: *const c_float, b: *const c_float, k: c_float, out: *mut c_float) -> c_float;
    pub fn ref_vt_region(op: c_int, lo: *const c_float, hi: *const c_float, q: *const c_float, out: *mut c_float) -> c_float;
    pub fn ref_vt_int(op: c_int, lo: *const c_int, hi: *const c_int) -> c_int;
    pub fn ref_vt_rgb_get_as_int(rgb: *const c_float) -> c_int;
    pub fn ref_vt_rgb_set_from_int(v: c_int, out: *mut c_float);
    pub fn ref_vt_real(op: c_int, a: c_float, b: c_float, c: c_float, d: c_float) -> c_float;
    pub fn ref_vt_intfn(op: c_int, a: c_int, b: c_int, c: c_int) -> c_int;

    // GameType.cpp, GameCommon.cpp and the enum headers (wrap_game_types.cpp)
    pub fn ref_enum_values(which: c_int, out: *mut c_longlong, cap: c_int) -> c_int;
    pub fn ref_name_table(which: c_int, i: c_int) -> *const c_char;
    pub fn ref_game_common_angle(op: c_int, a: c_float, b: c_float) -> c_float;
    pub fn ref_attack_type(op: c_int, t: c_int) -> c_int;

    // BitFlags.h/BitFlagsIO.h and the flag types (wrap_flags.cpp); masks are 4 x u64
    pub fn ref_flags_op(which: c_int, code: c_int, a: *const u64, b: *const u64, c: *const u64, i: c_int, out: *mut u64) -> c_int;
    pub fn ref_flags_names(
        which: c_int,
        code: c_int,
        a: *const u64,
        token: *const c_char,
        i: c_int,
        out: *mut u64,
        name: *mut c_char,
        cap: c_int,
    ) -> c_int;
    pub fn ref_flags_name_at(which: c_int, i: c_int) -> *const c_char;
    pub fn ref_flags_parse(
        which: c_int,
        line: *mut c_char,
        start: *const u64,
        out: *mut u64,
        single: c_int,
        index: *mut c_int,
        desc: *mut c_char,
        cap: c_int,
    ) -> c_int;
    pub fn ref_flags_xfer(which: c_int, mode: c_int, path: *const c_char, input: *const u64, out: *mut u64) -> c_int;
    pub fn ref_flags_enum_values(which: c_int, out: *mut c_longlong, cap: c_int) -> c_int;
    pub fn ref_flags_globals(which: c_int, out: *mut u64);
    pub fn ref_sparse_match(
        which: c_int,
        masks: *const u64,
        counts: *const c_int,
        n_entries: c_int,
        queries: *const u64,
        n_queries: c_int,
        out: *mut c_int,
    ) -> c_int;

    // BaseType.h (asm transcriptions)
    pub fn ref_fast_float2long_round(f: c_float) -> c_int;
    pub fn ref_fast_float_trunc(f: c_float) -> c_float;
    pub fn ref_fast_float_floor(f: c_float) -> c_float;
    pub fn ref_fast_float_ceil(f: c_float) -> c_float;
    pub fn ref_real_to_int(x: c_float) -> c_int;
    pub fn ref_real_to_unsigned_int(x: c_float) -> c_uint;
    pub fn ref_real_to_short(x: c_float) -> c_short;
    pub fn ref_real_to_unsigned_short(x: c_float) -> c_ushort;
    pub fn ref_real_to_byte(x: c_float) -> c_schar;
    pub fn ref_real_to_unsigned_byte(x: c_float) -> c_uchar;
    pub fn ref_double_to_int(x: c_double) -> c_int;
    pub fn ref_real_to_int_ceil(x: c_float) -> c_int;
    pub fn ref_real_to_int_floor(x: c_float) -> c_int;

    // wwmath.h
    pub fn ref_wwmath_init_tables();
    pub fn ref_fast_sin_table(i: c_int) -> c_float;
    pub fn ref_fast_inv_sin_table(i: c_int) -> c_float;
    pub fn ref_fast_acos_table(i: c_int) -> c_float;
    pub fn ref_fast_asin_table(i: c_int) -> c_float;
    pub fn ref_fabs(v: c_float) -> c_float;
    pub fn ref_float_to_int_chop(f: c_float) -> c_int;
    pub fn ref_float_to_int_floor(f: c_float) -> c_int;
    pub fn ref_cos(v: c_float) -> c_float;
    pub fn ref_sin(v: c_float) -> c_float;
    pub fn ref_sqrt(v: c_float) -> c_float;
    pub fn ref_inv_sqrt(v: c_float) -> c_float;
    pub fn ref_float_to_long(f: c_float) -> c_longlong;
    pub fn ref_float_to_long_d(f: c_double) -> c_longlong;
    pub fn ref_fast_sin(v: c_float) -> c_float;
    pub fn ref_fast_inv_sin(v: c_float) -> c_float;
    pub fn ref_fast_cos(v: c_float) -> c_float;
    pub fn ref_fast_inv_cos(v: c_float) -> c_float;
    pub fn ref_fast_acos(v: c_float) -> c_float;
    pub fn ref_acos(v: c_float) -> c_float;
    pub fn ref_fast_asin(v: c_float) -> c_float;
    pub fn ref_asin(v: c_float) -> c_float;
    pub fn ref_atan(v: c_float) -> c_float;
    pub fn ref_atan2(y: c_float, x: c_float) -> c_float;
    pub fn ref_sign(v: c_float) -> c_float;
    pub fn ref_ceil(v: c_float) -> c_float;
    pub fn ref_floor(v: c_float) -> c_float;
    pub fn ref_fast_is_float_positive(v: c_float) -> c_int;
    pub fn ref_is_power_of_2(v: c_uint) -> c_int;
    pub fn ref_clamp(v: c_float, lo: c_float, hi: c_float) -> c_float;
    pub fn ref_clamp_d(v: c_double, lo: c_double, hi: c_double) -> c_double;
    pub fn ref_clamp_int(v: c_int, lo: c_int, hi: c_int) -> c_int;
    pub fn ref_wrap(v: c_float, lo: c_float, hi: c_float) -> c_float;
    pub fn ref_wrap_d(v: c_double, lo: c_double, hi: c_double) -> c_double;
    pub fn ref_min(a: c_float, b: c_float) -> c_float;
    pub fn ref_max(a: c_float, b: c_float) -> c_float;
    pub fn ref_float_as_int(f: c_float) -> c_int;
    pub fn ref_lerp(a: c_float, b: c_float, t: c_float) -> c_float;
    pub fn ref_lerp_d(a: c_double, b: c_double, t: c_float) -> c_double;
    pub fn ref_unit_float_to_byte(f: c_float) -> c_uchar;
    pub fn ref_byte_to_unit_float(b: c_uchar) -> c_float;
    pub fn ref_is_valid_float(x: c_float) -> c_int;
    /// glibc's own function, looked up with dlsym (0 sinf, 1 cosf, 2 acosf,
    /// 3 asinf, 4 atanf): bypasses the test binaries' interposition.
    pub fn ref_glibc_f1(which: c_int, x: c_float) -> c_float;
    pub fn ref_glibc_atan2f(y: c_float, x: c_float) -> c_float;
    /// glibc's double functions by dlsym (0 sin, 1 cos, 2 acos, 3 asin, 4 atan).
    pub fn ref_glibc_d1(which: c_int, x: c_double) -> c_double;
    pub fn ref_glibc_atan2(y: c_double, x: c_double) -> c_double;

    // vector2.h
    pub fn ref_v2_op(op: c_int, a: *const c_float, b: *const c_float, k: c_float, out: *mut c_float);
    pub fn ref_v2_scalar(op: c_int, a: *const c_float, b: *const c_float) -> c_float;
    pub fn ref_v2_pred(op: c_int, a: *const c_float, b: *const c_float, k: c_float) -> c_int;
    pub fn ref_v2_rotate_towards(inout: *mut c_float, target: *const c_float, max_s: c_float, max_c: c_float) -> c_int;
    pub fn ref_v2_rotate_towards_theta(inout: *mut c_float, target: *const c_float, max_theta: c_float) -> c_int;

    // vector3.h
    pub fn ref_v3_op(op: c_int, a: *const c_float, b: *const c_float, k: c_float, out: *mut c_float);
    pub fn ref_v3_scalar(op: c_int, a: *const c_float, b: *const c_float) -> c_float;
    pub fn ref_v3_find(op: c_int, v: c_float, p1: *const c_float, p2: *const c_float) -> c_float;
    pub fn ref_v3_pred(op: c_int, a: *const c_float, b: *const c_float, k: c_float) -> c_int;

    // vector4.h
    pub fn ref_v4_op(op: c_int, a: *const c_float, b: *const c_float, k: c_float, out: *mut c_float);
    pub fn ref_v4_dot(a: *const c_float, b: *const c_float) -> c_float;
    pub fn ref_v4_eq(a: *const c_float, b: *const c_float) -> c_int;

    // matrix3d.h / matrix3d.cpp
    pub fn ref_m3_op(op: c_int, m: *mut c_float, p: *const c_float);
    pub fn ref_m3_multiply(a: *const c_float, b: *const c_float, alias: c_int, out: *mut c_float);
    pub fn ref_m3_vec(op: c_int, m: *const c_float, v: *const c_float, out: *mut c_float);
    pub fn ref_m3_scalar(op: c_int, m: *const c_float) -> c_float;
    pub fn ref_m3_aabox(op: c_int, m: *const c_float, a: *const c_float, b: *const c_float, out: *mut c_float);
    pub fn ref_m3_solve(m: *mut c_float) -> c_int;
    pub fn ref_m3_eq(a: *const c_float, b: *const c_float) -> c_int;
    pub fn ref_m3_static(which: c_int, out: *mut c_float);
    // --- p1w1-small ---
    // LatchRestore.h and Overridable.h/Override.h (wrap_latch_override.cpp)
    pub fn ref_latch_restore(pair: c_int, ops: *const c_int, n: c_int, out: *mut c_int, cap: c_int) -> c_int;
    pub fn ref_overridable_chains(spec: *const c_int, n: c_int, out: *mut c_int, cap: c_int) -> c_int;
    // STLTypedefs.h comparators (wrap_stl_typedefs.cpp)
    pub fn ref_stl_compare(a: *const c_char, ka: c_int, b: *const c_char, kb: c_int, out: *mut c_int);
    // SubsystemInterface.h/.cpp (wrap_subsystem_interface.cpp)
    pub fn ref_subsys_run(ops: *const c_int, n: c_int, out: *mut c_char, cap: c_int) -> c_int;
}

// --- p1w1-unicode ---
extern "C" {
    // UnicodeString.cpp (wrap_unicode_string.cpp, script interpreter)
    pub fn ref_unicode_run(script: *const c_uchar, len: c_int, log: *mut c_uchar, cap: c_int) -> c_int;

    // --- p1w1-partition ---
    // PartitionSolver.cpp (wrap_partition_solver.cpp): pairs of (id, size)
    // as parallel arrays; the solution as (entry id, space id) pairs.
    pub fn ref_partition_solve(
        how: c_int,
        data_ids: *const c_int,
        data_sizes: *const c_uint,
        n_data: c_int,
        space_ids: *const c_int,
        space_sizes: *const c_uint,
        n_spaces: c_int,
        out: *mut c_int,
        out_cap: c_int,
    ) -> c_int;
    // --- end p1w1-partition ---
}
