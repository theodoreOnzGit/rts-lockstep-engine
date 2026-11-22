/* The reference PROGRAM for the window-bound list functions of src/drafts/ui/wol_lobby_menu.rs (#215, group wol7, upstream commit 0a05454d): `playerTooltip`,
** `populateGroupRoomListbox`, `LookupSmallRankImage` (called by `insertPlayerInListbox`), `insertPlayerInListbox` and `PopulateLobbyPlayerListbox`, on upstream's OWN window
** manager and gadgets (the program of groups ui4 and ui5: GadgetListBox.cpp, GadgetComboBox.cpp, ... and WindowLayout.cpp; see differential/wrap_draft_ui4_disconnect.cpp for how
** it is built; the shared stand-ins are in ui4_host.h and ui4_main.h), built by build.rs `drafts_wol7`. The path is `WOL7_LOBBY_EXE`.
**
** Upstream text, cut VERBATIM by build.rs (each anchor asserted once):
**   peer_buddy.inc   PeerDefs.h, `BuddyInfo` and `BuddyInfoMap`;     peer_room.inc  `GameSpyGroupRoom` and `GroupRoomMap`;
**   peer_player.inc  PeerDefs.h, `PlayerInfo`, `AsciiComparator`, `PlayerInfoMap` and the `GameSpyColors` enum;
**   peer_cmp.inc     PeerDefs.cpp, `AsciiComparator::operator()` (stricmp is strcasecmp: the C locale folds ASCII only);
**   lobby_lists.inc  WOLLobbyMenu.cpp, from `playerTooltip` to `WOLLobbyMenuInit`: `playerTooltip`, `populateGroupRoomListbox`, `rankNames`, `LookupSmallRankImage`,
**                    `insertPlayerInListbox`, `PopulateLobbyPlayerListbox`. EIGHT edits (the last: MSVC 6's `for` scope, `Int i;` declared before the loop that restores the selection): the six `UnicodeString::format` calls become `w7_fmt_*` (the draft's host formats the
**                    texts, the shim's `format` is glibc's 32-bit vswprintf: each stand-in below says what it produces), and `Int *selectedIndices;` is initialised to NULL
**                    (`GadgetListBoxGetSelected(list, (Int *)(&selectedIndices))` writes a 32-bit pointer over the whole 32-bit variable in the game; here the other half must be 0).
** The file's `enum { COLUMN_PLAYERNAME = 1 };` (line 102) and `listboxLobbyPlayers` are written below.
**
** Stand-ins of this file (what the cuts call outside themselves):
**   - TheGameSpyInfo: `getPlayerInfoMap`, `getBuddyMap`, `getGroupRoomList`, `getLocalName`, `getCurrentGroupRoom`, `didPlayerPreorder` over the case's state (`@ w7 ...`); the
**     `PlayerInfo::isIgnored` is GameSpyInfo::isSavedIgnored(profile) or isIgnored(name) (PeerDefs.cpp:827, :753, :779) over the case's two sets.
**   - TheGameSpyConfig: `getQMChannel`, `isPlayerVIP` (the case's QM channel and VIP set; GSConfig.cpp:473).
**   - TheRankPointValues->m_ranks (the case's table), ThePlayerTemplateStore->getNthPlayerTemplate(n)->getSide() (the case's list, NULL out of it).
**   - TheMappedImageCollection: the shim of wrap_wm.cpp, edited by build.rs: it answers the images the case declares, at the width the case gives, and logs `find <name>`.
**   - TheMouse->setCursorTooltip: logs `tip <text> delay <d> width <bits of the float>` (edited in wrap_wm.cpp's text by build.rs).
**   - UnicodeString::format of the six calls: `w7_fmt_s` (format, `|`, the text), `w7_fmt_sii` (format, `|`, the text, `|`, a, `|`, b) and `w7_fmt_nl_ss` (the real format:
**     a line feed, the side's text, a space, the rank's text). The draft's host does the first two the same way (tests/differential_draft_wol_lobby_lists.rs).
**   - TheGameText: ui4_host.h's (`T:` + the label).
**
** Operations (besides ui4_main.h's), `@ w7 <op>`: `me <name>` `cur <id>` `qm <id>` `rk <14 ints>` `img <name> <width>` `tpl <n> <side>` `room <id> <hex units>` `roomclr`
** `pl <name> <profile> <flags> <wins> <losses> <locale> <rank> <side> <preorder>` `plclr` `bud <id>` `vip <id>` `pre <id>` `ign <id>` `ignn <name>` `col <i> <v>` (names as hex
** bytes, `-` for none) and the calls `lbp <win>` (sets listboxLobbyPlayers), `popgr <win>`, `poppl`, `ins <color> <player as pl>`, `tip <win> <mouse>`, `la <win> <hex> <color> <row> <col>`
** (GadgetListBoxAddEntryText), `lsel <win> <n> <idx>...`, `ltop <win> <idx>`, `lres <win>`, `lone <win>` (winSetLoneWindow). The dump adds listboxLobbyPlayers and every list box's item data. */
#include <set>
#include <map>
#include <string>
#include <stdint.h>
#include <strings.h>
#include <limits.h>
static bool w7_in_menu = false; /* the lookups the menu's functions make are logged, not the gadgets' own default-look ones */
static std::map<std::string, int> w7_images;
static bool w7_image_exists(const char *n) { return w7_images.find(n) != w7_images.end(); }
static int w7_image_width(const char *n) { return w7_images[n]; }
#include "w7_wm_base.inc"
#include "ui4_host.h"
#include "GameClient/Image.h"

typedef Int GPProfile;
typedef Int GPEnum;
#define stricmp strcasecmp
#include "peer_buddy.inc"
#include "peer_room.inc"
#include "peer_player.inc"
Color GameSpyColor[GSCOLOR_MAX];
#include "peer_cmp.inc"

enum { COLUMN_PLAYERNAME = 1 };
/* the GameSpy peer SDK's flag (not in the upstream tree; the draft's value, a stand-in on both sides) */
enum { PEER_FLAG_OP = 1 << 1 };
static GameWindow *listboxLobbyPlayers = NULL;

/* ---- the case's GameSpy state */
static std::set<Int> w7_preorder, w7_saved_ignored, w7_vip;
static std::set<AsciiString> w7_ignored_names;
Bool PlayerInfo::isIgnored(void) { return m_profileID ? (w7_saved_ignored.find(m_profileID) != w7_saved_ignored.end()) : (w7_ignored_names.find(m_name) != w7_ignored_names.end()); }
class GameSpyInfo
{
public:
	PlayerInfoMap m_players;
	BuddyInfoMap m_buddies;
	GroupRoomMap m_rooms;
	AsciiString m_local;
	Int m_current;
	GameSpyInfo() : m_current(0) {}
	PlayerInfoMap *getPlayerInfoMap() { return &m_players; }
	BuddyInfoMap *getBuddyMap() { return &m_buddies; }
	GroupRoomMap *getGroupRoomList() { return &m_rooms; }
	AsciiString getLocalName() { return m_local; }
	Int getCurrentGroupRoom() { return m_current; }
	Bool didPlayerPreorder(Int id) const { return w7_preorder.find(id) != w7_preorder.end(); }
};
static GameSpyInfo w7_info;
static GameSpyInfo *TheGameSpyInfo = &w7_info;
class GameSpyConfig
{
public:
	Int m_qm;
	GameSpyConfig() : m_qm(0) {}
	Int getQMChannel() { return m_qm; }
	Bool isPlayerVIP(Int id) { return w7_vip.find(id) != w7_vip.end(); }
};
static GameSpyConfig w7_config;
static GameSpyConfig *TheGameSpyConfig = &w7_config;
struct RankPointValue { Int m_ranks[14]; };
static RankPointValue w7_ranks;
static RankPointValue *TheRankPointValues = &w7_ranks;
class PlayerTemplate
{
public:
	AsciiString m_side;
	const AsciiString &getSide() const { return m_side; }
};
static std::vector<PlayerTemplate> w7_templates;
class PlayerTemplateStore
{
public:
	const PlayerTemplate *getNthPlayerTemplate(Int i) const { return (i >= 0 && i < (Int)w7_templates.size()) ? &w7_templates[i] : NULL; }
};
static PlayerTemplateStore w7_pts;
static PlayerTemplateStore *ThePlayerTemplateStore = &w7_pts;

/* ---- the formats */
static void w7_cat(UnicodeString &o, const UnicodeString &t) { for (Int i = 0; i < t.getLength(); ++i) o.concat(t.getCharAt(i)); }
static void w7_cat(UnicodeString &o, const char *s) { for (; *s; ++s) o.concat((WideChar)(unsigned char)*s); }
static void w7_fmt_s(UnicodeString &out, UnicodeString fmt, UnicodeString s)
{
	out.clear();
	w7_cat(out, fmt);
	w7_cat(out, "|");
	w7_cat(out, s);
}
static void w7_fmt_sii(UnicodeString &out, UnicodeString fmt, UnicodeString s, int a, int b)
{
	out.clear();
	w7_cat(out, fmt);
	w7_cat(out, "|");
	w7_cat(out, s);
	char buf[64];
	snprintf(buf, sizeof buf, "|%d|%d", a, b);
	w7_cat(out, buf);
}
static void w7_fmt_nl_ss(UnicodeString &out, UnicodeString side, UnicodeString rank)
{
	out.clear();
	out.concat((WideChar)'\n');
	w7_cat(out, side);
	out.concat((WideChar)' ');
	w7_cat(out, rank);
}

#include "lobby_lists.inc"

static WindowMsgHandledType W7Sys(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData) { return MSG_IGNORED; }
static WindowMsgHandledType W7Inp(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData) { return MSG_IGNORED; }
static const U4Menu u4_menus[] = {
	{ "w7", W7Sys, W7Inp },
};
static void u4_menu_setup(void) {}
/* a name or text on the input: bytes in hex pairs, `-` for none */
static std::string w7_name(const std::string &h)
{
	std::string o;
	if (h == "-") return o;
	for (size_t i = 0; i + 1 < h.size(); i += 2) o += (char)strtoul(h.substr(i, 2).c_str(), NULL, 16);
	return o;
}
static PlayerInfo w7_read_player(std::istringstream &in)
{
	std::string name, locale;
	PlayerInfo p;
	in >> name >> p.m_profileID >> p.m_flags >> p.m_wins >> p.m_losses >> locale >> p.m_rankPoints >> p.m_side >> p.m_preorder;
	p.m_name = AsciiString(w7_name(name).c_str());
	p.m_locale = AsciiString(w7_name(locale).c_str());
	return p;
}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "w7") return false;
	std::string f;
	in >> f;
	if (f == "me") { std::string n; in >> n; w7_info.m_local = AsciiString(w7_name(n).c_str()); }
	else if (f == "cur") in >> w7_info.m_current;
	else if (f == "qm") in >> w7_config.m_qm;
	else if (f == "rk") { for (int i = 0; i < 14; ++i) in >> w7_ranks.m_ranks[i]; }
	else if (f == "img") { std::string n; int w; in >> n >> w; w7_images[w7_name(n)] = w; }
	else if (f == "tpl") { int k; std::string s; in >> k >> s; PlayerTemplate t; t.m_side = AsciiString(w7_name(s).c_str()); if ((int)w7_templates.size() <= k) w7_templates.resize(k + 1); w7_templates[k] = t; }
	else if (f == "room")
	{
		GameSpyGroupRoom r;
		std::string t;
		in >> r.m_groupID >> t;
		r.m_translatedName = u4_text(t);
		w7_info.m_rooms[r.m_groupID] = r;
	}
	else if (f == "roomclr") w7_info.m_rooms.clear();
	else if (f == "pl") { PlayerInfo p = w7_read_player(in); w7_info.m_players[p.m_name] = p; }
	else if (f == "plclr") w7_info.m_players.clear();
	else if (f == "bud") { Int id; in >> id; BuddyInfo b; b.m_id = id; w7_info.m_buddies[id] = b; }
	else if (f == "vip") { Int id; in >> id; w7_vip.insert(id); }
	else if (f == "pre") { Int id; in >> id; w7_preorder.insert(id); }
	else if (f == "ign") { Int id; in >> id; w7_saved_ignored.insert(id); }
	else if (f == "ignn") { std::string n; in >> n; w7_ignored_names.insert(AsciiString(w7_name(n).c_str())); }
	else if (f == "col") { int i, v; in >> i >> v; GameSpyColor[i] = v; }
	else if (f == "lbp") { int a = u4_li(in); listboxLobbyPlayers = u4_win(a); }
	else if (f == "popgr") { int a = u4_li(in); w7_in_menu = true; populateGroupRoomListbox(u4_win(a)); w7_in_menu = false; }
	else if (f == "poppl") { w7_in_menu = true; PopulateLobbyPlayerListbox(); w7_in_menu = false; }
	else if (f == "ins") { Color c = u4_li(in); PlayerInfo p = w7_read_player(in); w7_in_menu = true; Int r = insertPlayerInListbox(p, c); w7_in_menu = false; g_log << "inserted " << r << "\n"; }
	else if (f == "tip") { int a = u4_li(in); unsigned m = 0; in >> m; w7_in_menu = true; playerTooltip(u4_win(a), NULL, m); w7_in_menu = false; }
	else if (f == "la") { int a = u4_li(in); std::string t; in >> t; int c = u4_li(in), r = u4_li(in), k = u4_li(in); Int ret = GadgetListBoxAddEntryText(u4_win(a), u4_text(t), c, r, k); g_log << "added " << ret << "\n"; }
	else if (f == "lsel") { int a = u4_li(in); int n = u4_li(in); Int idx[64]; for (int i = 0; i < n; ++i) idx[i] = u4_li(in); GadgetListBoxSetSelected(u4_win(a), idx, n); }
	else if (f == "ltop") { int a = u4_li(in); int p = u4_li(in); GadgetListBoxSetTopVisibleEntry(u4_win(a), p); }
	else if (f == "lone") { int a = u4_li(in); wm->winSetLoneWindow(u4_win(a)); }
	else if (f == "lres") { int a = u4_li(in); GadgetListBoxReset(u4_win(a)); }
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	o << "w7 lbp " << widx(listboxLobbyPlayers) << "\n";
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i] || !BitTest(g_windows[i]->winGetInstanceData()->getStyle(), GWS_SCROLL_LISTBOX)) continue;
		ListboxData *l = (ListboxData *)g_windows[i]->winGetUserData();
		if (!l) continue;
		o << "w7 items " << i;
		for (Int r = 0; r < l->endPos; ++r) o << " " << (unsigned)(uintptr_t)GadgetListBoxGetItemData(g_windows[i], r, 0);
		o << "\n";
	}
}
#include "ui4_main.h"
