/* Differential-test driver for FireWeaponCollide (p1w13-sonnet, batch R62; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/FireWeaponCollide.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the object (id 1: its status bits the
** test's) and two others it may touch (ids 2, 3), the weapon store (two templates, WEAP_A and WEAP_B, with the facts Weapon's constructor
** reads: the pitch limits, the shots per barrel, the suspend-FX delay; findWeaponTemplate finds them), the stand-in Weapon with Weapon.cpp's
** own constructor, crc, xfer and loadPostProcess (build.rs, mod_gen/Weapon_r62.inc) and its loadAmmoNow and fireWeapon as lines, the REAL
** CollideModule, INI and Xfer; the game logic (its frame). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/FireWeaponCollide.cpp"
#include "Weapon_r62.inc"   /* Weapon.cpp's constructor, crc, xfer and loadPostProcess, verbatim (build.rs) */

void Weapon::loadAmmoNow(const Object *source)
{
	ulog("loadammo %d %s", source ? (int)source->m_oid : 0, m_template ? m_template->getName().str() : "");
}
Bool Weapon::fireWeapon(const Object *source, Object *target, ObjectID *)
{
	ulog("fire %d %d %s", source ? (int)source->m_oid : 0, target ? (int)target->m_oid : 0, m_template ? m_template->getName().str() : "");
	return FALSE;
}
extern const void *(*g_weaponFindHook)(const char *);
extern "C" const void *(*ref_weapon_lookup_hook)(const char *);

namespace {

ModuleData *l9r62Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FireWeaponCollide::friend_newModuleData(&ini);
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

WeaponTemplate g_r62Tmpl[2];
const char *const g_r62Names[2] = { "WEAP_A", "WEAP_B" };
const void *r62Lookup(const char *name)
{
	for (int i = 0; i < 2; ++i) if (strcasecmp(name, g_r62Names[i]) == 0) return &g_r62Tmpl[i];
	return 0;   /* "None" and any other name: NULL */
}
int r62Code(const WeaponTemplate *t) { return t == &g_r62Tmpl[0] ? 1 : (t == &g_r62Tmpl[1] ? 2 : 0); }
WeaponStore g_r62Store;
std::vector<unsigned char> g_r62Saved;
Object *g_r62Objs[3];
Object *r62Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 3) ? g_r62Objs[(int)id - 1] : 0; }
Real r62F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
void r62PutMask(const ObjectStatusMaskType &m)
{
	unsigned w[2] = { 0, 0 };
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (m.test((ObjectStatusTypes)i)) w[i / 32] |= 1u << (i % 32);
	putU32(w[0]); putU32(w[1]);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a FireWeaponCollide and make the module on object 1 of three at frame cfg[0]. `cfg` (12 words; 1-2
** also settable by op 2): 1-2 the object's status bits (0-31, 32-44); read once: 3-5 WEAP_A's minimum and maximum target pitch (float bits)
** and shots per barrel, 6 its suspend-FX delay, 7-10 WEAP_B's. The data dump goes first (the weapon template: 0 NULL, 1 WEAP_A, 2 WEAP_B;
** fire once; the required and forbidden status, two words each), then whether the weapon was made. Then `n` ops of 5 words [op, a, b, c, d]:
** 0 onCollide(object a; else NULL)  1 the frame = a  2 cfg[a] = b  3 xfer save  4 xfer load of the last save  5 xfer CRC. Each op appends: the
** CATCH_ALL status, the effect log, the xfer status, bytes and CRC, m_everFired. */
static int l9r62RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r62Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	for (int k = 0; k < 2; ++k) {
		g_r62Tmpl[k].m_name = AsciiString(g_r62Names[k]);
		g_r62Tmpl[k].m_minTargetPitch = r62F(cfg_in[3 + 4 * k]);
		g_r62Tmpl[k].m_maxTargetPitch = r62F(cfg_in[4 + 4 * k]);
		g_r62Tmpl[k].m_shotsPerBarrel = (Int)cfg_in[5 + 4 * k];
		g_r62Tmpl[k].m_suspendFXDelay = cfg_in[6 + 4 * k];
	}
	const void *(*oldLookup)(const char *) = ref_weapon_lookup_hook;
	ref_weapon_lookup_hook = r62Lookup;
	try { d = l9r62Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_weapon_lookup_hook = oldLookup;
	if (status) return status;
	unsigned cfg[12];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const FireWeaponCollideModuleData *fd = (const FireWeaponCollideModuleData *)d;
		putU32((unsigned)r62Code(fd->m_collideWeaponTemplate));
		putU32(fd->m_fireOnce ? 1 : 0);
		r62PutMask(fd->m_requiredStatus);
		r62PutMask(fd->m_forbiddenStatus);
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	TheWeaponStore = &g_r62Store;
	g_weaponFindHook = r62Lookup;
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 3; ++k) {
		g_r62Objs[k] = newObject();
		g_r62Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r62Objs[k]->m_behaviors = none;
	}
	Object *me = g_r62Objs[0];
	auto scene = [&]() {
		me->m_status.clear();
		for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (cfg[1 + i / 32] >> (i % 32) & 1) me->m_status.set((ObjectStatusTypes)i);
	};
	scene();
	g_modFindHook = r62Find;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	FireWeaponCollide *m = 0;
	try { m = newInstance(FireWeaponCollide)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	putU32(m->m_collideWeapon ? 1 : 0);
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
		try {
			switch (o[0]) {
			case 0: m->onCollide(r62Find((ObjectID)o[1]), 0, 0); break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r62Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r62Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_everFired ? 1 : 0);
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_weaponFindHook = 0;
	TheWeaponStore = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 3; ++k) {
		g_r62Objs[k]->m_behaviors = 0;
		delete g_r62Objs[k];
		g_r62Objs[k] = 0;
	}
	TheGameLogic = 0;
	return 0;
}

int ref_l9r62_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r62RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
