/* Differential-test driver for FireWeaponWhenDeadBehavior (p1w11-sonnet, behavior batch K; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FireWeaponWhenDeadBehavior.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_dies_d.inc, whose object list helpers it does not
** use, and l9_dies_j.inc) on the stand-ins of differential/shim_module: an Object with a controlling player, status bits, completed upgrades and a position,
** three players with completed upgrades, a weapon store whose templates are WEAP_A and WEAP_B (found by INI::parseWeaponTemplate through the lookup hook of
** wrap_ini.cpp) and whose createAndFireTempWeapon logs, and the REAL BehaviorModule, DieMuxData, UpgradeMux, UpgradeCenter, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FireWeaponWhenDeadBehavior.cpp"

extern "C" const void *(*ref_weapon_lookup_hook)(const char *);

namespace {

/* the weapon store's templates: the name of each is what the log prints */
struct KWeapon { const char *name; };
KWeapon g_kWeapons[2] = { { "WEAP_A" }, { "WEAP_B" } };
const void *kWeaponLookup(const char *name)
{
	for (int i = 0; i < 2; ++i) if (strcasecmp(name, g_kWeapons[i].name) == 0) return &g_kWeapons[i];
	return 0;    /* "None" and any other name: NULL ("could be null!") */
}
ModTestWeaponStore g_kStore;

ModuleData *l9fkRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FireWeaponWhenDeadBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_fkSaved;

}  // namespace

WeaponStore *TheWeaponStore = 0;
void WeaponStore::createAndFireTempWeapon(const WeaponTemplate *wt, const Object *source, const Coord3D *pos)
{
	if (wt == NULL) return;   /* Weapon.cpp 1539-1540 (p1w13-sonnet, batch R50: PhysicsBehavior fires a crash weapon that may be none) */
	ulog("weapon %s %d %u,%u,%u", ((const KWeapon *)wt)->name, (int)source->m_oid, realBits(pos->x), realBits(pos->y), realBits(pos->z));
}

extern "C" {

/* Read `filename` (block `D`) as the data of a FireWeaponWhenDeadBehavior and make the module on an Object (id 1) whose controlling player is player
** cfg[0] of three (0xFFFFFFFF none). `cfg`: [the controlling player, unused, unused, unused, the object's completed upgrades as four words]. The data dump
** goes first (the die mux, the upgrade mux, StartsActive, the name of the death weapon or ""). Then the constructor's effect log (count, then each as a
** string). Then `n` ops of 6 words [op, a, b, c, d, e]: 0 onDie (death type a, veterancy b, status words c d)  1 attemptUpgrade(m)  2 wouldUpgrade(m)
** 3 resetUpgrade(m)  4 isAlreadyUpgraded  5 forceRefreshUpgrade (m = a..d as 128 bits)  6 the object's completed upgrades = m  7 the three players'
** completed upgrades = m  8 the controlling player = a - 1 (0 none)  9 xfer save  10 xfer load of the last save  11 xfer CRC  12 the object's position =
** (a, b, c) float bits. Each op appends: the CATCH_ALL status, the Bool result, isAlreadyUpgraded(), the effect log (count, then each as a string:
** "fx<id>", "fxobj <primary> <secondary>", "remove <name>", "weapon <name> <object> <position>"), the xfer status, bytes and CRC. */
int ref_l9fk_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len,
	char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_fkSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	ref_weapon_lookup_hook = kWeaponLookup;
	TheWeaponStore = &g_kStore;
	try { d = l9fkRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { ref_weapon_lookup_hook = 0; TheWeaponStore = 0; return status; }
	{
		const FireWeaponWhenDeadBehaviorModuleData *fd = (const FireWeaponWhenDeadBehaviorModuleData *)d;
		putDieMux(fd->m_dieMuxData);
		putMux(fd->m_upgradeMuxData);
		putU32(fd->m_initiallyActive ? 1 : 0);
		const char *wn = fd->m_deathWeapon ? ((const KWeapon *)fd->m_deathWeapon)->name : "";
		putStr(wn, (int)strlen(wn));
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) players[i].m_idx = i;
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	obj->m_pos.x = 11.0f; obj->m_pos.y = 22.0f; obj->m_pos.z = 5.0f;
	g_fxObjHook = jFxObj;
	GameLogic gl;
	TheGameLogic = &gl;
	g_events.clear();
	g_ulog.clear();
	FireWeaponWhenDeadBehavior *m = 0;
	try { m = new FireWeaponWhenDeadBehavior(obj, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		g_fxObjHook = 0; TheGameLogic = 0; ref_weapon_lookup_hook = 0; TheWeaponStore = 0;
		return 5;
	}
	{
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	}
	/* the object's completed upgrades come after the constructor (its removals have nothing to remove): a new object has none */
	obj->m_objectUpgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, cfg + 4);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		g_events.clear();
		g_ulog.clear();
		int st = 0, res = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		try {
			switch (o[0]) {
			case 0: {
				DamageInfo info;
				info.in.m_deathType = (DeathType)o[1];
				obj->m_level = (VeterancyLevel)o[2];
				obj->m_status = maskFromU32<ObjectStatusMaskType>(OBJECT_STATUS_COUNT, o + 3);
				dynamic_cast<DieModuleInterface *>(m)->onDie(&info);
				break;
			}
			case 1: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 2: res = m->wouldUpgrade(mask) ? 1 : 0; break;
			case 3: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 4: res = m->isAlreadyUpgraded() ? 1 : 0; break;
			case 5: m->forceRefreshUpgrade(); break;
			case 6: obj->m_objectUpgradesCompleted = mask; break;
			case 7: for (int p = 0; p < 3; ++p) players[p].m_upgradesCompleted = mask; break;
			case 8: obj->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			case 9: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_fkSaved.assign(bytes, bytes + blen); break;
			case 10: {
				std::vector<unsigned char> b = g_fkSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 11: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 12: memcpy(&obj->m_pos.x, &o[1], 4); memcpy(&obj->m_pos.y, &o[2], 4); memcpy(&obj->m_pos.z, &o[3], 4); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32((unsigned)res);
		putU32(m->isAlreadyUpgraded() ? 1 : 0);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	g_fxObjHook = 0;
	TheGameLogic = 0;
	ref_weapon_lookup_hook = 0;
	TheWeaponStore = 0;
	return 0;
}

}  /* extern "C" */
