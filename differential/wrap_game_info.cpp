/* Differential-test wrapper for GameInfo (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameInfo.cpp      (#included below, unchanged but for the for-scope
**                                                                    edit build.rs makes in its copy, ginfo_gen)
**   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameInfo.h        (unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/FileTransfer.cpp   (lines 138-239, cut verbatim by build.rs)
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/strtok_r.cpp      (#included below, unchanged, renamed by macro)
**   (Xfer::xferMapName is the Xfer build's, through hooks in its GameState stand-in)
** on the stand-ins of differential/shim_ginfo: the map cache, the file system, the game text, the map-path conversions of
** TheGameState, the multiplayer settings, the player templates, the global data's default starting cash, GetTickCount,
** and Windows' two UTF-8 converters. Money is the real Money.cpp of the money build (upstream_money); its
** recordIncome stand-in counts the income records (ref_money_record_income_calls).
**
** Entry point (extern "C", tests/differential_game_info.rs):
**   ref_ginfo_run(cfg, ops, n, strs, out, cap)  runs n operations of six words on a SkirmishGameInfo and writes the log
** The operations and the log are described at run_op below and mirrored in the test. */
#define GameState GiTestGameState
#define TheGameState GiTestTheGameState
#define TheGameInfo GiTestTheGameInfo
#define TheSkirmishGameInfo GiTestTheSkirmishGameInfo
#define TheChallengeGameInfo GiTestTheChallengeGameInfo
#define GameSlot GiTestGameSlot
#define GameInfo GiTestGameInfo
#define SkirmishGameInfo GiTestSkirmishGameInfo
#define GameInfoToAsciiString GiTestGameInfoToAsciiString
#define ParseAsciiStringToGameInfo GiTestParseAsciiStringToGameInfo
#define File GiTestFile
#define FileSystem GiTestFileSystem
#define TheFileSystem GiTestTheFileSystem
#define GameTextInterface GiTestGameTextInterface
#define TheGameText GiTestTheGameText
#define MapMetaData GiTestMapMetaData
#define MapCache GiTestMapCache
#define TheMapCache GiTestTheMapCache
#define MultiplayerSettings GiTestMultiplayerSettings
#define MultiplayerColorDefinition GiTestMultiplayerColorDefinition
#define TheMultiplayerSettings GiTestTheMultiplayerSettings
#define PlayerTemplate GiTestPlayerTemplate
#define PlayerTemplateStore GiTestPlayerTemplateStore
#define ThePlayerTemplateStore GiTestThePlayerTemplateStore
#define NET_CRC_INTERVAL GiTestNET_CRC_INTERVAL
#define TheGameSpyGame GiTestTheGameSpyGame
#define TheLAN GiTestTheLAN
#define TheGlobalData GiTestTheGlobalData
#define GetTickCount GiTestGetTickCount
#define GetBasePathFromPath GiTestGetBasePathFromPath
#define GetFileFromPath GiTestGetFileFromPath
#define GetExtensionFromFile GiTestGetExtensionFromFile
#define GetBaseFileFromFile GiTestGetBaseFileFromFile
#define GetPreviewFromMap GiTestGetPreviewFromMap
#define GetINIFromMap GiTestGetINIFromMap
#define GetStrFileFromMap GiTestGetStrFileFromMap
#define GetSoloINIFromMap GiTestGetSoloINIFromMap
#define GetAssetUsageFromMap GiTestGetAssetUsageFromMap
#define GetReadmeFromMap GiTestGetReadmeFromMap
#define MultiByteToWideCharSingleLine GiTestMultiByteToWideCharSingleLine
#define WideCharStringToMultiByte GiTestWideCharStringToMultiByte

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <string>
#include <vector>
#define PI 3.14159265359f   /* GameCommon.h uses it; the real PreRTS.h brings it from WWMath */
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
class INI;   /* Money.h declares parseMoneyAmount(INI *, ...) */
#include "Common/Money.h"
#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"

/* the global data's one field GameInfo.cpp reads (the real PreRTS.h brings GlobalData.h) */
struct GiTestGlobalDataClass { Money m_defaultStartingCash; };
extern GiTestGlobalDataClass *TheGlobalData;
unsigned long GetTickCount();

/* Westwood's strtok_r, under another name (glibc declares strtok_r with other exception specifications) */
#define strtok_r wwStrtokR
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/strtok_r.cpp"

#define protected public
#define private public
#include "GameNetwork/GameInfo.h"
#undef protected
#undef private
/* build.rs's copy (OUT_DIR/ginfo_gen): adjustSlotsForMap's for-scope edit, described there */
#include "GameInfo.cpp"
/* FileTransfer.cpp lines 138-239 (GetBasePathFromPath ... GetReadmeFromMap), cut verbatim by build.rs */
#include "file_transfer_paths.inc"

/* Xfer::xferMapName is virtual: the Xfer build's (wrap_xfer.cpp) runs, and its GameState stand-in calls these hooks,
** which this test points at its own map-path conversions */
extern AsciiString (*g_xferRealToPortableHook)(const AsciiString &);
extern AsciiString (*g_xferPortableToRealHook)(const AsciiString &);

extern "C" int ref_money_record_income_calls(void);

namespace {

std::string g_log;
void logf(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
void logf(const char *fmt, ...)
{
	static char buf[1 << 17];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	g_log += buf;
	g_log += '\n';
}

/* cfg words: [0] flags (bit 0 a map cache, bit 1 multiplayer settings, bit 2 random template shown, bit 3 random colour,
** bit 4 random start, bit 5 a default starting cash of 10000 else 0), [1] the colour count, [2] the template count,
** [3] the first tick, [4] map 0's players, [5] map 0's CRC, [6] map 1's players, [7] map 1's CRC, [8] the file-exists
** salt */
unsigned g_cfg[9];
unsigned g_tick;

std::string lower(const char *s) { std::string r(s); for (size_t i = 0; i < r.size(); ++i) r[i] = (char)tolower((unsigned char)r[i]); return r; }

/* the deterministic answers both sides give (the test mirrors them) */
bool fileExists(const char *p)
{
	unsigned h = g_cfg[8];
	for (const unsigned char *q = (const unsigned char *)p; *q; ++q) h = h * 31u + *q;
	return (h >> 7) % 3 == 0;
}

} // namespace

/* ---- the stand-ins ------------------------------------------------------------------------------- */
Int NET_CRC_INTERVAL = 100;
GameInfo *TheGameSpyGame = NULL;
LANAPI *TheLAN = NULL;
static GiTestGlobalDataClass g_globalData;
GiTestGlobalDataClass *TheGlobalData = &g_globalData;
unsigned long GetTickCount() { unsigned t = g_tick; g_tick += 7; return t; }

void File::close() {}
static File g_file;
File *FileSystem::openFile(const char *path) { logf("open %s", path); return fileExists(path) ? &g_file : NULL; }
static FileSystem g_fs;
FileSystem *TheFileSystem = &g_fs;

UnicodeString GameTextInterface::fetch(const char *label)
{
	UnicodeString u;
	std::basic_string<WideChar> w;
	w += 'T'; w += ':';
	for (const char *p = label; *p; ++p) w += (WideChar)(unsigned char)*p;
	u.set(w.c_str());
	return u;
}
static GameTextInterface g_text;
GameTextInterface *TheGameText = &g_text;

static MapCache g_mapCache;
MapCache *TheMapCache = NULL;
static MultiplayerSettings g_mp;
MultiplayerSettings *TheMultiplayerSettings = NULL;
static PlayerTemplateStore g_templates;
PlayerTemplateStore *ThePlayerTemplateStore = &g_templates;

/* the map paths: real "data\..." is portable "maps\...", both lower-cased (any other path only lower-cased) */
static AsciiString swapPrefix(const AsciiString &in, const char *from, const char *to)
{
	std::string s = lower(in.str());
	if (s.compare(0, strlen(from), from) == 0) s = std::string(to) + s.substr(strlen(from));
	return AsciiString(s.c_str());
}
AsciiString GameState::realMapPathToPortableMapPath(const AsciiString &in) const { return swapPrefix(in, "data\\", "maps\\"); }
AsciiString GameState::portableMapPathToRealMapPath(const AsciiString &in) const { return swapPrefix(in, "maps\\", "data\\"); }
static GameState g_gameState;
GameState *TheGameState = &g_gameState;
static AsciiString hookRealToPortable(const AsciiString &in) { return TheGameState->realMapPathToPortableMapPath(in); }
static AsciiString hookPortableToReal(const AsciiString &in) { return TheGameState->portableMapPathToRealMapPath(in); }

/* Windows' UTF-8 converters as the port reads them (see common::game_info): UTF-8 decoded with an ill-formed sequence
** as U+FFFD (one per maximal invalid subpart, as Windows and Rust's from_utf8_lossy do), each '\n' and '\r' a space,
** cut at a 0; UTF-16 encoded to UTF-8 with a lone surrogate as U+FFFD. */
static void putUtf16(GiWString &out, unsigned cp)
{
	if (cp >= 0x10000) { cp -= 0x10000; out += (WideChar)(0xD800 + (cp >> 10)); out += (WideChar)(0xDC00 + (cp & 0x3FF)); }
	else out += (WideChar)cp;
}
GiWString MultiByteToWideCharSingleLine( const char *orig )
{
	const unsigned char *s = (const unsigned char *)orig;
	size_t n = strlen(orig), i = 0;
	GiWString out;
	while (i < n) {
		unsigned c = s[i];
		if (c < 0x80) { putUtf16(out, c); ++i; continue; }
		int len = 0; unsigned cp = 0, lo = 0x80, hi = 0xBF;
		if (c >= 0xC2 && c <= 0xDF) { len = 2; cp = c & 0x1F; }
		else if (c >= 0xE0 && c <= 0xEF) { len = 3; cp = c & 0x0F; if (c == 0xE0) lo = 0xA0; if (c == 0xED) hi = 0x9F; }
		else if (c >= 0xF0 && c <= 0xF4) { len = 4; cp = c & 0x07; if (c == 0xF0) lo = 0x90; if (c == 0xF4) hi = 0x8F; }
		else { putUtf16(out, 0xFFFD); ++i; continue; }
		size_t j = 1;
		for (; j < (size_t)len; ++j) {
			if (i + j >= n) break;
			unsigned b = s[i + j];
			unsigned l = (j == 1) ? lo : 0x80, h = (j == 1) ? hi : 0xBF;
			if (b < l || b > h) break;
			cp = (cp << 6) | (b & 0x3F);
		}
		if (j == (size_t)len) { putUtf16(out, cp); i += len; }
		else { putUtf16(out, 0xFFFD); i += j; }
	}
	for (size_t k = 0; k < out.size(); ++k) {
		if (out[k] == 0) { out.resize(k); break; }
		if (out[k] == '\n' || out[k] == '\r') out[k] = ' ';
	}
	return out;
}
std::string WideCharStringToMultiByte( const WideChar *orig )
{
	std::string out;
	for (size_t i = 0; orig[i]; ++i) {
		unsigned c = orig[i];
		if (c >= 0xD800 && c <= 0xDBFF && orig[i + 1] >= 0xDC00 && orig[i + 1] <= 0xDFFF) {
			c = 0x10000 + ((c - 0xD800) << 10) + (orig[i + 1] - 0xDC00);
			++i;
		} else if (c >= 0xD800 && c <= 0xDFFF) {
			c = 0xFFFD;
		}
		if (c < 0x80) out += (char)c;
		else if (c < 0x800) { out += (char)(0xC0 | (c >> 6)); out += (char)(0x80 | (c & 0x3F)); }
		else if (c < 0x10000) { out += (char)(0xE0 | (c >> 12)); out += (char)(0x80 | ((c >> 6) & 0x3F)); out += (char)(0x80 | (c & 0x3F)); }
		else { out += (char)(0xF0 | (c >> 18)); out += (char)(0x80 | ((c >> 12) & 0x3F)); out += (char)(0x80 | ((c >> 6) & 0x3F)); out += (char)(0x80 | (c & 0x3F)); }
	}
	return out;
}

namespace {

SkirmishGameInfo *g_game = NULL;
SkirmishGameInfo *g_other = NULL;
std::vector<std::string> g_strs;
std::vector<unsigned char> g_saved;
char g_tmpName[64];

std::string wide(const UnicodeString &u)
{
	std::string r;
	char b[8];
	for (int i = 0; i < u.getLength(); ++i) {
		unsigned c = (unsigned)u.getCharAt(i);
		if (c >= 0x20 && c < 0x7f && c != '\\') r += (char)c;
		else { snprintf(b, sizeof b, "\\u%04x", c); r += b; }
	}
	return r;
}

void dumpGame(const char *what, GameInfo *g)
{
	logf("%s: pre %d crc %d in %d prog %d sur %d id %d ip %X map [%s] mc %X ms %u mm %d seed %d stats %d sr %u cash %u old %d",
		what, g->m_preorderMask, g->m_crcInterval, (int)g->m_inGame, (int)g->m_inProgress, (int)g->m_surrendered, g->m_gameID,
		g->m_localIP, g->m_mapName.str(), g->m_mapCRC, g->m_mapSize, g->m_mapMask, g->m_seed, g->m_useStats,
		(unsigned)g->m_superweaponRestriction, g->m_startingCash.countMoney(), (int)g->m_oldFactionsOnly);
	for (int i = 0; i < MAX_SLOTS; ++i) {
		const GameSlot *s = g->getConstSlot(i);
		logf(" slot %d: st %d acc %d map %d mute %d col %d sp %d pt %d team %d ocol %d osp %d opt %d name [%s] ip %X port %u nat %d last %u dis %d",
			i, (int)s->m_state, (int)s->m_isAccepted, (int)s->m_hasMap, (int)s->m_isMuted, s->m_color, s->m_startPos,
			s->m_playerTemplate, s->m_teamNumber, s->m_origColor, s->m_origStartPos, s->m_origPlayerTemplate,
			wide(s->m_name).c_str(), s->m_IP, (unsigned)s->m_port, (int)s->m_NATBehavior, s->m_lastFrameInGame,
			(int)s->m_disconnected);
	}
}

const char *str(unsigned i) { return g_strs.empty() ? "" : g_strs[i % g_strs.size()].c_str(); }

UnicodeString uname(unsigned i)
{
	UnicodeString u;
	u.translate(AsciiString(str(i)));
	return u;
}

void run_op(const unsigned *w)
{
	unsigned a = w[1], b = w[2], c = w[3], d = w[4], e = w[5];
	GameInfo *g = g_game;
	switch (w[0]) {
	case 0: {
		int before = ref_money_record_income_calls();
		Bool r = ParseAsciiStringToGameInfo((b & 1) ? NULL : g, AsciiString(str(a)));
		logf("parse %d income %d", (int)r, ref_money_record_income_calls() - before);
		break;
	}
	case 1: logf("string [%s]", GameInfoToAsciiString((b & 1) ? NULL : g).str()); break;
	case 2: g->reset(); logf("reset"); break;
	case 3: g->enterGame(); logf("enter"); break;
	case 4: g->leaveGame(); logf("leave"); break;
	case 5: g->startGame((Int)a); logf("start %d", (int)a); break;
	case 6: g->endGame(); logf("end"); break;
	case 7: g->getSlot(a % MAX_SLOTS)->setState((SlotState)(Int)b, uname(c), d); logf("setstate %u %d", a % MAX_SLOTS, (int)b); break;
	case 8: {
		GameSlot s;
		s.setState((SlotState)(Int)(b % 7), uname(c), d);
		s.setColor((Int)(e & 0xff) - 2);
		s.setPlayerTemplate((Int)((e >> 8) & 0xff) - 3);
		s.setStartPos((Int)((e >> 16) & 0xf) - 2);
		s.setTeamNumber((Int)((e >> 20) & 0xf) - 2);
		if (e & (1u << 24)) s.unAccept();
		if (e & (1u << 25)) s.setMapAvailability(false);
		g->setSlot((Int)a, s);
		logf("setslot %d", (int)a);
		break;
	}
	case 9:
		logf("q players %d nonobs %d max %d local %d host %d multi %d pre %d%d%d", g->getNumPlayers(), g->getNumNonObserverPlayers(),
			g->getMaxPlayers(), g->getLocalSlotNum(), (int)g->amIHost(), (int)g->isMultiPlayer(), (int)g->isPlayerPreorder((Int)a),
			(int)g->isPlayerPreorder((Int)b), (int)g->isPlayerPreorder(-1));
		break;
	case 10: logf("slotnum %d", g->getSlotNum(AsciiString(str(a)))); break;
	case 11: logf("skirmish %d", (int)g->isSkirmish()); break;
	case 12: logf("sandbox %d", (int)g->isSandbox()); break;
	case 13: g->setMap(AsciiString(str(a))); logf("setmap"); break;
	case 14: g->setMapCRC(a); logf("setmapcrc"); break;
	case 15: g->setMapSize(a); logf("setmapsize"); break;
	case 16: g->setCRCInterval((Int)a); logf("setcrc"); break;
	case 17: g->setLocalIP(a); logf("setip"); break;
	case 18: g->markPlayerAsPreorder((Int)a); logf("preorder"); break;
	case 19: logf("taken %d %d", (int)g->isColorTaken((Int)a, (Int)b), (int)g->isStartPositionTaken((Int)c, (Int)b)); break;
	case 20: g->resetAccepted(); logf("resetaccepted"); break;
	case 21: g->resetStartSpots(); logf("resetspots"); break;
	case 22: g->adjustSlotsForMap(); logf("adjust"); break;
	case 23: g->closeOpenSlots(); logf("close"); break;
	case 24: g->clearSlotList(); logf("clear"); break;
	case 25: {
		const GameSlot *s = g->getConstSlot(a % MAX_SLOTS);
		logf("apparent %u: pt %d col %d sp %d name [%s]", a % MAX_SLOTS, s->getApparentPlayerTemplate(), s->getApparentColor(),
			s->getApparentStartPos(), wide(s->getApparentPlayerTemplateDisplayName()).c_str());
		break;
	}
	case 26: TheGameInfo = (a % 3 == 0) ? NULL : (a % 3 == 1) ? (GameInfo *)g_game : (GameInfo *)g_other; logf("the %u", a % 3); break;
	case 27: {
		XferSave x;
		x.open(AsciiString(g_tmpName));
		g_game->xfer(&x);
		x.close();
		FILE *f = fopen(g_tmpName, "rb");
		g_saved.clear();
		int ch;
		while (f && (ch = fgetc(f)) != EOF) g_saved.push_back((unsigned char)ch);
		if (f) fclose(f);
		std::string hex;
		char hb[4];
		for (size_t i = 0; i < g_saved.size(); ++i) { snprintf(hb, sizeof hb, "%02x", g_saved[i]); hex += hb; }
		logf("save %s", hex.c_str());
		break;
	}
	case 28: {
		std::vector<unsigned char> bytes = g_saved;
		if ((b & 1) && !bytes.empty()) bytes[c % bytes.size()] = (unsigned char)d;
		FILE *f = fopen(g_tmpName, "wb");
		if (!bytes.empty()) fwrite(&bytes[0], 1, bytes.size(), f);
		fclose(f);
		XferLoad x;
		x.open(AsciiString(g_tmpName));
		try {
			g_game->xfer(&x);
			logf("load");
		} catch (...) {
			logf("load threw");
		}
		x.close();
		dumpGame("loaded", g_game);
		break;
	}
	case 29: dumpGame("game", g_game); break;
	case 30: {
		GameSlot *s = g->getSlot(a % MAX_SLOTS);
		switch (b % 8) {
		case 0: s->setColor((Int)c); break;
		case 1: s->setStartPos((Int)c); break;
		case 2: s->setPlayerTemplate((Int)c); break;
		case 3: s->setTeamNumber((Int)c); break;
		case 4: s->saveOffOriginalInfo(); break;
		case 5: s->unAccept(); break;
		case 6: s->setMapAvailability((c & 1) != 0); break;
		default: s->setAccept(); break;
		}
		logf("slotset %u %u %d", a % MAX_SLOTS, b % 8, (int)c);
		break;
	}
	default: logf("op?"); break;
	}
}

} // namespace

/* the stand-ins' configuration (also for differential/wrap_recorder.cpp, whose GameInfo is this build's) */
extern "C" void ref_ginfo_configure(const unsigned *cfg)
{
	memcpy(g_cfg, cfg, sizeof g_cfg);
	g_tick = g_cfg[3];
	snprintf(g_tmpName, sizeof g_tmpName, "/tmp/ref_ginfo_%d.sav", (int)getpid());
	g_globalData.m_defaultStartingCash.init();
	if (g_cfg[0] & 32) g_globalData.m_defaultStartingCash.deposit(10000, FALSE);
	g_mapCache.clear();
	g_mapCache[AsciiString("data\\alpha\\alpha.map")].m_numPlayers = (Int)g_cfg[4];
	g_mapCache[AsciiString("data\\alpha\\alpha.map")].m_CRC = g_cfg[5];
	g_mapCache[AsciiString("data\\beta\\beta.map")].m_numPlayers = (Int)g_cfg[6];
	g_mapCache[AsciiString("data\\beta\\beta.map")].m_CRC = g_cfg[7];
	TheMapCache = (g_cfg[0] & 1) ? &g_mapCache : NULL;
	g_mp.m_numColors = (Int)g_cfg[1];
	g_mp.m_randomTemplate = (g_cfg[0] & 4) != 0;
	g_mp.m_randomColor = (g_cfg[0] & 8) != 0;
	g_mp.m_randomStart = (g_cfg[0] & 16) != 0;
	g_mp.m_observer.m_color = 0x123456;
	TheMultiplayerSettings = (g_cfg[0] & 2) ? &g_mp : NULL;
	g_templates.m_count = (Int)g_cfg[2];
	for (int i = 0; i < 64; ++i) {
		char nb[16];
		snprintf(nb, sizeof nb, "PT%d", i);
		std::basic_string<WideChar> w;
		for (const char *p = nb; *p; ++p) w += (WideChar)*p;
		g_templates.m_t[i].m_name.set(w.c_str());
	}
}

extern "C" int ref_ginfo_run(const unsigned *cfg, const unsigned *ops, int n, const char *strs, int nstrs, char *out, int cap)
{
	g_log.clear();
	ref_ginfo_configure(cfg);
	g_strs.clear();
	for (int i = 0, at = 0; i < nstrs; ++i) {
		int len = (unsigned char)strs[at] | ((unsigned char)strs[at + 1] << 8);
		g_strs.push_back(std::string(strs + at + 2, len));
		at += 2 + len;
	}
	g_saved.clear();
	g_xferRealToPortableHook = hookRealToPortable;
	g_xferPortableToRealHook = hookPortableToReal;
	/* NEW SkirmishGameInfo: the engine's allocator zero-fills (m_localIP is never set by a constructor) */
	void *mem = calloc(1, sizeof(SkirmishGameInfo));
	g_game = new (mem) SkirmishGameInfo;
	void *mem2 = calloc(1, sizeof(SkirmishGameInfo));
	g_other = new (mem2) SkirmishGameInfo;
	TheGameInfo = g_game;
	logf("made");
	dumpGame("game", g_game);
	for (int i = 0; i < n; ++i)
		run_op(ops + 6 * i);
	TheGameInfo = NULL;
	g_xferRealToPortableHook = NULL;
	g_xferPortableToRealHook = NULL;
	g_game->~SkirmishGameInfo(); free(mem);
	g_other->~SkirmishGameInfo(); free(mem2);
	g_game = g_other = NULL;
	unlink(g_tmpName);
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}
