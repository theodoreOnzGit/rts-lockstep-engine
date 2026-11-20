/* Forced include (-include) for the net4 wrappers that use upstream's AsciiString (#215, group net4): a `strcpy` that copies forward one byte at a time, as
** MSVC's does. AsciiString::nextToken ends with `this->set(end)` where `end` points INSIDE the string's own buffer (source above destination, overlapping);
** that is undefined for `strcpy`, and glibc's vectorised one corrupts the text where MSVC's byte loop gives the remainder the code expects. A stand-in for
** the C runtime, never upstream code. */
#ifndef NET4_PRELUDE_H
#define NET4_PRELUDE_H
#include <string.h>
static inline char *net4_strcpy(char *d, const char *s) { char *r = d; while ((*d++ = *s++) != 0) {} return r; }
#define strcpy net4_strcpy
#endif
