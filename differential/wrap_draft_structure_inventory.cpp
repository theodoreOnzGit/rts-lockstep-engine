/* Differential-test wrapper for the control bar's structure inventory (#215, group ui3b, upstream commit 0a05454d):
** src/drafts/ui/control_bar_structure_inventory.rs. Cut verbatim by build.rs (drafts_ui3b), no edit:
**   structure_inventory.inc  GameClient/GUI/ControlBar/ControlBarStructureInventory.cpp, from `#define STOP_ID` to the end (STOP_ID, EVACUATE_ID,
**                            PopulateButtonInfo, ControlBar::populateButtonProc, populateStructureInventory, updateContextStructureInventory);
**   sinv_max_cmds.inc, sinv_max_inv.inc   GameClient/ControlBar.h: the enums MAX_COMMANDS_PER_SET (18) and MAX_STRUCTURE_INVENTORY_BUTTONS (10).
** Stand-ins (the shared ones in differential/shim_ui3b/ui3b_stubs.h: windows with their status word, the focus):
**   ControlBar: the members the cuts use (m_commandWindows, the static m_containData, m_lastRecordedInventoryCount, m_currentSelectedDrawable);
**     resetContainData() zeroes m_containData and logs [1]; findCommandButton(name) answers a token (1 Command_Evacuate, 2 Command_Stop,
**     3 Command_StructureExit, or NULL for a name the case leaves undefined); setControlCommand(window, command) logs [4, window id, token];
**     calculateVeterancyOverlayForObject(obj) is the object's overlay token;
**   Image: a token. GadgetButtonSetEnabledImage(w, image) logs [5, window id, token]; GadgetButtonDrawOverlayImage(w, image) logs [6, window id, token or 0];
**   Object / Template / Drawable: getID, getTemplate()->getButtonImage() (a token), isLocallyControlled, getTeam, getDrawable (or NULL), getContain;
**   ContainModuleInterface: getContainMax, getContainCount (the case's numbers, which need not agree with the list), iterateContained(proc, userData, reverse)
**     calling proc on the contained objects in the case's order;
**   ThePlayerList->getLocalPlayer()->getRelationship(team): NEUTRAL or not by the case (the object's flag); TheInGameUI->deselectDrawable(draw) logs [7, object];
**   TheHotKeyManager: present or NULL by the case; reset() logs [2].
** Not covered (upstream indexes out of bounds or dereferences NULL): a contained list longer than the command windows it can reach; a missing window
** among the buttons the loop reaches (0..11, and the contained buttons' indices); a NULL m_currentSelectedDrawable. The draft skips those.
**
** Entry point ref_ui3b_sinv_run(in, n, out, cap): `in` = [scene block of 18 windows, present mask of windows 12..17 (bit k: window 12 + k), the three
** commands defined (bits 0..2), hot key manager present, nobj, nobj x [id, button image token, overlay token, flags (1 has contain, 2 locally controlled,
** 4 relationship not neutral, 8 has drawable), contain max, contain count, ncontained, contained object indices], selected object, last recorded count,
** nsteps, nsteps x [kind (0 populateStructureInventory(object a), 1 updateContextStructureInventory, 2 set object a's contain count to b,
** 3 select object a, 4 set the last recorded count to b), a, b]]. Output per step: the calls logged, then [100, the scene's dump, m_containData (18 x [control
** window id or 0, objectID]), m_lastRecordedInventoryCount]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"

namespace ui3bsinv {

#include "ui3b_stubs.h"
#include "sinv_max_cmds.inc"
#include "sinv_max_inv.inc"

enum { WIN_STATUS_NOT_READY = 0x00400000, WIN_STATUS_ALWAYS_COLOR = 0x01000000 };
typedef unsigned ObjectID;
enum Relationship { ENEMIES, NEUTRAL, ALLIES };
struct Image { unsigned token; };
struct CommandButton { unsigned token; };
struct Team { bool rel; };
struct AsciiString {};
#define DEBUG_ASSERTCRASH(cond, msg)
struct Drawable;
struct Object;
struct ContainModuleInterface;
typedef void (*ContainIterFunc)(Object *obj, void *userData);

struct Template { Image *buttonImage; Image *getButtonImage() { return buttonImage; } };
struct Object
{
	ObjectID id; Template tmpl; Image button, overlay; unsigned token; Team team;
	bool local, rel, hasDraw, hasContain; Int cmax, ccount; std::vector<Object *> contained;
	ObjectID getID() { return id; }
	Template *getTemplate() { return &tmpl; }
	bool isLocallyControlled() { return local; }
	Team *getTeam() { return &team; }
	Drawable *getDrawable();
	ContainModuleInterface *getContain();
};
struct ContainModuleInterface
{
	Object *o;
	Int getContainMax() { return o->cmax; }
	Int getContainCount() { return o->ccount; }
	void iterateContained(ContainIterFunc f, void *userData, Bool) { for (size_t i = 0; i < o->contained.size(); ++i) f(o->contained[i], userData); }
};
static std::vector<ContainModuleInterface> g_contains;
static std::vector<Object> g_objs;
ContainModuleInterface *Object::getContain() { return hasContain ? &g_contains[token] : NULL; }
struct Drawable { Object *obj; Object *getObject() { return obj; } };
static std::vector<Drawable> g_draws;
Drawable *Object::getDrawable() { return hasDraw ? &g_draws[token] : NULL; }

struct Player { Relationship getRelationship(Team *t) { return t->rel ? ALLIES : NEUTRAL; } };

struct PlayerList
{
	Player p;
	Player *getLocalPlayer() { return &p; }
};
static PlayerList g_pl;
static PlayerList *ThePlayerList = &g_pl;

struct InGameUI { void deselectDrawable(Drawable *d) { lg(7); lg(d->obj->token); } };
static InGameUI g_ui;
static InGameUI *TheInGameUI = &g_ui;
struct HotKeyManager { void reset() { lg(2); } };
static HotKeyManager g_hk;
static HotKeyManager *TheHotKeyManager = &g_hk;

static bool g_cmdDefined[3];
static CommandButton g_cmds[3] = { { 1 }, { 2 }, { 3 } };
static void GadgetButtonSetEnabledImage(GameWindow *w, const Image *img) { lg(5); lg((unsigned)w->id); lg(img ? img->token : 0); }
static void GadgetButtonDrawOverlayImage(GameWindow *w, const Image *img) { lg(6); lg((unsigned)w->id); lg(img ? img->token : 0); }

class ControlBar
{
public:
	GameWindow *m_commandWindows[MAX_COMMANDS_PER_SET];
	struct ContainEntry { GameWindow *control; ObjectID objectID; };
	static ContainEntry m_containData[MAX_COMMANDS_PER_SET];
	Int m_lastRecordedInventoryCount;
	Drawable *m_currentSelectedDrawable;
	ControlBar() : m_lastRecordedInventoryCount(0), m_currentSelectedDrawable(NULL) { for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i) m_commandWindows[i] = NULL; }
	void populateStructureInventory(Object *building);
	void updateContextStructureInventory(void);
	static void populateButtonProc(Object *obj, void *userData);
	void resetContainData(void) { lg(1); for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i) { m_containData[i].control = NULL; m_containData[i].objectID = 0; } }
	const CommandButton *findCommandButton(const char *name)
	{
		if (!strcmp(name, "Command_Evacuate")) return g_cmdDefined[0] ? &g_cmds[0] : NULL;
		if (!strcmp(name, "Command_Stop")) return g_cmdDefined[1] ? &g_cmds[1] : NULL;
		if (!strcmp(name, "Command_StructureExit")) return g_cmdDefined[2] ? &g_cmds[2] : NULL;
		lg(998);
		return NULL;
	}
	void setControlCommand(GameWindow *w, const CommandButton *c) { lg(4); lg((unsigned)w->id); lg(c ? c->token : 0); }
	static const Image *calculateVeterancyOverlayForObject(const Object *obj) { return &obj->overlay; }
};
ControlBar::ContainEntry ControlBar::m_containData[MAX_COMMANDS_PER_SET];

#include "structure_inventory.inc"

}

extern "C" int ref_ui3b_sinv_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bsinv;
	(void)n;
	int p = readScene(in, 0);
	unsigned present = in[p++];
	for (int k = 0; k < 3; ++k) g_cmdDefined[k] = (in[p] >> k) & 1;
	++p;
	TheHotKeyManager = in[p++] ? &g_hk : NULL;
	unsigned nobj = in[p++];
	g_objs.assign(nobj, Object());
	g_contains.assign(nobj, ContainModuleInterface());
	g_draws.assign(nobj, Drawable());
	static std::vector<std::vector<unsigned> > lists;
	lists.assign(nobj, std::vector<unsigned>());
	for (unsigned i = 0; i < nobj; ++i) {
		Object &o = g_objs[i];
		o.id = in[p++];
		o.button.token = in[p++];
		o.tmpl.buttonImage = &o.button;
		o.overlay.token = in[p++];
		unsigned flags = in[p++];
		o.hasContain = (flags & 1) != 0; o.local = (flags & 2) != 0; o.rel = (flags & 4) != 0; o.hasDraw = (flags & 8) != 0; o.team.rel = o.rel;
		o.cmax = (Int)in[p++]; o.ccount = (Int)in[p++];
		unsigned nc = in[p++];
		for (unsigned k = 0; k < nc; ++k) lists[i].push_back(in[p++]);
		o.token = i;
		g_contains[i].o = &o;
		g_draws[i].obj = &o;
	}
	for (unsigned i = 0; i < nobj; ++i) for (size_t k = 0; k < lists[i].size(); ++k) g_objs[i].contained.push_back(&g_objs[lists[i][k]]);
	ControlBar bar;
	for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i) { ControlBar::m_containData[i].control = NULL; ControlBar::m_containData[i].objectID = 0; }   /* a static: zero at the start of each case */
	for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i) {
		bool have = i < 12 || ((present >> (i - 12)) & 1);
		bar.m_commandWindows[i] = have ? &g_wins[i] : NULL;
	}
	unsigned sel = in[p++];
	bar.m_currentSelectedDrawable = &g_draws[sel];
	bar.m_lastRecordedInventoryCount = (Int)in[p++];
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], a = in[p++], b = in[p++];
		g_log.clear();
		if (kind == 0) bar.populateStructureInventory(&g_objs[a]);
		else if (kind == 1) bar.updateContextStructureInventory();
		else if (kind == 2) g_objs[a].ccount = (Int)b;
		else if (kind == 3) { sel = a; bar.m_currentSelectedDrawable = &g_draws[sel]; }
		else bar.m_lastRecordedInventoryCount = (Int)b;
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100);
		dumpScene(all);
		for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i) {
			all.push_back(ControlBar::m_containData[i].control ? (unsigned)ControlBar::m_containData[i].control->id : 0);
			all.push_back(ControlBar::m_containData[i].objectID);
		}
		all.push_back((unsigned)bar.m_lastRecordedInventoryCount);
	}
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
