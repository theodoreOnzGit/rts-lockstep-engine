/* Differential-test wrapper for the control bar resizer draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/control_bar_resizer.rs (findResizerWindow, newResizerWindow, sizeWindowsDefault, sizeWindowsAlt),
** cut verbatim by build.rs into OUT_DIR/draft_guilogic2_gen:
**   rz_h.inc       GameClient/ControlBarResizer.h, class ResizerWindow and class ControlBarResizer
**   rz_c1.inc      GUI/ControlBar/ControlBarResizer.cpp, ResizerWindow::ResizerWindow, ControlBarResizer's constructor and destructor
**   rz_c2.inc      findResizerWindow .. sizeWindowsAlt
**   rz_gw1.inc     GUI/GameWindow.cpp, GameWindow::normalizeWindowRegion
**   rz_gw2.inc     GameWindow::winSetPosition, winGetPosition
**   rz_gw3.inc     GameWindow::winSetSize, winGetSize
** Stand-ins: class GameWindow (only m_region, m_size and the members of the cuts above), TheWindowManager
** (winGetWindowFromId over the windows the test gives, first match; winSendSystemMsg does nothing: the draft passes a host with
** no callbacks), TheNameKeyGenerator->nameToKey (a key per name), TheDisplay (getWidth, getHeight), FieldParse and
** ControlBarResizer::init (INI loading).
** Entry point (extern "C", tests/differential_draft_resizer.rs): ref_resizer_run(in, nin, out, cap), a program (every word an int):
**   display w h, window count n, per window: name idx, x, y, width, height (names "W0".."W7"); then ops:
**   0 name        newResizerWindow -> [index in the list, -1 for NULL]
**   1 name        findResizerWindow -> [index or -1]
**   2 idx ax ay aw ah   m_altPos, m_altSize of list entry idx
**   3             sizeWindowsDefault       4  sizeWindowsAlt
**   5 win x y w h winSetPosition(x, y) then winSetSize(w, h) of window win
**   6 w h         the display's size
**   7             dump: per window [lo.x lo.y hi.x hi.y size.x size.y]; then per list entry [default pos, size, alt pos, size] */
#include "PreRTS.h"
#include <vector>
#include <string>
#include <list>
#include <map>
#include <string.h>
#include <stdio.h>
#include "Common/AsciiString.h"
#include "Common/GameMemory.h"

struct FieldParse { int unused; };
typedef int NameKeyType;
typedef unsigned WindowMsgData;
enum { GGM_RESIZED = 1 };
enum { WIN_ERR_OK = 0, WIN_ERR_INVALID_PARAMETER = -1 };

class GameWindow
{
public:
	Int winSetPosition( Int x, Int y );
	Int winGetPosition( Int *x, Int *y );
	Int winSetSize( Int width, Int height );
	Int winGetSize( Int *width, Int *height );
	void normalizeWindowRegion( void );
	IRegion2D m_region;
	ICoord2D m_size;
	NameKeyType m_key;
};

namespace {
struct G2RzNameKeys
{
	std::map<std::string, int> m;
	int nameToKey( const AsciiString &n ) { std::string s = n.str(); if( !m.count( s ) ) { int k = (int)m.size() + 1; m[s] = k; } return m[s]; }
};
struct G2RzWindows
{
	std::vector<GameWindow *> list;
	GameWindow *winGetWindowFromId( GameWindow *window, NameKeyType id )
	{
		for( size_t i = 0; i < list.size(); ++i ) if( list[i]->m_key == id ) return list[i];
		return 0;
	}
	void winSendSystemMsg( GameWindow *w, unsigned msg, WindowMsgData a, WindowMsgData b ) { }
};
struct G2RzDisplay { Int w, h; Int getWidth() { return w; } Int getHeight() { return h; } };
G2RzNameKeys nameKeys;
G2RzWindows windows;
G2RzDisplay display;
G2RzNameKeys *TheNameKeyGenerator = &nameKeys;
G2RzWindows *TheWindowManager = &windows;
G2RzDisplay *TheDisplay = &display;
}

#include "rz_gw1.inc"
#include "rz_gw2.inc"
#include "rz_gw3.inc"
#include "rz_h.inc"
void ControlBarResizer::init( void ) { }
const FieldParse ControlBarResizer::m_controlBarResizerParseTable[] = { { 0 } };
#include "rz_c1.inc"
#include "rz_c2.inc"

extern "C" int ref_resizer_run( const int *in, int nin, int *outp, int cap )
{
	std::vector<int> out;
	int at = 0;
	for( size_t i = 0; i < windows.list.size(); ++i ) delete windows.list[i];
	windows.list.clear();
	display.w = in[at]; display.h = in[at + 1]; at += 2;
	int nw = in[at++];
	char nm[8];
	for( int i = 0; i < nw; ++i )
	{
		GameWindow *w = new GameWindow;
		snprintf( nm, sizeof nm, "W%d", in[at] );
		w->m_key = TheNameKeyGenerator->nameToKey( AsciiString( nm ) );
		w->m_region.lo.x = in[at + 1]; w->m_region.lo.y = in[at + 2];
		w->m_size.x = in[at + 3]; w->m_size.y = in[at + 4];
		w->m_region.hi.x = w->m_region.lo.x + w->m_size.x; w->m_region.hi.y = w->m_region.lo.y + w->m_size.y;
		w->normalizeWindowRegion();
		windows.list.push_back( w );
		at += 5;
	}
	ControlBarResizer *r = new ControlBarResizer;
	auto indexOf = [&]( ResizerWindow *rw ) -> int { int i = 0; for( std::list<ResizerWindow *>::iterator it = r->m_resizerWindowsList.begin(); it != r->m_resizerWindowsList.end(); ++it, ++i ) if( *it == rw ) return i; return -1; };
	auto nth = [&]( int idx ) -> ResizerWindow * { int i = 0; for( std::list<ResizerWindow *>::iterator it = r->m_resizerWindowsList.begin(); it != r->m_resizerWindowsList.end(); ++it, ++i ) if( i == idx ) return *it; return 0; };
	while( at < nin )
	{
		int kind = in[at++];
		switch( kind )
		{
			case 0: { snprintf( nm, sizeof nm, "W%d", in[at++] ); out.push_back( indexOf( r->newResizerWindow( AsciiString( nm ) ) ) ); break; }
			case 1: { snprintf( nm, sizeof nm, "W%d", in[at++] ); out.push_back( indexOf( r->findResizerWindow( AsciiString( nm ) ) ) ); break; }
			case 2: { ResizerWindow *rw = nth( in[at] ); if( rw ) { rw->m_altPos.x = in[at + 1]; rw->m_altPos.y = in[at + 2]; rw->m_altSize.x = in[at + 3]; rw->m_altSize.y = in[at + 4]; } at += 5; break; }
			case 3: r->sizeWindowsDefault(); break;
			case 4: r->sizeWindowsAlt(); break;
			case 5: { GameWindow *w = windows.list[in[at]]; w->winSetPosition( in[at + 1], in[at + 2] ); w->winSetSize( in[at + 3], in[at + 4] ); at += 5; break; }
			case 6: display.w = in[at]; display.h = in[at + 1]; at += 2; break;
			default:
			{
				for( size_t i = 0; i < windows.list.size(); ++i )
				{
					GameWindow *w = windows.list[i];
					out.push_back( w->m_region.lo.x ); out.push_back( w->m_region.lo.y ); out.push_back( w->m_region.hi.x ); out.push_back( w->m_region.hi.y );
					out.push_back( w->m_size.x ); out.push_back( w->m_size.y );
				}
				for( std::list<ResizerWindow *>::iterator it = r->m_resizerWindowsList.begin(); it != r->m_resizerWindowsList.end(); ++it )
				{
					ResizerWindow *rw = *it;
					out.push_back( rw->m_defaultPos.x ); out.push_back( rw->m_defaultPos.y ); out.push_back( rw->m_defaultSize.x ); out.push_back( rw->m_defaultSize.y );
					out.push_back( rw->m_altPos.x ); out.push_back( rw->m_altPos.y ); out.push_back( rw->m_altSize.x ); out.push_back( rw->m_altSize.y );
				}
				break;
			}
		}
	}
	delete r;
	if( (int)out.size() > cap ) return -1;
	memcpy( outp, out.data(), out.size() * sizeof( int ) );
	return (int)out.size();
}
