/* The world of ScriptActions.cpp, the ScriptActions fixture only (differential tests, p1w13-sonnet, layer 11). */
#ifndef SA_WORLD_H
#define SA_WORLD_H
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
#ifndef TRUE
#define TRUE true
#define FALSE false
#endif
#ifndef NULL
#define NULL 0
#endif
#define NEW new
#define DEBUG_LOG(x)
#define DEBUG_ASSERTCRASH(c, x)
#define DEBUG_CRASH(x)
#define DEBUG_ASSERTLOG(c, x)
#define BitTest(val, bit) (((val) & (bit)) != 0)
#define INVALID_ID 0
#define LOGICFRAMES_PER_SECOND 30
#define MAP_XY_FACTOR (10.0f)
#define INVALID_ANGLE -100.0f
void saLog(const char *fmt, ...);

class SubsystemInterface
{
public:
	virtual ~SubsystemInterface() {}
	virtual void init(void) = 0;
	virtual void reset(void) = 0;
	virtual void update(void) = 0;
};

/* the values of the enums the file names (as the engine's) */
enum PlayerType { PLAYER_HUMAN = 0, PLAYER_COMPUTER };
enum TimeOfDay { TIME_OF_DAY_INVALID = 0, TIME_OF_DAY_MORNING, TIME_OF_DAY_AFTERNOON, TIME_OF_DAY_EVENING, TIME_OF_DAY_NIGHT };
enum { COMMAND_OPTION_NEED_OBJECT_TARGET = 1 | 2 | 4, COMMAND_FIRED_BY_SCRIPT = 262144 };
enum DistanceCalculationType { FROM_CENTER_2D = 0, FROM_CENTER_3D, FROM_BOUNDINGSPHERE_2D, FROM_BOUNDINGSPHERE_3D };
enum IterOrderType { ITER_FASTEST = 0, ITER_SORTED_NEAR_TO_FAR, ITER_SORTED_FAR_TO_NEAR, ITER_SORTED_CHEAP_TO_EXPENSIVE, ITER_SORTED_EXPENSIVE_TO_CHEAP };
enum ScienceAvailabilityType { SCIENCE_AVAILABILITY_INVALID = -1, SCIENCE_AVAILABLE, SCIENCE_DISABLED, SCIENCE_HIDDEN, SCIENCE_AVAILABILITY_COUNT };
enum FilterTypes { FT_NULL_FILTER = 0, FT_VIEW_BW_FILTER, FT_VIEW_MOTION_BLUR_FILTER, FT_VIEW_CROSSFADE, FT_VIEW_DEFAULT, FT_MAX };
enum FilterModes { FM_NULL_MODE = 0, FM_VIEW_BW_BLACK_AND_WHITE, FM_VIEW_BW_RED_AND_WHITE, FM_VIEW_BW_GREEN_AND_WHITE, FM_VIEW_CROSSFADE_CIRCLE,
	FM_VIEW_CROSSFADE_FB_MASK, FM_VIEW_MB_IN_AND_OUT_ALPHA, FM_VIEW_MB_IN_AND_OUT_SATURATE, FM_VIEW_MB_IN_ALPHA, FM_VIEW_MB_OUT_ALPHA,
	FM_VIEW_MB_IN_SATURATE, FM_VIEW_MB_OUT_SATURATE, FM_VIEW_MB_END_PAN_ALPHA, FM_VIEW_DEFAULT, FM_VIEW_MB_PAN_ALPHA };
enum RadarEventType { RADAR_EVENT_INVALID = 0 };
enum EvacDisposition { EVAC_INVALID = 0 };
enum BuildableStatus { BSTATUS_YES = 0 };
enum AttitudeType { AI_SLEEP = -2, AI_PASSIVE, AI_NORMAL, AI_ALERT, AI_AGGRESSIVE, AI_INVALID };
enum AudioAffect : int { AudioAffect_Music = 1, AudioAffect_Sound = 2, AudioAffect_Sound3D = 4, AudioAffect_Speech = 8, AudioAffect_All = 15,
	AudioAffect_SystemSetting = 16 };
enum GUICommandType { GUI_COMMAND_NONE = 0, GUI_COMMAND_DOZER_CONSTRUCT, GUI_COMMAND_DOZER_CONSTRUCT_CANCEL, GUI_COMMAND_UNIT_BUILD,
	GUI_COMMAND_CANCEL_UNIT_BUILD, GUI_COMMAND_PLAYER_UPGRADE, GUI_COMMAND_OBJECT_UPGRADE, GUI_COMMAND_CANCEL_UPGRADE, GUI_COMMAND_ATTACK_MOVE,
	GUI_COMMAND_GUARD, GUI_COMMAND_GUARD_WITHOUT_PURSUIT, GUI_COMMAND_GUARD_FLYING_UNITS_ONLY, GUI_COMMAND_STOP, GUI_COMMAND_WAYPOINTS,
	GUI_COMMAND_EXIT_CONTAINER, GUI_COMMAND_EVACUATE, GUI_COMMAND_EXECUTE_RAILED_TRANSPORT, GUI_COMMAND_BEACON_DELETE, GUI_COMMAND_SET_RALLY_POINT,
	GUI_COMMAND_SELL, GUI_COMMAND_FIRE_WEAPON, GUI_COMMAND_SPECIAL_POWER, GUI_COMMAND_PURCHASE_SCIENCE, GUI_COMMAND_HACK_INTERNET,
	GUI_COMMAND_TOGGLE_OVERCHARGE, GUI_COMMAND_COMBATDROP, GUI_COMMAND_SWITCH_WEAPON, GUICOMMANDMODE_HIJACK_VEHICLE,
	GUICOMMANDMODE_CONVERT_TO_CARBOMB, GUICOMMANDMODE_SABOTAGE_BUILDING, GUICOMMANDMODE_PLACE_BEACON };
enum CommandSourceType { CMD_FROM_PLAYER = 0, CMD_FROM_SCRIPT, CMD_FROM_AI, CMD_FROM_DOZER, CMD_DEFAULT_SWITCH_WEAPON };
enum LocomotorSetType { LOCOMOTORSET_INVALID = -1, LOCOMOTORSET_NORMAL, LOCOMOTORSET_NORMAL_UPGRADED, LOCOMOTORSET_FREEFALL,
	LOCOMOTORSET_WANDER, LOCOMOTORSET_PANIC };
enum GuardMode { GUARDMODE_NORMAL = 0, GUARDMODE_GUARD_WITHOUT_PURSUIT, GUARDMODE_GUARD_FLYING_UNITS_ONLY };
enum AudioHandleSpecialValues { AHSV_Error = 0, AHSV_NoSound, AHSV_Muted, AHSV_NotForLocal, AHSV_StopTheMusic, AHSV_StopTheMusicFade };
enum ValueOrThreat { VOT_CashValue = 1, VOT_ThreatValue };
enum KindOfType { KINDOF_STRUCTURE = 7, KINDOF_INFANTRY = 8, KINDOF_TRANSPORT = 18, KINDOF_NO_GARRISON = 24, KINDOF_MONEY_HACKER = 94,
	KINDOF_FS_INTERNET_CENTER = 99, KINDOF_BLAST_CRATER = 100 };
enum ObjectStatusTypes { OBJECT_STATUS_REPULSOR = 9 };
enum ObjectScriptStatusBit { OBJECT_STATUS_SCRIPT_DISABLED = 0x01, OBJECT_STATUS_SCRIPT_UNPOWERED = 0x02, OBJECT_STATUS_SCRIPT_UNSELLABLE = 0x04,
	OBJECT_STATUS_SCRIPT_UNSTEALTHED = 0x08, OBJECT_STATUS_SCRIPT_TARGETABLE = 0x10 };
enum SpecialPowerType { SPECIAL_INVALID = 0, SPECIAL_SNEAK_ATTACK = 45 };
enum DamageType { DAMAGE_UNRESISTABLE = 11 };
enum DeathType { DEATH_NORMAL = 0 };
enum DisabledType { DISABLED_HELD = 3, DISABLED_UNMANNED = 5 };
enum { MAX_COMMANDS_PER_SET = 18 };
static const Int NO_MAX_SHOTS_LIMIT = 0x7fffffff;
static const UnsignedInt NEVER = 0;
static const UnsignedInt FOREVER = 0x3fffffff;
static const PlayerMaskType PLAYERMASK_ALL = 0xffff;
enum { DRAWABLE_FRAMES_PER_FLASH = LOGICFRAMES_PER_SECOND / 2 };
typedef Int ScienceType;
enum { SCIENCE_INVALID = -1 };
enum Relationship { ENEMIES = 0, NEUTRAL, ALLIES };
enum { ALLOW_ALLIES = 1 << ALLIES, ALLOW_ENEMIES = 1 << ENEMIES, ALLOW_NEUTRAL = 1 << NEUTRAL };


/* ---------------------------------------------------------------------------------------------------------------------------
** The configuration and the log. g_cfg is laid out in tests/differential_script_actions.rs, whose host answers from the same
** words with the same functions. Floats in the log are their bits (%08x), so both sides print them exactly. */
enum { SA_CFG = 256, SA_PLAYERS = 8, SA_PROTOS = 8, SA_TEAMS0 = 16, SA_OBJS0 = 16, SA_WAYPOINTS = 8, SA_TEMPLATES = 8 };
extern unsigned g_cfg[SA_CFG];
inline unsigned saHash(unsigned seed, unsigned a, unsigned b)
{
	unsigned x = seed ^ (a * 0x9E3779B1u) ^ (b * 0x85EBCA77u);
	x ^= x >> 15; x *= 0x2C1B3C6Du; x ^= x >> 12; x *= 0x297A2D39u; x ^= x >> 15;
	return x;
}
inline unsigned saStrHash(const char *s) { unsigned h = 2166136261u; for (; *s; ++s) { h ^= (unsigned char)*s; h *= 16777619u; } return h; }
inline unsigned fb(Real r) { unsigned u; memcpy(&u, &r, 4); return u; }
inline int saNum(const char *s, const char *prefix)
{
	size_t n = strlen(prefix);
	if (strncmp(s, prefix, n) != 0 || !s[n]) return -1;
	int v = 0;
	for (const char *p = s + n; *p; ++p) { if (*p < '0' || *p > '9' || v > 1000) return -1; v = v * 10 + (*p - '0'); }
	return v;
}
#define SA_SEED g_cfg[7]

class AsciiString
{
public:
	AsciiString() {}
	AsciiString(const char *s) : m_s(s ? s : "") {}
	const char *str() const { return m_s.c_str(); }
	Bool isEmpty() const { return m_s.empty(); }
	Bool isNotEmpty() const { return !m_s.empty(); }
	Int getLength() const { return (Int)m_s.size(); }
	void concat(const char *s) { m_s += s; }
	void concat(const AsciiString &s) { m_s += s.m_s; }
	void concat(char c) { m_s += c; }
	void translate(const class UnicodeString &u);
	Bool endsWith(const char *s) const { size_t n = strlen(s); return m_s.size() >= n && m_s.compare(m_s.size() - n, n, s) == 0; }
	void format(const char *fmt, ...) { char b[512]; va_list ap; va_start(ap, fmt); vsnprintf(b, sizeof(b), fmt, ap); va_end(ap); m_s = b; }
	Bool operator==(const AsciiString &o) const { return m_s == o.m_s; }
	Bool operator!=(const AsciiString &o) const { return m_s != o.m_s; }
	Bool operator==(const char *o) const { return m_s == o; }
	Bool operator!=(const char *o) const { return m_s != o; }
	Bool operator<(const AsciiString &o) const { return m_s < o.m_s; }
	static AsciiString TheEmptyString;
	std::string m_s;
};
inline AsciiString saName(const char *prefix, int n) { char b[32]; snprintf(b, sizeof(b), "%s%d", prefix, n); return AsciiString(b); }
class UnicodeString
{
public:
	UnicodeString() {}
	const WideChar *str() const { return m_s.c_str(); }
	void format(const WideChar *fmt, ...) { m_s = fmt; }
	void translate(const AsciiString &a) { m_s.clear(); for (const char *p = a.str(); *p; ++p) m_s += (WideChar)*p; }
	Bool isEmpty() const { return m_s.empty(); }
	Int getLength() const { return (Int)m_s.size(); }
	WideChar getCharAt(Int i) const { return i >= 0 && i < (Int)m_s.size() ? m_s[i] : 0; }
	std::wstring m_s;
};
inline void AsciiString::translate(const UnicodeString &u) { m_s.clear(); for (size_t i = 0; i < u.m_s.size(); ++i) m_s += (char)u.m_s[i]; }
/* a UnicodeString as ASCII, for the log */
inline std::string saW(const UnicodeString &u) { std::string s; for (size_t i = 0; i < u.m_s.size(); ++i) s += (char)u.m_s[i]; return s; }

struct Coord2D { Real x, y; };
struct Coord3D { Real x, y, z; void zero() { x = y = z = 0; } Real lengthSqr() const { return x * x + y * y + z * z; } Real length() const { return (Real)sqrt(lengthSqr()); } };
struct ICoord2D { Int x, y; };
class Vector3 { public: Vector3() : X(0), Y(0), Z(0) {} Vector3(Real x, Real y, Real z) : X(x), Y(y), Z(z) {} Real X, Y, Z; };
class Matrix3D
{
public:
	/* a rotation about z by m_angle, then the translation */
	static void Transform_Vector(const Matrix3D &m, const Vector3 &in, Vector3 *out)
	{
		Real c = (Real)cos(m.m_angle), s = (Real)sin(m.m_angle);
		out->X = c * in.X - s * in.Y + m.m_pos.x; out->Y = s * in.X + c * in.Y + m.m_pos.y; out->Z = in.Z + m.m_pos.z;
	}
	Real m_angle;
	Coord3D m_pos;
};
class RGBColor { public: Real red, green, blue; Int getAsInt() const { return ((Int)(red * 255.0) << 16) | ((Int)(green * 255.0) << 8) | ((Int)(blue * 255.0) << 0); } void setFromInt(Int c) { red = ((c >> 16) & 255) / 255.0f; green = ((c >> 8) & 255) / 255.0f; blue = (c & 255) / 255.0f; } };

class NameKeyGenerator { public: NameKeyType nameToKey(const AsciiString &s) { return (NameKeyType)(saStrHash(s.str()) & 0x7fffffff); } };
extern NameKeyGenerator *TheNameKeyGenerator;
#define NAMEKEY(s) TheNameKeyGenerator->nameToKey(s)

class MemoryPoolObject
{
public:
	virtual ~MemoryPoolObject() {}
	void deleteInstance() { if (this) delete this; }
};
#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(cls, name) protected: virtual ~cls(); public:
#define newInstance(cls) new cls
class MemoryPoolObjectHolder
{
public:
	MemoryPoolObjectHolder(MemoryPoolObject *o = NULL) : m_o(o) {}
	~MemoryPoolObjectHolder() { if (m_o) m_o->deleteInstance(); }
	MemoryPoolObject *m_o;
};

class KindOfMaskType
{
public:
	KindOfMaskType() { memset(m_bits, 0, sizeof(m_bits)); }
	void set(Int i, Int value = 1) { if (value) m_bits[i / 32] |= 1u << (i % 32); else m_bits[i / 32] &= ~(1u << (i % 32)); }
	Bool test(Int i) const { return (m_bits[i / 32] >> (i % 32) & 1) != 0; }
	unsigned m_bits[4];
};
extern KindOfMaskType KINDOFMASK_NONE;
inline KindOfMaskType MAKE_KINDOF_MASK(Int k) { KindOfMaskType m; m.set(k); return m; }
class ObjectStatusMaskType
{
public:
	ObjectStatusMaskType() : m_bits(0) {}
	unsigned long long m_bits;
};
inline ObjectStatusMaskType MAKE_OBJECT_STATUS_MASK(Int a) { ObjectStatusMaskType m; m.m_bits = 1ull << a; return m; }

/* Scripts.h: Parameter, ScriptAction, Script; ScriptEngine.h: SequentialScript */
class Parameter
{
public:
	enum { LESS_THAN = 0, LESS_EQUAL, EQUAL, GREATER_EQUAL, GREATER, NOT_EQUAL };
	enum { REL_ENEMY = ENEMIES, REL_NEUTRAL = NEUTRAL, REL_FRIEND = ALLIES };
	Int getInt() const { return m_int; }
	Real getReal() const { return m_real; }
	AsciiString getString() const { return m_string; }
	enum { COORD3D = 16 };
	/* Scripts.cpp's: zero unless the parameter is a COORD3D */
	void getCoord3D(Coord3D *c) const { c->x = c->y = c->z = 0; if (m_type == COORD3D) *c = m_coord; }
	Int m_type;
	Int m_int;
	Real m_real;
	AsciiString m_string;
	Coord3D m_coord;
};
enum { MAX_PARMS = 12 };
class ScriptAction
{
public:
	enum ScriptActionType
	{
DEBUG_MESSAGE_BOX,
SET_FLAG,
SET_COUNTER,
VICTORY,
DEFEAT,
NO_OP,
SET_TIMER,
PLAY_SOUND_EFFECT,
ENABLE_SCRIPT,
DISABLE_SCRIPT,
CALL_SUBROUTINE,
PLAY_SOUND_EFFECT_AT,
DAMAGE_MEMBERS_OF_TEAM,
MOVE_TEAM_TO,
MOVE_CAMERA_TO,
INCREMENT_COUNTER,
DECREMENT_COUNTER,
MOVE_CAMERA_ALONG_WAYPOINT_PATH,
ROTATE_CAMERA,
RESET_CAMERA,
SET_MILLISECOND_TIMER,
CAMERA_MOD_FREEZE_TIME,
SET_VISUAL_SPEED_MULTIPLIER,
CREATE_OBJECT,
SUSPEND_BACKGROUND_SOUNDS,
RESUME_BACKGROUND_SOUNDS,
CAMERA_MOD_SET_FINAL_ZOOM,
CAMERA_MOD_SET_FINAL_PITCH,
CAMERA_MOD_FREEZE_ANGLE,
CAMERA_MOD_SET_FINAL_SPEED_MULTIPLIER,
CAMERA_MOD_SET_ROLLING_AVERAGE,
CAMERA_MOD_FINAL_LOOK_TOWARD,
CAMERA_MOD_LOOK_TOWARD,
TEAM_ATTACK_TEAM,
CREATE_REINFORCEMENT_TEAM,
MOVE_CAMERA_TO_SELECTION,
TEAM_FOLLOW_WAYPOINTS,
TEAM_SET_STATE,
MOVE_NAMED_UNIT_TO,
NAMED_ATTACK_NAMED,
CREATE_NAMED_ON_TEAM_AT_WAYPOINT,
CREATE_UNNAMED_ON_TEAM_AT_WAYPOINT,
NAMED_APPLY_ATTACK_PRIORITY_SET,
TEAM_APPLY_ATTACK_PRIORITY_SET,
SET_BASE_CONSTRUCTION_SPEED,
NAMED_SET_ATTITUDE,
TEAM_SET_ATTITUDE,
NAMED_ATTACK_AREA,
NAMED_ATTACK_TEAM,
TEAM_ATTACK_AREA,
TEAM_ATTACK_NAMED,
TEAM_LOAD_TRANSPORTS,
NAMED_ENTER_NAMED,
TEAM_ENTER_NAMED,
NAMED_EXIT_ALL,
TEAM_EXIT_ALL,
NAMED_FOLLOW_WAYPOINTS,
NAMED_GUARD,
TEAM_GUARD,
NAMED_HUNT,
TEAM_HUNT,
PLAYER_SELL_EVERYTHING,
PLAYER_DISABLE_BASE_CONSTRUCTION,
PLAYER_DISABLE_FACTORIES,
PLAYER_DISABLE_UNIT_CONSTRUCTION,
PLAYER_ENABLE_BASE_CONSTRUCTION,
PLAYER_ENABLE_FACTORIES,
PLAYER_ENABLE_UNIT_CONSTRUCTION,
CAMERA_MOVE_HOME,
BUILD_TEAM,
NAMED_DAMAGE,
NAMED_DELETE,
TEAM_DELETE,
NAMED_KILL,
TEAM_KILL,
PLAYER_KILL,
DISPLAY_TEXT,
CAMEO_FLASH,
NAMED_FLASH,
TEAM_FLASH,
MOVIE_PLAY_FULLSCREEN,
MOVIE_PLAY_RADAR,
SOUND_PLAY_NAMED,
SPEECH_PLAY,
PLAYER_TRANSFER_OWNERSHIP_PLAYER,
NAMED_TRANSFER_OWNERSHIP_PLAYER,
PLAYER_RELATES_PLAYER,
RADAR_CREATE_EVENT,
RADAR_DISABLE,
RADAR_ENABLE,
MAP_REVEAL_AT_WAYPOINT,
TEAM_AVAILABLE_FOR_RECRUITMENT,
TEAM_COLLECT_NEARBY_FOR_TEAM,
TEAM_MERGE_INTO_TEAM,
DISABLE_INPUT,
ENABLE_INPUT,
PLAYER_HUNT,
SOUND_AMBIENT_PAUSE,
SOUND_AMBIENT_RESUME,
MUSIC_SET_TRACK,
SET_TREE_SWAY,
DEBUG_STRING,
MAP_REVEAL_ALL,
TEAM_GARRISON_SPECIFIC_BUILDING,
EXIT_SPECIFIC_BUILDING,
TEAM_GARRISON_NEAREST_BUILDING,
TEAM_EXIT_ALL_BUILDINGS,
NAMED_GARRISON_SPECIFIC_BUILDING,
NAMED_GARRISON_NEAREST_BUILDING,
NAMED_EXIT_BUILDING,
PLAYER_GARRISON_ALL_BUILDINGS,
PLAYER_EXIT_ALL_BUILDINGS,
TEAM_WANDER,
TEAM_PANIC,
SETUP_CAMERA,
CAMERA_LETTERBOX_BEGIN,
CAMERA_LETTERBOX_END,
ZOOM_CAMERA,
PITCH_CAMERA,
CAMERA_FOLLOW_NAMED,
OVERSIZE_TERRAIN,
CAMERA_FADE_ADD,
CAMERA_FADE_SUBTRACT,
CAMERA_FADE_SATURATE,
CAMERA_FADE_MULTIPLY,
CAMERA_BW_MODE_BEGIN,
CAMERA_BW_MODE_END,
DRAW_SKYBOX_BEGIN,
DRAW_SKYBOX_END,
SET_ATTACK_PRIORITY_THING,
SET_ATTACK_PRIORITY_KIND_OF,
SET_DEFAULT_ATTACK_PRIORITY,
CAMERA_STOP_FOLLOW,
CAMERA_MOTION_BLUR,
CAMERA_MOTION_BLUR_JUMP,
CAMERA_MOTION_BLUR_FOLLOW,
CAMERA_MOTION_BLUR_END_FOLLOW,
FREEZE_TIME,
UNFREEZE_TIME,
SHOW_MILITARY_CAPTION,
CAMERA_SET_AUDIBLE_DISTANCE,
SET_STOPPING_DISTANCE,
NAMED_SET_STOPPING_DISTANCE,
SET_FPS_LIMIT,
MUSIC_SET_VOLUME,
MAP_SHROUD_AT_WAYPOINT,
MAP_SHROUD_ALL,
SET_RANDOM_TIMER,
SET_RANDOM_MSEC_TIMER,
STOP_TIMER,
RESTART_TIMER,
ADD_TO_MSEC_TIMER,
SUB_FROM_MSEC_TIMER,
TEAM_TRANSFER_TO_PLAYER,
PLAYER_SET_MONEY,
PLAYER_GIVE_MONEY,
DISABLE_SPECIAL_POWER_DISPLAY,
ENABLE_SPECIAL_POWER_DISPLAY,
NAMED_HIDE_SPECIAL_POWER_DISPLAY,
NAMED_SHOW_SPECIAL_POWER_DISPLAY,
DISPLAY_COUNTDOWN_TIMER,
HIDE_COUNTDOWN_TIMER,
ENABLE_COUNTDOWN_TIMER_DISPLAY,
DISABLE_COUNTDOWN_TIMER_DISPLAY,
NAMED_STOP_SPECIAL_POWER_COUNTDOWN,
NAMED_START_SPECIAL_POWER_COUNTDOWN,
NAMED_SET_SPECIAL_POWER_COUNTDOWN,
NAMED_ADD_SPECIAL_POWER_COUNTDOWN,
NAMED_FIRE_SPECIAL_POWER_AT_WAYPOINT,
NAMED_FIRE_SPECIAL_POWER_AT_NAMED,
REFRESH_RADAR,
CAMERA_TETHER_NAMED,
CAMERA_STOP_TETHER_NAMED,
CAMERA_SET_DEFAULT,
NAMED_STOP,
TEAM_STOP,
TEAM_STOP_AND_DISBAND,
RECRUIT_TEAM,
TEAM_SET_OVERRIDE_RELATION_TO_TEAM,
TEAM_REMOVE_OVERRIDE_RELATION_TO_TEAM,
TEAM_REMOVE_ALL_OVERRIDE_RELATIONS,
CAMERA_LOOK_TOWARD_OBJECT,
NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH,
TEAM_SET_OVERRIDE_RELATION_TO_PLAYER,
TEAM_REMOVE_OVERRIDE_RELATION_TO_PLAYER,
PLAYER_SET_OVERRIDE_RELATION_TO_TEAM,
PLAYER_REMOVE_OVERRIDE_RELATION_TO_TEAM,
UNIT_EXECUTE_SEQUENTIAL_SCRIPT,
UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING,
UNIT_STOP_SEQUENTIAL_SCRIPT,
TEAM_EXECUTE_SEQUENTIAL_SCRIPT,
TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING,
TEAM_STOP_SEQUENTIAL_SCRIPT,
UNIT_GUARD_FOR_FRAMECOUNT,
UNIT_IDLE_FOR_FRAMECOUNT,
TEAM_GUARD_FOR_FRAMECOUNT,
TEAM_IDLE_FOR_FRAMECOUNT,
WATER_CHANGE_HEIGHT,
NAMED_USE_COMMANDBUTTON_ABILITY_ON_NAMED,
NAMED_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT,
WATER_CHANGE_HEIGHT_OVER_TIME,
MAP_SWITCH_BORDER,
TEAM_GUARD_POSITION,
TEAM_GUARD_OBJECT,
TEAM_GUARD_AREA,
OBJECT_FORCE_SELECT,
CAMERA_LOOK_TOWARD_WAYPOINT,
UNIT_DESTROY_ALL_CONTAINED,
RADAR_FORCE_ENABLE,
RADAR_REVERT_TO_NORMAL,
SCREEN_SHAKE,
TECHTREE_MODIFY_BUILDABILITY_OBJECT,
WAREHOUSE_SET_VALUE,
OBJECT_CREATE_RADAR_EVENT,
TEAM_CREATE_RADAR_EVENT,
DISPLAY_CINEMATIC_TEXT,
DEBUG_CRASH_BOX,
SOUND_DISABLE_TYPE,
SOUND_ENABLE_TYPE,
SOUND_ENABLE_ALL,
AUDIO_OVERRIDE_VOLUME_TYPE,
AUDIO_RESTORE_VOLUME_TYPE,
AUDIO_RESTORE_VOLUME_ALL_TYPE,
INGAME_POPUP_MESSAGE,
SET_CAVE_INDEX,
NAMED_SET_HELD,
NAMED_SET_TOPPLE_DIRECTION,
UNIT_MOVE_TOWARDS_NEAREST_OBJECT_TYPE,
TEAM_MOVE_TOWARDS_NEAREST_OBJECT_TYPE,
MAP_REVEAL_ALL_PERM,
MAP_REVEAL_ALL_UNDO_PERM,
NAMED_SET_REPULSOR,
TEAM_SET_REPULSOR,
TEAM_WANDER_IN_PLACE,
TEAM_INCREASE_PRIORITY,
TEAM_DECREASE_PRIORITY,
DISPLAY_COUNTER,
HIDE_COUNTER,
TEAM_USE_COMMANDBUTTON_ABILITY_ON_NAMED,
TEAM_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT,
NAMED_USE_COMMANDBUTTON_ABILITY,
TEAM_USE_COMMANDBUTTON_ABILITY,
NAMED_FLASH_WHITE,
TEAM_FLASH_WHITE,
SKIRMISH_BUILD_BUILDING,
SKIRMISH_FOLLOW_APPROACH_PATH,
IDLE_ALL_UNITS,
RESUME_SUPPLY_TRUCKING,
NAMED_CUSTOM_COLOR,
SKIRMISH_MOVE_TO_APPROACH_PATH,
SKIRMISH_BUILD_BASE_DEFENSE_FRONT,
SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST,
NAMED_RECEIVE_UPGRADE,
PLAYER_REPAIR_NAMED_STRUCTURE,
SKIRMISH_BUILD_BASE_DEFENSE_FLANK,
SKIRMISH_BUILD_STRUCTURE_FRONT,
SKIRMISH_BUILD_STRUCTURE_FLANK,
SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE,
SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT,
SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_ALL,
SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_PARTIAL,
TEAM_SPIN_FOR_FRAMECOUNT,
TEAM_ALL_USE_COMMANDBUTTON_ON_NAMED,
TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_UNIT,
TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_GARRISONED_BUILDING,
TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_KINDOF,
TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING,
TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING_CLASS,
TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_OBJECTTYPE,
TEAM_PARTIAL_USE_COMMANDBUTTON,
TEAM_CAPTURE_NEAREST_UNOWNED_FACTION_UNIT,
PLAYER_CREATE_TEAM_FROM_CAPTURED_UNITS,
PLAYER_ADD_SKILLPOINTS,
PLAYER_ADD_RANKLEVEL,
PLAYER_SET_RANKLEVEL,
PLAYER_SET_RANKLEVELLIMIT,
PLAYER_GRANT_SCIENCE,
PLAYER_PURCHASE_SCIENCE,
TEAM_HUNT_WITH_COMMAND_BUTTON,
TEAM_WAIT_FOR_NOT_CONTAINED_ALL,
TEAM_WAIT_FOR_NOT_CONTAINED_PARTIAL,
TEAM_FOLLOW_WAYPOINTS_EXACT,
NAMED_FOLLOW_WAYPOINTS_EXACT,
TEAM_SET_EMOTICON,
NAMED_SET_EMOTICON,
AI_PLAYER_BUILD_SUPPLY_CENTER,
AI_PLAYER_BUILD_UPGRADE,
OBJECTLIST_ADDOBJECTTYPE,
OBJECTLIST_REMOVEOBJECTTYPE,
MAP_REVEAL_PERMANENTLY_AT_WAYPOINT,
MAP_UNDO_REVEAL_PERMANENTLY_AT_WAYPOINT,
NAMED_SET_STEALTH_ENABLED,
TEAM_SET_STEALTH_ENABLED,
EVA_SET_ENABLED_DISABLED,
OPTIONS_SET_OCCLUSION_MODE,
LOCALDEFEAT,
OPTIONS_SET_DRAWICON_UI_MODE,
OPTIONS_SET_PARTICLE_CAP_MODE,
PLAYER_SCIENCE_AVAILABILITY,
UNIT_AFFECT_OBJECT_PANEL_FLAGS,
TEAM_AFFECT_OBJECT_PANEL_FLAGS,
PLAYER_SELECT_SKILLSET,
SCRIPTING_OVERRIDE_HULK_LIFETIME,
NAMED_FACE_NAMED,
NAMED_FACE_WAYPOINT,
TEAM_FACE_NAMED,
TEAM_FACE_WAYPOINT,
COMMANDBAR_REMOVE_BUTTON_OBJECTTYPE,
COMMANDBAR_ADD_BUTTON_OBJECTTYPE_SLOT,
UNIT_SPAWN_NAMED_LOCATION_ORIENTATION,
PLAYER_AFFECT_RECEIVING_EXPERIENCE,
PLAYER_EXCLUDE_FROM_SCORE_SCREEN,
TEAM_GUARD_SUPPLY_CENTER,
ENABLE_SCORING,
DISABLE_SCORING,
SOUND_SET_VOLUME,
SPEECH_SET_VOLUME,
DISABLE_BORDER_SHROUD,
ENABLE_BORDER_SHROUD,
OBJECT_ALLOW_BONUSES,
SOUND_REMOVE_ALL_DISABLED,
SOUND_REMOVE_TYPE,
TEAM_GUARD_IN_TUNNEL_NETWORK,
QUICKVICTORY,
SET_INFANTRY_LIGHTING_OVERRIDE,
RESET_INFANTRY_LIGHTING_OVERRIDE,
TEAM_DELETE_LIVING,
RESIZE_VIEW_GUARDBAND,
DELETE_ALL_UNMANNED,
CHOOSE_VICTIM_ALWAYS_USES_NORMAL,
CAMERA_ENABLE_SLAVE_MODE,
CAMERA_DISABLE_SLAVE_MODE,
CAMERA_ADD_SHAKER_AT,
SET_TRAIN_HELD,
NAMED_SET_EVAC_LEFT_OR_RIGHT,
ENABLE_OBJECT_SOUND,
DISABLE_OBJECT_SOUND,
NAMED_USE_COMMANDBUTTON_ABILITY_USING_WAYPOINT_PATH,
NAMED_SET_UNMANNED_STATUS,
TEAM_SET_UNMANNED_STATUS,
NAMED_SET_BOOBYTRAPPED,
TEAM_SET_BOOBYTRAPPED,
SHOW_WEATHER,
AI_PLAYER_BUILD_TYPE_NEAREST_TEAM,
		NUM_ITEMS
	};
	enum ScriptActionType getActionType(void) { return m_actionType; }
	Int getNumParameters(void) { return m_numParms; }
	Parameter *getParameter(Int ndx) { if (ndx >= 0 && ndx < m_numParms) return m_parms[ndx]; return NULL; }
	enum ScriptActionType m_actionType;
	Int m_numParms;
	Parameter *m_parms[MAX_PARMS];
};
class Script { public: AsciiString getName() const { return m_name; } AsciiString m_name; };
class Team;
class SequentialScript : public MemoryPoolObject
{
public:
	SequentialScript() : m_teamToExecOn(NULL), m_objectID(0), m_scriptToExecuteSequentially(NULL), m_currentInstruction(-1), m_timesToLoop(0),
		m_framesToWait(0), m_dontAdvanceInstruction(FALSE), m_nextScriptInSequence(NULL) {}
	Team *m_teamToExecOn;
	ObjectID m_objectID;
	Script *m_scriptToExecuteSequentially;
	Int m_currentInstruction;
	Int m_timesToLoop;
	Int m_framesToWait;
	Bool m_dontAdvanceInstruction;
	SequentialScript *m_nextScriptInSequence;
};

#include "sa_world_b.h"
#endif
