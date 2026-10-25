/* Differential-test driver for CleanupHazardUpdate (p1w13-sonnet, batch R56; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CleanupHazardUpdate.cpp   (#included through build.rs's mod_gen/CleanupHazardUpdate_r56.cpp:
**     unchanged but for setCleanupAreaParameters, defined as R56Cleanup::r56SetCleanupAreaParameters with its body unchanged, because the
**     class's own name is special power batch H's stand-in (differential/l9_powers_h.inc, the line CleanupAreaPower's test reads); the
**     module is made as an R56Cleanup, a subclass that adds nothing else)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: five objects (ids 1-5; x and y floats of the
** test's), the unit (object 1: maybe an AI whose isIdle, isBusy and last command source are the test's and whose aiBusy, aiMoveToPosition and
** aiAttackObject are lines; maybe a weapon in each slot, its template's attack range the test's), the closest hazard (the test's: the call a
** line, batch R34's), the distances (to an object: in the plane, from the positions; to a position: batch R's "cdist" line),
** Object::setWeaponLock (batch R55's line), Object::setWeaponSetFlag (a line), the REAL logic random values (the test's seed), UpdateModule,
** INI and Xfer; the game logic (its frame). */
class R56Cleanup : public CleanupHazardUpdate
{
public:
	R56Cleanup( Thing *thing, const ModuleData* moduleData ) : CleanupHazardUpdate( thing, moduleData ) {}
	void r56SetCleanupAreaParameters( const Coord3D *pos, Real range );
};
#include "CleanupHazardUpdate_r56.cpp"   /* (build.rs, mod_gen) */

void AIUpdateInterface::aiBusy(CommandSourceType cmdSource) { ulog("aibusy %d %d", (int)m_owner->m_oid, (int)cmdSource); }
void AIUpdateInterface::aiAttackObject(Object *victim, Int maxShotsToFire, CommandSourceType cmdSource)
{
	ulog("aiattack %d %d %d %d", (int)m_owner->m_oid, victim ? (int)victim->m_oid : 0, (int)maxShotsToFire, (int)cmdSource);
}

namespace {

ModuleData *l9r56Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = CleanupHazardUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r56Saved;
Object *g_r56Objs[5];
Object *r56Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 5) ? g_r56Objs[(int)id - 1] : 0; }
Real r56Dist(const Object *a, const Object *b)
{
	Real dx = b->m_pos.x - a->m_pos.x, dy = b->m_pos.y - a->m_pos.y;
	return dx * dx + dy * dy;
}
Real r56F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a CleanupHazardUpdate and make the module on object 1 of five at frame cfg[0], then onObjectCreated.
** `cfg` (16 words; 2-14 also settable by op 2): 1 the random seed, 2 the AI: bit 0 there is one, 1 isIdle, 2 isBusy, bits 4-7 its last command
** source; 3 the weapons: per slot k (bits 4k..4k+3) 0 none, else the weapon of template (v - 1) % 2; 4-5 the two templates' attack
** ranges (float bits); 6 the closest hazard (objects 1-5, else NULL); 7-11 the objects' x, 12-14 objects 1-3's y (float bits; 4 and 5 at y 0).
** The data dump goes first, then onObjectCreated's effects and the state. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a
** 2 cfg[a] = b  3 setCleanupAreaParameters((b, c, 0) as float bits, d as float bits)  4 xfer save  5 xfer load of the last save  6 xfer CRC
** 7 onObjectCreated. Each op appends: the CATCH_ALL status, the result, the effect log, the xfer status, bytes and CRC, the state (the target,
** in range, the two countdowns, whether the weapon template was found, the area's centre and move range). */
static int l9r56RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r56Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r56Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[16];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const CleanupHazardUpdateModuleData *cd = (const CleanupHazardUpdateModuleData *)d;
		putU32((unsigned)cd->m_weaponSlot);
		putU32(cd->m_scanFrames);
		putU32(realBits(cd->m_scanRange));
	}
	InitGameLogicRandom(cfg[1]);
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	ThePartitionManager = &g_ahPm;
	AIUpdateInterface ai;
	Weapon weapons[3][2];   /* per slot and template: the module keeps the template it found, and each template's range is the test's */
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 5; ++k) {
		g_r56Objs[k] = newObject();
		g_r56Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r56Objs[k]->m_behaviors = none;
	}
	Object *me = g_r56Objs[0];
	ai.m_owner = me;
	auto scene = [&]() {
		unsigned a = cfg[2];
		me->m_ai = (a & 1) ? &ai : 0;
		ai.m_idle = (a >> 1 & 1) != 0;
		ai.m_busy = (a >> 2 & 1) != 0;
		ai.m_lastCmdSource = (Int)(a >> 4 & 15);
		for (int s = 0; s < 3; ++s) {
			unsigned v = cfg[3] >> (4 * s) & 15;
			for (int t = 0; t < 2; ++t) {
				weapons[s][t].m_slot = (WeaponSlotType)s;
				weapons[s][t].m_tmplObj.m_attackRange = r56F(cfg[4 + t]);
			}
			me->m_slotWeapon[s] = v ? &weapons[s][(v - 1) % 2] : 0;
		}
		g_r34Closest = (cfg[6] >= 1 && cfg[6] <= 5) ? g_r56Objs[cfg[6] - 1] : 0;
		for (int k = 0; k < 5; ++k) {
			Object *o = g_r56Objs[k];
			o->m_pos.x = r56F(cfg[7 + k]);
			o->m_pos.y = k < 3 ? r56F(cfg[12 + k]) : 0.0f;
			o->m_pos.z = 0.0f;
		}
	};
	scene();
	g_modFindHook = r56Find;
	g_distHook = r56Dist;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	R56Cleanup *m = 0;
	try { m = newInstance(R56Cleanup)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	auto putState = [&]() {
		putU32((unsigned)m->m_bestTargetID);
		putU32(m->m_inRange ? 1 : 0);
		putU32((unsigned)m->m_nextScanFrames);
		putU32((unsigned)m->m_nextShotAvailableInFrames);
		putU32(m->m_weaponTemplate ? 1 : 0);
		putU32(realBits(m->m_pos.x));
		putU32(realBits(m->m_pos.y));
		putU32(realBits(m->m_pos.z));
		putU32(realBits(m->m_moveRange));
	};
	g_events.clear();
	g_ulog.clear();
	m->onObjectCreated();
	putLog();
	putState();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 14) cfg[o[1]] = o[2];
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
			case 3: { Coord3D p; p.x = r56F(o[1]); p.y = r56F(o[2]); p.z = 0.0f; m->r56SetCleanupAreaParameters(&p, r56F(o[3])); break; }
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r56Saved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_r56Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 6: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 7: m->onObjectCreated(); break;
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
		putState();
	}
	*len = finish(buf, cap);
	g_r34Closest = 0;
	g_modFindHook = 0;
	g_distHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 5; ++k) {
		Object *x = g_r56Objs[k];
		x->m_ai = 0; x->m_behaviors = 0;
		for (int s = 0; s < 3; ++s) x->m_slotWeapon[s] = 0;
		delete x;
		g_r56Objs[k] = 0;
	}
	ThePartitionManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r56_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r56RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
