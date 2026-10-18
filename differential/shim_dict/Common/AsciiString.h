/* Shim for GameEngine/Include/Common/AsciiString.h, for the Dict build only
** (differential tests). Dict.cpp stores each AsciiString in the `void*`
** slot of a pair (Dict.cpp asserts sizeof(AsciiString) <= sizeof(void*)),
** zero-fills new pairs, and moves pairs by bitwise copies (sortPairs). So,
** like the real class (one pointer to a reference-counted buffer, NULL for
** the empty string), this one is a single pointer, NULL when empty, with
** the value semantics the real one has: construction from a C string,
** copy, assignment, clear(), str(), getLength(), isEmpty(), compareNoCase(). */
#ifndef SHIM_DICT_ASCIISTRING_H
#define SHIM_DICT_ASCIISTRING_H
#include <string>
#include <string.h>
#include <strings.h>
class AsciiString
{
public:
	static const AsciiString TheEmptyString;
	AsciiString() : m_p(0) {}
	AsciiString(const char *s) : m_p(s && *s ? new std::string(s) : 0) {}
	AsciiString(const AsciiString &o) : m_p(o.m_p ? new std::string(*o.m_p) : 0) {}
	~AsciiString() { delete m_p; }
	AsciiString &operator=(const AsciiString &o)
	{
		if (this != &o) { std::string *n = o.m_p ? new std::string(*o.m_p) : 0; delete m_p; m_p = n; }
		return *this;
	}
	void clear() { delete m_p; m_p = 0; }
	const char *str() const { return m_p ? m_p->c_str() : ""; }
	int getLength() const { return m_p ? (int)m_p->size() : 0; }
	bool isEmpty() const { return getLength() == 0; }
	/* for the comparator of the shim STLTypedefs.h (not used by Dict) */
	int compareNoCase(const AsciiString &o) const { return strcasecmp(str(), o.str()); }
private:
	std::string *m_p;
};
#endif
