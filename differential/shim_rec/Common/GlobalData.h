/* Stand-in for Common/GlobalData.h, differential tests only (differential/wrap_recorder.cpp): the pending file, the map name, the two CRCs and the user data path. */
#ifndef SHIM_REC_GLOBALDATA_H
#define SHIM_REC_GLOBALDATA_H
#include "Common/AsciiString.h"
class GlobalData
{
public:
	AsciiString m_pendingFile, m_mapName, m_userData;
	UnsignedInt m_exeCRC, m_iniCRC;
	AsciiString getPath_UserData() const { return m_userData; }
};
extern GlobalData *TheGlobalData;
extern GlobalData *TheWritableGlobalData;
#endif
