/* Stand-in for Common/KindOf.h in the sides/scripts build. The real header
** defines enum KindOfType in full, which cannot coexist with the opaque
** enum KindOfType of the shim STLTypedefs.h that Xfer.h needs (and the chunk
** prelude includes first). Scripts.cpp uses the type only as the int range
** [KINDOF_FIRST, KINDOF_COUNT) of a KIND_OF_PARAM (in getUiText, not run) and
** KindOfMaskType's getNameFromSingleBit and getBitNames, the shim class of
** differential/shim/Common/BitFlagsIO.h; both forward to the REAL name
** table (KindOf.cpp) through wrap_flags.cpp's ref_kindof_name_from_single_bit.
** KINDOF_COUNT is the number of names in that table (checked by wrap_sides.cpp
** at start-up). */
#ifndef SHIM_SIDES_KINDOF_H
#define SHIM_SIDES_KINDOF_H
#include "Common/BitFlagsIO.h"
static const int KINDOF_FIRST = 0;
static const int KINDOF_COUNT = 116; /* checked against the table: ref_sides_selfcheck */
#endif
