/* Differential-test driver for DumbProjectileBehavior (p1w13-sonnet, batch R68; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/DumbProjectileBehavior.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end (the projectile's physics is the REAL PhysicsBehavior compiled by
** differential/l9_physics_r50.inc under R50 names, batch R52's way), on the module build's stand-ins: the projectile (id 1: a position, maybe a
** physics behaviour, maybe a drawable whose setDrawableHidden is batch R2's line, maybe a controlling player whose academy is batch P's line,
** the module build's MBody whose attemptDamage is a line), the launcher (id 2, maybe: its weapon bonus flags, scoreTheKill batch P's line),
** the victim (id 3, maybe: a cylinder of the test's height, a position), a building (id 4: a contain whose count, garrisonability, immunity
** and list are the test's), three garrisoned units (ids 5-7: kinds, effectively dead, an MBody); the weapon store's WEAP_A and WEAP_B (the
** speeds and ranges the test gives; their shouldProjectileCollideWith answers the test's set), Weapon::positionProjectileForLaunch and
** WeaponStore::handleProjectileDetonation (lines), the partition manager's terrain extremes along a line (the test's answer, a line) and the
** REAL distance procedures (batch R57's hook), the terrain's layers (batch O's tables: all ground), Object::setLayer (batch R8's line),
** destroyObject (a line), the REAL Object::kill, BezierSegment, UpdateModule, INI, Xfer and logic random values; the game logic (its frame). */
/* (the file includes MissileAIUpdate.h, a layer-10 header it uses nothing of: its include guard is defined first; and it uses Lib/BaseType.h's
** max macro (line 105, verbatim), which the module build leaves out, as batch R21 does with min) */
#define _MISSILE_AI_UPDATE_H_
#define max(x,y) (((x)>(y)) ? (x) : (y))
#define PhysicsBehavior R50PhysicsBehavior
#define getPhysics r50GetPhysics
#define destroyObject r50DestroyObject   /* (as in l9_physics_r50.inc: the inline one loses to other units' out-of-line ones) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/DumbProjectileBehavior.cpp"
#undef destroyObject
#undef getPhysics
#undef PhysicsBehavior
#undef max

namespace {

ModuleData *l9r68Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = DumbProjectileBehavior::friend_newModuleData(&ini);
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

/* the building's contain: the count, garrisonability, immunity and list the test gives */
class R68Contain : public UpgContain
{
public:
	virtual UnsignedInt getContainCount() const { return (UnsignedInt)m_items.size(); }
	virtual Bool isGarrisonable() const { return m_garrisonable; }
	virtual Bool isImmuneToClearBuildingAttacks() const { return m_immune; }
	virtual const ContainedItemsList* getContainedItemsList() const { return &m_items; }
	ContainedItemsList m_items;
	Bool m_garrisonable = false, m_immune = false;
};

const KindOfType g_r68Kinds[3] = { KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_HERO };
std::vector<unsigned char> g_r68Saved;
Object *g_r68Objs[7];
bool g_r68Exists[7];
WeaponTemplate g_r68Tmpl[2];
const char *const g_r68Names[2] = { "WEAP_A", "WEAP_B" };
WeaponStore g_r68Store;
unsigned g_r68Collides = 0;   /* the objects (a bit each, by id) shouldProjectileCollideWith says the projectile collides with */
unsigned g_r68OnMap = 1;
Real g_r68MaxZ = 0.0f;
const void *r68Lookup(const char *name)
{
	for (int i = 0; i < 2; ++i) if (strcasecmp(name, g_r68Names[i]) == 0) return &g_r68Tmpl[i];
	return 0;
}
int r68Code(const WeaponTemplate *t) { return t == &g_r68Tmpl[0] ? 1 : (t == &g_r68Tmpl[1] ? 2 : 0); }
Object *r68Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 7 && g_r68Exists[(int)id - 1]) ? g_r68Objs[(int)id - 1] : 0; }
Real r68F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
void r68Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
int r68Id(const Object *o) { return o ? (int)o->m_oid : 0; }
/* Thing::setTransformMatrix (Thing.cpp 250-262): the position is the matrix's translation (the stand-in's setTransformMatrix keeps only the
** matrix) */
void r68Xform(Object *o, const Matrix3D *mx) { o->m_pos.x = mx->Get_X_Translation(); o->m_pos.y = mx->Get_Y_Translation(); o->m_pos.z = mx->Get_Z_Translation(); }

}  // namespace

void Weapon::positionProjectileForLaunch(Object* projectile, const Object *launcher, WeaponSlotType wslot, Int specificBarrelToUse)
{
	ulog("positionlaunch %d %d %d %d", r68Id(projectile), r68Id(launcher), (int)wslot, (int)specificBarrelToUse);
}
Bool WeaponTemplate::shouldProjectileCollideWith(const Object* projectileLauncher, const Object* projectile, const Object* thingWeCollidedWith, ObjectID intendedVictimID) const
{
	ulog("shouldcollide %s %d %d %d %d", m_name.str(), r68Id(projectileLauncher), r68Id(projectile), r68Id(thingWeCollidedWith), (int)intendedVictimID);
	return thingWeCollidedWith && (g_r68Collides >> (int)thingWeCollidedWith->m_oid & 1) != 0;
}
void WeaponStore::handleProjectileDetonation( const WeaponTemplate* w, const Object *source, const Coord3D* pos, UnsignedInt extraBonusFlags, Bool inflictDamage )
{
	ulog("detonation %s %d %08x %08x %08x %u", w ? w->getName().str() : "", r68Id(source), realBits(pos->x), realBits(pos->y), realBits(pos->z), extraBonusFlags);
}
Bool PartitionManager::estimateTerrainExtremesAlongLine(const Coord3D& s, const Coord3D& e, Real* minZ, Real* maxZ, Coord2D* minZPos, Coord2D* maxZPos)
{
	ulog("extremes %08x %08x %08x %08x %08x %08x", realBits(s.x), realBits(s.y), realBits(s.z), realBits(e.x), realBits(e.y), realBits(e.z));
	if (maxZ) *maxZ = g_r68MaxZ;
	return g_r68OnMap != 0;
}

extern void (*g_xformHook)(Object *o, const Matrix3D *mx);

extern "C" {

/* Read `filename` (block `D`) as the data of a DumbProjectileBehavior and make the module on the projectile (id 1) at frame cfg[0]. `cfg`
** (32 words; 3-21 also settable by op 2): 1 the logic random seed; 2 the projectile: bit 0 a physics behaviour, 1 a drawable, 2 a controlling
** player (team 1, player 0); 3-5 its position; 6 the launcher: bit 0 exists, 7 its weapon bonus flags; 8 the victim exists; 9-11 its position,
** 12 its height (float bits); 13 the building: bit 0 garrisonable, 1 immune, bits 4-15 its list (nibbles of 5-7, 0 ends); 14-16 units 5-7:
** bits 0-2 their kinds (of g_r68Kinds), 3 effectively dead; 17 the collision set (a bit per id); 18 on the map; 19 the highest terrain (float
** bits); read once: 22-26 WEAP_A's speed, minimum speed, scale flag, attack range, minimum attack range; 27-31 WEAP_B's. The data dump goes
** first. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3 xfer save  4 xfer load of the last save
** 5 xfer CRC  6 projectileLaunchAtObjectOrPosition(a bit 0: the victim, else NULL; position (b, c, d); the launcher; a bits 1-2 the slot, 3-4
** the barrel, 5 the template WEAP_B, else WEAP_A)  7 projectileHandleCollision(object a, 0 NULL). Each op appends: the CATCH_ALL status, the
** result, the effect log (but the layer lines), the xfer status, bytes and CRC, the module's state, the projectile's position and transform,
** its no-collisions status and its physics' yaw, roll and pitch rates. */
static int l9r68RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r68Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	initKindOfMasks();
	for (int k = 0; k < 2; ++k) {
		g_r68Tmpl[k].m_name = AsciiString(g_r68Names[k]);
		g_r68Tmpl[k].m_weaponSpeed = r68F(cfg_in[22 + 5 * k]);
		g_r68Tmpl[k].m_minWeaponSpeed = r68F(cfg_in[23 + 5 * k]);
		g_r68Tmpl[k].m_isScaleWeaponSpeed = cfg_in[24 + 5 * k] != 0;
		g_r68Tmpl[k].m_attackRange = r68F(cfg_in[25 + 5 * k]);
		g_r68Tmpl[k].m_minimumAttackRange = r68F(cfg_in[26 + 5 * k]);
	}
	try { d = l9r68Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const DumbProjectileBehaviorModuleData *dd = (const DumbProjectileBehaviorModuleData *)d;
		putU32(realBits(dd->m_firstHeight));
		putU32(realBits(dd->m_secondHeight));
		putU32(realBits(dd->m_firstPercentIndent));
		putU32(realBits(dd->m_secondPercentIndent));
		putU32(dd->m_maxLifespan);
		putU32((dd->m_tumbleRandomly ? 1 : 0) | (dd->m_orientToFlightPath ? 2 : 0) | (dd->m_detonateCallsKill ? 4 : 0));
		putI32(dd->m_garrisonHitKillCount);
		for (int i = 0; i < KINDOF_COUNT; ++i) if (dd->m_garrisonHitKillKindof.test((KindOfType)i)) putU32((unsigned)i);
		putU32(0xFFFFFFFFu);
		for (int i = 0; i < KINDOF_COUNT; ++i) if (dd->m_garrisonHitKillKindofNot.test((KindOfType)i)) putU32((unsigned)i);
		putU32(0xFFFFFFFFu);
		putU32((unsigned)fxId(dd->m_garrisonHitKillFX));
		putU32(realBits(dd->m_flightPathAdjustDistPerFrame));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	gl.m_awakenHook = r35Awaken;
	InitGameLogicRandom(cfg[1]);
	ThePartitionManager = &g_ahPm;
	TheWeaponStore = &g_r68Store;
	g_weaponFindHook = r68Lookup;
	g_objXformSyncPos = true;
	g_distPosDcHook = r57DistPos;   /* (l9_slaved_r57.inc's, earlier in this unit) */
	for (int i = 0; i < 8; ++i) g_hdLayer[i] = LAYER_GROUND;   /* (batch O's layer table, earlier in this unit: every cell ground, as the Rust side's) */
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 7; ++k) {
		g_r68Objs[k] = newObject();
		g_r68Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r68Objs[k]->m_behaviors = none;
		g_r68Exists[k] = true;
	}
	g_r68Exists[1] = (cfg[6] & 1) != 0;
	g_r68Exists[2] = (cfg[8] & 1) != 0;
	Object *me = g_r68Objs[0];
	Player player;
	player.m_idx = 0;
	Team team;
	team.m_dummy = 1;
	team.m_controller = &player;
	if (cfg[2] >> 2 & 1) { me->m_team = &team; me->m_player = &player; }
	Drawable myDrawable;
	myDrawable.m_ownerObject = me;
	me->m_drawable = (cfg[2] >> 1 & 1) ? &myDrawable : 0;
	MBody bodies[4];
	bodies[0].m_owner = me;
	me->m_body = &bodies[0];
	for (int k = 0; k < 3; ++k) { bodies[k + 1].m_owner = g_r68Objs[4 + k]; g_r68Objs[4 + k]->m_body = &bodies[k + 1]; }
	R68Contain contain;
	g_r68Objs[3]->m_contain = &contain;
	R50PhysicsBehaviorModuleData physData;
	R50PhysicsBehavior *phys = 0;
	BehaviorModule *myList[3] = { 0, 0, 0 };
	auto scene = [&]() {
		me->m_pos.x = r68F(cfg[3]); me->m_pos.y = r68F(cfg[4]); me->m_pos.z = r68F(cfg[5]);
		g_r68Objs[1]->m_weaponBonusCondition = cfg[7];
		Object *v = g_r68Objs[2];
		v->m_pos.x = r68F(cfg[9]); v->m_pos.y = r68F(cfg[10]); v->m_pos.z = r68F(cfg[11]);
		v->m_geometry = GeometryInfo(GEOMETRY_CYLINDER, false, r68F(cfg[12]), 5.0f, 5.0f);
		contain.m_garrisonable = (cfg[13] & 1) != 0;
		contain.m_immune = (cfg[13] >> 1 & 1) != 0;
		contain.m_items.clear();
		for (int k = 0; k < 3; ++k) { unsigned id = cfg[13] >> (4 + 4 * k) & 15; if (id == 0) break; if (id >= 5 && id <= 7) contain.m_items.push_back(g_r68Objs[id - 1]); }
		for (int k = 0; k < 3; ++k) {
			Object *o = g_r68Objs[4 + k];
			unsigned f = cfg[14 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 3; ++b) if (f >> b & 1) o->m_kindOf.set(g_r68Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_effDead = (f >> 3 & 1) != 0;
		}
		g_r68Collides = cfg[17];
		g_r68OnMap = cfg[18] & 1;
		g_r68MaxZ = r68F(cfg[19]);
	};
	scene();
	me->poseMatrix();
	g_modFindHook = r68Find;
	g_modDestroyHook = r68Destroy;
	g_xformHook = r68Xform;
	bool loading = false;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) {
				const std::string &l = g_ulog[(size_t)e.p];
				if (l.compare(0, 13, "settransform ") == 0 || l.compare(0, 9, "setlayer ") == 0) continue;
				log.push_back(l);
			}
			else if (e.kind == 1 && e.p) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	DumbProjectileBehavior *m = 0;
	try { m = newInstance(DumbProjectileBehavior)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	myList[0] = m;
	if (cfg[2] & 1) {
		phys = newInstance(R50PhysicsBehavior)(me, &physData);
		me->m_r50Physics = phys;
		myList[1] = phys;
	}
	me->m_behaviors = myList;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 3 && o[1] <= 21) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		if (o[0] == 2) scene();
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
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r68Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r68Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 6: {
				Coord3D pos;
				pos.x = r68F(o[2]); pos.y = r68F(o[3]); pos.z = r68F(o[4]);
				m->projectileLaunchAtObjectOrPosition((o[1] & 1) ? r68Find((ObjectID)3) : 0, &pos, r68Find((ObjectID)2), (WeaponSlotType)(o[1] >> 1 & 3),
					(Int)(o[1] >> 3 & 3), &g_r68Tmpl[o[1] >> 5 & 1], 0);
				break;
			}
			case 7: res = m->projectileHandleCollision(o[1] ? r68Find((ObjectID)o[1]) : 0) ? 1 : 0; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		loading = o[0] == 4;
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32((unsigned)m->m_launcherID);
		putU32((unsigned)m->m_victimID);
		putU32((unsigned)r68Code(m->m_detonationWeaponTmpl));
		putU32(m->m_lifespanFrame);
		putU32((unsigned)m->m_flightPathSegments);
		putU32(realBits(m->m_flightPathSpeed));
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_flightPathStart.x)[k]));
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_flightPathEnd.x)[k]));
		putU32((unsigned)m->m_currentFlightPathStep);
		putU32(m->m_extraBonusFlags);
		putU32(m->m_hasDetonated ? 1 : 0);
		putU32((unsigned)m->m_flightPath.size());
		for (size_t j = 0; j < m->m_flightPath.size(); ++j) for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_flightPath[j].x)[k]));
		for (int k = 0; k < 3; ++k) putU32(realBits((&me->m_pos.x)[k]));
		const Matrix3D *mx = me->getTransformMatrix();
		for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) putU32(realBits((*mx)[r][c]));
		putU32(me->m_status.test(OBJECT_STATUS_NO_COLLISIONS) ? 1 : 0);
		putU32(phys ? realBits(phys->m_yawRate) : 0);
		putU32(phys ? realBits(phys->m_rollRate) : 0);
		putU32(phys ? realBits(phys->m_pitchRate) : 0);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	g_xformHook = 0;
	g_weaponFindHook = 0;
	g_objXformSyncPos = false;
	g_distPosDcHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete phys;
	delete d;
	g_r68Objs[3]->m_contain = 0;
	for (int k = 0; k < 7; ++k) {
		Object *x = g_r68Objs[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_drawable = 0; x->m_body = 0; x->m_r50Physics = 0;
		delete x;
		g_r68Objs[k] = 0;
	}
	TheWeaponStore = 0;
	ThePartitionManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r68_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r68RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
