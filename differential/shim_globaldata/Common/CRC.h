/* Shim for GameEngine/Include/Common/CRC.h (file crc.h; Windows is case
** insensitive), differential tests only. The real header's shipped branch is
** MSVC x86 inline assembly (`_asm`), which clang cannot assemble for x86_64,
** and its other branch (`_DEBUG`) would turn on GlobalData's debug members.
** The class is reproduced with the C++ loop that crc.h keeps "for reference
** purposes" (lines 90-103), which its own comment says the assembly
** matches. GlobalData's constructor feeds it the bytes of files that the
** test file system does not have, so computeCRC is never called: get() is
** the CRC of no bytes, 0. */
#ifndef SHIM_GLOBALDATA_CRC_H
#define SHIM_GLOBALDATA_CRC_H
#include "Lib/BaseType.h"
class CRC
{
public:
	CRC(void) { crc = 0; }
	void computeCRC(const void *buf, Int len)
	{
		if (!buf || len < 1)
			return;
		for (const UnsignedByte *uintPtr = (const UnsignedByte *)buf; len > 0; len--, uintPtr++)
		{
			int hibit = (crc & 0x80000000) ? 1 : 0;
			crc <<= 1;
			crc += *uintPtr;
			crc += hibit;
		}
	}
	void clear(void) { crc = 0; }
	UnsignedInt get(void) const { return crc; }
private:
	UnsignedInt crc;
};
#endif
