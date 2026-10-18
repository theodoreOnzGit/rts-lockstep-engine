/* Stand-in for GameEngine/Include/GameClient/Drawable.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate). (Renamed TtDrawable by
** thingtemplate_prelude.h.) */
#ifndef SHIM_TT_DRAWABLE_H
#define SHIM_TT_DRAWABLE_H
#include "Lib/BaseType.h"
enum DrawableStatus
{
	DRAWABLE_STATUS_NONE = 0x00000000
};
class Drawable
{
public:
	int m_unused;
};
#endif
