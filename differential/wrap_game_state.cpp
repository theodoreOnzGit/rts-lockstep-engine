/* Differential-test wrapper for GameState (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/Common/GameState.h            (unchanged, renamed by the macros below)
**   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp, the pieces build.rs cuts verbatim into
**   OUT_DIR/gstate_gen/game_state_cut.inc (see build.rs)
** on stand-ins for the global data (the user data path, the map name), the map cache's two directories (MapUtil.cpp
** lines 355-365, copied), the world dict's map name, the campaign manager, the partition manager's update, GetLocalTime
** and _access; the Xfer classes are the Xfer build's (XferSave, XferLoad), whose GameState stand-in forwards the
** post-process requests here (g_xferAddPostProcessHook). The test's snapshots are logging blocks of an Int and a string.
**
** Batch M2 adds GameStateMap.cpp from its global to the end, cut verbatim by build.rs into
** OUT_DIR/gstate_gen/game_state_map_cut.inc, on stand-ins for the file system (TheFileSystem's files, the C library's
** streams and Win32's directory search, all on one in-memory disk whose paths are case-blind, as Windows'), the game
** logic and client (the game mode, the id counters, startNewGame), with the GameInfo build's SkirmishGameInfo
** (differential/wrap_game_info.cpp, configured by ref_ginfo_configure) and the Xfer build's map-path hooks pointed at
** this GameState.
**
** Entry points (extern "C"):
**   ref_gstate_run(cfg, ops, n, strs, nstrs, out, cap)  runs n operations of six words and writes the log
**                                                       (tests/differential_game_state.rs)
**   ref_gsmap_run(cfg, ops, n, strs, nstrs, out, cap)   the same for GameStateMap (tests/differential_game_state_map.rs) */
#define GameState GsTestGameState
#define TheGameState GsTestTheGameState
#define SaveGameInfo GsTestSaveGameInfo
#define SaveDate GsTestSaveDate
#define AvailableGameInfo GsTestAvailableGameInfo
#define GlobalData GsTestGlobalData
#define TheGlobalData GsTestTheGlobalData
#define MapCache GsTestMapCache
#define TheMapCache GsTestTheMapCache
#define MapObject GsTestMapObject
#define Dict GsTestDict
#define TheKey_mapName GsTestTheKey_mapName
#define CampaignManager GsTestCampaignManager
#define TheCampaignManager GsTestTheCampaignManager
#define Campaign GsTestCampaign
#define PartitionManager GsTestPartitionManager
#define ThePartitionManager GsTestThePartitionManager
#define GetLocalTime GsTestGetLocalTime
#define _access GsTestAccess
/* batch M2: GameStateMap, its stand-ins, and GameInfo (the GameInfo build's, under its names) */
#define GameStateMap GsTestGameStateMap
#define TheGameStateMap GsTestTheGameStateMap
#define TheWritableGlobalData GsTestTheWritableGlobalData
#define File GsTestFile
#define FileSystem GsTestFileSystem
#define TheFileSystem GsTestTheFileSystem
#define GameLogic GsTestGameLogic
#define TheGameLogic GsTestTheGameLogic
#define GameClient GsTestGameClient
#define TheGameClient GsTestTheGameClient
#define TheGameInfo GiTestTheGameInfo
#define TheSkirmishGameInfo GiTestTheSkirmishGameInfo
#define TheChallengeGameInfo GiTestTheChallengeGameInfo
#define GameSlot GiTestGameSlot
#define GameInfo GiTestGameInfo
#define SkirmishGameInfo GiTestSkirmishGameInfo
#define GameInfoToAsciiString GiTestGameInfoToAsciiString
#define ParseAsciiStringToGameInfo GiTestParseAsciiStringToGameInfo

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <string>
#include <vector>
#include <list>
#include <set>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/Snapshot.h"
/* Windows' SYSTEMTIME, which GameState.h's declarations name */
typedef struct { UnsignedShort wYear, wMonth, wDayOfWeek, wDay, wHour, wMinute, wSecond, wMilliseconds; } SYSTEMTIME;
void GetLocalTime(SYSTEMTIME *t);
class INI;   /* Money.h declares parseMoneyAmount(INI *, ...) */
#define protected public
#define private public
#include "../upstream/GeneralsMD/Code/GameEngine/Include/Common/GameState.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Include/Common/GameStateMap.h"
#include "GameNetwork/GameInfo.h"
#undef protected
#undef private

#ifndef _MAX_PATH
#define _MAX_PATH 260
#endif

namespace {
std::string g_log;
void logf(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
void logf(const char *fmt, ...)
{
	static char buf[1 << 16];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	g_log += buf;
	g_log += '\n';
}
unsigned g_cfg[8];
std::set<std::string> g_files;
std::vector<std::string> g_strs;
const char *str(unsigned i) { return g_strs.empty() ? "" : g_strs[i % g_strs.size()].c_str(); }
}

/* ---- stand-ins ---- */
struct GlobalData { AsciiString m_mapName, m_userData; AsciiString getPath_UserData() const { return m_userData; } };
static GlobalData g_gd;
GlobalData *TheGlobalData = &g_gd;
/* MapUtil.cpp lines 355-365 */
class MapCache
{
public:
	AsciiString getMapDir() const { return AsciiString("Maps"); }
	AsciiString getUserMapDir() const { AsciiString tmp = TheGlobalData->getPath_UserData(); tmp.concat(getMapDir()); return tmp; }
};
static MapCache g_mapCache;
MapCache *TheMapCache = &g_mapCache;
typedef Int NameKeyType;
NameKeyType TheKey_mapName = 1;
class Dict { public: AsciiString m_name; Bool m_has; AsciiString getAsciiString(NameKeyType, Bool *exists) const { if (exists) *exists = m_has; return m_has ? m_name : AsciiString(); } };
static Dict g_dict;
class MapObject { public: static Dict *getWorldDict() { return (g_cfg[0] & 1) ? &g_dict : NULL; } };
class Campaign { public: AsciiString m_name; };
class CampaignManager
{
public:
	enum { INVALID_MISSION_NUMBER = -1 };
	Campaign m_c; Int m_mission;
	Campaign *getCurrentCampaign() { return (g_cfg[0] & 4) ? &m_c : NULL; }
	Int getCurrentMissionNumber() { return m_mission; }
};
static CampaignManager g_campaign;
CampaignManager *TheCampaignManager = &g_campaign;
class PartitionManager { public: void update() { logf("partition update"); } };
static PartitionManager g_pm;
PartitionManager *ThePartitionManager = &g_pm;
static UnsignedShort g_clock;
void GetLocalTime(SYSTEMTIME *t)
{
	UnsignedShort *w = &t->wYear;
	for (int i = 0; i < 8; ++i) w[i] = (UnsignedShort)(g_clock + i);
	g_clock = (UnsignedShort)(g_clock + 8);
}
int _access(const char *path, int) { return g_files.count(path) ? 0 : -1; }
void GameState::clearAvailableGames(void) {}
void GameState::iterateSaveFiles(IterateSaveFileCallback, void *) {}
static GameState *g_gsObj;
GameState *TheGameState = NULL;

/* ---- the upstream code ---- */
#include "game_state_cut.inc"

/* the Xfer build's GameState stand-in forwards XferLoad::xferSnapshot's post-process requests here */
extern void (*g_xferAddPostProcessHook)(Snapshot *);
static void addPostProcessHook(Snapshot *s) { TheGameState->addPostProcessSnapshot(s); }

namespace {

class TestBlock : public Snapshot
{
public:
	Int m_id, m_value;
	AsciiString m_text;
	virtual void crc(Xfer *) {}
	virtual void xfer(Xfer *x)
	{
		x->xferInt(&m_value);
		x->xferAsciiString(&m_text);
		logf("block %d xfer mode %d value %d text [%s]", m_id, (int)x->getXferMode(), m_value, m_text.str());
	}
	virtual void loadPostProcess() { logf("post %d", m_id); }
};
TestBlock g_blocks[6];
std::vector<unsigned char> g_saved;
char g_tmp[64];

std::string hexOf(const std::vector<unsigned char> &b)
{
	std::string h;
	char t[4];
	for (size_t i = 0; i < b.size(); ++i) { snprintf(t, sizeof t, "%02x", b[i]); h += t; }
	return h;
}

void readTmp(std::vector<unsigned char> &out)
{
	out.clear();
	FILE *f = fopen(g_tmp, "rb");
	int ch;
	while (f && (ch = fgetc(f)) != EOF) out.push_back((unsigned char)ch);
	if (f) fclose(f);
}

void writeTmp(const std::vector<unsigned char> &b)
{
	FILE *f = fopen(g_tmp, "wb");
	if (!b.empty()) fwrite(&b[0], 1, b.size(), f);
	fclose(f);
}

void dumpInfo()
{
	SaveGameInfo *i = TheGameState->getSaveGameInfo();
	std::string d;
	char t[8];
	for (int k = 0; k < i->description.getLength(); ++k) { snprintf(t, sizeof t, " %04x", (unsigned)i->description.getCharAt(k)); d += t; }
	logf("info label [%s] date %u %u %u %u %u %u %u %u side [%s] mission %d type %d missionmap [%s] pristine [%s] desc%s",
		i->mapLabel.str(), i->date.year, i->date.month, i->date.day, i->date.dayOfWeek, i->date.hour, i->date.minute, i->date.second,
		i->date.milliseconds, i->campaignSide.str(), i->missionNumber, (int)i->saveFileType, i->missionMapName.str(),
		i->pristineMapName.str(), d.c_str());
}

void run_op(const unsigned *w)
{
	unsigned a = w[1], b = w[2], c = w[3], d = w[4], e = w[5];
	GameState *gs = TheGameState;
	switch (w[0]) {
	case 0: gs->addSnapshotBlock(AsciiString(str(a)), &g_blocks[b % 6], (SnapshotType)(c % 3)); logf("add [%s] %u %u", str(a), b % 6, c % 3); break;
	case 1: g_blocks[a % 6].m_value = (Int)b; g_blocks[a % 6].m_text = AsciiString(str(c)); logf("set %u", a % 6); break;
	case 2: {
		XferSave x;
		x.open(AsciiString(g_tmp));
		try {
			if (b & 1) gs->friend_xferSaveDataForCRC(&x, (SnapshotType)(a % 3)); else gs->xferSaveData(&x, (SnapshotType)(a % 3));
			logf("save");
		} catch (...) { logf("save threw"); }
		x.close();
		readTmp(g_saved);
		logf("bytes %s", hexOf(g_saved).c_str());
		break;
	}
	case 3: {
		std::vector<unsigned char> bytes = g_saved;
		if ((b & 1) && !bytes.empty()) bytes[c % bytes.size()] = (unsigned char)d;
		if ((b & 2) && !bytes.empty()) bytes.resize(bytes.size() - 1 - (e % bytes.size()));
		writeTmp(bytes);
		XferLoad x;
		x.open(AsciiString(g_tmp));
		if (b & 4) x.setOptions(XO_NO_POST_PROCESSING);
		try { gs->xferSaveData(&x, (SnapshotType)(a % 3)); logf("load"); } catch (...) { logf("load threw"); }
		x.close();
		std::string pl;
		char t[8];
		for (std::list<Snapshot *>::iterator it = gs->m_snapshotPostProcessList.begin(); it != gs->m_snapshotPostProcessList.end(); ++it) {
			snprintf(t, sizeof t, " %d", static_cast<TestBlock *>(*it)->m_id); pl += t;
		}
		logf("postlist%s", pl.c_str());
		break;
	}
	case 4: gs->gameStatePostProcessLoad(); logf("postprocessed"); break;
	case 5: gs->reset(); logf("reset"); break;
	case 6: {   /* the info block alone */
		XferSave x;
		x.open(AsciiString(g_tmp));
		try { gs->xfer(&x); logf("infosave"); } catch (...) { logf("infosave threw"); }
		x.close();
		readTmp(g_saved);
		logf("bytes %s", hexOf(g_saved).c_str());
		break;
	}
	case 7: {
		std::vector<unsigned char> bytes = g_saved;
		if ((b & 1) && !bytes.empty()) bytes[c % bytes.size()] = (unsigned char)d;
		writeTmp(bytes);
		XferLoad x;
		x.open(AsciiString(g_tmp));
		try { gs->xfer(&x); logf("infoload"); } catch (...) { logf("infoload threw"); }
		x.close();
		dumpInfo();
		break;
	}
	case 8: {
		SaveGameInfo *i = gs->getSaveGameInfo();
		i->saveFileType = (SaveFileType)(a % 3);
		i->missionMapName = AsciiString(str(b));
		i->pristineMapName = AsciiString(str(c));
		WideChar wd[3] = { (WideChar)('a' + d % 26), (WideChar)(e & 0xffff), 0 };
		i->description = UnicodeString(wd);
		logf("setinfo");
		break;
	}
	case 9: dumpInfo(); break;
	case 10: logf("next [%s]", gs->findNextSaveFilename(UnicodeString()).str()); break;
	case 11: g_files.insert(str(a)); logf("file [%s]", str(a)); break;
	case 12: logf("portable [%s] -> [%s]", str(a), gs->realMapPathToPortableMapPath(AsciiString(str(a))).str()); break;
	case 13: logf("real [%s] -> [%s]", str(a), gs->portableMapPathToRealMapPath(AsciiString(str(a))).str()); break;
	case 14: logf("leaf [%s] -> [%s] insave %d", str(a), gs->getMapLeafName(AsciiString(str(a))).str(), (int)gs->isInSaveDirectory(AsciiString(str(a)))); break;
	case 15: {
		SaveDate x, y;
		UnsignedShort *px = &x.year, *py = &y.year;
		for (int k = 0; k < 8; ++k) { px[k] = (UnsignedShort)((a >> (4 * k)) & 3); py[k] = (UnsignedShort)((b >> (4 * k)) & 3); }
		logf("newer %d", (int)x.isNewerThan(&y));
		break;
	}
	default: logf("op?"); break;
	}
}

} // namespace

extern "C" int ref_gstate_run(const unsigned *cfg, const unsigned *ops, int n, const char *strs, int nstrs, char *out, int cap)
{
	g_log.clear();
	memcpy(g_cfg, cfg, sizeof g_cfg);
	g_strs.clear();
	for (int i = 0, at = 0; i < nstrs; ++i) {
		int len = (unsigned char)strs[at] | ((unsigned char)strs[at + 1] << 8);
		g_strs.push_back(std::string(strs + at + 2, len));
		at += 2 + len;
	}
	g_gd.m_userData = AsciiString((g_cfg[0] & 2) ? "C:\\Users\\Me\\Generals\\" : "data\\");
	g_gd.m_mapName = AsciiString(str(g_cfg[1]));
	g_dict.m_has = (g_cfg[0] & 8) != 0;
	g_dict.m_name = AsciiString(str(g_cfg[2]));
	g_campaign.m_c.m_name = AsciiString(str(g_cfg[3]));
	g_campaign.m_mission = (Int)g_cfg[4];
	g_clock = (UnsignedShort)g_cfg[5];
	g_files.clear();
	snprintf(g_tmp, sizeof g_tmp, "/tmp/ref_gstate_%d.sav", (int)getpid());
	g_saved.clear();
	for (int i = 0; i < 6; ++i) { g_blocks[i].m_id = i; g_blocks[i].m_value = 0; g_blocks[i].m_text = AsciiString(); }
	void *mem = calloc(1, sizeof(GameState));
	g_gsObj = new (mem) GameState;
	TheGameState = g_gsObj;
	g_xferAddPostProcessHook = addPostProcessHook;
	for (int i = 0; i < n; ++i)
		run_op(ops + 6 * i);
	g_xferAddPostProcessHook = NULL;
	unlink(g_tmp);
	TheGameState = NULL;
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}

/* GameState's constructor and destructor (GameState.cpp lines 281-302), which the cut leaves out: the constructor sets
** two members, the destructor clears the lists */
GameState::GameState(void) { m_availableGames = NULL; m_isInLoadGame = FALSE; }
GameState::~GameState(void) {}
void GameState::init(void) {}

/* ==== batch M2: GameStateMap ==================================================================================== */

GlobalData *TheWritableGlobalData = &g_gd;
/* SkirmishGameOptionsMenu.cpp line 78 (the GameInfo build leaves it out) */
SkirmishGameInfo *TheSkirmishGameInfo = NULL;
static void gsFreeSkirmish() { delete TheSkirmishGameInfo; TheSkirmishGameInfo = NULL; }

/* GameLogic.h lines 74-84 (the game modes) */
enum
{
	GAME_SINGLE_PLAYER,
	GAME_LAN,
	GAME_SKIRMISH,
	GAME_REPLAY,
	GAME_SHELL,
	GAME_INTERNET,
	GAME_NONE
};

namespace {
/* the disk: paths case-blind (kept lower case), as Windows' */
std::map<std::string, std::vector<unsigned char> > g_disk;
unsigned g_faults;   /* 1 a short file-system read, 2 a short fread, 4 fopen for writing fails, 8 a short fwrite */
std::string g_cwd;
std::string lowerOf(const char *p) { std::string r(p); for (size_t i = 0; i < r.size(); ++i) if (r[i] >= 'A' && r[i] <= 'Z') r[i] = (char)(r[i] + 32); return r; }
}

class GameLogic
{
public:
	Int m_mode; ObjectID m_objectID; Bool m_loadingSave;
	void setLoadingSave(Bool b) { m_loadingSave = b; logf("loadingsave %d", (int)b); }
	Int getGameMode() { return m_mode; }
	void setGameMode(Int m) { m_mode = m; logf("setmode %d", m); }
	ObjectID getObjectIDCounter() { return m_objectID; }
	void setObjectIDCounter(ObjectID id) { m_objectID = id; logf("objid %d", (int)id); }
	void startNewGame(Bool b) { logf("startnewgame %d", (int)b); }
};
static GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
class GameClient
{
public:
	DrawableID m_drawableID;
	DrawableID getDrawableIDCounter() { return m_drawableID; }
	void setDrawableIDCounter(DrawableID id) { m_drawableID = id; logf("drawid %d", (int)id); }
};
static GameClient g_client;
GameClient *TheGameClient = &g_client;

/* TheFileSystem's file */
class File
{
public:
	enum { READ = 0x1, BINARY = 0x20 };
	enum seekMode { START, CURRENT, END };
	std::string m_path; Int m_pos;
	Int seek(Int pos, seekMode mode)
	{
		Int size = (Int)g_disk[m_path].size();
		m_pos = mode == END ? size + pos : mode == START ? pos : m_pos + pos;
		if (mode == END) logf("fssize %d", (int)m_pos);
		return m_pos;
	}
	Int read(void *buf, Int n)
	{
		std::vector<unsigned char> &d = g_disk[m_path];
		Int avail = (Int)d.size() - m_pos;
		Int got = n < avail ? n : avail;
		if ((g_faults & 1) && got > 0) --got;
		if (got > 0) memcpy(buf, &d[m_pos], got);
		m_pos += got;
		logf("fsread %d -> %d", (int)n, (int)got);
		return got;
	}
	void close() { logf("fsclose"); }
};
class FileSystem
{
public:
	File m_file;
	File *openFile(const char *path, Int)
	{
		bool ok = g_disk.count(lowerOf(path)) != 0;
		logf("fsopen [%s] %d", path, (int)ok);
		if (!ok) return NULL;
		m_file.m_path = lowerOf(path);
		m_file.m_pos = 0;
		return &m_file;
	}
};
static FileSystem g_fileSystem;
FileSystem *TheFileSystem = &g_fileSystem;

/* the C library's streams, for GameStateMap.cpp only (the macros below) */
namespace {
struct Stream { std::string path; long pos; } g_stream;
FILE *gsmFopen(const char *path, const char *mode)
{
	bool write = strchr(mode, 'w') != NULL;
	bool ok = write ? (g_faults & 4) == 0 : g_disk.count(lowerOf(path)) != 0;
	logf("fopen [%s] %s %d", path, mode, (int)ok);
	if (!ok) return NULL;
	g_stream.path = lowerOf(path);
	g_stream.pos = 0;
	if (write) g_disk[g_stream.path].clear();
	return (FILE *)&g_stream;
}
int gsmFseek(FILE *, long off, int whence)
{
	long size = (long)g_disk[g_stream.path].size();
	g_stream.pos = whence == SEEK_END ? size + off : whence == SEEK_SET ? off : g_stream.pos + off;
	return 0;
}
long gsmFtell(FILE *) { logf("ftell %ld", g_stream.pos); return g_stream.pos; }
size_t gsmFread(void *buf, size_t, size_t n, FILE *)
{
	std::vector<unsigned char> &d = g_disk[g_stream.path];
	size_t avail = d.size() > (size_t)g_stream.pos ? d.size() - g_stream.pos : 0;
	size_t got = n < avail ? n : avail;
	if ((g_faults & 2) && got > 0) --got;
	if (got > 0) memcpy(buf, &d[g_stream.pos], got);
	g_stream.pos += (long)got;
	logf("fread %d -> %d", (int)n, (int)got);
	return got;
}
size_t gsmFwrite(const void *buf, size_t, size_t n, FILE *)
{
	size_t put = n;
	if ((g_faults & 8) && put > 0) --put;
	std::vector<unsigned char> &d = g_disk[g_stream.path];
	const unsigned char *b = (const unsigned char *)buf;
	for (size_t i = 0; i < put; ++i) {
		if ((size_t)g_stream.pos < d.size()) d[g_stream.pos] = b[i]; else d.push_back(b[i]);
		++g_stream.pos;
	}
	logf("fwrite %d -> %d", (int)n, (int)put);
	return put;
}
int gsmFclose(FILE *) { logf("fclose"); return 0; }

/* Win32's directory search: ".", "..", then the directory's files and subdirectories in order; a directory with no
** files does not exist (FindFirstFile fails) */
typedef void *HANDLE;
#define INVALID_HANDLE_VALUE ((HANDLE)(intptr_t)-1)
#define FILE_ATTRIBUTE_DIRECTORY 0x10
typedef struct { unsigned long dwFileAttributes; char cFileName[260]; } WIN32_FIND_DATA;
std::vector<std::pair<std::string, bool> > g_found;
size_t g_foundAt;
void fillItem(WIN32_FIND_DATA *item)
{
	item->dwFileAttributes = g_found[g_foundAt].second ? FILE_ATTRIBUTE_DIRECTORY : 0;
	snprintf(item->cFileName, sizeof item->cFileName, "%s", g_found[g_foundAt].first.c_str());
}
unsigned long GetCurrentDirectory(unsigned long n, char *buf) { snprintf(buf, n, "%s", g_cwd.c_str()); return (unsigned long)g_cwd.size(); }
int SetCurrentDirectory(const char *dir) { g_cwd = dir; logf("cd [%s]", dir); return 1; }
HANDLE FindFirstFile(const char *pattern, WIN32_FIND_DATA *item)
{
	std::map<std::string, bool> names;
	std::string dir = lowerOf(g_cwd.c_str());
	for (std::map<std::string, std::vector<unsigned char> >::iterator it = g_disk.begin(); it != g_disk.end(); ++it) {
		if (it->first.compare(0, dir.size(), dir) != 0 || it->first.size() == dir.size()) continue;
		std::string rest = it->first.substr(dir.size());
		size_t bs = rest.find('\\');
		if (bs == std::string::npos) names[rest] = false; else names[rest.substr(0, bs)] = true;
	}
	g_found.clear();
	g_foundAt = 0;
	if (names.empty()) { logf("findfirst [%s] -> none", pattern); return INVALID_HANDLE_VALUE; }
	g_found.push_back(std::make_pair(std::string("."), true));
	g_found.push_back(std::make_pair(std::string(".."), true));
	for (std::map<std::string, bool>::iterator it = names.begin(); it != names.end(); ++it) g_found.push_back(*it);
	fillItem(item);
	logf("findfirst [%s] -> [%s] %d", pattern, item->cFileName, (int)g_found[0].second);
	return (HANDLE)&g_found;
}
int FindNextFile(HANDLE, WIN32_FIND_DATA *item)
{
	if (g_foundAt + 1 >= g_found.size()) { logf("findnext -> none"); return 0; }
	++g_foundAt;
	fillItem(item);
	logf("findnext -> [%s] %d", item->cFileName, (int)g_found[g_foundAt].second);
	return 1;
}
int FindClose(HANDLE) { logf("findclose"); return 1; }
int DeleteFile(const char *name) { logf("delete [%s]", name); g_disk.erase(lowerOf((g_cwd + name).c_str())); return 1; }
}
#ifndef stricmp
#define stricmp strcasecmp
#endif

/* ---- the upstream code ---- */
#define fopen gsmFopen
#define fseek gsmFseek
#define ftell gsmFtell
#define fread gsmFread
#define fwrite gsmFwrite
#define fclose gsmFclose
#include "game_state_map_cut.inc"
#undef fopen
#undef fseek
#undef ftell
#undef fread
#undef fwrite
#undef fclose

extern AsciiString (*g_xferRealToPortableHook)(const AsciiString &);
extern AsciiString (*g_xferPortableToRealHook)(const AsciiString &);
extern "C" void ref_ginfo_configure(const unsigned *cfg);

namespace {

AsciiString gsRealToPortable(const AsciiString &in) { return TheGameState->realMapPathToPortableMapPath(in); }
AsciiString gsPortableToReal(const AsciiString &in) { return TheGameState->portableMapPathToRealMapPath(in); }

alignas(GameStateMap) unsigned char g_mapMem[sizeof(GameStateMap)];
GameStateMap *g_map;

void dumpMapState()
{
	SaveGameInfo *i = TheGameState->getSaveGameInfo();
	logf("state mode %d objid %d drawid %d loading %d map [%s] savemap [%s] pristine [%s] cwd [%s]", g_logic.m_mode,
		(int)g_logic.m_objectID, (int)g_client.m_drawableID, (int)g_logic.m_loadingSave, g_gd.m_mapName.str(),
		i->saveGameMapName.str(), i->pristineMapName.str(), g_cwd.c_str());
	SkirmishGameInfo *s = TheSkirmishGameInfo;
	if (s == NULL) { logf("skirmish none"); return; }
	std::string slots;
	char t[16];
	for (int k = 0; k < MAX_SLOTS; ++k) { snprintf(t, sizeof t, " %d", (int)s->getConstSlot(k)->getState()); slots += t; }
	logf("skirmish seed %d map [%s] crc %u ip %u gid %d crcint %d cash %u slots%s", (int)s->getSeed(), s->getMap().str(),
		(unsigned)s->getMapCRC(), (unsigned)s->m_localIP, (int)s->getGameID(), (int)s->getCRCInterval(),
		(unsigned)s->getStartingCash().countMoney(), slots.c_str());
}

void dumpPostList()
{
	std::string pl;
	for (std::list<Snapshot *>::iterator it = TheGameState->m_snapshotPostProcessList.begin(); it != TheGameState->m_snapshotPostProcessList.end(); ++it)
		pl += *it == g_map ? " M" : *it == TheSkirmishGameInfo ? " S" : TheSkirmishGameInfo && *it == &TheSkirmishGameInfo->m_startingCash ? " C" : " ?";
	logf("postlist%s", pl.c_str());
}

void dumpDisk()
{
	std::string d;
	for (std::map<std::string, std::vector<unsigned char> >::iterator it = g_disk.begin(); it != g_disk.end(); ++it) {
		unsigned h = 0;
		for (size_t k = 0; k < it->second.size(); ++k) h = h * 31u + it->second[k];
		char t[32];
		snprintf(t, sizeof t, " %u:%08x", (unsigned)it->second.size(), h);
		d += " [" + it->first + "]" + t;
	}
	logf("disk%s", d.c_str());
}

std::vector<unsigned char> damaged(unsigned b, unsigned c, unsigned d, unsigned e)
{
	std::vector<unsigned char> bytes = g_saved;
	if ((b & 1) && !bytes.empty()) bytes[c % bytes.size()] = (unsigned char)d;
	if ((b & 2) && !bytes.empty()) bytes.resize(bytes.size() - 1 - (e % bytes.size()));
	return bytes;
}

void run_map_op(const unsigned *w)
{
	unsigned a = w[1], b = w[2], c = w[3], d = w[4], e = w[5];
	GameState *gs = TheGameState;
	switch (w[0]) {
	case 0: g_gd.m_mapName = AsciiString(str(a)); logf("mapname [%s]", str(a)); break;
	case 1: {
		std::vector<unsigned char> &f = g_disk[lowerOf(str(a))];
		f.clear();
		for (unsigned k = 0; k < b % 40; ++k) f.push_back((unsigned char)(c + k * 7));
		logf("put [%s] %u", str(a), b % 40);
		break;
	}
	case 2: g_logic.m_mode = (Int)(a % 7); logf("mode %d", (int)(a % 7)); break;
	case 3: g_logic.m_objectID = (ObjectID)a; g_client.m_drawableID = (DrawableID)b; logf("counters"); break;
	case 4: g_faults = a & 15; logf("faults %u", g_faults); break;
	case 5:
	case 12: {
		XferSave x;
		x.open(AsciiString(g_tmp));
		try {
			if (w[0] == 5) g_map->xfer(&x); else gs->xferSaveData(&x, SNAPSHOT_SAVELOAD);
			logf("save");
		}
		catch (XferStatus s) { logf("save threw xfer %d", (int)s); }
		catch (SaveCode s) { logf("save threw save %d", (int)s); }
		x.close();
		readTmp(g_saved);
		logf("bytes %s", hexOf(g_saved).c_str());
		dumpMapState();
		dumpDisk();
		break;
	}
	case 6:
	case 13: {
		writeTmp(damaged(b, c, d, e));
		XferLoad x;
		x.open(AsciiString(g_tmp));
		if (b & 4) x.setOptions(XO_NO_POST_PROCESSING);
		try {
			if (w[0] == 6) g_map->xfer(&x); else gs->xferSaveData(&x, SNAPSHOT_SAVELOAD);
			logf("load");
		}
		catch (XferStatus s) { logf("load threw xfer %d", (int)s); }
		catch (SaveCode s) { logf("load threw save %d", (int)s); }
		x.close();
		dumpMapState();
		dumpPostList();
		dumpDisk();
		break;
	}
	case 7: g_map->clearScratchPadMaps(); logf("cleared"); dumpDisk(); break;
	case 8: dumpMapState(); dumpDisk(); break;
	case 9:
		if (TheSkirmishGameInfo) {
			TheSkirmishGameInfo->setSeed((Int)a);
			TheSkirmishGameInfo->setMap(AsciiString(str(b)));
			TheSkirmishGameInfo->setMapCRC(c);
			TheSkirmishGameInfo->getSlot(d % MAX_SLOTS)->setState((SlotState)(e % 6));
		}
		logf("skset");
		break;
	case 10: gs->gameStatePostProcessLoad(); logf("postprocessed"); break;
	case 11: gs->reset(); logf("reset"); break;
	default: logf("op?"); break;
	}
}

} // namespace

extern "C" int ref_gsmap_run(const unsigned *cfg, const unsigned *ops, int n, const char *strs, int nstrs, char *out, int cap)
{
	g_log.clear();
	memcpy(g_cfg, cfg, sizeof g_cfg);
	g_strs.clear();
	for (int i = 0, at = 0; i < nstrs; ++i) {
		int len = (unsigned char)strs[at] | ((unsigned char)strs[at + 1] << 8);
		g_strs.push_back(std::string(strs + at + 2, len));
		at += 2 + len;
	}
	/* GameInfo's stand-ins: cfg[6] bit 0 a default starting cash of 10000, the first tick cfg[7] */
	unsigned ginfo_cfg[9] = { (g_cfg[6] & 1) ? 32u : 0u, 0, 0, g_cfg[7], 0, 0, 0, 0, 0 };
	ref_ginfo_configure(ginfo_cfg);
	g_gd.m_userData = AsciiString((g_cfg[0] & 2) ? "C:\\Users\\Me\\Generals\\" : "data\\");
	g_gd.m_mapName = AsciiString(str(g_cfg[1]));
	g_logic.m_mode = (Int)(g_cfg[2] % 7);
	g_logic.m_objectID = (ObjectID)g_cfg[3];
	g_logic.m_loadingSave = FALSE;
	g_client.m_drawableID = (DrawableID)g_cfg[4];
	g_cwd = "C:\\Game\\";
	g_faults = 0;
	g_disk.clear();
	snprintf(g_tmp, sizeof g_tmp, "/tmp/ref_gsmap_%d.sav", (int)getpid());
	g_saved.clear();
	void *mem = calloc(1, sizeof(GameState));
	g_gsObj = new (mem) GameState;
	TheGameState = g_gsObj;
	g_map = new (g_mapMem) GameStateMap;
	TheGameStateMap = g_map;
	gsFreeSkirmish();
	TheGameState->addSnapshotBlock(AsciiString("CHUNK_GameStateMap"), g_map, SNAPSHOT_SAVELOAD);
	g_xferAddPostProcessHook = addPostProcessHook;
	g_xferRealToPortableHook = gsRealToPortable;
	g_xferPortableToRealHook = gsPortableToReal;
	for (int i = 0; i < n; ++i)
		run_map_op(ops + 6 * i);
	g_xferAddPostProcessHook = NULL;
	g_xferRealToPortableHook = NULL;
	g_xferPortableToRealHook = NULL;
	gsFreeSkirmish();
	TheGameStateMap = NULL;
	unlink(g_tmp);
	TheGameState = NULL;
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}
