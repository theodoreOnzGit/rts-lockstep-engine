/* Differential-test wrapper for the game-results queue draft (#215, group net4, upstream commit 0a05454d), src/drafts/network/game_results_thread.rs, cut
** verbatim by build.rs (drafts_net4) into OUT_DIR/draft_net4_gen:
**   gres_types.inc   Include/GameNetwork/GameSpy/GameResultsThread.h, GameResultsRequest, GameResultsResponse, GameResultsInterface;
**   gres_queue.inc   Source/GameNetwork/GameSpy/Thread/GameResultsThread.cpp from `static const Int NumWorkerThreads` to before Thread_Function: class
**                    GameResultsQueue (its constructor starts the threads), createNewGameResultsInterface, class GameResultsThreadClass, every queue member.
** Stand-ins (outside the cuts): the BaseType typedefs; `MutexClass` and `LockClass` (a lock that never fails); `ThreadClass` (`Execute` sets a flag, `Is_Running`
**   reads it); `SubsystemInterface` (the three pure virtuals the queue overrides); NEW is new. Not compared: `Thread_Function` and `sendGameResults` (winsock).
** Entry point (extern "C", tests/differential_draft_gres.rs): ref_gres_run(in, n, out, cap), a program of queue operations, see run(). */
#include <string.h>
#include <vector>
#include <queue>
#include <string>

typedef int Int;
typedef unsigned int UnsignedInt;
typedef unsigned short UnsignedShort;
typedef bool Bool;
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
class SubsystemInterface
{
public:
	virtual ~SubsystemInterface() {}
	virtual void init() = 0;
	virtual void reset() = 0;
	virtual void update() = 0;
};

#include "gres_types.inc"
#include "gres_queue.inc"

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

extern "C" int ref_gres_run(const unsigned *in, int n, unsigned *out, int cap)
{
	(void)n;
	const unsigned *p = in;
	GameResultsQueue q;
	Words w;
	w.push_back(q.areThreadsRunning() ? 1u : 0u);
	unsigned ops = *p++;
	for (unsigned o = 0; o < ops; ++o)
	{
		unsigned op = *p++;
		w.push_back(0xA5A5A5A5u);
		w.push_back(op);
		switch (op)
		{
		case 0: q.startThreads(); break;
		case 1: q.endThreads(); break;
		case 2: w.push_back(q.areThreadsRunning() ? 1u : 0u); break;
		case 3: { GameResultsRequest r; r.hostname = getStr(p); r.port = (UnsignedShort)*p++; r.results = getStr(p); q.addRequest(r); break; }
		case 4: { GameResultsRequest r; Bool ok = q.getRequest(r); w.push_back(ok ? 1u : 0u); if (ok) { putStr(w, r.hostname); w.push_back(r.port); putStr(w, r.results); } break; }
		case 5: { GameResultsResponse r; r.hostname = getStr(p); r.port = (UnsignedShort)*p++; r.sentOk = *p++ != 0; q.addResponse(r); break; }
		case 6: { GameResultsResponse r; Bool ok = q.getResponse(r); w.push_back(ok ? 1u : 0u); if (ok) { putStr(w, r.hostname); w.push_back(r.port); w.push_back(r.sentOk ? 1u : 0u); } break; }
		case 7: w.push_back(q.areGameResultsBeingSent() ? 1u : 0u); break;
		}
	}
	if ((int)w.size() > cap) return -1;
	for (size_t i = 0; i < w.size(); ++i) out[i] = w[i];
	return (int)w.size();
}
