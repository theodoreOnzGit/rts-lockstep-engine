/* Differential-test wrapper for the animate window manager and the control bar's use of it (GitHub #137, upstream
** commit 0a05454d), cut verbatim by build.rs into OUT_DIR/animate_window_gen:
**   aw_mgr_h.inc     GameEngine/Include/GameClient/AnimateWindowManager.h, from the forward references
**                    (`class GameWindow;`) to `// EXTERNALS`: AnimTypes, AnimateWindow, AnimateWindowList,
**                    AnimateWindowManager and the inline accessors;
**   aw_proc_h.inc    GameEngine/Include/GameClient/ProcessAnimateWindow.h, from `class AnimateWindow;` to `// INLINING`:
**                    ProcessAnimateWindow and its eight processes;
**   aw_mgr.inc       GameEngine/Source/GameClient/GUI/AnimateWindowManager.cpp, from `AnimateWindow::AnimateWindow( void )`
**                    to the end;
**   aw_proc.inc      GameEngine/Source/GameClient/GUI/ProcessAnimateWindow.cpp, from the slide from the right's
**                    constructor to the end;
**   aw_gw_norm.inc   GameEngine/Source/GameClient/GUI/GameWindow.cpp, GameWindow::normalizeWindowRegion (to the comment
**                    of findFirstLeaf);
**   aw_gw_pos.inc    GameWindow.cpp, GameWindow::winSetPosition and winGetPosition (to the comment of
**                    winSetCursorPosition);
**   aw_gw_size.inc   GameWindow.cpp, GameWindow::winGetSize (to the comment of winEnable);
**   aw_cb_init.inc   GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp, ControlBar::init's three
**                    `NEW AnimateWindowManager` lines (to `m_buildToolTipLayout = `);
**   aw_cb_reset.inc  ControlBar.cpp, ControlBar::reset's three managers' resets (to `if(m_videoManager)`);
**   aw_cb_update.inc ControlBar.cpp, ControlBar::update's animate lines (from the bar's manager's update to
**                    `if( !m_buildToolTipLayout->isHidden())`);
**   aw_cb_short.inc  ControlBar.cpp, ControlBar::animateSpecialPowerShortcut (to showSpecialPowerShortcut).
** No edit. They are compiled inside namespace aw (so that they meet no other wrapper's stand-ins of the same names), with
** `private` and `protected` opened for the headers' cuts only, so that the dump can read the lists (the classes' layout
** does not change). Stand-ins written for this test: GameWindow (its region, size, hidden bit and user data; the three
** cut functions above, and winIsHidden / winHide / winGetUserData as GameWindow.cpp's for this state: winHide sets or
** clears WIN_STATUS_HIDDEN, the rest of it is the window manager's focus, which nothing here reads), TheDisplay (its
** width), timeGetTime() (the case's clock), TheNameKeyGenerator / TheWindowManager (`ControlBar.wnd:ControlBarParent`
** found, as window 0, when the case names it), `min` (windows.h's macro, as a function), and a ControlBar holding the
** members the cuts read. `cos` and `sin` of a float are cosf and sinf (#77's reading: MSVC's float overloads, see
** differential/shim_wt/PreRTS.h), which the test binary binds to petir's (tests/support/mod.rs).
**
** Entry point (extern "C", tests/differential_animate_window.rs):
**   ref_animate_window_seq(in, nin, out, cap) runs one sequence and writes its words to out.
**   `in`: the setup, 7 words [the display's width at init, the windows' count n, ControlBarParent named (window 0),
**   the shortcut parent's window (-1 none), m_currentlyUsedSpecialPowersButtons, the buttons' user data (bit i set:
**   button i has some), the clock], then n windows of 5 words [x, y, width, height, hidden], then steps of 7 words
**   [kind, a, b, c, d, e, f]; manager m: 0 m_animateWindowManager, 1 m_animateWindowManagerForGenShortcuts,
**   2 m_generalsScreenAnimate:
**     kind 0  manager a's registerGameWindow(window b, type c, needsToFinish d, ms e, delayMs f)
**     kind 1  manager a's reverseAnimateWindow()
**     kind 2  manager a's update()
**     kind 3  manager a's reset()
**     kind 4  manager a's resetToRestPosition()
**     kind 5  manager a's init()
**     kind 6  the clock moved on by a (unsigned)
**     kind 7  ControlBar::update's animate lines
**     kind 8  ControlBar::reset's managers' lines
**     kind 9  ControlBar::animateSpecialPowerShortcut(a)
**     kind 10 the display's width set to a
**     kind 11 window a's winSetPosition(b, c)
**     kind 12 window a's winHide(b)
**   After each step the dump: [300, windows], each window [x, y, hidden]; then each manager [301, !isFinished,
**   isReversed, isEmpty, the must-finish list's size, the plain list's size], and each animation of the must-finish
**   list, then the plain list, of 17 words [window, type, cur x, y, start x, y, end x, y, rest x, y, the velocity's x
**   and y bits, start time, end time, delay, finished, needsToFinish]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <list>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"
#include "Common/GameMemory.h"
#include "Common/SubsystemInterface.h"

namespace aw {

std::vector<int> g_out;
void put(Int v) { g_out.push_back(v); }

/* ---- the clock, the display, min, cos and sin ---- */
UnsignedInt g_now;
UnsignedInt timeGetTime( void ) { return g_now; }
class Display
{
public:
	UnsignedInt m_width;
	UnsignedInt getWidth( void ) { return m_width; }
};
Display g_display;
Display *TheDisplay = &g_display;
inline Int min( Int a, Int b ) { return (a) < (b) ? (a) : (b); }
static inline float cos( float x ) { return cosf(x); }
static inline float sin( float x ) { return sinf(x); }

/* ---- the windows ---- */
enum { WIN_STATUS_HIDDEN = 0x00000002 };
class GameWindow
{
public:
	IRegion2D m_region;
	ICoord2D m_size;
	UnsignedInt m_status;
	void *m_userData;
	void normalizeWindowRegion( void );
	Int winSetPosition( Int x, Int y );
	Int winGetPosition( Int *x, Int *y );
	Int winGetSize( Int *width, Int *height );
	Bool winIsHidden( void ) { return BitTest( m_status, WIN_STATUS_HIDDEN ); }
	Int winHide( Bool hide )
	{
		if( hide ) BitSet( m_status, WIN_STATUS_HIDDEN );
		else BitClear( m_status, WIN_STATUS_HIDDEN );
		return 0;
	}
	void *winGetUserData( void ) { return m_userData; }
};
enum { WIN_ERR_OK = 0, WIN_ERR_INVALID_PARAMETER = -1 };
#include "aw_gw_norm.inc"
#include "aw_gw_pos.inc"
#include "aw_gw_size.inc"

enum { MAX_WINDOWS = 8, MAX_SPECIAL_POWER_SHORTCUTS = 11 };
GameWindow g_win[MAX_WINDOWS];
Int g_nwin;
GameWindow g_buttons[MAX_SPECIAL_POWER_SHORTCUTS];
Bool g_parentNamed;
Int win_index( GameWindow *w ) { return w ? (Int)(w - g_win) : -1; }

typedef Int NameKeyType;
class NameKeyGenerator
{
public:
	NameKeyType nameToKey( const AsciiString &s ) { return !strcmp(s.str(), "ControlBar.wnd:ControlBarParent") ? 1 : 2; }
};
NameKeyGenerator g_keys;
NameKeyGenerator *TheNameKeyGenerator = &g_keys;
class GameWindowManager
{
public:
	GameWindow *winGetWindowFromId( GameWindow *parent, NameKeyType id ) { return (parent == NULL && id == 1 && g_parentNamed) ? &g_win[0] : NULL; }
};
GameWindowManager g_wm;
GameWindowManager *TheWindowManager = &g_wm;

/* ---- the cuts ---- */
#define private public
#define protected public
#include "aw_mgr_h.inc"
#include "aw_proc_h.inc"
#undef private
#undef protected
#include "aw_mgr.inc"
#include "aw_proc.inc"

/* ---- TheControlBar ---- */
class ControlBar
{
public:
	AnimateWindowManager *m_animateWindowManager;
	AnimateWindowManager *m_generalsScreenAnimate;
	AnimateWindowManager *m_animateWindowManagerForGenShortcuts;
	GameWindow *m_specialPowerShortcutParent;
	GameWindow *m_specialPowerShortcutButtons[ MAX_SPECIAL_POWER_SHORTCUTS ];
	Int m_currentlyUsedSpecialPowersButtons;
	void initAnim( void );
	void resetAnim( void );
	void updateAnim( void );
	void animateSpecialPowerShortcut( Bool isOn );
};

void ControlBar::initAnim( void )
{
#include "aw_cb_init.inc"
}
void ControlBar::resetAnim( void )
{
#include "aw_cb_reset.inc"
}
void ControlBar::updateAnim( void )
{
#include "aw_cb_update.inc"
}
#include "aw_cb_short.inc"

void dump_list( AnimateWindowList &l )
{
	for (AnimateWindowList::iterator it = l.begin(); it != l.end(); ++it)
	{
		AnimateWindow *a = *it;
		unsigned int vx, vy;
		memcpy(&vx, &a->m_vel.x, 4);
		memcpy(&vy, &a->m_vel.y, 4);
		Int w[17] = { win_index(a->m_win), (Int)a->m_animType, a->m_curPos.x, a->m_curPos.y, a->m_startPos.x, a->m_startPos.y,
			a->m_endPos.x, a->m_endPos.y, a->m_restPos.x, a->m_restPos.y, (Int)vx, (Int)vy, (Int)a->m_startTime,
			(Int)a->m_endTime, (Int)a->m_delay, a->m_isFinished ? 1 : 0, a->m_needsToFinish ? 1 : 0 };
		g_out.insert(g_out.end(), w, w + 17);
	}
}

void dump( ControlBar &cb )
{
	put(300);
	put(g_nwin);
	for (Int i = 0; i < g_nwin; ++i)
	{
		put(g_win[i].m_region.lo.x);
		put(g_win[i].m_region.lo.y);
		put(g_win[i].winIsHidden() ? 1 : 0);
	}
	AnimateWindowManager *m[3] = { cb.m_animateWindowManager, cb.m_animateWindowManagerForGenShortcuts, cb.m_generalsScreenAnimate };
	for (Int i = 0; i < 3; ++i)
	{
		put(301);
		put(m[i]->isFinished() ? 0 : 1);
		put(m[i]->isReversed() ? 1 : 0);
		put(m[i]->isEmpty() ? 1 : 0);
		put((Int)m[i]->m_winMustFinishList.size());
		put((Int)m[i]->m_winList.size());
		dump_list(m[i]->m_winMustFinishList);
		dump_list(m[i]->m_winList);
	}
}

ControlBar g_cb;

}  // namespace aw

extern "C" int ref_animate_window_seq(const int *in, int nin, int *out, int cap)
{
	using namespace aw;
	g_out.clear();
	/* the last case's managers go first (their destructors put their windows at rest: the windows are set again below) */
	delete g_cb.m_animateWindowManager;
	delete g_cb.m_generalsScreenAnimate;
	delete g_cb.m_animateWindowManagerForGenShortcuts;
	memset(&g_cb, 0, sizeof(g_cb));
	g_display.m_width = (UnsignedInt)in[0];
	g_nwin = in[1];
	g_parentNamed = in[2] != 0;
	g_now = (UnsignedInt)in[6];
	memset(g_win, 0, sizeof(g_win));
	memset(g_buttons, 0, sizeof(g_buttons));
	for (Int i = 0; i < g_nwin; ++i)
	{
		const int *w = in + 7 + 5 * i;
		g_win[i].m_size.x = w[2];
		g_win[i].m_size.y = w[3];
		g_win[i].winSetPosition(w[0], w[1]);
		g_win[i].winHide(w[4] != 0);
	}
	g_cb.m_specialPowerShortcutParent = in[3] >= 0 ? &g_win[in[3]] : NULL;
	g_cb.m_currentlyUsedSpecialPowersButtons = in[4];
	for (Int i = 0; i < MAX_SPECIAL_POWER_SHORTCUTS; ++i)
	{
		g_buttons[i].m_userData = (in[5] >> i) & 1 ? (void *)&g_buttons[i] : NULL;
		g_cb.m_specialPowerShortcutButtons[i] = &g_buttons[i];
	}
	g_cb.initAnim();
	AnimateWindowManager *m[3] = { g_cb.m_animateWindowManager, g_cb.m_animateWindowManagerForGenShortcuts, g_cb.m_generalsScreenAnimate };
	for (int i = 7 + 5 * g_nwin; i + 7 <= nin; i += 7)
	{
		const int *s = in + i;
		switch (s[0])
		{
			case 0: m[s[1]]->registerGameWindow(&g_win[s[2]], (AnimTypes)s[3], s[4] != 0, (UnsignedInt)s[5], (UnsignedInt)s[6]); break;
			case 1: m[s[1]]->reverseAnimateWindow(); break;
			case 2: m[s[1]]->update(); break;
			case 3: m[s[1]]->reset(); break;
			case 4: m[s[1]]->resetToRestPosition(); break;
			case 5: m[s[1]]->init(); break;
			case 6: g_now += (UnsignedInt)s[1]; break;
			case 7: g_cb.updateAnim(); break;
			case 8: g_cb.resetAnim(); break;
			case 9: g_cb.animateSpecialPowerShortcut(s[1] != 0); break;
			case 10: g_display.m_width = (UnsignedInt)s[1]; break;
			case 11: g_win[s[1]].winSetPosition(s[2], s[3]); break;
			case 12: g_win[s[1]].winHide(s[2] != 0); break;
		}
		dump(g_cb);
	}
	int n = (int)g_out.size();
	for (int i = 0; i < n && i < cap; ++i) out[i] = g_out[i];
	return n;
}
