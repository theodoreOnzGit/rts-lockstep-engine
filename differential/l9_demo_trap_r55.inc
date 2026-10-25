/* Differential-test driver for DemoTrapUpdate (p1w13-sonnet, batch R55; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DemoTrapUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: five objects (ids 1-5; kinds, the status
** bits, effectively dead, a team of three, maybe a current weapon (its slot and damage type), x a float of the test's (y 0, z 0 or 5: the
** terrain is flat at 0), the module build's MBody, whose attemptDamage is a line), the teams' relationships, the scan (the objects the test
** lists; the call a line), the distance (in the plane, from the positions), Object::setWeaponLock (a line), Object::setWeaponSetFlag (batch
** E's line), the weapon store (behavior batch K's WEAP_A and WEAP_B; createAndFireTempWeapon a line), the REAL Object::kill and
** Object::getRelationship, UpdateModule, INI and Xfer; the game logic (its frame). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DemoTrapUpdate.cpp"

Bool Object::setWeaponLock( WeaponSlotType weaponSlot, WeaponLockType lockType )
{
	ulog("weaponlock %d %d %d", (int)m_oid, (int)weaponSlot, (int)lockType);
	return TRUE;
}

namespace {

ModuleData *l9r55Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = DemoTrapUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r55Saved;
/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_r55Kinds[3] = { KINDOF_DOZER, KINDOF_INFANTRY, KINDOF_VEHICLE };
const ObjectStatusTypes g_r55Status[3] = { OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_SOLD, OBJECT_STATUS_IS_ATTACKING };
Object *g_r55Objs[5];
Team g_r55Teams[3];
Player g_r55Players[3];
int g_r55Rel[3][3];
Object *r55Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 5) ? g_r55Objs[(int)id - 1] : 0; }
Relationship r55TeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return (Relationship)g_r55Rel[from->m_dummy - 1][to->m_dummy - 1];
}
Real r55Dist(const Object *a, const Object *b)
{
	Real dx = b->m_pos.x - a->m_pos.x, dy = b->m_pos.y - a->m_pos.y;
	return dx * dx + dy * dy;
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a DemoTrapUpdate and make the module on object 1 of five at frame cfg[0], then onObjectCreated.
** `cfg` (16 words; 1-12 also settable by op 2): 1-5 objects 1-5: bits 0-2 their kinds (of g_r55Kinds), 3-5 status bits (of g_r55Status), 6
** effectively dead, 7-8 their team (0 none, else 1-3), 9-10 the current weapon (0 none, else its slot + 1), 11 its damage type DAMAGE_DISARM
** (else DAMAGE_EXPLOSION), 12 above the terrain (z 5); 6 the teams' relationships (2 bits each, row-major, team a to team b: 0 enemies, 1
** neutral, 2 allies); 7 the scan (nibbles of object numbers 1-5, 0 ends); 8-12 the objects' x (float bits). The data dump goes first, then
** onObjectCreated's effects. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3 detonate()  4 xfer save
** 5 xfer load of the last save  6 xfer CRC  7 onObjectCreated. Each op appends: the CATCH_ALL status, the result, the effect log, the xfer
** status, bytes and CRC, the state (the scan countdown, the detonated flag). */
static int l9r55RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r55Saved.clear();
	*len = 0;
	ref_weapon_lookup_hook = kWeaponLookup;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r55Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_weapon_lookup_hook = 0;
	if (status) return status;
	unsigned cfg[16];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const DemoTrapUpdateModuleData *dd = (const DemoTrapUpdateModuleData *)d;
		for (int i = 0; i < KINDOF_COUNT; ++i) if (dd->m_ignoreKindOf.test((KindOfType)i)) putU32((unsigned)i);
		putU32(0xFFFFFFFFu);
		putU32((unsigned)dd->m_manualModeWeaponSlot);
		putU32((unsigned)dd->m_detonationWeaponSlot);
		putU32((unsigned)dd->m_proximityModeWeaponSlot);
		putU32(realBits(dd->m_triggerDetonationRange));
		const WeaponTemplate *wt = dd->m_detonationWeaponTemplate;
		putI32(wt == 0 ? 0 : (wt == (const WeaponTemplate *)&g_kWeapons[0] ? 1 : 2));
		putU32(dd->m_scanFrames);
		putU32((dd->m_defaultsToProximityMode ? 1 : 0) | (dd->m_friendlyDetonation ? 2 : 0) | (dd->m_detonateWhenKilled ? 4 : 0));
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	ThePartitionManager = &g_ahPm;
	TheWeaponStore = &g_kStore;
	MBody bodies[5];
	Weapon weapons[5];
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 5; ++k) {
		g_r55Objs[k] = newObject();
		g_r55Objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = g_r55Objs[k];
		g_r55Objs[k]->m_behaviors = none;
	}
	for (int t = 0; t < 3; ++t) {
		g_r55Players[t].m_idx = t;
		g_r55Teams[t].m_dummy = t + 1;
		g_r55Teams[t].m_controller = &g_r55Players[t];
	}
	Object *me = g_r55Objs[0];
	auto scene = [&]() {
		for (int k = 0; k < 5; ++k) {
			Object *o = g_r55Objs[k];
			unsigned f = cfg[1 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 3; ++b) if (f >> b & 1) o->m_kindOf.set(g_r55Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_status.clear();
			for (int b = 0; b < 3; ++b) if (f >> (3 + b) & 1) o->m_status.set(g_r55Status[b]);
			o->m_effDead = (f >> 6 & 1) != 0;
			unsigned t = f >> 7 & 3;
			o->m_team = t ? &g_r55Teams[t - 1] : 0;
			o->m_player = t ? &g_r55Players[t - 1] : 0;
			unsigned w = f >> 9 & 3;
			weapons[k].m_slot = (WeaponSlotType)(w ? w - 1 : 0);
			weapons[k].m_damageType = (f >> 11 & 1) ? (Int)DAMAGE_DISARM : (Int)DAMAGE_EXPLOSION;
			o->m_curWeapon = w ? &weapons[k] : 0;
			o->m_body = &bodies[k];   /* always: Object::kill reads the body unchecked */
			memcpy(&o->m_pos.x, &cfg[8 + k], 4);
			o->m_pos.y = 0.0f;
			o->m_pos.z = (f >> 12 & 1) ? 5.0f : 0.0f;
		}
		for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r55Rel[a][b] = (int)(cfg[6] >> (2 * (3 * a + b)) & 3);
		g_ahRange.clear();
		for (int k = 0; k < 5; ++k) { unsigned id = cfg[7] >> (4 * k) & 15; if (id == 0) break; if (id <= 5) g_ahRange.push_back(g_r55Objs[id - 1]); }
	};
	scene();
	g_ahRangeHook = r35ScanHook;
	g_teamRelHook = r55TeamRel;
	g_modFindHook = r55Find;
	g_distHook = r55Dist;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	DemoTrapUpdate *m = 0;
	try { m = newInstance(DemoTrapUpdate)(me, d); }
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
		if (o[0] == 2 && o[1] >= 1 && o[1] <= 12) cfg[o[1]] = o[2];
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
			case 3: m->detonate(); break;
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r55Saved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_r55Saved;
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
		putU32((unsigned)m->m_nextScanFrames);
		putU32(m->m_detonated ? 1 : 0);
	}
	*len = finish(buf, cap);
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_teamRelHook = 0;
	g_modFindHook = 0;
	g_distHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 5; ++k) {
		Object *x = g_r55Objs[k];
		x->m_body = 0; x->m_team = 0; x->m_player = 0; x->m_curWeapon = 0; x->m_behaviors = 0;
		delete x;
		g_r55Objs[k] = 0;
	}
	TheWeaponStore = 0;
	ThePartitionManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r55_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r55RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
