/* The driver of the Weapon firing differential (p1w8-sonnet slice B): the definitions of the stand-ins of
** fire_standins.h, the program reader, the operations, and the entry point ref_weapon_fire_run. See
** tests/differential_weapon_fire.rs for the program format. Included by differential/wrap_weapon.cpp after the cut
** Weapon.cpp and WeaponSet.cpp. */
#ifndef WPNFIRE_DRIVER_H
#define WPNFIRE_DRIVER_H

#include <string.h>
#include "Common/RandomValue.h"
#include "Common/XferCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"

/* ---- globals ----------------------------------------------------------------------------------------------- */
GlobalData g_globalData;
GlobalData* TheGlobalData = &g_globalData;
GameLogic g_gameLogic; GameLogic* TheGameLogic = &g_gameLogic;
PartitionManager g_partition; PartitionManager* ThePartitionManager = &g_partition;
TerrainLogic g_terrain; TerrainLogic* TheTerrainLogic = &g_terrain;
AIManager g_ai; AIManager* TheAI = &g_ai;
PlayerList g_playerList; PlayerList* ThePlayerList = &g_playerList;
static unsigned long long g_idOfSeed = 0;

static const char* fxName(const void* p, const char* prefix, char* buf)
{
	if (!p) return "-";
	sprintf(buf, "%s%u", prefix, (unsigned)(size_t)p);
	return buf;
}
static int tplIndex(const WeaponTemplate* t)
{
	if (!t) return -1;
	for (size_t i = 0; i < TheWeaponStore->m_weaponTemplateVector.size(); ++i) if (TheWeaponStore->m_weaponTemplateVector[i] == t) return (int)i;
	return -2;
}
static int oid(const Object* o) { return o ? (int)o->getID() : 0; }

/* ---- Object ------------------------------------------------------------------------------------------------ */
Object::Object() : m_geom(GEOMETRY_SPHERE, false, 0.0f, 0.0f, 0.0f)
{
	m_id = 0; m_template = NULL; m_player = -1; m_pos.x = m_pos.y = m_pos.z = 0; m_orientation = 0; m_xform.Make_Identity();
	m_statusBits = 0; m_scriptStatus = 0; m_veterancy = 0; m_locallyControlled = false; m_bonusCond = 0; m_containedBy = 0; m_producer = 0;
	m_effDead = m_destroyed = m_ableToAttack = false; m_layer = LAYER_GROUND; m_aboveTerrain = m_signifAbove = false;
	m_consecutiveShots = 0; m_estimate = 0; m_hasDrawable = false; m_barrels[0] = m_barrels[1] = m_barrels[2] = 1; m_fxHandled = false;
	m_hasLaunchOffset = false; m_launchXform.Make_Identity(); m_turretRotPos.x = m_turretRotPos.y = m_turretRotPos.z = 0;
	m_turretPitchPos = m_turretRotPos; m_hasLaserUpdate = false;
	m_hasAI = m_sneaky = false; m_sneakyOffset = m_turretRotPos; m_locoSet = 0; m_adjustsDest = m_assaultIface = false;
	for (int i = 0; i < 3; ++i) { m_turret[i] = TURRET_INVALID; m_turretAngle[i] = 0; m_turretPitch[i] = 0; m_aiming[i] = false; }
	m_hasContain = false; m_containCount = 0; m_garrisonable = m_immuneClear = m_passedBonus = false; m_passedFlags = 0; m_bestGarrison = false;
	m_passengerFire = false; m_hasItems = false; m_apparent = -2;
	m_hasStealth = m_disguised = false; m_disguisedPlayer = 0; m_hasSpawn = false; m_spawnResult = 0;
	m_hasLandMine = m_hasPUI = m_hasParking = m_hasSPCDie = m_hasStream = m_hasAssist = m_assistFree = m_hasCountermeasures = m_hasPhysics = false;
	m_launcherId = 0;
	m_hasTracker = false; m_firingTracker = NULL; m_tracker.m_owner = this; for (int i = 0; i < 3; ++i) m_lastWeaponCondition[i] = 0;
	m_drawable = NULL; m_drawableStorage.m_owner = this; m_laser.m_owner = this; m_ai.m_owner = this; m_assault.m_owner = this; m_contain.m_owner = this; m_stealth.m_owner = this;
	m_spawn.m_owner = this; m_pui.m_owner = this; m_lmi.m_owner = this; m_ppi.m_owner = this; m_spcDie.m_owner = this; m_stream.m_owner = this;
	m_assist.m_owner = this; m_phys.m_owner = this; m_xp.m_owner = this;
	for (int i = 0; i < 3; ++i) { m_mods[i].m_owner = this; m_mods[i].m_kind = i; }
	m_modList[0] = m_modList[1] = m_modList[2] = m_modList[3] = NULL;
}
Real Object::estimateDamage(DamageInfoInput& di) const
{
	Rec("est victim=%d src=%d dt=%d death=%d amt=%08x", m_id, (int)di.m_sourceID, (int)di.m_damageType, (int)di.m_deathType, FB(di.m_amount));
	return m_estimate;
}
void Object::attemptDamage(DamageInfo* d)
{
	const DamageInfoInput& i = d->in;
	Rec("dmg victim=%d src=%d mask=%u dt=%d dst=%d fxo=%d death=%d amt=%08x kill=%d sw=%08x,%08x,%08x swa=%08x swr=%08x swt=%08x", m_id, (int)i.m_sourceID,
		(unsigned)i.m_sourcePlayerMask, (int)i.m_damageType, (int)i.m_damageStatusType, (int)i.m_damageFXOverride, (int)i.m_deathType, FB(i.m_amount),
		i.m_kill ? 1 : 0, FB(i.m_shockWaveVector.x), FB(i.m_shockWaveVector.y), FB(i.m_shockWaveVector.z), FB(i.m_shockWaveAmount),
		FB(i.m_shockWaveRadius), FB(i.m_shockWaveTaperOff));
}
Relationship Object::getRelationship(const Object* that) const
{
	if (m_player < 0 || !that || that->m_player < 0) return NEUTRAL;
	return (Relationship)g_wcfg.players[m_player].rel[that->m_player];
}
Player* Object::getControllingPlayer() const { return m_player >= 0 ? g_players[m_player] : NULL; }
Team* Object::getTeam() const { return m_player >= 0 ? &g_players[m_player]->m_team : NULL; }
const Object* Object::getContainedBy() const { return m_containedBy ? wpnFind(m_containedBy) : NULL; }
UpdateModule* Object::findUpdateModule(NameKeyType key) const
{
	static NameKeyType kStream = TheNameKeyGenerator->nameToKey("ProjectileStreamUpdate");
	static NameKeyType kAssist = TheNameKeyGenerator->nameToKey("AssistedTargetingUpdate");
	if (key == kStream && m_hasStream) return const_cast<ProjectileStreamUpdate*>(&m_stream);
	if (key == kAssist && m_hasAssist) return const_cast<AssistedTargetingUpdate*>(&m_assist);
	return NULL;
}
void Object::reportMissileForCountermeasures(Object* missile) { Rec("cm.report o=%d missile=%d", m_id, oid(missile)); }
void Object::setProducer(const Object* obj) { m_producer = obj ? (int)obj->getID() : 0; Rec("producer p=%d s=%d", m_id, oid(obj)); }
void Object::setPosition(const Coord3D* pos) { m_pos = *pos; Rec("setpos o=%d %08x,%08x,%08x", m_id, FB(pos->x), FB(pos->y), FB(pos->z)); }
void Object::setTransformMatrix(const Matrix3D* mx)
{
	m_xform = *mx;
	char b[256]; int n = 0;
	for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) n += sprintf(b + n, "%08x ", FB((*mx)[r][c]));
	Rec("xform o=%d %s", m_id, b);
}
/* the stand-in conversion (Thing's and the drawable's): the world position is the bone position moved by the object's position
** (nothing for no bone position), the world transform is the object's transform times the bone's. The inputs are read before any
** output is written (upstream calls it with the same pointer in and out). */
void Object::convertBonePosToWorldPos(const Coord3D* bonePos, const Matrix3D* boneTransform, Coord3D* worldPos, Matrix3D* worldTransform) const
{
	Coord3D p; p.x = p.y = p.z = 0;
	if (bonePos) { p.x = bonePos->x + m_pos.x; p.y = bonePos->y + m_pos.y; p.z = bonePos->z + m_pos.z; }
	Matrix3D t(true);
	if (boneTransform) t.mul(m_xform, *boneTransform);
	if (worldTransform) *worldTransform = t;
	if (worldPos) *worldPos = p;
}
Int Drawable::getPristineBonePositions(const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const
{
	unsigned h = 0;
	for (const char* c = boneNamePrefix; *c; ++c) h = h * 31u + (unsigned char)*c;
	Int count = (Int)((h + (unsigned)startIndex) % 4u);
	if (count > maxBones) count = maxBones;
	for (Int i = 0; i < count; ++i)
	{
		Coord3D p; p.x = (Real)(h % 97u) + (Real)i; p.y = (Real)(i * 2); p.z = (Real)startIndex + 0.5f;
		if (positions) positions[i] = p;
		if (transforms) { transforms[i].Make_Identity(); transforms[i].Rotate_Z(0.25f * (Real)(i + 1)); transforms[i].Set_Translation(Vector3(p.x, p.y, p.z)); }
	}
	return count;
}
void Drawable::convertBonePosToWorldPos(const Coord3D* bonePos, const Matrix3D* boneTransform, Coord3D* worldPos, Matrix3D* worldTransform) const
{
	m_owner->convertBonePosToWorldPos(bonePos, boneTransform, worldPos, worldTransform);
}
Bool AIUpdateInterface::getTurretRotAndPitch(WhichTurretType tur, Real* turretAngle, Real* turretPitch) const
{
	if (tur < 0 || tur > 2) return false;
	if (turretAngle) *turretAngle = m_owner->m_turretAngle[tur];
	if (turretPitch) *turretPitch = m_owner->m_turretPitch[tur];
	return true;
}
/* command sets: "Cmd<k>" has a button i when (k + i) % 3 != 0, and that button is an upgrade (id (k*7 + i*3) % 5, 0 = not an upgrade) */
const CommandButton* CommandSet::getCommandButton(Int i) const { return ((m_k + i) % 3 != 0) ? &m_buttons[i] : NULL; }
const CommandSet* ControlBar::findCommandSet(const AsciiString& name) const
{
	static std::map<int, CommandSet*> sets;
	if (strncmp(name.str(), "Cmd", 3) != 0) return NULL;
	int k = atoi(name.str() + 3);
	if (sets.find(k) == sets.end())
	{
		CommandSet* cs = new CommandSet; cs->m_k = k;
		for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i) cs->m_buttons[i].m_up = (const UpgradeTemplate*)(size_t)((k * 7 + i * 3) % 5);
		sets[k] = cs;
	}
	return sets[k];
}
static ControlBar g_controlBar;
ControlBar* TheControlBar = &g_controlBar;

/* ---- modules ------------------------------------------------------------------------------------------------ */
LandMineInterface* BehaviorModule::getLandMineInterface() { return m_kind == 0 ? &m_owner->m_lmi : NULL; }
ProjectileUpdateInterface* BehaviorModule::getProjectileUpdateInterface() { return m_kind == 1 ? &m_owner->m_pui : NULL; }
ParkingPlaceBehaviorInterface* BehaviorModule::getParkingPlaceBehaviorInterface() { return m_kind == 2 ? &m_owner->m_ppi : NULL; }
void ProjectileUpdateInterface::projectileLaunchAtObjectOrPosition(const Object* victim, const Coord3D* victimPos, const Object* launcher, WeaponSlotType wslot,
	Int specificBarrelToUse, const WeaponTemplate* detWeap, const ParticleSystemTemplate* exhaustSysOverride)
{
	char b[32];
	Rec("launch proj=%d victim=%d dest=%08x,%08x,%08x src=%d slot=%d bar=%d tpl=%d exhaust=%s", m_owner->m_id, oid(victim), FB(victimPos->x), FB(victimPos->y),
		FB(victimPos->z), oid(launcher), (int)wslot, (int)specificBarrelToUse, tplIndex(detWeap), fxName(exhaustSysOverride, "ps", b));
	m_owner->m_launcherId = (int)launcher->getID();
}
ObjectID ProjectileUpdateInterface::projectileGetLauncherID() const { return (ObjectID)m_owner->m_launcherId; }
void LandMineInterface::disarm() { Rec("lm.disarm o=%d", m_owner->m_id); }
Bool ParkingPlaceBehaviorInterface::hasReservedSpace(ObjectID id) const { return m_owner->m_reserved.count((int)id) != 0; }
void SpecialPowerCompletionDie::notifyScriptEngine() { Rec("spc.notify o=%d", m_owner->m_id); }
void SpecialPowerCompletionDie::setCreator(ObjectID id) { Rec("spc.creator o=%d c=%d", m_owner->m_id, (int)id); }
void ProjectileStreamUpdate::setPosition(const Coord3D* p) { Rec("stream.pos o=%d %08x,%08x,%08x", m_owner->m_id, FB(p->x), FB(p->y), FB(p->z)); }
void ProjectileStreamUpdate::addProjectile(ObjectID s, ObjectID n, ObjectID v, const Coord3D* vp)
{
	Rec("stream.add o=%d src=%d proj=%d victim=%d pos=%08x,%08x,%08x", m_owner->m_id, (int)s, (int)n, (int)v, FB(vp->x), FB(vp->y), FB(vp->z));
}
Bool AssistedTargetingUpdate::isFreeToAssist() const { return m_owner->m_assistFree; }
void AssistedTargetingUpdate::assistAttack(const Object* req, Object* victim) { Rec("assist o=%d req=%d victim=%d", m_owner->m_id, oid(req), oid(victim)); }
void LaserUpdate::initLaser(const Object* parent, const Object* target, const Coord3D* startPos, const Coord3D* endPos, AsciiString bone)
{
	Rec("laser o=%d src=%d victim=%d start=%08x,%08x,%08x end=%08x,%08x,%08x bone=%s", m_owner->m_id, oid(parent), oid(target), FB(startPos->x), FB(startPos->y),
		FB(startPos->z), FB(endPos->x), FB(endPos->y), FB(endPos->z), bone.str());
}
void PhysicsBehavior::transferVelocityTo(PhysicsBehavior* that) const { Rec("phys.transfer from=%d to=%d", m_owner->m_id, that->m_owner->m_id); }
void PhysicsBehavior::setIgnoreCollisionsWith(const Object* obj) { Rec("phys.ignore o=%d with=%d", m_owner->m_id, oid(obj)); }
void ExperienceTracker::setExperienceSink(ObjectID sink) { Rec("xpsink o=%d sink=%d", m_owner->m_id, (int)sink); }
void AssaultTransportAIInterface::beginAssault(const Object* t) const { Rec("assault o=%d victim=%d", m_owner->m_id, oid(t)); }
void AcademyStats::recordMineCleared() { Rec("academy.mine p=%d", m_player); }

Bool AIUpdateInterface::getSneakyTargetingOffset(Coord3D* offset) const
{
	if (!m_owner->m_sneaky) return false;
	if (offset) *offset = m_owner->m_sneakyOffset;
	return true;
}
LocomotorSetType AIUpdateInterface::getCurLocomotorSetType() const { return (LocomotorSetType)m_owner->m_locoSet; }
Bool AIUpdateInterface::isAircraftThatAdjustsDestination() const { return m_owner->m_adjustsDest; }
AssaultTransportAIInterface* AIUpdateInterface::getAssaultTransportAIInterface() const { return m_owner->m_assaultIface ? &m_owner->m_assault : NULL; }
WhichTurretType AIUpdateInterface::getWhichTurretForWeaponSlot(WeaponSlotType wslot, Real* a, Real* p) const
{
	*a = m_owner->m_turretAngle[wslot]; *p = m_owner->m_turretPitch[wslot];
	return (WhichTurretType)m_owner->m_turret[wslot];
}
Bool AIUpdateInterface::isWeaponSlotOnTurretAndAimingAtTarget(WeaponSlotType wslot, const Object* victim) const { return m_owner->m_aiming[wslot]; }

Int ContainModuleInterface::getContainCount() const { return m_owner->m_containCount; }
Bool ContainModuleInterface::isGarrisonable() const { return m_owner->m_garrisonable; }
Bool ContainModuleInterface::isImmuneToClearBuildingAttacks() const { return m_owner->m_immuneClear; }
Bool ContainModuleInterface::isWeaponBonusPassedToPassengers() const { return m_owner->m_passedBonus; }
WeaponBonusConditionFlags ContainModuleInterface::getWeaponBonusPassedToPassengers() const { return m_owner->m_passedFlags; }
Bool ContainModuleInterface::isEnclosingContainerFor(const Object* obj) const { return m_owner->m_enclosing.count((int)obj->getID()) != 0; }
Bool ContainModuleInterface::calcBestGarrisonPosition(Coord3D* sourcePos, const Coord3D* targetPos)
{
	if (!m_owner->m_bestGarrison) return false;
	sourcePos->x = targetPos->x + 1.0f; sourcePos->y = targetPos->y + 2.0f; sourcePos->z = targetPos->z + 3.0f;
	return true;
}
Bool ContainModuleInterface::isPassengerAllowedToFire() const { return m_owner->m_passengerFire; }
const ContainedItemsList* ContainModuleInterface::getContainedItemsList() const { return m_owner->m_hasItems ? &m_owner->m_items : NULL; }
const Player* ContainModuleInterface::getApparentControllingPlayer(const Player* observer) const
{
	return m_owner->m_apparent >= 0 ? g_players[m_owner->m_apparent] : NULL;
}
Bool StealthUpdate::isDisguised() const { return m_owner->m_disguised; }
Int StealthUpdate::getDisguisedPlayerIndex() const { return m_owner->m_disguisedPlayer; }
CanAttackResult SpawnBehaviorInterface::getCanAnySlavesUseWeaponAgainstTarget(AbleToAttackType, const Object*, const Coord3D*, CommandSourceType) const
{
	return (CanAttackResult)m_owner->m_spawnResult;
}

Relationship Team::getRelationship(const Team* that) const { return (Relationship)g_wcfg.players[m_index].rel[that->m_index]; }
void Player::applyBattlePlanBonusesForObject(Object* obj) const { Rec("battleplan p=%d by=%d", oid(obj), m_index); }
void Player::iterateObjects(void (*func)(Object*, void*), void* userData)
{
	for (size_t i = 0; i < g_objs.size(); ++i) if (g_objs[i]->m_player == m_index && !g_objs[i]->m_destroyed) func(g_objs[i], userData);
}
Player* PlayerList::getNthPlayer(Int i) { return (i >= 0 && i < (Int)g_players.size()) ? g_players[i] : NULL; }

/* ---- the client ----------------------------------------------------------------------------------------------- */
void FXList::doFXPos(const FXList* fx, const Coord3D* primary, const Matrix3D* primaryMtx, Real primarySpeed, const Coord3D* secondary, Real radius)
{
	char b[32];
	Rec("fx.pos fx=%s at=%08x,%08x,%08x spd=%08x t=%08x,%08x,%08x rad=%08x", fxName(fx, "fx", b), FB(primary->x), FB(primary->y), FB(primary->z), FB(primarySpeed),
		FB(secondary->x), FB(secondary->y), FB(secondary->z), FB(radius));
}
Object* ObjectCreationList::create(const ObjectCreationList* ocl, const Object* primary, const Object* secondary)
{
	Rec("ocl id=%u primary=%d", (unsigned)(size_t)ocl, oid(primary));
	return NULL;
}
Int Drawable::getBarrelCount(WeaponSlotType wslot) const { return m_owner->m_barrels[wslot]; }
Bool Drawable::handleWeaponFireFX(WeaponSlotType wslot, Int specificBarrelToUse, const FXList* fxl, Real weaponSpeed, Real recoilAmount, Real recoilAngle,
	const Coord3D* victimPos, Real damageRadius)
{
	char b[32];
	Rec("fx.fire src=%d slot=%d bar=%d fx=%s spd=%08x ra=%08x rd=%08x t=%08x,%08x,%08x rad=%08x", m_owner->m_id, (int)wslot, (int)specificBarrelToUse,
		fxName(fxl, "fx", b), FB(weaponSpeed), FB(recoilAmount), FB(recoilAngle), FB(victimPos->x), FB(victimPos->y), FB(victimPos->z), FB(damageRadius));
	return m_owner->m_fxHandled;
}
const Coord3D* Drawable::getPosition() const { return &m_owner->m_pos; }
const Matrix3D* Drawable::getTransformMatrix() const { return &m_owner->m_xform; }
Bool Drawable::getProjectileLaunchOffset(WeaponSlotType wslot, Int specificBarrelToUse, Matrix3D* launchPos, WhichTurretType tur, Coord3D* rot, Coord3D* pitch) const
{
	if (!m_owner->m_hasLaunchOffset) return false;
	if (launchPos) *launchPos = m_owner->m_launchXform;
	if (rot) *rot = m_owner->m_turretRotPos;
	if (pitch) *pitch = m_owner->m_turretPitchPos;
	return true;
}
static std::string mtxStr(const Matrix3D& m)
{
	char t[200];
	snprintf(t, sizeof t, "%08x,%08x,%08x,%08x,%08x,%08x,%08x,%08x,%08x,%08x,%08x,%08x", FB(m[0][0]), FB(m[0][1]), FB(m[0][2]), FB(m[0][3]), FB(m[1][0]), FB(m[1][1]), FB(m[1][2]), FB(m[1][3]), FB(m[2][0]), FB(m[2][1]), FB(m[2][2]), FB(m[2][3]));
	return t;
}
static std::string mcFlagsStr(const ModelConditionFlags& f)
{
	std::string s;
	for (int b = 0; b < MODELCONDITION_COUNT; ++b) if (f.test(b)) { char t[16]; snprintf(t, sizeof t, "%d,", b); s += t; }
	return s;
}
void Drawable::updateDrawableClipStatus(UnsignedInt ammoInClip, UnsignedInt maxClipSize, WeaponSlotType slot) { Rec("clipstatus o=%d ammo=%u clip=%u slot=%d", m_owner->m_id, ammoInClip, maxClipSize, (int)slot); }
void Drawable::clearAndSetModelConditionFlags(const ModelConditionFlags& clr, const ModelConditionFlags& set) { Rec("mcflags o=%d clear=[%s] set=[%s]", m_owner->m_id, mcFlagsStr(clr).c_str(), mcFlagsStr(set).c_str()); }
void Drawable::setAnimationLoopDuration(UnsignedInt numFrames) { Rec("animloop o=%d frames=%u", m_owner->m_id, numFrames); }
void FiringTracker::shotFired(const Weapon* w, ObjectID victimID) { Rec("tracker o=%d slot=%d victim=%d next=%u last=%u", m_owner->m_id, (int)w->getWeaponSlot(), (int)victimID, w->getPossibleNextShotFrame(), w->getLastShotFrame()); }
void Object::friend_setUndetectedDefector(Bool status) { Rec("undetected o=%d v=%d", m_id, status ? 1 : 0); }
void Drawable::setDrawableHidden(Bool hidden) { Rec("unhide o=%d hidden=%d", m_owner->m_id, hidden ? 1 : 0); }
LaserUpdate* Drawable::findClientUpdateModule(NameKeyType key)
{
	static NameKeyType kLaser = TheNameKeyGenerator->nameToKey("LaserUpdate");
	return (key == kLaser && m_owner->m_hasLaserUpdate) ? &m_owner->m_laser : NULL;
}

/* ---- the partition manager, the terrain, the game logic -------------------------------------------------------- */
ObjectIterator* PartitionManager::iteratePotentialCollisions(const Coord3D* pos, const GeometryInfo& geom, Real angle) const
{
	/* the object whose position and geometry are given: its collision list (found by its position) */
	SimpleObjectIterator* it = new SimpleObjectIterator;
	for (size_t i = 0; i < g_objs.size(); ++i)
		if (&g_objs[i]->m_pos == pos)
			for (std::set<int>::const_iterator c = g_objs[i]->m_collides.begin(); c != g_objs[i]->m_collides.end(); ++c)
				if (Object* o = wpnFind(*c)) it->m_list.push_back(std::make_pair(o, 0.0f));
	return it;
}
void Pathfinder::adjustTargetDestination(const Object* obj, const Object* target, const Coord3D* targetPos, const Weapon* weapon, Coord3D* dest) const
{
	Rec("adjust src=%d target=%d tpl=%d", oid(obj), oid(target), tplIndex(weapon->getTemplate()));
	dest->x += 1.0f; dest->y += 2.0f;
}
void GameLogic::destroyObject(Object* obj) { Rec("destroy %d", oid(obj)); }

/* ---- the program reader --------------------------------------------------------------------------------------- */
struct Reader
{
	const unsigned* p; const unsigned* e;
	unsigned u() { if (p >= e) abort(); return *p++; }
	int i() { return (int)u(); }
	bool b() { return u() != 0; }
	float f() { unsigned w = u(); float x; memcpy(&x, &w, 4); return x; }
};
struct ThingProto { Reader proto; int id; };
static std::vector<std::vector<unsigned> > g_protos;  /* the object-record words of each thing template, for newObject */
static std::vector<WeaponBonusSet*> g_extraSets;

static Coord3D rdCoord(Reader& r) { Coord3D c; c.x = r.f(); c.y = r.f(); c.z = r.f(); return c; }

/* one object record (the words listed in tests/differential_weapon_fire.rs, "OBJECT") */
static void readObject(Reader& r, Object* o)
{
	int tmplId = r.i();
	o->m_template = (tmplId >= 1 && tmplId <= (int)g_thingTemplates.size()) ? g_thingTemplates[tmplId - 1] : g_thingTemplates[0];
	o->m_player = r.i();
	o->m_pos = rdCoord(r);
	o->m_orientation = r.f();
	o->m_xform.Make_Identity();
	o->m_xform.Rotate_Z(o->m_orientation);
	o->m_xform.Set_Translation(Vector3(o->m_pos.x, o->m_pos.y, o->m_pos.z));
	{ GeometryType gt = (GeometryType)r.u(); Bool small = r.b(); Real h = r.f(), ma = r.f(), mi = r.f(); o->m_geom = GeometryInfo(gt, small, h, ma, mi); }
	{ unsigned n = r.u(); for (unsigned k = 0; k < n; ++k) o->m_kindOf.set((KindOfType)r.u()); }
	{ unsigned lo = r.u(); unsigned hi = r.u(); o->m_statusBits = ((unsigned long long)hi << 32) | lo; }
	o->m_scriptStatus = r.u(); o->m_veterancy = r.i(); o->m_locallyControlled = r.b(); o->m_bonusCond = r.u();
	o->m_containedBy = r.i(); o->m_producer = r.i();
	o->m_effDead = r.b(); o->m_destroyed = r.b(); o->m_ableToAttack = r.b(); o->m_layer = r.i(); o->m_aboveTerrain = r.b(); o->m_signifAbove = r.b();
	o->m_consecutiveShots = r.i(); o->m_estimate = r.f();
	{ unsigned m = r.u(); o->m_wsFlags.clear(); for (int k = 0; k < 17; ++k) if (m & (1u << k)) o->m_wsFlags.set(k); }
	o->m_hasDrawable = r.b(); o->m_drawable = o->m_hasDrawable ? &o->m_drawableStorage : NULL; for (int k = 0; k < 3; ++k) o->m_barrels[k] = r.i(); o->m_fxHandled = r.b();
	o->m_hasLaunchOffset = r.b();
	{ Real rz = r.f(); Coord3D t = rdCoord(r); o->m_launchXform.Make_Identity(); o->m_launchXform.Rotate_Z(rz); o->m_launchXform.Set_Translation(Vector3(t.x, t.y, t.z)); }
	o->m_turretRotPos = rdCoord(r); o->m_turretPitchPos = rdCoord(r); o->m_hasLaserUpdate = r.b();
	o->m_hasAI = r.b(); o->m_sneaky = r.b(); o->m_sneakyOffset = rdCoord(r); o->m_locoSet = r.i(); o->m_adjustsDest = r.b(); o->m_assaultIface = r.b();
	for (int k = 0; k < 3; ++k) o->m_turret[k] = r.i();
	for (int k = 0; k < 3; ++k) o->m_turretAngle[k] = r.f();
	for (int k = 0; k < 3; ++k) o->m_turretPitch[k] = r.f();
	for (int k = 0; k < 3; ++k) o->m_aiming[k] = r.b();
	o->m_hasContain = r.b(); o->m_containCount = r.i(); o->m_garrisonable = r.b(); o->m_immuneClear = r.b(); o->m_passedBonus = r.b(); o->m_passedFlags = r.u();
	{ unsigned n = r.u(); for (unsigned k = 0; k < n; ++k) o->m_enclosing.insert(r.i()); }
	o->m_bestGarrison = r.b(); o->m_passengerFire = r.b(); o->m_hasItems = r.b();
	{ unsigned n = r.u(); for (unsigned k = 0; k < n; ++k) o->m_itemIds.push_back(r.i()); }
	o->m_apparent = r.i();
	o->m_hasStealth = r.b(); o->m_disguised = r.b(); o->m_disguisedPlayer = r.i(); o->m_hasSpawn = r.b(); o->m_spawnResult = r.i();
	o->m_hasLandMine = r.b(); o->m_hasPUI = r.b(); o->m_hasParking = r.b(); o->m_hasSPCDie = r.b(); o->m_hasStream = r.b(); o->m_hasAssist = r.b();
	o->m_assistFree = r.b(); o->m_hasCountermeasures = r.b(); o->m_hasPhysics = r.b(); o->m_launcherId = r.i();
	{ unsigned n = r.u(); for (unsigned k = 0; k < n; ++k) o->m_reserved.insert(r.i()); }
	{ unsigned n = r.u(); for (unsigned k = 0; k < n; ++k) o->m_collides.insert(r.i()); }
	{ int c = r.i(); char nm[24]; if (c < 0) nm[0] = 0; else if (c >= 100) snprintf(nm, sizeof nm, "Other"); else snprintf(nm, sizeof nm, "Cmd%d", c); o->m_commandSetStringOverride = nm; }
	o->m_hasTracker = r.b(); o->m_firingTracker = o->m_hasTracker ? &o->m_tracker : NULL; for (int k = 0; k < 3; ++k) o->m_lastWeaponCondition[k] = (Byte)r.i();
	/* the module list: land mine (0), projectile update (1), parking place (2), in that order */
	int n = 0;
	if (o->m_hasLandMine) o->m_modList[n++] = &o->m_mods[0];
	if (o->m_hasPUI) o->m_modList[n++] = &o->m_mods[1];
	if (o->m_hasParking) o->m_modList[n++] = &o->m_mods[2];
	o->m_modList[n] = NULL;
}
/* the contained items refer to objects that may not exist yet when the record is read */
static void finishObject(Object* o)
{
	o->m_items.clear();
	for (std::list<int>::const_iterator it = o->m_itemIds.begin(); it != o->m_itemIds.end(); ++it)
		if (Object* m = wpnFind(*it)) o->m_items.push_back(m);
}

Object* ThingFactory::newObject(const ThingTemplate* tmplt, Team* team)
{
	unsigned id = (unsigned)(size_t)tmplt;
	Rec("newobj tpl=%u team=%d -> %d", id, team->m_index, g_nextObjectId);
	Object* o = new Object;
	{
		/* the template's prototype object (a template id beyond the list is taken modulo its length) */
		const std::vector<unsigned>& proto = g_protos[(id - 1) % g_protos.size()];
		Reader r; r.p = &proto[0]; r.e = r.p + proto.size();
		readObject(r, o);
	}
	o->m_id = g_nextObjectId++;
	o->m_player = team->m_index;
	o->m_pos.x = o->m_pos.y = o->m_pos.z = 0;
	o->m_xform.Make_Identity();
	g_objs.push_back(o);
	return o;
}


/* ---- the dumps -------------------------------------------------------------------------------------------------- */
static std::string dumpWeapon(const Weapon* w)
{
	char b[512]; int n = 0;
	n += sprintf(b + n, "tpl=%d slot=%d st=%d ammo=%u next=%u pre=%u rel=%u last=%u sfx=%u strm=%d max=%d bar=%d nsb=%d pl=%d lr=%d sc=[",
		tplIndex(w->m_template), (int)w->m_wslot, (int)w->m_status, w->m_ammoInClip, w->m_whenWeCanFireAgain, w->m_whenPreAttackFinished,
		w->m_whenLastReloadStarted, w->m_lastFireFrame, w->m_suspendFXFrame, (int)w->m_projectileStreamID, (int)w->m_maxShotCount, (int)w->m_curBarrel,
		(int)w->m_numShotsForCurBarrel, w->m_pitchLimited ? 1 : 0, w->m_leechWeaponRangeActive ? 1 : 0);
	std::string s(b);
	for (size_t i = 0; i < w->m_scatterTargetsUnused.size(); ++i) { sprintf(b, "%d,", (int)w->m_scatterTargetsUnused[i]); s += b; }
	return s + "]";
}
static void dumpSnapshot(const std::vector<Weapon*>& reg)
{
	char b[1024];
	sprintf(b, "frame=%u rng=%08x", g_wcfg.frame, GetGameLogicRandomSeedCRC());
	Rec("%s", b);
	for (size_t i = 0; i < reg.size(); ++i) if (reg[i]) Rec("W%d %s", (int)i, dumpWeapon(reg[i]).c_str());
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		Object* o = g_objs[i];
		WeaponSet& ws = o->m_weaponSet;
		int tset = -1;
		if (ws.m_curWeaponTemplateSet && o->m_template)
			for (size_t k = 0; k < o->m_template->m_weaponTemplateSets.size(); ++k)
				if (&o->m_template->m_weaponTemplateSets[k] == ws.m_curWeaponTemplateSet) tset = (int)k;
		Rec("O%d pos=%08x,%08x,%08x launcher=%d bonus=%u status=%016llx cur=%d lock=%d mask=%u anti=%d pl=%d dw=%d tset=%d dmg=%u,%u lwc=%d,%d,%d",
			(int)o->m_id, FB(o->m_pos.x), FB(o->m_pos.y), FB(o->m_pos.z), (int)o->m_launcherId, o->m_bonusCond, o->m_statusBits, (int)ws.m_curWeapon,
			(int)ws.m_curWeaponLockedStatus, ws.m_filledWeaponSlotMask, (int)ws.m_totalAntiMask, ws.m_hasPitchLimit ? 1 : 0, ws.m_hasDamageWeapon ? 1 : 0, tset,
			(unsigned)ws.m_totalDamageTypeMask.count(), (unsigned)(ws.m_totalDamageTypeMask.test(DAMAGE_EXPLOSION) ? 1 : 0),
			(int)o->m_lastWeaponCondition[0], (int)o->m_lastWeaponCondition[1], (int)o->m_lastWeaponCondition[2]);
		for (int k = 0; k < WEAPONSLOT_COUNT; ++k)
			if (ws.m_weapons[k]) Rec("O%d.%d %s", (int)o->m_id, k, dumpWeapon(ws.m_weapons[k]).c_str());
	}
	for (size_t i = 0; i < TheWeaponStore->m_weaponTemplateVector.size(); ++i)
	{
		const WeaponTemplate* t = TheWeaponStore->m_weaponTemplateVector[i];
		if (t->m_historicDamage.empty()) continue;
		std::string s;
		for (HistoricWeaponDamageList::const_iterator it = t->m_historicDamage.begin(); it != t->m_historicDamage.end(); ++it)
		{ sprintf(b, "%u:%08x,%08x,%08x ", it->frame, FB(it->location.x), FB(it->location.y), FB(it->location.z)); s += b; }
		Rec("H%d %s", (int)i, s.c_str());
	}
	for (std::list<WeaponStore::WeaponDelayedDamageInfo>::const_iterator it = TheWeaponStore->m_weaponDDI.begin(); it != TheWeaponStore->m_weaponDDI.end(); ++it)
		Rec("DD tpl=%d pos=%08x,%08x,%08x frame=%u src=%d victim=%d bonus=%08x,%08x,%08x,%08x,%08x", tplIndex(it->m_delayedWeapon), FB(it->m_delayDamagePos.x),
			FB(it->m_delayDamagePos.y), FB(it->m_delayDamagePos.z), it->m_delayDamageFrame, (int)it->m_delaySourceID, (int)it->m_delayIntendedVictimID,
			FB(it->m_bonus.getField(WeaponBonus::DAMAGE)), FB(it->m_bonus.getField(WeaponBonus::RADIUS)), FB(it->m_bonus.getField(WeaponBonus::RANGE)),
			FB(it->m_bonus.getField(WeaponBonus::RATE_OF_FIRE)), FB(it->m_bonus.getField(WeaponBonus::PRE_ATTACK)));
}

static std::vector<unsigned char> readFileBytes(const char* path)
{
	std::vector<unsigned char> v;
	FILE* f = fopen(path, "rb");
	if (!f) return v;
	unsigned char buf[4096]; size_t n;
	while ((n = fread(buf, 1, sizeof buf, f)) > 0) v.insert(v.end(), buf, buf + n);
	fclose(f);
	return v;
}
static unsigned hashBytes(const std::vector<unsigned char>& v)
{
	unsigned h = 2166136261u;
	for (size_t i = 0; i < v.size(); ++i) h = (h ^ v[i]) * 16777619u;
	return h;
}

static WeaponBonus rdBonus(Reader& r)
{
	WeaponBonus b;
	for (int f = 0; f < WeaponBonus::FIELD_COUNT; ++f) b.setField((WeaponBonus::Field)f, r.f());
	return b;
}

/* ---- the world and the operations ----------------------------------------------------------------------------- */
static void buildWorld(Reader& r)
{
	g_lines->clear();
	for (size_t i = 0; i < g_objs.size(); ++i) delete g_objs[i];
	g_objs.clear(); g_thingTemplates.clear(); g_players.clear(); g_protos.clear(); g_nextObjectId = 1;
	unsigned seed = r.u();
	InitGameLogicRandom(seed);
	g_wcfg.frame = r.u();
	g_globalData.m_historicDamageLimit = r.u();
	g_wcfg.extent.lo = rdCoord(r); g_wcfg.extent.hi = rdCoord(r);
	delete g_globalData.m_weaponBonusSet; g_globalData.m_weaponBonusSet = NULL;
	if (r.b())
	{
		g_globalData.m_weaponBonusSet = new WeaponBonusSet;
		unsigned n = r.u();
		for (unsigned k = 0; k < n; ++k) { int c = r.i(); int f = r.i(); float v = r.f(); g_globalData.m_weaponBonusSet->m_bonus[c].setField((WeaponBonus::Field)f, v); }
	}
	/* the weapon templates */
	TheWeaponStore->m_weaponTemplateVector.clear();
	TheWeaponStore->m_weaponDDI.clear();
	unsigned nTpl = r.u();
	std::vector<int> histWeapon;
	for (unsigned k = 0; k < nTpl; ++k)
	{
		char nm[16]; sprintf(nm, "W%u", k);
		WeaponTemplate* t = TheWeaponStore->newWeaponTemplate(AsciiString(nm));
		t->m_primaryDamage = r.f(); t->m_primaryDamageRadius = r.f(); t->m_secondaryDamage = r.f(); t->m_secondaryDamageRadius = r.f();
		t->m_shockWaveAmount = r.f(); t->m_shockWaveRadius = r.f(); t->m_shockWaveTaperOff = r.f();
		t->m_attackRange = r.f(); t->m_minimumAttackRange = r.f(); t->m_requestAssistRange = r.f(); t->m_aimDelta = r.f();
		t->m_scatterRadius = r.f(); t->m_scatterTargetScalar = r.f();
		{ unsigned n = r.u(); for (unsigned q = 0; q < n; ++q) { Coord2D c; c.x = r.f(); c.y = r.f(); t->m_scatterTargets.push_back(c); } }
		t->m_damageType = (DamageType)r.i(); t->m_deathType = (DeathType)r.i();
		t->m_weaponSpeed = r.f(); t->m_weaponRecoil = r.f(); t->m_minTargetPitch = r.f(); t->m_maxTargetPitch = r.f(); t->m_radiusDamageAngle = r.f();
		{ unsigned id = r.u(); t->m_projectileTmpl = (const ThingTemplate*)(size_t)id; }
		{ unsigned k2 = r.u(); if (k2 == 1) t->m_laserName = "ProjLaser"; else if (k2 == 2) t->m_laserName = "Other"; }
		{ unsigned k2 = r.u(); if (k2 == 1) t->m_projectileStreamName = "ProjStream"; else if (k2 == 2) t->m_projectileStreamName = "Other"; }
		t->m_laserBoneName = "Bone";
		for (int v = 0; v < 4; ++v) t->m_fireOCLs[v] = (const ObjectCreationList*)(size_t)r.u();
		for (int v = 0; v < 4; ++v) t->m_projectileDetonationOCLs[v] = (const ObjectCreationList*)(size_t)r.u();
		for (int v = 0; v < 4; ++v) t->m_fireFXs[v] = (const FXList*)(size_t)r.u();
		for (int v = 0; v < 4; ++v) t->m_projectileDetonateFXs[v] = (const FXList*)(size_t)r.u();
		for (int v = 0; v < 4; ++v) t->m_projectileExhausts[v] = (const ParticleSystemTemplate*)(size_t)r.u();
		if (r.b())
		{
			t->m_extraBonus = new WeaponBonusSet;
			unsigned n = r.u();
			for (unsigned q = 0; q < n; ++q) { int c = r.i(); int f = r.i(); float v = r.f(); t->m_extraBonus->m_bonus[c].setField((WeaponBonus::Field)f, v); }
		}
		t->m_clipSize = r.i(); t->m_clipReloadTime = r.i(); t->m_minDelayBetweenShots = r.i(); t->m_maxDelayBetweenShots = r.i();
		t->m_continuousFireOneShotsNeeded = r.i(); t->m_continuousFireTwoShotsNeeded = r.i(); t->m_continuousFireCoastFrames = r.u();
		t->m_autoReloadWhenIdleFrames = r.u(); t->m_shotsPerBarrel = r.i();
		t->m_antiMask = r.u(); t->m_affectsMask = r.u(); t->m_collideMask = r.u();
		t->m_damageDealtAtSelfPosition = r.b(); t->m_reloadType = (WeaponReloadType)r.i(); t->m_prefireType = (WeaponPrefireType)r.i();
		t->m_historicBonusTime = r.u(); t->m_historicBonusRadius = r.f(); t->m_historicBonusCount = r.i(); histWeapon.push_back(r.i());
		t->m_leechRangeWeapon = r.b(); t->m_capableOfFollowingWaypoint = r.b(); t->m_isShowsAmmoPips = r.b(); t->m_allowAttackGarrisonedBldgs = r.b();
		t->m_playFXWhenStealthed = r.b(); t->m_preAttackDelay = r.i(); t->m_continueAttackRange = r.f(); t->m_infantryInaccuracyDist = r.f();
		t->m_damageStatusType = (ObjectStatusTypes)r.i(); t->m_suspendFXDelay = r.u();
	}
	for (unsigned k = 0; k < nTpl; ++k)
		TheWeaponStore->m_weaponTemplateVector[k]->m_historicBonusWeapon = histWeapon[k] >= 0 ? TheWeaponStore->m_weaponTemplateVector[histWeapon[k]] : NULL;
	/* the players */
	g_wcfg.nPlayers = (int)r.u();
	for (int k = 0; k < g_wcfg.nPlayers; ++k)
	{
		g_wcfg.players[k].battlePlans = r.i(); g_wcfg.players[k].mask = r.u();
		for (int q = 0; q < 4; ++q) g_wcfg.players[k].rel[q] = r.i();
		Player* p = new Player; p->m_index = k; p->m_team.m_index = k; p->m_academy.m_player = k; g_players.push_back(p);
	}
	/* the thing templates: group, fence width, and the object record dynamic objects copy */
	unsigned nThing = r.u();
	for (unsigned k = 0; k < nThing; ++k)
	{
		ThingTemplate* tt = new ThingTemplate; tt->m_id = (int)k + 1; tt->m_group = r.i(); tt->m_fenceWidth = r.f();
		{ char nm[24]; if (k % 4 == 3) nm[0] = 0; else snprintf(nm, sizeof nm, "Cmd%d", (int)k); tt->m_cmdSet = nm; }
		g_thingTemplates.push_back(tt);
		unsigned len = r.u(); std::vector<unsigned> words; for (unsigned q = 0; q < len; ++q) words.push_back(r.u());
		g_protos.push_back(words);
	}
	/* the weapon template sets, attached to their thing templates */
	unsigned nSets = r.u();
	for (unsigned k = 0; k < nSets; ++k)
	{
		int tid = r.i(); unsigned flags = r.u();
		WeaponTemplateSet ws;
		for (int b = 0; b < 17; ++b) if (flags & (1u << b)) ws.m_types.set(b);
		for (int s = 0; s < WEAPONSLOT_COUNT; ++s) { int idx = r.i(); ws.m_template[s] = idx >= 0 ? TheWeaponStore->m_weaponTemplateVector[idx] : NULL; }
		for (int s = 0; s < WEAPONSLOT_COUNT; ++s) ws.m_autoChooseMask[s] = r.u();
		for (int s = 0; s < WEAPONSLOT_COUNT; ++s) { unsigned n = r.u(); CLEAR_KINDOFMASK(ws.m_preferredAgainst[s]); for (unsigned q = 0; q < n; ++q) ws.m_preferredAgainst[s].set((KindOfType)r.u()); }
		ws.m_isReloadTimeShared = r.b(); ws.m_isWeaponLockSharedAcrossSets = r.b();
		ThingTemplate* tt = g_thingTemplates[tid - 1];
		ws.m_thingTemplate = tt;
		tt->m_weaponTemplateSets.push_back(ws);
	}
	for (size_t k = 0; k < g_thingTemplates.size(); ++k)
		for (size_t q = 0; q < g_thingTemplates[k]->m_weaponTemplateSets.size(); ++q) g_thingTemplates[k]->m_weaponTemplateSets[q].m_thingTemplate = g_thingTemplates[k];
	/* the objects */
	unsigned nObj = r.u();
	for (unsigned k = 0; k < nObj; ++k)
	{
		Object* o = new Object;
		unsigned len = r.u();
		Reader sub = r; sub.e = r.p + len;
		readObject(sub, o);
		r.p += len;
		o->m_id = g_nextObjectId++;
		g_objs.push_back(o);
	}
	for (size_t k = 0; k < g_objs.size(); ++k) finishObject(g_objs[k]);
}

static const char* ST[5] = { "ready", "oom", "between", "reload", "pre" };

/* fire_run: see tests/differential_weapon_fire.rs */
static int runProgram(const unsigned* words, int n, const char* tmpPath)
{
	Reader r; r.p = words; r.e = words + n;
	buildWorld(r);
	std::vector<Weapon*> reg;
	unsigned nOps = r.u();
	dumpSnapshot(reg);
	for (unsigned opn = 0; opn < nOps; ++opn)
	{
		unsigned op = r.u();
		char b[256];
		Rec("op %u", op);
		switch (op)
		{
		case 0: g_wcfg.frame = r.u(); break;
		case 1: { unsigned t = r.u(); int slot = r.i(); reg.push_back(TheWeaponStore->allocateNewWeapon(TheWeaponStore->m_weaponTemplateVector[t], (WeaponSlotType)slot)); break; }
		case 2: { Weapon* s = reg[r.u()]; Weapon* c = TheWeaponStore->allocateNewWeapon(s->m_template, s->m_wslot); *c = *s; reg.push_back(c); break; }
		case 3: { unsigned d = r.u(); unsigned s = r.u(); *reg[d] = *reg[s]; break; }
		case 4: { Weapon* w = reg[r.u()]; Object* src = wpnFind(r.i()); unsigned mode = r.u(); if (mode == 0) w->loadAmmoNow(src); else w->reloadAmmo(src); break; }
		case 5: { Weapon* w = reg[r.u()]; WeaponStatus st = w->getStatus(); Rec("ret status=%s remaining=%u pct=%08x next=%u", ST[st], w->getRemainingAmmo(), FB(w->getPercentReadyToFire()), w->getPossibleNextShotFrame()); break; }
		case 6: { Weapon* w = reg[r.u()]; float p = r.f(); bool a = r.b(); w->setClipPercentFull(p, a); break; }
		case 7: { Weapon* w = reg[r.u()]; Object* src = wpnFind(r.i()); w->onWeaponBonusChange(src); break; }
		case 8: {
			Weapon* w = reg[r.u()]; Object* src = wpnFind(r.i()); unsigned kind = r.u(); Object* victim = wpnFind(r.i()); Coord3D pos = rdCoord(r);
			unsigned extra = r.u(); bool inflict = r.b();
			ObjectID pid = (ObjectID)12345; Bool res = 0; Object* forced = NULL;
			switch (kind)
			{
			case 0: res = w->fireWeapon(src, victim, &pid); break;
			case 1: res = w->fireWeapon(src, &pos, &pid); break;
			case 2: w->fireProjectileDetonationWeapon(src, victim, extra, inflict); break;
			case 3: w->fireProjectileDetonationWeapon(src, &pos, extra, inflict); break;
			case 4: forced = w->forceFireWeapon(src, &pos); break;
			}
			Rec("ret fire kind=%u reloaded=%d pid=%d forced=%d", kind, res ? 1 : 0, (int)pid, oid(forced));
			break; }
		case 9: { Weapon* w = reg[r.u()]; Object* src = wpnFind(r.i()); Object* victim = wpnFind(r.i()); w->preFireWeapon(src, victim); break; }
		case 10: {
			Weapon* w = reg[r.u()]; Object* src = wpnFind(r.i()); Object* victim = wpnFind(r.i()); Coord3D pos = rdCoord(r); Coord3D goal = rdCoord(r);
			Rec("ret ranges inobj=%d inpos=%d closeobj=%d closepos=%d goalpos=%d srcgoal=%d,%d range=%08x dist=%08x pdr=%08x crt=%d dmgw=%d pitch=%d est=%08x los=%d,%d,%d,%d",
				victim ? w->isWithinAttackRange(src, victim) : -1, w->isWithinAttackRange(src, &pos), victim ? w->isTooClose(src, victim) : -1, w->isTooClose(src, &pos),
				victim ? w->isGoalPosWithinAttackRange(src, &goal, victim, &pos) : w->isGoalPosWithinAttackRange(src, &goal, NULL, &pos),
				w->isSourceObjectWithGoalPositionWithinAttackRange(src, &goal, victim, &pos), w->isSourceObjectWithGoalPositionWithinAttackRange(src, &goal, NULL, &pos),
				FB(w->getAttackRange(src)), FB(w->getAttackDistance(src, victim, &pos)), FB(w->getPrimaryDamageRadius(src)), (int)w->getClipReloadTime(src),
				w->isDamageWeapon() ? 1 : 0, victim ? w->isWithinTargetPitch(src, victim) : -1, victim ? FB(w->estimateWeaponDamage(src, victim)) : 0u,
				victim ? w->isClearFiringLineOfSightTerrain(src, victim) : -1, w->isClearFiringLineOfSightTerrain(src, pos),
				victim ? w->isClearGoalFiringLineOfSightTerrain(src, goal, victim) : -1, w->isClearGoalFiringLineOfSightTerrain(src, goal, pos));
			break; }
		case 11: {
			Weapon* w = reg[r.u()]; Object* src = wpnFind(r.i()); Object* target = wpnFind(r.i()); bool hasPos = r.b(); Coord3D pos = rdCoord(r); Real ao = r.f();
			Coord3D out; out.x = out.y = out.z = 0;
			Bool close = w->computeApproachTarget(src, target, hasPos ? &pos : NULL, ao, out);
			Rec("ret approach close=%d out=%08x,%08x,%08x", close ? 1 : 0, FB(out.x), FB(out.y), FB(out.z));
			break; }
		case 12: {
			const WeaponTemplate* t = TheWeaponStore->m_weaponTemplateVector[r.u()]; Object* src = wpnFind(r.i()); Object* victim = wpnFind(r.i());
			bool hasPos = r.b(); Coord3D pos = rdCoord(r); WeaponBonus bonus = rdBonus(r);
			Rec("ret estimate %08x", FB(t->estimateWeaponTemplateDamage(src, victim, hasPos ? &pos : NULL, bonus)));
			break; }
		case 13: {
			const WeaponTemplate* t = TheWeaponStore->m_weaponTemplateVector[r.u()]; Object* launcher = wpnFind(r.i()); Object* proj = wpnFind(r.i());
			Object* thing = wpnFind(r.i()); ObjectID intended = (ObjectID)r.i();
			Rec("ret collide %d", t->shouldProjectileCollideWith(launcher, proj, thing, intended) ? 1 : 0);
			break; }
		case 14: {
			const WeaponTemplate* t = TheWeaponStore->m_weaponTemplateVector[r.u()]; ObjectID src = (ObjectID)r.i(); ObjectID victim = (ObjectID)r.i(); Coord3D pos = rdCoord(r);
			WeaponBonus bonus = rdBonus(r); bool isDet = r.b();
			t->dealDamageInternal(src, victim, &pos, bonus, isDet);
			break; }
		case 15: {
			const WeaponTemplate* t = TheWeaponStore->m_weaponTemplateVector[r.u()]; Object* src = wpnFind(r.i()); unsigned kind = r.u(); Object* victim = wpnFind(r.i());
			Coord3D pos = rdCoord(r); unsigned extra = r.u(); bool inflict = r.b();
			if (kind == 0) TheWeaponStore->createAndFireTempWeapon(t, src, &pos);
			else if (kind == 1) TheWeaponStore->createAndFireTempWeapon(t, src, victim);
			else TheWeaponStore->handleProjectileDetonation(t, src, &pos, extra, inflict);
			break; }
		case 16: TheWeaponStore->update(); break;
		case 17: {
			Object* launcher = wpnFind(r.i()); int wslot = r.i(); int barrel = r.i();
			Matrix3D m(true); Coord3D p;
			Weapon::calcProjectileLaunchPosition(launcher, (WeaponSlotType)wslot, barrel, m, p);
			n = sprintf(b, "ret launchpos pos=%08x,%08x,%08x m=", FB(p.x), FB(p.y), FB(p.z));
			for (int rr = 0; rr < 3; ++rr) for (int c = 0; c < 4; ++c) n += sprintf(b + n, "%08x ", FB(m[rr][c]));
			Rec("%s", b);
			break; }
		case 18: {
			Object* proj = wpnFind(r.i()); Object* launcher = wpnFind(r.i()); int wslot = r.i(); int barrel = r.i();
			Weapon::positionProjectileForLaunch(proj, launcher, (WeaponSlotType)wslot, barrel);
			break; }
		case 19: {
			Weapon* w = reg[r.u()];
			{ XferCRC xc; xc.open(AsciiString("w")); xc.xferSnapshot(w); xc.close(); Rec("ret crc %08x", xc.getCRC()); }
			XferSave xs; xs.open(AsciiString(tmpPath)); xs.xferSnapshot(w); xs.close();
			std::vector<unsigned char> bytes = readFileBytes(tmpPath);
			Rec("ret xfer-save len=%u hash=%08x", (unsigned)bytes.size(), hashBytes(bytes));
			Weapon* c = TheWeaponStore->allocateNewWeapon(w->m_template, w->m_wslot);
			unsigned status = 0;
			XferLoad xl;
			try { xl.open(AsciiString(tmpPath)); xl.xferSnapshot(c); xl.close(); } catch (XferStatus s) { status = 1u + (unsigned)s; try { xl.close(); } catch (...) {} }
			catch (...) { status = 100; try { xl.close(); } catch (...) {} }
			Rec("ret xfer-load %u %s", status, dumpWeapon(c).c_str());
			c->deleteInstance();
			break; }
		case 20: { Object* o = wpnFind(r.i()); o->m_weaponSet.updateWeaponSet(o); break; }
		case 21: { Object* o = wpnFind(r.i()); bool now = r.b(); o->m_weaponSet.reloadAllAmmo(o, now); break; }
		case 22: { Object* o = wpnFind(r.i()); o->m_weaponSet.weaponSetOnWeaponBonusChange(o); break; }
		case 23: {
			Object* o = wpnFind(r.i()); Object* v = wpnFind(r.i()); int crit = r.i(); int cmd = r.i();
			Rec("ret choose %d", o->m_weaponSet.chooseBestWeaponForTarget(o, v, (WeaponChoiceCriteria)crit, (CommandSourceType)cmd) ? 1 : 0);
			break; }
		case 24: {
			Object* o = wpnFind(r.i()); Object* v = wpnFind(r.i()); int at = r.i(); int cmd = r.i(); int slot = r.i();
			Rec("ret able %d", (int)o->m_weaponSet.getAbleToAttackSpecificObject((AbleToAttackType)at, o, v, (CommandSourceType)cmd, (WeaponSlotType)slot));
			break; }
		case 25: {
			Object* o = wpnFind(r.i()); Object* v = wpnFind(r.i()); bool hasPos = r.b(); Coord3D pos = rdCoord(r); int at = r.i(); int cmd = r.i(); int slot = r.i();
			Rec("ret ableuse %d", (int)o->m_weaponSet.getAbleToUseWeaponAgainstTarget((AbleToAttackType)at, o, v, hasPos ? &pos : NULL, (CommandSourceType)cmd, (WeaponSlotType)slot));
			break; }
		case 26: {
			Object* o = wpnFind(r.i()); int what = r.i(); int slot = r.i(); int type = r.i();
			if (what == 0) Rec("ret lock %d", o->m_weaponSet.setWeaponLock((WeaponSlotType)slot, (WeaponLockType)type) ? 1 : 0);
			else o->m_weaponSet.releaseWeaponLock((WeaponLockType)type);
			break; }
		case 27: {
			Object* o = wpnFind(r.i()); WeaponSet& ws = o->m_weaponSet;
			const Weapon* pip = ws.findAmmoPipShowingWeapon(); Weapon* wp = ws.findWaypointFollowingCapableWeapon();
			Rec("ret setq oom=%d any=%d anydmg=%d single=%d,%d locked=%d slot=%d shared=%d pip=%d wp=%d most=%u cmd=%u,%u,%u cur=%d", ws.isOutOfAmmo() ? 1 : 0,
				ws.hasAnyWeapon() ? 1 : 0, ws.hasAnyDamageWeapon() ? 1 : 0, ws.hasSingleDamageType(DAMAGE_EXPLOSION) ? 1 : 0, ws.hasWeaponToDealDamageType(DAMAGE_SMALL_ARMS) ? 1 : 0,
				ws.isCurWeaponLocked() ? 1 : 0, (int)ws.getCurWeaponSlot(), ws.isSharedReloadTime() ? 1 : 0, pip ? (int)pip->getWeaponSlot() : -1,
				wp ? (int)wp->getWeaponSlot() : -1, ws.getMostPercentReadyToFireAnyWeapon(), ws.getNthCommandSourceMask(PRIMARY_WEAPON),
				ws.getNthCommandSourceMask(SECONDARY_WEAPON), ws.getNthCommandSourceMask(TERTIARY_WEAPON), ws.getCurWeapon() ? (int)ws.getCurWeapon()->getWeaponSlot() : -1);
			break; }
		case 28: {
			Object* o = wpnFind(r.i());
			XferSave xs; xs.open(AsciiString(tmpPath)); xs.xferSnapshot(&o->m_weaponSet); xs.close();
			std::vector<unsigned char> bytes = readFileBytes(tmpPath);
			Rec("ret setxfer-save len=%u hash=%08x", (unsigned)bytes.size(), hashBytes(bytes));
			WeaponSet copy; unsigned status = 0;
			XferLoad xl;
			try { xl.open(AsciiString(tmpPath)); xl.xferSnapshot(&copy); xl.close(); } catch (XferStatus s) { status = 1u + (unsigned)s; try { xl.close(); } catch (...) {} }
			catch (...) { status = 100; try { xl.close(); } catch (...) {} }
			Rec("ret setxfer-load %u cur=%d lock=%d mask=%u anti=%d dw=%d", status, (int)copy.m_curWeapon, (int)copy.m_curWeaponLockedStatus, copy.m_filledWeaponSlotMask,
				(int)copy.m_totalAntiMask, copy.m_hasDamageWeapon ? 1 : 0);
			for (int k = 0; k < WEAPONSLOT_COUNT; ++k) if (copy.m_weapons[k]) Rec("ret setxfer-w%d %s", k, dumpWeapon(copy.m_weapons[k]).c_str());
			break; }
		case 29: { Object* o = wpnFind(r.i()); unsigned m = r.u(); o->m_wsFlags.clear(); for (int k = 0; k < 17; ++k) if (m & (1u << k)) o->m_wsFlags.set(k); break; }
		case 30: { Object* o = wpnFind(r.i()); o->m_weaponSet.clearLeechRangeModeForAllWeapons(); break; }
		case 31: {
			int slot = r.i(); int cond = r.i();
			ModelConditionFlags f = WeaponSet::getModelConditionForWeaponSlot((WeaponSlotType)slot, (WeaponSetConditionType)cond);
			std::string s; for (int k = 0; k < MODELCONDITION_COUNT; ++k) if (f.test(k)) { sprintf(b, "%d,", k); s += b; }
			Rec("ret modelcond %s", s.c_str());
			break; }
		case 32: { Object* o = wpnFind(r.i()); o->m_bonusCond = r.u(); break; }
		case 33: { Object* o = wpnFind(r.i()); int bit = r.i(); bool on = r.b(); if (on) o->m_statusBits |= (1ull << bit); else o->m_statusBits &= ~(1ull << bit); break; }
		case 34: { Object* o = wpnFind(r.i()); Coord3D p = rdCoord(r); o->m_pos = p; o->m_xform.Set_Translation(Vector3(p.x, p.y, p.z)); break; }
		case 35: { unsigned w = r.u(); reg[w]->deleteInstance(); reg[w] = NULL; break; }
		case 36: {
			/* setClipPercentFull on a weapon owned by an object's weapon set (how a set's weapon gets to be out of ammo) */
			Object* o = wpnFind(r.i()); int slot = r.i(); float p = r.f(); bool a = r.b();
			if (o && o->m_weaponSet.m_weapons[slot]) o->m_weaponSet.m_weapons[slot]->setClipPercentFull(p, a);
			break; }
		case 37: {
			/* the object-level questions (Object.cpp's text) */
			Object* o = wpnFind(r.i()); int dt = r.i(); int slot = r.i();
			WeaponSlotType cs = (WeaponSlotType)-1; Weapon* cur = o->getCurrentWeapon(&cs);
			const Object* co = o; WeaponSlotType ccs = (WeaponSlotType)-1; const Weapon* ccur = co->getCurrentWeapon(&ccs);
			Weapon* wp = o->findWaypointFollowingCapableWeapon();
			Int nt = -7, nf = -7; Bool pip = o->getAmmoPipShowingInfo(nt, nf);
			Rec("ret objq any=%d dmg=%d ready=%u dtype=%d range=%08x oom=%d locked=%d shared=%d mask=%08x lastshot=%u cur=%d,%d ccur=%d,%d wp=%d pip=%d,%d,%d",
				o->hasAnyWeapon() ? 1 : 0, o->hasAnyDamageWeapon() ? 1 : 0, o->getMostPercentReadyToFireAnyWeapon(), o->hasWeaponToDealDamageType((DamageType)dt) ? 1 : 0,
				FB(o->getLargestWeaponRange()), o->isOutOfAmmo() ? 1 : 0, o->isCurWeaponLocked() ? 1 : 0, o->isReloadTimeShared() ? 1 : 0,
				o->getWeaponInWeaponSlotCommandSourceMask((WeaponSlotType)slot), o->getLastShotFiredFrame(),
				cur ? 1 : 0, (int)cs, ccur ? 1 : 0, (int)ccs, wp ? (int)wp->getWeaponSlot() : -1, pip ? 1 : 0, (int)nt, (int)nf);
			break; }
		case 38: { Object* o = wpnFind(r.i()); bool now = r.b(); o->reloadAllAmmo(now); break; }
		case 39: { Object* o = wpnFind(r.i()); o->setFiringConditionForCurrentWeapon(); break; }
		case 40: { Object* o = wpnFind(r.i()); o->adjustModelConditionForWeaponStatus(); break; }
		case 41: {
			Object* o = wpnFind(r.i()); Object* v = wpnFind(r.i()); Coord3D pos = rdCoord(r); bool hasPos = r.b(); bool atPos = r.b();
			if (atPos) o->fireCurrentWeapon(hasPos ? &pos : NULL); else o->fireCurrentWeapon(v);
			break; }
		case 42: { Object* o = wpnFind(r.i()); Object* v = wpnFind(r.i()); o->preFireCurrentWeapon(v); break; }
		case 43: {
			Object* o = wpnFind(r.i()); Object* v = wpnFind(r.i()); bool hasPos = r.b(); Coord3D pos = rdCoord(r); int at = r.i(); int cmd = r.i(); int slot = r.i(); int crit = r.i(); int kind = r.u();
			if (kind == 0) Rec("ret oable %d", (int)o->getAbleToAttackSpecificObject((AbleToAttackType)at, v, (CommandSourceType)cmd, (WeaponSlotType)slot));
			else if (kind == 1) Rec("ret oableuse %d", (int)o->getAbleToUseWeaponAgainstTarget((AbleToAttackType)at, v, hasPos ? &pos : NULL, (CommandSourceType)cmd, (WeaponSlotType)slot));
			else Rec("ret ochoose %d", o->chooseBestWeaponForTarget(v, (WeaponChoiceCriteria)crit, (CommandSourceType)cmd) ? 1 : 0);
			break; }
		case 44: {
			Object* o = wpnFind(r.i()); int what = r.i(); int slot = r.i(); int type = r.i();
			if (what == 0) Rec("ret olock %d", o->setWeaponLock((WeaponSlotType)slot, (WeaponLockType)type) ? 1 : 0);
			else if (what == 1) o->releaseWeaponLock((WeaponLockType)type);
			else o->clearLeechRangeModeForAllWeapons();
			break; }
		case 45: { Object* o = wpnFind(r.i()); int slot = r.i(); int victim = r.i(); Weapon* w = o->getWeaponInWeaponSlot((WeaponSlotType)slot); if (w) o->notifyFiringTrackerShotFired(w, (ObjectID)victim); break; }
		case 46: {
			Object* o = wpnFind(r.i()); bool wantPos = r.b(); bool wantTr = r.b(); unsigned kind = r.u(); int turret = r.i(); int nameIdx = r.i(); int maxBones = r.i(); bool toWorld = r.b();
			static const char* NAMES[4] = { "Bone", "Muzzle", "Turret01", "FXBone" };
			const char* name = NAMES[nameIdx & 3];
			Coord3D pos; pos.x = pos.y = pos.z = -99.0f; Matrix3D tr(true); tr.Set_Translation(Vector3(-77.0f, -77.0f, -77.0f));
			if (kind == 0 || kind == 1)
			{
				Bool found = kind == 0 ? o->getSingleLogicalBonePosition(name, wantPos ? &pos : NULL, wantTr ? &tr : NULL)
				                       : o->getSingleLogicalBonePositionOnTurret((WhichTurretType)turret, name, wantPos ? &pos : NULL, wantTr ? &tr : NULL);
				Rec("ret bone%u found=%d pos=%08x,%08x,%08x tr=%s", kind, found ? 1 : 0, FB(pos.x), FB(pos.y), FB(pos.z), mtxStr(tr).c_str());
			}
			else
			{
				Coord3D ps[4]; Matrix3D ts[4];
				for (int i = 0; i < 4; ++i) { ps[i].x = ps[i].y = ps[i].z = -99.0f; ts[i].Make_Identity(); ts[i].Set_Translation(Vector3(-77.0f, -77.0f, -77.0f)); }
				Int n = o->getMultiLogicalBonePosition(name, maxBones, wantPos ? ps : NULL, wantTr ? ts : NULL, toWorld);
				std::string s;
				for (int i = 0; i < 4; ++i) { char t[160]; snprintf(t, sizeof t, "[%08x,%08x,%08x %s]", FB(ps[i].x), FB(ps[i].y), FB(ps[i].z), mtxStr(ts[i]).c_str()); s += t; }
				Rec("ret bonemulti n=%d %s", (int)n, s.c_str());
			}
			break; }
		case 47: {
			Object* o = wpnFind(r.i()); unsigned up = r.u();
			Rec("ret cmdset %s", o->getCommandSetString().str());
			Rec("ret canproduce %d", o->canProduceUpgrade((const UpgradeTemplate*)(size_t)up) ? 1 : 0);
			break; }
		default: abort();
		}
		dumpSnapshot(reg);
	}
	for (size_t i = 0; i < reg.size(); ++i) if (reg[i]) reg[i]->deleteInstance();
	return 0;
}

#endif
