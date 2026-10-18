/* Differential-test wrapper for Armor, Damage and DamageFX (upstream commit
** 0a05454d):
**   GameEngine/Source/GameLogic/Object/Armor.cpp    (compiled unchanged)
**   GameEngine/Source/Common/DamageFX.cpp           (compiled unchanged)
**   GameEngine/Source/Common/INI/INIDamageFX.cpp    (compiled unchanged)
**   GameEngine/Source/GameLogic/System/Damage.cpp   (a copy: build.rs adds
**       the `template<>` that g++ and clang require before the explicit
**       specialisation of DamageTypeFlags::s_bitNameList, asserted once)
** with the headers Armor.h, Damage.h, DamageFX.h, INI.h, NameKeyGenerator.h
** unchanged, in the BitFlags build's setup (clang++ -fms-extensions, the real
** BitFlagsIO.h) plus the stand-ins of differential/shim_armor: ThingTemplate
** and ThingFactory (a name-carrying template for any non-empty name), FXList
** (an opaque class; doFXObj records the effect it was given), Object (a
** veterancy level), and an ordered std::hash_map.
**
** INI: the blocks are read by the REAL INI::load (upstream_ini), through the
** hooks of wrap_ini.cpp (ref_ini_reset, ref_ini_add_file, ref_ini_load); the
** `Armor` and `DamageFX` blocks reach Armor.cpp and INIDamageFX.cpp (the
** definitions in wrap_ini.cpp are weak, so these override them). The stores
** are then dumped here (private members are made public for this file only).
**
** FXLists are never resolved by the Rust, so the stand-in
** FXListStore::findFXList answers a distinct pointer for every name other than
** "None" (any case), as the real one returns NULL for "None"; the dump maps the
** pointers back to the names.
**
** Xfer: DamageInfo, DamageInfoInput and DamageInfoOutput through the real
** XferSave, XferLoad and XferCRC (upstream_xfer). The fields cross as 17
** words (see fillInfo). */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <strings.h>
#include <string>
#include <map>
#include <vector>
#include <list>
#include <set>
#define private public
#define protected public
#include "Common/NameKeyGenerator.h"
#include "Common/XferCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/ThingFactory.h"
#include "GameClient/FXList.h"
#include "GameLogic/Object.h"
#include "GameLogic/Armor.h"
#include "GameLogic/Damage.h"
#include "Common/DamageFX.h"
#undef private
#undef protected

/* ---- stand-ins */
namespace {
std::map<std::string, FXList *> g_fx;
std::map<const FXList *, std::string> g_fxName;
const FXList *g_lastFx = 0;
std::map<std::string, ThingTemplate *> g_things;
ThingFactory g_thingFactory;
FXListStore g_fxStore;
}

ThingFactory *TheThingFactory = &g_thingFactory;
FXListStore *TheFXListStore = &g_fxStore;

const ThingTemplate *ThingFactory::findTemplate(const AsciiString &name, Bool)
{
	if (name.getLength() == 0) return 0;
	std::string s(name.str());
	std::map<std::string, ThingTemplate *>::iterator it = g_things.find(s);
	if (it == g_things.end()) it = g_things.insert(std::make_pair(s, new ThingTemplate(name))).first;
	return it->second;
}

const FXList *FXListStore::findFXList(const char *name) const
{
	if (strcasecmp(name, "None") == 0) return 0;
	std::string s(name);
	std::map<std::string, FXList *>::iterator it = g_fx.find(s);
	if (it == g_fx.end()) {
		FXList *fx = new FXList;
		it = g_fx.insert(std::make_pair(s, fx)).first;
		g_fxName[fx] = s;
	}
	return it->second;
}

void FXList::doFXObj(const FXList *fx, const Object *, const Object *) { g_lastFx = fx; }
/* p1w7-sonnet: the Weapon test reads FXList pointers its INI parsers stored (through this findFXList) back as names */
extern "C" const char *ref_armor_fx_name(const void *fx)
{
	std::map<const FXList *, std::string>::const_iterator it = g_fxName.find((const FXList *)fx);
	return it == g_fxName.end() ? "" : it->second.c_str();
}

namespace {

std::vector<unsigned char> g_dump;
void put(const void *p, int n) { g_dump.insert(g_dump.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void putU32(unsigned v) { put(&v, 4); }
void putReal(Real r) { put(&r, 4); }
void putFx(const FXList *fx)
{
	if (!fx) { g_dump.push_back(0); return; }
	g_dump.push_back(1);
	const std::string &n = g_fxName[fx];
	putU32((unsigned)n.size());
	put(n.data(), (int)n.size());
}
int finish(unsigned char *buf, int cap)
{
	int n = (int)g_dump.size();
	if (n <= cap && n > 0) memcpy(buf, &g_dump[0], n);
	return n;
}

/* the 17 words: sourceID, sourcePlayerMask, damageType, damageFXOverride,
** deathType, amount (bits), kill, damageStatusType, shockWaveVector x y z
** (bits), shockWaveAmount, Radius, TaperOff (bits), actualDamageDealt,
** actualDamageClipped (bits), noEffect */
unsigned bitsOf(Real r) { unsigned u; memcpy(&u, &r, 4); return u; }
Real realOf(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
unsigned byteOf(const Bool *b) { unsigned char c; memcpy(&c, b, 1); return c; }
void fillInfo(DamageInfo &d, const unsigned *w, const char *tmpl)
{
	d.in.m_sourceID = (ObjectID)w[0];
	d.in.m_sourcePlayerMask = (PlayerMaskType)w[1];
	d.in.m_damageType = (DamageType)w[2];
	d.in.m_damageFXOverride = (DamageType)w[3];
	d.in.m_deathType = (DeathType)w[4];
	d.in.m_amount = realOf(w[5]);
	d.in.m_kill = w[6] != 0;
	d.in.m_damageStatusType = (ObjectStatusTypes)w[7];
	d.in.m_shockWaveVector.x = realOf(w[8]);
	d.in.m_shockWaveVector.y = realOf(w[9]);
	d.in.m_shockWaveVector.z = realOf(w[10]);
	d.in.m_shockWaveAmount = realOf(w[11]);
	d.in.m_shockWaveRadius = realOf(w[12]);
	d.in.m_shockWaveTaperOff = realOf(w[13]);
	d.out.m_actualDamageDealt = realOf(w[14]);
	d.out.m_actualDamageClipped = realOf(w[15]);
	d.out.m_noEffect = w[16] != 0;
	d.in.m_sourceTemplate = tmpl[0] ? TheThingFactory->findTemplate(AsciiString(tmpl)) : 0;
}
void readInfo(const DamageInfo &d, unsigned *w, char *tmpl, int cap)
{
	w[0] = (unsigned)d.in.m_sourceID;
	w[1] = d.in.m_sourcePlayerMask;
	w[2] = (unsigned)d.in.m_damageType;
	w[3] = (unsigned)d.in.m_damageFXOverride;
	w[4] = (unsigned)d.in.m_deathType;
	w[5] = bitsOf(d.in.m_amount);
	w[6] = byteOf(&d.in.m_kill) != 0; /* the raw byte: a loaded Bool need not be 0 or 1 */
	w[7] = (unsigned)d.in.m_damageStatusType;
	w[8] = bitsOf(d.in.m_shockWaveVector.x);
	w[9] = bitsOf(d.in.m_shockWaveVector.y);
	w[10] = bitsOf(d.in.m_shockWaveVector.z);
	w[11] = bitsOf(d.in.m_shockWaveAmount);
	w[12] = bitsOf(d.in.m_shockWaveRadius);
	w[13] = bitsOf(d.in.m_shockWaveTaperOff);
	w[14] = bitsOf(d.out.m_actualDamageDealt);
	w[15] = bitsOf(d.out.m_actualDamageClipped);
	w[16] = byteOf(&d.out.m_noEffect) != 0;
	const char *n = d.in.m_sourceTemplate ? d.in.m_sourceTemplate->getName().str() : "";
	strncpy(tmpl, n, cap - 1);
	tmpl[cap - 1] = 0;
}

/* kind 0 DamageInfo, 1 DamageInfoInput, 2 DamageInfoOutput */
Snapshot *snapshotOf(DamageInfo &d, int kind)
{
	return kind == 0 ? (Snapshot *)&d : kind == 1 ? (Snapshot *)&d.in : (Snapshot *)&d.out;
}

} /* namespace */

extern "C" {

/* a NameKeyGenerator (installed as TheNameKeyGenerator, reset: keys start at
** 1 again) and empty stores */
void ref_armor_reset(void)
{
	static NameKeyGenerator *gen = 0;
	static ArmorStore *armor = 0;
	static DamageFXStore *fx = 0;
	if (!gen) gen = new NameKeyGenerator;
	gen->reset();
	TheNameKeyGenerator = gen;
	delete armor;
	delete fx;
	TheArmorStore = armor = new ArmorStore;
	TheDamageFXStore = fx = new DamageFXStore;
}

unsigned ref_armor_name_key(const char *name) { return (unsigned)TheNameKeyGenerator->nameToKey(name); }

/* ArmorStore: count, then per template (in key order) its key and the 38
** coefficients (bits). Returns the length (may exceed cap). */
int ref_armor_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	putU32((unsigned)TheArmorStore->m_armorTemplates.size());
	for (ArmorStore::ArmorTemplateMap::const_iterator it = TheArmorStore->m_armorTemplates.begin();
		it != TheArmorStore->m_armorTemplates.end(); ++it) {
		putU32((unsigned)it->first);
		for (int i = 0; i < DAMAGE_NUM_TYPES; ++i) putReal(it->second.m_damageCoefficient[i]);
	}
	return finish(buf, cap);
}

/* DamageFXStore: count, then per DamageFX (in key order) its key and, for
** each damage type and level: amountForMajorFX (bits), throttle time, the
** major and the minor FXList (0, or 1 len name). */
int ref_dfx_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	putU32((unsigned)TheDamageFXStore->m_dfxmap.size());
	for (DamageFXStore::DamageFXMap::const_iterator it = TheDamageFXStore->m_dfxmap.begin();
		it != TheDamageFXStore->m_dfxmap.end(); ++it) {
		putU32((unsigned)it->first);
		for (int dt = 0; dt < DAMAGE_NUM_TYPES; ++dt)
			for (int v = 0; v < LEVEL_COUNT; ++v) {
				const DamageFX::DFX &d = it->second.m_dfx[dt][v];
				putReal(d.m_amountForMajorFX);
				putU32(d.m_damageFXThrottleTime);
				putFx(d.m_majorDamageFXList);
				putFx(d.m_minorDamageFXList);
			}
	}
	return finish(buf, cap);
}

/* findArmorTemplate(name), makeArmor, Armor::adjustDamage, and the template's
** own adjustDamage. Returns 1 if there is no such template, else 0 with the
** results (bits) in out[0] (Armor) and out[1] (ArmorTemplate). */
int ref_armor_adjust(const char *name, int dt, float damage, unsigned *out)
{
	const ArmorTemplate *tmpl = TheArmorStore->findArmorTemplate(AsciiString(name));
	if (!tmpl) return 1;
	Armor a = TheArmorStore->makeArmor(tmpl);
	out[0] = bitsOf(a.adjustDamage((DamageType)dt, damage));
	out[1] = bitsOf(tmpl->adjustDamage((DamageType)dt, damage));
	return 0;
}

/* Armor() has no template, so passes the damage through; so does an Armor
** that was given a template and then clear()ed. out[0], out[1] (bits). */
void ref_armor_none(float damage, unsigned *out)
{
	Armor a;
	out[0] = bitsOf(a.adjustDamage(DAMAGE_FLAME, damage));
	if (!TheArmorStore->m_armorTemplates.empty())
		a = Armor(&TheArmorStore->m_armorTemplates.begin()->second);
	a.clear();
	out[1] = bitsOf(a.adjustDamage(DAMAGE_FLAME, damage));
}

/* DamageFX getters. level -1: no source object. Returns 1 if there is no
** such DamageFX. out[0] = throttle time; fx = the FXList of getDamageFXList
** (as name: *fxLen = -1 for NULL), and *viaDo = the same through doDamageFX
** (0 none, 1 same pointer as getDamageFXList's, 2 different). */
int ref_dfx_query(const char *name, int dt, float amount, int level, unsigned *throttle,
	char *fx, int cap, int *fxLen, int *viaDo)
{
	const DamageFX *dfx = TheDamageFXStore->findDamageFX(AsciiString(name));
	if (!dfx) return 1;
	Object obj((VeterancyLevel)(level < 0 ? 0 : level));
	const Object *src = level < 0 ? 0 : &obj;
	*throttle = dfx->getDamageFXThrottleTime((DamageType)dt, src);
	const FXList *l = dfx->getDamageFXList((DamageType)dt, amount, src);
	if (!l) *fxLen = -1;
	else {
		const std::string &n = g_fxName[l];
		*fxLen = (int)n.size();
		if ((int)n.size() < cap) memcpy(fx, n.data(), n.size());
	}
	static FXList sentinel;
	g_lastFx = &sentinel;
	dfx->doDamageFX((DamageType)dt, amount, src, 0);
	*viaDo = g_lastFx == l ? 1 : 2;
	if (!l && g_lastFx != 0) *viaDo = 2;
	return 0;
}

/* the inline helpers of Damage.h: op 0 IsSubdualDamage, 1 IsHealthDamagingDamage */
int ref_damage_inline(int op, int dt)
{
	return op == 0 ? IsSubdualDamage((DamageType)dt) : IsHealthDamagingDamage((DamageType)dt);
}

/* initDamageTypeFlags(), then DAMAGE_TYPE_FLAGS_ALL and _NONE as bit words */
void ref_damage_flags(unsigned long long *all, unsigned long long *none)
{
	initDamageTypeFlags();
	unsigned long long a = 0, n = 0;
	for (int i = 0; i < DAMAGE_NUM_TYPES; ++i) {
		if (DAMAGE_TYPE_FLAGS_ALL.test(i)) a |= 1ULL << i;
		if (DAMAGE_TYPE_FLAGS_NONE.test(i)) n |= 1ULL << i;
	}
	*all = a;
	*none = n;
}

/* the defaults of DamageInfoInput() and DamageInfoOutput() */
void ref_damage_default(unsigned *w)
{
	DamageInfo d;
	char t[8];
	readInfo(d, w, t, 8);
}

/* Save: the fields to a file at `path` through XferSave (kind: 0 DamageInfo,
** 1 DamageInfoInput, 2 DamageInfoOutput), then the file's bytes. Returns the
** XferStatus thrown (0 none; -1 anything else); *len is the file's length. */
int ref_damage_save(int kind, const unsigned *w, const char *tmpl, const char *path,
	unsigned char *out, int cap, int *len)
{
	DamageInfo d;
	fillInfo(d, w, tmpl);
	int status = 0;
	{
		XferSave x;
		try {
			x.open(AsciiString(path));
			x.xferSnapshot(snapshotOf(d, kind));
		} catch (XferStatus s) { status = (int)s; } catch (...) { status = -1; }
		try { x.close(); } catch (...) {}
	}
	*len = 0;
	if (FILE *f = fopen(path, "rb")) {
		*len = (int)fread(out, 1, cap, f);
		fclose(f);
	}
	return status;
}

/* Load: `bytes` written to `path`, read through XferLoad into DamageInfo
** fields that start as `w`/`tmpl`; the resulting fields go back to `w` and
** `tmpl` (as far as the load got). Returns the XferStatus thrown. */
int ref_damage_load(int kind, unsigned *w, char *tmpl, int tmplCap, const unsigned char *bytes, int n, const char *path)
{
	if (FILE *f = fopen(path, "wb")) { if (n > 0) fwrite(bytes, 1, n, f); fclose(f); }
	DamageInfo d;
	fillInfo(d, w, tmpl);
	int status = 0;
	{
		XferLoad x;
		try {
			x.open(AsciiString(path));
			x.xferSnapshot(snapshotOf(d, kind));
		} catch (XferStatus s) { status = (int)s; } catch (...) { status = -1; }
		try { x.close(); } catch (...) {}
	}
	readInfo(d, w, tmpl, tmplCap);
	return status;
}

/* CRC: the XferCRC of the snapshot (its crc() does nothing, so the CRC of
** nothing). Returns the status; *crc is getCRC(). */
int ref_damage_crc(int kind, const unsigned *w, const char *tmpl, unsigned *crc)
{
	DamageInfo d;
	fillInfo(d, w, tmpl);
	int status = 0;
	XferCRC x;
	try {
		x.open(AsciiString("crc"));
		x.xferSnapshot(snapshotOf(d, kind));
		x.close();
	} catch (XferStatus s) { status = (int)s; } catch (...) { status = -1; }
	*crc = x.getCRC();
	return status;
}

} /* extern "C" */
