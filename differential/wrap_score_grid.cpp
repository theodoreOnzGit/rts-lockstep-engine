/* Differential-test wrapper for the score screen's grid (GitHub #89 part 2, upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ScoreScreen.cpp, the pieces build.rs cuts
**   verbatim into OUT_DIR/score_grid_gen/score_grid_cut.inc: struct ScoreGather and its prototype; populatePlayerInfo up
**   to its skirmish tail (the battle honours, gap #92, and the internet branch's stats report, a multiplayer gap: the
**   cut ends before `if ( screenType == SCORESCREEN_SKIRMISH && player->isLocalPlayer() )` and closes the function);
**   grabMultiPlayerInfo, the relations' enum, grabSinglePlayerInfo, hideWindows, setObserverWindows, populateSideInfo
**   (to the end of the file).
**
** Everything below is a STAND-IN, written for this test:
** - GameWindow (hidden, its text, its enabled text colour and its enabled image's name, the status' image bit) and
**   TheWindowManager->winGetWindowFromId (the case's windows by key; the parent searched is not checked: every window
**   of the case is the parent's), GadgetStaticTextSetText, GadgetListBoxAddEntryText (the academy's rows);
** - TheNameKeyGenerator (keys in order of first use), TheGameText (`T:` and the label), TheMappedImageCollection (the
**   case's images by name), GameSpyColor[GSCOLOR_DEFAULT] (Chat.cpp: GameMakeColor(255,255,255,255));
** - Player (the case's: name key, display name, colour, observer, type, base side, listed on the score screen, its
**   relationship to each player's default team, its template), ThePlayerList (getLocalPlayer, getNthPlayer,
**   findPlayerWithNameKey over the first getPlayerCount() players), PlayerTemplate (getSideIconImage through the
**   collection, getScoreScreen), ScoreKeeper (the case's totals; calculateScore as ScoreKeeper.cpp's: 100 a unit and a
**   building built, the money earned, 100 a unit and a building destroyed of every player but its own),
**   AcademyStats::calculateAcademyAdvice (the case's tips, each "\n\n" and its text), TheGameLogic (the case's mode).
** - The stand-ins' globals (TheMappedImageCollection, TheNameKeyGenerator, TheWindowManager, TheGameText,
**   ThePlayerList, TheGameLogic) are `static`, local to this file. **Corrected 2026-11-10 (#104):** they were external
**   definitions, which clashed with wrap_ini.cpp's at link time (duplicate TheGameText, TheMappedImageCollection,
**   TheNameKeyGenerator) whenever a test linked both, and stopped tests/differential_recorder.rs from linking.
**
** Entry point (extern "C"):
**   ref_score_grid_run(in, out, cap)  runs one case (tests/differential_score_grid.rs's text) and writes the dump; its
**                                     length. */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include <map>
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "GameClient/Color.h"

template <typename T> static T max(T a, T b) { return a > b ? a : b; }

enum { MAX_SLOTS = 8, MAX_PLAYER_COUNT = 16, MAX_ADVICE_TIPS = 3 };
enum { PLAYER_HUMAN = 0, PLAYER_COMPUTER = 1 };
enum Relationship { ENEMIES = 0, NEUTRAL = 1, ALLIES = 2 };
enum { WIN_STATUS_IMAGE = 0x00000080 };
enum { GSCOLOR_DEFAULT = 0 };
enum { SCORESCREEN_SINGLEPLAYER = 0, SCORESCREEN_SKIRMISH, SCORESCREEN_LAN, SCORESCREEN_INTERNET, SCORESCREEN_REPLAY };
typedef Int NameKeyType;

namespace {

std::string narrow(const UnicodeString &u)
{
	std::string s;
	for (int i = 0; i < u.getLength(); ++i) s += (char)u.getCharAt(i);
	return s;
}

UnicodeString wide(const std::string &s)
{
	UnicodeString u;
	std::basic_string<WideChar> w;
	for (size_t i = 0; i < s.size(); ++i) w += (WideChar)(unsigned char)s[i];
	u.set(w.c_str());
	return u;
}

} // namespace

class Image
{
public:
	std::string name;
};

class SgImageCollection
{
public:
	std::map<std::string, Image> images;
	const Image *findImageByName(const AsciiString &name)
	{
		std::map<std::string, Image>::iterator it = images.find(name.str());
		return it == images.end() ? NULL : &it->second;
	}
};
static SgImageCollection g_collection;
static SgImageCollection *TheMappedImageCollection = &g_collection;

class GameWindow
{
public:
	std::string name;
	bool hidden;
	std::string text;
	Color color, border;
	std::string image;
	UnsignedInt status;
	std::vector<std::string> rows; /* a list box's */
	void winHide(Bool h) { hidden = h != 0; }
	Color winGetEnabledTextBorderColor() const { return border; }
	void winSetEnabledTextColors(Color c, Color b) { color = c; border = b; }
	void winSetEnabledImage(Int, const Image *img) { image = img ? img->name : std::string("-"); }
	UnsignedInt winGetStatus() const { return status; }
	void winSetStatus(UnsignedInt s) { status |= s; }
};

class SgNameKeyGenerator
{
public:
	std::map<std::string, Int> keys;
	NameKeyType nameToKey(const AsciiString &name)
	{
		std::map<std::string, Int>::iterator it = keys.find(name.str());
		if (it != keys.end()) return it->second;
		Int k = (Int)keys.size() + 1;
		keys[name.str()] = k;
		return k;
	}
};
static SgNameKeyGenerator g_keys;
static SgNameKeyGenerator *TheNameKeyGenerator = &g_keys;

class SgWindowManager
{
public:
	std::map<Int, GameWindow *> byKey;
	GameWindow *winGetWindowFromId(GameWindow *, Int id)
	{
		std::map<Int, GameWindow *>::iterator it = byKey.find(id);
		return it == byKey.end() ? NULL : it->second;
	}
};
static SgWindowManager g_wm;
static SgWindowManager *TheWindowManager = &g_wm;

void GadgetStaticTextSetText(GameWindow *w, UnicodeString text) { w->text = narrow(text); }
Int GadgetListBoxAddEntryText(GameWindow *lb, UnicodeString text, Color, Int)
{
	lb->rows.push_back(narrow(text));
	return (Int)lb->rows.size() - 1;
}

class SgGameText
{
public:
	UnicodeString fetch(const AsciiString &label) { return wide(std::string("T:") + label.str()); }
	UnicodeString fetch(const char *label) { return wide(std::string("T:") + label); }
};
static SgGameText g_text;
static SgGameText *TheGameText = &g_text;

static Color GameSpyColor[1] = { (Color)0xFFFFFFFF };

class ScoreKeeper
{
public:
	Int me, earned, spent, unitsBuilt, unitsLost, buildingsBuilt, buildingsLost;
	Int unitsDestroyed[MAX_PLAYER_COUNT], buildingsDestroyed[MAX_PLAYER_COUNT];
	Int getTotalMoneyEarned() { return earned; }
	Int getTotalMoneySpent() { return spent; }
	Int getTotalUnitsBuilt() { return unitsBuilt; }
	Int getTotalUnitsLost() { return unitsLost; }
	Int getTotalBuildingsBuilt() { return buildingsBuilt; }
	Int getTotalBuildingsLost() { return buildingsLost; }
	Int getTotalUnitsDestroyed() { Int n = 0; for (int i = 0; i < MAX_PLAYER_COUNT; ++i) n += unitsDestroyed[i]; return n; }
	Int getTotalBuildingsDestroyed() { Int n = 0; for (int i = 0; i < MAX_PLAYER_COUNT; ++i) n += buildingsDestroyed[i]; return n; }
	Int calculateScore()
	{
		Int score = 0;
		score += unitsBuilt * 100;
		score += earned;
		score += buildingsBuilt * 100;
		for (int i = 0; i < MAX_PLAYER_COUNT; ++i) {
			if (i == me) continue;
			score += unitsDestroyed[i] * 100;
			score += buildingsDestroyed[i] * 100;
		}
		return score;
	}
};

struct AcademyAdviceInfo
{
	UnicodeString advice[MAX_ADVICE_TIPS];
	UnsignedInt numTips;
};
static std::vector<std::string> g_tips;
class AcademyStats
{
public:
	Bool calculateAcademyAdvice(AcademyAdviceInfo *info)
	{
		info->numTips = 0;
		for (size_t i = 0; i < g_tips.size() && i < MAX_ADVICE_TIPS; ++i) {
			info->advice[i] = wide("\n\n" + narrow(TheGameText->fetch(g_tips[i].c_str())));
			info->numTips++;
		}
		return info->numTips != 0;
	}
};

class PlayerTemplate
{
public:
	std::string icon, screen;
	const Image *getSideIconImage() const { return TheMappedImageCollection->findImageByName(AsciiString(icon.c_str())); }
	AsciiString getScoreScreen() const { return AsciiString(screen.c_str()); }
};
static PlayerTemplate g_templates[4];

class Team {};
class Player
{
public:
	Int index;
	NameKeyType key;
	std::string display;
	Color color;
	Bool observer, human, listed;
	std::string side;
	Int templ;
	Int rel[MAX_PLAYER_COUNT];
	ScoreKeeper keeper;
	AcademyStats academy;
	Team team;
	Color getPlayerColor() const { return color; }
	ScoreKeeper *getScoreKeeper() { return &keeper; }
	UnicodeString getPlayerDisplayName() const { return wide(display); }
	AcademyStats *getAcademyStats() { return &academy; }
	const PlayerTemplate *getPlayerTemplate() const { return templ >= 0 ? &g_templates[templ] : NULL; }
	Bool isPlayerObserver() const { return observer; }
	Int getPlayerType() const { return human ? PLAYER_HUMAN : PLAYER_COMPUTER; }
	AsciiString getBaseSide() const { return AsciiString(side.c_str()); }
	Bool getListInScoreScreen() const { return listed; }
	Team *getDefaultTeam() { return &team; }
	Relationship getRelationship(const Team *t) const;
};

class SgPlayerList
{
public:
	Player players[MAX_PLAYER_COUNT];
	Int count;
	Int local;
	Player *getLocalPlayer() { return local >= 0 ? &players[local] : NULL; }
	Player *getNthPlayer(Int i) { return (i < 0 || i >= MAX_PLAYER_COUNT) ? NULL : &players[i]; }
	Player *findPlayerWithNameKey(NameKeyType key)
	{
		for (Int i = 0; i < count; ++i)
			if (players[i].key == key) return &players[i];
		return NULL;
	}
};
static SgPlayerList g_players;
static SgPlayerList *ThePlayerList = &g_players;

Relationship Player::getRelationship(const Team *t) const
{
	for (int q = 0; q < MAX_PLAYER_COUNT; ++q)
		if (&ThePlayerList->players[q].team == t) return (Relationship)rel[q];
	return NEUTRAL;
}

static Int g_mode;
class SgGameLogic
{
public:
	Bool isInSkirmishGame() const { return g_mode == 2; }
	Bool isInInternetGame() const { return g_mode == 5; }
	Bool isInSinglePlayerGame() const { return g_mode == 0; }
};
static SgGameLogic g_logic;
static SgGameLogic *TheGameLogic = &g_logic;

/* the file's statics the cut reads */
static GameWindow *parent = NULL;
static Bool overidePlayerDisplayName = FALSE;
static Int screenType;
GameWindow *listboxAcademyWindowScoreScreen = NULL;
GameWindow *staticTextAcademyTitle = NULL;

/* the file's own prototypes the cut relies on (ScoreScreen.cpp's top) */
void grabMultiPlayerInfo( void );
void grabSinglePlayerInfo( void );
void hideWindows( Int pos );
void setObserverWindows( Player *player, Int i );

/* ---- the upstream code ---- */
#include "score_grid_cut.inc"

namespace {
std::vector<GameWindow *> g_windows;
}

/* The case's text: `cfg <single> <screenType> <mode> <override> <count> <local> <parentStatus>`, `img <name>` a line
** each, `tpl <i> <icon>|<screen>`, `tip <label>`, `player <i> <key name>|<display>|<color> <observer> <human> <listed>
** <side>|<template> <me> <earned> <spent> <ub> <ul> <bb> <bl> <ud0..3> <bd0..3> <rel0..15>`, `win <name> <hidden>
** <color> <border>` for each window in dump order. */
extern "C" int ref_score_grid_run(const char *in, char *out, int cap)
{
	g_keys.keys.clear();
	g_wm.byKey.clear();
	g_collection.images.clear();
	g_tips.clear();
	for (int i = 0; i < 4; ++i) { g_templates[i].icon.clear(); g_templates[i].screen.clear(); }
	for (size_t i = 0; i < g_windows.size(); ++i) delete g_windows[i];
	g_windows.clear();
	int single = 0, over = 0, parentStatus = 0;
	g_players.count = 0;
	g_players.local = -1;
	for (int i = 0; i < MAX_PLAYER_COUNT; ++i) {
		Player &p = g_players.players[i];
		p.index = i; p.key = -1; p.display.clear(); p.color = 0; p.observer = FALSE; p.human = TRUE; p.listed = FALSE;
		p.side.clear(); p.templ = -1;
		memset(p.rel, 0, sizeof p.rel);
		memset(&p.keeper, 0, sizeof p.keeper);
	}
	std::string text(in);
	size_t at = 0;
	while (at < text.size()) {
		size_t nl = text.find('\n', at);
		if (nl == std::string::npos) nl = text.size();
		std::string line = text.substr(at, nl - at);
		at = nl + 1;
		if (line.compare(0, 4, "cfg ") == 0) {
			sscanf(line.c_str() + 4, "%d %d %d %d %d %d %d", &single, &screenType, &g_mode, &over, &g_players.count, &g_players.local, &parentStatus);
		} else if (line.compare(0, 4, "img ") == 0) {
			Image im;
			im.name = line.substr(4);
			g_collection.images[im.name] = im;
		} else if (line.compare(0, 4, "tpl ") == 0) {
			int i = atoi(line.c_str() + 4);
			std::string rest = line.substr(line.find(' ', 4) + 1);
			size_t bar = rest.find('|');
			g_templates[i].icon = rest.substr(0, bar);
			g_templates[i].screen = rest.substr(bar + 1);
		} else if (line.compare(0, 4, "tip ") == 0) {
			g_tips.push_back(line.substr(4));
		} else if (line.compare(0, 7, "player ") == 0) {
			int i = atoi(line.c_str() + 7);
			Player &p = g_players.players[i];
			std::string rest = line.substr(line.find(' ', 7) + 1);
			size_t b1 = rest.find('|'), b2 = rest.find('|', b1 + 1);
			std::string keyName = rest.substr(0, b1);
			p.key = keyName.empty() ? -1 : TheNameKeyGenerator->nameToKey(AsciiString(keyName.c_str()));
			p.display = rest.substr(b1 + 1, b2 - b1 - 1);
			const char *q = rest.c_str() + b2 + 1;
			int color, obs, hum, lst, n = 0;
			char side[64] = "";
			sscanf(q, "%d %d %d %d %63[^|]|%n", &color, &obs, &hum, &lst, side, &n);
			if (n == 0) { /* an empty side: `... <listed> |<template>` */
				sscanf(q, "%d %d %d %d |%n", &color, &obs, &hum, &lst, &n);
				side[0] = 0;
			}
			p.color = color; p.observer = obs; p.human = hum; p.listed = lst; p.side = side;
			q += n;
			char *e;
			p.templ = (int)strtol(q, &e, 10); q = e;
			long v[7 + 8 + 16];
			for (int k = 0; k < 31; ++k) { v[k] = strtol(q, &e, 10); q = e; }
			ScoreKeeper &s = p.keeper;
			s.me = (Int)v[0]; s.earned = (Int)v[1]; s.spent = (Int)v[2]; s.unitsBuilt = (Int)v[3]; s.unitsLost = (Int)v[4];
			s.buildingsBuilt = (Int)v[5]; s.buildingsLost = (Int)v[6];
			for (int k = 0; k < 4; ++k) { s.unitsDestroyed[k] = (Int)v[7 + k]; s.buildingsDestroyed[k] = (Int)v[11 + k]; }
			for (int k = 0; k < 16; ++k) p.rel[k] = (Int)v[15 + k];
		} else if (line.compare(0, 4, "win ") == 0) {
			char name[128];
			int hid, color, border;
			sscanf(line.c_str() + 4, "%127s %d %d %d", name, &hid, &color, &border);
			GameWindow *w = new GameWindow();
			w->name = name; w->hidden = hid != 0; w->color = color; w->border = border; w->image = "-"; w->status = 0;
			g_windows.push_back(w);
			g_wm.byKey[TheNameKeyGenerator->nameToKey(AsciiString(name))] = w;
		}
	}
	GameWindow parentWin;
	parentWin.name = "ScoreScreen.wnd:ParentScoreScreen"; parentWin.hidden = false; parentWin.color = 0; parentWin.border = 0; parentWin.image = "-";
	parentWin.status = (UnsignedInt)parentStatus;
	parent = &parentWin;
	overidePlayerDisplayName = over != 0;
	listboxAcademyWindowScoreScreen = TheWindowManager->winGetWindowFromId(parent, TheNameKeyGenerator->nameToKey(AsciiString("ScoreScreen.wnd:ListboxWarschoolAdvice")));
	staticTextAcademyTitle = TheWindowManager->winGetWindowFromId(parent, TheNameKeyGenerator->nameToKey(AsciiString("ScoreScreen.wnd:StaticTextWarSchool")));
	if (single) grabSinglePlayerInfo();
	else grabMultiPlayerInfo();
	std::string o;
	char b[1024];
	snprintf(b, sizeof b, "parent img:%s image %d\n", parentWin.image.c_str(), (parentWin.status & WIN_STATUS_IMAGE) ? 1 : 0);
	o += b;
	for (size_t i = 0; i < g_windows.size(); ++i) {
		GameWindow *w = g_windows[i];
		snprintf(b, sizeof b, "%s h%d [%s] #%08x img:%s\n", w->name.c_str(), w->hidden ? 1 : 0, w->text.c_str(), (unsigned)w->color, w->image.c_str());
		o += b;
		for (size_t r = 0; r < w->rows.size(); ++r) {
			o += "  row [" + w->rows[r] + "]\n";
		}
	}
	parent = NULL;
	listboxAcademyWindowScoreScreen = NULL;
	staticTextAcademyTitle = NULL;
	int n = (int)o.size();
	if (n <= cap) memcpy(out, o.data(), n);
	return n;
}
