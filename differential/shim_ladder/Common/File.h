/* Stand-in for Common/File.h, differential tests only (differential/wrap_draft_ladder.cpp, #215): a file whose bytes the wrapper
** holds; `read` hands out up to `n` bytes and `eof` says the end was reached. */
#ifndef SHIM_LADDER_FILE_H
#define SHIM_LADDER_FILE_H
#include <string>
#include "Lib/BaseType.h"
class File
{
public:
	enum { READ = 1, TEXT = 2 };
	std::string m_data; size_t m_pos;
	File() : m_pos(0) {}
	Int read(void *buf, Int n) { size_t k = m_data.size() - m_pos; if ((size_t)n < k) k = (size_t)n; for (size_t i = 0; i < k; ++i) ((char *)buf)[i] = m_data[m_pos + i]; m_pos += k; return (Int)k; }
	Bool eof() { return m_pos >= m_data.size(); }
	void close() {}
};
#endif
