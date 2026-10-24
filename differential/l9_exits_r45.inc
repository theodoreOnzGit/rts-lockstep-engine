/* Differential-test driver for the last two production exits (p1w13-sonnet, batch R45; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/SupplyCenterProductionExitUpdate.cpp (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/SpawnPointProductionExitUpdate.cpp   (edited by build.rs for
**                                                                                                                          MSVC 6's for-loop scoping)
** on batch R45's scene (differential/l9_exits_r45.inc, whose AI, pathfinder and terrain stand-ins this reuses), plus: the producer's drawable
** (maybe) with the test's spawn bones (positions and transforms), its STEALTHED bit; each object put out maybe a supply truck's AI (batch
** R44's truck), a stealth module (whether its grant is temporary) and CAN_STEALTH; setDisabled (the ActiveBody driver's line); and the
** objects the test removes (no object to findObjectByID).
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the producer (id 1: position, orientation and layer, its transform made from
** them, maybe a physics behaviour with a velocity), three objects to put out (ids 2 to 4: each maybe an AI that moves on the ground or not, and a
** physics behaviour with a mass and a centre of mass offset; the contain driver's physics memory, whose applyMotiveForce and setPitchRate log) and a
** budding host (id 5); the height-die driver's terrain (its tables the test's); the pathfinder, whose addObjectToPathfindMap, adjustDestination and
** snapPosition log and answer from the test; the objects' setOrientation and setLayer (lines). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/SupplyCenterProductionExitUpdate.cpp"
#include "SpawnPointProductionExitUpdate_edit.cpp"

const Matrix3D *(*g_boneXformHook)(const char *) = 0;

namespace {

ModuleData *l9r45Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			if (kind == 0) data = SupplyCenterProductionExitUpdate::friend_newModuleData(&ini);
			else data = SpawnPointProductionExitUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r45Saved;
Object *g_r45Objs[5];
unsigned g_r45Gone = 0;
Object *r45Find(ObjectID id) { return (id >= 1 && id <= 5 && !(g_r45Gone >> (id - 1) & 1)) ? g_r45Objs[id - 1] : 0; }
/* the art's spawn bones (the same in the Rust test): <name>01 to <name>NN, NN = g_r45Bones, the k-th at (5k - 12, (-3)^k, 1) turned by 0.3k - 1 */
unsigned g_r45Bones = 0;
AsciiString g_r45BoneName;
Coord3D g_r45Bone;
Matrix3D g_r45Xform;
int r45BoneIndex(const char *name)
{
	size_t n = strlen(g_r45BoneName.str());
	if (strncmp(name, g_r45BoneName.str(), n) != 0 || strlen(name) != n + 2) return 0;
	int k = atoi(name + n);
	return (k >= 1 && k <= (int)g_r45Bones) ? k : 0;
}
const Coord3D *r45Bone(const char *name)
{
	int k = r45BoneIndex(name);
	if (!k) return 0;
	g_r45Bone.set(5.0f * k - 12.0f, (k & 1) ? -3.0f : 3.0f, 1.0f);
	return &g_r45Bone;
}
const Matrix3D *r45Xform(const char *name)
{
	int k = r45BoneIndex(name);
	if (!k) return 0;
	g_r45Xform.Make_Identity();
	g_r45Xform.Rotate_Z(0.3f * k - 1.0f);
	g_r45Xform.Set_Translation(Vector3(5.0f * k - 12.0f, (k & 1) ? -3.0f : 3.0f, 1.0f));
	return &g_r45Xform;
}
void r45PutCoord(const Coord3D &c) { putU32(realBits(c.x)); putU32(realBits(c.y)); putU32(realBits(c.z)); }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a SupplyCenterProductionExitUpdate (`kind` 0) or a SpawnPointProductionExitUpdate (1) and make the
** module on the producer (id 1). `cfg` (17 words): batch R18's 0-15 (the physics parts unused), with the flags' bits 10-12: objects 2 to 4's AI
** is a supply truck's, 13-15 they have a stealth module, 16-18 its grant is temporary, 19-21 they have CAN_STEALTH, 22 the producer is
** STEALTHED, 23 it has a drawable; 16 the art's spawn bones (see r45Bone). The data dump goes first (kind 0: the grant only when the test's
** block sets it: the data's constructor leaves it uninitialised; `cfg_has_grant`). Then batch R18's ops, plus 15 remove object a (2 to 4).
** Each op appends batch R18's outputs, with the module's state in the Rust module's state_words order. */
int ref_l9r45_run(int kind, int cfg_has_grant, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r45Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r45Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	if (kind == 0) {
		const SupplyCenterProductionExitUpdateModuleData *md = (const SupplyCenterProductionExitUpdateModuleData *)d;
		r45PutCoord(md->m_unitCreatePoint);
		r45PutCoord(md->m_naturalRallyPoint);
		if (cfg_has_grant) putU32(md->m_grantTemporaryStealthFrames);
	} else {
		const SpawnPointProductionExitUpdateModuleData *md = (const SpawnPointProductionExitUpdateModuleData *)d;
		putStr(md->m_spawnPointBoneNameData.str(), (int)md->m_spawnPointBoneNameData.getLength());
		g_r45BoneName = md->m_spawnPointBoneNameData;
	}
	g_r45Bones = cfg[16];
	g_r45Gone = 0;
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	TheTerrainLogic = &g_hdTerrain;
	for (int i = 0; i < 8; ++i) { g_hdGround[i] = ctF(cfg[14]); g_hdLayerHeight[i] = ctF(cfg[15]); }
	g_objXformFromPose = true;
	unsigned flags = cfg[4];
	AIUpdateInterface ais[5];
	R44Truck trucks[5];
	StealthUpdate stealths[5];
	Drawable myDraw;
	g_ctN = 5;
	g_ctMass.clear();
	unsigned char physData[5][sizeof(PhysicsBehaviorModuleData) + 16];
	for (int k = 0; k < 5; ++k) {
		g_r45Objs[k] = newObject();
		g_r45Objs[k]->m_oid = (ObjectID)(k + 1);
		ais[k].m_owner = g_r45Objs[k];
		trucks[k].m_owner = g_r45Objs[k];
		stealths[k].m_owner = g_r45Objs[k];
		memset(g_ctPhysMem[k], 0, sizeof(g_ctPhysMem[k]));
		PhysicsBehavior *phys = (PhysicsBehavior *)g_ctPhysMem[k];
		memset(physData[k], 0, sizeof(physData[k]));
		((PhysicsBehaviorModuleData *)physData[k])->m_centerOfMassOffset = ctF(cfg[7]);
		phys->m_moduleData = (const ModuleData *)physData[k];
		g_ctMass[phys] = ctF(cfg[6]);
	}
	Object *me = g_r45Objs[0], *host = g_r45Objs[4];
	myDraw.m_ownerObject = me;
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
	g_modFindHook = r45Find;
	gl.m_awakenCalls = 0;
	g_r30BoneHook = r45Bone;
	g_boneXformHook = r45Xform;
	UpdateModule *m = kind == 0 ? (UpdateModule *)new SupplyCenterProductionExitUpdate(me, d) : (UpdateModule *)new SpawnPointProductionExitUpdate(me, d);
	/* the constructor leaves m_rallyPoint uninitialised: the port's zero (R23's precedent) */
	if (kind == 0) ((SupplyCenterProductionExitUpdate *)m)->m_rallyPoint.zero();
	/* GameLogic::friend_awakenUpdateModule for an object not yet in the list (GameLogic.cpp:3035-3046): the wake frame is the next call frame; a
	** module that sets none (QueueProductionExitUpdate) is called from the next frame, as the Rust world schedules it when it makes the object */
	m->friend_setNextCallFrame(gl.m_awakenCalls ? gl.m_awakenFrame : 1);
	ExitInterface *ex = m->getUpdateExitInterface();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 11) flags = o[1];
		for (int k = 1; k < 4; ++k) {
			g_r45Objs[k]->m_ai = (flags >> (k - 1) & 1) ? &ais[k] : 0;
			ais[k].m_groundMovement = (flags >> (k + 2) & 1) != 0;
			g_r45Objs[k]->m_physics = (flags >> (k + 5) & 1) ? (PhysicsBehavior *)g_ctPhysMem[k] : 0;
		}
		me->m_physics = (flags & 512) ? myPhys : 0;
		for (int k = 1; k < 4; ++k) {
			ais[k].m_supplyTruck = (flags >> (k + 9) & 1) ? &trucks[k] : 0;
			g_r45Objs[k]->m_stealth = (flags >> (k + 12) & 1) ? &stealths[k] : 0;
			stealths[k].m_temporaryGrant = (flags >> (k + 15) & 1) != 0;
			g_r45Objs[k]->m_status.clear();
			if (flags >> (k + 18) & 1) g_r45Objs[k]->m_status.set(OBJECT_STATUS_CAN_STEALTH);
		}
		me->m_status.clear();
		if (flags >> 22 & 1) me->m_status.set(OBJECT_STATUS_STEALTHED);
		me->m_drawable = (flags >> 23 & 1) ? &myDraw : 0;
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
			case 0: ex->exitObjectViaDoor(r45Find((ObjectID)o[1]), DOOR_1); break;
			case 1: res.push_back((unsigned)ex->reserveDoorForExit(0, 0)); break;
			case 2: ex->unreserveDoorForExit(DOOR_1); break;
			case 3: ex->exitObjectByBudding(r45Find((ObjectID)o[1]), o[2] ? r45Find((ObjectID)o[2]) : 0); break;
			case 4: { Coord3D c; c.x = ctF(o[1]); c.y = ctF(o[2]); c.z = ctF(o[3]); ex->setRallyPoint(&c); break; }
			case 5: { const Coord3D *c = ex->getRallyPoint(); if (c) { res.push_back(realBits(c->x)); res.push_back(realBits(c->y)); res.push_back(realBits(c->z)); } break; }
			case 6: res.push_back(ex->useSpawnRallyPoint() ? 1 : 0); break;
			case 7: { Coord3D c; res.push_back(ex->getNaturalRallyPoint(c, o[1] != 0) ? 1 : 0); res.push_back(realBits(c.x)); res.push_back(realBits(c.y)); res.push_back(realBits(c.z)); break; }
			case 8: { Coord3D c; res.push_back(ex->getExitPosition(c) ? 1 : 0); res.push_back(realBits(c.x)); res.push_back(realBits(c.y)); res.push_back(realBits(c.z)); break; }
			case 9: res.push_back((unsigned)m->update()); break;
			case 10: res.push_back(ex->isExitBusy() ? 1 : 0); break;
			case 12: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r45Saved.assign(bytes, bytes + blen); break;
			case 13: {
				std::vector<unsigned char> b = g_r45Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 14: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 15: if (o[1] >= 2 && o[1] <= 4) g_r45Gone |= 1u << (o[1] - 1); break;
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
			SupplyCenterProductionExitUpdate *sm = (SupplyCenterProductionExitUpdate *)m;
			r45PutCoord(sm->m_rallyPoint);
			putU32(sm->m_rallyPointExists ? 1 : 0);
		} else {
			SpawnPointProductionExitUpdate *pm = (SpawnPointProductionExitUpdate *)m;
			putU32(pm->m_bonesInitialized ? 1 : 0);
			putU32((unsigned)pm->m_spawnPointCount);
			for (int k = 0; k < MAX_SPAWN_POINTS; ++k) {
				r45PutCoord(pm->m_worldCoordSpawnPoints[k]);
				putU32(realBits(pm->m_worldAngleSpawnPoints[k]));
				putU32((unsigned)pm->m_spawnPointOccupier[k]);
			}
		}
		for (int k = 1; k < 4; ++k) r45PutCoord(g_r45Objs[k]->m_pos);
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_r30BoneHook = 0;
	g_boneXformHook = 0;
	g_r45Gone = 0;
	me->m_drawable = 0;
	g_objXformFromPose = false;
	g_ctN = 0;
	g_ctMass.clear();
	TheTerrainLogic = 0;
	delete m;
	delete d;
	for (int k = 0; k < 5; ++k) { g_r45Objs[k]->m_ai = 0; g_r45Objs[k]->m_physics = 0; g_r45Objs[k]->m_stealth = 0; delete g_r45Objs[k]; }
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
