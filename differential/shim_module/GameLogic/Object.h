/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8), Module build
** only (differential tests, p1w3-modules): an Object that is a Thing and has
** what the module files call: the veterancy level and the status bits
** (DieMuxData::isDieApplicable), and removeUpgrade
** (UpgradeMuxData::muxDataProcessUpgradeRemoval), which is defined in
** differential/wrap_module.cpp and logs the template it is given. The class
** is renamed by the wrapper's macros (module_prelude.h). */
#ifndef SHIM_MODULE_OBJECT_H
#define SHIM_MODULE_OBJECT_H
#include "Common/Thing.h"
#include "Common/GameCommon.h"
#include "Common/ObjectStatusTypes.h"
class UpgradeTemplate;
/* --- p1l9-example --- */
#include "Common/Upgrade.h"
#include "Common/AsciiString.h"
class Player;
/* --- end p1l9-example --- */
/* --- p1w10-sonnet --- */
#include "GameLogic/WeaponSetType.h"
#include "GameLogic/Weapon.h"
#include "Common/ModelState.h"
#include "Common/GameType.h"
/* GameLogic/WeaponSet.h lines 87-106 (commit 0a05454d), verbatim: the real header needs the weapon layer */
static const ModelConditionFlagType TheWeaponSetTypeToModelConditionTypeMap[WEAPONSET_COUNT] =
{
	/*WEAPONSET_VETERAN*/								MODELCONDITION_WEAPONSET_VETERAN,		
	/*WEAPONSET_ELITE*/									MODELCONDITION_WEAPONSET_ELITE,
	/*WEAPONSET_HERO*/									MODELCONDITION_WEAPONSET_HERO,
	/*WEAPONSET_PLAYER_UPGRADE*/				MODELCONDITION_WEAPONSET_PLAYER_UPGRADE,
	/*WEAPONSET_CRATEUPGRADE_ONE*/			MODELCONDITION_WEAPONSET_CRATEUPGRADE_ONE,
	/*WEAPONSET_CRATEUPGRADE_TWO*/			MODELCONDITION_WEAPONSET_CRATEUPGRADE_TWO,
	/*WEAPONSET_VEHICLE_HIJACK*/				MODELCONDITION_INVALID,
	/*WEAPONSET_CARBOMB*/								MODELCONDITION_INVALID,
	/*WEAPONSET_MINE_CLEARING_DETAIL*/	MODELCONDITION_INVALID,
	/*WEAPONSET_RIDER1*/								MODELCONDITION_RIDER1,
	/*WEAPONSET_RIDER2*/								MODELCONDITION_RIDER2,
	/*WEAPONSET_RIDER3*/								MODELCONDITION_RIDER3,
	/*WEAPONSET_RIDER4*/								MODELCONDITION_RIDER4,
	/*WEAPONSET_RIDER5*/								MODELCONDITION_RIDER5,
	/*WEAPONSET_RIDER6*/								MODELCONDITION_RIDER6,
	/*WEAPONSET_RIDER7*/								MODELCONDITION_RIDER7,
	/*WEAPONSET_RIDER8*/								MODELCONDITION_RIDER8,
};
class BodyModuleInterface;
class Drawable;
class ExperienceTracker;
/* --- end p1w10-sonnet --- */
class Object : public Thing
{
public:
	Object() : m_level(LEVEL_REGULAR), m_player(0), m_body(0), m_drawable(0), m_xp(0), m_weaponBonusCondition(0), m_shroudRange(0), m_producerID((ObjectID)0) { m_weaponSetFlags[0] = m_weaponSetFlags[1] = 0; }
	VeterancyLevel getVeterancyLevel() const { return m_level; }
	const ObjectStatusMaskType &getStatusBits() const { return m_status; }
	void removeUpgrade(const UpgradeTemplate *t);
	VeterancyLevel m_level;
	ObjectStatusMaskType m_status;
	/* --- p1l9-example --- */
	/* what CommandSetUpgrade reads and writes; m_player is NULL unless a test gives one */
	Player *getControllingPlayer() const { return m_player; }
	UpgradeMaskType getObjectCompletedUpgradeMask() const { return m_objectUpgradesCompleted; }
	void setCommandSetStringOverride(AsciiString s) { m_commandSetStringOverride = s; }
	Player *m_player;
	UpgradeMaskType m_objectUpgradesCompleted;
	AsciiString m_commandSetStringOverride;
	/* --- end p1l9-example --- */
	/* --- p1w10-sonnet --- (the layer-9 upgrade batch: what the eight upgrade modules call on their object; every call is
	** a line of the effect log, the state is what the dump reads) */
	void setWeaponSetFlag(WeaponSetType wst);
	void setWeaponBonusCondition(WeaponBonusConditionType wst);
	BodyModuleInterface *getBodyModule() const { return m_body; }
	Drawable *getDrawable() const { return m_drawable; }
	ExperienceTracker *getExperienceTracker() const { return m_xp; }
	void setShroudRange(Real r);
	void handlePartitionCellMaintenance();
	void setModelConditionState(ModelConditionFlagType a);
	BodyModuleInterface *m_body;
	Drawable *m_drawable;
	ExperienceTracker *m_xp;
	unsigned long long m_weaponSetFlags[2];
	unsigned m_weaponBonusCondition;
	Real m_shroudRange;
	/* --- end p1w10-sonnet --- */
	/* --- p1w10-sonnet --- (the layer-9 helper and die batch: what ObjectHelper's subclasses and DestroyDie/UpgradeDie call on their object; every
	** call is a line of the helper log of differential/l9_helpers.inc) */
	void setStatus(ObjectStatusMaskType objectStatus, Bool set = true);
	inline void clearStatus(ObjectStatusMaskType objectStatus) { setStatus(objectStatus, false); }
	void clearSpecialModelConditionStates();
	void adjustModelConditionForWeaponStatus();
	void clearWeaponBonusCondition(WeaponBonusConditionType wst);
	ObjectID getProducerID() const { return m_producerID; }
	Bool hasUpgrade(const UpgradeTemplate *upgradeT) const;   /* Object.cpp:4456, in l9_helpers.inc */
	ObjectID m_producerID;
	/* --- end p1w10-sonnet --- */
protected:
	virtual Object *asObjectMeth() { return this; }
	virtual const Object *asObjectMeth() const { return this; }
};
#endif
