/* Differential-test wrapper for the ping pool draft (#215, group net4, upstream commit 0a05454d), src/drafts/network/ping_thread.rs, cut verbatim by
** build.rs (drafts_net4) into OUT_DIR/draft_net4_gen:
**   ping_types.inc   Include/GameNetwork/GameSpy/PingThread.h, PingRequest, PingResponse, PingerInterface;
**   ping_pinger.inc  Source/GameNetwork/GameSpy/Thread/PingThread.cpp from `static const Int NumWorkerThreads` to before Thread_Function: class Pinger,
**                    createNewPingerInterface, ThePinger, class PingThreadClass, and every Pinger member (startThreads .. getPingString).
** Built on upstream's own UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; classes renamed by build.rs).
** Stand-ins (outside the cuts): `MutexClass` and `LockClass` (a lock that never fails: the test is single-threaded); `ThreadClass` (`Execute` sets a flag,
**   `Is_Running` reads it, the destructor stops); NEW is new. Not compared: `PingThreadClass::Thread_Function` and `doPing` (winsock name resolution and ICMP).
** Entry points (extern "C", tests/differential_draft_ping.rs): ref_ping_run(in, n, out, cap): a program of pool operations, see run(). */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <vector>
#include <queue>
#include <map>
#include <string>

namespace {
DynamicMemoryAllocator theAllocator;
}
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theAllocator;
CriticalSection *TheUnicodeStringCriticalSection = 0;

#define NULL 0
#define NEW new

class MutexClass
{
public:
	class LockClass
	{
	public:
		LockClass(MutexClass &, int = -1) {}
		bool Failed() { return false; }
	};
};
class ThreadClass
{
public:
	ThreadClass() : running(false) {}
	virtual ~ThreadClass() {}
	void Execute() { running = true; }
	bool Is_Running() { return running; }
	bool running;
};

#include "ping_types.inc"
#include "ping_pinger.inc"

typedef std::vector<unsigned> Words;
static void putStr(Words &w, const std::string &s)
{
	w.push_back((unsigned)s.size());
	for (size_t i = 0; i < s.size(); ++i) w.push_back((unsigned char)s[i]);
}
static std::string getStr(const unsigned *&p)
{
	unsigned n = *p++;
	std::string s;
	for (unsigned i = 0; i < n; ++i) s.push_back((char)*p++);
	return s;
}

/* A program: [number of operations] then per operation its opcode and arguments (strings as length + bytes). The output is, per operation, the marker
** 0xA5A5A5A5, the opcode and the result (if any). */
extern "C" int ref_ping_run(const unsigned *in, int n, unsigned *out, int cap)
{
	(void)n;
	const unsigned *p = in;
	Pinger pinger;
	Words w;
	unsigned ops = *p++;
	for (unsigned o = 0; o < ops; ++o)
	{
		unsigned op = *p++;
		w.push_back(0xA5A5A5A5u);
		w.push_back(op);
		switch (op)
		{
		case 0: pinger.startThreads(); break;
		case 1: pinger.endThreads(); break;
		case 2: w.push_back(pinger.areThreadsRunning() ? 1u : 0u); break;
		case 3: { PingRequest r; r.hostname = getStr(p); r.repetitions = (Int)*p++; r.timeout = (Int)*p++; pinger.addRequest(r); break; }
		case 4: { PingRequest r; Bool ok = pinger.getRequest(r); w.push_back(ok ? 1u : 0u); if (ok) { putStr(w, r.hostname); w.push_back((unsigned)r.repetitions); w.push_back((unsigned)r.timeout); } break; }
		case 5: { PingResponse r; r.hostname = getStr(p); r.avgPing = (Int)*p++; r.repetitions = (Int)*p++; pinger.addResponse(r); break; }
		case 6: { PingResponse r; Bool ok = pinger.getResponse(r); w.push_back(ok ? 1u : 0u); if (ok) { putStr(w, r.hostname); w.push_back((unsigned)r.avgPing); w.push_back((unsigned)r.repetitions); } break; }
		case 7: w.push_back(pinger.arePingsInProgress() ? 1u : 0u); break;
		case 8: { AsciiString h; std::string s = getStr(p); h.set(s.c_str()); w.push_back((unsigned)pinger.getPing(h)); break; }
		case 9: pinger.clearPingMap(); break;
		case 10: { Int t = (Int)*p++; AsciiString s = pinger.getPingString(t); putStr(w, std::string(s.str())); break; }
		}
	}
	if ((int)w.size() > cap) return -1;
	for (size_t i = 0; i < w.size(); ++i) out[i] = w[i];
	return (int)w.size();
}
