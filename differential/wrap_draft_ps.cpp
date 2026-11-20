/* Differential-test wrapper for the GameSpy persistent-storage draft (#215, group net4, upstream commit 0a05454d),
** src/drafts/network/persistent_storage_thread.rs, cut verbatim by build.rs (drafts_net4) into OUT_DIR/draft_net4_gen:
**   ps_types.inc  Include/GameNetwork/GameSpy/PersistentStorageThread.h from `typedef std::map<Int, UnsignedInt> PerGeneralMap;` to the include guard's #endif
**                 (PSPlayerStats, PSRequest, PSResponse, GameSpyPSMessageQueueInterface);
**   ps_req.inc    PersistentStorageThread.cpp, PSRequest::PSRequest;
**   ps_inc.inc    PersistentStorageThread.cpp, INCORPORATE_MAP, PSPlayerStats::incorporate, the copy constructor;
**   ps_qdecl.inc  the queue typedefs, class GameSpyPSMessageQueue, createNewMessageQueue, TheGameSpyPSMessageQueue;
**   ps_qmem.inc   the queue's members (constructor, destructor, start/endThread, isThreadRunning, add/get request/response, getThread,
**                 trackPlayerStats, findPlayerStatsByID);
**   ps_kv.inc     PSPlayerStats::PSPlayerStats, reset, parsePlayerKVPairs, formatPlayerKVPairs (to the end of the file).
** Stand-ins (outside the cuts): the BaseType typedefs (Int, UnsignedInt, Real, Bool); `MutexClass` and its `LockClass` (a lock that never fails: the
**   draft's queue is single-threaded in the test); `PSThreadClass` (the SDK thread: `Execute` sets a flag, `Is_Running` reads it);
**   `ThePlayerTemplateStore` (shim_draft_net4/Common/PlayerTemplate.h: only `getPlayerTemplateCount`, set by the caller); NEW is new; DEBUG_LOG is
**   nothing (it only logs); `_snprintf` is snprintf (both write at most n bytes; the cut terminates the buffer itself).
** The copy constructor leaves `QMwinsInARow` and `maxQMwinsInARow` uninitialised (a defect of upstream, reported on #212); every dump of a copied record
**   writes 0 for that pair, on both sides, so the uninitialised bytes are never compared.
** Entry points (extern "C", tests/differential_draft_ps.rs): ref_ps_parse, ref_ps_format, ref_ps_run (a program of queue and record operations). */
#include <string>
#include <map>
#include <queue>
#include <vector>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

typedef int Int;
typedef unsigned int UnsignedInt;
typedef float Real;
typedef bool Bool;
#define TRUE true
#define FALSE false
#define NEW new
#define DEBUG_LOG(x) ((void)0)
#define _snprintf snprintf
/* Renamed (#215, merge of net4): unrenamed, this global collides at link time with wrap_playertemplate.cpp's `ThePlayerTemplateStore`
** in differential_player_template (rust-lld: duplicate symbol). The other wrappers rename it the same way (wrap_game_info.cpp, wrap_draft_ladder.cpp). */
#define ThePlayerTemplateStore N4ThePlayerTemplateStore

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
class PSThreadClass
{
public:
	PSThreadClass() : running(false) {}
	void Execute() { running = true; }
	bool Is_Running() { return running; }
	bool running;
};

#include "Common/PlayerTemplate.h"
Net4PlayerTemplateStore thePlayerTemplateStore = { 0 };
Net4PlayerTemplateStore *ThePlayerTemplateStore = &thePlayerTemplateStore;

#include "ps_types.inc"
#include "ps_req.inc"
#include "ps_inc.inc"
#include "ps_qdecl.inc"
#include "ps_qmem.inc"
#include "ps_kv.inc"

/* ---- the stream: the record as 32-bit words ---- */
typedef std::vector<unsigned> Words;

static Int PSPlayerStats::* const INTS[21] = {
	&PSPlayerStats::id, &PSPlayerStats::locale, &PSPlayerStats::gamesAsRandom, &PSPlayerStats::lastGeneral, &PSPlayerStats::gamesInRowWithLastGeneral,
	&PSPlayerStats::challengeMedals, &PSPlayerStats::battleHonors, &PSPlayerStats::QMwinsInARow, &PSPlayerStats::maxQMwinsInARow,
	&PSPlayerStats::winsInARow, &PSPlayerStats::maxWinsInARow, &PSPlayerStats::lossesInARow, &PSPlayerStats::maxLossesInARow,
	&PSPlayerStats::disconsInARow, &PSPlayerStats::maxDisconsInARow, &PSPlayerStats::desyncsInARow, &PSPlayerStats::maxDesyncsInARow,
	&PSPlayerStats::builtParticleCannon, &PSPlayerStats::builtNuke, &PSPlayerStats::builtSCUD, &PSPlayerStats::lastLadderPort };
static PerGeneralMap PSPlayerStats::* const MAPS[24] = {
	&PSPlayerStats::wins, &PSPlayerStats::losses, &PSPlayerStats::games, &PSPlayerStats::duration, &PSPlayerStats::unitsKilled,
	&PSPlayerStats::unitsLost, &PSPlayerStats::unitsBuilt, &PSPlayerStats::buildingsKilled, &PSPlayerStats::buildingsLost,
	&PSPlayerStats::buildingsBuilt, &PSPlayerStats::earnings, &PSPlayerStats::techCaptured, &PSPlayerStats::discons, &PSPlayerStats::desyncs,
	&PSPlayerStats::surrenders, &PSPlayerStats::gamesOf2p, &PSPlayerStats::gamesOf3p, &PSPlayerStats::gamesOf4p, &PSPlayerStats::gamesOf5p,
	&PSPlayerStats::gamesOf6p, &PSPlayerStats::gamesOf7p, &PSPlayerStats::gamesOf8p, &PSPlayerStats::customGames, &PSPlayerStats::QMGames };

static void putStr(Words &w, const std::string &s)
{
	w.push_back((unsigned)s.size());
	for (size_t i = 0; i < s.size(); ++i) w.push_back((unsigned char)s[i]);
}
static void dumpStats(Words &w, const PSPlayerStats &s, bool zeroQM)
{
	for (int i = 0; i < 21; ++i)
	{
		bool qm = (i == 7 || i == 8);
		w.push_back(zeroQM && qm ? 0u : (unsigned)(s.*INTS[i]));
	}
	unsigned f;
	memcpy(&f, &s.lastFPS, 4);
	w.push_back(f);
	putStr(w, s.options);
	putStr(w, s.systemSpec);
	putStr(w, s.lastLadderHost);
	for (int m = 0; m < 24; ++m)
	{
		const PerGeneralMap &pm = s.*MAPS[m];
		w.push_back((unsigned)pm.size());
		for (PerGeneralMap::const_iterator it = pm.begin(); it != pm.end(); ++it) { w.push_back((unsigned)it->first); w.push_back(it->second); }
	}
}
static std::string getStr(const unsigned *&p)
{
	unsigned n = *p++;
	std::string s;
	for (unsigned i = 0; i < n; ++i) s.push_back((char)*p++);
	return s;
}
static PSPlayerStats readStats(const unsigned *&p)
{
	PSPlayerStats s;
	for (int i = 0; i < 21; ++i) s.*INTS[i] = (Int)*p++;
	unsigned f = *p++;
	memcpy(&s.lastFPS, &f, 4);
	s.options = getStr(p);
	s.systemSpec = getStr(p);
	s.lastLadderHost = getStr(p);
	for (int m = 0; m < 24; ++m)
	{
		unsigned n = *p++;
		for (unsigned i = 0; i < n; ++i) { Int k = (Int)*p++; UnsignedInt v = *p++; (s.*MAPS[m])[k] = v; }
	}
	return s;
}
static int emit(const Words &w, unsigned *out, int cap)
{
	if ((int)w.size() > cap) return -1;
	for (size_t i = 0; i < w.size(); ++i) out[i] = w[i];
	return (int)w.size();
}

extern "C" int ref_ps_parse(const char *kv, int len, unsigned *out, int cap)
{
	PSPlayerStats s = GameSpyPSMessageQueueInterface::parsePlayerKVPairs(std::string(kv, (size_t)len));
	Words w;
	dumpStats(w, s, false);
	return emit(w, out, cap);
}

extern "C" int ref_ps_format(const unsigned *in, int templateCount, char *out, int cap)
{
	ThePlayerTemplateStore->count = templateCount;
	const unsigned *p = in;
	PSPlayerStats s = readStats(p);
	std::string r = GameSpyPSMessageQueueInterface::formatPlayerKVPairs(s);
	if ((int)r.size() > cap) return -1;
	memcpy(out, r.data(), r.size());
	return (int)r.size();
}

static void dumpReq(Words &w, const PSRequest &r)
{
	w.push_back((unsigned)r.requestType);
	dumpStats(w, r.player, true);
	putStr(w, r.cdkey); putStr(w, r.nick); putStr(w, r.password); putStr(w, r.email); putStr(w, r.results);
	w.push_back(r.addDiscon ? 1u : 0u); w.push_back(r.addDesync ? 1u : 0u); w.push_back((unsigned)r.lastHouse);
}

extern "C" int ref_ps_run(const unsigned *in, int n, unsigned *out, int cap)
{
	(void)n;
	const unsigned *p = in;
	GameSpyPSMessageQueue q;
	Words w;
	unsigned ops = *p++;
	for (unsigned o = 0; o < ops; ++o)
	{
		unsigned op = *p++;
		w.push_back(0xA5A5A5A5u);
		w.push_back(op);
		switch (op)
		{
		case 0: { PSPlayerStats a = readStats(p); PSPlayerStats b = readStats(p); a.incorporate(b); dumpStats(w, a, false); break; }
		case 1: { PSPlayerStats a = readStats(p); PSPlayerStats c(a); dumpStats(w, c, true); break; }
		case 2: { PSPlayerStats a = readStats(p); q.trackPlayerStats(a); break; }
		case 3: { Int id = (Int)*p++; PSPlayerStats r = q.findPlayerStatsByID(id); dumpStats(w, r, true); break; }
		case 4: { PSRequest r; r.requestType = (decltype(r.requestType))*p++; r.player = readStats(p); r.cdkey = getStr(p); r.nick = getStr(p); r.password = getStr(p); r.email = getStr(p); r.results = getStr(p); r.addDiscon = *p++ != 0; r.addDesync = *p++ != 0; r.lastHouse = (Int)*p++; q.addRequest(r); break; }
		case 5: { PSRequest r; Bool ok = q.getRequest(r); w.push_back(ok ? 1u : 0u); if (ok) dumpReq(w, r); break; }
		case 6: { PSResponse r; r.responseType = (decltype(r.responseType))*p++; r.player = readStats(p); r.preorder = *p++ != 0; q.addResponse(r); break; }
		case 7: { PSResponse r; r.responseType = PSResponse::PSRESPONSE_PLAYERSTATS; r.preorder = false; Bool ok = q.getResponse(r); w.push_back(ok ? 1u : 0u); if (ok) { w.push_back((unsigned)r.responseType); dumpStats(w, r.player, true); w.push_back(r.preorder ? 1u : 0u); } break; }
		case 8: q.startThread(); break;
		case 9: q.endThread(); break;
		case 10: w.push_back(q.isThreadRunning() ? 1u : 0u); break;
		case 11: q.setLocalPlayerID((Int)*p++); break;
		case 12: w.push_back((unsigned)q.getLocalPlayerID()); break;
		case 13: { std::string e = getStr(p), ni = getStr(p), pw = getStr(p); q.setEmail(e); q.setNick(ni); q.setPassword(pw); break; }
		case 14: putStr(w, q.getEmail()); putStr(w, q.getNick()); putStr(w, q.getPassword()); break;
		case 15: { PSRequest r; dumpReq(w, r); break; }
		case 16: { PSPlayerStats s; dumpStats(w, s, false); break; }
		}
	}
	return emit(w, out, cap);
}
