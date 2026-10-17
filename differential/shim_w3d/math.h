/* Shim for <math.h>, differential tests only: the C semantics of
** differential/shim/math.h (see there), for the W3D builds, which cannot put
** differential/shim on their include path (its always.h would replace
** WWLib's). */
#ifndef SHIM_MATH_H
#define SHIM_MATH_H
#define _GLIBCXX_INCLUDE_NEXT_C_HEADERS
#include_next <math.h>
#undef _GLIBCXX_INCLUDE_NEXT_C_HEADERS
#endif
