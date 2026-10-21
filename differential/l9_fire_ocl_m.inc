/* Differential-test driver for FireOCLAfterWeaponCooldownUpdate (p1w12-sonnet, update batch M; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireOCLAfterWeaponCooldownUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit (#included at that file's end, after l9_updates_i.inc, whose players it shares, and
** l9_upgrades_b.inc, whose object creation lists OCL_A and OCL_B and whose ObjectCreationList::createInternal it uses) on the stand-ins of
** differential/shim_module: three players (a completed-upgrade mask), an Object with a controlling player, a current weapon and a weapon per slot (a
** Weapon with a slot, a last shot frame and a possible next shot frame), the game logic whose frame the test sets, and the REAL UpdateModule,
** UpgradeMux, UpgradeCenter, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireOCLAfterWeaponCooldownUpdate.cpp"

namespace {

ModuleData *l9foRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FireOCLAfterWeaponCooldownUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_foSaved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a FireOCLAfterWeaponCooldownUpdate and make the module on an Object whose controlling player is player
** cfg[0] of three (0xFFFFFFFF none), at the frame cfg[1]. The data dump goes first (the mux, then the list (0 none, 1 OCL_A, 2 OCL_B), the slot, the
** minimum shots, the lifetime per second, the cap). Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 the frame += 1 and,
** if a & 1, the current weapon (if any) has just fired (last = frame - 1, next = frame + 2)  3 the current weapon = none (a = 0) or slot a - 1 with
** last shot b and possible next shot c  4 the weapons in the slots = bits a & 7  5 the object's completed upgrades = m (a..d, 128 bits)  6 the
** players' completed upgrades = m  7 the controlling player = a - 1 (0 none)  8 attemptUpgrade(m)  9 wouldUpgrade(m)  10 resetUpgrade(m)  11
** isAlreadyUpgraded  12 forceRefreshUpgrade  13 xfer save  14 xfer load of the last save  15 xfer CRC. Each op appends: the CATCH_ALL status, the result
** (the sleep time update() returned, or the Bool of the upgrade calls), isAlreadyUpgraded(), the effect log (count, then each as a string: "ocl <i>",
** "oclsec <id>", "ocllife <frames>", "fx<id>", "remove <name>"), the xfer status, bytes and CRC, and the state words (valid, shots, start frame). */
static int l9foRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_foSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9foRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const FireOCLAfterWeaponCooldownUpdateModuleData *fd = (const FireOCLAfterWeaponCooldownUpdateModuleData *)d;
		putMux(fd->m_upgradeMuxData);
		putU32(fd->m_ocl == 0 ? 0 : (fd->m_ocl == &g_upOcls[0] ? 1 : 2));
		putI32((int)fd->m_weaponSlot);
		putU32(fd->m_minShotsRequired);
		putU32(fd->m_oclLifetimePerSecond);
		putU32(fd->m_oclMaxFrames);
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_upgradesCompleted = UpgradeMaskType();
	}
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	Weapon cur;
	Weapon slotWeapons[3];
	for (int i = 0; i < 3; ++i) slotWeapons[i].m_slot = (WeaponSlotType)i;
	bool haveCur = false;
	GameLogic gl;
	gl.m_frame = cfg[1];
	TheGameLogic = &gl;
	FireOCLAfterWeaponCooldownUpdate *m = new FireOCLAfterWeaponCooldownUpdate(obj, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
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
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 1: gl.m_frame = o[1]; break;
			case 2:
				gl.m_frame += 1;
				if ((o[1] & 1) && haveCur) { cur.m_last = gl.m_frame - 1; cur.m_next = gl.m_frame + 2; }
				break;
			case 3:
				haveCur = o[1] != 0;
				if (haveCur) { cur.m_slot = (WeaponSlotType)(o[1] - 1); cur.m_last = o[2]; cur.m_next = o[3]; }
				obj->m_curWeapon = haveCur ? &cur : 0;
				break;
			case 4: for (int k = 0; k < 3; ++k) obj->m_slotWeapon[k] = (o[1] >> k & 1) ? &slotWeapons[k] : 0; break;
			case 5: obj->m_objectUpgradesCompleted = mask; break;
			case 6: for (int k = 0; k < 3; ++k) players[k].m_upgradesCompleted = mask; break;
			case 7: obj->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			case 8: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 9: res = m->wouldUpgrade(mask) ? 1 : 0; break;
			case 10: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 11: res = m->isAlreadyUpgraded() ? 1 : 0; break;
			case 12: m->forceRefreshUpgrade(); break;
			case 13: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_foSaved.assign(bytes, bytes + blen); break;
			case 14: {
				std::vector<unsigned char> b = g_foSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			default: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
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
		putU32(m->m_valid ? 1 : 0);
		putU32(m->m_consecutiveShots);
		putU32(m->m_startFrame);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	return 0;
}

int ref_l9fo_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9foRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
