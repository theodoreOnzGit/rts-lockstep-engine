/* Differential-test wrapper for the network layer's bookkeeping drafts (GitHub #215, group netlogic; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/FrameDataManager.cpp, FrameData.cpp, NetCommandList.cpp, NetCommandRef.cpp,
**   NetCommandMsg.cpp, NetworkUtil.cpp                                       (#included below, unchanged)
**   .../GameNetwork/FrameMetrics.cpp                                         (build.rs's copy, netlogic_gen: one for-scope edit)
** on the stand-ins of differential/shim_netlogic (Display.h: `TheDisplay->getAverageFPS()`, the three history lengths of
** `TheGlobalData` and `timeGetTime()`, all renamed Nl* so that no symbol is shared with another wrapper's library) and of
** differential/shim_netpkt (NetworkUtil.h, Player, PlayerList, GameState). `FRAME_DATA_LENGTH` and `MAX_FRAMES_AHEAD` (NetworkDefs.h
** `extern Int`) are defined here and set per case by the test.
**
** Entry point (extern "C", tests/differential_draft_netlogic.rs): nl_run(in, nin, out, cap), the op in in[0]. */
#define GameMessage MsTestGameMessage
#define GameMessageArgument MsTestGameMessageArgument
#define GameMessageList MsTestGameMessageList
#define GameMessageTranslator MsTestGameMessageTranslator
#define MessageStream MsTestMessageStream
#define CommandList MsTestCommandList
#define TheMessageStream MsTestTheMessageStream
#define TheCommandList MsTestTheCommandList
#define buildRegion MsTestBuildRegion
#define isInvalidDebugCommand MsTestIsInvalidDebugCommand
#define Player NpPlayer
#define PlayerList NpPlayerList
#define ThePlayerList NpThePlayerList
#define NameKeyGenerator NpNameKeyGenerator
#define TheNameKeyGenerator NpTheNameKeyGenerator
#define GameState NpGameState
#define TheGameState NpTheGameState
#define GameMessageParser NpGameMessageParser
#define GameMessageParserArgumentType NpGameMessageParserArgumentType
#define Display NlDisplay
#define TheDisplay NlTheDisplay
#define GlobalData NlGlobalData
#define TheGlobalData NlTheGlobalData
#define timeGetTime nl_timeGetTime

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <netdb.h>
#include <arpa/inet.h>
#include <algorithm>
#include <string>
#include <vector>
using std::min;
using std::max;
#define PI 3.14159265359f
#define _MAX_PATH 260   /* Windows' */
#define REAL_TO_INT(x) ((Int)(x))
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#define protected public
#define private public
#include "Common/MessageStream.h"
#include "GameNetwork/NetCommandMsg.h"
#include "GameNetwork/NetCommandList.h"
#include "GameNetwork/NetCommandRef.h"
#include "GameNetwork/FrameData.h"
#include "GameNetwork/FrameDataManager.h"
#include "GameNetwork/FrameMetrics.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetworkUtil.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandMsg.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandRef.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandList.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/FrameData.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/FrameDataManager.cpp"
#include "FrameMetrics.cpp"   /* build.rs's copy (OUT_DIR/netlogic_gen), one edit */
#undef protected
#undef private

/* ---- the stand-ins ---- */
/* (MAX_FRAMES_AHEAD and FRAME_DATA_LENGTH are NetworkUtil.cpp's) */
static NpPlayerList g_playerList;
NpPlayerList *NpThePlayerList = &g_playerList;
static NpNameKeyGenerator g_nameKeys;
NpNameKeyGenerator *NpTheNameKeyGenerator = &g_nameKeys;
AsciiString NpGameState::realMapPathToPortableMapPath(const AsciiString &in) const { AsciiString r; r.set((std::string("P:") + in.str()).c_str()); return r; }
AsciiString NpGameState::portableMapPathToRealMapPath(const AsciiString &in) const { AsciiString r; r.set((std::string("R:") + in.str()).c_str()); return r; }
static NpGameState g_gameState;
NpGameState *NpTheGameState = &g_gameState;

static unsigned g_now = 0;
static Real g_fps = 0;
struct FixedDisplay : public NlDisplay { Real getAverageFPS(void) { return g_fps; } };
static FixedDisplay g_display;
NlDisplay *NlTheDisplay = &g_display;
static NlGlobalData g_global;
NlGlobalData *NlTheGlobalData = &g_global;
unsigned int nl_timeGetTime() { return g_now; }

namespace {

typedef std::vector<unsigned> Words;

struct Rd
{
	const unsigned *w; int n; int i;
	unsigned next() { return i < n ? w[i++] : 0; }
};

void emit(Words &o, unsigned v) { o.push_back(v); }

/* A command ("spec"): [kind, player, frame, id, payload...]; kinds 3 (frame info: commandCount), 9 (keep alive), 14 (progress:
** percentage), 5 (player leave: leavingPlayerID), 21 (frame resend request: frame). */
NetCommandMsg *makeMsg(Rd &r)
{
	unsigned kind = r.next(), player = r.next(), frame = r.next(), id = r.next();
	NetCommandMsg *m = NULL;
	switch (kind) {
	case NETCOMMANDTYPE_FRAMEINFO: { NetFrameCommandMsg *a = newInstance(NetFrameCommandMsg); a->setCommandCount((UnsignedShort)r.next()); m = a; break; }
	case NETCOMMANDTYPE_KEEPALIVE: m = newInstance(NetKeepAliveCommandMsg); break;
	case NETCOMMANDTYPE_PROGRESS: { NetProgressCommandMsg *a = newInstance(NetProgressCommandMsg); a->setPercentage((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_PLAYERLEAVE: { NetPlayerLeaveCommandMsg *a = newInstance(NetPlayerLeaveCommandMsg); a->setLeavingPlayerID((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_FRAMERESENDREQUEST: { NetFrameResendRequestCommandMsg *a = newInstance(NetFrameResendRequestCommandMsg); a->setFrameToResend(r.next()); m = a; break; }
	default: m = newInstance(NetCommandMsg); break;
	}
	m->setNetCommandType((NetCommandType)(int)kind);
	m->setPlayerID(player); m->setExecutionFrame(frame); m->setID((UnsignedShort)id);
	return m;
}

int posOf(NetCommandList *l, NetCommandRef *r)
{
	int i = 0;
	for (NetCommandRef *x = l->getFirstMessage(); x; x = x->getNext(), ++i) if (x == r) return i;
	return -1;
}

void summary(Words &o, NetCommandList *l)
{
	emit(o, (unsigned)l->length());
	for (NetCommandRef *x = l->getFirstMessage(); x; x = x->getNext()) {
		NetCommandMsg *m = x->getCommand();
		emit(o, (unsigned)(int)m->getNetCommandType()); emit(o, m->getPlayerID()); emit(o, m->getExecutionFrame()); emit(o, m->getID());
		emit(o, x->getRelay()); emit(o, (unsigned)m->getSortNumber());
	}
	emit(o, (unsigned)posOf(l, l->m_last));
}

unsigned bitsOf(Real f) { unsigned b; memcpy(&b, &f, 4); return b; }
Real realOf(unsigned b) { Real f; memcpy(&f, &b, 4); return f; }

/* op 1: a FrameDataManager. in: [1, MAX_FRAMES_AHEAD, FRAME_DATA_LENGTH, isLocal, nops, ops...]:
**   0 init  1 reset  2 update  3 spec addNetCommandMsg  4 frame flag allCommandsReady  5 frame getFrameCommandList
**   6 frame isAdvancing resetFrame  7 frame getCommandCount  8 frame count setFrameCommandCount  9 frame getFrameCommandCount
**   10 start n zeroFrames  11 destroyGameMessages  12 frame setQuitFrame
** after each op: [quit frame, isQuitting, then per frame index: getFrame (the ring's), getCommandCount, getFrameCommandCount]. */
void op_fdm(Rd &r, Words &o)
{
	MAX_FRAMES_AHEAD = (Int)r.next();
	FRAME_DATA_LENGTH = (Int)r.next();
	Bool local = r.next() & 1;
	FrameDataManager *m = new FrameDataManager(local);
	unsigned nops = r.next();
	for (unsigned k = 0; k < nops; ++k) {
		unsigned code = r.next();
		switch (code) {
		case 0: m->init(); break;
		case 1: m->reset(); break;
		case 2: m->update(); break;
		case 3: m->addNetCommandMsg(makeMsg(r)); break;
		case 4: { unsigned f = r.next(); emit(o, (unsigned)m->allCommandsReady(f, (r.next() & 1) != 0)); break; }
		case 5: { NetCommandList *l = m->getFrameCommandList(r.next()); emit(o, l != NULL); if (l) summary(o, l); break; }
		case 6: { unsigned f = r.next(); m->resetFrame(f, (r.next() & 1) != 0); break; }
		case 7: emit(o, m->getCommandCount(r.next())); break;
		case 8: { unsigned f = r.next(); m->setFrameCommandCount(f, r.next()); break; }
		case 9: emit(o, m->getFrameCommandCount(r.next())); break;
		case 10: { unsigned s = r.next(); m->zeroFrames(s, r.next()); break; }
		case 11: m->destroyGameMessages(); break;
		case 12: m->setQuitFrame(r.next()); break;
		default: break;
		}
		emit(o, m->getQuitFrame()); emit(o, m->getIsQuitting());
		for (Int i = 0; i < FRAME_DATA_LENGTH; ++i) {
			emit(o, m->m_frameData[i].getFrame()); emit(o, m->m_frameData[i].getCommandCount()); emit(o, m->m_frameData[i].getFrameCommandCount());
			NetCommandList *l = m->m_frameData[i].getCommandList();
			emit(o, l != NULL);
			if (l) summary(o, l);
		}
	}
	delete m;
}

/* op 2: a FrameMetrics. in: [2, fpsLen, latencyLen, cushionLen, MAX_FRAMES_AHEAD, nops, ops...]:
**   0 init  1 reset  2 time fpsBits frame doPerFrameMetrics  3 time frame processLatencyResponse  4 cushion addCushion
** after each op: [getAverageFPS, getAverageLatency (bits), getMinimumCushion, fps index, last fps time, cushion index, the fps list
** (bits), the latency list (bits), the pending latencies (low and high words)]. */
void op_metrics(Rd &r, Words &o)
{
	g_global.m_networkFPSHistoryLength = r.next();
	g_global.m_networkLatencyHistoryLength = r.next();
	g_global.m_networkCushionHistoryLength = r.next();
	MAX_FRAMES_AHEAD = (Int)r.next();
	FrameMetrics *m = new FrameMetrics;
	unsigned nops = r.next();
	for (unsigned k = 0; k < nops; ++k) {
		unsigned code = r.next();
		switch (code) {
		case 0: m->init(); break;
		case 1: m->reset(); break;
		case 2: { g_now = r.next(); g_fps = realOf(r.next()); m->doPerFrameMetrics(r.next()); break; }
		case 3: { g_now = r.next(); m->processLatencyResponse(r.next()); break; }
		case 4: m->addCushion((Int)r.next()); break;
		default: break;
		}
		emit(o, (unsigned)m->getAverageFPS()); emit(o, bitsOf(m->getAverageLatency())); emit(o, (unsigned)m->getMinimumCushion());
		emit(o, (unsigned)m->m_fpsListIndex); emit(o, (unsigned)m->m_lastFpsTimeThing); emit(o, m->m_cushionIndex); emit(o, bitsOf(m->m_averageFps));
		for (unsigned i = 0; i < g_global.m_networkFPSHistoryLength; ++i) emit(o, bitsOf(m->m_fpsList[i]));
		for (unsigned i = 0; i < g_global.m_networkLatencyHistoryLength; ++i) emit(o, bitsOf(m->m_latencyList[i]));
		for (Int i = 0; i < MAX_FRAMES_AHEAD; ++i) { emit(o, (unsigned)(m->m_pendingLatencies[i] & 0xffffffff)); emit(o, (unsigned)((unsigned long)m->m_pendingLatencies[i] >> 32)); }
	}
	delete m;
}

} // namespace

extern "C" int nl_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	Rd r = {in, nin, 0};
	Words o;
	unsigned op = r.next();
	switch (op) {
	case 1: op_fdm(r, o); break;
	case 2: op_metrics(r, o); break;
	default: break;
	}
	int n = (int)o.size();
	if (n <= cap) memcpy(out, o.data(), n * sizeof(unsigned));
	return n;
}
