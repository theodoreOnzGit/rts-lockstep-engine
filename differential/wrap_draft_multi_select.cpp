/* Differential-test wrapper for the control bar's multi-select context (#215, group ui3b, upstream commit 0a05454d):
** src/drafts/ui/control_bar_multi_select.rs. Cut verbatim by build.rs (drafts_ui3b), no edit:
**   multi_select.inc  GameClient/GUI/ControlBar/ControlBarMultiSelect.cpp, from `void ControlBar::resetCommonCommandData` to the end (resetCommonCommandData,
**                     addCommonCommands, populateMultiSelect, updateContextMultiSelect);
**   ms_options.inc, ms_avail.inc   GameClient/ControlBar.h: the enums CommandOption (OK_FOR_MULTI_SELECT, CHECK_LIKE, ...) and CommandAvailability;
**   sinv_max_cmds.inc              ControlBar.h's MAX_COMMANDS_PER_SET (18).
** Stand-ins (shared ones in differential/shim_ui3b/ui3b_stubs.h: the windows, their status words and button data):
**   ControlBar: the members the cuts use (m_commandWindows, m_commonCommands); setControlCommand(w, command) logs [6, window id, command token] and stores the token as the
**     window's button data; getCommandAvailability(command, obj, w) logs [3, command token, object index, window id] and answers by a fixed hash of the three (the Rust test's
**     function); setPortraitByObject(obj) logs [7, object index or 0xFFFFFFFF];
**   GadgetButtonDrawOverlayImage(w, NULL) logs [5, window id] (nothing for a NULL window, as upstream's); GadgetButtonGetData(w) is the window's button data;
**   GadgetCheckLikeButtonSetVisualCheck(w, b) logs [4, window id, b];
**   CommandButton (options, whether it is the attack move command), CommandSet (18 slots), findCommandSet(name) (NULL for no set), Object (isKindOf(IGNORED_IN_GUI),
**     the command set, the sold bit), Drawable (its object, its template's selected portrait image), TheInGameUI->getAllSelectedDrawables() (the case's list).
** Not covered (upstream dereferences NULL): a selected drawable without an object. The draft skips it.
**
** Entry point ref_ui3b_msel_run(in, n, out, cap): `in` = [scene block of 18 windows (the status words; the window slots present: bit k of the mask), mask, per window button data
** token, per slot initial common command token, ncmd, ncmd x [options, attack move], nset, nset x [18 command tokens], nobj, nobj x [ignored, sold, command set (0 none, else index + 1),
** portrait token], nsel, nsel x [object index], nsteps, nsteps x [kind (0 populateMultiSelect, 1 updateContextMultiSelect, 2 addCommonCommands(drawable a, first b),
** 3 resetCommonCommandData, 4 the selection becomes the b objects starting at a (a, a+1, ..) mod nobj), a, b]]. Output per step: the calls logged, then [100, the scene's
** dump, the windows' button data, m_commonCommands (18 tokens)]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include <list>
#include "Lib/BaseType.h"

namespace ui3bmsel {

#include "ui3b_stubs.h"
#include "sinv_max_cmds.inc"
#include "ms_options.inc"
#include "ms_avail.inc"

enum { WIN_STATUS_NOT_READY = 0x00400000, WIN_STATUS_ALWAYS_COLOR = 0x01000000 };
enum { KINDOF_IGNORED_IN_GUI = 1 };
enum { OBJECT_STATUS_SOLD = 2 };
enum { GUI_COMMAND_ATTACK_MOVE = 7, GUI_COMMAND_OTHER = 3 };
#define DEBUG_ASSERTCRASH(cond, msg)

struct CommandButton
{
	unsigned token, options; bool attackMove;
	unsigned getOptions() const { return options; }
	int getCommandType() const { return attackMove ? GUI_COMMAND_ATTACK_MOVE : GUI_COMMAND_OTHER; }
};
struct CommandSet { const CommandButton *slots[MAX_COMMANDS_PER_SET]; const CommandButton *getCommandButton(Int i) const { return slots[i]; } };
struct AsciiString { unsigned set; };
struct StatusBits { bool sold; bool test(int bit) const { return bit == OBJECT_STATUS_SOLD && sold; } };
struct Image { unsigned token; };
struct DrawableTemplate { const Image *img; const Image *getSelectedPortraitImage() const { return img; } };
struct Object
{
	bool ignored, sold; unsigned cmdset; unsigned index;
	bool isKindOf(int k) const { return k == KINDOF_IGNORED_IN_GUI && ignored; }
	AsciiString getCommandSetString() const { AsciiString a; a.set = cmdset; return a; }
	StatusBits getStatusBits() const { StatusBits b; b.sold = sold; return b; }
};
struct Drawable { Object *obj; DrawableTemplate tmpl; Object *getObject() { return obj; } DrawableTemplate *getTemplate() { return &tmpl; } };

static std::vector<CommandButton> g_cmds;
static std::vector<CommandSet> g_sets;
static std::vector<Object> g_objs;
static std::vector<Drawable> g_draws;
static std::vector<Image> g_imgs;
typedef std::list<Drawable *> DrawableList;
typedef DrawableList::const_iterator DrawableListCIt;
static DrawableList g_selected;

static const CommandSet *findCommandSet(AsciiString name) { return name.set ? &g_sets[name.set - 1] : NULL; }
struct InGameUI { const DrawableList *getAllSelectedDrawables() { return &g_selected; } };
static InGameUI g_ui;
static InGameUI *TheInGameUI = &g_ui;

static void GadgetButtonDrawOverlayImage(GameWindow *g, const Image *) { if (g == NULL) return; lg(5); lg((unsigned)g->id); }
static const CommandButton *GadgetButtonGetData(GameWindow *g) { return g->gdata ? &g_cmds[g->gdata - 1] : NULL; }
static void GadgetCheckLikeButtonSetVisualCheck(GameWindow *g, Bool b) { lg(4); lg((unsigned)g->id); lg(b ? 1 : 0); }

static unsigned availHash(unsigned cmd, unsigned obj, unsigned win)
{
	unsigned h = 2166136261u;
	h ^= cmd; h *= 16777619u; h ^= obj; h *= 16777619u; h ^= win; h *= 16777619u;
	return (h >> 7) % 6;
}

class ControlBar
{
public:
	GameWindow *m_commandWindows[MAX_COMMANDS_PER_SET];
	const CommandButton *m_commonCommands[MAX_COMMANDS_PER_SET];
	void resetCommonCommandData(void);
	void addCommonCommands(Drawable *draw, Bool firstDrawable);
	void populateMultiSelect(void);
	void updateContextMultiSelect(void);
	void setControlCommand(GameWindow *w, const CommandButton *c) { lg(6); lg((unsigned)w->id); lg(c ? c->token : 0); w->gdata = c ? c->token : 0; }
	CommandAvailability getCommandAvailability(const CommandButton *c, Object *o, GameWindow *w)
	{
		lg(3); lg(c->token); lg(o->index); lg((unsigned)w->id);
		static const CommandAvailability map[6] = { COMMAND_RESTRICTED, COMMAND_AVAILABLE, COMMAND_ACTIVE, COMMAND_HIDDEN, COMMAND_NOT_READY, COMMAND_CANT_AFFORD };
		return map[availHash(c->token, o->index, (unsigned)w->id)];
	}
	void setPortraitByObject(Object *o) { lg(7); lg(o ? o->index : 0xFFFFFFFFu); }
};

#include "multi_select.inc"

}

extern "C" int ref_ui3b_msel_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bmsel;
	(void)n;
	int p = readScene(in, 0);
	unsigned mask = in[p++];
	for (int i = 0; i < 18; ++i) g_wins[i].gdata = in[p++];
	ControlBar bar;
	unsigned initial[18];
	for (int i = 0; i < 18; ++i) initial[i] = in[p++];
	unsigned ncmd = in[p++];
	g_cmds.assign(ncmd, CommandButton());
	for (unsigned i = 0; i < ncmd; ++i) { g_cmds[i].token = i + 1; g_cmds[i].options = in[p++]; g_cmds[i].attackMove = in[p++] != 0; }
	for (int i = 0; i < 18; ++i) bar.m_commonCommands[i] = initial[i] ? &g_cmds[initial[i] - 1] : NULL;
	unsigned nset = in[p++];
	g_sets.assign(nset, CommandSet());
	for (unsigned i = 0; i < nset; ++i) for (int k = 0; k < 18; ++k) { unsigned t = in[p++]; g_sets[i].slots[k] = t ? &g_cmds[t - 1] : NULL; }
	unsigned nobj = in[p++];
	g_objs.assign(nobj, Object());
	g_draws.assign(nobj, Drawable());
	g_imgs.assign(64, Image());
	for (unsigned i = 0; i < 64; ++i) g_imgs[i].token = i;
	for (unsigned i = 0; i < nobj; ++i) {
		g_objs[i].ignored = in[p++] != 0; g_objs[i].sold = in[p++] != 0; g_objs[i].cmdset = in[p++]; g_objs[i].index = i;
		unsigned pt = in[p++];
		g_draws[i].obj = &g_objs[i];
		g_draws[i].tmpl.img = pt ? &g_imgs[pt] : NULL;
	}
	for (int i = 0; i < 18; ++i) bar.m_commandWindows[i] = ((mask >> i) & 1) ? &g_wins[i] : NULL;
	g_selected.clear();
	unsigned nsel = in[p++];
	for (unsigned i = 0; i < nsel; ++i) g_selected.push_back(&g_draws[in[p++]]);
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], a = in[p++], b = in[p++];
		g_log.clear();
		if (kind == 0) bar.populateMultiSelect();
		else if (kind == 1) bar.updateContextMultiSelect();
		else if (kind == 2) bar.addCommonCommands(&g_draws[a], b != 0);
		else if (kind == 3) bar.resetCommonCommandData();
		else { g_selected.clear(); for (unsigned i = 0; i < b; ++i) g_selected.push_back(&g_draws[(a + i) % nobj]); }
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100);
		dumpScene(all);
		for (int i = 0; i < 18; ++i) all.push_back(g_wins[i].gdata);
		for (int i = 0; i < 18; ++i) all.push_back(bar.m_commonCommands[i] ? bar.m_commonCommands[i]->token : 0);
	}
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
