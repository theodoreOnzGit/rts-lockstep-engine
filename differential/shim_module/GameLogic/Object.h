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
class AIUpdateInterface;
class ContainModuleInterface;
class SpawnBehaviorInterface;
#include "Common/KindOf.h"
class Drawable;
class ExperienceTracker;
/* --- end p1w10-sonnet --- */
/* --- p1w12-sonnet --- */
#include "Common/NameKeyGenerator.h"
class BehaviorModule;
class UpdateModule;
class Team;
#include "matrix3d.h"
#include "Common/DisabledTypes.h"
#include "Common/ThingTemplate.h"
/* --- end p1w12-sonnet --- */
/* --- p1w13-sonnet --- */
#include "GameLogic/Damage.h"
#include "Common/SpecialPowerType.h"
class PhysicsBehavior;
class SpecialAbilityUpdate;
/* batch P (ActiveBody): GameLogic/WeaponSet.h lines 188-195 (the real header needs the weapon layer), verbatim apart from the comments */
class ProjectileUpdateInterface;
enum CanAttackResult
{
	ATTACKRESULT_NOT_POSSIBLE,					///< can't possibly attack the target
	ATTACKRESULT_INVALID_SHOT,					///< not a clear shot
	ATTACKRESULT_POSSIBLE_AFTER_MOVING,	///< possible, after moving closer
	ATTACKRESULT_POSSIBLE,							///< possible now
};
/* --- end p1w13-sonnet --- */
/* --- p1w12-sonnet --- (Common/STLTypedefs.h lines 88-89, verbatim) */
#include <list>
#include "Common/Geometry.h"
class Object;
class DamageInfo;
typedef std::list<Object *>																ObjectPointerList;
typedef std::list<Object *>::iterator											ObjectPointerListIterator;
/* --- end p1w12-sonnet --- */
class Object : public Thing
{
public:
	Object() : m_level(LEVEL_REGULAR), m_player(0), m_body(0), m_drawable(0), m_xp(0), m_weaponBonusCondition(0), m_shroudRange(0), m_producerID((ObjectID)0), m_ai(0), m_contain(0), m_spawn(0), m_disabled(false), m_behaviors(0), m_team(0), m_oid(0), m_destroyed(false), m_next(0), m_disabledBits(0), m_geometry(GEOMETRY_SPHERE, false, 1.0f, 1.0f, 1.0f) { m_pos.x = m_pos.y = m_pos.z = 0; m_weaponSetFlags[0] = m_weaponSetFlags[1] = 0; }
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
	void clearWeaponSetFlag(WeaponSetType wst);   /* p1w13-sonnet, batch R2 (TransportContain): defined in differential/l9_open_contain_r.inc */
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
	/* --- p1w10-sonnet --- (the layer-9 upgrade batch B: what StatusBitsUpgrade, LocomotorSetUpgrade, PassengersFireUpgrade, StealthUpgrade,
	** SubObjectsUpgrade and ObjectCreationUpgrade call on their object; the modules the object "has" are the test's) */
	AIUpdateInterface *getAIUpdateInterface() const { return m_ai; }
	ContainModuleInterface *getContain() const { return m_contain; }
	SpawnBehaviorInterface *getSpawnBehaviorInterface() const { return m_spawn; }
	Bool isKindOf(KindOfType t) const { return m_kindOf.test(t); }
	AIUpdateInterface *m_ai;
	ContainModuleInterface *m_contain;
	SpawnBehaviorInterface *m_spawn;
	KindOfMaskType m_kindOf;
	/* --- end p1w10-sonnet --- */
	/* --- p1w12-sonnet --- (upgrade batch C: what PowerPlantUpgrade, RadarUpgrade and UnpauseSpecialPowerUpgrade call on their object; the
	** behaviour modules are the test's, a null-terminated array as upstream's m_behaviors) */
	Bool isDisabled() const { return m_disabled; }
	BehaviorModule **getBehaviorModules() const { return m_behaviors; }
	UpdateModule *findUpdateModule(NameKeyType key) const;   /* defined in l9_upgrades_c.inc */
	Bool m_disabled;
	BehaviorModule **m_behaviors;
	/* ReplaceObjectUpgrade: the object's place, team and id, and whether destroyObject has been called on it */
	const Matrix3D *getTransformMatrix() const { return &m_matrix; }
	void setTransformMatrix(const Matrix3D *mx);   /* defined in l9_replace_object.inc (logs) */
	Team *getTeam() const { return m_team; }
	Matrix3D m_matrix;
	Team *m_team;
	Int m_oid;
	Bool m_destroyed;
	/* die batch D: the game logic's object list, the position, the disabled bits and the template of the object */
	Object *getNextObject() const { return m_next; }
	const Coord3D *getPosition() const { return &m_pos; }
	ObjectID getID() const { return (ObjectID)m_oid; }
	const ThingTemplate *getTemplate() const { return &m_tmpl; }
	void clearDisabled(DisabledType type);              /* differential/l9_dies_d.inc */
	Bool isSignificantlyAboveTerrain();                 /* differential/l9_dies_d.inc */
	/* crate batch F: what CrateCollide asks of the object that collides (the test's) and of the crate (differential/l9_crates_f.inc) */
	void setPosition(const Coord3D *pos) { m_pos = *pos; }   /* special power batch H: the view object and the detonation object are placed */
	void defect(Team *newTeam, UnsignedInt detectionTime);   /* l9_powers_h.inc */
	void setShroudClearingRange(Real r);   /* l9_powers_h.inc */
	Bool isNeutralControlled() const;   /* l9_crates_f.inc: Player is incomplete here */
	/* update batch L: what AutoDepositUpdate asks of its object (differential/l9_updates_l.inc) */
	/* update batch M: FireOCLAfterWeaponCooldownUpdate asks for the current weapon and the weapon in a slot (the test's) */
	Weapon *getCurrentWeapon(WeaponSlotType *wslot = NULL) { if (wslot && m_curWeapon) *wslot = m_curWeapon->getWeaponSlot(); return m_curWeapon; }
	Weapon *getWeaponInWeaponSlot(WeaponSlotType wslot) const { return m_slotWeapon[wslot]; }
	Weapon *m_curWeapon = 0;
	Weapon *m_slotWeapon[3] = { 0, 0, 0 };
	Bool isLocallyControlled() const;
	Real getConstructionPercent() const { return m_constructionPercent; }
	Bool testStatus(ObjectStatusTypes bit) const { return m_status.test(bit); }
	/* update batch N: the REAL GeometryInfo (Geometry.cpp, compiled by wrap_geometry.cpp) */
	const GeometryInfo &getGeometryInfo() const { return m_geometry; }
	void setGeometryInfo(const GeometryInfo &geom);   /* differential/l9_checkpoint_n.inc */
	Real m_constructionPercent = -1.0f;   /* CONSTRUCTION_COMPLETE */
	GeometryInfo m_geometry;
	Bool isKindOfMulti(const KindOfMaskType &mustBeSet, const KindOfMaskType &mustBeClear) const { return m_kindOf.testSetAndClear(mustBeSet, mustBeClear); }
	Bool isEffectivelyDead() const { return m_effDead; }
	Bool isAboveTerrain() const { return m_pos.z > 0; }   /* the test's terrain is flat at zero */
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	Bool isUsingAirborneLocomotor() const { return m_airborneLoco; }
	Bool m_airborneLoco = false;
	Int getTransportSlotCount() const;   /* Object.cpp, verbatim: differential/l9_open_contain_r.inc */
	class DockUpdateInterface *getDockUpdateInterface( void );   /* batch R3: Object.cpp, verbatim: differential/l9_open_contain_r.inc */
	/* Thing.h: the test's bone table holds world positions already */
	/* p1w13-sonnet, batch R8: Thing's, through the transform setOrientation makes from m_orientation and m_pos (differential/l9_open_contain_r.inc) */
	void convertBonePosToWorldPos(const Coord3D* bonePos, const Matrix3D* boneTransform, Coord3D* worldPos, Matrix3D* worldTransform) const;
	class ExitInterface *getObjectExitInterface() const;   /* batch R8: Object.cpp's, verbatim (differential/l9_open_contain_r.inc) */
	/* --- end p1w13-sonnet --- */
	Bool m_effDead = false;
	/* update batch M: what AutoHealBehavior asks of the objects it scans, and the healing functions (cut verbatim from Object.cpp by build.rs,
	** mod_gen/ObjectHealing.inc, compiled by differential/l9_auto_heal_m.inc) */
	Bool isOffMap() const { return m_offMap; }
	Bool isAnyKindOf(const KindOfMaskType &anyKindOf) const { return m_kindOf.anyIntersectionWith(anyKindOf); }
	/* asm labels: the other module-build units (wrap_l9updates.cpp) define their own ModTestObject::attemptHealing with another layout */
	void attemptHealing(Real amount, const Object *source) __asm__("l9m_Object_attemptHealing");
	ObjectID getSoleHealingBenefactor(void) const __asm__("l9m_Object_getSoleHealingBenefactor");
	Bool attemptHealingFromSoleBenefactor(Real amount, const Object *source, UnsignedInt duration) __asm__("l9m_Object_attemptHealingFromSoleBenefactor");
	Bool m_offMap = false;
	/* update batch N: CheckpointUpdate */
	Real getVisionRange() const { return m_vision; }
	Real getOrientation() const { return m_orientation; }   /* update batch N: OCLUpdate */
	Real m_orientation = 0.0f;
	/* update batch N: FlammableUpdate (differential/l9_flammable_n.inc) */
	void attemptDamage( DamageInfo *damageInfo );
	void clearModelConditionState(ModelConditionFlagType a);
	Real m_vision = 0.0f;
	ObjectID m_soleHealingBenefactorID = INVALID_ID;
	UnsignedInt m_soleHealingBenefactorExpirationFrame = 0;
	/* helper batch G: the defector flag ObjectDefectionHelper reads and clears */
	Bool getIsUndetectedDefector(void) const { return m_undetected; }
	void friend_setUndetectedDefector(Bool status) { m_undetected = status != 0; }
	Bool m_undetected = false;
	Object *m_next;
	Coord3D m_pos;
	unsigned m_disabledBits;
	ThingTemplate m_tmpl;
	/* --- end p1w12-sonnet --- */
	/* --- p1w13-sonnet --- (update batch O: HeightDieUpdate asks what contains the object, and kills it; differential/l9_height_die_o.inc) */
	Object *getContainedBy() { return m_containedBy; }
	const Object *getContainedBy() const { return m_containedBy; }
	void kill( DamageType damageType = DAMAGE_UNRESISTABLE, DeathType deathType = DEATH_NORMAL );
	Object *m_containedBy = 0;
	void setTeam( Team *team );   /* update batch O: TechBuildingBehavior (differential/l9_tech_building_o.inc) */
	/* update batch O: SquishCollide (differential/l9_squish_collide_o.inc) */
	AIUpdateInterface *getAI() const { return m_ai; }
	UnsignedByte getCrusherLevel() const { return m_crusher; }
	PhysicsBehavior *getPhysics() const { return m_physics; }
	SpecialAbilityUpdate *findSpecialAbilityUpdate( SpecialPowerType type ) const { return (m_sauType == type) ? m_sau : 0; }
	Relationship getRelationship( const Object *that ) const;   /* Object.cpp, cut by build.rs (mod_gen/ObjectRelationship.inc) */
	UnsignedByte m_crusher = 0;
	PhysicsBehavior *m_physics = 0;
	SpecialAbilityUpdate *m_sau = 0;
	int m_sauType = -1;
	/* DynamicShroudClearingRangeUpdate reads the range that setShroudClearingRange (l9_powers_h.inc) writes */
	Real getShroudClearingRange() const { return m_shroudRange; }
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch P: ActiveBody; the calls are lines of the effect log, differential/l9_active_body_p.inc) */
	void scoreTheKill( const Object *victim );
	void onDie( DamageInfo *damageInfo );
	void setDisabled( DisabledType type );
	void doStatusDamage( ObjectStatusTypes status, Real duration );
	void notifySubdualDamage( Real amount );
	void setEffectivelyDead( Bool dead ) { m_effDead = dead; }
	void setGeometryInfoZ( Real newZ );
	ProjectileUpdateInterface *getProjectileUpdateInterface() const;
	Int getMultiLogicalBonePosition( const char *boneNamePrefix, Int maxBones, Coord3D *positions, Matrix3D *transforms, Bool convertToWorld = TRUE ) const;
	Bool isAirborneTarget() const { return m_status.test( OBJECT_STATUS_AIRBORNE_TARGET ); }
	Bool isFactionStructure() const;
	CanAttackResult getAbleToAttackSpecificObject( AbleToAttackType t, const Object *target, CommandSourceType commandSource, WeaponSlotType specificSlot = (WeaponSlotType)-1 ) const;
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (SlowDeathBehavior; differential/l9_slow_death_q.inc) */
	Real getHeightAboveTerrain() const { return m_pos.z; }   /* the test's terrain is flat at zero */
	void setPositionZ( Real z ) { m_pos.z = z; }
	Bool isDisabledByType( DisabledType type ) const { return ( m_disabledBits >> type ) & 1; }
	void clearAndSetModelConditionFlags( const ModelConditionFlags &clr, const ModelConditionFlags &set );
	void clearModelConditionFlags( const ModelConditionFlags &clr );
	void setModelConditionFlags( const ModelConditionFlags &set );
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R: OpenContain; the calls are lines of the effect log, differential/l9_open_contain_r.inc) */
	PathfindLayerEnum getLayer() const { return m_layer; }
	void setLayer( PathfindLayerEnum layer );
	void setOrientation( Real angle );
	void setSafeOcclusionFrame( UnsignedInt frame );   /* p1w13-sonnet, batch R10: TunnelContain (differential/l9_open_contain_r.inc) */
	void leaveGroup();
	Bool checkAndDetonateBoobyTrap( const Object *victim );
	void onContainedBy( Object *containedBy );
	void onRemovedFrom( Object *removedFrom );
	class StealthUpdate *getStealth() const { return m_stealth; }
	Bool getSingleLogicalBonePosition( const char *boneName, Coord3D *position, Matrix3D *transform ) const;
	Bool getSingleLogicalBonePositionOnTurret( WhichTurretType whichTurret, const char *boneName, Coord3D *position, Matrix3D *transform ) const;
	void clearAndSetModelConditionState( ModelConditionFlagType clr, ModelConditionFlagType set );
	UnsignedInt getWeaponBonusCondition() const { return m_weaponBonusCondition; }
	void friend_setContainedBy( Object *containedBy ) { m_containedBy = containedBy; }
	/* p1w13-sonnet, batch R7: HealContain reads when the object went in (Object.h) */
	UnsignedInt getContainedByFrame() const { return m_containedByFrame; }
	UnsignedInt m_containedByFrame = 0;
	PathfindLayerEnum m_layer = LAYER_GROUND;
	class StealthUpdate *m_stealth = 0;
	/* --- end p1w13-sonnet --- */
protected:
	virtual Object *asObjectMeth() { return this; }
	virtual const Object *asObjectMeth() const { return this; }
};
#endif
