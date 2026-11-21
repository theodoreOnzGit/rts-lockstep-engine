/* The reference PROGRAM for src/drafts/ui/popup_ladder_select.rs (#215, group ui5, upstream commit 0a05454d): upstream's PopupLadderSelect.cpp on upstream's OWN window manager and
** gadgets (see differential/wrap_draft_ui5_ndc.cpp and wrap_draft_ui4_disconnect.cpp for how the program is built). The path is `UI5_LADDER_EXE`.
**
** Upstream text, cut VERBATIM by build.rs `drafts_ui5` (anchors asserted once): ladder_select.inc, GUICallbacks/Menus/PopupLadderSelect.cpp from `static NameKeyType parentID` to the end,
** FOUR edits, the four `line.format(fmt, args)` calls of updateLadderDetails (`%ls` / `%d` through the shim's glibc vswprintf would read 32-bit units for the 16-bit WideChar):
** `ui5_fmt_name`, `ui5_fmt_text`, `ui5_fmt_int` (below) join the format and the arguments with `|` so that every argument is observed; the real formatting is MSVC's _vsnwprintf, not tested.
**
** Stand-ins of this file (network, INI and game-data layers): TheLadderList (findLadderByIndex), the LadderInfo of LadderDefs.h, the staging room list (compiled, not run: see below),
** EncryptString (`reverse(s) + "#"`: only equality with the ladder's stored password matters), TheMapCache->findMap (the case's `@ ll map`), GameSpyCloseOverlay (logged), and
** the six functions of the other menus' files that this file declares and calls (WOLQuickMatchMenu.cpp: PopulateQMLadderComboBox, PopulateQMLadderListBox, HandleQMLadderSelection;
** PopupHostGame.cpp: PopulateCustomLadderComboBox, PopulateCustomLadderListBox, HandleCustomLadderSelection, CustomMatchHideHostPopup): each logs its call, and PopulateCustomLadderListBox
** fills the list box from the case's rows (`@ ll rows`), through the real gadget functions. TheGameText: ui4_host.h's (`T:` + the label).
**
** NOT run: RCGameDetailsMenuInit / RCGameDetailsMenuSystem / closeRightClickMenu (they are compiled, with an empty staging room list): they need layouts made from WND files, the lone window
** and the staging rooms of the GameSpy layer; see the draft's list of what is left untested.
**
** Operations (besides ui4_main.h's): `@ ll init`, `sel <win> <pos>` (GadgetListBoxSetSelected), the case's state: `ladder <index> <hexname> <ppt> <hexloc> <hexurl> <hexdesc> <crypted|-> <minw> <maxw> <rf> <rm> <factions,or-> <maps,or->`,
** `map <name> <hexdisplay>`, `rows <n> <id>... <selected>`, `ldcb` (ladderSelectedCallback), `lbdata <win> <row>`, `mode` (the dump has the mode). */
#include "ui4_wm_base.inc"
#include "ui4_host.h"
#include "GameClient/KeyDefs.h"
#include <vector>
#include <list>

enum { GSOVERLAY_LADDERSELECT = 3 };
static void GameSpyCloseOverlay(int o) { g_log << "overlay close " << o << "\n"; }
#define NAMEKEY(name) TheNameKeyGenerator->nameToKey(name)

struct LadderInfo
{
	Int index;
	UnicodeString name, location, homepageURL, description;
	Int playersPerTeam, minWins, maxWins;
	AsciiString cryptedPassword;
	Bool randomFactions, randomMaps;
	std::list<AsciiString> validFactions, validMaps;
};
typedef std::list<AsciiString> AsciiStringList;
typedef std::list<AsciiString>::iterator AsciiStringListIterator;
static std::vector<LadderInfo *> u5_ladders;
class LadderList
{
public:
	const LadderInfo *findLadderByIndex(Int index)
	{
		if (index == 0) return NULL;
		for (size_t i = 0; i < u5_ladders.size(); ++i)
			if (u5_ladders[i]->index == index) return u5_ladders[i];
		return NULL;
	}
	const LadderInfo *findLadder(const AsciiString &, UnsignedShort) { return NULL; }
};
static LadderList u5_ladder_list;
static LadderList *TheLadderList = &u5_ladder_list;

/* the staging rooms (RCGameDetailsMenuSystem, compiled only) */
class GameSpyStagingRoom
{
public:
	AsciiString getLadderIP() { return AsciiString(""); }
	UnsignedShort getLadderPort() { return 0; }
};
typedef std::map<Int, GameSpyStagingRoom *> StagingRoomMap;
class U5GameSpyInfo
{
public:
	StagingRoomMap m_rooms;
	StagingRoomMap *getStagingRoomList() { return &m_rooms; }
};
static U5GameSpyInfo u5_gsi;
static U5GameSpyInfo *TheGameSpyInfo = &u5_gsi;

static AsciiString EncryptString(const char *s)
{
	std::string r(s);
	std::string t(r.rbegin(), r.rend());
	return AsciiString((t + "#").c_str());
}

struct MapMetaData
{
	UnicodeString m_displayName;
};
static std::map<std::string, MapMetaData> u5_maps;
class MapCache
{
public:
	const MapMetaData *findMap(const AsciiString &name)
	{
		std::map<std::string, MapMetaData>::iterator it = u5_maps.find(name.str());
		return it == u5_maps.end() ? NULL : &it->second;
	}
};
static MapCache u5_map_cache;
static MapCache *TheMapCache = &u5_map_cache;

/* ---- the formats (see the header comment) */
static void ui5_fmt_name(UnicodeString &out, UnicodeString fmt, const UnicodeString &name, Int a, Int b)
{
	out = fmt;
	out.concat(L"|");
	out.concat(name);
	char buf[64];
	snprintf(buf, sizeof buf, "|%d|%d", (int)a, (int)b);
	for (const char *c = buf; *c; ++c) out.concat((WideChar)*c);
}
static void ui5_fmt_text(UnicodeString &out, UnicodeString fmt, const UnicodeString &t)
{
	out = fmt;
	out.concat(L"|");
	out.concat(t);
}
static void ui5_fmt_int(UnicodeString &out, UnicodeString fmt, Int v)
{
	out = fmt;
	char buf[32];
	snprintf(buf, sizeof buf, "|%d", (int)v);
	for (const char *c = buf; *c; ++c) out.concat((WideChar)*c);
}

/* ---- the functions of the other menus' files */
struct U5Row
{
	int id;
	std::string text;
};
static std::vector<int> u5_rows;
static int u5_rows_selected = 0;
static GameWindow *listboxLadderSelect_fwd = NULL;
void PopulateQMLadderComboBox(void) { g_log << "pop qm combo\n"; }
void PopulateCustomLadderComboBox(void) { g_log << "pop custom combo\n"; }
void PopulateQMLadderListBox(GameWindow *win) { g_log << "pop qm list " << widx(win) << "\n"; }
void PopulateCustomLadderListBox(GameWindow *win)
{
	g_log << "pop custom list " << widx(win) << "\n";
	if (!win) return;
	GadgetListBoxReset(win);
	for (size_t i = 0; i < u5_rows.size(); ++i)
	{
		UnicodeString t;
		t.concat((WideChar)('A' + (int)i));
		Int index = GadgetListBoxAddEntryText(win, t, (Color)0xFFFFFFFFu, -1);
		GadgetListBoxSetItemData(win, (void *)(intptr_t)u5_rows[i], index);
	}
	GadgetListBoxSetSelected(win, u5_rows_selected);
}
void HandleQMLadderSelection(Int id) { g_log << "handle qm " << id << "\n"; }
void HandleCustomLadderSelection(Int id) { g_log << "handle custom " << id << "\n"; }
void CustomMatchHideHostPopup(Bool hide) { g_log << "hide host popup " << (int)hide << "\n"; }

#include "ladder_select.inc"

static const U4Menu u4_menus[] = {
	{ "ladder", PopupLadderSelectSystem, PopupLadderSelectInput },
};
static void u4_menu_setup(void) {}
static std::list<AsciiString> u5_split(const std::string &s)
{
	std::list<AsciiString> r;
	if (s == "-") return r;
	size_t p = 0;
	while (p <= s.size())
	{
		size_t c = s.find(',', p);
		if (c == std::string::npos) c = s.size();
		r.push_back(AsciiString(s.substr(p, c - p).c_str()));
		p = c + 1;
	}
	return r;
}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "ll") return false;
	std::string f;
	in >> f;
	if (f == "init") PopupLadderSelectInit(NULL, NULL);
	else if (f == "ldcb") ladderSelectedCallback();
	else if (f == "lbdata") { int a = u4_li(in); int r = u4_li(in); g_log << "lbdata " << (unsigned)(uintptr_t)GadgetListBoxGetItemData(u4_win(a), r) << "\n"; }
	else if (f == "sel") { int a = u4_li(in); int p = u4_li(in); GadgetListBoxSetSelected(u4_win(a), p); }
	else if (f == "ladder")
	{
		LadderInfo *l = new LadderInfo;
		std::string n, loc, url, desc, crypted, fac, maps;
		int rf, rm;
		in >> l->index >> n >> l->playersPerTeam >> loc >> url >> desc >> crypted >> l->minWins >> l->maxWins >> rf >> rm >> fac >> maps;
		l->name = u4_text(n);
		l->location = u4_text(loc);
		l->homepageURL = u4_text(url);
		l->description = u4_text(desc);
		l->cryptedPassword = crypted == "-" ? AsciiString("") : AsciiString(crypted.c_str());
		l->randomFactions = rf != 0;
		l->randomMaps = rm != 0;
		l->validFactions = u5_split(fac);
		l->validMaps = u5_split(maps);
		u5_ladders.push_back(l);
	}
	else if (f == "map") { std::string n, d; in >> n >> d; u5_maps[n].m_displayName = u4_text(d); }
	else if (f == "rows")
	{
		int n = u4_li(in);
		u5_rows.clear();
		for (int i = 0; i < n; ++i) u5_rows.push_back(u4_li(in));
		u5_rows_selected = u4_li(in);
	}
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	o << "ll ids " << parentID << " " << listboxLadderSelectID << " " << listboxLadderDetailsID << " " << staticTextLadderNameID << " " << buttonOkID << " " << buttonCancelID << " " << passwordParentID << " "
	  << buttonPasswordOkID << " " << buttonPasswordCancelID << " " << textEntryPasswordID << " " << badPasswordParentID << " " << buttonBadPasswordOkID << " mode " << (int)s_currentMode << " index " << ladderIndex << "\n";
	o << "ll wins " << widx(parent) << " " << widx(listboxLadderSelect) << " " << widx(listboxLadderDetails) << " " << widx(staticTextLadderName) << " " << widx(buttonOk) << " " << widx(buttonCancel) << " "
	  << widx(passwordParent) << " " << widx(textEntryPassword) << " " << widx(badPasswordParent) << "\n";
}
#include "ui4_main.h"
