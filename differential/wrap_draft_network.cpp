/* Differential-test wrapper for the network draft (#215, upstream commit 0a05454d), src/drafts/sweep_gui_logic/network.rs, for a Network with NO
** connection manager (`m_conMgr == NULL`, which the functions below handle upstream), cut verbatim by build.rs (drafts_guilogic4) from
** GameNetwork/Network.cpp into:
**   net_ctor.inc    Network::Network (to the destructor's comment);  net_xfer.inc   isTransferCommand;   net_exec.inc   getExecutionFrame;
**   net_ready.inc   AllCommandsReady;   net_runahead.inc   processRunAheadCommand;   net_destroy.inc   processDestroyPlayerCommand;   net_time.inc   timeForNewFrame;   net_fdr.inc   isFrameDataReady;
**   net_stats.inc   getIncomingBytesPerSecond .. getPacketArrivalCushion;   net_quit.inc   quitGame .. getLocalPlayerID;   net_num.inc   getNumPlayers and
**   getSlotAverageFPS;   net_notify.inc   notifyOthersOfCurrentFrame and notifyOthersOfNewFrame (to the end).
** Built on the REAL AsciiString.h and AsciiString.cpp (shim_ascii; classes renamed by build.rs).
** Stand-ins (outside the cuts): class Network (the data members of Network.cpp's class in NetworkInterface.h, and the cut methods' declarations; the
** `_DEBUG` m_networkOn is absent, as in the release build); ConnectionManager (declarations only, never called: m_conMgr is NULL); GameMessage (type; the
** boolean argument and the player index, each logged), the Type enumerators the cuts use (MSG_BEGIN_NETWORK_MESSAGES = 1000 and MSG_END_NETWORK_MESSAGES =
** 1999 as MessageStream.h:486 and :608 give them, which build.rs asserts; MSG_CLEAR_GAME_DATA outside, MSG_SELF_DESTRUCT inside that range); NetworkLocalStatus
** (NetLocalStatus, NetworkDefs.h:162, as it is); TheMessageStream and TheCommandList (appendMessage: logged); TheGameLogic (getFrame, settable); ThePlayerList
** (findPlayerWithNameKey: player<k> exists for the bits of a mask the test gives, and has player index 100 + k; each query logged), NAMEKEY (the name itself),
** newInstance(T) (new T); NetDestroyPlayerCommandMsg (getPlayerIndex); NetRunAheadCommandMsg (getRunAhead, getFrameRate) and ConnectionManager::setFrameGrouping (logs the value;
** the one operation that gives the Network a connection manager, for the call); TheGlobalData (m_networkRunAheadSlack); QueryPerformanceCounter (the value the test gives);
** the types NetLocalStatus's users need; DEBUG_LOG, DEBUG_ASSERTCRASH (compiled out).
**
** Entry point (extern "C", tests/differential_draft_network.rs): ref_net_run(in, n, out, cap, log, logcap): see the test. */
#include <new>
#include <time.h>
#include <string.h>
#include <stdio.h>
#include <stdarg.h>
#include <string>
#include <list>
#include <vector>

#define DEBUG_LOG(x)
#define DEBUG_ASSERTCRASH(x, y)
typedef long long __int64;
struct LARGE_INTEGER { long long QuadPart; };

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theNetAllocator;

static std::string theLog;
static void logf( const char *fmt, ... )
{
	char b[256];
	va_list ap;
	va_start( ap, fmt );
	vsnprintf( b, sizeof( b ), fmt, ap );
	va_end( ap );
	theLog += b;
	theLog += "\n";
}

enum { MAX_SLOTS = 8 };
#include "net_enum.inc"

class GameMessage
{
public:
	enum Type { MSG_CLEAR_GAME_DATA = 1, MSG_BEGIN_NETWORK_MESSAGES = 1000, MSG_SELF_DESTRUCT = 1001, MSG_END_NETWORK_MESSAGES = 1999 };
	GameMessage( Type t ) : m_type( t ), m_player( -1 ) {}
	Type getType( void ) const { return m_type; }
	void appendBooleanArgument( Bool b ) { logf( "bool %d", (int)b ); }
	void friend_setPlayerIndex( Int i ) { logf( "player %d", (int)i ); m_player = i; }
	Type m_type;
	Int m_player;
};
#define newInstance(T) new T
static const char *typeName( GameMessage::Type t ) { return t == GameMessage::MSG_SELF_DESTRUCT ? "SELF_DESTRUCT" : t == GameMessage::MSG_CLEAR_GAME_DATA ? "CLEAR_GAME_DATA" : "other"; }
class MessageStreamStandIn { public: GameMessage *appendMessage( GameMessage::Type t ) { logf( "stream.append %s", typeName( t ) ); return new GameMessage( t ); } };
static MessageStreamStandIn theStream;
static MessageStreamStandIn *TheMessageStream = &theStream;
class CommandListStandIn { public: void appendMessage( GameMessage *m ) { logf( "commandlist.append %s", typeName( m->getType() ) ); } };
static CommandListStandIn theCommandList;
static CommandListStandIn *TheCommandList = &theCommandList;
static unsigned theGameFrame;
class GameLogicStandIn { public: UnsignedInt getFrame( void ) { return theGameFrame; } };
static GameLogicStandIn theLogic;
static GameLogicStandIn *TheGameLogic = &theLogic;
static int thePlayerMask;
class Player { public: Int idx; Int getPlayerIndex( void ) const { return idx; } };
class PlayerListStandIn
{
public:
	Player *findPlayerWithNameKey( AsciiString name )
	{
		logf( "find %s", name.str() );
		for (int k = 0; k < 32; ++k)
		{
			char b[32];
			snprintf( b, sizeof( b ), "player%d", k );
			if (((thePlayerMask >> k) & 1) && name == AsciiString( b )) { static Player p; p.idx = 100 + k; return &p; }
		}
		return NULL;
	}
};
static PlayerListStandIn thePlayers;
static PlayerListStandIn *ThePlayerList = &thePlayers;
#define NAMEKEY(x) (x)
class NetDestroyPlayerCommandMsg { public: UnsignedInt idx; UnsignedInt getPlayerIndex( void ) { return idx; } };
struct GlobalDataStandIn { UnsignedInt m_networkRunAheadSlack; };
static GlobalDataStandIn theGlobalData;
static GlobalDataStandIn *TheGlobalData = &theGlobalData;
static long long theCounter;
static void QueryPerformanceCounter( LARGE_INTEGER *p ) { p->QuadPart = theCounter; }

class ConnectionManager
{
public:
	/* (never called but through setFrameGrouping: m_conMgr is NULL elsewhere) */
	Real getMinimumCushion() { return 0; }
	Bool allCommandsReady( UnsignedInt ) { return 1; }
	Real getIncomingBytesPerSecond() { return 0; } Real getIncomingPacketsPerSecond() { return 0; } Real getOutgoingBytesPerSecond() { return 0; }
	Real getOutgoingPacketsPerSecond() { return 0; } Real getUnknownBytesPerSecond() { return 0; } Real getUnknownPacketsPerSecond() { return 0; }
	UnsignedInt getPacketArrivalCushion() { return 0; }
	void quitGame() {} Bool isPacketRouter() { return 0; } void voteForPlayerDisconnect( Int ) {} void updateLoadProgress( Int ) {} void loadProgressComplete() {}
	void sendTimeOutGameStart() {} UnsignedInt getLocalPlayerID() { return 0; } Int getNumPlayers() { return 0; } Int getSlotAverageFPS( Int ) { return 0; }
	void notifyOthersOfCurrentFrame( UnsignedInt ) {} void notifyOthersOfNewFrame( UnsignedInt ) {}
	void setFrameGrouping( time_t frameGrouping ) { logf( "frameGrouping %lld", (long long)frameGrouping ); }
};
class NetRunAheadCommandMsg { public: Int ra, fr; Int getRunAhead() { return ra; } Int getFrameRate() { return fr; } };
class GameWindow;

class Network
{
public:
	Network();
	Bool isTransferCommand( GameMessage *msg );
	Int getExecutionFrame();
	Bool AllCommandsReady( UnsignedInt frame );
	void processDestroyPlayerCommand( NetDestroyPlayerCommandMsg *msg );
	void processRunAheadCommand( NetRunAheadCommandMsg *msg );
	Bool timeForNewFrame();
	Bool isFrameDataReady();
	Real getIncomingBytesPerSecond( void ); Real getIncomingPacketsPerSecond( void ); Real getOutgoingBytesPerSecond( void ); Real getOutgoingPacketsPerSecond( void );
	Real getUnknownBytesPerSecond( void ); Real getUnknownPacketsPerSecond( void ); UnsignedInt getPacketArrivalCushion( void );
	void quitGame(); void selfDestructPlayer( Int index ); Bool isPacketRouter( void ); void voteForPlayerDisconnect( Int slot ); void updateLoadProgress( Int percent );
	void loadProgressComplete( void ); void sendTimeOutGameStart( void ); UnsignedInt getLocalPlayerID();
	Int getNumPlayers(); Int getSlotAverageFPS( Int slot ); void notifyOthersOfCurrentFrame(); void notifyOthersOfNewFrame( UnsignedInt frame );

	ConnectionManager *m_conMgr;
	UnsignedInt m_lastFrame;
	NetLocalStatus m_localStatus;
	Int m_runAhead;
	Int m_frameRate;
	Int m_lastExecutionFrame;
	Int m_lastFrameCompleted;
	Bool m_didSelfSlug;
	__int64 m_perfCountFreq;
	__int64 m_nextFrameTime;
	Bool m_frameDataReady;
	Bool m_checkCRCsThisFrame;
	Bool m_sawCRCMismatch;
	std::list<Int> m_playersToDisconnect;
	GameWindow *m_messageWindow;
};

#include "net_ctor.inc"
#include "net_xfer.inc"
#include "net_exec.inc"
#include "net_ready.inc"
#include "net_runahead.inc"
#include "net_destroy.inc"
#include "net_time.inc"
#include "net_fdr.inc"
#include "net_stats.inc"
#include "net_quit.inc"
#include "net_num.inc"
#include "net_notify.inc"

static unsigned fbits( Real r ) { unsigned u; memcpy( &u, &r, 4 ); return u; }

/* The program (in, n 64-bit values): the setup [localStatus, runAhead, frameRate, lastExecutionFrame, lastFrameCompleted, didSelfSlug, perfCountFreq, nextFrameTime,
** frameDataReady, slack, gameFrame, perfCounter, playerMask, checkCRCs, sawCRCMismatch], then operations [kind, a, b]. Results go to out (the constructor's flags first,
** then each operation's answers, then the final state); the log of the stand-ins' calls to log. Returns the number of values in out (-1: out too small). */
extern "C" int ref_net_run( const long long *in, int n, long long *out, int cap, char *log, int logcap )
{
	TheDynamicMemoryAllocator = &theNetAllocator;
	theLog.clear();
	std::vector<long long> o;
	unsigned char raw[sizeof( Network )];
	memset( raw, 0xA5, sizeof( raw ) );
	Network *net = new ( raw ) Network();
	o.push_back( net->m_checkCRCsThisFrame ); o.push_back( net->m_didSelfSlug ); o.push_back( net->m_frameDataReady ); o.push_back( net->m_sawCRCMismatch );
	o.push_back( net->m_conMgr == NULL ); o.push_back( net->m_messageWindow == NULL );
	int k = 0;
	net->m_localStatus = (NetLocalStatus)in[k++]; net->m_runAhead = (Int)in[k++]; net->m_frameRate = (Int)in[k++]; net->m_lastExecutionFrame = (Int)in[k++];
	net->m_lastFrameCompleted = (Int)in[k++]; net->m_didSelfSlug = (Bool)in[k++]; net->m_perfCountFreq = in[k++]; net->m_nextFrameTime = in[k++];
	net->m_frameDataReady = (Bool)in[k++]; theGlobalData.m_networkRunAheadSlack = (UnsignedInt)in[k++]; theGameFrame = (unsigned)in[k++]; theCounter = in[k++];
	thePlayerMask = (int)in[k++]; net->m_checkCRCsThisFrame = (Bool)in[k++]; net->m_sawCRCMismatch = (Bool)in[k++];
	for (; k + 2 < n; k += 3)
	{
		long long kind = in[k], a = in[k + 1], b = in[k + 2];
		switch (kind)
		{
			case 0: o.push_back( net->getExecutionFrame() ); break;
			case 1: theCounter = a; o.push_back( net->timeForNewFrame() ); break;
			case 2: o.push_back( net->isFrameDataReady() ); break;
			case 3: o.push_back( net->AllCommandsReady( (UnsignedInt)a ) ); break;
			case 4: net->quitGame(); break;
			case 5: net->selfDestructPlayer( (Int)a ); break;
			case 6: { NetDestroyPlayerCommandMsg m; m.idx = (UnsignedInt)a; net->processDestroyPlayerCommand( &m ); break; }
			case 7: { GameMessage m( (GameMessage::Type)a ); o.push_back( net->isTransferCommand( b ? NULL : &m ) ); break; }
			case 8: o.push_back( fbits( net->getIncomingBytesPerSecond() ) ); o.push_back( fbits( net->getIncomingPacketsPerSecond() ) );
				o.push_back( fbits( net->getOutgoingBytesPerSecond() ) ); o.push_back( fbits( net->getOutgoingPacketsPerSecond() ) );
				o.push_back( fbits( net->getUnknownBytesPerSecond() ) ); o.push_back( fbits( net->getUnknownPacketsPerSecond() ) ); o.push_back( net->getPacketArrivalCushion() ); break;
			case 9: o.push_back( net->isPacketRouter() ); o.push_back( net->getLocalPlayerID() ); o.push_back( net->getNumPlayers() ); o.push_back( net->getSlotAverageFPS( (Int)a ) );
				net->voteForPlayerDisconnect( (Int)a ); net->updateLoadProgress( (Int)a ); net->loadProgressComplete(); net->sendTimeOutGameStart();
				net->notifyOthersOfCurrentFrame(); net->notifyOthersOfNewFrame( (UnsignedInt)a ); break;
			case 10: theGameFrame = (unsigned)a; break;
			case 11: { ConnectionManager cm; NetRunAheadCommandMsg m; m.ra = (Int)a; m.fr = (Int)b; net->m_conMgr = &cm; net->processRunAheadCommand( &m ); net->m_conMgr = NULL; break; }
		}
	}
	o.push_back( net->m_localStatus ); o.push_back( net->m_lastExecutionFrame ); o.push_back( net->m_lastFrameCompleted ); o.push_back( net->m_didSelfSlug );
	o.push_back( net->m_nextFrameTime ); o.push_back( net->m_runAhead ); o.push_back( net->m_frameRate ); o.push_back( net->m_frameDataReady );
	o.push_back( (long long)net->m_playersToDisconnect.size() );
	for (std::list<Int>::iterator it = net->m_playersToDisconnect.begin(); it != net->m_playersToDisconnect.end(); ++it) o.push_back( *it );
	net->~Network();
	if ((int)o.size() > cap || (int)theLog.size() + 1 > logcap) return -1;
	memcpy( out, &o[0], o.size() * sizeof( long long ) );
	memcpy( log, theLog.c_str(), theLog.size() + 1 );
	return (int)o.size();
}
