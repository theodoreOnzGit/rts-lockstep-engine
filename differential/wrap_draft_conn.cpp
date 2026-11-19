/* Differential-test wrapper for the network layer's connection and transport drafts (GitHub #215, group network2; upstream commit
** 0a05454d): src/drafts/network/connection.rs and transport.rs.
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/Connection.cpp, NetCommandMsg.cpp, NetCommandRef.cpp, NetCommandList.cpp,
**     NetworkUtil.cpp                                        (#included below, unchanged)
**   .../GameNetwork/NetPacket.cpp                            (build.rs's copy in netpkt_gen: two mechanical edits, see drafts_netpkt)
**   .../GameNetwork/Transport.cpp                            (cut verbatim by build.rs into network2_gen: the crypt functions, reset to the
**                                                             end of the file, and the tail of init after the bind; one edit, S_un.S_addr)
**   .../Source/Common/crc.cpp, Include/GameNetwork/Connection.h, Transport.h, User.h, NetPacket.h, ... (unchanged)
** on the stand-ins of differential/shim_network2: udp.h (the socket: in-memory datagram queues, replacing udp.cpp and Winsock; it does
** what the drafts' queued socket does, which is what the real UDP does on a working socket, plus a failure switch standing for the OS
** failing), transport.h / networkdefs.h (the lower-case names of Transport.h / NetworkDefs.h), GameLogic.h (Connection.cpp includes it
** and uses nothing from it); of differential/shim_netpkt (NetworkUtil.h, Player, PlayerList, GameState, as for the netpkt group); and
** here: `timeGetTime()` (a settable clock), the Transport constructor and destructor (upstream's call reset and Winsock; replaced by
** zeroing and a null socket), `WSACleanup` (a no-op: `m_winsockInit` is never set), and User (User.h's inline members only; User.cpp's
** operators are not used by Connection).
**
** Entry points (extern "C", tests/differential_draft_conn.rs): cn_run (a Connection against a Transport) and tr_run (the Transport
** against the socket stand-in); the op in in[0]. The encodings are at the two functions. */
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
#define timeGetTime n2_timeGetTime

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
unsigned int n2_timeGetTime();
static void WSACleanup() {}
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#define protected public
#define private public
#include "Common/MessageStream.h"
#include "GameNetwork/NetPacket.h"
#include "GameNetwork/NetCommandMsg.h"
#include "GameNetwork/NetCommandList.h"
#include "GameNetwork/NetCommandRef.h"
#include "GameNetwork/Transport.h"
#include "GameNetwork/Connection.h"
/* crc.h's release CRC is an MSVC `_asm` block that g++ cannot compile; _DEBUG (for these two lines only) selects its C++ class, the same
** loop (tests/differential_crc.rs checks the Rust against both) */
#define _DEBUG 1
#include "Common/crc.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/crc.cpp"
#undef _DEBUG
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/GameMessageParser.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetworkUtil.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandMsg.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandRef.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandList.cpp"
#include "NetPacket.cpp"   /* build.rs's copy (OUT_DIR/netpkt_gen), two edits */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/Connection.cpp"
#include "transport_crypt.inc"   /* build.rs's cut (OUT_DIR/network2_gen) */
#include "transport_main.inc"    /* build.rs's cut (OUT_DIR/network2_gen) */
/* the tail of Transport::init(UnsignedInt, UnsignedShort), verbatim (build.rs's cut), as a member of a class derived from Transport so
** that it sees the members as upstream's does; `i` is declared here because the cut's second loop uses the first loop's (MSVC's) scope */
struct InitTransport : public Transport
{
	void initTail(UnsignedShort port)
	{
		int i;
#include "transport_init_tail.inc"
	}
};
Transport::Transport(void) { m_winsockInit = false; m_udpsock = NULL; }
Transport::~Transport(void) {}
#undef protected
#undef private

/* ---- the stand-ins ---- */
static NpPlayerList g_playerList;
NpPlayerList *NpThePlayerList = &g_playerList;
static NpNameKeyGenerator g_nameKeys;
NpNameKeyGenerator *NpTheNameKeyGenerator = &g_nameKeys;
AsciiString NpGameState::realMapPathToPortableMapPath(const AsciiString &in) const { AsciiString r; r.set((std::string("P:") + in.str()).c_str()); return r; }
AsciiString NpGameState::portableMapPathToRealMapPath(const AsciiString &in) const { AsciiString r; r.set((std::string("R:") + in.str()).c_str()); return r; }
static NpGameState g_gameState;
NpGameState *NpTheGameState = &g_gameState;
static unsigned g_now = 0;
unsigned int n2_timeGetTime() { return g_now; }

namespace {

typedef std::vector<unsigned> Words;

struct Rd
{
	const unsigned *w; int n; int i;
	unsigned next() { return i < n ? w[i++] : 0; }
};

void emit(Words &o, unsigned v) { o.push_back(v); }
void emit64(Words &o, long long v) { o.push_back((unsigned)(v & 0xffffffffLL)); o.push_back((unsigned)((unsigned long long)v >> 32)); }

unsigned fnv(unsigned h, unsigned char c) { return (h ^ c) * 16777619u; }
unsigned hashBytes(const unsigned char *p, unsigned n) { unsigned h = 2166136261u; for (unsigned i = 0; i < n; ++i) h = fnv(h, p[i]); return h; }

std::vector<unsigned char> readBytes(Rd &r)
{
	unsigned n = r.next();
	std::vector<unsigned char> b(n);
	for (unsigned i = 0; i < n; i += 4) {
		unsigned v = r.next();
		for (unsigned j = 0; j < 4 && i + j < n; ++j) b[i + j] = (unsigned char)(v >> (8 * j));
	}
	return b;
}

/* A command ("spec"), in words: [kind (the NetCommandType), player, frame, id, payload...]; the kinds: 0 1 2 acks (commandID,
** originalPlayerID), 3 frame info (commandCount), 5 player leave (leavingPlayerID), 7 run-ahead metrics is not used, 9 keep alive,
** 14 progress (percentage), 18 file (portable filename as bytes, file data as bytes), 21 frame resend request (frame), 22 packet router
** query (a plain message). `bytes` is a count, then the bytes four to a word. */
NetCommandMsg *makeMsg(Rd &r)
{
	unsigned kind = r.next(), player = r.next(), frame = r.next(), id = r.next();
	NetCommandMsg *m = NULL;
	switch (kind) {
	case NETCOMMANDTYPE_ACKBOTH: { NetAckBothCommandMsg *a = newInstance(NetAckBothCommandMsg); a->setCommandID((UnsignedShort)r.next()); a->setOriginalPlayerID((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_ACKSTAGE1: { NetAckStage1CommandMsg *a = newInstance(NetAckStage1CommandMsg); a->setCommandID((UnsignedShort)r.next()); a->setOriginalPlayerID((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_ACKSTAGE2: { NetAckStage2CommandMsg *a = newInstance(NetAckStage2CommandMsg); a->setCommandID((UnsignedShort)r.next()); a->setOriginalPlayerID((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_FRAMEINFO: { NetFrameCommandMsg *a = newInstance(NetFrameCommandMsg); a->setCommandCount((UnsignedShort)r.next()); m = a; break; }
	case NETCOMMANDTYPE_PLAYERLEAVE: { NetPlayerLeaveCommandMsg *a = newInstance(NetPlayerLeaveCommandMsg); a->setLeavingPlayerID((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_KEEPALIVE: m = newInstance(NetKeepAliveCommandMsg); break;
	case NETCOMMANDTYPE_PROGRESS: { NetProgressCommandMsg *a = newInstance(NetProgressCommandMsg); a->setPercentage((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_FILE: {
		NetFileCommandMsg *a = newInstance(NetFileCommandMsg);
		std::vector<unsigned char> nm = readBytes(r), d = readBytes(r);
		std::string s(nm.begin(), nm.end());
		AsciiString as; as.set(s.c_str()); a->setPortableFilename(as);
		unsigned char dummy = 0;
		a->setFileData(d.empty() ? &dummy : &d[0], (UnsignedInt)d.size());
		m = a; break;
	}
	case NETCOMMANDTYPE_FRAMERESENDREQUEST: { NetFrameResendRequestCommandMsg *a = newInstance(NetFrameResendRequestCommandMsg); a->setFrameToResend(r.next()); m = a; break; }
	default: m = newInstance(NetCommandMsg); break;
	}
	m->setNetCommandType((NetCommandType)(int)kind);
	m->setPlayerID(player); m->setExecutionFrame(frame); m->setID((UnsignedShort)id);
	return m;
}

/* what the log says of one command: type, player, frame, id, then the payload by kind (the wrapper's chunk fields and a hash of its
** data: the wrapper commands a big command was split into) */
void describe(Words &o, NetCommandMsg *m)
{
	emit(o, (unsigned)(int)m->getNetCommandType()); emit(o, m->getPlayerID()); emit(o, m->getExecutionFrame()); emit(o, m->getID());
	switch (m->getNetCommandType()) {
	case NETCOMMANDTYPE_WRAPPER: {
		NetWrapperCommandMsg *a = (NetWrapperCommandMsg *)m;
		emit(o, a->getWrappedCommandID()); emit(o, a->getChunkNumber()); emit(o, a->getNumChunks()); emit(o, a->getDataOffset()); emit(o, a->getTotalDataLength());
		emit(o, a->getDataLength()); emit(o, hashBytes(a->getData(), a->getDataLength()));
		break;
	}
	case NETCOMMANDTYPE_FILE: {
		NetFileCommandMsg *a = (NetFileCommandMsg *)m;
		AsciiString nm = a->getPortableFilename();
		emit(o, hashBytes((const unsigned char *)nm.str(), nm.getLength())); emit(o, a->getFileLength()); emit(o, hashBytes(a->getFileData(), a->getFileLength()));
		break;
	}
	case NETCOMMANDTYPE_FRAMEINFO: emit(o, ((NetFrameCommandMsg *)m)->getCommandCount()); break;
	case NETCOMMANDTYPE_PLAYERLEAVE: emit(o, ((NetPlayerLeaveCommandMsg *)m)->getLeavingPlayerID()); break;
	case NETCOMMANDTYPE_PROGRESS: emit(o, ((NetProgressCommandMsg *)m)->getPercentage()); break;
	case NETCOMMANDTYPE_FRAMERESENDREQUEST: emit(o, ((NetFrameResendRequestCommandMsg *)m)->getFrameToResend()); break;
	default: break;
	}
}

void dumpTransportOut(Words &o, Transport *t)
{
	unsigned n = 0;
	for (int i = 0; i < MAX_MESSAGES; ++i) if (t->m_outBuffer[i].length != 0) ++n;
	emit(o, n);
	for (int i = 0; i < MAX_MESSAGES; ++i) {
		TransportMessage *m = &t->m_outBuffer[i];
		if (m->length == 0) continue;
		emit(o, i); emit(o, (unsigned)m->length); emit(o, m->addr); emit(o, m->port); emit(o, m->header.crc); emit(o, m->header.magic);
		emit(o, hashBytes(m->data, (unsigned)m->length));
	}
}

void dumpConnection(Words &o, Connection *c)
{
	emit(o, c->m_isQuitting); emit(o, c->m_quitTime);
	if (c->m_user) { emit(o, 1); emit(o, c->m_user->GetIPAddr()); emit(o, c->m_user->GetPort()); } else emit(o, 0);
	if (c->m_netCommandList) {
		emit(o, 1 + (unsigned)c->m_netCommandList->length());
		for (NetCommandRef *x = c->m_netCommandList->getFirstMessage(); x; x = x->getNext()) {
			describe(o, x->getCommand()); emit(o, x->getRelay()); emit64(o, x->getTimeLastSent());
		}
	} else emit(o, 0);
	unsigned al; Real a = c->m_averageLatency; memcpy(&al, &a, 4); emit(o, al);
	unsigned h = 2166136261u;
	for (int i = 0; i < CONNECTION_LATENCY_HISTORY_LENGTH; ++i) { unsigned b; Real f = c->m_latencies[i]; memcpy(&b, &f, 4); for (int k = 0; k < 4; ++k) h = fnv(h, (unsigned char)(b >> (8 * k))); }
	emit(o, h);
	emit(o, (unsigned)c->m_numRetries); emit64(o, c->m_lastTimeSent); emit64(o, c->m_retryMetricsTime); emit64(o, c->m_retryTime); emit64(o, c->m_frameGrouping);
}

}

/* in: [0, then the ops]; each op: [code, args...]. The state is dumped after every op.
**   0 init   1 reset   2 setUser(addr, port)   3 attachTransport   4 setFrameGrouping(t, signed)   5 sendNetCommandMsg(relay, spec)
**   6 clearCommandsExceptFrom(player)   7 doSend: returns the packets made   8 processAck(commandID, player): the returned reference
**   9 processAck(spec): the dispatch on the command's type   10 setQuitting   11 setTime(t)   12 fill the transport's first n slots
**   13 drain the transport   14 isQueueEmpty, isQuitting, getUser */
extern "C" int cn_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	Rd r = {in, nin, 1};
	Words o;
	static Transport *transport = NULL;
	if (transport == NULL) transport = new Transport;
	memset(transport, 0, sizeof(Transport));
	transport->m_udpsock = new UDP;
	g_now = 0;
	Connection *c = newInstance(Connection);
	unsigned nops = r.next();
	for (unsigned k = 0; k < nops; ++k) {
		unsigned op = r.next();
		switch (op) {
		case 0: c->init(); break;
		case 1: c->reset(); break;
		case 2: { User *u = newInstance(User); unsigned a = r.next(); u->SetIPAddr(a); u->SetPort((UnsignedShort)r.next()); c->setUser(u); break; }
		case 3: c->attachTransport(transport); break;
		case 4: c->setFrameGrouping((time_t)(int)r.next()); break;
		case 5: { unsigned relay = r.next(); NetCommandMsg *m = makeMsg(r); c->sendNetCommandMsg(m, (UnsignedByte)relay); m->detach(); break; }
		case 6: c->clearCommandsExceptFrom((Int)r.next()); break;
		case 7: emit(o, c->doSend()); break;
		case 8: {
			unsigned id = r.next(), pl = r.next();
			NetCommandRef *ref = c->processAck((UnsignedShort)id, (UnsignedByte)pl);
			if (ref) { emit(o, 1); describe(o, ref->getCommand()); emit(o, ref->getRelay()); emit64(o, ref->getTimeLastSent()); ref->deleteInstance(); } else emit(o, 0);
			break;
		}
		case 9: {
			NetCommandMsg *m = makeMsg(r);
			NetCommandRef *ref = c->processAck(m);
			if (ref) { emit(o, 1); describe(o, ref->getCommand()); emit(o, ref->getRelay()); emit64(o, ref->getTimeLastSent()); ref->deleteInstance(); } else emit(o, 0);
			m->detach();
			break;
		}
		case 10: c->setQuitting(); break;
		case 11: g_now = r.next(); break;
		case 12: { unsigned n = r.next(); for (unsigned i = 0; i < n && i < (unsigned)MAX_MESSAGES; ++i) transport->m_outBuffer[i].length = 1; break; }
		case 13: for (int i = 0; i < MAX_MESSAGES; ++i) transport->m_outBuffer[i].length = 0; break;
		case 14: emit(o, c->isQueueEmpty()); emit(o, c->isQuitting()); emit(o, c->getUser() != NULL); break;
		default: break;
		}
		emit(o, 0xA5A5A5A5u);
		dumpConnection(o, c);
		dumpTransportOut(o, transport);
	}
	c->deleteInstance();
	delete transport->m_udpsock;
	transport->m_udpsock = NULL;
	if ((int)o.size() > cap) return (int)o.size();
	memcpy(out, o.data(), o.size() * sizeof(unsigned));
	return (int)o.size();
}

namespace {
void dumpTransport(Words &o, Transport *t, size_t &sentSeen)
{
	dumpTransportOut(o, t);
	unsigned n = 0;
	for (int i = 0; i < MAX_MESSAGES; ++i) if (t->m_inBuffer[i].length != 0) ++n;
	emit(o, n);
	for (int i = 0; i < MAX_MESSAGES; ++i) {
		TransportMessage *m = &t->m_inBuffer[i];
		if (m->length == 0) continue;
		emit(o, i); emit(o, (unsigned)m->length); emit(o, m->addr); emit(o, m->port); emit(o, m->header.crc); emit(o, m->header.magic);
		emit(o, hashBytes(m->data, (unsigned)m->length));
	}
	unsigned h = 2166136261u;
	unsigned *arrs[6] = {t->m_incomingBytes, t->m_unknownBytes, t->m_outgoingBytes, t->m_incomingPackets, t->m_unknownPackets, t->m_outgoingPackets};
	for (int a = 0; a < 6; ++a) for (int i = 0; i < MAX_TRANSPORT_STATISTICS_SECONDS; ++i) for (int k = 0; k < 4; ++k) h = fnv(h, (unsigned char)(arrs[a][i] >> (8 * k)));
	emit(o, h); emit(o, (unsigned)t->m_statisticsSlot); emit(o, t->m_lastSecond); emit(o, t->m_port);
	if (t->m_udpsock) {
		emit(o, 1); emit(o, (unsigned)t->m_udpsock->m_lastError);
		emit(o, (unsigned)(t->m_udpsock->outbound.size() - sentSeen));
		for (size_t i = sentSeen; i < t->m_udpsock->outbound.size(); ++i) {
			StandInDatagram &d = t->m_udpsock->outbound[i];
			emit(o, d.addr); emit(o, d.port); emit(o, (unsigned)d.bytes.size()); emit(o, hashBytes(d.bytes.empty() ? NULL : &d.bytes[0], (unsigned)d.bytes.size()));
		}
		sentSeen = t->m_udpsock->outbound.size();
		emit(o, (unsigned)t->m_udpsock->inbound.size());
	} else emit(o, 0);
}
void emitReal(Words &o, Real f) { unsigned b; memcpy(&b, &f, 4); emit(o, b); }
}

/* in: [0, nops, ops]; each op: [code, args...]. The state is dumped after every op.
**   0 init tail (port, now): a fresh socket, then the tail of init   1 queueSend(addr, port, bytes): the result   2 inject a datagram
**   (addr, port, bytes) into the socket's inbound queue   3 doRecv   4 doSend(now)   5 update(now)   6 isGeneralsPacket(length, crc,
**   magic, bytes)   7 the six statistics getters   8 reset   9 the socket's failure switch (status, 0 = working)   10 mark the first n
**   out slots busy   11 mark the first n in slots busy   12 empty both buffers   13 allowBroadcasts is not used */
extern "C" int tr_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	Rd r = {in, nin, 1};
	Words o;
	InitTransport *t = new InitTransport;
	memset((void *)t, 0, sizeof(Transport));
	size_t sentSeen = 0;
	g_now = 0;
	unsigned nops = r.next();
	for (unsigned k = 0; k < nops; ++k) {
		unsigned op = r.next();
		switch (op) {
		case 0: { unsigned port = r.next(); g_now = r.next(); delete t->m_udpsock; t->m_udpsock = new UDP; sentSeen = 0; t->initTail((UnsignedShort)port); break; }
		case 1: { unsigned a = r.next(), p = r.next(); std::vector<unsigned char> b = readBytes(r); unsigned char z = 0; emit(o, t->queueSend(a, (UnsignedShort)p, b.empty() ? &z : &b[0], (Int)b.size())); break; }
		case 2: { unsigned a = r.next(), p = r.next(); std::vector<unsigned char> b = readBytes(r); if (t->m_udpsock) { StandInDatagram d; d.bytes = b; d.addr = a; d.port = (unsigned short)p; t->m_udpsock->inbound.push_back(d); } break; }
		case 3: emit(o, t->doRecv()); break;
		case 4: g_now = r.next(); emit(o, t->doSend()); break;
		case 5: g_now = r.next(); emit(o, t->update()); break;
		case 6: {
			TransportMessage m; memset(&m, 0, sizeof m);
			m.length = (Int)r.next(); m.header.crc = r.next(); m.header.magic = (UnsignedShort)r.next();
			std::vector<unsigned char> b = readBytes(r);
			for (size_t i = 0; i < b.size() && i < (size_t)MAX_MESSAGE_LEN; ++i) m.data[i] = b[i];
			emit(o, t->isGeneralsPacket(&m)); break;
		}
		case 7: emitReal(o, t->getIncomingBytesPerSecond()); emitReal(o, t->getIncomingPacketsPerSecond()); emitReal(o, t->getOutgoingBytesPerSecond()); emitReal(o, t->getOutgoingPacketsPerSecond()); emitReal(o, t->getUnknownBytesPerSecond()); emitReal(o, t->getUnknownPacketsPerSecond()); break;
		case 8: t->reset(); break;
		case 9: { unsigned s = r.next(); if (t->m_udpsock) { t->m_udpsock->m_fail = s != 0; t->m_udpsock->m_failStatus = -(Int)s; } break; }
		case 10: { unsigned n = r.next(); for (unsigned i = 0; i < n && i < (unsigned)MAX_MESSAGES; ++i) t->m_outBuffer[i].length = 1; break; }
		case 11: { unsigned n = r.next(); for (unsigned i = 0; i < n && i < (unsigned)MAX_MESSAGES; ++i) t->m_inBuffer[i].length = 1; break; }
		case 12: for (int i = 0; i < MAX_MESSAGES; ++i) { t->m_outBuffer[i].length = 0; t->m_inBuffer[i].length = 0; } break;
		default: break;
		}
		emit(o, 0xA5A5A5A5u);
		dumpTransport(o, t, sentSeen);
	}
	delete t->m_udpsock;
	t->m_udpsock = NULL;
	if ((int)o.size() > cap) return (int)o.size();
	memcpy(out, o.data(), o.size() * sizeof(unsigned));
	return (int)o.size();
}
