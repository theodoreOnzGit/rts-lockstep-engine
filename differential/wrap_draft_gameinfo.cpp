/* Differential-test wrapper for the LAN and GameSpy staging-room game-info drafts (#215, group gameinfo), upstream commit 0a05454d:
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANGameInfo.cpp           (cut verbatim by build.rs: the LANGameSlot and LANGameInfo
**                                                                           members, then GenerateGameOptionsString and
**                                                                           ParseGameOptionsString, in OUT_DIR/dgi_gen)
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/StagingRoomGameInfo.cpp  (cut verbatim by build.rs: GameSpyGameSlot's
**                                                                           constructor, then setPingString to reset, in OUT_DIR/dgi_gen)
**   GeneralsMD/Code/GameEngine/Include/GameNetwork/LANGameInfo.h, LANPlayer.h, GameSpy/StagingRoomGameInfo.h  (unchanged)
** on top of differential/wrap_game_info.cpp, which is #included whole: the GameInfo.cpp build (GameInfo, GameSlot, the options
** string) and its stand-ins (map cache, game text, map-path conversions, ...), exactly as the verified GameInfo test runs it.
** This file's own object is only linked by tests/differential_draft_gameinfo.rs, which does not call ref_ginfo_run, so the copy of
** wrap_game_info.cpp's symbols in upstream_game_info is not pulled in beside it.
**
** Stand-ins (what the cuts call outside themselves), every one renamed Dg* by the macros below so that no inline member of another
** wrapper's class of the same name can be merged with it by the linker:
**   LANAPI (TheLAN)                 GetLocalIP, ResetGameStartTimer, GetMyGame, AmIHost, RequestHasMap: fixed answers from the configuration,
**                                   the two calls logged
**   LANEnableStartButton, lanUpdateSlotList, updateGameOptions, timeGetTime   logged / a counter stepping by 11
**   TheGameSpyInfo                  getLocalName, getInternalIP, getLocalProfileID, getPlayerInfoMap (found by a hash of the name),
**                                   getPingValue (a hash of the string), didPlayerPreorder, leaveStagingRoom (logged)
**   TheNAT / NAT                    attachSlotList, establishConnectionPaths, getSlotPort, getTransport (all logged); `delete TheNAT` logged
**   TheNetwork / NetworkInterface   createNetwork, init, setLocalAddress, attachTransport, initTransport, parseUserList (all logged),
**                                   sawCRCMismatch, getAverageFPS (from the configuration); `delete TheNetwork` logged
**   TheGameLogic                    isInGame, clearGameData (logged)
**   DoAnyMapTransfers, TheMapCache (updateCache, findMap), GSMessageBoxOk, PopBackToLobby, InitGameLogicRandom   answers/logged
**   TheWritableGlobalData           m_pendingFile and m_useFpsLimit (logged when written)
**   TheMessageStream / GameMessage  appendMessage(MSG_NEW_GAME), appendIntegerArgument (logged)
**   TheBuddyMessageQueue            addRequest(BuddyRequest) (logged)
**   ThePlayerList / Player / ScoreKeeper / the player template's side   which slots have a `player<i>`, and their statistics, from the configuration
**   NAMEKEY(name)                   the slot number in "player<i>" (the name-key generator is not in this cut)
**   TheVictoryConditions            getEndFrame, hasAchievedVictory (from the configuration)
**   TheGameSpyPSMessageQueue        findPlayerStatsByID(id).locale (a hash of the id)
** The entry point and the operations are described at dgi_run_op and mirrored in tests/differential_draft_gameinfo.rs. */
#include "wrap_game_info.cpp"

#include <map>

#undef TheGameSpyGame
#define TheGameSpyGame DgTheGameSpyGame
#define NAT DgNAT
#define TheNAT DgTheNAT
#define NetworkInterface DgNetworkInterface
#define TheNetwork DgTheNetwork
#define GameLogic DgGameLogic
#define TheGameLogic DgTheGameLogic
#define DoAnyMapTransfers DgDoAnyMapTransfers
#undef TheMapCache
#define TheMapCache DgTheMapCache
#define GSMessageBoxOk DgGSMessageBoxOk
#define PopBackToLobby DgPopBackToLobby
#define InitGameLogicRandom DgInitGameLogicRandom
#define TheWritableGlobalData DgTheWritableGlobalData
#define GameMessage DgGameMessage
#define TheMessageStream DgTheMessageStream
#define BuddyRequest DgBuddyRequest
#define TheGameSpyBuddyMessageQueue DgTheGameSpyBuddyMessageQueue
#define Player DgPlayer
#define ScoreKeeper DgScoreKeeper
#define ThePlayerList DgThePlayerList
#define TheVictoryConditions DgTheVictoryConditions
#define TheGameSpyPSMessageQueue DgTheGameSpyPSMessageQueue
#define PSPlayerStats DgPSPlayerStats
#define TheGameSpyInfo DgTheGameSpyInfo
#define PlayerInfoMap DgPlayerInfoMap
#define NAMEKEY(x) dgNameKey(x)
#define LANEnableStartButton DgLANEnableStartButton
#define lanUpdateSlotList DgLanUpdateSlotList
#define updateGameOptions DgUpdateGameOptions
#define timeGetTime DgTimeGetTime
#define GenerateGameOptionsString DgGenerateGameOptionsString
#define ParseGameOptionsString DgParseGameOptionsString
#define LANAPI DgLANAPI
#undef TheLAN
#define TheLAN DgTheLAN

class Transport;
class DgLANAPI;
class LANGameInfo;

#define protected public
#define private public
#include "../upstream/GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/StagingRoomGameInfo.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Include/GameNetwork/LANGameInfo.h"
#undef protected
#undef private

extern "C" {
unsigned long dgLogOnly(void);
}

namespace {
/* the configuration words, see run_op */
unsigned g_dcfg[16];
unsigned g_dtime;

unsigned hashBytes(const char *s, unsigned salt)
{
	unsigned h = salt * 2654435761u + 17u;
	for (const unsigned char *q = (const unsigned char *)s; *q; ++q) h = h * 31u + *q;
	return h ^ (h >> 15);
}
unsigned hashInt(unsigned v, unsigned salt)
{
	unsigned h = (v + 0x9e3779b9u) * 2654435761u + salt * 40503u;
	return h ^ (h >> 13);
}
} // namespace

/* ---- stand-ins -------------------------------------------------------------------------------- */
static const char *const g_sides[4] = { "America", "China", "GLA", "Boss" };
static int g_sideOf[8];
struct DgPlayerTemplate { int slot; AsciiString getSide() const { return AsciiString(g_sides[g_sideOf[slot] & 3]); } };
static DgPlayerTemplate g_dgTemplates[8];
struct DgScoreKeeper
{
	int slot;
	int stat(int k) const { return (int)(hashInt((unsigned)slot * 16u + (unsigned)k, g_dcfg[12]) & 0xffff); }
	Int getTotalUnitsDestroyed() const { return stat(0); }
	Int getTotalUnitsLost() const { return stat(1); }
	Int getTotalUnitsBuilt() const { return stat(2); }
	Int getTotalBuildingsDestroyed() const { return stat(3); }
	Int getTotalBuildingsLost() const { return stat(4); }
	Int getTotalBuildingsBuilt() const { return stat(5); }
	Int getTotalMoneyEarned() const { return stat(6); }
	Int getTotalTechBuildingsCaptured() const { return stat(7); }
};
struct DgPlayer
{
	int slot;
	DgScoreKeeper keeper;
	DgScoreKeeper *getScoreKeeper() { return &keeper; }
	DgPlayerTemplate *getPlayerTemplate();
};
static DgPlayer g_dgPlayers[8];
DgPlayerTemplate *DgPlayer::getPlayerTemplate() { return &g_dgTemplates[slot]; }

int dgNameKey(const AsciiString &s)
{
	/* "player<i>" */
	const char *p = s.str();
	return atoi(p + 6);
}
struct DgPlayerList
{
	DgPlayer *findPlayerWithNameKey(int k) { return (k >= 0 && k < 8 && ((g_dcfg[9] >> k) & 1)) ? &g_dgPlayers[k] : NULL; }
};
static DgPlayerList g_dgPlayerList;
DgPlayerList *ThePlayerList = &g_dgPlayerList;

struct DgVictoryConditions
{
	Int getEndFrame() { return (Int)g_dcfg[7]; }
	Bool hasAchievedVictory(DgPlayer *p) { return (g_dcfg[10] >> p->slot) & 1; }
};
static DgVictoryConditions g_dgVictory;
DgVictoryConditions *TheVictoryConditions = &g_dgVictory;

struct DgPSPlayerStats { Int locale; };
struct DgPSQueue { DgPSPlayerStats findPlayerStatsByID(Int id) { DgPSPlayerStats s; s.locale = (Int)(hashInt((unsigned)id, 5) % 1000u); return s; } };
static DgPSQueue g_dgPS;
DgPSQueue *TheGameSpyPSMessageQueue = &g_dgPS;

struct DgPlayerInfo { Int m_profileID; AsciiString m_locale; Int m_rankPoints; Int m_side; };
struct DgNameLess { bool operator()(const AsciiString &a, const AsciiString &b) const { return strcmp(a.str(), b.str()) < 0; } };
typedef std::map<AsciiString, DgPlayerInfo, DgNameLess> DgPlayerInfoMap;
#define DgPlayerInfoMap_is_map 1
struct DgGameSpyInfo
{
	DgPlayerInfoMap m_map;
	DgPlayerInfoMap *getPlayerInfoMap() { return &m_map; }
	AsciiString getLocalName() { return AsciiString(str(g_dcfg[2])); }
	UnsignedInt getInternalIP() { return g_dcfg[3]; }
	Int getLocalProfileID() { return (Int)g_dcfg[4]; }
	Int getPingValue(AsciiString s) { return (Int)(hashBytes(s.str(), 9) % 500u); }
	Bool didPlayerPreorder(Int id) { return (g_dcfg[5] >> (id & 31)) & 1; }
	void leaveStagingRoom();
};
static DgGameSpyInfo g_dgInfo;
DgGameSpyInfo *TheGameSpyInfo = &g_dgInfo;

/* a transport the NAT hands out */
struct DgTransportTag {};
static DgTransportTag g_dgTransportObj;

struct DgNAT
{
	void attachSlotList(GameSlot **slotList, Int localSlot, UnsignedInt localIP)
	{
		int n = 0;
		for (int i = 0; i < MAX_SLOTS; ++i) if (slotList[i]) ++n;
		logf("nat attachSlotList slots %d local %d ip %u", n, (int)localSlot, (unsigned)localIP);
	}
	void establishConnectionPaths() { logf("nat establishConnectionPaths"); }
	UnsignedShort getSlotPort(Int slot) { logf("nat getSlotPort %d", (int)slot); return (UnsignedShort)(g_dcfg[8] + slot * 3); }
	Transport *getTransport() { logf("nat getTransport"); return (Transport *)&g_dgTransportObj; }
};
DgNAT *TheNAT = NULL;

struct DgNetworkInterface
{
	static DgNetworkInterface *createNetwork() { logf("network create"); return new DgNetworkInterface; }
	void init() { logf("network init"); }
	void setLocalAddress(UnsignedInt ip, UnsignedShort port) { logf("network setLocalAddress %u %u", (unsigned)ip, (unsigned)port); }
	void attachTransport(Transport *t) { logf("network attachTransport %d", t == (Transport *)&g_dgTransportObj); }
	void initTransport() { logf("network initTransport"); }
	void parseUserList(GameInfo *g) { logf("network parseUserList"); }
	Bool sawCRCMismatch() { return (g_dcfg[1] >> 8) & 1; }
	Int getAverageFPS() { return (Int)g_dcfg[6]; }
	~DgNetworkInterface() { logf("network delete"); }
};
DgNetworkInterface *TheNetwork = NULL;

struct DgGameLogic
{
	Bool isInGame() { return (g_dcfg[1] >> 5) & 1; }
	void clearGameData() { logf("gamelogic clearGameData"); }
};
static DgGameLogic g_dgLogic;
DgGameLogic *TheGameLogic = &g_dgLogic;

Bool DgDoAnyMapTransfers(GameInfo *game) { logf("doAnyMapTransfers"); return (g_dcfg[1] >> 6) & 1; }
struct DgMapCache
{
	void updateCache() { logf("mapcache updateCache"); }
	void *findMap(AsciiString m) { logf("mapcache findMap [%s]", m.str()); return ((g_dcfg[1] >> 7) & 1) ? (void *)this : NULL; }
};
static DgMapCache g_dgMapCache;
DgMapCache *TheMapCache = &g_dgMapCache;
void DgGSMessageBoxOk(UnicodeString title, UnicodeString body) { logf("messagebox [%s] [%s]", wide(title).c_str(), wide(body).c_str()); }
void DgPopBackToLobby(void) { logf("popBackToLobby"); }
void DgInitGameLogicRandom(Int seed) { logf("initGameLogicRandom %d", (int)seed); }

struct DgGlobalDataWritable
{
	AsciiString m_pendingFile;
	Bool m_useFpsLimit;
};
static DgGlobalDataWritable g_dgGlobal;
DgGlobalDataWritable *TheWritableGlobalData = &g_dgGlobal;

enum { DG_GAME_INTERNET = 5 };
struct DgGameMessage
{
	enum { MSG_NEW_GAME = 1 };
	void appendIntegerArgument(Int v) { logf("message appendIntegerArgument %d", (int)v); }
};
struct DgMessageStream
{
	DgGameMessage *appendMessage(int t) { static DgGameMessage m; logf("message append %d", t); return &m; }
};
static DgMessageStream g_dgStream;
DgMessageStream *TheMessageStream = &g_dgStream;

enum { GP_PLAYING_DG = 3 };
struct DgBuddyRequest
{
	enum { BUDDYREQUEST_SETSTATUS = 7 };
	int buddyRequestType;
	struct { struct { int status; char statusString[64]; char locationString[256]; } status; } arg;
};
struct DgBuddyQueue
{
	void addRequest(const DgBuddyRequest &r)
	{
		logf("buddy request %d status %d [%s] [%s]", r.buddyRequestType, r.arg.status.status, r.arg.status.statusString, r.arg.status.locationString);
	}
};
static DgBuddyQueue g_dgBuddy;
DgBuddyQueue *TheGameSpyBuddyMessageQueue = &g_dgBuddy;
#define GP_PLAYING GP_PLAYING_DG
#define GAME_INTERNET DG_GAME_INTERNET

void DgGameSpyInfo_leave();
void DgGameSpyInfo::leaveStagingRoom() { logf("gamespyinfo leaveStagingRoom"); }

/* LAN */
class DgLANAPI
{
public:
	LANGameInfo *m_my;
	UnsignedInt GetLocalIP() { return g_dcfg[0]; }
	void ResetGameStartTimer() { logf("lan resetGameStartTimer"); }
	LANGameInfo *GetMyGame() { return m_my; }
	Bool AmIHost() { return (g_dcfg[1] >> 1) & 1; }
	void RequestHasMap() { logf("lan requestHasMap"); }
};
static DgLANAPI g_dgLan;
DgLANAPI *DgTheLAN = NULL;
void DgLANEnableStartButton(Bool e) { logf("lan enableStartButton %d", (int)e); }
void DgLanUpdateSlotList() { logf("lan updateSlotList"); }
void DgUpdateGameOptions() { logf("lan updateGameOptions"); }
UnsignedInt DgTimeGetTime() { g_dtime += 11; return g_dtime; }

/* UnicodeString's operator< (UnicodeString.h lines 430-433: `wcscmp(s1.str(), s2.str()) < 0`), which the shim UnicodeString of the GameInfo
** build lacks and ParseGameOptionsString's std::map<UnicodeString, UnicodeString> needs. wcscmp compares 16-bit unsigned units (MSVC's wchar_t). */
inline Bool operator<(const UnicodeString &s1, const UnicodeString &s2)
{
	const WideChar *a = s1.str(), *b = s2.str();
	while (*a && *a == *b) { ++a; ++b; }
	return (unsigned)*a < (unsigned)*b;
}

/* ---- the cuts, verbatim (build.rs: dgi_gen) ---------------------------------------------------- */
#include "dgi_lan_slots.inc"
#include "dgi_lan_options.inc"
#include "dgi_staging.inc"

GameSpyStagingRoom *TheGameSpyGame = NULL; /* the real header's extern, here DgTheGameSpyGame */

namespace {

GameSpyStagingRoom *g_room = NULL;
LANGameInfo *g_lan = NULL;
std::string g_lastOptions;

std::string asc(const AsciiString &a) { return std::string(a.str()); }

void dumpRoom(const char *what)
{
	GameSpyStagingRoom *g = g_room;
	logf("%s: in %d prog %d ip %u map [%s] seed %d pre %d name [%s] id %d ping [%s] %d qm %d ladder [%s] %u pending [%s] fps %d",
		what, (int)g->m_inGame, (int)g->m_inProgress, (unsigned)g->m_localIP, g->m_mapName.str(), (int)g->m_seed, g->m_preorderMask,
		wide(g->m_gameName).c_str(), (int)g->m_id, g->m_pingStr.str(), (int)g->m_pingInt, (int)g->m_isQM,
		g->m_ladderIP.str(), (unsigned)g->m_ladderPort, g_dgGlobal.m_pendingFile.str(), (int)g_dgGlobal.m_useFpsLimit);
	for (int i = 0; i < MAX_SLOTS; ++i) {
		const GameSlot *s = g->getConstSlot(i);
		const GameSpyGameSlot &x = g->m_GameSpySlot[i];
		logf(" slot %d: st %d acc %d map %d team %d name [%s] ip %u dis %d | login [%s] pid %d loc [%s] ping [%s] %d wl %d %d rank %d side %d",
			i, (int)s->m_state, (int)s->m_isAccepted, (int)s->m_hasMap, s->m_teamNumber, wide(s->m_name).c_str(), (unsigned)s->m_IP,
			(int)s->m_disconnected, x.m_gameSpyLogin.str(), (int)x.m_profileID, x.m_gameSpyLocale.str(), x.m_pingStr.str(), (int)x.m_pingInt,
			(int)x.m_wins, (int)x.m_losses, (int)x.m_rankPoints, (int)x.m_favoriteSide);
	}
}

void dumpLan(const char *what)
{
	LANGameInfo *g = g_lan;
	logf("%s: in %d prog %d ip %u map [%s] crc %u seed %d last %u name [%s] direct %d", what, (int)g->m_inGame, (int)g->m_inProgress,
		(unsigned)g->m_localIP, g->m_mapName.str(), (unsigned)g->m_mapCRC, (int)g->m_seed, (unsigned)g->m_lastHeard,
		wide(g->m_gameName).c_str(), (int)g->m_isDirectConnect);
	for (int i = 0; i < MAX_SLOTS; ++i) {
		LANGameSlot &s = g->m_LANSlot[i];
		logf(" slot %d: st %d acc %d map %d name [%s] ip %u last %u serial [%s] login [%s] host [%s]", i,
			(int)s.m_state, (int)s.m_isAccepted, (int)s.m_hasMap, wide(s.m_name).c_str(), (unsigned)s.m_IP, (unsigned)s.m_lastHeard,
			s.m_serial.str(), wide(s.m_user.m_login).c_str(), wide(s.m_user.m_host).c_str());
	}
}

/* Operations: six words each (op a b c d e). Strings are g_strs (the str(i) of wrap_game_info.cpp).
**   0  room slot a: setState(b % 7, name str(c), ip d), then team (e & 15) - 2, the disconnected flag (bit 4), accepted (bit 5), the map flag (bit 6)
**   1  room: a == 0 enterGame, 1 leaveGame, 2 setGameInProgress(b & 1), 3 reset, 4 init
**   2  room: a: 0 setMap(str b), 1 seed = b, 2 setLocalName(str b), 3 setLadder(str b, port c), 4 markGameAsQM, 5 setGameName(str b), 6 setID(b),
**      7 localIP = b
**   3  room queries: amIHost, getLocalSlotNum, ping, getGameSpySlot(a) non-null
**   4  room setPingString(str a)
**   5  room gs slot a: login str(b), profile c, locale str(d), e: wins (low 8), losses (next 8), rank (next 8), side (top 8)
**   6  both results packets
**   7  startGame(a)
**   8  launchGame
**   9  room resetAccepted, then the accept flags
**   10 dump the room
**   11 game NAT: a == 0 none, 1 create one; network: b == 0 none, 1 create one (before an operation that starts a game)
**   20 lan slot a: setState(b % 7, str(c), d); setLogin(str(e & 255)), setHost(str((e >> 8) & 255)), setSerial(str((e >> 16) & 255)),
**      setLastHeard(e >> 24 * 1000); then lan->setSlot(a, slot)
**   21 lan queries: local slot, slot number of str(a), amIHost, hostIP, and for slot b: getIP, getPlayerLastHeard, getUser, isUser(user of slot c),
**      isUser(str(d)), isLocalPlayer
**   22 lan resetAccepted
**   23 lan: a: 0 enterGame, 1 leaveGame, 2 setMap(str b), 3 setSeed(b), 4 setName(str b), 5 setLastHeard(b), 6 setIsDirectConnect(b&1),
**      7 setPlayerName(who c, str b), 8 setPlayerLastHeard(who c, b), 9 setIP(who c, b)
**   24 lan ParseGameOptionsString(the room's options string, mutated: b selects the mutation, c the position, d the character)
**   25 GenerateGameOptionsString
**   26 dump the lan game
**   27 lan: set TheLAN's current game: a == 1 this, 0 none
*/
void dgi_run_op(const unsigned *w)
{
	unsigned a = w[1], b = w[2], c = w[3], d = w[4], e = w[5];
	GameSpyStagingRoom *r = g_room;
	LANGameInfo *l = g_lan;
	switch (w[0]) {
	case 0: {
		GameSlot *s = r->getSlot(a % MAX_SLOTS);
		s->setState((SlotState)(Int)(b % 7), uname(c), d);
		s->setTeamNumber((Int)(e & 15) - 2);
		s->m_disconnected = (e & 16) != 0;
		if (e & 32) s->setAccept(); else s->unAccept();
		s->setMapAvailability((e & 64) != 0);
		logf("room setslot %u", a % MAX_SLOTS);
		break;
	}
	case 1:
		switch (a % 5) {
		case 0: r->enterGame(); break;
		case 1: r->leaveGame(); break;
		case 2: r->setGameInProgress((b & 1) != 0); break;
		case 3: r->reset(); break;
		default: r->init(); break;
		}
		logf("room life %u", a % 5);
		break;
	case 2:
		switch (a % 8) {
		case 0: if (r->m_inGame) { logf("skip setMap in game"); break; } r->setMap(AsciiString(str(b))); break; /* in a game GameInfo::setMap calls the virtual amIHost(), which the staging room overrides and the verified base cannot: see the test */
		case 1: r->setSeed((Int)b); break;
		case 2: r->setLocalName(AsciiString(str(b))); break;
		case 3: r->setLadderIP(AsciiString(str(b))); r->setLadderPort((UnsignedShort)c); break;
		case 4: r->markGameAsQM(); break;
		case 5: r->setGameName(uname(b)); break;
		case 6: r->setID((Int)b); break;
		default: r->setLocalIP(b); break;
		}
		logf("room set %u", a % 8);
		break;
	case 3:
		logf("room q host %d local %d ping [%s] %d slot %d", (int)r->amIHost(), r->getLocalSlotNum(), r->getPingString().str(), r->getPingAsInt(),
			r->getGameSpySlot(a % 12 == 11 ? -1 : (Int)(a % 12)) != NULL);
		break;
	case 4: r->setPingString(AsciiString(str(a))); logf("room ping set"); break;
	case 5: {
		GameSpyGameSlot *s = &r->m_GameSpySlot[a % MAX_SLOTS];
		s->setLoginName(AsciiString(str(b)));
		s->setProfileID((Int)c);
		s->setLocale(AsciiString(str(d)));
		s->setWins((Int)(e & 255));
		s->setLosses((Int)((e >> 8) & 255));
		s->setSlotRankPoints((Int)((e >> 16) & 255));
		s->setFavoriteSide((Int)(e >> 24));
		s->setPingString(AsciiString(str(b)));
		logf("room gs slot %u", a % MAX_SLOTS);
		break;
	}
	case 6: {
		for (int i = 0; i < 8; ++i) { g_dgPlayers[i].slot = i; g_dgPlayers[i].keeper.slot = i; g_sideOf[i] = (int)((g_dcfg[11] >> (2 * i)) & 3); }
		for (int i = 0; i < 8; ++i) g_dgTemplates[i].slot = i;
		AsciiString s1 = r->generateGameSpyGameResultsPacket();
		logf("results gs [%s]", s1.str());
		AsciiString s2 = r->generateLadderGameResultsPacket();
		logf("results ladder [%s]", s2.str());
		break;
	}
	case 7: r->startGame((Int)a); logf("room start"); break;
	case 8: r->launchGame(); logf("room launch"); break;
	case 9: {
		r->resetAccepted();
		std::string f;
		for (int i = 0; i < MAX_SLOTS; ++i) f += r->getConstSlot(i)->m_isAccepted ? '1' : '0';
		logf("room resetAccepted %s", f.c_str());
		break;
	}
	case 10: dumpRoom("room"); break;
	case 11:
		if (a == 0 && TheNAT) { delete TheNAT; TheNAT = NULL; }
		if (a == 1 && !TheNAT) TheNAT = new DgNAT;
		if (b == 0 && TheNetwork) { delete TheNetwork; TheNetwork = NULL; }
		if (b == 1 && !TheNetwork) TheNetwork = new DgNetworkInterface;
		logf("globals nat %d network %d", TheNAT != NULL, TheNetwork != NULL);
		break;
	case 20: {
		LANGameSlot s;
		s.setState((SlotState)(Int)(b % 7), uname(c), d);
		s.setLogin(uname(e & 255));
		s.setHost(uname((e >> 8) & 255));
		s.setSerial(AsciiString(str((e >> 16) & 255)));
		s.setLastHeard((e >> 24) * 1000u);
		l->setSlot((Int)a - 1, s);
		logf("lan setslot %d", (int)a - 1);
		break;
	}
	case 21: {
		if (!TheLAN) { logf("skip"); break; } /* LANGameSlot::isLocalPlayer tests TheLAN; the draft takes the local IP (TheLAN is the caller) */
		LANGameSlot *sb = l->getLANSlot((Int)b % 10 - 1);
		LANGameSlot *sc = l->getLANSlot((Int)c % 10 - 1);
		logf("lan q local %d slotnum %d host %d hostip %u", l->getLocalSlotNum(), l->getSlotNum(uname(a)), (int)l->amIHost(), (unsigned)l->getHostIP());
		if (sb) {
			LANPlayer *u = sb->getUser();
			logf("lan q slot ip %u heard %u user %d [%s] ip %u heard %u login [%s] host [%s] isUser %d isName %d local %d serial [%s]",
				(unsigned)sb->getIP(), (unsigned)sb->getLastHeard(), u != NULL, u ? wide(u->getName()).c_str() : "", u ? (unsigned)u->getIP() : 0,
				u ? (unsigned)u->getLastHeard() : 0, u ? wide(u->getLogin()).c_str() : "", u ? wide(u->getHost()).c_str() : "",
				(int)sb->isUser(sc ? sc->getUser() : NULL), (int)sb->isUser(uname(d)), (int)sb->isLocalPlayer(), sb->getSerial().str());
		}
		Int who = (Int)(b % 8);
		logf("lan q who %d ip %u heard %u name [%s]", (int)who, (unsigned)l->getIP(who), (unsigned)l->getPlayerLastHeard(who), wide(l->getPlayerName(who)).c_str());
		break;
	}
	case 22: l->resetAccepted(); {
		std::string f;
		for (int i = 0; i < MAX_SLOTS; ++i) f += l->getConstLANSlot(i)->m_isAccepted ? '1' : '0';
		logf("lan resetAccepted %s", f.c_str());
	} break;
	case 23:
		switch (a % 10) {
		case 0: l->enterGame(); break;
		case 1: l->leaveGame(); break;
		case 2: l->setMap(AsciiString(str(b))); break;
		case 3: l->setSeed((Int)b); break;
		case 4: l->setName(uname(b)); break;
		case 5: l->setLastHeard(b); break;
		case 6: l->setIsDirectConnect((b & 1) != 0); break;
		case 7: l->setPlayerName((Int)(c % 8), uname(b)); break;
		case 8: l->setPlayerLastHeard((Int)(c % 8), b); break;
		default: l->setIP((Int)(c % 8), b); break;
		}
		logf("lan set %u", a % 10);
		break;
	case 24: {
		std::string o = GameInfoToAsciiString(r).str();
		switch (b % 6) {
		case 1: if (!o.empty()) o.erase(c % o.size(), 1); break;
		case 2: if (!o.empty()) o.insert(c % o.size(), 1, (char)(d ? d : 'x')); break;
		case 3: if (!o.empty()) o[c % o.size()] = (char)(d ? d : 'x'); break;
		case 4: if (!o.empty()) o.resize(c % o.size()); break;
		default: break;
		}
		g_lastOptions = o;
		Bool ok = ParseGameOptionsString(l, AsciiString(o.c_str()));
		logf("lan parse %d len %u", (int)ok, (unsigned)o.size());
		dumpLan("lan after parse");
		break;
	}
	case 25: if (!TheLAN) { logf("skip"); break; } logf("lan options [%s]", GenerateGameOptionsString().str()); break;
	case 26: dumpLan("lan"); break;
	case 28: TheLAN = (a == 1) ? &g_dgLan : NULL; logf("lan thelan %d", (int)(a == 1)); break;
	case 27: g_dgLan.m_my = (a == 1) ? l : NULL; logf("lan mygame %d", (int)(a == 1)); break;
	default: logf("op?"); break;
	}
}

} // namespace

/* cfg words (16): [0] TheLAN->GetLocalIP, [1] flags: bit 0 TheLAN set, bit 1 AmIHost, bit 5 TheGameLogic in game, bit 6 DoAnyMapTransfers
** succeeds, bit 7 the map cache has the map, bit 8 TheNetwork->sawCRCMismatch; [2] the string index of TheGameSpyInfo's local name, [3] its
** internal IP, [4] its local profile id, [5] the preorder mask by profile id, [6] the average FPS, [7] the end frame, [8] the NAT's slot port
** base, [9] the mask of slots that have a "player<i>", [10] the mask of those that achieved victory, [11] two bits per slot: the side,
** [12] the statistics' salt, [13] the player-info map's salt (a name is found if hash % 3 != 0). */
extern "C" int ref_dgi_run(const unsigned *cfg, const unsigned *ops, int n, const char *strs, int nstrs, char *out, int cap)
{
	g_log.clear();
	memcpy(g_dcfg, cfg, sizeof(unsigned) * 14);
	g_dtime = 0;
	/* the GameInfo stand-ins of the verified test: a map cache and multiplayer settings, 8 colours, 4 templates, two maps */
	{ static const unsigned gi[9] = { 3, 8, 4, 0, 4, 0x1234, 2, 0x77, 12345 }; memcpy(g_cfg, gi, sizeof gi); }
	ref_ginfo_configure(g_cfg);
	g_tick = 0;
	g_strs.clear();
	for (int i = 0, at = 0; i < nstrs; ++i) {
		int len = (unsigned char)strs[at] | ((unsigned char)strs[at + 1] << 8);
		g_strs.push_back(std::string(strs + at + 2, len));
		at += 2 + len;
	}
	g_xferRealToPortableHook = hookRealToPortable;
	g_xferPortableToRealHook = hookPortableToReal;
	/* the player-info map: a name is known if its hash says so */
	g_dgInfo.m_map.clear();
	for (size_t i = 0; i < g_strs.size(); ++i) {
		unsigned h = hashBytes(g_strs[i].c_str(), g_dcfg[13]);
		if (h % 3 != 0) {
			DgPlayerInfo pi;
			pi.m_profileID = (Int)(h & 0xffff);
			char lb[16]; snprintf(lb, sizeof lb, "L%u", (h >> 3) & 7);
			pi.m_locale = AsciiString(lb);
			pi.m_rankPoints = (Int)((h >> 5) & 0xfff);
			pi.m_side = (Int)((h >> 9) & 3);
			g_dgInfo.m_map[AsciiString(g_strs[i].c_str())] = pi;
		}
	}
	TheNAT = NULL;
	TheNetwork = NULL;
	g_dgLan.m_my = NULL;
	TheLAN = &g_dgLan;
	g_dgGlobal.m_pendingFile.clear();
	g_dgGlobal.m_useFpsLimit = true;
	void *mem = calloc(1, sizeof(GameSpyStagingRoom));
	g_room = new (mem) GameSpyStagingRoom;
	TheGameSpyGame = g_room;
	void *mem2 = calloc(1, sizeof(LANGameInfo));
	g_lan = new (mem2) LANGameInfo;
	logf("made");
	dumpRoom("room");
	if (g_lan) dumpLan("lan");
	for (int i = 0; i < n; ++i)
		dgi_run_op(ops + 6 * i);
	if (TheNAT) { delete TheNAT; TheNAT = NULL; }
	if (TheNetwork) { delete TheNetwork; TheNetwork = NULL; }
	g_xferRealToPortableHook = NULL;
	g_xferPortableToRealHook = NULL;
	g_room->~GameSpyStagingRoom(); free(mem);
	if (g_lan) { g_lan->~LANGameInfo(); }
	free(mem2);
	g_room = NULL; g_lan = NULL;
	TheGameSpyGame = NULL;
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}
