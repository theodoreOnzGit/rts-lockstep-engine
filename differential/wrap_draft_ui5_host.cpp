/* The reference PROGRAM for src/drafts/ui/popup_host_game.rs (#215, group ui5, upstream commit 0a05454d): upstream's PopupHostGame.cpp on upstream's OWN window manager and
** gadgets (see differential/wrap_draft_ui5_ndc.cpp and wrap_draft_ui4_disconnect.cpp for how the program is built). The path is `UI5_HOST_EXE`.
**
** Upstream text, cut VERBATIM by build.rs `drafts_ui5` (anchor asserted once, no edit): host_game.inc, GUICallbacks/Menus/PopupHostGame.cpp from `static NameKeyType parentPopupID`
** to the end.
**
** Stand-ins of this file (what the cut calls outside itself); every one is network, INI or file I/O, or a global of the GameSpy layer:
**   - CustomMatchPreferences (UserPreferences.cpp): the last ladder, `AllowObservers` (default TRUE), `Factions limited` (default FALSE), `UseStats` (default TRUE) of an in-memory
**     "CustomPref<profile>.ini" (`@ hg cpref`); the constructor loads it, `load(name)` logs `cpref load <name>`, `write()` stores and logs `cpref write ...`.
**   - LadderPreferences (`@ hg recent`): loadProfile(id) is nothing, getRecentLadders() the case's map. TheLadderList (`@ hg ladder`): findLadder (special, standard, local: LadderDefs.cpp:366),
**     findLadderByIndex (0 is NULL), getLocalLadders / getSpecialLadders / getStandardLadders.
**   - TheGameSpyInfo: getLocalName, getLocalProfileID (7), setCurrentGroupRoom (logged), getInternalIP, getPingString; TheGameSpyGame: the setters (logged); TheGameSpyConfig:
**     restrictGamesToLobby; TheGlobalData: m_exeCRC, m_iniCRC; GameSpyOpenOverlay, GameSpyCloseOverlay, SetLobbyAttemptHostJoin (logged); PeerRequest and the peer queue
**     (logged: `peer create ...`); GameSpyColor.
**
** Operations (besides ui4_main.h's): `@ hg init | update | hide <b> | ladsel <id> | popcombo | poplist <win> | lbdata <win> <row>`, and the case's state: `name <ascii|->`, `ip <n>`, `ping <ascii|->`,
** `crc <exe> <ini> <restrict>`, `ladder <kind 0 special 1 standard 2 local> <index> <port> <addr> <validCustom> <hexname>`, `recent <addr> <port>`, `cpref <addr|-> <port> <ao> <fl> <us>`. */
#include "ui4_wm_base.inc"
#include "ui4_host.h"
#include "GameClient/KeyDefs.h"
#include <set>
#include <vector>
#include <wctype.h>

/* ---- the GameSpy layer's globals */
enum { GSCOLOR_MAP_SELECTED, GSCOLOR_MAP_UNSELECTED };
static Color GameSpyColor[2] = { (Color)0xFF00FF00u, (Color)0xFFC0C0C0u };
enum { GSOVERLAY_GAMEOPTIONS = 2, GSOVERLAY_LADDERSELECT = 3 };
static void GameSpyOpenOverlay(int o) { g_log << "overlay open " << o << "\n"; }
static void GameSpyCloseOverlay(int o) { g_log << "overlay close " << o << "\n"; }
static void SetLobbyAttemptHostJoin(Bool b) { g_log << "lobby attemptjoin " << (int)b << "\n"; }

static AsciiString u5_local_name, u5_ping;
static UnsignedInt u5_internal_ip = 0, u5_exe_crc = 0, u5_ini_crc = 0;
static bool u5_restrict = false;
class U5GameSpyInfo
{
public:
	AsciiString getLocalName(void) { return u5_local_name; }
	Int getLocalProfileID(void) { return 7; }
	void setCurrentGroupRoom(Int r) { g_log << "gsi room " << r << "\n"; }
	UnsignedInt getInternalIP(void) { return u5_internal_ip; }
	AsciiString getPingString(void) { return u5_ping; }
};
static U5GameSpyInfo u5_gsi;
static U5GameSpyInfo *TheGameSpyInfo = &u5_gsi;
class U5GameSpyGame
{
public:
	void setGameName(UnicodeString n) { g_log << "game name"; dump_text(g_log, n); g_log << "\n"; }
	void setAllowObservers(Bool b) { g_log << "game obs " << (int)b << "\n"; }
	void setOldFactionsOnly(Bool b) { g_log << "game old " << (int)b << "\n"; }
	void setUseStats(Bool b) { g_log << "game stats " << (int)b << "\n"; }
	void setLadderIP(const char *ip) { g_log << "game ladderip " << ip << "\n"; }
	void setLadderPort(UnsignedShort p) { g_log << "game ladderport " << p << "\n"; }
};
static U5GameSpyGame u5_game;
static U5GameSpyGame *TheGameSpyGame = &u5_game;
class U5GameSpyConfig
{
public:
	Bool restrictGamesToLobby(void) { return u5_restrict; }
};
static U5GameSpyConfig u5_config;
static U5GameSpyConfig *TheGameSpyConfig = &u5_config;
struct U5GlobalData
{
	UnsignedInt m_exeCRC, m_iniCRC;
};
static U5GlobalData u5_global;
static U5GlobalData *TheGlobalData = &u5_global;

struct PeerRequest
{
	enum PeerRequestType { PEERREQUEST_CREATESTAGINGROOM };
	PeerRequestType peerRequestType;
	UnicodeString text;
	AsciiString password;
	std::string ladderIP;
	std::string hostPingStr;
	struct
	{
		Bool allowObservers, useStats, restrictGameList;
		UnsignedInt exeCRC, iniCRC, gameVersion;
		UnsignedShort ladPort;
	} stagingRoomCreation;
};
static void u5_hexs(std::ostringstream &o, const char *s)
{
	o << "[";
	for (const char *c = s; *c; ++c) { char b[8]; snprintf(b, sizeof b, "%s%x", c == s ? "" : ",", (unsigned)(unsigned char)*c); o << b; }
	o << "]";
}
class U5Queue
{
public:
	void addRequest(PeerRequest &r)
	{
		g_log << "peer create";
		dump_text(g_log, r.text);
		g_log << " pw ";
		u5_hexs(g_log, r.password.str());
		g_log << " obs " << (int)r.stagingRoomCreation.allowObservers << " stats " << (int)r.stagingRoomCreation.useStats << " exe " << r.stagingRoomCreation.exeCRC << " ini " << r.stagingRoomCreation.iniCRC << " ver "
		      << r.stagingRoomCreation.gameVersion << " restrict " << (int)r.stagingRoomCreation.restrictGameList << " ladip ";
		u5_hexs(g_log, r.ladderIP.c_str());
		g_log << " port " << r.stagingRoomCreation.ladPort << " ping ";
		u5_hexs(g_log, r.hostPingStr.c_str());
		g_log << "\n";
	}
};
static U5Queue u5_queue;
static U5Queue *TheGameSpyPeerMessageQueue = &u5_queue;

/* ---- the ladders */
struct LadderInfo
{
	AsciiString address;
	UnsignedShort port;
	UnicodeString name;
	Int index;
	Bool validCustom;
};
typedef std::list<const LadderInfo *> LadderInfoList;
class LadderList
{
public:
	LadderInfoList m_special, m_standard, m_local;
	const LadderInfo *findLadder(const AsciiString &addr, UnsignedShort port)
	{
		const LadderInfoList *ls[3] = { &m_special, &m_standard, &m_local };
		for (int k = 0; k < 3; ++k)
			for (LadderInfoList::const_iterator it = ls[k]->begin(); it != ls[k]->end(); ++it)
				if ((*it)->address == addr && (*it)->port == port) return *it;
		return NULL;
	}
	const LadderInfo *findLadderByIndex(Int index)
	{
		if (index == 0) return NULL;
		const LadderInfoList *ls[3] = { &m_special, &m_standard, &m_local };
		for (int k = 0; k < 3; ++k)
			for (LadderInfoList::const_iterator it = ls[k]->begin(); it != ls[k]->end(); ++it)
				if ((*it)->index == index) return *it;
		return NULL;
	}
	const LadderInfoList *getLocalLadders(void) { return &m_local; }
	const LadderInfoList *getSpecialLadders(void) { return &m_special; }
	const LadderInfoList *getStandardLadders(void) { return &m_standard; }
};
static LadderList u5_ladders;
static LadderList *TheLadderList = &u5_ladders;
struct LadderPref
{
	AsciiString address;
	UnsignedShort port;
};
typedef std::map<int, LadderPref> LadderPrefMap;
static LadderPrefMap u5_recent;
class LadderPreferences
{
public:
	void loadProfile(Int) {}
	const LadderPrefMap &getRecentLadders(void) { return u5_recent; }
};

/* ---- the custom match preferences (see the header comment) */
static AsciiString u5_cp_addr;
static UnsignedShort u5_cp_port = 0;
static Bool u5_cp_ao = TRUE, u5_cp_fl = FALSE, u5_cp_us = TRUE;
class CustomMatchPreferences
{
public:
	AsciiString addr;
	UnsignedShort port;
	Bool ao, fl, us;
	CustomMatchPreferences() { reload(); }
	void reload(void) { addr = u5_cp_addr; port = u5_cp_port; ao = u5_cp_ao; fl = u5_cp_fl; us = u5_cp_us; }
	void load(AsciiString name) { reload(); g_log << "cpref load " << name.str() << "\n"; }
	void setLastLadder(const AsciiString &a, UnsignedShort p) { addr = a; port = p; }
	AsciiString getLastLadderAddr(void) { return addr; }
	UnsignedShort getLastLadderPort(void) { return port; }
	Bool allowsObservers(void) { return ao; }
	Bool getFactionsLimited(void) const { return fl; }
	Bool getUseStats(void) const { return us; }
	void setAllowsObserver(Bool v) { ao = v; }
	void setFactionsLimited(Bool v) { fl = v; }
	void setUseStats(Bool v) { us = v; }
	Bool write(void)
	{
		u5_cp_addr = addr; u5_cp_port = port; u5_cp_ao = ao; u5_cp_fl = fl; u5_cp_us = us;
		g_log << "cpref write " << addr.str() << " " << port << " " << (int)ao << " " << (int)fl << " " << (int)us << "\n";
		return true;
	}
};

#include "host_game.inc"

static const U4Menu u4_menus[] = {
	{ "host", PopupHostGameSystem, PopupHostGameInput },
};
static void u4_menu_setup(void) {}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "hg") return false;
	std::string f;
	in >> f;
	if (f == "init") PopupHostGameInit(NULL, NULL);
	else if (f == "update") PopupHostGameUpdate(NULL, NULL);
	else if (f == "hide") CustomMatchHideHostPopup(u4_li(in) != 0);
	else if (f == "ladsel") HandleCustomLadderSelection(u4_li(in));
	else if (f == "popcombo") PopulateCustomLadderComboBox();
	else if (f == "poplist") PopulateCustomLadderListBox(u4_win(u4_li(in)));
	else if (f == "lbdata") { int a = u4_li(in); int r = u4_li(in); g_log << "lbdata " << (unsigned)(uintptr_t)GadgetListBoxGetItemData(u4_win(a), r) << "\n"; }
	else if (f == "name") { std::string t; in >> t; u5_local_name = t == "-" ? AsciiString("") : AsciiString(t.c_str()); }
	else if (f == "ip") { unsigned v = 0; in >> v; u5_internal_ip = v; }
	else if (f == "ping") { std::string t; in >> t; u5_ping = t == "-" ? AsciiString("") : AsciiString(t.c_str()); }
	else if (f == "crc") { unsigned a = 0, b = 0; int r = 0; in >> a >> b >> r; TheGlobalData->m_exeCRC = a; TheGlobalData->m_iniCRC = b; u5_restrict = r != 0; }
	else if (f == "ladder")
	{
		int kind, idx, port, valid;
		std::string addr, name;
		in >> kind >> idx >> port >> addr >> valid >> name;
		LadderInfo *l = new LadderInfo;
		l->address = AsciiString(addr.c_str());
		l->port = (UnsignedShort)port;
		l->name = u4_text(name);
		l->index = idx;
		l->validCustom = valid != 0;
		(kind == 0 ? u5_ladders.m_special : kind == 1 ? u5_ladders.m_standard : u5_ladders.m_local).push_back(l);
	}
	else if (f == "recent") { std::string addr; int port; in >> addr >> port; LadderPref p; p.address = AsciiString(addr.c_str()); p.port = (UnsignedShort)port; int k = (int)u5_recent.size(); u5_recent[k] = p; }
	else if (f == "cpref")
	{
		std::string addr;
		int port, a, b, c;
		in >> addr >> port >> a >> b >> c;
		u5_cp_addr = addr == "-" ? AsciiString("") : AsciiString(addr.c_str());
		u5_cp_port = (UnsignedShort)port; u5_cp_ao = a; u5_cp_fl = b; u5_cp_us = c;
	}
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	o << "hg ids " << parentPopupID << " " << textEntryGameNameID << " " << buttonCreateGameID << " " << checkBoxAllowObserversID << " " << textEntryGameDescriptionID << " " << buttonCancelID << " "
	  << textEntryLadderPasswordID << " " << comboBoxLadderNameID << " " << textEntryGamePasswordID << " " << checkBoxLimitArmiesID << " " << checkBoxUseStatsID << " populating " << (int)isPopulatingLadderBox << "\n";
	o << "hg wins " << widx(parentPopup) << " " << widx(textEntryGameName) << " " << widx(buttonCreateGame) << " " << widx(checkBoxAllowObservers) << " " << widx(textEntryGameDescription) << " " << widx(buttonCancel) << " "
	  << widx(comboBoxLadderName) << " " << widx(textEntryLadderPassword) << " " << widx(textEntryGamePassword) << " " << widx(checkBoxLimitArmies) << " " << widx(checkBoxUseStats) << "\n";
	o << "hg cpref ";
	u5_hexs(o, u5_cp_addr.str());
	o << " " << u5_cp_port << " " << (int)u5_cp_ao << " " << (int)u5_cp_fl << " " << (int)u5_cp_us << "\n";
}
#include "ui4_main.h"
