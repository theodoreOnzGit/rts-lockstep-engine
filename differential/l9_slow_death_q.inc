/* Differential-test driver for SlowDeathBehavior (p1w13-sonnet, batch Q; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SlowDeathBehavior.cpp   (#included, with two mechanical edits made by build.rs into
**   mod_gen/SlowDeathBehavior_edit.cpp: the function-local `static` name key of SlavedUpdate loses its `static`, and the call on the SlavedUpdate it finds
**   names l9sdSlaverDie, the real SlavedUpdate needing the whole slaved layer; both asserted once) with its header unchanged, in wrap_module.cpp's unit on the
**   stand-ins of differential/shim_module: an Object (id 1) with up to three SlowDeathBehaviors (the first reads the INI, the others take their numbers from
**   the scene), a body that knows its maximum health, a drawable and an AI that log, a physics behaviour that is a block of memory (its inline setters write
**   fields the dump reads; applyForce and setAngles log) and a slaved update that logs, the game detail manager (the slow death scale), the FX list store
**   (FX_A and FX_B), the object creation list store (OCL_A, OCL_B; l9_upgrades_b.inc), the weapon store (WEAP_A, WEAP_B; l9_behaviors_k.inc), the game logic
**   (frame, hulk override, destroyObject and deselectObject log), and the REAL UpdateModule, DieMuxData, INI, RandomValue, Matrix3D and Xfer. */
/* FireWeaponWhenDeadBehavior.cpp (l9_behaviors_k.inc) defines the same two constants, and SlowDeathBehavior.cpp uses the macros min and max of BaseType.h */
#define BEGIN_MIDPOINT_RATIO SdBEGIN_MIDPOINT_RATIO
#define END_MIDPOINT_RATIO SdEND_MIDPOINT_RATIO
#define min(x,y) (((x)<(y)) ? (x) : (y))
#define max(x,y) (((x)>(y)) ? (x) : (y))
void l9sdSlaverDie(UpdateModule *slave);
#include "SlowDeathBehavior_edit.cpp"
#undef BEGIN_MIDPOINT_RATIO
#undef END_MIDPOINT_RATIO
#undef min
#undef max

#include "GameLogic/Module/PhysicsUpdate.h"

namespace {

ModuleData *l9sdRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = SlowDeathBehavior::friend_newModuleData(&ini);
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

/* the FX lists the store finds: FX_A and FX_B (case matters); `None` and any other name are NULL */
FXList g_sdFx[2];
const char *const kSdFxNames[2] = { "FX_A", "FX_B" };
FXListStore g_sdFxStore;
GameLODManager g_sdLod;
ModTestWeaponStore g_sdWeaponStore;

/* the body: what the module asks of one is its maximum health */
class SdBody : public BodyModule
{
public:
	SdBody(Thing *thing, const ModuleData *md) : BodyModule(thing, md), m_max(0) {}
	virtual NameKeyType getModuleNameKey() const { return NAMEKEY("SdBody"); }
	virtual void attemptDamage(DamageInfo *) {}
	virtual void attemptHealing(DamageInfo *) {}
	virtual Real estimateDamage(DamageInfoInput &) const { return 0; }
	virtual Real getHealth() const { return m_max; }
	virtual Real getMaxHealth() const { return m_max; }
	virtual BodyDamageType getDamageState() const { return BODY_PRISTINE; }
	virtual void setDamageState(BodyDamageType) {}
	virtual void setAflame(Bool) {}
	virtual void onVeterancyLevelChanged(VeterancyLevel, VeterancyLevel, Bool) {}
	virtual void setArmorSetFlag(ArmorSetType) {}
	virtual void clearArmorSetFlag(ArmorSetType) {}
	virtual Bool testArmorSetFlag(ArmorSetType) { return FALSE; }
	virtual void internalChangeHealth(Real) {}
	Real m_max;
};

Object *g_sdObj = 0;
bool g_sdSlaved = false;
Object *g_sdTree = 0;
GameLogic *g_sdLogic = 0;
std::vector<unsigned char> g_sdSaved;

Object *sdFind(ObjectID id)
{
	if (g_sdObj && id == (ObjectID)g_sdObj->m_oid) return g_sdObj;
	if (g_sdTree && id == (ObjectID)g_sdTree->m_oid) return g_sdTree;
	return 0;
}
void sdDestroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
UpdateModule *sdFindUpdate(const Object *, NameKeyType key)
{
	static char slaved;
	return (g_sdSlaved && key == TheNameKeyGenerator->nameToKey("SlavedUpdate")) ? (UpdateModule *)&slaved : 0;
}

Real sdF(unsigned b) { Real r; memcpy(&r, &b, 4); return r; }
unsigned sdFirstFlag(const ModelConditionFlags &m)
{
	for (int i = 0; i < MODELCONDITION_COUNT; ++i) if (m.test(i)) return (unsigned)i;
	return 0xffffu;
}

}  // namespace

const FXList *FXListStore::findFXList(const char *name) const
{
	for (int i = 0; i < 2; ++i) if (strcmp(name, kSdFxNames[i]) == 0) return &g_sdFx[i];
	return 0;
}
FXListStore *TheFXListStore = &g_sdFxStore;
GameLODManager *TheGameLODManager = &g_sdLod;
const WeaponTemplate *WeaponStore::findWeaponTemplate(const char *name) const { return (const WeaponTemplate *)kWeaponLookup(name); }

void Drawable::setShadowsEnabled(Bool enable) { ulog("shadows %d", (int)enable); }
void Drawable::setTerrainDecalFadeTarget(Real target, Real rate) { ulog("decalfade %08x %08x", realBits(target), realBits(rate)); }
void AIUpdateInterface::markAsDead() { m_dead = true; ulog("markdead %d", (int)m_owner->m_oid); }
void Object::clearAndSetModelConditionFlags(const ModelConditionFlags &clr, const ModelConditionFlags &set)
{
	if (m_drawable) m_drawable->clearAndSetModelConditionState((ModelConditionFlagType)sdFirstFlag(clr), (ModelConditionFlagType)sdFirstFlag(set));
}
void Object::clearModelConditionFlags(const ModelConditionFlags &clr) { if (m_drawable) m_drawable->clearModelConditionState((ModelConditionFlagType)sdFirstFlag(clr)); }
void Object::setModelConditionFlags(const ModelConditionFlags &set) { if (m_drawable) m_drawable->setModelConditionState((ModelConditionFlagType)sdFirstFlag(set)); }

/* the physics behaviour's calls that are not inline in PhysicsUpdate.h */
void PhysicsBehavior::applyForce(const Coord3D *force) { ulog("pforce %d %08x %08x %08x", (int)g_sdObj->m_oid, realBits(force->x), realBits(force->y), realBits(force->z)); }
void PhysicsBehavior::setAngles(Real yaw, Real pitch, Real roll) { ulog("pangles %d %08x %08x %08x", (int)g_sdObj->m_oid, realBits(yaw), realBits(pitch), realBits(roll)); }
void l9sdSlaverDie(UpdateModule *) { ulog("slaverdie %d", (int)g_sdObj->m_oid); }

extern "C" {

/* Read `filename` (block `D`) as the data of a SlowDeathBehavior (module 0) and make `cfg[3]` of them on an Object (id 1). `cfg`: [0 the frame, 1 the slow death
** scale (float bits), 2 the hulk lifetime override, 3 the number of modules (1..3), 4 the kinds (bit 0 infantry, 1 hulk), 5 flags (bit 0 drawable, 1 AI, 2 physics,
** 3 slaved update, 4 body, 5 disabled held, 6 the AI is dead, 7 the tree object 2 exists), 6 the veterancy, 7 the maximum health (float bits), 8..10 the position
** (float bits), 11 the physics' last collidee, 12 the status bits, 13..34 the data of modules 1 and 2 (11 words each: the probability modifier, the overkill
** bonus, the sink rate, the sink delay, its variance, the destruction delay, its variance, the fling force, its variance, the fling pitch, its variance),
** 35 the logic random seed, 36 the number of damage infos, then 3 words each: the death type, the damage dealt and the damage clipped (float bits)]. Then `n` ops of 5 words [op, a, b, c,
** d]: 0 update(module a)  1 onDie(info a) of every module in order, as Object::onDie  2 onDie(info b) of module a  3 beginSlowDeath(info b) of module a  4
** getProbabilityModifier(info b) of module a  5 isDieApplicable(info b) of module a  6 the frame = a  7 the slow death scale = a (float bits)  8 the hulk
** override = a  9 the position = (a, b, c)  10 (nothing)  11 disabled held = a  12 the last collidee = a  13 the AI is dead = a  14 the drawable is there = a
** 15 the veterancy = a  16 the maximum health = a  17 the status bits = a  18 xfer save of module a  19 xfer load of the last save into module a  20 xfer CRC of
** module a. Each op appends (the first, the construction, only the last four items): the CATCH_ALL status, the result word, the sleep time, the effect log
** (count, then each as a string), the number of wake calls, the index of the module of the last and its frame, the state of every module (the sink, midpoint
** and destruction frames, the time scale, the flags), the xfer status, bytes and CRC, the object's position, the disabled-held bit, the AI's dead flag, the
** physics' fall and bounce flags and its extra bounciness and friction. */
static int l9sdRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_sdSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9sdRead(filename); }
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
	static unsigned char physMem[sizeof(PhysicsBehavior) + 16];
	memset(physMem, 0, sizeof(physMem));
	PhysicsBehavior *phys = (PhysicsBehavior *)physMem;
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
		try { mods[k] = new SlowDeathBehavior(obj, datas[k]); }
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
				case 18: xst = runXfer(mods[k], 0, xfer_path, bytes, 512, &blen, &crc); g_sdSaved.assign(bytes, bytes + blen); break;
				case 19: {
					std::vector<unsigned char> b = g_sdSaved;
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
			else if (e.kind == 1 && e.p) { char b[32]; sprintf(b, "fx %s", e.p == &g_sdFx[0] ? "FX_A" : "FX_B"); log.push_back(b); }
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
	}
	*len = finish(buf, cap);
	for (int j = 0; j < nMods; ++j) delete mods[j];
	for (int j = 1; j < nMods; ++j) delete datas[j];
	delete body;
	delete d;
	delete obj;
	delete tree;
	g_sdObj = g_sdTree = 0;
	TheGameLogic = 0; g_modFindHook = 0; g_modDestroyHook = 0; g_findUpdateHook = 0; TheWeaponStore = 0;
	return 0;
}

int ref_l9sd_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9sdRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
