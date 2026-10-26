/* Differential-test driver for NeutronMissileUpdate (p1w13-sonnet, batch R67; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/NeutronMissileUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end (after differential/l9_physics_r50.inc: the launcher's physics is the REAL
** PhysicsBehavior compiled there under R50 names, PhysicsBehavior and getPhysics being R50PhysicsBehavior and r50GetPhysics while the file is
** included, batch R52's way), on the module build's stand-ins: the missile (id 1: a position, an orientation, a bounding sphere, maybe a
** drawable whose setDrawableHidden (batch R2's line) and setInstanceMatrix (a line) are the test's, maybe a controlling player, an experience
** tracker whose setExperienceSink is batch R6's line, the module build's MBody whose attemptDamage is a line), the launcher (id 2, maybe: a
** position, an orientation, maybe a drawable whose launch bone is the test's, maybe a physics behaviour), the victim (id 3, maybe), a fourth
** object; the distance (the REAL partition procedures, batch R57's hook), the decal (batch O's RadiusDecal stand-ins), the particle system
** manager (batch M's templates; createAttachedParticleSystemID a line), destroyObject (a line), the REAL Object::kill, Object::onCollide,
** UpdateModule, INI, Xfer and logic random values; the game logic (its frame). */
/* (the header forward-declares `enum ParticleSystemID;`, which the shim's ParticleSys.h has as a typedef: renamed by macro for the include, to an
** enum defined first, as batch R60 does; the file's one use of the type is the return of createAttachedParticleSystemID, which it ignores) */
enum R67ParticleSystemIDFwd { R67_PARTICLE_SYSTEM_ID_FWD };
#define ParticleSystemID R67ParticleSystemIDFwd
#define PhysicsBehavior R50PhysicsBehavior
#define getPhysics r50GetPhysics
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/NeutronMissileUpdate.cpp"
#undef getPhysics
#undef PhysicsBehavior
#undef ParticleSystemID

AsciiString ParticleSystemTemplate::getName() const { return AsciiString(g_ahPsNames[(size_t)m_idx].c_str()); }
ParticleSystemID ParticleSystemManager::createAttachedParticleSystemID( const ParticleSystemTemplate *sysTemplate, Object* attachTo, Bool )
{
	ulog("psattach %s %d", sysTemplate ? g_ahPsNames[(size_t)sysTemplate->m_idx].c_str() : "", attachTo ? (int)attachTo->m_oid : 0);
	return INVALID_PARTICLE_SYSTEM_ID;
}
extern Real (*g_distPosDcHook)(const Object *, const Coord3D *, DistanceCalculationType);
extern void (*g_instanceHook)(const Drawable *, const Matrix3D *);

namespace {

ModuleData *l9r67Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = NeutronMissileUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r67Saved;
Object *g_r67Objs[4];
bool g_r67Exists[4];
Matrix3D g_r67Offset;
bool g_r67OffsetFound = false;
Object *r67Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 4 && g_r67Exists[(int)id - 1]) ? g_r67Objs[(int)id - 1] : 0; }
Real r67F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
void r67Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
void r67Instance(const Drawable *d, const Matrix3D *m)
{
	int id = d->m_ownerObject ? (int)d->m_ownerObject->m_oid : 0;
	if (!m) { ulog("instance %d null", id); return; }
	std::string s;
	char b[16];
	for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) { sprintf(b, " %08x", realBits((*m)[r][c])); s += b; }
	ulog("instance %d%s", id, s.c_str());
}

}  // namespace

Bool Drawable::getProjectileLaunchOffset(WeaponSlotType wslot, Int specificBarrelToUse, Matrix3D* launchPos, WhichTurretType tur, Coord3D* turretRotPos, Coord3D* turretPitchPos) const
{
	if (!g_r67OffsetFound) return FALSE;
	if (launchPos) *launchPos = g_r67Offset;
	return TRUE;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a NeutronMissileUpdate and make the module on the missile (id 1) at frame cfg[0]. `cfg` (32 words):
** 1 the logic random seed; 2 the missile: bit 0 a drawable, 1 a controlling player (team 1, player 0); 3-5 its position, 6 its orientation, 7
** its bounding sphere radius (float bits); 8 the launcher: bit 0 exists, 1 a drawable, 2 the launch bone is found, 3 a physics behaviour; 9-11
** its position, 12 its orientation, 13-15 the launch bone's translation, 16 its rotation about z (float bits); 17 the victim exists; 18-20 its
** position; 21-23 object 4's position. The data dump goes first (the eleven numbers, the two FX lists' numbers, the decal template's seven
** fields and radius). Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  3 xfer save  4 xfer load of the last save
** 5 xfer CRC  6 projectileLaunchAtObjectOrPosition(a bit 0: the victim, else NULL; position (b, c, d); the launcher; a bits 1-2 the slot, 3-4
** the barrel; bit 5: the exhaust EXH_A, else NULL)  7 projectileHandleCollision(object a, 0 NULL)  8 onDie  9 the launcher's physics
** addVelocityTo((a, b, c) float bits)  10 the missile's position = (a, b, c)  11 its orientation = a. Each op appends: the CATCH_ALL status,
** the result, the effect log (but the transform lines), the xfer status, bytes and CRC, the module's state, the missile's position and
** transform, whether it has OBJECT_STATUS_NO_COLLISIONS. */
static int l9r67RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r67Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r67Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const NeutronMissileUpdateModuleData *nd = (const NeutronMissileUpdateModuleData *)d;
		putU32(realBits(nd->m_initialDist));
		putU32(realBits(nd->m_maxTurnRate));
		putU32(realBits(nd->m_forwardDamping));
		putU32(realBits(nd->m_relativeSpeed));
		putU32(realBits(nd->m_targetFromDirectlyAbove));
		putU32(realBits(nd->m_specialAccelFactor));
		putU32(nd->m_specialSpeedTime);
		putU32(realBits(nd->m_specialSpeedHeight));
		putU32(realBits(nd->m_specialJitterDistance));
		putU32((unsigned)fxId(nd->m_launchFX));
		putU32((unsigned)fxId(nd->m_ignitionFX));
		const RadiusDecalTemplate &t = nd->m_deliveryDecalTemplate;
		putStr(t.m_name.str(), t.m_name.getLength());
		putU32((unsigned)t.m_shadowType);
		putU32(realBits(t.m_minOpacity));
		putU32(realBits(t.m_maxOpacity));
		putU32(t.m_opacityThrobTime);
		putU32((unsigned)t.m_color);
		putU32(t.m_onlyVisibleToOwningPlayer ? 1 : 0);
		putU32(realBits(nd->m_deliveryDecalRadius));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	gl.m_awakenHook = r35Awaken;
	InitGameLogicRandom(cfg[1]);
	ThePartitionManager = &g_ahPm;
	TheParticleSystemManager = &g_ahPsm;
	g_objXformSyncPos = true;
	g_r28ThingPose = true;   /* (setOrientation keeps the angle, batch R28's way) */
	g_distPosDcHook = r57DistPos;   /* (l9_slaved_r57.inc's, earlier in this unit) */
	g_instanceHook = r67Instance;
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 4; ++k) {
		g_r67Objs[k] = newObject();
		g_r67Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r67Objs[k]->m_behaviors = none;
	}
	g_r67Exists[0] = true;
	g_r67Exists[1] = (cfg[8] & 1) != 0;
	g_r67Exists[2] = (cfg[17] & 1) != 0;
	g_r67Exists[3] = true;
	Object *me = g_r67Objs[0], *launcher = g_r67Objs[1];
	Player player;
	player.m_idx = 0;
	Team team;
	team.m_dummy = 1;
	team.m_controller = &player;
	if (cfg[2] >> 1 & 1) { me->m_team = &team; me->m_player = &player; }
	Drawable myDrawable, launcherDrawable;
	myDrawable.m_ownerObject = me;
	launcherDrawable.m_ownerObject = launcher;
	me->m_drawable = (cfg[2] & 1) ? &myDrawable : 0;
	launcher->m_drawable = (cfg[8] >> 1 & 1) ? &launcherDrawable : 0;
	ExperienceTracker xp;
	xp.m_owner = me;
	me->m_xp = &xp;
	MBody body;
	body.m_owner = me;
	me->m_body = &body;
	me->m_geometry = GeometryInfo(GEOMETRY_SPHERE, false, 1.0f, r67F(cfg[7]), r67F(cfg[7]));
	me->m_pos.x = r67F(cfg[3]); me->m_pos.y = r67F(cfg[4]); me->m_pos.z = r67F(cfg[5]);
	me->setOrientation(r67F(cfg[6]));
	me->poseMatrix();
	launcher->m_pos.x = r67F(cfg[9]); launcher->m_pos.y = r67F(cfg[10]); launcher->m_pos.z = r67F(cfg[11]);
	launcher->setOrientation(r67F(cfg[12]));
	launcher->poseMatrix();
	g_r67OffsetFound = (cfg[8] >> 2 & 1) != 0;
	g_r67Offset.Make_Identity();
	g_r67Offset.Rotate_Z(r67F(cfg[16]));
	g_r67Offset.Set_Translation(Vector3(r67F(cfg[13]), r67F(cfg[14]), r67F(cfg[15])));
	for (int k = 2; k < 4; ++k) {
		Object *o = g_r67Objs[k];
		const unsigned *p = &cfg[k == 2 ? 18 : 21];
		o->m_pos.x = r67F(p[0]); o->m_pos.y = r67F(p[1]); o->m_pos.z = r67F(p[2]);
	}
	R50PhysicsBehaviorModuleData physData;
	R50PhysicsBehavior *phys = 0;
	BehaviorModule *launcherList[2] = { 0, 0 };
	if (cfg[8] >> 3 & 1) {
		phys = newInstance(R50PhysicsBehavior)(launcher, &physData);
		launcher->m_r50Physics = phys;
		launcherList[0] = phys;
		launcher->m_behaviors = launcherList;
	}
	g_modFindHook = r67Find;
	g_modDestroyHook = r67Destroy;
	/* (not compared, by design: the dclear of a load, as batch R20 (the Rust load has no world to tell the client); and the module build's
	** FXList::doFXObj records a NULL list, which upstream's inline FXList::doFXObj (FXList.h 151-157) skips) */
	bool loading = false;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) {
				const std::string &l = g_ulog[(size_t)e.p];
				if (l.compare(0, 10, "setorient ") == 0 || l.compare(0, 13, "settransform ") == 0) continue;
				if (loading && l.compare(0, 7, "dclear ") == 0) continue;
				log.push_back(l);
			}
			else if (e.kind == 1 && e.p) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	NeutronMissileUpdate *m = 0;
	try { m = newInstance(NeutronMissileUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	g_dsFirst = &m->m_deliveryDecal;   /* (the decal is decal 0) */
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	const ParticleSystemTemplate *exhaust = TheParticleSystemManager->findTemplate(AsciiString("EXH_A"));
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 1) gl.m_frame = o[1];
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
		Coord3D f3;
		f3.x = r67F(o[1]); f3.y = r67F(o[2]); f3.z = r67F(o[3]);
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r67Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r67Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 6: {
				Coord3D pos;
				pos.x = r67F(o[2]); pos.y = r67F(o[3]); pos.z = r67F(o[4]);
				m->projectileLaunchAtObjectOrPosition((o[1] & 1) ? r67Find((ObjectID)3) : 0, &pos, r67Find((ObjectID)2), (WeaponSlotType)(o[1] >> 1 & 3),
					(Int)(o[1] >> 3 & 3), 0, (o[1] >> 5 & 1) ? exhaust : 0);
				break;
			}
			case 7: res = m->projectileHandleCollision(o[1] ? r67Find((ObjectID)o[1]) : 0) ? 1 : 0; break;
			case 8: { DamageInfo info; m->onDie(&info); break; }
			case 9: if (phys) phys->addVelocityTo(&f3); break;
			case 10: me->m_pos = f3; break;
			case 11: me->setOrientation(r67F(o[1])); me->poseMatrix(); break;
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
		putU32((unsigned)m->m_state);
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_targetPos.x)[k]));
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_intermedPos.x)[k]));
		putU32((unsigned)m->m_launcherID);
		putU32((unsigned)m->m_attach_wslot);
		putU32((unsigned)m->m_attach_specificBarrelToUse);
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_accel.x)[k]));
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_vel.x)[k]));
		putU32(m->m_stateTimestamp);
		putU32(m->m_isLaunched ? 1 : 0);
		putU32(m->m_isArmed ? 1 : 0);
		putU32(realBits(m->m_noTurnDistLeft));
		putU32(m->m_reachedIntermediatePos ? 1 : 0);
		putU32(m->m_frameAtLaunch);
		putU32(realBits(m->m_heightAtLaunch));
		putU32(m->m_deliveryDecal.isEmpty() ? 1 : 0);
		for (int k = 0; k < 3; ++k) putU32(realBits((&me->m_pos.x)[k]));
		const Matrix3D *mx = me->getTransformMatrix();
		for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) putU32(realBits((*mx)[r][c]));
		putU32(me->m_status.test(OBJECT_STATUS_NO_COLLISIONS) ? 1 : 0);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	g_objXformSyncPos = false;
	g_r28ThingPose = false;
	g_distPosDcHook = 0;
	g_instanceHook = 0;
	g_dsFirst = 0;
	me->m_behaviors = 0;
	delete m;
	delete phys;
	delete d;
	for (int k = 0; k < 4; ++k) {
		Object *x = g_r67Objs[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_drawable = 0; x->m_xp = 0; x->m_body = 0; x->m_r50Physics = 0;
		delete x;
		g_r67Objs[k] = 0;
	}
	ThePartitionManager = 0;
	TheParticleSystemManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r67_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r67RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
