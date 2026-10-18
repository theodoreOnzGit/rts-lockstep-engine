#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Generate the mechanical parts of the GlobalData translation.

Reads only the pinned upstream submodule (commit 0a05454d):

  GeneralsMD/Code/GameEngine/Include/Common/GlobalData.h    the members
  GeneralsMD/Code/GameEngine/Source/Common/GlobalData.cpp   the GameData parse table
  GeneralsMD/Code/GameEngine/{Source,Include}/GameLogic/    which members game logic names

and writes, for the retail configuration (no `_DEBUG`, `_INTERNAL`,
`DUMP_PERF_STATS`, `ALLOW_SURRENDER` or `_ALLOW_DEBUG_CHEATS_IN_RELEASE`, as
the shipped game and the differential build are compiled):

  src/common/global_data/fields.rs        struct GlobalData (every member), the
                                          zero-filled value, the parse table
  differential/globaldata_dump.inc        the C++ side of the field-by-field dump
  tests/support/global_data_dump.rs       the Rust side of the same dump

The constructor's defaults are NOT generated: they are translated by hand in
`src/common/global_data.rs`, so that the differential test checks them against
upstream's constructor instead of against this script.

Usage, from the repository root:
  python3 -I scripts/gen_global_data.py          write the three files
  python3 -I scripts/gen_global_data.py --check  fail if they are out of date
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CODE = os.path.join(ROOT, "upstream/GeneralsMD/Code/GameEngine")
HEADER = os.path.join(CODE, "Include/Common/GlobalData.h")
SOURCE = os.path.join(CODE, "Source/Common/GlobalData.cpp")
LOGIC_DIRS = [os.path.join(CODE, "Source/GameLogic"), os.path.join(CODE, "Include/GameLogic")]

OUT_FIELDS = os.path.join(ROOT, "src/common/global_data/fields.rs")
OUT_CPP_DUMP = os.path.join(ROOT, "differential/globaldata_dump.inc")
OUT_RS_DUMP = os.path.join(ROOT, "tests/support/global_data_dump.rs")

# No macro of the debug/internal configurations is defined.
DEFINED = set()

# Rust names that the generic snake_case rule would spell badly.
RENAME = {
    "m_3DSoundPref": "sound_3d_pref",
    "m_sounds3DOn": "sounds_3d_on",
    "m_TiVOFastMode": "tivo_fast_mode",
    "m_use3WayTerrainBlends": "use_3way_terrain_blends",
}

# Enumerators used as array indices in the parse table.
INDEX = {
    "TIME_OF_DAY_MORNING": 1,
    "TIME_OF_DAY_AFTERNOON": 2,
    "TIME_OF_DAY_EVENING": 3,
    "TIME_OF_DAY_NIGHT": 4,
    "LEVEL_VETERAN": 1,
    "LEVEL_ELITE": 2,
    "LEVEL_HEROIC": 3,
    "PLAYER_HUMAN": 0,
    "PLAYER_COMPUTER": 1,
    "DIFFICULTY_EASY": 0,
    "DIFFICULTY_NORMAL": 1,
    "DIFFICULTY_HARD": 2,
}
# Array extents named by constants.
EXTENT = {
    "MAX_WATER_GRID_SETTINGS": 4,
    "MAX_GLOBAL_LIGHTS": 3,
    "TIME_OF_DAY_COUNT": 5,
    "PLAYERTYPE_COUNT": 2,
    "DIFFICULTY_COUNT": 3,
    "LEVEL_COUNT": 4,
}

# C++ type -> (Rust type, dump kind). Enumerations are stored as `Int`.
TYPES = {
    "Bool": "bool",
    "Int": "i32",
    "UnsignedInt": "u32",
    "Real": "f32",
    "Short": "i16",
    "UnsignedByte": "u8",
    "Color": "i32",
    "AsciiString": "Vec<u8>",
    "Coord3D": "Coord3D",
    "Coord2D": "Coord2D",
    "RGBColor": "RGBColor",
    "Money": "Money",
    "TimeOfDay": "i32",
    "Weather": "i32",
    "_TerrainLOD": "i32",
    "AIDebugOptions": "i32",
    "BodyDamageType": "i32",
    "TerrainLighting": "TerrainLighting",
    "std::vector<AsciiString>": "Vec<Vec<u8>>",
    "WeaponBonusSet*": "WeaponBonusSet",
}
# Types that are zero-filled by `Default`.
ENUM_TYPES = {"TimeOfDay", "Weather", "_TerrainLOD", "AIDebugOptions", "BodyDamageType"}


def read(path):
    with open(path, encoding="latin-1") as f:
        return f.read()


def strip_comments(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


def snake(cpp):
    if cpp in RENAME:
        return RENAME[cpp]
    name = cpp[2:] if cpp.startswith("m_") else cpp
    name = re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", name)
    return name.lower()


def preprocess(lines):
    """Keep the lines of the retail configuration: every `#if`/`#ifdef` here
    tests a macro that is not defined (or, with `#else`, its negation)."""
    out = []
    stack = []  # (active_parent, cond_true, in_else)
    active = True
    for line in lines:
        s = line.strip()
        m = re.match(r"#\s*(ifdef|ifndef|if|else|endif)\b(.*)", s)
        if not m:
            if active:
                out.append(line)
            continue
        kind, rest = m.group(1), m.group(2).strip()
        if kind in ("ifdef", "ifndef", "if"):
            if kind == "ifdef":
                cond = rest in DEFINED
            elif kind == "ifndef":
                cond = rest not in DEFINED
            else:
                expr = re.sub(r"defined\s*\(\s*(\w+)\s*\)", lambda g: str(g.group(1) in DEFINED), rest)
                expr = expr.replace("||", " or ").replace("&&", " and ")
                cond = bool(eval(expr, {"True": True, "False": False}))
            stack.append((active, cond))
            active = active and cond
        elif kind == "else":
            parent, cond = stack[-1]
            active = parent and not cond
        else:
            parent, _ = stack.pop()
            active = parent
    assert not stack, "unbalanced conditionals"
    return out


def parse_members():
    """The data members of `class GlobalData`, in declaration order:
    [(cpp_name, cpp_type, dims, comment)]."""
    text = read(HEADER)
    start = text.index("\tAsciiString m_mapName;")
    end = text.index("\tAsciiString m_modBIG;") + len("\tAsciiString m_modBIG;")
    body = text[start:end]
    body = re.sub(r"/\*.*?\*/", "", body, flags=re.S)
    lines = preprocess(body.split("\n"))
    members = []
    for line in lines:
        s = line.strip()
        if not s or s.startswith("//") or s.startswith("enum") or s.startswith("{") or s.startswith("}"):
            continue
        comment = ""
        if "//" in s:
            s, comment = s.split("//", 1)
            comment = comment.lstrip("/<! \t").strip()
            s = s.strip()
        m = re.match(r"^([\w:<>\*_]+(?:\s*\*)?)\s+(m_\w+)((?:\s*\[[^\]]+\])*)\s*;$", s)
        if not m:
            # the dangling continuation lines of the multi-line comment above
            if s.endswith(";") is False:
                continue
            raise SystemExit(f"cannot parse member line: {line!r}")
        ctype, name, dims = m.group(1).strip(), m.group(2), m.group(3)
        extents = [EXTENT[d.strip()] if not d.strip().isdigit() else int(d.strip()) for d in re.findall(r"\[\s*([^\]]+?)\s*\]", dims)]
        members.append((name, ctype, extents, comment))
    return members


def parse_table():
    """The retail GameData table: [(token, parser, user_data, member_path)]."""
    text = read(SOURCE)
    a = text.index("s_GlobalDataFieldParseTable[] =")
    b = re.compile(r"\{\s*NULL\s*,\s*NULL\s*,\s*NULL\s*,\s*0\s*\}").search(text, a).start()
    body = strip_comments("\n".join(preprocess(text[a:b].split("\n"))))
    entries = []
    for m in re.finditer(
        r'\{\s*"(\w+)"\s*,\s*([\w:]+)\s*,\s*(\w+)\s*,\s*offsetof\(\s*GlobalData\s*,\s*([^)]+?)\s*\)\s*\}',
        body,
    ):
        token, parser, user, path = m.groups()
        path = re.sub(r"\[\s*([A-Za-z_]\w*)\s*\]", lambda g: f"[{INDEX[g.group(1)]}]", path)
        path = re.sub(r"\s+", "", path)
        entries.append((token, parser, user, path))
    # every `{` of an entry must have been matched
    assert body.count("{") == len(entries) + 1, (body.count("{"), len(entries))
    return entries


def logic_reads(members):
    """Members named as `->m_x` on a code (non-comment) line of GameLogic."""
    names = set()
    for d in LOGIC_DIRS:
        for dirpath, _, files in os.walk(d):
            for f in files:
                if not f.endswith((".cpp", ".h")):
                    continue
                for line in read(os.path.join(dirpath, f)).split("\n"):
                    code = line.split("//")[0]
                    for m in re.finditer(r"(?:TheGlobalData|TheWritableGlobalData)\s*->\s*(m_\w+)", code):
                        names.add(m.group(1))
    return [n for n, _, _, _ in members if n in names], sorted(names - {n for n, _, _, _ in members})


def rust_type(ctype, extents):
    t = TYPES[ctype]
    for e in reversed(extents):
        t = f"[{t}; {e}]"
    return t


def gen_fields(members, table, logic):
    o = []
    w = o.append
    w("// SPDX-License-Identifier: GPL-3.0-only")
    w("//")
    w("// GENERATED by scripts/gen_global_data.py from (EA's release,")
    w("// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit")
    w("// 0a05454d8574207440a5fb15241b98ad0b435590):")
    w("//   GeneralsMD/Code/GameEngine/Include/Common/GlobalData.h")
    w("//   GeneralsMD/Code/GameEngine/Source/Common/GlobalData.cpp (the GameData parse table)")
    w("// Do not edit; edit the script and run it again (`--check` verifies this file).")
    w("//")
    w("// Original copyright notice of those files:")
    w("//   Command & Conquer Generals Zero Hour(tm)")
    w("//   Copyright 2025 Electronic Arts Inc.")
    w("//   (c) 2001-2003 Electronic Arts Inc.")
    w("//   Author: trolfs, Michael Booth, Colin Day, April 2001")
    w("//")
    w("// Licensed under the GNU General Public License version 3 (GPL-3.0-only for")
    w("// this translation), with the additional terms in NOTICE.md.")
    w("// Translated to Rust and modified; not the original program.")
    w("")
    w("//! The members of `GlobalData`, its zero-filled value and the `GameData` parse")
    w("//! table, for the retail configuration. See [`super`] for what they mean and how")
    w("//! they are used.")
    w("")
    w("use super::{BODY_DAMAGE_TYPE_NAMES, TERRAIN_LOD_NAMES};")
    w("use crate::common::base_type::{Coord2D, Coord3D, RGBColor};")
    w("use crate::common::game_type::{TIME_OF_DAY_NAMES, WEATHER_NAMES};")
    w("use crate::common::ini::{FieldParse, Ini, UserData};")
    w("use crate::common::money::{parse_money_amount, Money};")
    w("use crate::logic::weapon_bonus::WeaponBonusSet;")
    w("")
    w("/// `struct GlobalData::TerrainLighting`: one global light's colours and direction.")
    w("#[derive(Clone, Copy, Debug, Default, PartialEq)]")
    w("pub struct TerrainLighting {")
    w("    /// `ambient`.")
    w("    pub ambient: RGBColor,")
    w("    /// `diffuse`.")
    w("    pub diffuse: RGBColor,")
    w("    /// `lightPos`.")
    w("    pub light_pos: Coord3D,")
    w("}")
    w("")
    w("/// The members that game logic names (`TheGlobalData->m_x` or `TheWritableGlobalData->m_x` on a")
    w("/// code line of `Source/GameLogic` or `Include/GameLogic`; whether the line is compiled in the")
    w("/// retail game is not evaluated), by upstream name. Later layers: these are the ones logic")
    w("/// code may read; every other member is client, network or tool state.")
    w("pub const READ_BY_GAME_LOGIC: [&str; %d] = [" % len(logic[0]))
    for n in logic[0]:
        w(f'    "{n}",')
    w("];")
    w("")
    w("/// `class GlobalData`'s data members, retail configuration, in upstream's order. Members marked")
    w("/// **Read by game logic** are in [`READ_BY_GAME_LOGIC`]. Strings are the bytes of an")
    w("/// `AsciiString` (no NUL).")
    w("#[derive(Clone, Debug, PartialEq)]")
    w("pub struct GlobalData {")
    logic_set = set(logic[0])
    for name, ctype, extents, comment in members:
        doc = f"`{name}`"
        if comment:
            doc += f": {comment}"
        if not doc.endswith((".", "?", ")")):
            doc += "."
        if name in logic_set:
            doc += " **Read by game logic.**"
        w(f"    /// {doc}")
        w(f"    pub {snake(name)}: {rust_type(ctype, extents)},")
    w("    /// `m_userDataDir`: private upstream (\"no one is allowed to change it, ever\"), read by")
    w("    /// `getPath_UserData()`; see [`GlobalData::get_path_user_data`].")
    w("    pub(super) user_data_dir: Vec<u8>,")
    w("}")
    w("")
    w("impl GlobalData {")
    w("    /// Every member zero, false or empty: what `NEW GlobalData` (the engine's allocator zero-fills")
    w("    /// its blocks) holds before the constructor body runs. Members that the constructor does not")
    w("    /// set stay this way. `WeaponBonusSet` is the exception: its own constructor sets every")
    w("    /// bonus to 1.")
    w("    pub(super) fn zero_filled() -> Self {")
    w("        GlobalData {")
    for name, ctype, extents, _ in members:
        w(f"            {snake(name)}: Default::default(),")
    w("            user_data_dir: Vec::new(),")
    w("        }")
    w("    }")
    w("}")
    w("")
    w("/// `s_GlobalDataFieldParseTable` (retail): %d fields, in upstream's order, then the terminator." % len(table))
    w("pub static GLOBAL_DATA_FIELDS: [FieldParse<GlobalData>; %d] = [" % (len(table) + 1))
    for token, parser, user, path in table:
        rpath = "g." + path_to_rust(path, members)
        member = path.split("[")[0].split(".")[0]
        ctype = next(t for n, t, _, _ in members if n == path_member(path))
        w("    " + entry(token, parser, user, rpath, ctype))
    w("    FieldParse::end(),")
    w("];")
    return "\n".join(o) + "\n"


def path_member(path):
    return re.match(r"(m_\w+)", path).group(1)


def path_to_rust(path, members):
    m = re.match(r"(m_\w+)(.*)", path)
    return snake(m.group(1)) + m.group(2).replace("lightPos", "light_pos")


def entry(token, parser, user, rpath, ctype):
    names = {
        "TerrainLODNames": "TERRAIN_LOD_NAMES",
        "TimeOfDayNames": "TIME_OF_DAY_NAMES",
        "WeatherNames": "WEATHER_NAMES",
        "TheBodyDamageTypeNames": "BODY_DAMAGE_TYPE_NAMES",
    }
    ud = "UserData::None" if user == "NULL" else f"UserData::Names(&{names[user]})"
    if parser == "INI::parseBool":
        if ctype == "Bool":
            body = f"{rpath} = ini.parse_bool()?;"
        elif ctype == "Real":
            # parseBool writes one byte into a Real (little endian: its low byte)
            body = f"{rpath} = f32::from_bits(({rpath}.to_bits() & !0xff) | u32::from(ini.parse_bool()?));"
        else:
            assert ctype == "AIDebugOptions", ctype
            # ... and one byte into an enumeration
            body = f"{rpath} = ({rpath} & !0xff) | i32::from(ini.parse_bool()?);"
    elif parser == "INI::parseInt":
        cast = {"Int": "", "UnsignedInt": " as u32", "Short": " as i16"}[ctype]
        body = f"{rpath} = ini.parse_int()?{cast};"
    elif parser == "INI::parseUnsignedByte":
        assert ctype == "UnsignedByte"
        body = f"{rpath} = ini.parse_unsigned_byte()?;"
    elif parser in ("INI::parseReal", "INI::parseAngleReal", "INI::parseAccelerationReal", "INI::parsePercentToReal"):
        assert ctype == "Real"
        fn = {
            "INI::parseReal": "parse_real",
            "INI::parseAngleReal": "parse_angle_real",
            "INI::parseAccelerationReal": "parse_acceleration_real",
            "INI::parsePercentToReal": "parse_percent_to_real",
        }[parser]
        body = f"{rpath} = ini.{fn}()?;"
    elif parser == "INI::parseDurationUnsignedInt":
        assert ctype == "UnsignedInt"
        body = f"{rpath} = ini.parse_duration_unsigned_int()?;"
    elif parser == "INI::parseColorInt":
        assert ctype == "Color"
        body = f"{rpath} = ini.parse_color_int()?;"
    elif parser == "INI::parseAsciiString":
        assert ctype == "AsciiString"
        body = f"{rpath} = ini.parse_ascii_string()?;"
    elif parser == "INI::parseAsciiStringVectorAppend":
        body = f"ini.parse_ascii_string_vector_append(&mut {rpath})?;"
    elif parser == "INI::parseIndexList":
        body = f"{rpath} = ini.parse_index_list(ud.names())?;"
    elif parser == "INI::parseRGBColor":
        body = f"{rpath} = ini.parse_rgb_color()?;"
    elif parser == "INI::parseCoord3D":
        # upstream stores each component as it is scanned, so a failing Y leaves X stored
        body = (
            f'{rpath}.x = Ini::scan_real(&ini.get_next_sub_token(b"X")?)?; '
            f'{rpath}.y = Ini::scan_real(&ini.get_next_sub_token(b"Y")?)?; '
            f'{rpath}.z = Ini::scan_real(&ini.get_next_sub_token(b"Z")?)?;'
        )
    elif parser == "INI::parseCoord2D":
        body = (
            f'{rpath}.x = Ini::scan_real(&ini.get_next_sub_token(b"X")?)?; '
            f'{rpath}.y = Ini::scan_real(&ini.get_next_sub_token(b"Y")?)?;'
        )
    elif parser == "Money::parseMoneyAmount":
        body = f"parse_money_amount(ini, &mut {rpath}, ud)?;"
    elif parser == "WeaponBonusSet::parseWeaponBonusSetPtr":
        body = f"{rpath}.parse_weapon_bonus_set(ini)?;"
    else:
        raise SystemExit(f"unhandled parser {parser}")
    uses_ud = "ud" in re.findall(r"\bud\b", body)
    return f'FieldParse::new("{token}", |ini, g, {"ud" if uses_ud else "_"}| {{ {body} Ok(()) }}, {ud}),'


# ---- the dumps -------------------------------------------------------------


def gen_cpp_dump(members):
    o = ["/* GENERATED by scripts/gen_global_data.py (retail configuration); do not edit.",
         "** Included by differential/wrap_globaldata.cpp, inside the function that dumps one",
         "** GlobalData: every data member, in the order of GlobalData.h, through putAny. */"]
    for name, _, _, _ in members:
        o.append(f"putAny(g.{name});")
    o.append("putAny(g.m_userDataDir);")
    return "\n".join(o) + "\n"


KINDS = {
    "INI::parseBool": "bool",
    "INI::parseInt": "int",
    "INI::parseUnsignedByte": "ubyte",
    "INI::parseReal": "real",
    "INI::parseAngleReal": "angle",
    "INI::parseAccelerationReal": "accel",
    "INI::parsePercentToReal": "percent",
    "INI::parseDurationUnsignedInt": "duration",
    "INI::parseColorInt": "color",
    "INI::parseAsciiString": "string",
    "INI::parseAsciiStringVectorAppend": "strings",
    "INI::parseRGBColor": "rgb",
    "INI::parseCoord3D": "coord3d",
    "INI::parseCoord2D": "coord2d",
    "Money::parseMoneyAmount": "money",
    "WeaponBonusSet::parseWeaponBonusSetPtr": "weaponbonus",
}


def gen_rs_dump(members, table):
    o = ["// SPDX-License-Identifier: GPL-3.0-only",
         "// GENERATED by scripts/gen_global_data.py (retail configuration); do not edit.",
         "// The Rust side of `differential/wrap_globaldata.cpp`'s dump: every data member of a",
         "// `GlobalData`, in the order of `GlobalData.h`, as the C++ writes it (`putAny`).",
         "// Included (`include!`) by tests/differential_globaldata.rs, which defines `Dump`.",
         "",
         "/// Every member of `g`, in upstream's order; `marks` gets the offset at which each begins (the",
         "/// message of a mismatch names the member).",
         "pub fn dump_fields(g: &GlobalData, d: &mut Vec<u8>, marks: &mut Vec<(usize, &'static str)>) {"]
    for name, _, _, _ in members:
        o.append(f'    marks.push((d.len(), "{snake(name)}"));')
        o.append(f"    g.{snake(name)}.dump(d);")
    o.append('    marks.push((d.len(), "user_data_dir"));')
    o.append("    g.get_path_user_data().to_vec().dump(d);")
    o.append("}")
    o.append("")
    o.append("/// The kind of value each GameData token takes (by the upstream parser), in table order: the test")
    o.append("/// draws its values from this.")
    o.append("const FIELD_KINDS: [(&str, &str); %d] = [" % len(table))
    for token, parser, user, _ in table:
        kind = "index:" + user if parser == "INI::parseIndexList" else KINDS[parser]
        o.append(f'    ("{token}", "{kind}"),')
    o.append("];")
    return "\n".join(o) + "\n"


def main():
    members = parse_members()
    table = parse_table()
    logic = logic_reads(members)
    outputs = {
        OUT_FIELDS: gen_fields(members, table, logic),
        OUT_CPP_DUMP: gen_cpp_dump(members),
        OUT_RS_DUMP: gen_rs_dump(members, table),
    }
    if "--check" in sys.argv:
        bad = [p for p, t in outputs.items() if not os.path.exists(p) or read(p) != t]
        if bad:
            raise SystemExit("out of date: " + ", ".join(os.path.relpath(p, ROOT) for p in bad))
        return
    for p, t in outputs.items():
        with open(p, "w", encoding="latin-1") as f:
            f.write(t)
    print(f"{len(members)} members, {len(table)} GameData fields, {len(logic[0])} read by game logic")
    print("named by logic but not members (compiled out or stale):", ", ".join(logic[1]))


if __name__ == "__main__":
    main()
