/* Differential-test driver for NeutronMissileSlowDeathBehavior (p1w13-sonnet, batch R49; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/NeutronMissileSlowDeathUpdate.cpp   (#included, unchanged)
** on SlowDeathBehavior.cpp (batch Q's, in the unit) and batch R46's scene (l9_jet_death_r46.inc on batch Q's), with module 0 the missile (its
** data read from the INI) and up to two SlowDeathBehaviors from the scene, plus: up to three other objects (the ids after the scene's: each
** with a body whose attemptDamage is a line, maybe a drawable, maybe KINDOF_SHRUBBERY, a position), the range scan (batch M's list; the call
** a line), the terrain's ground height (the HeightDie driver's table; a line), the game client's scorch (batch R32's line), the REAL
** Object::topple (batch R28's: no object here has a ToppleUpdate) and Object::attemptDamage (batch N's). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/NeutronMissileSlowDeathUpdate.cpp"

namespace {

ModuleData *l9nmRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = NeutronMissileSlowDeathBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_nmSaved;

/* the other objects' body: batch Q's, whose attemptDamage is a line */
class NmBody : public SdBody
{
public:
	NmBody(Thing *t, const ModuleData *md) : SdBody(t, md) {}
	virtual void attemptDamage(DamageInfo *di)
	{
		ulog("damage %d %08x %d %d %d", (int)getObject()->m_oid, realBits(di->in.m_amount), (int)di->in.m_damageType, (int)di->in.m_deathType,
			(int)di->in.m_sourceID);
	}
};
/* findObjectByID: the other objects too (batch Q's sdFind knows the missile and the tree) */
Object *g_nmOthers[3] = { 0, 0, 0 };
Object *nmFind(ObjectID id)
{
	for (int k = 0; k < 3; ++k) if (g_nmOthers[k] && g_nmOthers[k]->m_oid == id) return g_nmOthers[k];
	return sdFind(id);
}
void nmScanHook(const Coord3D *pos, Real maxDist, PartitionFilter **)
{
	ulog("scan %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), g_ahRangeDc);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a NeutronMissileSlowDeathBehavior (module 0) and make it, and cfg[3] - 1 SlowDeathBehaviors, on
** an Object (id 1): batch R46's `ref_l9r46_run` (cfg[61..64], ops 21..23), with the missile's data after the base's in the dump, plus cfg[65]
** the number of other objects (0-3), cfg[66] their flags (a nibble each: bit 0 a drawable, bit 1 KINDOF_SHRUBBERY), cfg[67..75] their
** positions, cfg[76] the scan (nibbles: 1 the missile, 2-4 the other objects, 0 ends), cfg[77] the ground height; and the ops 24 the scan = a,
** 25 the ground = a. Each op appends batch Q's outputs, then the missile's activation frame, its two flag arrays (bits) and its scorch flag. */
static int l9nmRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_nmSaved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9nmRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const SlowDeathBehaviorModuleData *sd = (const SlowDeathBehaviorModuleData *)d;
		putU32(realBits(sd->m_sinkRate));
		putI32(sd->m_probabilityModifier);
		putU32(realBits(sd->m_modifierBonusPerOverkillPercent));
		putU32(sd->m_sinkDelay);
		putU32(sd->m_sinkDelayVariance);
		putU32(realBits(sd->m_destructionAltitude));
		putU32(sd->m_destructionDelay);
		putU32(sd->m_destructionDelayVariance);
		putU32(realBits(sd->m_flingForce));
		putU32(realBits(sd->m_flingForceVariance));
		putU32(realBits(sd->m_flingPitch));
		putU32(realBits(sd->m_flingPitchVariance));
		for (int ph = 0; ph < SD_PHASE_COUNT; ++ph) {
			putU32((unsigned)sd->m_fx[ph].size());
			for (size_t i = 0; i < sd->m_fx[ph].size(); ++i) putI32(sd->m_fx[ph][i] == 0 ? 0 : (sd->m_fx[ph][i] == &g_sdFx[0] ? 1 : 2));
			putU32((unsigned)sd->m_ocls[ph].size());
			for (size_t i = 0; i < sd->m_ocls[ph].size(); ++i) putI32(sd->m_ocls[ph][i] == 0 ? 0 : (sd->m_ocls[ph][i] == &g_upOcls[0] ? 1 : 2));
			putU32((unsigned)sd->m_weapons[ph].size());
			for (size_t i = 0; i < sd->m_weapons[ph].size(); ++i) putI32(sd->m_weapons[ph][i] == 0 ? 0 : (sd->m_weapons[ph][i] == (const WeaponTemplate *)&g_kWeapons[0] ? 1 : 2));
		}
		putU32((unsigned)sd->m_maskOfLoadedEffects);
		const NeutronMissileSlowDeathBehaviorModuleData *nd = (const NeutronMissileSlowDeathBehaviorModuleData *)d;
		for (int b = 0; b < MAX_NEUTRON_BLASTS; ++b) {
			const BlastInfo &bi = nd->m_blastInfo[b];
			putU32(bi.enabled ? 1 : 0);
			const Real r[8] = { bi.delay, bi.scorchDelay, bi.innerRadius, bi.outerRadius, bi.maxDamage, bi.minDamage, bi.toppleSpeed, bi.pushForceMag };
			for (int k = 0; k < 8; ++k) putU32(realBits(r[k]));
		}
		putU32(realBits(nd->m_scorchSize));
		putI32(jdFx(nd->m_fxList));
	}
	putU32(cfg[3]);
	initKindOfMasks();
	initDisabledMasks();
	Object *obj = newObject();
	obj->m_oid = 1;
	g_sdObj = obj;
	Object *tree = 0;
	if (cfg[5] >> 7 & 1) {
		tree = newObject();
		tree->m_oid = 2;
		tree->m_kindOf.set(KINDOF_SHRUBBERY);
	}
	g_sdTree = tree;
	/* the other objects (the ids after the scene's), the scan, the ground */
	int nOthers = (int)(cfg[65] > 3 ? 3 : cfg[65]);
	Object *others[3] = { 0, 0, 0 };
	NmBody *otherBodies[3] = { 0, 0, 0 };
	for (int k = 0; k < nOthers; ++k) {
		Object *o = newObject();
		o->m_oid = (ObjectID)((tree ? 3 : 2) + k);
		unsigned fl = cfg[66] >> (4 * k) & 15;
		o->m_drawable = (fl & 1) ? &g_cpDrawable : 0;
		o->m_kindOf.clear();
		if (fl & 2) o->m_kindOf.set(KINDOF_SHRUBBERY);
		o->m_pos.x = sdF(cfg[67 + 3 * k]); o->m_pos.y = sdF(cfg[68 + 3 * k]); o->m_pos.z = sdF(cfg[69 + 3 * k]);
		otherBodies[k] = new NmBody(o, new BodyModuleData);
		o->m_body = otherBodies[k];
		others[k] = o;
		g_nmOthers[k] = o;
	}
	unsigned scanCfg = cfg[76];
	auto nmScan = [&]() {
		g_ahRange.clear();
		for (int k = 0; k < 4; ++k) {
			unsigned c = scanCfg >> (4 * k) & 15;
			if (c == 0) break;
			if (c == 1) g_ahRange.push_back(obj);
			else if (c - 2 < (unsigned)nOthers) g_ahRange.push_back(others[c - 2]);
		}
	};
	nmScan();
	ThePartitionManager = &g_ahPm;
	g_ahRangeHook = nmScanHook;
	for (int c = 0; c < 8; ++c) g_hdGround[c] = sdF(cfg[77]);
	GameLogic gl;
	gl.m_frame = cfg[0];
	gl.m_hulkOverride = (Int)cfg[2];
	g_sdLod.m_slowDeathScale = 0;
	memcpy(&g_sdLod.m_slowDeathScale, &cfg[1], 4);
	g_sdLogic = &gl;
	TheGameLogic = &gl;
	TheWeaponStore = &g_sdWeaponStore;
	g_modFindHook = nmFind;
	g_modDestroyHook = sdDestroy;
	g_findUpdateHook = sdFindUpdate;
	g_sdSlaved = (cfg[5] >> 3 & 1) != 0;
	obj->m_pos.x = sdF(cfg[8]);
	obj->m_pos.y = sdF(cfg[9]);
	obj->m_pos.z = sdF(cfg[10]);
	obj->m_level = (VeterancyLevel)cfg[6];
	for (int b = 0; b < 8; ++b) if (cfg[12] >> b & 1) obj->m_status.set((ObjectStatusTypes)b);
	obj->m_kindOf.clear();
	if (cfg[4] & 1) obj->m_kindOf.set(KINDOF_INFANTRY);
	if (cfg[4] & 2) obj->m_kindOf.set(KINDOF_HULK);
	obj->m_drawable = (cfg[5] & 1) ? &g_cpDrawable : 0;
	AIUpdateInterface ai;
	ai.m_owner = obj;
	ai.m_dead = (cfg[5] >> 6 & 1) != 0;
	obj->m_ai = (cfg[5] >> 1 & 1) ? &ai : 0;
	/* the physics memory is the contain driver's first block, so that its setPitchRate (and setRollRate) name object 1 */
	g_ctN = 1;
	memset(g_ctPhysMem[0], 0, sizeof(g_ctPhysMem[0]));
	PhysicsBehavior *phys = (PhysicsBehavior *)g_ctPhysMem[0];
	/* the jet's scene: the gravity, the locomotor, the deck status, the terrain's layers */
	GlobalData globals;
	memcpy(&globals.m_gravity, &cfg[61], 4);
	TheGlobalData = &globals;
	memcpy(&g_ddGravity, &cfg[61], 4);
	TheAudio = &g_ddAudio;
	g_audioHandleLast = 0;
	TheTerrainLogic = &g_hdTerrain;
	unsigned jetFlags = cfg[62], layers = cfg[63], layerHeights = cfg[64];
	auto jetScene = [&]() {
		for (int c = 0; c < 8; ++c) { g_hdLayer[c] = (int)(layers >> (4 * c) & 15); g_hdLayerHeight[c] = (Real)(layerHeights >> (4 * c) & 15) * 0.5f; }
	};
	jetScene();
	ai.m_hasLoco = (jetFlags & 1) != 0;
	if (jetFlags & 2) obj->m_status.set(OBJECT_STATUS_DECK_HEIGHT_OFFSET);
	phys->m_lastCollidee = (ObjectID)cfg[11];
	obj->m_physics = (cfg[5] >> 2 & 1) ? phys : 0;
	SdBody *body = new NmBody(obj, new BodyModuleData);   /* (the missile's damage, when the scan lists it, is a line too) */
	body->m_max = sdF(cfg[7]);
	obj->m_body = (cfg[5] >> 4 & 1) ? body : 0;
	obj->m_disabledBits = (cfg[5] >> 5 & 1) ? (1u << DISABLED_HELD) : 0;
	InitGameLogicRandom(cfg[35]);
	unsigned nInfos = cfg[36];
	DamageInfo infos[8];
	for (unsigned i = 0; i < nInfos && i < 8; ++i) {
		infos[i].in.m_deathType = (DeathType)cfg[37 + 3 * i];
		infos[i].out.m_actualDamageDealt = sdF(cfg[38 + 3 * i]);
		infos[i].out.m_actualDamageClipped = sdF(cfg[39 + 3 * i]);
	}
	g_events.clear();
	g_ulog.clear();
	int nMods = (int)cfg[3];
	SlowDeathBehavior *mods[3] = { 0, 0, 0 };
	BehaviorModule *behaviors[4] = { 0, 0, 0, 0 };
	ModuleData *datas[3] = { d, 0, 0 };
	obj->m_behaviors = behaviors;
	for (int k = 0; k < nMods; ++k) {
		if (k > 0) {
			SlowDeathBehaviorModuleData *x = new SlowDeathBehaviorModuleData;
			const unsigned *w = cfg + 13 + 11 * (k - 1);
			x->m_probabilityModifier = (Int)w[0];
			x->m_modifierBonusPerOverkillPercent = sdF(w[1]);
			x->m_sinkRate = sdF(w[2]);
			x->m_sinkDelay = w[3];
			x->m_sinkDelayVariance = w[4];
			x->m_destructionDelay = w[5];
			x->m_destructionDelayVariance = w[6];
			x->m_flingForce = sdF(w[7]);
			x->m_flingForceVariance = sdF(w[8]);
			x->m_flingPitch = sdF(w[9]);
			x->m_flingPitchVariance = sdF(w[10]);
			datas[k] = x;
		}
		status = 0;
		try { mods[k] = k == 0 ? (SlowDeathBehavior *)new NeutronMissileSlowDeathBehavior(obj, datas[k]) : new SlowDeathBehavior(obj, datas[k]); }
		CATCH_ALL(status, code, msg, msgCap)
		if (status) {
			delete body;
			for (int j = 0; j < k; ++j) delete mods[j];
			for (int j = 1; j <= k; ++j) delete datas[j];
			delete d;
			delete obj;
			delete tree;
			TheGameLogic = 0; g_modFindHook = 0; g_modDestroyHook = 0; g_findUpdateHook = 0; TheWeaponStore = 0;
			return 5;
		}
		behaviors[k] = mods[k];
	}
	for (int phase = 0; phase < 1 + n; ++phase) {
		const unsigned *o = phase ? ops + 5 * (phase - 1) : 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		unsigned result = 0;
		int sleep = 0;
		if (phase) {
			g_events.clear();
			g_ulog.clear();
			gl.m_awakenCalls = 0;
			gl.m_awakenFrame = 0;
			gl.m_awakenModule = 0;
			int k = (int)(o[1] % (unsigned)nMods);
			const DamageInfo *info = infos + ((nInfos ? (o[0] == 1 ? o[1] : o[2]) % nInfos : 0) & 7);
			DamageInfo local = *info;
			try {
				switch (o[0]) {
				case 0: sleep = (int)mods[k]->update(); break;
				case 1: for (int j = 0; j < nMods; ++j) { DamageInfo di = local; mods[j]->onDie(&di); } break;
				case 2: { DamageInfo di = local; mods[k]->onDie(&di); break; }
				case 3: mods[k]->beginSlowDeath(&local); break;
				case 4: result = (unsigned)mods[k]->getProbabilityModifier(&local); break;
				case 5: result = mods[k]->isDieApplicable(&local) ? 1 : 0; break;
				case 6: gl.m_frame = o[1]; break;
				case 7: memcpy(&g_sdLod.m_slowDeathScale, &o[1], 4); break;
				case 8: gl.m_hulkOverride = (Int)o[1]; break;
				case 9: obj->m_pos.x = sdF(o[1]); obj->m_pos.y = sdF(o[2]); obj->m_pos.z = sdF(o[3]); break;
				case 10: break;   /* (the Rust side's kinds are its template's: no op) */
				case 11: obj->m_disabledBits = o[1] ? (1u << DISABLED_HELD) : 0; break;
				case 12: phys->m_lastCollidee = (ObjectID)o[1]; break;
				case 13: ai.m_dead = o[1] != 0; break;
				case 14: obj->m_drawable = o[1] ? &g_cpDrawable : 0; break;
				case 15: obj->m_level = (VeterancyLevel)o[1]; break;
				case 16: body->m_max = sdF(o[1]); break;
				case 17: obj->m_status.clear(); for (int b = 0; b < 8; ++b) if (o[1] >> b & 1) obj->m_status.set((ObjectStatusTypes)b); break;
				case 18: xst = runXfer(mods[k], 0, xfer_path, bytes, 512, &blen, &crc); g_nmSaved.assign(bytes, bytes + blen); break;
				case 21: jetFlags = o[1]; ai.m_hasLoco = (jetFlags & 1) != 0; if (jetFlags & 2) obj->m_status.set(OBJECT_STATUS_DECK_HEIGHT_OFFSET); else obj->m_status.set(OBJECT_STATUS_DECK_HEIGHT_OFFSET, 0); break;
				case 22: layers = o[1]; jetScene(); break;
				case 24: scanCfg = o[1]; nmScan(); break;
				case 25: for (int c = 0; c < 8; ++c) g_hdGround[c] = sdF(o[1]); break;
				case 23: layerHeights = o[1]; jetScene(); break;
				case 19: {
					std::vector<unsigned char> b = g_nmSaved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(mods[k], 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				default: xst = runXfer(mods[k], 2, "crc", bytes, 0, &blen, &crc); break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
		}
		if (phase) { putI32(st); putU32(result); putI32(sleep); }
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1 && e.p) { char b[32]; sprintf(b, "fx %s", jdFx((const FXList *)e.p) == 1 ? "FX_A" : "FX_B"); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		int lastModule = -1;
		for (int j = 0; j < nMods; ++j) if (gl.m_awakenModule == (UpdateModule *)mods[j]) lastModule = j;
		putU32((unsigned)gl.m_awakenCalls);
		putI32(gl.m_awakenCalls ? lastModule : -1);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		for (int j = 0; j < nMods; ++j) {
			putU32(mods[j]->m_sinkFrame);
			putU32(mods[j]->m_midpointFrame);
			putU32(mods[j]->m_destructionFrame);
			putU32(realBits(mods[j]->m_acceleratedTimeScale));
			putU32(mods[j]->m_flags);
		}
		if (phase) {
			putI32(xst);
			putU32((unsigned)blen);
			for (int j = 0; j < blen; ++j) putU32(bytes[j]);
			putU32(crc);
		}
		putU32(realBits(obj->m_pos.x));
		putU32(realBits(obj->m_pos.y));
		putU32(realBits(obj->m_pos.z));
		putU32(obj->m_disabledBits);
		putU32(ai.m_dead ? 1 : 0);
		putU32(phys->getAllowToFall() ? 1 : 0);
		putU32(phys->getFlag(PhysicsBehavior::ALLOW_BOUNCE) ? 1 : 0);
		putU32(realBits(phys->m_extraBounciness));
		putU32(realBits(phys->m_extraFriction));
		NeutronMissileSlowDeathBehavior *nm = (NeutronMissileSlowDeathBehavior *)mods[0];
		putU32(nm->m_activationFrame);
		unsigned bl = 0, sc = 0;
		for (int b = 0; b < MAX_NEUTRON_BLASTS; ++b) { bl |= (nm->m_completedBlasts[b] ? 1u : 0u) << b; sc |= (nm->m_completedScorchBlasts[b] ? 1u : 0u) << b; }
		putU32(bl);
		putU32(sc);
		putU32(nm->m_scorchPlaced ? 1 : 0);
	}
	*len = finish(buf, cap);
	for (int j = 0; j < nMods; ++j) delete mods[j];
	for (int j = 1; j < nMods; ++j) delete datas[j];
	delete body;
	delete d;
	delete obj;
	delete tree;
	g_sdObj = g_sdTree = 0;
	g_ctN = 0;
	for (int k = 0; k < nOthers; ++k) { delete otherBodies[k]; delete others[k]; g_nmOthers[k] = 0; }
	g_ahRange.clear();
	g_ahRangeHook = 0;
	ThePartitionManager = 0;
	for (int c = 0; c < 8; ++c) g_hdGround[c] = 0.0f;
	TheGlobalData = 0;
	TheAudio = 0;
	TheTerrainLogic = 0;
	TheGameLogic = 0; g_modFindHook = 0; g_modDestroyHook = 0; g_findUpdateHook = 0; TheWeaponStore = 0;
	return 0;
}

int ref_l9r49_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9nmRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
