/* Differential-test driver for MinefieldBehavior (p1w13-sonnet, batch R40; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/MinefieldBehavior.cpp   (#included with one mechanical edit made by build.rs:
**   mod_gen/MinefieldBehavior_edit.cpp, update's function-local static name key made a plain local)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: six objects (ids 1-6; a team of three whose relationships the test
** gives, the kinds INFANTRY, DOZER and STRUCTURE, effectively dead, maybe the module build's MBody (attemptDamage, attemptHealing and, on the
** mine, internalChangeHealth lines), maybe an AI (clearing mines or not, the test's goal), a position), the mine (object 1: a drawable whose
** model conditions are lines, a geometry, a producer, maybe the REAL AutoHealBehavior), the terrain's eight cells (ground, layer, layer
** height), setLayer (a line), the weapon store (WEAP_A, WEAP_B; createAndFireTempWeapon a line), the object creation lists (OCL_A, OCL_B;
** create a line), destroyObject (a line), the REAL Object::getRelationship, GeometryInfo::clipPointToFootprint, UpdateModule, INI and Xfer;
** the game logic (its frame, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them, counted per module). */
#define min(x,y) (((x)<(y)) ? (x) : (y))   /* Lib/BaseType.h, verbatim (the module build leaves the macro out) */
#include "MinefieldBehavior_edit.cpp"
#undef min

namespace {

ModuleData *l9r40Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = MinefieldBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r40Saved;
UpdateModule *g_r40Updating = 0;
UpdateModule *g_r40Mine = 0, *g_r40Ahb = 0;
unsigned g_r40Wakes[2], g_r40WakeLast[2];
int g_r40Ctor = -1;   /* the module being constructed (0 the mine, 1 the AutoHealBehavior), whose pointer is not known yet */
void r40Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	int k = u == g_r40Mine ? 0 : (u == g_r40Ahb ? 1 : g_r40Ctor);
	if (k >= 0) { ++g_r40Wakes[k]; g_r40WakeLast[k] = whenToWakeUp; }
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r40Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *g_r40Objs[6];
Object *r40Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 6) ? g_r40Objs[(int)id - 1] : 0; }
Object *r40Find(unsigned id) { return r40Find((ObjectID)id); }
const KindOfType g_r40Kinds[3] = { KINDOF_INFANTRY, KINDOF_DOZER, KINDOF_STRUCTURE };
/* the mine's body maximum, by index (the same list in the Rust test) */
const Real g_r40Max[4] = { 100.0f, 30.0f, 1000.0f, 7.0f };
Real r40F(unsigned bits) { Real f; memcpy(&f, &bits, 4); return f; }
Team g_r40Teams[3];
Player g_r40Players[3];
int g_r40Rel[3][3];
Relationship r40TeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return (Relationship)g_r40Rel[from->m_dummy - 1][to->m_dummy - 1];
}
void r40Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
bool g_r40HasAhb = false;
UpdateModule *r40FindUpdate(const Object *obj, NameKeyType key)
{
	if (obj == g_r40Objs[0] && g_r40HasAhb && key == TheNameKeyGenerator->nameToKey("AutoHealBehavior")) return g_r40Ahb;
	return 0;
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a MinefieldBehavior and make it on object 1 of six at frame cfg[0] (with the AutoHealBehavior,
** made first, if cfg[11]), then give the mine its producer cfg[10]. `cfg` (24 words; 1-10 also settable by op 2): 2-7 objects 1-6: bits 0-1
** the team (0 none, else 1-3), 2-4 the kinds (of g_r40Kinds), 5 effectively dead, 6 a body, 7 an AI, 8 the AI clearing mines; 8 the teams'
** relationships (2 bits each, row-major, team a to team b: 0 enemies, 1 neutral, 2 allies); 9 the AIs' goal object; 10 the mine's producer;
** 11 an AutoHealBehavior (read once); 12-17 objects 1-6's x (float bits; y is 10 k - 20, z is 0); 18 the mine's geometry (bits 0-1 the type,
** 2-3 the major radius of 1, 4, 10, 0.5, 4-5 the minor likewise; read once); 19-21 the eight cells' layers (nibbles: 1 ground, 2-3
** bridges), layer heights and ground heights (nibbles, as halves); 22 the mine's body maximum (of g_r40Max; read once; the health starts
** at the maximum). Every body's health starts at its maximum (1000 but the mine's). Then `n` ops of 4 words [op, a, b, c]: 0 update()
** 1 the frame = a  2 cfg[a] = b  3 onCollide(object a)  4 the mine's body takes damage (amount a as float bits, type b & 255, death b >> 8,
** source c), then onDamage  5 the body is healed (amount a, source c), then onHealing  6 onDie  7 setScootParms((a, 5, c), (b, 7, 0)) as
** float bits  8 disarm  9 xfer save  10 xfer load of the last save  11 xfer CRC. Each op appends: the CATCH_ALL status, the result, the
** effect log, the mine's and the AutoHealBehavior's wake calls (count, last frame), the xfer status, bytes and CRC, the mine's state, its
** position, its MASKED and NO_ATTACK_FROM_AI bits, and every body's health. */
int ref_l9r40_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r40Saved.clear();
	*len = 0;
	unsigned cfg[24];
	memcpy(cfg, cfg_in, sizeof(cfg));
	ref_weapon_lookup_hook = kWeaponLookup;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r40Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_weapon_lookup_hook = 0;
	if (status) return status;
	{
		const MinefieldBehaviorModuleData *md = (const MinefieldBehaviorModuleData *)d;
		const KWeapon *w = (const KWeapon *)md->m_detonationWeapon;
		putI32(w == 0 ? 0 : (w == &g_kWeapons[0] ? 1 : 2));
		putU32((unsigned)md->m_detonatedBy);
		putU32(md->m_stopsRegenAfterCreatorDies ? 1 : 0);
		putU32(md->m_regenerates ? 1 : 0);
		putU32(md->m_workersDetonate ? 1 : 0);
		putU32(md->m_creatorDeathCheckRate);
		putU32(md->m_scootFromStartingPointTime);
		putU32(md->m_numVirtualMines);
		putU32(realBits(md->m_repeatDetonateMoveThresh));
		putU32(realBits(md->m_healthPercentToDrainPerSecond));
		putI32(md->m_ocl == 0 ? 0 : (md->m_ocl == &g_upOcls[0] ? 1 : 2));
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	TheWeaponStore = &g_kStore;
	for (int t = 0; t < 3; ++t) { g_r40Teams[t].m_dummy = t + 1; g_r40Teams[t].m_controller = &g_r40Players[t]; g_r40Players[t].m_idx = t; }
	MBody bodies[6];
	AIUpdateInterface ais[6];
	Drawable draw;
	for (int k = 0; k < 6; ++k) {
		g_r40Objs[k] = newObject();
		g_r40Objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = g_r40Objs[k];
		bodies[k].m_max = k == 0 ? g_r40Max[cfg[22] & 3] : 1000.0f;
		bodies[k].m_health = bodies[k].m_max;
		ais[k].m_owner = g_r40Objs[k];
		g_r40Objs[k]->m_pos.x = r40F(cfg[12 + k]);
		g_r40Objs[k]->m_pos.y = 10.0f * k - 20.0f;
		g_r40Objs[k]->m_pos.z = 0.0f;
	}
	bodies[0].m_traceChange = true;
	{
		const Real radii[4] = { 1.0f, 4.0f, 10.0f, 0.5f };
		unsigned gt = cfg[18] & 3;
		GeometryType type = gt == 0 ? GEOMETRY_SPHERE : (gt == 1 ? GEOMETRY_CYLINDER : GEOMETRY_BOX);
		g_r40Objs[0]->m_geometry = GeometryInfo(type, false, 5.0f, radii[cfg[18] >> 2 & 3], radii[cfg[18] >> 4 & 3]);
	}
	for (int c = 0; c < 8; ++c) {
		g_hdLayer[c] = (int)(cfg[19] >> (4 * c) & 15);
		g_hdLayerHeight[c] = (Real)(cfg[20] >> (4 * c) & 15) * 0.5f;
		g_hdGround[c] = (Real)(cfg[21] >> (4 * c) & 15) * 0.5f;
	}
	Object *me = g_r40Objs[0];
	me->m_drawable = &draw;
	auto scene = [&]() {
		for (int k = 0; k < 6; ++k) {
			Object *o = g_r40Objs[k];
			unsigned f = cfg[2 + k];
			unsigned t = f & 3;
			o->m_team = t ? &g_r40Teams[t - 1] : 0;
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 3; ++b) if (f >> (2 + b) & 1) o->m_kindOf.set(g_r40Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_effDead = (f >> 5 & 1) != 0;
			o->m_body = (f >> 6 & 1) ? &bodies[k] : 0;
			o->m_ai = (f >> 7 & 1) ? &ais[k] : 0;
			ais[k].m_clearingMines = (f >> 8 & 1) != 0;
			ais[k].m_goal = r40Find(cfg[9]);
		}
		for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r40Rel[a][b] = (int)(cfg[8] >> (2 * (3 * a + b)) & 3);
	};
	scene();
	g_teamRelHook = r40TeamRel;
	g_modFindHook = r40Find;
	g_modDestroyHook = r40Destroy;
	g_findUpdateHook = r40FindUpdate;
	g_r40HasAhb = cfg[11] != 0;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r40Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	for (int k = 0; k < 2; ++k) g_r40Wakes[k] = g_r40WakeLast[k] = 0;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	auto putWakes = [&]() {
		for (int k = 0; k < 2; ++k) { putU32(g_r40Wakes[k]); putU32(g_r40WakeLast[k]); g_r40Wakes[k] = g_r40WakeLast[k] = 0; }
	};
	g_r40Ctor = 0;
	MinefieldBehavior *m = new MinefieldBehavior(me, d);
	g_r40Mine = m;
	AutoHealBehaviorModuleData *ahd = new AutoHealBehaviorModuleData;
	AutoHealBehavior *ahb = 0;
	g_r40Ctor = 1;
	if (g_r40HasAhb) { ahb = new AutoHealBehavior(me, ahd); g_r40Ahb = ahb; }
	g_r40Ctor = -1;
	BehaviorModule *behaviors[3] = { m, ahb, 0 };
	me->m_behaviors = behaviors;
	putLog();
	putWakes();
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	/* the producer is set after the object is made */
	me->m_producerID = (ObjectID)cfg[10];
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 2 && o[1] >= 1 && o[1] <= 10) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		if (o[0] == 2 && o[1] == 10) me->m_producerID = (ObjectID)cfg[10];
		scene();
		g_events.clear();
		g_ulog.clear();
		gl.m_awakenCalls = 0;
		gl.m_awakenFrame = 0;
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
			case 0: g_r40Updating = m; res = (unsigned)m->update(); break;
			case 3: { Object *other = r40Find(o[1]); m->onCollide(other, other ? other->getPosition() : 0, 0); break; }
			case 4: {
				DamageInfo di;
				di.in.m_amount = r40F(o[1]);
				di.in.m_damageType = (DamageType)(o[2] & 255);
				di.in.m_deathType = (DeathType)(o[2] >> 8);
				di.in.m_sourceID = (ObjectID)o[3];
				if (me->m_body) me->m_body->attemptDamage(&di);
				m->onDamage(&di);
				break;
			}
			case 5: {
				DamageInfo di;
				di.in.m_amount = r40F(o[1]);
				di.in.m_damageType = DAMAGE_HEALING;
				di.in.m_deathType = DEATH_NONE;
				di.in.m_sourceID = (ObjectID)o[3];
				if (me->m_body) me->m_body->attemptHealing(&di);
				m->onHealing(&di);
				break;
			}
			case 6: { DamageInfo di; m->onDie(&di); break; }
			case 7: {
				Coord3D s, e;
				s.x = r40F(o[1]); s.y = 5.0f; s.z = r40F(o[3]);
				e.x = r40F(o[2]); e.y = 7.0f; e.z = 0.0f;
				m->setScootParms(s, e);
				break;
			}
			case 8: m->disarm(); break;
			case 9: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r40Saved.assign(bytes, bytes + blen); break;
			case 10: {
				std::vector<unsigned char> b = g_r40Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 11: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r40Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putWakes();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		/* the state, in the Rust module's state_words order */
		std::vector<unsigned> w;
		w.push_back(m->m_nextDeathCheckFrame);
		w.push_back(m->m_scootFramesLeft);
		w.push_back(realBits(m->m_scootVel.x)); w.push_back(realBits(m->m_scootVel.y)); w.push_back(realBits(m->m_scootVel.z));
		w.push_back(realBits(m->m_scootAccel.x)); w.push_back(realBits(m->m_scootAccel.y)); w.push_back(realBits(m->m_scootAccel.z));
		w.push_back(m->m_virtualMinesRemaining);
		for (int k = 0; k < 3; ++k) { w.push_back((unsigned)m->m_immunes[k].id); w.push_back(m->m_immunes[k].collideTime); }
		w.push_back(m->m_ignoreDamage ? 1 : 0);
		w.push_back(m->m_regenerates ? 1 : 0);
		w.push_back(m->m_draining ? 1 : 0);
		w.push_back((unsigned)m->m_detonators.size());
		for (size_t k = 0; k < m->m_detonators.size(); ++k) {
			w.push_back((unsigned)m->m_detonators[k].id);
			w.push_back(realBits(m->m_detonators[k].where.x)); w.push_back(realBits(m->m_detonators[k].where.y)); w.push_back(realBits(m->m_detonators[k].where.z));
		}
		putU32((unsigned)w.size());
		for (size_t k = 0; k < w.size(); ++k) putU32(w[k]);
		putU32(realBits(me->m_pos.x));
		putU32(realBits(me->m_pos.y));
		putU32(realBits(me->m_pos.z));
		putU32(me->m_status.test(OBJECT_STATUS_MASKED) ? 1 : 0);
		putU32(me->m_status.test(OBJECT_STATUS_NO_ATTACK_FROM_AI) ? 1 : 0);
		for (int k = 0; k < 6; ++k) putU32(realBits(bodies[k].m_health));
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_teamRelHook = 0;
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	g_findUpdateHook = 0;
	TheWeaponStore = 0;
	me->m_behaviors = 0;
	me->m_drawable = 0;
	delete m;
	delete ahb;
	delete ahd;
	g_r40Mine = g_r40Ahb = 0;
	delete d;
	for (int k = 0; k < 6; ++k) {
		Object *x = g_r40Objs[k];
		x->m_body = 0; x->m_team = 0; x->m_ai = 0;
		delete x;
		g_r40Objs[k] = 0;
	}
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
