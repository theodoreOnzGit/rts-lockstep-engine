/* The driver of the BuildAssistant differential (p1w10-sonnet): the definitions of the stand-ins of ba_standins_decl.h, the program reader, the
** operations, the state dump, and the entry point ref_ba_run. See tests/differential_build_assistant.rs for the program format (the writer is its
** generator; this file is the reader) and for the method. Included by differential/wrap_build_assistant.cpp after the cut BuildAssistant.cpp. */
#ifndef BA_DRIVER_H
#define BA_DRIVER_H

/* ---- the world of a program ----------------------------------------------------------------------------------- */
static GlobalData g_globalData; GlobalData* TheGlobalData = &g_globalData;
static unsigned g_seedH, g_tileDensity, g_cellDensity, g_shroudDensity, g_layerDensity, g_seedTile, g_seedCell, g_seedShroud, g_seedLayer;
static Region3D g_extent, g_pfExtent;
static UnsignedInt g_frame = 0;
static std::vector<ThingTemplate*> g_tmpls;
static std::vector<Player*> g_players;
static std::vector<Object*> g_objs;      /* ids are indices + 1 */
static std::vector<char> g_removed;
static std::vector<CommandSet*> g_sets;
static std::vector<CommandButton*> g_buttons;
static std::vector<std::vector<int> > g_rel;
static CommandButton g_spButton;

static TerrainLogic g_terrain; TerrainLogic* TheTerrainLogic = &g_terrain;
static TerrainVisual g_terrainVisual; TerrainVisual* TheTerrainVisual = &g_terrainVisual;
static AI g_ai; AI* TheAI = &g_ai;
static Pathfinder g_pathfinder;
static PartitionManager g_partition; PartitionManager* ThePartitionManager = &g_partition;
static ControlBar g_controlBar; ControlBar* TheControlBar = &g_controlBar;
static AudioManager g_audio; AudioManager* TheAudio = &g_audio;
static GameLogic g_gameLogic; GameLogic* TheGameLogic = &g_gameLogic;
static ThingFactory g_thingFactory; ThingFactory* TheThingFactory = &g_thingFactory;
static BuildAssistant g_buildAssistant;   /* (upstream's BuildAssistant.cpp defines TheBuildAssistant, set in ref_ba_run) */

static unsigned h3(unsigned a, unsigned b, unsigned c)
{
	unsigned x = a * 73856093u ^ b * 19349663u ^ c * 83492791u;
	x ^= x >> 13; x *= 0x5bd1e995u; x ^= x >> 15;
	return x;
}
static int cellOf(Real v) { return (int)floorf(v / 10.0f); }
static int oid(const Object* o) { return o ? (int)o->m_id : 0; }

/* ---- the stand-ins' members ------------------------------------------------------------------------------------- */
Int ThingTemplate::calcCostToBuild(const Player*) const { return m_cost; }
Bool AIUpdateInterface::isBusy() const { return m_owner->m_aiBusy; }
void AIUpdateInterface::aiIdle(CommandSourceType) { Rec("ai.idle %d", oid(m_owner)); }
Bool AIUpdateInterface::isQuickPathAvailable(const Coord3D*) const { return m_owner->m_quickPath; }
void AIUpdateInterface::aiMoveToPositionEvenIfSleeping(const Coord3D* pos, CommandSourceType) { Rec("ai.move %d %08x %08x %08x", oid(m_owner), FB(pos->x), FB(pos->y), FB(pos->z)); }
Object* AIUpdateInterface::construct(const ThingTemplate* what, const Coord3D* pos, Real angle, Player* owner, Bool)
{
	Rec("ai.construct %d tmpl=%d pos=%08x,%08x,%08x angle=%08x player=%d", oid(m_owner), what->m_idx, FB(pos->x), FB(pos->y), FB(pos->z), FB(angle), owner->m_index);
	if (!m_owner->m_quickPath) return NULL;
	ObjectStatusMaskType st = MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_UNDER_CONSTRUCTION);
	Object* o = TheThingFactory->newObject(what, (Team*)owner, st);
	o->m_pos = *pos;
	o->m_orient = angle;
	Rec("ai.constructed %d", oid(o));
	return o;
}
const CommandButton* ProductionUpdateInterface::getSpecialPowerConstructionCommandButton() const
{
	if (m_owner->m_spConsTmpl < 0) return NULL;
	g_spButton.m_tmpl = g_tmpls[m_owner->m_spConsTmpl];
	return &g_spButton;
}
CanMakeInt ProductionUpdateInterface::canQueueCreateUnit(const ThingTemplate*) const { CanMakeInt r; r.v = m_owner->m_canQueue; return r; }
void ProductionUpdateInterface::cancelAndRefundAllProduction() { Rec("prod.cancel %d", oid(m_owner)); }
void ContainModuleInterface::onSelling() { Rec("contain.sell %d", oid(m_owner)); }
void ParkingPlaceBehaviorInterface::killAllParkedUnits() { Rec("parking.kill %d %d", oid(m_owner), m_idx); }
void CreateModuleInterface::onBuildComplete() { Rec("oncreate.done %d %d", oid(m_owner), m_n); }
void Drawable::setAnimationLoopDuration(UnsignedInt frames) { Rec("draw.anim %d %u", oid(m_owner), (unsigned)frames); }

Bool Object::testStatus(ObjectStatusTypes s) const { return (m_status >> s) & 1ull; }
Team* Object::getTeam() const { return (Team*)m_player; }
Relationship Object::getRelationship(const Object* that) const
{
	if (!that || !m_player || !that->m_player) return NEUTRAL;
	return (Relationship)g_rel[m_player->m_index][that->m_player->m_index];
}
AsciiString Object::getCommandSetString() const { char b[32]; if (m_cmdSet < 0) return AsciiString(""); snprintf(b, sizeof b, "cs%d", m_cmdSet); return AsciiString(b); }
void Object::setConstructionPercent(Real p) { Rec("cpct %d %08x", oid(this), FB(p)); m_cpct = p; }
void Object::setProducer(const Object* p) { m_producer = p ? p->m_id : INVALID_ID; Rec("producer %d %d", oid(this), oid(p)); }
void Object::setPosition(const Coord3D* p) { Rec("pos %d %08x %08x %08x", oid(this), FB(p->x), FB(p->y), FB(p->z)); m_pos = *p; }
void Object::setOrientation(Real a) { Rec("orient %d %08x", oid(this), FB(a)); m_orient = a; }
static std::string listOf(const std::vector<int>& v) { std::string s; for (size_t i = 0; i < v.size(); ++i) { char b[16]; snprintf(b, sizeof b, "%s%d", i ? "," : "", v[i]); s += b; } return s; }
static std::vector<int> bitsOf(const ModelConditionFlags& f) { std::vector<int> v; for (int i = 0; i < MODELCONDITION_COUNT; ++i) if (f.test(i)) v.push_back(i); return v; }
static void mcBit(unsigned long long* lo, unsigned long long* hi, int i, bool on)
{
	unsigned long long* w = i < 64 ? lo : hi; unsigned long long m = 1ull << (i & 63);
	if (on) *w |= m; else *w &= ~m;
}
static unsigned long long g_mcLo[4096], g_mcHi[4096];   /* per object id */
void Object::clearAndSetModelConditionFlags(const ModelConditionFlags& clr, const ModelConditionFlags& set)
{
	std::vector<int> c = bitsOf(clr), s = bitsOf(set);
	Rec("mc.clearset %d clear=%s set=%s", oid(this), listOf(c).c_str(), listOf(s).c_str());
	for (size_t i = 0; i < c.size(); ++i) mcBit(&g_mcLo[m_id], &g_mcHi[m_id], c[i], false);
	for (size_t i = 0; i < s.size(); ++i) mcBit(&g_mcLo[m_id], &g_mcHi[m_id], s[i], true);
}
void Object::setModelConditionFlags(const ModelConditionFlags& set)
{
	std::vector<int> s = bitsOf(set);
	Rec("mc.set %d set=%s", oid(this), listOf(s).c_str());
	for (size_t i = 0; i < s.size(); ++i) mcBit(&g_mcLo[m_id], &g_mcHi[m_id], s[i], true);
}
void Object::setStatus(ObjectStatusMaskType objectStatus, Bool set)
{
	std::vector<int> v;
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (objectStatus.test(i)) v.push_back(i);
	Rec("status %d %s set=%d", oid(this), listOf(v).c_str(), (int)set);
	for (size_t i = 0; i < v.size(); ++i) { if (set) m_status |= (1ull << v[i]); else m_status &= ~(1ull << v[i]); }
}
void Object::handlePartitionCellMaintenance() { Rec("partition.maint %d", oid(this)); }
Object* Object::getNextObject() const
{
	for (size_t i = m_id; i < g_objs.size(); ++i) if (!g_removed[i]) return g_objs[i];
	return NULL;
}

void Money::deposit(UnsignedInt amount) { Rec("deposit %d %u", m_idx, (unsigned)amount); m_money += amount; }
Team* Player::getDefaultTeam() const { return m_hasTeam ? (Team*)this : NULL; }
Relationship Player::getRelationship(const Team* that) const
{
	if (!that) return NEUTRAL;
	return (Relationship)g_rel[m_index][((const Player*)that)->m_index];
}
Bool Player::canBuild(const ThingTemplate* t) const { return (m_canBuildMask >> t->m_idx) & 1u; }
Bool Player::canBuildMoreOfType(const ThingTemplate* t) const { return (m_canMoreMask >> t->m_idx) & 1u; }
void Player::onStructureCreated(Object* builder, Object* structure) { Rec("p.structcreated %d %d %d", m_index, oid(builder), oid(structure)); }
void Player::onStructureConstructionComplete(Object* builder, Object* structure, Bool) { Rec("p.structdone %d %d %d", m_index, oid(builder), oid(structure)); }
void Player::onUnitCreated(Object* factory, Object* unit) { Rec("p.unitcreated %d %d %d", m_index, oid(factory), oid(unit)); }

Real TerrainLogic::getGroundHeight(Real x, Real y, Coord3D*) const { return (Real)(h3((unsigned)cellOf(x), (unsigned)cellOf(y), g_seedH) & 7u) * 0.75f; }
void TerrainLogic::getExtent(Region3D* r) const { *r = g_extent; }
void TerrainLogic::getMaximumPathfindExtent(Region3D* r) const { *r = g_pfExtent; }
PathfindLayerEnum TerrainLogic::getLayerForDestination(const Coord3D* pos) const
{
	if (g_layerDensity > 0 && h3((unsigned)cellOf(pos->x), (unsigned)cellOf(pos->y), g_seedLayer) % g_layerDensity == 0) return LAYER_WALL;
	return LAYER_GROUND;
}
TerrainType* TerrainVisual::getTerrainTile(Real x, Real y)
{
	static TerrainType tile;
	unsigned hh = h3((unsigned)cellOf(x), (unsigned)cellOf(y), g_seedTile);
	if (hh % 7u == 0) return NULL;
	tile.m_restrict = g_tileDensity > 0 && hh % g_tileDensity == 0;
	return &tile;
}
void TerrainVisual::addFactionBib(Object* o, Bool highlight, Real extra) { Rec("bib %d %d %08x", oid(o), (int)highlight, FB(extra)); }
void TerrainVisual::removeTreesAndPropsForConstruction(const Coord3D* pos, const GeometryInfo& geom, Real angle)
{
	Rec("trees %08x %08x %08x geom=%d,%08x,%08x angle=%08x", FB(pos->x), FB(pos->y), FB(pos->z), (int)geom.getGeomType(), FB(geom.getMajorRadius()), FB(geom.getMinorRadius()), FB(angle));
}
PathfindCell* Pathfinder::getCell(PathfindLayerEnum, Int x, Int y)
{
	static PathfindCell cell;
	unsigned hh = h3((unsigned)x, (unsigned)y, g_seedCell);
	if (hh % 23u == 0) return NULL;
	cell.m_type = (g_cellDensity > 0 && hh % g_cellDensity == 0) ? (PathfindCell::CellType)(((hh >> 8) % 6u) + 1) : PathfindCell::CELL_CLEAR;
	return &cell;
}
void Pathfinder::addObjectToPathfindMap(Object* obj) { Rec("pf.add %d", oid(obj)); }
Pathfinder* AI::pathfinder() { return &g_pathfinder; }

static Real dist2(const Coord3D* a, const Coord3D* b) { Real dx = b->x - a->x, dy = b->y - a->y; return dx * dx + dy * dy; }
ObjectIterator* PartitionManager::iteratePotentialCollisions(const Coord3D* pos, const GeometryInfo& geom, Real, Bool)
{
	ObjectIterator* it = newInstance(ObjectIterator);
	Real maxDist = geom.getBoundingCircleRadius();
	maxDist *= 1.1f;
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		Object* o = g_objs[i];
		if (g_removed[i] || o->m_destroyed) continue;
		Real r = maxDist + o->m_geom.getBoundingCircleRadius();
		if (dist2(pos, &o->m_pos) <= r * r) it->m_objs.push_back(o);
	}
	return it;
}
static bool filterAllows(PartitionFilter** filters, Object* o)
{
	if (filters && filters[0]) return o->m_tmpl->m_kinds.testForAll(filters[0]->m_must);
	return true;
}
ObjectIterator* PartitionManager::iterateObjectsInRange(const Coord3D* pos, Real maxDist, DistanceCalculationType, PartitionFilter** filters)
{
	ObjectIterator* it = newInstance(ObjectIterator);
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		Object* o = g_objs[i];
		if (g_removed[i] || o->m_destroyed || !filterAllows(filters, o)) continue;
		Real r = maxDist + o->m_geom.getBoundingCircleRadius();
		if (dist2(pos, &o->m_pos) <= r * r) it->m_objs.push_back(o);
	}
	return it;
}
Object* PartitionManager::getClosestObject(const Coord3D* pos, Real maxDist, DistanceCalculationType, PartitionFilter** filters)
{
	Object* best = NULL; Real bestD = 0;
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		Object* o = g_objs[i];
		if (g_removed[i] || o->m_destroyed || !filterAllows(filters, o)) continue;
		Real r = maxDist + o->m_geom.getBoundingCircleRadius();
		Real d = dist2(pos, &o->m_pos);
		if (d <= r * r && (!best || d < bestD)) { best = o; bestD = d; }
	}
	return best;
}
Bool PartitionManager::geomCollidesWithGeom(const Coord3D* pos1, const GeometryInfo& geom1, Real, const Coord3D* pos2, const GeometryInfo& geom2, Real) const
{
	Real r = geom1.getBoundingCircleRadius() + geom2.getBoundingCircleRadius();
	return dist2(pos1, pos2) < r * r;
}
void PartitionManager::worldToCell(Real wx, Real wy, Int* cx, Int* cy) const { *cx = cellOf(wx); *cy = cellOf(wy); }
CellShroudStatus PartitionManager::getShroudStatusForPlayer(Int playerIndex, Int x, Int y) const
{
	if (g_shroudDensity == 0) return CELLSHROUD_CLEAR;
	unsigned hh = h3((unsigned)x, (unsigned)y, g_seedShroud + (unsigned)playerIndex);
	if (hh % g_shroudDensity != 0) return CELLSHROUD_CLEAR;
	return ((hh >> 9) & 1u) ? CELLSHROUD_FOGGED : CELLSHROUD_SHROUDED;
}

const CommandSet* ControlBar::findCommandSet(const AsciiString& name)
{
	int n = -1;
	if (sscanf(name.str(), "cs%d", &n) != 1 || n < 0 || n >= (int)g_sets.size()) return NULL;
	return g_sets[n];
}
void AudioManager::addAudioEvent(const AudioEventRTS* e) { Rec("audio %d %d", e->m_tmpl, (int)e->m_id); }
UnsignedInt GameLogic::getFrame() const { return g_frame; }
Object* GameLogic::findObjectByID(ObjectID id) { return (id >= 1 && id <= g_objs.size() && !g_removed[id - 1]) ? g_objs[id - 1] : NULL; }
void GameLogic::destroyObject(Object* obj) { if (!obj->m_destroyed) { obj->m_destroyed = true; Rec("destroy %d", oid(obj)); } }
void GameLogic::deselectObject(Object* obj, PlayerMaskType, Bool) { Rec("deselect %d", oid(obj)); }
Object* GameLogic::getFirstObject() { for (size_t i = 0; i < g_objs.size(); ++i) if (!g_removed[i]) return g_objs[i]; return NULL; }

static Object* makeObject(ThingTemplate* t, Player* owner)
{
	Object* o = new Object;
	new (&o->m_geom) GeometryInfo(t->m_geom);
	o->m_id = (ObjectID)(g_objs.size() + 1);
	o->m_tmpl = t;
	o->m_player = owner;
	o->m_ai.m_owner = o; o->m_prod.m_owner = o; o->m_contain.m_owner = o; o->m_draw.m_owner = o;
	unsigned c = (t->m_flags >> 5) & 7u;
	bool parking = (t->m_flags >> 4) & 1u;
	int n = 0, nc = 0;
	if (parking) { BehaviorModule& m = o->m_modules[n]; m.m_owner = o; m.m_idx = n; m.m_hasParking = true; m.m_parking.m_owner = o; m.m_parking.m_idx = n; o->m_modPtrs[n] = &m; ++n; }
	for (unsigned k = 0; k < c; ++k) { BehaviorModule& m = o->m_modules[n]; m.m_owner = o; m.m_idx = n; m.m_hasCreate = true; m.m_create.m_owner = o; m.m_create.m_n = nc++; o->m_modPtrs[n] = &m; ++n; }
	o->m_nModules = n;
	o->m_modPtrs[n] = NULL;
	g_objs.push_back(o);
	g_removed.push_back(0);
	g_mcLo[o->m_id] = 0; g_mcHi[o->m_id] = 0;
	return o;
}
Object* ThingFactory::newObject(const ThingTemplate* tmplate, Team* team, ObjectStatusMaskType status)
{
	Player* owner = (Player*)team;
	Object* o = makeObject((ThingTemplate*)tmplate, owner);
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (status.test(i)) o->m_status |= (1ull << i);
	std::vector<int> v;
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (status.test(i)) v.push_back(i);
	Rec("new tmpl=%d owner=%d status=%s -> %d", tmplate->m_idx, owner->m_index, listOf(v).c_str(), oid(o));
	return o;
}

/* ---- the program ----------------------------------------------------------------------------------------------- */
struct Reader
{
	const unsigned* w; int n, p;
	unsigned u() { if (p >= n) { fprintf(stderr, "ba: program too short\n"); abort(); } return w[p++]; }
	int i() { return (int)u(); }
	Real f() { unsigned b = u(); Real r; memcpy(&r, &b, 4); return r; }
};

static const KindOfType kindTable[11] = { KINDOF_DOZER, KINDOF_STRUCTURE, KINDOF_INERT, KINDOF_SHRUBBERY, KINDOF_CLEARED_BY_BUILD, KINDOF_MINE, KINDOF_ALWAYS_SELECTABLE, KINDOF_IMMOBILE, KINDOF_LINEBUILD, KINDOF_CANNOT_BUILD_NEAR_SUPPLIES, KINDOF_SUPPLY_SOURCE };

static void clearWorld()
{
	for (size_t i = 0; i < g_objs.size(); ++i) delete g_objs[i];
	g_objs.clear(); g_removed.clear();
	for (size_t i = 0; i < g_tmpls.size(); ++i) delete g_tmpls[i];
	g_tmpls.clear();
	for (size_t i = 0; i < g_players.size(); ++i) delete g_players[i];
	g_players.clear();
	for (size_t i = 0; i < g_sets.size(); ++i) delete g_sets[i];
	g_sets.clear();
	for (size_t i = 0; i < g_buttons.size(); ++i) delete g_buttons[i];
	g_buttons.clear();
	g_rel.clear();
}

static void setupWorld(Reader& r)
{
	unsigned seed = r.u();
	ref_init_game_logic_random(seed);
	g_globalData.m_maxLineBuildObjects = r.i();
	g_globalData.m_MinDistFromEdgeOfMapForBuild = r.f();
	g_globalData.m_allowedHeightVariationForBuilding = r.f();
	g_globalData.m_SupplyBuildBorder = r.f();
	g_globalData.m_sellPercentage = r.f();
	g_extent.lo.x = r.f(); g_extent.lo.y = r.f(); g_extent.lo.z = r.f(); g_extent.hi.x = r.f(); g_extent.hi.y = r.f(); g_extent.hi.z = r.f();
	g_pfExtent.lo.x = r.f(); g_pfExtent.lo.y = r.f(); g_pfExtent.lo.z = r.f(); g_pfExtent.hi.x = r.f(); g_pfExtent.hi.y = r.f(); g_pfExtent.hi.z = r.f();
	g_seedH = r.u(); g_tileDensity = r.u(); g_cellDensity = r.u(); g_shroudDensity = r.u(); g_layerDensity = r.u();
	g_seedTile = g_seedH ^ 0x1111u; g_seedCell = g_seedH ^ 0x2222u; g_seedShroud = g_seedH ^ 0x3333u; g_seedLayer = g_seedH ^ 0x4444u;
	g_frame = r.u();
	int nT = r.i();
	for (int k = 0; k < nT; ++k)
	{
		ThingTemplate* t = new ThingTemplate;
		t->m_idx = k;
		unsigned km = r.u();
		for (int b = 0; b < 11; ++b) if ((km >> b) & 1u) t->m_kinds.set(kindTable[b]);
		unsigned gt = r.u(); unsigned small = r.u(); Real h = r.f(); Real maj = r.f(); Real mnr = r.f();
		new (&t->m_geom) GeometryInfo((GeometryType)gt, small != 0, h, maj, mnr);
		t->m_exitWidth = r.f(); t->m_extraBib = r.f(); t->m_vision = r.f();
		t->m_refund = r.i(); t->m_cost = r.i(); t->m_equiv = (int)r.u(); t->m_flags = r.u();
		t->m_voice.m_tmpl = k; t->m_voice.m_id = (ObjectID)0;
		g_tmpls.push_back(t);
	}
	int nP = r.i();
	for (int k = 0; k < nP; ++k)
	{
		Player* p = new Player;
		p->m_index = k;
		p->m_type = r.u() ? PLAYER_COMPUTER : PLAYER_HUMAN;
		p->m_skirmish = r.u() != 0;
		p->m_hasTeam = r.u() != 0;
		p->m_moneyObj.m_money = r.u();
		p->m_moneyObj.m_idx = k;
		p->m_canBuildMask = r.u();
		p->m_canMoreMask = r.u();
		g_players.push_back(p);
	}
	g_rel.assign(nP, std::vector<int>(nP, 1));
	for (int a = 0; a < nP; ++a) for (int b = 0; b < nP; ++b) g_rel[a][b] = (int)(r.u() % 3);
	int nCS = r.i();
	for (int k = 0; k < nCS; ++k)
	{
		CommandSet* cs = new CommandSet;
		for (int s = 0; s < MAX_COMMANDS_PER_SET; ++s)
		{
			cs->m_command[s] = NULL;
			if (r.u())
			{
				CommandButton* b = new CommandButton;
				b->m_type = r.i();
				b->m_tmpl = g_tmpls[r.u() % nT];
				b->m_id = (int)g_buttons.size();
				g_buttons.push_back(b);
				cs->m_command[s] = b;
			}
		}
		g_sets.push_back(cs);
	}
	int nO = r.i();
	for (int k = 0; k < nO; ++k)
	{
		unsigned ti = r.u(); unsigned owner = r.u();
		Object* o = makeObject(g_tmpls[ti % nT], owner >= (unsigned)nP ? NULL : g_players[owner]);
		o->m_pos.x = r.f(); o->m_pos.y = r.f(); o->m_pos.z = r.f();
		o->m_orient = r.f();
		{ unsigned lo = r.u(); unsigned hi = r.u(); o->m_status = (unsigned long long)lo | ((unsigned long long)hi << 32); }
		o->m_disabled = r.u() != 0; o->m_dead = r.u() != 0;
		o->m_scriptBits = r.u();
		o->m_aiBusy = r.u() != 0;
		unsigned cs = r.u(); o->m_cmdSet = cs >= (unsigned)nCS ? -1 : (int)cs;
		o->m_quickPath = r.u() != 0;
		o->m_cpct = r.f();
		unsigned pr = r.u(); o->m_producer = (ObjectID)pr;   /* an object id, or 0 */
		int sp = r.i(); o->m_spConsTmpl = (sp < 0 || sp >= nT) ? -1 : sp;
		o->m_canQueue = (int)(r.u() % 7u);
		unsigned gov = r.u(); Real gh = r.f(), gma = r.f(), gmi = r.f(); unsigned gty = r.u();
		if (gov) new (&o->m_geom) GeometryInfo((GeometryType)(gty % 3u), false, gh, gma, gmi);
	}
}

static Object* sel(unsigned s)
{
	if (s == 0 || g_objs.empty()) return NULL;
	return g_objs[(s - 1) % g_objs.size()];
}
static bool alive(Object* o) { return o && !g_removed[o->m_id - 1]; }

static unsigned hashBytes(const std::vector<unsigned char>& v)
{
	unsigned h = 2166136261u;
	for (size_t i = 0; i < v.size(); ++i) { h ^= v[i]; h *= 16777619u; }
	return h;
}

static void dumpState()
{
	Rec("W frame=%u next=%u", (unsigned)g_frame, (unsigned)g_objs.size() + 1);
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		if (g_removed[i]) continue;
		Object* o = g_objs[i];
		Rec("O %u t=%d p=%d pos=%08x,%08x,%08x o=%08x cp=%08x st=%llx d%d dead%d dst%d sb%u pr=%u mc=%llx,%llx", (unsigned)o->m_id, o->m_tmpl->m_idx, o->m_player ? o->m_player->m_index : -1,
			FB(o->m_pos.x), FB(o->m_pos.y), FB(o->m_pos.z), FB(o->m_orient), FB(o->m_cpct), o->m_status, (int)o->m_disabled, (int)o->m_dead, (int)o->m_destroyed, o->m_scriptBits,
			(unsigned)o->m_producer, g_mcLo[o->m_id], g_mcHi[o->m_id]);
	}
	BuildAssistant& ba = g_buildAssistant;
	std::string pos;
	for (int k = 0; k < ba.m_buildPositionSize && k < 4; ++k) { char b[64]; snprintf(b, sizeof b, "%s%08x,%08x,%08x", k ? " " : "", FB(ba.m_buildPositions[k].x), FB(ba.m_buildPositions[k].y), FB(ba.m_buildPositions[k].z)); pos += b; }
	std::string sells;
	for (ObjectSellListIterator it = ba.m_sellList.begin(); it != ba.m_sellList.end(); ++it) { char b[48]; snprintf(b, sizeof b, "%s%u:%u", sells.empty() ? "" : ",", (unsigned)(*it)->m_id, (unsigned)(*it)->m_sellFrame); sells += b; }
	Rec("B size=%d alloc=%d pos=[%s] sells=[%s]", ba.m_buildPositionSize, (ba.m_buildPositions && ba.m_buildPositionSize > 0) ? 1 : 0, pos.c_str(), sells.c_str());
	for (size_t i = 0; i < g_players.size(); ++i) Rec("P %d money=%u", (int)i, (unsigned)g_players[i]->m_moneyObj.m_money);
}

static std::vector<unsigned char> g_saved;
static bool g_haveSave = false;

static void doOp(unsigned op, Reader& r)
{
	unsigned a[16];
	for (int k = 0; k < 16; ++k) a[k] = r.u();
	Reader q; q.w = a; q.n = 16; q.p = 0;
	int nT = (int)g_tmpls.size(), nP = (int)g_players.size();
	BuildAssistant& ba = g_buildAssistant;
	switch (op)
	{
	case 0: ba.init(); Rec("ret init"); break;
	case 1: ba.reset(); Rec("ret reset"); break;
	case 2:
	{
		/* one update per frame, as the game logic calls it */
		unsigned reps = 1 + q.u() % 160u;
		for (unsigned k = 0; k < reps; ++k) { g_frame++; ba.update(); }
		Rec("ret update %u", reps);
		break;
	}
	case 3:
	{
		BaXferImpl x(XFER_SAVE);
		ba.xferTheSellList(&x);
		g_saved = x.m_data; g_haveSave = true;
		Rec("ret save len=%d hash=%08x", (int)g_saved.size(), hashBytes(g_saved));
		break;
	}
	case 4:
	{
		if (!g_haveSave) { Rec("skip nosave"); break; }
		BaXferImpl x(XFER_LOAD); x.m_data = g_saved;
		ba.xferTheSellList(&x);
		Rec("ret load");
		break;
	}
	case 5: case 6:
	{
		unsigned cs = q.u(), ti = q.u();
		Coord3D pos; pos.x = q.f(); pos.y = q.f(); pos.z = q.f();
		Real angle = q.f();
		unsigned pl = q.u();
		Coord3D end; end.x = q.f(); end.y = q.f(); end.z = q.f();
		Object* c = sel(cs);
		if ((cs != 0 && !alive(c)) || nT == 0 || nP == 0) { Rec("skip"); break; }
		Player* owner = g_players[pl % nP];
		const ThingTemplate* t = g_tmpls[ti % nT];
		if (op == 5)
		{
			Object* o = ba.buildObjectNow(c, t, &pos, angle, owner);
			Rec("ret build %d", oid(o));
		}
		else
		{
			ba.buildObjectLineNow(c, t, &pos, &end, angle, owner);
			Rec("ret buildline");
		}
		break;
	}
	case 7: case 8:
	{
		Coord3D pos; pos.x = q.f(); pos.y = q.f(); pos.z = q.f();
		unsigned ti = q.u(); Real angle = q.f(); unsigned options = q.u(); unsigned bs = q.u(); unsigned ps = q.u();
		Object* b = sel(bs);
		if ((bs != 0 && !alive(b)) || nT == 0 || nP == 0) { Rec("skip"); break; }
		Player* pl = ps == 0 ? NULL : g_players[(ps - 1) % nP];
		const ThingTemplate* t = g_tmpls[ti % nT];
		if (op == 7) Rec("ret legal %d", (int)ba.isLocationLegalToBuild(&pos, t, angle, options, b, pl));
		else Rec("ret clear %d", (int)ba.isLocationClearOfObjects(&pos, t, angle, b, options, pl));
		break;
	}
	case 9:
	{
		Coord3D pos; pos.x = q.f(); pos.y = q.f(); pos.z = q.f();
		unsigned ti = q.u();
		if (nT == 0) { Rec("skip"); break; }
		ba.addBibs(&pos, g_tmpls[ti % nT]);
		Rec("ret bibs");
		break;
	}
	case 10:
	{
		unsigned ti = q.u(); Real angle = q.f();
		Coord3D s, e; s.x = q.f(); s.y = q.f(); s.z = q.f(); e.x = q.f(); e.y = q.f(); e.z = q.f();
		Real tiling = q.f(); Int maxTiles = q.i(); unsigned bs = q.u();
		Object* b = sel(bs);
		if ((bs != 0 && !alive(b)) || nT == 0) { Rec("skip"); break; }
		BuildAssistant::TileBuildInfo* info = ba.buildTiledLocations(g_tmpls[ti % nT], angle, &s, &e, tiling, maxTiles, b);
		std::string ps;
		for (int k = 0; k < info->tilesUsed; ++k) { char bb[64]; snprintf(bb, sizeof bb, "%s%08x,%08x,%08x", k ? " " : "", FB(info->positions[k].x), FB(info->positions[k].y), FB(info->positions[k].z)); ps += bb; }
		Rec("ret tiled used=%d [%s]", info->tilesUsed, ps.c_str());
		break;
	}
	case 11:
	{
		unsigned ti = q.u();
		if (nT == 0) { Rec("skip"); break; }
		Rec("ret isline %d", (int)ba.isLineBuildTemplate(g_tmpls[ti % nT]));
		break;
	}
	case 12: case 13:
	{
		unsigned bs = q.u(), ti = q.u();
		Object* b = sel(bs);
		if (!alive(b) || nT == 0) { Rec("skip"); break; }
		if (op == 12) Rec("ret canmake %d", (int)ba.canMakeUnit(b, g_tmpls[ti % nT]));
		else Rec("ret possible %d", (int)ba.isPossibleToMakeUnit(b, g_tmpls[ti % nT]));
		break;
	}
	case 14:
	{
		Object* o = sel(q.u());
		if (!alive(o)) { Rec("skip"); break; }
		ba.sellObject(o);
		Rec("ret sell");
		break;
	}
	case 15:
	{
		/* GameLogic::processDestroyList: what was destroyed leaves the world */
		for (size_t i = 0; i < g_objs.size(); ++i) if (!g_removed[i] && g_objs[i]->m_destroyed) { g_removed[i] = 1; Rec("removed %d", (int)g_objs[i]->m_id); }
		break;
	}
	case 16:
	{
		Object* o = sel(q.u());
		unsigned which = q.u() % 6u;
		if (!alive(o)) { Rec("skip"); break; }
		switch (which)
		{
		case 0: o->m_pos.x = q.f(); o->m_pos.y = q.f(); o->m_pos.z = q.f(); break;
		case 1: { unsigned lo = q.u(); unsigned hi = q.u(); o->m_status = (unsigned long long)lo | ((unsigned long long)hi << 32); break; }
		case 2: o->m_disabled = q.u() != 0; o->m_dead = q.u() != 0; o->m_scriptBits = q.u(); break;
		case 3: o->m_cpct = q.f(); break;
		case 4: o->m_aiBusy = q.u() != 0; o->m_quickPath = q.u() != 0; break;
		default: if (nP) { unsigned p = q.u(); o->m_player = p % (nP + 1) == (unsigned)nP ? NULL : g_players[p % (nP + 1)]; } break;
		}
		Rec("ret set %d", which);
		break;
	}
	case 18: g_frame += q.u() % 500u; Rec("ret jump"); break;
	case 17:
	{
		if (nP == 0) { Rec("skip"); break; }
		Player* p = g_players[q.u() % nP];
		unsigned which = q.u() % 4u;
		unsigned v = q.u();
		switch (which)
		{
		case 0: p->m_moneyObj.m_money = v; break;
		case 1: p->m_canBuildMask = v; break;
		case 2: p->m_canMoreMask = v; break;
		default: p->m_hasTeam = (v & 1u) != 0; break;
		}
		Rec("ret setplayer %d", which);
		break;
	}
	default: Rec("ret nop");
	}
}

/* The BuildAssistant differential (tests/differential_build_assistant.rs): runs a program (the words) against the real BuildAssistant.cpp on the
** stand-ins, and writes the trace lines, each ended by '\n'. Returns the size of the text (written only if it fits `cap`). */
extern "C" int ref_ba_run(const unsigned* words, int n, const char* /*tmpPath*/, char* out, int cap)
{
	std::vector<std::string> lines;
	g_lines = &lines;
	TheBuildAssistant = &g_buildAssistant;
	g_haveSave = false; g_saved.clear();
	g_buildAssistant.m_buildPositions = NULL;
	g_buildAssistant.m_buildPositionSize = 0;
	g_buildAssistant.m_sellList.clear();
	Reader r; r.w = words; r.n = n; r.p = 0;
	setupWorld(r);
	int nOps = r.i();
	for (int k = 0; k < nOps; ++k)
	{
		unsigned op = r.u();
		doOp(op, r);
		dumpState();
	}
	std::string all;
	for (size_t i = 0; i < lines.size(); ++i) { all += lines[i]; all += '\n'; }
	g_buildAssistant.reset();
	clearWorld();
	if ((int)all.size() <= cap) memcpy(out, all.data(), all.size());
	return (int)all.size();
}

#endif
