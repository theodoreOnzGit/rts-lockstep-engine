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
	/* --- p1w4-sides --- */
	static const char **getBitNames(); /* BitFlags<KINDOF_COUNT>::getBitNames: the NULL-terminated name table (defined in wrap_sides.cpp) */
	/* --- end p1w4-sides --- */
};
#endif
