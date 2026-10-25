/* Differential-test driver for HelicopterSlowDeathBehavior (p1w13-sonnet, batch R48; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HelicopterSlowDeathUpdate.cpp   (#included through mod_gen/HelicopterSlowDeathUpdate_edit.cpp:
**   one mechanical edit made by build.rs, the particle system parser this unit's, as for AutoHealBehavior)
** on SlowDeathBehavior.cpp (batch Q's, in the unit) and batch Q's scene (differential/l9_slow_death_q.inc) with batch R46's additions
** (l9_jet_death_r46.inc: the locomotor's setMaxLift, the physics' setRollRate, the gravity, the terrain's layers and layer heights, the audio
** manager), module 0 the helicopter (its data read from the INI) and up to two SlowDeathBehaviors from the scene, plus: the locomotor's
** setMaxBraking (a line), the drawable's stopAmbientSound (a line) and convertBonePosToWorldPos (through the object's transform), the art's
** bones (BladeBone, PartBone: batch R30's hook), the object's transform (kept: setTransformMatrix keeps the position and the angle, as
** Thing's does; getTransformMatrix takes the position, which the base's fling moves), the particle system manager (batch M's), the
** thing factory (the template "Rubble"; objects 2 or 3, ...), the object's controlling player (EjectPilotDie::ejectPilot, the REAL one of
** die batch D), and the terrain's highest layer (the HeightDie driver's). */
#ifndef DOUBLE_TO_REAL
#define DOUBLE_TO_REAL(x) ((Real) (x))   /* BaseType.h line 238, which this build's shim of the header leaves out */
#endif
#include "HelicopterSlowDeathUpdate_edit.cpp"   /* (mod_gen: INI::parseParticleSystemTemplate is this unit's l9ahParsePsTemplate, build.rs) */

void Locomotor::setMaxBraking(Real braking) { ulog("maxbraking %d %08x", (int)((AIUpdateInterface *)(void *)this)->m_owner->m_oid, realBits(braking)); }
void Drawable::stopAmbientSound() { ulog("stopambient %d", m_ownerObject ? (int)m_ownerObject->m_oid : 0); }
void Drawable::convertBonePosToWorldPos(const Coord3D *bonePos, const Matrix3D *boneTransform, Coord3D *worldPos, Matrix3D *worldTransform) const
{
	/* Drawable.cpp's, through the drawable's transform, which is its object's */
	Vector3 v;
	Matrix3D::Transform_Vector(*m_ownerObject->getTransformMatrix(), Vector3(bonePos->x, bonePos->y, bonePos->z), &v);
	if (worldPos) { worldPos->x = v.X; worldPos->y = v.Y; worldPos->z = v.Z; }
}

namespace {

ModuleData *l9hcRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = HelicopterSlowDeathBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_hcSaved;
/* the art's bones (batch R30's hook) */
Coord3D g_hcBones[2];
unsigned g_hcBoneHas = 0;
const Coord3D *hcBone(const char *name)
{
	if (strcmp(name, "BladeBone") == 0 && (g_hcBoneHas & 1)) return &g_hcBones[0];
	if (strcmp(name, "PartBone") == 0 && (g_hcBoneHas & 2)) return &g_hcBones[1];
	return 0;
}
/* the thing factory: the template "Rubble" makes objects firstId, firstId + 1, ... */
ThingTemplate g_hcRubbleTmpl;
std::vector<Object *> g_hcRubble;
ObjectID g_hcFirstRubble = (ObjectID)3;
const ThingTemplate *hcFindTemplate(const AsciiString &name) { return strcmp(name.str(), "Rubble") == 0 ? &g_hcRubbleTmpl : 0; }
Object *hcNewObject(const ThingTemplate *, Team *)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)((int)g_hcFirstRubble + (int)g_hcRubble.size());
	g_hcRubble.push_back(o);
	return o;
}
/* Thing::setTransformMatrix keeps the position and the angle of the transform; the rubble's is a line */
void hcXform(Object *o, const Matrix3D *mx)
{
	if (o->m_oid == 1) {
		o->m_pos.x = mx->Get_X_Translation();
		o->m_pos.y = mx->Get_Y_Translation();
		o->m_pos.z = mx->Get_Z_Translation();
		o->m_orientation = mx->Get_Z_Rotation();
		return;
	}
	char b[256];
	int n = sprintf(b, "rubble %d", (int)o->m_oid);
	for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) n += sprintf(b + n, " %08x", realBits((*mx)[r][c]));
	ulog("%s", b);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a HelicopterSlowDeathBehavior (module 0) and make it, and cfg[3] - 1 SlowDeathBehaviors, on an
** Object (id 1): batch R46's `ref_l9r46_run` (cfg[61..64]: the gravity, flags (bit 0 the AI's current locomotor), the layers and their
** heights; ops 21..23), with the helicopter's data after the base's in the dump, plus cfg[65] the controlling player (0..2, else none),
** cfg[66] the bones the art has (bit 0 BladeBone, bit 1 PartBone), cfg[67..72] their positions, cfg[73] the object's angle; and op 24 the
** player = a. Each op appends batch Q's outputs, then the helicopter's eight words, the object's transform and angle, and the rubble count. */
static int l9hcRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_hcSaved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	/* (INI::parseParticleSystemTemplate asks the manager) */
	ParticleSystemManager psm;
	TheParticleSystemManager = &psm;
	g_ahPsLastId = 0;
	try { d = l9hcRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { TheParticleSystemManager = 0; return status; }
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
		const HelicopterSlowDeathBehaviorModuleData *hd = (const HelicopterSlowDeathBehaviorModuleData *)d;
		const Real reals[14] = { hd->m_spiralOrbitTurnRate, hd->m_spiralOrbitForwardSpeed, hd->m_spiralOrbitForwardSpeedDamping, hd->m_minSelfSpin,
			hd->m_maxSelfSpin, hd->m_selfSpinUpdateDelay, hd->m_selfSpinUpdateAmount, hd->m_fallHowFast, hd->m_minBladeFlyOffDelay,
			hd->m_maxBladeFlyOffDelay, hd->m_attachParticleLoc.x, hd->m_attachParticleLoc.y, hd->m_attachParticleLoc.z, hd->m_maxBraking };
		for (int k = 0; k < 14; ++k) putU32(realBits(reals[k]));
		putU32(realBits(hd->m_delayFromGroundToFinalDeath));
		const char *ps = ahPsName(hd->m_attachParticleSystem);
		putStr(ps, (int)strlen(ps));
		const AsciiString *strs[4] = { &hd->m_attachParticleBone, &hd->m_bladeObjectName, &hd->m_bladeBone, &hd->m_finalRubbleObject };
		for (int k = 0; k < 4; ++k) putStr(strs[k]->str(), (int)strs[k]->getLength());
		putStr(hd->m_deathSound.getEventName().str(), (int)hd->m_deathSound.getEventName().getLength());
		const FXList *fxs[3] = { hd->m_fxBlade, hd->m_fxHitGround, hd->m_fxFinalBlowUp };
		for (int k = 0; k < 3; ++k) putI32(jdFx(fxs[k]));
		const ObjectCreationList *ocls[4] = { hd->m_oclEjectPilot, hd->m_oclBlade, hd->m_oclHitGround, hd->m_oclFinalBlowUp };
		for (int k = 0; k < 4; ++k) putI32(jdOcl(ocls[k]));
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
	/* the helicopter's scene: its player, the art's bones, the factory, the particle systems, its transform */
	Player players[3];
	for (int p = 0; p < 3; ++p) players[p].m_idx = p;
	obj->m_player = cfg[65] < 3 ? &players[cfg[65]] : 0;
	g_hcBoneHas = cfg[66];
	for (int b = 0; b < 2; ++b) { g_hcBones[b].x = sdF(cfg[67 + 3 * b]); g_hcBones[b].y = sdF(cfg[68 + 3 * b]); g_hcBones[b].z = sdF(cfg[69 + 3 * b]); }
	g_r30BoneHook = hcBone;
	ThingFactory factory;
	TheThingFactory = &factory;
	g_findTemplateHook = hcFindTemplate;
	g_newObjectHook = hcNewObject;
	g_hcRubble.clear();
	g_hcFirstRubble = (ObjectID)(tree ? 3 : 2);
	g_xformHook = hcXform;
	g_objXformSyncPos = true;
	g_cpDrawable.m_ownerObject = obj;
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
	obj->m_orientation = sdF(cfg[73]);
	obj->poseMatrix();
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
		try { mods[k] = k == 0 ? (SlowDeathBehavior *)new HelicopterSlowDeathBehavior(obj, datas[k]) : new SlowDeathBehavior(obj, datas[k]); }
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
				case 24: obj->m_player = o[1] < 3 ? &players[o[1]] : 0; break;
				case 10: break;   /* (the Rust side's kinds are its template's: no op) */
				case 11: obj->m_disabledBits = o[1] ? (1u << DISABLED_HELD) : 0; break;
				case 12: phys->m_lastCollidee = (ObjectID)o[1]; break;
				case 13: ai.m_dead = o[1] != 0; break;
				case 14: obj->m_drawable = o[1] ? &g_cpDrawable : 0; break;
				case 15: obj->m_level = (VeterancyLevel)o[1]; break;
				case 16: body->m_max = sdF(o[1]); break;
				case 17: obj->m_status.clear(); for (int b = 0; b < 8; ++b) if (o[1] >> b & 1) obj->m_status.set((ObjectStatusTypes)b); break;
				case 18: xst = runXfer(mods[k], 0, xfer_path, bytes, 512, &blen, &crc); g_hcSaved.assign(bytes, bytes + blen); break;
				case 21: jetFlags = o[1]; ai.m_hasLoco = (jetFlags & 1) != 0; if (jetFlags & 2) obj->m_status.set(OBJECT_STATUS_DECK_HEIGHT_OFFSET); else obj->m_status.set(OBJECT_STATUS_DECK_HEIGHT_OFFSET, 0); break;
				case 22: layers = o[1]; jetScene(); break;
				case 23: layerHeights = o[1]; jetScene(); break;
				case 19: {
					std::vector<unsigned char> b = g_hcSaved;
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
			if (e.kind == 100) {
				/* (the helicopter's own transform is dumped: its line is not) */
				if (g_ulog[(size_t)e.p] != "settransform 1") log.push_back(g_ulog[(size_t)e.p]);
			}
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
		HelicopterSlowDeathBehavior *hc = (HelicopterSlowDeathBehavior *)mods[0];
		putI32(hc->m_orbitDirection);
		putU32(realBits(hc->m_forwardAngle));
		putU32(realBits(hc->m_forwardSpeed));
		putU32(realBits(hc->m_selfSpin));
		putU32(hc->m_selfSpinTowardsMax ? 1 : 0);
		putU32(hc->m_lastSelfSpinUpdateFrame);
		putU32(hc->m_bladeFlyOffFrame);
		putU32(hc->m_hitGroundFrame);
		const Matrix3D &mx = *obj->getTransformMatrix();
		for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) putU32(realBits(mx[r][c]));
		putU32(realBits(obj->m_orientation));
		putU32((unsigned)g_hcRubble.size());
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
	for (size_t j = 0; j < g_hcRubble.size(); ++j) delete g_hcRubble[j];
	g_hcRubble.clear();
	g_r30BoneHook = 0;
	TheThingFactory = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_xformHook = 0;
	g_objXformSyncPos = false;
	TheParticleSystemManager = 0;
	g_cpDrawable.m_ownerObject = 0;
	TheGlobalData = 0;
	TheAudio = 0;
	TheTerrainLogic = 0;
	TheGameLogic = 0; g_modFindHook = 0; g_modDestroyHook = 0; g_findUpdateHook = 0; TheWeaponStore = 0;
	return 0;
}

int ref_l9r48_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9hcRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
