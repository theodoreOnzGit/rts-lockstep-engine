/* Shim, differential tests only: the KindOfMaskType name lookups that
** Xfer::xferKindOf calls (NOT translated, NOT tested; abort if reached). */
#ifndef SHIM_BITFLAGSIO_H
#define SHIM_BITFLAGSIO_H
#include "Common/AsciiString.h"
class KindOfMaskType
{
public:
	static AsciiString getNameFromSingleBit(int);
	static int getSingleBitFromName(const char *);
};
#endif
