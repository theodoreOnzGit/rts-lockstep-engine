/* Shim for <math.h>, differential tests only. It selects C semantics for the
** global math functions, as the MSVC headers the game was built with had:
** `acos(float)` widens to double, calls the double `acos` and the caller
** rounds the result back. g++'s libstdc++ <math.h> instead brings the
** `float` overloads of <cmath> (std::acos(float) == acosf) into the global
** namespace, which would silently turn upstream's `(float)acos(val)` into an
** `acosf` call. _GLIBCXX_INCLUDE_NEXT_C_HEADERS makes libstdc++'s wrapper
** forward straight to the C library header. Explicit float calls (`sinf`,
** `cosf`, `floorf`, ...) are unaffected. The double functions are then bound
** to the Rust translation's petir functions in the test binaries
** (tests/support/mod.rs), as the float ones are. */
#ifndef SHIM_MATH_H
#define SHIM_MATH_H
#define _GLIBCXX_INCLUDE_NEXT_C_HEADERS
#include_next <math.h>
#undef _GLIBCXX_INCLUDE_NEXT_C_HEADERS
#endif
