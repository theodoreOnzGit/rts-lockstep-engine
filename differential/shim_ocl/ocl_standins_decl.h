/* The stand-ins of the world that ObjectCreationList.cpp reaches (p1w10-sonnet). Included by differential/wrap_ocl.cpp before the real header
** and the cut ObjectCreationList.cpp, with every class below renamed by the #define lines of that file (OclTest*), so that no symbol of this
** archive collides with another fixture's.
**
** Method: the classes answer from the program's configuration (members set by the driver, ocl_driver.h, from the program words); every call
** that has an effect writes a line to g_lines, formatted exactly as the Rust host (tests/differential_object_creation_list.rs) writes it.
** Calls that only answer a question write nothing. Reals are written as their bit patterns (FB).
**
** What is real: Lib/BaseType.h (Coord3D, REAL_TO_INT), GameType.h (ObjectID, INVALID_ANGLE), KindOf.h, ObjectStatusTypes.h and the script
** status bits, GameCommon.h (VeterancyLevel), DisabledTypes.h, Shadow.h (ShadowType, TheShadowNames), WeaponSet.h's slot names, the random value
** generator, WWMath's Matrix3D and Vector3, AsciiString, INI (and with it the parsers INI.cpp holds: parseReal, parseBitString32,
** parseFXList through wrap_armor.cpp's store, parseWeaponTemplate through ref_weapon_lookup_hook, parseAudioEventRTS, ...).
** What is a stand-in: the objects and the modules found in them, the templates and the factory, the players, the terrain, the pathfinder,
** the partition manager, the weapon store, the particle systems, the audio, the script engine, the game logic, the LOD manager and the
** global data. RadiusDecalTemplate (RadiusDecal.h unchanged; its parser a verbatim copy of RadiusDecal.cpp:118-133), DeliverPayloadData
** (a verbatim copy of DeliverPayloadAIUpdate.h:251-319 and cpp:60-101) and INI::parseStaticGameLODLevel (GameLOD.cpp:425-436) are in
** ocl_driver.h: the files they live in are other units. */
#ifndef OCL_STANDINS_DECL_H
#define OCL_STANDINS_DECL_H

#include <vector>
#include <string>
#include <list>
#include <map>
#include <set>
#include <algorithm>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include "Lib/BaseType.h"
#include "Common/GameMemory.h"
#include "Common/GameCommon.h"
#include "Common/GameType.h"
#include "Common/KindOf.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/DisabledTypes.h"
#include "Common/INI.h"
#include "Common/NameKeyGenerator.h"
#include "Common/SubsystemInterface.h"
#include "GameLogic/ObjectScriptStatusBits.h"
#include "GameLogic/LogicRandomValue.h"
#include "Lib/trig.h"
/* BaseType.h's x87 helpers on their transcriptions (differential/wrap_base_type.cpp), as the BuildAssistant fixture does */
extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
}
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
#define DEFINE_SHADOW_NAMES
#define DEFINE_WEAPONSLOTTYPE_NAMES
#include "GameClient/Shadow.h"
#include "GameLogic/WeaponSet.h"
#include "Common/AudioEventRTS.h"
extern "C" void ref_init_game_logic_random(unsigned int seed);   /* differential/wrap_random_value.cpp: InitGameLogicRandom */


/* CRCDebug.h with NO_DEBUG_CRC (which ObjectCreationList.cpp defines): the DUMP macros are empty */
#define DUMPVEL {}
#define DUMPACCEL {}
#define DUMPVECTOR3(x) {}
#define DUMPCOORD3D(x) {}
#define DUMPMATRIX3D(x) {}
#define DUMPREAL(x) {}

/* ---- the trace ---------------------------------------------------------------------------------------------- */
static std::vector<std::string>* g_lines = NULL;
static unsigned fbits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
static void Rec(const char* fmt, ...)
{
	char buf[2048];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	if (g_lines) g_lines->push_back(buf);
	static const bool trace = getenv("OCL_TRACE") != NULL;
	if (trace) fprintf(stderr, "%s\n", buf);
}
#define FB(x) fbits(x)

class Object; class Player; class Team; class ThingTemplate; class PhysicsBehavior;
class RadiusDecalTemplate;

/* ---- constants of headers the real ObjectCreationList.cpp includes that are not the real ones here ---------------- */
#define PATHFIND_CELL_SIZE 10            /* AIPathfind.h line 439 */
#define SECONDS_PER_LOGICFRAME_REAL (1.0f / 30.0f)   /* GameCommon.h: 1.0f / LOGICFRAMES_PER_SECOND */
/* GameLOD.h lines 78-86 */
enum StaticGameLODLevel { STATIC_GAME_LOD_UNKNOWN = -1, STATIC_GAME_LOD_LOW, STATIC_GAME_LOD_MEDIUM, STATIC_GAME_LOD_HIGH, STATIC_GAME_LOD_CUSTOM, STATIC_GAME_LOD_COUNT };
/* PartitionManager.h lines 110-148 */
enum FindPositionFlags { FPF_NONE = 0, FPF_USE_HIGHEST_LAYER = 0x80 };
#define RANDOM_START_ANGLE (99999.9f)
struct FindPositionOptions
{
	FindPositionOptions(void) { flags = FPF_NONE; minRadius = 0.0f; maxRadius = 0.0f; startAngle = RANDOM_START_ANGLE; maxZDelta = 1e10f; ignoreObject = NULL; sourceToPathToDest = NULL; relationshipObject = NULL; }
	FindPositionFlags flags;
	Real minRadius, maxRadius, startAngle, maxZDelta;
	const Object* ignoreObject;
	const Object* sourceToPathToDest;
	const Object* relationshipObject;
};
#define Bit_Test(v, b) (((v) & (b)) != 0)

/* ---- the global data and the game LOD ---------------------------------------------------------------------------- */
struct GlobalData { Bool m_preloadAssets; };
extern GlobalData* TheGlobalData;
class GameLODManager { public: Bool m_debrisSkipped; Bool isDebrisSkipped() const { return m_debrisSkipped; } };
extern GameLODManager* TheGameLODManager;

/* ---- the opaque things the parsers store pointers to ------------------------------------------------------------ */
class WeaponTemplate { public: int m_idx; };
class FXList { public: int m_idx; };
class ParticleSystemTemplate { public: int m_idx; };
class ParticleSystem { public: int m_tmplIdx; void attachToObject(Object* obj); };
class ParticleSystemManager
{
public:
	ParticleSystemTemplate* findTemplate(const AsciiString& name) const;
	ParticleSystem* createParticleSystem(const ParticleSystemTemplate* tmp);
};
extern ParticleSystemManager* TheParticleSystemManager;
class WeaponStore { public: void createAndFireTempWeapon(const WeaponTemplate* wt, const Object* source, const Coord3D* pos); };
extern WeaponStore* TheWeaponStore;
class AudioManager { public: void addAudioEvent(const AudioEventRTS* e); };
extern AudioManager* TheAudio;
class ScriptEngine { public: void transferObjectName(const AsciiString& name, Object* obj); };
extern ScriptEngine* TheScriptEngine;

/* ---- the modules found in an object ----------------------------------------------------------------------------- */
class Locomotor
{
public:
	Object* m_owner;
	Real m_maxSpeed, m_prefHeight;
	Real getMaxSpeedForCondition(Int damageState) const;
	Real getPreferredHeight() const;
};
class BodyModuleInterface
{
public:
	Object* m_owner;
	Int m_damageState;
	Int getDamageState() const;
	void setInitialHealth(Int percent);
};
class PhysicsBehavior
{
public:
	Object* m_owner;
	Coord3D m_vel, m_accel;
	Real m_mass;
	Coord3D getVelocityValue() const { return m_vel; }
	const Coord3D* getVelocity() const { return &m_vel; }
	const Coord3D* getAcceleration() const { return &m_accel; }
	Real getMass() const { return m_mass; }
	void applyMotiveForce(const Coord3D* force);
	void applyForce(const Coord3D* force);
	void setMass(Real mass);
	void setExtraFriction(Real f);
	void setExtraBounciness(Real b);
	void setAllowBouncing(Bool b);
	void setAllowToFall(Bool b);
	void setBounceSound(const AudioEventRTS* s);
	void setIgnoreCollisionsWith(const Object* o);
	void setAngles(Real yaw, Real pitch, Real roll);
	void setYawRate(Real r);
	void setRollRate(Real r);
	void setPitchRate(Real r);
};
class ContainModuleInterface
{
public:
	Object* m_owner;
	bool m_accepts;
	Bool isValidContainerFor(const Object* obj, Bool checkCapacity) const;
	void addToContain(Object* obj);
};
class SlavedUpdateInterface { public: Object* m_owner; void onEnslave(const Object* slaver); };
class BehaviorModule
{
public:
	Object* m_owner;
	SlavedUpdateInterface m_slaved;
	bool m_hasSlaved;
	SlavedUpdateInterface* getSlavedUpdateInterface() { return m_hasSlaved ? &m_slaved : NULL; }
};
class AIUpdateInterface
{
public:
	Object* m_owner;
	void aiAttackPosition(const Coord3D* pos, Int maxShots, CommandSourceType cmd);
};
class DeliverPayloadData;
class DeliverPayloadAIUpdate
{
public:
	Object* m_owner;
	Locomotor m_loco;
	Locomotor* getCurLocomotor() { return &m_loco; }
	void deliverPayload(const Coord3D* moveToPos, const Coord3D* targetPos, const DeliverPayloadData* data);
};
class LifetimeUpdate { public: Object* m_owner; void setLifetimeRange(UnsignedInt minFrames, UnsignedInt maxFrames); };
class FloatUpdate { public: Object* m_owner; void setEnabled(Bool b); };
class RadiusDecalTemplate;
class RadiusDecalUpdate
{
public:
	Object* m_owner;
	void createRadiusDecal(const RadiusDecalTemplate& tmpl, Real radius, const Coord3D& pos);
	void killWhenNoLongerAttacking(Bool b);
};
class SpecialPowerCompletionDie { public: Object* m_owner; void setCreator(ObjectID id); };
class ExperienceTracker
{
public:
	Object* m_owner;
	Bool m_trainable;
	Bool isTrainable() const { return m_trainable; }
	void setVeterancyLevel(VeterancyLevel level, Bool provideFeedback = TRUE);
};
class DebrisDrawInterface
{
public:
	Object* m_owner;
	Int m_idx;
	void setModelName(AsciiString name, Color color, ShadowType t);
	void setAnimNames(AsciiString initial, AsciiString flying, AsciiString final, const FXList* finalFX);
};
class DrawModule
{
public:
	Object* m_owner;
	DebrisDrawInterface m_debris;
	bool m_hasDebris;
	DebrisDrawInterface* getDebrisDrawInterface() { return m_hasDebris ? &m_debris : NULL; }
};
class Drawable
{
public:
	Object* m_owner;
	DrawModule m_modules[2];
	DrawModule* m_modPtrs[3];
	bool m_hidden;
	DrawModule** getDrawModules() { return m_modPtrs; }
	Bool isDrawableEffectivelyHidden() const { return m_hidden; }
	void setDrawableHidden(Bool h);
	void fadeIn(UnsignedInt frames);
	void fadeOut(UnsignedInt frames);
};

/* ---- the thing templates and the factory ------------------------------------------------------------------------ */
class ThingTemplate
{
public:
	int m_idx;
	AsciiString m_name;
	unsigned m_flags;      /* what an object made from it has, see ocl_driver.h */
	Real m_mass, m_maxSpeed, m_prefHeight;
	Int m_damageState;
	const AsciiString& getName() const { return m_name; }
};
class ThingFactory
{
public:
	const ThingTemplate* findTemplate(const AsciiString& name, Bool check = true);
	const ThingTemplate* findTemplate(const char* name) { return findTemplate(AsciiString(name)); }
	Object* newObject(const ThingTemplate* tmplate, Team* team);
};
extern ThingFactory* TheThingFactory;

/* ---- the object -------------------------------------------------------------------------------------------------- */
class Object
{
public:
	ObjectID m_id;
	const ThingTemplate* m_tmpl;
	Team* m_team;
	Player* m_player;
	Coord3D m_pos;
	Real m_orient;
	Matrix3D m_xfrm;
	PathfindLayerEnum m_layer;
	bool m_aboveTerrain, m_significantlyAbove, m_offMap;
	VeterancyLevel m_veterancy;
	AsciiString m_name;
	bool m_structure;
	Color m_indicatorColor;
	PhysicsBehavior m_phys;
	bool m_hasPhys;
	BodyModuleInterface m_body;
	bool m_hasBody;
	ContainModuleInterface m_contain;
	bool m_hasContain;
	Drawable m_draw;
	bool m_hasDraw;
	ExperienceTracker m_exp;
	AIUpdateInterface m_ai;
	bool m_hasAi;
	DeliverPayloadAIUpdate m_dpai;
	bool m_hasDpai;
	LifetimeUpdate m_life;
	bool m_hasLife;
	FloatUpdate m_float;
	bool m_hasFloat;
	RadiusDecalUpdate m_rdu;
	bool m_hasRdu;
	SpecialPowerCompletionDie m_spcd;
	bool m_hasSpcd;
	BehaviorModule m_mods[2];
	BehaviorModule* m_modPtrs[3];
	Coord3D m_dir2d;

	ObjectID getID() const { return m_id; }
	const ThingTemplate* getTemplate() const { return m_tmpl; }
	AsciiString getName() const { return m_name; }
	const Coord3D* getPosition() const { return &m_pos; }
	Real getOrientation() const { return m_orient; }
	const Matrix3D* getTransformMatrix() const { return &m_xfrm; }
	Player* getControllingPlayer() const { return m_player; }
	PathfindLayerEnum getLayer() const { return m_layer; }
	Bool isAboveTerrain() const { return m_aboveTerrain; }
	Bool isSignificantlyAboveTerrain() const { return m_significantlyAbove; }
	Bool isOffMap() const { return m_offMap; }
	Bool isKindOf(KindOfType t) const { return t == KINDOF_STRUCTURE && m_structure; }
	VeterancyLevel getVeterancyLevel() const { return m_veterancy; }
	Color getIndicatorColor() const { return m_indicatorColor; }
	PhysicsBehavior* getPhysics() { return m_hasPhys ? &m_phys : NULL; }
	const PhysicsBehavior* getPhysics() const { return m_hasPhys ? &m_phys : NULL; }
	BodyModuleInterface* getBodyModule() { return m_hasBody ? &m_body : NULL; }
	ContainModuleInterface* getContain() const { return m_hasContain ? const_cast<ContainModuleInterface*>(&m_contain) : NULL; }
	Drawable* getDrawable() const { return m_hasDraw ? const_cast<Drawable*>(&m_draw) : NULL; }
	ExperienceTracker* getExperienceTracker() { return &m_exp; }
	AIUpdateInterface* getAIUpdateInterface() { return m_hasAi ? &m_ai : NULL; }
	AIUpdateInterface* getAI() { return m_hasAi ? &m_ai : NULL; }
	BehaviorModule** getBehaviorModules() { return m_modPtrs; }
	const Coord3D* getUnitDirectionVector2D() const { return &m_dir2d; }
	SpecialPowerCompletionDie* findSpecialPowerCompletionDie() { return m_hasSpcd ? &m_spcd : NULL; }
	void* findUpdateModule(NameKeyType key);
	void setProducer(const Object* obj);
	void setPosition(const Coord3D* pos);
	void setOrientation(Real angle);
	void setTransformMatrix(const Matrix3D* mtx);
	void setLayer(PathfindLayerEnum layer);
	void setScriptStatus(ObjectScriptStatusBit bit, Bool set = true);
	void setDisabledUntil(DisabledType type, UnsignedInt frame);
	void setWeaponLock(WeaponSlotType slot, WeaponLockType lock);
	void goInvulnerable(UnsignedInt frames);
	void kill();
	void attemptDamage(DamageInfo* info);
};

/* ---- the players and the teams ---------------------------------------------------------------------------------- */
class Team { public: int m_id; Player* m_player; };
class Player
{
public:
	Team* m_team;
	bool m_active;
	Team* getDefaultTeam() const { return m_team; }
	Bool isPlayerActive() const { return m_active; }
};
class PlayerList
{
public:
	Player* m_neutral;
	Player* getNeutralPlayer() const { return m_neutral; }
};
extern PlayerList* ThePlayerList;

/* ---- the game logic ---------------------------------------------------------------------------------------------- */
class GameLogic
{
public:
	UnsignedInt m_frame;
	UnsignedInt getFrame() const { return m_frame; }
	void destroyObject(Object* obj);
};
extern GameLogic* TheGameLogic;

/* ---- the terrain and the pathfinder ------------------------------------------------------------------------------ */
class TerrainLogic
{
public:
	Real getGroundHeight(Real x, Real y, Coord3D* normal = NULL) const;
	PathfindLayerEnum getHighestLayerForDestination(const Coord3D* pos) const;
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer) const;
	void flattenTerrain(Object* obj);
	Bool isUnderwater(Real x, Real y, Real* waterZ = NULL, Real* terrainZ = NULL) const;
};
extern TerrainLogic* TheTerrainLogic;
class PathfindCell
{
public:
	enum CellType { CELL_CLEAR = 0, CELL_WATER = 1, CELL_CLIFF = 2, CELL_RUBBLE = 3, CELL_OBSTACLE = 4, CELL_BRIDGE_IMPASSABLE = 5, CELL_IMPASSABLE = 6 };
	CellType m_type;
	CellType getType() const { return m_type; }
};
class Pathfinder
{
public:
	PathfindCell* getCell(PathfindLayerEnum layer, Int x, Int y);
	void addObjectToPathfindMap(Object* obj);
};
class AI { public: Pathfinder* pathfinder(); };
extern AI* TheAI;

/* ---- the partition manager --------------------------------------------------------------------------------------- */
class PartitionManager
{
public:
	void findPositionAround(const Coord3D* center, const FindPositionOptions* options, Coord3D* result);
};
extern PartitionManager* ThePartitionManager;

#endif
