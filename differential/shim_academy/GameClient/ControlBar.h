/* Stand-in for GameEngine/Include/GameClient/ControlBar.h (client), academy tests
** only: findCommandSet (the dozer's command set, which init asks for and then
** discards, see the academy module documentation), CommandSet and CommandButton. */
#ifndef SHIM_ACADEMY_CONTROLBAR_H
#define SHIM_ACADEMY_CONTROLBAR_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#define MAX_COMMANDS_PER_SET 18
class ThingTemplate;
class CommandButton
{
public:
	CommandButton() : m_thing(NULL) {}
	const ThingTemplate *getThingTemplate() const { return m_thing; }
	const ThingTemplate *m_thing;
};
class CommandSet
{
public:
	CommandSet() {}
	const CommandButton *getCommandButton(Int i) const { return &m_buttons[i]; }
	CommandButton m_buttons[MAX_COMMANDS_PER_SET];
};
class ControlBar
{
public:
	const CommandSet *findCommandSet(AsciiString name) const;
};
extern ControlBar *TheControlBar;
#endif
