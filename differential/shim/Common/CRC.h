/* Shim, differential tests only: upstream includes "Common/CRC.h" but the file
** on disk is Common/crc.h (Windows file systems are case-insensitive). This
** forwards to the unchanged upstream header. */
#include "Common/crc.h"
