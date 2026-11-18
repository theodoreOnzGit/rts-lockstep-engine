/* Differential-test wrapper for the network layer's wire format (GitHub #215, the drafts of #202; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/NetPacket.cpp          (build.rs's copy, netpkt_gen, two edits: see there)
**   .../GameNetwork/NetCommandMsg.cpp, NetCommandRef.cpp, NetCommandList.cpp, NetCommandWrapperList.cpp, FrameData.cpp,
**   NetworkUtil.cpp, GameMessageParser.cpp                                  (#included below, unchanged)
**   .../Include/GameNetwork/NetPacket.h, NetCommandMsg.h, NetCommandList.h, NetCommandRef.h, NetCommandWrapperList.h,
**   FrameData.h, NetworkDefs.h                                              (unchanged)
**   .../Source/GameNetwork/Transport.cpp: encryptBuf and decryptBuf          (cut verbatim by build.rs into
**                                                                             netpkt_gen/transport_crypt.inc)
** on the stand-ins of differential/shim_netpkt: NetworkUtil.h (upstream's is the lower-case networkutil.h, which pulls in
** NetworkInterface.h), Player and PlayerList (constructGameMessage looks a player up by name: every name is one player;
** its index goes into the GameMessage, never onto the wire), the name key generator (every name is key 1) and GameState
** (the real <-> portable map path of the file commands: a prefix). GameMessage itself is MessageStream.cpp's, linked from
** wrap_message_stream.cpp under the same renamed names. REAL_TO_INT is `(Int)(x)` (the real one truncates through
** fast_float2long_round(fast_float_trunc(x))); _MAX_PATH is Windows' 260.
**
** Entry point (extern "C", tests/differential_draft_netpkt.rs): ref_np_run(in, nin, out, cap), the op in in[0]; the word
** encoding of a command ("spec") is described at makeMsg(). */
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
#define REAL_TO_INT(x) ((Int)(x))   /* the real BaseType.h: fast_float2long_round(fast_float_trunc(x)), a truncation */
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
#include "GameNetwork/NetCommandWrapperList.h"
#include "GameNetwork/FrameData.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/GameMessageParser.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetworkUtil.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandMsg.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandRef.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandList.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandWrapperList.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/FrameData.cpp"
#include "NetPacket.cpp"   /* build.rs's copy (OUT_DIR/netpkt_gen), two edits */
#include "transport_crypt.inc"   /* build.rs's cut (OUT_DIR/netpkt_gen): Transport.cpp's encryptBuf and decryptBuf */
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

namespace {

typedef std::vector<unsigned> Words;

struct Rd
{
	const unsigned *w; int n; int i;
	unsigned next() { return i < n ? w[i++] : 0; }
};

void emit(Words &o, unsigned v) { o.push_back(v); }

/* n, then the n bytes packed four to a word (little-endian, the last word zero-padded) */
void emitBytes(Words &o, const unsigned char *p, unsigned n)
{
	o.push_back(n);
	for (unsigned i = 0; i < n; i += 4) {
		unsigned v = 0;
		for (unsigned j = 0; j < 4 && i + j < n; ++j) v |= (unsigned)p[i + j] << (8 * j);
		o.push_back(v);
	}
}

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

void emitUStr(Words &o, UnicodeString t)
{
	o.push_back((unsigned)t.getLength());
	for (int i = 0; i < t.getLength(); ++i) o.push_back((unsigned)t.str()[i]);
}

UnicodeString readUStr(Rd &r)
{
	unsigned n = r.next();
	std::basic_string<WideChar> s;
	for (unsigned i = 0; i < n; ++i) { unsigned c = r.next() & 0xffff; s += (WideChar)(c ? c : 1); }
	UnicodeString u;
	u.set(s.c_str());
	return u;
}

/* the words one argument of this type holds (the union's leading words) */
unsigned argWords(unsigned type)
{
	switch (type) {
	case ARGUMENTDATATYPE_LOCATION: return 3;
	case ARGUMENTDATATYPE_PIXEL: return 2;
	case ARGUMENTDATATYPE_PIXELREGION: return 4;
	default: return 1;
	}
}

/* One command ("spec"), in words: [kind (the NetCommandType), player, frame, id, relay, payload...]. The payload by kind:
**   0 1 2 acks            commandID, originalPlayerID
**   3 frame               commandCount
**   4 game                message type, argument count, then per argument [data type, 1 to 4 words of its data (argWords)]
**   5 player leave        leavingPlayerID
**   6 run-ahead metrics   averageLatency (bits), averageFps
**   7 run-ahead           runAhead, frameRate
**   8 destroy player      playerIndex
**   10 disconnect chat    string (count, one 16-bit unit per word)
**   11 chat               playerMask, string
**   14 progress           percentage
**   17 wrapper            wrappedCommandID, chunkNumber, numChunks, dataOffset, totalDataLength, then the data as bytes
**   18 file               portable filename (bytes), file data (bytes)
**   19 file announce      portable filename (bytes), fileID, playerMask
**   20 file progress      fileID, progress
**   21 frame resend       frame
**   24 disconnect player  slot, frame
**   27 disconnect vote    slot, voteFrame
**   28 disconnect frame   frame
**   29 screen off         newFrame
** and nothing for the rest. `bytes` is a count, then the bytes four to a word; the wrapper's data length is its byte count.
** describe() writes this same encoding, so a command read back from a packet is compared word for word with the spec. */
NetCommandMsg *makeMsg(Rd &r, unsigned &relay)
{
	unsigned kind = r.next(), player = r.next(), frame = r.next(), id = r.next();
	relay = r.next();
	NetCommandMsg *m = NULL;
	switch (kind) {
	case NETCOMMANDTYPE_ACKBOTH: { NetAckBothCommandMsg *a = newInstance(NetAckBothCommandMsg); a->setCommandID((UnsignedShort)r.next()); a->setOriginalPlayerID((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_ACKSTAGE1: { NetAckStage1CommandMsg *a = newInstance(NetAckStage1CommandMsg); a->setCommandID((UnsignedShort)r.next()); a->setOriginalPlayerID((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_ACKSTAGE2: { NetAckStage2CommandMsg *a = newInstance(NetAckStage2CommandMsg); a->setCommandID((UnsignedShort)r.next()); a->setOriginalPlayerID((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_FRAMEINFO: { NetFrameCommandMsg *a = newInstance(NetFrameCommandMsg); a->setCommandCount((UnsignedShort)r.next()); m = a; break; }
	case NETCOMMANDTYPE_GAMECOMMAND: {
		NetGameCommandMsg *a = newInstance(NetGameCommandMsg);
		a->setGameMessageType((GameMessage::Type)(int)r.next());
		unsigned n = r.next();
		for (unsigned k = 0; k < n; ++k) {
			unsigned t = r.next();
			GameMessageArgumentType u;
			memset(&u, 0, sizeof u);
			unsigned nw = argWords(t), w[4] = {0, 0, 0, 0};
			for (unsigned q = 0; q < nw; ++q) w[q] = r.next();
			memcpy(&u, w, 16);
			a->addArgument((GameMessageArgumentDataType)t, u);
		}
		m = a; break;
	}
	case NETCOMMANDTYPE_PLAYERLEAVE: { NetPlayerLeaveCommandMsg *a = newInstance(NetPlayerLeaveCommandMsg); a->setLeavingPlayerID((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_RUNAHEADMETRICS: { NetRunAheadMetricsCommandMsg *a = newInstance(NetRunAheadMetricsCommandMsg); unsigned b = r.next(); Real f; memcpy(&f, &b, 4); a->setAverageLatency(f); a->setAverageFps((Int)r.next()); m = a; break; }
	case NETCOMMANDTYPE_RUNAHEAD: { NetRunAheadCommandMsg *a = newInstance(NetRunAheadCommandMsg); a->setRunAhead((UnsignedShort)r.next()); a->setFrameRate((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_DESTROYPLAYER: { NetDestroyPlayerCommandMsg *a = newInstance(NetDestroyPlayerCommandMsg); a->setPlayerIndex(r.next()); m = a; break; }
	case NETCOMMANDTYPE_KEEPALIVE: m = newInstance(NetKeepAliveCommandMsg); break;
	case NETCOMMANDTYPE_DISCONNECTCHAT: { NetDisconnectChatCommandMsg *a = newInstance(NetDisconnectChatCommandMsg); a->setText(readUStr(r)); m = a; break; }
	case NETCOMMANDTYPE_CHAT: { NetChatCommandMsg *a = newInstance(NetChatCommandMsg); a->setPlayerMask((Int)r.next()); a->setText(readUStr(r)); m = a; break; }
	case NETCOMMANDTYPE_PROGRESS: { NetProgressCommandMsg *a = newInstance(NetProgressCommandMsg); a->setPercentage((UnsignedByte)r.next()); m = a; break; }
	case NETCOMMANDTYPE_WRAPPER: {
		NetWrapperCommandMsg *a = newInstance(NetWrapperCommandMsg);
		a->setWrappedCommandID((UnsignedShort)r.next()); a->setChunkNumber(r.next()); a->setNumChunks(r.next()); a->setDataOffset(r.next()); a->setTotalDataLength(r.next());
		std::vector<unsigned char> b = readBytes(r);
		unsigned char dummy = 0;
		a->setData(b.empty() ? &dummy : &b[0], (UnsignedInt)b.size());
		m = a; break;
	}
	case NETCOMMANDTYPE_FILE: {
		NetFileCommandMsg *a = newInstance(NetFileCommandMsg);
		std::vector<unsigned char> nm = readBytes(r), d = readBytes(r);
		std::string s(nm.begin(), nm.end());
		AsciiString as; as.set(s.c_str()); a->setPortableFilename(as);
		unsigned char dummy = 0;
		a->setFileData(d.empty() ? &dummy : &d[0], (UnsignedInt)d.size());
		m = a; break;
	}
	case NETCOMMANDTYPE_FILEANNOUNCE: {
		NetFileAnnounceCommandMsg *a = newInstance(NetFileAnnounceCommandMsg);
		std::vector<unsigned char> nm = readBytes(r);
		std::string s(nm.begin(), nm.end());
		AsciiString as; as.set(s.c_str()); a->setPortableFilename(as);
		a->setFileID((UnsignedShort)r.next()); a->setPlayerMask((UnsignedByte)r.next());
		m = a; break;
	}
	case NETCOMMANDTYPE_FILEPROGRESS: { NetFileProgressCommandMsg *a = newInstance(NetFileProgressCommandMsg); a->setFileID((UnsignedShort)r.next()); a->setProgress((Int)r.next()); m = a; break; }
	case NETCOMMANDTYPE_FRAMERESENDREQUEST: { NetFrameResendRequestCommandMsg *a = newInstance(NetFrameResendRequestCommandMsg); a->setFrameToResend(r.next()); m = a; break; }
	case NETCOMMANDTYPE_DISCONNECTKEEPALIVE: m = newInstance(NetDisconnectKeepAliveCommandMsg); break;
	case NETCOMMANDTYPE_DISCONNECTPLAYER: { NetDisconnectPlayerCommandMsg *a = newInstance(NetDisconnectPlayerCommandMsg); a->setDisconnectSlot((UnsignedByte)r.next()); a->setDisconnectFrame(r.next()); m = a; break; }
	case NETCOMMANDTYPE_PACKETROUTERQUERY: m = newInstance(NetPacketRouterQueryCommandMsg); break;
	case NETCOMMANDTYPE_PACKETROUTERACK: m = newInstance(NetPacketRouterAckCommandMsg); break;
	case NETCOMMANDTYPE_DISCONNECTVOTE: { NetDisconnectVoteCommandMsg *a = newInstance(NetDisconnectVoteCommandMsg); a->setSlot((UnsignedByte)r.next()); a->setVoteFrame(r.next()); m = a; break; }
	case NETCOMMANDTYPE_DISCONNECTFRAME: { NetDisconnectFrameCommandMsg *a = newInstance(NetDisconnectFrameCommandMsg); a->setDisconnectFrame(r.next()); m = a; break; }
	case NETCOMMANDTYPE_DISCONNECTSCREENOFF: { NetDisconnectScreenOffCommandMsg *a = newInstance(NetDisconnectScreenOffCommandMsg); a->setNewFrame(r.next()); m = a; break; }
	case NETCOMMANDTYPE_LOADCOMPLETE:
	case NETCOMMANDTYPE_TIMEOUTSTART:
	default: m = newInstance(NetCommandMsg); break;   /* a plain message of that type (load complete and time-out start are plain ones upstream too) */
	}
	m->setNetCommandType((NetCommandType)(int)kind);
	m->setPlayerID(player); m->setExecutionFrame(frame); m->setID((UnsignedShort)id);
	return m;
}

void describe(Words &o, NetCommandMsg *m, unsigned relay)
{
	unsigned kind = (unsigned)(int)m->getNetCommandType();
	emit(o, kind); emit(o, m->getPlayerID()); emit(o, m->getExecutionFrame()); emit(o, m->getID()); emit(o, relay);
	switch (m->getNetCommandType()) {
	case NETCOMMANDTYPE_ACKBOTH: { NetAckBothCommandMsg *a = (NetAckBothCommandMsg *)m; emit(o, a->getCommandID()); emit(o, a->getOriginalPlayerID()); break; }
	case NETCOMMANDTYPE_ACKSTAGE1: { NetAckStage1CommandMsg *a = (NetAckStage1CommandMsg *)m; emit(o, a->getCommandID()); emit(o, a->getOriginalPlayerID()); break; }
	case NETCOMMANDTYPE_ACKSTAGE2: { NetAckStage2CommandMsg *a = (NetAckStage2CommandMsg *)m; emit(o, a->getCommandID()); emit(o, a->getOriginalPlayerID()); break; }
	case NETCOMMANDTYPE_FRAMEINFO: emit(o, ((NetFrameCommandMsg *)m)->getCommandCount()); break;
	case NETCOMMANDTYPE_GAMECOMMAND: {
		NetGameCommandMsg *a = (NetGameCommandMsg *)m;
		unsigned n = 0;
		for (GameMessageArgument *g = a->m_argList; g; g = g->m_next) ++n;
		emit(o, (unsigned)(int)a->m_type); emit(o, n);
		for (GameMessageArgument *g = a->m_argList; g; g = g->m_next) {
			unsigned t = (unsigned)(int)g->m_type, w[4];
			memcpy(w, &g->m_data, 16);
			emit(o, t);
			unsigned nw = argWords(t);
			for (unsigned q = 0; q < nw; ++q) {
				unsigned v = w[q];
				if (t == ARGUMENTDATATYPE_BOOLEAN) v &= 0xff;          /* a read argument's union is not cleared above its member */
				if (t == ARGUMENTDATATYPE_WIDECHAR) v &= 0xffff;
				emit(o, v);
			}
		}
		break;
	}
	case NETCOMMANDTYPE_PLAYERLEAVE: emit(o, ((NetPlayerLeaveCommandMsg *)m)->getLeavingPlayerID()); break;
	case NETCOMMANDTYPE_RUNAHEADMETRICS: { NetRunAheadMetricsCommandMsg *a = (NetRunAheadMetricsCommandMsg *)m; Real f = a->getAverageLatency(); unsigned b; memcpy(&b, &f, 4); emit(o, b); emit(o, (unsigned)a->getAverageFps()); break; }
	case NETCOMMANDTYPE_RUNAHEAD: { NetRunAheadCommandMsg *a = (NetRunAheadCommandMsg *)m; emit(o, a->getRunAhead()); emit(o, a->getFrameRate()); break; }
	case NETCOMMANDTYPE_DESTROYPLAYER: emit(o, ((NetDestroyPlayerCommandMsg *)m)->getPlayerIndex()); break;
	case NETCOMMANDTYPE_DISCONNECTCHAT: emitUStr(o, ((NetDisconnectChatCommandMsg *)m)->getText()); break;
	case NETCOMMANDTYPE_CHAT: { NetChatCommandMsg *a = (NetChatCommandMsg *)m; emit(o, (unsigned)a->getPlayerMask()); emitUStr(o, a->getText()); break; }
	case NETCOMMANDTYPE_PROGRESS: emit(o, ((NetProgressCommandMsg *)m)->getPercentage()); break;
	case NETCOMMANDTYPE_WRAPPER: {
		NetWrapperCommandMsg *a = (NetWrapperCommandMsg *)m;
		emit(o, a->getWrappedCommandID()); emit(o, a->getChunkNumber()); emit(o, a->getNumChunks()); emit(o, a->getDataOffset()); emit(o, a->getTotalDataLength());
		emitBytes(o, a->getData(), a->getDataLength());
		break;
	}
	case NETCOMMANDTYPE_FILE: {
		NetFileCommandMsg *a = (NetFileCommandMsg *)m;
		AsciiString nm = a->getPortableFilename();
		emitBytes(o, (const unsigned char *)nm.str(), (unsigned)nm.getLength());
		emitBytes(o, a->getFileData(), a->getFileLength());
		break;
	}
	case NETCOMMANDTYPE_FILEANNOUNCE: {
		NetFileAnnounceCommandMsg *a = (NetFileAnnounceCommandMsg *)m;
		AsciiString nm = a->getPortableFilename();
		emitBytes(o, (const unsigned char *)nm.str(), (unsigned)nm.getLength());
		emit(o, a->getFileID()); emit(o, a->getPlayerMask());
		break;
	}
	case NETCOMMANDTYPE_FILEPROGRESS: { NetFileProgressCommandMsg *a = (NetFileProgressCommandMsg *)m; emit(o, a->getFileID()); emit(o, (unsigned)a->getProgress()); break; }
	case NETCOMMANDTYPE_FRAMERESENDREQUEST: emit(o, ((NetFrameResendRequestCommandMsg *)m)->getFrameToResend()); break;
	case NETCOMMANDTYPE_DISCONNECTPLAYER: { NetDisconnectPlayerCommandMsg *a = (NetDisconnectPlayerCommandMsg *)m; emit(o, a->getDisconnectSlot()); emit(o, a->getDisconnectFrame()); break; }
	case NETCOMMANDTYPE_DISCONNECTVOTE: { NetDisconnectVoteCommandMsg *a = (NetDisconnectVoteCommandMsg *)m; emit(o, a->getSlot()); emit(o, a->getVoteFrame()); break; }
	case NETCOMMANDTYPE_DISCONNECTFRAME: emit(o, ((NetDisconnectFrameCommandMsg *)m)->getDisconnectFrame()); break;
	case NETCOMMANDTYPE_DISCONNECTSCREENOFF: emit(o, ((NetDisconnectScreenOffCommandMsg *)m)->getNewFrame()); break;
	default: break;
	}
	/* what the base class derives from it */
	emit(o, (unsigned)m->getSortNumber());
}

/* describe() of a ref, then the contents string (a game command's is built from MessageStream.cpp's name table) */
void describeRef(Words &o, NetCommandRef *r)
{
	describe(o, r->getCommand(), r->getRelay());
	AsciiString c = r->getCommand()->getContentsAsAsciiString();
	emitBytes(o, (const unsigned char *)c.str(), (unsigned)c.getLength());
}

void describeList(Words &o, NetCommandList *l)
{
	emit(o, (unsigned)l->length());
	for (NetCommandRef *r = l->getFirstMessage(); r; r = r->getNext()) describeRef(o, r);
}

void emitPacket(Words &o, NetPacket *p)
{
	emit(o, (unsigned)p->getLength()); emit(o, (unsigned)p->getNumCommands());
	emitBytes(o, p->getData(), (unsigned)p->getLength());
}

/* the isRoomFor function of the command's type (the one addCommand's add function calls first); 2 for a type without */
unsigned roomFor(NetPacket *p, NetCommandRef *ref)
{
	NetCommandMsg *m = ref->getCommand();
	switch (m->getNetCommandType()) {
	case NETCOMMANDTYPE_GAMECOMMAND: { GameMessage *g = ((NetGameCommandMsg *)m)->constructGameMessage(); unsigned v = p->isRoomForGameMessage(ref, g); g->deleteInstance(); return v; }
	case NETCOMMANDTYPE_ACKSTAGE1: case NETCOMMANDTYPE_ACKSTAGE2: case NETCOMMANDTYPE_ACKBOTH: return p->isRoomForAckMessage(ref);
	case NETCOMMANDTYPE_FRAMEINFO: return p->isRoomForFrameMessage(ref);
	case NETCOMMANDTYPE_PLAYERLEAVE: return p->isRoomForPlayerLeaveMessage(ref);
	case NETCOMMANDTYPE_RUNAHEADMETRICS: return p->isRoomForRunAheadMetricsMessage(ref);
	case NETCOMMANDTYPE_RUNAHEAD: return p->isRoomForRunAheadMessage(ref);
	case NETCOMMANDTYPE_DESTROYPLAYER: return p->isRoomForDestroyPlayerMessage(ref);
	case NETCOMMANDTYPE_KEEPALIVE: return p->isRoomForKeepAliveMessage(ref);
	case NETCOMMANDTYPE_DISCONNECTKEEPALIVE: return p->isRoomForDisconnectKeepAliveMessage(ref);
	case NETCOMMANDTYPE_DISCONNECTPLAYER: return p->isRoomForDisconnectPlayerMessage(ref);
	case NETCOMMANDTYPE_PACKETROUTERQUERY: return p->isRoomForPacketRouterQueryMessage(ref);
	case NETCOMMANDTYPE_PACKETROUTERACK: return p->isRoomForPacketRouterAckMessage(ref);
	case NETCOMMANDTYPE_DISCONNECTCHAT: return p->isRoomForDisconnectChatMessage(ref);
	case NETCOMMANDTYPE_DISCONNECTVOTE: return p->isRoomForDisconnectVoteMessage(ref);
	case NETCOMMANDTYPE_CHAT: return p->isRoomForChatMessage(ref);
	case NETCOMMANDTYPE_PROGRESS: return p->isRoomForProgressMessage(ref);
	case NETCOMMANDTYPE_LOADCOMPLETE: return p->isRoomForLoadCompleteMessage(ref);
	case NETCOMMANDTYPE_TIMEOUTSTART: return p->isRoomForTimeOutGameStartMessage(ref);
	case NETCOMMANDTYPE_WRAPPER: return p->isRoomForWrapperMessage(ref);
	case NETCOMMANDTYPE_FILE: return p->isRoomForFileMessage(ref);
	case NETCOMMANDTYPE_FILEANNOUNCE: return p->isRoomForFileAnnounceMessage(ref);
	case NETCOMMANDTYPE_FILEPROGRESS: return p->isRoomForFileProgressMessage(ref);
	case NETCOMMANDTYPE_DISCONNECTFRAME: return p->isRoomForDisconnectFrameMessage(ref);
	case NETCOMMANDTYPE_DISCONNECTSCREENOFF: return p->isRoomForDisconnectScreenOffMessage(ref);
	case NETCOMMANDTYPE_FRAMERESENDREQUEST: return p->isRoomForFrameResendRequestMessage(ref);
	default: return 2;
	}
}

bool isAck(NetCommandType t) { return t == NETCOMMANDTYPE_ACKBOTH || t == NETCOMMANDTYPE_ACKSTAGE1 || t == NETCOMMANDTYPE_ACKSTAGE2; }

/* FillBufferWithCommand into a zeroed and into a 0xFF-filled buffer: the bytes written are those up to the last byte that
** differs from either fill (the buffer is 64 bytes longer than the size says, so an overrun shows in the extent). The
** bytes are those of the zeroed buffer. Acks are skipped (0xFFFFFFFF): FillBufferWithAckCommand casts the NetCommandRef to
** an ack message and reads garbage (the draft's header lists this). */
void emitFill(Words &o, NetCommandRef *ref, unsigned size)
{
	if (isAck(ref->getCommand()->getNetCommandType())) { emit(o, 0xFFFFFFFFu); return; }
	unsigned cap = size + 64;
	std::vector<unsigned char> a(cap, 0x00), b(cap, 0xFF);
	NetPacket::FillBufferWithCommand(&a[0], ref);
	NetPacket::FillBufferWithCommand(&b[0], ref);
	unsigned ext = 0;
	for (unsigned i = 0; i < cap; ++i) if (a[i] != 0x00 || b[i] != 0xFF) ext = i + 1;
	emitBytes(o, &a[0], ext);
}

NetCommandRef *makeRef(Rd &r)
{
	unsigned relay;
	NetCommandMsg *m = makeMsg(r, relay);
	NetCommandRef *ref = NEW_NETCOMMANDREF(m);
	ref->setRelay((UnsignedByte)relay);
	m->detach();
	return ref;
}

NetPacket *packetFrom(Rd &r)
{
	unsigned len = r.next();
	std::vector<unsigned char> b(len);
	for (unsigned i = 0; i < len; i += 4) { unsigned v = r.next(); for (unsigned j = 0; j < 4 && i + j < len; ++j) b[i + j] = (unsigned char)(v >> (8 * j)); }
	TransportMessage *tm = new TransportMessage;
	memset(tm, 0, sizeof *tm);
	if (len) memcpy(tm->data, &b[0], len > MAX_MESSAGE_LEN ? MAX_MESSAGE_LEN : len);
	tm->length = (Int)len;
	tm->addr = 0x7f000001; tm->port = 8088;
	NetPacket *p = newInstance(NetPacket)(tm);
	delete tm;
	return p;
}

/* op 1: pack the commands one after the other into packets, the way ConnectionManager does (a full packet is sent and a new one
** started; a command that fits no packet is wrapped when its type has a command id). The log, per command: the size
** GetBufferSizeNeededForCommand gives, the bytes FillBufferWithCommand writes, isRoomFor, addCommand's result and what became
** of the command; then every finished packet. in: [1, ncmds, specs...]. */
void op_pack(Rd &r, Words &o)
{
	unsigned n = r.next();
	NetPacket *cur = newInstance(NetPacket);
	for (unsigned c = 0; c < n; ++c) {
		NetCommandRef *ref = makeRef(r);
		NetCommandMsg *m = ref->getCommand();
		unsigned size = NetPacket::GetBufferSizeNeededForCommand(m);
		emit(o, size);
		emitFill(o, ref, size);
		emit(o, roomFor(cur, ref));
		bool ok = cur->addCommand(ref);
		emit(o, ok);
		if (!ok) {
			bool wrapIt = false;
			if (cur->getNumCommands() > 0) {
				emitPacket(o, cur);
				cur = newInstance(NetPacket);
				bool ok2 = cur->addCommand(ref);
				emit(o, ok2);
				wrapIt = !ok2;
			} else wrapIt = true;
			if (wrapIt) {
				if (DoesCommandRequireACommandID(m->getNetCommandType())) {
					NetPacketList l = NetPacket::ConstructBigCommandPacketList(ref);
					emit(o, 0xB1C0u); emit(o, (unsigned)l.size());
					for (NetPacketList::iterator it = l.begin(); it != l.end(); ++it) emitPacket(o, *it);
				} else emit(o, 0xDEADu);
			}
		}
		ref->deleteInstance();
	}
	emit(o, 0xE0D0u);
	if (cur->getNumCommands() > 0) emitPacket(o, cur);
	else emit(o, 0xFFFFFFFFu);
}

/* op 2: getCommandList of a packet of these bytes. in: [2, len, packed bytes]. */
void op_parse(Rd &r, Words &o)
{
	NetPacket *p = packetFrom(r);
	emit(o, (unsigned)p->getNumCommands()); emit(o, p->getAddr()); emit(o, p->getPort());
	NetCommandList *l = p->getCommandList();
	describeList(o, l);
	l->deleteInstance();
	p->deleteInstance();
}

/* op 3: ConstructNetCommandMsgFromRawData of these bytes. in: [3, len, packed bytes]. */
void op_raw(Rd &r, Words &o)
{
	unsigned len = r.next();
	std::vector<unsigned char> b(len + 8, 0);
	for (unsigned i = 0; i < len; i += 4) { unsigned v = r.next(); for (unsigned j = 0; j < 4 && i + j < len; ++j) b[i + j] = (unsigned char)(v >> (8 * j)); }
	NetCommandRef *ref = NetPacket::ConstructNetCommandMsgFromRawData(&b[0], (UnsignedShort)len);
	if (!ref) { emit(o, 0); return; }
	emit(o, 1);
	describeRef(o, ref);
	ref->deleteInstance();
}

/* op 4: wrap one command (ConstructBigCommandPacketList), read every wrapper back out of its packet, feed them to a
** NetCommandWrapperList in the order given, and ask for the percentage after each; then getReadyCommands.
** in: [4, nOrder, order..., spec]. (order entries are taken modulo the chunk count.) */
void op_wrap(Rd &r, Words &o)
{
	unsigned nOrder = r.next();
	std::vector<unsigned> order(nOrder);
	for (unsigned i = 0; i < nOrder; ++i) order[i] = r.next();
	NetCommandRef *ref = makeRef(r);
	unsigned wrappedID = ref->getCommand()->getID();
	NetPacketList l = NetPacket::ConstructBigCommandPacketList(ref);
	emit(o, (unsigned)l.size());
	std::vector<NetPacket *> pk(l.begin(), l.end());
	std::vector<NetCommandList *> lists;
	for (size_t i = 0; i < pk.size(); ++i) {
		emitPacket(o, pk[i]);
		TransportMessage *tm = new TransportMessage;
		memset(tm, 0, sizeof *tm);
		memcpy(tm->data, pk[i]->getData(), pk[i]->getLength());
		tm->length = pk[i]->getLength();
		NetPacket *q = newInstance(NetPacket)(tm);
		delete tm;
		lists.push_back(q->getCommandList());
		q->deleteInstance();
	}
	NetCommandWrapperList wl;
	emit(o, (unsigned)wl.getPercentComplete((UnsignedShort)wrappedID));
	for (size_t k = 0; k < order.size() && !pk.empty(); ++k) {
		size_t idx = order[k] % pk.size();
		NetCommandRef *wr = lists[idx]->getFirstMessage();
		if (!wr) { emit(o, 0xFFFFFFFFu); continue; }
		unsigned wid = ((NetWrapperCommandMsg *)wr->getCommand())->getWrappedCommandID();
		wl.processWrapper(wr);
		emit(o, (unsigned)wl.getPercentComplete((UnsignedShort)wid));
	}
	NetCommandList *rdy = wl.getReadyCommands();
	describeList(o, rdy);
	emit(o, (unsigned)wl.getPercentComplete((UnsignedShort)wrappedID));
	ref->deleteInstance();
}

/* op 5: encryptBuf (in[1] = 0) or decryptBuf (1) of these bytes, length in[2] (a multiple of anything: only whole words are
** touched). in: [5, which, len, packed bytes]. */
void op_crypt(Rd &r, Words &o)
{
	unsigned which = r.next();
	unsigned len = r.next();
	std::vector<unsigned char> b(len + 4, 0);
	for (unsigned i = 0; i < len; i += 4) { unsigned v = r.next(); for (unsigned j = 0; j < 4 && i + j < len; ++j) b[i + j] = (unsigned char)(v >> (8 * j)); }
	if (which == 0) encryptBuf(&b[0], (Int)len); else decryptBuf(&b[0], (Int)len);
	emitBytes(o, &b[0], len);
}

/* op 6: the NetworkUtil predicates over every type from -1 to NETCOMMANDTYPE_MAX + 1, then n command ids from
** GenerateNextCommandID. in: [6, n]. */
void op_util(Rd &r, Words &o)
{
	unsigned n = r.next();
	for (int t = -1; t <= (int)NETCOMMANDTYPE_MAX + 1; ++t) {
		NetCommandType ty = (NetCommandType)t;
		NetCommandMsg *m = newInstance(NetCommandMsg);
		m->setNetCommandType(ty);
		emit(o, DoesCommandRequireACommandID(ty)); emit(o, CommandRequiresAck(m)); emit(o, CommandRequiresDirectSend(m)); emit(o, IsCommandSynchronized(ty));
		AsciiString s = GetAsciiNetCommandType(ty);
		emitBytes(o, (const unsigned char *)s.str(), (unsigned)s.getLength());
		m->deleteInstance();
	}
	for (unsigned i = 0; i < n; ++i) emit(o, GenerateNextCommandID());
}


int posOf(NetCommandList *l, NetCommandRef *r)
{
	int i = 0;
	for (NetCommandRef *x = l->getFirstMessage(); x; x = x->getNext(), ++i) if (x == r) return i;
	return -1;
}

/* a list in short: the length, then per command [kind, player, frame, id, relay, sort number], then where m_last is */
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

/* op 7: operations on a NetCommandList. in: [7, nops, ops...], an op being a code and its words:
**   0 spec            addMessage (the relay of the spec set on the new ref): [added, position]
**   1 spec            findMessage(msg): position
**   2 id player       findMessage(id, player): position
**   3 position        removeMessage of the ref at that position (modulo the length): [removed]
**   4 spec spec       isEqualCommandMsg
**   5 n specs         appendList of a second list made of these (addMessage) to the list
**   6                 reset
**   7                 appendList(NULL)
** after each op the list in short (summary). */
void op_list(Rd &r, Words &o)
{
	NetCommandList *L = newInstance(NetCommandList);
	L->init();
	unsigned nops = r.next();
	for (unsigned k = 0; k < nops; ++k) {
		unsigned code = r.next();
		switch (code) {
		case 0: {
			unsigned relay;
			NetCommandMsg *m = makeMsg(r, relay);
			NetCommandRef *ref = L->addMessage(m);
			if (ref) ref->setRelay((UnsignedByte)relay);
			emit(o, ref != NULL); emit(o, (unsigned)(ref ? posOf(L, ref) : -1));
			break;
		}
		case 1: {
			unsigned relay;
			NetCommandMsg *m = makeMsg(r, relay);
			emit(o, (unsigned)posOf(L, L->findMessage(m)));
			break;
		}
		case 2: {
			unsigned id = r.next(), pl = r.next();
			emit(o, (unsigned)posOf(L, L->findMessage((UnsignedShort)id, (UnsignedByte)pl)));
			break;
		}
		case 3: {
			unsigned a = r.next();
			int len = L->length();
			if (len > 0) {
				NetCommandRef *x = L->getFirstMessage();
				for (unsigned i = 0; i < a % (unsigned)len; ++i) x = x->getNext();
				L->removeMessage(x);
				emit(o, 1);
			} else emit(o, 0);
			break;
		}
		case 4: {
			unsigned r1, r2;
			NetCommandMsg *a = makeMsg(r, r1);
			NetCommandMsg *b = makeMsg(r, r2);
			emit(o, L->isEqualCommandMsg(a, b));
			break;
		}
		case 5: {
			NetCommandList *L2 = newInstance(NetCommandList);
			L2->init();
			unsigned n = r.next();
			for (unsigned i = 0; i < n; ++i) {
				unsigned relay;
				NetCommandMsg *m = makeMsg(r, relay);
				NetCommandRef *ref = L2->addMessage(m);
				if (ref) ref->setRelay((UnsignedByte)relay);
			}
			L->appendList(L2);
			break;
		}
		case 6: L->reset(); break;
		case 7: L->appendList(NULL); break;
		default: break;
		}
		summary(o, L);
	}
}

/* op 8: operations on a FrameData. in: [8, nops, ops...]:
**   0 init   1 reset   2 a setFrame   3 a setFrameCommandCount   4 spec addCommand   5 flag allCommandsReady(flag)
**   6 zeroFrame   7 destroyGameMessages   8 update
** after each op: [frame, frameCommandCount, commandCount, whether it has a list, the list in short]. */
void op_fd(Rd &r, Words &o)
{
	FrameData *f = new FrameData;
	unsigned nops = r.next();
	for (unsigned k = 0; k < nops; ++k) {
		unsigned code = r.next();
		switch (code) {
		case 0: f->init(); break;
		case 1: f->reset(); break;
		case 2: f->setFrame(r.next()); break;
		case 3: f->setFrameCommandCount(r.next()); break;
		case 4: { unsigned relay; NetCommandMsg *m = makeMsg(r, relay); f->addCommand(m); break; }
		case 5: emit(o, (unsigned)f->allCommandsReady((r.next() & 1) != 0)); break;
		case 6: f->zeroFrame(); break;
		case 7: f->destroyGameMessages(); break;
		case 8: f->update(); break;
		default: break;
		}
		emit(o, f->getFrame()); emit(o, f->getFrameCommandCount()); emit(o, f->getCommandCount());
		NetCommandList *l = f->getCommandList();
		emit(o, l != NULL);
		if (l) summary(o, l);
	}
}

} // namespace

extern "C" int ref_np_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	Rd r = {in, nin, 0};
	Words o;
	unsigned op = r.next();
	switch (op) {
	case 1: op_pack(r, o); break;
	case 2: op_parse(r, o); break;
	case 3: op_raw(r, o); break;
	case 4: op_wrap(r, o); break;
	case 5: op_crypt(r, o); break;
	case 6: op_util(r, o); break;
	case 7: op_list(r, o); break;
	case 8: op_fd(r, o); break;
	default: break;
	}
	int n = (int)o.size();
	if (n <= cap) memcpy(out, o.data(), n * sizeof(unsigned));
	return n;
}
