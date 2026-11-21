/* Differential-test wrapper for the control bar's beacon context (#215, group ui3b, upstream commit 0a05454d): src/drafts/ui/control_bar_beacon.rs.
** Cut verbatim by build.rs (drafts_ui3b), no edit:
**   beacon.inc      GameClient/GUI/ControlBar/ControlBarBeacon.cpp, from `void ControlBar::populateBeacon` to the end
**                   (populateBeacon, updateContextBeacon, BeaconWindowInput);
**   ui3b_msgs.inc, ui3b_gwm.inc   GameClient/GameWindow.h: WindowMsgHandledType, GameWindowMessage; and upstream's own GameClient/KeyDefs.h;
**   ui3b_winget.inc               GameWindowManager.cpp: GameWindowManager::winGetWindowFromId, verbatim.
** Stand-ins (the shared ones in differential/shim_ui3b/ui3b_stubs.h: the name keys, the windows, winGetWindowFromId, winSetFocus):
**   ControlBar: the class with the three members; setPortraitByObject(beacon) logs [1, object];
**   Object: isLocallyControlled(), getDrawable() (whose getCaptionText() answers the object's text as a UnicodeString of 16-bit units);
**   GadgetTextEntrySetText(window, text): logs [3, window id, the window's hidden flag at that moment, the text's length, the text];
**   TheInGameUI->deselectAllDrawables(post): logs [4, post].
** Entry point ref_ui3b_beacon_run(in, n, out, cap): `in` = [scene block, nobj, nobj x [locally controlled, caption length, caption units],
** nsteps, nsteps x [kind (0 populateBeacon(object a), 1 BeaconWindowInput(msg a, d1 b)), a, b]]. Output per step: the calls logged, then
** [100, result (input: 0 ignored, 1 handled; populate: 2), the scene's dump]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"
#include "GameClient/KeyDefs.h"

namespace ui3bbeacon {

#include "ui3b_stubs.h"
#include "ui3b_winget.inc"
#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"

struct UnicodeString { std::vector<unsigned short> w; };
struct Drawable { UnicodeString *caption; UnicodeString getCaptionText() { return *caption; } };
struct Object
{
	bool local; UnicodeString caption; Drawable d;
	bool isLocallyControlled() { return local; }
	Drawable *getDrawable() { d.caption = &caption; return &d; }
};
class ControlBar
{
public:
	void populateBeacon(Object *beacon);
	void updateContextBeacon(void);
	void setPortraitByObject(Object *o);
};
static std::vector<Object *> g_objs;
void ControlBar::setPortraitByObject(Object *o) { lg(1); for (size_t i = 0; i < g_objs.size(); ++i) if (g_objs[i] == o) lg((unsigned)i); }
static void GadgetTextEntrySetText(GameWindow *w, UnicodeString t)
{
	lg(3); lg((unsigned)w->id); lg(w->winIsHidden() ? 1 : 0); lg((unsigned)t.w.size());
	for (size_t i = 0; i < t.w.size(); ++i) lg(t.w[i]);
}
struct InGameUI { void deselectAllDrawables(Bool post) { lg(4); lg(post ? 1 : 0); } };
static InGameUI g_ui;
static InGameUI *TheInGameUI = &g_ui;

#include "beacon.inc"

}

extern "C" int ref_ui3b_beacon_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bbeacon;
	(void)n;
	int p = readScene(in, 0);
	unsigned nobj = in[p++];
	std::vector<Object> objs(nobj);
	g_objs.clear();
	for (unsigned i = 0; i < nobj; ++i) {
		objs[i].local = in[p++] != 0;
		unsigned len = in[p++];
		for (unsigned k = 0; k < len; ++k) objs[i].caption.w.push_back((unsigned short)in[p++]);
		g_objs.push_back(&objs[i]);
	}
	ControlBar bar;
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], a = in[p++], b = in[p++];
		g_log.clear();
		unsigned res = 2;
		if (kind == 0) bar.populateBeacon(&objs[a]);
		else res = BeaconWindowInput(NULL, a, (WindowMsgData)b, 0) == MSG_HANDLED ? 1 : 0;
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100); all.push_back(res);
		dumpScene(all);
	}
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
