/* Differential-test wrapper for the physics drafts (#215, upstream commit 0a05454d), src/drafts/sweep_gui_logic/physics_update.rs
** (get_z_friction, is_currently_overlapped, was_previously_overlapped), cut verbatim by build.rs (drafts_guilogic3):
**   phys_consts.inc  PhysicsUpdate.cpp, MIN_NON_AERO_FRICTION and MAX_FRICTION;
**   phys_z.inc       getZFriction;   phys_overlap.inc  isCurrentlyOverlapped, wasPreviouslyOverlapped.
** Stand-ins: Real is float; `Object` has only getID() (the real class is the whole game object); PhysicsBehavior holds the
** members the cuts read (m_extraFriction, m_currentOverlap, m_previousOverlap) and `getPhysicsBehaviorModuleData()` returns a
** struct with m_ZFriction (the module data is an INI-loaded class). The class's other members are not used by the cuts.
**
** Entry points (extern "C", tests/differential_draft_phys.rs):
**   ref_phys_zfriction(z, extra) -> float;   ref_phys_overlap(which, cur, prev, has_obj, obj_id) which 0 isCurrently, 1 wasPreviously. */
typedef float Real;
typedef int Bool;
typedef int ObjectID;
#define NULL 0

class Object { public: ObjectID getID() const { return m_id; } ObjectID m_id; };
struct PhysicsBehaviorModuleData { Real m_ZFriction; };

class PhysicsBehavior
{
public:
	const PhysicsBehaviorModuleData *getPhysicsBehaviorModuleData() const { return &m_data; }
	Real getZFriction() const;
	Bool isCurrentlyOverlapped(Object *obj) const;
	Bool wasPreviouslyOverlapped(Object *obj) const;
	PhysicsBehaviorModuleData m_data;
	Real m_extraFriction;
	ObjectID m_currentOverlap;
	ObjectID m_previousOverlap;
};

#include "phys_consts.inc"
#include "phys_z.inc"
#include "phys_overlap.inc"

extern "C" float ref_phys_zfriction(float z, float extra)
{
	PhysicsBehavior p;
	p.m_data.m_ZFriction = z;
	p.m_extraFriction = extra;
	return p.getZFriction();
}

extern "C" int ref_phys_overlap(int which, int cur, int prev, int has_obj, int obj_id)
{
	PhysicsBehavior p;
	p.m_currentOverlap = cur;
	p.m_previousOverlap = prev;
	Object o;
	o.m_id = obj_id;
	Object *op = has_obj ? &o : NULL;
	return which == 0 ? p.isCurrentlyOverlapped(op) : p.wasPreviouslyOverlapped(op);
}
