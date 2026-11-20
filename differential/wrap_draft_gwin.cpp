/* Differential-test wrapper for the GameWindow draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/game_window.rs, cut verbatim by build.rs (drafts_guilogic4) into:
**   gw_status.inc   GameClient/GameWindow.h, the window status enum (to "Message Box Button flags");
**   gw_err.inc      GameClient/GameWindow.h, the WIN_ERR enum;
**   gw_maxdraw.inc  GameClient/WinInstanceData.h, the MAX_DRAW_DATA enum;
**   gw_inl.inc      GameClient/WinInstanceData.h, the inline getText / getTextLength / getTooltipText / getTooltipTextLength;
**   gw_settext.inc  WinInstanceData.cpp, setTooltipText and setText;
**   gw_leaf.inc     GameWindow.cpp, findFirstLeaf, findLastLeaf, findPrevLeaf, findNextLeaf;
**   gw_cursor.inc, gw_region.inc, gw_offset.inc (winSetDrawOffset, winGetDrawOffset), gw_textlen.inc, gw_colors.inc (the text colour
**   getters), gw_instdata.inc (winSetInstanceData), gw_id.inc, gw_layout.inc, gw_funcs.inc (winSetSystemFunc to winDrawWindow),
**   gw_drawdata.inc (the draw data setters), gw_getfuncs.inc (winGetInputFunc to winGetDrawFunc).
** Stand-ins (outside the cuts): the types Int .. Color, ICoord2D, IRegion2D, WinDrawData, TextDrawData (WinInstanceData.h, the structs as
** they are); UnicodeString (a counted array of 16-bit characters, 64 at most) and DisplayString (holds one: setText, getText,
** getTextLength); TheDisplayStringManager->newDisplayString() (NEW DisplayString); WinInstanceData (the data members of
** WinInstanceData.h that winSetInstanceData copies, the pointers copied as the compiler-made assignment copies them; the
** constructor's init() is replaced by the test setting every field); GameWinSystemFunc, GameWinInputFunc, GameWinDrawFunc,
** GameWinTooltipFunc (function pointers upstream; a small class holding a handle here, `operator bool` for `if( system )`, and
** for the draw callback an operator() that counts the call); TheWindowManager (getDefaultSystem / Input / Draw, which return
** the W3D manager's defaults: handles 1000, 3000, 2000); DEBUG_LOG and DEBUG_ASSERTCRASH (compiled out in the release build, as is assert).
** GameWindow keeps only the members the cuts use.
**
** Entry points (extern "C", tests/differential_draft_gwin.rs): see the declarations there. */
#include <assert.h>
#include <string.h>
#define DEBUG_LOG(x)
#define DEBUG_ASSERTCRASH(x, y)
#ifndef NULL
#define NULL 0
#endif
#define TRUE 1
#define FALSE 0
#define BitTest( x, i ) ( ( (x) & (i) ) != 0 )
typedef int Int;
typedef unsigned int UnsignedInt;
typedef int Bool;
typedef int Color;
struct ICoord2D { Int x, y; };
struct IRegion2D { ICoord2D lo, hi; };
class Image;
class GameFont;
class VideoBuffer;
class WindowLayout;
#include "gw_status.inc"
#include "gw_err.inc"
#include "gw_maxdraw.inc"
struct WinDrawData { const Image *image; Color color; Color borderColor; };
struct TextDrawData { Color color; Color borderColor; };

struct UnicodeString { Int len; unsigned short s[64]; static UnicodeString TheEmptyString; };
UnicodeString UnicodeString::TheEmptyString = { 0, { 0 } };
class DisplayString
{
public:
	UnicodeString m_t;
	void setText( UnicodeString t ) { m_t = t; }
	UnicodeString getText( void ) { return m_t; }
	Int getTextLength( void ) { return m_t.len; }
};
struct DisplayStringManagerStandIn { DisplayString *newDisplayString( void ) { return new DisplayString; } };
static DisplayStringManagerStandIn theDsm;
static DisplayStringManagerStandIn *TheDisplayStringManager = &theDsm;

class WinInstanceData
{
public:
	void setTooltipText( UnicodeString tip );
	void setText( UnicodeString text );
	UnicodeString getTooltipText( void );
	UnicodeString getText( void );
	Int getTextLength( void );
	Int getTooltipTextLength( void );
	Int m_id;
	Int m_state;
	UnsignedInt m_style;
	UnsignedInt m_status;
	WinDrawData m_enabledDrawData[ MAX_DRAW_DATA ];
	WinDrawData m_disabledDrawData[ MAX_DRAW_DATA ];
	WinDrawData m_hiliteDrawData[ MAX_DRAW_DATA ];
	TextDrawData m_enabledText;
	TextDrawData m_disabledText;
	TextDrawData m_hiliteText;
	TextDrawData m_imeCompositeText;
	ICoord2D m_imageOffset;
	Int m_tooltipDelay;
	DisplayString *m_text;
	DisplayString *m_tooltip;
	VideoBuffer *m_videoBuffer;
};
#include "gw_inl.inc"
#include "gw_settext.inc"

class GameWindow;
static int theDraws;
struct GameWinSystemFunc { int h; GameWinSystemFunc() : h(0) {} GameWinSystemFunc( int v ) : h(v) {} explicit operator bool() const { return h != 0; } };
struct GameWinInputFunc { int h; GameWinInputFunc() : h(0) {} GameWinInputFunc( int v ) : h(v) {} explicit operator bool() const { return h != 0; } };
struct GameWinTooltipFunc { int h; GameWinTooltipFunc() : h(0) {} GameWinTooltipFunc( int v ) : h(v) {} explicit operator bool() const { return h != 0; } };
struct GameWinDrawFunc { int h; GameWinDrawFunc() : h(0) {} GameWinDrawFunc( int v ) : h(v) {} explicit operator bool() const { return h != 0; }
	void operator()( GameWindow *, WinInstanceData * ) const { ++theDraws; } };
struct WindowManagerStandIn
{
	GameWinSystemFunc getDefaultSystem( void ) { return GameWinSystemFunc( 1000 ); }
	GameWinInputFunc getDefaultInput( void ) { return GameWinInputFunc( 3000 ); }
	GameWinDrawFunc getDefaultDraw( void ) { return GameWinDrawFunc( 2000 ); }
};
static WindowManagerStandIn theWm;
static WindowManagerStandIn *TheWindowManager = &theWm;

class GameWindow
{
public:
	GameWindow *findFirstLeaf( void );
	GameWindow *findLastLeaf( void );
	GameWindow *findPrevLeaf( void );
	GameWindow *findNextLeaf( void );
	Int winGetCursorPosition( Int *x, Int *y );
	Int winGetRegion( IRegion2D *region );
	void winSetDrawOffset( Int x, Int y );
	void winGetDrawOffset( Int *x, Int *y );
	Int winGetTextLength();
	Color winGetEnabledTextColor( void );
	Color winGetEnabledTextBorderColor( void );
	Color winGetDisabledTextColor( void );
	Color winGetDisabledTextBorderColor( void );
	Color winGetIMECompositeTextColor( void );
	Color winGetIMECompositeBorderColor( void );
	Color winGetHiliteTextColor( void );
	Color winGetHiliteTextBorderColor( void );
	Int winSetInstanceData( WinInstanceData *data );
	Int winSetWindowId( Int id );
	void winSetLayout( WindowLayout *layout );
	Int winSetSystemFunc( GameWinSystemFunc system );
	Int winSetInputFunc( GameWinInputFunc input );
	Int winSetDrawFunc( GameWinDrawFunc draw );
	Int winSetTooltipFunc( GameWinTooltipFunc tooltip );
	Int winSetCallbacks( GameWinInputFunc input, GameWinDrawFunc draw, GameWinTooltipFunc tooltip );
	Int winDrawWindow( void );
	Int winSetEnabledImage( Int index, const Image *image );
	Int winSetEnabledColor( Int index, Color color );
	Int winSetEnabledBorderColor( Int index, Color color );
	Int winSetDisabledImage( Int index, const Image *image );
	Int winSetDisabledColor( Int index, Color color );
	Int winSetDisabledBorderColor( Int index, Color color );
	Int winSetHiliteImage( Int index, const Image *image );
	Int winSetHiliteColor( Int index, Color color );
	Int winSetHiliteBorderColor( Int index, Color color );
	GameWinInputFunc winGetInputFunc( void );
	GameWinSystemFunc winGetSystemFunc( void );
	GameWinTooltipFunc winGetTooltipFunc( void );
	GameWinDrawFunc winGetDrawFunc( void );

	GameWindow *m_next, *m_prev, *m_parent, *m_child;
	UnsignedInt m_status;
	Int m_cursorX, m_cursorY;
	IRegion2D m_region;
	WinInstanceData m_instData;
	GameWinSystemFunc m_system;
	GameWinInputFunc m_input;
	GameWinDrawFunc m_draw;
	GameWinTooltipFunc m_tooltip;
	WindowLayout *m_layout;
};
#include "gw_leaf.inc"
#include "gw_cursor.inc"
#include "gw_region.inc"
#include "gw_offset.inc"
#include "gw_textlen.inc"
#include "gw_colors.inc"
#include "gw_instdata.inc"
#include "gw_id.inc"
#include "gw_layout.inc"
#include "gw_funcs.inc"
#include "gw_drawdata.inc"
#include "gw_getfuncs.inc"

/* ---- the leaf functions: a forest given by arrays (index -1: NULL) ---- */
extern "C" int ref_gwin_leaf( int n, const int *parent, const int *child, const int *next, const int *prev, const unsigned *status, int which, int start )
{
	GameWindow *w = new GameWindow[n];
	for (int i = 0; i < n; ++i)
	{
		memset( &w[i], 0, sizeof( GameWindow ) );
		w[i].m_parent = parent[i] < 0 ? NULL : &w[parent[i]];
		w[i].m_child = child[i] < 0 ? NULL : &w[child[i]];
		w[i].m_next = next[i] < 0 ? NULL : &w[next[i]];
		w[i].m_prev = prev[i] < 0 ? NULL : &w[prev[i]];
		w[i].m_status = status[i];
	}
	GameWindow *r = which == 0 ? w[start].findFirstLeaf() : which == 1 ? w[start].findLastLeaf() : which == 2 ? w[start].findPrevLeaf() : w[start].findNextLeaf();
	int ri = r ? (int)(r - w) : -1;
	delete [] w;
	return ri;
}

/* ---- one window with its data: set up, operate, dump ---- */
static GameWindow theW;
static DisplayString theTextDs, theTipDs;
static DisplayString dataTextDs, dataTipDs;
static WinInstanceData theData;
static int dumpTextTag( WinInstanceData *d, long long *o, int k )
{
	UnicodeString t = d->getText();
	o[k++] = t.len;
	for (int i = 0; i < 8; ++i) o[k++] = i < t.len ? t.s[i] : -1;
	UnicodeString p = d->getTooltipText();
	o[k++] = d->getTooltipTextLength();
	for (int i = 0; i < 8; ++i) o[k++] = i < p.len ? p.s[i] : -1;
	return k;
}
static void fillDs( DisplayString *ds, WinInstanceData *d, int tooltip, const long long *init )
{
	/* init: length (-1: no DisplayString), then up to 8 characters */
	int len = (int)init[0];
	if (len < 0) { if (tooltip) d->m_tooltip = NULL; else d->m_text = NULL; return; }
	ds->m_t.len = len;
	for (int i = 0; i < len; ++i) ds->m_t.s[i] = (unsigned short)init[1 + i];
	if (tooltip) d->m_tooltip = ds; else d->m_text = ds;
}
/* the window's initial state: v = [status, cursorX, cursorY, region (4), text colours (8), id, state, style, instStatus, offset (2), delay,
** then the text (1 + 8) and the tooltip text (1 + 8), then three draw data sets (9 * 3 * 3)] */
extern "C" void ref_gwin_setup( const long long *v )
{
	int k = 0;
	memset( &theW, 0, sizeof( theW ) );
	theW.m_status = (unsigned)v[k++];
	theW.m_cursorX = (int)v[k++]; theW.m_cursorY = (int)v[k++];
	theW.m_region.lo.x = (int)v[k++]; theW.m_region.lo.y = (int)v[k++]; theW.m_region.hi.x = (int)v[k++]; theW.m_region.hi.y = (int)v[k++];
	WinInstanceData &d = theW.m_instData;
	d.m_enabledText.color = (int)v[k++]; d.m_enabledText.borderColor = (int)v[k++];
	d.m_disabledText.color = (int)v[k++]; d.m_disabledText.borderColor = (int)v[k++];
	d.m_hiliteText.color = (int)v[k++]; d.m_hiliteText.borderColor = (int)v[k++];
	d.m_imeCompositeText.color = (int)v[k++]; d.m_imeCompositeText.borderColor = (int)v[k++];
	d.m_id = (int)v[k++]; d.m_state = (int)v[k++]; d.m_style = (unsigned)v[k++]; d.m_status = (unsigned)v[k++];
	d.m_imageOffset.x = (int)v[k++]; d.m_imageOffset.y = (int)v[k++];
	d.m_tooltipDelay = (int)v[k++];
	fillDs( &theTextDs, &d, 0, v + k ); k += 9;
	fillDs( &theTipDs, &d, 1, v + k ); k += 9;
	WinDrawData *sets[3] = { d.m_enabledDrawData, d.m_disabledDrawData, d.m_hiliteDrawData };
	for (int s = 0; s < 3; ++s)
		for (int i = 0; i < MAX_DRAW_DATA; ++i)
		{
			sets[s][i].image = (const Image *)(long long)v[k++];
			sets[s][i].color = (int)v[k++];
			sets[s][i].borderColor = (int)v[k++];
		}
	theW.m_system = GameWinSystemFunc( 1000 );
	theW.m_input = GameWinInputFunc( 3000 );
	theW.m_draw = GameWinDrawFunc( 2000 );
	theW.m_tooltip = GameWinTooltipFunc( 0 );
	theW.m_layout = NULL;
}

/* the data winSetInstanceData is given: v = [id, state, style, status, offset (2), delay, text colours (6) and the IME colours (2), the text (1 + 8), the tooltip text
** (1 + 8), a video buffer (0: none), three draw data sets] */
extern "C" void ref_gwin_set_data( const long long *v )
{
	int k = 0;
	WinInstanceData &d = theData;
	memset( &d, 0, sizeof( d ) );
	d.m_id = (int)v[k++]; d.m_state = (int)v[k++]; d.m_style = (unsigned)v[k++]; d.m_status = (unsigned)v[k++];
	d.m_imageOffset.x = (int)v[k++]; d.m_imageOffset.y = (int)v[k++];
	d.m_tooltipDelay = (int)v[k++];
	d.m_enabledText.color = (int)v[k++]; d.m_enabledText.borderColor = (int)v[k++];
	d.m_disabledText.color = (int)v[k++]; d.m_disabledText.borderColor = (int)v[k++];
	d.m_hiliteText.color = (int)v[k++]; d.m_hiliteText.borderColor = (int)v[k++];
	d.m_imeCompositeText.color = (int)v[k++]; d.m_imeCompositeText.borderColor = (int)v[k++];
	fillDs( &dataTextDs, &d, 0, v + k ); k += 9;
	fillDs( &dataTipDs, &d, 1, v + k ); k += 9;
	d.m_videoBuffer = (VideoBuffer *)(long long)v[k++];
	WinDrawData *sets[3] = { d.m_enabledDrawData, d.m_disabledDrawData, d.m_hiliteDrawData };
	for (int s = 0; s < 3; ++s)
		for (int i = 0; i < MAX_DRAW_DATA; ++i)
		{
			sets[s][i].image = (const Image *)(long long)v[k++];
			sets[s][i].color = (int)v[k++];
			sets[s][i].borderColor = (int)v[k++];
		}
}

/* one operation; the results go to out */
extern "C" int ref_gwin_op( int op, long long a, long long b, long long c, long long *out )
{
	switch (op)
	{
		case 0: return theW.winSetEnabledColor( (int)a, (int)b );
		case 1: return theW.winSetDisabledImage( (int)a, (const Image *)b );
		case 2: return theW.winSetDisabledColor( (int)a, (int)b );
		case 3: return theW.winSetDisabledBorderColor( (int)a, (int)b );
		case 4: return theW.winSetHiliteImage( (int)a, (const Image *)b );
		case 5: return theW.winSetHiliteColor( (int)a, (int)b );
		case 6: return theW.winSetHiliteBorderColor( (int)a, (int)b );
		case 7: theW.winSetDrawOffset( (int)a, (int)b ); return 0;
		case 8: { int x = 0x7777, y = 0x7777; theW.winGetDrawOffset( a ? &x : NULL, b ? &y : NULL ); out[0] = x; out[1] = y; return 0; }
		case 9: return theW.winSetWindowId( (int)a );
		case 10: return theW.winSetSystemFunc( GameWinSystemFunc( (int)a ) );
		case 11: return theW.winSetDrawFunc( GameWinDrawFunc( (int)a ) );
		case 12: return theW.winSetCallbacks( GameWinInputFunc( (int)a ), GameWinDrawFunc( (int)b ), GameWinTooltipFunc( (int)c ) );
		case 13: theW.winSetLayout( (WindowLayout *)a ); return 0;
		case 14: return theW.winSetInstanceData( &theData );
		case 15: theDraws = 0; theW.winDrawWindow(); return theDraws;
		case 16: { int x = 0x7777, y = 0x7777; int r = theW.winGetCursorPosition( &x, &y ); out[0] = x; out[1] = y; return r; }
		case 17: { IRegion2D g = { { 0x7777, 0x7777 }, { 0x7777, 0x7777 } }; int r = theW.winGetRegion( &g ); out[0] = g.lo.x; out[1] = g.lo.y; out[2] = g.hi.x; out[3] = g.hi.y; return r; }
		case 18: return theW.winGetTextLength();
		case 19: out[0] = theW.winGetEnabledTextColor(); out[1] = theW.winGetDisabledTextColor(); out[2] = theW.winGetDisabledTextBorderColor();
			out[3] = theW.winGetHiliteTextColor(); out[4] = theW.winGetHiliteTextBorderColor(); out[5] = theW.winGetIMECompositeTextColor();
			out[6] = theW.winGetIMECompositeBorderColor(); return 0;
		case 21: theW.m_status = (unsigned)a; return 0;
		case 20: out[0] = theW.winGetSystemFunc().h; out[1] = theW.winGetInputFunc().h; out[2] = theW.winGetDrawFunc().h; out[3] = theW.winGetTooltipFunc().h; return 0;
	}
	return -999;
}

/* the window's state after the operations */
extern "C" int ref_gwin_dump( long long *o )
{
	int k = 0;
	WinInstanceData &d = theW.m_instData;
	o[k++] = theW.m_system.h; o[k++] = theW.m_input.h; o[k++] = theW.m_draw.h; o[k++] = theW.m_tooltip.h; o[k++] = (long long)theW.m_layout;
	o[k++] = d.m_id; o[k++] = d.m_state; o[k++] = d.m_style; o[k++] = d.m_status; o[k++] = d.m_imageOffset.x; o[k++] = d.m_imageOffset.y; o[k++] = d.m_tooltipDelay;
	o[k++] = d.m_enabledText.color; o[k++] = d.m_enabledText.borderColor; o[k++] = d.m_disabledText.color; o[k++] = d.m_disabledText.borderColor;
	o[k++] = d.m_hiliteText.color; o[k++] = d.m_hiliteText.borderColor;
	WinDrawData *sets[3] = { d.m_enabledDrawData, d.m_disabledDrawData, d.m_hiliteDrawData };
	for (int s = 0; s < 3; ++s)
		for (int i = 0; i < MAX_DRAW_DATA; ++i)
		{
			o[k++] = (long long)sets[s][i].image; o[k++] = sets[s][i].color; o[k++] = sets[s][i].borderColor;
		}
	o[k++] = d.m_videoBuffer != NULL;
	k = dumpTextTag( &d, o, k );
	return k;
}
