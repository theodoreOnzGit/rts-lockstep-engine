/* Stand-in for GameEngine/Include/GameClient/ControlBar.h (client), Module build only (differential
** tests, p1l9-example): markUIDirty, which the modules call to redraw the command bar. Defined in the
** wrapper, where it logs an event. Renamed so it cannot clash with another unit's ControlBar stand-in. */
#ifndef SHIM_MODULE_CONTROLBAR_H
#define SHIM_MODULE_CONTROLBAR_H
#define ControlBar ModTestControlBar
#define TheControlBar ModTestTheControlBar
class ControlBar
{
public:
	void markUIDirty();
};
extern ControlBar *TheControlBar;
#endif
