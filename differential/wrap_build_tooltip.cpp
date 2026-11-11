/* Differential-test wrapper for the control bar's popup description's timing (GitHub #138, upstream commit 0a05454d),
** cut verbatim by build.rs into OUT_DIR/build_tooltip_gen:
**   bt_popup.inc     GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarPopupDescription.cpp, from its file statics
**                    (`static WindowLayout *theLayout = NULL;`) through ControlBarPopupDescriptionUpdateFunc,
**                    ControlBar::showBuildTooltipLayout and repopulateBuildTooltipLayout (to populateBuildTooltipLayout);
**   bt_delete.inc    the same file, ControlBar::hideBuildTooltipLayout and deleteBuildTooltipLayout (to the end);
**   bt_callback.inc  GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp, commandButtonTooltip (to markUIDirty's
**                    comment);
**   bt_update.inc    ControlBar.cpp, ControlBar::update's popup part (`if( !m_buildToolTipLayout->isHidden())` and the
**                    commented-out else, to `updateSpecialPowerShortcut();` before the observer's comment).
** No edit. The stand-ins: a ControlBar declaring the members they use (ControlBar.h's names and types); eight windows
** (style, tooltip delay, the push button's command or NULL); the popup's WindowLayout (its hidden bit, whose update is
** ControlBarPopupDescriptionUpdateFunc, as ControlBar::init sets it); the clock (`timeGetTime`); the InGameUI's tooltips
** disabled and quit menu; the script engine's game ending; the replay game; the disconnect screen; the global data's
** m_animateWindows; an AnimateWindowManager whose every member aborts (the file's `useAnimation` is FALSE, so none is
** ever made: an abort would show otherwise). populateBuildTooltipLayout and the layout's hide are recorded.
** The function statics of showBuildTooltipLayout (`isInitialized`, `beginWaitTime`) cannot be reset between sequences:
** each sequence starts with `prevWindow` NULL, so the first call over a window sets both before either is read.
**
** Entry point (extern "C", tests/differential_build_tooltip.rs):
**   ref_build_tooltip_seq(in, nin, out, cap) runs one sequence and writes its words to out.
**   `in` is the setup, 8 windows of 3 words [style, tooltip delay, command id (-1: NULL)], then the layout's hidden bit,
**   then steps of 4 words [kind, a, b, c]:
**     kind 0  commandButtonTooltip(window a)       kind 1  ControlBar::update's popup part
**     kind 2  hideBuildTooltipLayout()             kind 3  deleteBuildTooltipLayout()
**     kind 4  repopulateBuildTooltipLayout()       kind 5  the clock advances by a (milliseconds)
**     kind 6  a flag: a 0 tooltips disabled, 1 game ending, 2 replay game, 3 quit menu, 4 disconnect screen shown
**             (5 no disconnect menu), 6 m_animateWindows; to b
**   Records of 4 words: [1, hide] the layout's hide; [2, command id or -1, window or -1] populateBuildTooltipLayout;
**   after each step [100, m_showBuildToolTipLayout, prevWindow (-1 NULL), the layout's hidden bit]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"

namespace btx {

std::vector<int> g_out;
void rec(Int a, Int b = 0, Int c = 0, Int d = 0)
{
	Int w[4] = { a, b, c, d };
	g_out.insert(g_out.end(), w, w + 4);
}

#ifndef NEW
#define NEW new
#endif

/* Gadget.h's style bits */
enum { GWS_PUSH_BUTTON = 0x00000001, GWS_STATIC_TEXT = 0x00000080, GWS_USER_WINDOW = 0x00000200 };
enum AnimTypes { WIN_ANIMATION_NONE = 0, WIN_ANIMATION_SLIDE_RIGHT, WIN_ANIMATION_SLIDE_RIGHT_FAST };

struct CommandButton { Int m_id; };
CommandButton g_commands[16];

class WinInstanceData;
class GameWindow
{
public:
	Int m_id;
	UnsignedInt m_style;
	Int m_tooltipDelay;
	void *m_userData;
	UnsignedInt winGetStyle( void ) { return m_style; }
	Int getTooltipDelay() { return m_tooltipDelay; }
};
GameWindow g_win[8];
void *GadgetButtonGetData( GameWindow *g ) { return g->m_userData; }

class WindowLayout;
typedef void (*WindowLayoutUpdateFunc)( WindowLayout *layout, void *param );
class WindowLayout
{
public:
	Bool m_hidden;
	WindowLayoutUpdateFunc m_update;
	Bool isHidden( void ) { return m_hidden; }
	void hide( Bool hide ) { rec(1, hide ? 1 : 0); m_hidden = hide; }
	GameWindow *getFirstWindow( void ) { return &g_win[0]; }
	void runUpdate( void *userData = NULL ) { if (m_update) m_update(this, userData); }
};
WindowLayout g_layout;

class AnimateWindowManager
{
public:
	Bool isReversed( void ) { abort(); }
	Bool isFinished( void ) { abort(); }
	void reverseAnimateWindow( void ) { abort(); }
	void update( void ) { abort(); }
	void reset( void ) { abort(); }
	void registerGameWindow( GameWindow *, AnimTypes, Bool, UnsignedInt, UnsignedInt = 0 ) { abort(); }
};

UnsignedInt g_now;
UnsignedInt timeGetTime( void ) { return g_now; }

struct InGameUIStub
{
	Bool m_tooltipsDisabled, m_quitMenu;
	Bool areTooltipsDisabled( void ) { return m_tooltipsDisabled; }
	Bool isQuitMenuVisible( void ) { return m_quitMenu; }
};
InGameUIStub g_ui;
InGameUIStub *TheInGameUI = &g_ui;
struct ScriptEngineStub { Bool m_ending; Bool isGameEnding( void ) { return m_ending; } };
ScriptEngineStub g_script;
ScriptEngineStub *TheScriptEngine = &g_script;
struct GameLogicStub { Bool m_replay; Bool isInReplayGame( void ) { return m_replay; } };
GameLogicStub g_logic;
GameLogicStub *TheGameLogic = &g_logic;
struct DisconnectMenuStub { Bool m_visible; Bool isScreenVisible( void ) { return m_visible; } };
DisconnectMenuStub g_disconnect;
DisconnectMenuStub *TheDisconnectMenu = &g_disconnect;
struct GlobalDataStub { Bool m_animateWindows; };
GlobalDataStub g_global;
GlobalDataStub *TheGlobalData = &g_global;

class ControlBar
{
public:
	WindowLayout *m_buildToolTipLayout;
	Bool m_showBuildToolTipLayout;
	void showBuildTooltipLayout( GameWindow *cmdButton );
	void hideBuildTooltipLayout( void );
	void deleteBuildTooltipLayout( void );
	Bool getShowBuildTooltipLayout( void ){return m_showBuildToolTipLayout;	}
	void populateBuildTooltipLayout( const CommandButton *commandButton, GameWindow *tooltipWin = NULL )
	{
		rec(2, commandButton ? commandButton->m_id : -1, tooltipWin ? tooltipWin->m_id : -1);
	}
	void repopulateBuildTooltipLayout( void );
	void updatePopupPart( void );
};
ControlBar g_bar;
ControlBar *TheControlBar = &g_bar;

#include "bt_popup.inc"
#include "bt_delete.inc"
#include "bt_callback.inc"
void ControlBar::updatePopupPart( void )
{
#include "bt_update.inc"
}

static void state(void)
{
	rec(100, g_bar.m_showBuildToolTipLayout ? 1 : 0, prevWindow ? prevWindow->m_id : -1, g_layout.m_hidden ? 1 : 0);
}

} // namespace btx

extern "C" int ref_build_tooltip_seq(const int *in, int nin, int *out, int cap)
{
	using namespace btx;
	g_out.clear();
	for (Int i = 0; i < 8; ++i) {
		g_win[i].m_id = i;
		g_win[i].m_style = (UnsignedInt)in[i * 3];
		g_win[i].m_tooltipDelay = in[i * 3 + 1];
		Int c = in[i * 3 + 2];
		g_win[i].m_userData = c < 0 ? NULL : &g_commands[c & 15];
	}
	for (Int i = 0; i < 16; ++i)
		g_commands[i].m_id = i;
	g_layout.m_hidden = in[24] != 0;
	/* ControlBar::init: m_buildToolTipLayout->setUpdate(ControlBarPopupDescriptionUpdateFunc) */
	g_layout.m_update = ControlBarPopupDescriptionUpdateFunc;
	g_bar.m_buildToolTipLayout = &g_layout;
	g_bar.m_showBuildToolTipLayout = FALSE;
	prevWindow = NULL;
	g_now = 0;
	g_ui.m_tooltipsDisabled = g_ui.m_quitMenu = FALSE;
	g_script.m_ending = FALSE;
	g_logic.m_replay = FALSE;
	g_disconnect.m_visible = FALSE;
	TheDisconnectMenu = &g_disconnect;
	g_global.m_animateWindows = TRUE;
	for (int k = 25; k + 4 <= nin; k += 4) {
		const int *w = in + k;
		switch (w[0]) {
		case 0: commandButtonTooltip(&g_win[w[1] & 7], NULL, 0); break;
		case 1: TheControlBar->updatePopupPart(); break;
		case 2: TheControlBar->hideBuildTooltipLayout(); break;
		case 3: TheControlBar->deleteBuildTooltipLayout(); break;
		case 4: TheControlBar->repopulateBuildTooltipLayout(); break;
		case 5: g_now += (UnsignedInt)w[1]; break;
		case 6:
			switch (w[1]) {
			case 0: g_ui.m_tooltipsDisabled = w[2] != 0; break;
			case 1: g_script.m_ending = w[2] != 0; break;
			case 2: g_logic.m_replay = w[2] != 0; break;
			case 3: g_ui.m_quitMenu = w[2] != 0; break;
			case 4: g_disconnect.m_visible = w[2] != 0; TheDisconnectMenu = &g_disconnect; break;
			case 5: TheDisconnectMenu = NULL; break;
			case 6: g_global.m_animateWindows = w[2] != 0; break;
			}
			break;
		}
		state();
	}
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n > 0)
		memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
