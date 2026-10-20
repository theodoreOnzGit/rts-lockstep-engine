/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8), create-module build only (p1l9-creates):
** the members the create modules call, each logging through `crt_log` (defined in wrap_create_modules.cpp). The
** class is renamed to CrtObject by create_prelude.h. */
#ifndef SHIM_CREATES_OBJECT_H
#define SHIM_CREATES_OBJECT_H
#include "Common/Thing.h"
#include "Common/GameCommon.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/ModelState.h"
#include "GameLogic/WeaponSet.h"
class Player;
class UpgradeTemplate;
class ExperienceTracker;
class BehaviorModule;
class Object : public Thing
{
public:
	Object() : m_id(0), m_controller(0), m_experience(0), m_behaviors(0) {}
	Player *getControllingPlayer() const;
	const ObjectStatusMaskType &getStatusBits() const { return m_status; }
	ExperienceTracker *getExperienceTracker() { return m_experience; }
	BehaviorModule **getBehaviorModules() const; // logs "behaviors <id>"
	void setModelConditionState(ModelConditionFlagType a);
	void clearModelConditionState(ModelConditionFlagType a);
	Bool setWeaponLock(WeaponSlotType weaponSlot, WeaponLockType lockType);
	void giveUpgrade(const UpgradeTemplate *upgradeT);

protected:
	/* Thing's AsObject hook: BehaviorModule::getObject goes through it (as shim_module/GameLogic/Object.h) */
	virtual Object *asObjectMeth() { return this; }
	virtual const Object *asObjectMeth() const { return this; }
public:
	unsigned m_id;
	Player *m_controller;
	ObjectStatusMaskType m_status;
	ExperienceTracker *m_experience;
	BehaviorModule **m_behaviors;
};
#endif
