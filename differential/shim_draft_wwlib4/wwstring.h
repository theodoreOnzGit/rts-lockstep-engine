/* (#215, group wwlib4) stand-in for WWLib/wwstring.h: the part of StringClass that mixfile.cpp, definition.cpp and definitionmgr.cpp use.
** Like the real class it is ONE pointer to a heap buffer (the callers move StringClass objects by memcpy: qsort swaps FileInfo entries,
** Vector.H's Insert memmoves), so it must not hold a std::string (whose small-string buffer points into itself). The buffer is a
** NUL-terminated char array: Get_Buffer(n) makes room for n chars (the callers fill n, the last being the NUL), the conversion to
** const char * is the C string (it stops at the first NUL), Get_Length() is strlen of it, += appends C text, Format is vsnprintf,
** Compare_No_Case is stricmp. */
#ifndef WWLIB4_WWSTRING_H
#define WWLIB4_WWSTRING_H
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <stdarg.h>
#include <stdio.h>
class StringClass {
public:
	char *p;
	void set(const char *c, size_t extra = 0) { size_t n = c ? strlen(c) : 0; p = (char *)malloc(n + extra + 1); if (n) memcpy(p, c, n); p[n] = 0; }
	StringClass(void) { set(""); }
	StringClass(bool) { set(""); }
	StringClass(const char *c) { set(c); }
	StringClass(const char *c, bool) { set(c); }
	StringClass(const StringClass &o) { set(o.p); }
	~StringClass(void) { free(p); }
	StringClass &operator=(const StringClass &o) { if (this != &o) { char *old = p; set(o.p); free(old); } return *this; }
	StringClass &operator=(const char *c) { char *old = p; set(c); free(old); return *this; }
	StringClass &operator+=(const char *c) { if (c) { size_t a = strlen(p), b = strlen(c); p = (char *)realloc(p, a + b + 1); memcpy(p + a, c, b + 1); } return *this; }
	operator const char *(void) const { return p; }
	char *Get_Buffer(int n) { free(p); p = (char *)calloc((size_t)n + 1, 1); return p; }
	char *Peek_Buffer(void) { return p; }
	int Get_Length(void) const { return (int)strlen(p); }
	int Compare_No_Case(const char *o) const { return strcasecmp(p, o); }
	int Format(const char *fmt, ...) { char b[1024]; va_list a; va_start(a, fmt); int n = vsnprintf(b, sizeof(b), fmt, a); va_end(a); *this = b; return n; }
	bool operator==(const StringClass &o) const { return strcmp(p, o.p) == 0; }
};
#endif
