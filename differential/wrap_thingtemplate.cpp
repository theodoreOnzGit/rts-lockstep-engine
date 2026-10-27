/* Differential-test wrapper for ThingTemplate and ThingFactory (upstream commit 0a05454d):
**   GameEngine/Source/Common/Thing/ThingTemplate.cpp
**   GameEngine/Source/Common/Thing/ThingFactory.cpp
**   GameEngine/Source/Common/INI/INIObject.cpp
** compiled UNCHANGED as their own translation units (build.rs), with the headers ThingTemplate.h,
** ThingFactory.h, ProductionPrerequisite.h, ModuleFactory.h, Module.h, ArmorSet.h, WeaponSet.h, Armor.h,
** DamageFX.h, Science.h, KindOf.h, Geometry.h, Overridable.h, Powers.h and ThingSort.h unchanged, against
** differential/shim_thingtemplate (stand-ins for the classes of the client and of layers 7 and 8 that they
** name, renamed Tt* by thingtemplate_prelude.h so that no symbol clashes with another fixture's).
**
** Included below, cut out of upstream by build.rs and compiled in THIS translation unit (the files
** themselves do not compile outside the engine):
**   - INI::parseDynamicAudioEventRTS and INI::parseWeaponTemplate of INI.cpp, and AI.h's
**     TheCommandSourceMaskNames (renamed, since the INI build's versions reach its own stand-ins);
**   - WeaponTemplateSet::clear to ::testWeaponSetFlag of WeaponSet.cpp;
**   - the members of ProductionPrerequisite.cpp that need no Player;
**   - every member of ModuleFactory.cpp but init() (which names the 205 module classes).
**
** What is real below the template: the INI reader (INI::load, every generic field parser,
** parseAndTranslateLabel, parseColorInt, parseBitString8, parseAsciiStringVector, scanScience), the
** NameKeyGenerator, the ArmorStore and DamageFXStore (their own `Armor` and `DamageFX` blocks define
** what ArmorSet blocks name), the ScienceStore (`Science` blocks), Module.cpp's ModuleData and the
** SparseMatchFinder. The game text is the Science fixture's stand-in: a label translates to "T:" and its
** bytes, and to nothing if it starts with NOTEXT.
**
** The Object and ObjectReskin blocks reach INIObject.cpp through the hook of wrap_ini.cpp
** (ref_object_block_hook): no other fixture may define INI::parseObjectDefinition again.
**
** The modules. ModuleFactory.cpp's init() registers 205 classes that cannot be compiled here. The wrapper
** registers instead a few TEST modules with the real addModuleInternal: behavior modules (TestUpdate,
** TestUpdate2, TestDie, TestDie2, TestBody, TestBody2, TestMulti, TestUpgrade; the data is a base
** ModuleData, read by the real Module::friend_newModuleData, so the block is just `End`), draw modules
** (TestDraw, TestDraw2) and a client update module (TestClient), whose data is the block of the client
** stand-in below: it records every field line and opens a nested block for the four fields of
** W3DModelDraw that do (the Rust keeps them as data, ThingTemplate's module docs say why). What the real
** classes would do for the 205 names is not tested; the Rust reports a name that is registered but not
** translated as an error. A name no module of this wrapper has is NOT given to the C++: upstream's
** newModuleDataFromINI returns NULL for it and parseModuleName calls isAiModuleData on that.
**
** The AI (batch A1). Two more behavior modules, AIUpdateInterface and TestAI2, have the REAL AIUpdateModuleData as
** their data: its declaration (shim_thingtemplate's AIUpdate.h includes ai_update_decl.inc) and its members from
** AIUpdate.cpp, with TurretAIData's from TurretAI.cpp (ai_update_data.inc), all cut verbatim by build.rs. The
** "Locomotor" field reaches the real parseLocomotorSet, which asks the Locomotor build's real TheLocomotorStore (the
** files' "Locomotor" blocks go to Locomotor.cpp's handler) through ref_loco_find_template; the dump writes each
** locomotor of a set as the key the store holds it under (ref_loco_template_key).
**
** Not compiled: ThingTemplate::calcCostToBuild, calcTimeToBuild and getBuildFacilityTemplate are linked
** (they are in ThingTemplate.cpp) against stand-in Player members that return constants, and are not
** called: they are layer 7 and the Rust does not translate them.
**
** The dumps are in this file: dumpFactory writes the whole template store (the list, the name map, every
** template of every override chain with every field), so that the Rust writes the same bytes. */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <strings.h>
#include <stdexcept>
#include <typeinfo>
#include <cxxabi.h>
#include <string>
#include <map>
#include <vector>
#include <list>
#include <set>
#define private public
#define protected public
#define DEFINE_WEAPONSLOTTYPE_NAMES
#define DEFINE_LOCOMOTORSET_NAMES
#define DEFINE_AUTOACQUIRE_NAMES
#include "Common/INI.h"
#include "Common/INIException.h"
#include "Common/NameKeyGenerator.h"
#include "Common/XferCRC.h"
#include "Common/Xfer.h"
#include "Common/Science.h"
#include "Common/Module.h"
#include "Common/ModuleFactory.h"
#include "Common/ThingTemplate.h"
#include "Common/ThingFactory.h"
#include "Common/ProductionPrerequisite.h"
#include "Common/DamageFX.h"
#include "Common/BitFlagsIO.h"
#include "Common/GlobalData.h"
#include "Common/GameAudio.h"
#include "Common/Player.h"
#include "GameLogic/Armor.h"
#include "GameLogic/WeaponSet.h"
#include "GameLogic/Object.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/PartitionManager.h"
#include "GameLogic/Weapon.h"
#include "GameLogic/Module/AIUpdate.h"
#include "GameClient/GameClient.h"
#include "GameClient/FXList.h"
#include "GameClient/Image.h"
#include "GameClient/GameText.h"
#undef private
#undef protected

/* ---- the stand-ins (the classes are renamed Tt* by the prelude) */

namespace {
GlobalData g_globalData;
AudioManager g_audioManager;
FXListStore g_fxStore;
std::map<std::string, FXList *> g_fxLists;
WeaponStore g_weaponStore;
std::map<NameKeyType, WeaponTemplate *> g_weapons;
GameLogic g_gameLogic;
GameClient g_gameClient;
PartitionManager g_partitionManager;
Object g_object;
Drawable g_drawable;
Handicap g_handicap;
Energy g_energy;
const ThingTemplate *g_lastObjectTemplate = 0;
ObjectStatusMaskType g_lastObjectStatus;
const ThingTemplate *g_lastDrawableTemplate = 0;
int g_lastDrawableStatus = 0;
UnsignedInt g_superweaponRestriction = 0;
}

GlobalData *TheGlobalData = &g_globalData;
AudioManager *TheAudio = &g_audioManager;
FXListStore *TheFXListStore = &g_fxStore;
WeaponStore *TheWeaponStore = &g_weaponStore;
GameLogic *TheGameLogic = &g_gameLogic;
GameClient *TheGameClient = &g_gameClient;
PartitionManager *ThePartitionManager = &g_partitionManager;
ImageCollection *TheMappedImageCollection = 0;
ModuleFactory *TheModuleFactory = 0;

/* FXListStore::findFXList: None is null; any other name calls NAMEKEY (FXList.cpp line 854) and answers
** with an object of its own (the client resolves the list; the Rust keeps the name) */
const FXList *FXListStore::findFXList(const char *name) const
{
	if (stricmp(name, "None") == 0)
		return NULL;
	TheNameKeyGenerator->nameToKey(name);
	std::map<std::string, FXList *>::iterator it = g_fxLists.find(name);
	if (it == g_fxLists.end()) {
		FXList *fx = new FXList;
		fx->m_name = name;
		it = g_fxLists.insert(std::make_pair(std::string(name), fx)).first;
	}
	return it->second;
}

/* WeaponStore::findWeaponTemplate (Weapon.cpp lines 1560-1567) over the weapons the wrapper registers */
const WeaponTemplate *WeaponStore::findWeaponTemplate(AsciiString name) const
{
	if (stricmp(name.str(), "None") == 0)
		return NULL;
	NameKeyType key = TheNameKeyGenerator->nameToKey(name);
	std::map<NameKeyType, WeaponTemplate *>::const_iterator it = g_weapons.find(key);
	return it == g_weapons.end() ? NULL : it->second;
}

const Image *ImageCollection::findImageByName(const AsciiString &) { abort(); }

Object *GameLogic::friend_createObject(const ThingTemplate *thing, const ObjectStatusMaskType &mask, Team *)
{
	g_lastObjectTemplate = thing;
	g_lastObjectStatus = mask;
	g_object.m_modules[0] = 0;
	g_object.m_modules[1] = 0;
	g_object.m_initialised = false;
	return &g_object;
}
UnsignedInt GameLogic::getSuperweaponRestriction() const { return g_superweaponRestriction; }

Drawable *GameClient::friend_createDrawable(const ThingTemplate *thing, DrawableStatus statusBits)
{
	g_lastDrawableTemplate = thing;
	g_lastDrawableStatus = (int)statusBits;
	return &g_drawable;
}

/* Player (layer 7): linked, not called */
Real Handicap::getHandicap(HandicapType, const ThingTemplate *) const { return 1.0f; }
Real Energy::getEnergySupplyRatio() const { return 1.0f; }
Real Player::getProductionCostChangePercent(AsciiString) const { return 0.0f; }
Real Player::getProductionCostChangeBasedOnKindOf(KindOfMaskType) const { return 1.0f; }
Real Player::getProductionTimeChangePercent(AsciiString) const { return 0.0f; }
const Handicap *Player::getHandicap() const { return &g_handicap; }
Energy *Player::getEnergy() const { return &g_energy; }
Bool Player::buildsInstantly() const { return false; }
void Player::countObjectsByThingTemplate(Int numTemplates, const ThingTemplate *const *, Bool, Int *counts) const
{
	for (Int i = 0; i < numTemplates; ++i) counts[i] = 0;
}
Bool Player::hasScience(ScienceType) const { return false; }

/* ProductionPrerequisite::getExistingBuildFacilityTemplate needs a Player (layer 7) */
const ThingTemplate *ProductionPrerequisite::getExistingBuildFacilityTemplate(const Player *) const { return NULL; }

/* TheLocomotorStore (layer 9) is the Locomotor build's, real (Locomotor.cpp, wrap_locomotor.cpp): the "Locomotor"
** blocks of the files reach its real handler, and parseLocomotorSet below asks it for a template by name key
** through ref_loco_find_template. This view is all the AIUpdateModuleData cut names of it. */
extern "C" const void *ref_loco_find_template(unsigned key);
extern "C" unsigned ref_loco_template_key(const void *tmpl);
extern "C" void ref_loco_store_new(void);
class LocomotorStore
{
public:
	const LocomotorTemplate *findLocomotorTemplate(NameKeyType namekey) const
	{
		return (const LocomotorTemplate *)ref_loco_find_template((unsigned)namekey);
	}
};
static LocomotorStore g_locomotorStoreView;
static LocomotorStore *const TheLocomotorStore = &g_locomotorStoreView;

/* ---- upstream text, cut out by build.rs (see the header) */
#include "ini_field_parsers.inc"
#include "weapon_template_set.inc"
#include "production_prerequisite.inc"
#include "ai_update_data.inc"
#include "module_factory_methods.inc"
void ModuleFactory::init(void) {}

/* ---- the test modules */

namespace {

/* the client modules' stand-in data: every field line and the nested blocks of W3DModelDraw */
class ClientModuleData : public ModuleData
{
public:
	struct Line
	{
		int depth;
		std::string field;
		std::vector<std::string> args;
	};
	std::vector<Line> lines;
	int depth;
	ClientModuleData() : depth(0) {}
};

void clientRecord(ClientModuleData *d, INI *ini, const char *field)
{
	ClientModuleData::Line l;
	l.depth = d->depth;
	l.field = field;
	for (const char *t = ini->getNextTokenOrNull(); t; t = ini->getNextTokenOrNull())
		l.args.push_back(t);
	d->lines.push_back(l);
}
void clientLine(INI *ini, void *instance, void *, const void *userData)
{
	clientRecord((ClientModuleData *)instance, ini, (const char *)userData);
}
void clientNested(INI *ini, void *instance, void *store, const void *userData);
const FieldParse g_clientFields[] = {
	{ "DefaultConditionState", clientNested, (void *)"DefaultConditionState", 0 },
	{ "ConditionState", clientNested, (void *)"ConditionState", 0 },
	{ "AliasConditionState", clientNested, (void *)"AliasConditionState", 0 },
	{ "TransitionState", clientNested, (void *)"TransitionState", 0 },
	{ 0, clientLine, NULL, 0 },
	{ 0, 0, 0, 0 }
};
void clientNested(INI *ini, void *instance, void *, const void *userData)
{
	ClientModuleData *d = (ClientModuleData *)instance;
	clientRecord(d, ini, (const char *)userData);
	d->depth++;
	ini->initFromINI(d, g_clientFields);
	d->depth--;
}
ModuleData *clientDataProc(INI *ini)
{
	ClientModuleData *d = new ClientModuleData;
	ini->initFromINI(d, g_clientFields);
	return d;
}
/* the behavior modules' data: a base ModuleData, `End` only */
ModuleData *baseDataProc(INI *ini) { return Module::friend_newModuleData(ini); }
/* batch A1: the AI's data, real (AIUpdateInterface's friend_newModuleData, MAKE_STANDARD_MODULE_DATA_MACRO_ABC) */
ModuleData *aiDataProc(INI *ini)
{
	AIUpdateModuleData *data = MSGNEW( "AllModuleData" ) AIUpdateModuleData;
	if (ini) ini->initFromINIMultiProc(data, AIUpdateModuleData::buildFieldParse);
	return data;
}

struct TestModule
{
	const char *name;
	ModuleType type;
	int mask;
};
const TestModule g_testModules[] = {
	{ "TestUpdate", MODULETYPE_BEHAVIOR, MODULEINTERFACE_UPDATE },
	{ "TestUpdate2", MODULETYPE_BEHAVIOR, MODULEINTERFACE_UPDATE },
	{ "TestDie", MODULETYPE_BEHAVIOR, MODULEINTERFACE_DIE },
	{ "TestDie2", MODULETYPE_BEHAVIOR, MODULEINTERFACE_DIE },
	{ "TestBody", MODULETYPE_BEHAVIOR, MODULEINTERFACE_BODY },
	{ "TestBody2", MODULETYPE_BEHAVIOR, MODULEINTERFACE_BODY },
	{ "TestMulti", MODULETYPE_BEHAVIOR, MODULEINTERFACE_UPDATE | MODULEINTERFACE_DIE },
	{ "TestUpgrade", MODULETYPE_BEHAVIOR, MODULEINTERFACE_UPGRADE },
	{ "TestDraw", MODULETYPE_DRAW, MODULEINTERFACE_DRAW },
	{ "TestDraw2", MODULETYPE_DRAW, MODULEINTERFACE_DRAW },
	{ "TestClient", MODULETYPE_CLIENT_UPDATE, MODULEINTERFACE_CLIENT_UPDATE },
	{ "AIUpdateInterface", MODULETYPE_BEHAVIOR, MODULEINTERFACE_UPDATE },
	{ "TestAI2", MODULETYPE_BEHAVIOR, MODULEINTERFACE_UPDATE },
};
const char *const g_testWeapons[] = { "Rifle", "Cannon", "Missile" };

/* ---- dump helpers */
std::vector<unsigned char> g_dump;
void put(const void *p, int n) { g_dump.insert(g_dump.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void put8(int v) { g_dump.push_back((unsigned char)v); }
void putU32(unsigned v) { put(&v, 4); }
void putI32(int v) { put(&v, 4); }
void putU64(unsigned long long v) { put(&v, 8); }
void putF32(float v) { put(&v, 4); }
void putUni(const UnicodeString &s)
{
	putU32((unsigned)s.getLength());
	for (int i = 0; i < s.getLength(); ++i) { unsigned short u = (unsigned short)s.getCharAt(i); put(&u, 2); }
}
void putAscii(const AsciiString &s) { putU32((unsigned)s.getLength()); put(s.str(), s.getLength()); }
void putStr(const std::string &s) { putU32((unsigned)s.size()); put(s.data(), (int)s.size()); }
int finish(unsigned char *buf, int cap)
{
	int n = (int)g_dump.size();
	if (n <= cap && n > 0) memcpy(buf, &g_dump[0], n);
	return n;
}

/* a BitFlags as four 64-bit words: bit i is bit i % 64 of word i / 64 */
template<size_t N> void putFlags(const BitFlags<N> &f)
{
	unsigned long long w[4] = { 0, 0, 0, 0 };
	for (size_t i = 0; i < N && i < 256; ++i)
		if (f.test((Int)i)) w[i / 64] |= 1ULL << (i % 64);
	for (int i = 0; i < 4; ++i) putU64(w[i]);
}

/* the position of a root in the list (0 is the head), -1 if it is not in the list */
int listPosition(const ThingTemplate *t)
{
	int pos = 0;
	for (const ThingTemplate *r = TheThingFactory->m_firstTemplate; r; r = r->friend_getNextTemplate(), ++pos)
		if (r == t) return pos;
	return -1;
}

int armorKey(const ArmorTemplate *a)
{
	if (!a) return 0;
	for (ArmorStore::ArmorTemplateMap::const_iterator it = TheArmorStore->m_armorTemplates.begin();
		it != TheArmorStore->m_armorTemplates.end(); ++it)
		if (&it->second == a) return (int)it->first;
	return -1;
}
int dfxKey(const DamageFX *d)
{
	if (!d) return 0;
	for (DamageFXStore::DamageFXMap::const_iterator it = TheDamageFXStore->m_dfxmap.begin();
		it != TheDamageFXStore->m_dfxmap.end(); ++it)
		if (&it->second == d) return (int)it->first;
	return -1;
}
int weaponKey(const WeaponTemplate *w)
{
	if (!w) return 0;
	for (std::map<NameKeyType, WeaponTemplate *>::const_iterator it = g_weapons.begin(); it != g_weapons.end(); ++it)
		if (it->second == w) return (int)it->first;
	return -1;
}
int templateId(const ThingTemplate *t) { return t ? (int)t->getTemplateID() : -1; }

/* batch A1: an AI's module data (the locomotor sets by set, each template by its key in the store; the turrets,
** every field; then the rest) */
void dumpTurretAiData(const TurretAIData *td)
{
	put8(td ? 1 : 0);
	if (!td) return;
	putF32(td->m_turnRate);
	putF32(td->m_pitchRate);
	putF32(td->m_naturalTurretAngle);
	putF32(td->m_naturalTurretPitch);
	for (int i = 0; i < WEAPONSLOT_COUNT; ++i) putF32(td->m_turretFireAngleSweep[i]);
	for (int i = 0; i < WEAPONSLOT_COUNT; ++i) putF32(td->m_turretSweepSpeedModifier[i]);
	putF32(td->m_firePitch);
	putF32(td->m_minPitch);
	putF32(td->m_groundUnitPitch);
	putU32(td->m_turretWeaponSlots);
	putF32(td->m_minIdleScanAngle);
	putF32(td->m_maxIdleScanAngle);
	putU32(td->m_minIdleScanInterval);
	putU32(td->m_maxIdleScanInterval);
	putU32(td->m_recenterTime);
	put8(td->m_initiallyDisabled ? 1 : 0);
	put8(td->m_firesWhileTurning ? 1 : 0);
	put8(td->m_isAllowsPitch ? 1 : 0);
}
void dumpAiModuleData(const AIUpdateModuleData *d)
{
	putU32((unsigned)d->m_locomotorTemplates.size());
	for (LocomotorTemplateMap::const_iterator it = d->m_locomotorTemplates.begin(); it != d->m_locomotorTemplates.end(); ++it) {
		putI32((int)it->first);
		putU32((unsigned)it->second.size());
		for (size_t k = 0; k < it->second.size(); ++k) putU32(ref_loco_template_key(it->second[k]));
	}
	for (int i = 0; i < MAX_TURRETS; ++i) dumpTurretAiData(d->m_turretData[i]);
	putU32(d->m_moodAttackCheckRate);
	put8(d->m_forbidPlayerCommands ? 1 : 0);
	put8(d->m_turretsLinked ? 1 : 0);
	putU32(d->m_autoAcquireEnemiesWhenIdle);
}

void dumpModuleInfo(const ModuleInfo &mi)
{
	putU32((unsigned)mi.m_info.size());
	for (size_t i = 0; i < mi.m_info.size(); ++i) {
		const ModuleInfo::Nugget &n = mi.m_info[i];
		putAscii(n.first);
		putAscii(n.m_moduleTag);
		putI32(n.interfaceMask);
		put8(n.copiedFromDefault ? 1 : 0);
		put8(n.inheritable ? 1 : 0);
		put8(n.overrideableByLikeKind ? 1 : 0);
		const ClientModuleData *c = dynamic_cast<const ClientModuleData *>(n.second);
		if (!n.second) {
			put8(9);
			continue;
		}
		put8(c ? 1 : 0);
		put8(n.second->isAiModuleData() ? 1 : 0);
		putU32((unsigned)n.second->m_moduleTagNameKey);
		if (n.second->isAiModuleData())
			dumpAiModuleData((const AIUpdateModuleData *)n.second);
		if (c) {
			putU32((unsigned)c->lines.size());
			for (size_t k = 0; k < c->lines.size(); ++k) {
				putI32(c->lines[k].depth);
				putStr(c->lines[k].field);
				putU32((unsigned)c->lines[k].args.size());
				for (size_t a = 0; a < c->lines[k].args.size(); ++a) putStr(c->lines[k].args[a]);
			}
		}
	}
}

/* every member of one template object, then what its getters say */
void dumpTemplate(const ThingTemplate *t)
{
	putAscii(t->m_nameString);
	putUni(t->m_displayName);
	putAscii(t->m_defaultOwningSide);
	putAscii(t->m_commandSetString);
	putAscii(t->m_selectedPortraitImageName);
	putAscii(t->m_buttonImageName);
	for (int i = 0; i < MAX_UPGRADE_CAMEO_UPGRADES; ++i) putAscii(t->m_upgradeCameoUpgradeNames[i]);
	putAscii(t->m_shadowTextureName);
	putAscii(t->m_moduleBeingReplacedName);
	putAscii(t->m_moduleBeingReplacedTag);
	const GeometryInfo &g = t->m_geometryInfo;
	putI32((int)g.getGeomType());
	put8(g.getIsSmall() ? 1 : 0);
	putF32(g.m_height);
	putF32(g.getMajorRadius());
	putF32(g.getMinorRadius());
	putF32(g.getBoundingCircleRadius());
	putF32(g.getBoundingSphereRadius());
	putFlags(t->m_kindof);
	for (int i = 0; i < TTAUDIO_COUNT; ++i) {
		const DynamicAudioEventRTS *a = t->m_audioarray.m_audio[i];
		put8(a ? 1 : 0);
		if (a) putAscii(a->m_event.getEventName());
		put8(t->hasAudio((ThingTemplateAudioType)i) ? 1 : 0);
		putAscii(t->getAudio((ThingTemplateAudioType)i)->getEventName());
	}
	dumpModuleInfo(t->m_behaviorModuleInfo);
	dumpModuleInfo(t->m_drawModuleInfo);
	dumpModuleInfo(t->m_clientUpdateModuleInfo);
	for (int i = 0; i < LEVEL_COUNT; ++i) {
		putI32(t->m_skillPointValues[i]);
		putI32(t->m_experienceValues[i]);
		putI32(t->m_experienceRequired[i]);
		putI32(t->getSkillPointValue(i));
		putI32(t->getExperienceValue(i));
		putI32(t->getExperienceRequired(i));
	}
	putU32((unsigned)t->m_prereqInfo.size());
	for (size_t i = 0; i < t->m_prereqInfo.size(); ++i) {
		const ProductionPrerequisite &p = t->m_prereqInfo[i];
		putU32((unsigned)p.m_prereqUnits.size());
		for (size_t u = 0; u < p.m_prereqUnits.size(); ++u) {
			putI32(templateId(p.m_prereqUnits[u].unit));
			putI32(p.m_prereqUnits[u].flags);
			putAscii(p.m_prereqUnits[u].name);
		}
		putU32((unsigned)p.m_prereqSciences.size());
		for (size_t s = 0; s < p.m_prereqSciences.size(); ++s) putI32((int)p.m_prereqSciences[s]);
		const ThingTemplate *tmpls[32];
		Int n = p.getAllPossibleBuildFacilityTemplates(tmpls, 32);
		putI32(n);
		for (Int k = 0; k < n; ++k) putI32(templateId(tmpls[k]));
	}
	putU32((unsigned)t->m_buildVariations.size());
	for (size_t i = 0; i < t->m_buildVariations.size(); ++i) putAscii(t->m_buildVariations[i]);
	putU32((unsigned)t->m_weaponTemplateSets.size());
	for (size_t i = 0; i < t->m_weaponTemplateSets.size(); ++i) {
		const WeaponTemplateSet &w = t->m_weaponTemplateSets[i];
		putI32(templateId(w.friend_getThingTemplate()));
		putFlags(w.m_types);
		for (int s = 0; s < WEAPONSLOT_COUNT; ++s) {
			putI32(weaponKey(w.m_template[s]));
			putU32(w.m_autoChooseMask[s]);
			putFlags(w.m_preferredAgainst[s]);
		}
		put8(w.m_isReloadTimeShared ? 1 : 0);
		put8(w.m_isWeaponLockSharedAcrossSets ? 1 : 0);
		put8(w.hasAnyWeapons() ? 1 : 0);
	}
	putU32((unsigned)t->m_armorTemplateSets.size());
	for (size_t i = 0; i < t->m_armorTemplateSets.size(); ++i) {
		const ArmorTemplateSet &a = t->m_armorTemplateSets[i];
		putFlags(a.m_types);
		putI32(armorKey(a.m_template));
		putI32(dfxKey(a.m_fx));
	}
	putU32((unsigned)t->m_perUnitSounds.size());
	for (PerUnitSoundMap::const_iterator it = t->m_perUnitSounds.begin(); it != t->m_perUnitSounds.end(); ++it) {
		putAscii(it->first);
		putAscii(it->second.getEventName());
	}
	putU32((unsigned)t->m_perUnitFX.size());
	for (PerUnitFXMap::const_iterator it = t->m_perUnitFX.begin(); it != t->m_perUnitFX.end(); ++it) {
		putAscii(it->first);
		put8(it->second ? 1 : 0);
		if (it->second) putAscii(it->second->m_name);
	}
	if (t->m_reskinnedFrom) putI32(listPosition(t->m_reskinnedFrom)); else putI32(-2);
	putF32(t->m_fenceWidth);
	putF32(t->m_fenceXOffset);
	putF32(t->m_visionRange);
	putF32(t->m_shroudClearingRange);
	putF32(t->m_shroudRevealToAllRange);
	putF32(t->m_placementViewAngle);
	putF32(t->m_factoryExitWidth);
	putF32(t->m_factoryExtraBibWidth);
	putF32(t->m_buildTime);
	putF32(t->m_assetScale);
	putF32(t->m_instanceScaleFuzziness);
	putF32(t->m_shadowSizeX);
	putF32(t->m_shadowSizeY);
	putF32(t->m_shadowOffsetX);
	putF32(t->m_shadowOffsetY);
	putI32(t->m_energyProduction);
	putI32(t->m_energyBonus);
	putI32((int)t->m_displayColor);
	putU32(t->m_occlusionDelay);
	putU32((unsigned)t->m_maxSimultaneousLinkKey);
	putU32(t->m_templateID);
	putU32(t->m_buildCost);
	putU32(t->m_refundValue);
	putU32(t->m_threatValue);
	putU32(t->m_maxSimultaneousOfType);
	put8(t->m_maxSimultaneousDeterminedBySuperweaponRestriction ? 1 : 0);
	put8(t->m_isPrerequisite ? 1 : 0);
	put8(t->m_isBridge ? 1 : 0);
	put8(t->m_isBuildFacility ? 1 : 0);
	put8(t->m_isTrainable ? 1 : 0);
	put8(t->m_enterGuard ? 1 : 0);
	put8(t->m_hijackGuard ? 1 : 0);
	put8(t->m_isForbidden ? 1 : 0);
	put8(t->m_armorCopiedFromDefault ? 1 : 0);
	put8(t->m_weaponsCopiedFromDefault ? 1 : 0);
	put8(t->m_radarPriority);
	put8(t->m_transportSlotCount);
	put8(t->m_buildable);
	put8(t->m_buildCompletion);
	put8(t->m_editorSorting);
	put8(t->m_structureRubbleHeight);
	put8(t->m_shadowType);
	put8(t->m_moduleParsingMode);
	put8(t->m_crusherLevel);
	put8(t->m_crushableLevel);
	/* the getters that compute something */
	putU32(t->getMaxSimultaneousOfType());
	put8(t->isBuildableItem() ? 1 : 0);
	put8(t->canPossiblyHaveAnyWeapon() ? 1 : 0);
	putF32(t->getStructureRubbleHeight());
	putI32(t->getBuildable());
	putU32(t->getThreatValue());
}

/* the root and every override of its chain: the override flag and the template */
void dumpChain(const ThingTemplate *root)
{
	int n = 0;
	for (const Overridable *o = root; o; o = o->m_nextOverride) ++n;
	putI32(n);
	for (const Overridable *o = root; o; o = o->m_nextOverride) {
		put8(o->m_isOverride ? 1 : 0);
		dumpTemplate((const ThingTemplate *)o);
	}
}

void dumpFactory()
{
	ThingFactory *f = TheThingFactory;
	putU32(f->m_nextTemplateID);
	int n = 0;
	for (const ThingTemplate *t = f->m_firstTemplate; t; t = t->friend_getNextTemplate()) ++n;
	putI32(n);
	for (const ThingTemplate *t = f->m_firstTemplate; t; t = t->friend_getNextTemplate()) dumpChain(t);
	/* the name map: every name in order, with the list position of what it finds */
	putU32((unsigned)f->m_templateHashMap.size());
	for (ThingTemplateHashMap::const_iterator it = f->m_templateHashMap.begin(); it != f->m_templateHashMap.end(); ++it) {
		putAscii(it->first);
		putI32(listPosition(it->second));
	}
	/* the module factory's kept data */
	/* (the Rust factory keeps the data of behavior modules only: the client modules' data is not kept) */
	unsigned kept = 0;
	for (size_t i = 0; i < TheModuleFactory->m_moduleDataList.size(); ++i)
		if (!dynamic_cast<const ClientModuleData *>(TheModuleFactory->m_moduleDataList[i])) ++kept;
	putU32(kept);
	for (size_t i = 0; i < TheModuleFactory->m_moduleDataList.size(); ++i)
		if (!dynamic_cast<const ClientModuleData *>(TheModuleFactory->m_moduleDataList[i]))
			putU32((unsigned)TheModuleFactory->m_moduleDataList[i]->m_moduleTagNameKey);
}

/* the INI's throw as a status (as wrap_science.cpp): 2 an INI_* code, 5 other */
int unnamedEnumStatus(unsigned *code)
{
	const std::type_info *t = abi::__cxa_current_exception_type();
	if (t && dynamic_cast<const abi::__enum_type_info *>(t)) { *code = (unsigned)ERROR_BAD_INI; return 2; }
	return 5;
}

/* the node of a chain: depth 0 is the root, -1 the final override */
const ThingTemplate *nodeAt(int position, int depth)
{
	const ThingTemplate *t = TheThingFactory->m_firstTemplate;
	for (; t && position > 0; --position) t = t->friend_getNextTemplate();
	if (!t) return 0;
	if (depth < 0) return (const ThingTemplate *)t->getFinalOverride();
	const Overridable *o = t;
	for (; o && depth > 0; --depth) o = o->getNextOverride();
	return (const ThingTemplate *)o;
}

} /* namespace */

/* the hook wrap_ini.cpp's Object and ObjectReskin handlers call (see the header) */
static void objectHook(INI *ini, int reskin)
{
	if (reskin) INI::parseObjectReskinDefinition(ini); else INI::parseObjectDefinition(ini);
}

extern "C" void (*ref_object_block_hook)(INI *, int);
extern "C" void ref_armor_reset(void);
extern "C" void ref_science_reset(void);

extern "C" {

/* Fresh systems: the Armor build's stores, then the Science build's name key generator (reset: keys start at
** 1 again), game text, hook and science store; a new ThingFactory and ModuleFactory; the test modules (in the
** order of g_testModules, each assigning a decorated name key) and the test weapons (a key each). The global
** data values are the arguments. */
void ref_tt_reset(unsigned defaultOcclusionDelay, unsigned superweaponRestriction)
{
	ref_object_block_hook = objectHook;
	ref_armor_reset();
	ref_science_reset();
	ref_loco_store_new();
	g_globalData.m_defaultOcclusionDelay = defaultOcclusionDelay;
	g_globalData.m_LowEnergyPenaltyModifier = 0.0f;
	g_globalData.m_MinLowEnergyProductionSpeed = 0.0f;
	g_globalData.m_MaxLowEnergyProductionSpeed = 0.0f;
	g_globalData.m_MultipleFactory = 0.0f;
	g_superweaponRestriction = superweaponRestriction;
	delete TheThingFactory;
	TheThingFactory = new ThingFactory;
	delete TheModuleFactory;
	TheModuleFactory = new ModuleFactory;
	for (size_t i = 0; i < sizeof(g_testModules) / sizeof(g_testModules[0]); ++i) {
		const TestModule &m = g_testModules[i];
		ModuleData *(*proc)(INI *) = m.type != MODULETYPE_BEHAVIOR ? clientDataProc : strncmp(m.name, "Test", 4) ? aiDataProc
			: !strcmp(m.name, "TestAI2") ? aiDataProc : baseDataProc;
		TheModuleFactory->addModuleInternal(0, proc, m.type, AsciiString(m.name), m.mask);
	}
	g_weapons.clear();
	for (size_t i = 0; i < sizeof(g_testWeapons) / sizeof(g_testWeapons[0]); ++i) {
		WeaponTemplate *w = new WeaponTemplate;
		w->m_name = g_testWeapons[i];
		g_weapons[TheNameKeyGenerator->nameToKey(g_testWeapons[i])] = w;
	}
	g_lastObjectTemplate = 0;
	g_lastDrawableTemplate = 0;
}

unsigned ref_tt_name_key(const char *name) { return (unsigned)TheNameKeyGenerator->nameToKey(name); }

/* INI::load(filename, loadType, xfer ? &crc : NULL). Returns the throw status (0 none, 1 INIException with the
** message in msg and its length in *code, 2 an INI_* code, 3 an ErrorCode, 4 std::out_of_range, 5 other). */
int ref_tt_load(const char *filename, int loadType, int withXfer, unsigned *crc, char *msg, int msgCap, unsigned *code)
{
	XferCRC x;
	int status = 0;
	try {
		INI ini;
		ini.load(AsciiString(filename), (INILoadType)loadType, withXfer ? &x : NULL);
	}
	catch (INIException &e) {
		status = 1;
		const char *m = e.mFailureMessage ? e.mFailureMessage : "";
		int n = (int)strlen(m);
		*code = (unsigned)n;
		if (n + 1 <= msgCap) memcpy(msg, m, n + 1);
	}
	catch (ErrorCode c) { status = 3; *code = (unsigned)c; }
	catch (std::out_of_range &) { status = 4; }
	catch (...) { status = unnamedEnumStatus(code); }
	*crc = x.getCRC();
	return status;
}

/* op 0 ThingFactory::reset(); 1 init(); 2 update(); 3 postProcessLoad() */
void ref_tt_op(int op)
{
	if (op == 0) TheThingFactory->reset();
	else if (op == 1) TheThingFactory->init();
	else if (op == 2) TheThingFactory->update();
	else TheThingFactory->postProcessLoad();
}

/* the whole store (see dumpFactory) */
int ref_tt_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	dumpFactory();
	return finish(buf, cap);
}

/* the tokens of the Object (which 0) or ObjectReskin (1) field table: count, then each (length, bytes) */
int ref_tt_fields(int which, unsigned char *buf, int cap)
{
	ThingTemplate t;
	const FieldParse *fp = which == 0 ? t.getFieldParse() : t.getReskinFieldParse();
	g_dump.clear();
	int n = 0;
	for (const FieldParse *p = fp; p->token; ++p) ++n;
	putI32(n);
	for (const FieldParse *p = fp; p->token; ++p) putStr(p->token);
	return finish(buf, cap);
}

/* findTemplate(name): its position in the list, -1 for NULL */
int ref_tt_find(const char *name)
{
	return listPosition(TheThingFactory->findTemplate(AsciiString(name), FALSE));
}

/* findByTemplateID(id): its position, -1 for NULL */
int ref_tt_find_by_id(unsigned id) { return listPosition(TheThingFactory->findByTemplateID((UnsignedShort)id)); }

/* isEquivalentTo between node (positionA, depthA) and node (positionB, depthB), depth -1 the final override;
** -1 if either is missing */
int ref_tt_equivalent(int pa, int da, int pb, int db)
{
	const ThingTemplate *a = nodeAt(pa, da), *b = nodeAt(pb, db);
	if (!a || !b) return -1;
	return a->isEquivalentTo(b) ? 1 : 0;
}

/* newObject(tmplate, NULL, mask): the template the object was made from (its id, or -1), the status bits (four
** 64-bit words) and the game logic random seed CRC afterwards. The node is (position, depth). Returns 0, or 1
** if there is no such node (newObject(NULL) throws ERROR_BAD_ARG: 2 for that). */
int ref_tt_new_object(int position, int depth, int nullTemplate, const unsigned long long *maskWords, int *madeFrom, unsigned long long *statusOut)
{
	const ThingTemplate *tmpl = nullTemplate ? 0 : nodeAt(position, depth);
	if (!nullTemplate && !tmpl) return 1;
	ObjectStatusMaskType mask;
	for (int i = 0; i < OBJECT_STATUS_COUNT && i < 256; ++i)
		if ((maskWords[i / 64] >> (i % 64)) & 1) mask.set(i);
	g_lastObjectTemplate = 0;
	try {
		TheThingFactory->newObject(tmpl, 0, mask);
	}
	catch (ErrorCode) { return 2; }
	*madeFrom = templateId(g_lastObjectTemplate);
	unsigned long long w[4] = { 0, 0, 0, 0 };
	for (int i = 0; i < OBJECT_STATUS_COUNT && i < 256; ++i)
		if (g_lastObjectStatus.test(i)) w[i / 64] |= 1ULL << (i % 64);
	for (int i = 0; i < 4; ++i) statusOut[i] = w[i];
	return 0;
}

/* newDrawable(tmplate, bits): the template and the bits the client was asked to make it with */
int ref_tt_new_drawable(int position, int depth, int nullTemplate, int bits, int *madeFrom, int *statusOut)
{
	const ThingTemplate *tmpl = nullTemplate ? 0 : nodeAt(position, depth);
	if (!nullTemplate && !tmpl) return 1;
	g_lastDrawableTemplate = 0;
	try {
		TheThingFactory->newDrawable(tmpl, (DrawableStatus)bits);
	}
	catch (ErrorCode) { return 2; }
	*madeFrom = templateId(g_lastDrawableTemplate);
	*statusOut = g_lastDrawableStatus;
	return 0;
}

/* findArmorTemplateSet / findWeaponTemplateSet of node (position, depth) for the flag words: the index in the
** template's vector, -1 for NULL; -2 if there is no such node. which 0 armor, 1 weapon. */
int ref_tt_find_set(int position, int depth, int which, unsigned long long flags)
{
	const ThingTemplate *t = nodeAt(position, depth);
	if (!t) return -2;
	if (which == 0) {
		ArmorSetFlags f;
		for (int i = 0; i < ARMORSET_COUNT; ++i) if ((flags >> i) & 1) f.set(i);
		const ArmorTemplateSet *s = t->findArmorTemplateSet(f);
		if (!s) return -1;
		/* a copy of a template carries the SparseMatchFinder cache of its source, whose pointers point into the
		** SOURCE's vector (cleared only when an ArmorSet is parsed): -3 */
		if (s < &t->m_armorTemplateSets[0] || s >= &t->m_armorTemplateSets[0] + t->m_armorTemplateSets.size()) return -3;
		return (int)(s - &t->m_armorTemplateSets[0]);
	}
	WeaponSetFlags f;
	for (int i = 0; i < WEAPONSET_COUNT; ++i) if ((flags >> i) & 1) f.set(i);
	const WeaponTemplateSet *s = t->findWeaponTemplateSet(f);
	if (!s) return -1;
	if (s < &t->m_weaponTemplateSets[0] || s >= &t->m_weaponTemplateSets[0] + t->m_weaponTemplateSets.size()) return -3;
	return (int)(s - &t->m_weaponTemplateSets[0]);
}

/* the per-unit lookups of node (position, depth): which 0 getPerUnitSound(name) (event name), 1 getPerUnitFX(name)
** (FX name; *has 0 for NULL) */
int ref_tt_per_unit(int position, int depth, int which, const char *name, char *out, int cap, int *has)
{
	const ThingTemplate *t = nodeAt(position, depth);
	if (!t) return -1;
	std::string s;
	*has = 1;
	if (which == 0) {
		s = t->getPerUnitSound(AsciiString(name))->getEventName().str();
	} else {
		const FXList *fx = t->getPerUnitFX(AsciiString(name));
		if (fx) s = fx->m_name.str(); else *has = 0;
	}
	if ((int)s.size() < cap) memcpy(out, s.data(), s.size());
	return (int)s.size();
}

} /* extern "C" */
