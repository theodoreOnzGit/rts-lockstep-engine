/* Stand-in for GameClient/Display.h and the pieces of GlobalData.h that FrameMetrics.cpp reaches (differential tests only,
** differential/wrap_draft_netlogic.cpp, #215): `TheDisplay->getAverageFPS()`, `TheGlobalData->m_networkFPSHistoryLength`,
** `m_networkLatencyHistoryLength` and `m_networkCushionHistoryLength` (all UnsignedInt there), and `timeGetTime()` (a DWORD). */
#ifndef SHIM_NETLOGIC_DISPLAY_H
#define SHIM_NETLOGIC_DISPLAY_H
#include "Lib/BaseType.h"
class Display { public: virtual Real getAverageFPS( void ) = 0; };
extern Display *TheDisplay;
class GlobalData { public: UnsignedInt m_networkFPSHistoryLength; UnsignedInt m_networkLatencyHistoryLength; UnsignedInt m_networkCushionHistoryLength; };
extern GlobalData *TheGlobalData;
unsigned int timeGetTime();
#endif
