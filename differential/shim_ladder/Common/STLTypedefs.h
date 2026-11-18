/* Forwarder plus the AsciiString list typedefs of the real STLTypedefs.h (lines 83-85), ladder build only (GitHub #215, differential
** tests). */
#ifndef SHIM_LADDER_STLTYPEDEFS_H
#define SHIM_LADDER_STLTYPEDEFS_H
#include "../../shim/Common/STLTypedefs.h"
#include <list>
typedef std::list< AsciiString >					AsciiStringList;
typedef std::list< AsciiString >::iterator			AsciiStringListIterator;
typedef std::list< AsciiString >::const_iterator	AsciiStringListConstIterator;
#endif
