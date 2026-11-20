/* Differential-test wrapper for the GameSpy lobby state draft (#215, group network3, upstream commit 0a05454d), src/drafts/network/peer_defs.rs
** (GameSpyInfo's group rooms, player info map, ignore lists, preorder set, staging-room list, ping value, accessors; AsciiComparator;
** the GameSpyColors enum), cut verbatim by build.rs (drafts_network3) into OUT_DIR/draft_network3_gen:
**   pd_types_a.inc   GameNetwork/GameSpy/PeerDefs.h, the IgnoreList and SavedIgnoreMap typedefs;
**   pd_types_b.inc   PeerDefs.h, GameSpyGroupRoom and GroupRoomMap;
**   pd_types_c.inc   PeerDefs.h, PlayerInfo, AsciiComparator and PlayerInfoMap;
**   pd_colors.inc    PeerDefs.h, enum GameSpyColors;
**   pd_cmp.inc       PeerDefs.cpp, AsciiComparator::operator();
**   pd_preorder.inc  PeerDefs.cpp, didPlayerPreorder, markPlayerAsPreorder, setLocalIPs;
**   pd_discon.inc    PeerDefs.cpp, getAdditionalDisconnects, clearAdditionalDisconnects;
**   pd_host.inc      PeerDefs.cpp, amIHost;
**   pd_rooms.inc     PeerDefs.cpp, isBuddy to leaveStagingRoom (addGroupRoom, joinGroupRoom, leaveGroupRoom, joinBestGroupRoom, updatePlayerInfo,
**                    playerLeftGroupRoom, the staging-room list members); ONE EDIT: addGroupRoom's `room.m_translatedName.format(L"%ls %d", ...)` stays
**                    verbatim (the build is upstream's own UnicodeString.cpp with a `_vsnwprintf` that reads `%ls`, as the chat build);
**   pd_motd.inc      PeerDefs.cpp, setMOTD, getMOTD, setConfig, getConfig;
**   pd_ignore.inc    PeerDefs.cpp, the ignore-list members, grabHexInt, getPingValue, PlayerInfo::isIgnored, loadSavedIgnoreList, the disallow-text
**                    and max-messages accessors.
** Built on upstream's own UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; classes renamed by build.rs).
** Stand-ins (outside the cuts), each logging what it is asked, as tests/differential_draft_peerdefs.rs's host does:
**   `GameSpyInfo`: the class PeerDefsImplementation.h declares is the whole GameSpy state and derives from GameSpyInfoInterface (PeerDefs.h needs
**     the GameSpy SDK's headers, which are not in the upstream tree); here it has the data members the cuts use and the one-line inline members of
**     the header the cuts call (clearGroupRoomList, getGroupRoomList, setCurrentGroupRoom, getPlayerInfoMap, getStagingRoomList, ...), copied from
**     the header as written; the constructor sets what `reset()` sets (reset itself is not in the cut: it resets the local staging room);
**   `GameSpyStagingRoom`: an id and `cleanUpSlotPointers` (logged);  `GameSpyConfig` (TheGameSpyConfig): getQMChannel, setQMChannel (logged),
**     getPingTimeoutInMs;  TheGameText->fetch(label, &exists): `exists` and the text are hashes of the label;  TheGameSpyPeerMessageQueue->addRequest
**     (PeerRequest: type, groupRoom.id: logged);  GSMessageBoxOk (logged with the two texts);  IgnorePreferences (setIgnore and write: logged;
**     getIgnores: a map from the configuration);  stricmp is strcasecmp, NEW is new;  DEBUG_*: none.
** Entry points (extern "C", tests/differential_draft_peerdefs.rs): ref_dpd_cmp(a, b) (AsciiComparator on two C strings) and
**   ref_dpd_run(in, n, out, cap): a program, see the test and the opcodes at run(); out = the log. */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <vector>
#include <set>
#include <map>
#include <string>

namespace {
DynamicMemoryAllocator theAllocator;
}
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theAllocator;
CriticalSection *TheUnicodeStringCriticalSection = 0;

#define NULL 0
#define NEW new
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
typedef Int Color;
typedef Int GPProfile;

#include "pd_types_a.inc"
#include "pd_types_b.inc"
#include "pd_types_c.inc"
#include "pd_colors.inc"

/* ---- the world the PeerDefs functions ask of ---- */
static std::vector<unsigned> theLog;
static unsigned fnv(unsigned h, unsigned char c) { return (h ^ c) * 16777619u; }
static unsigned hashStr(const char *s) { unsigned h = 2166136261u; for (; *s; ++s) h = fnv(h, (unsigned char)*s); return h; }
static unsigned hashUnits(const WideChar *s) { unsigned h = 2166136261u; for (; *s; ++s) { h = fnv(h, (unsigned char)(*s & 255)); h = fnv(h, (unsigned char)(*s >> 8)); } return h; }
static unsigned g_qm, g_timeout, g_salt;

struct GameSpyConfigStandIn
{
	Int getQMChannel() { return (Int)g_qm; }
	void setQMChannel(Int id) { theLog.push_back(30); theLog.push_back((unsigned)id); g_qm = (unsigned)id; }
	Int getPingTimeoutInMs() { return (Int)g_timeout; }
};
static GameSpyConfigStandIn theConfigObj;
static GameSpyConfigStandIn *TheGameSpyConfig = &theConfigObj;

struct GameTextStandIn
{
	/* the text is "T<hash>"; it exists when the hash says so (the real fetch formats a MISSING text and sets `exists` false) */
	UnicodeString fetch(AsciiString label, Bool *exists = NULL)
	{
		unsigned h = hashStr(label.str()) ^ g_salt;
		if (exists) *exists = (h % 3u) != 0;
		char t[32];
		snprintf(t, sizeof(t), "T%u", h % 100000u);
		WideChar w[40];
		int n = 0;
		for (const char *c = t; *c; ++c) w[n++] = (WideChar)*c;
		w[n] = 0;
		UnicodeString u;
		u.set(w);
		return u;
	}
	UnicodeString fetch(const char *label, Bool *exists = NULL) { AsciiString a; a.set(label); return fetch(a, exists); }
};
static GameTextStandIn theTextObj;
static GameTextStandIn *TheGameText = &theTextObj;

struct PeerRequest
{
	enum { PEERREQUEST_JOINGROUPROOM = 1, PEERREQUEST_LEAVEGROUPROOM = 2, PEERREQUEST_LEAVESTAGINGROOM = 3 };
	PeerRequest() { peerRequestType = 0; groupRoom.id = 0; }
	int peerRequestType;
	struct { Int id; } groupRoom;
};
struct PeerQueueStandIn
{
	void addRequest(const PeerRequest &r) { theLog.push_back(40); theLog.push_back((unsigned)r.peerRequestType); theLog.push_back((unsigned)r.groupRoom.id); }
};
static PeerQueueStandIn thePeerQueueObj;
static PeerQueueStandIn *TheGameSpyPeerMessageQueue = &thePeerQueueObj;

static void GSMessageBoxOk(UnicodeString title, UnicodeString text, void *cb)
{
	theLog.push_back(50); theLog.push_back(hashUnits(title.str())); theLog.push_back(hashUnits(text.str())); theLog.push_back(cb ? 1u : 0u);
}

static SavedIgnoreMap g_prefsIgnores;
struct IgnorePreferences
{
	void setIgnore(AsciiString nick, Int profileID, Bool ignore) { theLog.push_back(60); theLog.push_back(hashStr(nick.str())); theLog.push_back((unsigned)profileID); theLog.push_back(ignore ? 1u : 0u); }
	void write() { theLog.push_back(61); }
	SavedIgnoreMap getIgnores() { return g_prefsIgnores; }
};

class GameSpyStagingRoom
{
public:
	GameSpyStagingRoom() { m_id = 0; }
	Int getID() const { return m_id; }
	void setID(Int id) { m_id = id; }
	void cleanUpSlotPointers() { theLog.push_back(70); theLog.push_back((unsigned)m_id); }
	Int m_id;
};
typedef std::map<Int, GameSpyStagingRoom *> StagingRoomMap;

class GameSpyInfo
{
public:
	GameSpyInfo()
	{
		/* what reset() sets (PeerDefs.cpp:77) */
		m_sawFullGameList = FALSE; m_isDisconAfterGameStart = FALSE; m_currentGroupRoomID = 0; m_stagingRoomsDirty = FALSE;
		m_joinedStagingRoom = 0; m_isHosting = false; m_localStagingRoomID = 0; m_gotGroupRoomList = false; m_localProfileID = 0;
		m_maxMessagesPerUpdate = 100; m_disallowAsainText = FALSE; m_disallowNonAsianText = FALSE; m_disconReason = 0;
		m_internalIP = m_externalIP = 0; m_additionalDisconnects = -1;
	}
	/* the one-line members of PeerDefsImplementation.h, as written there */
	void clearGroupRoomList( void ) { m_groupRooms.clear(); m_gotGroupRoomList = false; }
	GroupRoomMap* getGroupRoomList( void ) { return &m_groupRooms; }
	void addGroupRoom( GameSpyGroupRoom room );
	Bool gotGroupRoomList( void ) { return m_gotGroupRoomList; }
	void joinGroupRoom( Int groupID );
	void leaveGroupRoom( void );
	void joinBestGroupRoom( void );
	void setCurrentGroupRoom( Int groupID ) { m_currentGroupRoomID = groupID; m_playerInfoMap.clear(); }
	Int  getCurrentGroupRoom( void ) { return m_currentGroupRoomID; }
	void updatePlayerInfo( PlayerInfo pi, AsciiString oldNick = AsciiString::TheEmptyString );
	void playerLeftGroupRoom( AsciiString nick );
	PlayerInfoMap* getPlayerInfoMap( void ) { return &m_playerInfoMap; }
	Bool isBuddy( Int id );
	void clearStagingRoomList( void );
	StagingRoomMap* getStagingRoomList( void ) { return &m_stagingRooms; }
	GameSpyStagingRoom* findStagingRoomByID( Int id );
	void addStagingRoom( GameSpyStagingRoom room );
	void updateStagingRoom( GameSpyStagingRoom room );
	void removeStagingRoom( GameSpyStagingRoom room );
	Bool hasStagingRoomListChanged( void );
	void leaveStagingRoom( void );
	Int getCurrentStagingRoomID( void ) { return m_localStagingRoomID; }
	void sawFullGameList( void ) { m_sawFullGameList = TRUE; }
	void setDisallowAsianText( Bool val );
	void setDisallowNonAsianText( Bool val );
	Bool getDisallowAsianText( void );
	Bool getDisallowNonAsianText(void );
	void setMOTD( const AsciiString& motd );
	const AsciiString& getMOTD( void );
	void setConfig( const AsciiString& config );
	const AsciiString& getConfig( void );
	void setPingString( const AsciiString& ping ) { m_pingString = ping; }
	const AsciiString& getPingString( void ) { return m_pingString; }
	Int getPingValue( const AsciiString& otherPing );
	Bool amIHost( void );
	void addToIgnoreList( AsciiString nick );
	void removeFromIgnoreList( AsciiString nick );
	Bool isIgnored( AsciiString nick );
	IgnoreList returnIgnoreList( void );
	void loadSavedIgnoreList( void );
	SavedIgnoreMap returnSavedIgnoreList( void );
	void addToSavedIgnoreList( Int profileID, AsciiString nick);
	void removeFromSavedIgnoreList( Int profileID );
	Bool isSavedIgnored( Int profileID );
	void setLocalIPs(UnsignedInt internalIP, UnsignedInt externalIP);
	UnsignedInt getInternalIP(void) { return m_internalIP; }
	UnsignedInt getExternalIP(void) { return m_externalIP; }
	Bool didPlayerPreorder( Int profileID ) const;
	void markPlayerAsPreorder( Int profileID );
	void setMaxMessagesPerUpdate( Int num );
	Int getMaxMessagesPerUpdate( void );
	Int getAdditionalDisconnects( void );
	void clearAdditionalDisconnects( void );

	Bool m_sawFullGameList;
	Bool m_isDisconAfterGameStart;
	Int m_disconReason;
	AsciiString m_rawMotd;
	AsciiString m_rawConfig;
	AsciiString m_pingString;
	GroupRoomMap m_groupRooms;
	StagingRoomMap m_stagingRooms;
	Bool m_stagingRoomsDirty;
	std::map<GPProfile, int> m_buddyMap;
	PlayerInfoMap m_playerInfoMap;
	Int m_currentGroupRoomID;
	Bool m_gotGroupRoomList;
	AsciiString m_localName;
	Int m_localProfileID;
	Bool m_disallowAsainText;
	Bool m_disallowNonAsianText;
	UnsignedInt m_internalIP, m_externalIP;
	Int m_maxMessagesPerUpdate;
	Int m_joinedStagingRoom;
	Bool m_isHosting;
	Int m_localStagingRoomID;
	IgnoreList m_ignoreList;
	SavedIgnoreMap m_savedIgnoreMap;
	std::set<Int> m_preorderPlayers;
	Int m_additionalDisconnects;
};
static GameSpyInfo *TheGameSpyInfo = NULL;

#define stricmp strcasecmp
#include "pd_cmp.inc"
#include "pd_preorder.inc"
#include "pd_discon.inc"
#include "pd_host.inc"
#include "pd_rooms.inc"
#include "pd_motd.inc"
#include "pd_ignore.inc"

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
};

void logStr(const char *s) { theLog.push_back((unsigned)strlen(s)); for (; *s; ++s) theLog.push_back((unsigned char)*s); }
void logUnits(const WideChar *s) { unsigned n = 0; while (s[n]) ++n; theLog.push_back(n); for (unsigned i = 0; i < n; ++i) theLog.push_back((unsigned)s[i]); }
AsciiString asc(const std::string &s) { AsciiString a; a.set(s.c_str()); return a; }

/* the state, field by field (the Rust test dumps the same words) */
void dump(GameSpyInfo &g)
{
	theLog.push_back(0xD0D0D0D0u);
	logStr(g.m_pingString.str());
	theLog.push_back((unsigned)g.m_groupRooms.size());
	for (GroupRoomMap::iterator it = g.m_groupRooms.begin(); it != g.m_groupRooms.end(); ++it) {
		const GameSpyGroupRoom &r = it->second;
		theLog.push_back((unsigned)it->first); logStr(r.m_name.str()); logUnits(r.m_translatedName.str());
		theLog.push_back((unsigned)r.m_groupID); theLog.push_back((unsigned)r.m_numWaiting); theLog.push_back((unsigned)r.m_maxWaiting);
		theLog.push_back((unsigned)r.m_numGames); theLog.push_back((unsigned)r.m_numPlaying);
	}
	theLog.push_back(g.m_gotGroupRoomList ? 1u : 0u);
	theLog.push_back((unsigned)g.m_currentGroupRoomID);
	theLog.push_back((unsigned)g.m_playerInfoMap.size());
	for (PlayerInfoMap::iterator it = g.m_playerInfoMap.begin(); it != g.m_playerInfoMap.end(); ++it) {
		const PlayerInfo &p = it->second;
		logStr(it->first.str()); logStr(p.m_name.str()); logStr(p.m_locale.str());
		theLog.push_back((unsigned)p.m_wins); theLog.push_back((unsigned)p.m_losses); theLog.push_back((unsigned)p.m_profileID);
		theLog.push_back((unsigned)p.m_flags); theLog.push_back((unsigned)p.m_rankPoints); theLog.push_back((unsigned)p.m_side); theLog.push_back((unsigned)p.m_preorder);
	}
	theLog.push_back((unsigned)g.m_ignoreList.size());
	for (IgnoreList::iterator it = g.m_ignoreList.begin(); it != g.m_ignoreList.end(); ++it) logStr(it->str());
	theLog.push_back((unsigned)g.m_savedIgnoreMap.size());
	for (SavedIgnoreMap::iterator it = g.m_savedIgnoreMap.begin(); it != g.m_savedIgnoreMap.end(); ++it) { theLog.push_back((unsigned)it->first); logStr(it->second.str()); }
	for (Int id = -2; id < 10; ++id) theLog.push_back(g.didPlayerPreorder(id) ? 1u : 0u);
	theLog.push_back((unsigned)g.m_stagingRooms.size());
	for (StagingRoomMap::iterator it = g.m_stagingRooms.begin(); it != g.m_stagingRooms.end(); ++it) { theLog.push_back((unsigned)it->first); theLog.push_back((unsigned)it->second->getID()); }
	theLog.push_back(g.m_isHosting ? 1u : 0u); theLog.push_back((unsigned)g.m_localStagingRoomID);
	theLog.push_back(g.m_internalIP); theLog.push_back(g.m_externalIP);
	logStr(g.m_rawMotd.str()); logStr(g.m_rawConfig.str());
	theLog.push_back(g.m_disallowAsainText ? 1u : 0u); theLog.push_back(g.m_disallowNonAsianText ? 1u : 0u);
	theLog.push_back((unsigned)g.m_maxMessagesPerUpdate);
	theLog.push_back((unsigned)g.m_buddyMap.size());
	for (std::map<GPProfile, int>::iterator it = g.m_buddyMap.begin(); it != g.m_buddyMap.end(); ++it) theLog.push_back((unsigned)it->first);
	theLog.push_back(g_qm);
}

} // namespace

extern "C" {

int ref_dpd_cmp(const char *a, const char *b)
{
	AsciiComparator c;
	return c(asc(a), asc(b)) ? 1 : 0;
}

/* in: [qm, timeout, salt, nPrefs, (id, bytes)..., ops...]; an op is an opcode word and its arguments:
**   1 setPingString(bytes)            2 getPingValue(bytes)             3 addGroupRoom(bytes name, id, waiting, max, games, playing)
**   4 joinGroupRoom(id)               5 leaveGroupRoom                  6 joinBestGroupRoom
**   7 updatePlayerInfo(name, locale, wins, losses, id, flags, rank, side, preorder, oldNick)   8 playerLeftGroupRoom(nick)
**   9 addToIgnoreList(nick)          10 removeFromIgnoreList(nick)     11 isIgnored(nick)
**  12 addToSavedIgnoreList(id, nick) 13 removeFromSavedIgnoreList(id)  14 isSavedIgnored(id)    15 loadSavedIgnoreList
**  16 PlayerInfo::isIgnored(name, id) 17 markPlayerAsPreorder(id)      18 didPlayerPreorder(id)
**  19 setLocalIPs(a, b)              20 setCurrentGroupRoom(id)        21 clearGroupRoomList         22 sawFullGameList
**  23 addStagingRoom(id)             24 updateStagingRoom(id)          25 removeStagingRoom(id)      26 hasStagingRoomListChanged
**  27 findStagingRoomByID(id)        28 clearStagingRoomList           29 leaveStagingRoom
**  30 setMOTD(bytes)                 31 getMOTD                        32 setConfig(bytes)           33 getConfig
**  34 setDisallowAsianText(v)        35 setDisallowNonAsianText(v)     36 getDisallow* (both)
**  37 setMaxMessagesPerUpdate(n)     38 getMaxMessagesPerUpdate        39 addBuddy(id)               40 isBuddy(id)
**  41 getAdditionalDisconnects       42 clearAdditionalDisconnects     43 amIHost                    44 getCurrentGroupRoom, gotGroupRoomList, getIPs
** After each op the state is dumped. */
int ref_dpd_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	theLog.clear();
	Rd r = { in, nin, 0 };
	g_qm = r.next(); g_timeout = r.next(); g_salt = r.next();
	g_prefsIgnores.clear();
	unsigned np = r.next();
	for (unsigned i = 0; i < np; ++i) { Int id = (Int)r.next(); std::string s = r.bytes(); g_prefsIgnores[id] = asc(s); }
	GameSpyInfo g;
	TheGameSpyInfo = &g;
	dump(g);
	while (r.i < r.n) {
		unsigned op = r.next();
		theLog.push_back(0xA5A5A5A5u);
		theLog.push_back(op);
		switch (op) {
		case 1: g.setPingString(asc(r.bytes())); break;
		case 2: { AsciiString o = asc(r.bytes()); theLog.push_back((unsigned)g.getPingValue(o)); break; }
		case 3: {
			GameSpyGroupRoom room; room.m_name = asc(r.bytes());
			room.m_groupID = (Int)r.next(); room.m_numWaiting = (Int)r.next(); room.m_maxWaiting = (Int)r.next(); room.m_numGames = (Int)r.next(); room.m_numPlaying = (Int)r.next();
			g.addGroupRoom(room); break; }
		case 4: g.joinGroupRoom((Int)r.next()); break;
		case 5: g.leaveGroupRoom(); break;
		case 6: g.joinBestGroupRoom(); break;
		case 7: {
			PlayerInfo pi; pi.m_name = asc(r.bytes()); pi.m_locale = asc(r.bytes());
			pi.m_wins = (Int)r.next(); pi.m_losses = (Int)r.next(); pi.m_profileID = (Int)r.next(); pi.m_flags = (Int)r.next();
			pi.m_rankPoints = (Int)r.next(); pi.m_side = (Int)r.next(); pi.m_preorder = (Int)r.next();
			AsciiString old = asc(r.bytes());
			g.updatePlayerInfo(pi, old); break; }
		case 8: g.playerLeftGroupRoom(asc(r.bytes())); break;
		case 9: g.addToIgnoreList(asc(r.bytes())); break;
		case 10: g.removeFromIgnoreList(asc(r.bytes())); break;
		case 11: theLog.push_back(g.isIgnored(asc(r.bytes())) ? 1u : 0u); break;
		case 12: { Int id = (Int)r.next(); g.addToSavedIgnoreList(id, asc(r.bytes())); break; }
		case 13: g.removeFromSavedIgnoreList((Int)r.next()); break;
		case 14: theLog.push_back(g.isSavedIgnored((Int)r.next()) ? 1u : 0u); break;
		case 15: g.loadSavedIgnoreList(); break;
		case 16: { PlayerInfo pi; pi.m_name = asc(r.bytes()); pi.m_profileID = (Int)r.next(); theLog.push_back(pi.isIgnored() ? 1u : 0u); break; }
		case 17: g.markPlayerAsPreorder((Int)r.next()); break;
		case 18: theLog.push_back(g.didPlayerPreorder((Int)r.next()) ? 1u : 0u); break;
		case 19: { unsigned a = r.next(), b = r.next(); g.setLocalIPs(a, b); break; }
		case 20: g.setCurrentGroupRoom((Int)r.next()); break;
		case 21: g.clearGroupRoomList(); break;
		case 22: g.sawFullGameList(); break;
		case 23: { GameSpyStagingRoom room; room.setID((Int)r.next()); g.addStagingRoom(room); break; }
		case 24: { GameSpyStagingRoom room; room.setID((Int)r.next()); g.updateStagingRoom(room); break; }
		case 25: { GameSpyStagingRoom room; room.setID((Int)r.next()); g.removeStagingRoom(room); break; }
		case 26: theLog.push_back(g.hasStagingRoomListChanged() ? 1u : 0u); break;
		case 27: { GameSpyStagingRoom *p = g.findStagingRoomByID((Int)r.next()); theLog.push_back(p ? 1u : 0u); if (p) theLog.push_back((unsigned)p->getID()); break; }
		case 28: g.clearStagingRoomList(); break;
		case 29: g.leaveStagingRoom(); break;
		case 30: g.setMOTD(asc(r.bytes())); break;
		case 31: logStr(g.getMOTD().str()); break;
		case 32: g.setConfig(asc(r.bytes())); break;
		case 33: logStr(g.getConfig().str()); break;
		case 34: g.setDisallowAsianText(r.next() != 0); break;
		case 35: g.setDisallowNonAsianText(r.next() != 0); break;
		case 36: theLog.push_back(g.getDisallowAsianText() ? 1u : 0u); theLog.push_back(g.getDisallowNonAsianText() ? 1u : 0u); break;
		case 37: g.setMaxMessagesPerUpdate((Int)r.next()); break;
		case 38: theLog.push_back((unsigned)g.getMaxMessagesPerUpdate()); break;
		case 39: { Int id = (Int)r.next(); g.m_buddyMap[id] = 1; break; }
		case 40: theLog.push_back(g.isBuddy((Int)r.next()) ? 1u : 0u); break;
		case 41: theLog.push_back((unsigned)g.getAdditionalDisconnects()); break;
		case 42: g.clearAdditionalDisconnects(); break;
		case 43: theLog.push_back(g.amIHost() ? 1u : 0u); break;
		case 44: theLog.push_back((unsigned)g.getCurrentGroupRoom()); theLog.push_back(g.gotGroupRoomList() ? 1u : 0u); theLog.push_back(g.getInternalIP()); theLog.push_back(g.getExternalIP()); theLog.push_back((unsigned)g.getCurrentStagingRoomID()); logStr(g.getPingString().str()); break;
		default: break;
		}
		dump(g);
	}
	/* the staging rooms are owned by the map (delete in clearStagingRoomList): free what the program left */
	g.clearStagingRoomList();
	TheGameSpyInfo = NULL;
	int n = (int)theLog.size();
	if (n > cap) n = cap;
	memcpy(out, &theLog[0], n * sizeof(unsigned));
	return (int)theLog.size();
}

/* the GameSpyColors enum: the 28 values (GSCOLOR_MAX last) */
int ref_dpd_color_max(void) { return (int)GSCOLOR_MAX; }
int ref_dpd_color(int i) { static const int v[] = { GSCOLOR_DEFAULT, GSCOLOR_CURRENTROOM, GSCOLOR_ROOM, GSCOLOR_GAME, GSCOLOR_GAME_FULL, GSCOLOR_GAME_CRCMISMATCH, GSCOLOR_PLAYER_NORMAL, GSCOLOR_PLAYER_OWNER, GSCOLOR_PLAYER_BUDDY, GSCOLOR_PLAYER_SELF, GSCOLOR_PLAYER_IGNORED, GSCOLOR_CHAT_NORMAL, GSCOLOR_CHAT_EMOTE, GSCOLOR_CHAT_OWNER, GSCOLOR_CHAT_OWNER_EMOTE, GSCOLOR_CHAT_PRIVATE, GSCOLOR_CHAT_PRIVATE_EMOTE, GSCOLOR_CHAT_PRIVATE_OWNER, GSCOLOR_CHAT_PRIVATE_OWNER_EMOTE, GSCOLOR_CHAT_BUDDY, GSCOLOR_CHAT_SELF, GSCOLOR_ACCEPT_TRUE, GSCOLOR_ACCEPT_FALSE, GSCOLOR_MAP_SELECTED, GSCOLOR_MAP_UNSELECTED, GSCOLOR_MOTD, GSCOLOR_MOTD_HEADING }; return v[i]; }

}
