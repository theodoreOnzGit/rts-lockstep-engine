/* Differential-test wrapper for the game's level of detail (GitHub #160, a part of #100; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/GameLOD.cpp, from `#define PROFILE_ERROR_LIMIT` to the end (the field
**     tables, the names, the INI handlers, GameLODManager and its info structs' constructors)
**   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp's W3DDisplay::updateAverageFPS
** cut by build.rs into OUT_DIR/game_lod_gen (see its comment: verbatim but for seven mechanical renames, each asserted
** once: the five INI handlers and the two INI level parsers made file-static functions, because the INI class is the
** real INI.h's, whose members another archive defines; wrap_ini.cpp's handlers reach these through
** ref_game_lod_block_hook, which ref_game_lod_run sets), compiled with the REAL Common/GameLOD.h and the REAL INI reader
** (wrap_ini.cpp's upstream_ini) in the campaign build's setup.
**
** Every class and global those files define is renamed here (Glod...), so that nothing clashes with the stand-in
** GameLODManager of other archives.
**
** Everything below is a STAND-IN, written for this test:
** - ParticlePriorityType and ParticlePriorityNames: copied from GameClient/ParticleSys.h lines 85-112 and 275-278 (the
**   real header needs the particle system);
** - TheGlobalData (TheWritableGlobalData): the fields GameLOD.cpp reads and writes, set by the case;
** - OptionPreferences: a map of the case's Options.ini entries that each construction reads and `write()` keeps; its
**   getStaticGameDetail and getIdealStaticGameDetail are copies of OptionsMenu.cpp lines 409-425; the custom level's
**   getters (getTextureReduction ... getTreesEnabled) answer the case's values;
** - testMinimumRequirements: the case's machine; an unknown speed, memory or benchmark leaves the pointer unwritten
**   (the Rust client's `None`); recorded;
** - TheGameClient (adjustLOD, releaseShadows, allocateShadows) and TheTerrainVisual (setShoreLineDetail,
**   setTerrainTracksDetail): recorded;
** - fopen / fclose (the Benchmark.txt dump): a memory stream, its text recorded;
** - the W3DDisplay of updateAverageFPS: m_averageFPS only; getPerformanceCounter / getPerformanceCounterFrequency the
**   case's ticks. updateAverageFPS's statics live for the whole test (the Rust test keeps one AverageFps as long);
** - the manager is constructed (placement new) on bytes filled with 0xA5: the members its constructor leaves
**   uninitialised (m_minDynamicParticlePriority, m_minDynamicParticleSkipPriority) read 0xA5A5A5A5 until a dynamic level
**   is applied, which the dump prints as `?` (the Rust's `None`, undefined upstream: never compared as a value).
**
** Entry point (extern "C"):
**   ref_game_lod_run(in, out, cap)  runs one case (tests/differential_game_lod.rs's text); the dump's length. */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <new>
#include <string>
#include <vector>
#include <map>
#include <typeinfo>
#include <cxxabi.h>
#include "Common/INI.h"
#include "Common/INIException.h"

#define GameLODManager GlodGameLODManager
#define TheGameLODManager GlodTheGameLODManager
#define StaticGameLODInfo GlodStaticGameLODInfo
#define DynamicGameLODInfo GlodDynamicGameLODInfo
#define LODPresetInfo GlodLODPresetInfo
#define BenchProfile GlodBenchProfile
#define OptionPreferences GlodOptionPreferences
#define GlobalData GlodGlobalData
#define TheGlobalData GlodTheGlobalData
#define GameClient GlodGameClient
#define TheGameClient GlodTheGameClient
#define TerrainVisual GlodTerrainVisual
#define TheTerrainVisual GlodTheTerrainVisual
#define testMinimumRequirements glodTestMinimumRequirements
#define W3DDisplay GlodW3DDisplay

/* ---- GameClient/ParticleSys.h lines 85-112 (copied) ---- */
enum ParticlePriorityType
{
	INVALID_PRIORITY = 0,
	PARTICLE_PRIORITY_LOWEST = 1,
	WEAPON_EXPLOSION = PARTICLE_PRIORITY_LOWEST,
	SCORCHMARK,
	DUST_TRAIL,
	BUILDUP,
	DEBRIS_TRAIL,
	UNIT_DAMAGE_FX,
	DEATH_EXPLOSION,
	SEMI_CONSTANT,
	CONSTANT,
	WEAPON_TRAIL,
	AREA_EFFECT,
	CRITICAL,
	ALWAYS_RENDER,
	PARTICLE_PRIORITY_HIGHEST = ALWAYS_RENDER,
	NUM_PARTICLE_PRIORITIES
};
/* ---- GameClient/ParticleSys.h lines 275-278 (copied) ---- */
static char *ParticlePriorityNames[] =
{
	"NONE", "WEAPON_EXPLOSION","SCORCHMARK","DUST_TRAIL","BUILDUP","DEBRIS_TRAIL","UNIT_DAMAGE_FX","DEATH_EXPLOSION","SEMI_CONSTANT","CONSTANT","WEAPON_TRAIL","AREA_EFFECT","CRITICAL", "ALWAYS_RENDER", NULL
};

#define private public
#define protected public
#include "Common/GameLOD.h"
#undef private
#undef protected

namespace {
std::string g_out;
void outf(const char *fmt, ...)
{
	char b[2048];
	va_list a;
	va_start(a, fmt);
	vsnprintf(b, sizeof b, fmt, a);
	va_end(a);
	g_out += b;
}
unsigned bits(Real r) { unsigned u; memcpy(&u, &r, 4); return u; }
Real real(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
std::string unhex(const char *h)
{
	std::string s;
	if (h[0] == '-' && h[1] == 0) return s;
	for (size_t i = 0; h[i] && h[i + 1]; i += 2) { unsigned v; sscanf(h + i, "%2x", &v); s += (char)v; }
	return s;
}
}

/* ---- TheGlobalData ---- */
struct GlodGlobalData
{
	Bool m_forceBenchmark, m_enableStaticLOD, m_enableDynamicLOD;
	Int m_textureReductionFactor, m_maxParticleCount, m_maxTankTrackEdges, m_maxTankTrackOpaqueEdges, m_maxTankTrackFadeDelay;
	Bool m_useShadowVolumes, m_useShadowDecals, m_enableBehindBuildingMarkers, m_useFpsLimit, m_useLightMap, m_useCloudMap;
	Bool m_showSoftWaterEdge, m_useHeatEffects, m_useDrawModuleLOD, m_useTreeSway, m_useTrees, m_shellMapOn;
};
static GlodGlobalData g_gd;
static GlodGlobalData *TheGlobalData = &g_gd;
#define TheWritableGlobalData TheGlobalData

/* ---- OptionPreferences ---- */
namespace { std::map<std::string, std::string> g_optionsFile; Int g_custom[13]; }
class GlodOptionPreferences
{
public:
	std::map<std::string, std::string> m_map;
	GlodOptionPreferences() : m_map(g_optionsFile) {}
	AsciiString &operator[](const char *key) { m_store[key] = AsciiString(); m_keys.push_back(key); return m_store[key]; }
	Bool write()
	{
		for (size_t i = 0; i < m_keys.size(); ++i) m_map[m_keys[i]] = m_store[m_keys[i]].str();
		g_optionsFile = m_map;
		outf("host write");
		for (std::map<std::string, std::string>::iterator it = m_map.begin(); it != m_map.end(); ++it) outf(" %s=%s", it->first.c_str(), it->second.c_str());
		outf("\n");
		return TRUE;
	}
	/* OptionsMenu.cpp lines 409-425 (copied; `find` / `end` on the map) */
	Int getIdealStaticGameDetail(void)
	{
		std::map<std::string, std::string>::const_iterator it = m_map.find("IdealStaticGameLOD");
		if (it == m_map.end())
			return STATIC_GAME_LOD_UNKNOWN;

		return TheGameLODManager->getStaticGameLODIndex(AsciiString(it->second.c_str()));
	}
	Int getStaticGameDetail(void)
	{
		std::map<std::string, std::string>::const_iterator it = m_map.find("StaticGameLOD");
		if (it == m_map.end())
			return TheGameLODManager->getStaticLODLevel();

		return TheGameLODManager->getStaticGameLODIndex(AsciiString(it->second.c_str()));
	}
	Int getTextureReduction(void) { return g_custom[0]; }
	Bool get3DShadowsEnabled(void) { return g_custom[1]; }
	Bool get2DShadowsEnabled(void) { return g_custom[2]; }
	Bool getBuildingOcclusionEnabled(void) { return g_custom[3]; }
	Int getParticleCap(void) { return g_custom[4]; }
	Bool getDynamicLODEnabled(void) { return g_custom[5]; }
	Bool getFPSLimitEnabled(void) { return g_custom[6]; }
	Bool getLightmapEnabled(void) { return g_custom[7]; }
	Bool getCloudShadowsEnabled(void) { return g_custom[8]; }
	Bool getSmoothWaterEnabled(void) { return g_custom[9]; }
	Bool getUseHeatEffects(void) { return g_custom[10]; }
	Bool getExtraAnimationsDisabled(void) { return g_custom[11]; }
	Bool getTreesEnabled(void) { return g_custom[12]; }
private:
	std::map<std::string, AsciiString> m_store;
	std::vector<std::string> m_keys;
};

/* ---- the machine ---- */
namespace {
Int g_video, g_cpu, g_freq, g_ram, g_hasFreq, g_hasRam, g_hasBench;
Real g_bench[3];
}
Bool glodTestMinimumRequirements(ChipsetType *videoChipType, CpuType *cpuType, Int *cpuFreq, Int *numRAM, Real *intBenchIndex, Real *floatBenchIndex, Real *memBenchIndex)
{
	if (videoChipType) { *videoChipType = (ChipsetType)g_video; outf("host video\n"); }
	if (cpuType) { *cpuType = (CpuType)g_cpu; outf("host cpu\n"); }
	if (cpuFreq && g_hasFreq) *cpuFreq = g_freq;
	if (numRAM && g_hasRam) *numRAM = g_ram;
	if (intBenchIndex && floatBenchIndex && memBenchIndex)
	{
		outf("host bench\n");
		if (g_hasBench) { *intBenchIndex = g_bench[0]; *floatBenchIndex = g_bench[1]; *memBenchIndex = g_bench[2]; }
	}
	return TRUE;
}
class GlodGameClient
{
public:
	void adjustLOD(Int adj) { outf("host adjustLOD %d\n", adj); }
	void releaseShadows() { outf("host shadows\n"); }
	void allocateShadows() {}
};
static GlodGameClient g_client;
static GlodGameClient *TheGameClient = &g_client;
class GlodTerrainVisual
{
public:
	void setShoreLineDetail() { outf("host shore\n"); }
	void setTerrainTracksDetail() { outf("host tracks\n"); }
};
static GlodTerrainVisual g_terrain;
static GlodTerrainVisual *TheTerrainVisual = &g_terrain;

/* ---- Benchmark.txt ---- */
namespace { char *g_benchBuf; size_t g_benchLen; }
static FILE *glodFopen(const char *, const char *) { return open_memstream(&g_benchBuf, &g_benchLen); }
static int glodFclose(FILE *f)
{
	int r = fclose(f);
	outf("host benchfile %s\n", g_benchBuf ? g_benchBuf : "");
	free(g_benchBuf);
	g_benchBuf = 0;
	return r;
}
#define fopen glodFopen
#define fclose glodFclose

#include "game_lod_cut.inc"

#undef fopen
#undef fclose

/* ---- W3DDisplay::updateAverageFPS ---- */
namespace { long long g_now, g_freq64; }
class GlodW3DDisplay
{
public:
	Real m_averageFPS;
	void updateAverageFPS(void);
};
inline Int64 getPerformanceCounter() { return g_now; }
inline Int64 getPerformanceCounterFrequency() { return g_freq64; }
#include "game_lod_fps.inc"
static GlodW3DDisplay g_display;

namespace {
void blockStatic(INI *ini) { glodParseStaticGameLODDefinition(ini); }
void blockDynamic(INI *ini) { glodParseDynamicGameLODDefinition(ini); }
void blockPreset(INI *ini) { glodParseLODPreset(ini); }
void blockBench(INI *ini) { glodParseBenchProfile(ini); }
void blockMHz(INI *ini) { glodParseReallyLowMHz(ini); }
alignas(16) unsigned char g_mgrStorage[sizeof(GameLODManager)];

void dumpPriority(Int v) { if (v == (Int)0xA5A5A5A5) outf(" ?"); else outf(" %d", v); }

void dump()
{
	GameLODManager *m = TheGameLODManager;
	outf("mgr %d %d %d %d %d %d %08x", m->m_currentStaticLOD, m->m_currentDynamicLOD, m->m_numParticleGenerations, m->m_dynamicParticleSkipMask,
		m->m_numDebrisGenerations, m->m_dynamicDebrisSkipMask, bits(m->m_slowDeathScale));
	dumpPriority((Int)m->m_minDynamicParticlePriority);
	dumpPriority((Int)m->m_minDynamicParticleSkipPriority);
	outf(" %d %d %d %d %d %d %d %d %d %d %d %d %d", m->m_videoPassed, m->m_cpuPassed, m->m_memPassed, m->m_numLevelPresets[0], m->m_numLevelPresets[1],
		m->m_numLevelPresets[2], m->m_numBenchProfiles, m->m_idealDetailLevel, m->m_videoChipType, m->m_cpuType, m->m_numRAM, m->m_cpuFreq,
		m->m_currentTextureReduction);
	outf(" %08x %08x %08x %08x %d\n", bits(m->m_intBenchIndex), bits(m->m_floatBenchIndex), bits(m->m_memBenchIndex), bits(m->m_compositeBenchIndex), m->m_reallyLowMHz);
	for (int i = 0; i < STATIC_GAME_LOD_COUNT; ++i)
	{
		const StaticGameLODInfo &l = m->m_staticGameLODInfo[i];
		outf("static %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n", i, l.m_minFPS, l.m_minProcessorFPS, l.m_sampleCount2D, l.m_sampleCount3D,
			l.m_streamCount, l.m_maxParticleCount, l.m_useShadowVolumes, l.m_useShadowDecals, l.m_useCloudMap, l.m_useLightMap, l.m_showSoftWaterEdge,
			l.m_maxTankTrackEdges, l.m_maxTankTrackOpaqueEdges, l.m_maxTankTrackFadeDelay, l.m_useBuildupScaffolds, l.m_useTreeSway,
			l.m_useEmissiveNightMaterials, l.m_useHeatEffects, l.m_textureReduction, l.m_useFpsLimit, l.m_enableDynamicLOD, l.m_useTrees);
	}
	for (int i = 0; i < DYNAMIC_GAME_LOD_COUNT; ++i)
	{
		const DynamicGameLODInfo &l = m->m_dynamicGameLODInfo[i];
		outf("dynamic %d %d %u %u %08x %d %d\n", i, l.m_minFPS, l.m_dynamicParticleSkipMask, l.m_dynamicDebrisSkipMask, bits(l.m_slowDeathScale),
			(Int)l.m_minDynamicParticlePriority, (Int)l.m_minDynamicParticleSkipPriority);
	}
	for (int i = 0; i < STATIC_GAME_LOD_CUSTOM; ++i)
		for (int j = 0; j < m->m_numLevelPresets[i] && j < MAX_LOD_PRESETS_PER_LEVEL; ++j)
		{
			const LODPresetInfo &p = m->m_lodPresets[i][j];
			outf("preset %d %d %d %d %d %d\n", i, j, (Int)p.m_cpuType, p.m_mhz, (Int)p.m_videoType, p.m_memory);
		}
	for (int k = 0; k < m->m_numBenchProfiles && k < MAX_BENCH_PROFILES; ++k)
	{
		const BenchProfile &b = m->m_benchProfiles[k];
		outf("bench %d %d %d %08x %08x %08x\n", k, (Int)b.m_cpuType, b.m_mhz, bits(b.m_intBenchIndex), bits(b.m_floatBenchIndex), bits(b.m_memBenchIndex));
	}
	const GlodGlobalData &g = g_gd;
	outf("gd %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n", g.m_forceBenchmark, g.m_enableStaticLOD, g.m_enableDynamicLOD,
		g.m_textureReductionFactor, g.m_maxParticleCount, g.m_maxTankTrackEdges, g.m_maxTankTrackOpaqueEdges, g.m_maxTankTrackFadeDelay,
		g.m_useShadowVolumes, g.m_useShadowDecals, g.m_enableBehindBuildingMarkers, g.m_useFpsLimit, g.m_useLightMap, g.m_useCloudMap,
		g.m_showSoftWaterEdge, g.m_useHeatEffects, g.m_useDrawModuleLOD, g.m_useTreeSway, g.m_useTrees, g.m_shellMapOn);
}

/* the INI's throw, as wrap_ini.cpp's CATCH_ALL reports it: 1 INIException (its message), 2 an INI_* status, 3 an
** ErrorCode, 5 other */
#define GLOD_CATCH                                                                                     \
	catch (INIException &e) { outf("throw 1 %s\n", e.mFailureMessage ? e.mFailureMessage : ""); }  \
	catch (ErrorCode c) { outf("throw 3 %u\n", (unsigned)c); }                                       \
	catch (...) {                                                                                    \
		const std::type_info *t = abi::__cxa_current_exception_type();                               \
		if (t && dynamic_cast<const abi::__enum_type_info *>(t)) outf("throw 2 %u\n", (unsigned)ERROR_BAD_INI); \
		else outf("throw 5\n");                                                                      \
	}
} /* namespace */

extern "C" {
extern void (*ref_game_lod_block_hook[5])(INI *);
void ref_ini_reset(void);
void ref_ini_add_file(const char *name, const char *data, int len);

int ref_game_lod_run(const char *in, char *out, int cap)
{
	g_out.clear();
	ref_ini_reset();
	ref_game_lod_block_hook[0] = blockStatic;
	ref_game_lod_block_hook[1] = blockDynamic;
	ref_game_lod_block_hook[2] = blockPreset;
	ref_game_lod_block_hook[3] = blockBench;
	ref_game_lod_block_hook[4] = blockMHz;
	g_optionsFile.clear();
	memset(g_mgrStorage, 0xA5, sizeof g_mgrStorage);
	TheGameLODManager = new (g_mgrStorage) GameLODManager;
	const char *p = in;
	char line[65536];
	while (*p)
	{
		size_t n = strcspn(p, "\n");
		if (n >= sizeof line) n = sizeof line - 1;
		memcpy(line, p, n);
		line[n] = 0;
		p += n;
		if (*p == '\n') ++p;
		char op[32] = "", a[65536] = "", b[65536] = "";
		sscanf(line, "%31s", op);
		std::string cmd(op);
		const char *rest = line + strlen(op);
		while (*rest == ' ') ++rest;
		if (cmd == "file")
		{
			sscanf(rest, "%65535s %65535s", a, b);
			std::string data = unhex(b);
			ref_ini_add_file(a, data.data(), (int)data.size());
			continue;
		}
		if (cmd == "opt")
		{
			sscanf(rest, "%65535s %65535s", a, b);
			g_optionsFile[a] = unhex(b);
			continue;
		}
		if (cmd == "machine")
		{
			unsigned bi, bf, bm;
			sscanf(rest, "%d %d %d %d %d %d %d %x %x %x", &g_video, &g_cpu, &g_hasFreq, &g_freq, &g_hasRam, &g_ram, &g_hasBench, &bi, &bf, &bm);
			g_bench[0] = real(bi); g_bench[1] = real(bf); g_bench[2] = real(bm);
			continue;
		}
		if (cmd == "custom")
		{
			sscanf(rest, "%d %d %d %d %d %d %d %d %d %d %d %d %d", &g_custom[0], &g_custom[1], &g_custom[2], &g_custom[3], &g_custom[4], &g_custom[5],
				&g_custom[6], &g_custom[7], &g_custom[8], &g_custom[9], &g_custom[10], &g_custom[11], &g_custom[12]);
			continue;
		}
		if (cmd == "gd")
		{
			int v[20];
			sscanf(rest, "%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d", &v[0], &v[1], &v[2], &v[3], &v[4], &v[5], &v[6], &v[7], &v[8], &v[9],
				&v[10], &v[11], &v[12], &v[13], &v[14], &v[15], &v[16], &v[17], &v[18], &v[19]);
			GlodGlobalData &g = g_gd;
			g.m_forceBenchmark = v[0]; g.m_enableStaticLOD = v[1]; g.m_enableDynamicLOD = v[2]; g.m_textureReductionFactor = v[3];
			g.m_maxParticleCount = v[4]; g.m_maxTankTrackEdges = v[5]; g.m_maxTankTrackOpaqueEdges = v[6]; g.m_maxTankTrackFadeDelay = v[7];
			g.m_useShadowVolumes = v[8]; g.m_useShadowDecals = v[9]; g.m_enableBehindBuildingMarkers = v[10]; g.m_useFpsLimit = v[11];
			g.m_useLightMap = v[12]; g.m_useCloudMap = v[13]; g.m_showSoftWaterEdge = v[14]; g.m_useHeatEffects = v[15]; g.m_useDrawModuleLOD = v[16];
			g.m_useTreeSway = v[17]; g.m_useTrees = v[18]; g.m_shellMapOn = v[19];
			continue;
		}
		/* the operations: each logged with its result, then the dump */
		GameLODManager *m = TheGameLODManager;
		outf("> %s\n", line);
		int x = 0;
		unsigned u = 0;
		if (cmd == "init") { try { m->init(); outf("ok\n"); } GLOD_CATCH }
		else if (cmd == "load")
		{
			sscanf(rest, "%65535s", a);
			try { INI ini; ini.load(AsciiString(a), INI_LOAD_OVERWRITE, NULL); outf("ok\n"); } GLOD_CATCH
		}
		else if (cmd == "find") outf("= %d\n", (Int)m->findStaticLODLevel());
		else if (cmd == "set") { sscanf(rest, "%d", &x); outf("= %d\n", m->setStaticLODLevel((StaticGameLODLevel)x)); }
		else if (cmd == "dfind") { sscanf(rest, "%x", &u); outf("= %d\n", (Int)m->findDynamicLODLevel(real(u))); }
		else if (cmd == "dset") { sscanf(rest, "%d", &x); outf("= %d\n", m->setDynamicLODLevel((DynamicGameLODLevel)x)); }
		else if (cmd == "rec") outf("= %d\n", m->getRecommendedTextureReduction());
		else if (cmd == "ltr") { sscanf(rest, "%d", &x); outf("= %d\n", m->getLevelTextureReduction((StaticGameLODLevel)x)); }
		else if (cmd == "ctr") { sscanf(rest, "%d", &x); m->setCurrentTextureReduction(x); outf("= %d\n", m->getCurrentTextureReduction()); }
		else if (cmd == "pskip" || cmd == "dskip")
		{
			sscanf(rest, "%d", &x);
			outf("=");
			for (int i = 0; i < x; ++i) outf(" %d", cmd == "pskip" ? m->isParticleSkipped() : m->isDebrisSkipped());
			outf("\n");
		}
		else if (cmd == "sidx") { sscanf(rest, "%65535s", a); outf("= %d\n", m->getStaticGameLODIndex(AsciiString(unhex(a).c_str()))); }
		else if (cmd == "didx") { sscanf(rest, "%65535s", a); outf("= %d\n", m->getDynamicGameLODIndex(AsciiString(unhex(a).c_str()))); }
		else if (cmd == "sname") { sscanf(rest, "%d", &x); outf("= %s\n", m->getStaticGameLODLevelName((StaticGameLODLevel)x)); }
		else if (cmd == "dname") { sscanf(rest, "%d", &x); outf("= %s\n", m->getDynamicGameLODLevelName((DynamicGameLODLevel)x)); }
		else if (cmd == "rlow") { sscanf(rest, "%d", &x); m->setReallyLowMHz(x); outf("= %d %d\n", m->isReallyLowMHz(), m->didMemPass()); }
		else if (cmd == "flags") { int s, d; sscanf(rest, "%d %d", &s, &d); g_gd.m_enableStaticLOD = s; g_gd.m_enableDynamicLOD = d; outf("ok\n"); }
		else if (cmd == "frame")
		{
			/* W3DDisplay::draw: updateAverageFPS, then the dynamic LOD block (W3DDisplay.cpp lines 1675-1684) */
			int show;
			sscanf(rest, "%lld %lld %d", &g_now, &g_freq64, &show);
			g_display.updateAverageFPS();
			outf("= %08x\n", bits(g_display.m_averageFPS));
			if (TheGlobalData->m_enableDynamicLOD && show)
			{
				DynamicGameLODLevel lod=TheGameLODManager->findDynamicLODLevel(g_display.m_averageFPS);
				TheGameLODManager->setDynamicLODLevel(lod);
			}
			else
			{	//if dynamic LOD is turned off, force highest LOD
				TheGameLODManager->setDynamicLODLevel(DYNAMIC_GAME_LOD_VERY_HIGH);
			}
		}
		else if (cmd == "dinit")
		{
			/* W3DDisplay::init (W3DDisplay.cpp lines 782-784) */
			if (TheGameLODManager->getStaticLODLevel() == STATIC_GAME_LOD_UNKNOWN)
				TheGameLODManager->setStaticLODLevel(TheGameLODManager->findStaticLODLevel());
			outf("ok\n");
		}
		else outf("unknown op\n");
		dump();
	}
	int len = (int)g_out.size();
	if (len > cap) len = cap;
	memcpy(out, g_out.data(), len);
	return (int)g_out.size();
}
}
