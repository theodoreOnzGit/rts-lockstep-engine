/* Differential-test driver for StickyBombUpdate (p1w13-sonnet, batch R39; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StickyBombUpdate.cpp   (#included with one mechanical edit made by build.rs:
**   mod_gen/StickyBombUpdate_edit.cpp, initStickyBomb's function-local static name key made a plain local; the class renamed R39StickyBombUpdate,
**   because R15's driver already declares StickyBombUpdate in this unit and stands in for two of its methods)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: six objects (ids 1-6; the kinds IMMOBILE, BOOBY_TRAP and STRUCTURE,
** effectively dead, maybe the module build's MBody (health 500 of 1000; attemptDamage a line), maybe an AI whose goal the test gives, a
** sphere geometry, a position, the status bits, which the module sets and clears), the bomb (object 1: maybe a player of three, maybe a
** LifetimeUpdate stand-in with the test's die frame, a producer), the template's per-unit sounds StickyBombCreated and UnitBombPing (R39Created,
** R39Ping), the audio manager (lines), the terrain's eight cells, the scan (the objects and numerics the test lists; the call a line), the
** weapon store (WEAP_A, WEAP_B, the test's damage numbers; INI::parseWeaponTemplate through the lookup hook), the FX lists (FX_A, FX_B;
** doFXPos a line with its radius), destroyObject (a line), the REAL Object::kill, UpdateModule, INI and Xfer; the game logic (its frame, its
** wake calls applied as GameLogic::friend_awakenUpdateModule applies them). */
#define StickyBombUpdate R39StickyBombUpdate
#define StickyBombUpdateModuleData R39StickyBombUpdateModuleData
#define getStickyBombUpdateModuleData getR39StickyBombUpdateModuleData
#define max(x,y) (((x)>(y)) ? (x) : (y))   /* Lib/BaseType.h:105, verbatim (the module build leaves the macro out) */
#include "StickyBombUpdate_r39.h"   /* the header under its new name: the original is already in the unit (R15's driver) */
#include "StickyBombUpdate_edit.cpp"
#undef StickyBombUpdate
#undef StickyBombUpdateModuleData
#undef getStickyBombUpdateModuleData
#undef max

extern "C" const void *(*ref_weapon_lookup_hook)(const char *);
extern const AudioEventRTS *(*g_perUnitSoundHook)(const AsciiString &);
extern bool g_fxPosRadius;

namespace {

ModuleData *l9r39Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = R39StickyBombUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r39Saved;
UpdateModule *g_r39Updating = 0;
void r39Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r39Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *g_r39Objs[6];
Object *r39Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 6) ? g_r39Objs[(int)id - 1] : 0; }
Object *r39Find(unsigned id) { return r39Find((ObjectID)id); }
const KindOfType g_r39Kinds[3] = { KINDOF_IMMOBILE, KINDOF_BOOBY_TRAP, KINDOF_STRUCTURE };
/* the spheres' radii, by index (the same list in the Rust test) */
const Real g_r39Radius[4] = { 1.0f, 5.0f, 20.0f, 0.5f };
Real r39F(unsigned bits) { Real f; memcpy(&f, &bits, 4); return f; }
/* the weapon store's templates; the numbers are the test's (cfg 20-23) */
WeaponTemplate g_r39Weapons[2];
const void *r39WeaponLookup(const char *name)
{
	if (strcasecmp(name, "WEAP_A") == 0) return &g_r39Weapons[0];
	if (strcasecmp(name, "WEAP_B") == 0) return &g_r39Weapons[1];
	return 0;
}
AudioEventRTS g_r39Created, g_r39Ping;
const AudioEventRTS *r39Sound(const AsciiString &name)
{
	if (strcmp(name.str(), "StickyBombCreated") == 0) return &g_r39Created;
	if (strcmp(name.str(), "UnitBombPing") == 0) return &g_r39Ping;
	return 0;
}
/* the LifetimeUpdate the bomb may have: LifetimeUpdate.h's class is UpdateModule and one UnsignedInt (m_dieFrame, read by the inline
** getDieFrame); this stand-in has the same layout, and the driver hands it to the bomb's findUpdateModule as a LifetimeUpdate */
class R39Life : public UpdateModule
{
public:
	R39Life(Thing *t, const ModuleData *d) : UpdateModule(t, d), m_dieFrame(0) {}
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_FOREVER; }
	virtual NameKeyType getModuleNameKey() const { return NAMEKEY_INVALID; }
	static Int getInterfaceMask() { return UpdateModule::getInterfaceMask(); }
	UnsignedInt m_dieFrame;
};
R39Life *g_r39Life = 0;
bool g_r39HasLife = false;
UpdateModule *r39FindUpdate(const Object *obj, NameKeyType key)
{
	if (obj == g_r39Objs[0] && g_r39HasLife && key == TheNameKeyGenerator->nameToKey("LifetimeUpdate")) return g_r39Life;
	return 0;
}
void r39Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
void r39ScanHook(const Coord3D *pos, Real maxDist, PartitionFilter **)
{
	ulog("scan %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), g_ahRangeDc);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a StickyBombUpdate and make it on object 1 of six at frame cfg[0], then onObjectCreated (no
** producer yet), then give the bomb its producer cfg[17]. `cfg` (24 words; 1-18 also settable by op 2): 1 the bomb's player + 1 (0 none), 2 a LifetimeUpdate (0 none),
** 3 its die frame, 4-9 objects 1-6: bits 0-2 their kinds (of g_r39Kinds), 3 effectively dead, 4 a body, 5 an AI, 6-7 the sphere's radius (of
** g_r39Radius), 8 BOOBY_TRAPPED at the start (read once); 10 the AIs' goal object; 11-16 objects 1-6's x (float bits; y is 10 k, z is k);
** 17 the bomb's producer; 18 the scan (nibbles of object numbers 1-6, 0 ends); 19 the eight ground heights (nibbles, as halves);
** 20-23 the two weapons' primary and secondary damage (as two halves: the low half primary) and radii (likewise), as small integers; the
** numerics of the scan are 5 k^2 for its k-th object. The data dump goes first, then the constructor's and onObjectCreated's effects and
** wake calls. Then `n` ops of 4 words [op, a, b, c]: 0 update()  1 the frame = a  2 cfg[a] = b  3 initStickyBomb(object a, object b, c ?
** (c as float bits, 3, 99) : NULL)  4 detonate  5 getTargetObject  6 setTargetObject(object a)  7 isTimedBomb  8 getDetonationFrame
** 9 xfer save  10 xfer load of the last save  11 xfer CRC  12 onObjectCreated. Each op appends: the CATCH_ALL status, the result, the effect
** log, the wake calls (count, last frame), the xfer status, bytes and CRC, the state (the target, the die frame, the next ping frame), the
** bomb's position and producer, and per object BOOBY_TRAPPED and the body's health. */
int ref_l9r39_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r39Saved.clear();
	g_fxIds.clear();
	*len = 0;
	unsigned cfg[24];
	memcpy(cfg, cfg_in, sizeof(cfg));
	for (int w = 0; w < 2; ++w) {
		WeaponTemplate &t = g_r39Weapons[w];
		t.m_primaryDamage = (Real)(cfg[20 + w] & 0xFFFF);
		t.m_secondaryDamage = (Real)(cfg[20 + w] >> 16);
		t.m_primaryDamageRadius = (Real)(cfg[22 + w] & 0xFFFF);
		t.m_secondaryDamageRadius = (Real)(cfg[22 + w] >> 16);
		t.m_damageTypeT = w == 0 ? 3 : 7;
		t.m_deathTypeT = w == 0 ? 2 : 5;
		t.m_damageStatusTypeT = w == 0 ? 0 : 4;
	}
	ref_weapon_lookup_hook = r39WeaponLookup;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r39Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_weapon_lookup_hook = 0;
	if (status) return status;
	{
		const R39StickyBombUpdateModuleData *sd = (const R39StickyBombUpdateModuleData *)d;
		putStr(sd->m_attachToBone.str(), (int)sd->m_attachToBone.getLength());
		putU32(realBits(sd->m_offsetZ));
		const WeaponTemplate *wt = sd->m_geometryBasedDamageWeaponTemplate;
		putI32(wt == 0 ? 0 : (wt == &g_r39Weapons[0] ? 1 : 2));
		putI32(fxId(sd->m_geometryBasedDamageFX));
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	ThePartitionManager = &g_ahPm;
	Player players[3];
	for (int p = 0; p < 3; ++p) players[p].m_idx = p;
	MBody bodies[6];
	AIUpdateInterface ais[6];
	for (int k = 0; k < 6; ++k) {
		g_r39Objs[k] = newObject();
		g_r39Objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = g_r39Objs[k];
		bodies[k].m_max = 1000.0f;
		bodies[k].m_health = 500.0f;
		Real r = g_r39Radius[cfg[4 + k] >> 6 & 3];
		g_r39Objs[k]->m_geometry = GeometryInfo(GEOMETRY_SPHERE, false, r, r, r);
		if (cfg[4 + k] >> 8 & 1) g_r39Objs[k]->m_status.set(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_BOOBY_TRAPPED));
	}
	for (int c = 0; c < 8; ++c) g_hdGround[c] = (Real)(cfg[19] >> (4 * c) & 15) * 0.5f;
	Object *me = g_r39Objs[0];
	g_r39Created.setEventName(AsciiString("R39Created"));
	g_r39Ping.setEventName(AsciiString("R39Ping"));
	auto scene = [&]() {
		unsigned tp = cfg[1];
		me->m_player = (tp >= 1 && tp <= 3) ? &players[tp - 1] : 0;
		g_r39HasLife = cfg[2] != 0;
		g_r39Life->m_dieFrame = cfg[3];
		for (int k = 0; k < 6; ++k) {
			Object *o = g_r39Objs[k];
			unsigned f = cfg[4 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 3; ++b) if (f >> b & 1) o->m_kindOf.set(g_r39Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_effDead = (f >> 3 & 1) != 0;
			o->m_body = (f >> 4 & 1) ? &bodies[k] : 0;
			ais[k].m_goal = r39Find(cfg[10]);
			o->m_ai = (f >> 5 & 1) ? &ais[k] : 0;
		}
		g_ahRange.clear();
		g_ahRangeNums.clear();
		for (int k = 0; k < 6; ++k) {
			unsigned id = cfg[18] >> (4 * k) & 15;
			if (id == 0) break;
			if (id <= 6) { g_ahRange.push_back(g_r39Objs[id - 1]); g_ahRangeNums.push_back(5.0f * (Real)(k * k)); }
		}
	};
	/* positions are the scene's at the start; after that the bomb moves them (object 1), the others keep theirs */
	for (int k = 0; k < 6; ++k) { g_r39Objs[k]->m_pos.x = r39F(cfg[11 + k]); g_r39Objs[k]->m_pos.y = 10.0f * k; g_r39Objs[k]->m_pos.z = (Real)k; }
	ModuleData *lifeData = new UpdateModuleData;
	g_r39Life = new R39Life(me, lifeData);
	scene();
	g_findUpdateHook = r39FindUpdate;
	g_perUnitSoundHook = r39Sound;
	g_fxPosRadius = true;
	g_modDestroyHook = r39Destroy;
	g_ahRangeHook = r39ScanHook;
	g_modFindHook = r39Find;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r39Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	R39StickyBombUpdate *m = new R39StickyBombUpdate(me, d);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	m->onObjectCreated();
	putLog();
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	/* the producer is set after the object is made (as the Rust world makes it: onObjectCreated runs inside newObject) */
	me->m_producerID = (ObjectID)cfg[17];
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 2 && o[1] >= 1 && o[1] <= 18) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
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
			case 0: g_r39Updating = m; res = (unsigned)m->update(); break;
			case 3: {
				Coord3D p; p.x = r39F(o[3]); p.y = 3.0f; p.z = 99.0f;
				m->initStickyBomb(r39Find(o[1]), r39Find(o[2]), o[3] ? &p : NULL);
				break;
			}
			case 4: m->detonate(); break;
			case 5: { Object *t = m->getTargetObject(); res = t ? (unsigned)t->m_oid : 0; break; }
			case 6: m->setTargetObject(r39Find(o[1])); break;
			case 7: res = m->isTimedBomb() ? 1 : 0; break;
			case 8: res = m->getDetonationFrame(); break;
			case 9: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r39Saved.assign(bytes, bytes + blen); break;
			case 10: {
				std::vector<unsigned char> b = g_r39Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 11: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 12: m->onObjectCreated(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r39Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32((unsigned)m->m_targetID);
		putU32(m->m_dieFrame);
		putU32(m->m_nextPingFrame);
		putU32(realBits(me->m_pos.x));
		putU32(realBits(me->m_pos.y));
		putU32(realBits(me->m_pos.z));
		putU32((unsigned)me->m_producerID);
		for (int k = 0; k < 6; ++k) {
			putU32(g_r39Objs[k]->m_status.test(OBJECT_STATUS_BOOBY_TRAPPED) ? 1 : 0);
			putU32(realBits(bodies[k].m_health));
		}
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_ahRangeNums.clear();
	g_modFindHook = 0;
	g_findUpdateHook = 0;
	g_perUnitSoundHook = 0;
	g_fxPosRadius = false;
	g_modDestroyHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	delete g_r39Life;
	g_r39Life = 0;
	delete lifeData;
	for (int k = 0; k < 6; ++k) {
		Object *x = g_r39Objs[k];
		x->m_body = 0; x->m_player = 0; x->m_ai = 0;
		delete x;
		g_r39Objs[k] = 0;
	}
	TheGameLogic = 0;
	ThePartitionManager = 0;
	return 0;
}

}  /* extern "C" */
