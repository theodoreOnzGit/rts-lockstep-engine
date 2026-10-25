/* Differential-test driver for CountermeasuresBehavior (p1w13-sonnet, batch R52; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/CountermeasuresBehavior.cpp   (#included as mod_gen/CountermeasuresBehavior_r52.cpp:
**     its test of the end iterator as a pointer made the test of the storage it stands for, by build.rs; its header unchanged)
** in wrap_module.cpp's unit, after differential/l9_physics_r50.inc: the physics behaviours are the REAL PhysicsBehavior compiled there under
** R50 names (PhysicsBehavior and getPhysics are R50PhysicsBehavior and r50GetPhysics while the file is included). The scene, on the module
** build's stand-ins: the aircraft (id 1: an angle, a position, maybe a physics behaviour whose velocity the test gives, maybe a controlling
** player with or without a default team, the airborne-target status, maybe effectively dead); two missiles (ids 2 and 3), each maybe with a
** projectile update whose setFramesTillCountermeasureDiversionOccurs is a line; the thing factory (FLARE_A, whose objects get a physics
** behaviour of the default data, and FLARE_B, whose objects get none; the objects are ids 4, 5, ...); findObjectByID over the objects the test
** has not removed; the partition manager's distance (the plane's, a line); the transform (Thing's: the angle posed at the position, R28's);
** the game logic (its frame, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them); and the REAL UpdateModule,
** UpgradeMux, UpgradeCenter, INI, Xfer and random values. */
#include "vector2.h"
#define PhysicsBehavior R50PhysicsBehavior
#define getPhysics r50GetPhysics
#include "CountermeasuresBehavior_r52.cpp"
#undef getPhysics
#undef PhysicsBehavior

/* Thing.cpp 122-128, verbatim but for the transform's name (the shim object's is getTransformMatrix()) */
void Object::getUnitDirectionVector3D(Coord3D& dir) const
{
	Vector3 vdir = getTransformMatrix()->Get_X_Vector();
	vdir.Normalize();
	dir.x = vdir.X;
	dir.y = vdir.Y;
	dir.z = vdir.Z;
}

namespace {

ModuleData *l9r52Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = CountermeasuresBehavior::friend_newModuleData(&ini);
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

/* a projectile update whose diversion call is a line */
class R52Projectile : public UpdateModule, public ProjectileUpdateInterface
{
public:
	R52Projectile(Thing *thing, const ModuleData *d) : UpdateModule(thing, d) {}
	virtual ProjectileUpdateInterface *getProjectileUpdateInterface() { return this; }
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
	virtual void projectileLaunchAtObjectOrPosition(const Object *, const Coord3D *, const Object *, WeaponSlotType, Int, const WeaponTemplate *, const ParticleSystemTemplate *) {}
	virtual void projectileFireAtObjectOrPosition(const Object *, const Coord3D *, const WeaponTemplate *, const ParticleSystemTemplate *) {}
	virtual Bool projectileIsArmed() const { return true; }
	virtual ObjectID projectileGetLauncherID() const { return INVALID_ID; }
	virtual Bool projectileHandleCollision(Object *) { return false; }
	virtual void setFramesTillCountermeasureDiversionOccurs(UnsignedInt frames) { ulog("projdecoy %d %u", (int)getObject()->getID(), frames); }
	virtual void projectileNowJammed() {}
	virtual void crc(Xfer *) {}
	virtual void xfer(Xfer *) {}
	virtual void loadPostProcess() {}
	virtual NameKeyType getModuleNameKey() const { return (NameKeyType)0; }   /* (no lookup by name reaches it) */
};

std::vector<unsigned char> g_r52Saved;
UpdateModule *g_r52Me = 0;
UpdateModule *g_r52Updating = 0;
int g_r52Wakes = 0;
UnsignedInt g_r52WakeFrame = 0;
void r52Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	if (u == g_r52Me) { ++g_r52Wakes; g_r52WakeFrame = whenToWakeUp; }
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r52Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
/* the objects findObjectByID finds; the flares the factory made */
std::vector<Object *> g_r52Objects;
std::vector<bool> g_r52Found;
std::vector<R50PhysicsBehavior *> g_r52FlarePhys;
std::vector<BehaviorModule **> g_r52FlareLists;
ThingTemplate g_r52Templates[2];
R50PhysicsBehaviorModuleData *g_r52FlareData = 0;
Object *r52Find(ObjectID id)
{
	for (size_t k = 0; k < g_r52Objects.size(); ++k) if (g_r52Objects[k]->m_oid == id && g_r52Found[k]) return g_r52Objects[k];
	return 0;
}
const ThingTemplate *r52FindTemplate(const AsciiString &name)
{
	for (int k = 0; k < 2; ++k) if (g_r52Templates[k].m_name.compare(name) == 0) return &g_r52Templates[k];
	return 0;
}
Object *r52NewObject(const ThingTemplate *tmplate, Team *team)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)(g_r52Objects.size() + 1);
	o->m_team = team;
	BehaviorModule **list = new BehaviorModule *[2];
	list[0] = list[1] = 0;
	R50PhysicsBehavior *p = 0;
	if (tmplate == &g_r52Templates[0]) {
		p = newInstance(R50PhysicsBehavior)(o, g_r52FlareData);
		/* (the Rust world schedules an update module with no wake frame at the next frame) */
		if (p->friend_getNextCallFrame() == 0) p->friend_setNextCallFrame(TheGameLogic->getFrame() ? TheGameLogic->getFrame() : 1);
		list[0] = p;
	}
	o->m_behaviors = list;
	o->m_r50Physics = p;
	g_r52Objects.push_back(o);
	g_r52Found.push_back(true);
	g_r52FlarePhys.push_back(p);
	g_r52FlareLists.push_back(list);
	return o;
}
Real r52Dist(const Object *a, const Object *b)
{
	ulog("distsq %d %d", (int)a->m_oid, (int)b->m_oid);
	Real dx = b->m_pos.x - a->m_pos.x, dy = b->m_pos.y - a->m_pos.y;
	return dx * dx + dy * dy;
}
Real r52F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a CountermeasuresBehavior and make the module on the aircraft (id 1). `cfg` (8 words): 0 the
** frame, 1 the random seed, 2 flags (0 a physics behaviour, 1 a controlling player, 2 airborne target, 3 effectively dead, 4 missile 2 has a
** projectile update, 5 missile 3 has one, 6 the player has a default team), 3 the angle, 4-6 the position (float bits). The data dump goes
** first (the mux, the flare and bone names, the evasion rate, the volley size, arc, velocity factor and delay, the number of volleys, the
** reload, decoy and reaction frames, the airfield flag); then the constructor's wake calls. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 the frame += 1
** 3 xfer save  4 xfer load of the last save  5 xfer CRC  6 attemptUpgrade(m)  7 resetUpgrade(m) (m: a..d, 128 bits)  8
** reportMissileForCountermeasures(object a, 0 NULL)  9 calculateCountermeasureToDivertTo(the aircraft)  10 reloadCountermeasures()  11 isActive()
** 12 flags = a (bits 1-3 and 6 of cfg[2])  13 the aircraft's physics addVelocityTo(a, b, c)  14 its transferVelocityTo(itself)  15 object a
** no longer found  16 the aircraft's angle = a (setOrientation). Each op appends: the CATCH_ALL status, the result, isAlreadyUpgraded, the
** effect log, the wake calls (count, last frame), the xfer status, bytes and CRC, the module's state, and the flares (count, then each: id,
** found, position, angle, physics (velocity, acceleration, speed and motive expiry, or none)). */
static int l9r52RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r52Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r52Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[8];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const CountermeasuresBehaviorModuleData *cd = (const CountermeasuresBehaviorModuleData *)d;
		putMux(cd->m_upgradeMuxData);
		putStr(cd->m_flareTemplateName.str(), (int)cd->m_flareTemplateName.getLength());
		putStr(cd->m_flareBoneBaseName.str(), (int)cd->m_flareBoneBaseName.getLength());
		putU32(realBits(cd->m_evasionRate));
		putU32(cd->m_volleySize);
		putU32(realBits(cd->m_volleyArcAngle));
		putU32(realBits(cd->m_volleyVelocityFactor));
		putU32(cd->m_framesBetweenVolleys);
		putU32(cd->m_numberOfVolleys);
		putU32(cd->m_reloadFrames);
		putU32(cd->m_missileDecoyFrames);
		putU32(cd->m_countermeasureReactionFrames);
		putU32(cd->m_mustReloadAtAirfield ? 1 : 0);
	}
	GlobalData globals;
	TheGlobalData = &globals;
	g_ddGravity = globals.m_gravity;
	ref_weapon_lookup_hook = kWeaponLookup;
	TheWeaponStore = &g_kStore;
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[1]);
	Player player;
	player.m_idx = 0;
	ThingFactory factory;
	TheThingFactory = &factory;
	g_r52Templates[0].m_name = AsciiString("FLARE_A");
	g_r52Templates[1].m_name = AsciiString("FLARE_B");
	g_r52FlareData = new R50PhysicsBehaviorModuleData;
	g_findTemplateHook = r52FindTemplate;
	g_newObjectHook = r52NewObject;
	g_modFindHook = r52Find;
	g_distHook = r52Dist;
	g_r28ThingPose = true;
	g_objXformSyncPos = true;
	g_r52Objects.clear();
	g_r52Found.clear();
	g_r52FlarePhys.clear();
	g_r52FlareLists.clear();
	gl.m_awakenHook = r52Awaken;
	R50PhysicsBehaviorModuleData physData;
	Object *objs[3];
	BehaviorModule *lists[3][2] = { { 0, 0 }, { 0, 0 }, { 0, 0 } };
	UpdateModuleData projData;
	R52Projectile *proj[2] = { 0, 0 };
	for (int k = 0; k < 3; ++k) {
		objs[k] = newObject();
		objs[k]->m_oid = (ObjectID)(k + 1);
		objs[k]->m_behaviors = lists[k];
		g_r52Objects.push_back(objs[k]);
		g_r52Found.push_back(true);
		g_r52FlarePhys.push_back(0);
		g_r52FlareLists.push_back(0);
	}
	Object *me = objs[0];
	Team team;
	team.m_dummy = 101;
	R50PhysicsBehavior *phys = 0;
	if (cfg[2] & 1) {
		phys = newInstance(R50PhysicsBehavior)(me, &physData);
		me->m_r50Physics = phys;
	}
	for (int k = 0; k < 2; ++k) if (cfg[2] >> (4 + k) & 1) { proj[k] = new R52Projectile(objs[k + 1], &projData); lists[k + 1][0] = proj[k]; }
	auto scene = [&]() {
		unsigned f = cfg[2];
		me->m_player = (f >> 1 & 1) ? &player : 0;
		player.m_defaultTeam = (f >> 6 & 1) ? &team : 0;
		me->m_status.set(OBJECT_STATUS_AIRBORNE_TARGET, (f >> 2 & 1) ? 1 : 0);
		me->m_effDead = (f >> 3 & 1) != 0;
	};
	scene();
	me->m_pos.x = r52F(cfg[4]); me->m_pos.y = r52F(cfg[5]); me->m_pos.z = r52F(cfg[6]);
	me->setOrientation(r52F(cfg[3]));
	g_r52Wakes = 0;
	g_r52WakeFrame = 0;
	CountermeasuresBehavior *m = 0;
	status = 0;
	try { m = newInstance(CountermeasuresBehavior)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;
	g_r52Me = m;
	lists[0][phys ? 1 : 0] = m;
	if (phys) lists[0][0] = phys;
	/* (the constructor's wake call came before g_r52Me was the module: it is the module's own) */
	putU32(1);
	putU32(m->friend_getNextCallFrame());
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 1) gl.m_frame = o[1];
		if (o[0] == 2) gl.m_frame += 1;
		if (o[0] == 12) cfg[2] = (cfg[2] & ~0x4Eu) | (o[1] & 0x4Eu);
		scene();
		g_events.clear();
		g_ulog.clear();
		g_r52Wakes = 0;
		g_r52WakeFrame = 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[1024];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		Coord3D f3;
		f3.x = r52F(o[1]); f3.y = r52F(o[2]); f3.z = r52F(o[3]);
		try {
			switch (o[0]) {
			case 0: g_r52Updating = m; res = (unsigned)m->update(); break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 1024, &blen, &crc); g_r52Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r52Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 6: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 7: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 8: m->reportMissileForCountermeasures(o[1] == 2 || o[1] == 3 ? objs[o[1] - 1] : 0); break;
			case 9: res = (unsigned)m->calculateCountermeasureToDivertTo(*me); break;
			case 10: m->reloadCountermeasures(); break;
			case 11: res = m->isActive() ? 1 : 0; break;
			case 13: if (phys) phys->addVelocityTo(&f3); break;
			case 14: if (phys) phys->transferVelocityTo(phys); break;
			case 15: for (size_t k = 0; k < g_r52Objects.size(); ++k) if (g_r52Objects[k]->m_oid == (ObjectID)o[1]) g_r52Found[k] = false; break;
			case 16: me->setOrientation(r52F(o[1])); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r52Updating = 0;
		putI32(st);
		putU32(res);
		putU32(m->isAlreadyUpgraded() ? 1 : 0);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) {
				const std::string &l = g_ulog[(size_t)e.p];
				/* (the angles and transforms are dumped: their lines are not) */
				if (l.compare(0, 10, "setorient ") == 0 || l.compare(0, 13, "settransform ") == 0) continue;
				log.push_back(l);
			}
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)g_r52Wakes);
		putU32(g_r52Wakes ? g_r52WakeFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_availableCountermeasures);
		putU32(m->m_activeCountermeasures);
		putU32(m->m_divertedMissiles);
		putU32(m->m_incomingMissiles);
		putU32(m->m_reactionFrame);
		putU32(m->m_nextVolleyFrame);
		putU32(m->m_reloadFrame);
		putU32(m->isAlreadyUpgraded() ? 1 : 0);
		putU32((unsigned)m->m_counterMeasures.size());
		for (size_t j = 0; j < m->m_counterMeasures.size(); ++j) putU32((unsigned)m->m_counterMeasures[j]);
		putU32((unsigned)(g_r52Objects.size() - 3));
		for (size_t k = 3; k < g_r52Objects.size(); ++k) {
			Object *f = g_r52Objects[k];
			putU32((unsigned)f->m_oid);
			putU32(g_r52Found[k] ? 1 : 0);
			putU32(realBits(f->m_pos.x)); putU32(realBits(f->m_pos.y)); putU32(realBits(f->m_pos.z));
			putU32(realBits(f->m_orientation));
			R50PhysicsBehavior *p = g_r52FlarePhys[k];
			putU32(p ? 1 : 0);
			if (p) {
				putU32(realBits(p->m_vel.x)); putU32(realBits(p->m_vel.y)); putU32(realBits(p->m_vel.z));
				putU32(realBits(p->m_accel.x)); putU32(realBits(p->m_accel.y)); putU32(realBits(p->m_accel.z));
				putU32(realBits(p->m_velMag));
				putU32(p->m_motiveForceExpires);
			}
		}
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_r52Me = 0;
	delete m;
	delete phys;
	for (int k = 0; k < 2; ++k) delete proj[k];
	for (size_t k = 0; k < g_r52Objects.size(); ++k) {
		g_r52Objects[k]->m_behaviors = 0;
		g_r52Objects[k]->m_r50Physics = 0;
		g_r52Objects[k]->m_player = 0;
		delete g_r52FlarePhys[k];
		delete[] g_r52FlareLists[k];
		delete g_r52Objects[k];
	}
	g_r52Objects.clear();
	g_r52Found.clear();
	g_r52FlarePhys.clear();
	g_r52FlareLists.clear();
	delete g_r52FlareData;
	g_r52FlareData = 0;
	delete d;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_modFindHook = 0;
	g_distHook = 0;
	g_r28ThingPose = false;
	g_objXformSyncPos = false;
	ref_weapon_lookup_hook = 0;
	TheWeaponStore = 0;
	TheGlobalData = 0;
	g_ddGravity = 0;
	TheThingFactory = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r52_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r52RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
