/* Differential-test wrapper for the control bar's OCL timer and under-construction contexts (#215, upstream commit
** 0a05454d): src/drafts/ui/control_bar_ocl_timer.rs and control_bar_under_construction.rs. Cut verbatim by build.rs:
**   ctl_ocl.inc     GameClient/GUI/ControlBar/ControlBarOCLTimer.cpp, from `void ControlBar::updateOCLTimerTextDisplay` to the end
**                   (updateOCLTimerTextDisplay, populateOCLTimer, updateContextOCLTimer);
**   ctl_uc.inc      ControlBarUnderConstruction.cpp, from `void ControlBar::updateConstructionTextDisplay` to the end
**                   (updateConstructionTextDisplay, populateUnderConstruction, updateContextUnderConstruction);
**   ctl_status.inc  GameClient/GameWindow.h, the window status enum (WIN_STATUS_HIDDEN, WIN_STATUS_USE_OVERLAY_STATES, ..);
**   ctl_frames.inc  Common/GameCommon.h, the enum holding LOGICFRAMES_PER_SECOND.
** No edit. Everything in namespace uilo, so these names cannot clash with another wrapper's.
**
** Stand-ins, for what the cuts call outside themselves (each is the C++ side of a method of the Rust drafts' host traits):
**   NameKeyType (a plain Int), NAMEKEY and TheNameKeyGenerator->nameToKey: FNV-1a of the name, 31 bits (the Rust host's function);
**   TheWindowManager->winGetWindowFromId(window, id): searches the flat window table from `window`'s place (or the start) for the
**     id, as upstream's does over a flat list (no children are used here);
**   GameWindow: an id and the status bits; winSetStatus ORs them in (GameWindow.cpp:719) and winHide(TRUE) sets WIN_STATUS_HIDDEN
**     (GameWindow.cpp:675; the image freeing and windowHiding it does are not state in this table);
**   UnicodeString and TheGameText->fetch: a text is a list of 16-bit words; fetch(label) is the label's FNV-1a (low 16 bits);
**     format(fmt, args..) is the format's words then each argument's (an Int as 2 words, low first; a Real promoted to a double
**     as C's varargs do, 4 words, low first). The game's own formatting of the text is not under test;
**   GadgetStaticTextSetText, GadgetProgressBarSetProgress: log the call (both return at once for a NULL window, as upstream's do);
**   ControlBar: the members the cuts use; findCommandButton gives a token by name (1 Command_Sell, 2 Command_SetRallyPoint,
**     3 Command_CancelConstruction, or NULL when the setup says the buttons are not defined); setControlCommand, showRallyPoint,
**     setPortraitByObject and evaluateContextUI log the call;
**   Object, Drawable, ExitInterface, OCLUpdate: the fields the cuts read.
** Not covered: a NULL sell/cancel button window or a NULL selected drawable (upstream dereferences them).
**
** Entry point (extern "C", tests/differential_draft_ctl_timers.rs):
**   ref_ctl_timers_seq(in, nin, out, cap) runs one sequence and writes its words; returns the count or -1.
**   `in`: [nwin, nwin ids (table order), ocl parent index or 0xFFFFFFFF, under-construction parent index or 0xFFFFFFFF,
**   buttons defined, nobj, nobj x [flags (1 tech, 2 auto rally point, 4 has exit, 8 under construction), frames, countdown percent
**   bits, construction percent bits], selected object, displayed OCL seconds, displayed construct percent bits, nsteps,
**   nsteps x [kind, a, b]]; kinds 0 updateOCLTimerTextDisplay(a, bits b), 1 populateOCLTimer(object a or NULL for 0xFFFFFFFF),
**   2 updateContextOCLTimer, 3 updateConstructionTextDisplay(object a), 4 populateUnderConstruction(a or NULL), 5
**   updateContextUnderConstruction, 6 set object a's remaining frames b, 7 its countdown percent bits b, 8 its construction percent
**   bits b, 9 its under-construction flag b, 10 select object a.
**   Output per step: the calls logged (1 text: [1, window, nwords, words..]; 2 progress: [2, window, value]; 3 setControlCommand:
**   [3, window, button]; 4 showRallyPoint; 5 portrait: [5, object]; 6 evaluateContextUI), then [100, displayed OCL seconds,
**   displayed construct percent bits, every window's status]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"

namespace uilo {

typedef Int NameKeyType;
#include "ctl_status.inc"
#include "ctl_frames.inc"

static std::vector<unsigned> g_out;

static unsigned fnv(const char *s)
{
	unsigned h = 2166136261u;
	for (; *s; ++s) { h ^= (unsigned char)*s; h *= 16777619u; }
	return h;
}
static NameKeyType keyOf(const char *s) { return (NameKeyType)(fnv(s) & 0x7FFFFFFFu); }
inline NameKeyType NAMEKEY(const char *name) { return keyOf(name); }
struct NameKeyGenerator { NameKeyType nameToKey(const char *n) { return keyOf(n); } };
static NameKeyGenerator g_nkg;
static NameKeyGenerator *TheNameKeyGenerator = &g_nkg;

/* ---- text ---- */
static void putArg(std::vector<unsigned short> &v, int x) { unsigned u = (unsigned)x; v.push_back((unsigned short)(u & 0xFFFF)); v.push_back((unsigned short)(u >> 16)); }
static void putArg(std::vector<unsigned short> &v, double x)
{
	uint64_t u; memcpy(&u, &x, sizeof u);
	for (int i = 0; i < 4; ++i) v.push_back((unsigned short)((u >> (16 * i)) & 0xFFFF));
}
static void putArg(std::vector<unsigned short> &v, float x) { putArg(v, (double)x); }
class UnicodeString
{
public:
	std::vector<unsigned short> w;
	template <class... A> void format(UnicodeString fmt, A... a) { w = fmt.w; (putArg(w, a), ...); }
};
class GameText
{
public:
	UnicodeString fetch(const char *label) { UnicodeString u; u.w.push_back((unsigned short)(fnv(label) & 0xFFFF)); return u; }
};
static GameText g_text;
static GameText *TheGameText = &g_text;

/* ---- windows ---- */
class GameWindow
{
public:
	Int m_id;
	UnsignedInt m_status;
	UnsignedInt winSetStatus(UnsignedInt s) { UnsignedInt o = m_status; m_status |= s; return o; }
	Int winHide(Bool hide) { if (hide) m_status |= WIN_STATUS_HIDDEN; else m_status &= ~(UnsignedInt)WIN_STATUS_HIDDEN; return 0; }
};
static std::vector<GameWindow> g_wins;
static unsigned tokenOf(const GameWindow *w) { return w ? (unsigned)(w - g_wins.data()) + 1 : 0; }
class WindowManager
{
public:
	GameWindow *winGetWindowFromId(GameWindow *window, NameKeyType id)
	{
		size_t i = window ? (size_t)(window - g_wins.data()) : 0;
		for (; i < g_wins.size(); ++i)
			if (g_wins[i].m_id == id) return &g_wins[i];
		return NULL;
	}
};
static WindowManager g_wm;
static WindowManager *TheWindowManager = &g_wm;

static void GadgetStaticTextSetText(GameWindow *window, UnicodeString text)
{
	if (!window) return;
	g_out.push_back(1); g_out.push_back(tokenOf(window)); g_out.push_back((unsigned)text.w.size());
	for (unsigned short x : text.w) g_out.push_back(x);
}
static void GadgetProgressBarSetProgress(GameWindow *g, Int progress)
{
	if (!g) return;
	g_out.push_back(2); g_out.push_back(tokenOf(g)); g_out.push_back((unsigned)progress);
}

/* ---- objects ---- */
enum { KINDOF_TECH_BUILDING, KINDOF_AUTO_RALLYPOINT };
enum { OBJECT_STATUS_UNDER_CONSTRUCTION = 1 };
struct Coord3D { Real x, y, z; };
struct ExitInterface { Coord3D p; const Coord3D *getRallyPoint() { return &p; } };
struct OCLUpdate
{
	UnsignedInt frames; Real pct;
	UnsignedInt getRemainingFrames() { return frames; }
	Real getCountdownPercent() { return pct; }
};
struct StatusBits { Bool on; Bool test(int) const { return on; } };
class Object
{
public:
	int idx; Bool tech, autoRally, hasExit, underConstruction;
	Real constructionPercent; OCLUpdate ocl; ExitInterface ei;
	Bool isKindOf(int k) const { return k == KINDOF_TECH_BUILDING ? tech : autoRally; }
	ExitInterface *getObjectExitInterface() const { return hasExit ? const_cast<ExitInterface *>(&ei) : NULL; }
	Real getConstructionPercent() const { return constructionPercent; }
	StatusBits getStatusBits() const { StatusBits s; s.on = underConstruction; return s; }
	void *findUpdateModule(NameKeyType) { return &ocl; }
};
class Drawable { public: Object *obj; Object *getObject() { return obj; } };
static std::vector<Object> g_objs;

/* ---- the control bar ---- */
class CommandButton { public: int token; };
static CommandButton g_buttons[3] = { { 1 }, { 2 }, { 3 } };
static Bool g_buttonsDefined = TRUE;
class ControlBar
{
public:
	enum { CP_OCL_TIMER, CP_UNDER_CONSTRUCTION, CP_COUNT };
	GameWindow *m_contextParent[CP_COUNT];
	UnsignedInt m_displayedOCLTimerSeconds;
	Real m_displayedConstructPercent;
	Drawable *m_currentSelectedDrawable;

	void updateOCLTimerTextDisplay( UnsignedInt totalSeconds, Real percent );
	void populateOCLTimer( Object *creatorObject );
	void updateContextOCLTimer( void );
	void updateConstructionTextDisplay( Object *obj );
	void populateUnderConstruction( Object *objectUnderConstruction );
	void updateContextUnderConstruction( void );

	const CommandButton *findCommandButton( const char *name )
	{
		if (!g_buttonsDefined) return NULL;
		if (!strcmp(name, "Command_Sell")) return &g_buttons[0];
		if (!strcmp(name, "Command_SetRallyPoint")) return &g_buttons[1];
		return &g_buttons[2];
	}
	void setControlCommand( GameWindow *control, const CommandButton *commandButton )
	{
		g_out.push_back(3); g_out.push_back(tokenOf(control)); g_out.push_back(commandButton ? (unsigned)commandButton->token : 0u);
	}
	void showRallyPoint( const Coord3D *loc ) { (void)loc; g_out.push_back(4); }
	void setPortraitByObject( Object *obj ) { g_out.push_back(5); g_out.push_back((unsigned)obj->idx); }
	void evaluateContextUI( void ) { g_out.push_back(6); }
};

#include "ctl_ocl.inc"
#include "ctl_uc.inc"

} // namespace uilo

extern "C" int ref_ctl_timers_seq(const unsigned *in, int nin, unsigned *out, int cap)
{
	using namespace uilo;
	g_out.clear();
	int p = 0;
	int nwin = (int)in[p++];
	g_wins.assign(nwin, GameWindow());
	for (int i = 0; i < nwin; ++i) { g_wins[i].m_id = (Int)in[p++]; g_wins[i].m_status = 0; }
	ControlBar cb;
	unsigned po = in[p++], pu = in[p++];
	cb.m_contextParent[ControlBar::CP_OCL_TIMER] = po == 0xFFFFFFFFu ? NULL : &g_wins[po];
	cb.m_contextParent[ControlBar::CP_UNDER_CONSTRUCTION] = pu == 0xFFFFFFFFu ? NULL : &g_wins[pu];
	g_buttonsDefined = in[p++] != 0;
	int nobj = (int)in[p++];
	g_objs.assign(nobj, Object());
	for (int i = 0; i < nobj; ++i)
	{
		Object &o = g_objs[i];
		unsigned f = in[p++];
		o.idx = i; o.tech = (f & 1) != 0; o.autoRally = (f & 2) != 0; o.hasExit = (f & 4) != 0; o.underConstruction = (f & 8) != 0;
		o.ocl.frames = in[p++];
		unsigned b = in[p++]; memcpy(&o.ocl.pct, &b, 4);
		b = in[p++]; memcpy(&o.constructionPercent, &b, 4);
	}
	Drawable dr; dr.obj = &g_objs[in[p++]];
	cb.m_currentSelectedDrawable = &dr;
	cb.m_displayedOCLTimerSeconds = in[p++];
	{ unsigned b = in[p++]; memcpy(&cb.m_displayedConstructPercent, &b, 4); }
	int nsteps = (int)in[p++];
	for (int s = 0; s < nsteps; ++s)
	{
		unsigned kind = in[p++], a = in[p++], b = in[p++];
		Object *oa = a == 0xFFFFFFFFu ? NULL : &g_objs[a];
		switch (kind)
		{
			case 0: { Real r; memcpy(&r, &b, 4); cb.updateOCLTimerTextDisplay(a, r); break; }
			case 1: cb.populateOCLTimer(oa); break;
			case 2: cb.updateContextOCLTimer(); break;
			case 3: cb.updateConstructionTextDisplay(oa); break;
			case 4: cb.populateUnderConstruction(oa); break;
			case 5: cb.updateContextUnderConstruction(); break;
			case 6: oa->ocl.frames = b; break;
			case 7: memcpy(&oa->ocl.pct, &b, 4); break;
			case 8: memcpy(&oa->constructionPercent, &b, 4); break;
			case 9: oa->underConstruction = b != 0; break;
			default: cb.m_currentSelectedDrawable->obj = oa; break;
		}
		g_out.push_back(100);
		g_out.push_back(cb.m_displayedOCLTimerSeconds);
		{ unsigned bits; memcpy(&bits, &cb.m_displayedConstructPercent, 4); g_out.push_back(bits); }
		for (size_t i = 0; i < g_wins.size(); ++i) g_out.push_back(g_wins[i].m_status);
	}
	(void)nin;
	if ((int)g_out.size() > cap) return -1;
	memcpy(out, g_out.data(), g_out.size() * sizeof(unsigned));
	return (int)g_out.size();
}
