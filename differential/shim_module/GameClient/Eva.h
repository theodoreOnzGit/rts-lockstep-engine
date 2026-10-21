/* Stand-in for GameEngine/Include/GameClient/Eva.h, Module build only (differential tests, p1w11-sonnet, special power batch H): the
** superweapon announcements SpecialPowerModule::aboutToDoSpecialPower makes, logged by differential/l9_powers_h.inc. The enumerators are the
** names of Eva.h (the order here is the stand-in's). Renamed ModTestEva by module_prelude.h. */
#ifndef SHIM_MODULE_EVA_H
#define SHIM_MODULE_EVA_H
enum EvaMessage
{
	EVA_SuperweaponLaunched_Own_ParticleCannon, EVA_SuperweaponLaunched_Own_Nuke, EVA_SuperweaponLaunched_Own_ScudStorm,
	EVA_SuperweaponLaunched_Ally_ParticleCannon, EVA_SuperweaponLaunched_Ally_Nuke, EVA_SuperweaponLaunched_Ally_ScudStorm,
	EVA_SuperweaponLaunched_Enemy_ParticleCannon, EVA_SuperweaponLaunched_Enemy_Nuke, EVA_SuperweaponLaunched_Enemy_ScudStorm,
	EVA_SuperweaponLaunched_Own_GPS_Scrambler, EVA_SuperweaponLaunched_Ally_GPS_Scrambler, EVA_SuperweaponLaunched_Enemy_GPS_Scrambler,
	EVA_SuperweaponLaunched_Own_Sneak_Attack, EVA_SuperweaponLaunched_Ally_Sneak_Attack, EVA_SuperweaponLaunched_Enemy_Sneak_Attack
};
class Eva
{
public:
	void setShouldPlay(EvaMessage messageToPlay);
};
extern Eva *TheEva;
#endif
