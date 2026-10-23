/* Differential-test driver for WeaponBonusUpdate (p1w13-sonnet, batch R17; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/WeaponBonusUpdate.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the module's object (id 1) and four more (ids 2 to 5), each with kinds the
** test gives and maybe a contain (the sabotage driver's SabContain) with passengers among them; the partition manager's range scan (the auto heal
** driver's: the objects the test lists, the call a line); doTempWeaponBonus a line; the game logic, and the REAL UpdateModule, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/WeaponBonusUpdate.cpp"

void Object::doTempWeaponBonus(WeaponBonusConditionType status, UnsignedInt duration) { ulog("tempbonus %d %d %u", (int)m_oid, (int)status, duration); }

namespace {

ModuleData *l9r17wRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = WeaponBonusUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r17wSaved;
/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_r17wKinds[4] = { KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_STRUCTURE, KINDOF_AIRCRAFT };
void r17wSetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 4; ++i) if (bits >> i & 1) o->m_kindOf.set(g_r17wKinds[i]);
	o->m_tmpl.m_kindOf = o->m_kindOf;
}
void r17wScanHook(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	int n = 0;
	while (filters && filters[n]) ++n;
	ulog("scan %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), n);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a WeaponBonusUpdate and make the module on object 1 of five. `cfg` (12 words): [0-4 the kinds of
** objects 1 to 5 (bits of g_r17wKinds), 5 which objects have a contain (bit k: object k + 1), 6-10 each contain's passengers (4 bits each: objects 2
** to 5, in order; bit 16 reversed), 11 the range scan (bits 0-4: objects 1 to 5, in order; bit 5 reversed)]. The data dump goes first (the
** kinds, the duration, delay and range, the condition type). Then `n` ops of 6 words [op, a, b, c, d, e]: 0 update()  1 the scan = a
** 2 object a's kinds = b  3 object a's contain: has = b & 1, passengers = c  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op appends:
** the CATCH_ALL status, the result (update's sleep time), the effect log (count, then each), the xfer status, bytes and CRC. */
int ref_l9r17w_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r17wSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r17wRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	const WeaponBonusUpdateModuleData *wd = (const WeaponBonusUpdateModuleData *)d;
	for (int i = 0; i < KINDOF_COUNT; ++i) putU32(wd->m_requiredAffectKindOf.test(i) ? 1 : 0);
	for (int i = 0; i < KINDOF_COUNT; ++i) putU32(wd->m_forbiddenAffectKindOf.test(i) ? 1 : 0);
	putU32(wd->m_bonusDuration);
	putU32(wd->m_bonusDelay);
	putU32(realBits(wd->m_bonusRange));
	putI32((int)wd->m_bonusConditionType);
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	Object *objs[5];
	SabContain contains[5];
	for (int k = 0; k < 5; ++k) {
		objs[k] = newObject();
		objs[k]->m_oid = (ObjectID)(k + 1);
		r17wSetKinds(objs[k], cfg[k]);
	}
	objs[0]->m_pos.x = 10.0f;
	objs[0]->m_pos.y = -2.5f;
	objs[0]->m_pos.z = 1.0f;
	unsigned scan = cfg[11];
	unsigned hasContain = cfg[5];
	unsigned pass[5];
	for (int k = 0; k < 5; ++k) pass[k] = cfg[6 + k];
	g_ahRangeHook = r17wScanHook;
	gl.m_awakenCalls = 0;
	WeaponBonusUpdate *m = new WeaponBonusUpdate(objs[0], d);
	/* GameLogic::friend_awakenUpdateModule for an object not yet in the list (GameLogic.cpp:3035-3046): the wake frame is the module's next call
	** frame; the stand-in game logic only records the call */
	if (gl.m_awakenCalls) m->friend_setNextCallFrame(gl.m_awakenFrame);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 1) scan = o[1];
		if (o[0] == 2) r17wSetKinds(objs[(o[1] - 1) % 5], o[2]);
		if (o[0] == 3) { unsigned k = (o[1] - 1) % 5; hasContain = (hasContain & ~(1u << k)) | ((o[2] & 1) << k); pass[k] = o[3]; }
		g_ahRange.clear();
		for (int k = 0; k < 5; ++k) if (scan >> k & 1) g_ahRange.push_back(objs[k]);
		if (scan & 32) std::reverse(g_ahRange.begin(), g_ahRange.end());
		for (int k = 0; k < 5; ++k) {
			contains[k].m_list.clear();
			for (int j = 0; j < 4; ++j) if (pass[k] >> j & 1) contains[k].m_list.push_back(objs[1 + j]);
			if (pass[k] & 16) std::reverse(contains[k].m_list.begin(), contains[k].m_list.end());
			objs[k]->m_contain = (hasContain >> k & 1) ? &contains[k] : 0;
		}
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
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r17wSaved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r17wSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
	}
	*len = finish(buf, cap);
	g_ahRangeHook = 0;
	g_ahRange.clear();
	delete m;
	delete d;
	for (int k = 0; k < 5; ++k) { objs[k]->m_contain = 0; delete objs[k]; }
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
