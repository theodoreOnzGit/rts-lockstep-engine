/* Shim for GameEngine/Include/Common/UnicodeString.h, STLTypedefs build only
** (differential tests). The pointer-sized UnicodeString of shim_dict, with
** one member more: compareNoCase, which less_than_nocase<UnicodeString>
** (STLTypedefs.h) calls. No UnicodeString is compared by this build: the
** UnicodeString comparators are tested in their own build with the real
** header (differential/wrap_stl_typedefs_unicode.cpp, build.rs
** "p1w2-dictmove"). The body aborts if it is ever reached rather than
** inventing an order. */
#ifndef SHIM_STL_UNICODESTRING_H
#define SHIM_STL_UNICODESTRING_H
#include <stdlib.h>
#include <vector>
#include "Lib/BaseType.h"
class UnicodeString
{
public:
	static const UnicodeString TheEmptyString;
	UnicodeString() : m_p(0) {}
	UnicodeString(const WideChar *s, int n) : m_p(n > 0 ? new std::vector<WideChar>(s, s + n) : 0) {}
	UnicodeString(const UnicodeString &o) : m_p(o.m_p ? new std::vector<WideChar>(*o.m_p) : 0) {}
	~UnicodeString() { delete m_p; }
	UnicodeString &operator=(const UnicodeString &o)
	{
		if (this != &o) { std::vector<WideChar> *n = o.m_p ? new std::vector<WideChar>(*o.m_p) : 0; delete m_p; m_p = n; }
		return *this;
	}
	void clear() { delete m_p; m_p = 0; }
	int getLength() const { return m_p ? (int)m_p->size() : 0; }
	WideChar getCharAt(int i) const { return (*m_p)[i]; }
	int compareNoCase(const UnicodeString &) const { abort(); }
private:
	std::vector<WideChar> *m_p;
};
#endif
