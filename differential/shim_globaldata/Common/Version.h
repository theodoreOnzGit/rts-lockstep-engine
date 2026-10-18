/* Shim for GameEngine/Include/Common/Version.h, differential tests only:
** GlobalData's constructor adds TheVersion's number to the executable CRC if
** TheVersion is non-null. wrap_globaldata.cpp defines it as null. */
#ifndef SHIM_GLOBALDATA_VERSION_H
#define SHIM_GLOBALDATA_VERSION_H
#include "Lib/BaseType.h"
class Version
{
public:
	UnsignedInt getVersionNumber() const { return 0; }
};
extern Version *TheVersion;
#endif
