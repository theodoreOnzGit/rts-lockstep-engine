/* Differential-test wrapper for TurretAI (upstream commit 0a05454d), p1w13-sonnet, layer 10 batch T1.
**
** TurretAI.cpp (two verbatim cuts but for one implicit `int`: turret_gen/turret_ai.inc), AIStates.cpp's `outOfWeaponRangeObject` and
** `AIAttackFireWeaponState` (verbatim: turret_gen/turret_fire.inc), the real TurretAI.h, AIStateMachine.h's NotifyWeaponFiredInterface
** and AIAttackFireWeaponState (a verbatim cut: turret_gen/turret_fire_decl.inc), and StateMachine.cpp (compiled unchanged beside
** this file), on the stand-ins of differential/shim_turret (renamed Tu* by its PreRTS.h). The answers of the stand-ins come from the
** scene (the cfg) or from a hash of their arguments that tests/differential_turret_t1.rs computes too; the calls with an effect are
** lines of a log. */
#include "PreRTS.h"
#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/XferCRC.h"
#define private public
#define protected public
#include "Common/StateMachine.h"
#include "turret_fire_decl.inc"
#include "GameLogic/TurretAI.h"
/* AIStateMachine.h's declaration of the fire state's condition, and CRCDebug.h's logging (the release build's: empty) */
extern Bool outOfWeaponRangeObject( State *thisState, void* userData );
#ifndef CRCDEBUG_LOG
#define CRCDEBUG_LOG(x)
#endif
#include "turret_ai.inc"
#include "turret_fire.inc"
#undef private
#undef protected

namespace {

struct TObj
{
	ObjectID id;
	Coord3D pos;
	UnsignedInt team;
	UnsignedInt kinds;
	unsigned long long statusLo, statusHi;
	UnsignedInt flags;
	Int containedBy;
	Int player;
	UnsignedInt moodIdle, moodAttack, nextMood;
	Int goal;
	UnsignedInt lastCmd;
	Int victim;
	Int curSlot;
	UnsignedInt weapons;
	UnsignedInt wstatus;
	Real range[3];
	Real contRange;
	Real maxHeight;
	Int turretSync;
};
enum { F_DEAD = 1, F_DESTROYED = 2, F_AIRBORNE = 4, F_ABOVE = 8, F_AI = 16, F_GROUND = 32, F_PREVENTING = 64, F_LINKED = 128,
	F_TEAM_COMMON = 256 };

struct Scene
{
	UnsignedInt seed;
	UnsignedInt rates[3];
	std::vector<TObj> objs;
};
Scene g_scene;
UnsignedInt g_frame;
UnsignedInt g_counter;
std::vector<Object> g_objects;
std::vector<AIUpdateInterface> g_ais;
std::vector<Weapon> g_weapons;
std::vector<Player> g_players;
std::map<UnsignedInt, Team> g_teams;
std::set<UnsignedInt> g_playing;
UnsignedInt g_nextHandle;
std::vector<unsigned> g_log;
AudioEventRTS g_turretSound;
GameLogic g_gameLogic;
GlobalData g_globalData;
PartitionManager g_partition;
TerrainLogic g_terrain;
AI g_ai;
Pathfinder g_pathfinder;
AudioManager g_audio;

unsigned bitsOf(Real r) { unsigned u; memcpy(&u, &r, 4); return u; }
Real floatOf(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

/* the answers' hash: the seed, a tag, the words (the same function in the Rust test) */
unsigned tuHash(unsigned tag, std::initializer_list<unsigned> words)
{
	unsigned x = g_scene.seed ^ (tag * 0x85ebca6bu);
	for (unsigned w : words) {
		x ^= w;
		x *= 0x9E3779B1u;
		x ^= x >> 15;
	}
	return x;
}
bool tuPick(unsigned h, unsigned rate) { return (h >> 8) % 100u < rate; }

/* one log line: its length, its code, its words */
void logLine(unsigned code, std::initializer_list<unsigned> words)
{
	g_log.push_back((unsigned)words.size() + 1);
	g_log.push_back(code);
	for (unsigned w : words) g_log.push_back(w);
}

TObj &obj(const Object *o) { return g_scene.objs[o->m_index]; }
unsigned idOf(const Object *o) { return o ? (unsigned)o->getID() : 0u; }
Object *objAtIndex(Int k) { return k < 0 ? NULL : &g_objects[k]; }
Object *byIndexPlus1(Int k) { return k <= 0 ? NULL : &g_objects[(k - 1) % (Int)g_objects.size()]; }

}  // namespace

/* ---- the stand-ins ------------------------------------------------------------------------------------------------------------ */
GameLogic *TheGameLogic = &g_gameLogic;
GlobalData *TheGlobalData = &g_globalData;
PartitionManager *ThePartitionManager = &g_partition;
TerrainLogic *TheTerrainLogic = &g_terrain;
AI *TheAI = &g_ai;
AudioManager *TheAudio = &g_audio;

UnsignedInt GameLogic::getFrame() { return g_frame; }
Object *GameLogic::findObjectByID(ObjectID id)
{
	for (Object &o : g_objects) if (o.getID() == id) return &o;
	return NULL;
}
Int tuGetGameLogicRandomValue(int lo, int hi, char *, int)
{
	unsigned h = tuHash(51, { g_counter++, (unsigned)lo, (unsigned)hi });
	Int r = hi >= lo ? lo + (Int)((h >> 8) % (unsigned)(hi - lo + 1)) : lo;
	logLine(20, { (unsigned)lo, (unsigned)hi, (unsigned)r });
	return r;
}
Real tuGetGameLogicRandomValueReal(Real lo, Real hi, char *, int)
{
	unsigned h = tuHash(52, { g_counter++, bitsOf(lo), bitsOf(hi) });
	Real r = lo + (Real)((h >> 8) % 10001u) / 10000.0f * (hi - lo);
	logLine(21, { bitsOf(lo), bitsOf(hi), bitsOf(r) });
	return r;
}

Bool AudioEventRTS::isCurrentlyPlaying() const { return g_playing.count(m_handle) != 0; }
UnsignedInt AudioManager::addAudioEvent(const AudioEventRTS *e)
{
	UnsignedInt h = ++g_nextHandle;
	g_playing.insert(h);
	logLine(17, { (unsigned)e->m_object, h });
	return h;
}
void AudioManager::removeAudioEvent(UnsignedInt handle)
{
	g_playing.erase(handle);
	logLine(18, { handle });
}
const AudioEventRTS *ThingTemplate::getPerUnitSound(const char *) const { return &g_turretSound; }

Object *Team::getTeamTargetObject() { return TheGameLogic->findObjectByID(m_target); }
void Team::setTeamTargetObject(const Object *target)
{
	m_target = target ? target->getID() : INVALID_ID;
	logLine(8, { m_id, idOf(target) });
}

WeaponStatus Weapon::getStatus() const { return (WeaponStatus)((g_scene.objs[m_owner].wstatus >> (3 * m_slot)) & 7); }
Bool Weapon::isWithinAttackRange(const Object *source, const Object *target) const
{
	return tuPick(tuHash(45, { idOf(source), (unsigned)m_slot, idOf(target) }), g_scene.rates[1]);
}
Bool Weapon::isWithinAttackRange(const Object *source, const Coord3D *pos) const
{
	return tuPick(tuHash(46, { idOf(source), (unsigned)m_slot, bitsOf(pos->x), bitsOf(pos->y) }), g_scene.rates[1]);
}
Real Weapon::getAttackRange(const Object *) const { return g_scene.objs[m_owner].range[m_slot]; }
Bool Weapon::isContactWeapon() const { return (g_scene.objs[m_owner].weapons >> (3 + m_slot)) & 1; }
Bool Weapon::hasLeechRange() const { return (g_scene.objs[m_owner].weapons >> (6 + m_slot)) & 1; }
Real Weapon::getContinueAttackRange() const { return g_scene.objs[m_owner].contRange; }
Bool Weapon::fireWeapon(const Object *source, const Coord3D *pos)
{
	Bool r = tuPick(tuHash(50, { idOf(source), (unsigned)m_slot, bitsOf(pos->x), bitsOf(pos->y) }), 50);
	logLine(10, { idOf(source), (unsigned)m_slot, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), r ? 1u : 0u });
	return r;
}
void Weapon::setPreAttackFinishedFrame(UnsignedInt frame) { logLine(9, { g_scene.objs[m_owner].id, (unsigned)m_slot, frame }); }

UnsignedInt AIUpdateInterface::getMoodMatrixActionAdjustment(MoodMatrixAction action) const
{
	return action == MM_Action_Idle ? g_scene.objs[m_owner].moodIdle : g_scene.objs[m_owner].moodAttack;
}
Bool AIUpdateInterface::areTurretsLinked() const { return (g_scene.objs[m_owner].flags & F_LINKED) != 0; }
Object *AIUpdateInterface::getCurrentVictim() const { return byIndexPlus1(g_scene.objs[m_owner].victim); }
CommandSourceType AIUpdateInterface::getLastCommandSource() const { return (CommandSourceType)g_scene.objs[m_owner].lastCmd; }
Bool AIUpdateInterface::isDoingGroundMovement() const { return (g_scene.objs[m_owner].flags & F_GROUND) != 0; }
UnsignedInt AIUpdateInterface::getNextMoodCheckTime() const { return g_scene.objs[m_owner].nextMood; }
void AIUpdateInterface::resetNextMoodCheckTime()
{
	g_scene.objs[m_owner].nextMood = g_frame;
	logLine(4, { g_scene.objs[m_owner].id });
}
WhichTurretType AIUpdateInterface::friend_getTurretSync() const { return (WhichTurretType)g_scene.objs[m_owner].turretSync; }
void AIUpdateInterface::friend_setTurretSync(WhichTurretType t)
{
	g_scene.objs[m_owner].turretSync = t;
	logLine(5, { g_scene.objs[m_owner].id, (unsigned)t });
}
Object *AIUpdateInterface::getGoalObject() { return byIndexPlus1(g_scene.objs[m_owner].goal); }
Object *AIUpdateInterface::getNextMoodTarget(Bool calledByAI, Bool calledDuringIdle)
{
	unsigned n = (unsigned)g_objects.size();
	unsigned h = tuHash(48, { g_scene.objs[m_owner].id, g_frame, g_counter++, calledByAI ? 1u : 0u, calledDuringIdle ? 1u : 0u });
	unsigned k = (h >> 8) % (n + 2);
	Object *r = k < n ? &g_objects[k] : NULL;
	logLine(19, { g_scene.objs[m_owner].id, idOf(r) });
	return r;
}
void AIUpdateInterface::addTargeter(ObjectID id, Bool add) { logLine(6, { g_scene.objs[m_owner].id, (unsigned)id, add ? 1u : 0u }); }
Bool AIUpdateInterface::isTemporarilyPreventingAimSuccess() const { return (g_scene.objs[m_owner].flags & F_PREVENTING) != 0; }

ObjectID Object::getID() const { return g_scene.objs[m_index].id; }
const Coord3D *Object::getPosition() const { return &g_scene.objs[m_index].pos; }
const ThingTemplate *Object::getTemplate() const { static ThingTemplate t; return &t; }
Bool Object::isKindOf(KindOfType t) const
{
	static const KindOfType kinds[6] = { KINDOF_BRIDGE, KINDOF_IMMOBILE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE, KINDOF_MINE,
		KINDOF_VEHICLE };
	for (int b = 0; b < 6; ++b) if (kinds[b] == t) return (g_scene.objs[m_index].kinds >> b) & 1;
	return false;
}
Bool Object::testStatus(ObjectStatusTypes bit) const
{
	const TObj &o = g_scene.objs[m_index];
	return bit < 64 ? ((o.statusLo >> bit) & 1) != 0 : ((o.statusHi >> (bit - 64)) & 1) != 0;
}
void Object::setStatus(ObjectStatusMaskType mask, Bool set)
{
	TObj &o = g_scene.objs[m_index];
	if (set) { o.statusLo |= mask.lo; o.statusHi |= mask.hi; } else { o.statusLo &= ~mask.lo; o.statusHi &= ~mask.hi; }
	logLine(1, { o.id, (unsigned)mask.lo, (unsigned)(mask.lo >> 32), (unsigned)mask.hi, set ? 1u : 0u });
}
Bool Object::isEffectivelyDead() const { return (g_scene.objs[m_index].flags & F_DEAD) != 0; }
Bool Object::isDestroyed() const { return (g_scene.objs[m_index].flags & F_DESTROYED) != 0; }
Object *Object::getContainedBy() { return byIndexPlus1(g_scene.objs[m_index].containedBy); }
Bool Object::isAirborneTarget() const { return (g_scene.objs[m_index].flags & F_AIRBORNE) != 0; }
Bool Object::isSignificantlyAboveTerrain() const { return (g_scene.objs[m_index].flags & F_ABOVE) != 0; }
Player *Object::getControllingPlayer() const
{
	Int p = g_scene.objs[m_index].player;
	return p < 0 ? NULL : &g_players[p % (Int)g_players.size()];
}
Team *Object::getTeam()
{
	UnsignedInt t = g_scene.objs[m_index].team;
	return t == 0 ? NULL : &g_teams[t];
}
const Team *Object::getTeam() const { return const_cast<Object *>(this)->getTeam(); }
Weapon *Object::getCurrentWeapon(WeaponSlotType *wslot)
{
	Int s = g_scene.objs[m_index].curSlot;
	if (s < 0 || s > 2 || !((g_scene.objs[m_index].weapons >> s) & 1)) return NULL;
	if (wslot) *wslot = (WeaponSlotType)s;
	return &g_weapons[m_index * 3 + s];
}
const Weapon *Object::getCurrentWeapon(WeaponSlotType *wslot) const { return const_cast<Object *>(this)->getCurrentWeapon(wslot); }
Weapon *Object::getWeaponInWeaponSlot(WeaponSlotType wslot) const
{
	return ((g_scene.objs[m_index].weapons >> wslot) & 1) ? &g_weapons[m_index * 3 + wslot] : NULL;
}
Bool Object::isAbleToAttack() const { return !testStatus(OBJECT_STATUS_NO_ATTACK); }
CanAttackResult Object::getAbleToAttackSpecificObject(AbleToAttackType t, const Object *target, CommandSourceType commandSource) const
{
	return (CanAttackResult)((tuHash(47, { getID(), (unsigned)t, idOf(target), (unsigned)commandSource }) >> 8) % 4u);
}
void Object::chooseBestWeaponForTarget(const Object *target, WeaponChoiceCriteria criteria, CommandSourceType commandSource)
{
	logLine(7, { getID(), idOf(target), (unsigned)criteria, (unsigned)commandSource });
}
void Object::preFireCurrentWeapon(const Object *victim) { logLine(13, { getID(), idOf(victim) }); }
void Object::setFiringConditionForCurrentWeapon() const { logLine(14, { getID() }); }
void Object::fireCurrentWeapon(Object *target) { logLine(15, { getID(), idOf(target) }); }
void Object::fireCurrentWeapon(const Coord3D *pos) { logLine(16, { getID(), bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) }); }
void Object::releaseWeaponLock(WeaponLockType lock) { logLine(11, { getID(), (unsigned)lock }); }
void Object::notifyFiringTrackerShotFired(const Weapon *weaponFired, ObjectID victimID)
{
	logLine(12, { getID(), (unsigned)weaponFired->m_slot, (unsigned)victimID });
}
void Object::setModelConditionState(ModelConditionFlagType a) { logLine(2, { getID(), (unsigned)a, 1u }); }
void Object::clearModelConditionState(ModelConditionFlagType a) { logLine(2, { getID(), (unsigned)a, 0u }); }
void Object::reactToTurretChange(WhichTurretType turret, Real oldRotation, Real oldPitch)
{
	logLine(3, { getID(), (unsigned)turret, bitsOf(oldRotation), bitsOf(oldPitch) });
}
const GeometryInfo &Object::getGeometryInfo() const
{
	static GeometryInfo g;
	g.m_maxHeight = g_scene.objs[m_index].maxHeight;
	return g;
}
AIUpdateInterface *Object::getAI() { return (g_scene.objs[m_index].flags & F_AI) ? &g_ais[m_index] : NULL; }
const AIUpdateInterface *Object::getAI() const { return const_cast<Object *>(this)->getAI(); }

Real PartitionManager::getRelativeAngle2D(const Object *obj, const Coord3D *pos) const
{
	unsigned h = tuHash(40, { idOf(obj), bitsOf(pos->x), bitsOf(pos->y) });
	return (Real)((h >> 8) % 62832u) / 10000.0f - 3.1416f;
}
Real PartitionManager::getDistanceSquared(const Object *obj, const Coord3D *pos, DistanceCalculationType dc) const
{
	return (Real)((tuHash(41, { idOf(obj), bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), (unsigned)dc }) >> 8) % 10000u);
}
void PartitionManager::getVectorTo(const Object *obj, const Coord3D *pos, DistanceCalculationType, Coord3D &vec) const
{
	const Coord3D *p = obj->getPosition();
	vec.x = pos->x - p->x; vec.y = pos->y - p->y; vec.z = pos->z - p->z;
}
Object *PartitionManager::getClosestObject(const Coord3D *pos, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters)
{
	PartitionFilterPossibleToAttack *a = (PartitionFilterPossibleToAttack *)filters[0];
	PartitionFilterSamePlayer *p = (PartitionFilterSamePlayer *)filters[1];
	PartitionFilterSameMapStatus *m = (PartitionFilterSameMapStatus *)filters[2];
	unsigned n = (unsigned)g_objects.size();
	unsigned h = tuHash(49, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(maxDist), (unsigned)dc, (unsigned)a->m_type, idOf(a->m_obj),
		(unsigned)a->m_source, p->m_player ? (unsigned)p->m_player->m_index : 0xffffffffu, idOf(m->m_obj) });
	unsigned k = (h >> 8) % (n + 1);
	Object *r = k < n ? &g_objects[k] : NULL;
	logLine(22, { idOf(r) });
	return r;
}
void TerrainLogic::getBridgeAttackPoints(const Object *bridge, TBridgeAttackInfo *info)
{
	unsigned h1 = tuHash(42, { idOf(bridge) }), h2 = tuHash(43, { idOf(bridge) });
	info->attackPoint1.x = (Real)((h1 >> 8) % 4000u) / 10.0f; info->attackPoint1.y = (Real)((h1 >> 16) % 4000u) / 10.0f;
	info->attackPoint1.z = 0;
	info->attackPoint2.x = (Real)((h2 >> 8) % 4000u) / 10.0f; info->attackPoint2.y = (Real)((h2 >> 16) % 4000u) / 10.0f;
	info->attackPoint2.z = 0;
}
Bool Pathfinder::isAttackViewBlockedByObstacle(const Object *attacker, const Coord3D &attackerPos, const Object *victim, const Coord3D &victimPos)
{
	return tuPick(tuHash(44, { idOf(attacker), bitsOf(attackerPos.x), bitsOf(attackerPos.y), idOf(victim), bitsOf(victimPos.x),
		bitsOf(victimPos.y) }), g_scene.rates[0]);
}
Pathfinder *AI::pathfinder() { return &g_pathfinder; }

/* ---- the driver --------------------------------------------------------------------------------------------------------------- */
namespace {

enum { OBJ_WORDS = 26 };

void readScene(const unsigned *cfg)
{
	g_scene.seed = cfg[0];
	g_scene.rates[0] = cfg[1]; g_scene.rates[1] = cfg[2]; g_scene.rates[2] = cfg[3];
	g_frame = cfg[4];
	unsigned n = cfg[5];
	g_scene.objs.assign(n, TObj());
	g_objects.assign(n, Object());
	g_ais.assign(n, AIUpdateInterface());
	g_weapons.assign(n * 3, Weapon());
	g_players.assign(4, Player());
	for (int p = 0; p < 4; ++p) g_players[p].m_index = p;
	g_teams.clear();
	for (unsigned k = 0; k < n; ++k) {
		const unsigned *c = cfg + 6 + k * OBJ_WORDS;
		TObj &o = g_scene.objs[k];
		o.id = (ObjectID)c[0];
		o.pos.x = floatOf(c[1]); o.pos.y = floatOf(c[2]); o.pos.z = floatOf(c[3]);
		o.team = c[4]; o.kinds = c[5];
		o.statusLo = 0; o.statusHi = 0;
		if (c[6] & 1) o.statusLo |= 1ull << OBJECT_STATUS_UNDER_CONSTRUCTION;
		if (c[6] & 2) o.statusLo |= 1ull << OBJECT_STATUS_MASKED;
		if (c[6] & 4) o.statusLo |= 1ull << OBJECT_STATUS_NO_ATTACK;
		o.flags = c[7]; o.containedBy = (Int)c[8]; o.player = (Int)c[9];
		o.moodIdle = c[10]; o.moodAttack = c[11]; o.nextMood = c[12];
		o.goal = (Int)c[13]; o.lastCmd = c[14]; o.victim = (Int)c[15]; o.curSlot = (Int)c[16];
		o.weapons = c[17]; o.wstatus = c[18];
		o.range[0] = floatOf(c[19]); o.range[1] = floatOf(c[20]); o.range[2] = floatOf(c[21]);
		o.contRange = floatOf(c[22]); o.maxHeight = floatOf(c[23]); o.turretSync = (Int)c[24];
		g_objects[k].m_index = (Int)k;
		g_ais[k].m_owner = (Int)k;
		for (int s = 0; s < 3; ++s) { g_weapons[k * 3 + s].m_owner = (Int)k; g_weapons[k * 3 + s].m_slot = s; }
		if (o.team != 0) {
			Team &t = g_teams[o.team];
			t.m_id = o.team;
			t.m_target = INVALID_ID;
			t.m_prototype.m_info.m_attackCommonTarget = (c[25] & 1) != 0;
		}
	}
}

void readData(const unsigned *d, TurretAIData &td)
{
	td.m_turnRate = floatOf(d[0]); td.m_pitchRate = floatOf(d[1]);
	td.m_naturalTurretAngle = floatOf(d[2]); td.m_naturalTurretPitch = floatOf(d[3]);
	for (int s = 0; s < 3; ++s) { td.m_turretFireAngleSweep[s] = floatOf(d[4 + s]); td.m_turretSweepSpeedModifier[s] = floatOf(d[7 + s]); }
	td.m_firePitch = floatOf(d[10]); td.m_minPitch = floatOf(d[11]); td.m_groundUnitPitch = floatOf(d[12]);
	td.m_turretWeaponSlots = d[13];
	td.m_minIdleScanAngle = floatOf(d[14]); td.m_maxIdleScanAngle = floatOf(d[15]);
	td.m_minIdleScanInterval = d[16]; td.m_maxIdleScanInterval = d[17]; td.m_recenterTime = d[18];
	td.m_initiallyDisabled = (d[19] & 1) != 0; td.m_firesWhileTurning = (d[19] & 2) != 0; td.m_isAllowsPitch = (d[19] & 4) != 0;
}

std::vector<unsigned> g_out;
void put(unsigned w) { g_out.push_back(w); }

void dumpTurret(TurretAI *t)
{
	put(bitsOf(t->m_angle)); put(bitsOf(t->m_pitch)); put(t->m_enableSweepUntil);
	put(t->m_victimInitialTeam ? t->m_victimInitialTeam->m_id : 0u); put((unsigned)t->m_target);
	put(t->m_continuousFireExpirationFrame); put(t->m_sleepUntil);
	put((t->m_playRotSound ? 1u : 0u) | (t->m_playPitchSound ? 2u : 0u) | (t->m_positiveSweep ? 4u : 0u) | (t->m_didFire ? 8u : 0u) |
		(t->m_enabled ? 16u : 0u) | (t->m_firesWhileTurning ? 32u : 0u) | (t->m_isForceAttacking ? 64u : 0u) |
		(t->m_targetWasSetByIdleMood ? 128u : 0u));
	put(t->m_turretRotOrPitchSound.m_handle);
	TurretStateMachine *sm = t->m_turretStateMachine;
	put((unsigned)sm->getCurrentStateID()); put(sm->m_sleepTill); put((unsigned)sm->m_goalObjectID);
	put(bitsOf(sm->m_goalPosition.x)); put(bitsOf(sm->m_goalPosition.y)); put(bitsOf(sm->m_goalPosition.z));
	put(sm->m_locked ? 1u : 0u);
}

}  // namespace

extern "C" {

/* Batch T1: a turret on object 0 of a scene. cfg: seed, three rates (view blocked, in range, unused), the frame, the object count,
** each object 26 words (id; position; team (0 none); kinds (bits: bridge, immobile, spawns are the weapons, structure, mine,
** vehicle); status (bits: under construction, masked, no attack); flags (dead, destroyed, airborne target, significantly above,
** an AI, ground movement, preventing aim, turrets linked); contained by and its AI's goal and victim (object index + 1, or 0); the
** controlling player (or -1); the AI's mood adjustments (idle, attack), next mood check, last command source; the current weapon
** slot (3: none); the weapons (bits 0-2 present, 3-5 contact, 6-8 leech); their statuses (3 bits each); three attack ranges, the
** continue range and the max height (float bits); the turret sync; the team's attack-common-target flag), then the turret's data
** (20 words, readData) and its turret. Ops of 6 words [op, a, b, c, d, e]: 0 frame = a  1 updateTurretAI  2 setTurretTargetObject(
** object a (or none: 0xffffffff), forced b)  3 setTurretTargetPosition((a, b) as float bits, none when c)  4 recenterTurret
** 5 setTurretEnabled(a)  6 isTurretInNaturalPosition  7 isTryingToAimAtTarget(a)  8 isOwnersCurWeaponOnTurret and
** isWeaponSlotOnTurret(a)  9 notifyNewVictimChosen(a)  10 object a's word b = c (as the cfg's; the status word replaces its three
** bits)  11 xfer save (the bytes)  12 xfer load of the last save, then loadPostProcess  13 the destructor (last). Each op appends,
** after a word that counts the rest: its results, the log of its calls, the turret's state (dumpTurret). */
int ref_turret_run(const unsigned *cfg, const unsigned *data, int n, const unsigned *ops, const char *xfer_path, unsigned *buf, int cap,
	int *len)
{
	g_out.clear();
	g_log.clear();
	g_counter = 0;
	g_nextHandle = 0;
	g_playing.clear();
	readScene(cfg);
	static TurretAIData td;
	readData(data, td);
	WhichTurretType which = (WhichTurretType)data[20];
	TurretAI *t = NULL;
	try {
		t = newInstance(TurretAI)(&g_objects[0], &td, which);
	} catch (...) {
		put(0xbadc0de1u);
		*len = (int)g_out.size();
		memcpy(buf, &g_out[0], g_out.size() * sizeof(unsigned));
		return 0;
	}
	std::vector<unsigned char> saved;
	{
		size_t mark = g_out.size();
		put(0);
		put((unsigned)g_log.size());
		for (unsigned w : g_log) put(w);
		g_log.clear();
		dumpTurret(t);
		g_out[mark] = (unsigned)(g_out.size() - mark - 1);
	}
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		std::vector<unsigned> res;
		size_t mark = g_out.size();
		put(0);
		switch (o[0]) {
		case 0: g_frame = o[1]; break;
		case 1: res.push_back((unsigned)t->updateTurretAI()); break;
		case 2: t->setTurretTargetObject(o[1] == 0xffffffffu ? NULL : &g_objects[o[1] % g_objects.size()], o[2] != 0); break;
		case 3: {
			Coord3D p; p.x = floatOf(o[1]); p.y = floatOf(o[2]); p.z = 0;
			t->setTurretTargetPosition(o[3] ? NULL : &p);
			break;
		}
		case 4: t->recenterTurret(); break;
		case 5: t->setTurretEnabled(o[1] != 0); break;
		case 6: res.push_back(t->isTurretInNaturalPosition() ? 1u : 0u); break;
		case 7: res.push_back(t->isTryingToAimAtTarget(&g_objects[o[1] % g_objects.size()]) ? 1u : 0u); break;
		case 8:
			res.push_back(t->isOwnersCurWeaponOnTurret() ? 1u : 0u);
			res.push_back(t->isWeaponSlotOnTurret((WeaponSlotType)(o[1] % 3)) ? 1u : 0u);
			break;
		case 9: t->notifyNewVictimChosen(&g_objects[o[1] % g_objects.size()]); break;
		case 10: {
			TObj &ob = g_scene.objs[o[1] % g_scene.objs.size()];
			unsigned v = o[3];
			switch (o[2]) {
			case 1: ob.pos.x = floatOf(v); break;
			case 4: ob.team = v; if (v) { Team &tm = g_teams[v]; tm.m_id = v; } break;
			case 5: ob.kinds = v; break;
			case 6:
				ob.statusLo &= ~((1ull << OBJECT_STATUS_UNDER_CONSTRUCTION) | (1ull << OBJECT_STATUS_MASKED) | (1ull << OBJECT_STATUS_NO_ATTACK));
				if (v & 1) ob.statusLo |= 1ull << OBJECT_STATUS_UNDER_CONSTRUCTION;
				if (v & 2) ob.statusLo |= 1ull << OBJECT_STATUS_MASKED;
				if (v & 4) ob.statusLo |= 1ull << OBJECT_STATUS_NO_ATTACK;
				break;
			case 7: ob.flags = v; break;
			case 10: ob.moodIdle = v; break;
			case 11: ob.moodAttack = v; break;
			case 12: ob.nextMood = v; break;
			case 13: ob.goal = (Int)v; break;
			case 15: ob.victim = (Int)v; break;
			case 16: ob.curSlot = (Int)v; break;
			case 17: ob.weapons = v; break;
			case 18: ob.wstatus = v; break;
			}
			break;
		}
		case 11: {
			XferSave x;
			int st = 0;
			try { x.open(AsciiString(xfer_path)); x.xferSnapshot(t); } catch (...) { st = 1; }
			try { x.close(); } catch (...) {}
			saved.clear();
			if (FILE *f = fopen(xfer_path, "rb")) {
				unsigned char b[4096];
				size_t m = fread(b, 1, sizeof b, f);
				fclose(f);
				saved.assign(b, b + m);
			}
			res.push_back((unsigned)st);
			res.push_back((unsigned)saved.size());
			for (unsigned char c : saved) res.push_back(c);
			break;
		}
		case 12: {
			if (FILE *f = fopen(xfer_path, "wb")) { if (!saved.empty()) fwrite(&saved[0], 1, saved.size(), f); fclose(f); }
			XferLoad x;
			int st = 0;
			try { x.open(AsciiString(xfer_path)); x.xferSnapshot(t); } catch (...) { st = 1; }
			try { x.close(); } catch (...) {}
			t->loadPostProcess();
			res.push_back((unsigned)st);
			break;
		}
		case 13: t->deleteInstance(); t = NULL; break;
		}
		put((unsigned)res.size());
		for (unsigned r : res) put(r);
		put((unsigned)g_log.size());
		for (unsigned w : g_log) put(w);
		g_log.clear();
		if (t) dumpTurret(t);
		g_out[mark] = (unsigned)(g_out.size() - mark - 1);
		if (!t) break;
	}
	if (t) t->deleteInstance();
	*len = (int)g_out.size();
	if (*len > cap) return 1;
	memcpy(buf, &g_out[0], g_out.size() * sizeof(unsigned));
	return 0;
}

}  // extern "C"
