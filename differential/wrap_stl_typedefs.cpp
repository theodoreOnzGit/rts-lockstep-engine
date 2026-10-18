/* Differential-test wrapper for GameEngine/Include/Common/STLTypedefs.h
** (upstream commit 0a05454d). The header is included unchanged, through the
** STLTypedefs build of build.rs (differential/shim_stl, with the real
** AsciiString.h renamed by macro so it cannot clash with another build's).
**
** AsciiString.cpp (unchanged) is compiled in the same build, for
** removeLastChar and the other out-of-line AsciiString functions.
**
** ref_stl_compare compares two AsciiStrings made from ka/kb:
**   kind 0: AsciiString() (no buffer)
**   kind 1: AsciiString(s)
**   kind 2: AsciiString("x") then removeLastChar(): an empty string that
**           keeps its buffer (s is not used)
** and writes four 0/1 results: rts::less_than_nocase(a, b), rts::equal_to(a, b),
** upstream's operator==(a, b), and rts::less_than_nocase(b, a). */
#include "Common/STLTypedefs.h"

/* AsciiString.cpp is compiled in this build with the same renames (build.rs),
** so the allocator it allocates from, and the empty UnicodeString, are
** defined here, as in wrap_ascii_string.cpp and wrap_dict.cpp. */
namespace {
DynamicMemoryAllocator theShimAllocator;
}
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theShimAllocator;
const UnicodeString UnicodeString::TheEmptyString;

namespace {
AsciiString make(const char *s, int kind)
{
	if (kind == 0) return AsciiString();
	if (kind == 2) {
		AsciiString a("x");
		a.removeLastChar();
		return a;
	}
	return AsciiString(s);
}
}

extern "C" void ref_stl_compare(const char *a, int ka, const char *b, int kb, int *out)
{
	AsciiString sa = make(a, ka);
	AsciiString sb = make(b, kb);
	out[0] = rts::less_than_nocase<AsciiString>()(sa, sb) ? 1 : 0;
	out[1] = rts::equal_to<AsciiString>()(sa, sb) ? 1 : 0;
	out[2] = (sa == sb) ? 1 : 0;
	out[3] = rts::less_than_nocase<AsciiString>()(sb, sa) ? 1 : 0;
}
