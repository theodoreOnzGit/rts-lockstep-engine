/* Differential-test wrapper for the Recorder (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/Recorder.cpp              (build.rs's copy, rec_gen, with one edit: see there)
**   GeneralsMD/Code/GameEngine/Include/Common/Recorder.h               (unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameMessageParser.cpp (#included below, unchanged)
** on the stand-ins of differential/shim_rec (the game logic's frame and mode, the global data, the version, the game text,
** the file system's createDirectory, no network), and the builds this test links to under the same names: MessageStream
** (wrap_message_stream.cpp: the messages, the stream, the command list, the local player, extended here with players by
** index), GameInfo (wrap_game_info.cpp: the game options string and its stand-ins, configured by ref_ginfo_configure),
** and RandomValue (wrap_random_value.cpp: InitRandom and GetGameLogicRandomSeed, unchanged).
**
** The Windows behaviour Recorder.cpp relies on is stood in for here, as the shipped build has it: a 32-bit time_t;
** SYSTEMTIME (eight 16-bit words) and GetLocalTime; and fwprintf(L"%ws"), fputwc and fgetwc on a binary stream, which
** write and read two-byte units, fgetwc returning WEOF, 0xFFFF, at the end (Windows' wint_t is an unsigned short).
** time() and GetLocalTime() count, so both sides agree.
**
** Entry point (extern "C", tests/differential_recorder.rs):
**   ref_rec_run(cfg, ginfo_cfg, ops, n, files, nfiles, out, cap)  runs n operations of six words and writes the log */
#define GameMessage MsTestGameMessage
#define GameMessageArgument MsTestGameMessageArgument
#define GameMessageList MsTestGameMessageList
#define GameMessageTranslator MsTestGameMessageTranslator
#define MessageStream MsTestMessageStream
#define CommandList MsTestCommandList
#define TheMessageStream MsTestTheMessageStream
#define TheCommandList MsTestTheCommandList
#define buildRegion MsTestBuildRegion
#define Player MsTestPlayer
#define PlayerList MsTestPlayerList
#define ThePlayerList MsTestThePlayerList
#define TheGameInfo GiTestTheGameInfo
#define TheSkirmishGameInfo GiTestTheSkirmishGameInfo
#define TheChallengeGameInfo GiTestTheChallengeGameInfo
#define GameSlot GiTestGameSlot
#define GameInfo GiTestGameInfo
#define SkirmishGameInfo GiTestSkirmishGameInfo
#define GameInfoToAsciiString GiTestGameInfoToAsciiString
#define ParseAsciiStringToGameInfo GiTestParseAsciiStringToGameInfo
#define FileSystem RcTestFileSystem
#define TheFileSystem RcTestTheFileSystem
#define GlobalData RcTestGlobalData
#define TheGlobalData RcTestTheGlobalData
#define TheWritableGlobalData RcTestTheWritableGlobalData
#define GameWindow RcTestGameWindow
#define GameWindowManager RcTestGameWindowManager
#define TheWindowManager RcTestTheWindowManager
#define GameTextInterface RcTestGameTextInterface
#define TheGameText RcTestTheGameText
#define LANAPI RcTestLANAPI
#define TheLAN RcTestTheLAN
#define TheGameSpyGame RcTestTheGameSpyGame
#define NetworkInterface RcTestNetworkInterface
#define TheNetwork RcTestTheNetwork
#define GameLogic RcTestGameLogic
#define TheGameLogic RcTestTheGameLogic
#define Version RcTestVersion
#define TheVersion RcTestTheVersion
#define RecorderClass RcTestRecorderClass
#define TheRecorder RcTestTheRecorder
#define createRecorder RcTestCreateRecorder
#define CRCInfo RcTestCRCInfo
#define ReplayGameInfo RcTestReplayGameInfo
#define REPLAY_CRC_INTERVAL RcTestREPLAY_CRC_INTERVAL
#define replayExtention RcTestReplayExtention
#define lastReplayFileName RcTestLastReplayFileName
#define GameMessageParser RcTestGameMessageParser
#define GameMessageParserArgumentType RcTestGameMessageParserArgumentType

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <time.h>
#include <wchar.h>
#include <string>
#include <vector>
#define PI 3.14159265359f   /* GameCommon.h uses it; the real PreRTS.h brings it from WWMath */
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
class INI;   /* Money.h declares parseMoneyAmount(INI *, ...) */
#include "Common/Money.h"

/* ---- the Windows behaviour (see the header comment) ---- */
typedef Int RcTime32;
#define time_t RcTime32
static RcTime32 rcTestTime(RcTime32 *t);
#define time(p) rcTestTime(p)
typedef struct { unsigned short w[8]; } SYSTEMTIME;
static void GetLocalTime(SYSTEMTIME *st);
static int rcFwprintf(FILE *f, const wchar_t *fmt, const WideChar *s);
static int rcFputwc(int c, FILE *f);
static int rcFgetwc(FILE *f);
/* every stream Recorder.cpp opens is tracked: upstream overwrites m_file without closing it (testVersionPlayback, a
** playback started while recording), and the leaked streams are closed only after the files are compared, so their
** unflushed output never reaches the disk, as in the game until it exits */
static FILE *rcFopen(const char *path, const char *mode);
static int rcFclose(FILE *f);
#define fopen rcFopen
#define fclose rcFclose
#define fwprintf rcFwprintf
#define fputwc rcFputwc
#define fgetwc rcFgetwc

#define protected public
#define private public
#include "Common/MessageStream.h"
#include "GameNetwork/GameInfo.h"
#include "Common/Recorder.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/GameMessageParser.cpp"
/* build.rs's copy (OUT_DIR/rec_gen): readUnicodeString's `= L""` initialiser, described there (CRCInfo, declared in it,
** is read by the dump: protected stays public) */
#include "Recorder.cpp"
#undef protected
#undef private

extern "C" void ref_ginfo_configure(const unsigned *cfg);

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

unsigned g_cfg[8];
RcTime32 g_time;
unsigned short g_localTime;
std::string g_dir;

} // namespace

#undef fopen
#undef fclose
static std::vector<FILE *> g_open;
static FILE *rcFopen(const char *path, const char *mode) { FILE *f = fopen(path, mode); if (f) g_open.push_back(f); return f; }
static int rcFclose(FILE *f)
{
	for (size_t i = 0; i < g_open.size(); ++i) if (g_open[i] == f) { g_open.erase(g_open.begin() + i); break; }
	return fclose(f);
}
static RcTime32 rcTestTime(RcTime32 *t) { RcTime32 v = g_time; g_time += 100; if (t) *t = v; return v; }
static void GetLocalTime(SYSTEMTIME *st) { for (int i = 0; i < 8; ++i) st->w[i] = (unsigned short)(g_localTime + i); g_localTime += 8; }
static int rcFwprintf(FILE *f, const wchar_t *, const WideChar *s)
{
	int n = 0;
	for (; s && s[n]; ++n) { fputc(s[n] & 0xff, f); fputc(s[n] >> 8, f); }
	return n;
}
static int rcFputwc(int c, FILE *f) { fputc(c & 0xff, f); fputc((c >> 8) & 0xff, f); return c & 0xffff; }
static int rcFgetwc(FILE *f)
{
	int a = fgetc(f);
	if (a == EOF) return 0xFFFF;
	int b = fgetc(f);
	if (b == EOF) return 0xFFFF;
	return a | (b << 8);
}

/* ---- the stand-ins ---- */
Bool FileSystem::createDirectory(AsciiString path) { logf("mkdir %s", path.str()); return TRUE; }
static FileSystem g_fs;
FileSystem *TheFileSystem = &g_fs;
static GlobalData g_gd;
GlobalData *TheGlobalData = &g_gd;
GlobalData *TheWritableGlobalData = &g_gd;
Int GameWindow::winHide(Bool) { return 0; }
GameWindow *GameWindowManager::winGetWindowFromId(GameWindow *, Int) { return NULL; }
static GameWindowManager g_wm;
GameWindowManager *TheWindowManager = &g_wm;
UnicodeString GameTextInterface::fetch(const char *label)
{
	std::basic_string<WideChar> w;
	w += 'T'; w += ':';
	for (const char *p = label; *p; ++p) w += (WideChar)(unsigned char)*p;
	UnicodeString u;
	u.set(w.c_str());
	return u;
}
static GameTextInterface g_text;
GameTextInterface *TheGameText = &g_text;
GameInfo *LANAPI::GetMyGame() { return NULL; }
LANAPI *TheLAN = NULL;
GameInfo *TheGameSpyGame = NULL;
NetworkInterface *TheNetwork = NULL;
void GameLogic::clearGameData(Bool) { logf("cleargamedata"); m_gameMode = GAME_NONE; }
static GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
static Version g_version;
Version *TheVersion = &g_version;
SkirmishGameInfo *TheSkirmishGameInfo = NULL;

namespace {

RecorderClass *g_rec = NULL;
SkirmishGameInfo *g_skirmish = NULL;
std::vector<std::string> g_files;

UnicodeString wideOf(const char *s)
{
	std::basic_string<WideChar> w;
	for (; *s; ++s) w += (WideChar)(unsigned char)*s;
	UnicodeString u;
	u.set(w.c_str());
	return u;
}

std::string hexOf(const unsigned char *p, size_t n)
{
	std::string h;
	char b[4];
	for (size_t i = 0; i < n; ++i) { snprintf(b, sizeof b, "%02x", p[i]); h += b; }
	return h;
}

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

void dumpList(const char *what, GameMessageList *list)
{
	logf("%s:", what);
	for (GameMessage *m = list->getFirstMessage(); m; m = m->next()) {
		std::string args;
		char b[64];
		for (GameMessageArgument *a = m->m_argList; a; a = a->m_next) {
			unsigned w[4];
			memcpy(w, &a->m_data, 16);
			snprintf(b, sizeof b, " %d:%08x.%08x.%08x.%08x", (int)a->m_type, w[0], w[1], w[2], w[3]);
			args += b;
		}
		logf(" msg type %d player %d count %u%s", (int)m->getType(), m->getPlayerIndex(), (unsigned)m->getArgumentCount(), args.c_str());
	}
}

void dumpRecorder()
{
	RecorderClass *r = g_rec;
	logf("rec mode %d orig %d next %u file %d name [%s] pos %ld desync %d interval %d current [%s] seed %u",
		(int)r->m_mode, r->m_originalGameMode, r->m_nextFrame, r->m_file ? 1 : 0, r->m_fileName.str(),
		r->m_file ? ftell(r->m_file) : -1L, (int)r->m_wasDesync, REPLAY_CRC_INTERVAL, r->getCurrentReplayFilename().str(),
		GetGameLogicRandomSeed());
	if (r->m_crcInfo) {
		std::string d;
		char b[16];
		for (std::list<UnsignedInt>::iterator it = r->m_crcInfo->m_data.begin(); it != r->m_crcInfo->m_data.end(); ++it) {
			snprintf(b, sizeof b, " %X", *it);
			d += b;
		}
		logf(" crc local %u saw %d data%s", r->m_crcInfo->m_localPlayer, (int)r->m_crcInfo->m_sawCRCMismatch, d.c_str());
	}
	GameInfo *g = &r->m_gameInfo;
	logf(" game in %d prog %d ip %X map [%s] seed %d crc %d mc %X ms %u mm %d", (int)g->m_inGame, (int)g->m_inProgress,
		g->m_localIP, g->m_mapName.str(), g->m_seed, g->m_crcInterval, g->m_mapCRC, g->m_mapSize, g->m_mapMask);
	for (int i = 0; i < MAX_SLOTS; ++i) {
		const GameSlot *s = g->getConstSlot(i);
		logf("  slot %d st %d name [%s] ip %X col %d pt %d sp %d team %d acc %d", i, (int)s->m_state, wide(s->m_name).c_str(),
			s->m_IP, s->m_color, s->m_playerTemplate, s->m_startPos, s->m_teamNumber, (int)s->m_isAccepted);
	}
}

std::string fileName(unsigned i) { return g_files.empty() ? std::string("none") : g_files[i % g_files.size()]; }

void run_op(const unsigned *w)
{
	unsigned a = w[1], b = w[2], c = w[3], d = w[4], e = w[5];
	switch (w[0]) {
	case 0: ThePlayerList->m_local.m_index = (Int)a; logf("local %d", (int)a); break;
	case 1: g_logic.m_frame = a; logf("frame %u", a); break;
	case 2: g_logic.m_gameMode = (Int)a; logf("mode %d", (int)a); break;
	case 3: {   /* a command on the command list: type a, b arguments of kinds from c, values from d and e */
		GameMessage *m = newInstance(GameMessage)((GameMessage::Type)(Int)a);
		for (unsigned i = 0; i < b; ++i) {
			unsigned k = (c >> (3 * (i % 10))) % 11, x = d + i * 0x01000193u, y = e ^ (i * 31u);
			Real rx, ry, rz;
			memcpy(&rx, &x, 4); memcpy(&ry, &y, 4); memcpy(&rz, &d, 4);
			switch (k) {
			case 0: m->appendIntegerArgument((Int)x); break;
			case 1: m->appendRealArgument(rx); break;
			case 2: m->appendBooleanArgument((x & 1) != 0); break;
			case 3: m->appendObjectIDArgument((ObjectID)x); break;
			case 4: m->appendDrawableIDArgument((DrawableID)x); break;
			case 5: m->appendTeamIDArgument(x); break;
			case 6: { Coord3D p; p.x = rx; p.y = ry; p.z = rz; m->appendLocationArgument(p); break; }
			case 7: { ICoord2D p; p.x = (Int)x; p.y = (Int)y; m->appendPixelArgument(p); break; }
			case 8: { IRegion2D p; p.lo.x = (Int)x; p.lo.y = (Int)y; p.hi.x = (Int)d; p.hi.y = (Int)e; m->appendPixelRegionArgument(p); break; }
			case 9: m->appendTimestampArgument(x); break;
			default: { WideChar ch = (WideChar)x; m->appendWideCharArgument(ch); break; }
			}
		}
		TheCommandList->appendMessage(m);
		logf("command %d args %u", (int)a, b);
		break;
	}
	case 4: TheCommandList->reset(); logf("command reset"); break;
	case 5: g_rec->update(); logf("update"); break;
	case 6: g_rec->stopRecording(); logf("stoprecording"); break;
	case 7: logf("playback %s: %d", fileName(a).c_str(), (int)g_rec->playbackFile(AsciiString(fileName(a).c_str()))); break;
	case 8: logf("testversion %s: %d", fileName(a).c_str(), (int)g_rec->testVersionPlayback(AsciiString(fileName(a).c_str()))); break;
	case 9: g_rec->handleCRCMessage(a, (Int)b, (c & 1) != 0); logf("crc %X %d %u", a, (int)b, c & 1); break;
	case 10: g_rec->logPlayerDisconnect(UnicodeString(), (Int)a); logf("discon %d", (int)a); break;
	case 11: g_rec->logCRCMismatch(); logf("crcmismatch"); break;
	case 12: dumpRecorder(); break;
	case 13: dumpList("commands", TheCommandList); dumpList("stream", TheMessageStream); break;
	case 14: g_rec->reset(); logf("reset"); break;
	case 15: g_rec->stopPlayback(); logf("stopplayback"); break;
	case 16: logf("multiplayer %d", (int)g_rec->isMultiplayer()); break;
	case 17: TheSkirmishGameInfo = (a & 1) ? g_skirmish : NULL; logf("skirmish %u", a & 1); break;
	case 18: TheMessageStream->propagateMessages(); logf("propagate"); break;
	case 19: {   /* players: count a, player i's name key NAMEKEY("player<k>") for k from b */
		ThePlayerList->m_count = (Int)(a % 9);
		for (int i = 0; i < 8; ++i) {
			char nb[32];
			snprintf(nb, sizeof nb, "player%d", (int)((b >> (4 * i)) & 0xf) - 1);
			ThePlayerList->m_players[i].m_nameKey = (UnsignedInt)NAMEKEY(nb);
		}
		logf("players %d", ThePlayerList->m_count);
		break;
	}
	case 20: g_rec->updatePlayback(); logf("updateplayback"); break;
	default: logf("op?"); break;
	}
}

} // namespace

extern "C" int ref_rec_run(const unsigned *cfg, const unsigned *ginfo_cfg, const unsigned *ops, int n, const char *files, int nfiles,
	char *out, int cap)
{
	g_log.clear();
	memcpy(g_cfg, cfg, sizeof g_cfg);
	ref_ginfo_configure(ginfo_cfg);
	g_time = (RcTime32)g_cfg[0];
	g_localTime = (unsigned short)g_cfg[1];
	char dir[64];
	snprintf(dir, sizeof dir, "/tmp/ref_rec_%d/", (int)getpid());
	g_dir = dir;
	mkdir(dir, 0700);
	g_gd.m_userData = AsciiString(dir);
	g_gd.m_pendingFile = AsciiString((g_cfg[2] & 1) ? "data\\alpha\\alpha.map" : "");
	g_gd.m_mapName = AsciiString("data\\beta\\beta.map");
	g_gd.m_exeCRC = g_cfg[3];
	g_gd.m_iniCRC = g_cfg[4];
	g_version.m_version = wideOf("1.04");
	g_version.m_buildTime = wideOf((g_cfg[2] & 2) ? "Oct 31 2026" : "Jan 1 2003");
	g_version.m_number = g_cfg[5];
	REPLAY_CRC_INTERVAL = 100;   /* the global's initial value; a playback sets it */
	g_logic.m_frame = 0;
	g_logic.m_gameMode = GAME_NONE;
	InitRandom(g_cfg[6]);
	if (!TheNameKeyGenerator) { TheNameKeyGenerator = new NameKeyGenerator; TheNameKeyGenerator->init(); }
	/* the replay files: name, then contents, each a 16-bit length and bytes */
	g_files.clear();
	for (int i = 0, at = 0; i < nfiles; ++i) {
		int ln = (unsigned char)files[at] | ((unsigned char)files[at + 1] << 8);
		std::string name(files + at + 2, ln);
		at += 2 + ln;
		int lc = (unsigned char)files[at] | ((unsigned char)files[at + 1] << 8) | ((unsigned char)files[at + 2] << 16);
		std::string path = g_dir + "Replays\\" + name;
		FILE *f = fopen(path.c_str(), "wb");
		if (lc > 0) fwrite(files + at + 3, 1, lc, f);
		fclose(f);
		at += 3 + lc;
		g_files.push_back(name);
	}
	TheMessageStream = new MessageStream;
	TheCommandList = new CommandList;
	ThePlayerList->m_local.m_index = 0;
	ThePlayerList->m_count = 0;
	void *mem = calloc(1, sizeof(SkirmishGameInfo));
	g_skirmish = new (mem) SkirmishGameInfo;
	TheSkirmishGameInfo = NULL;
	void *rmem = calloc(1, sizeof(RecorderClass));
	g_rec = new (rmem) RecorderClass;
	logf("made");
	dumpRecorder();
	for (int i = 0; i < n; ++i)
		run_op(ops + 6 * i);
	/* the end: the file closed, then every file of the replay directory, by name */
	if (g_rec->m_file) { rcFclose(g_rec->m_file); g_rec->m_file = NULL; }
	std::vector<std::string> names = g_files;
	names.push_back("00000000.rep");
	for (size_t i = 0; i < names.size(); ++i) {
		std::string path = g_dir + "Replays\\" + names[i];
		FILE *f = fopen(path.c_str(), "rb");
		if (!f) { logf("file %s: none", names[i].c_str()); continue; }
		std::vector<unsigned char> bytes;
		int ch;
		while ((ch = fgetc(f)) != EOF) bytes.push_back((unsigned char)ch);
		fclose(f);
		logf("file %s: %s", names[i].c_str(), hexOf(bytes.empty() ? NULL : &bytes[0], bytes.size()).c_str());
	}
	/* the leaked streams, closed after the comparison (their buffers then reach files that are deleted next) */
	while (!g_open.empty()) rcFclose(g_open.back());
	for (size_t i = 0; i < names.size(); ++i) unlink((g_dir + "Replays\\" + names[i]).c_str());
	rmdir(dir);
	TheSkirmishGameInfo = NULL;
	TheMessageStream = NULL;
	TheCommandList = NULL;
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}
