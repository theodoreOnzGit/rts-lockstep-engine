/* Differential-test driver for ProductionUpdate (p1w13-sonnet, batch R76; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the factory (id 1: a team of three, each
** with its player; sold, under construction or not; maybe a drawable, whose ACTIVELY_CONSTRUCTING flag is the test's and whose model
** conditions are batch R43's lines; selected or not; maybe an exit module whose door is the test's and whose calls are lines; maybe a parking
** place behaviour whose two answers are the test's, lines; an "upgrade module" that would act on the test's upgrades, once), the players
** (their money, the test's count, withdraw and deposit lines; their upgrades two masks, addUpgrade and removeUpgrade lines that keep them;
** their build permissions; the score keeper, the academy and onUnitCreated lines), the thing templates (UnitA, UnitB, Dozer (KINDOF_DOZER),
** Tower (KINDOF_STRUCTURE): their cost and build time the test's; newObject makes ids 6, 7, ... on the team; their sounds), the build
** assistant's canMakeUnit (the test's, a line), the object's canProduceUpgrade (the test's), the script engine, the in-game UI, the radar, EVA
** and the control bar (lines), the audio (addAudioEvent lines; isValidAudioEvent the test's), the REAL UpgradeCenter (the test's upgrades),
** UpdateModule, INI and Xfer; the game logic (its frame). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionUpdate.cpp"

/* Upgrade.cpp 433-456, verbatim (the module build's Player is not the upgrade build's: this unit's own copy) */
Bool UpgradeCenter::canAffordUpgrade( Player *player, const UpgradeTemplate *upgradeTemplate, Bool displayReason ) const
{

	// sanity
	if( player == NULL || upgradeTemplate == NULL )
		return FALSE;

	// money check
	Money *money = player->getMoney();
	if( money->countMoney() < upgradeTemplate->calcCostToBuild( player ) )
	{
		//Post reason why we can't make upgrade!
		if( displayReason )
		{
			TheInGameUI->message( "GUI:NotEnoughMoneyToUpgrade" );
		}
		return FALSE;
	}

	/// @todo maybe have prereq checks for upgrades???

	return TRUE;  // all is well

}  // end canAffordUpgrade

/* Upgrade.cpp 157-180, verbatim (the same) */
Int UpgradeTemplate::calcTimeToBuild( Player *player ) const
{
#if defined(_DEBUG) || defined(_INTERNAL) || defined(_ALLOW_DEBUG_CHEATS_IN_RELEASE)
	if( player->buildsInstantly() )
	{
		return 1;
	}
#endif

	///@todo modify this by power state of player
	return m_buildTime * LOGICFRAMES_PER_SECOND;

}  // end calcTimeToBuild

//-------------------------------------------------------------------------------------------------
/** Calculate the cost takes this player to build this upgrade */
//-------------------------------------------------------------------------------------------------
Int UpgradeTemplate::calcCostToBuild( Player *player ) const
{

	///@todo modify this by any player handicaps
	return m_cost;

}  // end calcCostToBuild

/* Player.cpp's allowedToBuild, verbatim */
Bool Player::allowedToBuild(const ThingTemplate *tmplate) const
{
	if (!m_canBuildBase && tmplate->isKindOf(KINDOF_STRUCTURE)) {
		return false;
	}
	if (!m_canBuildUnits && !tmplate->isKindOf(KINDOF_STRUCTURE)) {
		return false;
	}
	return true;
}
/* the upgrade list as its two masks (Player.cpp's addUpgrade and removeUpgrade keep them so); the calls are lines */
void Player::addUpgrade( const UpgradeTemplate *upgradeTemplate, UpgradeStatusType status )
{
	ulog("addupgrade %d %s %d", (int)m_idx, upgradeTemplate->getUpgradeName().str(), (int)status);
	UpgradeMaskType newMask = upgradeTemplate->getUpgradeMask();
	if( status == UPGRADE_STATUS_IN_PRODUCTION )
		m_upgradesInProduction.set( newMask );
	else if( status == UPGRADE_STATUS_COMPLETE )
	{
		m_upgradesInProduction.clear( newMask );
		m_upgradesCompleted.set( newMask );
	}
}
void Player::removeUpgrade( const UpgradeTemplate *upgradeTemplate )
{
	ulog("removeupgrade %d %s", (int)m_idx, upgradeTemplate->getUpgradeName().str());
	UpgradeMaskType oldMask = upgradeTemplate->getUpgradeMask();
	if( m_upgradesInProduction.testForAll( oldMask ) || m_upgradesCompleted.testForAll( oldMask ) )
	{
		m_upgradesInProduction.clear( oldMask );
		m_upgradesCompleted.clear( oldMask );
	}
}
void ScoreKeeper::addMoneySpent(Int money) { ulog("spent %d %d", (int)m_idx, (int)money); }
void PowerAcademyStats::recordProduction( const Object *obj, const Object *constructor )
{
	ulog("recordproduction %d %d %d", (int)((Player *)(void *)this)->m_idx, (int)obj->m_oid, (int)constructor->m_oid);
}
void PowerAcademyStats::recordUpgrade( const UpgradeTemplate *up, Bool granted )
{
	ulog("recordupgrade %d %s %d", (int)((Player *)(void *)this)->m_idx, up->getUpgradeName().str(), granted ? 1 : 0);
}
void ScriptEngine::notifyOfCompletedUpgrade(Int playerIndex, const AsciiString &upgrade, ObjectID sourceObject)
{
	ulog("scriptupgrade %d %s %d", (int)playerIndex, upgrade.str(), (int)sourceObject);
}
BuildAssistant *TheBuildAssistant = 0;
unsigned g_r76CanMake = 0;
const ThingTemplate *g_r76Tmpl[4];
static int r76TmplIndex(const ThingTemplate *t) { for (int i = 0; i < 4; ++i) if (t == g_r76Tmpl[i]) return i; return -1; }
CanMakeType BuildAssistant::canMakeUnit( Object *builder, const ThingTemplate *whatToBuild ) const
{
	int i = r76TmplIndex(whatToBuild);
	ulog("canmake %d %s", (int)builder->m_oid, whatToBuild ? whatToBuild->getName().str() : "(null)");
	return i < 0 ? CANMAKE_OK : (CanMakeType)(g_r76CanMake >> (3 * i) & 7);
}
unsigned g_r76Costs[4];
Int ThingTemplate::calcCostToBuild( const Player *player ) const { int i = r76TmplIndex(this); return i < 0 ? 0 : (Int)(Short)(g_r76Costs[i] & 0xffff); }
Int ThingTemplate::calcTimeToBuild( const Player *player ) const { int i = r76TmplIndex(this); return i < 0 ? 0 : (Int)(Short)(g_r76Costs[i] >> 16); }
/* the factory's selected template shows the test's upgrades as cameos */
const char *const kR76Upgrades[4] = { "R76P1", "R76P2", "R76O1", "R76O2" };
AsciiString ThingTemplate::getUpgradeCameoName( Int buttonIndex ) const
{
	extern Object *g_r76Me;
	if (g_r76Me && this == g_r76Me->getTemplate() && buttonIndex >= 1 && buttonIndex <= 4) return AsciiString(kR76Upgrades[buttonIndex - 1]);
	return AsciiString();
}
/* the object's "upgrade module": it would act on the test's upgrades (a mask) until it has */
UpgradeMaskType g_r76Trigger;
Bool g_r76Upgraded = false;
unsigned g_r76CanProduce = 0;
Object *g_r76Me = 0;
Bool Object::affectedByUpgrade( const UpgradeTemplate *upgrade ) const
{
	/* Object.cpp's: the union of the player's, the object's and this upgrade's masks, asked of the upgrade module */
	UpgradeMaskType maskToCheck;
	if (m_player) maskToCheck.set(m_player->getCompletedUpgradeMask());
	maskToCheck.set(m_objectUpgradesCompleted);
	maskToCheck.set(upgrade->getUpgradeMask());
	return this == g_r76Me && !g_r76Upgraded && maskToCheck.testForAny(g_r76Trigger);
}
Bool Object::canProduceUpgrade( const UpgradeTemplate *upgrade )
{
	for (int i = 0; i < 4; ++i) if (upgrade->getUpgradeName() == kR76Upgrades[i]) return (g_r76CanProduce >> i & 1) != 0;
	return FALSE;
}
void Object::giveUpgrade( const UpgradeTemplate *upgrade )
{
	/* Object.cpp's: the object's mask, then the upgrade modules (the test's one, once) unless under construction or without a player */
	if (upgrade) {
		m_objectUpgradesCompleted.set(upgrade->getUpgradeMask());
		if (testStatus(OBJECT_STATUS_UNDER_CONSTRUCTION) || !m_player) return;
		UpgradeMaskType maskToCheck = m_player->getCompletedUpgradeMask();
		maskToCheck.set(m_objectUpgradesCompleted);
		if (this == g_r76Me && !g_r76Upgraded && maskToCheck.testForAny(g_r76Trigger)) g_r76Upgraded = true;
	}
}
unsigned g_r76ValidSounds = 0;
Bool AudioManager::isValidAudioEvent( const AudioEventRTS *eventToCheck ) const
{
	return eventToCheck->getEventName() == "R76Done" && (g_r76ValidSounds & 1);
}
/* the exit module and the parking place behaviour */
class R76Exit : public UpdateModule, public ExitInterface
{
public:
	R76Exit(Thing *thing, const ModuleData *d) : UpdateModule(thing, d) {}
	virtual ~R76Exit() {}
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
	virtual NameKeyType getModuleNameKey() const { return (NameKeyType)0; }
	virtual ExitInterface* getUpdateExitInterface() { return m_r76On ? this : NULL; }
	virtual Bool isExitBusy() const { return FALSE; }
	virtual ExitDoorType reserveDoorForExit( const ThingTemplate* objType, Object *specificObject ) { ulog("reservedoor %d", (int)getObject()->m_oid); return m_r76Door; }
	virtual void exitObjectViaDoor( Object *newObj, ExitDoorType exitDoor ) { ulog("exitdoor %d %d %d", (int)getObject()->m_oid, (int)newObj->m_oid, (int)exitDoor); }
	virtual void exitObjectByBudding( Object *newObj, Object *budHost ) {}
	virtual void unreserveDoorForExit( ExitDoorType exitDoor ) { ulog("unreservedoor %d %d", (int)getObject()->m_oid, (int)exitDoor); }
	virtual void setRallyPoint( const Coord3D *pos ) {}
	virtual const Coord3D *getRallyPoint( void ) const { return NULL; }
	Bool m_r76On = false;
	ExitDoorType m_r76Door = DOOR_1;
};
class R76Park : public UpdateModule, public ParkingPlaceBehaviorInterface
{
public:
	R76Park(Thing *thing, const ModuleData *d) : UpdateModule(thing, d) {}
	virtual ~R76Park() {}
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
	virtual NameKeyType getModuleNameKey() const { return (NameKeyType)0; }
	virtual ParkingPlaceBehaviorInterface* getParkingPlaceBehaviorInterface() { return m_r76On ? this : NULL; }
	virtual Bool shouldReserveDoorWhenQueued(const ThingTemplate* thing) const
	{
		ulog("ppreserve %d %s", (int)getObject()->m_oid, thing ? thing->getName().str() : "(null)");
		return m_r76Reserve;
	}
	virtual Bool hasAvailableSpaceFor(const ThingTemplate* thing) const
	{
		ulog("ppspace %d %s", (int)getObject()->m_oid, thing ? thing->getName().str() : "(null)");
		return m_r76Space;
	}
	virtual Bool hasReservedSpace(ObjectID id) const { return FALSE; }
	virtual Int getSpaceIndex( ObjectID id ) const { return -1; }
	virtual Bool reserveSpace(ObjectID id, Real parkingOffset, PPInfo* info) { return FALSE; }
	virtual void releaseSpace(ObjectID id) {}
	virtual Bool reserveRunway(ObjectID id, Bool forLanding) { return FALSE; }
	virtual void calcPPInfo( ObjectID id, PPInfo *info ) {}
	virtual void releaseRunway(ObjectID id) {}
	virtual Int getRunwayCount() const { return 0; }
	virtual ObjectID getRunwayReservation( Int r, RunwayReservationType type ) { return INVALID_ID; }
	virtual void transferRunwayReservationToNextInLineForTakeoff(ObjectID id) {}
	virtual Real getApproachHeight() const { return 0.0f; }
	virtual Real getLandingDeckHeightOffset() const { return 0.0f; }
	virtual void setHealee(Object* healee, Bool add) {}
	virtual void killAllParkedUnits() {}
	virtual void defectAllParkedUnits(Team* newTeam, UnsignedInt detectionTime) {}
	virtual Bool calcBestParkingAssignment( ObjectID id, Coord3D *pos, Int *oldIndex, Int *newIndex ) { return FALSE; }
	virtual const std::vector<Coord3D>* getTaxiLocations( ObjectID id ) const { return NULL; }
	virtual const std::vector<Coord3D>* getCreationLocations( ObjectID id ) const { return NULL; }
	Bool m_r76On = false, m_r76Reserve = false, m_r76Space = false;
};

namespace {

ModuleData *l9r76Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = ProductionUpdate::friend_newModuleData(&ini);
		} catch (...) {
			char buff[1024];
			sprintf(buff, "Error parsing INI file '%s' (Line: '%s')\n", ini.m_filename.str(), currentLine.str());
			throw INIException(buff);
		}
	} catch (...) {
		ini.unPrepFile();
		throw;
	}
	ini.unPrepFile();
	return data;
}

std::vector<unsigned char> g_r76Saved;
std::vector<Object *> g_r76Made;
unsigned g_r76NextId = 6;
ThingTemplate g_r76Templates[4];
const char *const g_r76TmplNames[4] = { "UnitA", "UnitB", "Dozer", "Tower" };
Bool g_r76Selected = false;
AudioEventRTS g_r76VoiceCreate;
Object *r76Find(ObjectID id)
{
	if (id == 1) return g_r76Me;
	for (size_t i = 0; i < g_r76Made.size(); ++i) if (g_r76Made[i]->m_oid == id) return g_r76Made[i];
	return 0;
}
const ThingTemplate *r76FindTemplate(const AsciiString &name)
{
	for (int i = 0; i < 4; ++i) if (strcmp(name.str(), g_r76TmplNames[i]) == 0) return &g_r76Templates[i];
	return 0;
}
Object *r76NewObject(const ThingTemplate *tmplate, Team *team)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)g_r76NextId++;
	o->m_team = team;
	o->m_player = team ? team->m_controller : 0;
	o->m_tmplShared = tmplate;
	static BehaviorModule *none[1] = { 0 };
	o->m_behaviors = none;
	g_r76Made.push_back(o);
	return o;
}
const AudioEventRTS *r76Sound(const AsciiString &name) { return name == "VoiceCreate" ? &g_r76VoiceCreate : 0; }
Drawable *r76Selected() { return g_r76Selected && g_r76Me ? g_r76Me->getDrawable() : 0; }
void r76Message(const UnicodeString &text) { ulog("uimessage %s %s", text.m_label ? text.m_label : "", text.m_argLabel ? text.m_argLabel : ""); }

}  // namespace

extern const AudioEventRTS *(*g_perUnitSoundHook)(const AsciiString &);
extern Drawable *(*g_r76SelectHook)();
extern void (*g_r76MessageHook)(const UnicodeString &);

extern "C" {

/* Read `filename` (block `D`) as the data of a ProductionUpdate and make the module on the factory (id 1) at frame cfg[0]. `cfg` (32 words;
** 2-12 also settable by op 2): 1 the upgrade module's trigger (read once: bit i the test's upgrade i, R76P1, R76P2, R76O1, R76O2); 2 the factory:
** bits 0-1 its team (0 none, else 1-3), 2 sold, 3 under construction, 4 a drawable, 5 the drawable shows ACTIVELY_CONSTRUCTING, 6-7 the local
** player (3 none), 8 selected, 9 an exit module, 10-12 its door (0-3 the door, 4 DOOR_NONE_AVAILABLE, 5 DOOR_NONE_NEEDED, 6 door 5), 13 a
** parking place behaviour, 14 it reserves a door when queued, 15 it has space; 3 the players' money; 4 their permissions (bits 2p, 2p+1:
** base, units); 5 canMakeUnit for the four templates (3 bits each); 6 canProduceUpgrade for the four upgrades (a bit each); 7 the research
** sound is valid; 8-11 the four templates' cost (low half, signed) and build time (high half, signed). The data dump goes first (the
** numbers, the quantity modifiers, the disabled types), then the state. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a
** 2 cfg[a] = b  3 queueCreateUnit(template a (4: NULL), id b)  4 queueUpgrade(upgrade a (4: Up0, 5: NULL))  5 cancelUnitCreate(id a)
** 6 cancelUpgrade(upgrade a)  7 cancelAllUnitsOfType(template a)  8 cancelAndRefundAllProduction  9 onDie  10 setHoldDoorOpen(a, b)
** 11 the queries (canQueueCreateUnit(a) | canQueueUpgrade << 4 | isUpgradeInQueue(b) << 8 | countUnitTypeInQueue(a) << 9 |
** getProductionCount << 16)  12 requestUniqueUnitID  13 xfer save  14 xfer load of the last save  15 xfer CRC. Each op appends: the
** CATCH_ALL status, the result, the effect log, the xfer status, bytes and CRC, the state, the upgrades (the factory's mask, the upgrade
** module's flag, each player's completed and in production masks, of the four test upgrades), the objects made and the last one's
** producer and team. */
static int l9r76RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r76Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r76Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const ProductionUpdateModuleData *pd = (const ProductionUpdateModuleData *)d;
		putI32(pd->m_numDoorAnimations);
		putU32(pd->m_doorOpeningTime);
		putU32(pd->m_doorWaitOpenTime);
		putU32(pd->m_doorClosingTime);
		putU32(pd->m_constructionCompleteDuration);
		putI32(pd->m_maxQueueEntries);
		putU32((unsigned)pd->m_quantityModifiers.size());
		for (size_t i = 0; i < pd->m_quantityModifiers.size(); ++i) {
			putStr(pd->m_quantityModifiers[i].m_templateName.str(), pd->m_quantityModifiers[i].m_templateName.getLength());
			putI32(pd->m_quantityModifiers[i].m_quantity);
		}
		unsigned bits = 0;
		for (int t = 0; t < DISABLED_COUNT; ++t) if (pd->m_disabledTypesToProcess.test((DisabledType)t)) bits |= 1u << t;
		putU32(bits);
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	ThePlayerList = &g_spList;   /* (batch H's: the local player is g_spLocal) */
	AbAudio audio;   /* (batch P's: addAudioEvent is die batch D's line) */
	TheAudio = &audio;
	g_audioHandleLast = 0;
	g_r76NextId = 6;
	g_r76VoiceCreate.setEventName(AsciiString("R76VoiceCreate"));
	Player players[3];
	Team teams[3];
	for (int p = 0; p < 3; ++p) {
		players[p].m_idx = p;
		players[p].m_money.m_idx = p;
		players[p].m_score.m_idx = p;
		teams[p].m_dummy = p + 1;
		teams[p].m_controller = &players[p];
		players[p].m_defaultTeam = &teams[p];
	}
	for (int i = 0; i < 4; ++i) {
		ThingTemplate &t = g_r76Templates[i];
		t.m_name = AsciiString(g_r76TmplNames[i]);
		t.m_kindOf.clear();
		if (i == 2) t.m_kindOf.set(KINDOF_DOZER);
		if (i == 3) t.m_kindOf.set(KINDOF_STRUCTURE);
		char v[32];
		sprintf(v, "R76Voice%d", i);
		t.m_voiceCreated.setEventName(AsciiString(v));
		g_r76Tmpl[i] = &t;
	}
	ThingTemplate factoryTemplate;
	factoryTemplate.m_name = AsciiString("R76Factory");
	Object *me = newObject();
	me->m_oid = 1;
	me->m_tmplShared = &factoryTemplate;
	g_r76Me = me;
	g_r76Upgraded = false;
	g_r76Trigger.clear();
	for (int i = 0; i < 4; ++i) {
		if (!(cfg[1] >> i & 1)) continue;
		const UpgradeTemplate *u = TheUpgradeCenter->findUpgrade(AsciiString(kR76Upgrades[i]));
		if (u) g_r76Trigger.set(u->getUpgradeMask());
	}
	Drawable draw;
	draw.m_ownerObject = me;
	UpdateModuleData ud;
	R76Exit *exitModule = new R76Exit(me, &ud);
	R76Park *park = new R76Park(me, &ud);
	auto scene = [&]() {
		unsigned f = cfg[2];
		unsigned t = f & 3;
		me->m_team = t ? &teams[t - 1] : 0;
		me->m_player = t ? &players[t - 1] : 0;
		me->m_status.clear();
		if (f >> 2 & 1) me->m_status.set(OBJECT_STATUS_SOLD);
		if (f >> 3 & 1) me->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
		me->m_drawable = (f >> 4 & 1) ? &draw : 0;
		draw.m_condFlags.clear();
		if (f >> 5 & 1) draw.m_condFlags.set(MODELCONDITION_ACTIVELY_CONSTRUCTING);
		unsigned local = f >> 6 & 3;
		g_spLocal = local < 3 ? &players[local] : 0;
		g_r76Selected = (f >> 8 & 1) != 0;
		exitModule->m_r76On = (f >> 9 & 1) != 0;
		static const ExitDoorType kDoors[8] = { DOOR_1, DOOR_2, DOOR_3, DOOR_4, DOOR_NONE_AVAILABLE, DOOR_NONE_NEEDED, (ExitDoorType)5, DOOR_1 };
		exitModule->m_r76Door = kDoors[f >> 10 & 7];
		park->m_r76On = (f >> 13 & 1) != 0;
		park->m_r76Reserve = (f >> 14 & 1) != 0;
		park->m_r76Space = (f >> 15 & 1) != 0;
		for (int p = 0; p < 3; ++p) {
			players[p].m_money.m_count = cfg[3];
			players[p].m_canBuildBase = (cfg[4] >> (2 * p) & 1) != 0;
			players[p].m_canBuildUnits = (cfg[4] >> (2 * p + 1) & 1) != 0;
		}
		g_r76CanMake = cfg[5];
		g_r76CanProduce = cfg[6];
		g_r76ValidSounds = cfg[7];
		for (int i = 0; i < 4; ++i) g_r76Costs[i] = cfg[8 + i];
	};
	scene();
	g_modFindHook = r76Find;
	g_findTemplateHook = r76FindTemplate;
	g_newObjectHook = r76NewObject;
	g_mcHook = r43Mc;
	g_perUnitSoundHook = r76Sound;
	g_r76SelectHook = r76Selected;
	g_r76MessageHook = r76Message;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			if (g_events[j].kind == 3) { log.push_back("uidirty"); continue; }
			if (g_events[j].kind != 100) continue;
			const std::string &l = g_ulog[(size_t)g_events[j].p];
			if (l.compare(0, 10, "setorient ") == 0 || l.compare(0, 13, "settransform ") == 0) continue;
			log.push_back(l);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	ProductionUpdate *m = 0;
	try { m = newInstance(ProductionUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[4] = { m, exitModule, park, 0 };
	me->m_behaviors = behaviors;
	auto upgradeIndex = [](const UpgradeTemplate *u) -> unsigned {
		if (!u) return 0xffffffffu;
		for (int i = 0; i < 4; ++i) if (u->getUpgradeName() == kR76Upgrades[i]) return (unsigned)i;
		return 100u;
	};
	auto putState = [&]() {
		putU32((unsigned)m->m_uniqueID);
		putU32(m->m_productionCount);
		putU32(m->m_constructionCompleteFrame);
		putU32(m->m_flagsDirty ? 1u : 0u);
		for (int i = 0; i < DOOR_COUNT_MAX; ++i) {
			putU32(m->m_doors[i].m_doorOpenedFrame);
			putU32(m->m_doors[i].m_doorWaitOpenFrame);
			putU32(m->m_doors[i].m_doorClosedFrame);
			putU32(m->m_doors[i].m_holdOpen ? 1u : 0u);
		}
		const ModelConditionFlags *fs[2] = { &m->m_clearFlags, &m->m_setFlags };
		for (int k = 0; k < 2; ++k) {
			for (int i = 0; i < MODELCONDITION_COUNT; ++i) if (fs[k]->test((ModelConditionFlagType)i)) putU32((unsigned)i);
			putU32(0xffffffffu);
		}
		unsigned count = 0;
		for (const ProductionEntry *p = m->firstProduction(); p; p = m->nextProduction(p)) ++count;
		putU32(count);
		for (const ProductionEntry *p = m->firstProduction(); p; p = m->nextProduction(p)) {
			putU32((unsigned)p->m_type);
			putU32(p->m_type == PRODUCTION_UNIT ? (unsigned)r76TmplIndex(p->m_objectToProduce) : 0xffffffffu);
			putU32(p->m_type == PRODUCTION_UNIT ? 0xffffffffu : upgradeIndex(p->m_upgradeToResearch));
			putU32((unsigned)p->m_productionID);
			putU32(realBits(p->m_percentComplete));
			putU32((unsigned)p->m_framesUnderConstruction);
			putU32((unsigned)p->m_productionQuantityTotal);
			putU32((unsigned)p->m_productionQuantityProduced);
			putU32((unsigned)p->m_exitDoor);
		}
	};
	auto putUpgrades = [&]() {
		unsigned bits = 0;
		for (int i = 0; i < 4; ++i) {
			const UpgradeTemplate *u = TheUpgradeCenter->findUpgrade(AsciiString(kR76Upgrades[i]));
			if (!u) continue;
			if (me->m_objectUpgradesCompleted.testForAll(u->getUpgradeMask())) bits |= 1u << i;
			for (int p = 0; p < 3; ++p) {
				if (players[p].m_upgradesCompleted.testForAll(u->getUpgradeMask())) bits |= 1u << (4 + 8 * p + i);
				if (players[p].m_upgradesInProduction.testForAll(u->getUpgradeMask())) bits |= 1u << (8 + 8 * p + i);
			}
		}
		putU32(bits);
		putU32(g_r76Upgraded ? 1u : 0u);
	};
	putState();
	putUpgrades();
	auto tmpl = [](unsigned a) -> const ThingTemplate * { return a < 4 ? g_r76Tmpl[a] : 0; };
	auto upg = [](unsigned a) -> const UpgradeTemplate * {
		if (a < 4) return TheUpgradeCenter->findUpgrade(AsciiString(kR76Upgrades[a]));
		if (a == 4) return TheUpgradeCenter->findUpgrade(AsciiString("Up0"));
		return 0;
	};
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 12) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[2048];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: res = m->queueCreateUnit(tmpl(o[1]), (ProductionID)o[2]) ? 1 : 0; break;
			case 4: res = m->queueUpgrade(upg(o[1])) ? 1 : 0; break;
			case 5: m->cancelUnitCreate((ProductionID)o[1]); break;
			case 6: m->cancelUpgrade(upg(o[1])); break;
			case 7: m->cancelAllUnitsOfType(tmpl(o[1])); break;
			case 8: m->cancelAndRefundAllProduction(); break;
			case 9: { DamageInfo info; m->onDie(&info); break; }
			case 10: m->setHoldDoorOpen((ExitDoorType)(Int)o[1], o[2] != 0); break;
			case 11: {
				const ThingTemplate *t = tmpl(o[1] % 4);
				res = (unsigned)m->canQueueCreateUnit(t) | (unsigned)m->canQueueUpgrade(upg(o[2])) << 4 | (m->isUpgradeInQueue(upg(o[2])) ? 1u : 0u) << 8
					| m->countUnitTypeInQueue(tmpl(o[1])) << 9 | m->getProductionCount() << 16;
				break;
			}
			case 12: res = (unsigned)m->requestUniqueUnitID(); break;
			case 13: xst = abRunXfer(m, 0, xfer_path, bytes, 2048, &blen, &crc); g_r76Saved.assign(bytes, bytes + blen); break;
			case 14: {
				std::vector<unsigned char> b = g_r76Saved;
				if (b.size() > 2048) b.resize(2048);
				xst = abRunXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 15: xst = abRunXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		if (st) res = 0;
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putState();
		putUpgrades();
		putU32((unsigned)g_r76Made.size());
		putU32(g_r76Made.empty() ? 0u : (unsigned)g_r76Made.back()->m_producerID);
		putU32(g_r76Made.empty() || !g_r76Made.back()->m_team ? 0u : (unsigned)g_r76Made.back()->m_team->m_dummy);
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_mcHook = 0;
	g_perUnitSoundHook = 0;
	g_r76SelectHook = 0;
	g_r76MessageHook = 0;
	g_spLocal = 0;
	g_r76Me = 0;
	delete m;   /* (its destructor empties the queue through the object's exit module: before the object lets go of its modules) */
	me->m_behaviors = 0;
	me->m_drawable = 0;
	me->m_team = 0; me->m_player = 0; me->m_tmplShared = 0;
	delete exitModule;
	delete park;
	delete d;
	delete me;
	for (size_t i = 0; i < g_r76Made.size(); ++i) { g_r76Made[i]->m_team = 0; g_r76Made[i]->m_player = 0; g_r76Made[i]->m_behaviors = 0; g_r76Made[i]->m_tmplShared = 0; delete g_r76Made[i]; }
	g_r76Made.clear();
	ThePlayerList = 0;
	TheAudio = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r76_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r76RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
