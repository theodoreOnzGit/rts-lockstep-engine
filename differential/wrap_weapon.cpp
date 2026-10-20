/* Differential-test wrapper for WeaponTemplate and WeaponStore (upstream commit 0a05454d), p1w7-sonnet slice A:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp
** with the regions that need an Object, the partition manager, the drawable or the damage application cut out by
** build.rs (OUT_DIR/weapon_gen/Weapon_cut.cpp; every cut asserted to occur once, upstream never edited):
** estimateWeaponTemplateDamage, shouldProjectileCollideWith, trimOldHistoricDamage, dealDamageInternal and
** fireWeaponTemplate; handleProjectileDetonation and createAndFireTempWeapon; WeaponStore::update; the class
** Weapon from its constructor to loadPostProcess. What is kept: the field parsers and the field table, the
** template's constructor, reset, parsers, postProcessLoad and getters, the store (without update), the block
** handler, and WeaponBonus/WeaponBonusSet. The real headers Weapon.h, INI.h and the rest, unchanged.
**
** INI: the blocks are read by the REAL INI::load (upstream_ini), through the hooks of wrap_ini.cpp: the `Weapon`
** block reaches WeaponStore::parseWeaponTemplateDefinition through ref_weapon_block_hook, and INI::parseWeaponTemplate
** (HistoricBonusWeapon) reaches WeaponStore::findWeaponTemplate through ref_weapon_lookup_hook. The classes of this
** unit are renamed below so that no symbol of this archive clashes with another fixture's (GlobalData's copy of the
** WeaponBonus code, ThingTemplate's WeaponStore stand-in, ...). The FXLists and particle systems are never resolved by
** the Rust: the stand-in stores answer a distinct pointer for every name but "None" (any case), and the dump maps the
** pointers back to the names. The thing factory and the OCL store answer for the names that start with "Proj" and
** "OCL_" (an id computed from the name); the audio event is its name.
**
** Entry points (extern "C", tests/differential_weapon.rs): ref_weapon_reset, ref_weapon_dump, ref_weapon_find,
** ref_weapon_post_process, ref_weapon_getters, ref_weapon_delayed, ref_weapon_store_reset, ref_weapon_historic. */
/* the name lists Weapon.h defines under these (Weapon.cpp defines them before it includes Weapon.h; here Weapon.h comes first) */
#define DEFINE_DEATH_NAMES
#define DEFINE_WEAPONBONUSCONDITION_NAMES
#define DEFINE_WEAPONBONUSFIELD_NAMES
#define DEFINE_WEAPONCOLLIDEMASK_NAMES
#define DEFINE_WEAPONAFFECTSMASK_NAMES
#define DEFINE_WEAPONRELOAD_NAMES
#define DEFINE_WEAPONPREFIRE_NAMES
#define WeaponBonus WpnTestWeaponBonus
#define WeaponBonusSet WpnTestWeaponBonusSet
#define WeaponTemplate WpnTestWeaponTemplate
#define WeaponStore WpnTestWeaponStore
#define Weapon WpnTestWeapon
#define TheWeaponStore WpnTestTheWeaponStore
#define HistoricWeaponDamageInfo WpnTestHistoricWeaponDamageInfo
#define HistoricWeaponDamageList WpnTestHistoricWeaponDamageList
#define ThingFactory WpnTestThingFactory
#define TheThingFactory WpnTestTheThingFactory
#define ObjectCreationListStore WpnTestOclStore
#define TheObjectCreationListStore WpnTestTheOclStore
/* slice B (p1w8-sonnet): the firing half of Weapon.cpp and all of WeaponSet.cpp, on the stand-ins of
** shim_weapon/fire_standins.h (every class and global of which is renamed here) */
#define WeaponSet WpnTestWeaponSet
#define WeaponTemplateSet WpnTestWeaponTemplateSet
#define WeaponTemplateSetVector WpnTestWeaponTemplateSetVector
#define WeaponTemplateSetFinder WpnTestWeaponTemplateSetFinder
#define Object WpnTestObject
#define Drawable WpnTestDrawable
#define Player WpnTestPlayer
#define PlayerList WpnTestPlayerList
#define ThePlayerList WpnTestThePlayerList
#define Team WpnTestTeam
#define ThingTemplate WpnTestThingTemplate
#define BehaviorModule WpnTestBehaviorModule
#define ContainModuleInterface WpnTestContainModuleInterface
#define AIUpdateInterface WpnTestAIUpdateInterface
#define StealthUpdate WpnTestStealthUpdate
#define SpawnBehaviorInterface WpnTestSpawnBehaviorInterface
#define ProjectileUpdateInterface WpnTestProjectileUpdateInterface
#define LandMineInterface WpnTestLandMineInterface
#define ParkingPlaceBehaviorInterface WpnTestParkingPlaceBehaviorInterface
#define SpecialPowerCompletionDie WpnTestSpecialPowerCompletionDie
#define UpdateModule WpnTestUpdateModule
#define ProjectileStreamUpdate WpnTestProjectileStreamUpdate
#define AssistedTargetingUpdate WpnTestAssistedTargetingUpdate
#define LaserUpdate WpnTestLaserUpdate
#define PhysicsBehavior WpnTestPhysicsBehavior
#define ExperienceTracker WpnTestExperienceTracker
#define AssaultTransportAIInterface WpnTestAssaultTransportAIInterface
#define AcademyStats WpnTestAcademyStats
#define FiringTracker WpnTestFiringTracker
#define UpgradeTemplate WpnTestUpgradeTemplate
#define CommandButton WpnTestCommandButton
#define CommandSet WpnTestCommandSet
#define ControlBar WpnTestControlBar
#define TheControlBar WpnTestTheControlBar
#define MAX_COMMANDS_PER_SET WpnTestMAX_COMMANDS_PER_SET
#define FXList WpnTestFXList
#define ObjectCreationList WpnTestObjectCreationList
#define SimpleObjectIterator WpnTestSimpleObjectIterator
#define ObjectIterator WpnTestObjectIterator
#define PartitionManager WpnTestPartitionManager
#define ThePartitionManager WpnTestThePartitionManager
#define TerrainLogic WpnTestTerrainLogic
#define TheTerrainLogic WpnTestTheTerrainLogic
#define Pathfinder WpnTestPathfinder
#define AIManager WpnTestAIManager
#define TheAI WpnTestTheAI
#define GameLogic WpnTestGameLogic
#define TheGameLogic WpnTestTheGameLogic
#define GlobalData WpnTestGlobalData
#define TheGlobalData WpnTestTheGlobalData
#define TBridgeAttackInfo WpnTestTBridgeAttackInfo
#define WpnWorldCfg WpnTestWorldCfg
#define WpnPlayerCfg WpnTestPlayerCfg

#include <limits.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <strings.h>
#include <string>
#include <map>
#include <vector>
#include <list>
#include <set>
#include <algorithm>
#include "Common/NameKeyGenerator.h"
#include "Common/INI.h"
#include "Common/Xfer.h"
/* REAL_TO_INT_FLOOR of the real BaseType.h is `fast_float2long_round(fast_float_floor(x))`, MSVC `__asm` upstream. The
** shared shim's BaseType.h does not have it; the hand transcription of the asm is in wrap_base_type.cpp (verified
** against the Rust by differential_base_type.rs), and the macro is composed here exactly as upstream composes it. */
extern "C" { int ref_fast_float2long_round(float f); float ref_fast_float_floor(float f); float ref_fast_float_ceil(float f); }
#define REAL_TO_INT_FLOOR(x) (ref_fast_float2long_round(ref_fast_float_floor(x)))
#define REAL_TO_INT_CEIL(x) (ref_fast_float2long_round(ref_fast_float_ceil(x)))
/* `new(ShimZeroFill()) T` (differential/shim/Common/GameMemory.h) parses as a function type under clang; braces do not */
#include "Common/GameMemory.h"
#undef newInstance
#define newInstance(ARGCLASS) new(ShimZeroFill{}) ARGCLASS
/* The GameClient stand-in header of the INI build: the particle-system lookup INI.cpp's parser calls. Its definition
** below replaces the weak abort stub of wrap_ini.cpp. (The FXList lookup is wrap_armor.cpp's, which answers a
** distinct pointer for every name but "None" and gives the name back through ref_armor_fx_name; the audio event's
** setEventName is wrap_upgrade.cpp's, on a class with the same single member as the stand-in below.) */
#include "shim_ini/GameClient/ParticleSys.h"
extern "C" const char *ref_armor_fx_name(const void *fx);

/* the real headers, the stand-ins of the world, the cut Weapon.cpp and WeaponSet.cpp, with their members readable (the dumps
** read them) */
#define private public
#define protected public
#include "Common/ObjectStatusTypes.h"
#include "Common/Geometry.h"
#include "GameLogic/Weapon.h"
#include "GameLogic/WeaponSet.h"
#include "Lib/trig.h"
#include "shim_weapon/fire_standins.h"
/* BaseType.h's min and max are macros upstream; defined here, after the standard headers, and undone after the cut files */
#define min(x,y) (((x)<(y)) ? (x) : (y))
#define max(x,y) (((x)>(y)) ? (x) : (y))
#include "Weapon_cut.cpp"
#include "WeaponSet_cut.cpp"
#include "Object_weapons.cpp"
#undef min
#undef max
#undef private
#undef protected

/* ---- stand-ins */
namespace {
std::map<std::string, void *> g_psys;
std::map<const void *, std::string> g_psysName;
ThingFactory g_thingFactory;
ObjectCreationListStore g_oclStore;
ParticleSystemManager g_psysManager;

unsigned idOf(const char *name)
{
	unsigned sum = 0;
	for (const char *p = name; *p; ++p) sum += (unsigned char)*p;
	return 1 + sum % 1000;
}
}

ThingFactory *TheThingFactory = &g_thingFactory;
ObjectCreationListStore *TheObjectCreationListStore = &g_oclStore;

const ThingTemplate *ThingFactory::findTemplate(const AsciiString &name, Bool)
{
	/* p1w8-sonnet: "T<n>" is the n-th stand-in thing template of the firing test (what WeaponSet::xfer saves and loads) */
	if (name.str()[0] == 'T' && name.str()[1] >= '0' && name.str()[1] <= '9')
	{
		int n = atoi(name.str() + 1);
		return (n >= 1 && n <= (int)g_thingTemplates.size()) ? g_thingTemplates[n - 1] : 0;
	}
	if (strncmp(name.str(), "Proj", 4) != 0) return 0;
	return (const ThingTemplate *)(size_t)idOf(name.str());
}
#define private public
#define protected public
#include "shim_weapon/fire_driver.h"
#undef private
#undef protected
const ObjectCreationList *ObjectCreationListStore::findObjectCreationList(const char *name) const
{
	if (strncmp(name, "OCL_", 4) != 0) return 0;
	return (const ObjectCreationList *)(size_t)idOf(name);
}
ParticleSystemTemplate *ParticleSystemManager::findTemplate(const AsciiString &name) const
{
	if (strcasecmp(name.str(), "None") == 0) return 0;
	std::string s(name.str());
	std::map<std::string, void *>::iterator it = g_psys.find(s);
	if (it == g_psys.end()) {
		void *p = new char[1];
		it = g_psys.insert(std::make_pair(s, p)).first;
		g_psysName[p] = s;
	}
	return (ParticleSystemTemplate *)it->second;
}

/* ---- the dump */
namespace {
std::vector<unsigned char> g_dump;
void put(const void *p, int n) { g_dump.insert(g_dump.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void putU32(unsigned v) { put(&v, 4); }
void putReal(Real r) { put(&r, 4); }
void putStr(const char *s) { unsigned n = (unsigned)strlen(s); putU32(n); put(s, (int)n); }
void putFx(const void *fx, bool particle)
{
	if (!fx) { g_dump.push_back(0); return; }
	g_dump.push_back(1);
	putStr(particle ? g_psysName[fx].c_str() : ref_armor_fx_name(fx));
}
int finish(unsigned char *buf, int cap)
{
	int n = (int)g_dump.size();
	if (n <= cap && n > 0) memcpy(buf, &g_dump[0], n);
	return n;
}
void putBonus(const WeaponBonus &b) { for (int f = 0; f < WeaponBonus::FIELD_COUNT; ++f) putReal(b.getField((WeaponBonus::Field)f)); }

void dumpTemplate(const WeaponTemplate *t)
{
	putStr(t->m_name.str()); putU32((unsigned)t->m_nameKey);
	putStr(t->m_projectileStreamName.str()); putStr(t->m_laserName.str()); putStr(t->m_laserBoneName.str());
	putReal(t->m_primaryDamage); putReal(t->m_primaryDamageRadius); putReal(t->m_secondaryDamage); putReal(t->m_secondaryDamageRadius);
	putReal(t->m_shockWaveAmount); putReal(t->m_shockWaveRadius); putReal(t->m_shockWaveTaperOff);
	putReal(t->m_attackRange); putReal(t->m_minimumAttackRange); putReal(t->m_requestAssistRange); putReal(t->m_aimDelta);
	putReal(t->m_scatterRadius); putReal(t->m_scatterTargetScalar);
	putU32((unsigned)t->m_scatterTargets.size());
	for (size_t i = 0; i < t->m_scatterTargets.size(); ++i) { putReal(t->m_scatterTargets[i].x); putReal(t->m_scatterTargets[i].y); }
	putU32((unsigned)t->m_damageType); putU32((unsigned)t->m_deathType);
	putReal(t->m_weaponSpeed); putReal(t->m_minWeaponSpeed); putU32(t->m_isScaleWeaponSpeed ? 1 : 0);
	putReal(t->m_weaponRecoil); putReal(t->m_minTargetPitch); putReal(t->m_maxTargetPitch); putReal(t->m_radiusDamageAngle);
	putStr(t->m_projectileName.str()); putU32((unsigned)(size_t)t->m_projectileTmpl);
	for (int i = 0; i < LEVEL_COUNT; ++i)
	{
		putStr(t->m_fireOCLNames[i].str()); putStr(t->m_projectileDetonationOCLNames[i].str());
		putFx(t->m_projectileExhausts[i], true);
		putU32((unsigned)(size_t)t->m_fireOCLs[i]); putU32((unsigned)(size_t)t->m_projectileDetonationOCLs[i]);
		putFx(t->m_fireFXs[i], false); putFx(t->m_projectileDetonateFXs[i], false);
	}
	putStr(t->m_fireSound.m_eventName.str()); putU32(t->m_fireSoundLoopTime);
	if (t->m_extraBonus) { g_dump.push_back(1); for (int i = 0; i < WEAPONBONUSCONDITION_COUNT; ++i) putBonus(t->m_extraBonus->m_bonus[i]); }
	else g_dump.push_back(0);
	putU32((unsigned)t->m_clipSize); putU32((unsigned)t->m_clipReloadTime); putU32((unsigned)t->m_minDelayBetweenShots); putU32((unsigned)t->m_maxDelayBetweenShots);
	putU32((unsigned)t->m_continuousFireOneShotsNeeded); putU32((unsigned)t->m_continuousFireTwoShotsNeeded);
	putU32(t->m_continuousFireCoastFrames); putU32(t->m_autoReloadWhenIdleFrames); putU32((unsigned)t->m_shotsPerBarrel);
	putU32((unsigned)t->m_antiMask); putU32((unsigned)t->m_affectsMask); putU32((unsigned)t->m_collideMask);
	putU32(t->m_damageDealtAtSelfPosition ? 1 : 0); putU32((unsigned)t->m_reloadType); putU32((unsigned)t->m_prefireType);
	putU32(t->m_historicBonusTime); putReal(t->m_historicBonusRadius); putU32((unsigned)t->m_historicBonusCount);
	putU32(t->m_historicBonusWeapon ? (unsigned)t->m_historicBonusWeapon->m_nameKey : 0);
	putU32(t->m_leechRangeWeapon ? 1 : 0); putU32(t->m_capableOfFollowingWaypoint ? 1 : 0); putU32(t->m_isShowsAmmoPips ? 1 : 0);
	putU32(t->m_allowAttackGarrisonedBldgs ? 1 : 0); putU32(t->m_playFXWhenStealthed ? 1 : 0);
	putU32((unsigned)t->m_preAttackDelay); putReal(t->m_continueAttackRange); putReal(t->m_infantryInaccuracyDist);
	putU32((unsigned)t->m_damageStatusType); putU32(t->m_suspendFXDelay); putU32(t->m_dieOnDetonate ? 1 : 0);
	putU32((unsigned)t->m_historicDamage.size());
	for (HistoricWeaponDamageList::const_iterator it = t->m_historicDamage.begin(); it != t->m_historicDamage.end(); ++it)
	{ putU32(it->frame); putReal(it->location.x); putReal(it->location.y); putReal(it->location.z); }
}
}  // namespace

static WeaponStore *g_store = 0;
static void weaponBlockHook(INI *ini) { WeaponStore::parseWeaponTemplateDefinition(ini); }
static const WeaponTemplate *weaponLookupHook(const char *name) { return g_store->findWeaponTemplate(AsciiString(name)); }

extern "C" {
extern void (*ref_weapon_block_hook)(INI *);
extern const void *(*ref_weapon_lookup_hook)(const char *);

/* a fresh name-key generator and an empty store; installs the INI hooks */
void ref_weapon_reset(void)
{
	static NameKeyGenerator *gen = 0;
	if (!gen) gen = new NameKeyGenerator;
	gen->reset();
	TheNameKeyGenerator = gen;
	delete g_store;
	TheWeaponStore = g_store = new WeaponStore;
	ref_weapon_block_hook = weaponBlockHook;
	ref_weapon_lookup_hook = (const void *(*)(const char *))weaponLookupHook;
}

unsigned ref_weapon_name_key(const char *name) { return (unsigned)TheNameKeyGenerator->nameToKey(name); }

/* The store: the template count, each template (see dumpTemplate), the delayed damage list. */
int ref_weapon_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	putU32((unsigned)g_store->m_weaponTemplateVector.size());
	for (size_t i = 0; i < g_store->m_weaponTemplateVector.size(); ++i) dumpTemplate(g_store->m_weaponTemplateVector[i]);
	putU32((unsigned)g_store->m_weaponDDI.size());
	for (std::list<WeaponStore::WeaponDelayedDamageInfo>::const_iterator it = g_store->m_weaponDDI.begin(); it != g_store->m_weaponDDI.end(); ++it)
	{
		putU32(it->m_delayedWeapon ? (unsigned)it->m_delayedWeapon->m_nameKey : 0);
		putReal(it->m_delayDamagePos.x); putReal(it->m_delayDamagePos.y); putReal(it->m_delayDamagePos.z);
		putU32(it->m_delayDamageFrame); putU32((unsigned)it->m_delaySourceID); putU32((unsigned)it->m_delayIntendedVictimID);
		putBonus(it->m_bonus);
	}
	return finish(buf, cap);
}

/* findWeaponTemplate(name): the template's index in the store, or -1 (and the name key is assigned) */
int ref_weapon_find(const char *name)
{
	const WeaponTemplate *t = g_store->findWeaponTemplate(AsciiString(name));
	if (!t) return -1;
	for (size_t i = 0; i < g_store->m_weaponTemplateVector.size(); ++i) if (g_store->m_weaponTemplateVector[i] == t) return (int)i;
	return -1;
}

void ref_weapon_post_process(void) { g_store->postProcessLoad(); }
void ref_weapon_store_reset(void) { g_store->reset(); }
void ref_weapon_historic(int index, unsigned frame, float x, float y, float z)
{
	WeaponTemplate *t = g_store->m_weaponTemplateVector[index];
	t->m_historicDamage.push_back(HistoricWeaponDamageInfo(frame, Coord3D()));
	t->m_historicDamage.back().location.x = x; t->m_historicDamage.back().location.y = y; t->m_historicDamage.back().location.z = z;
}

/* setDelayedDamage(op 0, template index, frame, source, victim, x, y, z, the five bonus fields) or
** deleteAllDelayedDamage (op 1) */
void ref_weapon_delayed(int op, int index, unsigned frame, int source, int victim, float x, float y, float z, const float *bonus)
{
	if (op == 1) { g_store->deleteAllDelayedDamage(); return; }
	WeaponBonus b;
	for (int f = 0; f < WeaponBonus::FIELD_COUNT; ++f) b.setField((WeaponBonus::Field)f, bonus[f]);
	Coord3D pos; pos.x = x; pos.y = y; pos.z = z;
	g_store->setDelayedDamage(g_store->m_weaponTemplateVector[index], &pos, frame, (ObjectID)source, (ObjectID)victim, b);
}

/* the getters of template `index` under the bonus: attack range, minimum attack range, unmodified range, delay
** between shots (draws from the logic random numbers when it is a range), clip reload time, pre-attack delay,
** primary and secondary damage and radius, contact weapon, auto-reloads. out has 12 words. */
void ref_weapon_getters(int index, const float *bonus, unsigned *out)
{
	const WeaponTemplate *t = g_store->m_weaponTemplateVector[index];
	WeaponBonus b;
	for (int f = 0; f < WeaponBonus::FIELD_COUNT; ++f) b.setField((WeaponBonus::Field)f, bonus[f]);
	Real r;
	#define BITS(x) do { r = (x); memcpy(&out[n++], &r, 4); } while (0)
	int n = 0;
	BITS(t->getAttackRange(b)); BITS(t->getMinimumAttackRange()); BITS(t->getUnmodifiedAttackRange());
	out[n++] = (unsigned)t->getDelayBetweenShots(b); out[n++] = (unsigned)t->getClipReloadTime(b); out[n++] = (unsigned)t->getPreAttackDelay(b);
	BITS(t->getPrimaryDamage(b)); BITS(t->getPrimaryDamageRadius(b)); BITS(t->getSecondaryDamage(b)); BITS(t->getSecondaryDamageRadius(b));
	out[n++] = t->isContactWeapon() ? 1 : 0; out[n++] = t->getAutoReloadsClip() ? 1 : 0;
	#undef BITS
}

/* The firing differential (tests/differential_weapon_fire.rs): runs a program (the words) against the real Weapon.cpp and
** WeaponSet.cpp on the stand-ins, and writes the trace lines, each ended by '\n'. Returns the size of the text (the text
** is written only if it fits `cap`). tmpPath is a file the XferSave/XferLoad operations use. */
int ref_weapon_fire_run(const unsigned *words, int n, const char *tmpPath, char *out, int cap)
{
	std::vector<std::string> lines;
	g_lines = &lines;
	runProgram(words, n, tmpPath);
	g_lines = 0;
	std::string all;
	for (size_t i = 0; i < lines.size(); ++i) { all += lines[i]; all += '\n'; }
	if ((int)all.size() <= cap) memcpy(out, all.data(), all.size());
	return (int)all.size();
}
}  /* extern "C" */
