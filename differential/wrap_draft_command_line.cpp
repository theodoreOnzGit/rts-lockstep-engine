/* Differential-test wrapper for the command line draft (#215, upstream commit 0a05454d), src/drafts/logic_common/command_line.rs:
**   GeneralsMD/Code/GameEngine/Source/Common/CommandLine.cpp from `Bool TheDebugIgnoreSyncErrors = FALSE;` to the end of the file
**   (ConvertShortMapPathToLongMapPath, every handler, the `params` table and parseCommandLine), cut verbatim by build.rs into
**   command_line.inc. It is compiled as the SHIPPED game is, with NO _DEBUG and NO _INTERNAL: the preprocessor keeps the release
**   branches and the release table (-noshellmap -win -xres -yres -fullscreen -fullVersion -particleEdit -scriptDebug -playStats -mod
**   -noshaders -quickstart). Handlers that exist only inside `#if _DEBUG` (parseFPSLimit among them) are not in the C++ here.
**
** Stand-ins (everything the cut calls outside itself): GlobalData, reduced to the members the release handlers write, with the
** types of GlobalData.h, and getPath_UserData(); TheWritableGlobalData and TheGlobalData (one object); AsciiString is upstream's own
** (AsciiString.cpp unchanged, renamed by build.rs); TheVersion->setShowFullVersion (recorded); TheArchiveFileSystem->loadMods (counted);
** TheLocalFileSystem->doesFileExist and `_stat` (answers computed from a hash of the path and the case's seed, the same function the
** test uses); `strnicmp` is strncasecmp; DX8Wrapper_PreserveFPU. DEBUG_LOG and DEBUG_CRASH are the shim's release no-ops.
**
** Entry point (extern "C", tests/differential_draft_command_line.rs):
**   ref_cl_run(handler, argc, argv, init, strs, seed, out): a fresh GlobalData set from `init` (25 Bools, then x_resolution,
**   y_resolution, play_stats, net_min_players, frames_per_second_limit, fixed_seed, chip_set_type, no_draw) and `strs` (map name,
**   shell map name, mod dir, mod big, user data dir: NUL-terminated, one after another); argv is the `argc` strings joined by NULs.
**   handler -1 runs parseCommandLine(argc, argv); k runs the k-th handler of the table below on (argv, argc). Writes [return value
**   (0 for parseCommandLine), the 25 Bools, the 8 numbers, the 4 strings as a length and bytes, TheDebugIgnoreSyncErrors,
**   DX8Wrapper_PreserveFPU, the last setShowFullVersion argument (2 if never called), the number of loadMods calls]. */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include "Common/Debug.h"
#include <string.h>
#include <stdlib.h>
#include <strings.h>
#include <vector>
#define NULL 0
#define strnicmp strncasecmp
#define TRUE true
#define FALSE false

/* every stand-in and the cut are in an anonymous namespace: the engine classes of these names (GlobalData ...) are defined by other
** differential wrappers with other bodies */
namespace {

struct GlobalData
{
	Bool m_windowed, m_winCursors, m_musicOn, m_videoOn, m_useShadowVolumes, m_useShadowDecals, m_useFpsLimit, m_useFX, m_speechOn, m_soundsOn;
	Bool m_showTeamDot, m_shouldUpdateTGAToDDS, m_shellMapOn, m_scriptDebug, m_playSizzle, m_particleEdit, m_loadScreenDemo, m_incrementalAGPBuf;
	Bool m_forceBenchmark, m_dumpAssetUsage, m_disableCameraMovement, m_constantDebugUpdate, m_buildMapCache, m_audioOn, m_animateWindows;
	Int m_xResolution, m_yResolution, m_playStats, m_netMinPlayers, m_framesPerSecondLimit, m_fixedSeed, m_chipSetType;
	UnsignedInt m_noDraw;
	AsciiString m_mapName, m_shellMapName, m_modDir, m_modBIG, m_userData;
	AsciiString getPath_UserData() const { return m_userData; }
};
GlobalData theGD;
GlobalData *TheWritableGlobalData = &theGD;
GlobalData *TheGlobalData = &theGD;
Int DX8Wrapper_PreserveFPU = 0;
int theShowFull = 2;
int theLoadMods = 0;
unsigned theSeed = 0;

struct VersionStandIn { void setShowFullVersion(Bool v) { theShowFull = v ? 1 : 0; } };
VersionStandIn theVersion;
VersionStandIn *TheVersion = &theVersion;
struct ArchiveFileSystemStandIn { void loadMods() { ++theLoadMods; } };
ArchiveFileSystemStandIn theAFS;
ArchiveFileSystemStandIn *TheArchiveFileSystem = &theAFS;

unsigned hash_path(const char *p, unsigned seed)
{
	unsigned h = 2166136261u ^ seed;
	for (; *p; ++p) { h ^= (unsigned char)*p; h *= 16777619u; }
	return h;
}
struct LocalFileSystemStandIn { bool doesFileExist(const char *p) { return (hash_path(p, theSeed) >> 7) % 4 != 0; } };
LocalFileSystemStandIn theLFS;
LocalFileSystemStandIn *TheLocalFileSystem = &theLFS;
struct _stat { int st_mode; };
const int _S_IFDIR = 0x4000;
int _stat(const char *p, struct _stat *s)
{
	unsigned h = hash_path(p, theSeed ^ 0x5bd1e995u);
	if ((h >> 5) % 5 == 0) return -1;
	s->st_mode = (h >> 11) % 2 ? _S_IFDIR : 0x8000;
	return 0;
}

#include "command_line.inc"

typedef Int (*Handler)(char *[], int);
Handler theHandlers[] = {
	parseNoLogOrCrash, parseFPUPreserve, parseFullVersion, parseMapName, parseNoFPSLimit, parseJumpToFrame, parseSync, parseShellMap,
	parseQuickStart, parseMod, parseNoMusic, parseNoVideo, parseNoAudio, parseNoWin, parseNoShadows, parseScriptDebug, parseParticleEdit,
	parseBuildMapCache, parseNoFX, parseForceBenchmark, parseNoMoveCamera, parseNoShellMap, parseNoShaders, parseNoSizzle,
	parseNoWindowAnimation, parseWinCursors, parseConstantDebug, parseShowTeamDot, parseIncrAGPBuf, parseDemoLoadScreen,
	parseDumpAssetUsage, parseUpdateImages, parseWin, parseXRes, parseYRes, parseSeed, parseNetMinPlayers, parsePlayStats,
};

void put_str(const AsciiString &s, unsigned *out, int &n)
{
	int len = s.getLength();
	out[n++] = len;
	for (int i = 0; i < len; ++i) out[n++] = (unsigned char)s.str()[i];
}

}

extern "C" int ref_cl_run(int handler, int argc, const char *argv_joined, const unsigned *init, const char *strs, unsigned seed, unsigned *out)
{
	theSeed = seed;
	theShowFull = 2;
	theLoadMods = 0;
	TheDebugIgnoreSyncErrors = false;
	DX8Wrapper_PreserveFPU = 0;
	GlobalData &g = theGD;
	Bool *bools[25] = { &g.m_windowed, &g.m_winCursors, &g.m_musicOn, &g.m_videoOn, &g.m_useShadowVolumes, &g.m_useShadowDecals, &g.m_useFpsLimit,
		&g.m_useFX, &g.m_speechOn, &g.m_soundsOn, &g.m_showTeamDot, &g.m_shouldUpdateTGAToDDS, &g.m_shellMapOn, &g.m_scriptDebug, &g.m_playSizzle,
		&g.m_particleEdit, &g.m_loadScreenDemo, &g.m_incrementalAGPBuf, &g.m_forceBenchmark, &g.m_dumpAssetUsage, &g.m_disableCameraMovement,
		&g.m_constantDebugUpdate, &g.m_buildMapCache, &g.m_audioOn, &g.m_animateWindows };
	for (int i = 0; i < 25; ++i) *bools[i] = init[i] != 0;
	g.m_xResolution = (Int)init[25]; g.m_yResolution = (Int)init[26]; g.m_playStats = (Int)init[27]; g.m_netMinPlayers = (Int)init[28];
	g.m_framesPerSecondLimit = (Int)init[29]; g.m_fixedSeed = (Int)init[30]; g.m_chipSetType = (Int)init[31]; g.m_noDraw = init[32];
	AsciiString *ss[5] = { &g.m_mapName, &g.m_shellMapName, &g.m_modDir, &g.m_modBIG, &g.m_userData };
	for (int i = 0; i < 5; ++i) { *ss[i] = AsciiString(strs); strs += strlen(strs) + 1; }
	std::vector<std::vector<char> > args;
	std::vector<char *> argv;
	const char *p = argv_joined;
	for (int i = 0; i < argc; ++i) { size_t l = strlen(p); args.push_back(std::vector<char>(p, p + l + 1)); p += l + 1; }
	for (int i = 0; i < argc; ++i) argv.push_back(&args[i][0]);
	argv.push_back(0);
	int ret = 0;
	if (handler < 0) parseCommandLine(argc, &argv[0]);
	else ret = theHandlers[handler](&argv[0], argc);
	int n = 0;
	out[n++] = ret;
	for (int i = 0; i < 25; ++i) out[n++] = *bools[i] ? 1 : 0;
	out[n++] = g.m_xResolution; out[n++] = g.m_yResolution; out[n++] = g.m_playStats; out[n++] = g.m_netMinPlayers;
	out[n++] = g.m_framesPerSecondLimit; out[n++] = g.m_fixedSeed; out[n++] = g.m_chipSetType; out[n++] = g.m_noDraw;
	for (int i = 0; i < 4; ++i) put_str(*ss[i], out, n);
	out[n++] = TheDebugIgnoreSyncErrors ? 1 : 0;
	out[n++] = DX8Wrapper_PreserveFPU;
	out[n++] = theShowFull;
	out[n++] = theLoadMods;
	return n;
}
