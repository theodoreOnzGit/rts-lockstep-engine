/* Differential-test wrapper for the LAN API draft (#215, group network3, upstream commit 0a05454d), src/drafts/network/lan_api.rs: the LANMessage
** layout, GetMessageTypeString, sendMessage, fillInLANMessage, the request functions, the game and player lists, AmIHost and setIsActive.
** Cut verbatim by build.rs (drafts_network3) into OUT_DIR/draft_network3_gen:
**   la_net.inc        NetworkDefs.h, `enum ConnectionNumbers` to the packet-size comment (MAX_PLAYER, MAX_SLOTS), and MAX_PACKET_SIZE;
**   la_consts.inc     LANAPI.h, the name-length constants to `struct LANMessage;`;
**   la_chat.inc, la_ret.inc   LANAPI.h, `enum ChatType` and `enum ReturnType` (inside the stand-in LANAPIInterface);
**   la_pending.inc    LANAPI.h, `enum PendingActionType`;
**   la_msg.inc        LANAPI.h, `struct LANMessage` to `#pragma pack(pop)`; ONE EDIT (asserted once): the struct is named LANMessagePod, and the
**                     wrapper's LANMessage derives from it and zero-fills it (upstream's `LANMessage msg;` is uninitialised stack, so the bytes
**                     past the written fields are stack garbage on the wire; the draft's are 0);
**   la_head.inc, la_head2.inc   LANAPI.cpp, `lobbyPort` and `s_resendDelta`;
**   la_send.inc       LANAPI.cpp, sendMessage and GetMessageTypeString;
**   la_requests.inc  LANAPI.cpp, RequestLocations to RequestGameOptions (all the requests but RequestGameCreate);
**   la_misc.inc       LANAPI.cpp, RequestSetName, fillInLANMessage, RequestLobbyLeave, the Lookup*, remove* and add* members;
**   la_ishost.inc     LANAPI.cpp, AmIHost and setIsActive;
**   and the REAL LANPlayer.h (self-contained).
** Built on upstream's own UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; classes renamed by build.rs; shim_draft_n3/n3_prelude.h adds
** `%hs` to the wide printf stand-in and a 16-bit `wcsncpy`) and crc.h/crc.cpp (unchanged, _DEBUG for those lines only).
** Stand-ins (outside the cuts), logging into theLog as tests/differential_draft_lanapi.rs's host does:
**   LANAPI (the class: the members the cuts use and the On* callbacks, which log), LANAPIInterface (the two enums), LANGameInfo and its slots (name, next, per slot
**   human flag, ip and hasMap, the direct-connect and in-progress flags, the map; getLocalSlotNum is `isHuman() && TheLAN->GetLocalIP() == getIP()`
**   as LANGameSlot::isLocalPlayer reads), Transport (queueSend logs the datagram, update does nothing observable), timeGetTime (settable),
**   TheGlobalData->m_exeCRC and m_iniCRC, GetStringFromRegistry (a settable serial), TheGameState (a "P:" prefix for the portable map name; the leaf name is the
**   name itself), WouldMapTransfer (the parity of a hash of the map), TheMapCache->findMap (NULL), TheGameText->fetch (two fixed formats), TheNetwork->isPacketRouter
**   (settable), GameInfoToAsciiString (a fixed string: RequestGameAnnounce is NOT compared, the draft uses the verified one).
** Entry points (extern "C", tests/differential_draft_lanapi.rs): ref_la_run (programs, see there) and ref_la_msg (the message layout). */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <vector>
#include <map>
#include <string>
#include <netinet/in.h>

namespace {
DynamicMemoryAllocator theAllocator;
}
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theAllocator;
CriticalSection *TheUnicodeStringCriticalSection = 0;

#define NULL 0
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define DEBUG_CRASH(m) ((void)0)
#define DEBUG_LOG(m) ((void)0)
typedef Int Color;

/* crc.h's release CRC is an MSVC `_asm` block that g++ cannot compile; _DEBUG (for these lines only) selects its C++ class, the same loop */
#define _DEBUG 1
#include "Common/crc.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/crc.cpp"
#undef _DEBUG

static std::vector<unsigned> theLog;
static unsigned fnv(unsigned h, unsigned char c) { return (h ^ c) * 16777619u; }
static unsigned hashBytes(const unsigned char *p, unsigned n) { unsigned h = 2166136261u; for (unsigned i = 0; i < n; ++i) h = fnv(h, p[i]); return h; }
static unsigned hashUnits(const WideChar *s) { unsigned h = 2166136261u; for (; *s; ++s) { h = fnv(h, (unsigned char)(*s & 255)); h = fnv(h, (unsigned char)(*s >> 8)); } return h; }
static unsigned hashStr(const char *s) { return hashBytes((const unsigned char *)s, (unsigned)strlen(s)); }
static unsigned g_now;
static unsigned timeGetTime() { return g_now; }

#include "la_net.inc"
#include "la_consts.inc"
#include "GameNetwork/LANPlayer.h"

class LANAPIInterface
{
public:
#include "la_chat.inc"
#include "la_ret.inc"
};

#pragma pack(push, 1)
#include "la_msg.inc"
struct LANMessage : public LANMessagePod
{
	LANMessage() { memset((void *)this, 0, sizeof(LANMessagePod)); }
};
#pragma pack(pop)

/* ---- the world the cuts ask of ---- */
struct GameSlot { Bool m_human; UnsignedInt m_ip; Bool m_hasMap; Bool isHuman() const { return m_human; } UnsignedInt getIP() const { return m_ip; } Bool hasMap() const { return m_hasMap; } };
static UnsignedInt g_localIP;
class LANGameInfo
{
public:
	LANGameInfo() { m_next = NULL; m_direct = FALSE; m_inProgress = FALSE; for (int i = 0; i < MAX_SLOTS; ++i) { m_slots[i].m_human = FALSE; m_slots[i].m_ip = 0; m_slots[i].m_hasMap = FALSE; } }
	UnicodeString getName() { return m_name; }
	LANGameInfo *getNext() { return m_next; }
	void setNext(LANGameInfo *n) { m_next = n; }
	UnsignedInt getIP(Int i) { return m_slots[i].m_ip; }
	Bool getIsDirectConnect() { return m_direct; }
	Bool isGameInProgress() { return m_inProgress; }
	AsciiString getMap() { return m_map; }
	GameSlot *getSlot(Int i) { static GameSlot none = { FALSE, 0, FALSE }; return (i < 0) ? &none : &m_slots[i]; }   /* slot -1: upstream indexes before the array (undefined); no map here, as the draft says */
	Int getLocalSlotNum()
	{
		for (Int i = 0; i < MAX_SLOTS; ++i)
			if (m_slots[i].isHuman() && g_localIP == m_slots[i].getIP()) return i;
		return -1;
	}
	UnicodeString m_name; LANGameInfo *m_next; Bool m_direct; Bool m_inProgress; AsciiString m_map; GameSlot m_slots[MAX_SLOTS];
};

struct TransportStandIn
{
	void queueSend(UnsignedInt addr, UnsignedShort port, unsigned char *data, Int len)
	{
		theLog.push_back(90); theLog.push_back(addr); theLog.push_back(port); theLog.push_back((unsigned)len); theLog.push_back(hashBytes(data, (unsigned)len));
	}
	void update() {}
};

struct GlobalDataStandIn { UnsignedInt m_exeCRC; UnsignedInt m_iniCRC; };
static GlobalDataStandIn g_global;
static GlobalDataStandIn *TheGlobalData = &g_global;
static std::string g_serial;
static Bool GetStringFromRegistry(AsciiString path, AsciiString key, AsciiString &val) { val.set(g_serial.c_str()); return TRUE; }
struct GameStateStandIn
{
	AsciiString realMapPathToPortableMapPath(const AsciiString &in) const { AsciiString r; r.set((std::string("P:") + in.str()).c_str()); return r; }
	AsciiString getMapLeafName(const AsciiString &in) const { return in; }
};
static GameStateStandIn theStateObj;
static GameStateStandIn *TheGameState = &theStateObj;
static Bool WouldMapTransfer(AsciiString map) { return (hashStr(map.str()) & 1u) != 0; }
struct MapMetaData { UnicodeString m_displayName; Bool m_isOfficial; };
struct MapCacheStandIn { const MapMetaData *findMap(AsciiString) { return NULL; } };
static MapCacheStandIn theMapCacheObj;
static MapCacheStandIn *TheMapCache = &theMapCacheObj;
struct GameTextStandIn
{
	UnicodeString fetch(const char *label)
	{
		UnicodeString u;
		if (strcmp(label, "GUI:LocalPlayerNoMapWillTransfer") == 0) u.set(L"NOMAPT:%ls"); else u.set(L"NOMAP:%ls");
		return u;
	}
};
static GameTextStandIn theTextObj;
static GameTextStandIn *TheGameText = &theTextObj;
static Bool g_packetRouter;
struct NetworkStandIn { Bool isPacketRouter() { return g_packetRouter; } };
static NetworkStandIn theNetObj;
static NetworkStandIn *TheNetwork = &theNetObj;
static AsciiString GameInfoToAsciiString(LANGameInfo *g) { AsciiString r; r.set("OPTS"); return r; }

class LANAPI : public LANAPIInterface
{
public:
#include "la_pending.inc"
	LANAPI() { m_lobbyPlayers = NULL; m_games = NULL; m_currentGame = NULL; m_transport = &m_transportObj; m_broadcastAddr = INADDR_BROADCAST; }
	void RequestLocations( void );
	void RequestGameJoin( LANGameInfo *game, UnsignedInt ip = 0 );
	void RequestGameJoinDirectConnect( UnsignedInt ipaddress );
	void RequestGameLeave( void );
	void RequestGameAnnounce( void );
	void RequestAccept( void );
	void RequestHasMap( void );
	void RequestChat( UnicodeString message, ChatType format );
	void RequestGameStart( void );
	void RequestGameStartTimer( Int seconds );
	void ResetGameStartTimer( void );
	void RequestGameOptions( AsciiString gameOptions, Bool isPublic, UnsignedInt ip = 0 );
	void RequestSetName( UnicodeString newName );
	void RequestLobbyLeave( Bool forced );
	LANGameInfo * LookupGame( UnicodeString gameName );
	LANGameInfo * LookupGameByListOffset( Int offset );
	LANPlayer * LookupPlayer( UnsignedInt playerIP );
	Bool AmIHost( void );
	UnsignedInt GetLocalIP( void ) { return m_localIP; }
	void fillInLANMessage( LANMessage *msg );
	void setIsActive(Bool isActive);
	void sendMessage(LANMessage *msg, UnsignedInt ip = 0);
	void removePlayer(LANPlayer *player);
	void removeGame(LANGameInfo *game);
	void addPlayer(LANPlayer *player);
	void addGame(LANGameInfo *game);
	/* the callbacks (LANAPICallbacks.cpp is the GUI): they log what they are told */
	void OnGameJoin( ReturnType ret, LANGameInfo *theGame ) { theLog.push_back(92); theLog.push_back((unsigned)ret); theLog.push_back(theGame ? 1u : 0u); }
	void OnPlayerLeave( UnicodeString player ) { theLog.push_back(93); theLog.push_back(hashUnits(player.str())); }
	void OnChat( UnicodeString player, UnsignedInt ip, UnicodeString message, ChatType format ) { theLog.push_back(94); theLog.push_back(hashUnits(player.str())); theLog.push_back(ip); theLog.push_back(hashUnits(message.str())); theLog.push_back((unsigned)format); }
	void OnGameStart( void ) { theLog.push_back(95); }
	void OnGameStartTimer( Int seconds ) { theLog.push_back(96); theLog.push_back((unsigned)seconds); }
	void OnGameOptions( UnsignedInt playerIP, Int playerSlot, AsciiString options ) { theLog.push_back(97); theLog.push_back(playerIP); theLog.push_back((unsigned)playerSlot); theLog.push_back(hashStr(options.str())); }
	void OnNameChange( UnsignedInt IP, UnicodeString newName ) { theLog.push_back(98); theLog.push_back(IP); theLog.push_back(hashUnits(newName.str())); }

	static const UnsignedInt s_resendDelta;
	LANPlayer *m_lobbyPlayers;
	LANGameInfo *m_games;
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
	LANGameInfo *m_currentGame;
	UnsignedInt m_localIP;
	TransportStandIn *m_transport;
	TransportStandIn m_transportObj;
	UnsignedInt m_broadcastAddr;
	AsciiString m_lastGameopt;
	Bool m_isActive;
};

#include "la_head.inc"
AsciiString GetMessageTypeString(UnsignedInt type);
#include "la_head2.inc"
#include "la_send.inc"
#include "la_requests.inc"
#include "la_misc.inc"
#include "la_ishost.inc"

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

void logStr(const char *s) { theLog.push_back((unsigned)strlen(s)); for (; *s; ++s) theLog.push_back((unsigned char)*s); }
void logUnits(const WideChar *s) { unsigned n = 0; while (s[n]) ++n; theLog.push_back(n); for (unsigned i = 0; i < n; ++i) theLog.push_back((unsigned)s[i]); }
AsciiString asc(const std::string &s) { AsciiString a; a.set(s.c_str()); return a; }

void dump(LANAPI &a)
{
	theLog.push_back(0xD0D0D0D0u);
	unsigned ng = 0; for (LANGameInfo *g = a.m_games; g; g = g->getNext()) ++ng;
	theLog.push_back(ng);
	for (LANGameInfo *g = a.m_games; g; g = g->getNext()) {
		logUnits(g->m_name.str()); theLog.push_back(g->m_direct ? 1u : 0u); theLog.push_back(g->m_inProgress ? 1u : 0u); logStr(g->m_map.str());
		for (int i = 0; i < MAX_SLOTS; ++i) { theLog.push_back(g->m_slots[i].m_human ? 1u : 0u); theLog.push_back(g->m_slots[i].m_ip); theLog.push_back(g->m_slots[i].m_hasMap ? 1u : 0u); }
	}
	unsigned np = 0; for (LANPlayer *p = a.m_lobbyPlayers; p; p = p->getNext()) ++np;
	theLog.push_back(np);
	for (LANPlayer *p = a.m_lobbyPlayers; p; p = p->getNext()) {
		logUnits(p->getName().str()); logUnits(p->getLogin().str()); logUnits(p->getHost().str()); theLog.push_back(p->getLastHeard()); theLog.push_back(p->getIP());
	}
	/* the index of the current game in the list, -1 for none, -2 for a game that is not in the list */
	int cur = -1;
	if (a.m_currentGame) { cur = -2; int k = 0; for (LANGameInfo *g = a.m_games; g; g = g->getNext(), ++k) if (g == a.m_currentGame) cur = k; }
	theLog.push_back((unsigned)cur);
	theLog.push_back(a.m_inLobby ? 1u : 0u); theLog.push_back((unsigned)a.m_pendingAction); theLog.push_back(a.m_expiration);
	theLog.push_back(a.m_gameStartTime); theLog.push_back((unsigned)a.m_gameStartSeconds); theLog.push_back(a.m_lastResendTime);
	logStr(a.m_lastGameopt.str()); theLog.push_back(a.m_localIP); theLog.push_back(a.m_directConnectRemoteIP); theLog.push_back(a.m_isActive ? 1u : 0u);
	logUnits(a.m_name.str());
}

LANGameInfo *nth(LANAPI &a, int k) { LANGameInfo *g = a.m_games; while (g && k-- > 0) g = g->getNext(); return g; }
LANPlayer *nthP(LANAPI &a, int k) { LANPlayer *p = a.m_lobbyPlayers; while (p && k-- > 0) p = p->getNext(); return p; }
int indexOf(LANAPI &a, LANGameInfo *g) { int k = 0; for (LANGameInfo *x = a.m_games; x; x = x->getNext(), ++k) if (x == g) return k; return -1; }
int indexOfP(LANAPI &a, LANPlayer *p) { int k = 0; for (LANPlayer *x = a.m_lobbyPlayers; x; x = x->getNext(), ++k) if (x == p) return k; return -1; }

} // namespace

extern "C" {

/* in: [clock, localIP, name units, userName bytes, hostName bytes, exeCRC, iniCRC, serial bytes, packetRouter, actionTimeout, broadcast, ops...];
**  1 addGame(name units, direct, inProgress, map bytes, 8 x (human, ip, hasMap))   2 addPlayer(name units, ip)   3 removeGame(i)   4 removePlayer(i)
**  5 LookupGame(name units)   6 LookupGameByListOffset(n)   7 LookupPlayer(ip)   8 set m_currentGame (i, -1 none)   9 set m_inLobby   10 set m_pendingAction
**  11 set m_localIP   12 set m_name(units)   13 advance the clock(dt)   14 set m_directConnectRemoteIP   15 set m_expiration
**  20 RequestLocations   21 RequestGameJoin(game index or -1, ip)   22 RequestGameJoinDirectConnect(ip)   23 RequestGameLeave   24 RequestAccept
**  25 RequestHasMap   26 RequestChat(units, format)   27 RequestGameStart   28 RequestGameStartTimer(n)   29 RequestGameOptions(bytes, isPublic, ip)
**  30 RequestSetName(units)   31 RequestLobbyLeave(forced)   32 ResetGameStartTimer   33 AmIHost   34 setIsActive(v)   35 fillInLANMessage (the message's hash)
**  36 GetMessageTypeString(type)   37 set the user name(bytes)   38 set the host name(bytes) */
int ref_la_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	theLog.clear();
	Rd r = { in, nin, 0 };
	LANAPI a;
	g_now = r.next(); g_localIP = a.m_localIP = r.next();
	a.m_name = r.units(); a.m_userName = asc(r.bytes()); a.m_hostName = asc(r.bytes());
	g_global.m_exeCRC = r.next(); g_global.m_iniCRC = r.next(); g_serial = r.bytes(); g_packetRouter = r.next() != 0; a.m_actionTimeout = r.next(); a.m_broadcastAddr = r.next();
	a.m_pendingAction = LANAPI::ACT_NONE; a.m_expiration = 0; a.m_inLobby = true; a.m_gameStartTime = 0; a.m_gameStartSeconds = 0; a.m_lastResendTime = 0;
	a.m_directConnectRemoteIP = 0; a.m_isActive = TRUE;
	dump(a);
	while (r.i < r.n) {
		unsigned op = r.next();
		theLog.push_back(0xA5A5A5A5u); theLog.push_back(op);
		switch (op) {
		case 1: {
			LANGameInfo *g = new LANGameInfo();
			g->m_name = r.units(); g->m_direct = r.next() != 0; g->m_inProgress = r.next() != 0; g->m_map = asc(r.bytes());
			for (int i = 0; i < MAX_SLOTS; ++i) { g->m_slots[i].m_human = r.next() != 0; g->m_slots[i].m_ip = r.next(); g->m_slots[i].m_hasMap = r.next() != 0; }
			a.addGame(g); break; }
		case 2: { LANPlayer *p = new LANPlayer(); p->setName(r.units()); p->setIP(r.next()); a.addPlayer(p); break; }
		case 3: { LANGameInfo *g = nth(a, (int)r.next()); a.removeGame(g); if (g && g == a.m_currentGame) a.m_currentGame = NULL; break; }   /* the node leaks; upstream's callers clear m_currentGame themselves, the draft's index is cleared by remove_game */
		case 4: { LANPlayer *p = nthP(a, (int)r.next()); a.removePlayer(p); break; }
		case 5: theLog.push_back((unsigned)indexOf(a, a.LookupGame(r.units()))); break;
		case 6: theLog.push_back((unsigned)indexOf(a, a.LookupGameByListOffset((Int)r.next()))); break;
		case 7: theLog.push_back((unsigned)indexOfP(a, a.LookupPlayer(r.next()))); break;
		case 8: { int k = (int)r.next(); a.m_currentGame = k < 0 ? NULL : nth(a, k); break; }
		case 9: a.m_inLobby = r.next() != 0; break;
		case 10: a.m_pendingAction = (LANAPI::PendingActionType)r.next(); break;
		case 11: g_localIP = a.m_localIP = r.next(); break;
		case 12: a.m_name = r.units(); break;
		case 13: g_now += r.next(); break;
		case 14: a.m_directConnectRemoteIP = r.next(); break;
		case 15: a.m_expiration = r.next(); break;
		case 20: a.RequestLocations(); break;
		case 21: { int k = (int)r.next(); a.RequestGameJoin(k < 0 ? NULL : nth(a, k), r.next()); break; }
		case 22: { UnsignedInt ipv = r.next(); if (a.m_name.getLength() > g_lanPlayerNameLength) break; /* upstream copies the whole name into playerName[13]: an overflow (#210), not compared */ a.RequestGameJoinDirectConnect(ipv); break; }
		case 23: a.RequestGameLeave(); break;
		case 24: a.RequestAccept(); break;
		case 25: a.RequestHasMap(); break;
		case 26: { UnicodeString m = r.units(); a.RequestChat(m, (LANAPI::ChatType)r.next()); break; }
		case 27: a.RequestGameStart(); break;
		case 28: a.RequestGameStartTimer((Int)r.next()); break;
		case 29: { AsciiString o = asc(r.bytes()); Bool pub = r.next() != 0; a.RequestGameOptions(o, pub, r.next()); break; }
		case 30: a.RequestSetName(r.units()); break;
		case 31: a.RequestLobbyLeave(r.next() != 0); break;
		case 32: a.ResetGameStartTimer(); break;
		case 33: theLog.push_back(a.AmIHost() ? 1u : 0u); break;
		case 34: a.setIsActive(r.next() != 0); break;
		case 35: { LANMessage m; memset((void *)&m, 0x5A, sizeof(LANMessagePod)); a.fillInLANMessage(&m); theLog.push_back(hashBytes((unsigned char *)&m, sizeof(LANMessagePod))); break; }
		case 36: logStr(GetMessageTypeString(r.next()).str()); break;
		case 37: a.m_userName = asc(r.bytes()); break;
		case 38: a.m_hostName = asc(r.bytes()); break;
		default: break;
		}
		dump(a);
	}
	int n = (int)theLog.size();
	if (n > cap) n = cap;
	memcpy(out, &theLog[0], n * sizeof(unsigned));
	return (int)theLog.size();
}

/* the layout: sizeof(LANMessage) and the offset of each field (type, name, userName, hostName, then the union's members' fields in the order of the struct) */
int ref_la_layout(unsigned *out)
{
	int n = 0;
	LANMessagePod *m = 0;
#define OFF(f) out[n++] = (unsigned)(unsigned long)&(((LANMessagePod *)0)->f)
	out[n++] = sizeof(LANMessagePod);
	OFF(LANMessageType); OFF(name); OFF(userName); OFF(hostName);
	OFF(StartTimer.seconds); OFF(GameToLeave.gameName);
	OFF(GameInfo.gameName); OFF(GameInfo.inProgress); OFF(GameInfo.options); OFF(GameInfo.isDirectConnect);
	OFF(PlayerInfo.ip); OFF(PlayerInfo.playerName);
	OFF(GameToJoin.gameIP); OFF(GameToJoin.exeCRC); OFF(GameToJoin.iniCRC); OFF(GameToJoin.serial);
	OFF(GameJoined.gameName); OFF(GameJoined.gameIP); OFF(GameJoined.playerIP); OFF(GameJoined.slotPosition);
	OFF(GameNotJoined.gameName); OFF(GameNotJoined.gameIP); OFF(GameNotJoined.playerIP); OFF(GameNotJoined.reason);
	OFF(Accept.gameName); OFF(Accept.isAccepted);
	OFF(MapStatus.gameName); OFF(MapStatus.mapCRC); OFF(MapStatus.hasMap);
	OFF(Chat.gameName); OFF(Chat.chatType); OFF(Chat.message);
	OFF(GameOptions.options);
#undef OFF
	(void)m;
	out[n++] = LANMessagePod::MSG_MAX;
	out[n++] = g_lanPlayerNameLength; out[n++] = g_lanLoginNameLength; out[n++] = g_lanHostNameLength; out[n++] = g_lanGameNameLength; out[n++] = g_lanGameNameReservedLength;
	out[n++] = g_lanMaxChatLength; out[n++] = m_lanMaxOptionsLength; out[n++] = g_maxSerialLength;
	out[n++] = LANAPIInterface::RET_MAX; out[n++] = LANAPIInterface::LANCHAT_MAX;
	return n;
}

/* GetMessageTypeString of one type: length and bytes */
int ref_la_typestring(unsigned type, char *out, int cap)
{
	AsciiString s = GetMessageTypeString(type);
	int n = (int)strlen(s.str());
	if (n >= cap) n = cap - 1;
	memcpy(out, s.str(), n); out[n] = 0;
	return n;
}

}
