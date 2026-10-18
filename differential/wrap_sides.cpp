/* Differential-test wrapper for the map's sides and scripts (upstream commit
** 0a05454d):
**   GameEngine/Source/GameLogic/Map/SidesList.cpp
**   GameEngine/Source/GameLogic/ScriptEngine/Scripts.cpp
** with GameLogic/SidesList.h and GameLogic/Scripts.h, compiled as separate
** translation units beside this one (build.rs), and the chunk reader and
** writer of the chunk build (DataChunk.cpp, NameKeyGenerator.cpp, the
** TheKey_* well-known keys; differential/wrap_chunk.cpp, wrap_namekey.cpp).
** Scripts.cpp is unchanged. SidesList.cpp has ONE mechanical edit, made by
** build.rs in a copy under OUT_DIR and asserted to match once: the call that
** opens the skirmish scripts file, `theInputStream.open(path)`, becomes a call
** to sides_open_skirmish_stream below (the real open needs the file system
** and CompressionManager::isDataCompressed, which aborts here), so that the
** rest of prepareForMP_or_Skirmish, the reading of that file included, runs
** as written. The stand-ins are in differential/shim_sides (each says what
** it stands in for); ScriptEngine is reduced to the two template tables
** Scripts.cpp asks it for, filled by ref_sides_setup.
**
** This file reaches into private and protected members (`#define private
** public` around the upstream headers): the dumps below read every field of
** the classes, including those that have no accessor (Parameter's
** m_initialized, m_objectStatus). That changes no layout (the Itanium ABI
** lays out members regardless of access).
**
** ref_sides_run parses a map file (mode 0: a SidesList chunk through
** SidesList::ParseSidesDataChunk; mode 1: a PlayerScriptsList chunk through
** ScriptList::ParseScriptsDataChunk, the lists taken with getReadScripts),
** dumps everything it read, runs a script of operations on TheSidesList
** (mode 0; see the table at ref_sides_run), writes the result with
** SidesList::WriteSidesDataChunk / ScriptList::WriteScriptsDataChunk through a
** DataChunkOutput, and dumps again (validateSides runs after the write, and
** WriteParameter changes KIND_OF strings). The log format is the one
** tests/differential_sides_scripts.rs documents and reproduces. */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <string>
#include <vector>
#include <map>
#include <new>
#include <algorithm>
#define private public
#define protected public
#include "Common/DataChunk.h"
#include "Common/WellKnownKeys.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/KindOf.h"
#include "GameLogic/SidesList.h"
#include "GameLogic/Scripts.h"
#include "GameLogic/ScriptEngine.h"
#include "GameLogic/AI.h"
#undef private
#undef protected

extern "C" const char *ref_kindof_name_from_single_bit(int i);
extern "C" const char *(*ref_chunk_user_data_path_hook)();

/* ---- the stand-ins the shims declare ------------------------------------ */

ScriptEngine *TheScriptEngine = NULL;

static ConditionTemplate g_cond[Condition::NUM_ITEMS];
static ActionTemplate g_act[ScriptAction::NUM_ITEMS];

/* ScriptEngine.cpp lines 7694-7712: an index out of range is 0 */
const ActionTemplate *ScriptEngine::getActionTemplate(Int ndx)
{
	if (ndx < 0 || ndx >= ScriptAction::NUM_ITEMS) ndx = 0;
	return &g_act[ndx];
}
const ConditionTemplate *ScriptEngine::getConditionTemplate(Int ndx)
{
	if (ndx < 0 || ndx >= Condition::NUM_ITEMS) ndx = 0;
	return &g_cond[ndx];
}
void ScriptEngine::signalUIInteract(const AsciiString &) { abort(); }

/* AISideBuildList::addInfo is reached only by BuildListInfo::parseStructure (INI) */
void AISideBuildList::addInfo(BuildListInfo *) { abort(); }

/* BitFlags<KINDOF_COUNT>::getBitNames(): the NULL-terminated name table of KindOf.cpp,
** read through wrap_flags.cpp's lookup (which answers NULL at index KINDOF_COUNT) */
const char **KindOfMaskType::getBitNames()
{
	static std::vector<const char *> names;
	if (names.empty()) {
		for (int i = 0;; ++i) {
			const char *n = ref_kindof_name_from_single_bit(i);
			names.push_back(n);
			if (!n) break;
		}
	}
	return &names[0];
}

namespace {

std::vector<unsigned char> g_log;
std::vector<char> g_skirmish;
std::string g_tmpDir;

const char *userDataDir() { return g_tmpDir.c_str(); }

class TestStream : public CachedFileInputStream
{
public:
	void setData(const char *data, int len)
	{
		m_buffer = new char[len];
		memcpy(m_buffer, data, len);
		m_size = len;
		m_pos = 0;
	}
};

class MemOut : public OutputStream
{
public:
	std::vector<unsigned char> b;
	Int write(const void *p, Int n)
	{
		b.insert(b.end(), (const unsigned char *)p, (const unsigned char *)p + n);
		return n;
	}
};

void put(const void *p, int n) { g_log.insert(g_log.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void put8(int v) { g_log.push_back((unsigned char)v); }
void put32(unsigned v) { put(&v, 4); }
void putF(float f) { put(&f, 4); }
void putStr(const char *s) { unsigned n = (unsigned)strlen(s); put32(n); put(s, (int)n); }
void putS(const AsciiString &s) { putStr(s.str()); }
void putW(const UnicodeString &s)
{
	unsigned n = 0;
	while (s.str()[n]) ++n;
	put32(n);
	put(s.str(), (int)n * 2);
}

/* ---- the dumps ---------------------------------------------------------- */

void dumpDict(const Dict &d)
{
	put32((unsigned)d.getPairCount());
	for (std::map<NameKeyType, Dict::Value>::const_iterator it = d.pairs().begin(); it != d.pairs().end(); ++it) {
		putStr(TheNameKeyGenerator->keyToName(it->first).str());
		const Dict::Value &x = it->second;
		put8(x.type);
		switch (x.type) {
		case Dict::DICT_BOOL: put8(x.b ? 1 : 0); break;
		case Dict::DICT_INT: put(&x.i, 4); break;
		case Dict::DICT_REAL: put(&x.r, 4); break;
		case Dict::DICT_ASCIISTRING: putS(x.a); break;
		case Dict::DICT_UNICODESTRING: putW(x.u); break;
		default: abort();
		}
	}
}

void dumpParam(Parameter *p)
{
	put32((unsigned)p->m_paramType);
	put8(p->m_initialized ? 1 : 0);
	put32((unsigned)p->m_int);
	putF(p->m_real);
	putS(p->m_string);
	putF(p->m_coord.x); putF(p->m_coord.y); putF(p->m_coord.z);
	unsigned n = 0;
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (p->m_objectStatus.test(i)) ++n;
	put32(n);
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (p->m_objectStatus.test(i)) put32((unsigned)i);
}

void dumpParms(Parameter **parms, int numParms)
{
	for (int i = 0; i < numParms; ++i) {
		if (parms[i]) { put8(1); dumpParam(parms[i]); } else put8(0);
	}
}

void dumpCondition(Condition *c)
{
	put32((unsigned)c->m_conditionType);
	put32((unsigned)c->m_numParms);
	put32((unsigned)c->m_hasWarnings);
	put32((unsigned)c->m_customData);
	put32(c->m_customFrame);
	dumpParms(c->m_parms, c->m_numParms);
}

void dumpAction(ScriptAction *a)
{
	put32((unsigned)a->m_actionType);
	put32((unsigned)a->m_numParms);
	put8(a->m_hasWarnings ? 1 : 0);
	dumpParms(a->m_parms, a->m_numParms);
}

void dumpActions(ScriptAction *a)
{
	unsigned n = 0;
	for (ScriptAction *x = a; x; x = x->m_nextAction) ++n;
	put32(n);
	for (ScriptAction *x = a; x; x = x->m_nextAction) dumpAction(x);
}

void dumpScript(Script *s)
{
	putS(s->m_scriptName); putS(s->m_comment); putS(s->m_conditionComment); putS(s->m_actionComment);
	put32((unsigned)s->m_delayEvaluationSeconds);
	put8(s->m_isActive); put8(s->m_isOneShot); put8(s->m_isSubroutine);
	put8(s->m_easy); put8(s->m_normal); put8(s->m_hard);
	unsigned n = 0;
	for (OrCondition *o = s->m_condition; o; o = o->m_nextOr) ++n;
	put32(n);
	for (OrCondition *o = s->m_condition; o; o = o->m_nextOr) {
		unsigned m = 0;
		for (Condition *c = o->m_firstAnd; c; c = c->m_nextAndCondition) ++m;
		put32(m);
		for (Condition *c = o->m_firstAnd; c; c = c->m_nextAndCondition) dumpCondition(c);
	}
	dumpActions(s->m_action);
	dumpActions(s->m_actionFalse);
	put32(s->m_frameToEvaluateAt);
	put8(s->m_hasWarnings);
	putS(s->m_conditionTeamName);
	putF(s->m_conditionTime);
	putF(s->m_curTime);
	put32((unsigned)s->m_conditionExecutedCount);
}

void dumpScripts(Script *s)
{
	unsigned n = 0;
	for (Script *x = s; x; x = x->m_nextScript) ++n;
	put32(n);
	for (Script *x = s; x; x = x->m_nextScript) dumpScript(x);
}

void dumpScriptList(ScriptList *l)
{
	unsigned n = 0;
	for (ScriptGroup *g = l->m_firstGroup; g; g = g->m_nextGroup) ++n;
	put32(n);
	for (ScriptGroup *g = l->m_firstGroup; g; g = g->m_nextGroup) {
		putS(g->m_groupName);
		put8(g->m_isGroupActive); put8(g->m_isGroupSubroutine); put8(g->m_hasWarnings);
		dumpScripts(g->m_firstScript);
	}
	dumpScripts(l->m_firstScript);
}

void dumpBuild(BuildListInfo *b)
{
	putS(b->m_buildingName); putS(b->m_templateName);
	putF(b->m_location.x); putF(b->m_location.y); putF(b->m_location.z);
	putF(b->m_rallyPointOffset.x); putF(b->m_rallyPointOffset.y);
	putF(b->m_angle);
	put8(b->m_isInitiallyBuilt);
	put32(b->m_numRebuilds);
	putS(b->m_script);
	put32((unsigned)b->m_health);
	put8(b->m_whiner); put8(b->m_unsellable); put8(b->m_repairable); put8(b->m_automaticallyBuild); put8(b->m_selected);
	put32((unsigned)b->m_objectID);
	put32(b->m_objectTimestamp);
	put8(b->m_underConstruction);
	for (int i = 0; i < BuildListInfo::MAX_RESOURCE_GATHERERS; ++i) put32((unsigned)b->m_resourceGatherers[i]);
	put8(b->m_isSupplyBuilding);
	put32((unsigned)b->m_desiredGatherers); put32((unsigned)b->m_currentGatherers);
	put8(b->m_priorityBuild);
}

void dumpSide(SidesInfo &s)
{
	dumpDict(s.m_dict);
	unsigned n = 0;
	for (BuildListInfo *b = s.m_pBuildList; b; b = b->m_nextBuildList) ++n;
	put32(n);
	for (BuildListInfo *b = s.m_pBuildList; b; b = b->m_nextBuildList) dumpBuild(b);
	if (s.m_scripts) { put8(1); dumpScriptList(s.m_scripts); } else put8(0);
}

void dumpTeams(TeamsInfoRec &t)
{
	put32((unsigned)t.m_numTeams);
	for (int i = 0; i < t.m_numTeams; ++i) dumpDict(t.m_teams[i].m_dict);
}

/* 'S', then the sides, the skirmish sides, the teams, the skirmish teams */
void dumpState()
{
	put8('S');
	SidesList &S = *TheSidesList;
	put32((unsigned)S.m_numSides);
	for (int i = 0; i < S.m_numSides; ++i) dumpSide(S.m_sides[i]);
	put32((unsigned)S.m_numSkirmishSides);
	for (int i = 0; i < S.m_numSkirmishSides; ++i) dumpSide(S.m_skirmishSides[i]);
	dumpTeams(S.m_teamrec);
	dumpTeams(S.m_skirmishTeamrec);
}

/* ---- the operations ------------------------------------------------------ */

struct Rd
{
	const unsigned char *p; int n; int i;
	int u8() { return i < n ? p[i++] : 0; }
	int i8() { return (signed char)u8(); }
	std::string str() { int l = u8(); std::string s; for (int k = 0; k < l; ++k) s += (char)u8(); return s; }
};

Script *nthScript(Script *s, int n) { while (s && n > 0) { s = s->m_nextScript; --n; } return s; }
ScriptGroup *nthGroup(ScriptGroup *g, int n) { while (g && n > 0) { g = g->m_nextGroup; --n; } return g; }
OrCondition *nthOr(OrCondition *o, int n) { while (o && n > 0) { o = o->m_nextOr; --n; } return o; }
Condition *nthCond(Condition *c, int n) { while (c && n > 0) { c = c->m_nextAndCondition; --n; } return c; }
ScriptAction *nthAction(ScriptAction *a, int n) { while (a && n > 0) { a = a->m_nextAction; --n; } return a; }
BuildListInfo *nthBuild(BuildListInfo *b, int n) { while (b && n > 0) { b = b->m_nextBuildList; --n; } return b; }

ScriptList *sideScripts(int side)
{
	SidesList &S = *TheSidesList;
	return (side >= 0 && side < S.m_numSides) ? S.m_sides[side].m_scripts : NULL;
}

/* One operation. 'x' in the log: skipped (an index outside what exists).
**   0 i8                  removeSide(i)
**   1 i8                  removeTeam(i)
**   2 str                 addPlayerByTemplate(name)
**   3                     validateSides() -> u8
**   4 str                 findSideInfo(name) -> i32 index or -1
**   5 str                 findTeamInfo(name) -> i32 index or -1
**   6 u8                  isPlayerDefaultTeam(team i) -> u8
**   7                     prepareForMP_or_Skirmish() with the skirmish file of the case
**   8 s d str str str     m_sides[d].scripts = m_sides[s].scripts->duplicateAndQualify(q, tpl, new)
**   9 s b i8              m_sides[s].reorderInBuildList(entry b, position)
**  10 s i                 side s scripts: deleteScript(script i)
**  11 s i                 deleteGroup(group i)
**  12 s g i               group g: deleteScript(i)
**  13 s i a               script i: deleteAction(a)
**  14 s i o               script i: deleteOrCondition(o)
**  15 s i o c             script i, or o: deleteCondition(c)
**  16 s i a               script i: deleteFalseAction(a)
**  17 s i i8              addScript(script i duplicated, position)
**  18                     ScriptList::updateDefaults()
**  19                     ScriptList::reset()
**  20 i j                 m_sides[j] = m_sides[i]  (SidesInfo::operator=)
**  21 s i s2 i2           script i of side s updateFrom(a duplicate of script i2 of side s2)
**  22 s g s2 i2 i8        group g: addScript(a duplicate of script i2 of side s2, position)
**  23 s i o               script i: findPreviousOrCondition(or o) -> i32 index or -1
**  24 s i o c             script i, or o: findPreviousCondition(c) -> i32 index or -1
**  25 s d                 m_sides[d].scripts = m_sides[s].scripts->duplicate()  */
void runOp(Rd &r)
{
	SidesList &S = *TheSidesList;
	int op = r.u8();
	put8('o'); put8(op);
	switch (op) {
	case 0: S.removeSide(r.i8()); break;
	case 1: S.removeTeam(r.i8()); break;
	case 2: S.addPlayerByTemplate(AsciiString(r.str().c_str())); break;
	case 3: put8(S.validateSides() ? 1 : 0); break;
	case 4: { Int idx = -1; S.findSideInfo(AsciiString(r.str().c_str()), &idx); put32((unsigned)idx); break; }
	case 5: { Int idx = -1; S.findTeamInfo(AsciiString(r.str().c_str()), &idx); put32((unsigned)idx); break; }
	case 6: {
		int i = r.u8();
		if (i < S.getNumTeams()) put8(S.isPlayerDefaultTeam(S.getTeamInfo(i)) ? 1 : 0); else put8('x');
		break;
	}
	case 7: S.prepareForMP_or_Skirmish(); break;
	case 8: {
		int s = r.u8(), d = r.u8();
		std::string q = r.str(), tpl = r.str(), nn = r.str();
		if (s >= S.m_numSides || d >= S.m_numSides) { put8('x'); break; }
		ScriptList *src = S.m_sides[s].m_scripts;
		ScriptList *nl = src ? src->duplicateAndQualify(AsciiString(q.c_str()), AsciiString(tpl.c_str()), AsciiString(nn.c_str())) : NULL;
		S.m_sides[d].m_scripts->deleteInstance();
		S.m_sides[d].m_scripts = nl;
		break;
	}
	case 9: {
		int s = r.u8(), b = r.u8(), pos = r.i8();
		BuildListInfo *bl = s < S.m_numSides ? nthBuild(S.m_sides[s].m_pBuildList, b) : NULL;
		if (!bl) { put8('x'); break; }
		S.m_sides[s].reorderInBuildList(bl, pos);
		break;
	}
	case 10: case 11: {
		int s = r.u8(), i = r.u8();
		ScriptList *l = sideScripts(s);
		if (!l) { put8('x'); break; }
		if (op == 10) { Script *x = nthScript(l->m_firstScript, i); if (x) l->deleteScript(x); else put8('x'); }
		else { ScriptGroup *x = nthGroup(l->m_firstGroup, i); if (x) l->deleteGroup(x); else put8('x'); }
		break;
	}
	case 12: {
		int s = r.u8(), g = r.u8(), i = r.u8();
		ScriptList *l = sideScripts(s);
		ScriptGroup *gr = l ? nthGroup(l->m_firstGroup, g) : NULL;
		Script *x = gr ? nthScript(gr->m_firstScript, i) : NULL;
		if (!x) { put8('x'); break; }
		gr->deleteScript(x);
		break;
	}
	case 13: case 14: case 16: {
		int s = r.u8(), i = r.u8(), a = r.u8();
		ScriptList *l = sideScripts(s);
		Script *sc = l ? nthScript(l->m_firstScript, i) : NULL;
		if (!sc) { put8('x'); break; }
		if (op == 13) { ScriptAction *x = nthAction(sc->m_action, a); if (x) sc->deleteAction(x); else put8('x'); }
		else if (op == 16) { ScriptAction *x = nthAction(sc->m_actionFalse, a); if (x) sc->deleteFalseAction(x); else put8('x'); }
		else { OrCondition *x = nthOr(sc->m_condition, a); if (x) sc->deleteOrCondition(x); else put8('x'); }
		break;
	}
	case 15: {
		int s = r.u8(), i = r.u8(), o = r.u8(), c = r.u8();
		ScriptList *l = sideScripts(s);
		Script *sc = l ? nthScript(l->m_firstScript, i) : NULL;
		OrCondition *or_ = sc ? nthOr(sc->m_condition, o) : NULL;
		Condition *cd = or_ ? nthCond(or_->m_firstAnd, c) : NULL;
		if (!cd) { put8('x'); break; }
		or_->deleteCondition(cd);
		break;
	}
	case 17: {
		int s = r.u8(), i = r.u8(), pos = r.i8();
		ScriptList *l = sideScripts(s);
		Script *sc = l ? nthScript(l->m_firstScript, i) : NULL;
		if (!sc) { put8('x'); break; }
		l->addScript(sc->duplicate(), pos);
		break;
	}
	case 18: ScriptList::updateDefaults(); break;
	case 19: ScriptList::reset(); break;
	case 20: {
		int i = r.u8(), j = r.u8();
		if (i >= S.m_numSides || j >= S.m_numSides) { put8('x'); break; }
		*S.getSideInfo(j) = *S.getSideInfo(i);
		break;
	}
	case 21: {
		int s = r.u8(), i = r.u8(), s2 = r.u8(), i2 = r.u8();
		ScriptList *l = sideScripts(s), *l2 = sideScripts(s2);
		Script *dst = l ? nthScript(l->m_firstScript, i) : NULL;
		Script *src = l2 ? nthScript(l2->m_firstScript, i2) : NULL;
		if (!dst || !src) { put8('x'); break; }
		Script *tmp = src->duplicate();
		dst->updateFrom(tmp);
		tmp->deleteInstance();
		break;
	}
	case 22: {
		int s = r.u8(), g = r.u8(), s2 = r.u8(), i2 = r.u8(), pos = r.i8();
		ScriptList *l = sideScripts(s), *l2 = sideScripts(s2);
		ScriptGroup *gr = l ? nthGroup(l->m_firstGroup, g) : NULL;
		Script *src = l2 ? nthScript(l2->m_firstScript, i2) : NULL;
		if (!gr || !src) { put8('x'); break; }
		gr->addScript(src->duplicate(), pos);
		break;
	}
	case 23: {
		int s = r.u8(), i = r.u8(), o = r.u8();
		ScriptList *l = sideScripts(s);
		Script *sc = l ? nthScript(l->m_firstScript, i) : NULL;
		OrCondition *or_ = sc ? nthOr(sc->m_condition, o) : NULL;
		if (!or_) { put8('x'); break; }
		OrCondition *prev = sc->findPreviousOrCondition(or_);
		int idx = -1;
		for (OrCondition *x = sc->m_condition; x; x = x->m_nextOr) { ++idx; if (x == prev) break; }
		put32(prev ? (unsigned)idx : 0xFFFFFFFFu);
		break;
	}
	case 24: {
		int s = r.u8(), i = r.u8(), o = r.u8(), c = r.u8();
		ScriptList *l = sideScripts(s);
		Script *sc = l ? nthScript(l->m_firstScript, i) : NULL;
		OrCondition *or_ = sc ? nthOr(sc->m_condition, o) : NULL;
		Condition *cd = or_ ? nthCond(or_->m_firstAnd, c) : NULL;
		if (!cd) { put8('x'); break; }
		Condition *prev = or_->findPreviousCondition(cd);
		int idx = -1;
		for (Condition *x = or_->m_firstAnd; x; x = x->m_nextAndCondition) { ++idx; if (x == prev) break; }
		put32(prev ? (unsigned)idx : 0xFFFFFFFFu);
		break;
	}
	case 25: {
		int s = r.u8(), d = r.u8();
		if (s >= S.m_numSides || d >= S.m_numSides) { put8('x'); break; }
		ScriptList *src = S.m_sides[s].m_scripts;
		ScriptList *nl = src ? src->duplicate() : NULL;
		S.m_sides[d].m_scripts->deleteInstance();
		S.m_sides[d].m_scripts = nl;
		break;
	}
	default: put8('?'); break;
	}
}

/* The writer: a DataChunkOutput over memory, then the bytes into the log */
template <class F> void writeChunk(F f)
{
	MemOut mo;
	try {
		DataChunkOutput w(&mo);
		f(w);
	} catch (ErrorCode c) {
		put8('T'); put32((unsigned)c);
	}
	put8('W');
	put32((unsigned)mo.b.size());
	put(mo.b.empty() ? "" : (const char *)&mo.b[0], (int)mo.b.size());
}

} /* namespace */

/* The stand-in of build.rs's one edit of SidesList.cpp (see the top): the
** skirmish scripts file is the case's bytes. */
Bool sides_open_skirmish_stream(CachedFileInputStream &stream, const AsciiString &)
{
	if (g_skirmish.empty()) return FALSE;
	/* what open() leaves for uncompressed data: the buffer, its size, position 0 (the protected
	** members, through a pointer to member named in a derived class) */
	struct Access : CachedFileInputStream
	{
		static void set(CachedFileInputStream &s, const char *data, int len)
		{
			char *CachedFileInputStream::*pb = &Access::m_buffer;
			int CachedFileInputStream::*ps = &Access::m_size;
			int CachedFileInputStream::*pp = &Access::m_pos;
			s.*pb = new char[len];
			memcpy(s.*pb, data, len);
			s.*ps = len;
			s.*pp = 0;
		}
	};
	Access::set(stream, &g_skirmish[0], (int)g_skirmish.size());
	return TRUE;
}

extern "C" {

/* Install the pieces the run needs, once. `blob`:
**   u32 n; n times (u16 len, bytes)   names to give name keys to first, in order
**   109 + 344 times: u16 len, bytes (the template's internal name), u8 count, count x u8 (the
**   parameter types)                  Condition::NUM_ITEMS conditions, then ScriptAction::NUM_ITEMS actions
** and the directory (with its separator) for DataChunkOutput's temporary file. */
void ref_sides_setup(const char *tmpDir, const unsigned char *blob, int blobLen)
{
	g_tmpDir = tmpDir;
	ref_chunk_user_data_path_hook = userDataDir;
	if (!TheNameKeyGenerator) {
		TheNameKeyGenerator = new NameKeyGenerator;
		TheNameKeyGenerator->init();
	}
	if (!TheScriptEngine) TheScriptEngine = new ScriptEngine;
	Rd r = { blob, blobLen, 0 };
	unsigned n = 0;
	for (int k = 0; k < 4; ++k) n |= (unsigned)r.u8() << (8 * k);
	for (unsigned k = 0; k < n; ++k) {
		int l = r.u8(); l |= r.u8() << 8;
		std::string s;
		for (int j = 0; j < l; ++j) s += (char)r.u8();
		TheNameKeyGenerator->nameToKey(s.c_str());
	}
	for (int k = 0; k < Condition::NUM_ITEMS + ScriptAction::NUM_ITEMS; ++k) {
		int l = r.u8(); l |= r.u8() << 8;
		std::string s;
		for (int j = 0; j < l; ++j) s += (char)r.u8();
		Template *t = k < Condition::NUM_ITEMS ? (Template *)&g_cond[k] : (Template *)&g_act[k - Condition::NUM_ITEMS];
		t->m_internalName = AsciiString(s.c_str());
		t->m_internalNameKey = TheNameKeyGenerator->nameToKey(s.c_str());
		t->m_numParameters = r.u8();
		for (int j = 0; j < t->m_numParameters; ++j) t->m_parameters[j] = (Parameter::ParameterType)r.u8();
	}
}

/* The constants the Rust translation names, in the order of the Rust test's
** table: out[0..n). Returns n. */
int ref_sides_constants(int *out)
{
	int n = 0;
#define V(x) out[n++] = (int)(x);
	V(Condition::NUM_ITEMS) V(ScriptAction::NUM_ITEMS) V(Parameter::NUM_ITEMS) V(MAX_PARMS) V(MAX_PLAYER_COUNT)
	V(KINDOF_COUNT) V(OBJECT_STATUS_COUNT)
	V(ScriptAction::NO_OP) V(ScriptAction::MOVE_CAMERA_TO) V(ScriptAction::MOVE_CAMERA_ALONG_WAYPOINT_PATH) V(ScriptAction::ROTATE_CAMERA)
	V(ScriptAction::RESET_CAMERA) V(ScriptAction::CAMERA_MOD_SET_FINAL_ZOOM) V(ScriptAction::CAMERA_MOD_SET_FINAL_PITCH)
	V(ScriptAction::TEAM_FOLLOW_WAYPOINTS) V(ScriptAction::NAMED_SET_ATTITUDE) V(ScriptAction::TEAM_SET_ATTITUDE) V(ScriptAction::SPEECH_PLAY)
	V(ScriptAction::MAP_REVEAL_AT_WAYPOINT) V(ScriptAction::MAP_REVEAL_ALL) V(ScriptAction::ZOOM_CAMERA) V(ScriptAction::PITCH_CAMERA)
	V(ScriptAction::MAP_SHROUD_AT_WAYPOINT) V(ScriptAction::MAP_SHROUD_ALL) V(ScriptAction::CAMERA_LOOK_TOWARD_OBJECT)
	V(ScriptAction::CAMERA_LOOK_TOWARD_WAYPOINT) V(ScriptAction::MAP_REVEAL_ALL_PERM) V(ScriptAction::MAP_REVEAL_ALL_UNDO_PERM)
	V(ScriptAction::SKIRMISH_BUILD_BASE_DEFENSE_FRONT) V(ScriptAction::SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST)
	V(ScriptAction::SKIRMISH_BUILD_BASE_DEFENSE_FLANK)
	V(Condition::CONDITION_FALSE) V(Condition::TEAM_INSIDE_AREA_PARTIALLY) V(Condition::TEAM_INSIDE_AREA_ENTIRELY)
	V(Condition::TEAM_OUTSIDE_AREA_ENTIRELY) V(Condition::TEAM_ENTERED_AREA_ENTIRELY) V(Condition::TEAM_ENTERED_AREA_PARTIALLY)
	V(Condition::TEAM_EXITED_AREA_ENTIRELY) V(Condition::TEAM_EXITED_AREA_PARTIALLY) V(Condition::SKIRMISH_SPECIAL_POWER_READY)
	V(Parameter::INT) V(Parameter::REAL) V(Parameter::SCRIPT) V(Parameter::TEAM) V(Parameter::COUNTER) V(Parameter::FLAG)
	V(Parameter::COMPARISON) V(Parameter::WAYPOINT) V(Parameter::BOOLEAN) V(Parameter::TRIGGER_AREA) V(Parameter::TEXT_STRING)
	V(Parameter::SIDE) V(Parameter::SOUND) V(Parameter::SCRIPT_SUBROUTINE) V(Parameter::UNIT) V(Parameter::OBJECT_TYPE)
	V(Parameter::COORD3D) V(Parameter::ANGLE) V(Parameter::TEAM_STATE) V(Parameter::RELATION) V(Parameter::AI_MOOD)
	V(Parameter::DIALOG) V(Parameter::MUSIC) V(Parameter::MOVIE) V(Parameter::WAYPOINT_PATH) V(Parameter::LOCALIZED_TEXT)
	V(Parameter::BRIDGE) V(Parameter::KIND_OF_PARAM) V(Parameter::ATTACK_PRIORITY_SET) V(Parameter::RADAR_EVENT_TYPE)
	V(Parameter::SPECIAL_POWER) V(Parameter::SCIENCE) V(Parameter::UPGRADE) V(Parameter::COMMANDBUTTON_ABILITY)
	V(Parameter::BOUNDARY) V(Parameter::BUILDABLE) V(Parameter::SURFACES_ALLOWED) V(Parameter::SHAKE_INTENSITY)
	V(Parameter::COMMAND_BUTTON) V(Parameter::FONT_NAME) V(Parameter::OBJECT_STATUS) V(Parameter::COMMANDBUTTON_ALL_ABILITIES)
	V(Parameter::SKIRMISH_WAYPOINT_PATH) V(Parameter::COLOR) V(Parameter::EMOTICON) V(Parameter::OBJECT_PANEL_FLAG)
	V(Parameter::FACTION_NAME) V(Parameter::OBJECT_TYPE_LIST) V(Parameter::REVEALNAME) V(Parameter::SCIENCE_AVAILABILITY)
	V(Parameter::LEFT_OR_RIGHT) V(Parameter::PERCENT)
#undef V
	return n;
}

/* mode 0: `data` is a map file whose top level holds a SidesList chunk. mode 1: a
** PlayerScriptsList chunk. `ops` (mode 0) is the script of operations of runOp. The log:
**   'R' u8 | 'T' u32      parse result, or the ErrorCode thrown
**   mode 0: 'S' dump ...  then, per operation: 'o' op, its result bytes, 'S' dump ...
**   mode 1: 'L' u8 count, per list u8 present + the list
**   'W' u32 n bytes      the written file (a 'T' u32 before it if the writer threw)
**   mode 0: 'S' dump (after the write: validateSides ran); mode 1: 'L' lists again
**   'I' u32              ScriptList::getNextID() (taking a number)
** Returns 0. */
int ref_sides_run(int mode, const char *data, int len, const unsigned char *ops, int nops, const char *skirmish, int skLen,
                  unsigned char *log, int cap, int *logLen)
{
	g_log.clear();
	g_skirmish.assign(skirmish, skirmish + skLen);
	delete TheSidesList;
	TheSidesList = new SidesList;
	{
		TestStream stream;
		stream.setData(data, len);
		try {
			DataChunkInput in(&stream);
			if (mode == 0) in.registerParser(AsciiString("SidesList"), AsciiString(""), SidesList::ParseSidesDataChunk);
			else in.registerParser(AsciiString("PlayerScriptsList"), AsciiString(""), ScriptList::ParseScriptsDataChunk);
			Bool r = in.parse(NULL);
			put8('R'); put8(r ? 1 : 0);
		} catch (ErrorCode c) {
			put8('T'); put32((unsigned)c);
		}
	}
	if (mode == 0) {
		dumpState();
		Rd rd = { ops, nops, 0 };
		while (rd.i < rd.n) {
			try {
				runOp(rd);
			} catch (ErrorCode c) {
				put8('T'); put32((unsigned)c);
			}
			dumpState();
		}
		writeChunk([](DataChunkOutput &w) { SidesList::WriteSidesDataChunk(w); });
		dumpState();
	} else {
		ScriptList *lists[MAX_PLAYER_COUNT];
		for (int i = 0; i < MAX_PLAYER_COUNT; ++i) lists[i] = NULL;
		int n = ScriptList::getReadScripts(lists);
		put8('L'); put8(n);
		for (int i = 0; i < n; ++i) { if (lists[i]) { put8(1); dumpScriptList(lists[i]); } else put8(0); }
		writeChunk([&](DataChunkOutput &w) { ScriptList::WriteScriptsDataChunk(w, lists, n); });
		put8('L'); put8(n);
		for (int i = 0; i < n; ++i) { if (lists[i]) { put8(1); dumpScriptList(lists[i]); } else put8(0); }
		for (int i = 0; i < n; ++i) lists[i]->deleteInstance();
	}
	put8('I'); put32((unsigned)ScriptList::getNextID());
	int n = (int)g_log.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(log, &g_log[0], n);
	*logLen = (int)g_log.size();
	return 0;
}

} /* extern "C" */
