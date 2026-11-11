/* Differential-test wrapper for hiding, showing, lowering and raising the control bar (GitHub #136, upstream commit
** 0a05454d), cut verbatim by build.rs into OUT_DIR/control_bar_stage_gen:
**   cbs_enum.inc      GameEngine/Include/GameClient/ControlBar.h, `enum ControlBarStages` (to `class ControlBar`);
**   cbs_stage.inc     GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp, `ControlBar::toggleControlBarStage`,
**                     `switchControlBarStage` and `setDefaultControlBarConfig` (to `setSquishedControlBarConfig`, which
**                     nothing reaches: its case is commented out);
**   cbs_low.inc       ControlBar.cpp, `setLowControlBarConfig` and `setHiddenControlBar` (to the commented-out
**                     `showCommandMarkers`);
**   cbs_updown.inc    ControlBar.cpp, `setUpDownImages` (to `getForegroundMarkerPos`);
**   cbs_purchase.inc  ControlBar.cpp, `hidePurchaseScience` (to `togglePurchaseScience`);
**   cbs_callback.inc  GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarCallback.cpp, the replay controls' externs,
**                     `ShowControlBar`, `HideControlBar` and `ToggleControlBar` (to "Resize the control bar");
**   cbs_replay.inc    GameEngine/Source/GameClient/InGameUI.cpp, `showReplayControls`, `hideReplayControls` and
**                     `toggleReplayControls` (to `SuperweaponInfo::SuperweaponInfo`);
**   cbs_meta.inc      GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp, translateGameMessage's
**                     `case GameMessage::MSG_META_TOGGLE_CONTROL_BAR` (to `MSG_META_TOGGLE_ATTACKMOVE`);
** and, GitHub #138, the other callers of HideControlBar and ShowControlBar:
**   cbs_letterbox.inc GameEngine/Source/GameLogic/ScriptEngine/ScriptActions.cpp, `ScriptActions::doLetterBoxMode` (to
**                     `doBlackWhiteMode`'s comment);
**   cbs_create.inc    InGameUI.cpp, `InGameUI::createControlBar` (to `createReplayControl`'s comment);
**   cbs_engine.inc    GameEngine/Source/Common/GameEngine.cpp, the end of `GameEngine::init` from
**                     `TheSubsystemList->resetAll();` (to `}  // end init`: the wrapper closes it);
**   cbs_new_game.inc  GameEngine/Source/GameLogic/System/GameLogic.cpp, startNewGame's `if(m_gameMode == GAME_SHELL)`
**                     block that hides or shows the bar (to `#ifdef DO_UNIT_TIMINGS` / `// Turn off the UI`);
**   cbs_clear.inc     GameEngine/Source/GameLogic/System/GameLogicDispatch.cpp, clearGameData's `HideControlBar();` and
**                     `closeWindows();` (to `TheMouse->setVisibility(TRUE);`).
** No edit. They are compiled inside namespace cbs on stand-ins: a `ControlBar` declaring the members they use
** (ControlBar.h's names and types), four windows (0 ControlBarParent, which is also CP_MASTER; 1 ButtonLarge; 2 the
** replay window; 3 CP_PURCHASE_SCIENCE) that keep their hidden bit and position, a window manager that finds 0 and 1 by
** name, the shell's top layout, the recorder, the display's height, the tactical view, the animate window manager and the
** six toggle images (1 UpIn, 2 UpOn, 3 UpPushed, 4 DownIn, 5 DownOn, 6 DownPushed). The calls that leave are recorded.
**
** Entry point (extern "C", tests/differential_control_bar_stage.rs):
**   ref_control_bar_stage_seq(in, nin, out, cap) runs one sequence and writes its words to out.
**   `in` is the setup, 16 words [the stage (0..3), default x, default y, display height, playback, in a replay game,
**   shell active, shell top (0 none, 1 shown, 2 hidden; a change of its hidden bit needs one), animate window manager, ControlBarParent found by name,
**   ButtonLarge exists, replay window exists, the windows' hidden bits (bit i: window i), window 0's x, y, window 1's
**   x], then steps of 4 words [kind, a, b, c]:
**     kind 0  the MSG_META_TOGGLE_CONTROL_BAR case
**     kind 1  TheControlBar->toggleControlBarStage()
**     kind 2  TheControlBar->switchControlBarStage(a)
**     kind 3  ShowControlBar(a)
**     kind 4  HideControlBar(a)  (never with a TRUE and ControlBarParent not found, nor a FALSE without an animate
**             window manager: upstream dereferences NULL there)
**     kind 5  ToggleControlBar(a)
**     kind 6  the world changes: a 0 playback b, 1 in a replay game b, 2 shell active b, 3 the shell top's hidden b,
**             4 the display's height b, 5 window b's own hidden bit c (winHide)
**     (#138, never a hide with ControlBarParent not found)
**     kind 7  TheScriptActions->doLetterBoxMode(a)
**     kind 8  TheInGameUI->createControlBar()
**     kind 9  the end of GameEngine::init
**     kind 10 the end of GameLogic::startNewGame with m_gameMode GAME_SHELL (a 1) or GAME_SKIRMISH (a 0)
**     kind 11 GameLogic::clearGameData's HideControlBar and closeWindows
**   Records of 6 words: [1, win, hide] winHide; [2, win, x, y] winSetPosition; [3, h] setHeight; [4, win, enabled,
**   hilite, hilite selected] the images; [5] showSpecialPowerShortcut; [6] hideSpecialPowerShortcut; [7, on]
**   animateSpecialPowerShortcut; [8] repopulateBuildTooltipLayout; [9] markUIDirty; [10] animate reset; [11, win]
**   registerGameWindow(SLIDE_BOTTOM, TRUE, 500, 0); [12] reverseAnimateWindow; [13, hide] the shell top's hide;
**   [20, disposition] after a meta case; (#138) [14, on] TheDisplay->enableLetterBox; [15] winCreateFromScript of
**   ControlBar.wnd; [17] TheSubsystemList->resetAll; [18] closeWindows; after each step [100, stage, the hidden bits, window 0's x, y, the shell
**   top's hidden]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"

namespace cbs {

std::vector<int> g_out;
void rec(Int tag, Int a = 0, Int b = 0, Int c = 0, Int d = 0, Int e = 0)
{
	Int w[6] = { tag, a, b, c, d, e };
	g_out.insert(g_out.end(), w, w + 6);
}

/* ---- the windows ---- */
class GameWindow
{
public:
	Int m_id;
	Bool m_hidden;
	Int m_x, m_y;
	Bool winIsHidden( void ) { return m_hidden; }
	Int winHide( Bool hide ) { rec(1, m_id, hide ? 1 : 0); m_hidden = hide; return 0; }
	Int winSetPosition( Int x, Int y ) { rec(2, m_id, x, y); m_x = x; m_y = y; return 0; }
};
GameWindow g_win[4];
Bool g_parentFound, g_buttonLarge, g_replayExists;

typedef Int NameKeyType;
enum { KEY_PARENT = 1, KEY_BUTTON_LARGE = 2, KEY_OTHER = 3 };
class NameKeyGenerator
{
public:
	NameKeyType nameToKey( const AsciiString &s )
	{
		if (!strcmp(s.str(), "ControlBar.wnd:ControlBarParent")) return KEY_PARENT;
		if (!strcmp(s.str(), "ControlBar.wnd:ButtonLarge")) return KEY_BUTTON_LARGE;
		return KEY_OTHER;
	}
};
NameKeyGenerator g_keys;
NameKeyGenerator *TheNameKeyGenerator = &g_keys;

class GameWindowManager
{
public:
	GameWindow *winCreateFromScript( AsciiString name ) { if (!strcmp(name.str(), "ControlBar.wnd")) rec(15); else rec(99); return NULL; }
	GameWindow *winGetWindowFromId( GameWindow *, Int id )
	{
		if (id == KEY_PARENT) return g_parentFound ? &g_win[0] : NULL;
		if (id == KEY_BUTTON_LARGE) return g_buttonLarge ? &g_win[1] : NULL;
		return NULL;
	}
};
GameWindowManager g_wm;
GameWindowManager *TheWindowManager = &g_wm;

/* ---- the images and the button's setters ---- */
struct Image { Int m_id; };
Image g_images[7] = { {0}, {1}, {2}, {3}, {4}, {5}, {6} };
GameWindow *g_imageWin;
Int g_imageCalls[3];
static void imageSet(GameWindow *w, Int slot, const Image *i)
{
	g_imageWin = w;
	g_imageCalls[slot] = i ? i->m_id : 0;
	if (slot == 2) rec(4, w->m_id, g_imageCalls[0], g_imageCalls[1], g_imageCalls[2]);
}
void GadgetButtonSetEnabledImage( GameWindow *w, const Image *i ) { imageSet(w, 0, i); }
void GadgetButtonSetHiliteImage( GameWindow *w, const Image *i ) { imageSet(w, 1, i); }
void GadgetButtonSetHiliteSelectedImage( GameWindow *w, const Image *i ) { imageSet(w, 2, i); }

/* ---- the display, the view, the recorder, the logic, the shell ---- */
struct DisplayStub
{
	UnsignedInt m_height;
	UnsignedInt getHeight() { return m_height; }
	void enableLetterBox( Bool enable ) { rec(14, enable ? 1 : 0); }
};
DisplayStub g_display;
DisplayStub *TheDisplay = &g_display;
struct TacticalViewStub { void setHeight( Int h ) { rec(3, h); } };
TacticalViewStub g_view;
TacticalViewStub *TheTacticalView = &g_view;
enum RecorderModeType { RECORDERMODETYPE_RECORD, RECORDERMODETYPE_PLAYBACK, RECORDERMODETYPE_NONE };
struct RecorderStub { RecorderModeType m_mode; RecorderModeType getMode() { return m_mode; } };
RecorderStub g_recorder;
RecorderStub *TheRecorder = &g_recorder;
struct GameLogicStub { Bool m_replay; Bool isInReplayGame() { return m_replay; } };
GameLogicStub g_logic;
GameLogicStub *TheGameLogic = &g_logic;
struct WindowLayout
{
	Bool m_hidden;
	Bool isHidden( void ) { return m_hidden; }
	void hide( Bool hide ) { rec(13, hide ? 1 : 0); m_hidden = hide; }
};
WindowLayout g_top;
Bool g_topExists;
struct ShellStub
{
	Bool m_active;
	Bool isShellActive( void ) { return m_active; }
	WindowLayout *top( void ) { return g_topExists ? &g_top : NULL; }
};
ShellStub g_shell;
ShellStub *TheShell = &g_shell;

/* InGameUI.cpp's file-scope window (line 108) */
GameWindow *m_replayWindow = NULL;

/* ---- the animate window manager ---- */
enum AnimTypes { WIN_ANIMATION_NONE = 0, WIN_ANIMATION_SLIDE_RIGHT, WIN_ANIMATION_SLIDE_RIGHT_FAST, WIN_ANIMATION_SLIDE_LEFT,
	WIN_ANIMATION_SLIDE_TOP, WIN_ANIMATION_SLIDE_BOTTOM };
class AnimateWindowManager
{
public:
	void reset( void ) { rec(10); }
	void registerGameWindow( GameWindow *w, AnimTypes t, Bool needsToFinish, UnsignedInt ms, UnsignedInt delayMs )
	{
		/* only the slide-in reaches here */
		if (t != WIN_ANIMATION_SLIDE_BOTTOM || !needsToFinish || ms != 500 || delayMs != 0) rec(99);
		rec(11, w->m_id);
	}
	void reverseAnimateWindow( void ) { rec(12); }
};
AnimateWindowManager g_anim;

/* ---- TheControlBar ---- */
#include "cbs_enum.inc"
enum ControlBarContextParent { CP_MASTER, CP_PURCHASE_SCIENCE, NUM_CONTEXT_PARENTS };
class ControlBar
{
public:
	ControlBarStages m_currentControlBarStage;
	GameWindow *m_contextParent[ NUM_CONTEXT_PARENTS ];
	ICoord2D m_defaultControlBarPosition;
	const Image *m_toggleButtonUpIn, *m_toggleButtonUpOn, *m_toggleButtonUpPushed;
	const Image *m_toggleButtonDownIn, *m_toggleButtonDownOn, *m_toggleButtonDownPushed;
	AnimateWindowManager *m_animateWindowManager;

	void toggleControlBarStage( void );
	void switchControlBarStage( ControlBarStages stage );
	void setDefaultControlBarConfig( void );
	void setSquishedControlBarConfig( void );
	void setLowControlBarConfig( void );
	void setHiddenControlBar( void );
	void setUpDownImages( void );
	void hidePurchaseScience( void );
	void repopulateBuildTooltipLayout( void ) { rec(8); }
	void showSpecialPowerShortcut( void ) { rec(5); }
	void hideSpecialPowerShortcut( void ) { rec(6); }
	void animateSpecialPowerShortcut( Bool on ) { rec(7, on ? 1 : 0); }
	void markUIDirty( void ) { rec(9); }
};
ControlBar g_bar;
ControlBar *TheControlBar = &g_bar;

/* GUICallbacks.h:323-325's declarations (the defaults the meta case's `ToggleControlBar()` takes) */
void ToggleControlBar( Bool immediate = TRUE );
void HideControlBar( Bool immediate = TRUE );
void ShowControlBar( Bool immediate = TRUE );

#include "cbs_stage.inc"
#include "cbs_low.inc"
#include "cbs_updown.inc"
#include "cbs_purchase.inc"
#include "cbs_replay.inc"
#include "cbs_callback.inc"

/* ---- the meta case ---- */
enum GameMessageDisposition { KEEP_MESSAGE, DESTROY_MESSAGE };
struct GameMessage { enum Type { MSG_META_TOGGLE_CONTROL_BAR = 107, MSG_META_TOGGLE_ATTACKMOVE }; };
GameMessageDisposition metaCase( GameMessage::Type t )
{
	GameMessageDisposition disp = KEEP_MESSAGE;
	switch (t)
	{
#include "cbs_meta.inc"
		default:
			break;
	}
	return disp;
}

/* ---- #138: the other callers ---- */
class ScriptActions { public: void doLetterBoxMode(Bool startLetterbox); };
#include "cbs_letterbox.inc"
ScriptActions g_actions;
ScriptActions *TheScriptActions = &g_actions;

class InGameUI { public: void createControlBar( void ); };
#include "cbs_create.inc"
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;

struct SubsystemListStub { void resetAll() { rec(17); } };
SubsystemListStub g_subsystems;
SubsystemListStub *TheSubsystemList = &g_subsystems;
void gameEngineInitEnd( void )
{
#include "cbs_engine.inc"
}

/* GameType.h's GameMode */
enum GameMode { GAME_SINGLE_PLAYER, GAME_LAN, GAME_SKIRMISH, GAME_REPLAY, GAME_SHELL, GAME_INTERNET, GAME_NONE };
struct GameLogicStart
{
	GameMode m_gameMode;
	void endOfStartNewGame( void )
	{
#include "cbs_new_game.inc"
	}
};
GameLogicStart g_start;

struct GameLogicClear
{
	void closeWindows( void ) { rec(18); }
	void clearGameDataControlBar( void )
	{
#include "cbs_clear.inc"
	}
};
GameLogicClear g_clear;

static void state(void)
{
	Int bits = 0;
	for (Int i = 0; i < 4; ++i)
		if (g_win[i].m_hidden) bits |= 1 << i;
	rec(100, (Int)g_bar.m_currentControlBarStage, bits, g_win[0].m_x, g_win[0].m_y, g_top.m_hidden ? 1 : 0);
}

} // namespace cbs

extern "C" int ref_control_bar_stage_seq(const int *in, int nin, int *out, int cap)
{
	using namespace cbs;
	g_out.clear();
	for (Int i = 0; i < 4; ++i) {
		g_win[i].m_id = i;
		g_win[i].m_hidden = (in[12] >> i) & 1;
		g_win[i].m_x = g_win[i].m_y = 0;
	}
	g_win[0].m_x = in[13];
	g_win[0].m_y = in[14];
	g_win[1].m_x = in[15];
	g_bar.m_currentControlBarStage = (ControlBarStages)in[0];
	g_bar.m_defaultControlBarPosition.x = in[1];
	g_bar.m_defaultControlBarPosition.y = in[2];
	g_display.m_height = (UnsignedInt)in[3];
	g_recorder.m_mode = in[4] ? RECORDERMODETYPE_PLAYBACK : RECORDERMODETYPE_RECORD;
	g_logic.m_replay = in[5] != 0;
	g_shell.m_active = in[6] != 0;
	g_topExists = in[7] != 0;
	g_top.m_hidden = in[7] == 2;
	g_bar.m_animateWindowManager = in[8] ? &g_anim : NULL;
	g_parentFound = in[9] != 0;
	g_buttonLarge = in[10] != 0;
	g_replayExists = in[11] != 0;
	m_replayWindow = g_replayExists ? &g_win[2] : NULL;
	g_bar.m_contextParent[CP_MASTER] = &g_win[0];
	g_bar.m_contextParent[CP_PURCHASE_SCIENCE] = &g_win[3];
	g_bar.m_toggleButtonUpIn = &g_images[1];
	g_bar.m_toggleButtonUpOn = &g_images[2];
	g_bar.m_toggleButtonUpPushed = &g_images[3];
	g_bar.m_toggleButtonDownIn = &g_images[4];
	g_bar.m_toggleButtonDownOn = &g_images[5];
	g_bar.m_toggleButtonDownPushed = &g_images[6];
	for (int k = 16; k + 4 <= nin; k += 4) {
		const int *w = in + k;
		switch (w[0]) {
		case 0: rec(20, (Int)metaCase(GameMessage::MSG_META_TOGGLE_CONTROL_BAR)); break;
		case 1: TheControlBar->toggleControlBarStage(); break;
		case 2: TheControlBar->switchControlBarStage((ControlBarStages)w[1]); break;
		case 3: ShowControlBar(w[1] != 0); break;
		case 4: HideControlBar(w[1] != 0); break;
		case 5: ToggleControlBar(w[1] != 0); break;
		case 6:
			switch (w[1]) {
			case 0: g_recorder.m_mode = w[2] ? RECORDERMODETYPE_PLAYBACK : RECORDERMODETYPE_RECORD; break;
			case 1: g_logic.m_replay = w[2] != 0; break;
			case 2: g_shell.m_active = w[2] != 0; break;
			case 3: if (g_topExists) g_top.m_hidden = w[2] != 0; break;
			case 4: g_display.m_height = (UnsignedInt)w[2]; break;
			case 5: g_win[w[2] & 3].winHide(w[3] != 0); break;
			}
			break;
		case 7: TheScriptActions->doLetterBoxMode(w[1] != 0); break;
		case 8: TheInGameUI->createControlBar(); break;
		case 9: gameEngineInitEnd(); break;
		case 10: g_start.m_gameMode = w[1] ? GAME_SHELL : GAME_SKIRMISH; g_start.endOfStartNewGame(); break;
		case 11: g_clear.clearGameDataControlBar(); break;
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
