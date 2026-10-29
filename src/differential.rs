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

/// The scene of the AI build (`AiEnv` of `differential/shim_ai/ai_world.h`, p1w13-sonnet batch A2): the Locomotor build's
/// [`LocoEnv`] field for field, then the AI's. Same fields, same order, 4-byte fields only.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AiEnv {
    pub loco: LocoEnv,
    pub object_id: c_int,
    pub crusher_level: c_int,
    pub disabled_unmanned: c_int,
    pub allow_to_fall: c_int,
    pub has_group: c_int,
    pub seed: c_uint,
    pub line_rate: c_uint,
    pub ground_rate: c_uint,
    pub cliff_rate: c_uint,
    pub pinch_rate: c_uint,
    pub valid_pos_rate: c_uint,
    // batch A4: the pathfinder's goal position
    pub pf_goal: [c_float; 3],
    // batch A3
    pub effectively_dead: c_int,
    pub player_type: c_int,
    pub team_attack_common: c_int,
    pub team_target: c_int,
    pub able_rate: c_uint,
    pub stealth: c_int,
    pub container_id: c_int,
    pub container_allows_fire: c_int,
    pub container_radius: c_float,
    pub last_damage_type: c_int,
    pub last_damage_source: c_int,
    pub force_idle_frames: c_int,
    pub attack_uses_los: c_int,
    pub ignore_insignificant: c_int,
    pub vision_range: c_float,
    pub closest_enemy: c_int,
    pub weapon_anti: c_int,
    pub upgrades: c_int,
    pub horde: c_int,
    pub exist_mask: c_uint,
    pub ai_mask: c_uint,
    pub has_machine: c_int,
    // batch A4
    pub pf_has_goal: c_int,
    pub disabled_mask: c_int,
    pub repulsor: c_int,
    pub obj_vision_range: c_float,
    pub turret_rate: c_uint,
    pub state_rate: c_uint,
    // batch A5
    pub mobile: c_int,
    pub partition_cell_size: c_float,
    pub extent_hi: [c_float; 2],
    pub team_mask: c_uint,
    pub action_rate: c_uint,
    pub other_ai_path: c_uint,
    pub move_away_rate: c_uint,
    pub path_exists_rate: c_uint,
    pub continue_range: c_float,
    pub contact_weapon: c_int,
    pub closest_attack: c_int,
    pub find_pos_rate: c_uint,
    pub has_contain: c_int,
    pub modules: c_int,
    pub has_command_set: c_int,
    pub command_slots: [c_int; 18],
    pub button_types: [c_int; 4],
    // batch A6
    pub pf_extent_hi: [c_float; 2],
    pub service_rate: c_uint,
    pub pf_answer_rate: c_uint,
    pub range_rate: c_uint,
    pub repulsed_distance: c_float,
    pub waypoint_links: c_uint,
    // batch A7
    pub sounds: c_int,
    pub formation_id: c_int,
    pub group_speed: c_float,
    pub max_height_above_position: c_float,
    // batch A9
    pub waypoint_links2: c_uint,
    pub wall_height: c_float,
    pub group_center: [c_float; 3],
    pub team_waypoint: c_int,
    pub skirmish: c_int,
    pub team_center: [c_float; 3],
    pub team_count: c_int,
    pub skirmish_fudge: c_float,
    // batch A10a
    pub weapon_status: c_int,
    pub leech_range: c_int,
    pub max_shots_init: c_int,
    pub aim_delta: c_float,
    pub cur_slot: c_int,
    pub slot_mask: c_int,
    pub able_to_attack: c_int,
    pub out_of_ammo: c_int,
    pub cur_weapon_locked: c_int,
    pub has_any_weapon: c_int,
    pub crushes_infantry: c_int,
    pub object_rate: c_uint,
    pub garrisonable: c_int,
    pub contain_count: c_int,
    pub rel_angle: c_float,
    pub turret_turn_rate: [c_float; 2],
    pub turret_cur_weapon: c_int,
    pub team_epoch: c_int,
    pub turrets: c_int,
    // batch A12
    pub units_hunt: c_int,
    // batch A13
    pub exit_flags: c_int,
    pub exit_door: c_int,
    // batch A14
    pub minor_radius: c_float,
    pub geom_box: c_int,
    // batch A15
    pub contained_mask: c_uint,
    // batch A16
    pub difficulty: c_int,
    pub choose_normal: c_int,
    // batch A17
    pub dock_mask: c_uint,
    pub dock_flags: c_int,
    pub dock_pos: [c_float; 3],
    pub dock_index: c_int,
    pub drone_mask: c_uint,
    pub dock_rate: c_uint,
    // batch A18
    pub clearable_attacker: c_int,
    pub guard_flags: c_int,
    pub objects_changed_frame: c_uint,
    pub guard_scan_rate: c_uint,
    pub guard_return_scan_rate: c_uint,
    pub guard_chase_frames: c_uint,
    // batch A20: the tunnel network's tunnels (after the owner's container), current nemesis, the tunnels' damage times and
    // no-effect bits, other objects' AIs' goal
    pub tunnel_ids: [c_int; 4],
    pub tunnel_nemesis: c_int,
    pub tunnel_stamp: c_uint,
    pub tunnel_no_effect: c_uint,
    pub other_ai_goal: c_int,
    // batch A23: other objects' AIs' machine: its current state (set, not entered) and goal position
    pub other_ai_state: c_int,
    pub other_ai_goal_pos: [c_float; 3],
    // batch A30: the AI's class (0 AIUpdateInterface, 1 WanderAIUpdate, 2 TransportAIUpdate; batch A31: 3 DeployStyleAIUpdate)
    pub subclass: c_int,
    // batch A31: a DeployStyleAIUpdate's data's bits (reset turret before packing, turrets function only when deployed, turrets
    // must center before packing, manual deploy animations) and the object's drawable (bit 4); its unpack and pack times
    pub deploy_flags: c_int,
    pub deploy_times: [c_uint; 2],
    // batch A32: an AssaultTransportAIUpdate's data's members-get-healed-at life ratio
    pub assault_ratio: c_float,
    // batch A33: the railed transport's named waypoints ("railStartNN", "railEndNN": bit NN-1) and its docks (bit 0 the object's
    // dock, bit 1 a railed transport dock)
    pub rail_start_mask: c_uint,
    pub rail_end_mask: c_uint,
    pub rail_flags: c_int,
}

/// Batch A22: one party of the collisions' scene (`differential/wrap_ai_collide.cpp`'s `ColParty`, field for field).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColParty {
    pub id: c_int,
    pub pos: [c_float; 3],
    pub dir: [c_float; 3],
    pub kinds: c_int,
    pub using_ability: c_int,
    pub formation: c_int,
    pub has_physics: c_int,
    pub vel: [c_float; 3],
    pub crush: [c_int; 3],
    pub angle_to_other: c_float,
    pub angle_to_path: c_float,
    pub has_ai: c_int,
    pub ignore_until: c_uint,
    pub can_path_through: c_int,
    pub is_blocked: c_int,
    pub cur_max_blocked_speed: c_float,
    pub blocked_frames: c_int,
    pub is_blocked_and_stuck: c_int,
    pub loco_goal_type: c_int,
    pub goal_pos: [c_float; 3],
    pub goal_cell: [c_int; 2],
    pub moving: c_int,
    pub ground: c_int,
    pub waiting: c_int,
    pub moving_away: c_int,
    pub busy: c_int,
    pub idle: c_int,
    pub dead: c_int,
    pub state_id: c_int,
    pub has_loco: c_int,
    pub backwards: c_int,
    pub wander: c_float,
    pub has_path: c_int,
    pub path_pos: [c_float; 3],
    pub adjust: [c_float; 2],
}

/// Batch A22: the collisions' scene (`ColScene`): the AI asked (0) and the other (1), the frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColScene {
    pub p: [ColParty; 2],
    pub frame: c_uint,
}

extern "C" {
    // batch A22: the AI's collisions (wrap_ai_collide.cpp)
    pub fn ref_ai_collide(op: c_int, scene: *const ColScene, out: *mut c_uint, log: *mut c_uint, log_cap: c_int, log_len: *mut c_int) -> c_int;
}

/// Batches A24 and A25: the AI group's scene's objects (`differential/wrap_ai_group.cpp`'s `GrpObj`, field for field).
pub const GRP_OBJECTS: usize = 12;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GrpObj {
    pub id: c_int,
    /// `GRP_F_*`
    pub flags: c_int,
    /// bits: structure, always selectable, immobile, aircraft
    pub kinds: c_int,
    /// bits: can stealth, stealthed, detected
    pub status: c_int,
    pub pos: [c_float; 3],
    /// 0..3
    pub player: c_int,
    pub formation: c_int,
    pub damage: c_int,
    pub loco_speed: c_float,
    /// passengers' ids (0: none)
    pub contained: [c_int; 3],
    /// -1: no stealth module
    pub stealth_delay: c_int,
    /// -1: no command set; else a bit per command type 0..7
    pub command_set: c_int,
    // batch A25
    pub formation_offset: [c_float; 2],
    pub bounding_radius: c_float,
    pub major_radius: c_float,
    /// the AI's `m_tmpInt` at the start
    pub tmp: c_int,
    /// -1: no current locomotor; else its `LocomotorPriority`
    pub loco_priority: c_int,
}

/// Batch A24: the AI group's scene (`GrpScene`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GrpScene {
    pub o: [GrpObj; GRP_OBJECTS],
    pub frame: c_uint,
    pub seed: c_uint,
    pub penalty: c_int,
    pub next_group_id: c_int,
    pub next_formation_id: c_int,
    // batch A25: the map's extent and the pathable one (lo.x, lo.y, hi.x, hi.y), the gather factor, AI_DEBUG_TERRAIN, the AI's
    // group-move data, the players' types (bit p: a computer)
    pub extent: [c_float; 4],
    pub path_extent: [c_float; 4],
    pub gather_factor: c_float,
    pub debug_terrain: c_int,
    pub distance_requires_group: c_float,
    pub min_distance_for_group: c_float,
    pub min_infantry_for_group: c_int,
    pub min_vehicles_for_group: c_int,
    pub player_types: c_int,
}

/// Batch A24: one operation on the group (`GrpOp`; its meaning per kind in `ref_ai_group_run`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GrpOp {
    pub kind: c_int,
    pub a: c_int,
    pub b: c_int,
    pub c: c_int,
    pub x: c_float,
    pub y: c_float,
    pub z: c_float,
    pub w: c_float,
}

extern "C" {
    // batch A27: the AI's data (wrap_ai_data.cpp), loaded through ref_science_load
    pub fn ref_ai_data_reset();
    pub fn ref_ai_data_dump(buf: *mut u8, cap: c_int) -> c_int;
    pub fn ref_ai_data_crc(crc: *mut c_uint, xfer_crc: *mut c_uint);
}

extern "C" {
    // batch A24: the AI group (wrap_ai_group.cpp)
    pub fn ref_ai_group_run(scene: *const GrpScene, ops: *const GrpOp, n_ops: c_int, out: *mut c_uint, out_cap: c_int, log: *mut c_uint, log_cap: c_int, log_len: *mut c_int) -> c_int;
    pub fn grpAnswer(seed: c_uint, kind: c_uint, obj: c_uint, arg: c_uint) -> c_uint;
}

// --- p1w13-sonnet --- (batches A28 and A29: the AI player and the skirmish AI player, differential/wrap_ai_player.cpp; field for field its ApScene)
/// Batch A28: the AI player's world, table sizes.
pub const AP_OBJECTS: usize = 48;
pub const AP_TEMPLATES: usize = 10;
pub const AP_PROTOS: usize = 6;
pub const AP_TEAMS: usize = 64;
pub const AP_PLAYERS: usize = 3;
pub const AP_BUILD: usize = 10;
pub const AP_SIDES: usize = 2;
pub const AP_UPGRADES: usize = 4;
/// Batch A29: a faction's skirmish build list's entries, the approach paths, their waypoints.
pub const AP_SIDE_BUILD: usize = 6;
pub const AP_PATHS: usize = 3;
pub const AP_WAYPOINTS: usize = 4;

/// Batch A28: an object (`ApObj`; `id` 0: none, `player` -1: none, `cmd_buttons` -1: no command set).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ApObj {
    pub id: c_int,
    pub tmpl: c_int,
    pub player: c_int,
    pub team: c_int,
    pub flags: c_int,
    pub status: c_int,
    pub pos: [c_float; 3],
    pub radius: c_float,
    pub builder: c_int,
    pub spawner: c_int,
    pub reconstructed: c_int,
    pub dock: c_int,
    pub boxes: c_int,
    pub prod_count: c_int,
    pub damage_state: c_int,
    pub damage_stamp: c_int,
    pub cmd_buttons: c_int,
    pub rally: [c_float; 3],
}

/// Batch A28: a template (`ApTmpl`; `new_flags`: a new object's flags).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ApTmpl {
    pub kinds: c_int,
    pub cost: c_int,
    pub equiv: c_int,
    pub new_flags: c_int,
    pub radius: c_float,
    pub angle: c_float,
}

/// Batch A28: a team prototype (`ApProto`; `unit_tmpl` -1: a name no template has, `condition` -1: none).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ApProto {
    pub player: c_int,
    pub max_instances: c_int,
    pub priority: c_int,
    pub idle_frames: c_int,
    pub flags: c_int,
    pub num_units: c_int,
    pub condition: c_int,
    pub unit_tmpl: [c_int; 4],
    pub unit_min: [c_int; 4],
    pub unit_max: [c_int; 4],
    pub home: [c_float; 3],
}

/// Batch A28: a team (`ApTeam`; `id` 0: none).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ApTeam {
    pub id: c_int,
    pub proto: c_int,
    pub active: c_int,
    pub idle: c_int,
}

/// Batch A28: a player (`ApPlayer`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ApPlayer {
    pub money: c_int,
    pub flags: c_int,
    pub side: c_int,
    pub attacked_frame: c_int,
    pub science_points: c_int,
    pub default_team: c_int,
    /// Batch A29: the skirmish AI's enemy (-1: none) and the start position.
    pub enemy: c_int,
    pub mp_start: c_int,
}

/// Batch A28: an entry of player 0's build list (`ApBuild`; `tmpl` -1: no name, -2: a name no template has).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ApBuild {
    pub tmpl: c_int,
    pub object_id: c_int,
    pub timestamp: c_int,
    pub num_rebuilds: c_int,
    pub flags: c_int,
    pub desired: c_int,
    pub current: c_int,
    pub health: c_int,
    pub name: c_int,
    pub script: c_int,
    pub loc: [c_float; 3],
    pub angle: c_float,
    pub rally: [c_float; 2],
}

/// Batch A28: a side of the AI data (`ApSide`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ApSide {
    pub side: c_int,
    pub easy: c_int,
    pub normal: c_int,
    pub hard: c_int,
    pub num_skills: [c_int; 5],
    pub skills: [[c_int; 4]; 5],
    /// Batch A29: the base defence structure's template (-1: none).
    pub base_defense: c_int,
}

/// Batch A28: the AI player's scene (`ApScene`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApScene {
    pub o: [ApObj; AP_OBJECTS],
    pub t: [ApTmpl; AP_TEMPLATES],
    pub pr: [ApProto; AP_PROTOS],
    pub tm: [ApTeam; AP_TEAMS],
    pub pl: [ApPlayer; AP_PLAYERS],
    pub b: [ApBuild; AP_BUILD],
    pub sides: [ApSide; AP_SIDES],
    pub num_build: c_int,
    pub frame: c_int,
    pub seed: c_int,
    pub debug_ai: c_int,
    pub box_value: c_int,
    pub difficulty: c_int,
    pub skirmish: c_int,
    pub enemy: c_int,
    pub next_object_id: c_int,
    pub next_team_id: c_int,
    pub legal_pct: c_int,
    pub rel: [[c_int; AP_PLAYERS]; AP_PLAYERS],
    pub extent: [c_float; 4],
    pub rebuild_delay_seconds: c_int,
    pub resources_poor: c_int,
    pub resources_wealthy: c_int,
    pub structure_seconds: c_float,
    pub structures_poor_mod: c_float,
    pub structures_wealthy_mod: c_float,
    pub team_resources_to_build: c_float,
    pub max_recruit_distance: c_float,
    pub safe_radius: c_float,
    pub team_seconds: c_float,
    pub team_poor_mod: c_float,
    pub team_wealthy_mod: c_float,
    /// Batch A29: the factions' skirmish build lists, the approach paths (Center, Flank, Backdoor, numbered `path_start`), the AI
    /// data's base rotation and defence distance.
    pub side_build: [[ApBuild; AP_SIDE_BUILD]; AP_SIDES],
    pub side_build_count: [c_int; AP_SIDES],
    pub paths: [[[c_float; 3]; AP_WAYPOINTS]; AP_PATHS],
    pub path_len: [c_int; AP_PATHS],
    pub path_start: c_int,
    pub rotate_bases: c_int,
    pub defense_extra: c_float,
}

impl Default for ApScene {
    fn default() -> Self {
        ApScene {
            o: [ApObj::default(); AP_OBJECTS],
            t: [ApTmpl::default(); AP_TEMPLATES],
            pr: [ApProto::default(); AP_PROTOS],
            tm: [ApTeam::default(); AP_TEAMS],
            pl: [ApPlayer::default(); AP_PLAYERS],
            b: [ApBuild::default(); AP_BUILD],
            sides: [ApSide::default(); AP_SIDES],
            num_build: 0,
            frame: 0,
            seed: 0,
            debug_ai: 0,
            box_value: 0,
            difficulty: 0,
            skirmish: 0,
            enemy: 0,
            next_object_id: 0,
            next_team_id: 0,
            legal_pct: 0,
            rel: [[0; AP_PLAYERS]; AP_PLAYERS],
            extent: [0.0; 4],
            rebuild_delay_seconds: 0,
            resources_poor: 0,
            resources_wealthy: 0,
            structure_seconds: 0.0,
            structures_poor_mod: 0.0,
            structures_wealthy_mod: 0.0,
            team_resources_to_build: 0.0,
            max_recruit_distance: 0.0,
            safe_radius: 0.0,
            team_seconds: 0.0,
            team_poor_mod: 0.0,
            team_wealthy_mod: 0.0,
            side_build: [[ApBuild::default(); AP_SIDE_BUILD]; AP_SIDES],
            side_build_count: [0; AP_SIDES],
            paths: [[[0.0; 3]; AP_WAYPOINTS]; AP_PATHS],
            path_len: [0; AP_PATHS],
            path_start: 0,
            rotate_bases: 0,
            defense_extra: 0.0,
        }
    }
}

/// Batch A28: one operation on the AI player (`ApOp`; its meaning per kind in `ref_ai_player_run`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ApOp {
    pub kind: c_int,
    pub a: c_int,
    pub b: c_int,
    pub c: c_int,
    pub x: c_float,
    pub y: c_float,
    pub z: c_float,
    pub w: c_float,
}

extern "C" {
    // batch A28: the AI player (wrap_ai_player.cpp)
    pub fn ref_ai_player_run(
        scene: *const ApScene,
        ops: *const ApOp,
        n_ops: c_int,
        out: *mut c_uint,
        cap: c_int,
        out_len: *mut c_int,
        log: *mut c_uint,
        log_cap: c_int,
        log_len: *mut c_int,
    ) -> c_int;
    pub fn ref_ai_player_scene_size() -> c_int;
    pub fn apAnswer(seed: c_uint, kind: c_uint, a: c_uint, b: c_uint) -> c_uint;
}
// --- end p1w13-sonnet ---

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
// --- p1l9-creates ---
extern "C" {
    // wrap_create_modules.cpp: the seven create modules of GameLogic/Object/Create, unchanged, on the stand-ins of
    // differential/shim_creates. A block of INI (read by the real INI) and a program of state changes and calls; the
    // text lines the program produces (see tests/differential_create_modules.rs). Returns 0; `out_len` is the length
    // that was written (or would have been, if it is larger than `cap`), and -1 is never returned for a valid block.
    pub fn ref_create_run(
        text: *const c_char,
        text_len: c_int,
        prog: *const c_uint,
        n_prog: c_int,
        out: *mut c_char,
        cap: c_int,
        out_len: *mut c_int,
    ) -> c_int;
}
// --- end p1l9-creates ---
// --- p1l9-updates ---
extern "C" {
    // LifetimeUpdate.cpp and DeletionUpdate.cpp, unchanged (differential/wrap_l9updates.cpp): one module built from the
    // INI text, then `n` ops of 6 words; see the wrapper's header for the words written.
    pub fn ref_l9updates_run(
        kind: c_int,
        ini_text: *const c_char,
        ini_len: c_int,
        seed: c_uint,
        frame: c_uint,
        hulk: c_int,
        override_frames: c_int,
        n: c_int,
        ops: *const c_uint,
        out: *mut c_uint,
        out_cap: c_int,
        out_len: *mut c_int,
        status: *mut c_int,
        code: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        xfer_path: *const c_char,
    ) -> c_int;
    // BaseRegenerateUpdate.cpp (the same wrapper): the module with the global data given, then `n` ops of 6 words.
    pub fn ref_l9updates_regen_run(
        ini_text: *const c_char,
        ini_len: c_int,
        pct: c_float,
        delay: c_uint,
        seed: c_uint,
        frame: c_uint,
        n: c_int,
        ops: *const c_uint,
        out: *mut c_uint,
        out_cap: c_int,
        out_len: *mut c_int,
        status: *mut c_int,
        code: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        xfer_path: *const c_char,
    ) -> c_int;
    // SmartBombTargetHomingUpdate.cpp (the same wrapper): the module from the INI text, then `n` ops of 6 words.
    pub fn ref_l9updates_smart_run(
        ini_text: *const c_char,
        ini_len: c_int,
        seed: c_uint,
        frame: c_uint,
        n: c_int,
        ops: *const c_uint,
        out: *mut c_uint,
        out_cap: c_int,
        out_len: *mut c_int,
        status: *mut c_int,
        code: *mut c_uint,
        msg: *mut c_char,
        msg_cap: c_int,
        xfer_path: *const c_char,
    ) -> c_int;
}
// --- end p1l9-updates ---


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
// --- p1w10-sonnet --- (the layer-9 upgrade batch, differential/l9_upgrades.inc)
extern "C" {
    #[allow(clippy::too_many_arguments)]
    pub fn ref_l9up_run(
        kind: c_int,
        filename: *const c_char,
        has_player: c_int,
        player_mask: *const c_uint,
        obj_mask: *const c_uint,
        cfg: *const c_uint,
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
// --- end p1w10-sonnet ---
// --- p1w10-sonnet --- (the layer-9 helper and die batch, differential/l9_helpers.inc)
extern "C" {
    #[allow(clippy::too_many_arguments)]
    pub fn ref_l9help_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
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
// --- end p1w10-sonnet ---
// --- p1w9-sonnet ---
extern "C" {
    // Team.cpp on stand-ins (wrap_team.cpp, shim_team/team_driver.h)
    pub fn ref_team_run(words: *const c_uint, n: c_int, tmp_path: *const c_char, out: *mut c_char, cap: c_int) -> c_int;
}
// --- end p1w9-sonnet ---
// --- p1w10-sonnet ---
extern "C" {
    // Player.cpp (slice 1) and Team.cpp on stand-ins (wrap_player.cpp, shim_player/player_driver.h)
    pub fn ref_player_run(words: *const c_uint, n: c_int, tmp_path: *const c_char, out: *mut c_char, cap: c_int) -> c_int;
    pub fn ref_ba_run(words: *const c_uint, n: c_int, tmp_path: *const c_char, out: *mut c_char, cap: c_int) -> c_int;
    // ObjectCreationList.cpp on stand-ins (wrap_ocl.cpp, shim_ocl/ocl_driver.h): an empty store, the dump of the store, a program
    pub fn ref_ocl_reset(preload_assets: c_int);
    pub fn ref_ocl_dump(out: *mut c_char, cap: c_int) -> c_int;
    pub fn ref_ocl_run(words: *const c_uint, n: c_int, out: *mut c_char, cap: c_int) -> c_int;
    // the upgrade modules of batch B on stand-ins (l9_upgrades_b.inc in wrap_module.cpp)
    pub fn ref_l9upb_run(
        kind: c_int,
        filename: *const c_char,
        has_player: c_int,
        player_mask: *const c_uint,
        obj_mask: *const c_uint,
        cfg: *const c_uint,
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
// --- end p1w10-sonnet ---
// --- p1w11-sonnet ---
extern "C" {
    // the upgrade modules of batch C on stand-ins (l9_upgrades_c.inc in wrap_module.cpp)
    pub fn ref_l9upc_run(
        kind: c_int,
        filename: *const c_char,
        ctrl: c_int,
        player_mask: *const c_uint,
        obj_mask: *const c_uint,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // ReplaceObjectUpgrade on stand-ins (l9_replace_object.inc in wrap_module.cpp)
    pub fn ref_l9ro_run(
        filename: *const c_char,
        player_mask: *const c_uint,
        obj_mask: *const c_uint,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // the dies of batch D on stand-ins (l9_dies_d.inc in wrap_module.cpp)
    pub fn ref_l9dd_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // the update modules of batch E on stand-ins (l9_updates_e.inc in wrap_module.cpp)
    pub fn ref_l9ue_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // the special powers of batch H on stand-ins (l9_powers_h.inc in wrap_module.cpp)
    // update batch L: AutoDepositUpdate on stand-ins (l9_updates_l.inc in wrap_module.cpp)
    pub fn ref_l9ul_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- p1w12-sonnet ---
    // update batch M: FireOCLAfterWeaponCooldownUpdate on stand-ins (l9_fire_ocl_m.inc in wrap_module.cpp)
    pub fn ref_l9fo_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w12-sonnet ---
    // --- p1w12-sonnet ---
    // update batch M: AutoHealBehavior on stand-ins (l9_auto_heal_m.inc in wrap_module.cpp)
    pub fn ref_l9ah_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w12-sonnet ---
    // --- p1w12-sonnet ---
    // update batch N: CheckpointUpdate on stand-ins (l9_checkpoint_n.inc in wrap_module.cpp)
    pub fn ref_l9cp_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w12-sonnet ---
    // --- p1w12-sonnet ---
    // update batch N: FlammableUpdate on stand-ins (l9_flammable_n.inc in wrap_module.cpp)
    pub fn ref_l9fl_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w12-sonnet ---
    // --- p1w12-sonnet ---
    // update batch N: OCLUpdate on stand-ins (l9_ocl_update_n.inc in wrap_module.cpp)
    pub fn ref_l9oc_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w12-sonnet ---
    // --- p1w13-sonnet ---
    // update batch O: HeightDieUpdate on stand-ins (l9_height_die_o.inc in wrap_module.cpp)
    pub fn ref_l9hd_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // update batch O: DynamicShroudClearingRangeUpdate on stand-ins (l9_dynamic_shroud_o.inc in wrap_module.cpp)
    pub fn ref_l9ds_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet ---
    // update batch O: SquishCollide on stand-ins (l9_squish_collide_o.inc in wrap_module.cpp)
    pub fn ref_l9sq_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet ---
    // update batch O: ProneUpdate on stand-ins (l9_prone_o.inc in wrap_module.cpp)
    pub fn ref_l9pr_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet ---
    // update batch O: EnemyNearUpdate on stand-ins (l9_enemy_near_o.inc in wrap_module.cpp)
    pub fn ref_l9en_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet ---
    // update batch O: FloatUpdate on stand-ins (l9_float_o.inc in wrap_module.cpp)
    pub fn ref_l9flr_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet ---
    // update batch O: TechBuildingBehavior on stand-ins (l9_tech_building_o.inc in wrap_module.cpp)
    pub fn ref_l9tb_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet ---
    // batch P: ActiveBody on stand-ins (l9_active_body_p.inc in wrap_module.cpp)
    pub fn ref_l9ab_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // batch R: OpenContain on stand-ins (l9_open_contain_r.inc in wrap_module.cpp)
    pub fn ref_l9ct_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // batch Q: SlowDeathBehavior on stand-ins (l9_slow_death_q.inc in wrap_module.cpp)
    pub fn ref_l9sd_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet ---
    // update batch O: FireSpreadUpdate on stand-ins (l9_fire_spread_o.inc in wrap_module.cpp)
    pub fn ref_l9fs_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // --- end p1w13-sonnet ---
    // behavior batch K: FireWeaponWhenDeadBehavior on stand-ins (l9_behaviors_k.inc in wrap_module.cpp)
    pub fn ref_l9fk_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // die batch J: FXListDie on stand-ins (l9_dies_j.inc in wrap_module.cpp)
    pub fn ref_l9fj_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // update batch I: SpyVisionUpdate on stand-ins (l9_updates_i.inc in wrap_module.cpp)
    pub fn ref_l9ui_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    pub fn ref_l9sp_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    // the helpers of batch G on stand-ins (l9_helpers_g.inc in wrap_module.cpp)
    pub fn ref_l9hg_run(
        kind: c_int,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
    ) -> c_int;
    // the crate modules of batch F on stand-ins (l9_crates_f.inc in wrap_module.cpp)
    pub fn ref_l9cf_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R13: the sabotage crates (differential/l9_sabotage_r13.inc).
    pub fn ref_l9sab_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R37: PropagandaTowerBehavior (differential/l9_propaganda_r37.inc).
    pub fn ref_l9r37_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R38: BridgeTowerBehavior (differential/l9_bridge_tower_r38.inc).
    pub fn ref_l9r38_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R39: StickyBombUpdate (differential/l9_sticky_bomb_r39.inc).
    pub fn ref_l9r39_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R40: MinefieldBehavior (differential/l9_minefield_r40.inc).
    pub fn ref_l9r40_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R41: GenerateMinefieldBehavior (differential/l9_gen_minefield_r41.inc).
    pub fn ref_l9r41_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R42: ProjectileStreamUpdate (differential/l9_projectile_stream_r42.inc).
    pub fn ref_l9r42_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R43: DockUpdate, RepairDockUpdate (differential/l9_repair_dock_r43.inc).
    pub fn ref_l9r43_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R44: SupplyWarehouseDockUpdate (kind 0), SupplyCenterDockUpdate (1) (differential/l9_supply_docks_r44.inc).
    pub fn ref_l9r44_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R46: JetSlowDeathBehavior (differential/l9_jet_death_r46.inc).
    pub fn ref_l9r46_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R47: BattleBusSlowDeathBehavior (differential/l9_battle_bus_r47.inc).
    pub fn ref_l9r47_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R48: HelicopterSlowDeathBehavior (differential/l9_heli_death_r48.inc).
    pub fn ref_l9r48_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R49: NeutronMissileSlowDeathBehavior (differential/l9_neutron_death_r49.inc).
    pub fn ref_l9r49_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R68: DumbProjectileBehavior (differential/l9_dumb_projectile_r68.inc).
    pub fn ref_l9r68_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R69: CommandButtonHuntUpdate (differential/l9_command_button_hunt_r69.inc).
    pub fn ref_l9r69_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R70: WaveGuideUpdate (differential/l9_wave_guide_r70.inc).
    pub fn ref_l9r70_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R71: BattlePlanUpdate (differential/l9_battle_plan_r71.inc).
    pub fn ref_l9r71_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R74: ParkingPlaceBehavior (differential/l9_parking_r74.inc).
    pub fn ref_l9r74_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R75: ParticleUplinkCannonUpdate (differential/l9_uplink_r75.inc).
    pub fn ref_l9r75_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, layer 10 batch P2: PathfindCellInfo and PathfindCell (differential/wrap_pathfind.cpp).
    pub fn ref_pf_p2_run(cfg: *const c_uint, n: c_int, ops: *const c_uint, buf: *mut c_uint, cap: c_int, len: *mut c_int) -> c_int;
    pub fn ref_pf_p3_run(cfg: *const c_uint, n: c_int, ops: *const c_uint, buf: *mut c_uint, cap: c_int, len: *mut c_int) -> c_int;
    pub fn ref_pf_p4_run(cfg: *const c_uint, n: c_int, ops: *const c_uint, buf: *mut c_uint, cap: c_int, len: *mut c_int) -> c_int;
    pub fn ref_pf_p5_run(cfg: *const c_uint, n: c_int, ops: *const c_uint, buf: *mut c_uint, cap: c_int, len: *mut c_int) -> c_int;
    pub fn ref_pf_p6_run(cfg: *const c_uint, n: c_int, ops: *const c_uint, buf: *mut c_uint, cap: c_int, len: *mut c_int) -> c_int;
    /// p1w13-sonnet, layer 10 batch A2: AIUpdateInterface's members without the state machine (differential/wrap_ai.cpp).
    pub fn ref_ai_a2_reset();
    pub fn ref_ai_a2_load(filename: *const c_char, load_type: c_int) -> c_int;
    pub fn ref_ai_a2_new(sets: *const c_uint, env: *const AiEnv, pos: *const c_float, angle: c_float, log: *mut c_uint, cap: c_int) -> c_int;
    #[allow(clippy::too_many_arguments)]
    pub fn ref_ai_a2_op(
        op: c_int,
        fa: *const c_float,
        ia: *const c_int,
        env: *const AiEnv,
        out: *mut c_uint,
        log: *mut c_uint,
        cap: c_int,
        log_len: *mut c_int,
    ) -> c_int;
    pub fn ref_ai_a2_dump(buf: *mut c_uint, cap: c_int) -> c_int;
    /// Batch A4: the file the machine's xfer goes through (one per test thread).
    pub fn ref_ai_a4_xfer_path(path: *const c_char);
    /// p1w13-sonnet, layer 10 batch T1: TurretAI (differential/wrap_turret.cpp).
    pub fn ref_turret_run(cfg: *const c_uint, data: *const c_uint, n: c_int, ops: *const c_uint, xfer_path: *const c_char, buf: *mut c_uint, cap: c_int, len: *mut c_int) -> c_int;
    /// p1w13-sonnet, layer 10 batch P1: PathNode and Path (differential/wrap_pathfind.cpp).
    pub fn ref_pf_p1_run(xfer_path: *const c_char, cfg: *const c_uint, n: c_int, ops: *const c_uint, buf: *mut c_uint, cap: c_int, len: *mut c_int) -> c_int;
    /// p1w13-sonnet, batch R77: BridgeBehavior (differential/l9_bridge_r77.inc).
    pub fn ref_l9r77_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R76: ProductionUpdate (differential/l9_production_r76.inc).
    pub fn ref_l9r76_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R73: SpawnBehavior (differential/l9_spawn_r73.inc).
    pub fn ref_l9r73_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R72: SpectreGunshipUpdate (differential/l9_spectre_gunship_r72.inc).
    pub fn ref_l9r72_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R67: NeutronMissileUpdate (differential/l9_neutron_missile_r67.inc).
    pub fn ref_l9r67_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R66: PointDefenseLaserUpdate (differential/l9_point_defense_r66.inc).
    pub fn ref_l9r66_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R65: FireWeaponWhenDamagedBehavior (differential/l9_fire_weapon_damaged_r65.inc).
    pub fn ref_l9r65_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R64: FireWeaponUpdate (differential/l9_fire_weapon_update_r64.inc).
    pub fn ref_l9r64_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R62: FireWeaponCollide (differential/l9_fire_weapon_collide_r62.inc).
    pub fn ref_l9r62_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R61: RailedTransportDockUpdate (differential/l9_railed_dock_r61.inc).
    pub fn ref_l9r61_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R60: SpectreGunshipDeploymentUpdate (differential/l9_spectre_deploy_r60.inc).
    pub fn ref_l9r60_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R59: AssistedTargetingUpdate (differential/l9_assisted_r59.inc).
    pub fn ref_l9r59_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R58: MobMemberSlavedUpdate (differential/l9_mob_member_r58.inc).
    pub fn ref_l9r58_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R57: SlavedUpdate (differential/l9_slaved_r57.inc).
    pub fn ref_l9r57_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R56: CleanupHazardUpdate (differential/l9_cleanup_hazard_r56.inc).
    pub fn ref_l9r56_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R55: DemoTrapUpdate (differential/l9_demo_trap_r55.inc).
    pub fn ref_l9r55_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R54: BunkerBusterBehavior (differential/l9_bunker_buster_r54.inc).
    pub fn ref_l9r54_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R53: MissileLauncherBuildingUpdate (differential/l9_missile_launcher_r53.inc).
    pub fn ref_l9r53_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R52: CountermeasuresBehavior (differential/l9_countermeasures_r52.inc).
    pub fn ref_l9r52_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R50: PhysicsBehavior (differential/l9_physics_r50.inc).
    pub fn ref_l9r50_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R45: SupplyCenterProductionExitUpdate (kind 0), SpawnPointProductionExitUpdate (1) (differential/l9_exits_r45.inc).
    pub fn ref_l9r45_run(
        kind: c_int,
        cfg_has_grant: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R36: StructureToppleUpdate (differential/l9_structure_topple_r36.inc).
    pub fn ref_l9r36_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R35: NeutronBlastBehavior (kind 0), BridgeScaffoldBehavior (1) (differential/l9_neutron_scaffold_r35.inc).
    pub fn ref_l9r35_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R34: TensileFormationUpdate (differential/l9_tensile_r34.inc).
    pub fn ref_l9r34_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R33: EMPUpdate, LeafletDropBehavior (differential/l9_emp_r33.inc).
    pub fn ref_l9r33_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R32: FirestormDynamicGeometryInfoUpdate (differential/l9_firestorm_r32.inc).
    pub fn ref_l9r32_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R31: TransitionDamageFX (differential/l9_transition_r31.inc).
    pub fn ref_l9r31_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R30: BoneFXUpdate, BoneFXDamage (differential/l9_bonefx_r30.inc).
    pub fn ref_l9r30_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R29: StructureCollapseUpdate (differential/l9_collapse_r29.inc).
    pub fn ref_l9r29_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R28: ToppleUpdate (differential/l9_topple_r28.inc).
    pub fn ref_l9r28_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R27: StealthDetectorUpdate (differential/l9_stealth_detector_r27.inc).
    pub fn ref_l9r27_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R26: StealthUpdate (differential/l9_stealth_r26.inc).
    pub fn ref_l9r26_run(
        filename: *const c_char,
        filename2: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R25: HordeUpdate (differential/l9_horde_r25.inc).
    pub fn ref_l9r25_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R24: OverchargeBehavior (differential/l9_overcharge_r24.inc).
    pub fn ref_l9r24_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R23: DynamicGeometryInfoUpdate (kind 0), GrantStealthBehavior (kind 1) (differential/l9_geom_stealth_r23.inc).
    pub fn ref_l9r23_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R20: RadiusDecalUpdate (kind 0), AnimationSteeringUpdate (kind 1) (differential/l9_decal_steer_r20.inc).
    pub fn ref_l9r20_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R19: AutoFindHealingUpdate (kind 0), PilotFindVehicleUpdate (kind 1) (differential/l9_find_r19.inc).
    pub fn ref_l9r19_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R18: DefaultProductionExitUpdate (kind 0), QueueProductionExitUpdate (kind 1) (differential/l9_exits_r18.inc).
    pub fn ref_l9r18_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R17: WeaponBonusUpdate (differential/l9_weapon_bonus_r17.inc).
    pub fn ref_l9r17w_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R17: SupplyWarehouseCripplingBehavior (differential/l9_crippling_r17.inc).
    pub fn ref_l9r17_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R16: PoisonedBehavior (differential/l9_poisoned_r16.inc).
    pub fn ref_l9r16p_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R16: InstantDeathBehavior (differential/l9_instant_death_r16.inc).
    pub fn ref_l9r16_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R15: RebuildHoleExposeDie (kind 0), RebuildHoleBehavior (kind 1) (differential/l9_hole_r15.inc).
    pub fn ref_l9r15_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R14c: CreateCrateDie (differential/l9_dies_r14.inc).
    pub fn ref_l9r14c_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        ncr: c_int,
        crates: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R14b: CrushDie (differential/l9_dies_r14.inc).
    pub fn ref_l9r14b_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R14: the die modules (differential/l9_dies_r14.inc).
    pub fn ref_l9r14_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R13e: ConvertToHijackedVehicleCrateCollide and HijackerUpdate (differential/l9_crates_r13e.inc).
    pub fn ref_l9r13e_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R13d: ConvertToCarBombCrateCollide (differential/l9_crates_r13d.inc).
    pub fn ref_l9r13d_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R13c: SalvageCrateCollide (differential/l9_crates_r13c.inc).
    pub fn ref_l9r13c_run(
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
        n: c_int,
        ops: *const c_uint,
        buf: *mut c_uchar,
        cap: c_int,
        len: *mut c_int,
        msg: *mut c_char,
        msg_cap: c_int,
        code: *mut c_uint,
    ) -> c_int;
    /// p1w13-sonnet, batch R13b: UnitCrateCollide and VeterancyCrateCollide (differential/l9_crates_r13b.inc).
    pub fn ref_l9r13b_run(
        kind: c_int,
        filename: *const c_char,
        xfer_path: *const c_char,
        cfg: *const c_uint,
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
// --- end p1w11-sonnet ---
