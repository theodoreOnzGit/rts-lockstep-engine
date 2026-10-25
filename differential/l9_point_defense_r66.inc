/* Differential-test driver for PointDefenseLaserUpdate (p1w13-sonnet, batch R66; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PointDefenseLaserUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: six objects (ids 1-6: the laser and five
** others; kinds, status bits, effectively dead, a team of three, a position of the test's), the teams' relationships, the scan (the objects the
** test lists; the call a line, batch R35's), the distance (in the plane, from the positions), the weapon store of batches R62-R65 (WEAP_A and
** WEAP_B: the attack range, the delays between shots and the anti mask the test gives; Weapon.cpp's getAttackRange and getDelayBetweenShots),
** the stand-in Weapon with Weapon.cpp's own constructor (its loadAmmoNow and fireWeapon batch R62's lines); the REAL Object::getRelationship,
** UpdateModule, INI, Xfer and logic random values; the game logic (its frame, its wake calls applied, batch R35's way). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PointDefenseLaserUpdate.cpp"

/* Weapon.cpp 499-514, verbatim, on the stand-in template's members */
Int WeaponTemplate::getDelayBetweenShots(const WeaponBonus& bonus) const 
{
	// yes, divide, not multiply; the larger the rate-of-fire bonus, the shorter
	// we want the delay time to be.
	Int delayToUse;
	if( m_minDelayBetweenShots == m_maxDelayBetweenShots )
		delayToUse = m_minDelayBetweenShots; // Random number thing doesn't like this case
	else
		delayToUse = GameLogicRandomValue( m_minDelayBetweenShots, m_maxDelayBetweenShots );

	Real bonusROF = bonus.getField(WeaponBonus::RATE_OF_FIRE);
	//CRCDEBUG_LOG(("WeaponTemplate::getDelayBetweenShots() - min:%d max:%d val:%d, bonusROF=%g/%8.8X\n",
		//m_minDelayBetweenShots, m_maxDelayBetweenShots, delayToUse, bonusROF, AS_INT(bonusROF)));

	return REAL_TO_INT_FLOOR(delayToUse / bonusROF); 
}

namespace {

ModuleData *l9r66Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = PointDefenseLaserUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r66Saved;
/* the kinds and status bits the test gives an object, by bit (the same lists in the Rust test) */
const KindOfType g_r66Kinds[5] = { KINDOF_SMALL_MISSILE, KINDOF_BALLISTIC_MISSILE, KINDOF_AIRCRAFT, KINDOF_IMMOBILE, KINDOF_INFANTRY };
const ObjectStatusTypes g_r66Status[4] = { OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED };
Object *g_r66Objs[6];
Team g_r66Teams[3];
Player g_r66Players[3];
int g_r66Rel[3][3];
WeaponTemplate g_r66Tmpl[2];
const char *const g_r66Names[2] = { "WEAP_A", "WEAP_B" };
WeaponStore g_r66Store;
const void *r66Lookup(const char *name)
{
	for (int i = 0; i < 2; ++i) if (strcasecmp(name, g_r66Names[i]) == 0) return &g_r66Tmpl[i];
	return 0;   /* "None" and any other name: NULL */
}
int r66Code(const WeaponTemplate *t) { return t == &g_r66Tmpl[0] ? 1 : (t == &g_r66Tmpl[1] ? 2 : 0); }
Object *r66Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 6) ? g_r66Objs[(int)id - 1] : 0; }
Relationship r66TeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return (Relationship)g_r66Rel[from->m_dummy - 1][to->m_dummy - 1];
}
Real r66Dist(const Object *a, const Object *b)
{
	Real dx = b->m_pos.x - a->m_pos.x, dy = b->m_pos.y - a->m_pos.y;
	return dx * dx + dy * dy;
}
Real r66F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a PointDefenseLaserUpdate and make the module on object 1 of six at frame cfg[0], then
** onObjectCreated. `cfg` (32 words; 2-21 also settable by op 2): 1 the logic random seed; 2-7 objects 1-6: bits 0-4 their kinds (of g_r66Kinds),
** 5-8 status bits (of g_r66Status), 9 effectively dead, 10-11 their team (0 none, else 1-3); 8 the teams' relationships (2 bits each,
** row-major, team a to team b: 0 enemies, 1 neutral, 2 allies); 9 the scan (nibbles of object numbers 1-6, 0 ends); 10-15 the objects' x,
** 16-21 their y (float bits); read once: 22-25 WEAP_A's attack range (float bits), minimum and maximum delay between shots and anti mask,
** 26-29 WEAP_B's. The data dump goes first (the template: 0 NULL, 1 WEAP_A, 2 WEAP_B; the kinds, two lists ending 0xFFFFFFFF; the scan rate,
** range and velocity factor), then the constructor's next call frame and onObjectCreated's effects. Then `n` ops of 5 words [op, a, b, c, d]:
** 0 update()  1 the frame = a  2 cfg[a] = b  3 xfer save  4 xfer load of the last save  5 xfer CRC  6 onObjectCreated. Each op appends: the
** CATCH_ALL status, the result, the effect log, the xfer status, bytes and CRC, the state (the best target, in range, the frames to the next
** scan and the next shot). */
static int l9r66RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r66Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	for (int k = 0; k < 2; ++k) {
		g_r66Tmpl[k].m_name = AsciiString(g_r66Names[k]);
		g_r66Tmpl[k].m_attackRange = r66F(cfg_in[22 + 4 * k]);
		g_r66Tmpl[k].m_minDelayBetweenShots = (Int)cfg_in[23 + 4 * k];
		g_r66Tmpl[k].m_maxDelayBetweenShots = (Int)cfg_in[24 + 4 * k];
		g_r66Tmpl[k].m_antiMask = (Int)cfg_in[25 + 4 * k];
	}
	const void *(*oldLookup)(const char *) = ref_weapon_lookup_hook;
	ref_weapon_lookup_hook = r66Lookup;
	try { d = l9r66Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_weapon_lookup_hook = oldLookup;
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	initKindOfMasks();
	{
		const PointDefenseLaserUpdateModuleData *pd = (const PointDefenseLaserUpdateModuleData *)d;
		putU32((unsigned)r66Code(pd->m_weaponTemplate));
		for (int i = 0; i < KINDOF_COUNT; ++i) if (pd->m_primaryTargetKindOf.test((KindOfType)i)) putU32((unsigned)i);
		putU32(0xFFFFFFFFu);
		for (int i = 0; i < KINDOF_COUNT; ++i) if (pd->m_secondaryTargetKindOf.test((KindOfType)i)) putU32((unsigned)i);
		putU32(0xFFFFFFFFu);
		putU32(pd->m_scanFrames);
		putU32(realBits(pd->m_scanRange));
		putU32(realBits(pd->m_velocityFactor));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	gl.m_awakenHook = r35Awaken;   /* (batch R35's: the wake calls applied as GameLogic::friend_awakenUpdateModule applies them) */
	InitGameLogicRandom(cfg[1]);
	ThePartitionManager = &g_ahPm;
	TheWeaponStore = &g_r66Store;
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 6; ++k) {
		g_r66Objs[k] = newObject();
		g_r66Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r66Objs[k]->m_behaviors = none;
	}
	for (int t = 0; t < 3; ++t) {
		g_r66Players[t].m_idx = t;
		g_r66Teams[t].m_dummy = t + 1;
		g_r66Teams[t].m_controller = &g_r66Players[t];
	}
	Object *me = g_r66Objs[0];
	auto scene = [&]() {
		for (int k = 0; k < 6; ++k) {
			Object *o = g_r66Objs[k];
			unsigned f = cfg[2 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 5; ++b) if (f >> b & 1) o->m_kindOf.set(g_r66Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_status.clear();
			for (int b = 0; b < 4; ++b) if (f >> (5 + b) & 1) o->m_status.set(g_r66Status[b]);
			o->m_effDead = (f >> 9 & 1) != 0;
			unsigned t = f >> 10 & 3;
			o->m_team = t ? &g_r66Teams[t - 1] : 0;
			o->m_player = t ? &g_r66Players[t - 1] : 0;
			o->m_pos.x = r66F(cfg[10 + k]);
			o->m_pos.y = r66F(cfg[16 + k]);
			o->m_pos.z = 0.0f;
		}
		for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r66Rel[a][b] = (int)(cfg[8] >> (2 * (3 * a + b)) & 3);
		g_ahRange.clear();
		for (int k = 0; k < 6; ++k) { unsigned id = cfg[9] >> (4 * k) & 15; if (id == 0) break; if (id <= 6) g_ahRange.push_back(g_r66Objs[id - 1]); }
	};
	scene();
	g_ahRangeHook = r35ScanHook;
	g_teamRelHook = r66TeamRel;
	g_modFindHook = r66Find;
	g_distHook = r66Dist;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	PointDefenseLaserUpdate *m = 0;
	try { m = newInstance(PointDefenseLaserUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	putU32(m->friend_getNextCallFrame());
	g_events.clear();
	g_ulog.clear();
	m->onObjectCreated();
	putLog();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 21) cfg[o[1]] = o[2];
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
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r66Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r66Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 6: m->onObjectCreated(); break;
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
		putU32((unsigned)m->m_bestTargetID);
		putU32(m->m_inRange ? 1 : 0);
		putU32((unsigned)m->m_nextScanFrames);
		putU32((unsigned)m->m_nextShotAvailableInFrames);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_teamRelHook = 0;
	g_modFindHook = 0;
	g_distHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 6; ++k) {
		Object *x = g_r66Objs[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0;
		delete x;
		g_r66Objs[k] = 0;
	}
	TheWeaponStore = 0;
	ThePartitionManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r66_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r66RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
