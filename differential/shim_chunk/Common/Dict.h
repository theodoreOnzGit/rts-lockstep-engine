/* Shim for GameEngine/Include/Common/Dict.h (upstream commit 0a05454d),
** differential tests only. The real Dict stores each value, an AsciiString
** or UnicodeString included, inside a pointer-sized slot ("it assumes all
** the data types will fit into a pointer", Dict.cpp line 228); the
** std::string-based AsciiString/UnicodeString shims do not fit, so Dict.cpp
** cannot be compiled against them. This stand-in keeps what
** DataChunkInput::readDict (compiled unchanged) relies on, with the meaning
** of Dict.cpp: the DataType values (Dict.h lines 74-82), the constructor's
** preallocation, which throws ERROR_OUT_OF_MEMORY above MAX_LEN = 32767
** pairs (Dict.cpp lines 239-240 and 160-161), and the setters, which add a
** pair for a new key or replace the type and value of an existing one
** (setPrep, Dict.cpp lines 419-433). Pairs are kept sorted by key, as
** sortPairs does. The getNth* accessors are for the test wrapper. */
#ifndef SHIM_DICT_H
#define SHIM_DICT_H
#include <map>
#include "Common/Errors.h"
#include "Common/NameKeyGenerator.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
class Dict
{
public:
	enum { MAX_LEN = 32767 };
	enum DataType
	{
		DICT_NONE = -1,
		DICT_BOOL = 0,
		DICT_INT,
		DICT_REAL,
		DICT_ASCIISTRING,
		DICT_UNICODESTRING
	};
	struct Value { DataType type; Bool b; Int i; Real r; AsciiString a; UnicodeString u; };
	Dict(Int numPairsToPreAllocate = 0)
	{
		if (numPairsToPreAllocate > MAX_LEN)
			throw ERROR_OUT_OF_MEMORY;
	}
	void setBool(NameKeyType key, Bool value) { Value &v = m_pairs[key]; v = Value(); v.type = DICT_BOOL; v.b = value; }
	void setInt(NameKeyType key, Int value) { Value &v = m_pairs[key]; v = Value(); v.type = DICT_INT; v.i = value; }
	void setReal(NameKeyType key, Real value) { Value &v = m_pairs[key]; v = Value(); v.type = DICT_REAL; v.r = value; }
	void setAsciiString(NameKeyType key, const AsciiString &value) { Value &v = m_pairs[key]; v = Value(); v.type = DICT_ASCIISTRING; v.a = value; }
	void setUnicodeString(NameKeyType key, const UnicodeString &value) { Value &v = m_pairs[key]; v = Value(); v.type = DICT_UNICODESTRING; v.u = value; }
	Int getPairCount() const { return (Int)m_pairs.size(); }
	/* the n-th pair in key order (DataChunkOutput::writeDict, not tested) */
	NameKeyType getNthKey(Int n) const { return nth(n).first; }
	DataType getNthType(Int n) const { return nth(n).second.type; }
	Bool getNthBool(Int n) const { return nth(n).second.b; }
	Int getNthInt(Int n) const { return nth(n).second.i; }
	Real getNthReal(Int n) const { return nth(n).second.r; }
	AsciiString getNthAsciiString(Int n) const { return nth(n).second.a; }
	UnicodeString getNthUnicodeString(Int n) const { return nth(n).second.u; }
	const std::map<NameKeyType, Value> &pairs() const { return m_pairs; }
	/* --- p1w4-sides --- */
	/* clear() and the keyed getters of Dict.h/Dict.cpp: a missing key, or one of another
	** type, gives the default value and *exists false */
	void clear() { m_pairs.clear(); }
	Bool getBool(NameKeyType key, Bool *exists = NULL) const
	{
		std::map<NameKeyType, Value>::const_iterator it = m_pairs.find(key);
		if (it != m_pairs.end() && it->second.type == DICT_BOOL) { if (exists) *exists = true; return it->second.b; }
		if (exists) *exists = false;
		return false;
	}
	AsciiString getAsciiString(NameKeyType key, Bool *exists = NULL) const
	{
		std::map<NameKeyType, Value>::const_iterator it = m_pairs.find(key);
		if (it != m_pairs.end() && it->second.type == DICT_ASCIISTRING) { if (exists) *exists = true; return it->second.a; }
		if (exists) *exists = false;
		return AsciiString::TheEmptyString;
	}
	/* --- end p1w4-sides --- */
private:
	const std::pair<const NameKeyType, Value> &nth(Int n) const
	{
		std::map<NameKeyType, Value>::const_iterator it = m_pairs.begin();
		while (n-- > 0) ++it;
		return *it;
	}
	std::map<NameKeyType, Value> m_pairs;
};
#endif
