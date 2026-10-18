/* Stand-in for GameEngine/Include/GameClient/FXList.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate): the lookup that
** UnitSpecificFX makes. The FXList is the client's; the lookup answers every
** name but None with an object of its own (the Rust keeps the name).
** (Renamed Tt* by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_FXLIST_H
#define SHIM_TT_FXLIST_H
#include "Common/AsciiString.h"
class FXList
{
public:
	AsciiString m_name;
};
class FXListStore
{
public:
	const FXList *findFXList(const char *name) const;
};
extern FXListStore *TheFXListStore;
#endif
