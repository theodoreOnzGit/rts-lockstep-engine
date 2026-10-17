/* Shim for GameEngine/Include/Common/AsciiString.h, differential tests only.
** The real class is reference-counted on the engine's memory pools. What the
** Xfer and archive files use is reproduced here on std::string with the
** upstream meaning: construction, assignment, str(), getLength(), isEmpty(),
** isNotEmpty(), set(const char*) (copies a NUL-terminated C string), clear(),
** concat(), toLower() (tolower per char), find()/endsWith() (strchr,
** strncmp at the end), compare/compareNoCase (strcmp/_stricmp), the
** comparison operators (strcmp), TheEmptyString, and nextToken(), whose body
** is copied from AsciiString.cpp lines 368-396 with its helpers skipSeps /
** skipNonSeps (lines 55-68): on an empty string it returns false and leaves
** the token unchanged. */
#ifndef SHIM_ASCIISTRING_H
#define SHIM_ASCIISTRING_H
#include <string>
#include <string.h>
#include <strings.h>
#include <ctype.h>
class AsciiString
{
public:
	static const AsciiString TheEmptyString;
	AsciiString() {}
	AsciiString(const char *s) : m_s(s ? s : "") {}
	const char *str() const { return m_s.c_str(); }
	int getLength() const { return (int)m_s.size(); }
	bool isEmpty() const { return m_s.empty(); }
	bool isNotEmpty() const { return !m_s.empty(); }
	void set(const char *s) { m_s = s ? s : ""; }
	void clear() { m_s.clear(); }
	void concat(const AsciiString &a) { m_s += a.m_s; }
	void concat(const char *a) { m_s += a; }
	void concat(char c) { m_s += c; }
	void toLower() { for (size_t i = 0; i < m_s.size(); ++i) m_s[i] = (char)tolower(m_s[i]); }
	const char *find(char c) const { return strchr(str(), c); }
	bool endsWith(const char *p) const
	{
		if (*p == 0) return true;
		int lenThis = getLength(), lenThat = (int)strlen(p);
		if (lenThis < lenThat) return false;
		return strncmp(str() + lenThis - lenThat, p, lenThat) == 0;
	}
	int compare(const AsciiString &o) const { return strcmp(str(), o.str()); }
	int compareNoCase(const AsciiString &o) const { return strcasecmp(str(), o.str()); }
	bool nextToken(AsciiString *tok, const char *seps)
	{
		if (this->isEmpty() || tok == this)
			return false;
		if (seps == NULL)
			seps = " \n\r\t";
		const char *start = str();
		while (*start && strchr(seps, *start) != NULL) ++start;
		const char *end = start;
		while (*end && strchr(seps, *end) == NULL) ++end;
		if (end > start)
		{
			tok->m_s.assign(start, end - start);
			std::string rest(end);
			m_s = rest;
			return true;
		}
		else
		{
			this->clear();
			tok->clear();
			return false;
		}
	}
	/* test helper (not upstream): set from bytes that may contain NUL */
	void setBytes(const char *p, int n) { m_s.assign(p, n); }
private:
	std::string m_s;
};
inline bool operator==(const AsciiString &a, const AsciiString &b) { return strcmp(a.str(), b.str()) == 0; }
inline bool operator!=(const AsciiString &a, const AsciiString &b) { return strcmp(a.str(), b.str()) != 0; }
inline bool operator<(const AsciiString &a, const AsciiString &b) { return strcmp(a.str(), b.str()) < 0; }
#endif
