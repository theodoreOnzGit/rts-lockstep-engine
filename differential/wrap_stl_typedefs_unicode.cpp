/* Differential-test wrapper for GameEngine/Include/Common/STLTypedefs.h, the
** UnicodeString comparator (upstream commit 0a05454d). The sibling of
** differential/wrap_stl_typedefs.cpp: that build's UnicodeString is a stand-in
** whose compareNoCase aborts, so this one is its own build (build.rs,
** "p1w2-dictmove"), with the REAL UnicodeString.h and UnicodeString.cpp
** unchanged (-fshort-wchar, the 16-bit wide C runtime of
** shim_unicode/unicode_prelude.h, via shim_stl_unicode/stl_unicode_prelude.h),
** and the REAL AsciiString.h (shim_ascii) that STLTypedefs.h declares its
** AsciiString functors against. Classes are renamed by macro, as in the other
** builds. UnicodeString.cpp defines UnicodeString::TheEmptyString itself.
**
** ref_stl_compare_unicode compares two UnicodeStrings made from the units a
** and b (no 0 unit; n units each):
**   kind 0: UnicodeString() (no buffer)
**   kind 1: UnicodeString(units) (the text)
**   kind 2: UnicodeString(L"x") then removeLastChar(): an empty string that
**           keeps its buffer (the units are not used)
** and writes three 0/1 results: rts::less_than_nocase(a, b),
** rts::equal_to(a, b) and rts::less_than_nocase(b, a). upstream has no
** equal_to<UnicodeString> specialisation, so the generic functor (operator==,
** wcscmp of str()) is what is called. */
#include "Common/STLTypedefs.h"

#include <vector>

/* TheDynamicMemoryAllocator is renamed by build.rs; this build's
** UnicodeString.cpp allocates from it. */
namespace {
DynamicMemoryAllocator theShimAllocator;
}
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theShimAllocator;

namespace {
UnicodeString make(const unsigned short *u, int n, int kind)
{
	if (kind == 0) return UnicodeString();
	if (kind == 2) {
		UnicodeString x(L"x");
		x.removeLastChar();
		return x;
	}
	std::vector<WideChar> buf;
	for (int i = 0; i < n; ++i) buf.push_back((WideChar)u[i]);
	buf.push_back(0);
	return UnicodeString(&buf[0]);
}
}

extern "C" void ref_stl_compare_unicode(const unsigned short *a, int na, int ka, const unsigned short *b, int nb, int kb, int *out)
{
	UnicodeString sa = make(a, na, ka);
	UnicodeString sb = make(b, nb, kb);
	out[0] = rts::less_than_nocase<UnicodeString>()(sa, sb) ? 1 : 0;
	out[1] = rts::equal_to<UnicodeString>()(sa, sb) ? 1 : 0;
	out[2] = rts::less_than_nocase<UnicodeString>()(sb, sa) ? 1 : 0;
}
