/* The reference PROGRAM for src/drafts/ui/game_info_window.rs (#215, group ui4, upstream commit 0a05454d): upstream's GameInfoWindow.cpp on upstream's OWN window
** manager and gadgets (see differential/wrap_draft_ui4_disconnect.cpp for how the programs are built; the shared stand-ins are in ui4_host.h and ui4_main.h).
** The path is `UI4_GAMEINFO_EXE`.
**
** Upstream text, cut VERBATIM by build.rs (anchor asserted once): game_info_window.inc, GUICallbacks/Menus/GameInfoWindow.cpp from `static GameWindow *parent`
** to the end, ONE edit: `UnicodeString.TheEmptyString` (MSVC 6 accepts `.` for `::`) is spelled `UnicodeString::TheEmptyString`. The base's image collection
** (wrap_wm.cpp's `findImageByName`: names that start with `Img`) also finds the names that start with `Gameinfo` (one asserted edit of the generated base).
**
** Stand-ins of this file (what the cut calls outside itself):
**   - GameInfo / LANGameInfo / GameSlot (GameInfo.h, LANAPI.h): the case's map, first player's name and eight slots (`@ gi map`, `@ gi name0`, `@ gi slot`);
**     `getSlot(i)`, `getMap()` (a reference: see there), `getPlayerName(0)`, and the slot's isOccupied, getColor, isAI, getState, isHuman, getName, getPlayerTemplate.
**   - TheMapCache (MapUtil.h's MapCache, a std::map<AsciiString, MapMetaData>): the case's entries (`@ gi cache`), each with a display name.
**   - TheMultiplayerSettings (getNumColors, getColor(c)->getColor()): the case's colours (`@ gi colors`).
**   - ThePlayerTemplateStore (getPlayerTemplateCount, getNthPlayerTemplate(n)->getSideIconImage()): the case's count (`@ gi templates`); template n's icon is the
**     image `ImgSide<n>`, NULL when n % 5 == 4.
**   - the layout's init callback (`runInit`): `layout init` is logged and GameInfoWindowInit run (the shell's layout init of the real game).
**
** Operations (besides ui4_main.h's): `@ gi create <win>` (CreateLANGameInfoWindow of that window, -1: NULL), `destroy`, `init` (GameInfoWindowInit), `hide <b>`,
** `refresh <hasInfo> <gameName>`, and the data above: `map <hex bytes>`, `name0 <text>`, `slot <i> <occ> <ai> <human> <state 0 easy 1 med 2 brutal> <color> <template> <name>`,
** `cache <hex bytes of the key> <text>`, `colors <n> <c>...`, `templates <n>`. The dump adds the file's statics. */
#include "ui4_wm_base.inc"
#include "ui4_host.h"
#include <map>

enum { MAX_SLOTS = 8 };
enum { PLAYERTEMPLATE_OBSERVER = -2 };
enum SlotState { SLOT_EASY_AI, SLOT_MED_AI, SLOT_BRUTAL_AI, SLOT_PLAYER };
class GameSlot
{
public:
	Bool occ, ai, human;
	SlotState state;
	Int color, tmpl;
	UnicodeString name;
	Bool isOccupied() { return occ; }
	Bool isAI() { return ai; }
	Bool isHuman() { return human; }
	SlotState getState() { return state; }
	Int getColor() { return color; }
	Int getPlayerTemplate() { return tmpl; }
	UnicodeString getName() { return name; }
};
class GameInfo
{
public:
	GameSlot slots[MAX_SLOTS];
	AsciiString map;
	GameSlot *getSlot(Int i) { return (i >= 0 && i < MAX_SLOTS) ? &slots[i] : NULL; }
	/* a reference, not a copy: upstream's AsciiString shares its buffer, so `getMap().reverseFind(...)`'s pointer outlives the temporary; the shim's std::string would not */
	AsciiString &getMap() { return map; }
};
class LANGameInfo : public GameInfo
{
public:
	UnicodeString name0;
	UnicodeString getPlayerName(Int) { return name0; }
};
static LANGameInfo u4_game_info;

struct MapMetaData { UnicodeString m_displayName; };
class U4MapCache : public std::map<AsciiString, MapMetaData> {};
static U4MapCache u4_map_cache;
static U4MapCache *TheMapCache = &u4_map_cache;

class MultiplayerColorDefinition { public: Color c; Color getColor() { return c; } };
class U4MultiplayerSettings
{
public:
	std::vector<MultiplayerColorDefinition> colors;
	Int getNumColors() { return (Int)colors.size(); }
	MultiplayerColorDefinition *getColor(Int i) { return &colors[i]; }
};
static U4MultiplayerSettings u4_settings;
static U4MultiplayerSettings *TheMultiplayerSettings = &u4_settings;

class PlayerTemplate
{
public:
	Int n;
	const Image *getSideIconImage()
	{
		if (n % 5 == 4) return NULL;
		char b[32];
		sprintf(b, "ImgSide%d", n);
		return TheMappedImageCollection->findImageByName(AsciiString(b));
	}
};
class U4PlayerTemplateStore
{
public:
	Int count;
	PlayerTemplate t;
	Int getPlayerTemplateCount() { return count; }
	const PlayerTemplate *getNthPlayerTemplate(Int n) { t.n = n; return &t; }
};
static U4PlayerTemplateStore u4_templates;
static U4PlayerTemplateStore *ThePlayerTemplateStore = &u4_templates;

#include "game_info_window.inc"

static void u4_gi_layout_init(WindowLayout *layout, void *userData)
{
	g_log << "layout init\n";
	GameInfoWindowInit(layout, userData);
}
static const U4Menu u4_menus[] = {
	{ "gameinfo", GameInfoWindowSystem, GameInfoWindowSystem },
};
static void u4_menu_setup(void) { u4_layout_init = u4_gi_layout_init; }
static AsciiString u4_bytes(const std::string &t)
{
	std::string r;
	size_t p = 0;
	while (p < t.size())
	{
		size_t c = t.find(',', p);
		if (c == std::string::npos) c = t.size();
		r += (char)strtoul(t.substr(p, c - p).c_str(), NULL, 16);
		p = c + 1;
	}
	AsciiString a;
	a.set(r.c_str());
	return a;
}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "gi") return false;
	std::string f, t;
	in >> f;
	if (f == "create") { int w = u4_li(in); CreateLANGameInfoWindow(u4_win(w)); }
	else if (f == "destroy") DestroyGameInfoWindow();
	else if (f == "init") GameInfoWindowInit(NULL, NULL);
	else if (f == "hide") HideGameInfoWindow(u4_li(in) != 0);
	else if (f == "refresh") { int has; in >> has >> t; RefreshGameInfoWindow(has ? &u4_game_info : NULL, u4_text(t)); }
	else if (f == "map") { in >> t; u4_game_info.map = u4_bytes(t); }
	else if (f == "name0") { in >> t; u4_game_info.name0 = u4_text(t); }
	else if (f == "slot")
	{
		int i, occ, ai, human, st, color, tmpl;
		std::string n;
		in >> i >> occ >> ai >> human >> st >> color >> tmpl >> n;
		GameSlot &s = u4_game_info.slots[i];
		s.occ = occ != 0; s.ai = ai != 0; s.human = human != 0; s.state = (SlotState)st; s.color = color; s.tmpl = tmpl; s.name = u4_text(n);
	}
	else if (f == "cache") { std::string k, v; in >> k >> v; u4_map_cache[u4_bytes(k)].m_displayName = u4_text(v); }
	else if (f == "colors") { int n = u4_li(in); u4_settings.colors.clear(); for (int i = 0; i < n; ++i) { MultiplayerColorDefinition d; d.c = (Color)u4_li(in); u4_settings.colors.push_back(d); } }
	else if (f == "templates") u4_templates.count = u4_li(in);
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	o << "gi parent " << widx(parent) << " gn " << widx(staticTextGameName) << " mn " << widx(staticTextMapName) << " list " << widx(listBoxPlayers) << " crates " << widx(winCrates) << " super " << widx(winSuperWeapons) << " ffa " << widx(winFreeForAll)
	  << " layout " << u4_lay_index(gameInfoWindowLayout) << " ids " << (parentID != NAMEKEY_INVALID) << (staticTextGameNameID != NAMEKEY_INVALID) << (staticTextMapNameID != NAMEKEY_INVALID) << (listBoxPlayersID != NAMEKEY_INVALID) << (winCratesID != NAMEKEY_INVALID) << (winSuperWeaponsID != NAMEKEY_INVALID) << (winFreeForAllID != NAMEKEY_INVALID) << "\n";
}
#include "ui4_main.h"
