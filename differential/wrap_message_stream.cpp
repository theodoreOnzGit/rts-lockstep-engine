/* Differential-test wrapper for MessageStream (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/MessageStream.cpp
**   GeneralsMD/Code/GameEngine/Include/Common/MessageStream.h
** (the header unchanged, the .cpp in build.rs's copy with one for-scope edit, #included below), on the stand-ins of differential/shim_msg (the local player's index; the
** headers only the debug-build isInvalidDebugCommand uses, empty). The pool glue is the shared shim's zero-filling new.
**
** Entry points (extern "C", tests/differential_message_stream.rs):
**   ref_msgs_run(ops, n, out, cap)  runs n operations of five words each and writes the log (returns its length)
**   ref_msgs_type_value(i)          the value of the i-th name of differential/msg_types.inc, -1 past the end
**   ref_msgs_type_name(t, out, cap) GameMessage::getCommandTypeAsAsciiString(t)
** The operations and the log are described at run_op below and mirrored in the test. */
/* The names of this file are renamed apart from the stand-ins of other differential builds that link into the same
** binary (wrap_engine.cpp defines TheMessageStream, several builds a PlayerList); the upstream text sees them only
** through these macros. */
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
#define Player MsTestPlayer
#define PlayerList MsTestPlayerList
#define ThePlayerList MsTestThePlayerList

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include <map>
#include <unistd.h>
#define PI 3.14159265359f   /* GameCommon.h uses it; the real PreRTS.h brings it from WWMath */
#include "PreRTS.h"
/* the dump reads m_lastMessage */
#define protected public
#include "Common/MessageStream.h"
#undef protected
/* build.rs's copy (OUT_DIR/msg_gen): the one for-scope edit described there */
#include "MessageStream.cpp"

static PlayerList g_playerList;
PlayerList *ThePlayerList = &g_playerList;

namespace {

std::string g_log;
void logf(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
void logf(const char *fmt, ...)
{
	char buf[256];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	g_log += buf;
	g_log += '\n';
}

std::vector<GameMessage *> g_handles;   /* every message made, by handle; a freed one stays (the test never uses it) */
GameMessageList *g_extra = NULL;

int handleOf(GameMessage *m)
{
	if (!m) return -1;
	for (int i = (int)g_handles.size() - 1; i >= 0; --i)   /* the newest handle: an address the allocator reused */
		if (g_handles[i] == m) return i;
	return -2;
}

GameMessageList *listOf(unsigned l)
{
	switch (l % 3) {
	case 0: return TheMessageStream;
	case 1: return TheCommandList;
	default: return g_extra;
	}
}

/* The test's translator: logs what it sees; destroys when (type + k) % 3 == 0; when (type * 7 + k) % 5 == 0 and fewer
** than g_budget messages were appended in this propagation, appends a message of type (type + k + 1) % 2010 to the
** stream (a handle). */
int g_budget = 0;
class TestTranslator : public GameMessageTranslator
{
public:
	int m_k;
	TestTranslator(int k) : m_k(k) {}
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg)
	{
		int t = (int)msg->getType();
		logf("xlat %d sees %d type %d", m_k, handleOf(const_cast<GameMessage *>(msg)), t);
		if (((t * 7 + m_k) % 5 + 5) % 5 == 0 && g_budget > 0) {
			--g_budget;
			GameMessage *m = TheMessageStream->appendMessage((GameMessage::Type)(((t + m_k + 1) % 2010 + 2010) % 2010));
			g_handles.push_back(m);
			logf("xlat %d appends %d", m_k, (int)g_handles.size() - 1);
		}
		return (((t + m_k) % 3 + 3) % 3 == 0) ? DESTROY_MESSAGE : KEEP_MESSAGE;
	}
};

void dumpList(unsigned l)
{
	GameMessageList *list = listOf(l);
	std::string s;
	char b[32];
	for (GameMessage *m = list->getFirstMessage(); m; m = m->next()) {
		snprintf(b, sizeof b, " %d", handleOf(m));
		s += b;
	}
	logf("list %u:%s last %d", l % 3, s.c_str(), handleOf(list->m_lastMessage));
}

/* One operation: w[0] the code, w[1..4] its words. */
void run_op(const unsigned *w)
{
	unsigned a = w[1], b = w[2], c = w[3], d = w[4];
	switch (w[0]) {
	case 0: {   /* new message (not on a list) */
		GameMessage *m = newInstance(GameMessage)((GameMessage::Type)(int)a);
		g_handles.push_back(m);
		logf("new %d player %d", (int)g_handles.size() - 1, m->getPlayerIndex());
		break;
	}
	case 1: g_handles[a]->deleteInstance(); logf("delete %u", a); break;
	case 2: listOf(b)->appendMessage(g_handles[a]); logf("append %u to %u", a, b % 3); break;
	case 3: listOf(b)->insertMessage(g_handles[a], g_handles[c]); logf("insert %u after %u on %u", a, c, b % 3); break;
	case 4: listOf(b)->removeMessage(g_handles[a]); logf("remove %u from %u", a, b % 3); break;
	case 5: logf("contains %u %d: %d", b % 3, (int)a, (int)listOf(b)->containsMessageOfType((GameMessage::Type)(int)a)); break;
	case 6: {   /* append arguments: kind b % 11, words c, d, a few derived; repeated (d >> 24) + 1 times */
		GameMessage *m = g_handles[a];
		unsigned reps = (d >> 24) + 1;
		for (unsigned r = 0; r < reps; ++r) {
			unsigned x = c + r, y = d ^ (r * 0x9e3779b9u), z = c * 31u + d, q = d * 17u + r;
			Real rx, ry, rz;
			memcpy(&rx, &x, 4); memcpy(&ry, &y, 4); memcpy(&rz, &z, 4);
			switch (b % 11) {
			case 0: m->appendIntegerArgument((Int)x); break;
			case 1: m->appendRealArgument(rx); break;
			case 2: m->appendBooleanArgument((x & 1) != 0); break;
			case 3: m->appendObjectIDArgument((ObjectID)x); break;
			case 4: m->appendDrawableIDArgument((DrawableID)x); break;
			case 5: m->appendTeamIDArgument(x); break;
			case 6: { Coord3D p; p.x = rx; p.y = ry; p.z = rz; m->appendLocationArgument(p); break; }
			case 7: { ICoord2D p; p.x = (Int)x; p.y = (Int)y; m->appendPixelArgument(p); break; }
			case 8: { IRegion2D p; p.lo.x = (Int)x; p.lo.y = (Int)y; p.hi.x = (Int)z; p.hi.y = (Int)q; m->appendPixelRegionArgument(p); break; }
			case 9: m->appendTimestampArgument(x); break;
			default: { WideChar ch = (WideChar)x; m->appendWideCharArgument(ch); break; }
			}
		}
		logf("args %u kind %u x%u count %u", a, b % 11, reps, (unsigned)m->getArgumentCount());
		break;
	}
	case 7: {
		const GameMessageArgumentType *t = g_handles[a]->getArgument((Int)b);
		unsigned words[4];
		memcpy(words, t, 16);
		logf("arg %u %d: %08x %08x %08x %08x", a, (int)b, words[0], words[1], words[2], words[3]);
		break;
	}
	case 8: logf("argtype %u %d: %d", a, (int)b, (int)g_handles[a]->getArgumentDataType((Int)b)); break;
	case 9: {
		GameMessage *m = g_handles[a];
		logf("msg %u type %d player %d count %u next %d prev %d", a, (int)m->getType(), m->getPlayerIndex(),
			(unsigned)m->getArgumentCount(), handleOf(m->next()), handleOf(m->prev()));
		break;
	}
	case 10: logf("name %d %s", (int)a, GameMessage::getCommandTypeAsAsciiString((GameMessage::Type)(int)a).str()); break;
	case 11: {
		GameMessage *m = TheMessageStream->appendMessage((GameMessage::Type)(int)a);
		g_handles.push_back(m);
		logf("stream append %d", (int)g_handles.size() - 1);
		break;
	}
	case 12: {
		GameMessage *m = TheMessageStream->insertMessage((GameMessage::Type)(int)a, g_handles[c]);
		g_handles.push_back(m);
		logf("stream insert %d after %u", (int)g_handles.size() - 1, c);
		break;
	}
	case 13: TheCommandList->reset(); logf("command reset"); break;
	case 14: TheCommandList->appendMessageList(g_handles[a]); logf("command appendlist %u", a); break;
	case 15: dumpList(a); break;
	case 16: {
		TranslatorID id = TheMessageStream->attachTranslator((b & 1) ? NULL : new TestTranslator((int)(c % 97)), a);
		logf("attach %u", (unsigned)id);
		break;
	}
	case 17: {
		TestTranslator *t = static_cast<TestTranslator *>(TheMessageStream->findTranslator(a));
		logf("find %u: %d", a, t ? t->m_k : -1);
		break;
	}
	case 18: TheMessageStream->removeTranslator(a); logf("detach %u", a); break;
	case 19: g_budget = (int)(a % 4); TheMessageStream->propagateMessages(); logf("propagated"); break;
	case 20: {
		ICoord2D an, de;
		IRegion2D r;
		an.x = (Int)a; an.y = (Int)b; de.x = (Int)c; de.y = (Int)d;
		buildRegion(&an, &de, &r);
		logf("region %d %d %d %d", r.lo.x, r.lo.y, r.hi.x, r.hi.y);
		break;
	}
	case 21: g_handles[a]->friend_setPlayerIndex((Int)b); logf("setplayer %u %d", a, (int)b); break;
	case 22: ThePlayerList->getLocalPlayer()->m_index = (Int)a; logf("local %d", (int)a); break;
	default: logf("op?"); break;
	}
}

} // namespace

extern "C" int ref_msgs_run(const unsigned *ops, int n, char *out, int cap)
{
	g_log.clear();
	g_handles.clear();
	g_playerList.m_local.m_index = 0;
	TheMessageStream = new MessageStream;
	TheCommandList = new CommandList;
	g_extra = new GameMessageList;
	TheMessageStream->init();
	TheCommandList->init();
	/* a program the Rust side called defined never loops upstream; with a planted bug the two states can differ, and an
	** endless walk here then ends the process (SIGALRM) instead of the test run hanging */
	alarm(20);
	for (int i = 0; i < n; ++i)
		run_op(ops + 5 * i);
	alarm(0);
	/* The lists are leaked, not destroyed: a destructor would delete messages a later list may still link (the test
	** keeps such states legal up to their use). */
	TheMessageStream = NULL;
	TheCommandList = NULL;
	g_extra = NULL;
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}

static const int g_typeValues[] = {
#define MT(x) (int)GameMessage::x,
#include "msg_types.inc"
#undef MT
};

extern "C" int ref_msgs_type_value(int i)
{
	if (i < 0 || i >= (int)(sizeof g_typeValues / sizeof g_typeValues[0])) return -1;
	return g_typeValues[i];
}

extern "C" int ref_msgs_type_name(int t, char *out, int cap)
{
	AsciiString s = GameMessage::getCommandTypeAsAsciiString((GameMessage::Type)t);
	int len = (int)s.getLength();
	if (len <= cap) memcpy(out, s.str(), len);
	return len;
}
