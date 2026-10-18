/* The stand-ins of the units Object.cpp calls into (p1w6-object), part 3: the bodies, which need the
** complete Object. Every stand-in that has an effect writes the record the Rust side writes. */
#ifndef SHIM_OBJECT_STANDINS3_H
#define SHIM_OBJECT_STANDINS3_H

static const char* const g_helperNames[8] = { "Smc", "StatusDamage", "SubdualDamage", "Repulsor", "Defection", "WeaponStatus", "FiringTracker", "TempWeaponBonus" };
static const char* const g_soundNames[1] = { "" };

Rec& Rec::coord(const char* k, const Coord3D* c, bool opt)
{
	s += ' '; s += k;
	if (opt) s += c ? " Some" : " None";
	if (c)
	{
		char b[160];
		snprintf(b, sizeof b, " Coord3D x f:%08x y f:%08x z f:%08x", fbits(c->x), fbits(c->y), fbits(c->z));
		s += b;
	}
	return *this;
}

/* ---- BehaviorModule ---------------------------------------------------------------------------------------- */
void BehaviorModule::onDisabledEdge(Bool b) const { probe("OnDisabledEdge"); }
void BehaviorModule::onCapture(Player* o, Player* n) const { probe("OnCapture"); }
void BehaviorModule::onDie(DamageInfo* i) const { probe("OnDie"); }
void BehaviorModule::onCollide(Object* other, const Coord3D* loc, const Coord3D* normal) const
{
	if (!probe("OnCollide")) return;
	running = true;
	if (a.collideSetsNoCollisions)
		m_object->setStatus(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_NO_COLLISIONS));
	running = false;
}
void BehaviorModule::attemptDamage(DamageInfo* info) const
{
	if (!probe("AttemptDamage")) return;
	running = true;
	/* ActiveBody in miniature: the damage taken, then death once (module_probe.rs) */
	Real dealt;
	if (info->in.m_kill)
		dealt = health;
	else
	{
		Real amount = info->in.m_amount * damageScalar;
		dealt = amount < health ? amount : health;
	}
	health -= dealt;
	info->out.m_actualDamageDealt = dealt;
	info->out.m_actualDamageClipped = dealt;
	info->out.m_noEffect = false;
	lastDamageSource = info->in.m_sourceID;
	if (health <= 0.0f && !died)
	{
		died = true;
		m_object->onDie(info);
	}
	running = false;
}
void BehaviorModule::attemptHealing(DamageInfo* info) const
{
	if (!probe("AttemptHealing")) return;
	Real room = maxHealth - health;
	Real healed = (info->in.m_damageType == DAMAGE_HEALING && info->in.m_amount < room) ? info->in.m_amount : room;
	health += healed;
	info->out.m_actualDamageDealt = healed;
}
const ContainedItemsList* BehaviorModule::getContainedItemsList() const
{
	static ContainedItemsList items;
	items.clear();
	if (!probe("GetContainedItems")) return &items;
	for (size_t i = 0; i < a.contained.size(); ++i)
	{
		Object* o = TheGameLogic->findObjectByID((ObjectID)a.contained[i]);
		if (o) items.push_back(o);
	}
	return &items;
}
const Object* BehaviorModule::friend_getRider() const
{
	if (!probe("GetRider")) return NULL;
	return TheGameLogic->findObjectByID((ObjectID)a.rider);
}
void BehaviorModule::iterateContained(void (*func)(Object*, void*), void* userData, Bool reverse) const
{
	/* the only caller is localIsHero: a hero among the contained objects sets the flag */
	if (!probe("IsHeroInside")) return;
	if (a.heroInside) *(Bool*)userData = TRUE;
}
Player* BehaviorModule::getApparentControllingPlayer(const Player* observer) const
{
	if (!probe("GetApparentControllingPlayer")) return NULL;
	return ThePlayerList->getNthPlayer(a.apparentPlayer);
}
void BehaviorModule::defectAllParkedUnits(Team* t, UnsignedInt d) const { probe("DefectAllParkedUnits"); }
void BehaviorModule::sleepUntil(UnsignedInt when) const
{
	/* ObjectHelper::sleepUntil: nothing for a destroyed object */
	if (m_object->isDestroyed()) return;
	CUT("HelperSleepUntil").i("object", m_object->getID()).e("kind", g_helperNames[m_helperKind]).i("when", when);
}
SpecialPowerType SpecialPowerTemplate::getSpecialPowerType() const
{
	return (SpecialPowerType)m_owner->a.specialPowerType; /* the call was recorded by getSpecialPowerTemplate */
}
Bool SpecialPowerTemplate::isShortcutPower() const
{
	BehaviorModule* m = m_owner;
	if (!m->probe("IsShortcutPower")) return false;
	return m->a.shortcut;
}

/* AIUpdateInterface */
void BehaviorModule::setAttitude(Int a_) const { CUT("AiSetAttitude").i("object", m_object->getID()).i("team", a_); }
void BehaviorModule::setAttackInfo(const AttackPriorityInfo* i) const { CUT("AiSetAttackInfo").i("object", m_object->getID()).i("team", atoi(i->m_name.str())); }
void BehaviorModule::aiIdle(CommandSourceType c) const { CUT("AiIdle").i("object", m_object->getID()); }
void BehaviorModule::transferAttack(ObjectID o, ObjectID n) const { CUT("AiTransferAttack").i("object", m_object->getID()).i("old_victim", o).i("new_victim", n); }
void BehaviorModule::getTurretRotAndPitch(WhichTurretType t, Real* rot, Real* pitch) const
{
	std::map<std::pair<int, int>, float>::iterator it = g_cfg.turretRotation.find(std::make_pair((int)m_object->getID(), (int)t));
	*rot = it == g_cfg.turretRotation.end() ? 0.0f : it->second;
	*pitch = 0.0f;
}
WhichTurretType BehaviorModule::getWhichTurretForWeaponSlot(WeaponSlotType s, Real* dummy) const
{
	std::map<std::pair<int, int>, int>::iterator it = g_cfg.aiTurrets.find(std::make_pair((int)m_object->getID(), (int)s));
	return it == g_cfg.aiTurrets.end() ? TURRET_INVALID : (WhichTurretType)it->second;
}
Bool BehaviorModule::isTurretEnabled(WhichTurretType t) const { return g_cfg.turretsDisabled.count(std::make_pair((int)m_object->getID(), (int)t)) == 0; }
BehaviorModule* BehaviorModule::getCurLocomotor() const { return self(); }
BehaviorModule* BehaviorModule::getDozerAIInterface() const { return g_cfg.dozerAIs.count((int)m_object->getID()) ? self() : NULL; }
void BehaviorModule::cancelTask(Int t) const { CUT("DozerCancelTask").i("object", m_object->getID()); }
UnsignedInt BehaviorModule::getLegalSurfaces() const { return g_cfg.aiAirborne.count((int)m_object->getID()) ? LOCOMOTORSURFACE_AIR : 1; }

/* ---- teams and players -------------------------------------------------------------------------------------- */
Player* Team::getControllingPlayer() const { return ThePlayerList->getNthPlayer(m_player); }
Relationship Team::getRelationship(const Team* that) const
{
	if (!that) return NEUTRAL;
	std::map<std::pair<int, int>, int>::iterator it = g_cfg.teamRel.find(std::make_pair((int)m_id, (int)that->m_id));
	return it == g_cfg.teamRel.end() ? NEUTRAL : (Relationship)it->second;
}
Bool Team::isInList_TeamMemberList(const Object* o) const { return std::find(m_members.begin(), m_members.end(), o) != m_members.end(); }
void Team::removeFrom_TeamMemberList(Object* o) { m_members.remove(o); }
void Team::prependTo_TeamMemberList(Object* o) { m_members.push_front(o); }
void Team::notifyTeamOfObjectDeath() { CUT("TeamNotifyTeamOfObjectDeath").i("team", m_id); }
void Team::setEnteredExited() { CUT("TeamSetEnteredExited").i("team", m_id); }
void ScoreKeeper::addObjectCaptured(const Object* o) { CUT("PlayerScoreAddObjectCaptured").i("player", m_p->m_index).i("object", o->getID()); }
void ScoreKeeper::addObjectLost(const Object* o) { CUT("PlayerScoreAddObjectLost").i("player", m_p->m_index).i("object", o->getID()); }
void ScoreKeeper::addObjectDestroyed(const Object* o) { CUT("PlayerScoreAddObjectDestroyed").i("player", m_p->m_index).i("object", o->getID()); }
void Energy::objectEnteringInfluence(Object* o) { CUT("EnergyObjectEnteringInfluence").i("player", m_p->m_index).i("object", o->getID()); }
void Energy::objectLeavingInfluence(Object* o) { CUT("EnergyObjectLeavingInfluence").i("player", m_p->m_index).i("object", o->getID()); }
void Energy::adjustPower(Int amount, Bool add) { CUT("EnergyAdjustPower").i("player", m_p->m_index).i("amount", amount).b("add", add); }
void AcademyStats::recordMine() { CUT("AcademyStatsRecordMine"); }
static const ObjCfgPlayer& cfgPlayer(Int i) { return g_cfg.players[i]; }
PlayerMaskType Player::getPlayerMask() const { return (PlayerMaskType)(1 << m_index); }
Color Player::getPlayerColor() const { return cfgPlayer(m_index).color; }
Color Player::getPlayerNightColor() const { return cfgPlayer(m_index).nightColor; }
Bool Player::isPlayerActive() const { return cfgPlayer(m_index).active; }
Bool Player::isPlayableSide() const { return cfgPlayer(m_index).playable; }
Bool Player::isSkirmishAIPlayer() { return cfgPlayer(m_index).skirmish; }
Team* Player::getDefaultTeam() { return TheTeamFactory->findTeamByID((TeamID)cfgPlayer(m_index).defaultTeam); }
const Team* Player::getDefaultTeam() const { return TheTeamFactory->findTeamByID((TeamID)cfgPlayer(m_index).defaultTeam); }
Relationship Player::getRelationship(const Team* that) const
{
	if (!that) return NEUTRAL;
	std::map<std::pair<int, int>, int>::iterator it = g_cfg.playerRel.find(std::make_pair((int)m_index, (int)that->m_id));
	return it == g_cfg.playerRel.end() ? NEUTRAL : (Relationship)it->second;
}
UpgradeMaskType Player::getCompletedUpgradeMask() const
{
	UpgradeMaskType m;
	const ObjCfgPlayer& p = cfgPlayer(m_index);
	for (int i = 0; i < 64; ++i)
	{
		if ((p.upgradesLo >> i) & 1) m.set(i, 1);
		if ((p.upgradesHi >> i) & 1) m.set(64 + i, 1);
	}
	return m;
}
Int Player::getNumBattlePlansActive() const { return cfgPlayer(m_index).battlePlans; }
VeterancyLevel Player::getProductionVeterancyLevel(AsciiString) const { return (VeterancyLevel)cfgPlayer(m_index).productionLevel; }
void Player::becomingTeamMember(Object* o, Bool incoming) { CUT("PlayerBecomingTeamMember").i("player", m_index).i("object", o->getID()).b("incoming", incoming); }
void Player::applyBattlePlanBonusesForObject(Object* o) const { CUT("PlayerApplyBattlePlanBonusesForObject").i("player", m_index).i("object", o->getID()); }
void Player::friend_applyDifficultyBonusesForObject(Object* o, Bool apply) { CUT("PlayerApplyDifficultyBonusesForObject").i("player", m_index).i("object", o->getID()).b("apply", apply); }
void Player::addSkillPointsForKill(Object* killer, const Object* victim) { CUT("PlayerAddSkillPointsForKill").i("player", m_index).i("killer", killer->getID()).i("victim", victim->getID()); }
void Player::doBountyForKill(Object* killer, const Object* victim) { CUT("PlayerDoBountyForKill").i("player", m_index).i("killer", killer->getID()).i("victim", victim->getID()); }
void Player::removeRadar(Bool p) { CUT("PlayerRemoveRadar").i("player", m_index).b("disable_proof", p); }
void Player::addRadar(Bool p) { CUT("PlayerAddRadar").i("player", m_index).b("disable_proof", p); }
AcademyStats* Player::getAcademyStats() { static AcademyStats s; return &s; }
PlayerMaskType PlayerList::getPlayersWithRelationship(Int srcPlayerIndex, UnsignedInt allowedRelationships)
{
	PlayerMaskType retVal = 0;
	if (allowedRelationships == 0) return retVal;
	Player* srcPlayer = getNthPlayer(srcPlayerIndex);
	if (!srcPlayer) return retVal;
	if (BitTest(allowedRelationships, ALLOW_SAME_PLAYER)) BitSet(retVal, srcPlayer->getPlayerMask());
	for (Int i = 0; i < getPlayerCount(); ++i)
	{
		Player* player = getNthPlayer(i);
		if (!player) continue;
		if (player == srcPlayer) continue;
		switch (srcPlayer->getRelationship(player->getDefaultTeam()))
		{
			case ENEMIES: if (BitTest(allowedRelationships, ALLOW_ENEMIES)) BitSet(retVal, player->getPlayerMask()); break;
			case ALLIES: if (BitTest(allowedRelationships, ALLOW_ALLIES)) BitSet(retVal, player->getPlayerMask()); break;
			case NEUTRAL: if (BitTest(allowedRelationships, ALLOW_NEUTRAL)) BitSet(retVal, player->getPlayerMask()); break;
		}
	}
	return retVal;
}

/* ---- the client ----------------------------------------------------------------------------------------------- */
void AudioManager::addAudioEvent(const AudioEventRTS* e)
{
	EVT("Sound").e("sound", e->m_name).i("object_id", e->m_objectID).coord("position", e->m_hasPos ? &e->m_pos : NULL);
}
void Drawable::setID(DrawableID id) { m_id = id; EVT("DrawableSetId").i("object_id", m_obj->getID()).i("drawable_id", id); }
void Drawable::changedTeam() { EVT("DrawableChangedTeam").i("object_id", m_obj->getID()); }
void Drawable::reactToGeometryChange() { EVT("DrawableReactToGeometryChange").i("object_id", m_obj->getID()); }
void Drawable::setSelectable(Bool s) { EVT("DrawableSetSelectable").i("object_id", m_obj->getID()).b("selectable", s); }
void Drawable::setTintStatus(TintStatus s) { EVT("DrawableTint").i("object_id", m_obj->getID()).i("status", s).b("on", true); }
void Drawable::clearTintStatus(TintStatus s) { EVT("DrawableTint").i("object_id", m_obj->getID()).i("status", s).b("on", false); }
static std::string mcOp(const char* name, const char* k, const std::vector<int>& v)
{
	std::string s = name; s += ' '; s += k;
	for (size_t i = 0; i < v.size(); ++i) { char b[32]; snprintf(b, sizeof b, " %d", v[i]); s += b; }
	return s;
}
static std::vector<int> bitsOf(const ModelConditionFlags& f) { std::vector<int> v; for (int i = 0; i < f.size(); ++i) if (f.test(i)) v.push_back(i); return v; }
void Drawable::setModelConditionState(ModelConditionFlagType a_) { EVT("DrawableModelCondition").i("object_id", m_obj->getID()).e("op", "Set").ints(NULL, std::vector<int>(1, (int)a_)); }
void Drawable::clearModelConditionState(ModelConditionFlagType a_) { EVT("DrawableModelCondition").i("object_id", m_obj->getID()).e("op", "Clear").ints(NULL, std::vector<int>(1, (int)a_)); }
void Drawable::clearAndSetModelConditionState(ModelConditionFlagType c, ModelConditionFlagType s_)
{
	EVT("DrawableModelCondition").i("object_id", m_obj->getID()).e("op", "ClearAndSet").ints("clear", std::vector<int>(1, (int)c)).ints("set", std::vector<int>(1, (int)s_));
}
void Drawable::setModelConditionFlags(const ModelConditionFlags& f) { EVT("DrawableModelCondition").i("object_id", m_obj->getID()).e("op", "Set").ints(NULL, bitsOf(f)); }
void Drawable::clearModelConditionFlags(const ModelConditionFlags& f) { EVT("DrawableModelCondition").i("object_id", m_obj->getID()).e("op", "Clear").ints(NULL, bitsOf(f)); }
void Drawable::clearAndSetModelConditionFlags(const ModelConditionFlags& c, const ModelConditionFlags& s_)
{
	EVT("DrawableModelCondition").i("object_id", m_obj->getID()).e("op", "ClearAndSet").ints("clear", bitsOf(c)).ints("set", bitsOf(s_));
}
void Drawable::setTerrainDecalFadeTarget(Real target, Real rate) { EVT("DrawableTerrainDecalFadeTarget").i("object_id", m_obj->getID()).f("target", target).f("rate", rate); }
void Drawable::flashAsSelected() { EVT("DrawableFlashAsSelected").i("object_id", m_obj->getID()); }
void Radar::addObject(Object* o)
{
	RadarPriorityType p = o->getRadarPriority();
	if (p == RADAR_PRIORITY_INVALID || p == RADAR_PRIORITY_NOT_ON_RADAR) return; /* isPriorityVisible */
	o->friend_setRadarData(new RadarObject());
	EVT("Radar").e("call", "AddObject").i("object_id", o->getID()).coord("position", NULL);
}
void Radar::removeObject(Object* o)
{
	RadarObject* r = o->friend_getRadarData();
	if (!r) return;
	o->friend_setRadarData(NULL);
	delete r;
	EVT("Radar").e("call", "RemoveObject").i("object_id", o->getID()).coord("position", NULL);
}
void Radar::tryUnderAttackEvent(const Object* o) { EVT("Radar").e("call", "TryUnderAttackEvent").i("object_id", o->getID()).coord("position", NULL); }
void Radar::tryInfiltrationEvent(const Object* o) { EVT("Radar").e("call", "TryInfiltrationEvent").i("object_id", o->getID()).coord("position", NULL); }
void Radar::tryEvent(RadarEventType t, const Coord3D* p) { EVT("Radar").e("call", "TryFakeEvent").i("object_id", INVALID_ID).coord("position", p); }
void Eva::setShouldPlay(EvaMessage m) { EVT("Eva").e("message", m == EVA_BuldingLost ? "BuildingLost" : "UnitLost"); }
void InGameUI::objectChangedTeam(const Object* o, Int oldP, Int newP) { EVT("ObjectChangedTeam").i("object_id", o->getID()).i("old_player", oldP).i("new_player", newP); }
void InGameUI::removeIdleWorker(Object* o, Int p) { EVT("RemoveIdleWorker").i("object_id", o->getID()).i("player_index", p); }
void InGameUI::addWorldAnimation(const Object* o, Anim2DTemplate* t, const Coord3D* pos, Int options, Real displayTime, Real zRise) { EVT("LevelGainAnimation").i("object_id", o->getID()).coord("position", pos, false); }
void ControlBar::markUIDirty() { EVT("ControlBarMarkUiDirty"); }
void GameClient::notifyTerrainObjectMoved(Object* o) { EVT("TerrainObjectMoved").i("object_id", o->getID()); }
void BuildAssistant::sellObject(Object* o) { CUT("BuildAssistantSellObject").i("object", o->getID()); }
Bool SpecialPowerStore::canUseSpecialPower(const Object* o, const SpecialPowerTemplate* t) { return g_cfg.specialPowerDenied.count((int)o->getID()) == 0; }
const UpgradeTemplate* UpgradeCenter::findVeterancyUpgrade(VeterancyLevel l)
{
	if (g_cfg.veterancyUpgrade[l] == 0) return NULL;
	templates[l].m_mask.clear();
	templates[l].m_mask.set(g_cfg.veterancyUpgrade[l] - 1, 1);
	return &templates[l];
}
void ScriptEngine::notifyOfObjectCreationOrDestruction() { CUT("ScriptEngineNotifyOfObjectCreationOrDestruction"); }
extern ObjectID g_deletingId;
/* ~Object resets m_id first: the record names the id the object had */
void ScriptEngine::notifyOfObjectDestruction(Object* o) { CUT("ScriptEngineNotifyOfObjectDestruction").i("object", g_deletingId); }
void ScriptEngine::AppendDebugMessage(const char* s, Bool b) { CUT("ScriptEngineAppendDebugMessage").b("too_many_trigger_areas", true); }
const AttackPriorityInfo* ScriptEngine::getAttackInfo(AsciiString name)
{
	static AttackPriorityInfo info;
	info.m_name = name;
	return &info;
}

/* ---- polygon triggers, terrain, pathfinder ----------------------------------------------------------------------- */
PolygonTrigger* PolygonTrigger::s_first = NULL;
void TerrainLogic::getExtent(Region3D* r)
{
	r->lo.x = 0; r->lo.y = 0; r->lo.z = 0; r->hi.x = g_cfg.extentHiX; r->hi.y = g_cfg.extentHiY; r->hi.z = 0;
}
Real TerrainLogic::getLayerHeight(Real x, Real y, PathfindLayerEnum layer) { return layer == LAYER_WALL ? g_cfg.wallHeight : 0.0f; }
void TerrainLogic::alignOnTerrain(Real angle, const Coord3D& pos, Bool stick, Matrix3D& mtx) { /* flat terrain: not reached by the programs */ mtx.Make_Identity(); }
PolygonTrigger* TerrainLogic::getTriggerAreaByName(AsciiString name)
{
	for (PolygonTrigger* t = PolygonTrigger::s_first; t; t = t->m_next)
		if (t->m_name == name) return t;
	return NULL;
}
void Pathfinder::updatePos(Object* o, const Coord3D* p) { CUT("PathfinderUpdatePos").i("object", o->getID()); }
void Pathfinder::removePos(Object* o) { CUT("PathfinderRemovePos").i("object", o->getID()); }
void Pathfinder::addObjectToPathfindMap(Object* o) { CUT("PathfinderAddObjectToPathfindMap").i("object", o->getID()); }
void Pathfinder::removeObjectFromPathfindMap(Object* o) { CUT("PathfinderRemoveObjectFromPathfindMap").i("object", o->getID()); }
void Pathfinder::removeWallPiece(Object* o) { CUT("PathfinderRemoveWallPiece").i("object", o->getID()); }

/* ---- the partition manager -------------------------------------------------------------------------------------------- */
void PartitionData::makeDirty(Bool b) { CUT("PartitionMakeDirty").i("object", m_obj->getID()); }
ObjectShroudStatus PartitionData::getShroudedStatus(Int playerIndex) const
{
	std::map<std::pair<int, int>, int>::iterator it = g_cfg.shroudedStatus.find(std::make_pair((int)m_obj->getID(), (int)playerIndex));
	return it == g_cfg.shroudedStatus.end() ? OBJECTSHROUD_CLEAR : (ObjectShroudStatus)it->second;
}
void PartitionManager::registerObject(Object* o)
{
	PartitionData* pd = new PartitionData();
	pd->m_obj = o;
	o->friend_setPartitionData(pd);
	CUT("PartitionRegisterObject").i("object", o->getID());
}
void PartitionManager::unRegisterObject(Object* o)
{
	PartitionData* pd = o->friend_getPartitionData();
	o->friend_setPartitionData(NULL);
	delete pd;
	CUT("PartitionUnregisterObject").i("object", o->getID());
}
#define PMREC(name, ...) CUT(name).f("x", x).f("y", y).f("range", r).i("mask", m)
void PartitionManager::doShroudReveal(Real x, Real y, Real r, PlayerMaskType m) { CUT("PartitionDoShroudReveal").f("x", x).f("y", y).f("range", r).i("mask", m); }
void PartitionManager::queueUndoShroudReveal(Real x, Real y, Real r, PlayerMaskType m) { CUT("PartitionQueueUndoShroudReveal").f("x", x).f("y", y).f("range", r).i("mask", m); }
void PartitionManager::doShroudCover(Real x, Real y, Real r, PlayerMaskType m) { CUT("PartitionDoShroudCover").f("x", x).f("y", y).f("range", r).i("mask", m); }
void PartitionManager::undoShroudCover(Real x, Real y, Real r, PlayerMaskType m) { CUT("PartitionUndoShroudCover").f("x", x).f("y", y).f("range", r).i("mask", m); }
void PartitionManager::doValueAffect(Real x, Real y, Real r, UnsignedInt d, PlayerMaskType m) { CUT("PartitionDoValueAffect").f("x", x).f("y", y).f("how_far", r).i("data", d).i("for_whom", m); }
void PartitionManager::undoValueAffect(Real x, Real y, Real r, UnsignedInt d, PlayerMaskType m) { CUT("PartitionUndoValueAffect").f("x", x).f("y", y).f("how_far", r).i("data", d).i("for_whom", m); }
void PartitionManager::doThreatAffect(Real x, Real y, Real r, UnsignedInt d, PlayerMaskType m) { CUT("PartitionDoThreatAffect").f("x", x).f("y", y).f("how_far", r).i("data", d).i("for_whom", m); }
void PartitionManager::undoThreatAffect(Real x, Real y, Real r, UnsignedInt d, PlayerMaskType m) { CUT("PartitionUndoThreatAffect").f("x", x).f("y", y).f("how_far", r).i("data", d).i("for_whom", m); }
ObjectIterator* PartitionManager::iteratePotentialCollisions(const Coord3D* pos, const GeometryInfo& geom, Real angle)
{
	ObjectIterator* it = new ObjectIterator();
	for (size_t i = 0; i < g_cfg.potentialCollisions.size(); ++i)
	{
		Object* o = TheGameLogic->findObjectByID((ObjectID)g_cfg.potentialCollisions[i]);
		if (o) it->v.push_back(o);
	}
	return it;
}
ObjectIterator* PartitionManager::iterateObjectsInRange(const Coord3D* pos, Real maxDist, Int distType, PartitionFilter** filters, Int order)
{
	ObjectIterator* it = new ObjectIterator();
	for (size_t i = 0; i < g_cfg.boobyTrapsInRange.size(); ++i)
	{
		Object* o = TheGameLogic->findObjectByID((ObjectID)g_cfg.boobyTrapsInRange[i]);
		if (o) it->v.push_back(o);
	}
	return it;
}

/* ---- weapons, AI groups, module factory ----------------------------------------------------------------------------------- */
static Object* weaponSetOwner(const WeaponSet* ws)
{
	return (Object*)((char*)ws - ((size_t) & ((Object*)0x1000)->m_weaponSet - 0x1000));
}
Weapon* WeaponSet::getWeaponInWeaponSlot(WeaponSlotType s) const
{
	static Weapon w;
	return g_cfg.weapons.count(std::make_pair((int)weaponSetOwner(this)->getID(), (int)s)) ? &w : NULL;
}
Bool WeaponSet::hasAnyWeapon() const { return g_cfg.weaponSetsWithWeapons.count((int)weaponSetOwner(this)->getID()) != 0; }
void WeaponSet::updateWeaponSet(const Object* o) { CUT("WeaponSetUpdate").i("object", o->getID()); }
void WeaponSet::weaponSetOnWeaponBonusChange(Object* o) { CUT("WeaponSetOnWeaponBonusChange").i("object", o->getID()); }
void WeaponSet::xfer(Xfer* x)
{
	/* WeaponSet::xfer (version 1) with no weapon in any slot, as WeaponSetRecord::xfer */
	XferVersion cur = 1, v = 1;
	x->xferVersion(&v, cur);
	AsciiString name;
	x->xferAsciiString(&name);
	WeaponSetFlags flags;
	flags.xfer(x);
	for (int i = 0; i < WEAPONSLOT_COUNT; ++i) { Bool has = false; x->xferBool(&has); }
	Int curWeapon = 0, locked = 0;
	x->xferUser(&curWeapon, sizeof(Int));
	x->xferUser(&locked, sizeof(Int));
	UnsignedInt filled = 0; x->xferUnsignedInt(&filled);
	Int anti = 0; x->xferInt(&anti);
	Bool hasDamage = false; x->xferBool(&hasDamage);
	x->xferBool(&hasDamage);
	BitFlags<64, -1> damageTypes;
	damageTypes.xfer(x);
}
void AIGroup::remove(Object* o) { CUT("AiGroupRemove").i("group", m_id).i("object", o->getID()); }

BehaviorModule* RebuildHoleBehavior::getRebuildHoleBehaviorInterfaceFromObject(Object* o) { return (BehaviorModule*)o->findModule(NAMEKEY("RebuildHoleBehavior")); }

/* ---- the game logic ------------------------------------------------------------------------------------------------------------ */
void GameLogic::addObjectToLookupTable(Object* o) { m_table[(int)o->getID()] = o; }
void GameLogic::removeObjectFromLookupTable(Object* o) { m_table.erase((int)o->getID()); }
Object* GameLogic::findObjectByID(ObjectID id)
{
	if (id == INVALID_ID) return NULL;
	std::map<int, Object*>::iterator it = m_table.find((int)id);
	return it == m_table.end() ? NULL : it->second;
}
void GameLogic::registerObject(Object* o) { o->prependToList(&m_objList); }
void GameLogic::sendObjectCreated(Object* o)
{
	EVT("ObjectCreated").i("template_id", o->getTemplate()->getTemplateID()).i("drawable_status_bits", 0).i("object_id", o->getID());
	Drawable* d = new Drawable();
	d->m_id = (DrawableID)m_nextDrawable++;
	d->m_obj = o;
	m_drawables[(int)o->getID()] = d;
	o->friend_bindToDrawable(d);
}
void GameLogic::sendObjectDestroyed(Object* o)
{
	EVT("ObjectDestroyed").i("object_id", o->getID());
	std::map<int, Drawable*>::iterator it = m_drawables.find((int)o->getID());
	if (it != m_drawables.end()) { delete it->second; m_drawables.erase(it); }
	o->friend_bindToDrawable(NULL);
}
void GameLogic::updateObjectsChangedTriggerAreas() { CUT("GameLogicUpdateObjectsChangedTriggerAreas"); }
void GameLogic::deselectObject(const Object* o, PlayerMaskType mask, Bool affectClient) { CUT("GameLogicDeselectObject").i("object", o->getID()).i("player_mask", mask).b("affect_client", affectClient); }

#endif
