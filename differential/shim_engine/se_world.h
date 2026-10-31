/* Stand-in world for ScriptEngine.cpp, the ScriptEngine fixture only (differential tests, p1w13-sonnet, layer 11 batch E2): every
** header ScriptEngine.cpp includes, but GameLogic/ScriptEngine.h itself, is this one file. The world answers from the configuration
** g_cfg and its own state (layout in differential/wrap_engine.cpp); every call with an effect writes a line (floats as their bits).
** The script classes (Parameter .. ScriptList) are stand-ins with the accessors the engine uses, the conditions and actions of
** TheScriptConditions and TheScriptActions stand-ins that answer from a hash and call back into the engine as the real ones do. */
#ifndef SE_WORLD_H
#define SE_WORLD_H
typedef int Int;
typedef unsigned int UnsignedInt;
typedef bool Bool;
typedef float Real;
typedef char Char;
typedef wchar_t WideChar;
typedef short Short;
typedef unsigned short UnsignedShort;
typedef unsigned char UnsignedByte;
typedef Int ObjectID;
typedef UnsignedInt PlayerMaskType;
typedef Int NameKeyType;
typedef Int Color;
typedef Int ScienceType;
typedef Int TeamID;
#define TEAM_ID_INVALID 0
enum { SC_INVALID_DATA = 0xBAD };
typedef std::vector<ScienceType> ScienceVec;
typedef void *HMODULE;
typedef void (*FARPROC)();
#define TRUE true
#define FALSE false
#ifndef NULL
#define NULL 0
#endif
#define NEW new
#define DEBUG_LOG(x)
#define DEBUG_ASSERTCRASH(c, x)
#define DEBUG_CRASH(x)
#define INVALID_ID 0
#define LOGICFRAMES_PER_SECOND 30
#define MSEC_PER_LOGICFRAME_REAL (1000.0f / 30.0f)
#define PI 3.14159265359f
#define MAX_PLAYER_COUNT 16
#define DECLARE_PERF_TIMER(x)
#define USE_PERF_TIMER(x)
#define EMPTY_DTOR(cls) inline cls::~cls() {}
enum GameDifficulty { DIFFICULTY_EASY, DIFFICULTY_NORMAL, DIFFICULTY_HARD };
enum PlayerType { PLAYER_HUMAN = 0, PLAYER_COMPUTER };
typedef Int KindOfType;
enum { SE_CFG = 256, SE_PLAYERS = 8, SE_PROTOS = 8, SE_TEAMS = 8, SE_OBJS = 16, SE_TEMPLATES = 8 };
extern unsigned g_cfg[SE_CFG];
extern unsigned g_frame;
void seLog(const char *fmt, ...);
inline unsigned seHash(unsigned seed, unsigned a, unsigned b)
{
	unsigned x = seed ^ (a * 0x9E3779B1u) ^ (b * 0x85EBCA77u);
	x ^= x >> 15; x *= 0x2C1B3C6Du; x ^= x >> 12; x *= 0x297A2D39u; x ^= x >> 15;
	return x;
}
inline unsigned seStrHash(const char *s) { unsigned h = 2166136261u; for (; *s; ++s) { h ^= (unsigned char)*s; h *= 16777619u; } return h; }
inline unsigned fb(Real r) { unsigned u; memcpy(&u, &r, 4); return u; }
inline int seNum(const char *s, const char *prefix)
{
	size_t n = strlen(prefix);
	if (strncmp(s, prefix, n) != 0 || !s[n]) return -1;
	int v = 0;
	for (const char *p = s + n; *p; ++p) { if (*p < '0' || *p > '9' || v > 1000) return -1; v = v * 10 + (*p - '0'); }
	return v;
}
#define SE_SEED g_cfg[7]

class AsciiString
{
public:
	AsciiString() {}
	AsciiString(const char *s) : m_s(s ? s : "") {}
	const char *str() const { return m_s.c_str(); }
	Bool isEmpty() const { return m_s.empty(); }
	Bool isNotEmpty() const { return !m_s.empty(); }
	Int getLength() const { return (Int)m_s.size(); }
	void clear() { m_s.clear(); }
	void concat(const char *s) { m_s += s; }
	void concat(const AsciiString &s) { m_s += s.m_s; }
	int compare(const AsciiString &o) const { return strcmp(str(), o.str()); }
	int compare(const char *o) const { return strcmp(str(), o); }
	void format(const char *fmt, ...) { char b[512]; va_list ap; va_start(ap, fmt); vsnprintf(b, sizeof(b), fmt, ap); va_end(ap); m_s = b; }
	Bool operator==(const AsciiString &o) const { return m_s == o.m_s; }
	Bool operator!=(const AsciiString &o) const { return m_s != o.m_s; }
	Bool operator==(const char *o) const { return m_s == o; }
	Bool operator!=(const char *o) const { return m_s != o; }
	Bool operator<(const AsciiString &o) const { return m_s < o.m_s; }
	static AsciiString TheEmptyString;
	std::string m_s;
};
class UnicodeString { public: std::wstring m_s; };
inline AsciiString seName(const char *prefix, int n) { char b[32]; snprintf(b, sizeof(b), "%s%d", prefix, n); return AsciiString(b); }
struct Coord2D { Real x, y; };
struct Coord3D
{
	Real x, y, z;
	Real length() const { return (Real)sqrt(x * x + y * y + z * z); }
	void normalize() { Real len = length(); if (len != 0.0f) { x /= len; y /= len; z /= len; } }
};
inline Real Sin(Real x) { return sinf(x); }
inline Real Cos(Real x) { return cosf(x); }
extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
float ref_fast_float_ceil(float f);
}
/* as BaseType.h's, on the reference fast_float functions (differential/wrap_base_type.cpp) */
inline Int REAL_TO_INT_CEIL(Real x) { return ref_fast_float2long_round(ref_fast_float_ceil(x)); }
inline UnsignedInt REAL_TO_UNSIGNEDINT(Real x) { return (UnsignedInt)ref_fast_float2long_round(ref_fast_float_trunc(x)); }
inline Real ConvertDurationFromMsecsToFrames(Real msec) { return msec / MSEC_PER_LOGICFRAME_REAL; }
Int GameLogicRandomValue(Int lo, Int hi);

class NameKeyGenerator { public: NameKeyType nameToKey(const AsciiString &s) { return (NameKeyType)(seStrHash(s.str()) & 0x7fffffff); } };
extern NameKeyGenerator *TheNameKeyGenerator;
#define NAMEKEY(s) TheNameKeyGenerator->nameToKey(s)

class MemoryPoolObject
{
public:
	virtual ~MemoryPoolObject() {}
	void deleteInstance() { if (this) delete this; }
};
#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(cls, name) public: virtual ~cls(); public:
#define newInstance(cls) new cls
class SubsystemInterface { public: virtual ~SubsystemInterface() {} virtual void init() = 0; virtual void reset() = 0; virtual void update() = 0; };
class Xfer;
class Snapshot { public: virtual ~Snapshot() {} virtual void crc(Xfer *) = 0; virtual void xfer(Xfer *) = 0; virtual void loadPostProcess() = 0; };
typedef UnsignedByte XferVersion;
enum XferMode { XFER_INVALID = 0, XFER_SAVE, XFER_LOAD, XFER_CRC };
/* save and load are batch E4's; until then the calls do nothing */
class Xfer
{
public:
	XferMode getXferMode() const { return XFER_SAVE; }
	void xferVersion(XferVersion *, XferVersion) {}
	void xferAsciiString(AsciiString *) {}
	void xferUnsignedShort(UnsignedShort *) {}
	void xferShort(Short *) {}
	void xferInt(Int *) {}
	void xferUnsignedInt(UnsignedInt *) {}
	void xferReal(Real *) {}
	void xferBool(Bool *) {}
	void xferObjectID(ObjectID *) {}
	void xferCoord3D(Coord3D *) {}
	void xferUser(void *, Int) {}
	void xferSnapshot(Snapshot *) {}
	void xferScienceVec(ScienceVec *) {}
};
class INI;
struct FieldParse { const char *token; void (*parse)(INI *, void *, void *, const void *); const void *userData; int offset; };
class INI { public: static void parseAsciiString(INI *, void *, void *, const void *) {} void initFromINI(void *, const FieldParse *) {} };
typedef std::pair<AsciiString, class Object *> NamedRequest;
typedef std::vector<NamedRequest> VecNamedRequests;
typedef std::vector<NamedRequest>::iterator VecNamedRequestsIt;
template <class T> class LatchRestore
{
public:
	LatchRestore(T &v, const T &n) : m_v(v), m_old(v) { m_v = n; }
	~LatchRestore() { m_v = m_old; }
	T &m_v; T m_old;
};

/* ---- the scripts (Scripts.h) ---- */
#define THIS_TEAM "<This Team>"
#define THIS_OBJECT "<This Object>"
#define THIS_PLAYER "<This Player>"
#define LOCAL_PLAYER "<Local Player>"
#define THE_PLAYER "ThePlayer"
#define TEAM_THE_PLAYER "teamThePlayer"
#define THIS_PLAYER_ENEMY "<This Player's Enemy>"
#define MY_INNER_PERIMETER "[Skirmish]MyInnerPerimeter"
#define MY_OUTER_PERIMETER "[Skirmish]MyOuterPerimeter"
#define ENEMY_OUTER_PERIMETER "[Skirmish]EnemyOuterPerimeter"
#define ENEMY_INNER_PERIMETER "[Skirmish]EnemyInnerPerimeter"
#define INNER_PERIMETER "InnerPerimeter"
#define OUTER_PERIMETER "OuterPerimeter"
#include "se_enums.h"
enum { MAX_PARMS = 12 };
class Parameter
{
public:
	enum ParameterType { SE_PARAMETER_TYPES };
	enum { LESS_THAN = 0, LESS_EQUAL, EQUAL, GREATER_EQUAL, GREATER, NOT_EQUAL };
	Int getInt() const { return m_int; }
	Real getReal() const { return m_real; }
	AsciiString getString() const { return m_string; }
	void friend_setInt(Int i) { m_int = i; }
	ParameterType getParameterType() const { return (ParameterType)m_type; }
	Int m_type, m_int; Real m_real; AsciiString m_string;
};
class Condition
{
public:
	enum ConditionType { SE_CONDITION_TYPES };
	Int getConditionType() const { return m_type; }
	Int getNumParameters() const { return m_numParms; }
	Parameter *getParameter(Int i) { return i >= 0 && i < m_numParms ? m_parms[i] : NULL; }
	Condition *getNext() { return m_next; }
	Int m_type, m_numParms; Parameter *m_parms[MAX_PARMS]; Condition *m_next; Int m_customData;
};
class OrCondition
{
public:
	Condition *getFirstAndCondition() { return m_first; }
	OrCondition *getNextOrCondition() { return m_next; }
	Condition *m_first; OrCondition *m_next;
};
class ScriptAction
{
public:
	enum ScriptActionType { SE_ACTION_TYPES };
	ScriptAction() {}
	ScriptAction(Int t) : m_type(t), m_numParms(0), m_next(NULL) {}
	Int getActionType() const { return m_type; }
	Int getNumParameters() const { return m_numParms; }
	Parameter *getParameter(Int i) { return i >= 0 && i < m_numParms ? m_parms[i] : NULL; }
	ScriptAction *getNext() { return m_next; }
	void setNextAction(ScriptAction *a) { m_next = a; }
	AsciiString getUiText() { return AsciiString("ui"); }
	Int m_type, m_numParms; Parameter *m_parms[MAX_PARMS]; ScriptAction *m_next;
};
class Script
{
public:
	AsciiString getName() const { return m_name; }
	Bool isActive() const { return m_active; }
	void setActive(Bool b) { m_active = b; }
	Bool isOneShot() const { return m_oneShot; }
	Bool isSubroutine() const { return m_subroutine; }
	Bool isEasy() const { return m_easy; }
	Bool isNormal() const { return m_normal; }
	Bool isHard() const { return m_hard; }
	Int getDelayEvalSeconds() const { return m_delay; }
	UnsignedInt getFrameToEvaluate() const { return m_frameToEvaluate; }
	void setFrameToEvaluate(UnsignedInt f) { m_frameToEvaluate = f; }
	AsciiString getConditionTeamName() const { return m_conditionTeamName; }
	void setConditionTeamName(const AsciiString &n) { m_conditionTeamName = n; }
	OrCondition *getOrCondition() { return m_or; }
	ScriptAction *getAction() { return m_action; }
	ScriptAction *getFalseAction() { return m_falseAction; }
	Script *getNext() { return m_next; }
	void setCurTime(Real t) { m_curTime = t; }
	Real getCurTime() const { return m_curTime; }
	AsciiString m_name, m_conditionTeamName;
	Bool m_active, m_oneShot, m_subroutine, m_easy, m_normal, m_hard;
	Int m_delay; UnsignedInt m_frameToEvaluate; Real m_curTime;
	OrCondition *m_or; ScriptAction *m_action, *m_falseAction; Script *m_next;
};
class ScriptGroup
{
public:
	AsciiString getName() const { return m_name; }
	Bool isActive() const { return m_active; }
	void setActive(Bool b) { m_active = b; }
	Bool isSubroutine() const { return m_subroutine; }
	Script *getScript() { return m_first; }
	ScriptGroup *getNext() { return m_next; }
	AsciiString m_name; Bool m_active, m_subroutine; Script *m_first; ScriptGroup *m_next;
};
class ScriptList
{
public:
	Script *getScript() { return m_first; }
	ScriptGroup *getScriptGroup() { return m_groups; }
	static void reset() { seLog("scriptlist reset"); }
	Script *m_first; ScriptGroup *m_groups;
};
class Template
{
public:
	Template() : m_numUiStrings(0), m_numParameters(0) {}
	AsciiString getName() const { return m_uiName; }
	AsciiString m_uiName, m_uiName2, m_internalName, m_helpText;
	NameKeyType m_internalNameKey;
	Int m_numUiStrings; AsciiString m_uiStrings[MAX_PARMS];
	Int m_numParameters; Parameter::ParameterType m_parameters[MAX_PARMS];
};
class ConditionTemplate : public Template {};
class ActionTemplate : public Template {};
struct TScriptListReadInfo;

/* ---- players, teams, objects ---- */
class ParticleSystem;
class ParticleSystemTemplate;
class File;
class Team;
class Object;
class Player
{
public:
	unsigned w() const { return g_cfg[16 + 2 * m_index]; }
	Int getPlayerIndex() const { return m_index; }
	PlayerMaskType getPlayerMask() const { return 1u << m_index; }
	PlayerType getPlayerType() const { return (w() & 1) ? PLAYER_HUMAN : PLAYER_COMPUTER; }
	GameDifficulty getPlayerDifficulty() const { return (GameDifficulty)((w() >> 1 & 3) % 3); }
	Bool isPlayerActive() const { return (w() >> 4 & 1) != 0; }
	Bool isSkirmishAIPlayer() const { return (w() >> 5 & 1) != 0; }
	Int getMpStartIndex() const { return (Int)(w() >> 8 & 15); }
	Player *getCurrentEnemy();
	NameKeyType getPlayerNameKey() const { return NAMEKEY(seName("player", m_index)); }
	Team *getDefaultTeam();
	Int m_index;
};
class PlayerList
{
public:
	Int getPlayerCount() const;
	Player *getNthPlayer(Int i);
	Player *getLocalPlayer();
	Player *findPlayerWithNameKey(NameKeyType key);
	void updateTeamStates() { seLog("playerlist updateteamstates"); }
};
extern PlayerList *ThePlayerList;
class AIGroup
{
public:
	Bool isIdle() const { return (seHash(SE_SEED, 300 + m_team, g_frame) & 1) != 0; }
	Bool isGroupAiDead() const { return (seHash(SE_SEED, 400 + m_team, g_frame) & 7) == 0; }
	Int m_id, m_team;
};
class AI { public: AIGroup *createGroup(); };
extern AI *TheAI;
struct TeamTemplateInfo { Int m_maxInstances; };
class TeamPrototype;
class Team
{
public:
	unsigned w() const { return g_cfg[40 + m_index]; }
	Int getID() const { return m_index + 1; }
	AsciiString getName() const;
	Bool isActive() const { return (w() & 1) != 0; }
	Player *getControllingPlayer() const;
	void getTeamAsAIGroup(AIGroup *g) { g->m_team = getID(); seLog("team %d asgroup %d", getID(), g->m_id); }
	Int m_index;
};
template <class T> class DLINK_ITERATOR
{
public:
	DLINK_ITERATOR(const std::vector<T *> &v) : m_v(v), m_i(0) {}
	Bool done() const { return m_i >= m_v.size(); }
	void advance() { ++m_i; }
	T *cur() const { return done() ? NULL : m_v[m_i]; }
	std::vector<T *> m_v; size_t m_i;
};
class TeamPrototype
{
public:
	unsigned w() const { return g_cfg[32 + m_index]; }
	Bool getIsSingleton() const { return (w() & 1) != 0; }
	const TeamTemplateInfo *getTemplateInfo() { m_info.m_maxInstances = (Int)(w() >> 1 & 7); return &m_info; }
	std::vector<Team *> instances() const;
	Team *getFirstItemIn_TeamInstanceList() const { std::vector<Team *> v = instances(); return v.empty() ? NULL : v[0]; }
	Int countTeamInstances() const { return (Int)instances().size(); }
	DLINK_ITERATOR<Team> iterate_TeamInstanceList() const { return DLINK_ITERATOR<Team>(instances()); }
	Int m_index; TeamTemplateInfo m_info;
};
class TeamFactory { public: TeamPrototype *findTeamPrototype(const AsciiString &name); Team *findTeamByID(TeamID id); };
extern TeamFactory *TheTeamFactory;
class AIUpdateInterface
{
public:
	Bool isIdle() const { return (seHash(SE_SEED, 200 + m_obj, g_frame) & 1) != 0; }
	Int m_obj;
};
class ThingTemplate
{
public:
	AsciiString getName() const { return seName("tmpl", m_index); }
	const ThingTemplate *getFinalOverride() const;
	Bool isKindOf(KindOfType k) const { return (seHash(SE_SEED, 500 + m_index, (unsigned)k) & 1) != 0; }
	const ThingTemplate *friend_getNextTemplate() const;
	Int m_index;
};
class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &name);
	const ThingTemplate *firstTemplate();
};
extern ThingFactory *TheThingFactory;
class Object
{
public:
	unsigned w() const { return g_cfg[64 + m_index]; }
	ObjectID getID() const { return m_index + 1; }
	AsciiString getName() const { return m_name; }
	void setName(const AsciiString &n) { m_name = n; seLog("obj %d name %s", getID(), n.str()); }
	const ThingTemplate *getTemplate() const;
	Player *getControllingPlayer() const;
	AIUpdateInterface *getAIUpdateInterface() { m_ai.m_obj = getID(); return (w() >> 1 & 1) ? &m_ai : NULL; }
	Bool isEffectivelyDead() const { return (seHash(SE_SEED, 600 + getID(), g_frame) & 7) == 0; }
	Bool hasCustomIndicatorColor() const { return m_hasColor; }
	Color getIndicatorColor() const { return m_color; }
	void setCustomIndicatorColor(Color c) { m_hasColor = true; m_color = c; seLog("obj %d color %d", getID(), c); }
	void removeCustomIndicatorColor() { m_hasColor = false; seLog("obj %d nocolor", getID()); }
	Object *getNextObject();
	Int m_index; AsciiString m_name; Bool m_hasColor; Color m_color; AIUpdateInterface m_ai;
};
class GameLogic
{
public:
	UnsignedInt getFrame() const { return g_frame; }
	Object *findObjectByID(ObjectID id);
	Object *getFirstObject();
};
extern GameLogic *TheGameLogic;
class Waypoint { public: const Coord3D *getLocation() const { return &m_pos; } Coord3D m_pos; };
class PolygonTrigger { public: Int m_id; };
class TerrainLogic
{
public:
	Waypoint *getWaypointByName(const AsciiString &name);
	PolygonTrigger *getTriggerAreaByName(const AsciiString &name);
};
extern TerrainLogic *TheTerrainLogic;
class PartitionManager
{
public:
	void doShroudReveal(Real x, Real y, Real r, PlayerMaskType m) { seLog("reveal %08x %08x %08x %u", fb(x), fb(y), fb(r), m); }
	void undoShroudReveal(Real x, Real y, Real r, PlayerMaskType m) { seLog("unreveal %08x %08x %08x %u", fb(x), fb(y), fb(r), m); }
};
extern PartitionManager *ThePartitionManager;
class ObjectTypes : public MemoryPoolObject, public Snapshot
{
public:
	ObjectTypes() {}
	ObjectTypes(const AsciiString &n) : m_name(n) {}
	void crc(Xfer *) {}
	void xfer(Xfer *) {}
	void loadPostProcess() {}
	AsciiString getListName() const { return m_name; }
	void addObjectType(const AsciiString &t) { if (std::find(m_types.begin(), m_types.end(), t) == m_types.end()) m_types.push_back(t); }
	void removeObjectType(const AsciiString &t) { std::vector<AsciiString>::iterator it = std::find(m_types.begin(), m_types.end(), t); if (it != m_types.end()) m_types.erase(it); }
	Int getListSize() const { return (Int)m_types.size(); }
	AsciiString getNthInList(Int i) const { return i >= 0 && i < (Int)m_types.size() ? m_types[i] : AsciiString(); }
	AsciiString m_name; std::vector<AsciiString> m_types;
};
class AudioEventRTS { public: AudioEventRTS(const AsciiString &n) : m_name(n) {} void setPlayerIndex(Int) {} AsciiString m_name; };
class GameAudio
{
public:
	Real getAudioLengthMS(const AudioEventRTS *e) { return (Real)(seStrHash(e->m_name.str()) % 4000) * 0.75f; }
	void addAudioEvent(AudioEventRTS *e) { seLog("audio add %s", e->m_name.str()); }
};
extern GameAudio *TheAudio;
class GameMessage { public: enum { MSG_CLEAR_GAME_DATA = 7 }; };
class MessageStream { public: GameMessage *appendMessage(Int t) { seLog("message %d", t); return NULL; } };
extern MessageStream *TheMessageStream;
struct Campaign { Bool m_isChallengeCampaign; };
class CampaignManager { public: Campaign *getCurrentCampaign() { m_c.m_isChallengeCampaign = (g_cfg[0] & 2) != 0; return (g_cfg[0] & 1) ? &m_c : NULL; } Campaign m_c; };
extern CampaignManager *TheCampaignManager;
class GameEngine { public: void setFramesPerSecondLimit(Int f) { seLog("fps %d", f); } };
extern GameEngine *TheGameEngine;
struct GlobalData { Bool m_windowed, m_scriptDebug, m_particleEdit; Int m_framesPerSecondLimit; Int m_maxParticleCount; Bool m_disableCameraFade; };
extern GlobalData *TheGlobalData;
extern GlobalData *TheWritableGlobalData;
inline HMODULE LoadLibrary(const char *) { return NULL; }
inline FARPROC GetProcAddress(HMODULE, const char *) { return NULL; }
inline void FreeLibrary(HMODULE) {}

/* TheScriptActions and TheScriptConditions: stand-ins (the real ones are batches A1-A10 and C1) */
class ScriptActions
{
public:
	void init() { seLog("actions init"); }
	void reset() { seLog("actions reset"); }
	void update() { seLog("actions update"); }
	void closeWindows(Bool b) { seLog("actions closewindows %d", (int)b); }
	void executeAction(ScriptAction *a);
	void doEnableOrDisableObjectDifficultyBonuses(Bool b) { seLog("actions bonuses %d", (int)b); }
};
extern ScriptActions *TheScriptActions;
class ScriptConditions
{
public:
	void init() { seLog("conditions init"); }
	void reset() { seLog("conditions reset"); }
	void update() { seLog("conditions update"); }
	Bool evaluateCondition(Condition *c);
	Bool evaluateSkirmishCommandButtonIsReady(Player *, Parameter *p1, Parameter *p2, Bool all);
	Bool evaluateTeamIsContained(Parameter *p0, Bool all);
};
extern ScriptConditions *TheScriptConditions;
class SidesInfo { public: ScriptList *getScriptList() { return m_list; } ScriptList *m_list; };
class SidesList { public: Int getNumSides() const; SidesInfo *getSideInfo(Int i); };
extern SidesList *TheSidesList;
#endif
