/* Differential-test driver for BoneFXUpdate and BoneFXDamage (p1w13-sonnet, batch R30; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BoneFXUpdate.cpp   (#included with mechanical edits made by build.rs into
**     mod_gen/BoneFXUpdate_edit.cpp: its particle system parser is this unit's l9ahParsePsTemplate, it includes the renamed header copy, and its
**     function-local name key is not `static`, the test resetting the name key generator for every case; each asserted)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/BoneFXDamage.cpp   (#included with its two name keys not `static`, the same way:
**     mod_gen/BoneFXDamage_edit.cpp)
** in wrap_module.cpp's unit, at its end. StructureCollapseUpdate's driver (l9_collapse_r29.inc) has a stand-in BoneFXUpdate, so the classes of
** BoneFXUpdate.h are compiled here under R30 names (build.rs's copy of the header, mod_gen/BoneFXUpdate_r30.h, renamed by the macros below). The stand-ins: the object (id 1: a body, the module build's MBody,
** whose last damage is none or the test's; a drawable that is hidden or not and whose art has the bones B1-B4 at the test's positions; a
** position and an orientation, which convertBonePosToWorldPos pushes the bones through), findUpdateModule and findDamageModule (the two
** modules by the keys of their names), the FX list store (FX_A, FX_B; FXList::doFXPos is "fxpos"), the object creation list store (OCL_A,
** OCL_B; "oclat"), the particle system manager (a template for every name but None, ids 1, 2, ...: "psys", "psattach", "psstop", "psdestroy"),
** the game logic (its frame, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them), and the REAL UpdateModule,
** DamageModule, INI, RandomValue (the logic's and the client's) and Xfer. */
#define BoneFXUpdate R30BoneFXUpdate
#define BoneFXUpdateModuleData R30BoneFXUpdateModuleData
#define getBoneFXUpdateModuleData getR30BoneFXUpdateModuleData
#define BoneLocInfo R30BoneLocInfo
#define BaseBoneListInfo R30BaseBoneListInfo
#define BoneFXListInfo R30BoneFXListInfo
#define BoneOCLInfo R30BoneOCLInfo
#define BoneParticleSystemInfo R30BoneParticleSystemInfo
#define BONE_FX_MAX_BONES R30_BONE_FX_MAX_BONES
/* StructureCollapseUpdate.cpp (l9_collapse_r29.inc) and FireWeaponWhenDeadBehavior.cpp define these in this unit too */
#define MAX_IDX BfxMAX_IDX
#define inList BfxInList
#define buildNonDupRandomIndexList BfxBuildNonDupRandomIndexList
#include "BoneFXUpdate_edit.cpp"
#include "BoneFXDamage_edit.cpp"
#undef MAX_IDX
#undef inList
#undef buildNonDupRandomIndexList

/* the art's bones (Drawable::getPristineBonePositions, differential/shim_module/GameClient/Drawable.h) */
const Coord3D *(*g_r30BoneHook)(const char *) = 0;

namespace {

ModuleData *l9r30Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = R30BoneFXUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r30Saved;
UpdateModule *g_r30Updating = 0;
void r30Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r30Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Coord3D g_r30Bones[4];
const Coord3D *r30Bone(const char *name)
{
	static const char *const kNames[4] = { "B1", "B2", "B3", "B4" };
	for (int i = 0; i < 4; ++i) if (strcmp(name, kNames[i]) == 0) return &g_r30Bones[i];
	return 0;
}
R30BoneFXUpdate *g_r30Update = 0;
BoneFXDamage *g_r30Damage = 0;
UpdateModule *r30FindUpdate(const Object *, NameKeyType key) { return key == TheNameKeyGenerator->nameToKey("BoneFXUpdate") ? g_r30Update : 0; }
DamageModule *(*g_r30FindDamageHook)(NameKeyType) = 0;
DamageModule *r30FindDamage(NameKeyType key) { return key == TheNameKeyGenerator->nameToKey("BoneFXDamage") ? g_r30Damage : 0; }

}  // namespace

DamageModule *Object::findDamageModule(NameKeyType key) const { return g_r30FindDamageHook ? g_r30FindDamageHook(key) : 0; }
void ParticleSystem::stop(void) { ulog("psstop %u", m_id); }

extern "C" {

/* Read `filename` (block `D`) as the data of a BoneFXUpdate and make it on object 1 at frame cfg[0], with a BoneFXDamage if cfg[3] bit 4. `cfg`
** (16 words; 3-7 also settable by op 5): 0 the frame, 1 the logic random seed, 2 the client random seed, 3 flags (0 drawable, 1 hidden, 2 body,
** 3 a last damage, 4 a BoneFXDamage), 4 the last damage's type, 5-6 the position (float bits), 7 the orientation (float bits), 8-15 the bones
** B1-B4 (x, y float bits; z is 1-4). The data dump goes first (the three damage type masks; every entry's bone, once flag, delay when it has a
** bone, effect code), then the constructor's wake calls. Then `n` ops of 4 words [op, a, b, c]: 0 update()  1 a body damage state change a -> b
** (BoneFXDamage::onBodyDamageStateChange if there is one, else BoneFXUpdate::changeBodyDamageState)  2 stopAllBoneFX  3 onObjectCreated (the
** update's, then the damage's)  4 the frame = a  5 cfg[a] = b  6 xfer save  7 xfer load of the last save  8 xfer CRC. Each op appends: the
** CATCH_ALL status, the result, the effect log, the wake calls (count, last frame), the xfer status, bytes and CRC, the state (the next frames,
** the bone positions, the state, the resolved flags, the active flag, the particle system ids). */
int ref_l9r30_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r30Saved.clear();
	g_fxIds.clear();
	g_ahPsLastId = 0;
	g_ahPsNext = 0;
	g_abDeadMod = 0;
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r30Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[16];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const R30BoneFXUpdateModuleData *bd = (const R30BoneFXUpdateModuleData *)d;
		const DamageTypeFlags *masks[3] = { &bd->m_damageFXTypes, &bd->m_damageOCLTypes, &bd->m_damageParticleTypes };
		for (int k = 0; k < 3; ++k) {
			unsigned w[2] = { 0, 0 };
			for (int t = 0; t < DAMAGE_NUM_TYPES && t < 64; ++t) if (getDamageTypeFlag(*masks[k], (DamageType)t)) w[t / 32] |= 1u << (t % 32);
			putU32(w[0]);
			putU32(w[1]);
		}
		for (int k = 0; k < 3; ++k) for (int s = 0; s < BODYDAMAGETYPE_COUNT; ++s) for (int i = 0; i < R30_BONE_FX_MAX_BONES; ++i) {
			const R30BaseBoneListInfo *b = k == 0 ? (const R30BaseBoneListInfo *)&bd->m_fxList[s][i] : k == 1 ? (const R30BaseBoneListInfo *)&bd->m_OCL[s][i]
				: (const R30BaseBoneListInfo *)&bd->m_particleSystem[s][i];
			putStr(b->locInfo.boneName.str(), (int)b->locInfo.boneName.getLength());
			putU32(b->onlyOnce ? 1 : 0);
			if (!b->locInfo.boneName.isEmpty()) {
				if (k == 2) {
					putU32(realBits(b->gameClientDelay.getMinimumValue()));
					putU32(realBits(b->gameClientDelay.getMaximumValue()));
					putU32((unsigned)b->gameClientDelay.getDistributionType());
				} else {
					putU32(realBits(b->gameLogicDelay.getMinimumValue()));
					putU32(realBits(b->gameLogicDelay.getMaximumValue()));
					putU32((unsigned)b->gameLogicDelay.getDistributionType());
				}
			}
			if (k == 0) putI32(fxId(bd->m_fxList[s][i].fx));
			else if (k == 1) putI32(bd->m_OCL[s][i].ocl == 0 ? 0 : (bd->m_OCL[s][i].ocl == &g_upOcls[0] ? 1 : 2));
			else { const char *nm = ahPsName(bd->m_particleSystem[s][i].particleSysTemplate); putStr(nm, (int)strlen(nm)); }
		}
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitRandom(cfg[2]);
	InitGameLogicRandom(cfg[1]);
	g_findUpdateHook = r30FindUpdate;
	g_r30FindDamageHook = r30FindDamage;
	g_r30BoneHook = r30Bone;
	Object *me = newObject();
	me->m_oid = 1;
	MBody body;
	body.m_owner = me;
	static DamageInfo lastDamage;
	Drawable draw;
	draw.m_ownerObject = me;
	auto scene = [&]() {
		unsigned f = cfg[3];
		me->m_drawable = (f & 1) ? &draw : 0;
		draw.m_hiddenR26 = (f >> 1 & 1) != 0;
		me->m_body = (f >> 2 & 1) ? &body : 0;
		lastDamage = DamageInfo();
		lastDamage.in.m_damageType = (DamageType)cfg[4];
		body.m_lastDamage = (f >> 3 & 1) ? &lastDamage : 0;
		memcpy(&me->m_pos.x, &cfg[5], 4);
		memcpy(&me->m_pos.y, &cfg[6], 4);
		me->m_pos.z = 0.0f;
		memcpy(&me->m_orientation, &cfg[7], 4);
		for (int k = 0; k < 4; ++k) {
			memcpy(&g_r30Bones[k].x, &cfg[8 + 2 * k], 4);
			memcpy(&g_r30Bones[k].y, &cfg[9 + 2 * k], 4);
			g_r30Bones[k].z = (Real)(k + 1);
		}
	};
	scene();
	gl.m_awakenHook = r30Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	R30BoneFXUpdate *m = new R30BoneFXUpdate(me, d);
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	DamageModuleData damageData;
	BoneFXDamage *dm = (cfg[3] >> 4 & 1) ? new BoneFXDamage(me, &damageData) : 0;
	g_r30Update = m;
	g_r30Damage = dm;
	BehaviorModule *behaviors[3] = { m, dm, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 5 && o[1] >= 3 && o[1] <= 7) cfg[o[1]] = o[2];
		if (o[0] == 4) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		gl.m_awakenCalls = 0;
		gl.m_awakenFrame = 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[4096];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: g_r30Updating = m; res = (unsigned)m->update(); break;
			case 1:
				if (dm) { DamageInfo di; dm->onBodyDamageStateChange(&di, (BodyDamageType)o[1], (BodyDamageType)o[2]); }
				else m->changeBodyDamageState((BodyDamageType)o[1], (BodyDamageType)o[2]);
				break;
			case 2: m->stopAllBoneFX(); break;
			case 3: m->onObjectCreated(); if (dm) dm->onObjectCreated(); break;
			case 6: xst = abRunXfer(m, 0, xfer_path, bytes, 4096, &blen, &crc); g_r30Saved.assign(bytes, bytes + blen); break;
			case 7: {
				std::vector<unsigned char> b = g_r30Saved;
				if (b.size() > 4096) b.resize(4096);
				xst = abRunXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 8: xst = abRunXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r30Updating = 0;
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		for (int k = 0; k < 3; ++k) for (int s = 0; s < BODYDAMAGETYPE_COUNT; ++s) for (int j = 0; j < R30_BONE_FX_MAX_BONES; ++j)
			putU32((unsigned)(k == 0 ? m->m_nextFXFrame[s][j] : k == 1 ? m->m_nextOCLFrame[s][j] : m->m_nextParticleSystemFrame[s][j]));
		for (int k = 0; k < 3; ++k) for (int s = 0; s < BODYDAMAGETYPE_COUNT; ++s) for (int j = 0; j < R30_BONE_FX_MAX_BONES; ++j) {
			const Coord3D &c = k == 0 ? m->m_FXBonePositions[s][j] : k == 1 ? m->m_OCLBonePositions[s][j] : m->m_PSBonePositions[s][j];
			putU32(realBits(c.x));
			putU32(realBits(c.y));
			putU32(realBits(c.z));
		}
		putU32((unsigned)m->m_curBodyState);
		for (int s = 0; s < BODYDAMAGETYPE_COUNT; ++s) putU32(m->m_bonesResolved[s] ? 1 : 0);
		putU32(m->m_active ? 1 : 0);
		putU32((unsigned)m->m_particleSystemIDs.size());
		for (size_t j = 0; j < m->m_particleSystemIDs.size(); ++j) putU32(m->m_particleSystemIDs[j]);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_findUpdateHook = 0;
	g_r30FindDamageHook = 0;
	g_r30BoneHook = 0;
	me->m_behaviors = 0;
	/* (the destructor kills the running particle systems: not compared, as the Rust side has no destruction hook) */
	m->m_particleSystemIDs.clear();
	delete m;
	delete dm;
	delete d;
	me->m_body = 0; me->m_drawable = 0;
	delete me;
	g_r30Update = 0;
	g_r30Damage = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
