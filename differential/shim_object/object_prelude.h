/* Forced include (-include) for differential/wrap_object.cpp, differential tests only (p1w6-object).
**
** The stand-ins the compiled text of Object.cpp needs: the engine's basic types, the bit-flag masks, the
** classes of the units Object calls into (Team, Player, the partition manager, the radar, the AI, the
** script engine, the audio, the module interfaces and helpers, ...) and the Xfer classes. Nothing here is
** upstream text, except where a comment says so (the bodies that are copies of one-liners). Every stand-in
** that has an effect records it as a RECORD (see `emit_record`): the Rust side records the same calls
** (`CutCall`, `ObjectClientEvent`, `ModuleCall`) and the test compares the two streams word for word.
**
** All class and global names that could clash with another fixture's stand-in are renamed by macro to
** `ObjOp...`, as in the other wrappers, so that no symbol of this archive collides with another's. */
#ifndef SHIM_OBJECT_PRELUDE_H
#define SHIM_OBJECT_PRELUDE_H

#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <math.h>
#include <string>
#include <map>
#include <set>
#include <list>
#include <vector>
#include <bitset>
#include <algorithm>

/* ---- renames -------------------------------------------------------------------------------- */
#define Object ObjOpObject
#define Thing ObjOpThing
#define Team ObjOpTeam
#define TeamPrototype ObjOpTeamPrototype
#define Player ObjOpPlayer
#define Drawable ObjOpDrawable
#define Module ObjOpModule
#define BehaviorModule ObjOpBehaviorModule
#define UpdateModule ObjOpUpdateModule
#define ThingTemplate ObjOpThingTemplate
#define GeometryInfo ObjOpGeometryInfo
#define Xfer ObjOpXfer
#define XferCRC ObjOpXferCRC
#define XferSave ObjOpXferSave
#define XferLoad ObjOpXferLoad
#define Snapshot ObjOpSnapshot
#define SightingInfo ObjOpSightingInfo
#define ExperienceTracker ObjOpExperienceTracker
#define WeaponSet ObjOpWeaponSet
#define AsciiString ObjOpAsciiString
#define Coord3D ObjOpCoord3D
#define ICoord3D ObjOpICoord3D
#define Coord2D ObjOpCoord2D
#define Region3D ObjOpRegion3D
#define Matrix3D ObjOpMatrix3D
#define Vector3 ObjOpVector3
#define DamageInfo ObjOpDamageInfo
#define DamageInfoInput ObjOpDamageInfoInput
#define DamageInfoOutput ObjOpDamageInfoOutput
#define PolygonTrigger ObjOpPolygonTrigger
#define MemoryPoolObject ObjOpMemoryPoolObject
#define MemoryPoolObjectHolder ObjOpMemoryPoolObjectHolder
#define ObjectIterator ObjOpObjectIterator
#define PartitionFilter ObjOpPartitionFilter
#define PartitionFilterAcceptByKindOf ObjOpPartitionFilterAcceptByKindOf
#define PartitionFilterSameMapStatus ObjOpPartitionFilterSameMapStatus
#define AudioEventRTS ObjOpAudioEventRTS
#define TTriggerInfo ObjOpTTriggerInfo
#define BitFlags ObjOpBitFlags
#define Rec ObjOpRec
#define ModuleData ObjOpModuleData
#define NameKeyType ObjOpNameKeyType
#define TheGameLogic ObjOpTheGameLogic
#define GameLogic ObjOpGameLogic
#define ThePlayerList ObjOpThePlayerList
#define PlayerList ObjOpPlayerList
#define TheTeamFactory ObjOpTheTeamFactory
#define TeamFactory ObjOpTeamFactory
#define ThePartitionManager ObjOpThePartitionManager
#define PartitionManager ObjOpPartitionManager
#define PartitionData ObjOpPartitionData
#define TheRadar ObjOpTheRadar
#define Radar ObjOpRadar
#define RadarObject ObjOpRadarObject
#define TheAudio ObjOpTheAudio
#define TheInGameUI ObjOpTheInGameUI
#define TheControlBar ObjOpTheControlBar
#define TheEva ObjOpTheEva
#define TheGameClient ObjOpTheGameClient
#define TheScriptEngine ObjOpTheScriptEngine
#define TheAI ObjOpTheAI
#define TheTerrainLogic ObjOpTheTerrainLogic
#define TheGlobalData ObjOpTheGlobalData
#define TheSpecialPowerStore ObjOpTheSpecialPowerStore
#define TheUpgradeCenter ObjOpTheUpgradeCenter
#define TheBuildAssistant ObjOpTheBuildAssistant
#define TheModuleFactory ObjOpTheModuleFactory
#define TheNameKeyGenerator ObjOpTheNameKeyGenerator
#define UpgradeTemplate ObjOpUpgradeTemplate
#define SpecialPowerTemplate ObjOpSpecialPowerTemplate
#define ObjectSMCHelper ObjOpObjectSMCHelper
#define StatusDamageHelper ObjOpStatusDamageHelper
#define SubdualDamageHelper ObjOpSubdualDamageHelper
#define ObjectRepulsorHelper ObjOpObjectRepulsorHelper
#define ObjectDefectionHelper ObjOpObjectDefectionHelper
#define ObjectWeaponStatusHelper ObjOpObjectWeaponStatusHelper
#define FiringTracker ObjOpFiringTracker
#define TempWeaponBonusHelper ObjOpTempWeaponBonusHelper

/* MSVC's min and max macros (Object.cpp uses them) */
#define min(a, b) (((a) < (b)) ? (a) : (b))
#define max(a, b) (((a) > (b)) ? (a) : (b))
#define MIN(a, b) (((a) < (b)) ? (a) : (b))
#define MAX(a, b) (((a) > (b)) ? (a) : (b))

/* the compiled text reaches into classes the real headers keep private */
#define private public
#define protected public

/* the upstream functions this archive links against (defined in the global namespace, by upstream_gameengine) */
extern float Cos(float);
extern float Sin(float);
extern float normalizeAngle(float);

namespace ObjOpWorld {

/* ---- basic types and macros (BaseType.h, GameType.h, ...) ------------------------------------ */
typedef int Int;
typedef unsigned int UnsignedInt;
typedef float Real;
typedef bool Bool;
typedef unsigned char UnsignedByte;
typedef signed char Byte;
typedef unsigned short UnsignedShort;
typedef long long Int64;
typedef char Char;
typedef unsigned int Color;
#define TRUE true
#define FALSE false
#ifndef NULL
#define NULL 0
#endif
#define BitTest(x, i) (((x) & (i)) != 0)
#define BitSet(x, i) ((x) |= (i))
#define BitClear(x, i) ((x) &= ~(i))
#define INT_TO_REAL(x) ((Real)(x))
static inline Real fast_float_trunc(Real f) { return truncf(f); }
static inline Int fast_float2long_round(Real f) { return (Int)lrintf(f); }
#define REAL_TO_INT(x) ((Int)(fast_float2long_round(fast_float_trunc(x))))
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define DEBUG_CRASH(m) ((void)0)
#define DEBUG_LOG(m) ((void)0)
#define USE_PERF_TIMER(x)
#define MSGNEW(x) new
#define newInstance(T) new T
#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(C, s) public: virtual ~C();
#define MEMORY_POOL_GLUE_ABC(C) public: virtual ~C();
#define MEMORY_POOL_DELETEINSTANCE_VISIBILITY(v)
#define MAKE_DLINK(C, name)
#define GameMakeColor(r, g, b, a) ((Color)(((a) << 24) | ((r) << 16) | ((g) << 8) | (b)))
#define _isnan(x) isnan(x)
#define PI 3.14159265358979323846f

#include "enums.inc"
typedef int ObjectScriptStatusBit_;
enum { LOGICFRAMES_PER_SECOND = 30 };
enum { NEVER = 0, FOREVER = 0x3fffffff };
enum { CONSTRUCTION_COMPLETE = -1 };
enum { MAX_PLAYER_COUNT = 16 };
typedef UnsignedShort PlayerMaskType;
enum { PLAYERMASK_ALL = 0xffff, PLAYERMASK_NONE = 0 };

enum ObjectID { INVALID_ID = 0, FORCE_OBJECTID_TO_LONG_SIZE = 0x7ffffff };
enum DrawableID { INVALID_DRAWABLE_ID = 0, FORCE_DRAWABLE_TO_LONG_SIZE = 0x7ffffff };
typedef UnsignedInt TeamID;
enum { TEAM_ID_INVALID = 0 };
enum FormationID { NO_FORMATION_ID = 0, FORCE_FORMATION_TO_LONG_SIZE = 0x7ffffff };
enum CommandSourceType { CMD_FROM_PLAYER = 0, CMD_FROM_SCRIPT, CMD_FROM_AI };
enum NameKeyType { NAMEKEY_INVALID = 0 };
enum { ALLOW_SAME_PLAYER = 1, ALLOW_ALLIES = 2, ALLOW_ENEMIES = 4, ALLOW_NEUTRAL = 8 };
enum { FROM_CENTER_2D = 0 };
enum { ITER_SORTED_NEAR_TO_FAR = 0 };
enum { WSF_INVALID = -1, WSF_NONE = 0, WSF_FIRING, WSF_BETWEEN, WSF_RELOADING, WSF_PREATTACK, WSF_COUNT };
enum { WORLD_ANIM_FADE_ON_EXPIRE = 1 };
enum StaticGameLODLevelStub { UNUSED_LOD };
enum { MODULETYPE_BEHAVIOR = 0 };
enum { DOZER_TASK_INVALID = -1 };
enum { LOCOMOTORSURFACE_AIR = 8 };
enum CanAttackResult { ATTACKRESULT_NOT_POSSIBLE = 0 };
enum AbleToAttackType { ATTACK_STUB = 0 };
enum WeaponChoiceCriteria { PREFER_MOST_DAMAGE = 0 };
enum WeaponLockType { NOT_LOCKED = 0 };

/* DamageType and DeathType (Damage.h), the ones used */
#define HUGE_DAMAGE_AMOUNT 999999.0f
#define WEAPONSLOT_COUNT_STUB 3

/* GameType enumerators used by the masks: the same numbers as the Rust tables */
typedef UnsignedInt WeaponBonusConditionFlags;
enum StealthStub { STEALTH_STUB };

/* ---- the name tables of the flag types (set by the driver: the same strings as the Rust tables) ---- */
extern const char* const* g_flagNames[4]; /* 0 object status, 1 disabled, 2 weapon set, 3 special power */
extern int g_flagNameCount[4];

/* ---- output: records as text lines -------------------------------------------------------------
** The Rust side formats a CutCall, an ObjectClientEvent or a ModuleCall name with `{:?}` and turns it into
** the same tokens: "Name field value field value ...", reals as `f:<8 hex digits of the bits>`, booleans
** `true`/`false`, enum variants by their Rust name, `Some Coord3D x .. y .. z ..` / `None` for an
** Option<Coord3D>. A record is "<tag> <tokens>" with the tag `cut`, `evt` or `mod`. */
extern std::vector<std::string>* g_lines;
static inline unsigned fbits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
struct Rec
{
	std::string s;
	Rec(const char* tag, const char* name) { s = tag; s += ' '; s += name; }
	~Rec() { if (g_lines) g_lines->push_back(s); }
	Rec& i(const char* k, long long v) { char b[64]; snprintf(b, sizeof b, " %s %lld", k, v); s += b; return *this; }
	Rec& b(const char* k, bool v) { s += ' '; s += k; s += v ? " true" : " false"; return *this; }
	Rec& f(const char* k, float v) { char b[64]; snprintf(b, sizeof b, " %s f:%08x", k, fbits(v)); s += b; return *this; }
	Rec& e(const char* k, const char* v) { s += ' '; s += k; s += ' '; s += v; return *this; }
	Rec& coord(const char* k, const struct Coord3D* c, bool opt = true);
	Rec& ints(const char* k, const std::vector<int>& v) { if (k) { s += ' '; s += k; } for (size_t n = 0; n < v.size(); ++n) { char b[32]; snprintf(b, sizeof b, " %d", v[n]); s += b; } return *this; }
};
#define CUT(name) Rec("cut", name)
#define EVT(name) Rec("evt", name)
#define MODC(name) Rec("mod", name)

/* ---- strings, vectors, matrices ---------------------------------------------------------------- */
class AsciiString
{
public:
	std::string s;
	static const AsciiString TheEmptyString;
	AsciiString() {}
	AsciiString(const char* c) : s(c ? c : "") {}
	Bool isEmpty() const { return s.empty(); }
	Bool isNotEmpty() const { return !s.empty(); }
	const char* str() const { return s.c_str(); }
	void clear() { s.clear(); }
	int compare(const char* c) const { return s.compare(c); }
	Bool operator==(const AsciiString& o) const { return s == o.s; }
	Bool operator!=(const AsciiString& o) const { return s != o.s; }
	void format(const char* f, ...);
};

struct Coord2D { Real x, y; };
#include "basetype_coords.inc"
struct Vector3
{
	Real X, Y, Z;
	Vector3() : X(0), Y(0), Z(0) {}
	Vector3(Real a, Real b, Real c) : X(a), Y(b), Z(c) {}
	void Set(Real a, Real b, Real c) { X = a; Y = b; Z = c; }
	void Normalize() { Real len2 = X * X + Y * Y + Z * Z; if (len2 != 0.0f) { Real oo = 1.0f / sqrtf(len2); X *= oo; Y *= oo; Z *= oo; } }
};
struct Matrix3D
{
	Real Row[3][4];
	Matrix3D() { Make_Identity(); }
	void Make_Identity() { memset(Row, 0, sizeof(Row)); Row[0][0] = Row[1][1] = Row[2][2] = 1.0f; }
	void Set(Real m11, Real m12, Real m13, Real m14, Real m21, Real m22, Real m23, Real m24, Real m31, Real m32, Real m33, Real m34)
	{
		Row[0][0] = m11; Row[0][1] = m12; Row[0][2] = m13; Row[0][3] = m14;
		Row[1][0] = m21; Row[1][1] = m22; Row[1][2] = m23; Row[1][3] = m24;
		Row[2][0] = m31; Row[2][1] = m32; Row[2][2] = m33; Row[2][3] = m34;
	}
	void Set_X_Translation(Real x) { Row[0][3] = x; }
	void Set_Y_Translation(Real y) { Row[1][3] = y; }
	void Set_Z_Translation(Real z) { Row[2][3] = z; }
	Real Get_X_Translation() const { return Row[0][3]; }
	Real Get_Y_Translation() const { return Row[1][3]; }
	Real Get_Z_Translation() const { return Row[2][3]; }
	Real Get_Z_Rotation() const { return atan2f(Row[1][0], Row[0][0]); }
	Vector3 Get_X_Vector() const { return Vector3(Row[0][0], Row[1][0], Row[2][0]); }
	void Transform_Vector(const Matrix3D& tm, const Vector3& in, Vector3* out) const
	{
		Real x = tm.Row[0][0] * in.X + tm.Row[0][1] * in.Y + tm.Row[0][2] * in.Z + tm.Row[0][3];
		Real y = tm.Row[1][0] * in.X + tm.Row[1][1] * in.Y + tm.Row[1][2] * in.Z + tm.Row[1][3];
		Real z = tm.Row[2][0] * in.X + tm.Row[2][1] * in.Y + tm.Row[2][2] * in.Z + tm.Row[2][3];
		out->X = x; out->Y = y; out->Z = z;
	}
	void mul(const Matrix3D& a, const Matrix3D& b)
	{
		Matrix3D t;
		for (int r = 0; r < 3; ++r)
		{
			for (int c = 0; c < 3; ++c)
				t.Row[r][c] = a.Row[r][0] * b.Row[0][c] + a.Row[r][1] * b.Row[1][c] + a.Row[r][2] * b.Row[2][c];
			t.Row[r][3] = a.Row[r][0] * b.Row[0][3] + a.Row[r][1] * b.Row[1][3] + a.Row[r][2] * b.Row[2][3] + a.Row[r][3];
		}
		*this = t;
	}
};

/* ---- Xfer (Xfer.h, XferCRC.cpp, XferSave.cpp: the primitives the compiled text uses) --------------- */
enum XferMode { XFER_INVALID = 0, XFER_SAVE, XFER_LOAD, XFER_CRC };
typedef unsigned char XferVersion;
enum SaveCode { SC_INVALID_DATA = 1 };
class Snapshot;
class Xfer
{
public:
	XferMode m_xferMode;
	Xfer() : m_xferMode(XFER_INVALID) {}
	virtual ~Xfer() {}
	XferMode getXferMode() const { return m_xferMode; }
	virtual void xferImplementation(void* data, Int size) = 0;
	virtual void xferSnapshot(Snapshot* snapshot) = 0;
	virtual Int beginBlock() = 0;
	virtual void endBlock() = 0;
	virtual void skip(Int n) = 0;
	virtual void xferVersion(XferVersion* v, XferVersion cur) { xferImplementation(v, sizeof(XferVersion)); }
	void xferByte(Byte* b) { xferImplementation(b, sizeof(Byte)); }
	void xferUnsignedByte(UnsignedByte* b) { xferImplementation(b, sizeof(UnsignedByte)); }
	void xferBool(Bool* b) { xferImplementation(b, sizeof(Bool)); }
	void xferInt(Int* i) { xferImplementation(i, sizeof(Int)); }
	void xferUnsignedInt(UnsignedInt* i) { xferImplementation(i, sizeof(UnsignedInt)); }
	void xferUnsignedShort(UnsignedShort* i) { xferImplementation(i, sizeof(UnsignedShort)); }
	void xferReal(Real* r) { xferImplementation(r, sizeof(Real)); }
	void xferUser(void* data, Int size) { xferImplementation(data, size); }
	void xferObjectID(ObjectID* id) { xferImplementation(id, sizeof(ObjectID)); }
	void xferDrawableID(DrawableID* id) { xferImplementation(id, sizeof(DrawableID)); }
	void xferColor(Color* c) { xferImplementation(c, sizeof(Color)); }
	void xferCoord3D(Coord3D* c) { xferReal(&c->x); xferReal(&c->y); xferReal(&c->z); }
	void xferICoord3D(ICoord3D* c) { xferInt(&c->x); xferInt(&c->y); xferInt(&c->z); }
	void xferCoord2D(Coord2D* c) { xferReal(&c->x); xferReal(&c->y); }
	virtual void xferAsciiString(AsciiString* s) = 0;
	void xferMatrix3D(Matrix3D* m)
	{
		XferVersion v = 1;
		xferVersion(&v, 1);
		for (int r = 0; r < 3; ++r)
			for (int c = 0; c < 4; ++c)
				xferReal(&m->Row[r][c]);
	}
	void xferUpgradeMask(void* mask); /* defined with the mask type below */
};
class Snapshot
{
public:
	virtual ~Snapshot() {}
	virtual void crc(Xfer* xfer) = 0;
	virtual void xfer(Xfer* xfer) = 0;
	virtual void loadPostProcess() = 0;
};

/* XferCRC.cpp lines 30-160, as a stand-in (copied): the word-wise CRC */
static inline UnsignedInt objHtonl(UnsignedInt v) { return ((v & 0xff) << 24) | ((v & 0xff00) << 8) | ((v >> 8) & 0xff00) | (v >> 24); }
class XferCRC : public Xfer
{
public:
	UnsignedInt m_crc;
	XferCRC() { m_xferMode = XFER_CRC; m_crc = 0; }
	void addCRC(UnsignedInt val)
	{
		int hibit;
		val = objHtonl(val);
		if (m_crc & 0x80000000) hibit = 1; else hibit = 0;
		m_crc <<= 1;
		m_crc += val;
		m_crc += hibit;
	}
	Int beginBlock() { return 0; }
	void endBlock() {}
	void skip(Int) {}
	void xferAsciiString(AsciiString*) {}
	void xferSnapshot(Snapshot* snapshot) { if (snapshot) snapshot->crc(this); }
	void xferImplementation(void* data, Int dataSize)
	{
		if (!data || dataSize < 1) return;
		const UnsignedInt* uintPtr = (const UnsignedInt*)(data);
		for (Int i = 0; i < dataSize / 4; i++)
			addCRC(*uintPtr++);
		int leftover = dataSize & 3;
		if (leftover)
		{
			UnsignedInt val = 0;
			const unsigned char* c = (const unsigned char*)uintPtr;
			for (Int i = 0; i < leftover; i++)
				val += (c[i] << (i * 8));
			val = objHtonl(val);
			addCRC(val);
		}
	}
	UnsignedInt getCRC() { return objHtonl(m_crc); }
};

/* a save into memory, with XferSave.cpp's block sizes (a patched Int in front of the block) and strings
** (an UnsignedByte length, the bytes) */
class XferSave : public Xfer
{
public:
	std::vector<unsigned char> bytes;
	std::vector<size_t> blocks;
	XferSave() { m_xferMode = XFER_SAVE; }
	void xferImplementation(void* data, Int size) { const unsigned char* p = (const unsigned char*)data; bytes.insert(bytes.end(), p, p + size); }
	void xferSnapshot(Snapshot* s) { if (s) s->xfer(this); }
	Int beginBlock() { blocks.push_back(bytes.size()); Int z = 0; xferImplementation(&z, sizeof(Int)); return 0; }
	void endBlock()
	{
		size_t at = blocks.back();
		blocks.pop_back();
		Int size = (Int)(bytes.size() - at - sizeof(Int));
		memcpy(&bytes[at], &size, sizeof(Int));
	}
	void skip(Int) {}
	void xferAsciiString(AsciiString* s)
	{
		UnsignedByte len = (UnsignedByte)s->s.size();
		xferUnsignedByte(&len);
		if (len) xferImplementation((void*)s->s.data(), len);
	}
};

/* a load from memory */
class XferLoad : public Xfer
{
public:
	std::vector<unsigned char> bytes;
	size_t at;
	XferLoad() : at(0) { m_xferMode = XFER_LOAD; }
	void xferImplementation(void* data, Int size)
	{
		if (at + size > bytes.size()) { memset(data, 0, size); at = bytes.size(); return; }
		memcpy(data, &bytes[at], size);
		at += size;
	}
	void xferSnapshot(Snapshot* s) { if (s) s->xfer(this); }
	Int beginBlock() { Int size = 0; xferImplementation(&size, sizeof(Int)); return size; }
	void endBlock() {}
	void skip(Int n) { at += n; }
	void xferAsciiString(AsciiString* s)
	{
		UnsignedByte len = 0;
		xferUnsignedByte(&len);
		s->s.assign(len, '\0');
		if (len) xferImplementation(&s->s[0], len);
	}
};

/* ---- the bit-flag masks (BitFlags.h, BitFlagsIO.h xfer) ---------------------------------------------- */
template <int NUMBITS, int KIND>
class BitFlags
{
public:
	std::bitset<NUMBITS> m_bits;
	enum { kInit = 1 };
	BitFlags() {}
	BitFlags(int, int a) { set(a, 1); }
	BitFlags(int, int a, int b) { set(a, 1); set(b, 1); }
	BitFlags(int, int a, int b, int c) { set(a, 1); set(b, 1); set(c, 1); }
	Bool test(int i) const { return m_bits.test((size_t)i); }
	void set(int i, int val) { m_bits.set((size_t)i, val != 0); }
	void set(int i) { m_bits.set((size_t)i, true); }
	void set(const BitFlags& o) { m_bits |= o.m_bits; }
	void clear() { m_bits.reset(); }
	void clear(const BitFlags& o) { m_bits &= ~o.m_bits; }
	Bool any() const { return m_bits.any(); }
	Int count() const { return (Int)m_bits.count(); }
	Int size() const { return NUMBITS; }
	Bool testForAll(const BitFlags& o) const { return (m_bits & o.m_bits) == o.m_bits; }
	Bool testForAny(const BitFlags& o) const { return (m_bits & o.m_bits).any(); }
	Bool anyIntersectionWith(const BitFlags& o) const { return (m_bits & o.m_bits).any(); }
	Int countInverseIntersection(const BitFlags& o) const { return (Int)(m_bits & ~o.m_bits).count(); }
	void clearAndSet(const BitFlags& clr, const BitFlags& set_) { m_bits &= ~clr.m_bits; m_bits |= set_.m_bits; }
	Bool operator==(const BitFlags& o) const { return m_bits == o.m_bits; }
	Bool operator!=(const BitFlags& o) const { return m_bits != o.m_bits; }
	void xfer(Xfer* xfer)
	{
		/* BitFlagsIO.h lines 159-235 */
		XferVersion currentVersion = 1;
		XferVersion version = currentVersion;
		xfer->xferVersion(&version, currentVersion);
		if (xfer->getXferMode() == XFER_SAVE)
		{
			Int c = count();
			xfer->xferInt(&c);
			for (Int i = 0; i < size(); ++i)
			{
				if (!test(i)) continue;
				AsciiString bitNameA = (KIND >= 0 && i < g_flagNameCount[KIND]) ? g_flagNames[KIND][i] : "";
				xfer->xferAsciiString(&bitNameA);
			}
		}
		else if (xfer->getXferMode() == XFER_LOAD)
		{
			clear();
			Int c;
			xfer->xferInt(&c);
			AsciiString string;
			for (Int i = 0; i < c; ++i)
			{
				xfer->xferAsciiString(&string);
				for (int k = 0; k < g_flagNameCount[KIND]; ++k)
					if (string.s == g_flagNames[KIND][k]) { set(k, 1); break; }
			}
		}
		else if (xfer->getXferMode() == XFER_CRC)
		{
			UnsignedInt low = (UnsignedInt)(m_bits.to_ullong() & 0xffffffffu);
			xfer->xferUser(&low, sizeof(low));
		}
	}
};
typedef BitFlags<OBJECT_STATUS_COUNT, 0> ObjectStatusMaskType;
typedef BitFlags<DISABLED_COUNT, 1> DisabledMaskType;
typedef BitFlags<WEAPONSET_COUNT, 2> WeaponSetFlags;
typedef BitFlags<SPECIALPOWER_COUNT, 3> SpecialPowerMaskType;
typedef BitFlags<128, -1> UpgradeMaskType;
typedef BitFlags<KINDOF_COUNT, -1> KindOfMaskType;
typedef BitFlags<MODELCONDITION_COUNT, -1> ModelConditionFlags;
typedef BitFlags<ARMORSET_COUNT, -1> ArmorSetFlags;
#define MAKE_OBJECT_STATUS_MASK(k) ObjectStatusMaskType(ObjectStatusMaskType::kInit, (k))
#define MAKE_OBJECT_STATUS_MASK2(k, a) ObjectStatusMaskType(ObjectStatusMaskType::kInit, (k), (a))
#define MAKE_MODELCONDITION_MASK(k) ModelConditionFlags(ModelConditionFlags::kInit, (k))
#define MAKE_KINDOF_MASK(k) KindOfMaskType(KindOfMaskType::kInit, (k))
extern KindOfMaskType KINDOFMASK_NONE_;
extern KindOfMaskType KINDOFMASK_FS_;
#define KINDOFMASK_NONE KINDOFMASK_NONE_
#define KINDOFMASK_FS KINDOFMASK_FS_
extern DisabledMaskType DISABLEDMASK_NONE_;
#define DISABLEDMASK_NONE DISABLEDMASK_NONE_
#define TEST_DISABLEDMASK(m, t) ((m).test(t))
#define SET_SPECIALPOWERMASK(m, t) ((m).set(t, 1))
#define TEST_SPECIALPOWERMASK(m, t) ((m).test(t))
#define SPECIALPOWERMASK_ANY_SET(m) ((m).any())
#define OBJ_UPGRADE_NAMES_STUB 0


} /* namespace ObjOpWorld */
using namespace ObjOpWorld;

#endif
