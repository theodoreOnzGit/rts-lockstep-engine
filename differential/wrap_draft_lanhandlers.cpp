/* Differential-test wrapper for src/drafts/network/lan_api_handlers.rs (#215, group lan5, upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANAPIhandlers.cpp   (the whole file after its includes: all 16 handlers, cut verbatim by build.rs)
** on the REAL game-info classes, exactly as the LAN game-info draft's wrapper (wrap_draft_gameinfo.cpp) builds them: wrap_game_info.cpp is #included whole (the
** real GameInfo.cpp on its shims), and then
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANGameInfo.cpp      (cut verbatim: LANGameSlot and LANGameInfo members, GenerateGameOptionsString, ParseGameOptionsString)
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANAPI.cpp           (cut verbatim: sendMessage, RequestGameJoin, RequestGameOptions, fillInLANMessage,
**                                                                       LookupGame, LookupGameByListOffset, removeGame, LookupPlayer, removePlayer, addGame,
**                                                                       addPlayer, AmIHost)
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANAPI.cpp's lobbyPort, LANAPI.h's constants, `struct LANMessage` (one asserted edit: renamed LANMessagePod,
**                                                                       the wrapper's LANMessage derives from it and zero-fills it; upstream's `LANMessage reply;`
**                                                                       is uninitialised stack, the draft's message is zeroed), NetworkDefs.h's NETWORK_BASE_PORT_NUMBER
**   GeneralsMD/Code/GameEngine/Source/Common/System/QuotedPrintable.cpp (cut verbatim: UnicodeStringToQuotedPrintable and its two digit helpers)
**   GeneralsMD/Code/GameEngine/Include/GameNetwork/LANGameInfo.h, LANPlayer.h (unchanged)
** Everything is a separate set of symbols from the other game-info wrappers (macros below rename every class and global this file defines).
**
** Stand-ins (what the cuts call outside themselves), every one logged into the text the test compares:
**   LANAPI (the class: its data members and the declarations of the cut members; GetMyGame, GetLocalIP; the On* callbacks; RequestHasMap and ResetGameStartTimer,
**           which are LANAPI.cpp functions compared by tests/differential_draft_lanapi.rs and here only logged)
**   Transport (queueSend logs the datagram's address, port, length and a hash of its bytes, `update` does nothing observable: it is a socket write)
**   TheGlobalData (m_exeCRC, m_iniCRC), GetStringFromRegistry (a settable serial), TheNetwork (non-null with isPacketRouter true exactly when the configuration says so),
**   LANPreferences (operator[] and write(): write logs the key and value; the real one reads and writes Options.ini), timeGetTime (a clock that steps by 11 per call),
**   LANEnableStartButton, lanUpdateSlotList, updateGameOptions (GUI calls: logged),
**   the map cache, TheGameState's portable map path and the multiplayer settings: wrap_game_info.cpp's (the verified GameInfo test's).
** Entry point (extern "C", tests/differential_draft_lanhandlers.rs): ref_lh_run(in, n, out, cap), the operations are described at lh_run. */
#include "wrap_game_info.cpp"

#include <map>
#include <ctype.h>
#include <netinet/in.h>

#undef TheGameSpyGame
#undef TheLAN
#undef TheGlobalData
#define LANAPI LhLANAPI
#define LANAPIInterface LhLANAPIInterface
#define LANGameInfo LhLANGameInfo
#define LANGameSlot LhLANGameSlot
#define LANPlayer LhLANPlayer
#define LANMessage LhLANMessage
#define LANMessagePod LhLANMessagePod
#define TheLAN LhTheLAN
#define TheGlobalData LhTheGlobalData
#define TheNetwork LhTheNetwork
#define GenerateGameOptionsString LhGenerateGameOptionsString
#define ParseGameOptionsString LhParseGameOptionsString
#define LANEnableStartButton LhLANEnableStartButton
#define lanUpdateSlotList LhLanUpdateSlotList
#define updateGameOptions LhUpdateGameOptions
#define timeGetTime LhTimeGetTime
#define GetStringFromRegistry LhGetStringFromRegistry
#define LANPreferences LhLANPreferences
#define UnicodeStringToQuotedPrintable LhUnicodeStringToQuotedPrintable
#define intToHexDigit LhIntToHexDigit
#define hexDigitToInt LhHexDigitToInt
#define lobbyPort LhLobbyPort
#define CRC LhCRC
#define CRC32 LhCRC32
/* the C library's wcsncpy is over 32-bit wchar_t; the engine's WideChar is 16 bits (a C-runtime stand-in, never upstream code) */
static inline WideChar *lh_wcsncpy(WideChar *dst, const WideChar *src, size_t n)
{
	size_t i = 0;
	for (; i < n && src[i] != 0; ++i) dst[i] = src[i];
	for (; i < n; ++i) dst[i] = 0;
	return dst;
}
#define wcsncpy lh_wcsncpy

/* crc.h's release CRC is an MSVC `_asm` block that g++ cannot compile; _DEBUG (for these lines only) selects its C++ class, the same loop */
#define _DEBUG 1
#include "Common/crc.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/crc.cpp"
#undef _DEBUG

class Transport;
class LhLANAPI;
#define protected public
#define private public
#include "../upstream/GeneralsMD/Code/GameEngine/Include/GameNetwork/LANGameInfo.h"
#undef protected
#undef private

namespace {
unsigned g_lhTime;
}
static UnsignedInt LhTimeGetTime() { g_lhTime += 11; return g_lhTime; }

#include "lh_consts.inc"
class LhLANAPIInterface
{
public:
#include "lh_chat.inc"
#include "lh_ret.inc"
};
#pragma pack(push, 1)
#include "lh_msg.inc"
struct LhLANMessage : public LhLANMessagePod
{
	LhLANMessage() { memset((void *)this, 0, sizeof(LhLANMessagePod)); }
};
#pragma pack(pop)

namespace {
std::string g_sendLog;
void sendf(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
void sendf(const char *fmt, ...)
{
	static char buf[4096];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	g_sendLog += buf;
	g_sendLog += '\n';
}
unsigned hashBytesLh(const unsigned char *p, unsigned n) { unsigned h = 2166136261u; for (unsigned i = 0; i < n; ++i) h = (h ^ p[i]) * 16777619u; return h; }
}

struct LhTransport
{
	void queueSend(UnsignedInt addr, UnsignedShort port, unsigned char *data, Int len)
	{
		sendf("send ip %u port %u len %d type %d hash %u", (unsigned)addr, (unsigned)port, (int)len, (int)((LhLANMessagePod *)data)->LANMessageType, hashBytesLh(data, (unsigned)len));
	}
};

struct LhGlobalData { UnsignedInt m_exeCRC; UnsignedInt m_iniCRC; };
static LhGlobalData g_lhGlobalData;
static LhGlobalData *LhTheGlobalData = &g_lhGlobalData;
static std::string g_lhSerial;
static Bool LhGetStringFromRegistry(AsciiString path, AsciiString key, AsciiString &val) { val.set(g_lhSerial.c_str()); return TRUE; }
struct LhNetworkInterface { Bool isPacketRouter() { return TRUE; } };
static LhNetworkInterface g_lhNet;
static LhNetworkInterface *LhTheNetwork = NULL;

/* LANPreferences (a UserPreferences over Options.ini): operator[] gives the value of a key and write() saves the file; here write() logs the keys */
class LhLANPreferences
{
public:
	AsciiString &operator[](const char *k) { return m_map[std::string(k)]; }
	void write()
	{
		for (std::map<std::string, AsciiString>::iterator it = m_map.begin(); it != m_map.end(); ++it)
			logf("prefs write %s [%s]", it->first.c_str(), it->second.str());
	}
	std::map<std::string, AsciiString> m_map;
};

/* UnicodeString's operator< (UnicodeString.h: `wcscmp(s1.str(), s2.str()) < 0`), which the shim UnicodeString lacks and ParseGameOptionsString's
** std::map<UnicodeString, UnicodeString> needs; wcscmp compares 16-bit unsigned units */
inline Bool operator<(const UnicodeString &s1, const UnicodeString &s2)
{
	const WideChar *a = s1.str(), *b = s2.str();
	while (*a && *a == *b) { ++a; ++b; }
	return (unsigned)*a < (unsigned)*b;
}

class LhLANAPI : public LhLANAPIInterface
{
public:
#include "lh_pending.inc"
	LhLANAPI() { m_lobbyPlayers = NULL; m_games = NULL; m_currentGame = NULL; m_transport = &m_transportObj; m_broadcastAddr = INADDR_BROADCAST; m_inLobby = TRUE; m_pendingAction = ACT_NONE; }
	/* the requests (LANAPI.cpp: RequestGameJoin and RequestGameOptions are cut below; the other two are logged here) */
	void RequestGameJoin( LhLANGameInfo *game, UnsignedInt ip = 0 );
	void RequestGameOptions( AsciiString gameOptions, Bool isPublic, UnsignedInt ip = 0 );
	void RequestHasMap( void ) { logf("lan requestHasMap"); }
	void ResetGameStartTimer( void ) { logf("lan resetGameStartTimer"); }
	LhLANGameInfo * LookupGame( UnicodeString gameName );
	LhLANGameInfo * LookupGameByListOffset( Int offset );
	LhLANPlayer * LookupPlayer( UnsignedInt playerIP );
	Bool AmIHost( void );
	LhLANGameInfo *GetMyGame( void ) { return m_currentGame; }
	UnsignedInt GetLocalIP( void ) { return m_localIP; }
	void fillInLANMessage( LhLANMessage *msg );
	void sendMessage(LhLANMessage *msg, UnsignedInt ip = 0);
	void removePlayer(LhLANPlayer *player);
	void removeGame(LhLANGameInfo *game);
	void addPlayer(LhLANPlayer *player);
	void addGame(LhLANGameInfo *game);

	/* the callbacks (LANAPICallbacks.cpp is the GUI): they log what they are told */
	void OnGameList( LhLANGameInfo *gameList );
	void OnPlayerList( LhLANPlayer *playerList );
	void OnGameJoin( ReturnType ret, LhLANGameInfo *theGame );
	void OnPlayerJoin( Int slot, UnicodeString playerName );
	void OnHostLeave( void ) { logf("cb hostleave"); }
	void OnPlayerLeave( UnicodeString player );
	void OnAccept( UnsignedInt playerIP, Bool status ) { logf("cb accept %u %d", (unsigned)playerIP, (int)status); }
	void OnHasMap( UnsignedInt playerIP, Bool status ) { logf("cb hasmap %u %d", (unsigned)playerIP, (int)status); }
	void OnChat( UnicodeString player, UnsignedInt ip, UnicodeString message, ChatType format );
	void OnGameStart( void ) { logf("cb gamestart"); }
	void OnGameStartTimer( Int seconds ) { logf("cb gamestarttimer %d", (int)seconds); }
	void OnGameOptions( UnsignedInt playerIP, Int playerSlot, AsciiString options ) { logf("cb gameoptions %u %d [%s]", (unsigned)playerIP, (int)playerSlot, options.str()); }
	void OnNameChange( UnsignedInt IP, UnicodeString newName );

	void handleRequestLocations( LhLANMessage *msg, UnsignedInt senderIP );
	void handleGameAnnounce( LhLANMessage *msg, UnsignedInt senderIP );
	void handleLobbyAnnounce( LhLANMessage *msg, UnsignedInt senderIP );
	void handleRequestGameInfo( LhLANMessage *msg, UnsignedInt senderIP );
	void handleRequestJoin( LhLANMessage *msg, UnsignedInt senderIP );
	void handleJoinAccept( LhLANMessage *msg, UnsignedInt senderIP );
	void handleJoinDeny( LhLANMessage *msg, UnsignedInt senderIP );
	void handleRequestGameLeave( LhLANMessage *msg, UnsignedInt senderIP );
	void handleRequestLobbyLeave( LhLANMessage *msg, UnsignedInt senderIP );
	void handleSetAccept( LhLANMessage *msg, UnsignedInt senderIP );
	void handleHasMap( LhLANMessage *msg, UnsignedInt senderIP );
	void handleChat( LhLANMessage *msg, UnsignedInt senderIP );
	void handleGameStart( LhLANMessage *msg, UnsignedInt senderIP );
	void handleGameStartTimer( LhLANMessage *msg, UnsignedInt senderIP );
	void handleGameOptions( LhLANMessage *msg, UnsignedInt senderIP );
	void handleInActive( LhLANMessage *msg, UnsignedInt senderIP );

	LhLANPlayer *m_lobbyPlayers;
	LhLANGameInfo *m_games;
	UnicodeString m_name;
	AsciiString m_userName;
	AsciiString m_hostName;
	UnsignedInt m_gameStartTime;
	Int m_gameStartSeconds;
	PendingActionType m_pendingAction;
	UnsignedInt m_expiration;
	UnsignedInt m_actionTimeout;
	UnsignedInt m_directConnectRemoteIP;
	UnsignedInt m_lastResendTime;
	Bool m_inLobby;
	LhLANGameInfo *m_currentGame;
	UnsignedInt m_localIP;
	LhTransport *m_transport;
	LhTransport m_transportObj;
	UnsignedInt m_broadcastAddr;
	AsciiString m_lastGameopt;
};
LhLANAPI *LhTheLAN = NULL;

void LhLANEnableStartButton(Bool e) { logf("lan enableStartButton %d", (int)e); }
void LhLanUpdateSlotList() { logf("lan updateSlotList"); }
void LhUpdateGameOptions() { logf("lan updateGameOptions"); }

/* ---- the cuts, verbatim (build.rs: draft_lan5_gen) ---------------------------------------------- */
#include "lh_qp.inc"
#include "lh_lan_slots.inc"
#include "lh_lan_options.inc"
#include "lh_head.inc"
#include "lh_send.inc"
#include "lh_join.inc"
#include "lh_options.inc"
#include "lh_fill.inc"
#include "lh_lists.inc"
#include "lh_host.inc"
#include "lh_handlers.inc"

namespace {
std::string wideLh(const UnicodeString &u)
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
}

void LhLANAPI::OnGameList( LhLANGameInfo *gameList )
{
	std::string s;
	int n = 0;
	for (LhLANGameInfo *g = gameList; g; g = g->getNext(), ++n) { s += wideLh(g->getName()); s += '|'; }
	logf("cb gamelist %d [%s]", n, s.c_str());
}
void LhLANAPI::OnPlayerList( LhLANPlayer *playerList )
{
	std::string s;
	int n = 0;
	for (LhLANPlayer *p = playerList; p; p = p->getNext(), ++n) { char b[16]; snprintf(b, sizeof b, "@%u|", (unsigned)p->getIP()); s += wideLh(p->getName()); s += b; }
	logf("cb playerlist %d [%s]", n, s.c_str());
}
void LhLANAPI::OnGameJoin( ReturnType ret, LhLANGameInfo *theGame )
{
	logf("cb gamejoin %d [%s]", (int)ret, theGame ? wideLh(theGame->getName()).c_str() : "-");
}
void LhLANAPI::OnPlayerJoin( Int slot, UnicodeString playerName ) { logf("cb playerjoin %d [%s]", (int)slot, wideLh(playerName).c_str()); }
void LhLANAPI::OnPlayerLeave( UnicodeString player ) { logf("cb playerleave [%s]", wideLh(player).c_str()); }
void LhLANAPI::OnChat( UnicodeString player, UnsignedInt ip, UnicodeString message, ChatType format )
{
	logf("cb chat [%s] %u [%s] %d", wideLh(player).c_str(), (unsigned)ip, wideLh(message).c_str(), (int)format);
}
void LhLANAPI::OnNameChange( UnsignedInt IP, UnicodeString newName ) { logf("cb namechange %u [%s]", (unsigned)IP, wideLh(newName).c_str()); }

namespace {

struct Rd
{
	const unsigned *w; int n; int i;
	unsigned next() { return i < n ? w[i++] : 0; }
	std::string bytes()
	{
		unsigned len = next();
		std::string s;
		for (unsigned k = 0; k < len; k += 4) {
			unsigned v = next();
			for (unsigned j = 0; j < 4 && k + j < len; ++j) s += (char)(v >> (8 * j));
		}
		return s;
	}
	UnicodeString units()
	{
		unsigned len = next();
		std::vector<WideChar> v;
		for (unsigned k = 0; k < len; k += 2) {
			unsigned w2 = next();
			v.push_back((WideChar)(w2 & 0xffff));
			if (k + 1 < len) v.push_back((WideChar)(w2 >> 16));
		}
		v.push_back(0);
		UnicodeString u;
		u.set(&v[0]);
		return u;
	}
};

AsciiString ascLh(const std::string &s) { AsciiString a; a.set(s.c_str()); return a; }

void dumpLh(LhLANAPI &a)
{
	int cur = -1;
	if (a.m_currentGame) { cur = -2; int k = 0; for (LhLANGameInfo *g = a.m_games; g; g = g->getNext(), ++k) if (g == a.m_currentGame) cur = k; }
	logf("state: lobby %d cur %d pend %d exp %u gst %u gss %d lrt %u ip %u dcip %u name [%s] opt [%s] clock %u", (int)a.m_inLobby, cur, (int)a.m_pendingAction,
		(unsigned)a.m_expiration, (unsigned)a.m_gameStartTime, (int)a.m_gameStartSeconds, (unsigned)a.m_lastResendTime, (unsigned)a.m_localIP,
		(unsigned)a.m_directConnectRemoteIP, wideLh(a.m_name).c_str(), a.m_lastGameopt.str(), g_lhTime);
	int ng = 0;
	for (LhLANGameInfo *g = a.m_games; g; g = g->getNext()) ++ng;
	logf("games %d", ng);
	int gi = 0;
	for (LhLANGameInfo *g = a.m_games; g; g = g->getNext(), ++gi) {
		logf("game %d: in %d prog %d ip %u map [%s] crc %u seed %d last %u name [%s] direct %d", gi, (int)g->m_inGame, (int)g->m_inProgress, (unsigned)g->m_localIP,
			g->m_mapName.str(), (unsigned)g->m_mapCRC, (int)g->m_seed, (unsigned)g->m_lastHeard, wideLh(g->m_gameName).c_str(), (int)g->m_isDirectConnect);
		for (int i = 0; i < MAX_SLOTS; ++i) {
			LhLANGameSlot &s = g->m_LANSlot[i];
			logf(" slot %d: st %d acc %d map %d name [%s] ip %u port %u tmpl %d team %d col %d sp %d last %u serial [%s] login [%s] host [%s]", i,
				(int)s.m_state, (int)s.m_isAccepted, (int)s.m_hasMap, wideLh(s.m_name).c_str(), (unsigned)s.m_IP, (unsigned)s.m_port, s.m_playerTemplate,
				s.m_teamNumber, s.m_color, s.m_startPos, (unsigned)s.m_lastHeard, s.m_serial.str(), wideLh(s.m_user.m_login).c_str(), wideLh(s.m_user.m_host).c_str());
		}
	}
	int np = 0;
	for (LhLANPlayer *p = a.m_lobbyPlayers; p; p = p->getNext()) ++np;
	logf("players %d", np);
	for (LhLANPlayer *p = a.m_lobbyPlayers; p; p = p->getNext())
		logf(" player [%s] login [%s] host [%s] last %u ip %u", wideLh(p->getName()).c_str(), wideLh(p->getLogin()).c_str(), wideLh(p->getHost()).c_str(), (unsigned)p->getLastHeard(), (unsigned)p->getIP());
}

LhLANGameInfo *nthG(LhLANAPI &a, int k) { LhLANGameInfo *g = a.m_games; while (g && k-- > 0) g = g->getNext(); return g; }
LhLANPlayer *nthP(LhLANAPI &a, int k) { LhLANPlayer *p = a.m_lobbyPlayers; while (p && k-- > 0) p = p->getNext(); return p; }

}

/* in: [clock, localIP, name units, userName bytes, hostName bytes, exeCRC, iniCRC, serial bytes, packetRouter, actionTimeout, broadcast, ops...];
**  1 addGame(name units, direct, inProgress, inGame, map bytes, seed, 8 x (state, name units, ip, serial bytes, login units, host units, lastHeard, flags))
**        flags: bit 0 accepted, bit 1 has the map, bits 4-6 the player template + 2 (so -2 is the observer)
**  2 addPlayer(name units, ip, login units, host units, lastHeard)   3 removeGame(i)   4 removePlayer(i)   8 set the current game (i, -1 none)   9 set m_inLobby
**  10 set m_pendingAction   11 set m_localIP   12 set m_name(units)   13 advance the clock(dt)   14 set m_directConnectRemoteIP   15 set m_expiration
**  16 set m_gameStartTime   17 set the packet router flag   18 set the user name(bytes)   19 set the host name(bytes)   20 set the serial(bytes)
**  40 a handler: (which 0..15 in the order of LANAPI.h, sender ip, the LANMessage's bytes) */
extern "C" int ref_lh_run(const unsigned *in, int nin, char *out, int cap)
{
	g_log.clear();
	g_sendLog.clear();
	{ static const unsigned gi[9] = { 3, 8, 4, 0, 4, 0x1234, 2, 0x77, 12345 }; ref_ginfo_configure(gi); }
	Rd r = { in, nin, 0 };
	g_lhTime = r.next();
	LhLANAPI *ap = new LhLANAPI();
	LhLANAPI &a = *ap;
	LhTheLAN = &a;
	a.m_localIP = r.next();
	a.m_name = r.units(); a.m_userName = ascLh(r.bytes()); a.m_hostName = ascLh(r.bytes());
	g_lhGlobalData.m_exeCRC = r.next(); g_lhGlobalData.m_iniCRC = r.next(); g_lhSerial = r.bytes();
	LhTheNetwork = (r.next() != 0) ? &g_lhNet : NULL;
	a.m_actionTimeout = r.next(); a.m_broadcastAddr = r.next();
	a.m_expiration = 0; a.m_gameStartTime = 0; a.m_gameStartSeconds = 0; a.m_lastResendTime = 0; a.m_directConnectRemoteIP = 0;
	g_xferRealToPortableHook = hookRealToPortable;
	g_xferPortableToRealHook = hookPortableToReal;
	dumpLh(a);
	while (r.i < r.n) {
		unsigned op = r.next();
		logf("op %u", op);
		size_t mark = g_log.size();
		switch (op) {
		case 1: {
			LhLANGameInfo *g = NEW LhLANGameInfo;
			g->setName(r.units());
			g->setIsDirectConnect(r.next() != 0);
			g->m_inProgress = r.next() != 0;
			g->m_inGame = r.next() != 0;
			g->m_mapName = ascLh(r.bytes());
			g->m_seed = (Int)r.next();
			for (int i = 0; i < MAX_SLOTS; ++i) {
				LhLANGameSlot s;
				unsigned st = r.next();
				UnicodeString nm = r.units();
				UnsignedInt ipv = r.next();
				std::string serial = r.bytes();
				UnicodeString login = r.units();
				UnicodeString host = r.units();
				unsigned lh = r.next();
				unsigned flags = r.next();
				s.setState((SlotState)(Int)(st % 7), nm, ipv);
				s.setLogin(login);
				s.setHost(host);
				s.setSerial(ascLh(serial));
				s.setLastHeard(lh);
				s.setPlayerTemplate((Int)((flags >> 4) & 7) - 2);
				if (flags & 1) s.setAccept(); else s.unAccept();
				s.setMapAvailability((flags & 2) != 0);
				g->setSlot(i, s);
			}
			a.addGame(g);
			break; }
		case 2: {
			LhLANPlayer *p = NEW LhLANPlayer;
			p->setName(r.units()); p->setIP(r.next()); p->setLogin(r.units()); p->setHost(r.units()); p->setLastHeard(r.next());
			a.addPlayer(p);
			break; }
		case 3: { LhLANGameInfo *g = nthG(a, (int)r.next()); a.removeGame(g); if (g && g == a.m_currentGame) a.m_currentGame = NULL; break; } /* the node leaks, as in the LANAPI test */
		case 4: { LhLANPlayer *p = nthP(a, (int)r.next()); a.removePlayer(p); break; }
		case 8: { int k = (int)r.next(); a.m_currentGame = k < 0 ? NULL : nthG(a, k); break; }
		case 9: a.m_inLobby = r.next() != 0; break;
		case 10: a.m_pendingAction = (LhLANAPI::PendingActionType)r.next(); break;
		case 11: a.m_localIP = r.next(); break;
		case 12: a.m_name = r.units(); break;
		case 13: g_lhTime += r.next(); break;
		case 14: a.m_directConnectRemoteIP = r.next(); break;
		case 15: a.m_expiration = r.next(); break;
		case 16: a.m_gameStartTime = r.next(); break;
		case 17: LhTheNetwork = (r.next() != 0) ? &g_lhNet : NULL; break;
		case 18: a.m_userName = ascLh(r.bytes()); break;
		case 19: a.m_hostName = ascLh(r.bytes()); break;
		case 20: g_lhSerial = r.bytes(); break;
		case 40: {
			unsigned which = r.next();
			UnsignedInt sender = r.next();
			LhLANMessage m;
			unsigned char *p = (unsigned char *)&m;
			for (unsigned k = 0; k < sizeof(LhLANMessagePod); k += 4) {
				unsigned v = r.next();
				for (unsigned j = 0; j < 4 && k + j < sizeof(LhLANMessagePod); ++j) p[k + j] = (unsigned char)(v >> (8 * j));
			}
			/* a game announcement of the game I am in (not yet started) parses over my own slots, which changes AmIHost() in the middle of the handler: the draft's host
			** trait only holds a snapshot of it, so that input is not run (by both sides); see the test */
			if (which == 1 && a.m_currentGame && !a.m_currentGame->isGameInProgress() && sender != a.m_localIP && sender != a.m_directConnectRemoteIP
				&& a.LookupGame(UnicodeString(m.GameInfo.gameName)) == a.m_currentGame) { logf("skipped"); break; }
			switch (which) {
			case 0: a.handleRequestLocations(&m, sender); break;
			case 1: a.handleGameAnnounce(&m, sender); break;
			case 2: a.handleLobbyAnnounce(&m, sender); break;
			case 3: a.handleRequestGameInfo(&m, sender); break;
			case 4: a.handleRequestJoin(&m, sender); break;
			case 5: a.handleJoinAccept(&m, sender); break;
			case 6: a.handleJoinDeny(&m, sender); break;
			case 7: a.handleRequestGameLeave(&m, sender); break;
			case 8: a.handleRequestLobbyLeave(&m, sender); break;
			case 9: a.handleSetAccept(&m, sender); break;
			case 10: a.handleHasMap(&m, sender); break;
			case 11: a.handleChat(&m, sender); break;
			case 12: a.handleGameStart(&m, sender); break;
			case 13: a.handleGameStartTimer(&m, sender); break;
			case 14: a.handleGameOptions(&m, sender); break;
			default: a.handleInActive(&m, sender); break;
			}
			break; }
		default: break;
		}
		/* the datagrams first, then the callbacks (the draft queues the one and calls the other) */
		std::string ev = g_log.substr(mark);
		g_log.resize(mark);
		g_log += g_sendLog;
		g_log += ev;
		g_sendLog.clear();
		dumpLh(a);
	}
	g_xferRealToPortableHook = NULL;
	g_xferPortableToRealHook = NULL;
	LhTheLAN = NULL;
	delete ap;
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}
