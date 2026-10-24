/* Differential-test driver for TransitionDamageFX (p1w13-sonnet, batch R31; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/TransitionDamageFX.cpp   (#included with one mechanical edit made by build.rs into
**     mod_gen/TransitionDamageFX_edit.cpp: its particle system parser is this unit's l9ahParsePsTemplate, asserted once)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the object (id 1: a body, the module build's MBody, whose last damage
** is none or the test's; a drawable whose art has the bones B1, B2 and F01-F03 at the test's positions, through the bone hook of
** Drawable::getPristineBonePositions; a position and an orientation, which convertBonePosToWorldPos pushes the bones through), the damage's
** source (id 2, at the test's position) through the game logic's findObjectByID, the FX list store (FX_A, FX_B; "fxpos"), the object creation
** list store (OCL_A, OCL_B; "oclat"), the particle system manager (a template for every name but None, ids 1, 2, ...: "psys", "psattach",
** "psdestroyid"), and the REAL DamageModule, INI, RandomValue and Xfer. */
#include "TransitionDamageFX_edit.cpp"

namespace {

ModuleData *l9r31Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = TransitionDamageFX::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r31Saved;
Coord3D g_r31Bones[5];
const Coord3D *r31Bone(const char *name)
{
	static const char *const kNames[5] = { "B1", "B2", "F01", "F02", "F03" };
	for (int i = 0; i < 5; ++i) if (strcmp(name, kNames[i]) == 0) return &g_r31Bones[i];
	return 0;
}
unsigned g_r31FCount = 0;
const Coord3D *r31ArtBone(const char *name)
{
	const Coord3D *p = r31Bone(name);
	if (p && name[0] == 'F' && (unsigned)(name[2] - '0') > g_r31FCount) return 0;
	return p;
}
Object *g_r31Me = 0, *g_r31Source = 0;
Object *r31Find(ObjectID id)
{
	if (g_r31Me && id == g_r31Me->m_oid) return g_r31Me;
	if (g_r31Source && id == g_r31Source->m_oid) return g_r31Source;
	return 0;
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a TransitionDamageFX and make it on object 1. `cfg` (20 words; 3-9 also settable by op 1): 0 the
** logic random seed, 1 the number of art bones F01, F02, F03 there are (0-3), 2 unused, 3 flags (0 drawable, 1 body, 2 a last damage), 4 the
** last damage's type, 5-6 the position (float bits), 7 the orientation (float bits), 8-9 the source's position (float bits), 10-19 the bones B1,
** B2, F01, F02, F03 (x, y float bits; z is 1-5). The data dump goes first (the three damage type masks; every entry's location type, bone, random
** flag, location and effect code). Then `n` ops of 4 words [op, a, b, c]: 0 onBodyDamageStateChange(a damage from object c, a -> b)  1 cfg[a] =
** b  2 xfer save  3 xfer load of the last save  4 xfer CRC. Each op appends: the CATCH_ALL status, the effect log, the xfer status, bytes and CRC,
** the particle system ids. */
int ref_l9r31_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r31Saved.clear();
	g_fxIds.clear();
	g_ahPsLastId = 0;
	g_ahPsNext = 0;
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r31Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[20];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const TransitionDamageFXModuleData *td = (const TransitionDamageFXModuleData *)d;
		const DamageTypeFlags *masks[3] = { &td->m_damageFXTypes, &td->m_damageOCLTypes, &td->m_damageParticleTypes };
		for (int k = 0; k < 3; ++k) {
			unsigned w[2] = { 0, 0 };
			for (int t = 0; t < DAMAGE_NUM_TYPES && t < 64; ++t) if (getDamageTypeFlag(*masks[k], (DamageType)t)) w[t / 32] |= 1u << (t % 32);
			putU32(w[0]);
			putU32(w[1]);
		}
		for (int k = 0; k < 3; ++k) for (int s = 0; s < BODYDAMAGETYPE_COUNT; ++s) for (int i = 0; i < DAMAGE_MODULE_MAX_FX; ++i) {
			const FXLocInfo &l = k == 0 ? td->m_fxList[s][i].locInfo : k == 1 ? td->m_OCL[s][i].locInfo : td->m_particleSystem[s][i].locInfo;
			putI32((int)l.locType);
			putStr(l.boneName.str(), (int)l.boneName.getLength());
			putU32(l.randomBone ? 1 : 0);
			putU32(realBits(l.loc.x));
			putU32(realBits(l.loc.y));
			putU32(realBits(l.loc.z));
			if (k == 0) putI32(fxId(td->m_fxList[s][i].fx));
			else if (k == 1) putI32(td->m_OCL[s][i].ocl == 0 ? 0 : (td->m_OCL[s][i].ocl == &g_upOcls[0] ? 1 : 2));
			else { const char *nm = ahPsName(td->m_particleSystem[s][i].particleSysTemplate); putStr(nm, (int)strlen(nm)); }
		}
	}
	GameLogic gl;
	gl.m_frame = 1;
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[0]);
	g_r30BoneHook = 0;
	Object *me = newObject(), *source = newObject();
	me->m_oid = 1;
	source->m_oid = 2;
	g_r31Me = me;
	g_r31Source = source;
	g_modFindHook = r31Find;
	MBody body;
	body.m_owner = me;
	static DamageInfo lastDamage;
	Drawable draw;
	draw.m_ownerObject = me;
	auto scene = [&]() {
		unsigned f = cfg[3];
		me->m_drawable = (f & 1) ? &draw : 0;
		me->m_body = (f >> 1 & 1) ? &body : 0;
		lastDamage = DamageInfo();
		lastDamage.in.m_damageType = (DamageType)cfg[4];
		body.m_lastDamage = (f >> 2 & 1) ? &lastDamage : 0;
		memcpy(&me->m_pos.x, &cfg[5], 4);
		memcpy(&me->m_pos.y, &cfg[6], 4);
		me->m_pos.z = 0.0f;
		memcpy(&me->m_orientation, &cfg[7], 4);
		memcpy(&source->m_pos.x, &cfg[8], 4);
		memcpy(&source->m_pos.y, &cfg[9], 4);
		source->m_pos.z = 0.0f;
		for (int k = 0; k < 5; ++k) {
			memcpy(&g_r31Bones[k].x, &cfg[10 + 2 * k], 4);
			memcpy(&g_r31Bones[k].y, &cfg[11 + 2 * k], 4);
			g_r31Bones[k].z = (Real)(k + 1);
		}
	};
	scene();
	/* the art's numbered bones: F01 .. F<cfg[1]> */
	g_r31FCount = cfg[1];
	g_r30BoneHook = r31ArtBone;
	TransitionDamageFX *m = new TransitionDamageFX(me, d);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 1 && o[1] >= 3 && o[1] <= 9) cfg[o[1]] = o[2];
		scene();
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[1024];
		int blen = 0;
		int xst = -2;
		try {
			switch (o[0]) {
			case 0: {
				DamageInfo di;
				di.in.m_sourceID = (ObjectID)o[3];
				m->onBodyDamageStateChange(&di, (BodyDamageType)o[1], (BodyDamageType)o[2]);
				break;
			}
			case 2: xst = abRunXfer(m, 0, xfer_path, bytes, 1024, &blen, &crc); g_r31Saved.assign(bytes, bytes + blen); break;
			case 3: {
				std::vector<unsigned char> b = g_r31Saved;
				if (b.size() > 1024) b.resize(1024);
				xst = abRunXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 4: xst = abRunXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
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
		for (int s = 0; s < BODYDAMAGETYPE_COUNT; ++s) for (int j = 0; j < DAMAGE_MODULE_MAX_FX; ++j) putU32(m->m_particleSystemID[s][j]);
	}
	*len = finish(buf, cap);
	g_r30BoneHook = 0;
	g_modFindHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	me->m_body = 0; me->m_drawable = 0;
	delete me;
	delete source;
	g_r31Me = g_r31Source = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */

