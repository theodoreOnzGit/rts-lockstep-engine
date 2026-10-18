/* Stand-in for GameEngine/Include/Common/INI.h, SubsystemInterface build only
** (differential tests). SubsystemInterfaceList::initSubsystem constructs an
** INI and calls load/loadDirectory only for the paths it is given; the
** wrapper gives NULL for all three, so these bodies are never reached (they
** abort if they are, rather than loading nothing silently). The real INI.h
** needs most of the engine (FieldParse, the block table, Xfer). */
#ifndef SHIM_SUBSYS_INI_H
#define SHIM_SUBSYS_INI_H
#include <stdlib.h>
#include "Common/STLTypedefs.h"
class Xfer;
enum INILoadType
{
	INI_LOAD_INVALID = 0,
	INI_LOAD_OVERWRITE = 1,
	INI_LOAD_CREATE_OVERRIDES = 2,
	INI_LOAD_MULTIFILE = 3
};
class INI
{
public:
	INI() {}
	~INI() {}
	void load(AsciiString, INILoadType, Xfer *) { abort(); }
	void loadDirectory(AsciiString, Bool, INILoadType, Xfer *) { abort(); }
};
#endif
