/* Differential-test wrapper for StateMachine (upstream commit 0a05454d), p1w3-statemachine:
**   GeneralsMD/Code/GameEngine/Source/Common/StateMachine.cpp   (compiled unchanged, own TU)
** with the header StateMachine.h unchanged (so are the Xfer, XferCRC, XferDeepCRC,
** XferSave and XferLoad headers and the Xfer family of wrap_xfer.cpp, which this
** build links against: the REAL Xfer classes drive the machine, nothing is
** simulated). The engine headers StateMachine.cpp includes that need the rest of
** the engine are replaced by differential/shim_statemachine: GameLogic.h (the
** frame and findObjectByID), Object.h (getID, getPosition), ThingTemplate.h and
** GlobalData.h (debug logging only). The classes are renamed Sm* by the prelude
** (statemachine_prelude.h) so that no symbol here clashes with another fixture's.
**
** ref_sm_run interprets a script (the encoding of tests/differential_state_machine.rs,
** which documents it): a table of condition sequences, a table of objects, a state
** graph, and a list of operations on the machine. It logs every callback the machine
** makes (enter, update, exit, condition, with the machine's own re-entrant actions),
** and after every operation the status (what was thrown, if anything), the result,
** and the machine's whole view: current state, sleep frame, default state, goal,
** lock, flags, the four state queries, and every state's counter. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include "Common/Errors.h"
#include "Common/StateMachine.h"
#include "Common/XferCRC.h"
#include "Common/XferDeepCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/GameState.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"

/* ---- stand-ins (all behind the Sm* names) -------------------------------- */
GameLogic *TheGameLogic = 0;


namespace {

static UnsignedInt g_frame = 0;
static std::vector<Object> g_objects;       /* storage, never shrinks while a run is live */
static std::set<ObjectID> g_live;           /* the registry findObjectByID reads */

/* The log has four categories, compared separately by the test:
**   0 EV  the machine's callbacks and conditions and the re-entrant actions (every transition)
**   1 RES the operation markers, statuses, return codes and the machine's view after each
**   2 BYT the bytes of the saves and of the deep CRC
**   3 CRC the CRC values
** ref_sm_run writes them out one after the other, each as its length then its words. */
enum { C_EV = 0, C_RES = 1, C_BYT = 2, C_CRC = 3 };
struct Log
{
	std::vector<unsigned> v[4];
	void put(int c, unsigned x) { v[c].push_back(x); }
	void putReal(int c, Real r) { unsigned u; memcpy(&u, &r, 4); put(c, u); }
};
static Log *g_log = 0;

struct Script
{
	const unsigned *w; int n; int i; bool bad;
	unsigned next() { if (i < n) return w[i++]; bad = true; return 0; }
};

/* callback log tags */
enum { T_ENTER_BEGIN = 10, T_ENTER_END = 11, T_UPDATE_BEGIN = 12, T_UPDATE_END = 13, T_EXIT_BEGIN = 14, T_EXIT_END = 15,
       T_COND = 16, T_ACTION = 17 };

struct Entry { int ret; unsigned act; unsigned arg; };

/* The condition tables: sequence k answers true/false cyclically per call. */
static std::vector<std::vector<unsigned> > g_conds;
static std::vector<unsigned> g_condCalls;

static int g_actDepth = 0;       /* re-entrant actions nest at most this deep */
static bool g_noActions = false; /* the destructor's onExit must not throw or re-enter */
static const int MAX_ACT_DEPTH = 2;

struct ActGuard { ActGuard() { ++g_actDepth; } ~ActGuard() { --g_actDepth; } };

static Bool condTest(State *s, void *ud)
{
	size_t idx = (size_t)(uintptr_t)ud;
	const std::vector<unsigned> &seq = g_conds[idx];
	bool r = seq.empty() ? false : seq[g_condCalls[idx]++ % seq.size()] != 0;
	g_log->put(C_EV, T_COND); g_log->put(C_EV, s->getID()); g_log->put(C_EV, (unsigned)idx); g_log->put(C_EV, r ? 1u : 0u);
	return r;
}

static void doAction(State *st, const Entry &e)
{
	if (g_noActions || g_actDepth >= MAX_ACT_DEPTH || e.act == 0) return;
	ActGuard guard;
	g_log->put(C_EV, T_ACTION); g_log->put(C_EV, st->getID()); g_log->put(C_EV, e.act); g_log->put(C_EV, e.arg);
	StateMachine *m = st->getMachine();
	switch (e.act) {
	case 1: m->setState(e.arg); break;
	case 2: m->internalSetState(e.arg); break;
	case 3: m->clear(); break;
	case 4: m->resetToDefaultState(); break;
	case 5: m->halt(); break;
	case 6: m->lock("script"); break;
	case 7: m->unlock(); break;
	case 8: m->setGoalObject(e.arg < g_objects.size() ? &g_objects[e.arg] : (const Object *)0); break;
	case 9: { Coord3D p; p.x = (Real)e.arg; p.y = (Real)e.arg * 2.0f; p.z = -(Real)e.arg; m->setGoalPosition(&p); break; }
	default: break;
	}
}

/* The scripted state: enter/update/exit results and re-entrant actions come
** from per-callback lists used cyclically; a counter counts every callback and
** is what the state xfers. */
class ScriptState : public State
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(ScriptState, "ScriptState")
public:
	ScriptState(StateMachine *machine) : State(machine, "ScriptState"), m_flags(0), m_nEnter(0), m_nUpdate(0), m_nExit(0), m_counter(0) { }
	unsigned m_flags;
	std::vector<Entry> m_enter, m_update, m_exit;
	unsigned m_nEnter, m_nUpdate, m_nExit;
	UnsignedInt m_counter;

	virtual StateReturnType onEnter()
	{
		g_log->put(C_EV, T_ENTER_BEGIN); g_log->put(C_EV, getID());
		Entry e = { 0, 0, 0 };
		if (!m_enter.empty()) e = m_enter[m_nEnter++ % m_enter.size()];
		++m_counter;
		doAction(this, e);
		g_log->put(C_EV, T_ENTER_END); g_log->put(C_EV, getID()); g_log->put(C_EV, (unsigned)e.ret);
		return (StateReturnType)e.ret;
	}
	virtual void onExit(StateExitType status)
	{
		g_log->put(C_EV, T_EXIT_BEGIN); g_log->put(C_EV, getID()); g_log->put(C_EV, (unsigned)status);
		Entry e = { 0, 0, 0 };
		if (!m_exit.empty()) e = m_exit[m_nExit++ % m_exit.size()];
		++m_counter;
		doAction(this, e);
		g_log->put(C_EV, T_EXIT_END); g_log->put(C_EV, getID());
	}
	virtual StateReturnType update()
	{
		g_log->put(C_EV, T_UPDATE_BEGIN); g_log->put(C_EV, getID());
		Entry e = { 0, 0, 0 };
		if (!m_update.empty()) e = m_update[m_nUpdate++ % m_update.size()];
		++m_counter;
		doAction(this, e);
		g_log->put(C_EV, T_UPDATE_END); g_log->put(C_EV, getID()); g_log->put(C_EV, (unsigned)e.ret);
		return (StateReturnType)e.ret;
	}
	virtual Bool isIdle() const { return (m_flags & 1) != 0; }
	virtual Bool isAttack() const { return (m_flags & 2) != 0; }
	virtual Bool isGuardIdle() const { return (m_flags & 4) != 0; }
	virtual Bool isBusy() const { return (m_flags & 8) != 0; }
protected:
	virtual void crc(Xfer *x) { x->xferUnsignedInt(&m_counter); x->xferUnsignedInt(&m_nUpdate); }
	virtual void xfer(Xfer *x)
	{
		XferVersion v = 1;
		x->xferVersion(&v, 1);
		x->xferUnsignedInt(&m_counter);
	}
	virtual void loadPostProcess() { }
};
inline ScriptState::~ScriptState() { }

class TestMachine : public StateMachine
{
public:
	TestMachine() : StateMachine(0, "sm") { }
	std::vector<ScriptState *> m_scripted; /* in definition order, for the counters dump */
	std::map<StateID, ScriptState *> m_byId;
};

/* the thrown types, as (type, value) */
static void logStatus(Log &lg, int type, unsigned v) { lg.put(C_RES, (unsigned)type); lg.put(C_RES, v); }

/* Runs f; logs (0,0) or (type,value) of what it threw; returns whether it threw. */
template <class F> static bool guarded(Log &lg, F f)
{
	try { f(); }
	catch (ErrorCode e) { logStatus(lg, 1, (unsigned)e); return true; }
	catch (XferStatus s) { logStatus(lg, 2, (unsigned)s); return true; }
	catch (SaveCode s) { logStatus(lg, 3, (unsigned)s); return true; }
	catch (...) { logStatus(lg, 9, 0); return true; }
	logStatus(lg, 0, 0);
	return false;
}

static void logBytes(Log &lg, const char *path)
{
	std::vector<unsigned char> b;
	FILE *fp = fopen(path, "rb");
	if (fp) {
		unsigned char buf[4096]; size_t n;
		while ((n = fread(buf, 1, sizeof buf, fp)) > 0) b.insert(b.end(), buf, buf + n);
		fclose(fp);
	}
	lg.put(C_BYT, (unsigned)b.size());
	for (size_t k = 0; k < b.size(); ++k) lg.put(C_BYT, b[k]);
}

static std::vector<unsigned char> readFile(const char *path)
{
	std::vector<unsigned char> b;
	FILE *fp = fopen(path, "rb");
	if (fp) {
		unsigned char buf[4096]; size_t n;
		while ((n = fread(buf, 1, sizeof buf, fp)) > 0) b.insert(b.end(), buf, buf + n);
		fclose(fp);
	}
	return b;
}

static void writeFile(const char *path, const std::vector<unsigned char> &b)
{
	FILE *fp = fopen(path, "wb");
	if (fp) { if (!b.empty()) fwrite(&b[0], 1, b.size(), fp); fclose(fp); }
}

/* An XferLoad that notices the two things upstream leaves undefined or
** unspecified when a stream does not fit the machine, so that the test can
** prove it never generates them (it fails if a marker appears):
**  - a Bool byte other than 0 or 1 (xferBool stores the raw byte into a `bool`);
**  - a short read of a field (fread writes the bytes that exist into the
**    destination before XFER_READ_ERROR, the Rust typed Xfer methods assign
**    only on success).
** It reads exactly as XferLoad does otherwise. */
class GuardedLoad : public XferLoad
{
public:
	GuardedLoad() : m_badBool(0), m_partial(0) { }
	int m_badBool, m_partial;
	virtual void xferBool(Bool *b) { UnsignedByte v = 0; xferImplementation(&v, 1); if (v > 1) ++m_badBool; *b = (v != 0); }
	virtual void xferImplementation(void *data, Int size)
	{
		if (m_fileFP) {
			long pos = ftell(m_fileFP);
			if (pos >= 0 && fseek(m_fileFP, 0, SEEK_END) == 0) {
				long end = ftell(m_fileFP);
				fseek(m_fileFP, pos, SEEK_SET);
				long rem = end - pos;
				if (rem > 0 && rem < size) ++m_partial;
			}
		}
		XferLoad::xferImplementation(data, size);
	}
};

/* The mutation of a saved stream that op 15 applies (mirrored in the Rust):
** kind 1 truncates to length a, 2 sets byte a to b, 3 sets the u32 at a to b. */
static void mutate(std::vector<unsigned char> &s, unsigned kind, unsigned a, unsigned b)
{
	if (kind == 1) { if (a < s.size()) s.resize(a); }
	else if (kind == 2) { if (a < s.size()) s[a] = (unsigned char)b; }
	else if (kind == 3) { if (a + 4 <= s.size()) for (int k = 0; k < 4; ++k) s[a + k] = (unsigned char)(b >> (8 * k)); }
}

/* what the log carries after every operation */
static void logView(Log &lg, TestMachine *m)
{
	lg.put(C_RES, m->getCurrentStateID());
	lg.put(C_RES, m->m_sleepTill);
	lg.put(C_RES, m->m_defaultStateID);
	lg.put(C_RES, (unsigned)m->m_goalObjectID);
	lg.putReal(C_RES, m->m_goalPosition.x); lg.putReal(C_RES, m->m_goalPosition.y); lg.putReal(C_RES, m->m_goalPosition.z);
	lg.put(C_RES, m->m_locked ? 1u : 0u);
	lg.put(C_RES, m->m_defaultStateInited ? 1u : 0u);
	lg.put(C_RES, m->isInIdleState() ? 1u : 0u);
	lg.put(C_RES, m->isInAttackState() ? 1u : 0u);
	lg.put(C_RES, m->isInForceAttackState() ? 1u : 0u);
	lg.put(C_RES, m->isInGuardIdleState() ? 1u : 0u);
	lg.put(C_RES, m->isInBusyState() ? 1u : 0u);
	lg.put(C_RES, m->isLocked() ? 1u : 0u);
	lg.put(C_RES, m->isGoalObjectDestroyed() ? 1u : 0u);
	const Object *go = ((const StateMachine *)m)->getGoalObject();
	lg.put(C_RES, go ? (unsigned)go->getID() : 0u);
	lg.put(C_RES, (unsigned)TheGameState->m_postProcessCount);
	for (std::map<StateID, ScriptState *>::iterator it = m->m_byId.begin(); it != m->m_byId.end(); ++it) {
		lg.put(C_RES, it->second->m_counter); lg.put(C_RES, it->second->m_nEnter); lg.put(C_RES, it->second->m_nUpdate); lg.put(C_RES, it->second->m_nExit);
	}
}

} /* namespace */

UnsignedInt GameLogic::getFrame() { return g_frame; }
Object *GameLogic::findObjectByID(ObjectID id)
{
	if (g_live.find(id) == g_live.end()) return 0;
	for (size_t k = 0; k < g_objects.size(); ++k) if (g_objects[k].m_id == id) return &g_objects[k];
	return 0;
}

extern "C" {

/* Returns the log length (the four categories, each as its length and words; > cap means overflow), or -1 for a malformed script. `path` is a scratch
** file prefix for the real XferSave/XferLoad/XferDeepCRC files. */
int ref_sm_run(const char *path, const unsigned *script, int n, unsigned *log, int cap)
{
	Script sc = { script, n, 0, false };
	Log lg;
	g_log = &lg;
	GameLogic theLogic;
	TheGameLogic = &theLogic;
	g_frame = 0; g_actDepth = 0; g_noActions = false;
	g_conds.clear(); g_condCalls.clear(); g_objects.clear(); g_live.clear();
	TheGameState->m_postProcessCount = 0;
	std::string pathSave(path); pathSave += ".sav";
	std::string pathDeep(path); pathDeep += ".deep";
	std::string pathLoad(path); pathLoad += ".load";

	unsigned nConds = sc.next();
	for (unsigned k = 0; k < nConds && !sc.bad; ++k) {
		unsigned len = sc.next();
		std::vector<unsigned> seq;
		for (unsigned j = 0; j < len && !sc.bad; ++j) seq.push_back(sc.next());
		g_conds.push_back(seq);
		g_condCalls.push_back(0);
	}
	unsigned nObj = sc.next();
	for (unsigned k = 0; k < nObj && !sc.bad; ++k) {
		Object o;
		o.m_id = (ObjectID)sc.next();
		unsigned bits[3]; for (int j = 0; j < 3; ++j) bits[j] = sc.next();
		memcpy(&o.m_position.x, &bits[0], 4); memcpy(&o.m_position.y, &bits[1], 4); memcpy(&o.m_position.z, &bits[2], 4);
		g_objects.push_back(o);
		g_live.insert(o.m_id);
	}

	TestMachine *m = new TestMachine;
	unsigned nStates = sc.next();
	for (unsigned k = 0; k < nStates && !sc.bad; ++k) {
		unsigned kind = sc.next();
		StateID id = sc.next(), succ = sc.next(), fail = sc.next();
		unsigned nTrans = sc.next();
		std::vector<StateConditionInfo> conds;
		for (unsigned j = 0; j < nTrans && !sc.bad; ++j) {
			unsigned ci = sc.next(); StateID to = sc.next();
			conds.push_back(StateConditionInfo(condTest, to, (void *)(uintptr_t)ci));
		}
		conds.push_back(StateConditionInfo((StateTransFuncPtr)0, 0, (void *)0));
		State *st = 0;
		switch (kind) {
		case 0: st = new SuccessState(m); break;
		case 1: st = new FailureState(m); break;
		case 2: st = new ContinueState(m); break;
		case 3: st = new SleepState(m); break;
		default: {
			ScriptState *ss = new ScriptState(m);
			ss->m_flags = sc.next();
			std::vector<Entry> *lists[3] = { &ss->m_enter, &ss->m_update, &ss->m_exit };
			for (int l = 0; l < 3; ++l) {
				unsigned cnt = sc.next();
				for (unsigned j = 0; j < cnt && !sc.bad; ++j) { Entry e; e.ret = (int)sc.next(); e.act = sc.next(); e.arg = sc.next(); lists[l]->push_back(e); }
			}
			m->m_scripted.push_back(ss);
			m->m_byId[id] = ss;
			st = ss;
			break; }
		}
		m->defineState(id, st, succ, fail, &conds[0]);
	}

	unsigned nOps = sc.next();
	std::vector<unsigned char> lastSave;
	for (unsigned o = 0; o < nOps && !sc.bad; ++o) {
		unsigned op = sc.next();
		lg.put(C_RES, 100 + op);
		StateReturnType ret = STATE_CONTINUE;
		bool hasRet = false;
		switch (op) {
		case 0: hasRet = true; if (!guarded(lg, [&]{ ret = m->initDefaultState(); })) lg.put(C_RES, (unsigned)ret); break;
		case 1: hasRet = true; if (!guarded(lg, [&]{ ret = m->updateStateMachine(); })) lg.put(C_RES, (unsigned)ret); break;
		case 2: { StateID id = sc.next(); hasRet = true; if (!guarded(lg, [&]{ ret = m->setState(id); })) lg.put(C_RES, (unsigned)ret); break; }
		case 3: hasRet = true; if (!guarded(lg, [&]{ ret = m->resetToDefaultState(); })) lg.put(C_RES, (unsigned)ret); break;
		case 4: guarded(lg, [&]{ m->clear(); }); break;
		case 5: m->lock("script"); logStatus(lg, 0, 0); break;
		case 6: m->unlock(); logStatus(lg, 0, 0); break;
		case 7: m->halt(); logStatus(lg, 0, 0); break;
		case 8: { unsigned idx = sc.next(); m->setGoalObject(idx < g_objects.size() ? &g_objects[idx] : (const Object *)0); logStatus(lg, 0, 0); break; }
		case 9: { unsigned has = sc.next(); Coord3D p; unsigned b[3]; for (int j = 0; j < 3; ++j) b[j] = sc.next();
			memcpy(&p.x, &b[0], 4); memcpy(&p.y, &b[1], 4); memcpy(&p.z, &b[2], 4);
			m->setGoalPosition(has ? &p : (const Coord3D *)0); logStatus(lg, 0, 0); break; }
		case 10: g_frame = sc.next(); logStatus(lg, 0, 0); break;
		case 11: { unsigned idx = sc.next(); if (idx < g_objects.size()) g_live.erase(g_objects[idx].m_id); logStatus(lg, 0, 0); break; }
		case 12: { unsigned idx = sc.next(); if (idx < g_objects.size()) g_live.insert(g_objects[idx].m_id); logStatus(lg, 0, 0); break; }
		case 13: { /* save with the real XferSave */
			XferSave xs;
			guarded(lg, [&]{ xs.open(AsciiString(pathSave.c_str())); xs.xferSnapshot(m); });
			try { xs.close(); } catch (...) { }
			lastSave = readFile(pathSave.c_str());
			lg.put(C_BYT, (unsigned)lastSave.size());
			for (size_t k = 0; k < lastSave.size(); ++k) lg.put(C_BYT, lastSave[k]);
			break; }
		case 14: { /* the light CRC (Snapshot::crc), the CRC of xfer(), the deep CRC of xfer() */
			XferCRC xc;
			if (!guarded(lg, [&]{ xc.open(AsciiString("crc")); xc.xferSnapshot(m); })) lg.put(C_CRC, xc.getCRC());
			XferCRC xc2;
			if (!guarded(lg, [&]{ xc2.open(AsciiString("crc2")); m->xfer(&xc2); })) lg.put(C_CRC, xc2.getCRC());
			XferDeepCRC xd;
			bool threw = guarded(lg, [&]{ xd.open(AsciiString(pathDeep.c_str())); m->xfer(&xd); });
			try { xd.close(); } catch (...) { }
			if (!threw) lg.put(C_CRC, xd.getCRC());
			logBytes(lg, pathDeep.c_str());
			break; }
		case 15: case 16: { /* load into the machine from the last save mutated (15) or from given bytes (16) */
			std::vector<unsigned char> bytes;
			if (op == 15) { unsigned kind = sc.next(), a = sc.next(), b = sc.next(); bytes = lastSave; mutate(bytes, kind, a, b); }
			else { unsigned cnt = sc.next(); for (unsigned k = 0; k < cnt && !sc.bad; ++k) bytes.push_back((unsigned char)sc.next()); }
			writeFile(pathLoad.c_str(), bytes);
			TheGameState->m_postProcessCount = 0;
			GuardedLoad xl;
			guarded(lg, [&]{ xl.open(AsciiString(pathLoad.c_str())); xl.xferSnapshot(m); });
			try { xl.close(); } catch (...) { }
			if (xl.m_badBool || xl.m_partial) { lg.put(C_RES, 0xDEAD0001u); lg.put(C_RES, (unsigned)xl.m_badBool); lg.put(C_RES, (unsigned)xl.m_partial); }
			break; }
		case 17: { StateID id = sc.next(); hasRet = true; if (!guarded(lg, [&]{ ret = m->internalSetState(id); })) lg.put(C_RES, (unsigned)ret); break; }
		default: sc.bad = true; break;
		}
		(void)hasRet;
		logView(lg, m);
	}
	if (sc.bad) { delete m; g_log = 0; return -1; }

	/* the destructor: onExit(EXIT_RESET) of the current state, no actions */
	lg.put(C_RES, 200);
	g_noActions = true;
	delete m;
	g_noActions = false;
	lg.put(C_RES, 201);
	g_log = 0;
	int n = 0;
	for (int c = 0; c < 4; ++c) {
		if (n < cap) log[n] = (unsigned)lg.v[c].size();
		++n;
		for (size_t k = 0; k < lg.v[c].size(); ++k) { if (n < cap) log[n] = lg.v[c][k]; ++n; }
	}
	return n;
}

} /* extern "C" */
