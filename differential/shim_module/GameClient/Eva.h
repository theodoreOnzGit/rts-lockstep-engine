/* Stand-in for GameEngine/Include/GameClient/Eva.h, Module build only (differential tests, p1w12-sonnet, special power batch H): the
** superweapon announcements SpecialPowerModule::aboutToDoSpecialPower makes, logged by differential/l9_powers_h.inc. The enumerators are the
** names of Eva.h (the order here is the stand-in's). Renamed ModTestEva by module_prelude.h. */
#ifndef SHIM_MODULE_EVA_H
#define SHIM_MODULE_EVA_H
enum EvaMessage
{
	EVA_Invalid = -1,   /* p1w13-sonnet, batch R26: Eva.h line 43; StealthUpdate's data keeps the index parseEvaMessageFromIni finds (differential/l9_stealth_r26.inc) */
	EVA_SuperweaponLaunched_Own_ParticleCannon = 0, EVA_SuperweaponLaunched_Own_Nuke, EVA_SuperweaponLaunched_Own_ScudStorm,
	EVA_SuperweaponLaunched_Ally_ParticleCannon, EVA_SuperweaponLaunched_Ally_Nuke, EVA_SuperweaponLaunched_Ally_ScudStorm,
	EVA_SuperweaponLaunched_Enemy_ParticleCannon, EVA_SuperweaponLaunched_Enemy_Nuke, EVA_SuperweaponLaunched_Enemy_ScudStorm,
	EVA_SuperweaponLaunched_Own_GPS_Scrambler, EVA_SuperweaponLaunched_Ally_GPS_Scrambler, EVA_SuperweaponLaunched_Enemy_GPS_Scrambler,
	EVA_SuperweaponLaunched_Own_Sneak_Attack, EVA_SuperweaponLaunched_Ally_Sneak_Attack, EVA_SuperweaponLaunched_Enemy_Sneak_Attack,
	EVA_BuildingSabotaged, EVA_CashStolen,   /* p1w13-sonnet, batch R13: the sabotage crates */
	EVA_VehicleStolen,   /* batch R13e: the hijacker */
	EVA_UpgradeComplete   /* batch R76: ProductionUpdate */
};
class Eva
{
public:
	void setShouldPlay(EvaMessage messageToPlay);
	static void parseEvaMessageFromIni( class INI *ini, void *instance, void *store, const void *userData );   /* p1w13-sonnet, batch R26 */
};
extern Eva *TheEva;
#endif
