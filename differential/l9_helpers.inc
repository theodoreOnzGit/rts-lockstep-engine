/* Differential-test driver for the layer-9 helper and die modules of p1l9-helpers (upstream commit 0a05454d), verified by p1w10-sonnet:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectHelper.cpp, ObjectSMCHelper.cpp, ObjectRepulsorHelper.cpp,
**   ObjectWeaponStatusHelper.cpp, StatusDamageHelper.cpp, TempWeaponBonusHelper.cpp and Die/DestroyDie.cpp, KeepObjectDie.cpp, UpgradeDie.cpp
**   (#included, unchanged)
** with their headers unchanged, in wrap_module.cpp's unit (it is #included at that file's end, in its marked block) on the stand-ins of
** differential/shim_module (an Object whose status mask is real and whose helper calls are lines of a log; a Drawable that logs its tint; a
** GameLogic whose destroyObject and findObjectByID go to the hooks below) and the REAL UpgradeCenter, INI, UpdateModule, DieModule. */
/* REAL_TO_INT_FLOOR is an MSVC __asm helper upstream; here, as in the weapon and build-assistant builds, the transcription of base_type's */
extern "C" int ref_fast_float2long_round(float f);
extern "C" float ref_fast_float_floor(float f);
#define REAL_TO_INT_FLOOR(x) (ref_fast_float2long_round(ref_fast_float_floor(x)))
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectHelper.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectSMCHelper.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectRepulsorHelper.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectWeaponStatusHelper.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/StatusDamageHelper.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/TempWeaponBonusHelper.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/DestroyDie.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/KeepObjectDie.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/UpgradeDie.cpp"

namespace {
/* the log of one op: what the modules did to their object, as (kind, argument) pairs, in order:
**   1 setStatus(bit)  2 clearStatus(bit)  3 clearSpecialModelConditionStates  4 adjustModelConditionForWeaponStatus
**   5 setWeaponBonusCondition(b)  6 clearWeaponBonusCondition(b)  7 setTintStatus(t)  8 clearTintStatus(t)  9 destroyObject
**   10 removeUpgrade (the number N of the template "UpN") */
std::vector<std::pair<int, unsigned> > g_hlog;
Object *g_producer = 0;   /* findObjectByID answers this, or NULL */
Object *g_destroyed = 0;
void hlog(int k, unsigned a) { g_hlog.push_back(std::make_pair(k, a)); }
unsigned firstBit(const ObjectStatusMaskType &m)
{
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (m.test(i)) return (unsigned)i;
	return 0xffffu;
}
}  // namespace

void modhelp_bonus(int b) { hlog(5, (unsigned)b); }
void modhelp_destroy(Object *obj) { g_destroyed = obj; hlog(9, 0); }
/* --- p1w11-sonnet --- die batch D answers findObjectByID by id (l9_dies_d.inc sets the hook); the helper batch's tests leave it null */
Object *(*g_modFindHook)(ObjectID) = 0;
Object *modhelp_find(ObjectID id) { return g_modFindHook ? g_modFindHook(id) : g_producer; }
/* --- end p1w11-sonnet --- */
/* --- p1w13-sonnet --- (batch P: ActiveBody logs the status it sets; null for the other drivers) */
void (*g_setStatusHook)(Object *, ObjectStatusMaskType, Bool) = 0;
/* --- end p1w13-sonnet --- */
void Object::setStatus(ObjectStatusMaskType objectStatus, Bool set)
{
	if (g_setStatusHook) g_setStatusHook(this, objectStatus, set);
	/* Object.cpp's setStatus does more (it reacts to a few of the bits); the helpers only need the bits */
	hlog(set ? 1 : 2, firstBit(objectStatus));
	if (set) m_status.set(objectStatus); else m_status.clear(objectStatus);
}
void Object::clearSpecialModelConditionStates() { hlog(3, 0); }
void Object::adjustModelConditionForWeaponStatus() { hlog(4, 0); }
void Object::clearWeaponBonusCondition(WeaponBonusConditionType wst) { hlog(6, (unsigned)wst); }
void Drawable::setTintStatus(TintStatus statusBits) { hlog(7, (unsigned)statusBits); }
void Drawable::clearTintStatus(TintStatus statusBits) { hlog(8, (unsigned)statusBits); }
/* Object.cpp:4456, verbatim */
Bool Object::hasUpgrade( const UpgradeTemplate *upgradeT ) const
{
	if( m_objectUpgradesCompleted.testForAll( upgradeT->getUpgradeMask() ) )
	{
		return TRUE;
	}
	return FALSE;
}

namespace {
ModuleData *helperRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = kind == 7 ? (ModuleData *)UpgradeDie::friend_newModuleData(&ini) : (ModuleData *)DieModule::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_hsaved;
}  // namespace

extern "C" {

/* Make the module `kind` (0 ObjectSMCHelper, 1 ObjectRepulsorHelper, 2 ObjectWeaponStatusHelper, 3 StatusDamageHelper, 4 TempWeaponBonusHelper,
** 5 DestroyDie, 6 KeepObjectDie, 7 UpgradeDie) on an Object and run `n` ops of 6 words [op, a, b, c, d, e]; the dies' data is the block `D` of
** `filename` (ignored for the helpers). The upgrades "Up0".. are the REAL upgrade center's, loaded by the caller. The dump, after the data (a
** die's mux, an UpgradeDie's name), then per op: the CATCH_ALL status, the wake record (calls, last frame, this object, this module), the last
** sleep time, the log (count, then pairs), the object's status mask (4 words of 64 bits), the producer's upgrade mask, and the module's words.
** Helper ops: 0 update()  1 doStatusDamage(status a, duration bits b) / doTempWeaponBonus(bonus a, duration b)  2 frame = a  3 destroyed bit =
** a&1, drawable present = a&2 (the status bit is toggled in the object's mask)  4 sleepUntil(a)  5 xfer save  6 xfer load of the last save
** 7 xfer CRC  8 friend_setNextCallFrame-like: friend_setNextCallFrame(a)  9 object status bit a set (a < OBJECT_STATUS_COUNT) / cleared (b&1)
** Die ops: 0 onDie(death type a, veterancy level b, status words c,d)  1 producer present = a&1, its upgrade mask = bits b..e  2 xfer save
** 3 xfer load  4 xfer CRC  5 the object's id (m_producerID) = a. */
int ref_l9help_run(int kind, const char *filename, const char *xfer_path, int n, const unsigned *ops, unsigned char *buf, int cap, int *len,
	char *msg, int msgCap, unsigned *code)
{
	initDisabledMasks();   /* DISABLEDMASK_ALL, which GameLogic::init makes before any module is built (DisabledTypes.cpp, in the flags build) */
	g_dump.clear();
	g_hlog.clear();
	g_hsaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	if (kind >= 5) {
		try { d = helperRead(kind, filename); }
		CATCH_ALL(status, code, msg, msgCap)
		if (status) return status;
		putDieMux(((DieModuleData *)d)->m_dieMuxData);
		if (kind == 7) putAscii(((UpgradeDieModuleData *)d)->m_upgradeName);
	} else {
		static ModuleData stand_in_data;
		d = &stand_in_data;
	}
	GameLogic gl;
	TheGameLogic = &gl;
	Object *obj = newObject();
	obj->m_drawable = 0;
	Object *producer = new Object;
	g_producer = 0;
	g_destroyed = 0;
	Drawable draw;
	Module *m = 0;
	UpdateModule *um = 0;
	switch (kind) {
	case 0: um = new ObjectSMCHelper(obj, d); break;
	case 1: um = new ObjectRepulsorHelper(obj, d); break;
	case 2: um = new ObjectWeaponStatusHelper(obj, d); break;
	case 3: um = new StatusDamageHelper(obj, d); break;
	case 4: um = new TempWeaponBonusHelper(obj, d); break;
	case 5: m = new DestroyDie(obj, d); break;
	case 6: m = new KeepObjectDie(obj, d); break;
	default: m = new UpgradeDie(obj, d); break;
	}
	if (um) m = um;
	UpdateSleepTime lastSleep = UPDATE_SLEEP_NONE;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		g_hlog.clear();
		g_events.clear();
		int st = 0;
		char m2[16];
		unsigned c2 = 0;
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		try {
			if (kind < 5) {
				switch (o[0]) {
				case 0: lastSleep = um->update(); break;
				case 1:
					if (kind == 3) ((StatusDamageHelper *)um)->doStatusDamage((ObjectStatusTypes)o[1], *(const Real *)&o[2]);
					else if (kind == 4) ((TempWeaponBonusHelper *)um)->doTempWeaponBonus((WeaponBonusConditionType)(int)o[1], o[2]);
					break;
				case 2: gl.m_frame = o[1]; break;
				case 3:
					if (o[1] & 1) obj->m_status.set(OBJECT_STATUS_DESTROYED); else obj->m_status.clear(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_DESTROYED));
					obj->m_drawable = (o[1] & 2) ? &draw : 0;
					break;
				case 4: ((ObjectHelper *)um)->sleepUntil(o[1]); break;
				case 5: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_hsaved.assign(bytes, bytes + blen); break;
				case 6: {
					std::vector<unsigned char> b = g_hsaved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				case 7: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
				case 8: um->friend_setNextCallFrame(o[1]); break;
				case 9:
					if (o[1] < (unsigned)OBJECT_STATUS_COUNT) { if (o[2] & 1) obj->m_status.set((int)o[1]); else obj->m_status.clear(MAKE_OBJECT_STATUS_MASK((int)o[1])); }
					break;
				}
			} else {
				switch (o[0]) {
				case 0: {
					DamageInfo info;
					info.in.m_deathType = (DeathType)o[1];
					obj->m_level = (VeterancyLevel)o[2];
					obj->m_status = maskFromU32<ObjectStatusMaskType>(OBJECT_STATUS_COUNT, o + 3);
					dynamic_cast<DieModuleInterface *>(m)->onDie(&info);
					break;
				}
				case 1:
					if (o[1] & 1) {
						producer->m_objectUpgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 2);
						g_producer = producer;
					} else g_producer = 0;
					break;
				case 2: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_hsaved.assign(bytes, bytes + blen); break;
				case 3: {
					std::vector<unsigned char> b = g_hsaved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				case 4: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
				case 5: obj->m_producerID = (ObjectID)o[1]; break;
				}
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		for (size_t j = 0; j < g_events.size(); ++j) {
			if (g_events[j].kind != 2) continue;
			const char *s = g_events[j].p ? ((const UpgradeTemplate *)g_events[j].p)->getUpgradeName().str() : "";
			while (*s && (*s < '0' || *s > '9')) ++s;
			hlog(10, (unsigned)atoi(s));
		}
		putI32(st);
		putI32(gl.m_awakenCalls);
		putU32(gl.m_awakenFrame);
		putU32(gl.m_awakenObject == obj ? 1 : 0);
		putU32(gl.m_awakenModule == um ? 1 : 0);
		putU32((unsigned)lastSleep);
		putU32((unsigned)g_hlog.size());
		for (size_t j = 0; j < g_hlog.size(); ++j) { putI32(g_hlog[j].first); putU32(g_hlog[j].second); }
		putStatusMask(obj->m_status);
		putUpgradeMask(producer->m_objectUpgradesCompleted);
		putI32(xst);
		putU32((unsigned)blen);
		if ((kind < 5 && o[0] == 5) || (kind >= 5 && o[0] == 2)) for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		if (um) {
			putU32(um->m_nextCallFrameAndPhase);
			putI32(um->m_indexInLogic);
			putU32((unsigned)um->getUpdatePhase());
			putU32((unsigned)um->getDisabledTypesToProcess().count());
		}
		if (kind == 3) { putI32((int)((StatusDamageHelper *)um)->m_statusToHeal); putU32(((StatusDamageHelper *)um)->m_frameToHeal); }
		if (kind == 4) { putI32((int)((TempWeaponBonusHelper *)um)->m_currentBonus); putU32(((TempWeaponBonusHelper *)um)->m_frameToRemove); }
		putU32(g_destroyed == obj ? 1 : 0);
	}
	*len = finish(buf, cap);
	delete m;
	if (kind >= 5) delete d;
	delete obj;
	delete producer;
	TheGameLogic = 0;
	g_producer = 0;
	return 0;
}

}  /* extern "C" */
