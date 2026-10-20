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

    // --- p1w1-geometry ---
    // Geometry.cpp (differential/wrap_geometry.cpp)
    pub fn ref_geometry_parse(
        file: *const c_char,
        data: *const c_char,
        len: c_int,
        type0: c_int,
        small0: c_int,
        height0: c_float,
        major0: c_float,
        minor0: c_float,
        out: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        msg_len: *mut c_int,
    ) -> c_int;
    pub fn ref_geometry_op(
        op: c_int,
        type0: c_int,
        small0: c_int,
        height0: c_float,
        major0: c_float,
        minor0: c_float,
        input: *const c_float,
        out: *mut c_float,
        out_count: *mut c_int,
    ) -> c_int;
    pub fn ref_geometry_random(
        op: c_int,
        seed: c_uint,
        type0: c_int,
        small0: c_int,
        height0: c_float,
        major0: c_float,
        minor0: c_float,
        count: c_int,
        out: *mut c_float,
    ) -> c_int;
    pub fn ref_geometry_save(path: *const c_char, type0: c_int, small0: c_int, height0: c_float, major0: c_float, minor0: c_float) -> c_int;
    pub fn ref_geometry_load(
        path: *const c_char,
        type0: c_int,
        small0: c_int,
        height0: c_float,
        major0: c_float,
        minor0: c_float,
        out: *mut c_uint,
    ) -> c_int;
    pub fn ref_geometry_deep_crc(path: *const c_char, type0: c_int, small0: c_int, height0: c_float, major0: c_float, minor0: c_float) -> c_uint;
    // BezierSegment.cpp and BezFwdIterator.cpp (differential/wrap_bezier.cpp)
    pub fn ref_bez_eval(cp: *const c_float, t: c_float, out: *mut c_float);
    pub fn ref_bez_points(cp: *const c_float, n: c_int, out: *mut c_float, cap: c_int, count: *mut c_int) -> c_int;
    pub fn ref_bez_length(cp: *const c_float, tol: c_float) -> c_float;
    pub fn ref_bez_split(cp: *const c_float, t: c_float, out: *mut c_float);
    pub fn ref_bez_iter(cp: *const c_float, n: c_int, out: *mut c_float);
    // Money.cpp and Handicap.cpp (differential/wrap_money.cpp, wrap_handicap.cpp)
    pub fn ref_money_op(op: c_int, money0: c_uint, player0: c_int, amount: c_uint, play_sound: c_int, out: *mut c_uint);
    pub fn ref_money_parse(
        file: *const c_char,
        data: *const c_char,
        len: c_int,
        out: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        msg_len: *mut c_int,
    );
    pub fn ref_money_save(path: *const c_char, money0: c_uint, player0: c_int) -> c_int;
    pub fn ref_money_load(path: *const c_char, money0: c_uint, player0: c_int, out: *mut c_uint);
    pub fn ref_money_deep_crc(path: *const c_char, money0: c_uint, player0: c_int) -> c_uint;
    pub fn ref_handicap_read(nset: c_int, names: *const c_char, values: *const c_float, out: *mut c_float, log: *mut c_char, log_cap: c_int, log_len: *mut c_int);
    // --- end p1w1-geometry ---
}

// --- p1w1-armor ---
extern "C" {
    // Armor.cpp, DamageFX.cpp, INIDamageFX.cpp, Damage.cpp (wrap_armor.cpp)
    pub fn ref_armor_reset();
    pub fn ref_armor_name_key(name: *const c_char) -> c_uint;
    pub fn ref_armor_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_dfx_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_armor_adjust(name: *const c_char, dt: c_int, damage: c_float, out: *mut c_uint) -> c_int;
    pub fn ref_armor_none(damage: c_float, out: *mut c_uint);
    pub fn ref_dfx_query(
        name: *const c_char,
        dt: c_int,
        amount: c_float,
        level: c_int,
        throttle: *mut c_uint,
        fx: *mut c_char,
        cap: c_int,
        fx_len: *mut c_int,
        via_do: *mut c_int,
    ) -> c_int;
    pub fn ref_damage_inline(op: c_int, dt: c_int) -> c_int;
    pub fn ref_damage_flags(all: *mut u64, none: *mut u64);
    pub fn ref_damage_default(w: *mut c_uint);
    pub fn ref_damage_save(
        kind: c_int,
        w: *const c_uint,
        tmpl: *const c_char,
        path: *const c_char,
        out: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
    ) -> c_int;
    pub fn ref_damage_load(
        kind: c_int,
        w: *mut c_uint,
        tmpl: *mut c_char,
        tmpl_cap: c_int,
        bytes: *const c_uchar,
        n: c_int,
        path: *const c_char,
    ) -> c_int;
    pub fn ref_damage_crc(kind: c_int, w: *const c_uint, tmpl: *const c_char, crc: *mut c_uint) -> c_int;
}
// --- end p1w1-armor ---

// --- p1w2-dictmove ---
extern "C" {
    // STLTypedefs.h, the UnicodeString comparator (wrap_stl_typedefs_unicode.cpp)
    pub fn ref_stl_compare_unicode(a: *const c_ushort, na: c_int, ka: c_int, b: *const c_ushort, nb: c_int, kb: c_int, out: *mut c_int);
}
// --- end p1w2-dictmove ---
// --- p1w2-terrain ---
extern "C" {
    // TerrainTypes.cpp, MultiplayerSettings.cpp, CrateSystem.cpp and their INI
    // blocks (wrap_terrain.cpp). Every dump is little-endian: u32/i32 words,
    // u8 flags, floats as their bit patterns, strings length-prefixed.
    pub fn ref_terrain_reset();
    pub fn ref_terrain_name_key(name: *const c_char) -> c_uint;
    pub fn ref_terrain_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_mp_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_mp_colors_by_query(buf: *mut c_uchar, cap: c_int, from: c_int, to: c_int) -> c_int;
    pub fn ref_mp_find_by_name(name: *const c_char, buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_crate_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_crate_reset();
    pub fn ref_crate_find(name: *const c_char, buf: *mut c_uchar, cap: c_int) -> c_int;
    // INI::load with a load type (wrap_ini.cpp), the crate tests' override files
    pub fn ref_ini_load_typed(
        filename: *const c_char,
        load_type: c_int,
        with_xfer: c_int,
        log: *mut c_uchar,
        cap: c_int,
        log_len: *mut c_int,
        crc: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
}
// --- end p1w2-terrain ---

// --- p1w2-globaldata ---
extern "C" {
    // GlobalData.cpp (a copy without the OptionPreferences tail, see build.rs),
    // INIGameData.cpp, and the WeaponBonusSet parts of Weapon.h/.cpp
    // (differential/wrap_globaldata.cpp)
    pub fn ref_globaldata_clear();
    pub fn ref_globaldata_create();
    pub fn ref_globaldata_reset();
    pub fn ref_globaldata_len() -> c_int;
    pub fn ref_globaldata_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_globaldata_field_count() -> c_int;
    pub fn ref_globaldata_field_token(i: c_int) -> *const c_char;
    pub fn ref_globaldata_set_time_of_day(tod: c_int) -> c_int;
    pub fn ref_globaldata_append_bonuses(flags: c_uint, out: *mut c_uint);
    pub fn ref_globaldata_load(
        filename: *const c_char,
        load_type: c_int,
        with_xfer: c_int,
        crc: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
}
// --- end p1w2-globaldata ---

// --- p1w2-science ---
extern "C" {
    // Science.cpp, RankInfo.cpp (wrap_science.cpp)
    pub fn ref_science_reset();
    pub fn ref_science_name_key(name: *const c_char) -> c_uint;
    pub fn ref_science_load(
        filename: *const c_char,
        load_type: c_int,
        with_xfer: c_int,
        crc: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    pub fn ref_science_op(op: c_int);
    pub fn ref_science_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_rank_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_science_web_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_science_probe(st: c_int, buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_science_name_probe(name: *const c_char, buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_science_names(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_rank_probe(level: c_int, buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_science_save(kind: c_int, vals: *const c_int, n: c_int, path: *const c_char, out: *mut c_uchar, cap: c_int, len: *mut c_int) -> c_int;
    pub fn ref_science_load_xfer(kind: c_int, vals: *mut c_int, cap: c_int, n: *mut c_int, bytes: *const c_uchar, nbytes: c_int, path: *const c_char) -> c_int;
    pub fn ref_science_crc(kind: c_int, vals: *const c_int, n: c_int, crc: *mut c_uint) -> c_int;
}
// --- end p1w2-science ---

// --- p1w2-upgrade ---
extern "C" {
    // Upgrade.cpp, INIUpgrade.cpp, SpecialPower.cpp, INISpecialPower.cpp (wrap_upgrade.cpp)
    pub fn ref_up_reset(init_names: c_int, init_center: c_int, n_sciences: c_int, sciences: *const *const c_char) -> c_int;
    pub fn ref_up_name_key(name: *const c_char) -> c_uint;
    pub fn ref_up_load(
        filename: *const c_char,
        load_type: c_int,
        with_xfer: c_int,
        crc: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    pub fn ref_up_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_up_find(name: *const c_char) -> c_int;
    pub fn ref_up_find_by_key(key: c_uint) -> c_int;
    pub fn ref_up_find_vet(level: c_int) -> c_int;
    pub fn ref_up_names(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_up_calc(index: c_int, time: *mut c_int, cost: *mut c_int) -> c_int;
    pub fn ref_up_new_upgrade(name: *const c_char) -> c_int;
    pub fn ref_up_type_name(i: c_int) -> *const c_char;
    pub fn ref_academy_name(i: c_int) -> *const c_char;
    pub fn ref_up_inst_xfer(
        kind: c_int,
        status: *mut c_int,
        path: *const c_char,
        bytes: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        crc: *mut c_uint,
    ) -> c_int;
    pub fn ref_sp_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_sp_by_name(name: *const c_char, out: *mut c_uint, names: *mut c_uchar, cap: c_int, nlen: *mut c_int) -> c_int;
    pub fn ref_sp_by_id(id: c_uint, out: *mut c_uint, names: *mut c_uchar, cap: c_int, nlen: *mut c_int) -> c_int;
    pub fn ref_sp_by_index(index: c_uint, out: *mut c_uint, names: *mut c_uchar, cap: c_int, nlen: *mut c_int) -> c_int;
    pub fn ref_sp_count() -> c_int;
    pub fn ref_sp_reset_store();
    pub fn ref_sp_real_name(i: c_int) -> *const c_char;
    pub fn ref_sp_default_detection() -> c_uint;
    pub fn ref_mask_op(
        which: c_int,
        op: c_int,
        a: *const u64,
        b: *const u64,
        c: *const u64,
        idx: *const c_int,
        nidx: c_int,
        out: *mut u64,
    ) -> c_int;
}
// --- end p1w2-upgrade ---
// --- p1w3-thing ---
extern "C" {
    // Thing.cpp (wrap_thing.cpp): a program of operations on one Thing
    pub fn ref_thing_run(
        kind_words: *const u64,
        terrain: *const c_float,
        n_ops: c_int,
        ops: *const c_int,
        params: *const c_float,
        out: *mut c_uint,
        cap: c_int,
    ) -> c_int;
    // SimpleObjectIterator.cpp (wrap_object_iter.cpp): a program on one iterator
    pub fn ref_object_iter_run(
        n_objs: c_int,
        costs: *const c_int,
        n_ops: c_int,
        ops: *const c_int,
        nums: *const c_float,
        out: *mut c_uint,
        cap: c_int,
    ) -> c_int;
}
// --- end p1w3-thing ---

// --- p1w3-playertemplate ---
extern "C" {
    // PlayerTemplate.cpp (wrap_playertemplate.cpp)
    pub fn ref_pt_reset();
    pub fn ref_pt_name_key(name: *const c_char) -> c_uint;
    pub fn ref_pt_load(
        filename: *const c_char,
        load_type: c_int,
        with_xfer: c_int,
        crc: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    pub fn ref_pt_op(op: c_int);
    pub fn ref_pt_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_pt_nth(i: c_int) -> c_int;
    pub fn ref_pt_find(key: c_uint) -> c_int;
    pub fn ref_pt_num_by_name(name: *const c_char) -> c_int;
    pub fn ref_pt_sides(initial: *const c_char, n: c_int, buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_pt_sides_null();
}
// --- end p1w3-playertemplate ---

// --- p1w3-statemachine ---
extern "C" {
    /// differential/wrap_statemachine.cpp: runs a script (encoding in
    /// `tests/differential_state_machine.rs`) on the REAL `StateMachine`;
    /// `path` is a scratch file prefix. Returns the log length, -1 if the
    /// script is malformed.
    pub fn ref_sm_run(path: *const c_char, script: *const c_uint, n: c_int, log: *mut c_uint, cap: c_int) -> c_int;
}
// --- end p1w3-statemachine ---

// --- p1w3-modules ---
extern "C" {
    // Module.cpp, BehaviorModule.cpp, UpdateModule.cpp, UpgradeModule.cpp, DieModule.cpp, CreateModule.cpp,
    // DamageModule.cpp, CollideModule.cpp, DestroyModule.cpp, BodyModule.cpp, and the body of
    // ModuleFactory::init (wrap_module.cpp)
    pub fn ref_mod_registrations(which: c_int, buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_mod_const(i: c_int) -> c_int;
    pub fn ref_mod_data(
        kind: c_int,
        filename: *const c_char,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    pub fn ref_mod_construct(cls: c_int, null_data: c_int) -> c_int;
    pub fn ref_mod_base_xfer(
        cls: c_int,
        prep: c_int,
        prep_scale: c_float,
        kind: c_int,
        path: *const c_char,
        bytes: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        crc: *mut c_uint,
        state: *mut c_uint,
    ) -> c_int;
    pub fn ref_mod_body_defaults(out: *mut c_uint) -> c_int;
    pub fn ref_mod_collide_defaults(out: *mut c_uint) -> c_int;
    pub fn ref_mod_body_enums(out: *mut c_int) -> c_int;
    pub fn ref_mod_update_run(phase: c_int, n: c_int, ops: *const c_uint, out: *mut c_uint, out_cap: c_int, path: *const c_char) -> c_int;
    pub fn ref_mod_upgrade_run(
        filename: *const c_char,
        n: c_int,
        ops: *const c_uint,
        names: *const *const c_char,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    pub fn ref_mod_upgrade_xfer(
        filename: *const c_char,
        executed: c_int,
        kind: c_int,
        path: *const c_char,
        bytes: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        crc: *mut c_uint,
        state: *mut c_uint,
    ) -> c_int;
    pub fn ref_mod_die_run(
        filename: *const c_char,
        n: c_int,
        queries: *const c_uint,
        out: *mut c_uchar,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
}
// --- end p1w3-modules ---
// --- p1w4-polygon ---
extern "C" {
    // PolygonTrigger.cpp (differential/wrap_polygon_trigger.cpp): the parse, the chain
    // written back, and the queries of each trigger. Returns the log length, or -1.
    #[allow(clippy::too_many_arguments)]
    pub fn ref_polygon_run(
        data: *const c_char,
        len: c_int,
        water_x: c_float,
        water_y: c_float,
        write_id: c_uint,
        queries: *const c_int,
        nq: c_int,
        log: *mut c_uchar,
        log_cap: c_int,
        log_len: *mut c_int,
        inside: *mut c_uchar,
        centers: *mut c_uint,
        radii: *mut c_uint,
        trig_cap: c_int,
        nt: *mut c_int,
    ) -> c_int;

    // MapUtil.cpp's ParseObjectDataChunk (differential/wrap_map_object.cpp): the objects
    // at their deletion and the waypoint map. Returns the parse's result (1 or 0), 3 with
    // the thrown ErrorCode in `code`, or -1.
    pub fn ref_map_object_run(data: *const c_char, len: c_int, log: *mut c_uchar, cap: c_int, log_len: *mut c_int, code: *mut c_uint) -> c_int;
}
// --- end p1w4-polygon ---

// --- p1w4-thingtemplate ---
extern "C" {
    // ThingTemplate.cpp, ThingFactory.cpp, INIObject.cpp and the cut-out members of WeaponSet.cpp,
    // ProductionPrerequisite.cpp and ModuleFactory.cpp (wrap_thingtemplate.cpp)
    pub fn ref_tt_reset(default_occlusion_delay: c_uint, superweapon_restriction: c_uint);
    pub fn ref_tt_name_key(name: *const c_char) -> c_uint;
    pub fn ref_tt_load(
        filename: *const c_char,
        load_type: c_int,
        with_xfer: c_int,
        crc: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    pub fn ref_tt_op(op: c_int);
    pub fn ref_tt_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_tt_fields(which: c_int, buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_tt_find(name: *const c_char) -> c_int;
    pub fn ref_tt_find_by_id(id: c_uint) -> c_int;
    pub fn ref_tt_equivalent(pa: c_int, da: c_int, pb: c_int, db: c_int) -> c_int;
    pub fn ref_tt_new_object(
        position: c_int,
        depth: c_int,
        null_template: c_int,
        mask_words: *const u64,
        made_from: *mut c_int,
        status_out: *mut u64,
    ) -> c_int;
    pub fn ref_tt_new_drawable(position: c_int, depth: c_int, null_template: c_int, bits: c_int, made_from: *mut c_int, status_out: *mut c_int) -> c_int;
    pub fn ref_tt_find_set(position: c_int, depth: c_int, which: c_int, flags: u64) -> c_int;
    pub fn ref_tt_per_unit(position: c_int, depth: c_int, which: c_int, name: *const c_char, out: *mut c_char, cap: c_int, has: *mut c_int) -> c_int;
}
// --- end p1w4-thingtemplate ---

// --- p1w4-sides ---
extern "C" {
    // wrap_sides.cpp: SidesList.cpp and Scripts.cpp. The meaning of every argument and of the
    // log is in differential/wrap_sides.cpp and tests/differential_sides_scripts.rs.
    pub fn ref_sides_setup(tmp_dir: *const c_char, blob: *const c_uchar, blob_len: c_int);
    pub fn ref_sides_constants(out: *mut c_int) -> c_int;
    pub fn ref_sides_run(
        mode: c_int,
        data: *const c_char,
        len: c_int,
        ops: *const c_uchar,
        nops: c_int,
        skirmish: *const c_char,
        sk_len: c_int,
        log: *mut c_uchar,
        cap: c_int,
        log_len: *mut c_int,
    ) -> c_int;
}
// --- end p1w4-sides ---

// --- p1w4-terrain ---
// TerrainLogic.cpp (differential/wrap_terrain_logic.cpp). `scenario` and `program` are
// word arrays (floats as their bits); the encodings are documented in
// tests/differential_terrain_logic.rs. Returns the words written to `out`, or -1 if they
// do not fit.
extern "C" {
    pub fn ref_terrain_logic_run(scenario: *const c_uint, n_scenario: c_int, program: *const c_uint, n_program: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}
// --- end p1w4-terrain ---

// --- p1w5-world ---
extern "C" {
    // GameLogic.cpp's object lifetime (wrap_game_logic.cpp): a program on a fresh GameLogic
    pub fn ref_world_run(prog: *const c_int, n_prog: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}
// --- end p1w5-world ---

// --- p1w6-object ---
extern "C" {
    // Object.cpp (wrap_object.cpp): a program on a fresh world of objects. `names` holds the four flag name
    // tables ("count\nname\n..." each); the lines the run writes go to `out` (separated by '\n'). Returns their
    // length, or minus the length needed when `cap` is too small. The encoding of the program is documented in
    // tests/differential_object.rs.
    pub fn ref_object_run(prog: *const c_uint, n_prog: c_int, names: *const c_char, n_names: c_int, out: *mut c_char, cap: c_int, verbose: c_int) -> c_int;
}
// --- end p1w6-object ---
// --- p1w6-locomotor ---
/// The scene of one Locomotor call (`LocoEnv` of differential/shim_locomotor/locomotor_world.h):
/// same fields, same order, 4-byte fields only.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LocoEnv {
    pub frame: c_uint,
    pub gravity: c_float,
    pub penalty: c_int,
    pub body_state: c_int,
    pub has_physics: c_int,
    pub stunned: c_int,
    pub motive: c_int,
    pub vel: [c_float; 3],
    pub vel_mag: c_float,
    pub fwd2d: c_float,
    pub fwd3d: c_float,
    pub mass: c_float,
    pub layer: c_int,
    pub kind_mask: c_int,
    pub deck_height: c_float,
    pub major_radius: c_float,
    pub bounding_circle_radius: c_float,
    pub disabled_held: c_int,
    pub ga: c_float,
    pub gb: c_float,
    pub gc: c_float,
    pub wx: c_float,
    pub wz: c_float,
    pub layer_offset: c_float,
    pub struct_offset: c_float,
    pub highest_layer: c_int,
    pub inv_enabled: c_int,
    pub inv_box: [c_float; 4],
}

extern "C" {
    // Locomotor.cpp, LocomotorSet and the INI block (wrap_locomotor.cpp). The meaning of every
    // argument is in differential/wrap_locomotor.cpp and tests/differential_locomotor.rs.
    pub fn ref_loco_reset();
    pub fn ref_loco_name_key(name: *const c_char) -> c_uint;
    pub fn ref_loco_dump_store(buf: *mut c_uint, cap: c_int) -> c_int;
    pub fn ref_loco_store_reset();
    pub fn ref_loco_new(name: *const c_char, frame: c_uint) -> c_int;
    pub fn ref_loco_xfer_save(handle: c_int, path: *const c_char, out: *mut c_uchar, cap: c_int, len: *mut c_int) -> c_int;
    pub fn ref_loco_xfer_load(name: *const c_char, frame: c_uint, path: *const c_char, new_handle: *mut c_int) -> c_int;
    pub fn ref_loco_obj(op: c_int, fa: *const c_float, ia: *const c_int);
    pub fn ref_loco_obj_state(out: *mut c_uint);
    pub fn ref_loco_call(
        handle: c_int,
        op: c_int,
        env: *const LocoEnv,
        fa: *const c_float,
        ia: *const c_int,
        rf: *mut c_uint,
        ri: *mut c_int,
        log: *mut c_uint,
        cap: c_int,
        log_len: *mut c_int,
    );
    pub fn ref_loco_state(handle: c_int, out: *mut c_uint);
    pub fn ref_loco_getters(handle: c_int, out: *mut c_uint);
    pub fn ref_locoset_new() -> c_int;
    pub fn ref_locoset_add(set: c_int, name: *const c_char, frame: c_uint) -> c_int;
    pub fn ref_locoset_clear(set: c_int);
    pub fn ref_locoset_find(set: c_int, mask: c_int) -> c_int;
    pub fn ref_locoset_dump(set: c_int, buf: *mut c_uint, cap: c_int) -> c_int;
    pub fn ref_locoset_state(set: c_int, index: c_int, out: *mut c_uint);
    pub fn ref_locoset_xfer_save(set: c_int, cur: c_int, path: *const c_char, out: *mut c_uchar, cap: c_int, len: *mut c_int) -> c_int;
    pub fn ref_locoset_xfer_load(set: c_int, frame: c_uint, path: *const c_char, cur: *mut c_int) -> c_int;
}
// --- end p1w6-locomotor ---
// --- p1w6-partition ---
extern "C" {
    // PartitionManager.cpp (wrap_partition.cpp): a program on a fresh PartitionManager over an installed
    // scenario; the encodings are documented in tests/differential_partition.rs. Returns the words
    // written, -1 if they do not fit, -3 for an unknown operation, -5 if Radar's calls differ from Display's.
    pub fn ref_partition_run(
        ig: *const c_int,
        fg: *const c_float,
        rel: *const c_uchar,
        io: *const c_int,
        fo: *const c_float,
        n_ops: c_int,
        ops: *const c_int,
        opf: *const c_float,
        out: *mut c_uint,
        cap: c_int,
    ) -> c_int;
}
// --- end p1w6-partition ---
// --- p1w6-trackers ---
extern "C" {
    // ObjectTypes.cpp (wrap_object_types.cpp): a program of list operations, logged word by word
    // (tests/differential_object_types.rs documents the encoding).
    pub fn ref_object_types_run(
        ops: *const c_int,
        n_ops: c_int,
        blob: *const c_uchar,
        blob_len: c_int,
        tmp_path: *const c_char,
        out: *mut c_uint,
        cap: c_int,
    ) -> c_int;
}
extern "C" {
    // ExperienceTracker.cpp (wrap_experience_tracker.cpp): a program of tracker operations on up to
    // four objects (tests/differential_experience_tracker.rs documents the encoding).
    pub fn ref_experience_run(
        n: c_int,
        spec: *const c_int,
        ops: *const c_int,
        n_ops: c_int,
        blob: *const c_uchar,
        blob_len: c_int,
        tmp_path: *const c_char,
        out: *mut c_uint,
        cap: c_int,
    ) -> c_int;
}
extern "C" {
    // FiringTracker.cpp (wrap_firing_tracker.cpp): a program of tracker operations on one tracker,
    // logging every call it makes on its owner, weapon and audio in order
    // (tests/differential_firing_tracker.rs documents the encoding).
    pub fn ref_firing_run(
        spec: *const c_int,
        n_spec: c_int,
        ops: *const c_int,
        n_ops: c_int,
        blob: *const c_uchar,
        blob_len: c_int,
        tmp_path: *const c_char,
        out: *mut c_uint,
        cap: c_int,
    ) -> c_int;
}
// --- end p1w6-trackers ---
// --- p1w6-energy ---
extern "C" {
    // Energy.cpp, unchanged (wrap_energy.cpp): one call per op on a single Energy, and its xfer
    pub fn ref_energy_reset();
    pub fn ref_energy_op(op: c_int, a: c_int, b: c_int, frame: c_uint, out: *mut c_uint);
    pub fn ref_energy_save(path: *const c_char) -> c_int;
    pub fn ref_energy_load(path: *const c_char, out: *mut c_uint);

    // ScoreKeeper.cpp, unchanged (wrap_score_keeper.cpp): a fixed table of 12 templates, one
    // ScoreKeeper, its readout (27 values with the status on load), and its xfer
    pub fn ref_score_setup(names: *const c_char, groups: *const c_int, known: *const c_int, kind_bits: *const c_int);
    pub fn ref_score_fresh();
    pub fn ref_score_op(op: c_int, t: c_int, c: c_int, uc: c_int, scoring: c_int, amount: c_int, out: *mut c_uint);
    pub fn ref_score_save(path: *const c_char) -> c_int;
    pub fn ref_score_load(path: *const c_char, out: *mut c_uint);

    // MissionStats.cpp, unchanged (wrap_mission_stats.cpp): its counters (34 values) and its xfer
    pub fn ref_mission_op(op: c_int, idx: c_int, value: c_int, out: *mut c_uint);
    pub fn ref_mission_save(path: *const c_char) -> c_int;
    pub fn ref_mission_load(path: *const c_char, out: *mut c_uint);

    // AcademyStats.cpp, unchanged (wrap_academy_stats.cpp): one player with its statistics, the
    // local and neutral players, the advice; the labels of its tips come back as text
    pub fn ref_academy_init(frame: c_uint, side: c_int, kinds: *const c_int, tunnels: *const c_int, n_objects: c_int);
    pub fn ref_academy_update(frame: c_uint, money: c_int, power: c_int, kinds: *const c_int, tunnels: *const c_int, n_objects: c_int);
    pub fn ref_academy_record(code: c_int, frame: c_uint, a: c_int, b: c_int, kinds: *const c_int, tunnel: c_int);
    pub fn ref_academy_advice(frame: c_uint, local_mines: c_int, neutral_sniped: c_int, alt_mouse: c_int, labels: *mut c_char, cap: c_int) -> c_int;
    pub fn ref_academy_probe(out: *mut c_uint);
    pub fn ref_academy_save(path: *const c_char) -> c_int;
    pub fn ref_academy_load(path: *const c_char, out: *mut c_uint);
}
// --- end p1w6-energy ---
// --- p1w7-sonnet ---
extern "C" {
    // TunnelTracker.cpp and CaveSystem.cpp, unchanged (wrap_tunnel_tracker.cpp): a program of
    // operations on a world of 8 objects, one tracker and a cave system; the text lines each
    // operation observes. Returns the length written, or minus the length needed if `cap` is
    // too small. The words of a program are documented in tests/differential_tunnel_tracker.rs.
    pub fn ref_tunnel_run(prog: *const c_uint, n_prog: c_int, out: *mut c_char, cap: c_int) -> c_int;

    // ResourceGatheringManager.cpp, unchanged (wrap_resource_gathering.cpp): a program of operations on a
    // world of 8 objects and one manager; the text lines each operation observes. Returns the length written,
    // or minus the length needed if `cap` is too small. The words are documented in
    // tests/differential_resource_gathering.rs.
    pub fn ref_resource_run(prog: *const c_uint, n_prog: c_int, out: *mut c_char, cap: c_int) -> c_int;

    // Weapon.cpp's WeaponTemplate and WeaponStore, unchanged but for the cuts of build.rs (wrap_weapon.cpp): the
    // `Weapon` block is read by the real INI::load through the hooks of wrap_ini.cpp.
    pub fn ref_weapon_reset();
    pub fn ref_weapon_name_key(name: *const c_char) -> c_uint;
    pub fn ref_weapon_dump(buf: *mut c_uchar, cap: c_int) -> c_int;
    pub fn ref_weapon_find(name: *const c_char) -> c_int;
    pub fn ref_weapon_post_process();
    pub fn ref_weapon_store_reset();
    pub fn ref_weapon_historic(index: c_int, frame: c_uint, x: c_float, y: c_float, z: c_float);
    pub fn ref_weapon_delayed(op: c_int, index: c_int, frame: c_uint, source: c_int, victim: c_int, x: c_float, y: c_float, z: c_float, bonus: *const c_float);
    pub fn ref_weapon_getters(index: c_int, bonus: *const c_float, out: *mut c_uint);
    // p1w8-sonnet slice B: the firing half of Weapon.cpp and all of WeaponSet.cpp on stand-ins (wrap_weapon.cpp, fire_driver.h)
    pub fn ref_weapon_fire_run(words: *const c_uint, n: c_int, tmp_path: *const c_char, out: *mut c_char, cap: c_int) -> c_int;
}
// --- end p1w7-sonnet ---

// --- p1l9-example ---
extern "C" {
    // CommandSetUpgrade.cpp, in wrap_module.cpp's unit (differential/l9_example.inc)
    #[allow(clippy::too_many_arguments)]
    pub fn ref_l9ex_cmdset_run(
        filename: *const c_char,
        has_player: c_int,
        player_mask: *const c_uint,
        obj_mask: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
}
// --- end p1l9-example ---
// --- p1w9-sonnet ---
extern "C" {
    // Team.cpp on stand-ins (wrap_team.cpp, shim_team/team_driver.h)
    pub fn ref_team_run(words: *const c_uint, n: c_int, tmp_path: *const c_char, out: *mut c_char, cap: c_int) -> c_int;
}
// --- end p1w9-sonnet ---
