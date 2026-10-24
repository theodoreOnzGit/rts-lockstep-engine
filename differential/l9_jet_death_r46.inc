/* Differential-test driver for JetSlowDeathBehavior (p1w13-sonnet, batch R46; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/JetSlowDeathBehavior.cpp   (#included, unchanged)
** on SlowDeathBehavior.cpp (batch Q's, in the unit) and batch Q's scene (differential/l9_slow_death_q.inc, whose stand-ins this reuses: the
** body, the drawable and the AI that log, the physics behaviour as a block of memory, the slaved update, the game detail manager, the FX, OCL
** and weapon stores, the game logic), with module 0 the jet (its data read from the INI) and up to two SlowDeathBehaviors from the scene,
** plus: the jet's AI's current locomotor (maybe; setMaxLift and setMaxTurnRate lines), the physics' setRollRate and setPitchRate (lines),
** the gravity (TheGlobalData's and isSignificantlyAboveTerrain's), the terrain's layers and layer heights (the HeightDie and OpenContain
** drivers' getLayerForDestination and getLayerHeight; setLayer a line), the DECK_HEIGHT_OFFSET status bit, and the audio manager (lines). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/JetSlowDeathBehavior.cpp"

void PhysicsBehavior::setRollRate(Real roll) { ulog("proll %d %08x", ctPhysOwner(this), realBits(roll)); }
void Locomotor::setMaxLift(Real lift) { ulog("maxlift %d %08x", (int)((AIUpdateInterface *)(void *)this)->m_owner->m_oid, realBits(lift)); }
void Locomotor::setMaxTurnRate(Real turn) { ulog("maxturn %d %08x", (int)((AIUpdateInterface *)(void *)this)->m_owner->m_oid, realBits(turn)); }

namespace {

ModuleData *l9jdRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = JetSlowDeathBehavior::friend_newModuleData(&ini);
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

/* batch Q's stand-ins (l9_slow_death_q.inc) serve: g_sdObj, g_sdTree, g_sdSlaved, sdFind, sdDestroy, sdFindUpdate, the FX store's g_sdFx, g_sdLod,
** the weapon store, SdBody, sdF */
std::vector<unsigned char> g_jdSaved;
/* the jet's own FX fields are read by INI::parseFXList, whose store lookup is wrap_armor.cpp's (a distinct pointer per
** name, named back by ref_armor_fx_name); the base's are Q's stand-in store (&g_sdFx[i]). Both are named here. */
extern "C" const char *ref_armor_fx_name(const void *fx);
int jdFx(const FXList *fx)
{
	if (fx == 0) return 0;
	if (fx == &g_sdFx[0] || strcmp(ref_armor_fx_name(fx), "FX_A") == 0) return 1;
	return 2;
}
int jdOcl(const ObjectCreationList *ocl) { return ocl == 0 ? 0 : (ocl == &g_upOcls[0] ? 1 : 2); }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a JetSlowDeathBehavior (module 0) and make it, and cfg[3] - 1 SlowDeathBehaviors, on an Object
** (id 1): batch Q's `ref_l9sd_run` (l9_slow_death_q.inc), with the jet's data after the base's in the dump, plus cfg[61] the gravity (float
** bits), 62 flags (bit 0 the AI has a current locomotor, 1 DECK_HEIGHT_OFFSET), 63 the eight cells' layers (nibbles), 64 their layer heights
** (nibbles, halves); and the ops 21 cfg[62] = a, 22 cfg[63] = a, 23 cfg[64] = a. Each op appends batch Q's outputs, then the jet's two
** frames and roll rate and the DECK_HEIGHT_OFFSET bit. */
static int l9jdRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_jdSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9jdRead(filename); }
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
		const JetSlowDeathBehaviorModuleData *jd = (const JetSlowDeathBehaviorModuleData *)d;
		const FXList *fxs[6] = { jd->m_fxOnGroundDeath, jd->m_fxInitialDeath, jd->m_fxSecondary, jd->m_fxHitGround, jd->m_fxFinalBlowUp, 0 };
		const ObjectCreationList *ocls[6] = { jd->m_oclOnGroundDeath, jd->m_oclInitialDeath, jd->m_oclSecondary, jd->m_oclHitGround, jd->m_oclFinalBlowUp, 0 };
		for (int k = 0; k < 5; ++k) { putI32(jdFx(fxs[k])); putI32(jdOcl(ocls[k])); }
		putU32(jd->m_delaySecondaryFromInitialDeath);
		putU32(jd->m_delayFinalBlowUpFromHitGround);
		putStr(jd->m_deathLoopSound.getEventName().str(), (int)jd->m_deathLoopSound.getEventName().getLength());
		putU32(realBits(jd->m_rollRate));
		putU32(realBits(jd->m_rollRateDelta));
		putU32(realBits(jd->m_pitchRate));
		putU32(realBits(jd->m_fallHowFast));
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
	GameLogic gl;
	gl.m_frame = cfg[0];
	gl.m_hulkOverride = (Int)cfg[2];
	g_sdLod.m_slowDeathScale = 0;
	memcpy(&g_sdLod.m_slowDeathScale, &cfg[1], 4);
	g_sdLogic = &gl;
	TheGameLogic = &gl;
	TheWeaponStore = &g_sdWeaponStore;
	g_modFindHook = sdFind;
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
	SdBody *body = new SdBody(obj, new BodyModuleData);
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
		try { mods[k] = k == 0 ? (SlowDeathBehavior *)new JetSlowDeathBehavior(obj, datas[k]) : new SlowDeathBehavior(obj, datas[k]); }
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
				case 18: xst = runXfer(mods[k], 0, xfer_path, bytes, 512, &blen, &crc); g_jdSaved.assign(bytes, bytes + blen); break;
				case 21: jetFlags = o[1]; ai.m_hasLoco = (jetFlags & 1) != 0; if (jetFlags & 2) obj->m_status.set(OBJECT_STATUS_DECK_HEIGHT_OFFSET); else obj->m_status.set(OBJECT_STATUS_DECK_HEIGHT_OFFSET, 0); break;
				case 22: layers = o[1]; jetScene(); break;
				case 23: layerHeights = o[1]; jetScene(); break;
				case 19: {
					std::vector<unsigned char> b = g_jdSaved;
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
		JetSlowDeathBehavior *jet = (JetSlowDeathBehavior *)mods[0];
		putU32(jet->m_timerDeathFrame);
		putU32(jet->m_timerOnGroundFrame);
		putU32(realBits(jet->m_rollRate));
		putU32(obj->m_status.test(OBJECT_STATUS_DECK_HEIGHT_OFFSET) ? 1 : 0);
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
	TheGlobalData = 0;
	TheAudio = 0;
	TheTerrainLogic = 0;
	TheGameLogic = 0; g_modFindHook = 0; g_modDestroyHook = 0; g_findUpdateHook = 0; TheWeaponStore = 0;
	return 0;
}

int ref_l9r46_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9jdRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
