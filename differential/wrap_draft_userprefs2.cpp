/* Differential-test wrapper for the online preference accessors draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_common_client/user_preferences.rs (QuickMatchPreferences, CustomMatchPreferences, GameSpyMiscPreferences),
** cut verbatim by build.rs into OUT_DIR/draft_guilogic2_gen:
**   up2_quick_h.inc, up2_custom_h.inc, up2_misc_h.inc   the three headers' class bodies (Common/QuickmatchPreferences.h,
**                    CustomMatchPreferences.h, GameSpyMiscPreferences.h)
**   up2_quick.inc, up2_custom.inc, up2_misc.inc   Common/UserPreferences.cpp, each class's section (constructor to the next banner)
**   up2_qp1.inc, up2_qp2.inc, up2_qp3.inc   Common/System/QuotedPrintable.cpp: MAGIC_CHAR and the hex helpers,
**                    AsciiStringToQuotedPrintable, QuotedPrintableToAsciiString
**   up2_enum.inc     GameNetwork/GameInfo.h, the PLAYERTEMPLATE_* enum
** and the base class REUSED from the verified differential/wrap_user_prefs.cpp's cut (OUT_DIR/user_prefs_gen/user_prefs_cut.inc,
** PreferenceMap, class UserPreferences and its functions), not cut again; it is compiled here under names of this wrapper's own
** (build.rs renames), with the real AsciiString.cpp.
** Stand-ins (outside the cuts): TheGlobalData (getPath_UserData: the folder the test gives), TheGameSpyInfo (getLocalProfileID),
** Money (deposit adds, countMoney; Money.cpp's deposit also tells the player's academy stats and may play a sound),
** TheMultiplayerSettings (getNumColors, getDefaultStartingMoney), ThePlayerTemplateStore and PlayerTemplate (the count,
** the Nth, getStartingBuilding, isOldFaction, getName), TheChallengeGenerals and GeneralPersona (isStartingEnabled),
** TheGameInfo (oldFactionsOnly), GetRegistryLanguage, getDefaultOfficialMap, isValidMap, isOfficialMap, all answering from the
** tables the test gives.
** Entry point (extern "C", tests/differential_draft_userprefs2.rs): ref_up2_run(in, nin, dir, out, cap): the program is ints;
** a string is its length then one int per byte. Header: numColors; templates (n, then per template: kind 0 normal / 1 empty
** starting building / 2 NULL, isOldFaction, name idx 0..3); gameInfo (0 none / 1 oldFactionsOnly / 2 present but not / 3 NULL);
** generals (n, then name idx, startsEnabled); language idx; default money; default map string; valid mask, official mask over
** "Map0".."Map7". Then ops, each a kind word: 0 raw obj key val | 1 setMapSelected s b | 2 isMapSelected s | 3 setLastLadder obj s port |
** 4 getLastLadderAddr obj | 5 getLastLadderPort obj | 6 quick set which v | 7 quick get which | 8 setPreferredColor v | 9 getPreferredColor |
** 10 setChatSizeSlider v | 11 getChatSizeSlider | 12 setPreferredFaction v | 13 getPreferredFaction | 14 custom bool set which v |
** 15 custom bool get which | 16 setPreferredMap s | 17 getPreferredMap | 18 setStartingCash v | 19 getStartingCash | 20 setLocale v |
** 21 getLocale | 22 setCachedStats s | 23 getCachedStats | 24 getQuickMatchResLocked | 25 getMaxMessagesPerUpdate | 26 dump the three maps |
** 27 kind profile: construct that class with the local profile id and report whether its marker key loaded. */
#include "PreRTS.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <map>
#include <string>
#include <vector>

/* (the allocator pointer is renamed in this build, see build.rs) */
DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theUp2Allocator;

namespace {
struct Up2GlobalData
{
	AsciiString m_userData;
	AsciiString getPath_UserData() const { return m_userData; }
};
Up2GlobalData *TheGlobalData = 0;
}

class Money
{
public:
	Money() : m_money( 0 ) {}
	UnsignedInt countMoney() const { return m_money; }
	void deposit( UnsignedInt amountToDeposit, Bool playSound = TRUE ) { m_money += amountToDeposit; }
	UnsignedInt m_money;
};

class PlayerTemplate
{
public:
	int kind; bool old; AsciiString name;
	AsciiString getStartingBuilding( void ) const { return kind == 1 ? AsciiString( "" ) : AsciiString( "Building" ); }
	Bool isOldFaction( void ) const { return old; }
	AsciiString getName( void ) const { return name; }
};
class GeneralPersona { public: Bool enabled; const Bool isStartingEnabled() const { return enabled; } };

namespace {
std::vector<PlayerTemplate *> g_templates;
std::map<std::string, GeneralPersona> g_generals;
int g_numColors = 0, g_oldOnly = 3, g_lang = 0, g_profile = 0;
UnsignedInt g_defaultMoney = 0;
std::string g_defaultMap;
int g_validMask = 0, g_officialMask = 0;
struct Up2Settings { int getNumColors() { return g_numColors; } Money getDefaultStartingMoney() { Money m; m.deposit( g_defaultMoney ); return m; } };
struct Up2Store { int getPlayerTemplateCount() { return (int)g_templates.size(); } const PlayerTemplate *getNthPlayerTemplate( int n ) { return ( n >= 0 && n < (int)g_templates.size() && g_templates[n]->kind != 2 ) ? g_templates[n] : 0; } };
struct Up2Challenge { const GeneralPersona *getGeneralByTemplateName( const AsciiString &n ) { std::map<std::string, GeneralPersona>::iterator it = g_generals.find( n.str() ); return it == g_generals.end() ? 0 : &it->second; } };
struct Up2Info { Bool oldFactionsOnly() { return g_oldOnly == 1; } };
struct Up2Spy { int getLocalProfileID() { return g_profile; } };
Up2Settings theSettings; Up2Store theStore; Up2Challenge theChallenge; Up2Info theInfo; Up2Spy theSpy;
Up2Settings *TheMultiplayerSettings = &theSettings;
Up2Store *ThePlayerTemplateStore = &theStore;
Up2Challenge *TheChallengeGenerals = &theChallenge;
Up2Info *TheGameInfo = 0;
Up2Spy *TheGameSpyInfo = &theSpy;
const char *langs[7] = { "english", "chinese", "korean", "CHINESE", "Korean", "", "koreanx" };
}
AsciiString GetRegistryLanguage( void ) { return AsciiString( langs[g_lang] ); }
static int mapIndex( const AsciiString &n ) { const char *s = n.str(); if( strncmp( s, "Map", 3 ) == 0 && s[3] >= '0' && s[3] <= '7' && s[4] == 0 ) return s[3] - '0'; return -1; }
AsciiString getDefaultOfficialMap() { return AsciiString( g_defaultMap.c_str() ); }
Bool isValidMap( AsciiString mapName, Bool isMultiplayer ) { int k = mapIndex( mapName ); return k >= 0 && ( g_validMask >> k & 1 ); }
Bool isOfficialMap( AsciiString mapName ) { int k = mapIndex( mapName ); return k >= 0 && ( g_officialMask >> k & 1 ); }

#include "up2_enum.inc"
#include "user_prefs_cut.inc"
#include "up2_qp1.inc"
#include "up2_qp2.inc"
#include "up2_qp3.inc"
#include "up2_quick_h.inc"
#include "up2_custom_h.inc"
#include "up2_misc_h.inc"
#include "up2_quick.inc"
#include "up2_custom.inc"
#include "up2_misc.inc"

namespace {
struct Out
{
	std::vector<int> b;
	void str( const char *s ) { int n = (int)strlen( s ); b.push_back( n ); for( int i = 0; i < n; ++i ) b.push_back( (unsigned char)s[i] ); }
};
void dumpMap( Out &o, const UserPreferences &p )
{
	o.b.push_back( (int)p.size() );
	for( PreferenceMap::const_iterator it = p.begin(); it != p.end(); ++it ) { o.str( it->first.str() ); o.str( it->second.str() ); }
}
}

extern "C" int ref_up2_run( const int *in, int nin, const char *dir, int *outp, int cap )
{
	TheDynamicMemoryAllocator = &theUp2Allocator;
	Up2GlobalData gd;
	gd.m_userData = dir;
	TheGlobalData = &gd;
	int at = 0;
	auto rdStr = [&]() { int n = in[at++]; std::string s; for( int i = 0; i < n; ++i ) s += (char)in[at++]; return s; };
	for( size_t i = 0; i < g_templates.size(); ++i ) delete g_templates[i];
	g_templates.clear(); g_generals.clear();
	g_numColors = in[at++];
	int nt = in[at++];
	char nm[16];
	for( int i = 0; i < nt; ++i )
	{
		PlayerTemplate *t = new PlayerTemplate;
		t->kind = in[at]; t->old = in[at + 1] != 0; snprintf( nm, sizeof nm, "Tpl%d", in[at + 2] ); t->name = nm; at += 3;
		g_templates.push_back( t );
	}
	g_oldOnly = in[at++];
	TheGameInfo = g_oldOnly == 3 ? 0 : &theInfo;
	int ng = in[at++];
	for( int i = 0; i < ng; ++i ) { snprintf( nm, sizeof nm, "Tpl%d", in[at] ); GeneralPersona g; g.enabled = in[at + 1] != 0; g_generals[nm] = g; at += 2; }
	g_lang = in[at++];
	g_defaultMoney = (UnsignedInt)in[at++];
	g_defaultMap = rdStr();
	g_validMask = in[at++]; g_officialMask = in[at++];

	std::vector<UserPreferences *> objs;
	QuickMatchPreferences *q = new QuickMatchPreferences; CustomMatchPreferences *c = new CustomMatchPreferences; GameSpyMiscPreferences *m = new GameSpyMiscPreferences;
	UserPreferences *bases[3] = { q, c, m };
	Out o;
	while( at < nin )
	{
		int kind = in[at++];
		switch( kind )
		{
			case 0: { int ob = in[at++]; std::string k = rdStr(); std::string v = rdStr(); bases[ob]->setAsciiString( AsciiString( k.c_str() ), AsciiString( v.c_str() ) ); break; }
			case 1: { std::string s = rdStr(); int b = in[at++]; q->setMapSelected( AsciiString( s.c_str() ), b != 0 ); break; }
			case 2: { std::string s = rdStr(); o.b.push_back( q->isMapSelected( AsciiString( s.c_str() ) ) ? 1 : 0 ); break; }
			case 3: { int ob = in[at++]; std::string s = rdStr(); int port = in[at++]; if( ob == 0 ) q->setLastLadder( AsciiString( s.c_str() ), (UnsignedShort)port ); else c->setLastLadder( AsciiString( s.c_str() ), (UnsignedShort)port ); break; }
			case 4: { int ob = in[at++]; AsciiString r = ob == 0 ? q->getLastLadderAddr() : c->getLastLadderAddr(); o.str( r.str() ); break; }
			case 5: { int ob = in[at++]; o.b.push_back( ob == 0 ? q->getLastLadderPort() : c->getLastLadderPort() ); break; }
			case 6: { int w = in[at++]; int v = in[at++];
				switch( w ) { case 0: q->setMaxDisconnects( v ); break; case 1: q->setMaxPoints( v ); break; case 2: q->setMinPoints( v ); break; case 3: q->setWaitTime( v ); break;
					case 4: q->setNumPlayers( v ); break; case 5: q->setMaxPing( v ); break; case 6: q->setColor( v ); break; default: q->setSide( v ); break; }
				break; }
			case 7: { int w = in[at++]; int r;
				switch( w ) { case 0: r = q->getMaxDisconnects(); break; case 1: r = q->getMaxPoints(); break; case 2: r = q->getMinPoints(); break; case 3: r = q->getWaitTime(); break;
					case 4: r = q->getNumPlayers(); break; case 5: r = q->getMaxPing(); break; case 6: r = q->getColor(); break; default: r = q->getSide(); break; }
				o.b.push_back( r ); break; }
			case 8: c->setPreferredColor( in[at++] ); break;
			case 9: o.b.push_back( c->getPreferredColor() ); break;
			case 10: c->setChatSizeSlider( in[at++] ); break;
			case 11: o.b.push_back( c->getChatSizeSlider() ); break;
			case 12: c->setPreferredFaction( in[at++] ); break;
			case 13: o.b.push_back( c->getPreferredFaction() ); break;
			case 14: { int w = in[at++]; Bool v = in[at++] != 0;
				switch( w ) { case 0: c->setUsesSystemMapDir( v ); break; case 1: c->setUsesLongGameList( v ); break; case 2: c->setAllowsObserver( v ); break; case 3: c->setDisallowAsianText( v ); break;
					case 4: c->setDisallowNonAsianText( v ); break; case 5: c->setSuperweaponRestricted( v ); break; case 6: c->setFactionsLimited( v ); break; default: c->setUseStats( v ); break; }
				break; }
			case 15: { int w = in[at++]; Bool r;
				switch( w ) { case 0: r = c->usesSystemMapDir(); break; case 1: r = c->usesLongGameList(); break; case 2: r = c->allowsObservers(); break; case 3: r = c->getDisallowAsianText(); break;
					case 4: r = c->getDisallowNonAsianText(); break; case 5: r = c->getSuperweaponRestricted(); break; case 6: r = c->getFactionsLimited(); break; default: r = c->getUseStats(); break; }
				o.b.push_back( r ? 1 : 0 ); break; }
			case 16: { std::string s = rdStr(); c->setPreferredMap( AsciiString( s.c_str() ) ); break; }
			case 17: { AsciiString r = c->getPreferredMap(); o.str( r.str() ); break; }
			case 18: { Money mo; mo.deposit( (UnsignedInt)in[at++] ); c->setStartingCash( mo ); break; }
			case 19: { Money mo = c->getStartingCash(); o.b.push_back( (int)mo.countMoney() ); break; }
			case 20: m->setLocale( in[at++] ); break;
			case 21: o.b.push_back( m->getLocale() ); break;
			case 22: { std::string s = rdStr(); m->setCachedStats( AsciiString( s.c_str() ) ); break; }
			case 23: { AsciiString r = m->getCachedStats(); o.str( r.str() ); break; }
			case 24: o.b.push_back( m->getQuickMatchResLocked() ? 1 : 0 ); break;
			case 25: o.b.push_back( m->getMaxMessagesPerUpdate() ); break;
			case 26: dumpMap( o, *q ); dumpMap( o, *c ); dumpMap( o, *m ); break;
			default:
			{
				int k = in[at++]; g_profile = in[at++];
				UserPreferences *p = k == 0 ? (UserPreferences *)new QuickMatchPreferences : k == 1 ? (UserPreferences *)new CustomMatchPreferences : (UserPreferences *)new GameSpyMiscPreferences;
				o.b.push_back( (int)p->size() ); o.str( p->getAsciiString( "Marker", "" ).str() ); o.str( p->getAsciiString( "Marker", "" ).str() );
				delete p;
				break;
			}
		}
	}
	delete q; delete c; delete m;
	if( (int)o.b.size() > cap ) return -1;
	memcpy( outp, o.b.data(), o.b.size() * sizeof( int ) );
	return (int)o.b.size();
}
