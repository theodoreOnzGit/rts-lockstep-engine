/* The world of ScriptActions.cpp, part 3 (sa_world_b.h includes it): the script engine, the terrain logic, the view, the partition, the
** game logic, the factory, the control bar, the client, audio, radar, display and the stores. */
#ifndef SA_WORLD_C_H
#define SA_WORLD_C_H

class ObjectTypes
{
public:
	Int getListSize() const { return (Int)m_types.size(); }
	AsciiString getNthInList(Int i) const { return i >= 0 && i < (Int)m_types.size() ? m_types[i] : AsciiString(); }
	std::vector<AsciiString> m_types;
};
class ScriptEngine
{
public:
	Player *getPlayerFromAsciiString(const AsciiString &name);
	Team *getTeamNamed(const AsciiString &name);
	Object *getUnitNamed(const AsciiString &name);
	Bool didUnitExist(const AsciiString &name) { int k = saNum(name.str(), "unit"); return k >= 0 && k < 32 && (g_cfg[10] >> k & 1); }
	PolygonTrigger *getQualifiedTriggerAreaByName(const AsciiString &name) { int k = saNum(name.str(), "trig"); return k >= 0 && k < 8 && (g_cfg[13] >> k & 1) ? &g_triggers[k] : NULL; }
	Player *getCurrentPlayer() { return saPlayer((int)(g_cfg[5] >> 8 & 15)); }
	Player *getSkirmishEnemyPlayer() { return saPlayer((int)(g_cfg[5] >> 12 & 15)); }
	ObjectTypes *getObjectTypes(const AsciiString &name);
	/* the named set when one is known, else the default one (its name empty): never NULL */
	const AttackPriorityInfo *getAttackInfo(const AsciiString &name) { m_api.m_name = (saHash(SA_SEED, saStrHash(name.str()), 70u) & 1) ? name : AsciiString(); return &m_api; }
	Script *findScriptByName(const AsciiString &name) { m_script.m_name = name; return saNum(name.str(), "script") >= 0 ? &m_script : NULL; }
#define SA_SE(fmt, ...) saLog("se " fmt, ##__VA_ARGS__)
	void AppendDebugMessage(const AsciiString &s, Bool pause) { SA_SE("debug %s %d", s.str(), pause ? 1 : 0); }
	void addObjectToCache(Object *o) { SA_SE("addcache %d", o->getID()); }
	void transferObjectName(const AsciiString &n, Object *o) { SA_SE("transfername %s %d", n.str(), o->getID()); }
	void appendSequentialScript(const SequentialScript *s)
	{
		SA_SE("appendseq %d %d %s %d", s->m_teamToExecOn ? s->m_teamToExecOn->getID() : 0, s->m_objectID, s->m_scriptToExecuteSequentially->getName().str(), s->m_timesToLoop);
		const_cast<SequentialScript *>(s)->deleteInstance();
	}
	void removeAllSequentialScripts(Object *o) { SA_SE("removeseqobj %d", o->getID()); }
	void removeAllSequentialScripts(Team *t) { SA_SE("removeseqteam %d", t->getID()); }
	void setSequentialTimer(Object *o, Int frames) { SA_SE("seqtimerobj %d %d", o->getID(), frames); }
	void setSequentialTimer(Team *t, Int frames) { SA_SE("seqtimerteam %d %d", t->getID(), frames); }
	void createNamedMapReveal(const AsciiString &n, const AsciiString &w, Real r, const AsciiString &p) { SA_SE("createreveal %s %s %08x %s", n.str(), w.str(), fb(r), p.str()); }
	void doNamedMapReveal(const AsciiString &n) { SA_SE("doreveal %s", n.str()); }
	void undoNamedMapReveal(const AsciiString &n) { SA_SE("undoreveal %s", n.str()); }
	void removeNamedMapReveal(const AsciiString &n) { SA_SE("removereveal %s", n.str()); }
	void doObjectTypeListMaintenance(const AsciiString &l, const AsciiString &o, Bool add) { SA_SE("objlist %s %s %d", l.str(), o.str(), add ? 1 : 0); }
	void setToppleDirection(const AsciiString &n, const Coord3D *d) { SA_SE("topple %s %08x %08x %08x", n.str(), fb(d->x), fb(d->y), fb(d->z)); }
	void setCurrentTrackName(const AsciiString &n) { SA_SE("track %s", n.str()); }
	void doFreezeTime() { SA_SE("freeze"); }
	void doUnfreezeTime() { SA_SE("unfreeze"); }
	void setChooseVictimAlwaysUsesNormal(Bool b) { SA_SE("victimnormal %d", b ? 1 : 0); }
	void setObjectsShouldReceiveDifficultyBonus(Bool b) { SA_SE("difficultybonus %d", b ? 1 : 0); }
	void startQuickEndGameTimer() { SA_SE("quickendgame"); }
	void startEndGameTimer() { SA_SE("endgame"); }
	void startCloseWindowTimer() { SA_SE("closewindow"); }
	Bool hasShownMPLocalDefeatWindow() { return (g_cfg[0] >> 8 & 1) != 0; }
	void markMPLocalDefeatWindowShown() { SA_SE("markdefeatshown"); }
#undef SA_SE
	ObjectTypes m_types;
	AttackPriorityInfo m_api;
	Script m_script;
};
extern ScriptEngine *TheScriptEngine;

class WaterHandle { public: Int m_index; };
class TerrainLogic
{
public:
	Waypoint *getWaypointByName(const AsciiString &name) { int k = saNum(name.str(), "wp"); return saWaypointExists(k) ? &g_waypoints[k] : NULL; }
	Waypoint *getFirstWaypoint() { for (int k = 0; k < SA_WAYPOINTS; ++k) if (saWaypointExists(k)) return &g_waypoints[k]; return NULL; }
	Waypoint *getClosestWaypointOnPath(const Coord3D *p, const AsciiString &label);
	Bool isPurposeOfPath(Waypoint *w, const AsciiString &label) { return w->getPathLabel1() == label || w->getPathLabel2() == label || w->getPathLabel3() == label; }
	Real getGroundHeight(Real x, Real y, Coord3D *normal = NULL) { return (Real)(saHash(SA_SEED, fb(x), fb(y)) % 16u) * 0.5f; }
	void createCraterInTerrain(Object *o) { saLog("terrain crater %d", o->getID()); }
	const WaterHandle *getWaterHandleByName(const AsciiString &n) { int k = saNum(n.str(), "water"); m_water.m_index = k; return k >= 0 && k < 4 ? &m_water : NULL; }
	void setWaterHeight(const WaterHandle *w, Real h, Real damage, Bool forcePathfind) { saLog("terrain waterheight %d %08x %08x %d", w->m_index, fb(h), fb(damage), forcePathfind ? 1 : 0); }
	void changeWaterHeightOverTime(const WaterHandle *w, Real h, Real t, Real damage) { saLog("terrain waterover %d %08x %08x %08x", w->m_index, fb(h), fb(t), fb(damage)); }
	void setActiveBoundary(Int b) { saLog("terrain boundary %d", b); }
	WaterHandle m_water;
};
extern TerrainLogic *TheTerrainLogic;

class View
{
public:
	enum CameraLockType { LOCK_FOLLOW, LOCK_TETHER };
	enum CameraShakeType { SHAKE_SUBTLE = 0, SHAKE_NORMAL, SHAKE_STRONG, SHAKE_SEVERE, SHAKE_CINE_EXTREME, SHAKE_CINE_INSANE };
#define SA_V(fmt, ...) saLog("view " fmt, ##__VA_ARGS__)
	void moveCameraTo(const Coord3D *p, Int ms, Int shutter, Bool orient, Real easeIn, Real easeOut) { SA_V("moveto %08x %08x %08x %d %d %d %08x %08x", fb(p->x), fb(p->y), fb(p->z), ms, shutter, orient ? 1 : 0, fb(easeIn), fb(easeOut)); }
	void moveCameraAlongWaypointPath(Waypoint *w, Int ms, Int shutter, Bool orient, Real easeIn, Real easeOut) { SA_V("alongpath %d %d %d %d %08x %08x", w->m_index, ms, shutter, orient ? 1 : 0, fb(easeIn), fb(easeOut)); }
	void zoomCamera(Real z, Int ms, Real easeIn, Real easeOut) { SA_V("zoom %08x %d %08x %08x", fb(z), ms, fb(easeIn), fb(easeOut)); }
	void pitchCamera(Real p, Int ms, Real easeIn, Real easeOut) { SA_V("pitch %08x %d %08x %08x", fb(p), ms, fb(easeIn), fb(easeOut)); }
	void rotateCamera(Real r, Int ms, Real easeIn, Real easeOut) { SA_V("rotate %08x %d %08x %08x", fb(r), ms, fb(easeIn), fb(easeOut)); }
	void rotateCameraTowardObject(ObjectID id, Int ms, Int hold, Real easeIn, Real easeOut) { SA_V("rotateobj %d %d %d %08x %08x", id, ms, hold, fb(easeIn), fb(easeOut)); }
	void rotateCameraTowardPosition(const Coord3D *p, Int ms, Real easeIn, Real easeOut, Bool reverse) { SA_V("rotatepos %08x %08x %08x %d %08x %08x %d", fb(p->x), fb(p->y), fb(p->z), ms, fb(easeIn), fb(easeOut), reverse ? 1 : 0); }
	void resetCamera(const Coord3D *p, Int ms, Real easeIn, Real easeOut) { SA_V("reset %08x %08x %08x %d %08x %08x", fb(p->x), fb(p->y), fb(p->z), ms, fb(easeIn), fb(easeOut)); }
	void scrollBy(Coord2D *d) { SA_V("scroll %08x %08x", fb(d->x), fb(d->y)); }
	void cameraModLookToward(Coord3D *p) { SA_V("modlook %08x %08x %08x", fb(p->x), fb(p->y), fb(p->z)); }
	void cameraModFinalLookToward(Coord3D *p) { SA_V("modfinallook %08x %08x %08x", fb(p->x), fb(p->y), fb(p->z)); }
	void cameraModFinalMoveTo(Coord3D *p) { SA_V("modfinalmove %08x %08x %08x", fb(p->x), fb(p->y), fb(p->z)); }
	void cameraModFinalPitch(Real p, Real easeIn, Real easeOut) { SA_V("modfinalpitch %08x %08x %08x", fb(p), fb(easeIn), fb(easeOut)); }
	void cameraModFinalZoom(Real z, Real easeIn, Real easeOut) { SA_V("modfinalzoom %08x %08x %08x", fb(z), fb(easeIn), fb(easeOut)); }
	void cameraModFinalTimeMultiplier(Int m) { SA_V("modtimemult %d", m); }
	void cameraModRollingAverage(Int n) { SA_V("modrolling %d", n); }
	void cameraModFreezeAngle() { SA_V("freezeangle"); }
	void cameraModFreezeTime() { SA_V("freezetime"); }
	void cameraEnableSlaveMode(const AsciiString &t, const AsciiString &b) { SA_V("slave %s %s", t.str(), b.str()); }
	void cameraDisableSlaveMode() { SA_V("unslave"); }
	void setCameraLock(ObjectID id) { SA_V("lock %d", id); }
	void snapToCameraLock() { SA_V("snaplock"); }
	void setSnapMode(CameraLockType t, Real d) { SA_V("snapmode %d %08x", (int)t, fb(d)); }
	void setDefaultView(Real pitch, Real angle, Real maxHeight) { SA_V("default %08x %08x %08x", fb(pitch), fb(angle), fb(maxHeight)); }
	void setFadeParameters(Int frames, Int dir) { SA_V("fade %d %d", frames, dir); }
	void setGuardBandBias(const Coord2D *g) { SA_V("guardband %08x %08x", fb(g->x), fb(g->y)); }
	void setTimeMultiplier(Int m) { SA_V("timemult %d", m); }
	Bool setViewFilter(enum FilterTypes f) { SA_V("filter %d", (int)f); return (saHash(SA_SEED, 150u, (unsigned)f) & 3) != 0; }
	Bool setViewFilterMode(enum FilterModes m) { SA_V("filtermode %d", (int)m); return (saHash(SA_SEED, 151u, (unsigned)m) & 3) != 0; }
	void setViewFilterPos(const Coord3D *p) { SA_V("filterpos %08x %08x %08x", fb(p->x), fb(p->y), fb(p->z)); }
	enum FilterTypes getViewFilterType() { return (enum FilterTypes)(g_cfg[15] & 3); }
	void getPosition(Coord3D *p) { p->x = 100.0f; p->y = 200.0f; p->z = 0.0f; }
	void lookAt(const Coord3D *p) { SA_V("lookat %08x %08x %08x", fb(p->x), fb(p->y), fb(p->z)); }
	void shake(const Coord3D *p, CameraShakeType t) { SA_V("shake %08x %08x %08x %d", fb(p->x), fb(p->y), fb(p->z), (int)t); }
	void Add_Camera_Shake(const Coord3D &p, Real r, Real d, Real a) { SA_V("addshake %08x %08x %08x %08x %08x %08x", fb(p.x), fb(p.y), fb(p.z), fb(r), fb(d), fb(a)); }
#undef SA_V
};
extern View *TheTacticalView;

/* the partition's searches answer from the world: see the test's host */
class PartitionFilter { public: virtual ~PartitionFilter() {} virtual int kind() const { return 0; } };
class PartitionFilterAcceptByKindOf : public PartitionFilter { public: PartitionFilterAcceptByKindOf(const KindOfMaskType &m, const KindOfMaskType &n) : m_m(m) {} int kind() const { return 1; } KindOfMaskType m_m; };
class PartitionFilterRejectByKindOf : public PartitionFilter { public: PartitionFilterRejectByKindOf(const KindOfMaskType &m, const KindOfMaskType &n) {} };
class PartitionFilterGarrisonable : public PartitionFilter { public: PartitionFilterGarrisonable(Bool b) {} };
class PartitionFilterGarrisonableByPlayer : public PartitionFilter { public: PartitionFilterGarrisonableByPlayer(Player *p, Bool a, CommandSourceType c) {} };
class PartitionFilterOnMap : public PartitionFilter {};
class PartitionFilterPlayerAffiliation : public PartitionFilter { public: PartitionFilterPlayerAffiliation(const Player *p, Int flags, Bool self) : m_p(p), m_flags(flags) {} int kind() const { return 2; } const Player *m_p; Int m_flags; };
class PartitionFilterPolygonTrigger : public PartitionFilter { public: PartitionFilterPolygonTrigger(const PolygonTrigger *t) {} };
class PartitionFilterSameMapStatus : public PartitionFilter { public: PartitionFilterSameMapStatus(const Object *o) {} };
class PartitionFilterThing : public PartitionFilter { public: PartitionFilterThing(const ThingTemplate *t, Bool match) : m_t(t) {} int kind() const { return 3; } const ThingTemplate *m_t; };
class PartitionFilterUnmannedObject : public PartitionFilter { public: PartitionFilterUnmannedObject(Bool b) {} };
class PartitionFilterValidCommandButtonTarget : public PartitionFilter { public: PartitionFilterValidCommandButtonTarget(Object *o, const CommandButton *b, Bool, CommandSourceType) {} };
class ObjectIterator : public MemoryPoolObject
{
public:
	Object *first() { m_i = 0; return next(); }
	Object *next() { return m_i < m_list.size() ? m_list[m_i++] : NULL; }
	std::vector<Object *> m_list;
	size_t m_i;
};
class SimpleObjectIterator : public ObjectIterator {};
class PartitionManager
{
public:
	SimpleObjectIterator *iterateObjectsInRange(const Coord3D *pos, Real range, DistanceCalculationType d, PartitionFilter *const filters[], IterOrderType order = ITER_FASTEST);
	SimpleObjectIterator *iterateObjectsInRange(const Object *obj, Real range, DistanceCalculationType d, PartitionFilter *const filters[], IterOrderType order = ITER_FASTEST);
	Object *getClosestObject(const Coord3D *pos, Real range, DistanceCalculationType d, PartitionFilter *const filters[], Real *closestDist = NULL, Coord3D *closestDistVec = NULL);
	Object *getClosestObject(const Object *obj, Real range, DistanceCalculationType d, PartitionFilter *const filters[], Real *closestDist = NULL, Coord3D *closestDistVec = NULL);
	Bool getNearestGroupWithValue(Int playerIndex, UnsignedInt allowed, ValueOrThreat vot, const Coord3D *src, Int value, Bool greater, Coord3D *out);
#define SA_PM(fmt, ...) saLog("partition " fmt, ##__VA_ARGS__)
	void doShroudCover(Real x, Real y, Real r, PlayerMaskType m) { SA_PM("shroudcover %08x %08x %08x %x", fb(x), fb(y), fb(r), m); }
	void undoShroudCover(Real x, Real y, Real r, PlayerMaskType m) { SA_PM("undoshroudcover %08x %08x %08x %x", fb(x), fb(y), fb(r), m); }
	void doShroudReveal(Real x, Real y, Real r, PlayerMaskType m) { SA_PM("shroudreveal %08x %08x %08x %x", fb(x), fb(y), fb(r), m); }
	void undoShroudReveal(Real x, Real y, Real r, PlayerMaskType m) { SA_PM("undoshroudreveal %08x %08x %08x %x", fb(x), fb(y), fb(r), m); }
	void refreshShroudForLocalPlayer() { SA_PM("refreshshroud"); }
	void revealMapForPlayer(Int i) { SA_PM("revealmap %d", i); }
	void revealMapForPlayerPermanently(Int i) { SA_PM("revealmapperm %d", i); }
	void undoRevealMapForPlayerPermanently(Int i) { SA_PM("undorevealmapperm %d", i); }
	void shroudMapForPlayer(Int i) { SA_PM("shroudmap %d", i); }
#undef SA_PM
};
extern PartitionManager *ThePartitionManager;

class GameLogic
{
public:
	UnsignedInt getFrame() { return g_cfg[1]; }
	Object *findObjectByID(ObjectID id) { return id >= 1 && id <= (ObjectID)g_world.objects.size() ? g_world.objects[id - 1] : NULL; }
	Object *getFirstObject() { return g_world.objects.empty() ? NULL : g_world.objects[0]; }
#define SA_GL(fmt, ...) saLog("logic " fmt, ##__VA_ARGS__)
	void closeWindows() { SA_GL("closewindows"); }
	void deselectObject(Object *o, PlayerMaskType m, Bool affectClient) { SA_GL("deselect %d %x %d", o->getID(), m, affectClient ? 1 : 0); }
	void destroyObject(Object *o) { SA_GL("destroy %d", o->getID()); }
	void enableScoring(Bool b) { SA_GL("scoring %d", b ? 1 : 0); }
	void setBuildableStatusOverride(const ThingTemplate *t, BuildableStatus s) { SA_GL("buildable %d %d", t->m_index, (int)s); }
	void setControlBarOverride(const AsciiString &cs, Int slot, const CommandButton *b);
	void setDrawIconUI(Bool b) { SA_GL("drawicon %d", b ? 1 : 0); }
	void setHulkMaxLifetimeOverride(Int f) { SA_GL("hulk %d", f); }
	void setRankLevelLimit(Int l) { SA_GL("ranklimit %d", l); }
	void setShowBehindBuildingMarkers(Bool b) { SA_GL("behindbuilding %d", b ? 1 : 0); }
	void setShowDynamicLOD(Bool b) { SA_GL("dynamiclod %d", b ? 1 : 0); }
#undef SA_GL
};
extern GameLogic *TheGameLogic;
inline Object *Object::getNextObject() const { return (size_t)m_index + 1 < g_world.objects.size() ? g_world.objects[m_index + 1] : NULL; }

class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &name, Bool check = TRUE) { int k = saNum(name.str(), "tmpl"); return k >= 0 && k < saNumTemplates() ? &g_templates[k] : NULL; }
	Object *newObject(const ThingTemplate *t, Team *team);
};
extern ThingFactory *TheThingFactory;

/* --- the control bar's buttons and sets --- */
class CommandButton
{
public:
	GUICommandType getCommandType() const { return (GUICommandType)(saHash(SA_SEED, (unsigned)m_index, 80u) % 30u); }
	AsciiString getName() const { return saName("btn", m_index); }
	UnsignedInt getOptions() const { return saHash(SA_SEED, (unsigned)m_index, 81u) & (COMMAND_OPTION_NEED_OBJECT_TARGET | 32); }
	const SpecialPowerTemplate *getSpecialPowerTemplate() const;
	Bool isValidToUseOn(const Object *src, const Object *target, const Coord3D *pos, CommandSourceType cmd) const;
	void setFlashCount(Int n) const { saLog("button %d flashcount %d", m_index, n); }
	Int m_index;
};
class CommandSet
{
public:
	const CommandButton *getCommandButton(Int i) const;
	Int m_index;
};
class ControlBar
{
public:
	const CommandButton *findCommandButton(const AsciiString &name) { int k = saNum(name.str(), "btn"); return k >= 0 && k < (int)(g_cfg[11] >> 8 & 15) ? &m_buttons[k] : NULL; }
	const CommandSet *findCommandSet(const AsciiString &name) { int k = saNum(name.str(), "cs"); if (k < 0 || k > 8) return NULL; m_set.m_index = k; return &m_set; }
	void setFlash(Bool b) { saLog("controlbar flash %d", b ? 1 : 0); }
	void deleteBuildTooltipLayout() { saLog("controlbar deletetooltip"); }
	CommandButton m_buttons[16];
	CommandSet m_set;
};
extern ControlBar *TheControlBar;

class InGameUI
{
public:
#define SA_UI(fmt, ...) saLog("ui " fmt, ##__VA_ARGS__)
	void message(const UnicodeString &s) { SA_UI("message %s", saW(s).c_str()); }
	void message(const AsciiString &s) { SA_UI("message %s", s.str()); }
	void militarySubtitle(const AsciiString &l, Int d) { SA_UI("subtitle %s %d", l.str(), d); }
	void playMovie(const AsciiString &n) { SA_UI("movie %s", n.str()); }
	void popupMessage(const AsciiString &m, Int x, Int y, Int w, Bool pause, Bool pauseMusic) { SA_UI("popup %s %d %d %d %d %d", m.str(), x, y, w, pause ? 1 : 0, pauseMusic ? 1 : 0); }
	void addNamedTimer(const AsciiString &n, const UnicodeString &t, Bool countdown) { SA_UI("addtimer %s %s %d", n.str(), saW(t).c_str(), countdown ? 1 : 0); }
	void removeNamedTimer(const AsciiString &n) { SA_UI("removetimer %s", n.str()); }
	void showNamedTimerDisplay(Bool b) { SA_UI("showtimers %d", b ? 1 : 0); }
	void clearAttackMoveToMode() { SA_UI("clearattackmove"); }
	void deselectAllDrawables(Bool post = TRUE) { SA_UI("deselectall"); }
	void selectDrawable(Drawable *d) { SA_UI("select %d", d ? d->m_obj : 0); }
	void setInputEnabled(Bool b) { SA_UI("input %d", b ? 1 : 0); }
	void setWaypointMode(Bool b) { SA_UI("waypointmode %d", b ? 1 : 0); }
	void setSuperweaponDisplayEnabledByScript(Bool b) { SA_UI("superweapondisplay %d", b ? 1 : 0); }
	void hideObjectSuperweaponDisplayByScript(const Object *o) { SA_UI("hidesuperweapon %d", o->getID()); }
	void showObjectSuperweaponDisplayByScript(const Object *o) { SA_UI("showsuperweapon %d", o->getID()); }
#undef SA_UI
};
extern InGameUI *TheInGameUI;

class AudioEventRTS
{
public:
	AudioEventRTS() : m_hasPos(FALSE), m_obj(0), m_logical(FALSE), m_player(-1), m_uninterruptable(FALSE), m_fade(FALSE) {}
	AudioEventRTS(const AsciiString &n) : m_name(n), m_hasPos(FALSE), m_obj(0), m_logical(FALSE), m_player(-1), m_uninterruptable(FALSE), m_fade(FALSE) {}
	AudioEventRTS(const AsciiString &n, const Coord3D *p) : m_name(n), m_hasPos(TRUE), m_pos(*p), m_obj(0), m_logical(FALSE), m_player(-1), m_uninterruptable(FALSE), m_fade(FALSE) {}
	AudioEventRTS(const AsciiString &n, ObjectID id) : m_name(n), m_hasPos(FALSE), m_obj(id), m_logical(FALSE), m_player(-1), m_uninterruptable(FALSE), m_fade(FALSE) {}
	void setIsLogicalAudio(Bool b) { m_logical = b; }
	void setPlayerIndex(Int i) { m_player = i; }
	void setObjectID(ObjectID id) { m_obj = id; }
	void setPosition(const Coord3D *p) { m_hasPos = TRUE; m_pos = *p; }
	void setUninterruptable(Bool b) { m_uninterruptable = b; }
	void setShouldFade(Bool b) { m_fade = b; }
	Bool m_uninterruptable, m_fade;
	void setAudioEventInfo(const void *) {}
	void setEventName(const AsciiString &n) { m_name = n; }
	AsciiString getEventName() const { return m_name; }
	AsciiString m_name;
	Bool m_hasPos;
	Coord3D m_pos;
	ObjectID m_obj;
	Bool m_logical;
	Int m_player;
};
class GameAudio
{
public:
	Int addAudioEvent(const AudioEventRTS *e)
	{
		if (e->m_hasPos) saLog("audio add %s %d %d %d %d %d %08x %08x %08x", e->m_name.str(), e->m_logical ? 1 : 0, e->m_player, e->m_obj, e->m_uninterruptable ? 1 : 0, e->m_fade ? 1 : 0, fb(e->m_pos.x), fb(e->m_pos.y), fb(e->m_pos.z));
		else saLog("audio add %s %d %d %d %d %d", e->m_name.str(), e->m_logical ? 1 : 0, e->m_player, e->m_obj, e->m_uninterruptable ? 1 : 0, e->m_fade ? 1 : 0);
		return AHSV_FirstHandle_saw;
	}
	enum { AHSV_FirstHandle_saw = 6 };
#define SA_AU(fmt, ...) saLog("audio " fmt, ##__VA_ARGS__)
	void removeAudioEvent(Int h) { SA_AU("removehandle %d", h); }
	void removeAudioEvent(const AsciiString &n) { SA_AU("remove %s", n.str()); }
	void removeDisabledEvents() { SA_AU("removedisabled"); }
	void pauseAmbient(Bool b) { SA_AU("pauseambient %d", b ? 1 : 0); }
	void pauseAudio(AudioAffect a) { SA_AU("pause %d", (int)a); }
	void resumeAudio(AudioAffect a) { SA_AU("resume %d", (int)a); }
	void setAudioEventEnabled(const AsciiString &n, Bool b) { SA_AU("enabled %s %d", n.str(), b ? 1 : 0); }
	void setAudioEventVolumeOverride(const AsciiString &n, Real v) { SA_AU("volumeoverride %s %08x", n.str(), fb(v)); }
	void setVolume(Real v, AudioAffect a) { SA_AU("volume %08x %d", fb(v), (int)a); }
#undef SA_AU
};
extern GameAudio *TheAudio;

class Radar
{
public:
	void createEvent(const Coord3D *p, RadarEventType t, Real secs = 4.0f) { saLog("radar event %08x %08x %08x %d", fb(p->x), fb(p->y), fb(p->z), (int)t); }
	void forceOn(Bool b) { saLog("radar forceon %d", b ? 1 : 0); }
	void hide(Bool b) { saLog("radar hide %d", b ? 1 : 0); }
	void refreshTerrain(TerrainLogic *) { saLog("radar refresh"); }
};
extern Radar *TheRadar;
class GameFont { public: AsciiString m_name; Int m_size; Bool m_bold; };
class Display
{
public:
	void enableLetterBox(Bool b) { saLog("display letterbox %d", b ? 1 : 0); }
	void playMovie(const AsciiString &n) { saLog("display movie %s", n.str()); }
	void setBorderShroudLevel(UnsignedByte l) { saLog("display bordershroud %d", (int)l); }
	void setCinematicFont(GameFont *f) { saLog("display font %s %d %d", f ? f->m_name.str() : "-", f ? f->m_size : 0, f && f->m_bold ? 1 : 0); }
	void setCinematicText(const AsciiString &s) { saLog("display cinematic %s", s.str()); }
	void setCinematicTextFrames(Int f) { saLog("display cinematicframes %d", f); }
};
extern Display *TheDisplay;
class CampaignManager { public: void SetVictorious(Bool b) { saLog("campaign victorious %d", b ? 1 : 0); } };
extern CampaignManager *TheCampaignManager;
class GameWindow { public: Int m_id; };
class GameWindowManager
{
public:
	GameWindow *winCreateFromScript(const AsciiString &n, void *info = NULL) { m_win.m_id = ++m_next; saLog("window create %s %d", n.str(), m_win.m_id); return &m_win; }
	Int winDestroy(GameWindow *w) { saLog("window destroy %d", w->m_id); return 0; }
	GameWindow m_win;
	Int m_next;
};
extern GameWindowManager *TheWindowManager;
class SpecialPowerStore
{
public:
	const SpecialPowerTemplate *findSpecialPowerTemplate(const AsciiString &name) { int k = saNum(name.str(), "power"); return k >= 0 && k < (int)(g_cfg[11] >> 4 & 15) ? &m_powers[k] : NULL; }
	SpecialPowerTemplate m_powers[16];
};
extern SpecialPowerStore *TheSpecialPowerStore;
class GlobalData
{
public:
	TimeOfDay m_timeOfDay;
	UnsignedByte m_clearAlpha, m_shroudAlpha;
	Int m_framesPerSecondLimit;
	Bool m_useFpsLimit;
	Real m_scriptOverrideInfantryLightScale;
	Bool m_disableMilitaryCaption, m_disableScriptedInputDisabling;
};
extern GlobalData *TheGlobalData;
extern GlobalData *TheWritableGlobalData;
/* a label's text: "T:label", or "*label" (a missing translation's marker) when the hash says so; it exists when the hash says so */
class GameText
{
public:
	UnicodeString fetch(const AsciiString &l, Bool *exists = NULL)
	{
		unsigned h = saHash(SA_SEED, saStrHash(l.str()), 140u);
		if (exists) *exists = (h & 1) != 0;
		std::string s = ((h & 2) ? "*" : "T:") + l.m_s;
		UnicodeString u; u.m_s = std::wstring(s.begin(), s.end());
		return u;
	}
	UnicodeString fetch(const char *l) { return fetch(AsciiString(l)); }
};
extern GameText *TheGameText;
class ScienceStore { public: ScienceType getScienceFromInternalName(const AsciiString &n) const { int k = saNum(n.str(), "sci"); return k >= 0 && k < 6 ? k : SCIENCE_INVALID; } };
extern ScienceStore *TheScienceStore;
class Mouse { public: void setVisibility(Bool b) { saLog("mouse visible %d", b ? 1 : 0); } };
extern Mouse *TheMouse;
class GameEngine { public: void setFramesPerSecondLimit(Int f) { saLog("engine fps %d", f); } };
extern GameEngine *TheGameEngine;
class VictoryConditionsInterface { public: Bool amIObserver() { return (g_cfg[0] >> 9 & 1) != 0; } };
extern VictoryConditionsInterface *TheVictoryConditions;
class UpgradeTemplate { public: AsciiString getUpgradeName() const { return saName("upg", m_index); } Int m_index; };
class UpgradeCenter { public: const UpgradeTemplate *findUpgrade(const AsciiString &n) { int k = saNum(n.str(), "upg"); m_u.m_index = k; return k >= 0 && k < 4 ? &m_u : NULL; } UpgradeTemplate m_u; };
extern UpgradeCenter *TheUpgradeCenter;
class SnowManager { public: void setVisible(Bool b) { saLog("snow %d", b ? 1 : 0); } };
extern SnowManager *TheSnowManager;
class LookAtTranslator { public: void resetModes() { saLog("lookat reset"); } };
extern LookAtTranslator *TheLookAtTranslator;
class GlobalLanguage { public: Int adjustFontSize(Int s) { return s + 1; } };
extern GlobalLanguage *TheGlobalLanguageData;
class GameClient
{
public:
	Drawable *firstDrawable() { for (size_t i = 0; i < g_world.objects.size(); ++i) { Drawable *d = g_world.objects[i]->getDrawable(); if (d) return d; } return NULL; }
};
inline Drawable *Drawable::getNextDrawable() const
{
	for (size_t i = (size_t)m_obj; i < g_world.objects.size(); ++i) { Drawable *d = g_world.objects[i]->getDrawable(); if (d) return d; }
	return NULL;
}
extern GameClient *TheGameClient;
class FontLibrary { public: GameFont *getFont(const AsciiString &n, Int size, Bool bold) { m_f.m_name = n; m_f.m_size = size; m_f.m_bold = bold; return &m_f; } GameFont m_f; };
extern FontLibrary *TheFontLibrary;
class Eva { public: void setEvaEnabled(Bool b) { saLog("eva %d", b ? 1 : 0); } };
extern Eva *TheEva;
extern AI *TheAI;
extern TeamFactory *TheTeamFactory;
extern const char *TheObjectFlagsNames[];
void oversizeTheTerrain(Int amount);
void HideControlBar(Bool immediate = TRUE);
void ShowControlBar(Bool immediate = TRUE);
void doSkyBoxSet(Bool startDraw);
#endif
