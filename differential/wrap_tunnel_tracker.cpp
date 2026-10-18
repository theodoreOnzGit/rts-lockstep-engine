/* Differential-test wrapper for TunnelTracker and CaveSystem (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/TunnelTracker.cpp
**   GeneralsMD/Code/GameEngine/Source/GameLogic/System/CaveSystem.cpp
** (#included below, unchanged), with the stand-ins of differential/shim_tunnel for the objects of a test world,
** TheGameLogic, the partition manager, the pathfinder, the drawable and the body module. The stand-ins'
** class names are renamed below, so that no symbol of this archive clashes with another fixture's.
**
** Entry point (extern "C", tests/differential_tunnel_tracker.rs): ref_tunnel_run runs a program of operations
** on one standalone tracker and a cave system, and writes what each operation observes as text lines; the Rust
** interpreter writes the same lines for the same program. The words of a program are listed in the test. */
#define Object TunnelTestObject
#define GameLogic TunnelTestGameLogic
#define TheGameLogic TunnelTestTheGameLogic
#define GlobalData TunnelTestGlobalData
#define TheGlobalData TunnelTestTheGlobalData
#define Drawable TunnelTestDrawable
#define BodyModuleInterface TunnelTestBodyModule
#define PartitionManager TunnelTestPartitionManager
#define ThePartitionManager TunnelTestThePartitionManager
#define Pathfinder TunnelTestPathfinder
#define AI TunnelTestAI
#define TheAI TunnelTestTheAI
#define DamageInfo TunnelTestDamageInfo
#define DamageInfoInput TunnelTestDamageInfoInput
#define DamageInfoOutput TunnelTestDamageInfoOutput
#define DamageType TunnelTestDamageType
#define DeathType TunnelTestDeathType
#define g_tunnelLog TunnelTestLog
#define tunnelLogf TunnelTestLogf
#define TheCaveSystem TunnelTestTheCaveSystem

#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <stdio.h>
#include <unistd.h>
#include <string>
#include <vector>
#include <list>
#include <algorithm>
#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
/* the real headers, with their members readable (the lists and counters are read by the state records, and
** loadPostProcess, which the game state calls, is protected) */
#define private public
#define protected public
#include "Common/TunnelTracker.h"
#include "GameLogic/CaveSystem.h"
#undef private
#undef protected
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/TunnelTracker.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/System/CaveSystem.cpp"

/* ---- stand-ins ----------------------------------------------------------- */
std::vector<std::string> g_tunnelLog;
void tunnelLogf(const char *fmt, ...)
{
	char b[256];
	va_list ap; va_start(ap, fmt); vsnprintf(b, sizeof b, fmt, ap); va_end(ap);
	g_tunnelLog.push_back(b);
}

static Object g_objects[9];       /* ids 1..8 */
static GameLogic g_gameLogic;
static GlobalData g_globalData;
static PartitionManager g_partition;
static AI g_ai;
GameLogic *TheGameLogic = &g_gameLogic;
GlobalData *TheGlobalData = &g_globalData;
PartitionManager *ThePartitionManager = &g_partition;
AI *TheAI = &g_ai;

Object *GameLogic::findObjectByID(ObjectID id)
{
	if ((int)id < 1 || (int)id > 8) return NULL;
	return g_objects[(int)id].m_exists ? &g_objects[(int)id] : NULL;
}
void GameLogic::destroyObject(Object *obj) { tunnelLogf("tt destroy %d", (int)obj->m_id); obj->m_destroyed = true; }
void Object::onContainedBy(Object *c) { tunnelLogf("tt onContainedBy %d %d", (int)m_id, c ? (int)c->m_id : 0); m_containedBy = c; m_frame = TheGameLogic->getFrame(); }
void Object::onRemovedFrom(Object *r) { tunnelLogf("tt onRemovedFrom %d %d", (int)m_id, r ? (int)r->m_id : 0); m_containedBy = NULL; m_frame = 0; }
void Object::leaveGroup() { if (m_group != 0) { tunnelLogf("tt leaveGroup %d", (int)m_id); m_group = 0; } }
void Drawable::setDrawableHidden(Bool hidden) { tunnelLogf("tt hide %d %d", m_owner, hidden ? 1 : 0); g_objects[m_owner].m_hidden = hidden; }
void BodyModuleInterface::attemptHealing(DamageInfo *info)
{
	unsigned bits; memcpy(&bits, &info->in.m_amount, sizeof bits);
	tunnelLogf("tt heal %d src %d type %d fx %d death %d amount %08x kill %d", m_owner, (int)info->in.m_sourceID, (int)info->in.m_damageType,
		(int)info->in.m_damageFXOverride, (int)info->in.m_deathType, bits, info->in.m_kill ? 1 : 0);
	m_health += info->in.m_amount;
	if (m_health > m_maxHealth) m_health = m_maxHealth;
}
void PartitionManager::unRegisterObject(Object *obj) { tunnelLogf("tt unregister %d", (int)obj->m_id); }
void Pathfinder::removeObjectFromPathfindMap(Object *obj) { tunnelLogf("tt pathfind %d", (int)obj->m_id); }

/* ---- the driver ------------------------------------------------------------ */
namespace {

struct Reader { const unsigned *w; int n; int at; unsigned u() { return at < n ? w[at++] : 0xFFFFFFFFu; } int i() { return (int)u(); } float f() { unsigned v = u(); float x; memcpy(&x, &v, 4); return x; } };

std::string g_out;
void line(const char *fmt, ...)
{
	char b[1024];
	va_list ap; va_start(ap, fmt); vsnprintf(b, sizeof b, fmt, ap); va_end(ap);
	g_out += b; g_out += '\n';
}
unsigned fbits(float f) { unsigned v; memcpy(&v, &f, 4); return v; }

TunnelTracker *g_t = NULL;
CaveSystem *g_cave = NULL;

TunnelTracker *pick(int which) { return which == 0 ? g_t : g_cave->getTunnelTrackerForCaveIndex(which - 1); }
Object *objOf(int id) { return id == 0 ? NULL : &g_objects[id]; }

void flushLog()
{
	for (size_t i = 0; i < g_tunnelLog.size(); ++i) line("%s", g_tunnelLog[i].c_str());
	g_tunnelLog.clear();
}

void trackerState(const char *name, TunnelTracker *t)
{
	if (!t) { line("state %s none", name); return; }
	std::string a, b;
	char buf[32];
	for (std::list<ObjectID>::const_iterator it = t->m_tunnelIDs.begin(); it != t->m_tunnelIDs.end(); ++it) { snprintf(buf, sizeof buf, " %d", (int)*it); a += buf; }
	for (ContainedItemsList::const_iterator it = t->m_containList.begin(); it != t->m_containList.end(); ++it) { snprintf(buf, sizeof buf, " %d", (int)(*it)->m_id); b += buf; }
	std::string c;
	for (std::list<ObjectID>::const_iterator it = t->m_xferContainList.begin(); it != t->m_xferContainList.end(); ++it) { snprintf(buf, sizeof buf, " %d", (int)*it); c += buf; }
	line("state %s tunnels[%s] list[%s] pending[%s] size %d count %u nemesis %d ts %u", name, a.c_str(), b.c_str(), c.c_str(), (int)t->m_containListSize,
		(unsigned)t->m_tunnelCount, (int)t->m_curNemesisID, (unsigned)t->m_nemesisTimestamp);
}

void worldState()
{
	trackerState("T", g_t);
	std::string cave;
	char buf[64];
	for (size_t i = 0; i < g_cave->m_tunnelTrackerVector.size(); ++i)
	{
		TunnelTracker *t = g_cave->m_tunnelTrackerVector[i];
		if (!t) cave += " -"; else { snprintf(buf, sizeof buf, " %d", (int)t->m_containListSize); cave += buf; }
	}
	line("state cave[%s]", cave.c_str());
	for (size_t i = 0; i < g_cave->m_tunnelTrackerVector.size(); ++i)
	{
		char nm[16]; snprintf(nm, sizeof nm, "C%d", (int)i);
		trackerState(nm, g_cave->m_tunnelTrackerVector[i]);
	}
	for (int i = 1; i <= 8; ++i)
	{
		Object &o = g_objects[i];
		line("obj %d exists %d by %d frame %u dead %d destroyed %d hidden %d group %u health %08x", i, o.m_exists ? 1 : 0, o.m_containedBy ? (int)o.m_containedBy->m_id : 0,
			(unsigned)o.m_frame, o.m_effectivelyDead ? 1 : 0, o.m_destroyed ? 1 : 0, o.m_hidden ? 1 : 0, (unsigned)o.m_group, fbits(o.m_body.m_health));
	}
}

std::string tmpPath()
{
	char b[64]; snprintf(b, sizeof b, "/tmp/tunnel_ref_%d.xfer", (int)getpid());
	return b;
}
/* the bytes of a file, as hex */
std::string fileHex(const std::string &path, size_t *len)
{
	std::string out;
	FILE *f = fopen(path.c_str(), "rb");
	if (!f) { *len = 0; return out; }
	unsigned char c; size_t n = 0; char b[4];
	while (fread(&c, 1, 1, f) == 1) { snprintf(b, sizeof b, "%02x", c); out += b; ++n; }
	fclose(f);
	*len = n;
	return out;
}
unsigned hashHex(const std::string &s) { unsigned h = 2166136261u; for (size_t i = 0; i < s.size(); ++i) { h ^= (unsigned char)s[i]; h *= 16777619u; } return h; }

/* XferSave of a snapshot to the temp file: status (0 ok, else XferStatus + 1); the file's length and hash */
int saveSnapshot(Snapshot *s, size_t *len, unsigned *hash)
{
	std::string path = tmpPath();
	XferSave x;
	int status = 0;
	try { x.open(AsciiString(path.c_str())); x.xferSnapshot(s); x.close(); }
	catch (XferStatus st) { status = (int)st + 1; try { x.close(); } catch (...) {} }
	std::string hex = fileHex(path, len);
	*hash = hashHex(hex);
	return status;
}
int loadSnapshot(Snapshot *s)
{
	std::string path = tmpPath();
	XferLoad x;
	int status = 0;
	try { x.open(AsciiString(path.c_str())); x.xferSnapshot(s); x.close(); }
	catch (XferStatus st) { status = (int)st + 1; try { x.close(); } catch (...) {} }
	return status;
}

}  // namespace

extern "C" {

/* Program words: [0] max tunnel capacity, [1] the frame, then for each of the 8 objects: kinds (bit per KindOfType),
** status (1 stealthed, 2 detected, 4 disguised), flags (1 exists, 2 effectively dead, 4 drawable, 8 body, 16 in a
** group), max health (bits), health (bits), contained-by id (0 none), contained-by frame; then operations, each an
** opcode and its arguments, ended by 99 (see tests/differential_tunnel_tracker.rs). The output text (one line per
** record) is written to `out`; the return value is its length, or minus the length needed if `cap` is too small. */
int ref_tunnel_run(const unsigned *prog, int n_prog, char *out, int cap)
{
	g_out.clear();
	g_tunnelLog.clear();
	Reader r = { prog, n_prog, 0 };
	for (int i = 0; i <= 8; ++i) g_objects[i] = Object();
	g_globalData.m_maxTunnelCapacity = r.i();
	g_gameLogic.m_frame = r.u();
	for (int i = 1; i <= 8; ++i)
	{
		Object &o = g_objects[i];
		o.m_id = (ObjectID)i; o.m_kinds = r.u(); o.m_status = r.u();
		unsigned flags = r.u();
		o.m_exists = (flags & 1) != 0; o.m_effectivelyDead = (flags & 2) != 0; o.m_hasDrawable = (flags & 4) != 0; o.m_hasBody = (flags & 8) != 0;
		o.m_group = (flags & 16) ? 1u : 0u;
		o.m_body.m_maxHealth = r.f(); o.m_body.m_health = r.f();
		int by = r.i(); o.m_containedBy = by ? &g_objects[by] : NULL; o.m_frame = r.u();
		o.m_drawable.m_owner = i; o.m_body.m_owner = i;
	}
	g_t = new TunnelTracker();
	g_cave = new CaveSystem();
	worldState();
	for (;;)
	{
		int op = r.i();
		if (op == 99 || r.at > r.n) break;
		switch (op)
		{
		case 1: { g_gameLogic.m_frame = r.u(); line("op frame"); break; }
		case 2: { int w = r.i(), id = r.i(); line("op add"); pick(w)->addToContainList(objOf(id)); break; }
		case 3: { int w = r.i(), id = r.i(); line("op remove"); pick(w)->removeFromContain(objOf(id), r.u() != 0); break; }
		case 4: { int w = r.i(), id = r.i(); line("op isIn"); line("ret %d", pick(w)->isInContainer(objOf(id)) ? 1 : 0); break; }
		case 5: { int w = r.i(), id = r.i(); int chk = r.i(); line("op isValid"); line("ret %d", pick(w)->isValidContainerFor(objOf(id), chk != 0) ? 1 : 0); break; }
		case 6: { int w = r.i(), id = r.i(); line("op tunnelCreated"); pick(w)->onTunnelCreated(objOf(id)); break; }
		case 7: { int w = r.i(), id = r.i(); line("op tunnelDestroyed"); pick(w)->onTunnelDestroyed(objOf(id)); break; }
		case 8: { int w = r.i(); float fr = r.f(); line("op heal"); pick(w)->healObjects(fr); break; }
		case 9: { int w = r.i(), id = r.i(); line("op updateNemesis"); pick(w)->updateNemesis(objOf(id)); break; }
		case 10: { int w = r.i(); line("op getNemesis"); Object *o = pick(w)->getCurNemesis(); line("ret %d", o ? (int)o->m_id : 0); break; }
		case 11: { g_globalData.m_maxTunnelCapacity = r.i(); line("op capacity"); break; }
		case 12: { /* mutate an object: field, value */
			int id = r.i(), field = r.i(), v = r.i(); Object &o = g_objects[id]; line("op mark");
			switch (field)
			{
			case 0: o.m_effectivelyDead = v != 0; break;
			case 1: o.m_status = (o.m_status & ~1u) | (v ? 1u : 0u); break;
			case 2: o.m_status = (o.m_status & ~2u) | (v ? 2u : 0u); break;
			case 3: o.m_status = (o.m_status & ~4u) | (v ? 4u : 0u); break;
			case 4: o.m_exists = v != 0; break;
			case 5: o.m_containedBy = v ? &g_objects[v] : NULL; break;
			case 6: o.m_group = (unsigned)v; break;
			}
			break; }
		case 13: { /* save the standalone tracker, load it into a fresh one, post-process */
			int w = r.i(); line("op saveLoad");
			TunnelTracker *t = pick(w);
			size_t len; unsigned h; int st = saveSnapshot(t, &len, &h);
			line("ret save %d len %u hash %08x", st, (unsigned)len, h);
			TunnelTracker *fresh = new TunnelTracker();
			int lst = loadSnapshot(fresh);
			line("ret load %d", lst);
			trackerState("loaded", fresh);
			int pst = 0;
			try { fresh->loadPostProcess(); } catch (...) { pst = 1; }
			line("ret post %d", pst);
			trackerState("post", fresh);
			flushLog();
			delete fresh;
			break; }
		case 20: { int idx = r.i(); line("op caveRegister"); g_cave->registerNewCave(idx); break; }
		case 21: { int a = r.i(), b = r.i(); line("op caveCanSwitch"); line("ret %d", g_cave->canSwitchIndexToIndex(a, b) ? 1 : 0); break; }
		case 22: { int idx = r.i(); line("op caveGet"); TunnelTracker *t = g_cave->getTunnelTrackerForCaveIndex(idx); line("ret %d", t ? 1 : 0); break; }
		case 23: { int idx = r.i(); line("op caveUnregister"); g_cave->unregisterCave(idx); break; }
		case 24: { line("op caveReset"); g_cave->reset(); break; }
		case 25: { /* save the cave system, load into a fresh one, post-process each tracker */
			line("op caveSaveLoad");
			size_t len; unsigned h; int st = saveSnapshot(g_cave, &len, &h);
			line("ret save %d len %u hash %08x", st, (unsigned)len, h);
			CaveSystem *fresh = new CaveSystem();
			int lst = loadSnapshot(fresh);
			line("ret load %d", lst);
			std::string cs;
			char buf[32];
			for (size_t i = 0; i < fresh->m_tunnelTrackerVector.size(); ++i)
			{
				TunnelTracker *t = fresh->m_tunnelTrackerVector[i];
				if (!t) cs += " -"; else { snprintf(buf, sizeof buf, " %d", (int)t->m_containListSize); cs += buf; }
			}
			line("ret loaded cave[%s]", cs.c_str());
			for (size_t i = 0; i < fresh->m_tunnelTrackerVector.size(); ++i)
			{
				char nm[16]; snprintf(nm, sizeof nm, "L%d", (int)i);
				trackerState(nm, fresh->m_tunnelTrackerVector[i]);
				int pst = 0;
				try { if (fresh->m_tunnelTrackerVector[i]) fresh->m_tunnelTrackerVector[i]->loadPostProcess(); } catch (...) { pst = 1; }
				line("ret post %d", pst);
			}
			flushLog();
			fresh->reset();
			delete fresh;
			break; }
		default: line("bad op %d", op); break;
		}
		flushLog();
		worldState();
	}
	g_cave->reset();
	delete g_cave; g_cave = NULL;
	delete g_t; g_t = NULL;
	int need = (int)g_out.size();
	if (need > cap) return -need;
	memcpy(out, g_out.data(), g_out.size());
	return need;
}

}  /* extern "C" */
