/* Differential-test driver for FireWeaponUpdate (p1w13-sonnet, batch R64; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireWeaponUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the object (id 1, at (11, 22, 5): under
** construction or not, its last shot frame the test's), the weapon store of batch R62 (WEAP_A, WEAP_B; differential/l9_fire_weapon_collide_r62.inc
** is earlier in this unit), the stand-in Weapon with Weapon.cpp's own constructor, crc, xfer, loadPostProcess and getStatus (build.rs,
** mod_gen/Weapon_r62.inc) and Weapon.h's setters, its loadAmmoNow (batch R62's) and forceFireWeapon as lines, the REAL UpdateModule, INI and
** Xfer; the game logic (its frame). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireWeaponUpdate.cpp"

Object *Weapon::forceFireWeapon(const Object *source, const Coord3D *pos)
{
	ulog("forcefire %d %08x %08x %08x %s", source ? (int)source->m_oid : 0, realBits(pos->x), realBits(pos->y), realBits(pos->z),
		m_template ? m_template->getName().str() : "");
	return 0;
}
extern const void *(*g_weaponFindHook)(const char *);
extern "C" const void *(*ref_weapon_lookup_hook)(const char *);

namespace {

ModuleData *l9r64Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FireWeaponUpdate::friend_newModuleData(&ini);
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

WeaponTemplate g_r64Tmpl[2];
const char *const g_r64Names[2] = { "WEAP_A", "WEAP_B" };
const void *r64Lookup(const char *name)
{
	for (int i = 0; i < 2; ++i) if (strcasecmp(name, g_r64Names[i]) == 0) return &g_r64Tmpl[i];
	return 0;   /* "None" and any other name: NULL */
}
int r64Code(const WeaponTemplate *t) { return t == &g_r64Tmpl[0] ? 1 : (t == &g_r64Tmpl[1] ? 2 : 0); }
WeaponStore g_r64Store;
std::vector<unsigned char> g_r64Saved;
Object *g_r64Me;
Object *r64Find(ObjectID id) { return (int)id == 1 ? g_r64Me : 0; }
Real r64F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a FireWeaponUpdate and make the module on object 1 at frame cfg[0]. `cfg` (12 words; 1-2 also
** settable by op 2): 1 bit 0 the object is under construction, 2 its last shot frame; read once: 3-5 WEAP_A's minimum and maximum target pitch
** (float bits) and shots per barrel, 6 its suspend-FX delay, 7-10 WEAP_B's. The data dump goes first (the weapon template: 0 NULL, 1 WEAP_A,
** 2 WEAP_B; the initial delay; the exclusive delay), then the constructor's effect log, whether the weapon was made and m_initialDelayFrame.
** Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3 xfer save  4 xfer load of the last save  5 xfer CRC
** 6 the weapon's setStatus(a)  7 its setPossibleNextShotFrame(a)  8 its setPreAttackFinishedFrame(a) (6-8 nothing without a weapon). Each op
** appends: the CATCH_ALL status, the result, the effect log, the xfer status, bytes and CRC, m_initialDelayFrame. */
static int l9r64RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r64Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	for (int k = 0; k < 2; ++k) {
		g_r64Tmpl[k].m_name = AsciiString(g_r64Names[k]);
		g_r64Tmpl[k].m_minTargetPitch = r64F(cfg_in[3 + 4 * k]);
		g_r64Tmpl[k].m_maxTargetPitch = r64F(cfg_in[4 + 4 * k]);
		g_r64Tmpl[k].m_shotsPerBarrel = (Int)cfg_in[5 + 4 * k];
		g_r64Tmpl[k].m_suspendFXDelay = cfg_in[6 + 4 * k];
	}
	const void *(*oldLookup)(const char *) = ref_weapon_lookup_hook;
	ref_weapon_lookup_hook = r64Lookup;
	try { d = l9r64Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_weapon_lookup_hook = oldLookup;
	if (status) return status;
	unsigned cfg[12];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const FireWeaponUpdateModuleData *fd = (const FireWeaponUpdateModuleData *)d;
		putU32((unsigned)r64Code(fd->m_weaponTemplate));
		putU32(fd->m_initialDelayFrames);
		putU32(fd->m_exclusiveWeaponDelay);
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	TheWeaponStore = &g_r64Store;
	g_weaponFindHook = r64Lookup;
	BehaviorModule *none[1] = { 0 };
	g_r64Me = newObject();
	Object *me = g_r64Me;
	me->m_oid = 1;
	me->m_behaviors = none;
	me->m_pos.x = 11.0f; me->m_pos.y = 22.0f; me->m_pos.z = 5.0f;
	auto scene = [&]() {
		me->m_status.clear();
		if (cfg[1] & 1) me->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
		me->m_lastShotFrame = cfg[2];
	};
	scene();
	g_modFindHook = r64Find;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	g_events.clear();
	g_ulog.clear();
	FireWeaponUpdate *m = 0;
	try { m = newInstance(FireWeaponUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	putLog();
	putU32(m->m_weapon ? 1 : 0);
	putU32(m->m_initialDelayFrame);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 1 && o[1] <= 2) cfg[o[1]] = o[2];
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
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r64Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r64Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 6: if (m->m_weapon) m->m_weapon->setStatus((WeaponStatus)o[1]); break;
			case 7: if (m->m_weapon) m->m_weapon->setPossibleNextShotFrame(o[1]); break;
			case 8: if (m->m_weapon) m->m_weapon->setPreAttackFinishedFrame(o[1]); break;
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
		putU32(m->m_initialDelayFrame);
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_weaponFindHook = 0;
	TheWeaponStore = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	delete me;
	g_r64Me = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r64_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r64RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
