/* Differential-test wrapper for the game-wide enums and name tables
** (upstream commit 0a05454d):
**   GameEngine/Include/Common/GameType.h, Source/Common/System/GameType.cpp
**   GameEngine/Include/Common/GameCommon.h, Source/Common/System/GameCommon.cpp
**   GameEngine/Include/Common/Errors.h, SpecialPowerType.h, ThingSort.h
**   GameEngine/Include/GameLogic/Powers.h, WeaponSetType.h, WeaponStatus.h,
**     WeaponBonusConditionFlags.h, ObjectScriptStatusBits.h
** All unchanged. Compiled in the INI wrapper build (build.rs), whose library
** also holds GameCommon.cpp (TheVeterancyNames, TheRelationshipNames,
** normalizeAngle); GameType.cpp is compiled beside this file.
**
** ref_enum_values lists the enumerators of each enum in declaration order,
** as the C++ compiler evaluates them; the Rust test lists its constants in
** the same order, so a transcription slip on either side is a mismatch. */
#include <stdlib.h>
#include "Common/GameType.h"
#include "Common/GameCommon.h"
#include "Common/Errors.h"
#include "Common/SpecialPowerType.h"
#define DEFINE_EDITOR_SORTING_NAMES
#include "Common/ThingSort.h"
#define DEFINE_POWER_NAMES
#include "GameLogic/Powers.h"
#include "GameLogic/WeaponSetType.h"
#include "GameLogic/WeaponStatus.h"
#include "GameLogic/WeaponBonusConditionFlags.h"
#include "GameLogic/ObjectScriptStatusBits.h"

extern char *TimeOfDayNames[];
extern char *WeatherNames[];

#define N(x) ((long long)(x))
static const long long k_game_type[] = {
	DEFAULT_WORLD_WIDTH, DEFAULT_WORLD_HEIGHT, INVALID_ID, FORCE_OBJECTID_TO_LONG_SIZE,
	INVALID_DRAWABLE_ID, FORCE_DRAWABLEID_TO_LONG_SIZE, NO_FORMATION_ID, FORCE_FORMATIONID_TO_LONG_SIZE,
	TIME_OF_DAY_INVALID, TIME_OF_DAY_FIRST, TIME_OF_DAY_MORNING, TIME_OF_DAY_AFTERNOON, TIME_OF_DAY_EVENING,
	TIME_OF_DAY_NIGHT, TIME_OF_DAY_COUNT, WEATHER_NORMAL, WEATHER_SNOWY, WEATHER_COUNT,
	SCORCH_1, SCORCH_2, SCORCH_3, SCORCH_4, SHADOW_SCORCH, SCORCH_COUNT,
	PRIMARY_WEAPON, SECONDARY_WEAPON, TERTIARY_WEAPON, WEAPONSLOT_COUNT,
	LAYER_INVALID, LAYER_GROUND, LAYER_WALL, LAYER_LAST,
};
static const long long k_game_common[] = {
	LOGICFRAMES_PER_SECOND, MSEC_PER_SECOND, MAX_PLAYER_COUNT, N(sizeof(PlayerMaskType)), PLAYERMASK_ALL, PLAYERMASK_NONE,
	MAX_GLOBAL_GENERAL_TYPES, GLOBAL_GENERAL_BEGIN, GLOBAL_GENERAL_END,
	DIFFICULTY_EASY, DIFFICULTY_NORMAL, DIFFICULTY_HARD, DIFFICULTY_COUNT,
	PLAYER_HUMAN, PLAYER_COMPUTER, PLAYERTYPE_COUNT,
	CELLSHROUD_CLEAR, CELLSHROUD_FOGGED, CELLSHROUD_SHROUDED, CELLSHROUD_COUNT,
	OBJECTSHROUD_INVALID, OBJECTSHROUD_CLEAR, OBJECTSHROUD_PARTIAL_CLEAR, OBJECTSHROUD_FOGGED, OBJECTSHROUD_SHROUDED,
	OBJECTSHROUD_INVALID_BUT_PREVIOUS_VALID, OBJECTSHROUD_COUNT,
	GUARDMODE_NORMAL, GUARDMODE_GUARD_WITHOUT_PURSUIT, GUARDMODE_GUARD_FLYING_UNITS_ONLY,
	NEVER, FOREVER,
	LEVEL_REGULAR, LEVEL_VETERAN, LEVEL_ELITE, LEVEL_HEROIC, LEVEL_COUNT,
	CMD_FROM_PLAYER, CMD_FROM_SCRIPT, CMD_FROM_AI, CMD_FROM_DOZER, CMD_DEFAULT_SWITCH_WEAPON,
	_ATTACK_FORCED, _ATTACK_CONTINUED, _ATTACK_TUNNELNETWORK_GUARD, ATTACK_NEW_TARGET, ATTACK_NEW_TARGET_FORCED,
	ATTACK_CONTINUED_TARGET, ATTACK_CONTINUED_TARGET_FORCED, ATTACK_TUNNEL_NETWORK_GUARD,
	TURRET_INVALID, TURRET_MAIN, TURRET_ALT, MAX_TURRETS,
	ENEMIES, NEUTRAL, ALLIES,
};
static const long long k_errors[] = {
	N(ERROR_BASE), N(ERROR_BUG), N(ERROR_OUT_OF_MEMORY), N(ERROR_BAD_ARG), N(ERROR_INVALID_FILE_VERSION),
	N(ERROR_CORRUPT_FILE_FORMAT), N(ERROR_BAD_INI), N(ERROR_INVALID_D3D), N(ERROR_LAST),
};
static const long long k_thing_sort[] = {
	ES_FIRST, ES_NONE, ES_STRUCTURE, ES_INFANTRY, ES_VEHICLE, ES_SHRUBBERY, ES_MISC_MAN_MADE, ES_MISC_NATURAL,
	ES_DEBRIS, ES_SYSTEM, ES_AUDIO, ES_TEST, ES_FOR_REVIEW, ES_ROAD, ES_WAYPOINT, ES_NUM_SORTING_TYPES,
};
static const long long k_powers[] = { POWER_NONE, POWER_FASTER, POWER_DOUBLE_SHOT, POWER_SELF_HEALING, POWERS_NUM_POWERS };
static const long long k_weapon[] = {
	WEAPONSET_VETERAN, WEAPONSET_ELITE, WEAPONSET_HERO, WEAPONSET_PLAYER_UPGRADE, WEAPONSET_CRATEUPGRADE_ONE,
	WEAPONSET_CRATEUPGRADE_TWO, WEAPONSET_VEHICLE_HIJACK, WEAPONSET_CARBOMB, WEAPONSET_MINE_CLEARING_DETAIL,
	WEAPONSET_RIDER1, WEAPONSET_RIDER2, WEAPONSET_RIDER3, WEAPONSET_RIDER4, WEAPONSET_RIDER5, WEAPONSET_RIDER6,
	WEAPONSET_RIDER7, WEAPONSET_RIDER8, WEAPONSET_COUNT,
	READY_TO_FIRE, OUT_OF_AMMO, BETWEEN_FIRING_SHOTS, RELOADING_CLIP, PRE_ATTACK, WEAPON_STATUS_COUNT,
	N(sizeof(WeaponBonusConditionFlags)),
	OBJECT_STATUS_SCRIPT_DISABLED, OBJECT_STATUS_SCRIPT_UNPOWERED, OBJECT_STATUS_SCRIPT_UNSELLABLE,
	OBJECT_STATUS_SCRIPT_UNSTEALTHED, OBJECT_STATUS_SCRIPT_TARGETABLE,
};
static const long long k_special_power[] = {
#define X(n) n,
#include "special_power_type_list.h"
#undef X
};

extern "C" {

/* which: 0 GameType.h, 1 GameCommon.h, 2 Errors.h, 3 ThingSort.h, 4 Powers.h,
** 5 the weapon headers and ObjectScriptStatusBits.h, 6 SpecialPowerType.h.
** Writes up to cap values; returns the count. */
int ref_enum_values(int which, long long *out, int cap)
{
	const long long *t; int n;
#define T(a) t = a; n = (int)(sizeof(a) / sizeof(a[0])); break
	switch (which) {
	case 0: T(k_game_type);
	case 1: T(k_game_common);
	case 2: T(k_errors);
	case 3: T(k_thing_sort);
	case 4: T(k_powers);
	case 5: T(k_weapon);
	case 6: T(k_special_power);
	default: abort();
	}
#undef T
	for (int i = 0; i < n && i < cap; ++i) out[i] = t[i];
	return n;
}

/* which: 0 TimeOfDayNames, 1 WeatherNames, 2 TheVeterancyNames,
** 3 TheRelationshipNames, 4 EditorSortingNames, 5 PowerNames.
** Entry i, or NULL at (and only at) the terminator. */
const char *ref_name_table(int which, int i)
{
	switch (which) {
	case 0: return TimeOfDayNames[i];
	case 1: return WeatherNames[i];
	case 2: return TheVeterancyNames[i];
	case 3: return TheRelationshipNames[i];
	case 4: return EditorSortingNames[i];
	case 5: return PowerNames[i];
	}
	abort();
}

/* op: 0 normalizeAngle(a), 1 stdAngleDiff(a, b) (inputs that terminate) */
float ref_game_common_angle(int op, float a, float b)
{
	return op == 0 ? normalizeAngle(a) : stdAngleDiff(a, b);
}

/* op: 0 isForcedAttack, 1 isContinuedAttack */
int ref_attack_type(int op, int t)
{
	return op == 0 ? isForcedAttack((AbleToAttackType)t) : isContinuedAttack((AbleToAttackType)t);
}

} /* extern "C" */
