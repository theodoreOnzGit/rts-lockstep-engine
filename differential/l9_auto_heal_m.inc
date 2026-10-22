/* Differential-test driver for AutoHealBehavior (p1w12-sonnet, update batch M; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/AutoHealBehavior.cpp   (#included, unchanged)
** with its header unchanged, and the healing functions of Object (Object.cpp, cut verbatim by build.rs: mod_gen/ObjectHealing.inc), in
** wrap_module.cpp's unit on the stand-ins of differential/shim_module: four objects (a player, kinds, an effectively-dead flag, an off-map flag, a body
** with a health and a maximum that logs the healing it is given, a position and a geometry height), three players (a list of objects for
** Player::iterateObjects), a partition manager whose range scan is the list the test gives, a particle system manager that logs, the world animation
** log of crate batch F, the game logic whose frame the test sets and whose wake recorder counts the calls setWakeFrame makes, and the REAL
** UpdateModule, UpgradeMux, UpgradeCenter, INI, Xfer and (RandomValue.cpp) logic random values. */
#include <strings.h>
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/AutoHealBehavior.cpp"

namespace {

std::vector<std::string> g_ahPsNames;
std::map<std::string, ParticleSystemTemplate *> g_ahPsTemplates;
ParticleSystem g_ahPsSlots[64];
int g_ahPsNext = 0;
ParticleSystemID g_ahPsLastId = 0;
ParticleSystemManager g_ahPsm;
PartitionManager g_ahPm;
std::vector<Object *> g_ahRange;

ModuleData *l9ahRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = AutoHealBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_ahSaved;

const char *ahPsName(const ParticleSystemTemplate *t)
{
	return t ? g_ahPsNames[(size_t)t->m_idx].c_str() : "";
}

}  // namespace

#include "ObjectHealing.inc"

/* update batch M: the particle system manager. findTemplate answers a distinct template for every name but "None" (as the real lookup answers NULL
** for the name of no template; INI::parseParticleSystemTemplate, in the INI archive, calls it). */
ParticleSystemTemplate *ParticleSystemManager::findTemplate( const AsciiString &name ) const
{
	if (strcasecmp(name.str(), "None") == 0) return 0;
	std::map<std::string, ParticleSystemTemplate *>::iterator it = g_ahPsTemplates.find(name.str());
	if (it == g_ahPsTemplates.end()) {
		ParticleSystemTemplate *t = new ParticleSystemTemplate;
		t->m_idx = (int)g_ahPsNames.size();
		g_ahPsNames.push_back(name.str());
		it = g_ahPsTemplates.insert(std::make_pair(std::string(name.str()), t)).first;
	}
	return it->second;
}
ParticleSystem *ParticleSystemManager::createParticleSystem( const ParticleSystemTemplate *sysTemplate, Bool createSlaves )
{
	ParticleSystem *s = &g_ahPsSlots[g_ahPsNext++ % 64];
	s->m_id = ++g_ahPsLastId;
	s->m_tmpl = sysTemplate->m_idx;
	return s;
}
void ParticleSystemManager::destroyParticleSystemByID( ParticleSystemID id ) {}
void ParticleSystem::setPosition( const Coord3D *pos )
{
	ulog("psys %s %u %08x,%08x,%08x", g_ahPsNames[(size_t)m_tmpl].c_str(), m_id, realBits(pos->x), realBits(pos->y), realBits(pos->z));
}
void Player::iterateObjects( void (*func)( Object *, void * ), void *userData )
{
	for (size_t i = 0; i < m_objects.size(); ++i) func(m_objects[i], userData);
}
/* --- p1w13-sonnet --- (batch P: ActiveBody logs its retaliation scan; null for AutoHealBehavior's) */
void (*g_ahRangeHook)(const Coord3D *, Real, PartitionFilter **) = 0;
/* --- end p1w13-sonnet --- */
SimpleObjectIterator *PartitionManager::iterateObjectsInRange( const Coord3D *pos, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters, IterOrderType order )
{
	if (g_ahRangeHook) g_ahRangeHook(pos, maxDist, filters);
	SimpleObjectIterator *it = new SimpleObjectIterator;
	it->m_list = g_ahRange;
	return it;
}

/* INI::parseParticleSystemTemplate (INI.cpp 1275-1287), on this unit's manager; AutoHealBehavior.h's field table names this function (build.rs) */
void l9ahParsePsTemplate( INI *ini, void *instance, void *store, const void *userData )
{
	const char *token = ini->getNextToken();
	const ParticleSystemTemplate *pSystemT = TheParticleSystemManager->findTemplate( AsciiString( token ) );
	typedef const ParticleSystemTemplate* ConstParticleSystemTemplatePtr;
	ConstParticleSystemTemplatePtr* theParticleSystemTemplate = (ConstParticleSystemTemplatePtr*)store;
	*theParticleSystemTemplate = pSystemT;
}
ParticleSystemManager *TheParticleSystemManager = 0;

extern "C" {

/* Read `filename` (block `D`) as the data of an AutoHealBehavior and make the module on object 1 of four. `cfg` (28 words): [0..4) the objects'
** controlling players + 1 (0 none), [4..8) their kinds (bit i of a word: the i-th of STRUCTURE, INFANTRY, VEHICLE, AIRCRAFT, SELECTABLE, CAN_ATTACK,
** SCORE, HUGE_VEHICLE), [8..12) their health (float bits), [12..16) their maximum health, [16..20) flags (1 has a body, 2 effectively dead, 4 off
** map), [20..24) their geometry height (float bits), [24] the game logic's frame, [25] the logic random seed, [26..28) unused. The objects are at
** (10 k, -3 k, z = 0.5 k) for k = 0..3. The data dump goes first (the mux, the fields). Then the constructor's effects (count, each a string) and wake calls.
** Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 onDamage  2 stopHealing  3 undoUpgrade  4 object a: field b = c (0 player + 1, 1 dead, 2 off
** map, 3 health bits, 4 maximum bits, 5 z bits, 6 height bits, 7 has body)  5 the range list = a (four nibbles of object numbers 1..4, lowest first, 0
** ends)  6 player a's object list = b (nibbles)  7 the animation globals: a & 1 collection present, a & 2 draw-icon UI, a & 4 the name is "ANIM_HEAL",
** b / c the time and z-rise (float bits)  8 attemptUpgrade(m)  9 wouldUpgrade(m)  10 resetUpgrade(m)  11 isAlreadyUpgraded  12 forceRefreshUpgrade
** (m = a..d as 128 bits)  13 the frame = a  14 xfer save  15 xfer load of the last save  16 xfer CRC. Each op appends: the CATCH_ALL status, the
** result (the sleep time update() returned, or the Bool of the upgrade calls), isAlreadyUpgraded(), the effect log (count, then each as a string:
** "heal <object> <amount bits> <source>", "psys <name> <id> <pos bits>", "findanim <name>", "anim <pos bits> <options> <time> <rise>", "fx<id>",
** "remove <name>"), the number of wake calls and the frame of the last, the xfer status, bytes and CRC, the state words (particle system id, soonest
** heal frame, stopped), and for each object its health bits and its sole healing benefactor (id, expiry). */
static int l9ahRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_ahSaved.clear();
	g_ahPsNames.clear();
	g_ahPsTemplates.clear();
	g_ahPsNext = 0;
	g_ahPsLastId = 0;
	g_ahRange.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9ahRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	static const KindOfType kinds[8] = { KINDOF_STRUCTURE, KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_AIRCRAFT, KINDOF_SELECTABLE, KINDOF_CAN_ATTACK, KINDOF_SCORE, KINDOF_HUGE_VEHICLE };
	{
		const AutoHealBehaviorModuleData *ad = (const AutoHealBehaviorModuleData *)d;
		putMux(ad->m_upgradeMuxData);
		putU32(ad->m_initiallyActive ? 1 : 0);
		putU32(ad->m_singleBurst ? 1 : 0);
		putI32(ad->m_healingAmount);
		putU32(ad->m_healingDelay);
		putU32(ad->m_startHealingDelay);
		putU32(realBits(ad->m_radius));
		putU32(ad->m_affectsWholePlayer ? 1 : 0);
		putU32(ad->m_skipSelfForHealing ? 1 : 0);
		std::string bits;
		for (int k = 0; k < KINDOF_COUNT; ++k) if (ad->m_kindOf.test((KindOfType)k)) { char b[16]; sprintf(b, "%d,", k); bits += b; }
		putStr(bits.c_str(), (int)bits.size());
		bits.clear();
		for (int k = 0; k < KINDOF_COUNT; ++k) if (ad->m_forbiddenKindOf.test((KindOfType)k)) { char b[16]; sprintf(b, "%d,", k); bits += b; }
		putStr(bits.c_str(), (int)bits.size());
		const char *n1 = ahPsName(ad->m_radiusParticleSystemTmpl);
		putStr(n1, (int)strlen(n1));
		const char *n2 = ahPsName(ad->m_unitHealPulseParticleSystemTmpl);
		putStr(n2, (int)strlen(n2));
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_objects.clear();
	}
	Object *objs[4];
	MBody bodies[4];
	for (int k = 0; k < 4; ++k) {
		objs[k] = newObject();
		objs[k]->m_oid = k + 1;
		objs[k]->m_player = cfg[k] ? &players[cfg[k] - 1] : 0;
		for (int i = 0; i < 8; ++i) if (cfg[4 + k] >> i & 1) objs[k]->m_kindOf.set(kinds[i]);
		bodies[k].m_owner = objs[k];
		memcpy(&bodies[k].m_health, &cfg[8 + k], 4);
		memcpy(&bodies[k].m_max, &cfg[12 + k], 4);
		objs[k]->m_body = (cfg[16 + k] & 1) ? &bodies[k] : 0;
		objs[k]->m_effDead = (cfg[16 + k] & 2) != 0;
		objs[k]->m_offMap = (cfg[16 + k] & 4) != 0;
		{ Real h; memcpy(&h, &cfg[20 + k], 4); objs[k]->m_geometry.set(GEOMETRY_BOX, false, h, 1.0f, 1.0f); }
		objs[k]->m_pos.x = objs[k]->m_pos.y = objs[k]->m_pos.z = 0.0f;   /* the objects are moved after the module is made, as the Rust test does */
	}
	GameLogic gl;
	gl.m_frame = cfg[24];
	TheGameLogic = &gl;
	TheParticleSystemManager = &g_ahPsm;
	ThePartitionManager = &g_ahPm;
	TheInGameUI = &g_cfUi;
	TheAnim2DCollection = &g_cfAnims;
	TheGlobalData->m_getHealedAnimationName.clear();
	TheGlobalData->m_getHealedAnimationDisplayTimeInSeconds = 0.0f;
	TheGlobalData->m_getHealedAnimationZRisePerSecond = 0.0f;
	InitGameLogicRandom(cfg[25]);
	g_events.clear();
	g_ulog.clear();
	AutoHealBehavior *m = 0;
	status = 0;
	try { m = new AutoHealBehavior(objs[0], d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0; TheParticleSystemManager = 0; ThePartitionManager = 0; TheInGameUI = 0; TheAnim2DCollection = 0;
		return 5;
	}
	for (int k = 0; k < 4; ++k) { objs[k]->m_pos.x = 10.0f * k; objs[k]->m_pos.y = -3.0f * k; objs[k]->m_pos.z = 0.5f * k; }
	for (int phase = 0; phase < 1 + n; ++phase) {
		const unsigned *o = phase ? ops + 5 * (phase - 1) : 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		if (phase) {
			g_events.clear();
			g_ulog.clear();
			gl.m_awakenCalls = 0;
			gl.m_awakenFrame = 0;
			UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
			try {
				switch (o[0]) {
				case 0: res = (unsigned)m->update(); break;
				case 1: { DamageInfo info; m->onDamage(&info); break; }
				case 2: m->stopHealing(); break;
				case 3: m->undoUpgrade(); break;
				case 4: {
					Object *t = objs[o[1] & 3];
					unsigned v = o[3];
					switch (o[2]) {
					case 0: t->m_player = v ? &players[v - 1] : 0; break;
					case 1: t->m_effDead = v != 0; break;
					case 2: t->m_offMap = v != 0; break;
					case 3: memcpy(&bodies[o[1] & 3].m_health, &v, 4); break;
					case 4: memcpy(&bodies[o[1] & 3].m_max, &v, 4); break;
					case 5: memcpy(&t->m_pos.z, &v, 4); break;
					case 6: { Real h; memcpy(&h, &v, 4); t->m_geometry.set(GEOMETRY_BOX, false, h, 1.0f, 1.0f); break; }
					default: t->m_body = v ? &bodies[o[1] & 3] : 0; break;
					}
					break;
				}
				case 5:
					g_ahRange.clear();
					for (int k = 0; k < 4; ++k) { unsigned id = o[1] >> (4 * k) & 15; if (id == 0) break; if (id <= 4) g_ahRange.push_back(objs[id - 1]); }
					break;
				case 6:
					players[o[1] % 3].m_objects.clear();
					for (int k = 0; k < 4; ++k) { unsigned id = o[2] >> (4 * k) & 15; if (id == 0) break; if (id <= 4) players[o[1] % 3].m_objects.push_back(objs[id - 1]); }
					break;
				case 7:
					TheAnim2DCollection = (o[1] & 1) ? &g_cfAnims : 0;
					gl.m_drawIconUI = (o[1] & 2) != 0;
					if (o[1] & 4) TheGlobalData->m_getHealedAnimationName = "ANIM_HEAL"; else TheGlobalData->m_getHealedAnimationName.clear();
					memcpy(&TheGlobalData->m_getHealedAnimationDisplayTimeInSeconds, &o[2], 4);
					memcpy(&TheGlobalData->m_getHealedAnimationZRisePerSecond, &o[3], 4);
					break;
				case 8: res = m->attemptUpgrade(mask) ? 1 : 0; break;
				case 9: res = m->wouldUpgrade(mask) ? 1 : 0; break;
				case 10: res = m->resetUpgrade(mask) ? 1 : 0; break;
				case 11: res = m->isAlreadyUpgraded() ? 1 : 0; break;
				case 12: m->forceRefreshUpgrade(); break;
				case 13: gl.m_frame = o[1]; break;
				case 14: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_ahSaved.assign(bytes, bytes + blen); break;
				case 15: {
					std::vector<unsigned char> b = g_ahSaved;
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
		}
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		if (phase) {
			putI32(xst);
			putU32((unsigned)blen);
			for (int j = 0; j < blen; ++j) putU32(bytes[j]);
			putU32(crc);
			putU32(m->m_radiusParticleSystemID);
			putU32(m->m_soonestHealFrame);
			putU32(m->m_stopped ? 1 : 0);
			for (int k = 0; k < 4; ++k) {
				putU32(objs[k]->m_body ? realBits(bodies[k].m_health) : 0);
				putU32((unsigned)objs[k]->m_soleHealingBenefactorID);
				putU32(objs[k]->m_soleHealingBenefactorExpirationFrame);
			}
		}
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) delete objs[k];
	TheGameLogic = 0;
	TheParticleSystemManager = 0;
	ThePartitionManager = 0;
	TheInGameUI = 0;
	TheAnim2DCollection = 0;
	return 0;
}

int ref_l9ah_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9ahRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
