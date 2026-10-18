/* Stand-in for Common/AsciiString.h, UnicodeString build only (differential
** tests): UnicodeString::translate reads its argument through getLength()
** and getCharAt() only. `char` is signed here as on MSVC, so a byte from
** 0x80 widens to 0xff80.. in translate's `(WideChar)getCharAt(i)`. */
#ifndef SHIM_UNICODE_ASCIISTRING_H
#define SHIM_UNICODE_ASCIISTRING_H
#include <string>
class AsciiString
{
public:
	AsciiString() {}
	explicit AsciiString(const char *s) : m_s(s) {}
	int getLength() const { return (int)m_s.size(); }
	char getCharAt(int i) const { return m_s[(size_t)i]; }
private:
	std::string m_s;
};
#endif
