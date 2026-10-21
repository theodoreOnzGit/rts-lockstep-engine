/* Differential-test wrapper for Upgrade and SpecialPower (upstream commit
** 0a05454d), p1w2-upgrade:
**   GeneralsMD/Code/GameEngine/Source/Common/System/Upgrade.cpp     (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/Common/INI/INIUpgrade.cpp     (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/SpecialPower.cpp   (#included from a copy: build.rs renames
**       the one definition `SpecialPowerMaskType::s_bitNameList` to `UpSpecialPowerRealNames`, asserted once,
**       because the BitFlags build already defines that member and the Enum field must keep using it; the
**       copy is dumped here so that the Rust list is checked against the real file's text)
**   GeneralsMD/Code/GameEngine/Source/Common/INI/INISpecialPower.cpp (#included, unchanged)
** with the headers Upgrade.h, SpecialPower.h, Overridable.h, Override.h, INI.h and
** NameKeyGenerator.h unchanged, in the BitFlags build's setup (clang++ -fms-extensions, the real
** BitFlagsIO.h) plus the stand-ins of differential/shim_upgrade: AudioEventRTS (keeps its name),
** Player, Object, InGameUI and Image (Player-taking and client code that the tests never reach).
**
** Renaming. The upstream text is included here (as wrap_money.cpp does) under macros that rename the
** classes and globals the Xfer build also stands in for (UpgradeCenter, UpgradeTemplate,
** TheUpgradeCenter: differential/wrap_xfer.cpp) or the INI build defines (SpecialPowerStore,
** TheSpecialPowerStore: wrap_ini.cpp), so that one test binary links them all; the upstream text is
** unchanged, only these names differ. AudioEventRTS and ScienceStore keep their names: the INI build
** (INI.cpp, upstream_ini) calls AudioEventRTS::setEventName, AudioManager::getInfoForAudioEvent and
** ScienceStore::friend_lookupScience by these names, and wrap_ini.cpp's aborting stand-ins for the three
** are WEAK so that the definitions below take their place.
**
** INI: the blocks are read by the REAL INI::load (upstream_ini) with the REAL INI::parseUpgradeDefinition
** and INI::parseSpecialPowerDefinition below (wrap_ini.cpp's stubs of them are WEAK), in the load type
** the test asks for (ref_up_load; wrap_ini.cpp's ref_ini_load always overwrites). The files come from
** wrap_ini.cpp's in-memory file system (ref_ini_reset, ref_ini_add_file).
**
** Science. INI::parseScience needs TheScienceStore and ScienceStore::friend_lookupScience, which is
** defined here as upstream's Science.cpp does it (NAMEKEY of the name, INI_INVALID_DATA unless it is a
** defined science), with "defined" a set the test fills (ref_up_reset).
**
** Dumps. Private members are made public for this file only. */
/* The renaming macros, `private`/`protected` made public, and the first headers come from
** shim_upgrade/upgrade_prelude.h (forced include): the INI prelude's BitFlagsIO.h reaches the real
** Upgrade.h through Xfer.h, so they must be in force before it. */
#include "Common/INI.h"
#include "Common/INIException.h"
#include "Common/NameKeyGenerator.h"
#include "Common/XferCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/GameAudio.h"
#include "Common/AcademyStats.h"
#include "Common/Upgrade.h"
#include "Common/SpecialPower.h"
#include "GameClient/InGameUI.h"

/* TheAcademyClassificationTypeNames, from the real AcademyStats.cpp (build.rs extracts the definition) */
#include "academy_names.inc"

/* the upstream files, unchanged (the SpecialPower.cpp copy: see above) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Upgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INIUpgrade.cpp"
#include "SpecialPower_names_renamed.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/INI/INISpecialPower.cpp"

/* ---- stand-ins ----------------------------------------------------------- */
InGameUI *TheInGameUI = 0;
void AudioEventRTS::setEventName(AsciiString name) { m_eventName = name; }
void AudioManager::getInfoForAudioEvent(const AudioEventRTS *) const {}

namespace {
std::set<NameKeyType> g_sciences;
ScienceStore g_scienceStore;
}

/* Science.cpp's ScienceStore::friend_lookupScience, line for line, against the test's set
** of sciences; installed as wrap_ini.cpp's ref_science_lookup_hook by ref_up_reset. */
extern "C" ScienceType (*ref_science_lookup_hook)(const ScienceStore *, const char *);
static ScienceType upgrade_lookup(const ScienceStore *, const char *scienceName)
{
	NameKeyType nkt = NAMEKEY(scienceName);
	ScienceType st = (ScienceType)nkt;
	if (g_sciences.find(nkt) == g_sciences.end())
		throw INI_INVALID_DATA;
	return st;
}

/* ---- the dump ------------------------------------------------------------ */
namespace {

std::vector<unsigned char> g_dump;
void put(const void *p, int n) { g_dump.insert(g_dump.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void putU32(unsigned v) { put(&v, 4); }
void putI32(int v) { put(&v, 4); }
void putReal(Real r) { put(&r, 4); }
void putU64(unsigned long long v) { put(&v, 8); }
void putStr(const char *s, int n) { putU32((unsigned)n); put(s, n); }
void putAscii(const AsciiString &s) { putStr(s.str(), s.getLength()); }
int finish(unsigned char *buf, int cap)
{
	int n = (int)g_dump.size();
	if (n <= cap && n > 0) memcpy(buf, &g_dump[0], n);
	return n;
}

template <class MASK> void maskToWords(const MASK &m, int bits, unsigned long long *w)
{
	w[0] = w[1] = w[2] = w[3] = 0;
	for (int i = 0; i < bits; ++i) if (m.test(i)) w[i / 64] |= 1ULL << (i % 64);
}
template <class MASK> MASK maskFromWords(int bits, const unsigned long long *w)
{
	MASK m;
	for (int i = 0; i < bits; ++i) if (w[i / 64] >> (i % 64) & 1) m.set(i);
	return m;
}
void putMask(const UpgradeMaskType &m)
{
	unsigned long long w[4];
	maskToWords(m, UPGRADE_MAX_COUNT, w);
	for (int i = 0; i < 4; ++i) putU64(w[i]);
}

void putTemplate(const UpgradeTemplate *t)
{
	putI32((int)t->m_type);
	putAscii(t->m_name);
	putU32((unsigned)t->m_nameKey);
	putAscii(t->m_displayNameLabel);
	putReal(t->m_buildTime);
	putI32(t->m_cost);
	putMask(t->m_upgradeMask);
	putAscii(t->m_researchSound.getEventName());
	putAscii(t->m_unitSpecificSound.getEventName());
	putI32((int)t->m_academyClassificationType);
	putAscii(t->m_buttonImageName);
}

void putSpecialPower(const SpecialPowerTemplate *t)
{
	putAscii(t->m_name);
	putU32(t->m_id);
	putI32((int)t->m_type);
	putU32(t->m_reloadTime);
	putI32((int)t->m_requiredScience);
	putAscii(t->m_initiateSound.getEventName());
	putAscii(t->m_initiateAtLocationSound.getEventName());
	putI32((int)t->m_academyClassificationType);
	putU32(t->m_detectionTime);
	putU32(t->m_viewObjectDuration);
	putReal(t->m_viewObjectRange);
	putReal(t->m_radiusCursorRadius);
	g_dump.push_back(t->m_publicTimer ? 1 : 0);
	g_dump.push_back(t->m_sharedNSync ? 1 : 0);
	g_dump.push_back(t->m_shortcutPower ? 1 : 0);
}

/* the INI's throw, as a status (the same as wrap_ini.cpp's ref_ini_load): 0 none, 1 INIException (message
** into msg), 2 an INI_* status code, 3 an ErrorCode, 4 std::out_of_range, 5 other */
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

} /* namespace */

extern "C" void ref_ini_reset(void);
extern "C" void ref_ini_add_file(const char *name, const char *data, int len);

extern "C" {

/* A NameKeyGenerator (installed as TheNameKeyGenerator; reset, and init()ed if asked: the well-known keys),
** a new UpgradeCenter (init()ed if asked) and an empty SpecialPowerStore, with the given names defined as
** sciences (their keys assigned in this order). Returns the status of init() (0, or 4 for out_of_range). */
int ref_up_reset(int init_names, int init_center, int n_sciences, const char *const *sciences)
{
	static NameKeyGenerator *gen = 0;
	if (!gen) gen = new NameKeyGenerator;
	gen->reset();
	if (init_names) gen->init();
	TheNameKeyGenerator = gen;
	ref_science_lookup_hook = upgrade_lookup;
	g_sciences.clear();
	TheScienceStore = &g_scienceStore;
	for (int i = 0; i < n_sciences; ++i) g_sciences.insert(TheNameKeyGenerator->nameToKey(sciences[i]));
	delete TheUpgradeCenter;
	delete TheSpecialPowerStore;
	TheUpgradeCenter = new UpgradeCenter;
	TheSpecialPowerStore = new SpecialPowerStore;
	int status = 0;
	if (init_center) {
		try { TheUpgradeCenter->init(); } catch (std::out_of_range &) { status = 4; }
	}
	return status;
}

unsigned ref_up_name_key(const char *name) { return (unsigned)TheNameKeyGenerator->nameToKey(name); }

/* INI::load(filename, load_type, xfer ? &crc : NULL) of a file added with ref_ini_add_file; the status is
** that of CATCH_ALL, with the XferCRC's getCRC() and the exception message or code */
int ref_up_load(const char *filename, int load_type, int with_xfer, unsigned *crc, char *msg, int msgCap, unsigned *code)
{
	XferCRC x;
	int status = 0;
	try {
		INI ini;
		ini.load(AsciiString(filename), (INILoadType)load_type, with_xfer ? &x : NULL);
	}
	CATCH_ALL(status, code, msg, msgCap)
	*crc = x.getCRC();
	return status;
}

/* ---- Upgrade -------------------------------------------------------------- */

/* count, nextTemplateMaskBit, then every template from firstUpgradeTemplate() along friend_getNext(): type,
** name, name key, display label, build time (bits), cost, mask (4 x u64), the two sounds' names, academy
** type, button image name */
int ref_up_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	unsigned n = 0;
	for (UpgradeTemplate *t = TheUpgradeCenter->firstUpgradeTemplate(); t; t = t->friend_getNext()) ++n;
	putU32(n);
	putI32(TheUpgradeCenter->m_nextTemplateMaskBit);
	for (UpgradeTemplate *t = TheUpgradeCenter->firstUpgradeTemplate(); t; t = t->friend_getNext()) putTemplate(t);
	return finish(buf, cap);
}

/* the list position (0 = first) of the template findUpgrade(name) returns, or -1 */
int ref_up_find(const char *name)
{
	const UpgradeTemplate *f = TheUpgradeCenter->findUpgrade(AsciiString(name));
	if (!f) return -1;
	int i = 0;
	for (UpgradeTemplate *t = TheUpgradeCenter->firstUpgradeTemplate(); t; t = t->friend_getNext(), ++i) if (t == f) return i;
	return -2;
}

/* ... findUpgradeByKey(key) */
int ref_up_find_by_key(unsigned key)
{
	const UpgradeTemplate *f = TheUpgradeCenter->findUpgradeByKey((NameKeyType)key);
	if (!f) return -1;
	int i = 0;
	for (UpgradeTemplate *t = TheUpgradeCenter->firstUpgradeTemplate(); t; t = t->friend_getNext(), ++i) if (t == f) return i;
	return -2;
}

/* ... findVeterancyUpgrade(level) */
int ref_up_find_vet(int level)
{
	const UpgradeTemplate *f = TheUpgradeCenter->findVeterancyUpgrade((VeterancyLevel)level);
	if (!f) return -1;
	int i = 0;
	for (UpgradeTemplate *t = TheUpgradeCenter->firstUpgradeTemplate(); t; t = t->friend_getNext(), ++i) if (t == f) return i;
	return -2;
}

/* getUpgradeNames(): count, then the names (length, bytes) */
int ref_up_names(unsigned char *buf, int cap)
{
	g_dump.clear();
	std::vector<AsciiString> v = TheUpgradeCenter->getUpgradeNames();
	putU32((unsigned)v.size());
	for (size_t i = 0; i < v.size(); ++i) putAscii(v[i]);
	return finish(buf, cap);
}

/* calcTimeToBuild(NULL) and calcCostToBuild(NULL) of the template at list position `index` (the player
** is unused in a release build); returns 1 if there is none */
int ref_up_calc(int index, int *time, int *cost)
{
	UpgradeTemplate *t = TheUpgradeCenter->firstUpgradeTemplate();
	for (int i = 0; t && i < index; ++i) t = t->friend_getNext();
	if (!t) return 1;
	*time = t->calcTimeToBuild(NULL);
	*cost = t->calcCostToBuild(NULL);
	return 0;
}

/* newUpgrade(name) directly (the INI path is ref_up_load); returns the status: 0, or 4 for out_of_range */
int ref_up_new_upgrade(const char *name)
{
	try { TheUpgradeCenter->newUpgrade(AsciiString(name)); } catch (std::out_of_range &) { return 4; }
	return 0;
}

/* TheUpgradeTypeNames[i], NULL at the end */
const char *ref_up_type_name(int i) { return UpTestTheUpgradeTypeNames[i]; }
/* TheAcademyClassificationTypeNames[i], NULL at the end */
const char *ref_academy_name(int i) { return TheAcademyClassificationTypeNames[i]; }

/* Upgrade (the instance) xfer, as wrap_armor.cpp's DamageInfo: kind 0 save (file at `path` through XferSave,
** then its bytes), 1 load (`bytes` written to `path`, read into an Upgrade whose status starts as *status;
** the resulting status back), 2 CRC (the XferCRC of the snapshot, which is nothing). Returns the XferStatus
** thrown (0 none; -1 anything else); *len is the saved file's length. */
int ref_up_inst_xfer(int kind, int *status, const char *path, unsigned char *bytes, int cap, int *len, unsigned *crc)
{
	Upgrade u(NULL);
	u.m_status = (UpgradeStatusType)*status;
	int st = 0;
	*len = 0;
	if (kind == 0) {
		{
			XferSave x;
			try { x.open(AsciiString(path)); x.xferSnapshot(&u); } catch (XferStatus s) { st = (int)s; } catch (...) { st = -1; }
			try { x.close(); } catch (...) {}
		}
		if (FILE *f = fopen(path, "rb")) { *len = (int)fread(bytes, 1, cap, f); fclose(f); }
	} else if (kind == 1) {
		if (FILE *f = fopen(path, "wb")) { if (cap > 0) fwrite(bytes, 1, cap, f); fclose(f); }
		{
			XferLoad x;
			try { x.open(AsciiString(path)); x.xferSnapshot(&u); } catch (XferStatus s) { st = (int)s; } catch (...) { st = -1; }
			try { x.close(); } catch (...) {}
		}
		memcpy(status, &u.m_status, 4);
	} else {
		XferCRC x;
		try { x.open(AsciiString("crc")); x.xferSnapshot(&u); x.close(); } catch (XferStatus s) { st = (int)s; } catch (...) { st = -1; }
		*crc = x.getCRC();
	}
	return st;
}

/* ---- SpecialPower --------------------------------------------------------- */

/* count of heads, m_nextSpecialPowerID, then per head: the chain length, and per node (head first) its
** m_isOverride flag and its fields (name, id, type, reload time, required science, the two sounds' names,
** academy type, detection time, view object duration and range, radius cursor radius, the three Bools) */
int ref_sp_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	putU32((unsigned)TheSpecialPowerStore->m_specialPowerTemplates.size());
	putU32(TheSpecialPowerStore->m_nextSpecialPowerID);
	for (size_t i = 0; i < TheSpecialPowerStore->m_specialPowerTemplates.size(); ++i) {
		const SpecialPowerTemplate *head = TheSpecialPowerStore->m_specialPowerTemplates[i];
		unsigned n = 0;
		for (const Overridable *o = head; o; o = o->getNextOverride()) ++n;
		putU32(n);
		for (const Overridable *o = head; o; o = o->getNextOverride()) {
			g_dump.push_back(o->m_isOverride ? 1 : 0);
			putSpecialPower((const SpecialPowerTemplate *)o);
		}
	}
	return finish(buf, cap);
}

/* the getters of the template findSpecialPowerTemplate(name) returns (getName() through getShortcut...):
** returns 1 if there is none; else out = id, type, reload time, required science, detection time, view
** duration, view range (bits), radius (bits), flags (1 public timer, 2 shared, 4 shortcut), academy
** (getAcademyClassificationType: the HEAD's), and the three names into `names` as length-prefixed strings
** (name, initiate sound, initiate-at-target sound) */
static int spGetters(const SpecialPowerTemplate *t, unsigned *out, unsigned char *names, int cap, int *nlen)
{
	if (!t) return 1;
	Real vr = t->getViewObjectRange(), rr = t->getRadiusCursorRadius();
	out[0] = t->getID();
	out[1] = (unsigned)t->getSpecialPowerType();
	out[2] = t->getReloadTime();
	out[3] = (unsigned)t->getRequiredScience();
	out[4] = t->getDetectionTime();
	out[5] = t->getViewObjectDuration();
	memcpy(&out[6], &vr, 4);
	memcpy(&out[7], &rr, 4);
	out[8] = (t->hasPublicTimer() ? 1 : 0) | (t->isSharedNSync() ? 2 : 0) | (t->isShortcutPower() ? 4 : 0);
	out[9] = (unsigned)t->getAcademyClassificationType();
	g_dump.clear();
	putAscii(t->getName());
	putAscii(t->getInitiateSound()->getEventName());
	putAscii(t->getInitiateAtTargetSound()->getEventName());
	*nlen = finish(names, cap);
	return 0;
}
int ref_sp_by_name(const char *name, unsigned *out, unsigned char *names, int cap, int *nlen)
{
	return spGetters(TheSpecialPowerStore->findSpecialPowerTemplate(AsciiString(name)), out, names, cap, nlen);
}
int ref_sp_by_id(unsigned id, unsigned *out, unsigned char *names, int cap, int *nlen)
{
	return spGetters(TheSpecialPowerStore->findSpecialPowerTemplateByID(id), out, names, cap, nlen);
}
int ref_sp_by_index(unsigned index, unsigned *out, unsigned char *names, int cap, int *nlen)
{
	return spGetters(TheSpecialPowerStore->getSpecialPowerTemplateByIndex(index), out, names, cap, nlen);
}
int ref_sp_count(void) { return TheSpecialPowerStore->getNumSpecialPowers(); }
void ref_sp_reset_store(void) { TheSpecialPowerStore->reset(); }

/* the real SpecialPower.cpp's name list, entry i (NULL at the end) */
const char *ref_sp_real_name(int i) { return UpSpecialPowerRealNames[i]; }
/* DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT */
unsigned ref_sp_default_detection(void) { return DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT; }

/* ---- the mask helpers ----------------------------------------------------- */

/* which 0 UpgradeMaskType, 1 SpecialPowerMaskType; masks as 4 x u64.
** op 0 TEST(a, idx[0]) (-1 if out_of_range), 1 TEST_ANY(a, b), 2 TEST_MULTI(a, b, c), 3 ANY_SET(a),
** 4 CLEAR(a) -> out, 5 SET(a, idx[0], idx[1]) -> out (special power only; -1 if out_of_range),
** 6 SET_ALL(a) -> out, 7 FLIP(a) -> out, 8 MAKE_MASK(idx[0..nidx], 1 to 5 indices) -> out (-1 if out_of_range).
** Returns the Bool, or -1. */
int ref_mask_op(int which, int op, const unsigned long long *a, const unsigned long long *b, const unsigned long long *c,
	const int *idx, int nidx, unsigned long long *out)
{
	out[0] = out[1] = out[2] = out[3] = 0;
	try {
		if (which == 0) {
			UpgradeMaskType A = maskFromWords<UpgradeMaskType>(UPGRADE_MAX_COUNT, a);
			UpgradeMaskType B = maskFromWords<UpgradeMaskType>(UPGRADE_MAX_COUNT, b);
			UpgradeMaskType C = maskFromWords<UpgradeMaskType>(UPGRADE_MAX_COUNT, c);
			switch (op) {
			case 0: return TEST_UPGRADE_MASK(A, idx[0]) ? 1 : 0;
			case 1: return TEST_UPGRADE_MASK_ANY(A, B) ? 1 : 0;
			case 2: return TEST_UPGRADE_MASK_MULTI(A, B, C) ? 1 : 0;
			case 3: return UPGRADE_MASK_ANY_SET(A) ? 1 : 0;
			case 4: CLEAR_UPGRADE_MASK(A); maskToWords(A, UPGRADE_MAX_COUNT, out); return 0;
			case 6: SET_ALL_UPGRADE_MASK_BITS(A); maskToWords(A, UPGRADE_MAX_COUNT, out); return 0;
			case 7: FLIP_UPGRADE_MASK(A); maskToWords(A, UPGRADE_MAX_COUNT, out); return 0;
			case 8: {
				UpgradeMaskType M;
				switch (nidx) {
				case 1: M = MAKE_UPGRADE_MASK(idx[0]); break;
				case 2: M = MAKE_UPGRADE_MASK2(idx[0], idx[1]); break;
				case 3: M = MAKE_UPGRADE_MASK3(idx[0], idx[1], idx[2]); break;
				case 4: M = MAKE_UPGRADE_MASK4(idx[0], idx[1], idx[2], idx[3]); break;
				default: M = MAKE_UPGRADE_MASK5(idx[0], idx[1], idx[2], idx[3], idx[4]); break;
				}
				maskToWords(M, UPGRADE_MAX_COUNT, out);
				return 0;
			}
			}
		} else {
			SpecialPowerMaskType A = maskFromWords<SpecialPowerMaskType>(SPECIALPOWER_COUNT, a);
			SpecialPowerMaskType B = maskFromWords<SpecialPowerMaskType>(SPECIALPOWER_COUNT, b);
			SpecialPowerMaskType C = maskFromWords<SpecialPowerMaskType>(SPECIALPOWER_COUNT, c);
			switch (op) {
			case 0: return TEST_SPECIALPOWERMASK(A, (SpecialPowerType)idx[0]) ? 1 : 0;
			case 1: return TEST_SPECIALPOWERMASK_ANY(A, B) ? 1 : 0;
			case 2: return TEST_SPECIALPOWERMASK_MULTI(A, B, C) ? 1 : 0;
			case 3: return SPECIALPOWERMASK_ANY_SET(A) ? 1 : 0;
			case 4: CLEAR_SPECIALPOWERMASK(A); maskToWords(A, SPECIALPOWER_COUNT, out); return 0;
			case 5: SET_SPECIALPOWERMASK(A, (SpecialPowerType)idx[0], idx[1]); maskToWords(A, SPECIALPOWER_COUNT, out); return 0;
			case 6: SET_ALL_SPECIALPOWERMASK_BITS(A); maskToWords(A, SPECIALPOWER_COUNT, out); return 0;
			case 7: FLIP_SPECIALPOWERMASK(A); maskToWords(A, SPECIALPOWER_COUNT, out); return 0;
			case 8: {
				SpecialPowerMaskType M = MAKE_SPECIALPOWER_MASK((SpecialPowerType)idx[0]);
				maskToWords(M, SPECIALPOWER_COUNT, out);
				return 0;
			}
			}
		}
	} catch (std::out_of_range &) {
		return -1;
	}
	abort();
}

} /* extern "C" */

/* --- p1w11-sonnet --- the raw template pointer findSpecialPowerTemplate(name) returns, and a template's id, for the module unit's
** UnpauseSpecialPowerUpgrade fixture (differential/l9_upgrades_c.inc), which parses its template through the real INI */
extern "C" const void *ref_sp_find_raw(const char *name) { return TheSpecialPowerStore->findSpecialPowerTemplate(AsciiString(name)); }
extern "C" unsigned ref_sp_raw_id(const void *t) { return t ? ((const SpecialPowerTemplate *)t)->getID() : 0; }
/* --- end p1w11-sonnet --- */
