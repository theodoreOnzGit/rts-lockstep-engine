/* Differential-test wrapper for the skirmish battle honours (GitHub #157, #92; upstream commit 0a05454d). The pieces
** build.rs cuts verbatim into OUT_DIR/battle_honors_gen/battle_honors_cut.inc:
**   GeneralsMD/Code/GameEngine/Include/Common/SkirmishBattleHonors.h: class SkirmishBattleHonors (the declarations
**     of the free functions after it are left out: their definitions repeat the default arguments);
**   GeneralsMD/Code/GameEngine/Source/Common/SkirmishBattleHonors.cpp: the constructor to the end of the file;
**   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/PopupPlayerInfo.cpp: BattleHonorTooltip,
**     rowsToSkip, ResetBattleHonorInsertion, InsertBattleHonor;
**   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp:
**     populateSkirmishBattleHonors (to the end of the file);
**   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ScoreScreen.cpp: isSlotLocalAlly and
**     updateSkirmishBattleHonors, updateChallengeMedals, and populatePlayerInfo's skirmish block (from its
**     `if ( screenType == SCORESCREEN_SKIRMISH && player->isLocalPlayer() )` to the internet block), the last in
**     a function of this file's (skirmishTail);
** with the REAL UserPreferences (OUT_DIR/user_prefs_gen/user_prefs_cut.inc, as wrap_user_prefs.cpp), the REAL
** AsciiString.h and AsciiString.cpp (classes renamed by build.rs) and the C library's fopen / fgets / fprintf on a
** real file in the folder the test gives.
**
** Everything below is a STAND-IN, written for this test: TheGlobalData's user data path; GameDifficulty and the slot
** states; NAMEKEY and TheWindowManager->winGetWindowFromId (the case's windows: the list and the four numbers);
** GadgetStaticTextSetText (recorded); the list box (GadgetListBoxReset, AddEntryImage, SetItemData, GetItemData,
** GetNumColumns, GetEntryBasedOnXY: a table of rows of four cells, a row past the end appended, as the Rust gadget
** with its insert position does; GetEntryBasedOnXY answers the case's cell); TheDisplay's size;
** TheMappedImageCollection (the case's images); TheMapCache (the case's maps); GetUnsignedIntFromRegistry (the case's
** pre-order); TheMouse->setCursorTooltip and TheGameText->fetch (the label recorded); and for the score screen's
** block: ThePlayerList's local player, its ScoreKeeper (the case's counts by template name and kind), TheThingFactory
** (a template per name), TheGameLogic's frame, TheGameInfo (the case's slots, local slot, map, and the Rust side's
** isSkirmish / isSandbox answers), TheVictoryConditions.
**
** Entry point (extern "C"):
**   ref_battle_honors_run(in, dir, out, cap)  runs one case (tests/differential_battle_honors.rs's text) and writes
**                                             the dump; its length. */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include <map>
#include <set>
#include "GameClient/Color.h"

const UnicodeString UnicodeString::TheEmptyString;
DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theHonorsAllocator;

template <typename T> static T max(T a, T b) { return a > b ? a : b; }
template <typename T> static T min(T a, T b) { return a < b ? a : b; }

namespace {
struct BhGlobalData
{
	AsciiString m_userData;
	AsciiString getPath_UserData() const { return m_userData; }
};
BhGlobalData *TheGlobalData = 0;
}

#include "user_prefs_cut.inc"

/* GameDifficulty, the slot states, MAX_SLOTS, MAX_GLOBAL_GENERAL_TYPES, BattleHonors.h's enums (the real header) */
enum GameDifficulty { DIFFICULTY_EASY, DIFFICULTY_NORMAL, DIFFICULTY_HARD, DIFFICULTY_COUNT };
enum SlotState { SLOT_OPEN, SLOT_CLOSED, SLOT_EASY_AI, SLOT_MED_AI, SLOT_BRUTAL_AI, SLOT_PLAYER };
enum { MAX_SLOTS = 8, MAX_GLOBAL_GENERAL_TYPES = 9, LOGICFRAMES_PER_SECOND = 30 };
enum ScoreScreenType { SCORESCREEN_SINGLEPLAYER = 0, SCORESCREEN_SKIRMISH, SCORESCREEN_LAN, SCORESCREEN_INTERNET, SCORESCREEN_REPLAY };
#include "Common/BattleHonors.h"
#define LOLONGTOSHORT(a)  ((a) & 0x0000FFFF)
#define HILONGTOSHORT(b)  (((b) & 0xFFFF0000) >> 16)
#ifndef DEBUG_LOG
#define DEBUG_LOG(x)
#endif

namespace {
std::string g_out;
void outf(const char *fmt, ...)
{
	char b[1024];
	va_list a;
	va_start(a, fmt);
	vsnprintf(b, sizeof b, fmt, a);
	va_end(a);
	g_out += b;
}
std::string narrow(const UnicodeString &u)
{
	std::string s;
	for (int i = 0; i < u.getLength(); ++i) s += (char)u.getCharAt(i);
	return s;
}
}

/* ---- windows ---- */
class WinInstanceData {};
class GameWindow;
typedef void (*GameWinTooltipFunc)(GameWindow *, WinInstanceData *, UnsignedInt);
class GameWindow
{
public:
	std::string name;
	GameWinTooltipFunc tooltip;
	GameWindow() : tooltip(0) {}
	void winSetTooltipFunc(GameWinTooltipFunc f) { tooltip = f; }
};
namespace {
std::map<std::string, GameWindow> g_windows; /* by name, the case's */
std::vector<std::string> g_keys;
}
static Int NAMEKEY(const char *n)
{
	for (size_t i = 0; i < g_keys.size(); ++i) if (g_keys[i] == n) return (Int)i + 1;
	g_keys.push_back(n);
	return (Int)g_keys.size();
}
class BhWindowManager
{
public:
	GameWindow *winGetWindowFromId(GameWindow *, Int id)
	{
		if (id < 1 || id > (Int)g_keys.size()) return NULL;
		std::map<std::string, GameWindow>::iterator it = g_windows.find(g_keys[id - 1]);
		return it == g_windows.end() ? NULL : &it->second;
	}
};
static BhWindowManager g_wm;
static BhWindowManager *TheWindowManager = &g_wm;
static void GadgetStaticTextSetText(GameWindow *w, UnicodeString text) { outf("text %s %s\n", w->name.c_str(), narrow(text).c_str()); }

/* ---- the list box ---- */
namespace {
struct Cell { int kind; std::string image; int w, h, color; long data; };
std::vector<std::vector<Cell> > g_rows;
int g_columns;
int g_tipRow, g_tipCol;
Cell blank() { Cell c; c.kind = 0; c.w = c.h = c.color = 0; c.data = 0; return c; }
}
class Image
{
public:
	std::string name;
};
static void GadgetListBoxReset(GameWindow *) { g_rows.clear(); }
static Int GadgetListBoxGetNumColumns(GameWindow *) { return g_columns; }
static Int GadgetListBoxAddEntryImage(GameWindow *, const Image *image, Int row, Int column, Int hight, Int width, Bool overwrite, Color color = -1)
{
	(void)overwrite;
	if (row == -1 || row >= (Int)g_rows.size()) {
		g_rows.push_back(std::vector<Cell>(g_columns, blank()));
		row = (Int)g_rows.size() - 1;
	}
	if (row < 0 || column < 0 || column >= g_columns) return -1;
	Cell &c = g_rows[row][column];
	c.kind = 2;
	c.image = image ? image->name : std::string("<none>");
	c.w = width;
	c.h = hight;
	c.color = color;
	return row;
}
static void GadgetListBoxSetItemData(GameWindow *, void *data, Int row, Int column = 0)
{
	if (row < 0 || row >= (Int)g_rows.size() || column < 0 || column >= g_columns) return;
	g_rows[row][column].data = (long)data;
}
static void *GadgetListBoxGetItemData(GameWindow *, Int row, Int column = 0)
{
	if (row < 0 || row >= (Int)g_rows.size() || column < 0 || column >= g_columns) return 0;
	return (void *)g_rows[row][column].data;
}
static void GadgetListBoxGetEntryBasedOnXY(GameWindow *, Int x, Int y, Int &row, Int &column)
{
	(void)x; (void)y;
	row = g_tipRow;
	column = g_tipCol;
}

/* ---- the display, the images, the registry, the mouse and the text ---- */
class BhDisplay
{
public:
	UnsignedInt w, h;
	UnsignedInt getWidth() { return w; }
	UnsignedInt getHeight() { return h; }
};
static BhDisplay g_display;
static BhDisplay *TheDisplay = &g_display;
class BhImageCollection
{
public:
	std::map<std::string, Image> images;
	const Image *findImageByName(const char *n)
	{
		std::map<std::string, Image>::iterator it = images.find(n);
		return it == images.end() ? NULL : &it->second;
	}
};
static BhImageCollection g_images;
static BhImageCollection *TheMappedImageCollection = &g_images;
namespace { unsigned g_preorder; }
static Bool GetUnsignedIntFromRegistry(AsciiString, AsciiString, UnsignedInt &val) { val = g_preorder; return TRUE; }
class RGBColor;
class BhMouse
{
public:
	void setCursorTooltip(UnicodeString tooltip, Int delay = -1, const RGBColor *color = NULL, Real width = 1.0f)
	{
		(void)color;
		outf("tooltip %s %d %g\n", narrow(tooltip).c_str(), delay, width);
	}
};
static BhMouse g_mouse;
static BhMouse *TheMouse = &g_mouse;
class BhGameText
{
public:
	UnicodeString fetch(const char *label)
	{
		UnicodeString u;
		for (const char *p = label; *p; ++p) u.push((WideChar)(unsigned char)*p);
		return u;
	}
};
static BhGameText g_text;
static BhGameText *TheGameText = &g_text;

/* ---- the map cache ---- */
struct MapMetaData
{
	Bool m_isOfficial;
	Bool m_isMultiplayer;
	Int m_numPlayers;
};
class MapCache : public std::map<AsciiString, MapMetaData> {};
static MapCache *TheMapCache = NULL;

/* ---- the score screen's game ---- */
class ThingTemplate
{
public:
	std::string name;
};
class BhThingFactory
{
public:
	std::map<std::string, ThingTemplate> t;
	const ThingTemplate *findTemplate(const char *n)
	{
		ThingTemplate &x = t[n];
		x.name = n;
		return &x;
	}
};
static BhThingFactory g_factory;
static BhThingFactory *TheThingFactory = &g_factory;
enum KindOfType { KINDOF_VEHICLE = 9, KINDOF_AIRCRAFT = 10 };
class KindOfMaskType
{
public:
	unsigned bits;
	KindOfMaskType() : bits(0) {}
	void set(KindOfType k) { bits |= 1u << k; }
	void clear() { bits = 0; }
};
namespace {
std::map<std::string, int> g_built;
int g_vehicles, g_aircraft;
unsigned g_frame;
}
class ScoreKeeper
{
public:
	Int getTotalObjectsBuilt(const ThingTemplate *t) { return g_built[t->name]; }
	Int getTotalUnitsBuilt(KindOfMaskType valid, KindOfMaskType invalid)
	{
		(void)invalid;
		return (valid.bits & (1u << KINDOF_VEHICLE)) ? g_vehicles : g_aircraft;
	}
};
class PlayerTemplate
{
public:
	AsciiString side;
	AsciiString getSide() const { return side; }
};
class Player
{
public:
	ScoreKeeper keeper;
	PlayerTemplate tpl;
	AsciiString side;
	Bool active;
	ScoreKeeper *getScoreKeeper() { return &keeper; }
	AsciiString getSide() const { return side; }
	Bool isLocalPlayer() const { return TRUE; }
	Bool isPlayerActive() const { return active; }
	const PlayerTemplate *getPlayerTemplate() const { return &tpl; }
};
static Player g_player;
class BhPlayerList
{
public:
	Player *getLocalPlayer() { return &g_player; }
};
static BhPlayerList g_playerList;
static BhPlayerList *ThePlayerList = &g_playerList;
class BhGameLogic
{
public:
	UnsignedInt getFrame() { return g_frame; }
};
static BhGameLogic g_logic;
static BhGameLogic *TheGameLogic = &g_logic;
class GameSlot
{
public:
	Int state, team;
	Bool isAI() const { return state == SLOT_EASY_AI || state == SLOT_MED_AI || state == SLOT_BRUTAL_AI; }
	SlotState getState() const { return (SlotState)state; }
	Int getTeamNumber() const { return team; }
};
class GameInfo
{
public:
	GameSlot slots[MAX_SLOTS];
	Int local;
	AsciiString map;
	Bool skirmish, sandbox;
	const GameSlot *getConstSlot(Int i) const { return (i < 0 || i >= MAX_SLOTS) ? NULL : &slots[i]; }
	Int getLocalSlotNum() const { return local; }
	AsciiString getMap() const { return map; }
	Bool isSkirmish() const { return skirmish; }
	Bool isSandbox() const { return sandbox; }
};
static GameInfo g_gameInfo;
static GameInfo *TheGameInfo = &g_gameInfo;
class BhVictory
{
public:
	Bool victory, defeat;
	Bool isLocalAlliedVictory() { return victory; }
	Bool isLocalAlliedDefeat() { return defeat; }
};
static BhVictory g_victory;
static BhVictory *TheVictoryConditions = &g_victory;

/* ---- the upstream code ---- */
#include "battle_honors_cut.inc"

namespace {
std::string hex(const std::string &s)
{
	std::string o;
	char b[4];
	for (size_t i = 0; i < s.size(); ++i) { snprintf(b, sizeof b, "%02x", (unsigned char)s[i]); o += b; }
	return o;
}
std::string unhex(const std::string &h)
{
	std::string o;
	for (size_t i = 0; i + 1 < h.size(); i += 2) o += (char)strtol(h.substr(i, 2).c_str(), NULL, 16);
	return o;
}
std::string readFile(const std::string &p)
{
	std::string s;
	FILE *fp = fopen(p.c_str(), "rb");
	if (fp) {
		char buf[4096];
		size_t n;
		while ((n = fread(buf, 1, sizeof buf, fp)) > 0) s.append(buf, n);
		fclose(fp);
	}
	return s;
}
std::vector<std::string> words(const std::string &l)
{
	std::vector<std::string> w;
	size_t i = 0;
	while (i < l.size()) {
		while (i < l.size() && l[i] == ' ') ++i;
		size_t j = i;
		while (j < l.size() && l[j] != ' ') ++j;
		if (j > i) w.push_back(l.substr(i, j - i));
		i = j;
	}
	return w;
}
void dumpMap(const UserPreferences &p)
{
	for (PreferenceMap::const_iterator it = p.begin(); it != p.end(); ++it) outf("key %s=%s\n", it->first.str(), it->second.str());
}
void dumpList()
{
	for (size_t r = 0; r < g_rows.size(); ++r) {
		outf("row %d:", (int)r);
		for (int c = 0; c < g_columns; ++c) {
			const Cell &x = g_rows[r][c];
			if (x.kind == 0) outf(" -/%ld", x.data);
			else outf(" %s %dx%d #%08x/%ld", x.image.c_str(), x.w, x.h, (unsigned)x.color, x.data);
		}
		outf("\n");
	}
}
}

/* the score screen's block in a function of its own */
static void skirmishTail(Player *player, Int screenType)
{
#include "battle_honors_tail.inc"
}

/* The case's text, one item a line: `file <hex>` (SkirmishStats.ini's bytes, `-` for none), then the mode's lines
** (see tests/differential_battle_honors.rs). */
extern "C" int ref_battle_honors_run(const char *in, const char *dir, char *out, int cap)
{
	TheDynamicMemoryAllocator = &theHonorsAllocator;
	BhGlobalData gd;
	gd.m_userData = dir;
	TheGlobalData = &gd;
	g_out.clear();
	g_rows.clear();
	g_windows.clear();
	g_images.images.clear();
	g_built.clear();
	g_preorder = 0;
	MapCache cache;
	TheMapCache = &cache;
	std::string path = std::string(dir) + "SkirmishStats.ini";
	remove(path.c_str());
	std::string text(in);
	size_t at = 0;
	std::vector<std::vector<std::string> > lines;
	while (at < text.size()) {
		size_t nl = text.find('\n', at);
		if (nl == std::string::npos) nl = text.size();
		lines.push_back(words(text.substr(at, nl - at)));
		at = nl + 1;
	}
	std::string mode;
	for (size_t li = 0; li < lines.size(); ++li) {
		const std::vector<std::string> &w = lines[li];
		if (w.empty()) continue;
		if (w[0] == "file" && w.size() > 1 && w[1] != "-") {
			std::string bytes = unhex(w[1]);
			FILE *fp = fopen(path.c_str(), "wb");
			if (!fp) return -1;
			fwrite(bytes.data(), 1, bytes.size(), fp);
			fclose(fp);
		} else if (w[0] == "mode") {
			mode = w[1];
		} else if (w[0] == "win") {
			g_windows[w[1]].name = w[1];
		} else if (w[0] == "img") {
			g_images.images[w[1]].name = w[1];
		} else if (w[0] == "map") {
			MapMetaData m;
			m.m_isOfficial = atoi(w[2].c_str()) != 0;
			m.m_isMultiplayer = atoi(w[3].c_str()) != 0;
			m.m_numPlayers = atoi(w[4].c_str());
			cache[AsciiString(w[1].c_str())] = m;
		} else if (w[0] == "cfg") {
			g_columns = atoi(w[1].c_str());
			g_display.w = (UnsignedInt)atoi(w[2].c_str());
			g_display.h = (UnsignedInt)atoi(w[3].c_str());
			g_preorder = (unsigned)atoi(w[4].c_str());
		} else if (w[0] == "built") {
			g_built[w[1]] = atoi(w[2].c_str());
		} else if (w[0] == "slot") {
			int i = atoi(w[1].c_str());
			g_gameInfo.slots[i].state = atoi(w[2].c_str());
			g_gameInfo.slots[i].team = atoi(w[3].c_str());
		} else if (w[0] == "game") {
			/* game sandbox victory defeat active local skirmish frame vehicles aircraft map templateSide side */
			g_gameInfo.sandbox = atoi(w[1].c_str()) != 0;
			g_victory.victory = atoi(w[2].c_str()) != 0;
			g_victory.defeat = atoi(w[3].c_str()) != 0;
			g_player.active = atoi(w[4].c_str()) != 0;
			g_gameInfo.local = atoi(w[5].c_str());
			g_gameInfo.skirmish = atoi(w[6].c_str()) != 0;
			g_frame = (unsigned)strtoul(w[7].c_str(), NULL, 10);
			g_vehicles = atoi(w[8].c_str());
			g_aircraft = atoi(w[9].c_str());
			g_gameInfo.map = AsciiString(w[10].c_str());
			g_player.tpl.side = AsciiString(w[11].c_str());
			g_player.side = AsciiString(w[12].c_str());
		}
	}
	if (mode == "stats") {
		SkirmishBattleHonors s;
		for (size_t li = 0; li < lines.size(); ++li) {
			const std::vector<std::string> &w = lines[li];
			if (w.size() < 2 || w[0] != "op") continue;
			int k = atoi(w[1].c_str());
			int a = w.size() > 2 ? atoi(w[2].c_str()) : 0;
			int b = w.size() > 3 ? atoi(w[3].c_str()) : 0;
			AsciiString str = w.size() > 4 ? AsciiString(w[4].c_str()) : AsciiString();
			switch (k) {
			case 0: outf("r %d\n", s.getWins()); break;
			case 1: s.setWins(a); break;
			case 2: outf("r %d\n", s.getLosses()); break;
			case 3: s.setLosses(a); break;
			case 4: outf("r %d\n", s.getWinStreak()); break;
			case 5: s.setWinStreak(a); break;
			case 6: outf("r %d\n", s.getBestWinStreak()); break;
			case 7: s.setBestWinStreak(a); break;
			case 8: outf("r %d\n", s.getChallengeMedals()); break;
			case 9: s.setChallengeMedals(a); break;
			case 10: outf("r %d\n", s.builtSCUD() ? 1 : 0); break;
			case 11: s.setBuiltSCUD(); break;
			case 12: outf("r %d\n", s.builtParticleCannon() ? 1 : 0); break;
			case 13: s.setBuiltParticleCannon(); break;
			case 14: outf("r %d\n", s.builtNuke() ? 1 : 0); break;
			case 15: s.setBuiltNuke(); break;
			case 16: outf("r %d\n", s.getHonors()); break;
			case 17: s.setHonors(a); break;
			case 18: outf("r %d\n", s.getEnduranceMedal(str, a)); break;
			case 19: s.setEnduranceMedal(str, a, b); break;
			case 20: outf("r %s\n", s.getLastGeneral().str()); break;
			case 21: s.setLastGeneral(str); break;
			case 22: outf("r %d\n", s.getNumGamesLoyal()); break;
			case 23: s.setNumGamesLoyal(a); break;
			case 24: outf("r %d\n", s.getUSACampaignComplete((GameDifficulty)a) ? 1 : 0); break;
			case 25: s.setUSACampaignComplete((GameDifficulty)a); break;
			case 26: outf("r %d\n", s.getCHINACampaignComplete((GameDifficulty)a) ? 1 : 0); break;
			case 27: s.setCHINACampaignComplete((GameDifficulty)a); break;
			case 28: outf("r %d\n", s.getGLACampaignComplete((GameDifficulty)a) ? 1 : 0); break;
			case 29: s.setGLACampaignComplete((GameDifficulty)a); break;
			case 30: outf("r %d\n", s.getChallengeCampaignComplete(a, (GameDifficulty)b) ? 1 : 0); break;
			case 31: s.setChallengeCampaignComplete(a, (GameDifficulty)b); break;
			case 32: s.clear(); break;
			case 33: {
				/* (the write first: function arguments are evaluated in no set order) */
				int ok = s.write() ? 1 : 0;
				outf("w %d %s\n", ok, hex(readFile(path)).c_str());
				break;
			}
			default: return -1;
			}
		}
		dumpMap(s);
	} else if (mode == "populate") {
		populateSkirmishBattleHonors();
		std::map<std::string, GameWindow>::iterator it = g_windows.find("SkirmishGameOptionsMenu.wnd:ListboxInfo");
		if (it != g_windows.end()) outf("tooltipfunc %d\n", it->second.tooltip == BattleHonorTooltip ? 1 : 0);
		dumpList();
	} else if (mode == "tooltip") {
		for (size_t li = 0; li < lines.size(); ++li) {
			const std::vector<std::string> &w = lines[li];
			if (w.size() < 5 || w[0] != "tip") continue;
			/* tip row col data extra: a list of two rows, the cell's and the one above */
			g_columns = 4;
			g_rows.assign(2, std::vector<Cell>(4, blank()));
			int col = atoi(w[2].c_str());
			g_rows[1][col & 3].data = strtol(w[3].c_str(), NULL, 10);
			g_rows[0][col & 3].data = strtol(w[4].c_str(), NULL, 10);
			g_tipRow = atoi(w[1].c_str());
			g_tipCol = col;
			GameWindow list;
			BattleHonorTooltip(&list, NULL, 0);
		}
	} else if (mode == "tail") {
		skirmishTail(&g_player, SCORESCREEN_SKIRMISH);
		outf("file %s\n", hex(readFile(path)).c_str());
	}
	remove(path.c_str());
	TheMapCache = NULL;
	int n = (int)g_out.size();
	if (n <= cap) memcpy(out, g_out.data(), n);
	return n;
}
