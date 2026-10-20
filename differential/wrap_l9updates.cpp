/* Differential-test wrapper for the layer-9 update modules of p1l9-updates (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/LifetimeUpdate.cpp   (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DeletionUpdate.cpp    (#included, unchanged)
** with the headers LifetimeUpdate.h and DeletionUpdate.h unchanged, in the module harness's setup
** (differential/wrap_module.cpp: clang++ -fms-extensions, module_prelude.h forced in, the same renames: the stand-ins
** of Object and GameLogic are ModTestObject and ModTestGameLogic). The base classes (UpdateModule, BehaviorModule,
** ModuleData, and the setWakeFrame that calls friend_awakenUpdateModule) are the ones wrap_module.cpp compiles, so the
** wake frames the modules set are recorded by the recorder of wrap_module's GameLogic stand-in (m_awakenCalls,
** m_awakenFrame, m_awakenObject, m_awakenModule).
**
** The stand-ins this file defines (declared in differential/shim_updates, which precedes shim_module on the include
** path; no data member is added to either class):
**   Object::isKindOf(KINDOF_HULK) answers the run's hulk flag; Object::kill() counts a kill;
**   GameLogic::destroyObject counts a destroy; GameLogic::getHulkMaxLifetimeOverride answers the run's override.
**
** The INI text of a run is read by the REAL INI (upstream_ini) as the block `D` of a file, through the friend_newModuleData
** of the class under test, as wrap_module.cpp's readData does. Data errors come back as the status of its CATCH_ALL.
**
** The program of a run: `n` ops of 6 words [op, a, b, c, d, e]. After every op the wrapper writes the common state
** block, then the op's own words:
**   common: the die frame, the kill count, the destroy count, the awake-call count, the frame of the last wake, whether
**   the last wake named this object and this module (1/0), the sleep time update() last returned, and the six words of
**   the game-logic random generator.
**   0 update(); out the sleep time it returned
**   1 setLifetimeRange(a, b); out nothing extra
**   2 gl.m_frame = a; out nothing extra
**   3 xfer save; out the XferStatus, the length and each byte (the bytes are kept for op 4)
**   4 xfer load of the bytes of the last save; out the XferStatus
**   5 xfer CRC; out the XferStatus and the CRC
** `kind`: 0 LifetimeUpdate, 1 DeletionUpdate. `xfer_path` is the file the xfer ops write (the caller's temporary file). Returns the status of the data read (0 ok, else CATCH_ALL's), with the
** number of words written in *outLen; nothing is run when the data read fails. */
#include "Common/INI.h"
#include "Common/INIException.h"
#include "Common/NameKeyGenerator.h"
#include "Common/XferCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/RandomValue.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/LifetimeUpdate.h"
#include "GameLogic/Module/DeletionUpdate.h"
#include "GameLogic/Module/BodyModule.h"
#include "GameLogic/Module/BaseRegenerateUpdate.h"
#include "GameLogic/Module/SmartBombTargetHomingUpdate.h"

/* the upstream files, unchanged */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/LifetimeUpdate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DeletionUpdate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BaseRenerateUpdate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SmartBombTargetHomingUpdate.cpp"

extern "C" void ref_ini_reset(void);
extern "C" void ref_ini_add_file(const char *name, const char *data, int len);
extern "C" void ref_get_state(int which, unsigned int *out6);
extern "C" void ref_init_game_logic_random(unsigned int seed);

/* ---- the run's world ------------------------------------------------------- */
namespace {
bool g_isHulk = false;
Int g_override = -1;
unsigned g_kills = 0;
unsigned g_destroys = 0;
}

Bool ModTestObject::isKindOf(KindOfType t) const { return t == KINDOF_HULK && g_isHulk; }
void ModTestObject::kill() { ++g_kills; }
void ModTestGameLogic::destroyObject(Object *) { ++g_destroys; }
Int ModTestGameLogic::getHulkMaxLifetimeOverride() const { return g_override; }

/* ---- BaseRegenerateUpdate's stand-ins: the body (a health, capped at its maximum by a heal, as ActiveBody caps it),
** the status bits (the object's own mask), and the heals the module asks for ---- */
namespace {
class RegenBody : public BodyModuleInterface {
public:
	RegenBody() : max(0), health(0) {}
	Real max;
	Real health;
	virtual void attemptDamage(DamageInfo *) {}
	virtual void attemptHealing(DamageInfo *) {}
	virtual Real estimateDamage(DamageInfoInput &) const { return 0; }
	virtual Real getHealth() const { return health; }
	virtual Real getMaxHealth() const { return max; }
	virtual Real getInitialHealth() const { return 0; }
	virtual Real getPreviousHealth() const { return 0; }
	virtual UnsignedInt getSubdualDamageHealRate() const { return 0; }
	virtual Real getSubdualDamageHealAmount() const { return 0; }
	virtual Bool hasAnySubdualDamage() const { return FALSE; }
	virtual Real getCurrentSubdualDamageAmount() const { return 0; }
	virtual BodyDamageType getDamageState() const { return BODY_PRISTINE; }
	virtual void setDamageState(BodyDamageType) {}
	virtual void setAflame(Bool) {}
	virtual void onVeterancyLevelChanged(VeterancyLevel, VeterancyLevel, Bool) {}
	virtual void setArmorSetFlag(ArmorSetType) {}
	virtual void clearArmorSetFlag(ArmorSetType) {}
	virtual Bool testArmorSetFlag(ArmorSetType) { return FALSE; }
	virtual const DamageInfo *getLastDamageInfo() const { return 0; }
	virtual UnsignedInt getLastDamageTimestamp() const { return 0; }
	virtual UnsignedInt getLastHealingTimestamp() const { return 0; }
	virtual ObjectID getClearableLastAttacker() const { return (ObjectID)0; }
	virtual void clearLastAttacker() {}
	virtual Bool getFrontCrushed() const { return FALSE; }
	virtual Bool getBackCrushed() const { return FALSE; }
	virtual void setInitialHealth(Int) {}
	virtual void setMaxHealth(Real, MaxHealthChangeType) {}
	virtual void setFrontCrushed(Bool) {}
	virtual void setBackCrushed(Bool) {}
	virtual void applyDamageScalar(Real) {}
	virtual Real getDamageScalar() const { return 1; }
	virtual void internalChangeHealth(Real) {}
	virtual void setIndestructible(Bool) {}
	virtual Bool isIndestructible() const { return FALSE; }
	virtual void evaluateVisualCondition() {}
	virtual void updateBodyParticleSystems() {}
};
RegenBody g_body;
std::vector<Real> g_heals;
}
GlobalData g_gd;
GlobalData *TheGlobalData = &g_gd;
Bool ModTestObject::testStatus(ObjectStatusTypes bit) const { return m_status.test(bit); }
/* SmartBombTargetHomingUpdate's position stand-ins: the position and the terrain answer of the run's one object */
namespace {
Coord3D g_pos;
Bool g_above = FALSE;
}
const Coord3D *ModTestObject::getPosition() const { return &g_pos; }
void ModTestObject::setPosition(const Coord3D *pos) { g_pos = *pos; }
Bool ModTestObject::isSignificantlyAboveTerrain() const { return g_above; }
BodyModuleInterface *ModTestObject::getBodyModule() { return &g_body; }
void ModTestObject::attemptHealing(Real amount, const ModTestObject *)
{
	g_heals.push_back(amount);
	g_body.health += amount;
	if (g_body.health > g_body.max) g_body.health = g_body.max;
}


/* the xfer runner of wrap_module.cpp (same behaviour): 0 save to `path` and copy its bytes out, 1 write `bytes` to
** `path` and load it, 2 CRC. Returns the XferStatus thrown (0 none, -1 anything else). */
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

#include <vector>
namespace {

std::vector<unsigned char> g_saved;

/* the module data of a run: the block `D` of the file, read as the class's friend_newModuleData does */
enum { KIND_LIFETIME = 0, KIND_DELETION = 1 };

ModuleData *readLifetimeData(const char *filename, int kind)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	ini.readLine();
	AsciiString currentLine = ini.m_buffer;
	try {
		data = kind == KIND_LIFETIME ? (ModuleData *)LifetimeUpdate::friend_newModuleData(&ini)
		                             : (ModuleData *)DeletionUpdate::friend_newModuleData(&ini);
	} catch (...) {
		char buff[1024];
		sprintf(buff, "Error parsing INI file '%s' (Line: '%s')\n", ini.m_filename.str(), currentLine.str());
		throw INIException(buff);
	}
	return data;
}

}  // namespace

extern "C" {

/* ref_l9updates_run: see the header. `ini_text` is the file of the run (the block `D`, then the fields and End). */
int ref_l9updates_run(int kind, const char *ini_text, int ini_len, unsigned seed, unsigned frame, int hulk, int override,
                      int n, const unsigned *ops, unsigned *out, int outCap, int *outLen, int *status, unsigned *code,
                      char *msg, int msgCap, const char *xfer_path)
{
	int k = 0;
	*outLen = 0;
	*status = 0;
	*code = 0;
	if (msgCap > 0) msg[0] = 0;
	ref_ini_reset();
	ref_ini_add_file("l9updates.ini", ini_text, ini_len);
	ref_init_game_logic_random(seed);
	g_saved.clear();
	g_isHulk = hulk != 0;
	g_override = override;
	g_kills = 0;
	g_destroys = 0;
	ModuleData *d = 0;
	try {
		d = readLifetimeData("l9updates.ini", kind);
	} catch (INIException &e) {
		*status = 1;
		const char *m = e.mFailureMessage ? e.mFailureMessage : "";
		int len = (int)strlen(m);
		*code = (unsigned)len;
		if (len + 1 <= msgCap) memcpy(msg, m, len + 1);
		return 1;
	} catch (...) {
		*status = 5;
		return 5;
	}
	GameLogic gl;
	gl.m_frame = frame;
	ModTestTheGameLogic = &gl;
	Object *obj = new Object;
	const bool lifetime = kind == KIND_LIFETIME;
	/* the constructor: its wake is recorded by the wrap_module GameLogic (friend_awakenUpdateModule) */
	LifetimeUpdate *lu = 0;
	DeletionUpdate *du = 0;
	UpdateModule *u = 0;
	if (lifetime) {
		lu = new LifetimeUpdate(obj, d);
		u = lu;
	} else {
		du = new DeletionUpdate(obj, d);
		u = du;
	}
	UpdateSleepTime lastSleep = UPDATE_SLEEP_NONE;
#define OUT(v) do { if (k < outCap) out[k] = (unsigned)(v); ++k; } while (0)
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		unsigned char buf[256];
		int len = 0;
		unsigned crc = 0;
		int st = 0;
		switch (o[0]) {
		case 0:
			lastSleep = lifetime ? lu->update() : du->update();
			break;
		case 1:
			if (lifetime) lu->setLifetimeRange((UnsignedInt)o[1], (UnsignedInt)o[2]);
			else du->setLifetimeRange((UnsignedInt)o[1], (UnsignedInt)o[2]);
			break;
		case 2:
			gl.m_frame = o[1];
			break;
		}
		/* the common state block */
		OUT(lifetime ? lu->getDieFrame() : du->getDieFrame());
		OUT(g_kills);
		OUT(g_destroys);
		OUT(gl.m_awakenCalls);
		OUT(gl.m_awakenFrame);
		OUT(gl.m_awakenObject == obj);
		OUT(gl.m_awakenModule == u);
		OUT((unsigned)lastSleep);
		unsigned rng[6];
		ref_get_state(0, rng);
		for (int j = 0; j < 6; ++j) OUT(rng[j]);
		/* op-specific words */
		if (o[0] == 3) {
			st = runXfer(u, 0, xfer_path, buf, 256, &len, &crc);
			g_saved.assign(buf, buf + len);
			OUT(st); OUT(len);
			for (int j = 0; j < len; ++j) OUT(buf[j]);
		} else if (o[0] == 4) {
			std::vector<unsigned char> b = g_saved;
			if (b.size() > 256) b.resize(256);
			st = runXfer(u, 1, xfer_path, b.empty() ? buf : &b[0], (int)b.size(), &len, &crc);
			OUT(st);
		} else if (o[0] == 5) {
			st = runXfer(u, 2, xfer_path, buf, 0, &len, &crc);
			OUT(st); OUT(crc);
		}
	}
#undef OUT
	ModTestTheGameLogic = 0;
	*outLen = k;
	return 0;
}

}  /* extern "C" */

/* ---- BaseRegenerateUpdate -------------------------------------------------- */

static unsigned bitsOf(Real r)
{
	unsigned b = 0;
	memcpy(&b, &r, 4);
	return b;
}

/* The module data of the block `D` (BaseRegenerateUpdate::friend_newModuleData: the base's fields and an empty table). */
static ModuleData *readRegenData(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ini.readLine();
	AsciiString currentLine = ini.m_buffer;
	try {
		return (ModuleData *)BaseRegenerateUpdate::friend_newModuleData(&ini);
	} catch (...) {
		char buff[1024];
		sprintf(buff, "Error parsing INI file '%s' (Line: '%s')\n", ini.m_filename.str(), currentLine.str());
		throw INIException(buff);
	}
}

extern "C" {

/* ref_l9updates_regen_run: a BaseRegenerateUpdate built from the INI text, with the global data `pct` (a float) and
** `delay` (frames), then `n` ops of 6 words:
**   0 update(); out the sleep it returned
**   1 onDamage with m_damageType = a
**   2 gl.m_frame = a
**   3 the status bits of the object: bit 0 OBJECT_STATUS_UNDER_CONSTRUCTION = a & 1, bit 1 OBJECT_STATUS_SOLD = a & 2
**   4 the body: maximum health = a, health = b (both below 2^24, exact as floats)
**   5 xfer save (out: status, length, the bytes); 6 xfer load of the last save (out: status); 7 xfer CRC (out: status, CRC)
** After every op the common block: the wake calls so far, the frame of the last, whether the last named this object and
** this module, the last sleep, the number of heals, the bits of the last heal (0 if none), the body's health bits, and the
** six words of the game-logic random generator.
** `status`: 0 ok, 1 the INI refused the block (the message in msg). Returns 0. */
int ref_l9updates_regen_run(const char *ini_text, int ini_len, float pct, unsigned delay, unsigned seed, unsigned frame, int n,
                            const unsigned *ops, unsigned *out, int outCap, int *outLen, int *status, unsigned *code, char *msg,
                            int msgCap, const char *xfer_path)
{
	int k = 0;
	*outLen = 0;
	*status = 0;
	*code = 0;
	if (msgCap > 0) msg[0] = 0;
	ref_ini_reset();
	ref_ini_add_file("l9updates.ini", ini_text, ini_len);
	ref_init_game_logic_random(seed);
	g_saved.clear();
	g_heals.clear();
	g_body.max = 0;
	g_body.health = 0;
	g_gd.m_baseRegenHealthPercentPerSecond = pct;
	g_gd.m_baseRegenDelay = delay;
	ModuleData *d = 0;
	try {
		d = readRegenData("l9updates.ini");
	} catch (INIException &e) {
		*status = 1;
		const char *m = e.mFailureMessage ? e.mFailureMessage : "";
		int len = (int)strlen(m);
		*code = (unsigned)len;
		if (len + 1 <= msgCap) memcpy(msg, m, len + 1);
		return 0;
	}
	GameLogic gl;
	gl.m_frame = frame;
	ModTestTheGameLogic = &gl;
	Object *obj = new Object;
	obj->m_status.clear();
	BaseRegenerateUpdate *br = new BaseRegenerateUpdate(obj, d);
	UpdateSleepTime lastSleep = UPDATE_SLEEP_NONE;
#define OUT(v) do { if (k < outCap) out[k] = (unsigned)(v); ++k; } while (0)
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		unsigned char buf[256];
		int len = 0;
		unsigned crc = 0;
		int st = 0;
		switch (o[0]) {
		case 0:
			lastSleep = br->update();
			break;
		case 1: {
			DamageInfo info;
			info.in.m_damageType = (DamageType)o[1];
			br->onDamage(&info);
			break;
		}
		case 2:
			gl.m_frame = o[1];
			break;
		case 3:
			obj->m_status.clear();
			if (o[1] & 1) obj->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
			if (o[1] & 2) obj->m_status.set(OBJECT_STATUS_SOLD);
			break;
		case 4:
			g_body.max = (Real)o[1];
			g_body.health = (Real)o[2];
			break;
		}
		/* the common state block */
		OUT(gl.m_awakenCalls);
		OUT(gl.m_awakenFrame);
		OUT(gl.m_awakenObject == obj);
		OUT(gl.m_awakenModule == br);
		OUT((unsigned)lastSleep);
		OUT(g_heals.size());
		OUT(g_heals.empty() ? 0u : bitsOf(g_heals.back()));
		unsigned hb = 0;
		memcpy(&hb, &g_body.health, 4);
		OUT(hb);
		unsigned rng[6];
		ref_get_state(0, rng);
		for (int j = 0; j < 6; ++j) OUT(rng[j]);
		/* op-specific words */
		if (o[0] == 5) {
			st = runXfer(br, 0, xfer_path, buf, 256, &len, &crc);
			g_saved.assign(buf, buf + len);
			OUT(st); OUT(len);
			for (int j = 0; j < len; ++j) OUT(buf[j]);
		} else if (o[0] == 6) {
			std::vector<unsigned char> b = g_saved;
			if (b.size() > 256) b.resize(256);
			st = runXfer(br, 1, xfer_path, b.empty() ? buf : &b[0], (int)b.size(), &len, &crc);
			OUT(st);
		} else if (o[0] == 7) {
			st = runXfer(br, 2, xfer_path, buf, 0, &len, &crc);
			OUT(st); OUT(crc);
		}
	}
#undef OUT
	ModTestTheGameLogic = 0;
	*outLen = k;
	return 0;
}

}  /* extern "C" */

/* ---- SmartBombTargetHomingUpdate ------------------------------------------- */

static Real floatOfBits(unsigned b)
{
	Real r;
	memcpy(&r, &b, 4);
	return r;
}

static ModuleData *readSmartData(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ini.readLine();
	AsciiString currentLine = ini.m_buffer;
	try {
		return (ModuleData *)SmartBombTargetHomingUpdate::friend_newModuleData(&ini);
	} catch (...) {
		char buff[1024];
		sprintf(buff, "Error parsing INI file '%s' (Line: '%s')\n", ini.m_filename.str(), currentLine.str());
		throw INIException(buff);
	}
}

extern "C" {

/* ref_l9updates_smart_run: a SmartBombTargetHomingUpdate built from the INI text, then `n` ops of 6 words; the float
** arguments are given as their bits:
**   0 update(); out the sleep it returned
**   1 SetTargetPosition((Coord3D){a, b, c})
**   2 gl.m_frame = a
**   3 isSignificantlyAboveTerrain = a & 1
**   4 setPosition((Coord3D){a, b, c}) (the object's position, as the test sets it)
**   5 xfer save (out: status, length, the bytes); 6 xfer load of the last save (out: status); 7 xfer CRC (out: status, CRC)
** After every op: the wake calls so far, the frame of the last, whether the last named this object and this module, the
** last sleep, whether a target was received, the target's and the position's bits (x, y, z), and the six random words.
** `status`: 0 ok, 1 the INI refused the block (the message in msg). Returns 0. */
int ref_l9updates_smart_run(const char *ini_text, int ini_len, unsigned seed, unsigned frame, int n, const unsigned *ops,
                            unsigned *out, int outCap, int *outLen, int *status, unsigned *code, char *msg, int msgCap,
                            const char *xfer_path)
{
	int k = 0;
	*outLen = 0;
	*status = 0;
	*code = 0;
	if (msgCap > 0) msg[0] = 0;
	ref_ini_reset();
	ref_ini_add_file("l9updates.ini", ini_text, ini_len);
	ref_init_game_logic_random(seed);
	g_saved.clear();
	g_pos.zero();
	g_above = FALSE;
	ModuleData *d = 0;
	try {
		d = readSmartData("l9updates.ini");
	} catch (INIException &e) {
		*status = 1;
		const char *m = e.mFailureMessage ? e.mFailureMessage : "";
		int len = (int)strlen(m);
		*code = (unsigned)len;
		if (len + 1 <= msgCap) memcpy(msg, m, len + 1);
		return 0;
	}
	GameLogic gl;
	gl.m_frame = frame;
	ModTestTheGameLogic = &gl;
	Object *obj = new Object;
	SmartBombTargetHomingUpdate *sb = new SmartBombTargetHomingUpdate(obj, d);
	UpdateSleepTime lastSleep = UPDATE_SLEEP_NONE;
#define OUT(v) do { if (k < outCap) out[k] = (unsigned)(v); ++k; } while (0)
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		unsigned char buf[256];
		int len = 0;
		unsigned crc = 0;
		int st = 0;
		switch (o[0]) {
		case 0:
			lastSleep = sb->update();
			break;
		case 1: {
			Coord3D t;
			t.x = floatOfBits(o[1]);
			t.y = floatOfBits(o[2]);
			t.z = floatOfBits(o[3]);
			sb->SetTargetPosition(t);
			break;
		}
		case 2:
			gl.m_frame = o[1];
			break;
		case 3:
			g_above = (o[1] & 1) != 0;
			break;
		case 4:
			g_pos.x = floatOfBits(o[1]);
			g_pos.y = floatOfBits(o[2]);
			g_pos.z = floatOfBits(o[3]);
			break;
		}
		/* the common state block */
		OUT(gl.m_awakenCalls);
		OUT(gl.m_awakenFrame);
		OUT(gl.m_awakenObject == obj);
		OUT(gl.m_awakenModule == sb);
		OUT((unsigned)lastSleep);
		OUT(sb->m_targetReceived);
		OUT(bitsOf(sb->m_target.x)); OUT(bitsOf(sb->m_target.y)); OUT(bitsOf(sb->m_target.z));
		OUT(bitsOf(g_pos.x)); OUT(bitsOf(g_pos.y)); OUT(bitsOf(g_pos.z));
		unsigned rng[6];
		ref_get_state(0, rng);
		for (int j = 0; j < 6; ++j) OUT(rng[j]);
		/* op-specific words */
		if (o[0] == 5) {
			st = runXfer(sb, 0, xfer_path, buf, 256, &len, &crc);
			g_saved.assign(buf, buf + len);
			OUT(st); OUT(len);
			for (int j = 0; j < len; ++j) OUT(buf[j]);
		} else if (o[0] == 6) {
			std::vector<unsigned char> b = g_saved;
			if (b.size() > 256) b.resize(256);
			st = runXfer(sb, 1, xfer_path, b.empty() ? buf : &b[0], (int)b.size(), &len, &crc);
			OUT(st);
		} else if (o[0] == 7) {
			st = runXfer(sb, 2, xfer_path, buf, 0, &len, &crc);
			OUT(st); OUT(crc);
		}
	}
#undef OUT
	ModTestTheGameLogic = 0;
	*outLen = k;
	return 0;
}

}  /* extern "C" */
