/* Shim for GameEngine/Include/Common/UnicodeString.h, for the Dict build only
** (differential tests): one pointer, NULL when empty, for the same reasons
** as shim_dict/Common/AsciiString.h. WideChar is 16 bits (the shim
** Lib/BaseType.h), as on Win32. */
#ifndef SHIM_DICT_UNICODESTRING_H
#define SHIM_DICT_UNICODESTRING_H
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
private:
	std::vector<WideChar> *m_p;
};
#endif
