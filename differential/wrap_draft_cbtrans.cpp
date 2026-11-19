/* Differential-test wrapper for the control bar transition draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/control_bar_command_processing.rs (process_command_transition_ui), cut verbatim by build.rs (drafts_guilogic3) into:
**   cbt_ctx.inc  GameClient/ControlBar.h, `enum ControlBarContext` (to `Context parents`);
**   cbt_fn.inc   ControlBarCommandProcessing.cpp, ControlBar::processCommandTransitionUI (to processCommandUI).
** Stand-ins (outside the cuts): CBCommandStatus (ControlBar.h:618-622, two enumerators); ControlBar (m_currContext, m_currentSelectedDrawable,
** switchToContext(context, object), which records the context asked for); Drawable (getObject(), an Object or NULL); GameWindow and GadgetGameMessage
** (unused names in the signature).
**
** Entry point (extern "C", tests/differential_draft_cbtrans.rs): ref_cbt(context, selection) with selection 0: no drawable, 1: a drawable with no object,
** 2: a drawable with an object; returns status | (switched to NONE) << 1. */
#define NULL 0
typedef int Int;
class Object {};
class Drawable { public: Object *getObject() const { return m_o; } Object *m_o; };
class GameWindow {};
typedef int GadgetGameMessage;
enum CBCommandStatus
{
	CBC_COMMAND_NOT_USED = 0,
	CBC_COMMAND_USED
};
#include "cbt_ctx.inc"
static int theSwitched;
class ControlBar
{
public:
	CBCommandStatus processCommandTransitionUI( GameWindow *control, GadgetGameMessage gadgetMessage );
	void switchToContext( ControlBarContext context, Drawable *draw ) { if (context == CB_CONTEXT_NONE && draw == NULL) theSwitched = 1; else theSwitched = 2; }
	ControlBarContext m_currContext;
	Drawable *m_currentSelectedDrawable;
};
#include "cbt_fn.inc"

extern "C" int ref_cbt(int context, int selection)
{
	theSwitched = 0;
	ControlBar cb;
	Drawable d; Object o;
	cb.m_currContext = (ControlBarContext)context;
	d.m_o = selection == 2 ? &o : NULL;
	cb.m_currentSelectedDrawable = selection == 0 ? (Drawable *)NULL : &d;
	int st = cb.processCommandTransitionUI(NULL, 0);
	return st | (theSwitched << 1);
}
