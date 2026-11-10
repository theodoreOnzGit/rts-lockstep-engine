/* Shim for GameEngine/Include/Common/NameKeyGenerator.h, differential/wrap_wm.cpp only (#70): keys 1, 2, ... in order
** of first use (wrap_wm.cpp). */
#ifndef SHIM_WM_NAMEKEYGENERATOR_H
#define SHIM_WM_NAMEKEYGENERATOR_H
#include "Common/AsciiString.h"
typedef UnsignedInt NameKeyType;
enum { NAMEKEY_INVALID = 0 };
class NameKeyGenerator
{
public:
	NameKeyType nameToKey(const AsciiString &name);
	NameKeyType nameToKey(const char *name);
};
extern NameKeyGenerator *TheNameKeyGenerator;
#endif
