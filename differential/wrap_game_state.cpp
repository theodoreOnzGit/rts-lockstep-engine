/* Differential-test wrapper for GameState (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/Common/GameState.h            (unchanged, renamed by the macros below)
**   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp, the pieces build.rs cuts verbatim into
**   OUT_DIR/gstate_gen/game_state_cut.inc (see build.rs)
** on stand-ins for the global data (the user data path, the map name), the map cache's two directories (MapUtil.cpp
** lines 355-365, copied), the world dict's map name, the campaign manager, the partition manager's update, GetLocalTime
** and _access; the Xfer classes are the Xfer build's (XferSave, XferLoad), whose GameState stand-in forwards the
** post-process requests here (g_xferAddPostProcessHook). The test's snapshots are logging blocks of an Int and a string.
**
** Entry point (extern "C", tests/differential_game_state.rs):
**   ref_gstate_run(cfg, ops, n, strs, nstrs, out, cap)  runs n operations of six words and writes the log */
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
#define protected public
#define private public
#include "../upstream/GeneralsMD/Code/GameEngine/Include/Common/GameState.h"
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
