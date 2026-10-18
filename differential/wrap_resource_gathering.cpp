/* Differential-test wrapper for ResourceGatheringManager (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/ResourceGatheringManager.cpp
** (#included below, unchanged), with the stand-ins of differential/shim_resource for the objects of a test world
** (their AI, the supply-truck interface, the dock interface, the update modules by name), TheGameLogic, the
** partition manager's distance (FROM_CENTER_3D, the one case the file uses), ActionManager's
** canTransferSuppliesAt and the name keys of the two modules it looks up. Every stand-in call is recorded.
** The stand-ins' class names are renamed below, so that no symbol of this archive clashes with another fixture's.
**
** Entry point (extern "C", tests/differential_resource_gathering.rs): ref_resource_run runs a program of
** operations on a world of 8 objects and one manager and writes what each operation observes as text lines; the
** Rust interpreter writes the same lines for the same program. */
#define Object RgmTestObject
#define GameLogic RgmTestGameLogic
#define TheGameLogic RgmTestTheGameLogic
#define PartitionManager RgmTestPartitionManager
#define ThePartitionManager RgmTestThePartitionManager
#define ActionManager RgmTestActionManager
#define TheActionManager RgmTestTheActionManager
#define UpdateModule RgmTestUpdateModule
#define SupplyWarehouseDockUpdate RgmTestSupplyWarehouseDockUpdate
#define DockUpdateInterface RgmTestDockUpdateInterface
#define SupplyTruckAIInterface RgmTestSupplyTruckAIInterface
#define AIUpdateInterface RgmTestAIUpdateInterface
#define DistanceCalculationType RgmTestDistanceCalculationType
#define rgmNameKey RgmTestNameKey

#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <stdio.h>
#include <unistd.h>
#include <float.h>
#include <string>
#include <vector>
#include <list>
#include <algorithm>
#include "PreRTS.h"
#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
/* the real header, with its members readable (the two lists are read by the state records) */
#define private public
#define protected public
#include "Common/ResourceGatheringManager.h"
#undef private
#undef protected
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/ResourceGatheringManager.cpp"

/* ---- stand-ins ----------------------------------------------------------- */
static std::vector<std::string> g_log;
static void logf_(const char *fmt, ...)
{
	char b[256];
	va_list ap; va_start(ap, fmt); vsnprintf(b, sizeof b, fmt, ap); va_end(ap);
	g_log.push_back(b);
}
static unsigned fbits(float f) { unsigned v; memcpy(&v, &f, 4); return v; }

static Object g_objects[9];
static GameLogic g_gameLogic;
static PartitionManager g_partition;
static ActionManager g_action;
static unsigned long long g_transfer[9]; /* g_transfer[query] bit (dest - 1) */
static ObjectID g_queryInProgress = INVALID_ID; /* the docker of isClearToApproach: the dock does not know its owner */
static Object *g_dockOwner[9];
GameLogic *TheGameLogic = &g_gameLogic;
PartitionManager *ThePartitionManager = &g_partition;
ActionManager *TheActionManager = &g_action;

Object *GameLogic::findObjectByID(ObjectID id)
{
	if ((int)id < 1 || (int)id > 8) return NULL;
	return g_objects[(int)id].m_exists ? &g_objects[(int)id] : NULL;
}
Bool ActionManager::canTransferSuppliesAt(const Object *obj, const Object *transferObj)
{
	Bool r = (g_transfer[(int)obj->m_id] >> ((int)transferObj->m_id - 1)) & 1u;
	logf_("rgm canTransfer %d %d -> %d", (int)obj->m_id, (int)transferObj->m_id, r ? 1 : 0);
	return r;
}
Bool DockUpdateInterface::isClearToApproach(Object const *docker) const
{
	/* the owner of this dock interface: the one object whose storage it is */
	int owner = 0;
	for (int i = 1; i <= 8; ++i) if (&g_objects[i].m_dockStorage == this) owner = i;
	logf_("rgm clear %d %d -> %d", owner, (int)docker->m_id, m_clear ? 1 : 0);
	return m_clear;
}
Real PartitionManager::getDistanceSquared(const Object *obj, const Object *other, DistanceCalculationType dc, void *vec) const
{
	/* FROM_CENTER_3D, the only kind the file asks for: theDistCalcProcs[FROM_CENTER_3D] of PartitionManager.cpp,
	** verified against the real one by differential_partition.rs */
	Real dx = other->m_pos[0] - obj->m_pos[0], dy = other->m_pos[1] - obj->m_pos[1], dz = other->m_pos[2] - obj->m_pos[2];
	Real d = dx * dx + dy * dy + dz * dz;
	logf_("rgm dist %d %d -> %08x", (int)obj->m_id, (int)other->m_id, fbits(d));
	return d;
}

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
void flushLog() { for (size_t i = 0; i < g_log.size(); ++i) line("%s", g_log[i].c_str()); g_log.clear(); }
Object *objOf(int id) { return id == 0 ? NULL : &g_objects[id]; }

void managerState(const char *name, ResourceGatheringManager *m)
{
	std::string a, c;
	char buf[32];
	for (std::list<ObjectID>::const_iterator it = m->m_supplyWarehouses.begin(); it != m->m_supplyWarehouses.end(); ++it) { snprintf(buf, sizeof buf, " %d", (int)*it); a += buf; }
	for (std::list<ObjectID>::const_iterator it = m->m_supplyCenters.begin(); it != m->m_supplyCenters.end(); ++it) { snprintf(buf, sizeof buf, " %d", (int)*it); c += buf; }
	line("state %s warehouses[%s] centers[%s]", name, a.c_str(), c.c_str());
}

std::string tmpPath() { char b[64]; snprintf(b, sizeof b, "/tmp/rgm_ref_%d.xfer", (int)getpid()); return b; }
unsigned hashFile(const std::string &path, size_t *len)
{
	unsigned h = 2166136261u; size_t n = 0;
	FILE *f = fopen(path.c_str(), "rb");
	if (f) { unsigned char c; char b[4]; while (fread(&c, 1, 1, f) == 1) { snprintf(b, sizeof b, "%02x", c); h ^= (unsigned char)b[0]; h *= 16777619u; h ^= (unsigned char)b[1]; h *= 16777619u; ++n; } fclose(f); }
	*len = n; return h;
}

}  // namespace

extern "C" {

/* Program words: for each of the 8 objects: exists, flags (1 AI, 2 supply-truck AI, 4 dock interface, 8 warehouse
** dock module, 16 center dock module, 32 dock clear to approach), preferred dock id, warehouse scan distance (bits),
** x, y, z (bits); then 8 words, the supply-transfer table (row = querying object, bit j-1 = may transfer at object j);
** then operations ended by 99 (see tests/differential_resource_gathering.rs). */
int ref_resource_run(const unsigned *prog, int n_prog, char *out, int cap)
{
	g_out.clear(); g_log.clear();
	Reader r = { prog, n_prog, 0 };
	for (int i = 0; i <= 8; ++i) g_objects[i] = Object();
	for (int i = 1; i <= 8; ++i)
	{
		Object &o = g_objects[i];
		o.m_id = (ObjectID)i;
		o.m_exists = r.u() != 0;
		unsigned fl = r.u();
		o.m_ai = (fl & 1) ? &o.m_aiStorage : NULL;
		o.m_aiStorage.m_truck = (fl & 2) ? &o.m_truckStorage : NULL;
		o.m_dock = (fl & 4) ? &o.m_dockStorage : NULL;
		o.m_hasWarehouseUpdate = (fl & 8) != 0;
		o.m_hasCenterUpdate = (fl & 16) != 0;
		o.m_dockStorage.m_clear = (fl & 32) != 0;
		o.m_truckStorage.m_preferred = (ObjectID)r.i();
		o.m_truckStorage.m_scan = r.f();
		o.m_pos[0] = r.f(); o.m_pos[1] = r.f(); o.m_pos[2] = r.f();
	}
	for (int i = 1; i <= 8; ++i) g_transfer[i] = r.u();
	ResourceGatheringManager *m = new ResourceGatheringManager();
	managerState("M", m);
	for (;;)
	{
		int op = r.i();
		if (op == 99 || r.at > r.n) break;
		switch (op)
		{
		case 1: { int id = r.i(); line("op addCenter"); m->addSupplyCenter(objOf(id)); break; }
		case 2: { int id = r.i(); line("op removeCenter"); m->removeSupplyCenter(objOf(id)); break; }
		case 3: { int id = r.i(); line("op addWarehouse"); m->addSupplyWarehouse(objOf(id)); break; }
		case 4: { int id = r.i(); line("op removeWarehouse"); m->removeSupplyWarehouse(objOf(id)); break; }
		case 5: { int id = r.i(); line("op findWarehouse"); Object *o = m->findBestSupplyWarehouse(objOf(id)); flushLog(); line("ret %d", o ? (int)o->m_id : 0); break; }
		case 6: { int id = r.i(); line("op findCenter"); Object *o = m->findBestSupplyCenter(objOf(id)); flushLog(); line("ret %d", o ? (int)o->m_id : 0); break; }
		case 7: { /* change an object: id, field, value */
			int id = r.i(), field = r.i(); unsigned v = r.u(); Object &o = g_objects[id]; line("op mark");
			switch (field)
			{
			case 0: o.m_exists = v != 0; break;
			case 1: o.m_dockStorage.m_clear = v != 0; break;
			case 2: o.m_truckStorage.m_preferred = (ObjectID)v; break;
			case 3: { float f; memcpy(&f, &v, 4); o.m_truckStorage.m_scan = f; break; }
			case 4: { float f; memcpy(&f, &v, 4); o.m_pos[0] = f; break; }
			case 5: { float f; memcpy(&f, &v, 4); o.m_pos[1] = f; break; }
			case 6: g_transfer[id] = (unsigned long long)v; break;
			}
			break; }
		case 8: { /* save, load into a fresh manager, post-process */
			line("op saveLoad");
			std::string path = tmpPath();
			int st = 0; size_t len = 0; unsigned h = 0;
			{ XferSave x; try { x.open(AsciiString(path.c_str())); x.xferSnapshot(m); x.close(); } catch (XferStatus s) { st = (int)s + 1; try { x.close(); } catch (...) {} } }
			h = hashFile(path, &len);
			line("ret save %d len %u hash %08x", st, (unsigned)len, h);
			ResourceGatheringManager *fresh = new ResourceGatheringManager();
			int lst = 0;
			{ XferLoad x; try { x.open(AsciiString(path.c_str())); x.xferSnapshot(fresh); x.close(); } catch (XferStatus s) { lst = (int)s + 1; try { x.close(); } catch (...) {} } }
			line("ret load %d", lst);
			managerState("loaded", fresh);
			fresh->loadPostProcess();
			fresh->crc(NULL);
			delete fresh;
			break; }
		default: line("bad op %d", op); break;
		}
		flushLog();
		managerState("M", m);
	}
	delete m;
	int need = (int)g_out.size();
	if (need > cap) return -need;
	memcpy(out, g_out.data(), g_out.size());
	return need;
}

}  /* extern "C" */
