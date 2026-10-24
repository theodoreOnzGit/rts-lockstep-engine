/* Differential-test wrapper for the module base classes (upstream commit 0a05454d), p1w3-modules:
**   GeneralsMD/Code/GameEngine/Source/Common/Thing/Module.cpp                       (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BehaviorModule.cpp  (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/UpdateModule.cpp      (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/UpgradeModule.cpp    (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/DieModule.cpp            (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/CreateModule.cpp      (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/DamageModule.cpp      (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CollideModule.cpp    (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Destroy/DestroyModule.cpp    (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/BodyModule.cpp          (#included, unchanged)
** with the headers Module.h, BehaviorModule.h, UpdateModule.h, UpgradeModule.h, DieModule.h, CreateModule.h,
** DamageModule.h, CollideModule.h, DestroyModule.h, BodyModule.h, ContainModule.h, Upgrade.h, INI.h, Damage.h,
** ObjectStatusTypes.h and NameKeyGenerator.h unchanged, in the BitFlags build's setup (clang++ -fms-extensions,
** the real BitFlagsIO.h) plus differential/shim_module (module_prelude.h, forced in): the stand-ins of Thing,
** ThingTemplate, Object, GameLogic, ExperienceTracker, Drawable and FXList (client and later-layer classes the
** module files only name), and differential/shim_upgrade for the Upgrade.h forwarder and the audio stand-ins.
**
** Renaming. The Upgrade classes keep the names the Upgrade build gives them (UpTestUpgradeCenter,
** UpTestTheUpgradeCenter, ...), so that `TheUpgradeCenter->findUpgrade` here reaches the REAL UpgradeCenter that
** differential/wrap_upgrade.cpp compiles from Upgrade.cpp and fills (ref_up_reset, ref_up_load): the module
** tests define their upgrades through those functions. The classes this file stands in for (Object, Thing,
** Drawable, FXList, GameLogic, ...) are renamed (ModTest...) so that no stand-in can clash with another unit's
** stand-in of the same class (the Armor build has its own Object and FXList).
**
** What is NOT compiled here (their .cpp files need Player, Eva, the script engine, the thing factory and more,
** and their tests are Rust-only, see tests/differential_module.rs): SpecialPowerModule.cpp,
** SpecialPowerUpdateModule.cpp (the SpecialPowerModuleData parse also needs TheSpecialPowerStore, which the INI
** build leaves null), ModuleFactory.cpp (205 module headers; only its init() body is preprocessed, below) and
** ContainModule.h (an interface of sixty pure virtuals, one inline function).
**
** The registrations. build.rs cuts the body of ModuleFactory::init verbatim out of ModuleFactory.cpp
** (module_registrations.inc, asserted once), and it is preprocessed here twice with `addModule` defined as a
** recorder: once with ALLOW_SURRENDER and ALLOW_DEMORALIZE defined (all 205 calls) and once as the shipped game
** (neither: 199).
**
** INI. The data is read by the REAL INI (upstream_ini) from wrap_ini.cpp's in-memory file system
** (ref_ini_reset, ref_ini_add_file), as the block `D` of a file: this file's functions read the `D` line, run
** the class's friend_newModuleData, and wrap an error as INI::load does (`Error parsing INI file '%s' (Line:
** '%s')`, the six lines of INI::load that matter, copied below).
**
** Dumps. Private and protected members are made public for this build only (module_prelude.h). */
/* --- p1w10-sonnet --- MaxHealthUpgrade.cpp reads TheMaxHealthChangeTypeNames, which BodyModule.h defines only under this macro (BodyModule.cpp below includes the header first) */
#define DEFINE_MAXHEALTHCHANGETYPE_NAMES
/* --- end p1w10-sonnet --- */
#include "Common/INI.h"
#include "Common/INIException.h"
#include "Common/NameKeyGenerator.h"
#include "Common/XferCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/Upgrade.h"
#include "Common/GameAudio.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameClient/FXList.h"

/* the upstream files, unchanged */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/Module.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BehaviorModule.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/UpdateModule.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/UpgradeModule.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/DieModule.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/CreateModule.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/DamageModule.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CollideModule.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Destroy/DestroyModule.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/BodyModule.cpp"

/* ---- stand-ins ----------------------------------------------------------- */
GameLogic *TheGameLogic = 0;

namespace {
/* what a module did to the world, in order: 0 the upgrade implementation, 1 FXList::doFXObj (p: the list),
** 2 Object::removeUpgrade (p: the template) */
struct Event { int kind; const void *p; };
std::vector<Event> g_events;
void event(int kind, const void *p) { Event e = { kind, p }; g_events.push_back(e); }
}
/* --- p1w11-sonnet --- (a driver that wants the objects of a doFXObj sets this hook; none by default) */
void (*g_fxObjHook)(const Object *primary, const Object *secondary) = 0;
/* --- end p1w11-sonnet --- */
void FXList::doFXObj(const FXList *fx, const Object *a, const Object *b) { event(1, fx); if (g_fxObjHook) g_fxObjHook(a, b); }
void Object::removeUpgrade(const UpgradeTemplate *t)
{
	event(2, t);
	/* --- p1l9-example --- */
	/* upstream's first line (Object.cpp:4517); its loop over the object's upgrade modules is not modelled
	** (a module test has one module, and its own resetUpgrade changes nothing observable mid-attempt) */
	if (t) m_objectUpgradesCompleted.clear(t->getUpgradeMask());
	/* --- end p1l9-example --- */
}

/* ---- the dump ------------------------------------------------------------ */
namespace {

std::vector<unsigned char> g_dump;
void put(const void *p, int n) { g_dump.insert(g_dump.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void putU32(unsigned v) { put(&v, 4); }
void putI32(int v) { put(&v, 4); }
void putU64(unsigned long long v) { put(&v, 8); }
void putStr(const char *s, int n) { putU32((unsigned)n); put(s, n); }
void putAscii(const AsciiString &s) { putStr(s.str(), s.getLength()); }
int finish(unsigned char *buf, int cap)
{
	int n = (int)g_dump.size();
	if (n <= cap && n > 0) memcpy(buf, &g_dump[0], n);
	return n;
}

template <class MASK> void maskWords(const MASK &m, int bits, unsigned long long *w)
{
	w[0] = w[1] = w[2] = w[3] = 0;
	for (int i = 0; i < bits; ++i) if (m.test(i)) w[i / 64] |= 1ULL << (i % 64);
}
template <class MASK> MASK maskFromU32(int bits, const unsigned *w)
{
	MASK m;
	for (int i = 0; i < bits; ++i) if (w[i / 32] >> (i % 32) & 1) m.set(i);
	return m;
}
void putUpgradeMask(const UpgradeMaskType &m)
{
	unsigned long long w[4];
	maskWords(m, UPGRADE_MAX_COUNT, w);
	for (int i = 0; i < 4; ++i) putU64(w[i]);
}
void putStatusMask(const ObjectStatusMaskType &m)
{
	unsigned long long w[4];
	maskWords(m, OBJECT_STATUS_COUNT, w);
	for (int i = 0; i < 4; ++i) putU64(w[i]);
}

/* the INI's throw, as a status (the same as wrap_upgrade.cpp's): 0 none, 1 INIException (message into msg),
** 2 an INI_* status code, 3 an ErrorCode, 4 std::out_of_range, 5 other */
int unnamedEnumStatus(unsigned *code)
{
	const std::type_info *t = abi::__cxa_current_exception_type();
	if (t && dynamic_cast<const abi::__enum_type_info *>(t)) { *code = (unsigned)ERROR_BAD_INI; return 2; }
	return 5;
}
#define CATCH_ALL(status, code, msg, msgCap)                                          \
	catch (INIException &e) {                                                         \
		status = 1;                                                                   \
		const char *m = e.mFailureMessage ? e.mFailureMessage : "";                   \
		int n = (int)strlen(m);                                                       \
		*code = (unsigned)n;                                                          \
		if (n + 1 <= msgCap) memcpy(msg, m, n + 1);                                   \
	}                                                                                 \
	catch (ErrorCode c) { status = 3; *code = (unsigned)c; }                          \
	catch (std::out_of_range &) { status = 4; }                                       \
	catch (...) { status = unnamedEnumStatus(code); }

/* the canonical number of an FXList pointer: 0 for NULL, else 1 + the order the pointers were first seen in
** this dump (the stand-in store of the Armor build answers a distinct pointer per name) */
std::map<const FXList *, int> g_fxIds;
int fxId(const FXList *fx)
{
	if (!fx) return 0;
	std::map<const FXList *, int>::iterator it = g_fxIds.find(fx);
	if (it == g_fxIds.end()) it = g_fxIds.insert(std::make_pair(fx, (int)g_fxIds.size() + 1)).first;
	return it->second;
}

/* ---- concrete classes for the abstract bases ------------------------------- */

#define COMMON_TEST_MODULE_BITS \
	virtual NameKeyType getModuleNameKey() const { return NAMEKEY_INVALID; }

class TestBehavior : public BehaviorModule {
public:
	TestBehavior(Thing *t, const ModuleData *d) : BehaviorModule(t, d) {}
	COMMON_TEST_MODULE_BITS
};

class TestObjectModule : public ObjectModule {
public:
	TestObjectModule(Thing *t, const ModuleData *d) : ObjectModule(t, d) {}
	COMMON_TEST_MODULE_BITS
};

class TestDrawableModule : public DrawableModule {
public:
	TestDrawableModule(Thing *t, const ModuleData *d) : DrawableModule(t, d) {}
	COMMON_TEST_MODULE_BITS
};

class TestUpdate : public UpdateModule {
public:
	TestUpdate(Thing *t, const ModuleData *d, SleepyUpdatePhase ph) : UpdateModule(t, d), m_phase(ph) {}
	COMMON_TEST_MODULE_BITS
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
	virtual SleepyUpdatePhase getUpdatePhase() const { return m_phase; }
	SleepyUpdatePhase m_phase;
};

/* the effects the UpgradeModule leaves to a concrete module: the implementation, logged */
class TestUpgrade : public UpgradeModule {
public:
	TestUpgrade(Thing *t, const ModuleData *d) : UpgradeModule(t, d) {}
	COMMON_TEST_MODULE_BITS
	virtual void upgradeImplementation() { event(0, 0); }
	virtual Bool isSubObjectsUpgrade() { return false; }
};

class TestCreate : public CreateModule {
public:
	TestCreate(Thing *t, const ModuleData *d) : CreateModule(t, d) {}
	COMMON_TEST_MODULE_BITS
	virtual void onCreate() {}
};

class TestDamage : public DamageModule {
public:
	TestDamage(Thing *t, const ModuleData *d) : DamageModule(t, d) {}
	COMMON_TEST_MODULE_BITS
	virtual void onDamage(DamageInfo *) {}
	virtual void onHealing(DamageInfo *) {}
	virtual void onBodyDamageStateChange(const DamageInfo *, BodyDamageType, BodyDamageType) {}
};

class TestCollide : public CollideModule {
public:
	TestCollide(Thing *t, const ModuleData *d) : CollideModule(t, d) {}
	COMMON_TEST_MODULE_BITS
	virtual void onCollide(Object *, const Coord3D *, const Coord3D *) {}
};

class TestDestroy : public DestroyModule {
public:
	TestDestroy(Thing *t, const ModuleData *d) : DestroyModule(t, d) {}
	COMMON_TEST_MODULE_BITS
	virtual void onDestroy() {}
};

class TestDie : public DieModule {
public:
	TestDie(Thing *t, const ModuleData *d) : DieModule(t, d) {}
	COMMON_TEST_MODULE_BITS
	virtual void onDie(const DamageInfo *) {}
};

class TestBody : public BodyModule {
public:
	TestBody(Thing *t, const ModuleData *d) : BodyModule(t, d) {}
	COMMON_TEST_MODULE_BITS
	virtual void attemptDamage(DamageInfo *) {}
	virtual void attemptHealing(DamageInfo *) {}
	virtual Real estimateDamage(DamageInfoInput &) const { return 0; }
	virtual Real getHealth() const { return 0; }
	virtual BodyDamageType getDamageState() const { return BODY_PRISTINE; }
	virtual void setDamageState(BodyDamageType) {}
	virtual void setAflame(Bool) {}
	virtual void onVeterancyLevelChanged(VeterancyLevel, VeterancyLevel, Bool) {}
	virtual void setArmorSetFlag(ArmorSetType) {}
	virtual void clearArmorSetFlag(ArmorSetType) {}
	virtual Bool testArmorSetFlag(ArmorSetType) { return FALSE; }
	virtual void internalChangeHealth(Real) {}
};

/* ---- reading a module data from the INI text ------------------------------ */

enum DataKind {
	DK_MODULE = 0, DK_UPDATE, DK_UPGRADE, DK_DIE, DK_DAMAGE, DK_COLLIDE, DK_BEHAVIOR, DK_BODY, DK_CREATE
};

/* the file's first line is `D` (the block header, as INI::load reads it), then the fields; the data class's
** friend_newModuleData (or buildFieldParse through initFromINIMultiProc where the class has no standard
** macro) reads them until End. An error leaves INI::load as the outer wrapper: INIException with the message
** of INI::load line 386-392. */
ModuleData *readData(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			switch (kind) {
			case DK_MODULE: data = Module::friend_newModuleData(&ini); break;
			case DK_UPDATE: data = UpdateModule::friend_newModuleData(&ini); break;
			case DK_UPGRADE: data = UpgradeModule::friend_newModuleData(&ini); break;
			case DK_DIE: data = DieModule::friend_newModuleData(&ini); break;
			case DK_DAMAGE: data = DamageModule::friend_newModuleData(&ini); break;
			case DK_COLLIDE: data = CollideModule::friend_newModuleData(&ini); break;
			case DK_BEHAVIOR: { BehaviorModuleData *d = new BehaviorModuleData; ini.initFromINIMultiProc(d, BehaviorModuleData::buildFieldParse); data = d; break; }
			case DK_BODY: { BodyModuleData *d = new BodyModuleData; ini.initFromINIMultiProc(d, BodyModuleData::buildFieldParse); data = d; break; }
			case DK_CREATE: { CreateModuleData *d = new CreateModuleData; ini.initFromINIMultiProc(d, CreateModuleData::buildFieldParse); data = d; break; }
			}
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

void putMux(const UpgradeMuxData &m)
{
	putU32((unsigned)m.m_triggerUpgradeNames.size());
	for (size_t i = 0; i < m.m_triggerUpgradeNames.size(); ++i) putAscii(m.m_triggerUpgradeNames[i]);
	putU32((unsigned)m.m_activationUpgradeNames.size());
	for (size_t i = 0; i < m.m_activationUpgradeNames.size(); ++i) putAscii(m.m_activationUpgradeNames[i]);
	putU32((unsigned)m.m_conflictingUpgradeNames.size());
	for (size_t i = 0; i < m.m_conflictingUpgradeNames.size(); ++i) putAscii(m.m_conflictingUpgradeNames[i]);
	putU32((unsigned)m.m_removalUpgradeNames.size());
	for (size_t i = 0; i < m.m_removalUpgradeNames.size(); ++i) putAscii(m.m_removalUpgradeNames[i]);
	putI32(fxId(m.m_fxListUpgrade));
	putUpgradeMask(m.m_activationMask);
	putUpgradeMask(m.m_conflictingMask);
	g_dump.push_back(m.m_requiresAllTriggers ? 1 : 0);
}

void putDieMux(const DieMuxData &m)
{
	putU32(m.m_deathTypes);
	putU32(m.m_veterancyLevels);
	putStatusMask(m.m_exemptStatus);
	putStatusMask(m.m_requiredStatus);
}

/* an Object stand-in with a veterancy level and status bits, for the modules to be made on */
Object *newObject() { return new Object; }

}  // namespace

extern "C" void ref_ini_reset(void);
extern "C" void ref_ini_add_file(const char *name, const char *data, int len);

extern "C" {

/* ---- the registrations ---------------------------------------------------- */

static std::vector<std::string> g_reg;
static void reg(const char *n) { g_reg.push_back(n); }
#undef addModule
#define addModule(classname) reg(#classname)

static void collectAll()
{
#define ALLOW_SURRENDER 1
#define ALLOW_DEMORALIZE 1
#include "module_registrations.inc"
#undef ALLOW_SURRENDER
#undef ALLOW_DEMORALIZE
}

static void collectShipped()
{
#include "module_registrations.inc"
}

/* The addModule calls of ModuleFactory::init, in order: which 0 with ALLOW_SURRENDER and ALLOW_DEMORALIZE
** defined, 1 as the shipped game. Count, then each name (length, bytes). */
int ref_mod_registrations(int which, unsigned char *buf, int cap)
{
	g_reg.clear();
	if (which == 0) collectAll(); else collectShipped();
	g_dump.clear();
	putU32((unsigned)g_reg.size());
	for (size_t i = 0; i < g_reg.size(); ++i) putStr(g_reg[i].c_str(), (int)g_reg[i].size());
	return finish(buf, cap);
}

/* ---- constants ------------------------------------------------------------ */

/* i: 0..11 the MODULEINTERFACE_* values in order, 12..14 MODULETYPE_BEHAVIOR/DRAW/CLIENT_UPDATE, 15 NUM_MODULE_TYPES,
** 16 FIRST_DRAWABLE_MODULE_TYPE, 17 LAST_DRAWABLE_MODULE_TYPE, 18 NUM_DRAWABLE_MODULE_TYPES, 19..28 the
** getInterfaceMask() of UpdateModule, UpgradeModule, DieModule, DamageModule, CreateModule, CollideModule,
** BodyModule, DestroyModule, BehaviorModule, and BehaviorModule::getModuleType(); 29 UPDATE_SLEEP_INVALID, 30 NONE,
** 31 FOREVER, 32..35 PHASE_INITIAL..PHASE_FINAL, 36 FOREVER (GameCommon.h) */
int ref_mod_const(int i)
{
	switch (i) {
	case 0: return MODULEINTERFACE_UPDATE; case 1: return MODULEINTERFACE_DIE; case 2: return MODULEINTERFACE_DAMAGE;
	case 3: return MODULEINTERFACE_CREATE; case 4: return MODULEINTERFACE_COLLIDE; case 5: return MODULEINTERFACE_BODY;
	case 6: return MODULEINTERFACE_CONTAIN; case 7: return MODULEINTERFACE_UPGRADE; case 8: return MODULEINTERFACE_SPECIAL_POWER;
	case 9: return MODULEINTERFACE_DESTROY; case 10: return MODULEINTERFACE_DRAW; case 11: return MODULEINTERFACE_CLIENT_UPDATE;
	case 12: return MODULETYPE_BEHAVIOR; case 13: return MODULETYPE_DRAW; case 14: return MODULETYPE_CLIENT_UPDATE;
	case 15: return NUM_MODULE_TYPES; case 16: return FIRST_DRAWABLE_MODULE_TYPE; case 17: return LAST_DRAWABLE_MODULE_TYPE;
	case 18: return NUM_DRAWABLE_MODULE_TYPES;
	case 19: return UpdateModule::getInterfaceMask(); case 20: return UpgradeModule::getInterfaceMask();
	case 21: return DieModule::getInterfaceMask(); case 22: return DamageModule::getInterfaceMask();
	case 23: return CreateModule::getInterfaceMask(); case 24: return CollideModule::getInterfaceMask();
	case 25: return BodyModule::getInterfaceMask(); case 26: return DestroyModule::getInterfaceMask();
	case 27: return BehaviorModule::getInterfaceMask(); case 28: return BehaviorModule::getModuleType();
	case 29: return UPDATE_SLEEP_INVALID; case 30: return UPDATE_SLEEP_NONE; case 31: return UPDATE_SLEEP_FOREVER;
	case 32: return PHASE_INITIAL; case 33: return PHASE_PHYSICS; case 34: return PHASE_NORMAL; case 35: return PHASE_FINAL;
	case 36: return FOREVER;
	}
	return -12345;
}

/* ---- the module data ------------------------------------------------------- */

/* Read the module data of `kind` from the file `filename` (added with ref_ini_add_file), as the block `D`.
** Status of CATCH_ALL (0 none, 1 INIException with its message, 2 INI_* code, 3 ErrorCode...). On success the dump:
** DK_UPGRADE: the UpgradeMuxData (putMux); DK_DIE: the DieMuxData; the others: nothing. */
int ref_mod_data(int kind, const char *filename, unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	int status = 0;
	*len = 0;
	try {
		ModuleData *d = readData(kind, filename);
		if (kind == DK_UPGRADE) putMux(((UpgradeModuleData *)d)->m_upgradeMuxData);
		if (kind == DK_DIE) putDieMux(((DieModuleData *)d)->m_dieMuxData);
		delete d;
		*len = finish(buf, cap);
	}
	CATCH_ALL(status, code, msg, msgCap)
	return status;
}

/* ---- the base-class checks and xfer chains --------------------------------- */

/* Xfer of a snapshot: kind 0 save (the file's bytes), 1 load (`bytes` into the snapshot), 2 CRC. Returns the
** XferStatus thrown (0 none, -1 anything else). */
static int runXfer(Snapshot *s, int kind, const char *path, unsigned char *bytes, int cap, int *len, unsigned *crc)
{
	int st = 0;
	*len = 0;
	if (kind == 0) {
		{
			XferSave x;
			try { x.open(AsciiString(path)); x.xferSnapshot(s); } catch (XferStatus e) { st = (int)e; } catch (...) { st = -1; }
			try { x.close(); } catch (...) {}
		}
		if (FILE *f = fopen(path, "rb")) { *len = (int)fread(bytes, 1, cap, f); fclose(f); }
	} else if (kind == 1) {
		if (FILE *f = fopen(path, "wb")) { if (cap > 0) fwrite(bytes, 1, cap, f); fclose(f); }
		{
			XferLoad x;
			try { x.open(AsciiString(path)); x.xferSnapshot(s); } catch (XferStatus e) { st = (int)e; } catch (...) { st = -1; }
			try { x.close(); } catch (...) {}
		}
	} else {
		XferCRC x;
		try { x.open(AsciiString("crc")); x.xferSnapshot(s); x.close(); } catch (XferStatus e) { st = (int)e; } catch (...) { st = -1; }
		*crc = x.getCRC();
	}
	return st;
}

/* Construct the abstract base `cls` with module data of the same kind and null module data, and report what the
** constructor threw: 0 made, 2 INI_INVALID_DATA (the unnamed enum), anything else 5. cls: 0 ObjectModule,
** 1 DrawableModule, 2 BehaviorModule, 3 UpdateModule, 4 UpgradeModule, 5 DieModule, 6 CreateModule, 7 BodyModule,
** 8 DamageModule, 9 CollideModule, 10 DestroyModule. `null_data` 1 passes NULL. */
int ref_mod_construct(int cls, int null_data)
{
	Object *obj = newObject();
	ModuleData *d = 0;
	if (!null_data) {
		switch (cls) {
		case 3: d = UpdateModule::friend_newModuleData(0); break;
		case 4: d = UpgradeModule::friend_newModuleData(0); break;
		case 5: d = DieModule::friend_newModuleData(0); break;
		case 6: d = new CreateModuleData; break;
		case 7: d = new BodyModuleData; break;
		case 8: d = DamageModule::friend_newModuleData(0); break;
		case 9: d = CollideModule::friend_newModuleData(0); break;
		default: d = Module::friend_newModuleData(0); break;
		}
	}
	int status = 0;
	unsigned code = 0;
	try {
		switch (cls) {
		case 0: delete new TestObjectModule(obj, d); break;
		case 1: delete new TestDrawableModule(obj, d); break;
		case 2: delete new TestBehavior(obj, d); break;
		case 3: delete new TestUpdate(obj, d, PHASE_NORMAL); break;
		case 4: delete new TestUpgrade(obj, d); break;
		case 5: delete new TestDie(obj, d); break;
		case 6: delete new TestCreate(obj, d); break;
		case 7: delete new TestBody(obj, d); break;
		case 8: delete new TestDamage(obj, d); break;
		case 9: delete new TestCollide(obj, d); break;
		case 10: delete new TestDestroy(obj, d); break;
		}
	}
	catch (INIException &) { status = 1; }
	catch (ErrorCode) { status = 3; }
	catch (std::out_of_range &) { status = 4; }
	catch (...) { status = unnamedEnumStatus(&code); }
	delete d;
	delete obj;
	return status;
}

/* The xfer of the abstract base `cls` (as ref_mod_construct) made on module data of its kind: kind 0 save, 1 load,
** 2 CRC (see runXfer). For cls 6 (CreateModule) the state before is onBuildComplete() called `prep` times (0: still to
** run), for cls 7 (BodyModule) applyDamageScalar(prep_scale) once; the state after a load is dumped: for CreateModule
** shouldDoOnBuildComplete(), for BodyModule getDamageScalar() (bits), else nothing. */
int ref_mod_base_xfer(int cls, int prep, float prep_scale, int kind, const char *path, unsigned char *bytes, int cap, int *len,
	unsigned *crc, unsigned *state)
{
	Object *obj = newObject();
	ModuleData *d = 0;
	Module *m = 0;
	switch (cls) {
	case 0: d = Module::friend_newModuleData(0); m = new TestObjectModule(obj, d); break;
	case 1: d = Module::friend_newModuleData(0); m = new TestDrawableModule(obj, d); break;
	case 2: d = Module::friend_newModuleData(0); m = new TestBehavior(obj, d); break;
	case 3: d = UpdateModule::friend_newModuleData(0); m = new TestUpdate(obj, d, PHASE_NORMAL); break;
	case 4: d = UpgradeModule::friend_newModuleData(0); m = new TestUpgrade(obj, d); break;
	case 5: d = DieModule::friend_newModuleData(0); m = new TestDie(obj, d); break;
	case 6: d = new CreateModuleData; m = new TestCreate(obj, d); break;
	case 7: d = new BodyModuleData; m = new TestBody(obj, d); break;
	case 8: d = DamageModule::friend_newModuleData(0); m = new TestDamage(obj, d); break;
	case 9: d = CollideModule::friend_newModuleData(0); m = new TestCollide(obj, d); break;
	default: d = Module::friend_newModuleData(0); m = new TestDestroy(obj, d); break;
	}
	if (cls == 6 && prep) ((CreateModule *)m)->onBuildComplete();
	if (cls == 7) ((BodyModule *)m)->applyDamageScalar(prep_scale);
	*state = 0;
	int st = runXfer(m, kind, path, bytes, cap, len, crc);
	if (cls == 6) *state = ((CreateModule *)m)->shouldDoOnBuildComplete() ? 1 : 0;
	if (cls == 7) { Real r = ((BodyModule *)m)->getDamageScalar(); memcpy(state, &r, 4); }
	delete m;
	delete d;
	delete obj;
	return st;
}

/* BodyModule's defaults (a TestBody that overrides none of them): 20 values:
** getMaxHealth, getPreviousHealth, getSubdualDamageHealRate, getSubdualDamageHealAmount, hasAnySubdualDamage,
** getCurrentSubdualDamageAmount, getInitialHealth, getLastDamageInfo != NULL, getLastDamageTimestamp,
** getLastHealingTimestamp, getClearableLastAttacker, getFrontCrushed, getBackCrushed, isIndestructible (before and
** after setIndestructible(FALSE)), getDamageScalar, then the same getMaxHealth/getFrontCrushed after setMaxHealth,
** setFrontCrushed(TRUE), setInitialHealth. Reals as bits. */
int ref_mod_body_defaults(unsigned *out)
{
	Object *obj = newObject();
	ModuleData *d = new BodyModuleData;
	TestBody *b = new TestBody(obj, d);
	Real r;
	int n = 0;
#define R(x) do { r = (x); memcpy(&out[n++], &r, 4); } while (0)
#define U(x) do { out[n++] = (unsigned)(x); } while (0)
	R(b->getMaxHealth()); R(b->getPreviousHealth()); U(b->getSubdualDamageHealRate()); R(b->getSubdualDamageHealAmount());
	U(b->hasAnySubdualDamage()); R(b->getCurrentSubdualDamageAmount()); R(b->getInitialHealth());
	U(b->getLastDamageInfo() != 0); U(b->getLastDamageTimestamp()); U(b->getLastHealingTimestamp());
	U(b->getClearableLastAttacker()); U(b->getFrontCrushed()); U(b->getBackCrushed()); U(b->isIndestructible());
	b->setIndestructible(FALSE); U(b->isIndestructible()); R(b->getDamageScalar());
	b->setMaxHealth(10.0f, FULLY_HEAL); b->setInitialHealth(50);
	R(b->getMaxHealth()); b->setFrontCrushed(TRUE); U(b->getFrontCrushed());
#undef R
#undef U
	delete b;
	delete d;
	delete obj;
	return n;
}

/* CollideModule's defaults: wouldLikeToCollideWith(NULL), isHijackedVehicleCrateCollide, isSabotageBuildingCrateCollide,
** isCarBombCrateCollide, isRailroad, isSalvageCrateCollide. */
int ref_mod_collide_defaults(unsigned *out)
{
	Object *obj = newObject();
	ModuleData *d = CollideModule::friend_newModuleData(0);
	TestCollide *c = new TestCollide(obj, d);
	out[0] = c->wouldLikeToCollideWith(0); out[1] = c->isHijackedVehicleCrateCollide(); out[2] = c->isSabotageBuildingCrateCollide();
	out[3] = c->isCarBombCrateCollide(); out[4] = c->isRailroad(); out[5] = c->isSalvageCrateCollide();
	delete c;
	delete d;
	delete obj;
	return 6;
}

/* BodyDamageType / MaxHealthChangeType / IS_CONDITION_* : out = the enumerators, then IS_CONDITION_WORSE(a, b) and
** IS_CONDITION_BETTER(a, b) for a, b in 0..3 (32 values). */
int ref_mod_body_enums(int *out)
{
	int n = 0;
	out[n++] = BODY_PRISTINE; out[n++] = BODY_DAMAGED; out[n++] = BODY_REALLYDAMAGED; out[n++] = BODY_RUBBLE; out[n++] = BODYDAMAGETYPE_COUNT;
	out[n++] = SAME_CURRENTHEALTH; out[n++] = PRESERVE_RATIO; out[n++] = ADD_CURRENT_HEALTH_TOO; out[n++] = FULLY_HEAL;
	for (int a = 0; a < 4; ++a) for (int b = 0; b < 4; ++b) { out[n++] = IS_CONDITION_WORSE(a, b); out[n++] = IS_CONDITION_BETTER(a, b); }
	return n;
}

/* ---- UpdateModule ---------------------------------------------------------- */

/* A TestUpdate of the phase, and `n` ops of 6 words [op, a, b, c, d, e]:
**   0 friend_setNextCallFrame(a); out priority
**   1 out: friend_getPriority, friend_getNextCallFrame, friend_getNextCallPhase, friend_getIndexInLogic
**   2 frame = a; out getWakeFrame()
**   3 frame = a; out frameToSleepTime(b, c, d, e)
**   4 frame = a; setWakeFrame(obj, (UpdateSleepTime)b); out: the number of friend_awakenUpdateModule calls so far, the
**     frame it was last called with, whether it was given this object and this module
**   5 friend_setIndexInLogic((Int)a)
**   6 xfer save: out the bytes (length, then each byte as a word)
**   7 xfer load of [1, 1, 1, 1] + the word a; out: friend_getPriority, friend_getIndexInLogic
**   8 xfer CRC; out the CRC
**   9 xfer load of [1, 1, 1, 1] + a, where the saved update-module version byte is b (a version above 1 fails); out the XferStatus
** `path` is the file the xfer ops use. The results go to `out` as words; returns their number. */
int ref_mod_update_run(int phase, int n, const unsigned *ops, unsigned *out, int outCap, const char *path)
{
	Object *obj = newObject();
	ModuleData *d = UpdateModule::friend_newModuleData(0);
	TestUpdate *u = new TestUpdate(obj, d, (SleepyUpdatePhase)phase);
	GameLogic gl;
	TheGameLogic = &gl;
	int k = 0;
#define OUT(v) do { if (k < outCap) out[k] = (unsigned)(v); ++k; } while (0)
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		unsigned char bytes[64];
		int len;
		unsigned crc;
		switch (o[0]) {
		case 0: u->friend_setNextCallFrame(o[1]); OUT(u->friend_getPriority()); break;
		case 1: OUT(u->friend_getPriority()); OUT(u->friend_getNextCallFrame()); OUT(u->friend_getNextCallPhase()); OUT(u->friend_getIndexInLogic()); break;
		case 2: gl.m_frame = o[1]; OUT(u->getWakeFrame()); break;
		case 3: gl.m_frame = o[1]; OUT(u->frameToSleepTime(o[2], o[3], o[4], o[5])); break;
		case 4:
			gl.m_frame = o[1];
			u->setWakeFrame(obj, (UpdateSleepTime)o[2]);
			OUT(gl.m_awakenCalls); OUT(gl.m_awakenFrame); OUT(gl.m_awakenObject == obj); OUT(gl.m_awakenModule == u);
			break;
		case 5: u->friend_setIndexInLogic((Int)o[1]); break;
		case 6: {
			int st = runXfer(u, 0, path, bytes, 64, &len, &crc);
			OUT(st); OUT(len);
			for (int j = 0; j < len; ++j) OUT(bytes[j]);
			break;
		}
		case 7: case 9: {
			bytes[0] = (o[0] == 9) ? (unsigned char)o[2] : 1; bytes[1] = 1; bytes[2] = 1; bytes[3] = 1;
			memcpy(bytes + 4, &o[1], 4);
			int st = runXfer(u, 1, path, bytes, 8, &len, &crc);
			if (o[0] == 9) OUT(st);
			OUT(u->friend_getPriority()); OUT(u->friend_getIndexInLogic());
			break;
		}
		case 8: { int st = runXfer(u, 2, "crc", bytes, 0, &len, &crc); OUT(st); OUT(crc); break; }
		}
	}
#undef OUT
	TheGameLogic = 0;
	delete u;
	delete d;
	delete obj;
	return k;
}

/* ---- UpgradeModule / UpgradeMux --------------------------------------------- */

/* Read the module data of `filename` (the block `D`, UpgradeModuleData) and make a TestUpgrade on it; then `n` ops
** of 5 words [op, m0, m1, m2, m3] (the key mask, 32 bits per word; op 6's m0 indexes `names`):
**   0 wouldUpgrade(mask)   1 attemptUpgrade(mask)   2 testUpgradeConditions(mask)   3 resetUpgrade(mask)
**   4 forceRefreshUpgrade()   5 isAlreadyUpgraded()   6 isTriggeredBy(names[m0])
** Each op appends: the status of CATCH_ALL (0, or 2 for INI_INVALID_DATA, ...), the Bool result (0/1; 0 if it threw
** or has none), isAlreadyUpgraded(); the effect log of this op in the order it happened (count, then each entry as
** length + bytes: "impl", "fx<id>", "remove <name>"); then the UpgradeMuxData after it (putMux). The data-read status is the return value;
** nothing else is written if it failed. */
int ref_mod_upgrade_run(const char *filename, int n, const unsigned *ops, const char *const *names, unsigned char *buf, int cap, int *len,
	char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	UpgradeModuleData *d = 0;
	try { d = (UpgradeModuleData *)readData(DK_UPGRADE, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	Object *obj = newObject();
	TestUpgrade *m = new TestUpgrade(obj, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		g_events.clear();
		int st = 0, res = 0;
		unsigned c2 = 0;
		char m2[16];
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		try {
			switch (o[0]) {
			case 0: res = m->wouldUpgrade(mask) ? 1 : 0; break;
			case 1: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 2: res = m->testUpgradeConditions(mask) ? 1 : 0; break;
			case 3: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 4: m->forceRefreshUpgrade(); break;
			case 5: res = m->isAlreadyUpgraded() ? 1 : 0; break;
			case 6: res = m->isTriggeredBy(std::string(names[o[1]])) ? 1 : 0; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		g_dump.push_back((unsigned char)res);
		g_dump.push_back(m->isAlreadyUpgraded() ? 1 : 0);
		/* the effects, in the order they happened */
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			char b[32];
			if (e.kind == 0) log.push_back("impl");
			else if (e.kind == 1) { sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putMux(d->m_upgradeMuxData);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	return 0;
}

/* The xfer of that TestUpgrade (kind 0 save, 1 load, 2 CRC, see runXfer) after `attempts` successful ops: the module
** is made on data read from `filename`, `executed` sets m_upgradeExecuted directly. The state after: isAlreadyUpgraded(). */
int ref_mod_upgrade_xfer(const char *filename, int executed, int kind, const char *path, unsigned char *bytes, int cap, int *len,
	unsigned *crc, unsigned *state)
{
	int status = 0;
	unsigned code = 0;
	char msg[16];
	UpgradeModuleData *d = 0;
	try { d = (UpgradeModuleData *)readData(DK_UPGRADE, filename); }
	CATCH_ALL(status, &code, msg, 16)
	if (status) return -2;
	Object *obj = newObject();
	TestUpgrade *m = new TestUpgrade(obj, d);
	m->setUpgradeExecuted(executed != 0);
	int st = runXfer(m, kind, path, bytes, cap, len, crc);
	*state = m->isAlreadyUpgraded() ? 1 : 0;
	delete m;
	delete d;
	delete obj;
	return st;
}

/* ---- DieMuxData ----------------------------------------------------------- */

/* Read DieModuleData from `filename`, then for each of `n` queries [deathType, veterancyLevel, status mask words (2 x u32)]
** the DieModuleData::isDieApplicable(obj, damageInfo) of an Object of that veterancy and status, and DieModule::
** isDieApplicable through a TestDie. Writes one 0/1 per query to `out`. Returns the data-read status. */
int ref_mod_die_run(const char *filename, int n, const unsigned *queries, unsigned char *out, char *msg, int msgCap, unsigned *code)
{
	int status = 0;
	DieModuleData *d = 0;
	try { d = (DieModuleData *)readData(DK_DIE, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	Object *obj = newObject();
	TestDie *m = new TestDie(obj, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *q = queries + 4 * i;
		DamageInfo info;
		info.in.m_deathType = (DeathType)q[0];
		obj->m_level = (VeterancyLevel)q[1];
		obj->m_status = maskFromU32<ObjectStatusMaskType>(OBJECT_STATUS_COUNT, q + 2);
		bool a = d->isDieApplicable(obj, &info);
		bool b = m->isDieApplicable(&info);
		out[i] = (a ? 1 : 0) | (b ? 2 : 0);
	}
	delete m;
	delete d;
	delete obj;
	return 0;
}

}  /* extern "C" */

/* ---- layer 9: the concrete modules, one driver file per batch, each #including its upstream .cpp files ---- */
/* --- p1l9-example --- */
#include "l9_example.inc"
/* --- end p1l9-example --- */
/* --- p1w10-sonnet --- */
#include "l9_upgrades.inc"
#include "l9_upgrades_b.inc"
#include "l9_helpers.inc"
/* --- end p1w10-sonnet --- */
/* --- p1w11-sonnet --- */
#include "l9_upgrades_c.inc"
#include "l9_replace_object.inc"
#include "l9_dies_d.inc"
#include "l9_updates_e.inc"
#include "l9_crates_f.inc"
#include "l9_helpers_g.inc"
#include "l9_powers_h.inc"
#include "l9_updates_i.inc"
#include "l9_updates_l.inc"
#include "l9_dies_j.inc"
#include "l9_behaviors_k.inc"
/* --- end p1w11-sonnet --- */
/* --- p1w12-sonnet --- */
#include "l9_crash_trap_m.inc"
#include "l9_body_m.inc"
#include "l9_fire_ocl_m.inc"
#include "l9_auto_heal_m.inc"
#include "l9_checkpoint_n.inc"
#include "l9_flammable_n.inc"
#include "l9_ocl_update_n.inc"
/* --- end p1w12-sonnet --- */
/* --- p1w13-sonnet --- */
#include "l9_height_die_o.inc"
#include "l9_dynamic_shroud_o.inc"
#include "l9_squish_collide_o.inc"
#include "l9_prone_o.inc"
#include "l9_enemy_near_o.inc"
#include "l9_float_o.inc"
#include "l9_tech_building_o.inc"
#include "l9_fire_spread_o.inc"
#include "l9_active_body_p.inc"
#include "l9_slow_death_q.inc"
#include "l9_open_contain_r.inc"
#include "l9_sabotage_r13.inc"   /* p1w13-sonnet, batch R13: the eight sabotage crates */
#include "l9_crates_r13b.inc"   /* p1w13-sonnet, batch R13b: UnitCrateCollide, VeterancyCrateCollide */
#include "l9_crates_r13c.inc"   /* p1w13-sonnet, batch R13c: SalvageCrateCollide */
#include "l9_crates_r13d.inc"   /* p1w13-sonnet, batch R13d: ConvertToCarBombCrateCollide */
#include "l9_crates_r13e.inc"   /* p1w13-sonnet, batch R13e: ConvertToHijackedVehicleCrateCollide, HijackerUpdate */
#include "l9_dies_r14.inc"   /* p1w13-sonnet, batch R14: die modules */
#include "l9_hole_r15.inc"   /* p1w13-sonnet, batch R15: RebuildHoleExposeDie, RebuildHoleBehavior */
#include "l9_instant_death_r16.inc"   /* p1w13-sonnet, batch R16: InstantDeathBehavior */
#include "l9_poisoned_r16.inc"   /* p1w13-sonnet, batch R16: PoisonedBehavior */
#include "l9_crippling_r17.inc"   /* p1w13-sonnet, batch R17: SupplyWarehouseCripplingBehavior */
#include "l9_weapon_bonus_r17.inc"   /* p1w13-sonnet, batch R17: WeaponBonusUpdate */
#include "l9_exits_r18.inc"   /* p1w13-sonnet, batch R18: DefaultProductionExitUpdate, QueueProductionExitUpdate */
#include "l9_find_r19.inc"   /* p1w13-sonnet, batch R19: AutoFindHealingUpdate, PilotFindVehicleUpdate */
#include "l9_decal_steer_r20.inc"   /* p1w13-sonnet, batch R20: RadiusDecalUpdate, AnimationSteeringUpdate */
#include "l9_geom_stealth_r23.inc"   /* p1w13-sonnet, batch R23: DynamicGeometryInfoUpdate, GrantStealthBehavior */
#include "l9_overcharge_r24.inc"   /* p1w13-sonnet, batch R24: OverchargeBehavior */
#include "l9_horde_r25.inc"   /* p1w13-sonnet, batch R25: HordeUpdate */
#include "l9_stealth_r26.inc"   /* p1w13-sonnet, batch R26: StealthUpdate */
#include "l9_stealth_detector_r27.inc"   /* p1w13-sonnet, batch R27: StealthDetectorUpdate */
#include "l9_topple_r28.inc"   /* p1w13-sonnet, batch R28: ToppleUpdate */
#include "l9_collapse_r29.inc"   /* p1w13-sonnet, batch R29: StructureCollapseUpdate */
#include "l9_bonefx_r30.inc"   /* p1w13-sonnet, batch R30: BoneFXUpdate, BoneFXDamage */
#include "l9_transition_r31.inc"   /* p1w13-sonnet, batch R31: TransitionDamageFX */
#include "l9_firestorm_r32.inc"   /* p1w13-sonnet, batch R32: FirestormDynamicGeometryInfoUpdate */
#include "l9_emp_r33.inc"   /* p1w13-sonnet, batch R33: EMPUpdate, LeafletDropBehavior */
#include "l9_tensile_r34.inc"   /* p1w13-sonnet, batch R34: TensileFormationUpdate */
#include "l9_neutron_scaffold_r35.inc"   /* p1w13-sonnet, batch R35: NeutronBlastBehavior, BridgeScaffoldBehavior */
#include "l9_structure_topple_r36.inc"   /* p1w13-sonnet, batch R36: StructureToppleUpdate */
#include "l9_propaganda_r37.inc"   /* p1w13-sonnet, batch R37: PropagandaTowerBehavior */
#include "l9_bridge_tower_r38.inc"   /* p1w13-sonnet, batch R38: BridgeTowerBehavior */
#include "l9_sticky_bomb_r39.inc"   /* p1w13-sonnet, batch R39: StickyBombUpdate */
#include "l9_minefield_r40.inc"   /* p1w13-sonnet, batch R40: MinefieldBehavior */
#include "l9_gen_minefield_r41.inc"   /* p1w13-sonnet, batch R41: GenerateMinefieldBehavior */
#include "l9_projectile_stream_r42.inc"   /* p1w13-sonnet, batch R42: ProjectileStreamUpdate */
#include "l9_repair_dock_r43.inc"   /* p1w13-sonnet, batch R43: DockUpdate, RepairDockUpdate */
#include "l9_supply_docks_r44.inc"   /* p1w13-sonnet, batch R44: SupplyWarehouseDockUpdate, SupplyCenterDockUpdate */
#include "l9_ocl_power_r22.inc"   /* p1w13-sonnet, batch R22: OCLSpecialPower (run by l9_powers_h.inc, kind 7) */
/* --- end p1w13-sonnet --- */
