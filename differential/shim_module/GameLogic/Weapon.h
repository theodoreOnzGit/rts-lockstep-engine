/* Stand-in for GameEngine/Include/GameLogic/Weapon.h, Module build only (p1w10-sonnet, the layer-9 upgrade batch): WeaponBonusUpgrade.cpp
** includes it for WeaponBonusConditionType and uses nothing else. The enum is copied verbatim from Weapon.h lines 172-211 (commit 0a05454d), with
** ALLOW_DEMORALIZE undefined, as the shipped game. The real header is the weapon fixtures' (shim_weapon) and needs the whole weapon layer. */
#ifndef SHIM_MODULE_WEAPON_H
#define SHIM_MODULE_WEAPON_H
/* p1w13-sonnet, batch R33 (EMPUpdate): Weapon.h lines 110-132, verbatim */
enum WeaponAffectsMaskType
{
	WEAPON_AFFECTS_SELF						= 0x01,
	WEAPON_AFFECTS_ALLIES					= 0x02,
	WEAPON_AFFECTS_ENEMIES				= 0x04,	// that is, enemies that aren't the primary target
	WEAPON_AFFECTS_NEUTRALS				= 0x08,	// ditto
	WEAPON_KILLS_SELF							= 0x10,	// ensures that it's not possible to survive self damage
	WEAPON_DOESNT_AFFECT_SIMILAR	= 0x20, // Doesn't affect others that are the same as us (like other terrorists for a terrorist bomb to prevent chain reaction)
	WEAPON_DOESNT_AFFECT_AIRBORNE	= 0x40, // Radius damage doesn't affect airborne units, unless they are the primary target. (used for poison fields.)
};
static const char *TheWeaponAffectsMaskNames[] = 
{
	"SELF",
	"ALLIES",
	"ENEMIES",
	"NEUTRALS",
	"SUICIDE",
	"NOT_SIMILAR",
	"NOT_AIRBORNE",
	NULL
};
enum WeaponBonusConditionType
{
	// The access and use of this enum has the bit shifting built in, so this is a 0,1,2,3,4,5 enum
	WEAPONBONUSCONDITION_INVALID = -1,

	WEAPONBONUSCONDITION_GARRISONED = 0,
	WEAPONBONUSCONDITION_HORDE,
	WEAPONBONUSCONDITION_CONTINUOUS_FIRE_MEAN,
	WEAPONBONUSCONDITION_CONTINUOUS_FIRE_FAST,
	WEAPONBONUSCONDITION_NATIONALISM,
	WEAPONBONUSCONDITION_PLAYER_UPGRADE,
	WEAPONBONUSCONDITION_DRONE_SPOTTING,
	WEAPONBONUSCONDITION_DEMORALIZED_OBSOLETE,
	WEAPONBONUSCONDITION_ENTHUSIASTIC,
	WEAPONBONUSCONDITION_VETERAN,
	WEAPONBONUSCONDITION_ELITE,
	WEAPONBONUSCONDITION_HERO,
	WEAPONBONUSCONDITION_BATTLEPLAN_BOMBARDMENT,
	WEAPONBONUSCONDITION_BATTLEPLAN_HOLDTHELINE,
	WEAPONBONUSCONDITION_BATTLEPLAN_SEARCHANDDESTROY,
	WEAPONBONUSCONDITION_SUBLIMINAL,
	WEAPONBONUSCONDITION_SOLO_HUMAN_EASY,
	WEAPONBONUSCONDITION_SOLO_HUMAN_NORMAL,
	WEAPONBONUSCONDITION_SOLO_HUMAN_HARD,
	WEAPONBONUSCONDITION_SOLO_AI_EASY,
	WEAPONBONUSCONDITION_SOLO_AI_NORMAL,
	WEAPONBONUSCONDITION_SOLO_AI_HARD,
	WEAPONBONUSCONDITION_TARGET_FAERIE_FIRE,
	WEAPONBONUSCONDITION_FANATICISM,
	WEAPONBONUSCONDITION_FRENZY_ONE,
	WEAPONBONUSCONDITION_FRENZY_TWO,
	WEAPONBONUSCONDITION_FRENZY_THREE,

	WEAPONBONUSCONDITION_COUNT
};
/* --- p1w12-sonnet --- (behavior batch K: FireWeaponWhenDeadBehavior.cpp) what that module asks of the weapon layer: a template it was given by the INI
** (opaque here) and the store that fires a temporary weapon, which differential/l9_behaviors_k.inc defines to log the call. Renamed by module_prelude.h. */
class Object;
#include "Lib/BaseType.h"
class WeaponTemplate;
class WeaponStore
{
public:
	void createAndFireTempWeapon(const WeaponTemplate *wt, const Object *source, const Coord3D *pos);
	const WeaponTemplate *findWeaponTemplate(const char *name) const;   /* p1w13-sonnet: SlowDeathBehavior (differential/l9_slow_death_q.inc) */
	const WeaponTemplate *findWeaponTemplate(const AsciiString &name) const { return findWeaponTemplate(name.str()); }   /* batch R36: StructureToppleUpdate (upstream's takes an AsciiString) */
	void handleProjectileDetonation( const WeaponTemplate* w, const Object *source, const Coord3D* pos, UnsignedInt extraBonusFlags, Bool inflictDamage = TRUE );   /* batch R68: a line */
	class Weapon *allocateNewWeapon(const WeaponTemplate *tmpl, Int wslot) const;   /* batch R62: FireWeaponCollide (Weapon.h 839-842; defined below) */
};
extern WeaponStore *TheWeaponStore;
/* --- end p1w12-sonnet --- */
/* --- p1w12-sonnet --- (update batch M, FireOCLAfterWeaponCooldownUpdate.cpp) a Weapon with the three facts that module reads of one; renamed
** ModTestWeapon by module_prelude.h so that no inline member clashes with the weapon fixtures' real Weapon */
#include "Common/GameType.h"
/* --- p1w13-sonnet --- (batch R2: TransportContain) */
#include "GameLogic/Damage.h"   /* batch R39: the template's damage and death types */
#include "GameLogic/WeaponStatus.h"   /* batch R59 (upstream's, an enum) */
/* --- p1w13-sonnet --- (batch R39: StickyBombUpdate's detonation reads a template's damage through a null bonus) Weapon.h lines 257-291, verbatim but
** for appendBonuses, which nothing here calls */
class WeaponBonus
{
public:

	enum Field
	{
		DAMAGE = 0,
		RADIUS,
		RANGE,
		RATE_OF_FIRE,
		PRE_ATTACK,

		FIELD_COUNT	// keep last
	};

	WeaponBonus()
	{
		clear();
	}

	inline void clear()
	{
		for (int i = 0; i < FIELD_COUNT; ++i)
			m_field[i] = 1.0f;
	}

	inline Real getField(Field f) const { return m_field[f]; }
	inline void setField(Field f, Real v) { m_field[f] = v; }

private:
	Real m_field[FIELD_COUNT];

};
/* --- end p1w13-sonnet --- */
/* --- p1w13-sonnet --- (batch R66) Weapon.h lines 97-107, verbatim */
enum WeaponAntiMaskType
{
	WEAPON_ANTI_AIRBORNE_VEHICLE	= 0x01,
	WEAPON_ANTI_GROUND						= 0x02,
	WEAPON_ANTI_PROJECTILE				= 0x04,
	WEAPON_ANTI_SMALL_MISSILE			= 0x08, // All missiles are also projectiles (but not all projectiles are missiles)
	WEAPON_ANTI_MINE							= 0x10,
	WEAPON_ANTI_AIRBORNE_INFANTRY	= 0x20, // When set, anti-air weapons will not target infantry paratroopers.
	WEAPON_ANTI_BALLISTIC_MISSILE	= 0x40, // Specifically large missiles that are vulnerable to base defenses
	WEAPON_ANTI_PARACHUTE					= 0x80, // Parachutes can be targeted directly
};
/* --- end p1w13-sonnet --- */
class WeaponTemplate
{
public:
	Bool isContactWeapon() const { return m_contact; }
	Bool m_contact = false;
	/* --- p1w13-sonnet --- (batch R39) the damage numbers, as Weapon.cpp's getters compute them (the field times the bonus's), and the three types */
	Real m_primaryDamage = 0.0f, m_primaryDamageRadius = 0.0f, m_secondaryDamage = 0.0f, m_secondaryDamageRadius = 0.0f;
	Int m_damageTypeT = 0, m_deathTypeT = 0, m_damageStatusTypeT = 0;
	Real getPrimaryDamage(const WeaponBonus& bonus) const { return m_primaryDamage * bonus.getField(WeaponBonus::DAMAGE); }
	Real getPrimaryDamageRadius(const WeaponBonus& bonus) const { return m_primaryDamageRadius * bonus.getField(WeaponBonus::RADIUS); }
	Real getSecondaryDamage(const WeaponBonus& bonus) const { return m_secondaryDamage * bonus.getField(WeaponBonus::DAMAGE); }
	Real getSecondaryDamageRadius(const WeaponBonus& bonus) const { return m_secondaryDamageRadius * bonus.getField(WeaponBonus::RADIUS); }
	DamageType getDamageType() const { return (DamageType)m_damageTypeT; }
	DeathType getDeathType() const { return (DeathType)m_deathTypeT; }
	ObjectStatusTypes getDamageStatusType() const { return (ObjectStatusTypes)m_damageStatusTypeT; }
	/* batch R56: the attack range; the getter is Weapon.cpp 461-469 (RATIONALIZE_ATTACK_RANGE), verbatim but for PATHFIND_CELL_SIZE_F's value */
	Real m_attackRange = 0.0f;
	Real getAttackRange(const WeaponBonus& bonus) const
	{
		const Real UNDERSIZE = 10.0f*0.25f;
		Real r = m_attackRange * bonus.getField(WeaponBonus::RANGE) - UNDERSIZE;
		if (r < 0.0f) r = 0.0f;
		return r;
	}
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R62: what Weapon's constructor, crc and xfer read of the template, Weapon.cpp's own text: build.rs) */
	AsciiString m_name;
	Real m_minTargetPitch = -PI, m_maxTargetPitch = PI;
	Int m_shotsPerBarrel = 1;
	UnsignedInt m_suspendFXDelay = 0;
	const AsciiString &getName() const { return m_name; }
	Real getMinTargetPitch() const { return m_minTargetPitch; }
	Real getMaxTargetPitch() const { return m_maxTargetPitch; }
	Int getShotsPerBarrel() const { return m_shotsPerBarrel; }
	UnsignedInt getSuspendFXDelay() const { return m_suspendFXDelay; }
	/* batch R66 (PointDefenseLaserUpdate): the delays between shots and the anti mask; getDelayBetweenShots is Weapon.cpp 499-514
	** (differential/l9_point_defense_r66.inc), getAntiMask Weapon.h's getter */
	Int m_minDelayBetweenShots = 0, m_maxDelayBetweenShots = 0;
	Int m_antiMask = 0;
	Int getDelayBetweenShots(const WeaponBonus& bonus) const;
	Int getAntiMask() const { return m_antiMask; }
	/* batch R68 (DumbProjectileBehavior): the speeds (Weapon.h's getters), the minimum and unmodified ranges (Weapon.cpp 478-496, verbatim but for
	** PATHFIND_CELL_SIZE_F's value, as getAttackRange above) and the collision test (differential/l9_dumb_projectile_r68.inc answers it) */
	Real m_weaponSpeed = 0.0f, m_minWeaponSpeed = 0.0f, m_minimumAttackRange = 0.0f;
	Bool m_isScaleWeaponSpeed = false;
	Real getWeaponSpeed() const { return m_weaponSpeed; }
	Real getMinWeaponSpeed() const { return m_minWeaponSpeed; }
	Bool isScaleWeaponSpeed() const { return m_isScaleWeaponSpeed; }
	Real getMinimumAttackRange() const
	{
		const Real UNDERSIZE = 10.0f*0.25f;
		Real r = m_minimumAttackRange - UNDERSIZE;
		if (r < 0.0f) r = 0.0f;
		return r;
	}
	Real getUnmodifiedAttackRange() const { return m_attackRange; }
	Bool shouldProjectileCollideWith(const Object* projectileLauncher, const Object* projectile, const Object* thingWeCollidedWith, ObjectID intendedVictimID) const;
	/* --- end p1w13-sonnet --- */
};
/* --- end p1w13-sonnet --- */
#include "Common/Snapshot.h"
class Weapon : public Snapshot
{
public:
	/* --- p1w13-sonnet --- (batch R62: FireWeaponCollide owns a weapon: Weapon.cpp's constructor, crc, xfer and loadPostProcess, verbatim (build.rs
	** mod_gen/Weapon_r62.inc, defined by differential/l9_fire_weapon_collide_r62.inc), on the members they use; loadAmmoNow and fireWeapon are
	** that driver's lines) */
	Weapon() {}
	virtual ~Weapon() {}
	void deleteInstance() { delete this; }   /* the memory pool glue (newInstance is the module build's new) */
	Weapon(const WeaponTemplate *tmpl, WeaponSlotType wslot);
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
	void loadAmmoNow(const Object *source);
	Bool fireWeapon(const Object *source, Object *target, ObjectID *projectileID = NULL);
	UnsignedInt getSuspendFXFrame() const { return m_suspendFXFrame; }
	const WeaponTemplate *m_template = 0;
	WeaponSlotType m_wslot = PRIMARY_WEAPON;
	mutable WeaponStatus m_status = READY_TO_FIRE;   /* (Weapon.h: mutable, as getStatus sets it; batch R64) */
	UnsignedInt m_ammoInClip = 0, m_whenWeCanFireAgain = 0, m_whenPreAttackFinished = 0, m_whenLastReloadStarted = 0, m_lastFireFrame = 0, m_suspendFXFrame = 0;
	ObjectID m_projectileStreamID = INVALID_ID;
	Int m_maxShotCount = 0x7fffffff, m_curBarrel = 0, m_numShotsForCurBarrel = 0;
	std::vector<Int> m_scatterTargetsUnused;
	Bool m_pitchLimited = false, m_leechWeaponRangeActive = false;
	/* --- end p1w13-sonnet --- */
	WeaponSlotType getWeaponSlot() const { return m_slot; }
	UnsignedInt getLastShotFrame() const { return m_last; }
	UnsignedInt getPossibleNextShotFrame() const { return m_next; }
	WeaponSlotType m_slot = PRIMARY_WEAPON;
	UnsignedInt m_last = 0, m_next = 0;
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	const WeaponTemplate *getTemplate() const { return &m_tmplObj; }
	Bool isDamageWeapon() const { return m_damageW; }
	void transferNextShotStatsFrom( const Weapon &weapon );   /* logs: differential/l9_open_contain_r.inc */
	WeaponTemplate m_tmplObj;
	Bool m_damageW = false;
	Int m_ownerId = 0;
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batches R59, R64: the weapon's status) */
	/* batch R59: AssistedTargetingUpdate asks whether the current weapon is ready: the test's status (its call is renamed to this by
	** differential/l9_assisted_r59.inc) */
	WeaponStatus r59GetStatus() const { return (WeaponStatus)m_r59Status; }
	Int m_r59Status = 0;
	/* batch R64: Weapon.cpp's getStatus, verbatim (build.rs, mod_gen/Weapon_r62.inc), forceFireWeapon
	** (a line: differential/l9_fire_weapon_update_r64.inc) and Weapon.h's three setters, verbatim */
	WeaponStatus getStatus() const;
	Object* forceFireWeapon( const Object *source, const Coord3D *pos );
	static void positionProjectileForLaunch(Object* projectile, const Object *launcher, WeaponSlotType wslot, Int specificBarrelToUse);   /* batch R68: a line */
	void reloadAmmo(const Object *source);   /* batch R65: a line (differential/l9_fire_weapon_damaged_r65.inc) */
	void setPossibleNextShotFrame( UnsignedInt frameNum ) { m_whenWeCanFireAgain = frameNum; }
	void setPreAttackFinishedFrame( UnsignedInt frameNum ) { m_whenPreAttackFinished = frameNum; }
	void setStatus( WeaponStatus status) { m_status = status; }
	/* --- p1w13-sonnet --- (batch R12: GarrisonContain) the range check (a line, the test's answer: differential/l9_open_contain_r.inc) and the damage type */
	Bool isWithinAttackRange(const Object *source, const Object *target) const;
	Bool isWithinAttackRange(const Object *source, const Coord3D *pos) const;
	Int getDamageType() const { return m_damageType; }   /* a DamageType */
	Int m_damageType = 0;
	/* --- end p1w13-sonnet --- */
};
/* --- end p1w12-sonnet --- */
inline Weapon *WeaponStore::allocateNewWeapon(const WeaponTemplate *tmpl, Int wslot) const { return newInstance(Weapon)(tmpl, (WeaponSlotType)wslot); }   /* batch R62 */
/* --- p1w13-sonnet --- (batch P: ActiveBody asks the guard to retaliate with no limit on the shots; Weapon.h line 56, verbatim) */
const Int NO_MAX_SHOTS_LIMIT = 0x7fffffff;
/* --- end p1w13-sonnet --- */
#endif
/* --- p1w13-sonnet --- (batch R17: WeaponBonusUpdate.cpp defines DEFINE_WEAPONBONUSCONDITION_NAMES and includes this header again, after the
** include guard above has closed: the names are outside it, under their own guard) Weapon.h lines 211-249, verbatim */
#ifdef DEFINE_WEAPONBONUSCONDITION_NAMES
#ifndef SHIM_MODULE_WEAPON_BONUS_NAMES
#define SHIM_MODULE_WEAPON_BONUS_NAMES
static const char *TheWeaponBonusNames[] = 
{
	// This is a RHS enum (weapon.ini will have WeaponBonus = IT) so it is all caps
	"GARRISONED",
	"HORDE",
	"CONTINUOUS_FIRE_MEAN",
	"CONTINUOUS_FIRE_FAST",
	"NATIONALISM",
	"PLAYER_UPGRADE",
	"DRONE_SPOTTING",
#ifdef ALLOW_DEMORALIZE
	"DEMORALIZED",
#else
	"DEMORALIZED_OBSOLETE",
#endif
	"ENTHUSIASTIC",
	"VETERAN",
	"ELITE",
	"HERO",
	"BATTLEPLAN_BOMBARDMENT",
	"BATTLEPLAN_HOLDTHELINE",
	"BATTLEPLAN_SEARCHANDDESTROY",
	"SUBLIMINAL",
	"SOLO_HUMAN_EASY",
	"SOLO_HUMAN_NORMAL",
	"SOLO_HUMAN_HARD",
	"SOLO_AI_EASY",
	"SOLO_AI_NORMAL",
	"SOLO_AI_HARD",
	"TARGET_FAERIE_FIRE",
  "FANATICISM", // FOR THE NEW GC INFANTRY GENERAL... adds to nationalism
	"FRENZY_ONE",
	"FRENZY_TWO",
	"FRENZY_THREE",

	NULL
};
#endif
#endif
/* --- end p1w13-sonnet --- */
