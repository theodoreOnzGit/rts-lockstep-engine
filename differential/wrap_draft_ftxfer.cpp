/* Differential-test wrapper for the map transfer of the file transfer draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/file_transfer.rs (do_file_transfer, do_any_map_transfers):
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/FileTransfer.cpp from `static Bool doFileTransfer(` to the end of the file (doFileTransfer, the
**   ten path helpers, DoAnyMapTransfers), cut verbatim by build.rs (drafts_guilogic4) into ft_xfer.inc.
** Built on the REAL AsciiString.h and AsciiString.cpp (shim_ascii; classes renamed by build.rs).
** Stand-ins (outside the cut), each of which writes one line to the log the test compares with the draft host's: DEBUG_LOG (compiled out),
** min (the Windows header's); MAX_SLOTS (8); timeGetTime() (the next of a scripted list, then 60,000 ms more at every call);
** Sleep(); GameInfo (amIHost, getConstSlot(i)->isHuman() / hasMap(), getMapContentsMask, getMap), TheGameInfo; TheNetwork (sendFileAnnounce,
** areAllQueuesEmpty, sendFile, getFileTransferProgress: each the next of a scripted list); TheShell (hideShell, showShell);
** MapTransferLoadScreen (init, setCurrentFilename, processTimeout, update, processProgress).
**
** Entry point (extern "C", tests/differential_draft_ftxfer.rs): see its declaration. */
#include <string.h>
#include <stdio.h>
#include <string>
#include <vector>

#define DEBUG_LOG(x)
template <class T> static inline T min( T a, T b ) { return a < b ? a : b; }
enum { MAX_SLOTS = 8 };

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theFtXferAllocator;

static std::string theLog;
static std::vector<UnsignedInt> theTimes;
static std::vector<int> theQueues;
static std::vector<int> theProgress;
static size_t nTime, nQueue, nProgress;
static UnsignedInt lastTime;
static int theAnnounceId;
static void logf( const char *fmt, ... ) __attribute__(( format( printf, 1, 2 ) ));
#include <stdarg.h>
static void logf( const char *fmt, ... )
{
	char b[512];
	va_list ap;
	va_start( ap, fmt );
	vsnprintf( b, sizeof( b ), fmt, ap );
	va_end( ap );
	theLog += b;
	theLog += "\n";
}

static UnsignedInt timeGetTime( void )
{
	UnsignedInt t;
	if (nTime < theTimes.size()) t = theTimes[nTime++]; else t = lastTime + 60000u;
	lastTime = t;
	logf( "time %u", t );
	return t;
}
static void Sleep( UnsignedInt ms ) { logf( "sleep %u", ms ); }

class GameSlotStandIn
{
public:
	Bool human, map;
	Bool isHuman( void ) const { return human; }
	Bool hasMap( void ) const { return map; }
};
class GameInfo
{
public:
	GameSlotStandIn slot[MAX_SLOTS];
	Bool host;
	Int contents;
	AsciiString mapName;
	Bool amIHost( void ) const { return host; }
	const GameSlotStandIn *getConstSlot( Int i ) const { return &slot[i]; }
	Int getMapContentsMask( void ) const { return contents; }
	AsciiString getMap( void ) const { return mapName; }
};
static GameInfo *TheGameInfo;

class NetworkStandIn
{
public:
	UnsignedShort sendFileAnnounce( AsciiString path, UnsignedByte mask ) { logf( "announce %s %u", path.str(), (unsigned)mask ); return (UnsignedShort)theAnnounceId; }
	Bool areAllQueuesEmpty( void ) { int v = nQueue < theQueues.size() ? theQueues[nQueue++] : 1; logf( "queues %d", v ); return v; }
	void sendFile( AsciiString path, UnsignedByte mask, UnsignedShort id ) { logf( "sendFile %s %u %u", path.str(), (unsigned)mask, (unsigned)id ); }
	Int getFileTransferProgress( Int slot, AsciiString path ) { int v = nProgress < theProgress.size() ? theProgress[nProgress++] : 100; logf( "fp %d %s %d", slot, path.str(), v ); return v; }
};
static NetworkStandIn theNetworkObj;
static NetworkStandIn *TheNetwork = &theNetworkObj;

class ShellStandIn
{
public:
	void hideShell( void ) { logf( "shell.hide" ); }
	void showShell( void ) { logf( "shell.show" ); }
};
static ShellStandIn theShellObj;
static ShellStandIn *TheShell = &theShellObj;

class MapTransferLoadScreen
{
public:
	void init( GameInfo *game ) { logf( "ls.init" ); }
	~MapTransferLoadScreen() { logf( "ls.delete" ); }
	void setCurrentFilename( AsciiString path ) { logf( "ls.filename %s", path.str() ); }
	void processTimeout( Int seconds ) { logf( "ls.timeout %d", seconds ); }
	void update( Int percent ) { logf( "ls.update %d", percent ); }
	void processProgress( Int playerId, Int percentage, const char *stateStr ) { logf( "ls.progress %d %d %s", playerId, percentage, stateStr ); }
};

#include "ft_xfer.inc"

/* The scenario: slots (bit i of humans / hasMap), host, contents mask, the map path, the announce id, the scripted lists.
** Returns DoAnyMapTransfers' result; the log is read with ref_ftx_log. */
extern "C" int ref_ftx_run( int host, unsigned humans, unsigned hasMap, int contents, const char *map, int announceId,
		const unsigned *times, int nTimes, const int *queues, int nQueues, const int *progress, int nProg )
{
	TheDynamicMemoryAllocator = &theFtXferAllocator;
	theLog.clear();
	theTimes.assign( times, times + nTimes );
	theQueues.assign( queues, queues + nQueues );
	theProgress.assign( progress, progress + nProg );
	nTime = nQueue = nProgress = 0;
	lastTime = nTimes ? times[nTimes - 1] : 0;
	theAnnounceId = announceId;
	GameInfo game;
	game.host = host;
	game.contents = contents;
	game.mapName = map;
	for (int i = 0; i < MAX_SLOTS; ++i) { game.slot[i].human = (humans >> i) & 1; game.slot[i].map = (hasMap >> i) & 1; }
	Bool r = DoAnyMapTransfers( &game );
	return r;
}
extern "C" int ref_ftx_log( char *out, int cap )
{
	int n = (int)theLog.size();
	if (n + 1 > cap) return -1;
	memcpy( out, theLog.c_str(), n + 1 );
	return n;
}
