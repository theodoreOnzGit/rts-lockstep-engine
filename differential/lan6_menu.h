/* The shared stand-ins of the two map-select reference PROGRAMS of group lan6 (#215): differential/wrap_draft_lan6_lan.cpp (LanMapSelectMenu.cpp) and
** differential/wrap_draft_lan6_wol.cpp (WOLMapSelectMenu.cpp), included by each right after `ui4_wm_base.inc` and `ui4_host.h` (group ui4's window manager and gadgets,
** built by build.rs `drafts_lan6` the way `drafts_ui5` builds its programs). Every name defined here is `static` (each program is its own executable).
**
** Upstream text cut VERBATIM by build.rs (each anchor asserted once, no edit), included here:
**   showunder.inc   GameNetwork/GUIUtil.cpp, `ShowUnderlyingGUIElements` (to `PopulateColorComboBox`);
**   findmap.inc     GameClient/MapUtil.cpp, `MapCache::findMap` (to the `copyFromBigToDir` comment).
**
** Stand-ins (what the menus call outside themselves). Each logs its call on both sides (the Rust test host writes the same line), and the state they hold is in the dump:
**   - TheMapCache: `std::map<AsciiString, MapMetaData>` with the three fields the menus read (`m_isOfficial`, `m_CRC`, `m_filesize`; MapUtil.h's has more); `findMap` is upstream's
**     (above), `find` / `end` the map's, `updateCache()` logs `mapcache update`. File I/O: the real one reads the map files.
**   - populateMapListbox(list, useSystemMaps, isMultiplayer, mapToSelect): logs `populate <list window> sys <b> mp <b> map <hex>` and fills nothing. It IS translated and verified
**     on its own (src/client/map_util.rs, tests/differential_map_list.rs against the C++ cut verbatim); here only the menus' calls to it are compared.
**   - positionStartSpots(map, buttons, window): logs `spots map <hex> btns <8 window indices> win <window>`. It reads the map's waypoints from the cache and moves the buttons
**     (SkirmishGameOptionsMenu.cpp; map file I/O).
**   - getMapPreviewImage(map): logs `preview map <hex>`; answers an Image named `P_<map>` for the maps the case lists (`@ ms image`), else NULL (the real one loads a .tga).
**   - the game (TheLAN->GetMyGame(), TheGameSpyGame): the map name, `setMap`, slot 0's `setMapAvailability`, `setMapCRC`, `setMapSize`, `resetStartSpots`, `adjustSlotsForMap`,
**     `resetAccepted` log `game ...`. (The real classes are LANGameInfo / GameSpyGameInfo, verified separately: tests/differential_draft_lanhandlers.rs and others.)
**   - the preferences (UserPreferences.h's std::map<AsciiString, AsciiString>): loaded from, and written to, an in-memory ini (`@ ms ini`); `write()` logs `prefs write`.
**     `usesSystemMapDir` (and the WOL ones) are upstream's, cut verbatim by the wrappers. File I/O: nothing else.
**   - TheShell::shutdownComplete(layout): logs `shell complete <layout>` and does what Shell::doPush does for the pending push of that layout (detaches the IME, brings the layout
**     forward), as group ui5's network_direct_connect program does.
**
** Operations (`@ ms ...`, besides ui4_main.h's): `mapcache <file> <official> <crc> <size>`, `gamemap <file|->`, `ini <value|->`, `image <lower map>`, `row <list> <hex text> <file|->`
** (a list box row, its item data the file name, a pointer to it), `layout <n> <win>...` (the menu's layout), `msl <0|1>` (the map select layout global: the menu's layout, or NULL),
** `stats <b>` (WOL), and each program's own. */
#include <set>
#include <vector>
#include <map>
#include <wctype.h>
#include "GameClient/KeyDefs.h"
#include "GameClient/GadgetListBox.h"
#include "GameClient/GadgetRadioButton.h"

#define stricmp strcasecmp
#ifndef NAMEKEY
#define NAMEKEY(x) TheNameKeyGenerator->nameToKey(x)
#endif
enum { MAX_SLOTS = 8 };

static std::string l6_hex(const AsciiString &a)
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

/* ---- the map cache */
struct MapMetaData
{
	Bool m_isOfficial;
	UnsignedInt m_CRC;
	UnsignedInt m_filesize;
};
class MapCache : public std::map<AsciiString, MapMetaData>
{
public:
	const MapMetaData *findMap(AsciiString mapName);
	void updateCache(void) { g_log << "mapcache update\n"; }
};
static MapCache l6_cache;
static MapCache *TheMapCache = &l6_cache;
#include "findmap.inc"

/* ---- the game */
static AsciiString l6_game_map;
static UnsignedInt l6_game_crc = 0, l6_game_size = 0;
static int l6_slot0_avail = -1;
class L6Slot
{
public:
	void setMapAvailability(Bool b) { l6_slot0_avail = b ? 1 : 0; g_log << "game slot0 avail " << (int)(b ? 1 : 0) << "\n"; }
};
static L6Slot l6_slot;
class L6Game
{
public:
	AsciiString getMap(void) { return l6_game_map; }
	void setMap(AsciiString m) { l6_game_map = m; g_log << "game setmap " << l6_hex(m) << "\n"; }
	void setMapCRC(UnsignedInt c) { l6_game_crc = c; g_log << "game crc " << c << "\n"; }
	void setMapSize(UnsignedInt s) { l6_game_size = s; g_log << "game size " << s << "\n"; }
	void resetStartSpots(void) { g_log << "game reset spots\n"; }
	void adjustSlotsForMap(void) { g_log << "game adjust slots\n"; }
	void resetAccepted(void) { g_log << "game reset accepted\n"; }
	L6Slot *getSlot(Int) { return &l6_slot; }
	L6Slot *getGameSpySlot(Int) { return &l6_slot; }
};
static L6Game l6_game;

/* ---- the map list, the preview, the start spots */
static Int populateMapListbox(GameWindow *listbox, Bool useSystemMaps, Bool isMultiplayer, AsciiString mapToSelect = AsciiString::TheEmptyString)
{
	g_log << "populate " << widx(listbox) << " sys " << (int)(useSystemMaps ? 1 : 0) << " mp " << (int)(isMultiplayer ? 1 : 0) << " map " << l6_hex(mapToSelect) << "\n";
	return 0;
}
static std::set<std::string> l6_previews;
static std::map<std::string, Image *> l6_images;
static Image *getMapPreviewImage(AsciiString mapName)
{
	g_log << "preview map " << l6_hex(mapName) << "\n";
	std::string k = mapName.str();
	if (l6_previews.find(k) == l6_previews.end()) return NULL;
	Image *&im = l6_images[k];
	if (!im)
	{
		im = newInstance(Image);
		im->setName(AsciiString((std::string("P_") + k).c_str()));
	}
	return im;
}
void positionStartSpots(AsciiString mapName, GameWindow *buttonMapStartPositions[], GameWindow *mapWindow)
{
	g_log << "spots map " << l6_hex(mapName) << " btns";
	for (int i = 0; i < MAX_SLOTS; ++i) g_log << " " << widx(buttonMapStartPositions[i]);
	g_log << " win " << widx(mapWindow) << "\n";
}

/* ---- the preferences */
typedef std::map<AsciiString, AsciiString> PreferenceMap;
typedef PreferenceMap OptionPreferences;
static std::map<std::string, std::string> l6_ini;
static void l6_prefs_load(PreferenceMap &m)
{
	for (std::map<std::string, std::string>::iterator it = l6_ini.begin(); it != l6_ini.end(); ++it) m[AsciiString(it->first.c_str())] = AsciiString(it->second.c_str());
}
static void l6_prefs_write(const PreferenceMap &m)
{
	l6_ini.clear();
	for (PreferenceMap::const_iterator it = m.begin(); it != m.end(); ++it) l6_ini[it->first.str()] = it->second.str();
	g_log << "prefs write\n";
}

/* ---- the shell */
class Shell
{
public:
	void shutdownComplete(WindowLayout *l, Bool impendingPush = FALSE)
	{
		g_log << "shell complete " << u4_lay_index(l) << "\n";
		TheIMEManager->detatch();
		l->bringForward();
	}
};
static Shell l6_shell;
static Shell *TheShell = &l6_shell;

/* ---- ShowUnderlyingGUIElements (upstream's) */
#include "showunder.inc"

static WindowLayout *l6_layout = NULL;
static bool l6_stats = false;

/* the operations the two programs share; false when `f` is none of them */
static bool l6_common_op(const std::string &f, std::istringstream &in)
{
	if (f == "mapcache")
	{
		std::string file;
		int official;
		unsigned crc, size;
		in >> file >> official >> crc >> size;
		MapMetaData m;
		m.m_isOfficial = official != 0;
		m.m_CRC = crc;
		m.m_filesize = size;
		l6_cache[AsciiString(file.c_str())] = m;
	}
	else if (f == "gamemap")
	{
		std::string t;
		in >> t;
		l6_game_map = t == "-" ? AsciiString("") : AsciiString(t.c_str());
	}
	else if (f == "ini")
	{
		std::string t;
		in >> t;
		if (t == "-") l6_ini.erase("UseSystemMapDir"); else l6_ini["UseSystemMapDir"] = t;
	}
	else if (f == "image") { std::string t; in >> t; l6_previews.insert(t); }
	else if (f == "stats") l6_stats = u4_li(in) != 0;
	else if (f == "row")
	{
		int list = u4_li(in);
		std::string t, file;
		in >> t >> file;
		GameWindow *w = u4_win(list);
		Int row = GadgetListBoxAddEntryText(w, u4_text(t), 0xFFFFFFFF, -1, 0, FALSE);
		if (row >= 0 && file != "-") GadgetListBoxSetItemData(w, (void *)strdup(file.c_str()), row, 0);
	}
	else if (f == "layout")
	{
		int n = u4_li(in);
		u4_layout_members.clear();
		for (int i = 0; i < n; ++i) u4_layout_members.push_back(u4_li(in));
		l6_layout = TheWindowManager->winCreateLayout(AsciiString("MapSelect.wnd"));
	}
	else return false;
	return true;
}

/* the state the two programs share, in the format tests/lan6_common/mod.rs writes */
static void l6_dump_common(std::ostringstream &o)
{
	o << "ms game map " << l6_hex(l6_game_map) << " crc " << l6_game_crc << " size " << l6_game_size << " avail " << l6_slot0_avail << "\n";
	o << "ms ini";
	for (std::map<std::string, std::string>::iterator it = l6_ini.begin(); it != l6_ini.end(); ++it)
	{
		o << " " << it->first << "=";
		for (size_t i = 0; i < it->second.size(); ++i) { char b[8]; snprintf(b, sizeof b, "%02x", (unsigned)(unsigned char)it->second[i]); o << b; }
	}
	o << "\n";
	o << "ms img";
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i]) continue;
		const Image *im = g_windows[i]->winGetEnabledImage(0);
		if (im && strncmp(im->getName().str(), "P_", 2) == 0) o << " " << i << ":" << im->getName().str();
	}
	o << "\nms user";
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i]) continue;
		void *p = g_windows[i]->winGetUserData();
		for (MapCache::iterator it = l6_cache.begin(); p && it != l6_cache.end(); ++it)
			if (&it->second == p) o << " " << i << ":" << it->first.str();
	}
	o << "\nms layout " << u4_lay_index(l6_layout) << "\n";
}
