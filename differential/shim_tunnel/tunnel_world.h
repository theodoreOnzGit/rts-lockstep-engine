/* The stand-ins of the units TunnelTracker.cpp and CaveSystem.cpp call into (the second half of
** p1w7-sonnet): the objects of the test world, TheGameLogic, the partition manager, the pathfinder, the
** drawable and the body module. Every call that has an effect appends a line to g_tunnelLog, which the Rust
** side writes for the same call. The class names are renamed by macros in wrap_tunnel_tracker.cpp. */
#ifndef SHIM_TUNNEL_WORLD_H
#define SHIM_TUNNEL_WORLD_H
#include <string>
#include <vector>
#include <list>
#include "Lib/BaseType.h"
#include "Common/GameType.h"
#include "Common/KindOf.h"
#include "Common/ObjectStatusTypes.h"
#ifndef PI
#define PI     3.14159265359f
#define TWO_PI 6.28318530718f
#endif
#include "Common/GameCommon.h"

/* GameState.h's SaveCode: the value is not read */
enum SaveCode { SC_INVALID_DATA = 1 };
enum DamageType { DAMAGE_EXPLOSION = 0, DAMAGE_HEALING = 10, DAMAGE_UNRESISTABLE = 11 };
enum DeathType { DEATH_NORMAL = 0, DEATH_NONE = 1 };
/* DamageInfoInput::DamageInfoInput() of Damage.h, the fields TunnelTracker reads and the defaults it leaves */
class DamageInfoInput
{
public:
	DamageInfoInput() { m_sourceID = INVALID_ID; m_damageType = DAMAGE_EXPLOSION; m_damageFXOverride = DAMAGE_UNRESISTABLE;
		m_deathType = DEATH_NORMAL; m_amount = 0; m_kill = FALSE; }
	ObjectID m_sourceID; DamageType m_damageType; DamageType m_damageFXOverride; DeathType m_deathType; Real m_amount; Bool m_kill;
};
class DamageInfoOutput { public: Real m_actualDamageDealt; };
class DamageInfo { public: DamageInfoInput in; DamageInfoOutput out; };

extern std::vector<std::string> g_tunnelLog;
void tunnelLogf(const char *fmt, ...);

class Drawable { public: void setDrawableHidden(Bool hidden); int m_owner; };
class BodyModuleInterface
{
public:
	Real getMaxHealth() const { return m_maxHealth; }
	void attemptHealing(DamageInfo *info);
	int m_owner; Real m_maxHealth; Real m_health;
};
class Object
{
public:
	Object() : m_id(INVALID_ID), m_kinds(0), m_status(0), m_effectivelyDead(false), m_frame(0), m_containedBy(NULL), m_group(0), m_destroyed(false),
		m_hidden(false), m_hasDrawable(false), m_hasBody(false), m_exists(true) { m_drawable.m_owner = 0; m_body.m_owner = 0; m_body.m_maxHealth = 0; m_body.m_health = 0; }
	ObjectID getID() const { return m_id; }
	Bool isKindOf(int t) const { return (m_kinds >> (unsigned)t) & 1u; }
	Bool testStatus(ObjectStatusTypes s) const
	{
		switch (s) { case OBJECT_STATUS_STEALTHED: return m_status & 1; case OBJECT_STATUS_DETECTED: return m_status & 2;
			case OBJECT_STATUS_DISGUISED: return m_status & 4; default: return false; }
	}
	Bool isEffectivelyDead() const { return m_effectivelyDead; }
	Object *getContainedBy() { return m_containedBy; }
	const Object *getContainedBy() const { return m_containedBy; }
	UnsignedInt getContainedByFrame() const { return m_frame; }
	void onContainedBy(Object *c);
	void onRemovedFrom(Object *r);
	void leaveGroup();
	Drawable *getDrawable() { return m_hasDrawable ? &m_drawable : NULL; }
	BodyModuleInterface *getBodyModule() { return m_hasBody ? &m_body : NULL; }

	ObjectID m_id; unsigned m_kinds; unsigned m_status; Bool m_effectivelyDead; UnsignedInt m_frame; Object *m_containedBy; UnsignedInt m_group;
	Bool m_destroyed, m_hidden, m_hasDrawable, m_hasBody, m_exists; Drawable m_drawable; BodyModuleInterface m_body;
};
class GameLogic
{
public:
	UnsignedInt getFrame() const { return m_frame; }
	Object *findObjectByID(ObjectID id);
	void destroyObject(Object *obj);
	UnsignedInt m_frame;
};
extern GameLogic *TheGameLogic;
class GlobalData { public: Int m_maxTunnelCapacity; };
extern GlobalData *TheGlobalData;
class PartitionManager { public: void unRegisterObject(Object *obj); };
extern PartitionManager *ThePartitionManager;
class Pathfinder { public: void removeObjectFromPathfindMap(Object *obj); };
class AI { public: Pathfinder *pathfinder() { return &m_pathfinder; } Pathfinder m_pathfinder; };
extern AI *TheAI;
#endif
