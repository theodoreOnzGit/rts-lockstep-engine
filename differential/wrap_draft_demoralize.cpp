/* Differential-test wrapper for the DemoralizeSpecialPower draft (#215, upstream commit 0a05454d),
** src/drafts/logic_common/demoralize_special_power.rs:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/DemoralizeSpecialPower.cpp from
**   `void DemoralizeSpecialPower::doSpecialPowerAtLocation` to `void DemoralizeSpecialPower::crc` (doSpecialPowerAtLocation and
**   doSpecialPowerAtObject), cut verbatim by build.rs into demoralize.inc. (Upstream's file is under `#ifdef ALLOW_DEMORALIZE`,
**   which the shipped game does not define; the cut is compiled as if it were.)
**
** Stand-ins (everything the cut calls outside itself; the engine's classes of these names are not read): the class
** DemoralizeSpecialPower, declaring the two functions, on a SpecialPowerModule base whose doSpecialPowerAtLocation records its call
** and whose getObject() is the one object; DemoralizeSpecialPowerModuleData (the seven fields, as the header); Object (isDisabled,
** getContain, getAIUpdateInterface, getPosition); ContainModuleInterface (getContainCount); AIUpdateInterface (setDemoralized records
** its call); the partition filters (constructors only) and KINDOF names; ThePartitionManager->iterateObjectsInRange records the
** location and range and returns the objects the test listed; ObjectIterator (first, next); MemoryPoolObjectHolder (deletes);
** FXList::doFXPos records its call. Every record goes to one log of words.
**
** Entry point (extern "C", tests/differential_draft_demoralize.rs):
**   ref_demoralize_run(in, out): in = [disabled, has_contain, contain_count, base_range bits, bonus_range bits, max_range bits,
**     base_dur, bonus_dur, max_dur, has_fx, kind, loc_null, loc xyz bits, angle bits, options, target_pos xyz bits, target_null,
**     n_objects, then per object [id, has_ai]]; kind 0 = doSpecialPowerAtLocation, 1 = doSpecialPowerAtObject.
**   Writes the log: [1, loc xyz, angle, options] for the base call; [2, range bits, loc xyz] for the scan; [3, id, duration]
**   for each setDemoralized; [4, loc xyz] for the FX. Returns the count of words. */
#include "Lib/BaseType.h"
#include <string.h>
#include <vector>
#define NULL 0

/* every stand-in and the cut are in an anonymous namespace: other differential wrappers define classes of these names (Object, Player,
** FXList ...) with other inline bodies, which must not be merged with these at link time */
namespace {

static std::vector<unsigned> theLog;
static unsigned bits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
static float flt(unsigned u) { float f; memcpy(&f, &u, 4); return f; }

struct FXList
{
	static void doFXPos(const FXList *, const Coord3D *loc) { theLog.push_back(4); theLog.push_back(bits(loc->x)); theLog.push_back(bits(loc->y)); theLog.push_back(bits(loc->z)); }
};

struct AIUpdateInterface
{
	unsigned id;
	void setDemoralized(UnsignedInt duration) { theLog.push_back(3); theLog.push_back(id); theLog.push_back(duration); }
};
struct ContainModuleInterface
{
	unsigned count;
	UnsignedInt getContainCount() const { return count; }
};
struct Object
{
	unsigned id;
	bool disabled;
	ContainModuleInterface *contain;
	AIUpdateInterface *ai;
	Coord3D pos;
	bool isDisabled() const { return disabled; }
	ContainModuleInterface *getContain() const { return contain; }
	AIUpdateInterface *getAIUpdateInterface() const { return ai; }
	const Coord3D *getPosition() const { return &pos; }
};
struct ModuleData {};
struct DemoralizeSpecialPowerModuleData : public ModuleData
{
	Real m_baseRange, m_bonusRangePerCaptured, m_maxRange;
	UnsignedInt m_baseDurationInFrames, m_bonusDurationPerCapturedInFrames, m_maxDurationInFrames;
	const FXList *m_fxList;
};

static Object *theSource = 0;
static Coord3D lastBaseLoc;

class SpecialPowerModule
{
public:
	virtual ~SpecialPowerModule() {}
	virtual void doSpecialPowerAtLocation(const Coord3D *loc, Real angle, UnsignedInt commandOptions)
	{
		theLog.push_back(1); theLog.push_back(bits(loc->x)); theLog.push_back(bits(loc->y)); theLog.push_back(bits(loc->z));
		theLog.push_back(bits(angle)); theLog.push_back(commandOptions);
	}
	Object *getObject() const { return theSource; }
};

class DemoralizeSpecialPower : public SpecialPowerModule
{
public:
	const DemoralizeSpecialPowerModuleData *md;
	virtual void doSpecialPowerAtLocation(const Coord3D *loc, Real angle, UnsignedInt commandOptions);
	virtual void doSpecialPowerAtObject(const Object *obj, Real angle, UnsignedInt commandOptions);
	const DemoralizeSpecialPowerModuleData *getDemoralizeSpecialPowerModuleData() const { return md; }
};

enum KindOfType { KINDOF_INFANTRY };
#define MAKE_KINDOF_MASK(k) (k)
#define KINDOFMASK_NONE 0
enum DistanceCalculationType { FROM_CENTER_2D };
struct PartitionFilter { virtual ~PartitionFilter() {} };
struct PartitionFilterRelationship : public PartitionFilter
{
	enum { ALLOW_ENEMIES = 1, ALLOW_NEUTRAL = 2 };
	PartitionFilterRelationship(Object *, unsigned) {}
};
struct PartitionFilterAcceptByKindOf : public PartitionFilter { PartitionFilterAcceptByKindOf(int, int) {} };
struct PartitionFilterSameMapStatus : public PartitionFilter { PartitionFilterSameMapStatus(Object *) {} };
static std::vector<Object *> theScanned;
struct ObjectIterator
{
	size_t at;
	Object *first() { at = 0; return at < theScanned.size() ? theScanned[at] : 0; }
	Object *next() { ++at; return at < theScanned.size() ? theScanned[at] : 0; }
};
struct MemoryPoolObjectHolder { ObjectIterator *it; MemoryPoolObjectHolder(ObjectIterator *i) : it(i) {} ~MemoryPoolObjectHolder() { delete it; } };
struct PartitionManagerStandIn
{
	ObjectIterator *iterateObjectsInRange(const Coord3D *loc, Real range, DistanceCalculationType, PartitionFilter **)
	{
		theLog.push_back(2); theLog.push_back(bits(range));
		theLog.push_back(bits(loc->x)); theLog.push_back(bits(loc->y)); theLog.push_back(bits(loc->z));
		return new ObjectIterator;
	}
};
static PartitionManagerStandIn thePM;
static PartitionManagerStandIn *ThePartitionManager = &thePM;

#include "demoralize.inc"

}

extern "C" int ref_demoralize_run(const unsigned *in, unsigned *out)
{
	theLog.clear();
	DemoralizeSpecialPowerModuleData md;
	md.m_baseRange = flt(in[3]); md.m_bonusRangePerCaptured = flt(in[4]); md.m_maxRange = flt(in[5]);
	md.m_baseDurationInFrames = in[6]; md.m_bonusDurationPerCapturedInFrames = in[7]; md.m_maxDurationInFrames = in[8];
	md.m_fxList = in[9] ? (const FXList *)1 : 0;
	ContainModuleInterface contain; contain.count = in[2];
	Object src; src.id = 1; src.disabled = in[0] != 0; src.contain = in[1] ? &contain : 0; src.ai = 0;
	src.pos.x = src.pos.y = src.pos.z = 0;
	theSource = &src;
	Coord3D loc; loc.x = flt(in[12]); loc.y = flt(in[13]); loc.z = flt(in[14]);
	Real angle = flt(in[15]);
	unsigned opts = in[16];
	Object target; target.id = 2; target.disabled = false; target.contain = 0; target.ai = 0;
	target.pos.x = flt(in[17]); target.pos.y = flt(in[18]); target.pos.z = flt(in[19]);
	unsigned n = in[21];
	std::vector<Object> objs(n);
	std::vector<AIUpdateInterface> ais(n);
	theScanned.clear();
	for (unsigned i = 0; i < n; ++i) {
		objs[i].id = in[22 + 2 * i]; objs[i].disabled = false; objs[i].contain = 0;
		ais[i].id = objs[i].id;
		objs[i].ai = in[23 + 2 * i] ? &ais[i] : 0;
		theScanned.push_back(&objs[i]);
	}
	DemoralizeSpecialPower p;
	p.md = &md;
	if (in[10] == 0) p.doSpecialPowerAtLocation(in[11] ? 0 : &loc, angle, opts);
	else p.doSpecialPowerAtObject(in[20] ? 0 : &target, angle, opts);
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}
