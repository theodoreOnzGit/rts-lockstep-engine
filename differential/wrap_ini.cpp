/* Differential-test wrapper for the INI reader (upstream commit 0a05454d):
**   GameEngine/Source/Common/INI/INI.cpp
** compiled as its own translation unit from a copy that build.rs makes with
** two mechanical edits for the MSVC dialect the file is written in (see
** build.rs: the pre-standard `for` scope in findFieldParse, and binding a
** `void*` to `const void*&` in initFromINIMulti), with
** differential/shim_ini/ini_prelude.h force-included and the stand-in
** headers of differential/shim_ini for the game systems it includes.
** GameEngine/Source/Common/System/GameCommon.cpp (TheVeterancyNames) is
** compiled unchanged beside it.
**
** INI::load opens files through TheFileSystem->openFile, whose stand-in
** (differential/wrap_big.cpp) forwards to the hook installed here; the File
** it returns (MemFile) serves a registered byte buffer, and is its own RAM
** file. loadDirectory lists files through FileSystem::getFileListInDirectory,
** defined here to answer with the list the test registers.
**
** The block table of INI.cpp (theTypeTable) is fixed, so the test handlers
** are defined here under five of its names; every other handler throws
** ERROR_BUG. The handlers fill a test record through the field parse tables
** below, whose entries wrap upstream's field parsers so that each parse
** appends its result to a log the Rust test reproduces byte for byte.
**
** Copied from upstream (labelled): DamageTypeFlags::s_bitNameList and
** DAMAGE_TYPE_FLAGS_NONE from GameLogic/System/Damage.cpp lines 42-83, which
** does not compile outside the engine. */
#include <stdlib.h>
#include <string.h>
#include <stdexcept>
#include <typeinfo>
#include <cxxabi.h>
#include <string>
#include <map>
#include <vector>
#include "Common/INI.h"
#include "Common/INIException.h"
#include "Common/File.h"
#include "Common/FileSystem.h"
#include "Common/XferCRC.h"
#include "Common/GameAudio.h"
#include "Common/DamageFX.h"
#include "Common/SpecialPower.h"
#include "Common/ThingFactory.h"
#include "Common/Upgrade.h"
#include "GameClient/Anim2D.h"
#include "GameClient/FXList.h"
#include "GameClient/GameText.h"
#include "GameClient/Image.h"
#include "GameClient/ParticleSys.h"
#include "GameLogic/Armor.h"
#include "GameLogic/Damage.h"
#include "GameLogic/FPUControl.h"
#include "GameLogic/ObjectCreationList.h"
#include "GameLogic/ScriptEngine.h"
#include "GameLogic/Weapon.h"

/* --- p1w1-armor ---
** The definitions marked WEAK here are also defined, for real, by the
** Armor/Damage/DamageFX build (differential/wrap_armor.cpp and the upstream
** files it compiles, linked ahead of this object): where both are linked,
** the real ones override these stand-ins. In the INI tests only these exist. */
#define WEAK __attribute__((weak))
/* p1w2-terrain: the five block handlers of INI/INITerrain.cpp, INIMultiplayer.cpp and
** INICrate.cpp, and ScienceStore::friend_lookupScience, are WEAK here: the terrain
** build (wrap_terrain.cpp) defines them for real. */
/* --- end p1w1-armor --- */

/* ---- Damage.cpp lines 42-83 (copied) */
template<> WEAK const char *DamageTypeFlags::s_bitNameList[] =
{
	"EXPLOSION", "CRUSH", "ARMOR_PIERCING", "SMALL_ARMS", "GATTLING", "RADIATION", "FLAME", "LASER",
	"SNIPER", "POISON", "HEALING", "UNRESISTABLE", "WATER", "DEPLOY", "SURRENDER", "HACK", "KILL_PILOT",
	"PENALTY", "FALLING", "MELEE", "DISARM", "HAZARD_CLEANUP", "PARTICLE_BEAM", "TOPPLING",
	"INFANTRY_MISSILE", "AURORA_BOMB", "LAND_MINE", "JET_MISSILES", "STEALTHJET_MISSILES",
	"MOLOTOV_COCKTAIL", "COMANCHE_VULCAN", "SUBDUAL_MISSILE", "SUBDUAL_VEHICLE", "SUBDUAL_BUILDING",
	"SUBDUAL_UNRESISTABLE", "MICROWAVE", "KILL_GARRISONED", "STATUS",
	NULL
};
WEAK DamageTypeFlags DAMAGE_TYPE_FLAGS_NONE;

/* ---- stand-ins for the game systems the untranslated field parsers use;
** the tests never reach them (they abort if reached). */
GameTextInterface *TheGameText = 0;
WEAK DamageFXStore *TheDamageFXStore = 0;
SpecialPowerStore *TheSpecialPowerStore = 0;
WEAK ThingFactory *TheThingFactory = 0;
Anim2DCollection *TheAnim2DCollection = 0;
WEAK FXListStore *TheFXListStore = 0;
ImageCollection *TheMappedImageCollection = 0;
ParticleSystemManager *TheParticleSystemManager = 0;
WEAK ArmorStore *TheArmorStore = 0;
ObjectCreationListStore *TheObjectCreationListStore = 0;
WeaponStore *TheWeaponStore = 0;
WEAK const DamageFX *DamageFXStore::findDamageFX(AsciiString) const { abort(); }
const SpecialPowerTemplate *SpecialPowerStore::findSpecialPowerTemplate(AsciiString) const { abort(); }
const ThingTemplate *ThingFactory::findTemplate(const AsciiString &) { abort(); }
Anim2DTemplate *Anim2DCollection::findTemplate(const AsciiString &) { abort(); }
WEAK const FXList *FXListStore::findFXList(const char *) const { abort(); }
const Image *ImageCollection::findImageByName(const AsciiString &) { abort(); }
ParticleSystemTemplate *ParticleSystemManager::findTemplate(const AsciiString &) const { abort(); }
WEAK const ArmorTemplate *ArmorStore::findArmorTemplate(AsciiString) const { abort(); }
const ObjectCreationList *ObjectCreationListStore::findObjectCreationList(const char *) const { abort(); }
const WeaponTemplate *WeaponStore::findWeaponTemplate(AsciiString) const { abort(); }
WEAK ScienceType ScienceStore::friend_lookupScience(const char *) const { abort(); }
void AudioEventRTS::setEventName(AsciiString) { abort(); }
void AudioManager::getInfoForAudioEvent(const AudioEventRTS *) const { abort(); }

/* GameLogic/FPUControl.h: x87 precision control (GameLogic.cpp). The tests
** compute in SSE single precision already, so it does nothing here. */
void setFPMode(void) {}

/* ---- the test log */
namespace {

std::vector<unsigned char> g_log;
std::map<std::string, std::vector<char> > g_files;
std::vector<std::string> g_dirList;

void put(const void *p, int n) { g_log.insert(g_log.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void put8(int v) { g_log.push_back((unsigned char)v); }
void putU32(unsigned v) { put(&v, 4); }
void putStr(const char *s, int n) { putU32((unsigned)n); put(s, n); }
void putCStr(const char *s) { putStr(s, (int)strlen(s)); }
void putAscii(const AsciiString &s) { putStr(s.str(), s.getLength()); }

enum { K_U8 = 1, K_I16, K_U16, K_I32, K_U32, K_F32, K_BOOL, K_STR, K_VEC, K_RGB, K_RGBA, K_C3, K_C2, K_IC2, K_DMG, K_RV };

/* the test record: the POD part starts as 0x5A bytes, so parsers that
** modify the stored value (parseBitInInt32, parseBitString32) see it */
struct Pod
{
	UnsignedByte u8; Short i16; UnsignedShort u16; Int i32; UnsignedInt u32; Real r; Bool b;
	UnsignedInt bit; Byte bits8; UnsignedInt bits32; RGBColor rgb; RGBAColorInt rgba; Int color;
	Coord3D c3; Coord2D c2; ICoord2D ic2; Int index; Byte bindex; Int lookup;
	UnsignedInt vet; UnsignedInt death; UnsignedShort dus;
};
struct Rec
{
	Pod p;
	AsciiString s;
	std::vector<AsciiString> sv;
	DamageTypeFlags damage;
	GameClientRandomVariable rv;
	Rec() { memset(&p, 0x5A, sizeof(p)); }
};

void logStore(int id, int kind, const void *store)
{
	put8(id);
	switch (kind) {
	case K_U8: put(store, 1); break;
	case K_I16: case K_U16: put(store, 2); break;
	case K_I32: case K_U32: case K_F32: put(store, 4); break;
	case K_BOOL: put8(*(const Bool *)store ? 1 : 0); break;
	case K_STR: putAscii(*(const AsciiString *)store); break;
	case K_VEC: {
		const std::vector<AsciiString> &v = *(const std::vector<AsciiString> *)store;
		putU32((unsigned)v.size());
		for (size_t i = 0; i < v.size(); ++i) putAscii(v[i]);
		break;
	}
	case K_RGB: put(store, 12); break;
	case K_RGBA: put(store, 16); break;
	case K_C3: put(store, 12); break;
	case K_C2: case K_IC2: put(store, 8); break;
	case K_DMG: {
		const DamageTypeFlags &f = *(const DamageTypeFlags *)store;
		unsigned long long v = 0;
		for (int i = 0; i < DAMAGE_NUM_TYPES; ++i) if (f.test(i)) v |= 1ULL << i;
		put(&v, 8);
		break;
	}
	case K_RV: {
		const GameClientRandomVariable &v = *(const GameClientRandomVariable *)store;
		Real lo = v.getMinimumValue(), hi = v.getMaximumValue();
		Int t = (Int)v.getDistributionType();
		put(&lo, 4); put(&hi, 4); put(&t, 4);
		break;
	}
	}
}

/* an upstream field parser, then its result into the log */
template <INIFieldParseProc P, int KIND, int ID>
void L(INI *ini, void *instance, void *store, const void *userData)
{
	P(ini, instance, store, userData);
	logStore(ID, KIND, store);
}

/* 40 names (more than 32: bit indices of 32 and up are undefined upstream) */
const char *TestNames[] =
{
	"N00", "N01", "N02", "N03", "N04", "N05", "N06", "N07", "N08", "N09",
	"N10", "N11", "N12", "N13", "N14", "N15", "N16", "N17", "N18", "N19",
	"N20", "N21", "N22", "N23", "N24", "N25", "N26", "N27", "N28", "N29",
	"N30", "N31", "N32", "N33", "N34", "N35", "N36", "N37", "N38", "N39",
	NULL
};
const LookupListRec TestLookup[] = { { "Low", 1 }, { "Mid", 5 }, { "High", -7 }, { "Max", 0x7fffffff }, { NULL, 0 } };

extern const FieldParse TheFieldTable[];

void parseNested(INI *ini, void *instance, void *, const void *)
{
	put8(38);
	ini->initFromINI(instance, TheFieldTable);
	put8(39);
}

void catchAll(INI *ini, void *, void *, const void *userData)
{
	put8(41);
	putCStr((const char *)userData);
	for (const char *t = ini->getNextTokenOrNull(); t; t = ini->getNextTokenOrNull()) { put8(42); putCStr(t); }
	put8(43);
}

#define OFF(m) ((Int)offsetof(Rec, m))
const FieldParse TheFieldTable[] =
{
	{ "Byte", L<INI::parseUnsignedByte, K_U8, 1>, NULL, OFF(p.u8) },
	{ "Short", L<INI::parseShort, K_I16, 2>, NULL, OFF(p.i16) },
	{ "UShort", L<INI::parseUnsignedShort, K_U16, 3>, NULL, OFF(p.u16) },
	{ "Int", L<INI::parseInt, K_I32, 4>, NULL, OFF(p.i32) },
	{ "UInt", L<INI::parseUnsignedInt, K_U32, 5>, NULL, OFF(p.u32) },
	{ "Real", L<INI::parseReal, K_F32, 6>, NULL, OFF(p.r) },
	{ "PosReal", L<INI::parsePositiveNonZeroReal, K_F32, 7>, NULL, OFF(p.r) },
	{ "Bool", L<INI::parseBool, K_BOOL, 8>, NULL, OFF(p.b) },
	{ "Bit", L<INI::parseBitInInt32, K_U32, 9>, (const void *)0x00010010, OFF(p.bit) },
	{ "Str", L<INI::parseAsciiString, K_STR, 10>, NULL, OFF(s) },
	{ "QStr", L<INI::parseQuotedAsciiString, K_STR, 11>, NULL, OFF(s) },
	{ "StrVec", L<INI::parseAsciiStringVector, K_VEC, 12>, NULL, OFF(sv) },
	{ "StrVecAdd", L<INI::parseAsciiStringVectorAppend, K_VEC, 13>, NULL, OFF(sv) },
	{ "Percent", L<INI::parsePercentToReal, K_F32, 14>, NULL, OFF(p.r) },
	{ "Bits8", L<INI::parseBitString8, K_U8, 15>, TestNames, OFF(p.bits8) },
	{ "Bits32", L<INI::parseBitString32, K_U32, 16>, TestNames, OFF(p.bits32) },
	{ "RGB", L<INI::parseRGBColor, K_RGB, 17>, NULL, OFF(p.rgb) },
	{ "RGBA", L<INI::parseRGBAColorInt, K_RGBA, 18>, NULL, OFF(p.rgba) },
	{ "Color", L<INI::parseColorInt, K_I32, 19>, NULL, OFF(p.color) },
	{ "Coord3D", L<INI::parseCoord3D, K_C3, 20>, NULL, OFF(p.c3) },
	{ "Coord2D", L<INI::parseCoord2D, K_C2, 21>, NULL, OFF(p.c2) },
	{ "ICoord2D", L<INI::parseICoord2D, K_IC2, 22>, NULL, OFF(p.ic2) },
	{ "Index", L<INI::parseIndexList, K_I32, 23>, TestNames, OFF(p.index) },
	{ "ByteIndex", L<INI::parseByteSizedIndexList, K_U8, 24>, TestNames, OFF(p.bindex) },
	{ "Lookup", L<INI::parseLookupList, K_I32, 25>, TestLookup, OFF(p.lookup) },
	{ "Angle", L<INI::parseAngleReal, K_F32, 26>, NULL, OFF(p.r) },
	{ "AngVel", L<INI::parseAngularVelocityReal, K_F32, 27>, NULL, OFF(p.r) },
	{ "DurReal", L<INI::parseDurationReal, K_F32, 28>, NULL, OFF(p.r) },
	{ "DurUInt", L<INI::parseDurationUnsignedInt, K_U32, 29>, NULL, OFF(p.u32) },
	{ "DurUShort", L<INI::parseDurationUnsignedShort, K_U16, 30>, NULL, OFF(p.dus) },
	{ "Vel", L<INI::parseVelocityReal, K_F32, 31>, NULL, OFF(p.r) },
	{ "Accel", L<INI::parseAccelerationReal, K_F32, 32>, NULL, OFF(p.r) },
	{ "Vet", L<INI::parseVeterancyLevelFlags, K_U32, 33>, NULL, OFF(p.vet) },
	{ "Death", L<INI::parseDeathTypeFlags, K_U32, 34>, NULL, OFF(p.death) },
	{ "Damage", L<INI::parseDamageTypeFlags, K_DMG, 35>, NULL, OFF(damage) },
	{ "Sounds", L<INI::parseSoundsList, K_VEC, 36>, NULL, OFF(sv) },
	{ "RandVar", L<INI::parseGameClientRandomVariable, K_RV, 37>, NULL, OFF(rv) },
	{ "Nested", parseNested, NULL, 0 },
	{ "Dead", NULL, NULL, 0 },
	{ NULL, NULL, NULL, 0 }
};
const FieldParse TheCatchTable[] =
{
	{ "Extra", L<INI::parseInt, K_I32, 40>, NULL, OFF(p.i32) },
	{ NULL, catchAll, NULL, 0 }
};

void buildCatchFirst(MultiIniFieldParse &p)
{
	p.add(TheCatchTable);
	p.add(TheFieldTable);
}

/* ---- File and FileSystem stand-ins */
class MemFile : public File
{
public:
	MemFile(const std::vector<char> &data) : m_bytes(data), m_pos(0) {}
	virtual ~MemFile() {}
	virtual Int read(void *buffer, Int bytes)
	{
		Int avail = (Int)m_bytes.size() - m_pos;
		Int n = bytes < avail ? bytes : avail;
		if (n < 0) n = 0;
		if (n > 0) memcpy(buffer, &m_bytes[m_pos], n);
		m_pos += n;
		return n;
	}
	virtual Int write(const void *, Int) { return -1; }
	virtual Int seek(Int, seekMode) { abort(); }
	virtual void nextLine(Char *, Int) { abort(); }
	virtual Bool scanInt(Int &) { abort(); }
	virtual Bool scanReal(Real &) { abort(); }
	virtual Bool scanString(AsciiString &) { abort(); }
	virtual char *readEntireAndClose() { abort(); }
	virtual File *convertToRAMFile() { return this; }
private:
	std::vector<char> m_bytes;
	Int m_pos;
};

File *openHook(const Char *name, Int access)
{
	std::map<std::string, std::vector<char> >::iterator it = g_files.find(name);
	if (it == g_files.end()) return NULL;
	MemFile *f = new MemFile(it->second);
	f->open(name, access);
	f->deleteOnClose();
	return f;
}

/* storage for TheFileSystem: only its non-virtual openFile and
** getFileListInDirectory are called, and neither touches the object */
alignas(16) unsigned char g_fsStorage[sizeof(FileSystem)];

int finish(int status, unsigned char *log, int cap, int *logLen)
{
	int n = (int)g_log.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(log, &g_log[0], n);
	*logLen = (int)g_log.size();
	return status;
}

/* the INI's throw, as a status: 0 none, 1 INIException (message into msg),
** 2 an INI_* status code, 3 an ErrorCode, 4 std::out_of_range, 5 other.
** The INI_* codes are enumerators of an UNNAMED enum (INI.h), and g++ gives
** an unnamed enum a distinct type in every translation unit (`._anon_N`),
** so no handler here can name the type INI.cpp throws. They are recognised
** instead as the one enum type thrown that is not ErrorCode (checked through
** the ABI's type_info); their value needs no reading, because INI.h defines
** every one of them as ERROR_BAD_INI (asserted below). */
static_assert(INI_INVALID_DATA == ERROR_BAD_INI && INI_UNKNOWN_TOKEN == ERROR_BAD_INI &&
	INI_MISSING_END_TOKEN == ERROR_BAD_INI && INI_INVALID_NAME_LIST == ERROR_BAD_INI &&
	INI_INVALID_PARAMS == ERROR_BAD_INI && INI_INVALID_DIRECTORY == ERROR_BAD_INI &&
	INI_CANT_OPEN_FILE == ERROR_BAD_INI && INI_FILE_ALREADY_OPEN == ERROR_BAD_INI &&
	INI_UNKNOWN_ERROR == ERROR_BAD_INI, "INI_* status codes");
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

/* INIFieldParse access to INI's protected isValidINIFilename */
class IniAccess : public INI
{
public:
	static Bool validName(const char *f) { return isValidINIFilename(f); }
};

/* ---- the block handlers of theTypeTable: five test handlers, and the rest
** throw ERROR_BUG (none of them is translated) */
void INI::parseObjectDefinition(INI *ini)
{
	put8(0xF0); put8(1);
	putCStr(ini->getNextToken());
	Rec rec;
	ini->initFromINI(&rec, TheFieldTable);
	put8(0xF1);
}
WEAK void INI::parseArmorDefinition(INI *ini)
{
	put8(0xF0); put8(2);
	putAscii(ini->getNextAsciiString());
	Rec rec;
	MultiIniFieldParse p;
	p.add(TheFieldTable);
	p.add(TheCatchTable);
	ini->initFromINIMulti(&rec, p);
	put8(0xF1);
}
void INI::parseWeaponTemplateDefinition(INI *ini)
{
	put8(0xF0); put8(3);
	putAscii(ini->getNextQuotedAsciiString());
	Rec rec;
	ini->initFromINIMultiProc(&rec, buildCatchFirst);
	put8(0xF1);
}
void INI::parseCredits(INI *ini)
{
	put8(0xF0); put8(4);
	for (const char *t = ini->getNextTokenOrNull(); t; t = ini->getNextTokenOrNull()) { put8(42); putCStr(t); }
	put8(0xF1);
}
void INI::parseScienceDefinition(INI *ini)
{
	put8(0xF0); put8(5);
	Rec rec;
	ini->initFromINI(&rec, TheFieldTable);
	put8(0xF1);
}

#define STUB(F) void F(INI *) { throw ERROR_BUG; }
#define WEAK_STUB(F) WEAK void F(INI *) { throw ERROR_BUG; }
STUB(INI::parseAIDataDefinition) STUB(INI::parseAnim2DDefinition)
STUB(INI::parseAudioEventDefinition) STUB(INI::parseAudioSettingsDefinition)
STUB(INI::parseTerrainBridgeDefinition) STUB(INI::parseCampaignDefinition)
STUB(INI::parseChallengeModeDefinition) STUB(INI::parseCommandButtonDefinition)
STUB(INI::parseMetaMapDefinition) STUB(INI::parseCommandSetDefinition)
STUB(INI::parseControlBarSchemeDefinition) STUB(INI::parseControlBarResizerDefinition)
WEAK_STUB(INI::parseCrateTemplateDefinition) STUB(INI::parseWindowTransitions)
WEAK_STUB(INI::parseDamageFXDefinition) STUB(INI::parseDialogDefinition)
STUB(INI::parseDrawGroupNumberDefinition) STUB(INI::parseEvaEvent)
STUB(INI::parseFXListDefinition) WEAK_STUB(INI::parseGameDataDefinition) /* p1w2-globaldata: INIGameData.cpp (wrap_globaldata.cpp) defines it */
STUB(INI::parseInGameUIDefinition) STUB(INI::parseLocomotorTemplateDefinition)
STUB(INI::parseLanguageDefinition) STUB(INI::parseMapCacheDefinition)
STUB(INI::parseMapDataDefinition) STUB(INI::parseMappedImageDefinition)
STUB(INI::parseMiscAudio) STUB(INI::parseMouseDefinition) STUB(INI::parseMouseCursorDefinition)
WEAK_STUB(INI::parseMultiplayerColorDefinition) WEAK_STUB(INI::parseMultiplayerStartingMoneyChoiceDefinition)
STUB(INI::parseOnlineChatColorDefinition) WEAK_STUB(INI::parseMultiplayerSettingsDefinition)
STUB(INI::parseMusicTrackDefinition) STUB(INI::parseObjectCreationListDefinition)
STUB(INI::parseObjectReskinDefinition) STUB(INI::parseParticleSystemDefinition)
STUB(INI::parsePlayerTemplateDefinition) STUB(INI::parseTerrainRoadDefinition)
STUB(INI::parseRankDefinition) STUB(INI::parseSpecialPowerDefinition)
STUB(INI::parseShellMenuSchemeDefinition) WEAK_STUB(INI::parseTerrainDefinition)
STUB(INI::parseUpgradeDefinition) STUB(INI::parseVideoDefinition)
STUB(INI::parseWaterSettingDefinition) STUB(INI::parseWaterTransparencyDefinition)
STUB(INI::parseWeatherDefinition) STUB(INI::parseWebpageURLDefinition)
STUB(INI::parseHeaderTemplateDefinition) STUB(INI::parseStaticGameLODDefinition)
STUB(INI::parseDynamicGameLODDefinition) STUB(INI::parseLODPreset) STUB(INI::parseBenchProfile)
STUB(parseReallyLowMHz) STUB(ScriptEngine::parseScriptAction) STUB(ScriptEngine::parseScriptCondition)

/* FileSystem::getFileListInDirectory (FileSystem.cpp, not compiled): the
** registered listing, for INI::loadDirectory */
void FileSystem::getFileListInDirectory(const AsciiString &, const AsciiString &, FilenameList &list, Bool) const
{
	for (size_t i = 0; i < g_dirList.size(); ++i) list.insert(AsciiString(g_dirList[i].c_str()));
}

extern File *(*shim_file_system_open_hook)(const Char *, Int);

extern "C" {

void ref_ini_reset(void)
{
	g_files.clear();
	g_dirList.clear();
	shim_file_system_open_hook = openHook;
	TheFileSystem = (FileSystem *)g_fsStorage;
}

void ref_ini_add_file(const char *name, const char *data, int len)
{
	g_files[name] = std::vector<char>(data, data + len);
}

void ref_ini_add_dir_entry(const char *name)
{
	g_dirList.push_back(name);
}

/* INI::load(filename, INI_LOAD_OVERWRITE, xfer ? &crc : NULL). Returns the
** throw status (see CATCH_ALL), with the log, the XferCRC's getCRC(), and the
** exception message or code. */
int ref_ini_load(const char *filename, int with_xfer, unsigned char *log, int cap, int *logLen,
	unsigned *crc, char *msg, int msgCap, unsigned *code)
{
	g_log.clear();
	XferCRC x;
	int status = 0;
	try {
		INI ini;
		ini.load(AsciiString(filename), INI_LOAD_OVERWRITE, with_xfer ? &x : NULL);
	}
	CATCH_ALL(status, code, msg, msgCap)
	*crc = x.getCRC();
	return finish(status, log, cap, logLen);
}

/* p1w2-terrain: INI::load with the load type given (the crate tests load
** INI_LOAD_CREATE_OVERRIDES files too). The same status mapping as ref_ini_load. */
int ref_ini_load_typed(const char *filename, int loadType, int with_xfer, unsigned char *log, int cap, int *logLen,
	unsigned *crc, char *msg, int msgCap, unsigned *code)
{
	g_log.clear();
	XferCRC x;
	int status = 0;
	try {
		INI ini;
		ini.load(AsciiString(filename), (INILoadType)loadType, with_xfer ? &x : NULL);
	}
	CATCH_ALL(status, code, msg, msgCap)
	*crc = x.getCRC();
	return finish(status, log, cap, logLen);
}

/* INI::loadDirectory(dir, TRUE, INI_LOAD_OVERWRITE, xfer ? &crc : NULL) */
int ref_ini_load_dir(const char *dir, int with_xfer, unsigned char *log, int cap, int *logLen,
	unsigned *crc, char *msg, int msgCap, unsigned *code)
{
	g_log.clear();
	XferCRC x;
	int status = 0;
	try {
		INI ini;
		ini.loadDirectory(AsciiString(dir), TRUE, INI_LOAD_OVERWRITE, with_xfer ? &x : NULL);
	}
	CATCH_ALL(status, code, msg, msgCap)
	*crc = x.getCRC();
	return finish(status, log, cap, logLen);
}

/* the static scanners: op 0 scanInt, 1 scanUnsignedInt, 2 scanReal (bits),
** 3 scanPercentToReal (bits), 4 scanBool, 5 scanIndexList(TestNames),
** 6 scanLookupList(TestLookup), 7 isValidINIFilename, 8 isEndOfBlock,
** 9 isDeclarationOfType("Object", "Tank", token); result in *out; returns
** the throw status */
int ref_ini_scan(int op, const char *token, unsigned *out)
{
	int status = 0;
	unsigned code = 0, *pcode = &code;
	char msg[4];
	try {
		switch (op) {
		case 0: *out = (unsigned)INI::scanInt(token); break;
		case 1: *out = INI::scanUnsignedInt(token); break;
		case 2: { Real r = INI::scanReal(token); memcpy(out, &r, 4); break; }
		case 3: { Real r = INI::scanPercentToReal(token); memcpy(out, &r, 4); break; }
		case 4: *out = INI::scanBool(token) ? 1 : 0; break;
		case 5: *out = (unsigned)INI::scanIndexList(token, TestNames); break;
		case 6: *out = (unsigned)INI::scanLookupList(token, TestLookup); break;
		case 7: *out = IniAccess::validName(token) ? 1 : 0; break;
		case 8: {
			std::vector<char> b(token, token + strlen(token) + 1);
			*out = INI::isEndOfBlock(&b[0]) ? 1 : 0;
			if (strcmp(&b[0], token) != 0) *out = 99; /* must restore the buffer */
			break;
		}
		case 9: {
			std::vector<char> b(token, token + strlen(token) + 1);
			*out = INI::isDeclarationOfType(AsciiString("Object"), AsciiString("Tank"), &b[0]) ? 1 : 0;
			if (strcmp(&b[0], token) != 0) *out = 99;
			break;
		}
		default: abort();
		}
	}
	CATCH_ALL(status, pcode, msg, 0)
	if (status != 0) *out = code;
	return status;
}

} /* extern "C" */
