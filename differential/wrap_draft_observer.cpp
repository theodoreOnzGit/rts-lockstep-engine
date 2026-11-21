/* Differential-test wrapper for the observer control bar (#215, group ui3b, upstream commit 0a05454d): src/drafts/ui/control_bar_observer.rs.
** Cut verbatim by build.rs (drafts_ui3b), no edit:
**   observer.inc   GameClient/GUI/ControlBar/ControlBarObserver.cpp, from `enum { MAX_BUTTONS = 8};` to the end (the statics, ControlBar::initObserverControls,
**                  ControlBarObserverSystem, ControlBar::populateObserverList, populateObserverInfoWindow);
**   obs_maxplayer.inc, obs_maxslots.inc   Common/GameCommon.h's MAX_PLAYER_COUNT (16) and GameNetwork/NetworkDefs.h's MAX_PLAYER and MAX_SLOTS;
**   the window manager's functions and enums (differential/shim_ui3b/ui3b_wm.h lists them: winGetWindowFromId with a start window, winHide, ...), upstream's own
**   GameClient/Color.h (GameMakeColor).
** Stand-ins: the mini window manager of ui3b_wm.h; AsciiString (format, conversion to const char *, assignment from a literal), UnicodeString (units; format(L"%d", n), and format(fmt,
**   name, team) as: the format's units, then the name's, then the team's, each followed by 0xFFFF); TheGameText->fetch(label) answers [FNV-1a of the label & 0xFFFF, the label's length]
**   (a UnicodeString); TheRecorder->isMultiplayer() (the case's flag); ThePlayerList (findPlayerWithNameKey(key): the player of slot i when key is the key of "player<i>", getNthPlayer(i), the
**   case's table of players); Player (isPlayerObserver, getPlayerType, getPlayerDisplayName, getPlayerColor, getPlayerTemplate()->getEnabledImage() / getFlagWaterMarkImage() (image tokens),
**   countObjects(mask, clear) = a hash of the player and the two masks, getScoreKeeper()->getTotalUnitsDestroyed() / getTotalUnitsLost()); KindOfMaskType (set(kindof), clear()) with the
**   KINDOF_ values SCORE, STRUCTURE, SCORE_CREATE, SCORE_DESTROY as bits 0..3; TheGameInfo->getConstSlot(i) (team number, isAI); GadgetButtonSetData / GadgetButtonGetData (the window's data
**   token = player + 1), GadgetButtonSetEnabledImage(w, image) logs [5, window id, image token], GadgetStaticTextSetText(w, text) logs [3, window id, length, units]; the window setters
**   winSetTooltip, winSetEnabledTextColors, winSetEnabledImage are the logged ones of the stub GameWindow; ControlBar (m_observerLookAtPlayer and its accessors; the cut's members).
** Not covered (upstream dereferences NULL, the draft skips): the observer bar's windows missing (the info and list windows, the eight buttons and texts under the list window, the seven
** info texts, the flag and the portrait are always present), a ninth player in the list; the draft's `slot_is_ai(..) && team == -1` is compared as upstream's `isAI() && getTeamNumber() == -1`.
**
** Entry point ref_ui3b_obs_run(in, n, out, cap): `in` = [tree scene block, nplayers, nplayers x [observer, human, color, enabled image, flag image, name length, name units, killed, lost],
** the players' slots (8 x player + 1 or 0), 8 x slot [team number, is AI], multiplayer, nsteps, nsteps x [kind (0 initObserverControls, 1 ControlBarObserverSystem(window a, msg b, d1 c; for
** GBM_SELECTED, GBM_SELECTED_RIGHT c is a window index + 1), 2 populateObserverList, 3 populateObserverInfoWindow, 4 m_observerLookAtPlayer = player a - 1 (0: NULL), 5 multiplayer = a,
** 6 the nth players of the single-player list = a (the human index order is the table's)), a, b, c]].
** Output per step: the calls logged, then [100, result (-1 for the others), the scene's dump, m_observerLookAtPlayer + 1, then the statics: the info window + 1, the list window + 1,
** buttonCancelID, the eight button ids and windows + 1, the eight text ids and windows + 1, the flag, portrait, five info texts + 1, every window's button data]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <stdarg.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"

namespace ui3bobs {

#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"
#include "ui3b_gadget.inc"
#include "ui3b_status.inc"
#include "ui3b_winerr.inc"
#include "ui3b_wm.h"
#include "GameClient/Color.h"

enum { 
#include "obs_maxplayer.inc"
};
enum {
#include "obs_maxslots.inc"
	NUM_CONNECTIONS };
static const Int MAX_SLOTS = MAX_PLAYER + 1;
#define DEBUG_ASSERTCRASH(cond, msg)

struct UnicodeString
{
	std::vector<unsigned short> w;
	UnicodeString() {}
	void format(const wchar_t *fmt, Int n) { (void)fmt; w.clear(); char b[32]; snprintf(b, sizeof b, "%d", n); for (const char *c = b; *c; ++c) w.push_back((unsigned short)*c); }
	void format(UnicodeString fmt, const unsigned short *name, const unsigned short *team)
	{
		w = fmt.w;
		for (; *name; ++name) w.push_back(*name);
		w.push_back(0xFFFF);
		for (; *team; ++team) w.push_back(*team);
		w.push_back(0xFFFF);
	}
	const unsigned short *str() { static std::vector<unsigned short> tmp[4]; static int k = 0; std::vector<unsigned short> &t = tmp[k++ & 3]; t = w; t.push_back(0); return &t[0]; }
};
struct AsciiString
{
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	void format(const char *fmt, ...) { char b[128]; va_list a; va_start(a, fmt); vsnprintf(b, sizeof b, fmt, a); va_end(a); s = b; }
	AsciiString &operator=(const char *c) { s = c; return *this; }
	operator const char *() const { return s.c_str(); }
};
struct GameText
{
	UnicodeString fetch(const char *label)
	{
		UnicodeString u;
		u.w.push_back((unsigned short)(fnv(label) & 0xFFFF)); u.w.push_back((unsigned short)strlen(label));
		return u;
	}
};
static GameText g_text;
static GameText *TheGameText = &g_text;

enum { KINDOF_SCORE = 0, KINDOF_STRUCTURE = 1, KINDOF_SCORE_CREATE = 2, KINDOF_SCORE_DESTROY = 3 };
struct KindOfMaskType { unsigned bits; KindOfMaskType() : bits(0) {} void set(int k) { bits |= 1u << k; } void clear() { bits = 0; } };
struct Image { unsigned token; };
struct PlayerTemplate { Image enabled, flag; const Image *getEnabledImage() { return &enabled; } const Image *getFlagWaterMarkImage() { return &flag; } };
struct ScoreKeeper { Int killed, lost; Int getTotalUnitsDestroyed() { return killed; } Int getTotalUnitsLost() { return lost; } };
enum { PLAYER_HUMAN = 1, PLAYER_COMPUTER = 2 };
struct Player
{
	unsigned index; bool observer; bool human; Int color; UnicodeString name; PlayerTemplate tmpl; ScoreKeeper score;
	Bool isPlayerObserver() { return observer; }
	Int getPlayerType() { return human ? PLAYER_HUMAN : PLAYER_COMPUTER; }
	UnicodeString getPlayerDisplayName() { return name; }
	Int getPlayerColor() { return color; }
	PlayerTemplate *getPlayerTemplate() { return &tmpl; }
	Int countObjects(KindOfMaskType mask, KindOfMaskType clear)
	{
		unsigned h = 2166136261u;
		h ^= index; h *= 16777619u; h ^= mask.bits; h *= 16777619u; h ^= clear.bits; h *= 16777619u;
		return (Int)((h >> 5) % 90);
	}
	ScoreKeeper *getScoreKeeper() { return &score; }
};
static std::vector<Player> g_players;
static unsigned g_slotPlayer[8];
static std::vector<unsigned> g_nth;
struct PlayerList
{
	Player *findPlayerWithNameKey(NameKeyType key)
	{
		for (int i = 0; i < 8; ++i) { char b[16]; snprintf(b, sizeof b, "player%d", i); if (keyOf(b) == key && g_slotPlayer[i]) return &g_players[g_slotPlayer[i] - 1]; }
		return NULL;
	}
	Player *getNthPlayer(Int i) { return (i >= 0 && (size_t)i < g_nth.size() && g_nth[i]) ? &g_players[g_nth[i] - 1] : NULL; }
};
static PlayerList g_pl;
static PlayerList *ThePlayerList = &g_pl;
static bool g_multi = false;
struct Recorder { Bool isMultiplayer() { return g_multi; } };
static Recorder g_rec;
static Recorder *TheRecorder = &g_rec;
struct GameSlot { Int team; bool ai; Int getTeamNumber() const { return team; } Bool isAI() const { return ai; } };
static GameSlot g_slots[8];
struct GameInfo { const GameSlot *getConstSlot(Int i) const { return &g_slots[i]; } };
static GameInfo g_gi;
static GameInfo *TheGameInfo = &g_gi;

static void GadgetButtonSetData(GameWindow *w, void *p) { w->gdata = p ? ((Player *)p)->index + 1 : 0; }
static void *GadgetButtonGetData(GameWindow *w) { return w->gdata ? &g_players[w->gdata - 1] : NULL; }
static void GadgetButtonSetEnabledImage(GameWindow *w, const Image *img) { lg(5); lg((unsigned)w->m_id); lg(img ? img->token : 0); }
static void GadgetStaticTextSetText(GameWindow *w, UnicodeString t)
{
	lg(3); lg((unsigned)w->m_id); lg((unsigned)t.w.size());
	for (size_t i = 0; i < t.w.size(); ++i) lg(t.w[i]);
}

class ControlBar
{
public:
	Player *m_observerLookAtPlayer;
	ControlBar() : m_observerLookAtPlayer(NULL) {}
	void initObserverControls(void);
	void populateObserverList(void);
	void populateObserverInfoWindow(void);
	void setObserverLookAtPlayer(Player *p) { m_observerLookAtPlayer = p; }
	Player *getObserverLookAtPlayer() { return m_observerLookAtPlayer; }
};
static ControlBar g_bar;
static ControlBar *TheControlBar = &g_bar;

#include "observer.inc"

}

extern "C" int ref_ui3b_obs_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bobs;
	(void)n;
	g_unitSystem = ControlBarObserverSystem;
	int p = readScene(in, 0);
	unsigned np = in[p++];
	g_players.assign(np, Player());
	for (unsigned i = 0; i < np; ++i) {
		Player &q = g_players[i];
		q.index = i; q.observer = in[p++] != 0; q.human = in[p++] != 0; q.color = (Int)in[p++];
		q.tmpl.enabled.token = in[p++]; q.tmpl.flag.token = in[p++];
		unsigned len = in[p++];
		for (unsigned k = 0; k < len; ++k) q.name.w.push_back((unsigned short)in[p++]);
		q.score.killed = (Int)in[p++]; q.score.lost = (Int)in[p++];
	}
	for (int i = 0; i < 8; ++i) g_slotPlayer[i] = in[p++];
	for (int i = 0; i < 8; ++i) { g_slots[i].team = (Int)in[p++]; g_slots[i].ai = in[p++] != 0; }
	g_multi = in[p++] != 0;
	g_nth.clear();
	for (int i = 0; i < 16; ++i) g_nth.push_back(i < (int)np ? (unsigned)i + 1 : 0);
	g_bar.m_observerLookAtPlayer = NULL;
	for (int i = 0; i < MAX_BUTTONS; ++i) { buttonPlayerID[i] = NAMEKEY_INVALID; staticTextPlayerID[i] = NAMEKEY_INVALID; buttonPlayer[i] = NULL; staticTextPlayer[i] = NULL; }
	ObserverPlayerInfoWindow = NULL; ObserverPlayerListWindow = NULL; buttonCancelID = NAMEKEY_INVALID;
	winFlag = winGeneralPortrait = staticTextNumberOfUnits = staticTextNumberOfBuildings = staticTextNumberOfUnitsKilled = staticTextNumberOfUnitsLost = staticTextPlayerName = NULL;
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], a = in[p++], b = in[p++], c = in[p++];
		g_log.clear();
		int res = -1;
		switch (kind) {
		case 0: g_bar.initObserverControls(); break;
		case 1: { WindowMsgData m1 = (b == GBM_SELECTED || b == GBM_SELECTED_RIGHT) ? (WindowMsgData)&g_wins[c - 1] : (WindowMsgData)c; res = ControlBarObserverSystem(&g_wins[a], b, m1, 0) == MSG_HANDLED ? 1 : 0; break; }
		case 2: g_bar.populateObserverList(); break;
		case 3: g_bar.populateObserverInfoWindow(); break;
		case 4: g_bar.m_observerLookAtPlayer = a ? &g_players[a - 1] : NULL; break;
		default: g_multi = a != 0; break;
		}
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100); all.push_back((unsigned)res);
		dumpScene(all);
		all.push_back(g_bar.m_observerLookAtPlayer ? g_bar.m_observerLookAtPlayer->index + 1 : 0);
		all.push_back(winIndex(ObserverPlayerInfoWindow)); all.push_back(winIndex(ObserverPlayerListWindow)); all.push_back((unsigned)buttonCancelID);
		for (int i = 0; i < MAX_BUTTONS; ++i) { all.push_back((unsigned)buttonPlayerID[i]); all.push_back(winIndex(buttonPlayer[i])); all.push_back((unsigned)staticTextPlayerID[i]); all.push_back(winIndex(staticTextPlayer[i])); }
		all.push_back(winIndex(winFlag)); all.push_back(winIndex(winGeneralPortrait)); all.push_back(winIndex(staticTextNumberOfUnits)); all.push_back(winIndex(staticTextNumberOfBuildings));
		all.push_back(winIndex(staticTextNumberOfUnitsKilled)); all.push_back(winIndex(staticTextNumberOfUnitsLost)); all.push_back(winIndex(staticTextPlayerName));
		for (size_t i = 0; i < g_wins.size(); ++i) all.push_back(g_wins[i].gdata);
	}
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
