/* Stand-in for GameEngine/Include/Common/GlobalData.h (layer 1), academy tests
** only: TheGlobalData->m_useAlternateMouse, the one field the advice reads. */
#ifndef SHIM_ACADEMY_GLOBALDATA_H
#define SHIM_ACADEMY_GLOBALDATA_H
#include "Lib/BaseType.h"
class GlobalData
{
public:
	Bool m_useAlternateMouse;
};
extern GlobalData *TheGlobalData;
#endif
