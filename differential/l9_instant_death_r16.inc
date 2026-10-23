/* Differential-test driver for InstantDeathBehavior (p1w13-sonnet, batch R16; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/InstantDeathBehavior.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the stand-ins of differential/l9_slow_death_q.inc: the FX list store (FX_A, FX_B), the object creation
** list store (OCL_A, OCL_B; l9_upgrades_b.inc), the weapon store (WEAP_A, WEAP_B; l9_behaviors_k.inc), an AI that logs markAsDead, the game logic
** (destroyObject logs), and the REAL DieModule, DieMuxData, INI, RandomValue and Xfer. Every call is a line of the effect log (ulog). */
/* SlowDeathBehavior.cpp (l9_slow_death_q.inc, the same unit) has file-static parsers of the same three names */
#define parseFX IdParseFX
#define parseOCL IdParseOCL
#define parseWeapon IdParseWeapon
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/InstantDeathBehavior.cpp"
#undef parseFX
#undef parseOCL
#undef parseWeapon

namespace {

ModuleData *l9r16Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = InstantDeathBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r16Saved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of an InstantDeathBehavior and make the module on an Object (id 1). `cfg` (6 words): [0 flags (bit 0 the
** object has an AI, 1 the AI is in its dead state), 1-3 the position (float bits), 4 the logic random seed, 5 unused]. The data dump goes first (the
** die mux; the FX, OCL and weapon lists, each entry 0 for NULL, else 1 or 2). Then `n` ops of 6 words [op, a, b, c, d, e]: 0 onDie (death type a,
** veterancy b, status words c and d)  1 flags = a  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op appends: the CATCH_ALL status, the
** effect log (count, then each), the xfer status, bytes and CRC, the AI's dead flag. */
int ref_l9r16_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r16Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r16Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	const InstantDeathBehaviorModuleData *id = (const InstantDeathBehaviorModuleData *)d;
	putDieMux(id->m_dieMuxData);
	putU32((unsigned)id->m_fx.size());
	for (size_t i = 0; i < id->m_fx.size(); ++i) putI32(id->m_fx[i] == 0 ? 0 : (id->m_fx[i] == &g_sdFx[0] ? 1 : 2));
	putU32((unsigned)id->m_ocls.size());
	for (size_t i = 0; i < id->m_ocls.size(); ++i) putI32(id->m_ocls[i] == 0 ? 0 : (id->m_ocls[i] == &g_upOcls[0] ? 1 : 2));
	putU32((unsigned)id->m_weapons.size());
	for (size_t i = 0; i < id->m_weapons.size(); ++i) putI32(id->m_weapons[i] == 0 ? 0 : (id->m_weapons[i] == (const WeaponTemplate *)&g_kWeapons[0] ? 1 : 2));
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	TheWeaponStore = &g_sdWeaponStore;
	Object *obj = newObject();
	obj->m_oid = 1;
	g_sdObj = obj;
	g_sdTree = 0;
	g_modFindHook = sdFind;
	g_modDestroyHook = sdDestroy;
	obj->m_pos.x = sdF(cfg[1]);
	obj->m_pos.y = sdF(cfg[2]);
	obj->m_pos.z = sdF(cfg[3]);
	AIUpdateInterface ai;
	ai.m_owner = obj;
	InitGameLogicRandom(cfg[4]);
	unsigned flags = cfg[0];
	DieModule *m = new InstantDeathBehavior(obj, d);
	ai.m_dead = (flags & 2) != 0;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 1) { flags = o[1]; ai.m_dead = (flags & 2) != 0; }
		obj->m_ai = (flags & 1) ? &ai : 0;
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
			case 0: {
				DamageInfo info;
				info.in.m_deathType = (DeathType)o[1];
				obj->m_level = (VeterancyLevel)o[2];
				obj->m_status = maskFromU32<ObjectStatusMaskType>(OBJECT_STATUS_COUNT, o + 3);
				dynamic_cast<DieModuleInterface *>(m)->onDie(&info);
				break;
			}
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r16Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r16Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1 && e.p) { char b[32]; sprintf(b, "fx %s", e.p == &g_sdFx[0] ? "FX_A" : "FX_B"); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(ai.m_dead ? 1 : 0);
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	g_sdObj = 0;
	delete m;
	delete d;
	obj->m_ai = 0;
	delete obj;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
