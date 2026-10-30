/* The world of ScriptActions.cpp, part 2 (differential/shim_actions/sa_world.h includes it): the objects, teams, players and the
** subsystems the actions reach. The objects and teams are dynamic (the factories make new ones); everything with an effect writes a
** line; the answers come from g_cfg and the world's state. tests/differential_script_actions.rs's host is the same world in Rust. */
#ifndef SA_WORLD_B_H
#define SA_WORLD_B_H
#include "../../upstream/GeneralsMD/Code/GameEngine/Include/Common/PartitionSolver.h"

class Object; class Team; class Player; class TeamPrototype; class ThingTemplate; class Drawable; class AIUpdateInterface; class AIGroup;
class Waypoint; class PolygonTrigger; class ContainModuleInterface; class BodyModuleInterface; class SpecialPowerTemplate;
class SpecialPowerModuleInterface; class CommandButton; class CommandSet; class UpgradeTemplate; class AttackPriorityInfo; class Weapon;
class ObjectTypes; class Money; class TeamTemplateInfo; class BehaviorModule; class CaveInterface; class GameWindow; class GameFont;
class WaterHandle; class DamageInfo; class SimpleObjectIterator; class ObjectIterator;
typedef void (*ContainIterateFunc)(Object *obj, void *userData);

/* the world's dynamic state */
struct SaWorld
{
	std::vector<Object *> objects;  /* index = id - 1 */
	std::vector<Team *> teams;      /* index = team id - 1 */
	int nextGroup;
};
extern SaWorld g_world;
inline int saOwner(unsigned w, int shift);

/* --- templates: "tmplK", K below cfg[11] & 7 --- */
class ThingTemplate
{
public:
	unsigned w() const { return g_cfg[24 + m_index]; }
	AsciiString getName() const { return m_name; }
	Bool isEquivalentTo(const ThingTemplate *o) const { return o && o->m_index == m_index; }
	Bool isKindOf(Int k) const { return (g_cfg[160 + m_index] >> ((unsigned)k & 31) & 1) != 0; }
	AsciiString friend_getCommandSetString() const { return saName("cs", m_index); }
	Real getMajorRadius() const { return (Real)(m_index + 1) * 2.5f; }
	Int getTransportSlotCount() const { return (Int)(w() >> 16 & 3); }
	UnsignedInt getContainMax() const { return w() >> 18 & 7; }
	Bool hasContain() const { return (w() >> 21 & 1) != 0; }
	Bool hasAI() const { return (w() >> 22 & 1) != 0; }
	Bool hasDeliverPayload() const { return (w() >> 23 & 1) != 0; }
	Int m_index;
	AsciiString m_name;
};
extern ThingTemplate g_templates[SA_TEMPLATES];
inline int saNumTemplates() { int n = (int)(g_cfg[11] & 15); return n > SA_TEMPLATES ? SA_TEMPLATES : n; }

class GeometryInfo
{
public:
	Real getMajorRadius() const { return m_r; }
	void makeRandomOffsetOnPerimeter(Coord3D &out) const { out.x = m_r; out.y = 0; out.z = 0; }
	Real m_r;
};

/* --- waypoints: "wpK", K < 8 when bit K of cfg[12]; the next one is K+1 when bit 8+K --- */
class Waypoint
{
public:
	unsigned w() const { return g_cfg[32 + m_index]; }
	const Coord3D *getLocation() const
	{
		m_loc.x = (Real)(w() & 255) * 2.5f; m_loc.y = (Real)(w() >> 8 & 255) * 2.5f; m_loc.z = (Real)(w() >> 16 & 15);
		return &m_loc;
	}
	AsciiString getName() const { return saName("wp", m_index); }
	Waypoint *getNext() const;
	Int getID() const { return m_index + 1; }
	AsciiString getPathLabel1() const { return saName("path", (int)(w() >> 20 & 7)); }
	AsciiString getPathLabel2() const { return saName("path", (int)(w() >> 23 & 7)); }
	AsciiString getPathLabel3() const { return saName("path", (int)(w() >> 26 & 7)); }
	Int getNumLinks() const { return getNext() ? 1 : 0; }
	Waypoint *getLink(Int) const { return getNext(); }
	Int m_index;
	mutable Coord3D m_loc;
};
extern Waypoint g_waypoints[SA_WAYPOINTS];
inline Bool saWaypointExists(int k) { return k >= 0 && k < SA_WAYPOINTS && (g_cfg[12] >> k & 1); }
inline Waypoint *Waypoint::getNext() const
{
	int n = m_index + 1;
	return (g_cfg[12] >> (8 + m_index) & 1) && saWaypointExists(n) ? &g_waypoints[n] : NULL;
}

/* --- trigger areas: "trigK", K < 8 when bit K of cfg[13] --- */
class PolygonTrigger
{
public:
	AsciiString getTriggerName() const { return saName("trig", m_index); }
	void getCenterPoint(Coord3D *c) const { c->x = m_index * 10.0f; c->y = m_index * 5.0f; c->z = 0.0f; }
	Real getRadius() const { return m_index * 2.0f + 1.0f; }
	Bool pointInTrigger(const struct ICoord3D &) const { return FALSE; }
	Int m_index;
};
extern PolygonTrigger g_triggers[8];

/* --- AI groups: each made one has a number; every order writes a line --- */
class AIGroup
{
public:
	void groupMoveToPosition(const Coord3D *p, Bool addWaypoint, CommandSourceType cmd) { saLog("group %d move %08x %08x %08x %d %d", m_id, fb(p->x), fb(p->y), fb(p->z), addWaypoint ? 1 : 0, (int)cmd); }
	void groupAttackTeam(const Team *t, Int maxShots, CommandSourceType cmd);
	void groupAttackObject(Object *o, Int maxShots, CommandSourceType cmd);
	void groupAttackArea(const PolygonTrigger *t, CommandSourceType cmd) { saLog("group %d attackarea %d %d", m_id, t->m_index, (int)cmd); }
	void groupAttackMoveToPosition(const Coord3D *p, Int maxShots, CommandSourceType cmd) { saLog("group %d attackmove %08x %08x %08x %d %d", m_id, fb(p->x), fb(p->y), fb(p->z), maxShots, (int)cmd); }
	void groupEnter(Object *o, CommandSourceType cmd);
	void groupEvacuate(CommandSourceType cmd) { saLog("group %d evacuate %d", m_id, (int)cmd); }
	void groupFollowWaypointPath(const Waypoint *w, CommandSourceType cmd) { saLog("group %d followpath %d %d", m_id, w->m_index, (int)cmd); }
	void groupFollowWaypointPathExact(const Waypoint *w, CommandSourceType cmd) { saLog("group %d followpathexact %d %d", m_id, w->m_index, (int)cmd); }
	void groupFollowWaypointPathAsTeam(const Waypoint *w, CommandSourceType cmd) { saLog("group %d followpathteam %d %d", m_id, w->m_index, (int)cmd); }
	void groupFollowWaypointPathAsTeamExact(const Waypoint *w, CommandSourceType cmd) { saLog("group %d followpathteamexact %d %d", m_id, w->m_index, (int)cmd); }
	void groupGuardArea(const PolygonTrigger *t, GuardMode m, CommandSourceType cmd) { saLog("group %d guardarea %d %d %d", m_id, t->m_index, (int)m, (int)cmd); }
	void groupGuardObject(Object *o, GuardMode m, CommandSourceType cmd);
	void groupGuardPosition(const Coord3D *p, GuardMode m, CommandSourceType cmd) { saLog("group %d guardpos %08x %08x %08x %d %d", m_id, fb(p->x), fb(p->y), fb(p->z), (int)m, (int)cmd); }
	void groupHunt(CommandSourceType cmd) { saLog("group %d hunt %d", m_id, (int)cmd); }
	void groupIdle(CommandSourceType cmd) { saLog("group %d idle %d", m_id, (int)cmd); }
	void groupSetEmoticon(const AsciiString &n, Int frames) { saLog("group %d emoticon %s %d", m_id, n.str(), frames); }
	void setAttitude(AttitudeType a) { saLog("group %d attitude %d", m_id, (int)a); }
	Bool getCenter(Coord3D *c);
	void groupDoCommandButton(const CommandButton *b, CommandSourceType cmd);
	void groupDoCommandButtonAtObject(const CommandButton *b, Object *o, CommandSourceType cmd);
	void groupDoCommandButtonAtPosition(const CommandButton *b, const Coord3D *p, CommandSourceType cmd);
	Object *getSpecialPowerSourceObject(UnsignedInt type);
	Object *getCommandButtonSourceObject(GUICommandType type);
	Int m_id;
	std::vector<Object *> m_members;
};
class AI
{
public:
	AIGroup *createGroup() { AIGroup *g = new AIGroup; g->m_id = ++g_world.nextGroup; m_groups.push_back(g); saLog("ai creategroup %d", g->m_id); return g; }
	class Pathfinder *pathfinder() { return m_pf; }
	std::vector<AIGroup *> m_groups;
	class Pathfinder *m_pf;
};
class Pathfinder { public: void addObjectToPathfindMap(Object *o); };

/* --- the AI of an object: every order writes a line --- */
class Locomotor { public: void setCloseEnoughDist(Real d) { saLog("loco %d closeenough %08x", m_obj, fb(d)); } void setUltraAccurate(Bool) {} Int m_obj; };
class AIUpdateInterface
{
public:
#define SA_AI(name) saLog("ai %d " name, m_obj)
	void clearWaypointQueue() { SA_AI("clearwaypoints"); }
	void chooseLocomotorSet(LocomotorSetType t) { saLog("ai %d locoset %d", m_obj, (int)t); }
	void aiMoveToPosition(const Coord3D *p, CommandSourceType cmd) { saLog("ai %d move %08x %08x %08x %d", m_obj, fb(p->x), fb(p->y), fb(p->z), (int)cmd); }
	void aiMoveToAndEvacuate(const Coord3D *p, CommandSourceType cmd) { saLog("ai %d moveevac %08x %08x %08x %d", m_obj, fb(p->x), fb(p->y), fb(p->z), (int)cmd); }
	void aiMoveToAndEvacuateAndExit(const Coord3D *p, CommandSourceType cmd) { saLog("ai %d moveevacexit %08x %08x %08x %d", m_obj, fb(p->x), fb(p->y), fb(p->z), (int)cmd); }
	void aiForceAttackObject(Object *o, Int maxShots, CommandSourceType cmd);
	void aiAttackArea(const PolygonTrigger *t, CommandSourceType cmd) { saLog("ai %d attackarea %d %d", m_obj, t->m_index, (int)cmd); }
	void aiAttackTeam(const Team *t, Int maxShots, CommandSourceType cmd);
	void aiEnter(Object *o, CommandSourceType cmd);
	void aiEvacuate(Bool exposeStealth, CommandSourceType cmd) { saLog("ai %d evacuate %d %d", m_obj, exposeStealth ? 1 : 0, (int)cmd); }
	void aiExit(Object *o, CommandSourceType cmd);
	void aiFaceObject(Object *o, CommandSourceType cmd);
	void aiFacePosition(const Coord3D *p, CommandSourceType cmd) { saLog("ai %d facepos %08x %08x %08x %d", m_obj, fb(p->x), fb(p->y), fb(p->z), (int)cmd); }
	void aiFollowWaypointPath(const Waypoint *w, CommandSourceType cmd) { saLog("ai %d followpath %d %d", m_obj, w->m_index, (int)cmd); }
	void aiFollowWaypointPathExact(const Waypoint *w, CommandSourceType cmd) { saLog("ai %d followpathexact %d %d", m_obj, w->m_index, (int)cmd); }
	void aiGuardPosition(const Coord3D *p, GuardMode m, CommandSourceType cmd) { saLog("ai %d guardpos %08x %08x %08x %d %d", m_obj, fb(p->x), fb(p->y), fb(p->z), (int)m, (int)cmd); }
	void aiGuardTunnelNetwork(GuardMode m, CommandSourceType cmd) { saLog("ai %d guardtunnel %d %d", m_obj, (int)m, (int)cmd); }
	void aiHunt(CommandSourceType cmd) { saLog("ai %d hunt %d", m_obj, (int)cmd); }
	void aiIdle(CommandSourceType cmd) { saLog("ai %d idle %d", m_obj, (int)cmd); }
	void aiMoveToObject(Object *o, CommandSourceType cmd);
	void aiPanic(const Waypoint *w, CommandSourceType cmd) { saLog("ai %d panic %d %d", m_obj, w->m_index, (int)cmd); }
	void aiWander(const Waypoint *w, CommandSourceType cmd) { saLog("ai %d wander %d %d", m_obj, w->m_index, (int)cmd); }
	void aiWanderInPlace(CommandSourceType cmd) { saLog("ai %d wanderinplace %d", m_obj, (int)cmd); }
	void setAttackInfo(const AttackPriorityInfo *info);
	void setAttitude(AttitudeType a) { saLog("ai %d attitude %d", m_obj, (int)a); }
	void setIsRecruitable(Bool b) { saLog("ai %d recruitable %d", m_obj, b ? 1 : 0); }
	Locomotor *getCurLocomotor() { if (!(saHash(SA_SEED, (unsigned)m_obj, 40u) & 1)) return NULL; m_loco.m_obj = m_obj; return &m_loco; }
	Int m_obj;
	Locomotor m_loco;
#undef SA_AI
};

/* --- the drawable of an object --- */
class Drawable
{
public:
	void enableAmbientSoundFromScript(Bool e) { saLog("draw %d ambient %d", m_obj, e ? 1 : 0); }
	Drawable *getNextDrawable() const;
	const Coord3D *getPosition() const;
	Bool isSelected() const { return (g_cfg[14] >> ((m_obj - 1) & 31) & 1) != 0; }
	void setEmoticon(const AsciiString &n, Int frames) { saLog("draw %d emoticon %s %d", m_obj, n.str(), frames); }
	void setFlashColor(Color c) { saLog("draw %d flashcolor %d", m_obj, (int)c); }
	void setFlashCount(Int n) { saLog("draw %d flashcount %d", m_obj, n); }
	void setIndicatorColor(Color c) { saLog("draw %d indicator %d", m_obj, (int)c); }
	Object *getObject() const;
	Int m_obj;
};

/* --- modules --- */
class BodyModuleInterface { public: void setIndestructible(Bool b) { saLog("body %d indestructible %d", m_obj, b ? 1 : 0); } Int m_obj; };
class ContainModuleInterface
{
public:
	void addToContain(Object *o);
	UnsignedInt getContainCount() const;
	UnsignedInt getContainMax() const;
	Bool isValidContainerFor(const Object *o, Bool checkCapacity) const;
	void iterateContained(ContainIterateFunc f, void *userData, Bool reverse);
	void removeAllContained(Bool exposeStealthUnits = FALSE);
	void setEvacDisposition(EvacDisposition d) { saLog("contain %d evacdisposition %d", m_obj, (int)d); }
	PlayerMaskType getPlayerWhoEntered() const;
	Bool isGarrisonable() const;
	Int m_obj;
};
class TransportContain : public ContainModuleInterface {};
class SpecialPowerTemplate
{
public:
	SpecialPowerType getSpecialPowerType() const { return (SpecialPowerType)(m_index + 1); }
	Real getRadiusCursorRadius() const { return (Real)(m_index * 10 + 5); }
	UnsignedInt getID() const { return (UnsignedInt)(m_index + 1); }
	AsciiString getName() const { return saName("power", m_index); }
	Int m_index;
};
class SpecialPowerModuleInterface
{
public:
	unsigned hs() const { return saHash(SA_SEED, (unsigned)m_obj, 400u + (unsigned)m_power); }
	Bool isReady() const { return (hs() & 2) != 0; }
	UnsignedInt getReadyFrame() const { return (hs() >> 8) % 2000u; }
	void setReadyFrame(UnsignedInt f) { saLog("spm %d %d readyframe %u", m_obj, m_power, f); }
	void pauseCountdown(Bool p) { saLog("spm %d %d pause %d", m_obj, m_power, p ? 1 : 0); }
	void doSpecialPowerAtLocation(const Coord3D *p, Real angle, UnsignedInt opts) { saLog("spm %d %d atloc %08x %08x %08x %08x %u", m_obj, m_power, fb(p->x), fb(p->y), fb(p->z), fb(angle), opts); }
	void doSpecialPowerAtObject(Object *o, UnsignedInt opts);
	const ThingTemplate *getReferenceThingTemplate() const { int t = (int)(hs() >> 4 & 7); return t < saNumTemplates() ? &g_templates[t] : NULL; }
	Int m_obj, m_power;
};
class CaveInterface { public: void tryToSetCaveIndex(Int i) { saLog("cave %d index %d", m_obj, i); } Int m_obj; };
class BehaviorModule { public: CaveInterface *getCaveInterface() { if (!(saHash(SA_SEED, (unsigned)m_obj, 50u) & 1)) return NULL; m_cave.m_obj = m_obj; return &m_cave; } Int m_obj; CaveInterface m_cave; };
class UpdateModule { public: virtual ~UpdateModule() {} Int m_obj; };
class DeliverPayloadAIUpdate : public UpdateModule
{
public:
	const ThingTemplate *getPutInContainerTemplateViaModuleData() const;
	void deliverPayloadViaModuleData(const Coord3D *p) { saLog("dp %d deliver %08x %08x %08x", m_obj, fb(p->x), fb(p->y), fb(p->z)); }
};
class CommandButtonHuntUpdate : public UpdateModule { public: void setCommandButton(const AsciiString &n) { saLog("cbhunt %d %s", m_obj, n.str()); } };
class StickyBombUpdate : public UpdateModule { public: void initStickyBomb(Object *target, const Object *bomber, const Coord3D *pos = NULL); };
class SupplyWarehouseDockUpdate : public UpdateModule { public: void setCashValue(Int v) { saLog("warehouse %d cash %d", m_obj, v); } };
class RailroadBehavior : public UpdateModule { public: void setHeld(Bool b) { saLog("railroad %d held %d", m_obj, b ? 1 : 0); } };
class Weapon { public: Object *forceFireWeapon(Object *src, const Coord3D *p); Int m_obj; };
class DamageInfo
{
public:
	struct In { ObjectID m_sourceID; DamageType m_damageType; DeathType m_deathType; Real m_amount; Bool m_kill; } in;
	DamageInfo() { in.m_sourceID = 0; in.m_damageType = DAMAGE_UNRESISTABLE; in.m_deathType = DEATH_NORMAL; in.m_amount = 0; in.m_kill = FALSE; }
};
class AttackPriorityInfo { public: AsciiString getName() const { return m_name; } AsciiString m_name; };

/* --- objects: the first sixteen from the configuration (id = index + 1), the rest made by the factory --- */
class Object
{
public:
	unsigned w(int k) const { return m_index < SA_OBJS0 ? g_cfg[40 + 4 * m_index + k] : m_w[k]; }
	ObjectID getID() const { return m_index + 1; }
	AsciiString getName() const { return m_name; }
	void setName(const AsciiString &n) { m_name = n; saLog("obj %d setname %s", getID(), n.str()); }
	const ThingTemplate *getTemplate() const { return m_tmpl; }
	Team *getTeam() const { return m_team; }
	void setTeam(Team *t);
	Player *getControllingPlayer() const;
	Bool isEffectivelyDead() const { return (w(0) >> 1 & 1) != 0 || m_killed; }
	Bool isKindOf(KindOfType k) const { return m_tmpl ? m_tmpl->isKindOf(k) : m_index < SA_OBJS0 && (g_cfg[168 + m_index] >> ((unsigned)k & 31) & 1) != 0; }
	const Coord3D *getPosition() const { return &m_pos; }
	void setPosition(const Coord3D *p) { m_pos = *p; saLog("obj %d setpos %08x %08x %08x", getID(), fb(p->x), fb(p->y), fb(p->z)); }
	void setOrientation(Real a) { m_angle = a; saLog("obj %d setangle %08x", getID(), fb(a)); }
	const GeometryInfo &getGeometryInfo() const { m_geom.m_r = m_tmpl ? m_tmpl->getMajorRadius() : 1.0f; return m_geom; }
	Int getTransportSlotCount() const { return m_tmpl ? m_tmpl->getTransportSlotCount() : (Int)(w(1) >> 16 & 3); }
	AIUpdateInterface *getAIUpdateInterface() const { if (!hasAI()) return NULL; m_ai.m_obj = getID(); return &m_ai; }
	AIUpdateInterface *getAI() const { return getAIUpdateInterface(); }
	Bool hasAI() const { return m_tmpl ? m_tmpl->hasAI() : (w(0) >> 4 & 1) != 0; }
	ContainModuleInterface *getContain() const { if (!(m_tmpl ? m_tmpl->hasContain() : (w(0) >> 3 & 1) != 0)) return NULL; m_contain.m_obj = getID(); return &m_contain; }
	Object *getContainedBy() const { return m_containedBy; }
	Drawable *getDrawable() const { if (!(w(0) >> 5 & 1) && m_index < SA_OBJS0) return NULL; m_draw.m_obj = getID(); return &m_draw; }
	BodyModuleInterface *getBodyModule() const { if (!(w(0) >> 2 & 1) && m_index < SA_OBJS0) return NULL; m_body.m_obj = getID(); return &m_body; }
	Bool isDisabledByType(DisabledType t) const { return t == DISABLED_HELD ? (w(0) >> 7 & 1) != 0 : t == DISABLED_UNMANNED ? (w(0) >> 8 & 1) != 0 : FALSE; }
	void setDisabled(DisabledType t) { saLog("obj %d disable %d", getID(), (int)t); }
	void setDisabledUntil(DisabledType t, UnsignedInt f) { saLog("obj %d disableuntil %d %u", getID(), (int)t, f); }
	void setScriptStatus(ObjectScriptStatusBit b, Bool v = TRUE) { saLog("obj %d scriptstatus %d %d", getID(), (int)b, v ? 1 : 0); }
	void setStatus(ObjectStatusMaskType m, Bool v = TRUE) { saLog("obj %d status %llx %d", getID(), m.m_bits, v ? 1 : 0); }
	Bool isSelectable() const { return (w(0) >> 9 & 1) != 0; }
	void setSelectable(Bool b) { saLog("obj %d selectable %d", getID(), b ? 1 : 0); }
	void leaveGroup() { saLog("obj %d leavegroup", getID()); }
	void kill(DamageType d = DAMAGE_UNRESISTABLE, DeathType t = DEATH_NORMAL) { m_killed = TRUE; saLog("obj %d kill %d %d", getID(), (int)d, (int)t); }
	void attemptDamage(DamageInfo *d) { saLog("obj %d damage %d %d %d %08x %d", getID(), d->in.m_sourceID, (int)d->in.m_damageType, (int)d->in.m_deathType, fb(d->in.m_amount), d->in.m_kill ? 1 : 0); }
	void updateUpgradeModules() { saLog("obj %d updateupgrades", getID()); }
	Color getIndicatorColor() const { return (Color)(0x100 + getID()); }
	Color getNightIndicatorColor() const { return (Color)(0x200 + getID()); }
	void setCustomIndicatorColor(Color c) { saLog("obj %d customcolor %d", getID(), (int)c); }
	void setReceivingDifficultyBonus(Bool b) { saLog("obj %d difficultybonus %d", getID(), b ? 1 : 0); }
	void giveUpgrade(const UpgradeTemplate *u);
	AsciiString getCommandSetString() const { return saName("cs", m_tmpl ? m_tmpl->m_index : 9); }
	UpdateModule *findUpdateModule(NameKeyType key) const;
	BehaviorModule **getBehaviorModules() const { m_beh.m_obj = getID(); m_behList[0] = &m_beh; m_behList[1] = NULL; return m_behList; }
	SpecialPowerModuleInterface *getSpecialPowerModule(const SpecialPowerTemplate *p) const
	{
		if (!(saHash(SA_SEED, (unsigned)getID(), 410u + (unsigned)p->m_index) & 1)) return NULL;
		m_spm.m_obj = getID(); m_spm.m_power = p->m_index; return &m_spm;
	}
	Weapon *findWaypointFollowingCapableWeapon() { if (!(saHash(SA_SEED, (unsigned)getID(), 60u) & 1)) return NULL; m_weapon.m_obj = getID(); return &m_weapon; }
	const Matrix3D *getTransformMatrix() const { m_mtx.m_angle = m_angle; m_mtx.m_pos = m_pos; return &m_mtx; }
	Object *getNextObject() const;
	void doCommandButton(const CommandButton *b, CommandSourceType cmd);
	void doCommandButtonAtObject(const CommandButton *b, Object *o, CommandSourceType cmd);
	void doCommandButtonAtPosition(const CommandButton *b, const Coord3D *p, CommandSourceType cmd);
	void doCommandButtonUsingWaypoints(const CommandButton *b, const Waypoint *w, CommandSourceType cmd);
	Int m_index;
	unsigned m_w[4];
	AsciiString m_name;
	const ThingTemplate *m_tmpl;
	Team *m_team;
	Coord3D m_pos;
	Real m_angle;
	Bool m_killed;
	Object *m_containedBy;
	std::vector<Object *> m_contained;
	mutable GeometryInfo m_geom;
	mutable AIUpdateInterface m_ai;
	mutable ContainModuleInterface m_contain;
	mutable Drawable m_draw;
	mutable BodyModuleInterface m_body;
	mutable BehaviorModule m_beh;
	mutable BehaviorModule *m_behList[2];
	mutable SpecialPowerModuleInterface m_spm;
	mutable Weapon m_weapon;
	mutable Matrix3D m_mtx;
	mutable DeliverPayloadAIUpdate m_dp;
	mutable CommandButtonHuntUpdate m_cbh;
	mutable StickyBombUpdate m_sticky;
	mutable SupplyWarehouseDockUpdate m_ware;
};
inline Object *Drawable::getObject() const { return g_world.objects[m_obj - 1]; }
inline const Coord3D *Drawable::getPosition() const { return getObject()->getPosition(); }

template <class T> class DLINK_ITERATOR
{
public:
	DLINK_ITERATOR(const std::vector<T *> *v) : m_v(*v), m_i(0) {}
	Bool done() const { return m_i >= m_v.size(); }
	void advance() { ++m_i; }
	T *cur() const { return m_i < m_v.size() ? m_v[m_i] : NULL; }  /* the real list's cursor is NULL at the end */
	std::vector<T *> m_v;  /* a copy: the list can change while it is walked */
	size_t m_i;
};

/* --- teams: sixteen from the configuration (id = index + 1), the rest made by the factory; prototypes "teamK" --- */
struct TeamUnitInfo { AsciiString unitThingName; Int maxUnits; Int minUnits; };
class TeamTemplateInfo
{
public:
	AsciiString m_startReinforceWaypoint;
	AsciiString m_transportUnitType;
	TeamUnitInfo m_unitsInfo[3];
	Int m_numUnitsInfo;
	Bool m_teamStartsFull;
	Bool m_transportsExit;
	Int m_productionPriority;
};
class TeamPrototype
{
public:
	unsigned w() const { return g_cfg[96 + m_index]; }
	DLINK_ITERATOR<Team> iterate_TeamInstanceList() { return DLINK_ITERATOR<Team>(&m_instances); }
	const TeamTemplateInfo *getTemplateInfo() const;
	Player *getControllingPlayer() const;
	AsciiString getName() const { return saName("team", m_index); }
	void increaseAIPriorityForSuccess() const { saLog("proto %d prioritysuccess", m_index); }
	void decreaseAIPriorityForFailure() const { saLog("proto %d priorityfailure", m_index); }
	Int m_index;
	std::vector<Team *> m_instances;
	mutable TeamTemplateInfo m_info;
};
extern TeamPrototype g_protos[SA_PROTOS];
class Team
{
public:
	unsigned w() const { return m_index < SA_TEAMS0 ? g_cfg[64 + m_index] : m_w; }
	Int getID() const { return m_index + 1; }
	AsciiString getName() const { return m_proto ? m_proto->getName() : saName("lone", m_index); }
	TeamPrototype *getPrototype() const { return m_proto; }
	Player *getControllingPlayer() const;
	void setControllingPlayer(Player *p);
	Bool hasAnyObjects() const { return (w() & 1) != 0; }
	Bool hasAnyUnits() const { return (w() >> 1 & 1) != 0; }
	DLINK_ITERATOR<Object> iterate_TeamMemberList() { return DLINK_ITERATOR<Object>(&m_members); }
	void iterateObjects(ContainIterateFunc f, void *userData) { std::vector<Object *> m = m_members; for (size_t i = 0; i < m.size(); ++i) f(m[i], userData); }
	void getTeamAsAIGroup(AIGroup *g) { std::string ids; for (size_t i = 0; i < m_members.size(); ++i) { char b[16]; snprintf(b, sizeof(b), " %d", m_members[i]->getID()); ids += b; } g->m_members = m_members; saLog("team %d asgroup %d%s", getID(), g->m_id, ids.c_str()); }
	void damageTeamMembers(Real amount) { saLog("team %d damage %08x", getID(), fb(amount)); }
	void setState(const AsciiString &s) { saLog("team %d state %s", getID(), s.str()); }
	void setActive() { saLog("team %d active", getID()); }
	void deleteTeam(Bool ignoreDead = FALSE) { saLog("team %d delete %d", getID(), ignoreDead ? 1 : 0); }
	void killTeam() { saLog("team %d kill", getID()); }
	void setRecruitable(Bool b) { saLog("team %d recruitable %d", getID(), b ? 1 : 0); }
	void setAttackPriorityName(const AsciiString &n) { saLog("team %d attackpriority %s", getID(), n.str()); }
	const Coord3D *getEstimateTeamPosition() const;
	void setOverrideTeamRelationship(Int id, Relationship r) { saLog("team %d overrideteam %d %d", getID(), id, (int)r); }
	Bool removeOverrideTeamRelationship(Int id) { saLog("team %d removeoverrideteam %d", getID(), id); return TRUE; }
	void setOverridePlayerRelationship(Int idx, Relationship r) { saLog("team %d overrideplayer %d %d", getID(), idx, (int)r); }
	Bool removeOverridePlayerRelationship(Int idx) { saLog("team %d removeoverrideplayer %d", getID(), idx); return TRUE; }
	Int m_index;
	unsigned m_w;
	TeamPrototype *m_proto;
	std::vector<Object *> m_members;
	Player *m_controller;  /* NULL: the prototype's owner */
	mutable Coord3D m_est;
};
inline void Object::setTeam(Team *t)
{
	if (m_team) { std::vector<Object *> &m = m_team->m_members; for (size_t i = 0; i < m.size(); ++i) if (m[i] == this) { m.erase(m.begin() + i); break; } }
	m_team = t;
	if (t) t->m_members.push_back(this);
	saLog("obj %d setteam %d", getID(), t ? t->getID() : 0);
}
inline const Coord3D *Team::getEstimateTeamPosition() const
{
	if (m_members.empty()) return NULL;
	m_est = *m_members[0]->getPosition();
	return &m_est;
}

class TeamFactory
{
public:
	TeamPrototype *findTeamPrototype(const AsciiString &name) { int k = saNum(name.str(), "team"); return k >= 0 && k < SA_PROTOS ? &g_protos[k] : NULL; }
	Team *findTeam(const AsciiString &name) { TeamPrototype *p = findTeamPrototype(name); return p && !p->m_instances.empty() ? p->m_instances[0] : NULL; }
	Team *createInactiveTeam(const AsciiString &name) { return make(name, "createinactive"); }
	Team *createTeam(const AsciiString &name) { return make(name, "create"); }
	Team *make(const AsciiString &name, const char *how);
};

/* --- players --- */
class Money
{
public:
	UnsignedInt countMoney() const { return m_amount; }
	void deposit(UnsignedInt a, Bool reason = TRUE) { m_amount += a; saLog("money %d deposit %u", m_player, a); }
	UnsignedInt withdraw(UnsignedInt a, Bool reason = TRUE) { if (a > m_amount) a = m_amount; m_amount -= a; saLog("money %d withdraw %u", m_player, a); return a; }
	UnsignedInt m_amount;
	Int m_player;
};
class Player
{
public:
	typedef std::list<TeamPrototype *> PlayerTeamList;
	unsigned w(int k) const { return g_cfg[128 + 4 * m_index + k]; }
	Int getPlayerIndex() const { return m_index; }
	PlayerMaskType getPlayerMask() const { return 1u << m_index; }
	PlayerType getPlayerType() const { return (w(0) & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN; }
	Bool isPlayerObserver() const { return (w(0) >> 1 & 1) != 0; }
	Money *getMoney() { return &m_money; }
	Team *getDefaultTeam() const;
	const PlayerTeamList *getPlayerTeams() const { return &m_teams; }
	Int getMpStartIndex() const { return (Int)(w(0) >> 8 & 15); }
	Int getRankLevel() const { return (Int)(w(0) >> 12 & 7); }
	AsciiString getPlayerNameKeyString() const { return saName("player", m_index); }
	ScienceAvailabilityType getScienceAvailabilityTypeFromString(const AsciiString &s)
	{
		if (s == "Available") return SCIENCE_AVAILABLE; if (s == "Disabled") return SCIENCE_DISABLED; if (s == "Hidden") return SCIENCE_HIDDEN;
		return SCIENCE_AVAILABILITY_INVALID;
	}
#define SA_P(fmt, ...) saLog("player %d " fmt, m_index, ##__VA_ARGS__)
	void addSkillPoints(Int n) { SA_P("addskill %d", n); }
	Bool attemptToPurchaseScience(ScienceType s) { SA_P("purchasescience %d", s); return TRUE; }
	void buildBaseDefense(Bool flank) { SA_P("basedefense %d", flank ? 1 : 0); }
	void buildBaseDefenseStructure(const AsciiString &n, Bool flank) { SA_P("basestructure %s %d", n.str(), flank ? 1 : 0); }
	void buildBySupplies(Int cash, const AsciiString &n) { SA_P("bysupplies %d %s", cash, n.str()); }
	void buildSpecificBuilding(const AsciiString &n) { SA_P("building %s", n.str()); }
	void buildSpecificBuildingNearestTeam(const AsciiString &n, const Team *t) { SA_P("buildingnearest %s %d", n.str(), t->getID()); }
	void buildSpecificTeam(TeamPrototype *p) { SA_P("buildteam %d", p->m_index); }
	void buildUpgrade(const AsciiString &n) { SA_P("upgrade %s", n.str()); }
	Bool calcClosestConstructionZoneLocation(const ThingTemplate *t, Coord3D *p);
	void checkBridges(Object *o, Waypoint *w) { SA_P("checkbridges %d %d", o->getID(), w->m_index); }
	Bool computeSuperweaponTarget(const SpecialPowerTemplate *p, Coord3D *pos, Int playerIndex, Real weight);
	void friend_setSkillset(Int n) { SA_P("skillset %d", n); }
	void garrisonAllUnits(CommandSourceType cmd) { SA_P("garrisonall %d", (int)cmd); }
	void ungarrisonAllUnits(CommandSourceType cmd) { SA_P("ungarrisonall %d", (int)cmd); }
	void grantScience(ScienceType s) { SA_P("grantscience %d", s); }
	void guardSupplyCenter(Team *t, Int minSupplies) { SA_P("guardsupply %d %d", t->getID(), minSupplies); }
	void killPlayer() { SA_P("kill"); }
	void recruitSpecificTeam(TeamPrototype *p, Real radius) { SA_P("recruit %d %08x", p->m_index, fb(radius)); }
	void removeTeamRelationship(const Team *t) { SA_P("removeteamrel %d", t->getID()); }
	void repairStructure(ObjectID id) { SA_P("repair %d", id); }
	void sellEverythingUnderTheSun() { SA_P("selleverything"); }
	void setCanBuildBase(Bool b) { SA_P("canbuildbase %d", b ? 1 : 0); }
	void setCanBuildUnits(Bool b) { SA_P("canbuildunits %d", b ? 1 : 0); }
	void setListInScoreScreen(Bool b) { SA_P("scorescreen %d", b ? 1 : 0); }
	void setObjectsEnabled(AsciiString t, Bool b) { SA_P("objectsenabled %s %d", t.str(), b ? 1 : 0); }
	void setPlayerRelationship(const Player *p, Relationship r) { SA_P("playerrel %d %d", p->m_index, (int)r); }
	void setRankLevel(Int n) { SA_P("ranklevel %d", n); }
	void setScienceAvailability(ScienceType s, ScienceAvailabilityType a) { SA_P("scienceavail %d %d", s, (int)a); }
	void setSkillPointsModifier(Real m) { SA_P("skillmod %08x", fb(m)); }
	void setTeamDelaySeconds(Int n) { SA_P("teamdelay %d", n); }
	void setTeamRelationship(const Team *t, Relationship r) { SA_P("teamrel %d %d", t->getID(), (int)r); }
	void setUnitsShouldHunt(Bool b, CommandSourceType cmd) { SA_P("hunt %d %d", b ? 1 : 0, (int)cmd); }
	void setUnitsShouldIdleOrResume(Bool idle) { SA_P("idleorresume %d", idle ? 1 : 0); }
	void transferAssetsFromThat(Player *p) { SA_P("transferfrom %d", p->m_index); }
#undef SA_P
	Int m_index;
	Money m_money;
	PlayerTeamList m_teams;
};
extern Player g_players[SA_PLAYERS];
inline int saNumPlayers() { int n = (int)(g_cfg[4] & 15); return n > SA_PLAYERS ? SA_PLAYERS : n; }
inline Player *saPlayer(int p) { return p >= 0 && p < saNumPlayers() ? &g_players[p] : NULL; }
inline Player *Object::getControllingPlayer() const { int p = (int)(w(0) >> 16 & 15); return m_index < SA_OBJS0 ? saPlayer(p) : (m_team ? m_team->getControllingPlayer() : NULL); }
inline Player *TeamPrototype::getControllingPlayer() const { return saPlayer((int)(w() & 15)); }
inline Player *Team::getControllingPlayer() const { return m_controller ? m_controller : (m_proto ? m_proto->getControllingPlayer() : saPlayer((int)(w() >> 8 & 15))); }
inline void Team::setControllingPlayer(Player *p) { m_controller = p; saLog("team %d controller %d", getID(), p ? p->m_index : -1); }
inline Team *Player::getDefaultTeam() const { unsigned t = w(1) & 255; return t < g_world.teams.size() ? g_world.teams[t] : NULL; }
class PlayerList
{
public:
	Player *getLocalPlayer() { return saPlayer((int)(g_cfg[4] >> 4 & 15)); }
	Player *getNeutralPlayer() { return saPlayer((int)(g_cfg[4] >> 8 & 15)); }
	Player *getNthPlayer(Int i) { return saPlayer(i); }
	Int getPlayerCount() { return saNumPlayers(); }
	Player *findPlayerWithNameKey(NameKeyType key) { for (int i = 0; i < saNumPlayers(); ++i) if (NAMEKEY(saName("player", i)) == key) return &g_players[i]; return NULL; }
};
extern PlayerList *ThePlayerList;
#include "sa_world_c.h"
#endif
