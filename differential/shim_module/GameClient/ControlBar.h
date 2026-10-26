/* Stand-in for GameEngine/Include/GameClient/ControlBar.h (client), Module build only (differential
** tests, p1l9-example): markUIDirty, which the modules call to redraw the command bar. Defined in the
** wrapper, where it logs an event. Renamed so it cannot clash with another unit's ControlBar stand-in. */
#ifndef SHIM_MODULE_CONTROLBAR_H
#define SHIM_MODULE_CONTROLBAR_H
#define ControlBar ModTestControlBar
#define TheControlBar ModTestTheControlBar
/* --- p1w13-sonnet --- (batch R69: CommandButtonHuntUpdate) GUICommandType, verbatim from ControlBar.h (ALLOW_SURRENDER is not defined), and
** what the logic reads of a command button and a command set: the test's (differential/l9_command_button_hunt_r69.inc), which also defines
** findCommandSet. CommandSet::getCommandButton is ControlBar.cpp's without the game logic's control bar overrides (none in the test).
** CommandButton and CommandSet are renamed by module_prelude.h. */
#include "Common/AsciiString.h"
#include "Common/GameCommon.h"
class SpecialPowerTemplate;
enum GUICommandType
{
	GUI_COMMAND_NONE = 0,									///< invalid command
	GUI_COMMAND_DOZER_CONSTRUCT,					///< dozer construct
	GUI_COMMAND_DOZER_CONSTRUCT_CANCEL,		///< cancel a dozer construction process
	GUI_COMMAND_UNIT_BUILD,								///< build a unit
	GUI_COMMAND_CANCEL_UNIT_BUILD,				///< cancel a unit build
	GUI_COMMAND_PLAYER_UPGRADE,						///< put an upgrade that applies to the player in the queue
	GUI_COMMAND_OBJECT_UPGRADE,						///< put an object upgrade in the queue
	GUI_COMMAND_CANCEL_UPGRADE,						///< cancel an upgrade
	GUI_COMMAND_ATTACK_MOVE,							///< attack move command
	GUI_COMMAND_GUARD,										///< guard command
	GUI_COMMAND_GUARD_WITHOUT_PURSUIT,		///< guard command, no pursuit out of guard area
	GUI_COMMAND_GUARD_FLYING_UNITS_ONLY,	///< guard command, ignore nonflyers
	GUI_COMMAND_STOP,											///< stop moving
	GUI_COMMAND_WAYPOINTS,								///< create a set of waypoints for this unit
	GUI_COMMAND_EXIT_CONTAINER,						///< an inventory box for a container like a structure or transport
	GUI_COMMAND_EVACUATE,									///< dump all our contents
	GUI_COMMAND_EXECUTE_RAILED_TRANSPORT,	///< execute railed transport sequence
	GUI_COMMAND_BEACON_DELETE,						///< delete a beacon
	GUI_COMMAND_SET_RALLY_POINT,					///< set rally point for a structure
	GUI_COMMAND_SELL,											///< sell a structure
	GUI_COMMAND_FIRE_WEAPON,							///< fire a weapon
	GUI_COMMAND_SPECIAL_POWER,						///< do a special power
	GUI_COMMAND_PURCHASE_SCIENCE,					///< purchase science
	GUI_COMMAND_HACK_INTERNET,						///< gain income from the ether (by hacking the internet)
	GUI_COMMAND_TOGGLE_OVERCHARGE,				///< Overcharge command for power plants
#ifdef ALLOW_SURRENDER
	GUI_COMMAND_POW_RETURN_TO_PRISON,			///< POW Truck, return to prison
#endif
	GUI_COMMAND_COMBATDROP,								///< rappel contents to ground or bldg
	GUI_COMMAND_SWITCH_WEAPON,						///< switch weapon use

	//Context senstive command modes
	GUICOMMANDMODE_HIJACK_VEHICLE,
	GUICOMMANDMODE_CONVERT_TO_CARBOMB,
	GUICOMMANDMODE_SABOTAGE_BUILDING,
#ifdef ALLOW_SURRENDER
	GUICOMMANDMODE_PICK_UP_PRISONER,			///< POW Truck assigned to pick up a specific prisoner
#endif

	// context-insensitive command mode(s)
	GUICOMMANDMODE_PLACE_BEACON,

	GUI_COMMAND_SPECIAL_POWER_FROM_SHORTCUT,			///< do a special power from localPlayer's command center, regardless of selection
	GUI_COMMAND_SPECIAL_POWER_CONSTRUCT,					///< do a special power using the construct building interface
	GUI_COMMAND_SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT, ///< do a shortcut special power using the construct building interface
	
	GUI_COMMAND_SELECT_ALL_UNITS_OF_TYPE,

	// add more commands here, don't forget to update the string command list below too ...

	GUI_COMMAND_NUM_COMMANDS							// keep this last
};
enum { MAX_COMMANDS_PER_SET = 18 };  // user interface max is 14 (but internally it's 18 for script only buttons!)
class CommandButton
{
public:
	const AsciiString& getName() const { return m_name; }
	GUICommandType getCommandType() const { return m_command; }
	const SpecialPowerTemplate* getSpecialPowerTemplate() const { return m_specialPower; }
	WeaponSlotType getWeaponSlot() const { return m_weaponSlot; }
	AsciiString m_name;
	GUICommandType m_command;
	WeaponSlotType m_weaponSlot;
	const SpecialPowerTemplate *m_specialPower;
};
class CommandSet
{
public:
	const AsciiString& getName() const { return m_name; }
	const CommandButton* getCommandButton(Int i) const { return m_command[i]; }
	AsciiString m_name;
	const CommandButton *m_command[ MAX_COMMANDS_PER_SET ];
};
/* --- end p1w13-sonnet --- */
class ControlBar
{
public:
	void markUIDirty();
	const CommandSet *findCommandSet( const AsciiString& name );   /* batch R69: the test's */
};
extern ControlBar *TheControlBar;
#endif
