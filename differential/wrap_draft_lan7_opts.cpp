/* The reference PROGRAM for src/drafts/ui/lan_game_options_menu.rs (#215, #231, group lan7, upstream commit 0a05454d): upstream's LanGameOptionsMenu.cpp on upstream's OWN window
** manager and gadgets (combo box, list box, text entry, static text, check box, push button; the program of group ui4, built by build.rs `drafts_lan7` the way `drafts_lan6`
** builds its programs, path `LAN7_OPT_EXE`).
**
** Upstream text, cut VERBATIM by build.rs (each anchor asserted once):
**   lgo.inc      GUICallbacks/Menus/LanGameOptionsMenu.cpp from `extern char *LANnextScreen;` to the end. NINE edits, all for the 16-bit WideChar of the game against the shim's
**                (the formats), plus MSVC 6's `for` scope: (1) `playerTooltip` declares `Int i;` before its loop (the code reads `i` after the loop); (2) to (9) the eight
**                `UnicodeString::format` calls with `%ls`, `%hs` or an argument become `l7_fmt_*` below (the shim's format goes through glibc's 32-bit wide printf). The calls
**                without an argument (`LAN:NeedMoreTeams`, `GUI:SandboxMode`) are upstream's own. Nothing else; the Mouse calls (`TheMouse->setCursor(Mouse::ARROW)`,
**                `setMouseText`, `setCursorTooltip`) go to `l7_mouse` through two `#define`s in this file, not through an edit.
**   slotupd.inc  GameNetwork/GUIUtil.cpp, `winInitialized`, `EnableSlotListUpdates`, `AreSlotListUpdatesEnabled`;
**   findmap.inc  GameClient/MapUtil.cpp, `MapCache::findMap`.
**
** Stand-ins (what the cut calls outside itself; each logs its call, and the Rust host writes the same line):
**   - TheLAN (LANAPI): GetMyGame (the game below, or NULL), AmIHost (`m_currentGame && m_currentGame->amIHost()`), GetMyName, GetLocalIP, RequestGameLeave, RequestAccept,
**     RequestHasMap, RequestChat, RequestGameStart, RequestGameStartTimer, RequestGameOptions, RequestGameAnnounce, ResetGameStartTimer, OnPlayerLeave, OnChat (all `lan ...`).
**     Network I/O: nothing of LANAPI is run.
**   - the game (LANGameInfo, LANGameSlot, GameSlot, GameInfo): TRANSCRIBED from GameInfo.cpp / LANGameInfo.cpp for the members the menu calls (reset, setState, unAccept,
**     setMapAvailability, setPlayerTemplate, isHuman / isOccupied / isAI / isOpen, setSlot, resetAccepted, setMapCRC / setMapSize, adjustSlotsForMap, amIHost, getLocalSlotNum,
**     isLocalPlayer, getUser); the setters the menu calls log `game ...` / `slot <i> ...`. (The real classes are verified separately: tests/differential_game_info.rs,
**     tests/differential_draft_lanhandlers.rs.) The Rust host holds the verified `common::game_info::GameInfo`, so this transcription is itself compared with it.
**   - TheMapCache: `std::map<AsciiString, MapMetaData>` (the five fields the menu reads); `findMap` is upstream's (above), `updateCache()` logs `mapcache update`; WouldMapTransfer
**     is `startsWithNoCase(user map dir)` with the dir `usr/`. File I/O: the real one reads the map files.
**   - GUIUtil.cpp's helpers, which have their own verified Rust translation (src/client/gui_util.rs): PopulateStartingCashComboBox, PopulateColorComboBox,
**     PopulatePlayerTemplateComboBox, PopulateTeamComboBox log their call and refill the combo box with the same fixed entries on both sides (entry `c<k>`, item data below);
**     UpdateSlotList, EnableAcceptControls, positionStartSpots and updateMapStartSpots only log (they read the maps' waypoints and the whole slot list).
**   - LANPreferences (`@ lo pref`: the five values the menu reads), GenerateGameOptionsString (a digest of the game's state), the tooltip callbacks (MapSelectorTooltip,
**     gameAcceptTooltip, playerTemplateComboBoxTooltip, playerTemplateListBoxTooltip: empty functions; the dump says which windows hold which), TheGlobalData (m_netMinPlayers),
**     TheMultiplayerSettings (getNumColors, getStartCountdownTimerSeconds), the Mouse (setCursor, setMouseText, setCursorTooltip log `mouse ...`).
**   - TheShell: popImmediate (the stack is the case's `@ lo stack`), push, shutdownComplete (as lan6 / ui5), reverseAnimatewindow, isAnimFinished; TheTransitionHandler: the one group
**     `LanGameOptionsFade`, modelled as the verified Rust handler does (as group ui5's network_direct_connect program does).
**   - the select-map layout (`Menus/LanMapSelectMenu.wnd`): winCreateLayout is ui4_host.h's (members from `@ lay`), its init callback logs `layout init <n>`.
**   - TheGameText: `T:` + the label, except the labels `l7_labels` gives a `%d` / `%ls` (so that the formats are checked).
**
** Operations (besides ui4_main.h's), `@ lo <op>`: `layout <n> <win>...` (the menu's layout), `init`, `shutdown <popImmediate>`, `update`, `input`-style calls are `@ inp`/`@ sys`;
** `igg` / `dgg` (InitLanGameGadgets, DeinitLanGameGadgets), `tip <win>` (playerTooltip), `start` (StartPressed), `ens <b>` (LANEnableStartButton), `lus` (lanUpdateSlotList),
** `ugo` (updateGameOptions), `pos` (LanPositionStartSpots), `post <n>` (PostToLanGameOptions), `msl <0|1>`, `stack <0|1>`, `globals <nextscreen|-> <shutting> <pushed>`,
** `slots <0|1>` (EnableSlotListUpdates), `pref <color> <faction> <map|-> <cash> <sw>`, `settings <numColors> <countdown> <minPlayers>`, `cbadd <win> <hex> <data>`,
** `cbsel <win> <pos>`, `chk <win> <b>`, `text <win> <hex>`; the game: `@ lg exists | ingame | inprogress | localip | myname | map | cash | sw | slot | user | crc | size | mapmeta`
** (`@ lg` ops below). The dump adds every static, the game, the tooltip callbacks, the accept colours, the shell and transition models, the combo boxes' item data. */
#include "ui4_wm_base.inc"
#include "ui4_host.h"
#include <set>
#include <vector>
#include <map>
#include <algorithm>
#include <wctype.h>
#include "GameClient/KeyDefs.h"
#include "GameClient/GadgetListBox.h"
#include "GameClient/GadgetComboBox.h"
#include "GameClient/GadgetTextEntry.h"
#include "GameClient/GadgetStaticText.h"
#include "GameClient/GadgetPushButton.h"
#include "GameClient/GadgetCheckBox.h"

#define stricmp strcasecmp
#ifndef NAMEKEY
#define NAMEKEY(x) TheNameKeyGenerator->nameToKey(x)
#endif
#ifndef GameMakeColor
#define GameMakeColor(r, g, b, a) ((Color)(((a) << 24) | ((r) << 16) | ((g) << 8) | (b)))
#endif
enum { MAX_SLOTS = 8 };
enum SlotState { SLOT_OPEN, SLOT_CLOSED, SLOT_EASY_AI, SLOT_MED_AI, SLOT_BRUTAL_AI, SLOT_PLAYER };
enum { PLAYERTEMPLATE_RANDOM = -1, PLAYERTEMPLATE_OBSERVER = -2, PLAYERTEMPLATE_MIN = PLAYERTEMPLATE_OBSERVER };
enum PostToLanGameType { SEND_GAME_OPTS = 0, MAP_BACK, POST_TO_LAN_GAME_TYPE_COUNT };

static std::string l7_hex(const AsciiString &a)
{
	if (a.isEmpty()) return "-";
	std::string s;
	for (const char *c = a.str(); *c; ++c)
	{
		char b[8];
		snprintf(b, sizeof b, "%s%x", s.empty() ? "" : ",", (unsigned)(unsigned char)*c);
		s += b;
	}
	return s;
}
static void l7_text(std::ostringstream &o, const UnicodeString &t) { dump_text(o, t); }

/* ---- the formats (the shim's UnicodeString::format goes through glibc's 32-bit wide printf) */
static void l7_cat(UnicodeString &o, const UnicodeString &t) { for (Int i = 0; i < t.getLength(); ++i) o.concat(t.getCharAt(i)); }
static void l7_cat(UnicodeString &o, const char *s) { for (; *s; ++s) o.concat((WideChar)(unsigned char)*s); }
static void l7_fmt_i(UnicodeString &out, UnicodeString fmt, int a)
{
	out.clear();
	l7_cat(out, fmt);
	char buf[32];
	snprintf(buf, sizeof buf, "|%d", a);
	l7_cat(out, buf);
}
static void l7_fmt_ls(UnicodeString &out, const WideChar *s)
{
	out.clear();
	for (; *s; ++s) out.concat(*s);
}
static void l7_fmt_hs(UnicodeString &out, const char *s)
{
	out.clear();
	l7_cat(out, s);
}
static void l7_fmt_ss(UnicodeString &out, UnicodeString fmt, UnicodeString a, UnicodeString b)
{
	out.clear();
	l7_cat(out, fmt);
	l7_cat(out, "|");
	l7_cat(out, a);
	l7_cat(out, "|");
	l7_cat(out, b);
}
static void l7_fmt_sss(UnicodeString &out, UnicodeString fmt, UnicodeString a, UnicodeString b, UnicodeString c)
{
	out.clear();
	l7_cat(out, fmt);
	l7_cat(out, "|");
	l7_cat(out, a);
	l7_cat(out, "|");
	l7_cat(out, b);
	l7_cat(out, "|");
	l7_cat(out, c);
}

/* ---- the globals of LANAPICallbacks.cpp, the colours */
char *LANnextScreen = NULL;
Bool LANisShuttingDown = false;
Bool LANbuttonPushed = false;
static Color chatSystemColor = GameMakeColor(255, 0, 255, 255);
static Color acceptTrueColor = GameMakeColor(0, 255, 0, 255);
static Color acceptFalseColor = GameMakeColor(255, 0, 0, 255);
static std::string l7_next_screen_store;

/* ---- the mouse */
class L7Mouse
{
public:
	enum { ARROW = 1 };
	void setCursor(int c) { g_log << "mouse cursor " << c << "\n"; }
	void setMouseText(UnicodeString t, void *, void *) { g_log << "mouse text"; dump_text(g_log, t); g_log << "\n"; }
	void setCursorTooltip(UnicodeString t, Int = -1, const void * = NULL, Real = 1.0f) { g_log << "mouse tip"; dump_text(g_log, t); g_log << "\n"; }
};
static L7Mouse l7_mouse_obj;
static L7Mouse *l7_mouse = &l7_mouse_obj;
#define TheMouse l7_mouse
#define Mouse L7Mouse

/* ---- the global data and the multiplayer settings */
class L7GlobalData { public: Int m_netMinPlayers; L7GlobalData() : m_netMinPlayers(0) {} };
static L7GlobalData l7_global_data;
static L7GlobalData *TheGlobalData = &l7_global_data;
static Int l7_num_colors = 0, l7_countdown = 0;
class L7MultiplayerSettings
{
public:
	Int getNumColors(void) { return l7_num_colors; }
	Int getStartCountdownTimerSeconds(void) { return l7_countdown; }
};
static L7MultiplayerSettings l7_multiplayer_settings;
static L7MultiplayerSettings *TheMultiplayerSettings = &l7_multiplayer_settings;

/* ---- text: labels with formats (so that the formats are compared) */
class L7GameText : public GameTextInterface
{
public:
	virtual void init() {}
	virtual void reset() {}
	virtual void update() {}
	virtual UnicodeString fetch(const Char *label, Bool *exists = NULL) { return fetch(AsciiString(label), exists); }
	virtual UnicodeString fetch(AsciiString label, Bool *exists = NULL)
	{
		std::string s = std::string("T:") + label.str();
		if (s == "T:Network:PlayerLeftGame") s += ":%ls:";
		UnicodeString u;
		for (size_t i = 0; i < s.size(); ++i) u.concat((WideChar)(unsigned char)s[i]);
		if (exists) *exists = TRUE;
		return u;
	}
	virtual AsciiStringVec &getStringsWithLabelPrefix(AsciiString) { static AsciiStringVec v; return v; }
	virtual void initMapStringFile(const AsciiString &) {}
};

/* ---- the money */
class Money
{
public:
	UnsignedInt m_money;
	Money() : m_money(0) {}
	UnsignedInt countMoney(void) const { return m_money; }
	void deposit(UnsignedInt amount, Bool playSound = TRUE) { m_money += amount; }
};

/* ---- the map cache */
struct MapMetaData
{
	Bool m_isOfficial;
	UnsignedInt m_CRC;
	UnsignedInt m_filesize;
	Int m_numPlayers;
	UnicodeString m_displayName;
};
class MapCache : public std::map<AsciiString, MapMetaData>
{
public:
	const MapMetaData *findMap(AsciiString mapName);
	void updateCache(void) { g_log << "mapcache update\n"; }
};
static MapCache l7_cache;
static MapCache *TheMapCache = &l7_cache;
#include "findmap.inc"
static Bool WouldMapTransfer(const AsciiString &mapName)
{
	const char *p = "usr/";
	const char *s = mapName.str();
	for (; *p; ++p, ++s)
	{
		if (!*s || tolower((unsigned char)*s) != *p) return FALSE;
	}
	return TRUE;
}

/* ---- the game */
class FirewallHelperClass { public: enum FirewallBehaviorType { FIREWALL_TYPE_UNKNOWN = 0, FIREWALL_TYPE_SIMPLE = 1 }; };
static UnsignedInt l7_local_ip = 0;
class LANPlayer
{
public:
	UnicodeString m_name, m_login, m_host;
	UnicodeString getName(void) { return m_name; }
	UnicodeString getLogin(void) { return m_login; }
	UnicodeString getHost(void) { return m_host; }
};
class GameSlot
{
public:
	SlotState m_state;
	Bool m_isAccepted, m_hasMap;
	Int m_color, m_startPos, m_playerTemplate, m_teamNumber;
	UnicodeString m_name;
	UnsignedInt m_IP;
	Int m_nat;
	int m_idx;
	GameSlot() : m_IP(0), m_idx(-1) { reset(); }
	void reset(void)
	{
		m_state = SLOT_CLOSED;
		m_isAccepted = false;
		m_hasMap = true;
		m_color = -1;
		m_startPos = -1;
		m_playerTemplate = -1;
		m_teamNumber = -1;
		m_nat = FirewallHelperClass::FIREWALL_TYPE_SIMPLE;
	}
	void setAccept(void) { m_isAccepted = true; }
	void unAccept(void) { if (isHuman()) m_isAccepted = false; }
	Bool isAccepted(void) const { return m_isAccepted; }
	void setMapAvailability(Bool hasMap) { if (isHuman()) m_hasMap = hasMap; }
	Bool hasMap(void) const { return m_hasMap; }
	void setState(SlotState state, UnicodeString name = UnicodeString::TheEmptyString, UnsignedInt IP = 0)
	{
		if (!(isAI() && (state == SLOT_EASY_AI || state == SLOT_MED_AI || state == SLOT_BRUTAL_AI)))
		{
			m_color = -1;
			m_startPos = -1;
			m_playerTemplate = -1;
			m_teamNumber = -1;
		}
		if (state == SLOT_PLAYER)
		{
			reset();
			m_state = state;
			m_name = name;
		}
		else
		{
			m_state = state;
			m_isAccepted = true;
			m_hasMap = true;
			switch (state)
			{
			case SLOT_OPEN: m_name = TheGameText->fetch("GUI:Open"); break;
			case SLOT_EASY_AI: m_name = TheGameText->fetch("GUI:EasyAI"); break;
			case SLOT_MED_AI: m_name = TheGameText->fetch("GUI:MediumAI"); break;
			case SLOT_BRUTAL_AI: m_name = TheGameText->fetch("GUI:HardAI"); break;
			case SLOT_CLOSED:
			default: m_name = TheGameText->fetch("GUI:Closed"); break;
			}
		}
		m_IP = IP;
	}
	SlotState getState(void) const { return m_state; }
	void setColor(Int c) { m_color = c; }
	Int getColor(void) const { return m_color; }
	void setStartPos(Int p) { m_startPos = p; }
	Int getStartPos(void) const { return m_startPos; }
	void setPlayerTemplate(Int t)
	{
		m_playerTemplate = t;
		if (t <= PLAYERTEMPLATE_MIN) m_startPos = -1;
	}
	Int getPlayerTemplate(void) const { return m_playerTemplate; }
	void setTeamNumber(Int t) { m_teamNumber = t; }
	Int getTeamNumber(void) const { return m_teamNumber; }
	UnicodeString getName(void) const { return m_name; }
	UnsignedInt getIP(void) const { return m_IP; }
	void setNATBehavior(Int n) { m_nat = n; }
	Bool isHuman(void) const { return m_state == SLOT_PLAYER; }
	Bool isOccupied(void) const { return m_state == SLOT_PLAYER || m_state == SLOT_EASY_AI || m_state == SLOT_MED_AI || m_state == SLOT_BRUTAL_AI; }
	Bool isAI(void) const { return m_state == SLOT_EASY_AI || m_state == SLOT_MED_AI || m_state == SLOT_BRUTAL_AI; }
	Bool isPlayer(UnsignedInt ip) const { return m_state == SLOT_PLAYER && m_IP == ip; }
	Bool isOpen(void) const { return m_state == SLOT_OPEN; }
};
class LANGameSlot : public GameSlot
{
public:
	LANPlayer m_user;
	LANPlayer *getUser(void)
	{
		if (isHuman())
		{
			m_user.m_name = getName();
			return &m_user;
		}
		return NULL;
	}
	Bool isLocalPlayer(void) const { return isHuman() && l7_local_ip == getIP(); }
};
class GameInfo
{
public:
	GameSlot *m_slot[MAX_SLOTS];
	AsciiString m_mapName;
	UnsignedInt m_mapCRC, m_mapSize, m_localIP;
	Money m_startingCash;
	UnsignedShort m_superweaponRestriction;
	Bool m_inGame, m_inProgress;
	GameInfo() : m_mapCRC(0), m_mapSize(0), m_localIP(0), m_superweaponRestriction(0), m_inGame(false), m_inProgress(false) { for (int i = 0; i < MAX_SLOTS; ++i) m_slot[i] = NULL; }
	GameSlot *getSlot(Int n) { return (n < 0 || n >= MAX_SLOTS) ? NULL : m_slot[n]; }
	const GameSlot *getConstSlot(Int n) const { return (n < 0 || n >= MAX_SLOTS) ? NULL : m_slot[n]; }
	virtual Bool amIHost(void) const
	{
		if (!m_inGame) return false;
		return getConstSlot(0)->isPlayer(m_localIP);
	}
	virtual Int getLocalSlotNum(void) const
	{
		if (!m_inGame) return -1;
		for (Int i = 0; i < MAX_SLOTS; ++i)
		{
			const GameSlot *slot = getConstSlot(i);
			if (slot == NULL) continue;
			if (slot->isPlayer(m_localIP)) return i;
		}
		return -1;
	}
	Bool isGameInProgress(void) const { return m_inProgress; }
	AsciiString getMap(void) const { return m_mapName; }
	void setMap(AsciiString m) { m_mapName = m; g_log << "game setmap " << l7_hex(m) << "\n"; }
	UnsignedInt getMapCRC(void) const { return m_mapCRC; }
	UnsignedInt getMapSize(void) const { return m_mapSize; }
	void updateLocalMapAvailability(const char *what);
	void setMapCRC(UnsignedInt c) { m_mapCRC = c; g_log << "game crc " << c << "\n"; updateLocalMapAvailability("crc"); }
	void setMapSize(UnsignedInt s) { m_mapSize = s; g_log << "game size " << s << "\n"; updateLocalMapAvailability("size"); }
	void setSlot(Int slotNum, GameSlot slotInfo)
	{
		if (slotNum < 0 || slotNum >= MAX_SLOTS) return;
		if (!m_slot[slotNum]) return;
		if (slotNum == 0)
		{
			slotInfo.setAccept();
			slotInfo.setMapAvailability(true);
		}
		int keep = m_slot[slotNum]->m_idx;
		*m_slot[slotNum] = slotInfo;
		m_slot[slotNum]->m_idx = keep;
	}
	void resetAccepted(void)
	{
		g_log << "game reset accepted\n";
		GameSlot *slot = getSlot(0);
		if (slot) slot->setAccept();
		for (int i = 1; i < MAX_SLOTS; i++)
		{
			slot = getSlot(i);
			if (slot) slot->unAccept();
		}
	}
	void adjustSlotsForMap(void);
	void setStartingCash(const Money &c) { m_startingCash = c; g_log << "game cash " << c.countMoney() << "\n"; }
	const Money &getStartingCash(void) const { return m_startingCash; }
	void setSuperweaponRestriction(UnsignedShort r) { m_superweaponRestriction = r; g_log << "game sw " << (unsigned)r << "\n"; }
	UnsignedShort getSuperweaponRestriction(void) const { return m_superweaponRestriction; }
};
class LANGameInfo : public GameInfo
{
public:
	LANGameSlot m_LANSlot[MAX_SLOTS];
	LANGameInfo() { for (int i = 0; i < MAX_SLOTS; ++i) { m_slot[i] = &m_LANSlot[i]; m_LANSlot[i].m_idx = i; } }
	LANGameSlot *getLANSlot(Int n) { return (n < 0 || n >= MAX_SLOTS) ? NULL : &m_LANSlot[n]; }
	virtual Int getLocalSlotNum(void) const
	{
		if (!m_inGame) return -1;
		for (Int i = 0; i < MAX_SLOTS; ++i)
			if (m_LANSlot[i].isLocalPlayer()) return i;
		return -1;
	}
	/* NOT const: it does not override GameInfo::amIHost() const (LANGameInfo.h:105), so a call through a `const GameInfo *` reaches the base's */
	Bool amIHost(void)
	{
		if (!m_inGame) return false;
		return m_LANSlot[0].isLocalPlayer();
	}
	UnicodeString getPlayerName(Int who) { return m_LANSlot[who].getName(); }
	UnsignedInt getIP(Int who) { return m_slot[who]->getIP(); }
};
static LANGameInfo l7_game;
static bool l7_game_exists = true;

void GameInfo::updateLocalMapAvailability(const char *what)
{
	if (!TheMapCache) return;
	if (m_inGame && getLocalSlotNum() >= 0)
	{
		AsciiString lowerMap = m_mapName;
		lowerMap.toLower();
		std::map<AsciiString, MapMetaData>::iterator it = TheMapCache->find(lowerMap);
		if (it == TheMapCache->end()) getSlot(getLocalSlotNum())->setMapAvailability(false);
		else if (m_mapCRC != it->second.m_CRC) getSlot(getLocalSlotNum())->setMapAvailability(false);
		else getSlot(getLocalSlotNum())->setMapAvailability(true);
	}
}
void GameInfo::adjustSlotsForMap(void)
{
	g_log << "game adjust slots\n";
	const MapMetaData *md = TheMapCache->findMap(m_mapName);
	if (md != NULL)
	{
		Int numPlayers = md->m_numPlayers;
		Int numPlayerSlots = 0;
		for (Int i = 0; i < MAX_SLOTS; ++i)
		{
			GameSlot *tempSlot = getSlot(i);
			if (tempSlot->isOccupied()) ++numPlayerSlots;
		}
		for (Int i = 0; i < MAX_SLOTS; ++i)
		{
			GameSlot *slot = getSlot(i);
			if (numPlayers > numPlayerSlots)
			{
				if (!(slot->isOccupied()))
				{
					GameSlot newSlot;
					newSlot.setState(SLOT_OPEN);
					setSlot(i, newSlot);
					++numPlayerSlots;
				}
			}
			else
			{
				if (!(slot->isOccupied()))
				{
					GameSlot newSlot;
					newSlot.setState(SLOT_CLOSED);
					setSlot(i, newSlot);
				}
			}
		}
	}
}

/* ---- GenerateGameOptionsString: a digest of the game's state (the real one is verified: tests/differential_game_info.rs) */
static AsciiString GenerateGameOptionsString(void)
{
	std::ostringstream o;
	o << "G;" << l7_game.m_mapName.str() << ";" << l7_game.m_startingCash.countMoney() << ";" << (unsigned)l7_game.m_superweaponRestriction;
	for (int i = 0; i < MAX_SLOTS; ++i)
	{
		const GameSlot &s = l7_game.m_LANSlot[i];
		o << ";" << (int)s.m_state << "," << s.m_color << "," << s.m_startPos << "," << s.m_playerTemplate << "," << s.m_teamNumber << "," << (int)s.m_isAccepted;
	}
	return AsciiString(o.str().c_str());
}

/* ---- the LAN API */
class LANAPIInterface
{
public:
	enum ChatType { LANCHAT_NORMAL = 0, LANCHAT_EMOTE, LANCHAT_SYSTEM };
};
static UnicodeString l7_my_name;
class LANAPI : public LANAPIInterface
{
public:
	LANGameInfo *GetMyGame(void) { return l7_game_exists ? &l7_game : NULL; }
	Bool AmIHost(void) { return l7_game_exists && l7_game.amIHost(); }
	UnicodeString GetMyName(void) { return l7_my_name; }
	UnsignedInt GetLocalIP(void) { return l7_local_ip; }
	void RequestGameLeave(void) { g_log << "lan leave\n"; }
	void RequestAccept(void) { g_log << "lan accept\n"; }
	void RequestHasMap(void) { g_log << "lan hasmap\n"; }
	void RequestChat(UnicodeString message, ChatType t) { g_log << "lan chat " << (int)t; dump_text(g_log, message); g_log << "\n"; }
	void RequestGameStart(void) { g_log << "lan start\n"; }
	void RequestGameStartTimer(Int seconds) { g_log << "lan starttimer " << seconds << "\n"; }
	void RequestGameOptions(AsciiString options, Bool isPublic, UnsignedInt ip = 0) { g_log << "lan options " << l7_hex(options) << " " << (int)isPublic << " " << ip << "\n"; }
	void RequestGameAnnounce(void) { g_log << "lan announce\n"; }
	void ResetGameStartTimer(void) { g_log << "lan resettimer\n"; }
	void OnPlayerLeave(UnicodeString player) { g_log << "lan playerleave"; dump_text(g_log, player); g_log << "\n"; }
	void OnChat(UnicodeString player, UnsignedInt ip, UnicodeString message, ChatType t)
	{
		g_log << "lan onchat"; dump_text(g_log, player); g_log << " " << ip; dump_text(g_log, message); g_log << " " << (int)t << "\n";
	}
};
static LANAPI l7_lan;
static LANAPI *TheLAN = &l7_lan;
typedef LANAPI::ChatType L7ChatType;

/* ---- the preferences */
static Int l7_pref_color = 0, l7_pref_faction = 0;
static AsciiString l7_pref_map;
static UnsignedInt l7_pref_cash = 0;
static Bool l7_pref_sw = false;
class LANPreferences
{
public:
	Int getPreferredColor(void) { return l7_pref_color; }
	Int getPreferredFaction(void) { return l7_pref_faction; }
	AsciiString getPreferredMap(void) { return l7_pref_map; }
	Money getStartingCash(void) const { Money m; m.deposit(l7_pref_cash, FALSE); return m; }
	Bool getSuperweaponRestricted(void) const { return l7_pref_sw; }
};

/* ---- the tooltip callbacks (empty: the dump says which windows hold which) */
void MapSelectorTooltip(GameWindow *window, WinInstanceData *instData, UnsignedInt mouse) {}
void gameAcceptTooltip(GameWindow *window, WinInstanceData *instData, UnsignedInt mouse) {}
void playerTemplateComboBoxTooltip(GameWindow *window, WinInstanceData *instData, UnsignedInt mouse) {}
void playerTemplateListBoxTooltip(GameWindow *window, WinInstanceData *instData, UnsignedInt mouse) {}

/* ---- GUIUtil.cpp's slot list switch (upstream's) */
#include "slotupd.inc"

/* ---- the transition handler's one group (see the header comment) */
static bool l7_tr_cur = false, l7_tr_pend = false;
static int l7_tr_dir = 0;
void GameWindowTransitionsHandler::setGroup(AsciiString groupName, Bool immidiate)
{
	if (l7_tr_cur)
	{
		if (l7_tr_dir != -1) l7_tr_dir = -1;
		l7_tr_pend = true;
		l7_tr_dir = 1;
		return;
	}
	l7_tr_cur = true;
	l7_tr_dir = 1;
}
void GameWindowTransitionsHandler::reverse(AsciiString groupName)
{
	if (l7_tr_cur)
	{
		l7_tr_dir = -1;
		return;
	}
	l7_tr_cur = true;
	l7_tr_dir = -1;
	l7_tr_pend = false;
}
Bool GameWindowTransitionsHandler::isFinished(void) { return TRUE; }

/* ---- the shell */
static bool l7_anim_needs_update = false, l7_anim_reverse = false;
static std::vector<WindowLayout *> l7_stack;
static WindowLayout *l7_layout = NULL;
class Shell
{
public:
	void popImmediate(void)
	{
		if (l7_stack.empty()) return;
		WindowLayout *top = l7_stack.back();
		g_log << "shell popimmediate\n";
		g_log << "shell shutdown pi 1 " << u4_lay_index(top) << "\n";
		l7_stack.pop_back();
		top->destroyWindows();
		top->deleteInstance();
		TheIMEManager->detatch();
		TheIMEManager->detatch();
	}
	void push(AsciiString name, Bool shutdownImmediate = FALSE)
	{
		/* the top is hidden here, so Shell::push completes at once (shutdownComplete(NULL), doPush: the layout is made, the IME detached, the layout brought forward) */
		u4_layout_members.clear();
		WindowLayout *l = TheWindowManager->winCreateLayout(name);
		TheIMEManager->detatch();
		l->bringForward();
	}
	void shutdownComplete(WindowLayout *l, Bool impendingPush = FALSE)
	{
		g_log << "shell complete " << u4_lay_index(l) << " " << (int)impendingPush << "\n";
		TheIMEManager->detatch();
		l->bringForward();
		l7_anim_needs_update = false;
		l7_anim_reverse = false;
	}
	void reverseAnimatewindow(void)
	{
		l7_anim_reverse = true;
		l7_anim_needs_update = true;
	}
	Bool isAnimFinished(void)
	{
		if (!TheTransitionHandler->isFinished()) return FALSE;
		return l7_anim_needs_update ? FALSE : TRUE;
	}
};
static Shell l7_shell;
static Shell *TheShell = &l7_shell;

/* ---- the declarations the cut relies on (headers upstream) */
class GameInfo;
void lanUpdateSlotList(void);
void updateGameOptions(void);
void LANEnableStartButton(Bool enabled);
void StartPressed(void);
extern GameWindow *listboxChatWindowLanGame;
void PopulateStartingCashComboBox(GameWindow *comboBox, GameInfo *myGame);
void PopulateColorComboBox(Int comboBox, GameWindow *comboArray[], GameInfo *myGame, Bool isObserver = FALSE);
void PopulatePlayerTemplateComboBox(Int comboBox, GameWindow *comboArray[], GameInfo *myGame, Bool allowObservers);
void PopulateTeamComboBox(Int comboBox, GameWindow *comboArray[], GameInfo *myGame, Bool isObserver = FALSE);
void UpdateSlotList(GameInfo *myGame, GameWindow *comboPlayer[], GameWindow *comboColor[], GameWindow *comboPlayerTemplate[], GameWindow *comboTeam[], GameWindow *buttonAccept[], GameWindow *buttonStart, GameWindow *buttonMapStartPosition[]);
void EnableAcceptControls(Bool Enabled, GameInfo *myGame, GameWindow *comboPlayer[], GameWindow *comboColor[], GameWindow *comboPlayerTemplate[], GameWindow *comboTeam[], GameWindow *buttonAccept[], GameWindow *buttonStart, GameWindow *buttonMapStartPosition[], Int slotNum = -1);

#include "lgo.inc"

/* ---- the GUIUtil.cpp stand-ins (the fixed entries both sides fill) */
static void l7_fill(GameWindow *combo, int n, const int *data, const char *prefix)
{
	GadgetComboBoxReset(combo);
	for (int k = 0; k < n; ++k)
	{
		UnicodeString t;
		l7_cat(t, prefix);
		char b[16];
		snprintf(b, sizeof b, "%d", k);
		l7_cat(t, b);
		Int idx = GadgetComboBoxAddEntry(combo, t, 0xFFFFFFFF);
		GadgetComboBoxSetItemData(combo, idx, (void *)(intptr_t)data[k]);
	}
}
void PopulateStartingCashComboBox(GameWindow *comboBox, GameInfo *myGame)
{
	g_log << "gui populate cash " << widx(comboBox) << "\n";
	static const int d[] = { 1000, 2000, 3000, 4000 };
	l7_fill(comboBox, 4, d, "cash");
}
void PopulateColorComboBox(Int comboBox, GameWindow *comboArray[], GameInfo *myGame, Bool isObserver)
{
	g_log << "gui populate color " << comboBox << " " << (int)isObserver << "\n";
	static const int d[] = { -1, 0, 1, 2, 3 };
	l7_fill(comboArray[comboBox], 5, d, "c");
}
void PopulatePlayerTemplateComboBox(Int comboBox, GameWindow *comboArray[], GameInfo *myGame, Bool allowObservers)
{
	g_log << "gui populate template " << comboBox << " " << (int)allowObservers << "\n";
	static const int d[] = { -1, 0, 1, 2, -2 };
	l7_fill(comboArray[comboBox], allowObservers ? 5 : 4, d, "t");
}
void PopulateTeamComboBox(Int comboBox, GameWindow *comboArray[], GameInfo *myGame, Bool isObserver)
{
	g_log << "gui populate team " << comboBox << " " << (int)isObserver << "\n";
	static const int d[] = { -1, 0, 1, 2, 3 };
	l7_fill(comboArray[comboBox], 5, d, "m");
}
void UpdateSlotList(GameInfo *myGame, GameWindow *comboPlayer[], GameWindow *comboColor[], GameWindow *comboPlayerTemplate[], GameWindow *comboTeam[], GameWindow *buttonAccept[], GameWindow *buttonStart, GameWindow *buttonMapStartPosition[])
{
	g_log << "gui updateslotlist " << (myGame ? 1 : 0) << "\n";
}
void EnableAcceptControls(Bool Enabled, GameInfo *myGame, GameWindow *comboPlayer[], GameWindow *comboColor[], GameWindow *comboPlayerTemplate[], GameWindow *comboTeam[], GameWindow *buttonAccept[], GameWindow *buttonStart, GameWindow *buttonMapStartPosition[], Int slotNum)
{
	g_log << "gui enableaccept " << (int)Enabled << " " << (myGame ? 1 : 0) << " " << slotNum << "\n";
}
void updateMapStartSpots(GameInfo *myGame, GameWindow *buttonMapStartPositions[], Bool onLoadScreen)
{
	g_log << "gui updatemapspots " << (myGame ? 1 : 0) << " " << (int)onLoadScreen;
	for (int i = 0; i < MAX_SLOTS; ++i) g_log << " " << widx(buttonMapStartPositions[i]);
	g_log << "\n";
}
void positionStartSpots(GameInfo *myGame, GameWindow *buttonMapStartPositions[], GameWindow *mapWindow)
{
	g_log << "gui positionspots " << (myGame ? 1 : 0);
	for (int i = 0; i < MAX_SLOTS; ++i) g_log << " " << widx(buttonMapStartPositions[i]);
	g_log << " win " << widx(mapWindow) << "\n";
}

/* ---- the select-map layout's init callback (LanMapSelectMenuInit, its own program: lan6) */
static void l7_layout_init(WindowLayout *l, void *) { g_log << "layout init " << u4_lay_index(l) << "\n"; }

static const U4Menu u4_menus[] = {
	{ "lgo", LanGameOptionsMenuSystem, LanGameOptionsMenuInput },
};
static void u4_menu_setup(void)
{
	u4_layout_init = l7_layout_init;
	for (int i = 0; i < MAX_SLOTS; ++i) { l7_game.m_slot[i] = &l7_game.m_LANSlot[i]; }
}
static bool l7_hexbool(std::istringstream &in) { int a = 0; in >> a; return a != 0; }
static UnicodeString l7_u(std::istringstream &in) { std::string t; in >> t; return u4_text(t); }
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "lo" && cmd != "lg") return false;
	std::string f;
	in >> f;
	if (cmd == "lg")
	{
		if (f == "exists") l7_game_exists = l7_hexbool(in);
		else if (f == "ingame") l7_game.m_inGame = l7_hexbool(in);
		else if (f == "inprogress") l7_game.m_inProgress = l7_hexbool(in);
		else if (f == "localip") { unsigned v = 0; in >> v; l7_local_ip = v; l7_game.m_localIP = v; }
		else if (f == "myname") l7_my_name = l7_u(in);
		else if (f == "map") { std::string t; in >> t; l7_game.m_mapName = t == "-" ? AsciiString("") : AsciiString(t.c_str()); }
		else if (f == "cash") { unsigned v = 0; in >> v; Money m; m.deposit(v, FALSE); l7_game.m_startingCash = m; }
		else if (f == "sw") { unsigned v = 0; in >> v; l7_game.m_superweaponRestriction = (UnsignedShort)v; }
		else if (f == "crc") { unsigned v = 0; in >> v; l7_game.m_mapCRC = v; }
		else if (f == "size") { unsigned v = 0; in >> v; l7_game.m_mapSize = v; }
		else if (f == "slot")
		{
			int i, state, color, start, tpl, team, acc, hasmap;
			unsigned ip;
			in >> i >> state >> color >> start >> tpl >> team >> acc >> hasmap >> ip;
			LANGameSlot &s = l7_game.m_LANSlot[i];
			s.m_state = (SlotState)state;
			s.m_color = color;
			s.m_startPos = start;
			s.m_playerTemplate = tpl;
			s.m_teamNumber = team;
			s.m_isAccepted = acc != 0;
			s.m_hasMap = hasmap != 0;
			s.m_IP = ip;
			s.m_name = l7_u(in);
		}
		else if (f == "user")
		{
			int i = u4_li(in);
			l7_game.m_LANSlot[i].m_user.m_login = l7_u(in);
			l7_game.m_LANSlot[i].m_user.m_host = l7_u(in);
		}
		else if (f == "mapmeta")
		{
			std::string file;
			int official, numPlayers;
			unsigned crc, size;
			in >> file >> official >> crc >> size >> numPlayers;
			MapMetaData m;
			m.m_isOfficial = official != 0;
			m.m_CRC = crc;
			m.m_filesize = size;
			m.m_numPlayers = numPlayers;
			m.m_displayName = l7_u(in);
			l7_cache[AsciiString(file.c_str())] = m;
		}
		return true;
	}
	if (f == "layout")
	{
		int n = u4_li(in);
		u4_layout_members.clear();
		for (int i = 0; i < n; ++i) u4_layout_members.push_back(u4_li(in));
		l7_layout = TheWindowManager->winCreateLayout(AsciiString("LanGameOptionsMenu.wnd"));
	}
	else if (f == "init") LanGameOptionsMenuInit(l7_layout, NULL);
	else if (f == "shutdown") { Bool pi = u4_li(in) != 0; LanGameOptionsMenuShutdown(l7_layout, &pi); }
	else if (f == "update") LanGameOptionsMenuUpdate(l7_layout, NULL);
	else if (f == "igg") InitLanGameGadgets();
	else if (f == "dgg") DeinitLanGameGadgets();
	else if (f == "tip") { int a = u4_li(in); playerTooltip(u4_win(a), NULL, 0); }
	else if (f == "start") StartPressed();
	else if (f == "ens") LANEnableStartButton(u4_li(in) != 0);
	else if (f == "lus") lanUpdateSlotList();
	else if (f == "ugo") updateGameOptions();
	else if (f == "pos") LanPositionStartSpots();
	else if (f == "post") PostToLanGameOptions((PostToLanGameType)u4_li(in));
	else if (f == "msl") mapSelectLayout = u4_li(in) ? l7_layout : NULL;
	else if (f == "stack") { l7_stack.clear(); if (u4_li(in)) l7_stack.push_back(l7_layout); }
	else if (f == "globals")
	{
		std::string n;
		in >> n;
		if (n == "-") LANnextScreen = NULL; else { l7_next_screen_store = n; LANnextScreen = (char *)l7_next_screen_store.c_str(); }
		LANisShuttingDown = u4_li(in) != 0;
		LANbuttonPushed = u4_li(in) != 0;
	}
	else if (f == "slots") EnableSlotListUpdates(u4_li(in) != 0);
	else if (f == "pref")
	{
		l7_pref_color = u4_li(in);
		l7_pref_faction = u4_li(in);
		std::string t;
		in >> t;
		l7_pref_map = t == "-" ? AsciiString("") : AsciiString(t.c_str());
		unsigned c = 0;
		in >> c;
		l7_pref_cash = c;
		l7_pref_sw = u4_li(in) != 0;
	}
	else if (f == "settings") { l7_num_colors = u4_li(in); l7_countdown = u4_li(in); TheGlobalData->m_netMinPlayers = u4_li(in); }
	else if (f == "cbadd") { int a = u4_li(in); UnicodeString t = l7_u(in); int d = u4_li(in); GameWindow *w = u4_win(a); Int idx = GadgetComboBoxAddEntry(w, t, 0xFFFFFFFF); GadgetComboBoxSetItemData(w, idx, (void *)(intptr_t)d); }
	else if (f == "cbsel") { int a = u4_li(in); int p = u4_li(in); GadgetComboBoxSetSelectedPos(u4_win(a), p, FALSE); }
	else if (f == "chk") { int a = u4_li(in); GadgetCheckBoxSetChecked(u4_win(a), u4_li(in) != 0); }
	else if (f == "text") { int a = u4_li(in); GadgetTextEntrySetText(u4_win(a), l7_u(in)); }
	return true;
}
static void l7_ids(std::ostringstream &o, const char *name, const NameKeyType *a)
{
	o << " " << name;
	for (int i = 0; i < MAX_SLOTS; ++i) o << " " << a[i];
}
static void l7_wins(std::ostringstream &o, const char *name, GameWindow *const *a)
{
	o << " " << name;
	for (int i = 0; i < MAX_SLOTS; ++i) o << " " << widx(a[i]);
}
static const char *l7_tip_name(GameWinTooltipFunc f)
{
	if (f == NULL) return NULL;
	if (f == MapSelectorTooltip) return "map";
	if (f == gameAcceptTooltip) return "accept";
	if (f == playerTooltip) return "player";
	if (f == playerTemplateComboBoxTooltip) return "tplcombo";
	if (f == playerTemplateListBoxTooltip) return "tpllist";
	return "other";
}
static void u4_menu_dump(std::ostringstream &o)
{
	o << "lo ids " << parentLanGameOptionsID << " " << textEntryChatID << " " << textEntryMapDisplayID << " " << buttonBackID << " " << buttonStartID << " " << buttonEmoteID << " " << buttonSelectMapID << " " << checkboxLimitSuperweaponsID << " " << comboBoxStartingCashID << " " << windowMapID << " " << listboxChatWindowLanGameID;
	l7_ids(o, "pl", comboBoxPlayerID);
	l7_ids(o, "acc", buttonAcceptID);
	l7_ids(o, "col", comboBoxColorID);
	l7_ids(o, "tpl", comboBoxPlayerTemplateID);
	l7_ids(o, "team", comboBoxTeamID);
	l7_ids(o, "sp", buttonMapStartPositionID);
	o << "\nlo wins " << widx(parentLanGameOptions) << " " << widx(buttonBack) << " " << widx(buttonStart) << " " << widx(buttonSelectMap) << " " << widx(buttonEmote) << " " << widx(textEntryChat) << " " << widx(textEntryMapDisplay) << " " << widx(checkboxLimitSuperweapons) << " " << widx(comboBoxStartingCash) << " " << widx(windowMap) << " " << widx(listboxChatWindowLanGame);
	l7_wins(o, "pl", comboBoxPlayer);
	l7_wins(o, "acc", buttonAccept);
	l7_wins(o, "col", comboBoxColor);
	l7_wins(o, "tpl", comboBoxPlayerTemplate);
	l7_wins(o, "team", comboBoxTeam);
	l7_wins(o, "sp", buttonMapStartPosition);
	o << "\nlo flags init " << (int)s_isIniting << " msl " << u4_lay_index(mapSelectLayout) << " shutting " << (int)LANisShuttingDown << " pushed " << (int)LANbuttonPushed << " next";
	if (LANnextScreen) o << " " << LANnextScreen; else o << " -";
	o << " slots " << (int)AreSlotListUpdatesEnabled() << "\n";
	o << "lo game exists " << (int)l7_game_exists << " ingame " << (int)l7_game.m_inGame << " progress " << (int)l7_game.m_inProgress << " ip " << l7_local_ip << "/" << l7_game.m_localIP << " map " << l7_hex(l7_game.m_mapName) << " crc " << l7_game.m_mapCRC << " size " << l7_game.m_mapSize << " cash " << l7_game.m_startingCash.countMoney() << " sw " << (unsigned)l7_game.m_superweaponRestriction << " name";
	l7_text(o, l7_my_name);
	o << "\n";
	for (int i = 0; i < MAX_SLOTS; ++i)
	{
		const LANGameSlot &s = l7_game.m_LANSlot[i];
		o << "lo slot " << i << " " << (int)s.m_state << " " << s.m_color << " " << s.m_startPos << " " << s.m_playerTemplate << " " << s.m_teamNumber << " " << (int)s.m_isAccepted << " " << (int)s.m_hasMap << " " << s.m_IP << " " << s.m_nat;
		l7_text(o, s.m_name);
		l7_text(o, s.m_user.m_login);
		l7_text(o, s.m_user.m_host);
		o << "\n";
	}
	o << "lo tips";
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i]) continue;
		const char *n = l7_tip_name(g_windows[i]->winGetTooltipFunc());
		if (n) o << " " << i << ":" << n;
	}
	o << "\nlo colors";
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i]) continue;
		if (g_windows[i]->winGetEnabledColor(0) == acceptTrueColor) o << " " << i;
	}
	o << "\nlo trans cur " << (int)l7_tr_cur << " pend " << (int)l7_tr_pend << " dir " << l7_tr_dir << " anim nu " << (int)l7_anim_needs_update << " rev " << (int)l7_anim_reverse << "\n";
	o << "lo items";
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i] || !BitTest(g_windows[i]->winGetInstanceData()->getStyle(), GWS_COMBO_BOX)) continue;
		o << " " << i << ":";
		Int n = GadgetComboBoxGetLength(g_windows[i]);
		for (Int k = 0; k < n; ++k) o << (k ? "," : "") << (Int)(intptr_t)GadgetComboBoxGetItemData(g_windows[i], k);
	}
	o << "\n";
}
#include "ui4_main.h"
