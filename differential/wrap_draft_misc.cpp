/* Differential-test wrapper for the small object accessor drafts (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/misc_object_accessors.rs, cut verbatim by build.rs (drafts_guilogic3) into:
**   misc_track.inc    RailroadGuideAIUpdate.cpp, TrainTrack::incReference, releaseReference;
**   misc_hack.inc     HackInternetAIUpdate.cpp, isHacking;
**   misc_worker.inc   WorkerAIUpdate.cpp, getRepairHealthPerSecond, getBoredTime;   misc_dozer.inc  DozerAIUpdate.cpp, the same two;
**   misc_jet1.inc     JetAIUpdate.cpp, friend_setAllowAirLoco;   misc_jet2.inc  friend_addWaypointToGoalPath;
**   misc_salvage.inc  SalvageCrateCollide.cpp, doLevelGain;   misc_crate.inc  CreateCrateDie.cpp, testVeterancyLevel;
**   misc_victim.inc   Object.cpp, getLastVictimID;   misc_power.inc  SpecialPowerModule.cpp, getPowerName.
** Built on the REAL AsciiString (shim_ascii; classes renamed by build.rs), for getPowerName's result.
** Stand-ins (outside the cuts): TrainTrack (m_refCount); HackInternetAIUpdate (getStateMachine()->getCurrentStateID(), the id the test sets;
** HACK_INTERNET is the enumerator HackInternetAIUpdate.h:126 gives a value in the module's state enum: here 4, as the draft takes it as a
** parameter); WorkerAIUpdate and DozerAIUpdate (the module data, with m_repairHealthPercentPerSecond and m_boredTime); JetAIUpdate (m_flags,
** the FlagType enum of JetAIUpdate.h:153-163, setFlag exactly as JetAIUpdate.cpp:97 defines it, and privateFollowPathAppend, which logs its
** pointer's Coord3D and its CommandSourceType); SalvageCrateCollide's `other->getExperienceTracker()->gainExpForLevel(1)` (the tracker logs the
** levels it is asked for); CreateCrateDie (getObject()->getVeterancyLevel(), CrateTemplate::m_veterancyLevel); Object (m_firingTracker with
** getLastShotVictim(), INVALID_ID as GameType.h declares it: 0); SpecialPowerModule (the module data's m_specialPowerTemplate->getName()).
**
** Entry points (extern "C", tests/differential_draft_misc.rs): ref_misc_track(start, in, n, out), ref_misc_hack(state), ref_misc_float(kind, bits),
** ref_misc_flags(flags, allow), ref_misc_waypoint(xbits, ybits, zbits, out), ref_misc_levelgain(), ref_misc_vet(crate, obj),
** ref_misc_victim(has_tracker, victim), ref_misc_power(name, out). */
#include <string.h>
#include <vector>

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theMiscAllocator;

#define NULL 0
typedef int ObjectID;
enum { INVALID_ID = 0 };
enum CommandSourceType { CMD_FROM_PLAYER = 0, CMD_FROM_SCRIPT, CMD_FROM_AI, CMD_FROM_DOZER };
enum VeterancyLevel { LEVEL_REGULAR = 0, LEVEL_VETERAN, LEVEL_ELITE, LEVEL_HEROIC };

class TrainTrack { public: void incReference(); Bool releaseReference(); Int m_refCount; };
#include "misc_track.inc"

enum { HACK_INTERNET = 4 };
struct StateMachineStandIn { Int getCurrentStateID() const { return m_id; } Int m_id; };
class HackInternetAIUpdate { public: Bool isHacking() const; StateMachineStandIn *getStateMachine() const { return &m_sm; } mutable StateMachineStandIn m_sm; };
#include "misc_hack.inc"

struct WorkerAIUpdateModuleData { Real m_repairHealthPercentPerSecond; Real m_boredTime; };
class WorkerAIUpdate { public: Real getRepairHealthPerSecond( void ) const; Real getBoredTime( void ) const; const WorkerAIUpdateModuleData *getWorkerAIUpdateModuleData() const { return &m_d; } WorkerAIUpdateModuleData m_d; };
#include "misc_worker.inc"
struct DozerAIUpdateModuleData { Real m_repairHealthPercentPerSecond; Real m_boredTime; };
class DozerAIUpdate { public: Real getRepairHealthPerSecond( void ) const; Real getBoredTime( void ) const; const DozerAIUpdateModuleData *getDozerAIUpdateModuleData() const { return &m_d; } DozerAIUpdateModuleData m_d; };
#include "misc_dozer.inc"

static std::vector<int> theAppend;
class JetAIUpdate
{
public:
	void friend_setAllowAirLoco(Bool allowAirLoco);
	void friend_addWaypointToGoalPath( const Coord3D &bestPos );
	void privateFollowPathAppend( const Coord3D *pos, CommandSourceType cmdSource )
	{
		union { Real f; int i; } x, y, z;
		x.f = pos->x; y.f = pos->y; z.f = pos->z;
		theAppend.push_back(x.i); theAppend.push_back(y.i); theAppend.push_back(z.i); theAppend.push_back((int)cmdSource);
	}
	enum FlagType { HAS_PENDING_COMMAND, ALLOW_AIR_LOCO, HAS_PRODUCER_LOCATION, TAKEOFF_IN_PROGRESS, LANDING_IN_PROGRESS, USE_SPECIAL_RETURN_LOCO, ALLOW_CIRCLING, ALLOW_INTERRUPT_AND_RESUME_OF_CUR_STATE_FOR_RELOAD, TAXI_IN_PROGRESS };
	void setFlag( FlagType f, Bool v)
	{
		if (v)
			m_flags |= (1<<f);
		else
			m_flags &= ~(1<<f);
	}
	Int m_flags;
};
#include "misc_jet1.inc"
#include "misc_jet2.inc"

static std::vector<int> theLevels;
struct ExperienceTrackerStandIn { Bool gainExpForLevel(Int levelsToGain, Bool canScaleForBonus = TRUE) { theLevels.push_back(levelsToGain); return TRUE; } };
class Object
{
public:
	ExperienceTrackerStandIn *getExperienceTracker() { static ExperienceTrackerStandIn t; return &t; }
	VeterancyLevel getVeterancyLevel() const { return m_level; }
	ObjectID getLastVictimID() const;
	VeterancyLevel m_level;
	struct FiringTrackerStandIn { ObjectID getLastShotVictim() const { return m_v; } ObjectID m_v; } *m_firingTracker;
};
class SalvageCrateCollide { public: void doLevelGain( Object *other ); };
#include "misc_salvage.inc"
struct CrateTemplate { VeterancyLevel m_veterancyLevel; };
class CreateCrateDie { public: Bool testVeterancyLevel( CrateTemplate const *currentCrateData ); Object *getObject() { return &m_o; } Object m_o; };
#include "misc_crate.inc"
#include "misc_victim.inc"

class SpecialPowerTemplateStandIn { public: AsciiString getName() const { return m_name; } AsciiString m_name; };
struct SpecialPowerModuleData { const SpecialPowerTemplateStandIn *m_specialPowerTemplate; };
class SpecialPowerModule { public: AsciiString getPowerName( void ) const; const SpecialPowerModuleData *getSpecialPowerModuleData() const { return &m_d; } SpecialPowerModuleData m_d; };
#include "misc_power.inc"

static int bitsOf(Real f) { union { Real f; int i; } u; u.f = f; return u.i; }
static Real realOf(int i) { union { Real f; int i; } u; u.i = i; return u.f; }

extern "C" int ref_misc_track(int start, const int *in, int n, int *out)
{
	TrainTrack t; t.m_refCount = start;
	int k = 0;
	for (int i = 0; i < n; ++i) { if (in[i] == 0) t.incReference(); else out[k++] = t.releaseReference(); }
	out[k++] = t.m_refCount;
	return k;
}
extern "C" int ref_misc_hack(int state) { HackInternetAIUpdate h; h.m_sm.m_id = state; return h.isHacking(); }
extern "C" int ref_misc_float(int kind, int bits)
{
	WorkerAIUpdate w; DozerAIUpdate d;
	w.m_d.m_repairHealthPercentPerSecond = realOf(bits); w.m_d.m_boredTime = realOf(bits ^ 0x00000101);
	d.m_d.m_repairHealthPercentPerSecond = realOf(bits ^ 0x00000202); d.m_d.m_boredTime = realOf(bits ^ 0x00000404);
	switch (kind)
	{
		case 0: return bitsOf(w.getRepairHealthPerSecond());
		case 1: return bitsOf(w.getBoredTime());
		case 2: return bitsOf(d.getRepairHealthPerSecond());
		default: return bitsOf(d.getBoredTime());
	}
}
extern "C" int ref_misc_flags(int flags, int allow) { JetAIUpdate j; j.m_flags = flags; j.friend_setAllowAirLoco(allow != 0); return j.m_flags; }
extern "C" int ref_misc_waypoint(int x, int y, int z, int *out)
{
	theAppend.clear();
	JetAIUpdate j; Coord3D p; p.x = realOf(x); p.y = realOf(y); p.z = realOf(z);
	j.friend_addWaypointToGoalPath(p);
	for (size_t i = 0; i < theAppend.size(); ++i) out[i] = theAppend[i];
	return (int)theAppend.size();
}
extern "C" int ref_misc_levelgain() { theLevels.clear(); SalvageCrateCollide s; Object o; s.doLevelGain(&o); return theLevels.size() == 1 ? theLevels[0] : -1000; }
extern "C" int ref_misc_vet(int crate, int obj) { CreateCrateDie d; CrateTemplate c; c.m_veterancyLevel = (VeterancyLevel)crate; d.m_o.m_level = (VeterancyLevel)obj; return d.testVeterancyLevel(&c); }
extern "C" int ref_misc_victim(int has, int v) { Object o; Object::FiringTrackerStandIn t; t.m_v = v; o.m_firingTracker = has ? &t : NULL; return o.getLastVictimID(); }
extern "C" int ref_misc_power(const char *name, char *out)
{
	TheDynamicMemoryAllocator = &theMiscAllocator;
	SpecialPowerTemplateStandIn t; t.m_name = AsciiString(name);
	SpecialPowerModule m; m.m_d.m_specialPowerTemplate = &t;
	AsciiString r = m.getPowerName();
	int n = (int)strlen(r.str());
	memcpy(out, r.str(), n + 1);
	return n;
}
