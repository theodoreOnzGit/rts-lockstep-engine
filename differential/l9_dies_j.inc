/* Differential-test driver for FXListDie (p1w11-sonnet, die batch J; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/FXListDie.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_dies_d.inc and l9_helpers.inc, whose
** `modhelp_find` hook it answers `findObjectByID` with) on the stand-ins of differential/shim_module: an Object with a controlling player, completed
** upgrades and a position, three players with completed upgrades, a damage dealer found by its id, FXList::doFXObj (the list, then through
** `g_fxObjHook` the two objects) and FXList::doFXPos (the list and the position), and the REAL DieModule, UpgradeMux, UpgradeCenter, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/FXListDie.cpp"

namespace {

Object *g_jDealer = 0;
Object *jFind(ObjectID id) { return g_jDealer && (ObjectID)g_jDealer->m_oid == id ? g_jDealer : 0; }
void jFxObj(const Object *primary, const Object *secondary) { ulog("fxobj %d %d", primary ? (int)primary->m_oid : -1, secondary ? (int)secondary->m_oid : -1); }

ModuleData *l9fjRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FXListDie::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_fjSaved;

}  // namespace

void FXList::doFXPos(const FXList *fx, const Coord3D *primary, const Matrix3D *primaryMtx, const Real primarySpeed, const Coord3D *secondary, const Real overrideRadius)
{
	if (fx) ulog("fxpos %d %u,%u,%u", fxId(fx), realBits(primary->x), realBits(primary->y), realBits(primary->z));
}

extern "C" {

/* Read `filename` (block `D`) as the data of an FXListDie and make the module on an Object (id 1) whose controlling player is player cfg[0] of three
** (0xFFFFFFFF none). `cfg`: [the controlling player, has a damage dealer (object 2), unused, unused, the object's initial completed upgrades as four
** words]. The data dump goes first (the die mux, the upgrade mux, the death FX list, OrientToObject, StartsActive). Then the
** constructor's effect log (count, then each as a string). Then `n` ops of 6 words [op, a, b, c, d, e]: 0 onDie (death type a, veterancy b, status
** words c d, the damage source's id e)  1 attemptUpgrade(m)  2 wouldUpgrade(m)  3 resetUpgrade(m)  4 isAlreadyUpgraded  5 forceRefreshUpgrade
** (m = a..d as 128 bits)  6 the object's completed upgrades = m  7 the three players' completed upgrades = m  8 the controlling player = a - 1 (0 none)
** 9 xfer save  10 xfer load of the last save  11 xfer CRC  12 the object's position = (a, b, c) float bits. Each op appends: the CATCH_ALL status, the
** Bool result, isAlreadyUpgraded(), the effect log (count, then each as a string: "fx<id>", "fxobj <primary> <secondary>", "fxpos <id> <position>",
** "remove <name>"), the xfer status, bytes and CRC. */
int ref_l9fj_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len,
	char *msg, int msgCap, unsigned *code)
{
	initDisabledMasks();
	g_dump.clear();
	g_fxIds.clear();
	g_fjSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9fjRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const FXListDieModuleData *fd = (const FXListDieModuleData *)d;
		putDieMux(fd->m_dieMuxData);
		putMux(fd->m_upgradeMuxData);
		putI32(fxId(fd->m_defaultDeathFX));
		putU32(fd->m_orientToObject ? 1 : 0);
		putU32(fd->m_initiallyActive ? 1 : 0);
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) players[i].m_idx = i;
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	obj->m_level = (VeterancyLevel)cfg[2];
	obj->m_pos.x = 11.0f; obj->m_pos.y = 22.0f; obj->m_pos.z = 5.0f;
	Object *dealer = 0;
	if (cfg[1]) { dealer = newObject(); dealer->m_oid = 2; }
	g_jDealer = dealer;
	g_modFindHook = jFind;
	g_fxObjHook = jFxObj;
	GameLogic gl;
	TheGameLogic = &gl;
	g_events.clear();
	g_ulog.clear();
	FXListDie *m = 0;
	try { m = new FXListDie(obj, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		g_modFindHook = 0; g_fxObjHook = 0; g_jDealer = 0; TheGameLogic = 0;
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
				info.in.m_sourceID = (ObjectID)o[5];
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
			case 9: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_fjSaved.assign(bytes, bytes + blen); break;
			case 10: {
				std::vector<unsigned char> b = g_fjSaved;
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
	delete dealer;
	g_modFindHook = 0;
	g_fxObjHook = 0;
	g_jDealer = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
