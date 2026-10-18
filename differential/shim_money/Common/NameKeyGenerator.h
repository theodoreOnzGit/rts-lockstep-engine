/* Stand-in for GameEngine/Include/Common/NameKeyGenerator.h, money and
** handicap tests only. Handicap::readFromDict interns the names it looks up
** through the global `TheNameKeyGenerator`; here nameToKey is a table of
** first-seen names (defined in wrap_handicap.cpp), and the test logs the
** names asked for, which the Rust side checks against its own. */
#ifndef SHIM_MONEY_NAMEKEYGENERATOR_H
#define SHIM_MONEY_NAMEKEYGENERATOR_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
typedef UnsignedInt NameKeyType;
class NameKeyGenerator
{
public:
	NameKeyType nameToKey(const AsciiString &name);
};
extern NameKeyGenerator *TheNameKeyGenerator;
#endif
