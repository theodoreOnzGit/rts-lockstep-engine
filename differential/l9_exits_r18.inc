/* Differential-test driver for the production exits (p1w13-sonnet, batch R18; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/DefaultProductionExitUpdate.cpp
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/QueueProductionExitUpdate.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the producer (id 1: position, orientation and layer, its transform made from
** them, maybe a physics behaviour with a velocity), three objects to put out (ids 2 to 4: each maybe an AI that moves on the ground or not, and a
** physics behaviour with a mass and a centre of mass offset; the contain driver's physics memory, whose applyMotiveForce and setPitchRate log) and a
** budding host (id 5); the height-die driver's terrain (its tables the test's); the pathfinder, whose addObjectToPathfindMap, adjustDestination and
** snapPosition log and answer from the test; the objects' setOrientation and setLayer (lines). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/DefaultProductionExitUpdate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/QueueProductionExitUpdate.cpp"

namespace {
bool g_r18AdjustHas = false;
Coord3D g_r18Adjust;
bool g_r18SnapHas = false;
Coord3D g_r18Snap;
}  // namespace

void AIUpdateInterface::aiFollowExitProductionPath(const std::vector<Coord3D> *path, Object *ignoreObject, CommandSourceType cmdSource)
{
	ulog("aiexitpath %d %d %d %s", (int)m_owner->m_oid, ignoreObject ? (int)ignoreObject->m_oid : 0, (int)cmdSource, ctPath(path).c_str());
}
Bool Pathfinder::adjustDestination(Object *obj, const LocomotorSet *, Coord3D *dest, const Coord3D *)
{
	ulog("adjust %d %08x %08x %08x", (int)obj->m_oid, realBits(dest->x), realBits(dest->y), realBits(dest->z));
	if (!g_r18AdjustHas) return FALSE;
	*dest = g_r18Adjust;
	return TRUE;
}
void Pathfinder::snapPosition(Object *obj, Coord3D *pos)
{
	ulog("snap %d %08x %08x %08x", (int)obj->m_oid, realBits(pos->x), realBits(pos->y), realBits(pos->z));
	if (g_r18SnapHas) *pos = g_r18Snap;
}

namespace {

ModuleData *l9r18Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			if (kind == 0) data = DefaultProductionExitUpdate::friend_newModuleData(&ini);
			else data = QueueProductionExitUpdate::friend_newModuleData(&ini);
		} catch (...) {
			char buff[1024];
			sprintf(buff, "Error parsing INI file '%s' (Line: '%s')\n", ini.m_filename.str(), currentLine.str());
			throw INIException(buff);
		}
	} catch (...) {
		ini.unPrepFile();
		throw;
	}
	ini.unPrepFile();
	return data;
}

std::vector<unsigned char> g_r18Saved;
Object *g_r18Objs[5];
Object *r18Find(ObjectID id) { return (id >= 1 && id <= 5) ? g_r18Objs[id - 1] : 0; }
void r18PutCoord(const Coord3D &c) { putU32(realBits(c.x)); putU32(realBits(c.y)); putU32(realBits(c.z)); }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a DefaultProductionExitUpdate (`kind` 0) or a QueueProductionExitUpdate (1) and make the module on the
** producer (id 1). `cfg` (16 words): [0-1 the producer's x and y, 2 its orientation (float bits), 3 its layer, 4 flags (bits 0-2 objects 2 to 4 have
** an AI, 3-5 it moves on the ground, 6-8 objects 2 to 4 have physics, 9 the producer has physics), 5 the producer's velocity z (float bits; x 2,
** y -1), 6 the objects' mass, 7 their centre of mass offset, 8 adjustDestination answers (bit 0) with (9, 10) z 0, 11 snapPosition answers (bit 0)
** with (12, 13) z 0, 14 the ground height and 15 the layer height (every tile; float bits)]. The data dump goes first. Then `n` ops of 6 words
** [op, a, b, c, d, e]: 0 exitObjectViaDoor(object a)  1 reserveDoorForExit(NULL, NULL)  2 unreserveDoorForExit(DOOR_1)  3 exitObjectByBudding
** (object a, host b: 0 none)  4 setRallyPoint(a, b, c)  5 getRallyPoint  6 useSpawnRallyPoint (kind 0)  7 getNaturalRallyPoint(offset a)
** 8 getExitPosition  9 update()  10 isExitBusy  11 flags = a  12 xfer save  13 xfer load of the last save  14 xfer CRC. Each op appends: the
** CATCH_ALL status, the result (count, then words), the effect log (count, then each), the xfer status, bytes and CRC, the module's state, then
** objects 2 to 4's positions. */
int ref_l9r18_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r18Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r18Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	if (kind == 0) {
		const DefaultProductionExitUpdateModuleData *md = (const DefaultProductionExitUpdateModuleData *)d;
		r18PutCoord(md->m_unitCreatePoint);
		r18PutCoord(md->m_naturalRallyPoint);
		putU32(md->m_useSpawnRallyPoint ? 1 : 0);
	} else {
		const QueueProductionExitUpdateModuleData *md = (const QueueProductionExitUpdateModuleData *)d;
		r18PutCoord(md->m_unitCreatePoint);
		r18PutCoord(md->m_naturalRallyPoint);
		putU32(md->m_exitDelayData);
		putU32(md->m_allowAirborneCreationData ? 1 : 0);
		putU32(md->m_initialBurst);
	}
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	TheTerrainLogic = &g_hdTerrain;
	for (int i = 0; i < 8; ++i) { g_hdGround[i] = ctF(cfg[14]); g_hdLayerHeight[i] = ctF(cfg[15]); }
	g_objXformFromPose = true;
	unsigned flags = cfg[4];
	AIUpdateInterface ais[5];
	g_ctN = 5;
	g_ctMass.clear();
	unsigned char physData[5][sizeof(PhysicsBehaviorModuleData) + 16];
	for (int k = 0; k < 5; ++k) {
		g_r18Objs[k] = newObject();
		g_r18Objs[k]->m_oid = (ObjectID)(k + 1);
		ais[k].m_owner = g_r18Objs[k];
		memset(g_ctPhysMem[k], 0, sizeof(g_ctPhysMem[k]));
		PhysicsBehavior *phys = (PhysicsBehavior *)g_ctPhysMem[k];
		memset(physData[k], 0, sizeof(physData[k]));
		((PhysicsBehaviorModuleData *)physData[k])->m_centerOfMassOffset = ctF(cfg[7]);
		phys->m_moduleData = (const ModuleData *)physData[k];
		g_ctMass[phys] = ctF(cfg[6]);
	}
	Object *me = g_r18Objs[0], *host = g_r18Objs[4];
	me->m_pos.x = ctF(cfg[0]);
	me->m_pos.y = ctF(cfg[1]);
	me->m_orientation = ctF(cfg[2]);
	me->m_layer = (PathfindLayerEnum)cfg[3];
	PhysicsBehavior *myPhys = (PhysicsBehavior *)g_ctPhysMem[0];
	myPhys->m_vel.x = 2.0f; myPhys->m_vel.y = -1.0f; myPhys->m_vel.z = ctF(cfg[5]);
	host->m_pos.x = -7.5f; host->m_pos.y = 12.0f; host->m_pos.z = 3.0f;
	host->m_orientation = 0.5f;
	host->m_layer = (PathfindLayerEnum)2;
	g_r18AdjustHas = (cfg[8] & 1) != 0; g_r18Adjust.x = ctF(cfg[9]); g_r18Adjust.y = ctF(cfg[10]); g_r18Adjust.z = 0;
	g_r18SnapHas = (cfg[11] & 1) != 0; g_r18Snap.x = ctF(cfg[12]); g_r18Snap.y = ctF(cfg[13]); g_r18Snap.z = 0;
	g_modFindHook = r18Find;
	gl.m_awakenCalls = 0;
	UpdateModule *m = kind == 0 ? (UpdateModule *)new DefaultProductionExitUpdate(me, d) : (UpdateModule *)new QueueProductionExitUpdate(me, d);
	/* GameLogic::friend_awakenUpdateModule for an object not yet in the list (GameLogic.cpp:3035-3046): the wake frame is the next call frame; a
	** module that sets none (QueueProductionExitUpdate) is called from the next frame, as the Rust world schedules it when it makes the object */
	m->friend_setNextCallFrame(gl.m_awakenCalls ? gl.m_awakenFrame : 1);
	ExitInterface *ex = m->getUpdateExitInterface();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 11) flags = o[1];
		for (int k = 1; k < 4; ++k) {
			g_r18Objs[k]->m_ai = (flags >> (k - 1) & 1) ? &ais[k] : 0;
			ais[k].m_groundMovement = (flags >> (k + 2) & 1) != 0;
			g_r18Objs[k]->m_physics = (flags >> (k + 5) & 1) ? (PhysicsBehavior *)g_ctPhysMem[k] : 0;
		}
		me->m_physics = (flags & 512) ? myPhys : 0;
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		std::vector<unsigned> res;
		try {
			switch (o[0]) {
			case 0: ex->exitObjectViaDoor(r18Find((ObjectID)o[1]), DOOR_1); break;
			case 1: res.push_back((unsigned)ex->reserveDoorForExit(0, 0)); break;
			case 2: ex->unreserveDoorForExit(DOOR_1); break;
			case 3: ex->exitObjectByBudding(r18Find((ObjectID)o[1]), o[2] ? r18Find((ObjectID)o[2]) : 0); break;
			case 4: { Coord3D c; c.x = ctF(o[1]); c.y = ctF(o[2]); c.z = ctF(o[3]); ex->setRallyPoint(&c); break; }
			case 5: { const Coord3D *c = ex->getRallyPoint(); if (c) { res.push_back(realBits(c->x)); res.push_back(realBits(c->y)); res.push_back(realBits(c->z)); } break; }
			case 6: res.push_back(ex->useSpawnRallyPoint() ? 1 : 0); break;
			case 7: { Coord3D c; res.push_back(ex->getNaturalRallyPoint(c, o[1] != 0) ? 1 : 0); res.push_back(realBits(c.x)); res.push_back(realBits(c.y)); res.push_back(realBits(c.z)); break; }
			case 8: { Coord3D c; res.push_back(ex->getExitPosition(c) ? 1 : 0); res.push_back(realBits(c.x)); res.push_back(realBits(c.y)); res.push_back(realBits(c.z)); break; }
			case 9: res.push_back((unsigned)m->update()); break;
			case 10: res.push_back(ex->isExitBusy() ? 1 : 0); break;
			case 12: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r18Saved.assign(bytes, bytes + blen); break;
			case 13: {
				std::vector<unsigned char> b = g_r18Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 14: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32((unsigned)res.size());
		for (size_t j = 0; j < res.size(); ++j) putU32(res[j]);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		if (kind == 0) {
			DefaultProductionExitUpdate *dm = (DefaultProductionExitUpdate *)m;
			r18PutCoord(dm->m_rallyPoint);
			putU32(dm->m_rallyPointExists ? 1 : 0);
		} else {
			QueueProductionExitUpdate *qm = (QueueProductionExitUpdate *)m;
			putU32(qm->m_currentDelay);
			r18PutCoord(qm->m_rallyPoint);
			putU32(qm->m_rallyPointExists ? 1 : 0);
			putU32(realBits(qm->m_creationClearDistance));
			putU32(qm->m_currentBurstCount);
		}
		for (int k = 1; k < 4; ++k) r18PutCoord(g_r18Objs[k]->m_pos);
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_objXformFromPose = false;
	g_ctN = 0;
	g_ctMass.clear();
	TheTerrainLogic = 0;
	delete m;
	delete d;
	for (int k = 0; k < 5; ++k) { g_r18Objs[k]->m_ai = 0; g_r18Objs[k]->m_physics = 0; delete g_r18Objs[k]; }
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
