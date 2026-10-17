/* Shim for GameEngine/Include/Common/AsciiString.h, differential tests only.
** The real class is reference-counted on the engine's memory pools. The Xfer
** files use only construction, assignment, str(), getLength(), set(const
** char*) and clear(), reproduced here on std::string with the same meaning
** (set() copies a NUL-terminated C string). */
#ifndef SHIM_ASCIISTRING_H
#define SHIM_ASCIISTRING_H
#include <string>
class AsciiString
{
public:
	AsciiString() {}
	AsciiString(const char *s) : m_s(s ? s : "") {}
	const char *str() const { return m_s.c_str(); }
	int getLength() const { return (int)m_s.size(); }
	void set(const char *s) { m_s = s ? s : ""; }
	void clear() { m_s.clear(); }
	/* test helper (not upstream): set from bytes that may contain NUL */
	void setBytes(const char *p, int n) { m_s.assign(p, n); }
private:
	std::string m_s;
};
#endif
