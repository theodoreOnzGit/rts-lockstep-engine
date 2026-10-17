#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Measure the Phase 1 porting graph of upstream GameLogic.

Source of the numbers in docs/phase1-porting-graph.md. Reads only the pinned
upstream submodule (commit 0a05454d). Everything is measured, nothing is
estimated:

- lines: newline count of each file (what `wc -l` prints);
- dependencies: the `#include` lines of each file (comments stripped),
  resolved case-insensitively against GameEngine/Include, as Windows did;
- a *unit* is a header and the .cpp file(s) of the same basename;
- client/device touches: include lines naming GameClient/, GameNetwork/,
  W3DDevice/, Win32Device/ or an audio header, plus counted references to the
  client-side singletons in CLIENT_GLOBALS (comments stripped).

Graphs. The full include graph (headers and .cpp files) is one big strongly
connected component, because .cpp files include each other's headers both
ways. The *header* graph (edges from .h files only) is a DAG apart from one
3-unit cycle. So levels are computed as: a header's level is 1 + the highest
level among the headers it includes (cycles collapsed); a unit's level is
1 + the highest header level among everything its header AND its .cpp include.
A unit at level n can therefore be translated once every declaration it uses
exists; the method bodies of a cycle are translated together.

Usage, from the repository root:
  python3 -I scripts/porting_graph.py            summary
  python3 -I scripts/porting_graph.py --json     everything, as JSON
  python3 -I scripts/porting_graph.py --table    the per-unit markdown table
"""
import json, os, re, sys
from collections import defaultdict

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "upstream", "GeneralsMD", "Code", "GameEngine")
INC = os.path.join(ROOT, "Include")
SRC = os.path.join(ROOT, "Source")
INCLUDE_RE = re.compile(r'^\s*#\s*include\s*[<"]([^">]+)[">]', re.M)
CLIENT_GLOBALS = [
    "TheGameClient", "TheInGameUI", "TheAudio", "TheTacticalView", "TheParticleSystemManager",
    "TheDisplay", "TheControlBar", "TheEva", "TheNetwork", "TheRadar", "TheWindowManager",
    "TheMouse", "TheKeyboard", "TheShell", "TheTerrainVisual", "TheGhostObjectManager",
    "TheDrawGroupInfo", "TheAnim2DCollection", "TheMappedImageCollection", "TheVideoPlayer",
    "TheFXListStore", "TheGameText", "TheDisplayStringManager", "TheSnowManager",
    "TheMessageStream", "TheCommandList", "TheRecorder", "TheGameSpyInfo", "TheLAN",
]
CLIENT_PREFIXES = ("gameclient/", "gamenetwork/", "w3ddevice/", "win32device/")
AUDIO = {h.lower() for h in ("Common/GameAudio.h", "Common/AudioEventRTS.h", "Common/AudioEventInfo.h",
         "Common/AudioHandleSpecialValues.h", "Common/DynamicAudioEventInfo.h", "Common/GameSounds.h",
         "Common/GameMusic.h", "Common/MiscAudio.h", "Common/AudioAffect.h", "Common/AudioRequest.h",
         "Common/AudioSettings.h", "Common/AudioRandomValue.h", "Common/GameSpeech.h")}

# Infrastructure that Rust replaces instead of translating (the reason is in
# docs/phase1-porting-graph.md). Edges INTO these units are not dependencies
# of the logic port; the units are still listed, with their size.
REPLACED = {
    "gamememory": "memory pools: Rust ownership (Vec/arena + index ids)",
    "debug": "DEBUG_LOG/ASSERT/CRASH: debug_assert!, Result, a log hook",
    "stackdump": "crash stack dumps",
    "systeminfo": "CPU identification",
    "criticalsection": "thread locks: logic is single-threaded",
    "perftimer": "profiling timers",
    "perfmetrics": "profiling counters",
    "minilog": "debug file log",
    "crcdebug": "debug-only CRC logging (DEBUG_CRC builds)",
    "copyprotection": "SafeDisk/launcher check",
    "cdmanager": "CD drive detection",
    "registry": "Windows registry",
    "commandline": "argv parsing: replaced by explicit configuration",
    "unittimings": "profiling",
}

# Translation layers (hand-assigned from the measured graph; see the doc).
# First match wins: a unit name, or a directory prefix of the unit's first
# .cpp (else its header), relative to GameEngine/.
LAYERS = [
    ("D", "done in Phase 0 (fully or partly)", ["randomvalue", "logicrandomvalue", "clientrandomvalue", "audiorandomvalue", "crc", "quicktrig",
        "xfer", "xfercrc", "xferdeepcrc", "xfersave", "xferload", "snapshot", "ini", "datachunk", "file", "filesystem", "localfilesystem",
        "archivefile", "archivefilesystem"]),
    ("R", "replaced, not translated", list(REPLACED) + ["list"]),
    ("C", "client/device/shell side: behind a cut-point interface", ["audioaffect", "audioeventinfo", "audiohandlespecialvalues", "audiosettings",
        "miscaudio", "audioeventrts", "audiorequest", "dynamicaudioeventinfo", "gameaudio", "gamemusic", "gamesounds", "userpreferences",
        "custommatchpreferences", "gamespymiscpreferences", "ignorepreferences", "ladderpreferences", "quickmatchpreferences", "gamelod",
        "osdisplay", "bordercolors", "clientupdatemodule", "drawmodule", "ghostobject", "radar", "statscollector", "gameengine",
        "mapreaderwriterinfo", "version", "quotedprintable", "functionlexicon", "skirmishbattlehonors", "language"]),
    ("1", "value types and enums", ["errors", "gametype", "gamecommon", "objectscriptstatusbits", "powers", "weaponbonusconditionflags",
        "weaponsettype", "weaponstatus", "specialpowertype", "thingsort", "fpucontrol", "discretecircle", "partitionsolver", "latchrestore",
        "overridable", "override", "iniexception"]),
    ("2", "strings, names, subsystem registry", ["asciistring", "unicodestring", "stltypedefs", "namekeygenerator", "wellknownkeys",
        "subsysteminterface"]),
    ("3", "flag sets and small containers", ["bitflags", "bitflagsio", "kindof", "objectstatustypes", "disabledtypes", "modelstate",
        "weaponsetflags", "armorset", "specialpowermasktype", "sparsematchfinder", "dict"]),
    ("4", "static data from INI (templates and stores)", ["geometry", "terraintypes", "science", "upgrade", "money", "handicap", "rankinfo",
        "armor", "damage", "damagefx", "multiplayersettings", "globaldata", "playertemplate", "specialpower", "cratesystem",
        "beziersegment", "bezfwditerator"]),
    ("5", "thing and module system", ["module", "modulefactory", "thing", "thingtemplate", "thingfactory", "behaviormodule", "bodymodule",
        "collidemodule", "containmodule", "createmodule", "damagemodule", "destroymodule", "diemodule", "updatemodule", "upgrademodule",
        "specialpowermodule", "specialpowerupdatemodule", "objectiter", "statemachine"]),
    ("6", "map data and terrain", ["mapobject", "sideslist", "polygontrigger", "terrainlogic", "scripts"]),
    ("7", "players, teams and economy", ["player", "playerlist", "team", "energy", "scorekeeper", "missionstats", "academystats",
        "resourcegatheringmanager", "productionprerequisite", "tunneltracker", "cavesystem", "actionmanager", "buildassistant"]),
    ("8", "object core, weapons, movement, partition", ["object", "objecttypes", "experiencetracker", "firingtracker", "weapon", "weaponset",
        "locomotorset", "locomotor", "partitionmanager", "simpleobjectiterator", "objectcreationlist"]),
    ("10", "AI and pathfinding", ["GameLogic/AI/", "GameLogic/Object/Update/AIUpdate/", "aistatemachine", "aiupdate", "turretai"]),
    ("9", "object modules", ["GameLogic/Object/", "GameLogic/Module/"]),
    ("11", "map scripts and victory", ["scriptengine", "scriptactions", "scriptconditions", "victoryconditions"]),
    ("12", "game loop, commands, replay, save", ["gamelogic", "gamelogicdispatch", "messagestream", "recorder", "gamestate", "gamestatemap"]),
]

def text(p):
    with open(p, "rb") as f:
        t = f.read().decode("latin-1")
    t = re.sub(r"/\*.*?\*/", " ", t, flags=re.S)
    return re.sub(r"//[^\n]*", "", t)

def lines(p):
    with open(p, "rb") as f:
        return f.read().count(b"\n")

def rel(p):
    return os.path.relpath(p, os.path.join(ROOT, "..")).replace(os.sep, "/")

def unit_of(p):
    return os.path.splitext(os.path.basename(p))[0].lower()

headers, cpps = {}, defaultdict(list)
for d, _, fs in os.walk(INC):
    for f in fs:
        if f.lower().endswith(".h"):
            p = os.path.join(d, f)
            headers[os.path.relpath(p, INC).replace(os.sep, "/").lower()] = p
for d, _, fs in os.walk(SRC):
    for f in fs:
        if f.lower().endswith(".cpp"):
            cpps[unit_of(f)].append(os.path.join(d, f))
INCS = {}
def incs(p):
    if p not in INCS:
        INCS[p] = INCLUDE_RE.findall(text(p))
    return INCS[p]
PRERTS = [i for i in incs(os.path.join(INC, "Precompiled", "PreRTS.h")) if "/" in i]

def in_scope(p):
    r = rel(p)
    return "/GameLogic/" in r or "/Common/" in r

# Seed with all of GameLogic plus PreRTS's engine headers; close over the
# Common/ and GameLogic/ headers reached, adding the .cpp of each header.
files = set()
for base in (os.path.join(SRC, "GameLogic"), os.path.join(INC, "GameLogic")):
    for d, _, fs in os.walk(base):
        files.update(os.path.join(d, f) for f in fs if f.lower().endswith((".cpp", ".h")))
for i in PRERTS:
    if i.lower() in headers:
        files.add(headers[i.lower()])
work = list(files)
while work:
    p = work.pop()
    for i in incs(p):
        il = i.lower()
        if il.startswith(("common/", "gamelogic/")) and il in headers:
            h = headers[il]
            for c in [h] + cpps.get(unit_of(h), []):
                if c not in files and in_scope(c):
                    files.add(c); work.append(c)
hfiles = {p for p in files if p.lower().endswith(".h")}

def resolved(p):
    """(in-scope header deps, client includes, audio includes) of one file."""
    deps, cl, au = set(), set(), set()
    for i in incs(p):
        il = i.lower()
        if il == "prerts.h":
            continue
        if il.startswith(CLIENT_PREFIXES):
            cl.add(i)
        elif il in AUDIO:
            au.add(i)
        elif il in headers and headers[il] in hfiles and headers[il] != p and unit_of(headers[il]) not in REPLACED:
            deps.add(headers[il])
    return deps, cl, au

# Header levels on the header graph, cycles collapsed (Tarjan).
hdeps = {h: resolved(h)[0] for h in hfiles}
index, low, stack, on, comp, sccs, n = {}, {}, [], set(), {}, [], [0]
def tarjan(v0):
    work = [(v0, iter(sorted(hdeps[v0])))]
    index[v0] = low[v0] = n[0]; n[0] += 1; stack.append(v0); on.add(v0)
    while work:
        v, it = work[-1]
        w = next(it, None)
        if w is None:
            work.pop()
            if work:
                low[work[-1][0]] = min(low[work[-1][0]], low[v])
            if low[v] == index[v]:
                c = []
                while True:
                    x = stack.pop(); on.discard(x); comp[x] = len(sccs); c.append(x)
                    if x == v:
                        break
                sccs.append(c)
        elif w not in index:
            index[w] = low[w] = n[0]; n[0] += 1; stack.append(w); on.add(w)
            work.append((w, iter(sorted(hdeps[w]))))
        elif w in on:
            low[v] = min(low[v], index[w])
for h in sorted(hfiles):
    if h not in index:
        tarjan(h)
clevel = {}
for ci, c in enumerate(sccs):  # reverse topological order
    clevel[ci] = max([clevel[comp[d]] + 1 for v in c for d in hdeps[v] if comp[d] != ci] or [0])
hlevel = {h: clevel[comp[h]] for h in hfiles}

units = defaultdict(lambda: {"files": [], "lines": 0, "h_lines": 0, "cpp_lines": 0, "deps": set(),
                             "client": set(), "audio": set(), "globals": defaultdict(int)})
for p in sorted(files):
    U = units[unit_of(p)]
    U["files"].append(rel(p))
    k = lines(p)
    U["lines"] += k
    U["h_lines" if p in hfiles else "cpp_lines"] += k
    d, cl, au = resolved(p)
    U["deps"].update(d); U["client"].update(cl); U["audio"].update(au)
    t = text(p)
    for g in CLIENT_GLOBALS:
        c = len(re.findall(r"\b" + g + r"\b", t))
        if c:
            U["globals"][g] += c
out = {"prerts": PRERTS, "header_cycles": [sorted(rel(h) for h in c) for c in sccs if len(c) > 1], "units": {}}
rdeps = defaultdict(set)
for u, U in units.items():
    for d in U["deps"]:
        if unit_of(d) != u:
            rdeps[unit_of(d)].add(u)
for u, U in sorted(units.items()):
    own = [d for d in U["deps"] if unit_of(d) != u]
    out["units"][u] = {
        "files": U["files"], "lines": U["lines"], "h_lines": U["h_lines"], "cpp_lines": U["cpp_lines"],
        "deps": sorted({unit_of(d) for d in own}), "rdeps": sorted(rdeps[u]),
        "client_includes": sorted(U["client"]), "audio_includes": sorted(U["audio"]),
        "client_globals": dict(sorted(U["globals"].items())),
        "level": 1 + max([hlevel[d] for d in own] or [-1]),
        "replaced": REPLACED.get(u),
    }

def layer_of(u):
    U = out["units"][u]
    f = [x for x in U["files"] if x.endswith(".cpp")] or U["files"]
    d = f[0].replace("GameEngine/Source/", "").replace("GameEngine/Include/", "")
    for key, _, rules in LAYERS:
        for r in rules:
            if (r.endswith("/") and d.startswith(r)) or r == u:
                return key
    return "?"
for u in out["units"]:
    out["units"][u]["layer"] = layer_of(u)
ORDER = {k: i for i, (k, _, _) in enumerate(LAYERS)}
NUM = {k: (int(k) if k.isdigit() else (0 if k == "D" else -1)) for k, _, _ in LAYERS}
# Upward edges: a unit that includes the header of a unit in a LATER
# numbered layer. Each is a cut point (an interface or a deferred call).
upward = []
for u, U in out["units"].items():
    lu = NUM[U["layer"]]
    if lu <= 0:
        continue
    for d in U["deps"]:
        ld = NUM[out["units"][d]["layer"]]
        if ld > lu:
            upward.append((u, d))
out["upward_edges"] = sorted(upward)

if "--layers" in sys.argv:
    print("| layer | contents | units | files | lines (h+cpp) | client/audio/net-touching units | upward includes |")
    print("|---|---|---:|---:|---:|---:|---:|")
    for key, desc, _ in sorted(LAYERS, key=lambda l: (NUM[l[0]] if NUM[l[0]] >= 0 else 99, l[0])):
        us = [U for U in out["units"].values() if U["layer"] == key]
        h = sum(U["h_lines"] for U in us); c = sum(U["cpp_lines"] for U in us)
        t = sum(1 for U in us if U["client_includes"] or U["audio_includes"] or U["client_globals"])
        up = sum(1 for a, _ in upward if out["units"][a]["layer"] == key)
        print(f"| {key} | {desc} | {len(us)} | {sum(len(U['files']) for U in us)} | {h}+{c} = {h + c} | {t} | {up} |")
    miss = [u for u, U in out["units"].items() if U["layer"] == "?"]
    if miss:
        print("UNASSIGNED:", miss)
    sys.exit(0)
if "--upward" in sys.argv:
    for a, b in upward:
        print(f"{out['units'][a]['layer']:>3} {a} -> {out['units'][b]['layer']:>3} {b}")
    sys.exit(0)
if "--json" in sys.argv:
    json.dump(out, sys.stdout, indent=1)
elif "--table" in sys.argv:
    up_of = defaultdict(list)
    for a, b in upward:
        up_of[a].append(b)
    def how(u, U):
        """Differential-test route, by rule from the measured facts (rules in the doc)."""
        if U["layer"] == "R":
            return "none: replaced"
        if U["layer"] == "C":
            return "none: interface; logic tests record its calls"
        touch = bool(U["client_includes"] or U["audio_includes"] or U["client_globals"])
        if U["cpp_lines"] == 0:
            r = "header: through a wrapper that instantiates it"
        else:
            r = "direct: compile unchanged with shims"
        if up_of[u]:
            r += f"; stand-ins for {len(up_of[u])} later-layer unit(s)"
        if touch:
            r += "; client stand-ins that log calls"
        return r
    print("| unit | layer | files | lines (h+cpp) | level | depends on (units) | used by (units) | client/audio/net touch | differential route |")
    print("|---|---|---|---:|---:|---|---|---|---|")
    for u, U in sorted(out["units"].items(), key=lambda kv: (NUM[kv[1]["layer"]] if NUM[kv[1]["layer"]] >= 0 else 99, kv[1]["layer"], kv[0])):
        touch = []
        if U["client_includes"]:
            touch.append("inc " + ", ".join(i.split("/")[-1] for i in U["client_includes"]))
        if U["audio_includes"]:
            touch.append("audio " + ", ".join(i.split("/")[-1] for i in U["audio_includes"]))
        if U["client_globals"]:
            touch.append("refs " + ", ".join(f"{g}x{c}" for g, c in U["client_globals"].items()))
        files_s = "<br>".join(f.replace("GameEngine/", "") for f in U["files"])
        deps_s = ", ".join(U["deps"]) if len(U["deps"]) <= 12 else ", ".join(U["deps"][:12]) + f", ... ({len(U['deps'])})"
        rd_s = ", ".join(U["rdeps"]) if len(U["rdeps"]) <= 8 else ", ".join(U["rdeps"][:8]) + f", ... ({len(U['rdeps'])})"
        print(f"| `{u}` | {U['layer']} | {files_s} | {U['h_lines']}+{U['cpp_lines']} | {U['level']} | {deps_s or '-'} | {rd_s or '-'} | {'; '.join(touch) or 'none'} | {how(u, U)} |")
else:
    us = out["units"]
    print(f"units {len(us)}, files {len(files)}, lines {sum(U['lines'] for U in us.values())}")
    print("header cycles:", out["header_cycles"])
    by = defaultdict(lambda: [0, 0])
    for U in us.values():
        by[U["level"]][0] += 1; by[U["level"]][1] += U["lines"]
    for l in sorted(by):
        print(f"level {l:>2}: {by[l][0]:>3} units {by[l][1]:>7} lines")
    print("units touching client/audio/net:", sum(1 for U in us.values() if U["client_includes"] or U["audio_includes"] or U["client_globals"]))
