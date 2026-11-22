/* The reference PROGRAM for the staging room's display functions of src/drafts/ui/wol_game_setup_menu.rs (#215, #232, group wol8, upstream commit 0a05454d):
** `WOLDisplayGameOptions`, `WOLDisplaySlotList`, `WOLPositionStartSpots` and, which the first calls, `handlePlayerTemplateSelection`, on upstream's OWN window manager and
** gadgets (the program of group wol7: see wrap_draft_wol7_lobby.cpp), built by build.rs `drafts_wol8`. The path is `WOL8_SETUP_EXE`.
**
** Upstream text, cut VERBATIM by build.rs from WOLGameSetupMenu.cpp (each anchor asserted once, no edit):
**   setup_pos.inc      `WOLPositionStartSpots` (to `savePlayerInfo`);   setup_tpl.inc  `handlePlayerTemplateSelection` (to `handleStartPositionSelection`);
**   setup_disp.inc     `WOLDisplayGameOptions` and `WOLDisplaySlotList` (to `InitWOLGameGadgets`'s doc comment).
**
** Stand-ins (what the cuts call outside themselves; the GameSpy SDK, the peer thread and the game logic's GUI helpers are not part of what is compared, and each logs its call on BOTH sides):
**   - GameInfo / GameSpyStagingRoom / GameSlot / GameSpyGameSlot: the members the cuts read, over the case's room and slots (`@ w8 room`, `@ w8 slot`); `setPingString` stores the string and
**     `getPingAsInt` is `atoi` of it (`TheGameSpyInfo->getPingValue` is the SDK's, StagingRoomGameInfo.cpp:451); `resetAccepted` logs.
**   - TheGameSpyInfo: `getCurrentStagingRoom` (NULL when the case has no room), `getPingString`, `amIHost`, `setGameOptions` (logs);  TheGameSpyConfig: the two ping cut-offs.
**   - TheMapCache->findMap(name): the case's entries (`@ w8 mapmd`: exact name, display text);  MapMetaData: `m_displayName`.
**   - PeerRequest and TheGameSpyPeerMessageQueue->addRequest: logs `utm <id> <nick hex> <options>`.
**   - UpdateSlotList, PopulatePlayerTemplateComboBox (GUIUtil.cpp), updateMapStartSpots and the two positionStartSpots (the map preview): log `UpdateSlotList`, `populateTpl <i> <allow>`,
**     `updateMapStartSpots`, `positionStartSpots map <hex> win <w>` / `positionStartSpots room win <w>`.
**   - the file's statics the cuts read (windowMap, textEntryMapDisplay, the check boxes, combo boxes, ping windows, `pingImages`, WOLMapSelectLayout, windowMapSelectMapID): `@ w8 bind`.
**   - TheGameText: ui4_host.h's (`T:` + the label).
** Operations (besides ui4_main.h's), `@ w7 la|lsel|img` (as the lobby program), `@ w8 <op>`: `bind <role> <i> <win>` (-1: NULL), `room ...`, `slot ...`, `pingstr`, `cutoff`, `mapmd`, `pingimg`, `mapsel`,
** `mapselid`, `maprow`, `cbadd`, `cbdata`, `cbsel`, `chk`, and the calls `opts`, `slots`, `pos`. The dump adds the check boxes' and static texts' state and the windows' tooltip texts. */
#include <set>
#include <map>
#include <string>
#include <vector>
#include <stdint.h>
#include <strings.h>
#include <limits.h>
static bool w7_in_menu = false;
static std::map<std::string, int> w7_images;
static bool w7_image_exists(const char *n) { return w7_images.find(n) != w7_images.end(); }
static int w7_image_width(const char *n) { return w7_images[n]; }
#include "w7_wm_base.inc"
#include "ui4_host.h"
#include "GameClient/Image.h"
#include "GameClient/GameWindow.h"

enum { MAX_SLOTS = 8, PLAYERTEMPLATE_RANDOM = -1, PLAYERTEMPLATE_OBSERVER = -2 };
static std::string w8_hexs(const char *s)
{
	std::string o;
	char b[4];
	if (!*s) return "-";
	for (; *s; ++s) { snprintf(b, sizeof b, "%02x", (unsigned char)*s); o += b; }
	return o;
}
static std::string w8_name(const std::string &h)
{
	std::string o;
	if (h == "-") return o;
	for (size_t i = 0; i + 1 < h.size(); i += 2) o += (char)strtoul(h.substr(i, 2).c_str(), NULL, 16);
	return o;
}

/* ---- the staging room */
class Money { public: UnsignedInt m_v; UnsignedInt countMoney(void) const { return m_v; } };
class GameSlot
{
public:
	Int m_template, m_startPos;
	Bool m_hasMap, m_human;
	UnicodeString m_name;
	Int getPlayerTemplate(void) const { return m_template; }
	void setPlayerTemplate(Int t) { m_template = t; }
	void setStartPos(Int p) { m_startPos = p; }
	Bool hasMap(void) const { return m_hasMap; }
	Bool isHuman(void) const { return m_human; }
	UnicodeString getName(void) const { return m_name; }
};
class GameSpyGameSlot : public GameSlot
{
public:
	AsciiString m_pingStr;
	Int m_pingInt;
	void setPingString(AsciiString s) { m_pingStr = s; m_pingInt = atoi(s.str()); }
	Int getPingAsInt(void) const { return m_pingInt; }
};
class GameInfo
{
public:
	AsciiString m_map;
	Int m_local, m_super;
	Bool m_inGame, m_useStats, m_old, m_obs;
	Money m_cash;
	GameSpyGameSlot m_slots[MAX_SLOTS];
	Int getLocalSlotNum(void) const { return m_local; }
	const GameSlot *getConstSlot(Int i) const { return &m_slots[i]; }
	GameSlot *getSlot(Int i) { return &m_slots[i]; }
	GameSpyGameSlot *getGameSpySlot(Int i) { return &m_slots[i]; }
	AsciiString getMap(void) const { return m_map; }
	Bool getUseStats(void) const { return m_useStats; }
	Bool oldFactionsOnly(void) const { return m_old; }
	Bool getAllowObservers(void) const { return m_obs; }
	Int getSuperweaponRestriction(void) const { return m_super; }
	Money getStartingCash(void) const { return m_cash; }
	Bool isInGame(void) const { return m_inGame; }
	void resetAccepted(void) { g_log << "resetAccepted\n"; }
};
class GameSpyStagingRoom : public GameInfo {};
static GameSpyStagingRoom w8_room_obj;
static bool w8_room_exists = false, w8_host = false;
static AsciiString w8_ping_string;
static Int w8_good = 0, w8_bad = 0;
class GameSpyInfo
{
public:
	GameSpyStagingRoom *getCurrentStagingRoom(void) { return w8_room_exists ? &w8_room_obj : NULL; }
	AsciiString getPingString(void) { return w8_ping_string; }
	Bool amIHost(void) { return w8_host; }
	void setGameOptions(void) { g_log << "setGameOptions\n"; }
};
static GameSpyInfo w8_info;
static GameSpyInfo *TheGameSpyInfo = &w8_info;
class GameSpyConfig
{
public:
	Int getPingCutoffGood(void) { return w8_good; }
	Int getPingCutoffBad(void) { return w8_bad; }
};
static GameSpyConfig w8_config;
static GameSpyConfig *TheGameSpyConfig = &w8_config;
class MapMetaData { public: UnicodeString m_displayName; };
static std::map<std::string, MapMetaData> w8_maps;
class MapCache
{
public:
	const MapMetaData *findMap(AsciiString name)
	{
		std::map<std::string, MapMetaData>::iterator it = w8_maps.find(name.str());
		return it == w8_maps.end() ? NULL : &it->second;
	}
};
static MapCache w8_map_cache;
static MapCache *TheMapCache = &w8_map_cache;
class PeerRequest
{
public:
	enum { PEERREQUEST_UTMPLAYER = 1 };
	struct { Bool isStagingRoom; } UTM;
	Int peerRequestType;
	std::string id, nick, options;
};
class PeerMessageQueue
{
public:
	void addRequest(const PeerRequest &r) { g_log << "utm " << r.id << " " << w8_hexs(r.nick.c_str()) << " " << r.options << "\n"; }
};
static PeerMessageQueue w8_queue;
static PeerMessageQueue *TheGameSpyPeerMessageQueue = &w8_queue;

/* ---- the file's statics */
static GameWindow *parentWOLGameSetup, *windowMap, *textEntryMapDisplay, *checkBoxUseStats, *checkBoxLimitArmies, *checkBoxLimitSuperweapons, *comboBoxStartingCash;
static GameWindow *comboBoxPlayer[MAX_SLOTS], *comboBoxColor[MAX_SLOTS], *comboBoxPlayerTemplate[MAX_SLOTS], *comboBoxTeam[MAX_SLOTS], *buttonAccept[MAX_SLOTS];
static GameWindow *buttonMapStartPosition[MAX_SLOTS], *genericPingWindow[MAX_SLOTS], *buttonStart;
static const Image *pingImages[3];
static WindowLayout *WOLMapSelectLayout = NULL;
static NameKeyType windowMapSelectMapID = 0;
void WOLDisplaySlotList(void);

/* ---- the GUI helpers (stand-ins) */
static void UpdateSlotList(GameInfo *, GameWindow **, GameWindow **, GameWindow **, GameWindow **, GameWindow **, GameWindow *, GameWindow **) { g_log << "UpdateSlotList\n"; }
static void PopulatePlayerTemplateComboBox(Int i, GameWindow **, GameInfo *, Bool allow) { g_log << "populateTpl " << i << " " << (int)allow << "\n"; }
void updateMapStartSpots(GameInfo *, GameWindow *[], Bool onLoadScreen = FALSE) { g_log << "updateMapStartSpots\n"; }
void positionStartSpots(GameInfo *, GameWindow *[], GameWindow *w) { g_log << "positionStartSpots room win " << widx(w) << "\n"; }
void positionStartSpots(AsciiString m, GameWindow *[], GameWindow *w) { g_log << "positionStartSpots map " << w8_hexs(m.str()) << " win " << widx(w) << "\n"; }

#include "setup_pos.inc"
#include "setup_tpl.inc"
#include "setup_disp.inc"

static WindowMsgHandledType W8Sys(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData) { return MSG_IGNORED; }
static WindowMsgHandledType W8Inp(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData) { return MSG_IGNORED; }
static const U4Menu u4_menus[] = {
	{ "w8", W8Sys, W8Inp },
};
static void u4_menu_setup(void) {}
static std::vector<char *> w8_names;
static GameWindow **w8_role(const std::string &r, int i)
{
	if (r == "parent") return &parentWOLGameSetup;
	if (r == "map") return &textEntryMapDisplay;
	if (r == "usestats") return &checkBoxUseStats;
	if (r == "limitarmies") return &checkBoxLimitArmies;
	if (r == "limitsuper") return &checkBoxLimitSuperweapons;
	if (r == "cash") return &comboBoxStartingCash;
	if (r == "windowmap") return &windowMap;
	if (r == "ping") return &genericPingWindow[i];
	if (r == "tpl") return &comboBoxPlayerTemplate[i];
	if (r == "color") return &comboBoxColor[i];
	if (r == "team") return &comboBoxTeam[i];
	return NULL;
}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd == "w7")
	{
		std::string f;
		in >> f;
		if (f == "img") { std::string n; int w; in >> n >> w; w7_images[w8_name(n)] = w; }
		else if (f == "la") { int a = u4_li(in); std::string t; in >> t; int c = u4_li(in), r = u4_li(in), k = u4_li(in); Int ret = GadgetListBoxAddEntryText(u4_win(a), u4_text(t), c, r, k); g_log << "added " << ret << "\n"; }
		else if (f == "lsel") { int a = u4_li(in); int n = u4_li(in); Int idx[64]; for (int i = 0; i < n; ++i) idx[i] = u4_li(in); GadgetListBoxSetSelected(u4_win(a), idx, n); }
		return true;
	}
	if (cmd != "w8") return false;
	std::string f;
	in >> f;
	if (f == "bind") { std::string r; int i, w; in >> r >> i >> w; GameWindow **p = w8_role(r, i); *p = u4_win(w); }
	else if (f == "room")
	{
		int exists, local, ingame, stats, old, obs, super, host;
		unsigned cash;
		std::string map;
		in >> exists >> map >> local >> ingame >> stats >> old >> obs >> super >> cash >> host;
		w8_room_exists = exists; w8_host = host;
		w8_room_obj.m_map = AsciiString(w8_name(map).c_str());
		w8_room_obj.m_local = local; w8_room_obj.m_inGame = ingame; w8_room_obj.m_useStats = stats; w8_room_obj.m_old = old; w8_room_obj.m_obs = obs; w8_room_obj.m_super = super; w8_room_obj.m_cash.m_v = cash;
	}
	else if (f == "slot")
	{
		int i, t, sp, hm, human, ping;
		std::string name;
		in >> i >> t >> sp >> hm >> human >> ping >> name;
		GameSpyGameSlot &s = w8_room_obj.m_slots[i];
		s.m_template = t; s.m_startPos = sp; s.m_hasMap = hm; s.m_human = human; s.m_pingInt = ping; s.m_name = u4_text(name);
	}
	else if (f == "pingstr") { std::string s; in >> s; w8_ping_string = AsciiString(s.c_str()); }
	else if (f == "cutoff") in >> w8_good >> w8_bad;
	else if (f == "mapmd") { std::string n, d; in >> n >> d; w8_maps[w8_name(n)].m_displayName = u4_text(d); }
	else if (f == "pingimg") { int k; std::string n; in >> k >> n; pingImages[k] = TheMappedImageCollection->findImageByName(AsciiString(w8_name(n).c_str())); }
	else if (f == "mapsel") { int on; in >> on; WOLMapSelectLayout = on ? (WindowLayout *)(uintptr_t)1 : NULL; }
	else if (f == "mapselid") { int a = u4_li(in); windowMapSelectMapID = a >= 0 ? u4_win(a)->winGetWindowId() : 0; }
	else if (f == "maprow")
	{
		int a = u4_li(in), row = u4_li(in);
		std::string n;
		in >> n;
		char *p = NULL;
		if (n != "-") { std::string s = w8_name(n); p = strdup(s.c_str()); }
		GadgetListBoxSetItemData(u4_win(a), p, row, 0);
	}
	else if (f == "cbadd") { int a = u4_li(in); std::string t; in >> t; int c = u4_li(in); Int r = GadgetComboBoxAddEntry(u4_win(a), u4_text(t), c); g_log << "cbadd " << r << "\n"; }
	else if (f == "cbdata") { int a = u4_li(in), idx = u4_li(in), d = u4_li(in); GadgetComboBoxSetItemData(u4_win(a), idx, (void *)(intptr_t)d); }
	else if (f == "cbsel") { int a = u4_li(in), idx = u4_li(in); GadgetComboBoxSetSelectedPos(u4_win(a), idx, TRUE); }
	else if (f == "chk") { int a = u4_li(in), on = u4_li(in); GadgetCheckBoxSetChecked(u4_win(a), on); }
	else if (f == "opts") { WOLDisplayGameOptions(); }
	else if (f == "slots") { WOLDisplaySlotList(); }
	else if (f == "pos") { WOLPositionStartSpots(); }
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i]) continue;
		Int style = g_windows[i]->winGetInstanceData()->getStyle();
		if (BitTest(style, GWS_USER_WINDOW))
		{
			const Image *img = g_windows[i]->winGetInstanceData()->m_enabledDrawData[0].image;
			o << "w8 img " << i << " " << (img ? img->getName().str() : "-") << "\n";
		}
		if (BitTest(style, GWS_CHECK_BOX)) o << "w8 chk " << i << " " << (int)GadgetCheckBoxIsChecked(g_windows[i]) << "\n";
		if (g_windows[i]->winGetInstanceData()->getTooltipDisplayString())
		{
			o << "w8 tt " << i;
			dump_text(o, g_windows[i]->winGetInstanceData()->getTooltipText());
			o << "\n";
		}
	}
}
#include "ui4_main.h"
