/* Differential-test wrapper for four small gui_logic drafts (#215, upstream commit 0a05454d), cut verbatim by build.rs (drafts_guilogic4) into:
**   gs_super.inc  GameNetwork/GameInfo.cpp, GameInfo::setSuperweaponRestriction (to setStartingCash);   -> src/drafts/sweep_gui_logic/game_info.rs
**   gs_lan.inc    GameNetwork/LANGameInfo.cpp, LANGameInfo::getConstLANSlot (to getLocalSlotNum);        -> lan_game_info.rs
**   gs_text.inc   GameClient/GUI/GameWindowGlobal.cpp, GameWindowManager::winFormatText and winGetTextSize (to winFontHeight);
**   gs_free.inc   GameClient/GUI/GameWindowManagerScript.cpp, GameWindowManager::freeStaticStrings (to WindowLayoutInfo's constructor);
**                 -> game_window_global.rs.
** Stand-ins (outside the cuts): GameInfo (only m_superweaponRestriction and the one method; the real class is the whole game setup);
** LANGameInfo (m_LANSlot[MAX_SLOTS] of LANGameSlot, each holding its own index; the one method) with MAX_SLOTS 8 (GameInfo.h); the DEBUG_ASSERTCRASH
** macro (compiled out in the release build); GameWindowManager (only the three methods), GameFont, UnicodeString (an empty class: the cut's
** parameters are unused), AsciiString (a std::string with clear()) and the four statics theSystemString .. theDrawString that the script reader
** declares (GameWindowManagerScript.cpp:127-130), as AsciiStrings.
**
** Entry points (extern "C", tests/differential_draft_glsmall.rs): ref_gs_super(restriction): the field after the call; ref_gs_lan(slot): the index of the
** slot returned, -1 for NULL; ref_gs_text(haveW, haveH): width and height as one value (w * 100000 + h, from 0x7777 sentinels); ref_gs_free(): the lengths
** of the four strings (from "abc") after freeStaticStrings, each * 10^k. */
#include <string>
#define DEBUG_ASSERTCRASH(x, y)
#ifndef NULL
#define NULL 0
#endif
typedef int Int;
typedef int Bool;
typedef int Color;
typedef unsigned short UnsignedShort;
enum { MAX_SLOTS = 8 };

class GameInfo
{
public:
	void setSuperweaponRestriction( UnsignedShort restriction );
	UnsignedShort m_superweaponRestriction;
};
#include "gs_super.inc"

class LANGameSlot { public: int index; };
class LANGameInfo
{
public:
	const LANGameSlot *getConstLANSlot( Int slotNum ) const;
	LANGameSlot m_LANSlot[MAX_SLOTS];
};
#include "gs_lan.inc"

class GameFont;
class UnicodeString {};
class GameWindowManager
{
public:
	void winFormatText( GameFont *font, UnicodeString text, Color color, Int x, Int y, Int width, Int height );
	void winGetTextSize( GameFont *font, UnicodeString text, Int *width, Int *height, Int maxWidth );
	void freeStaticStrings( void );
};
class AsciiString { public: std::string s; AsciiString() : s( "abc" ) {} void clear() { s.clear(); } };
static AsciiString theSystemString;
static AsciiString theInputString;
static AsciiString theTooltipString;
static AsciiString theDrawString;
#include "gs_text.inc"
#include "gs_free.inc"

extern "C" int ref_gs_super( int restriction )
{
	GameInfo g;
	g.m_superweaponRestriction = 0x1234;
	g.setSuperweaponRestriction( (UnsignedShort)restriction );
	return g.m_superweaponRestriction;
}
extern "C" int ref_gs_lan( int slot )
{
	LANGameInfo g;
	for (int i = 0; i < MAX_SLOTS; ++i) g.m_LANSlot[i].index = i;
	const LANGameSlot *s = g.getConstLANSlot( slot );
	return s ? s->index : -1;
}
extern "C" int ref_gs_text( int haveW, int haveH )
{
	GameWindowManager m;
	Int w = 0x7777, h = 0x7777;
	m.winFormatText( NULL, UnicodeString(), 1, 2, 3, 4, 5 );
	m.winGetTextSize( NULL, UnicodeString(), haveW ? &w : NULL, haveH ? &h : NULL, 9 );
	return w * 100000 + h;
}
extern "C" int ref_gs_free( void )
{
	GameWindowManager m;
	m.freeStaticStrings();
	return (int)theSystemString.s.size() + 10 * (int)theInputString.s.size() + 100 * (int)theTooltipString.s.size() + 1000 * (int)theDrawString.s.size();
}
