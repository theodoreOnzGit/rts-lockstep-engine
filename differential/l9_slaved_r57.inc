/* Differential-test driver for SlavedUpdate (p1w13-sonnet, batch R57; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SlavedUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: four objects (ids 1-4; a team of three,
** positions and geometries the test's), the drone (object 1: maybe an AI, whose locomotor is the AI itself, idle or not; maybe a drawable whose
** model condition calls and pristine bones are the test's), the masters and victims (objects 2-4: maybe an AI whose current victim and path
** are the test's, effectively dead, DISABLED_UNMANNED, stealthed, a layer, maybe the module build's MBody, whose healing is a line), the
** teams' relationships, the terrain's ground heights (batch O's cells), the distances (PartitionManager.cpp's four distance procedures,
** verbatim: build.rs's mod_gen/PartitionDist.inc), the AI's and the locomotor's calls (lines), setDisabled, clearDisabled, setStatus,
** defect and setLayer (lines), the particle systems (batch M's, the lifetime range a line), the misc audio's repair sparks (an audio line),
** the REAL logic random values (the test's seed), UpdateModule, INI and Xfer; the game logic (its frame). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SlavedUpdate.cpp"

#include "PartitionDist.inc"   /* (build.rs, mod_gen) */

void AIUpdateInterface::aiGoProne(const DamageInfo *damageInfo, CommandSourceType cmdSource)
{
	ulog("aiprone %d %d %08x %d", (int)m_owner->m_oid, (int)damageInfo->in.m_damageType, realBits(damageInfo->in.m_amount), (int)cmdSource);
}
void ParticleSystem::setLifetimeRange(Real min, Real max) { ulog("pslife2 %u %08x %08x", m_id, realBits(min), realBits(max)); }

namespace {

/* PartitionManager.cpp lines 951-957, verbatim */
typedef Bool (*R57DistCalcProc)(const Coord3D *posA, const Object *objA, const Coord3D *posB, const Object *objB, Real& abDistSqr, Coord3D& abVec, Real maxDistSqr);
R57DistCalcProc theDistCalcProcs[] =
{
	distCalcProc_CenterAndCenter_2D,
	distCalcProc_CenterAndCenter_3D,
	distCalcProc_BoundaryAndBoundary_2D,
	distCalcProc_BoundaryAndBoundary_3D,
};
const Real R57_HUGE_DIST_SQR = (1000000.0f*1000000.0f);   /* PartitionManager.cpp: HUGE_DIST_SQR, HUGE_DIST 1000000.0f */
Real r57Dist(const Object *a, const Object *b, DistanceCalculationType dc)
{
	Real d; Coord3D v;
	(*theDistCalcProcs[dc])(a->getPosition(), a, b->getPosition(), b, d, v, R57_HUGE_DIST_SQR);
	return d;
}
Real r57DistPos(const Object *a, const Coord3D *pos, DistanceCalculationType dc)
{
	Real d; Coord3D v;
	(*theDistCalcProcs[dc])(a->getPosition(), a, pos, NULL, d, v, R57_HUGE_DIST_SQR);
	return d;
}

ModuleData *l9r57Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = SlavedUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r57Saved;
Object *g_r57Objs[4];
AIUpdateInterface g_r57Ais[4];
Team g_r57Teams[3];
Player g_r57Players[3];
int g_r57Rel[3][3];
Coord3D g_r57Bone;
bool g_r57HasBone = false;
std::string g_r57BoneName;
Object *r57Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 4) ? g_r57Objs[(int)id - 1] : 0; }
Relationship r57TeamRel(const Team *from, const Team *to) { return (Relationship)g_r57Rel[from->m_dummy - 1][to->m_dummy - 1]; }
int r57LocoOwner(const Locomotor *l)
{
	for (int k = 0; k < 4; ++k) if ((const void *)&g_r57Ais[k] == (const void *)l) return k + 1;
	return 0;
}
const Coord3D *r57BoneHook(const char *name) { return (g_r57HasBone && g_r57BoneName == name) ? &g_r57Bone : 0; }
void r57SetStatus(Object *obj, ObjectStatusMaskType mask, Bool set)
{
	int bit = -1;
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (mask.test((ObjectStatusTypes)i)) { bit = i; break; }
	ulog("setstatus %d %d %d", (int)obj->m_oid, bit, set ? 1 : 0);
}
void r57Defect(Object *obj, Team *team, UnsignedInt detectionTime) { ulog("defect %d %d %u", (int)obj->m_oid, team ? team->m_dummy : 0, detectionTime); }
Real r57F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
const Real g_r57Radius[4] = { 2.0f, 5.0f, 10.0f, 0.5f };

}  // namespace

void Locomotor::setUsePreciseZPos(Bool u) { ulog("locoprecise %d %d", r57LocoOwner(this), u ? 1 : 0); }

extern "C" {

/* Read `filename` (block `D`) as the data of a SlavedUpdate and make the module on object 1 of four at frame cfg[0], then onObjectCreated. `cfg`
** (24 words; 2-23 also settable by op 2): 1 the random seed; 2 the drone: bit 0 an AI, 1 its locomotor, 2 idle, 3 a drawable, 4-5 its team (0
** none, else 1-3), 6 the art has the welding bone (at cfg 21's three bytes as halves, relative to the drone); 3-5 objects 2-4: bit 0 an AI, 1
** effectively dead, 2 DISABLED_UNMANNED, 3 STEALTHED, 4-5 the team, 6 a body (health cfg 6's byte k-2 of 200), 7-8 the layer, 9-11 the AI's
** victim (objects 1-4, else NULL), 12 the AI has a path (its last node cfg 18-19, z 0), 13-14 the geometry's radius (of g_r57Radius); 6 the
** healths; 7 the teams' relationships (2 bits each, row-major, team a to team b: 0 enemies, 1 neutral, 2 allies); 8-11 objects 1-4's x, 12-15
** their y (float bits); 16 the drone's z (float bits; the others' 0); 17 the terrain's eight ground heights (nibbles, as halves); 18-19 the path's
** last node; 20 unused; 21 the bone. The data dump goes first, then onObjectCreated's effects. Then `n` ops of 5 words [op, a, b, c, d]:
** 0 update()  1 the frame = a  2 cfg[a] = b  3 onEnslave(object a, else NULL)  4 onSlaverDie(NULL)  5 onSlaverDamage(damage type a, amount b
** bits)  6 xfer save  7 xfer load of the last save  8 xfer CRC  9 onObjectCreated  10 getSlaverID. Each op appends: the CATCH_ALL status, the
** result, the effect log, the xfer status, bytes and CRC, the state (the slaver, the guard offset, the countdown, the repair state, repairing),
** the drone's UNSELECTABLE bit and disabled bits, and objects 2-4's weapon bonus bits and health. */
static int l9r57RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r57Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r57Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[24];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const SlavedUpdateModuleData *sd = (const SlavedUpdateModuleData *)d;
		const Int ints[14] = { sd->m_guardMaxRange, sd->m_guardWanderRange, sd->m_attackRange, sd->m_attackWanderRange, sd->m_scoutRange,
			sd->m_scoutWanderRange, sd->m_distToTargetToGrantRangeBonus, sd->m_repairRange, sd->m_repairWhenHealthBelowPercentage,
			sd->m_minReadyFrames, sd->m_maxReadyFrames, sd->m_minWeldFrames, sd->m_maxWeldFrames, sd->m_stayOnSameLayerAsMaster ? 1 : 0 };
		for (int k = 0; k < 14; ++k) putI32(ints[k]);
		putU32(realBits(sd->m_repairMinAltitude));
		putU32(realBits(sd->m_repairMaxAltitude));
		putU32(realBits(sd->m_repairRatePerSecond));
		putStr(sd->m_weldingSysName.str(), (int)sd->m_weldingSysName.getLength());
		putStr(sd->m_weldingFXBone.str(), (int)sd->m_weldingFXBone.getLength());
		g_r57BoneName = sd->m_weldingFXBone.str();
	}
	InitGameLogicRandom(cfg[1]);
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	ThePartitionManager = &g_ahPm;
	TheParticleSystemManager = &g_ahPsm;
	TheAudio = &g_ddAudio;
	g_cfMisc.m_repairSparks.setEventName(AsciiString("repairSparks"));
	MBody bodies[4];
	Drawable draw;
	Path paths[4];
	PathNode nodes[4];
	BehaviorModule *none[1] = { 0 };
	for (int t = 0; t < 3; ++t) {
		g_r57Players[t].m_idx = t;
		g_r57Teams[t].m_dummy = t + 1;
		g_r57Teams[t].m_controller = &g_r57Players[t];
	}
	for (int k = 0; k < 4; ++k) {
		g_r57Objs[k] = newObject();
		g_r57Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r57Objs[k]->m_behaviors = none;
		bodies[k].m_owner = g_r57Objs[k];
		bodies[k].m_max = 200.0f;
		g_r57Ais[k] = AIUpdateInterface();
		g_r57Ais[k].m_owner = g_r57Objs[k];
		paths[k].m_pathTail = &nodes[k];
	}
	Object *me = g_r57Objs[0];
	draw.m_ownerObject = me;
	auto teamOf = [&](unsigned t) -> Team * { return (t >= 1 && t <= 3) ? &g_r57Teams[t - 1] : 0; };
	auto scene = [&]() {
		unsigned f = cfg[2];
		me->m_ai = (f & 1) ? &g_r57Ais[0] : 0;
		g_r57Ais[0].m_hasLoco = (f >> 1 & 1) != 0;
		g_r57Ais[0].m_idle = (f >> 2 & 1) != 0;
		me->m_drawable = (f >> 3 & 1) ? &draw : 0;
		me->m_team = teamOf(f >> 4 & 3);
		me->m_player = me->m_team ? me->m_team->m_controller : 0;
		g_r57HasBone = (f >> 6 & 1) != 0;
		g_r57Bone.x = (Real)(cfg[21] & 255) * 0.5f;
		g_r57Bone.y = (Real)(cfg[21] >> 8 & 255) * 0.5f;
		g_r57Bone.z = (Real)(cfg[21] >> 16 & 255) * 0.5f;
		me->m_geometry = GeometryInfo(GEOMETRY_SPHERE, false, 2.0f, 2.0f, 2.0f);
		for (int k = 1; k < 4; ++k) {
			Object *o = g_r57Objs[k];
			unsigned g = cfg[2 + k];
			o->m_ai = (g & 1) ? &g_r57Ais[k] : 0;
			o->m_effDead = (g >> 1 & 1) != 0;
			o->m_disabledBits = (g >> 2 & 1) ? (1u << DISABLED_UNMANNED) : 0;
			o->m_disabled = o->m_disabledBits != 0;
			o->m_status.clear();
			if (g >> 3 & 1) o->m_status.set(OBJECT_STATUS_STEALTHED);
			o->m_team = teamOf(g >> 4 & 3);
			o->m_player = o->m_team ? o->m_team->m_controller : 0;
			o->m_body = (g >> 6 & 1) ? &bodies[k] : 0;
			o->m_layer = (PathfindLayerEnum)(g >> 7 & 3);
			unsigned v = g >> 9 & 7;
			g_r57Ais[k].m_victim = (v >= 1 && v <= 4) ? g_r57Objs[v - 1] : 0;
			g_r57Ais[k].m_r57Path = (g >> 12 & 1) ? &paths[k] : 0;
			nodes[k].m_pos.x = r57F(cfg[18]);
			nodes[k].m_pos.y = r57F(cfg[19]);
			nodes[k].m_pos.z = 0.0f;
			Real r = g_r57Radius[g >> 13 & 3];
			o->m_geometry = GeometryInfo(GEOMETRY_SPHERE, false, r, r, r);
		}
		for (int k = 0; k < 4; ++k) {
			Object *o = g_r57Objs[k];
			o->m_pos.x = r57F(cfg[8 + k]);
			o->m_pos.y = r57F(cfg[12 + k]);
			o->m_pos.z = k == 0 ? r57F(cfg[16]) : 0.0f;
		}
		for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r57Rel[a][b] = (int)(cfg[7] >> (2 * (3 * a + b)) & 3);
		for (int c = 0; c < 8; ++c) g_hdGround[c] = (Real)(cfg[17] >> (4 * c) & 15) * 0.5f;
	};
	scene();
	for (int k = 1; k < 4; ++k) bodies[k].m_health = (Real)(cfg[6] >> (8 * (k - 1)) & 255);
	g_modFindHook = r57Find;
	g_teamRelHook = r57TeamRel;
	g_distDcHook = r57Dist;
	g_distPosDcHook = r57DistPos;
	g_locoOwnerHook = r57LocoOwner;
	g_r30BoneHook = r57BoneHook;
	g_setStatusHook = r57SetStatus;
	g_defectHook = r57Defect;
	g_ahPsLastId = 0;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	SlavedUpdate *m = 0;
	try { m = newInstance(SlavedUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	g_events.clear();
	g_ulog.clear();
	m->onObjectCreated();
	putLog();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 23) {
			cfg[o[1]] = o[2];
			if (o[1] == 6) for (int k = 1; k < 4; ++k) bodies[k].m_health = (Real)(cfg[6] >> (8 * (k - 1)) & 255);
		}
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: m->onEnslave(r57Find((ObjectID)o[1])); break;
			case 4: m->onSlaverDie(0); break;
			case 5: { DamageInfo di; di.in.m_damageType = (DamageType)o[1]; di.in.m_amount = r57F(o[2]); m->onSlaverDamage(&di); break; }
			case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r57Saved.assign(bytes, bytes + blen); break;
			case 7: {
				std::vector<unsigned char> b = g_r57Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 8: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 9: m->onObjectCreated(); break;
			case 10: res = (unsigned)m->getSlaverID(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32((unsigned)m->m_slaver);
		putU32(realBits(m->m_guardPointOffset.x));
		putU32(realBits(m->m_guardPointOffset.y));
		putU32(realBits(m->m_guardPointOffset.z));
		putU32((unsigned)m->m_framesToWait);
		putU32((unsigned)m->m_repairState);
		putU32(m->m_repairing ? 1 : 0);
		putU32(me->m_status.test(OBJECT_STATUS_UNSELECTABLE) ? 1 : 0);
		putU32(me->m_disabledBits);
		for (int k = 1; k < 4; ++k) {
			putU32((unsigned)g_r57Objs[k]->m_weaponBonusCondition);
			putU32(realBits(bodies[k].m_health));
		}
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_teamRelHook = 0;
	g_distDcHook = 0;
	g_distPosDcHook = 0;
	g_locoOwnerHook = 0;
	g_r30BoneHook = 0;
	g_setStatusHook = 0;
	g_defectHook = 0;
	me->m_behaviors = 0;
	me->m_drawable = 0;
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) {
		Object *x = g_r57Objs[k];
		x->m_ai = 0; x->m_body = 0; x->m_team = 0; x->m_player = 0; x->m_behaviors = 0;
		delete x;
		g_r57Objs[k] = 0;
	}
	TheAudio = 0;
	TheParticleSystemManager = 0;
	ThePartitionManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r57_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r57RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
