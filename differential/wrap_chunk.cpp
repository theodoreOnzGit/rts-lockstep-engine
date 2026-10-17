/* Differential-test wrapper for the chunk file reader (upstream commit
** 0a05454d): GameEngine/Source/Common/System/DataChunk.cpp and
** GameEngine/Source/Common/NameKeyGenerator.cpp compiled unchanged as their
** own translation units, with differential/shim_chunk/chunk_prelude.h
** force-included and the stand-in Dict of differential/shim_chunk (see
** that file: the real Dict.cpp cannot hold the std::string-based string
** shims).
**
** The stream is the real CachedFileInputStream with its buffer set
** directly (TestStream), as open() leaves it for uncompressed data; open()
** itself (the file system and CompressionManager) is not run. The reader is
** the real DataChunkInput (TestInput only exposes its table of contents).
** The parsers registered are a script interpreter (interp) that reads its
** chunk's own bytes as operations, a parser that fails, and one that reads
** nothing; each appends what it reads to a log the Rust test reproduces
** byte for byte. NameKeyTypes are logged as their names (keyToName). */
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include <algorithm>
#include "Common/DataChunk.h"
#include "Compression.h"

const UnicodeString UnicodeString::TheEmptyString;

/* ---- stand-ins, never reached (they abort): CachedFileInputStream::open's
** decompression and DataChunkOutput's temporary file */
Bool CompressionManager::isDataCompressed(const void *, Int) { abort(); }
Int CompressionManager::getUncompressedSize(const void *, Int) { abort(); }
Int CompressionManager::decompressData(void *, Int, void *, Int) { abort(); }
AsciiString GlobalData::getPath_UserData() const { abort(); }

namespace {

std::vector<unsigned char> g_log;

void put(const void *p, int n) { g_log.insert(g_log.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void put8(int v) { g_log.push_back((unsigned char)v); }
void putU16(unsigned short v) { put(&v, 2); }
void putU32(unsigned v) { put(&v, 4); }
void putCStr(const char *s) { int n = (int)strlen(s); putU32((unsigned)n); put(s, n); }
void putWide(const WideChar *s)
{
	int n = 0;
	while (s[n]) ++n;
	putU32((unsigned)n);
	put(s, n * 2);
}

class TestStream : public CachedFileInputStream
{
public:
	void setData(const char *data, int len)
	{
		m_buffer = new char[len];
		memcpy(m_buffer, data, len);
		m_size = len;
		m_pos = 0;
	}
};

class TestInput : public DataChunkInput
{
public:
	TestInput(ChunkInputStream *s) : DataChunkInput(s) {}
	AsciiString nameOf(UnsignedInt id) { return m_contents.getName(id); }
	UnsignedInt idOf(const char *name) { return m_contents.getID(AsciiString(name)); }
};

Bool interp(DataChunkInput &file, DataChunkInfo *info, void *userData)
{
	put8('I');
	putCStr(info->label.str());
	putCStr(info->parentLabel.str());
	putU16(info->version);
	putU32((unsigned)info->dataSize);
	putU32(file.getChunkDataSize());
	putU32(file.getChunkDataSizeLeft());
	putU16(file.getChunkVersion());
	putCStr(file.getChunkLabel().str());
	for (int k = 0; k < 64; ++k) {
		if (file.atEndOfChunk()) { put8('E'); break; }
		unsigned char op = (unsigned char)file.readByte();
		put8('o'); put8(op);
		switch (op % 12) {
		case 0: { Int i = file.readInt(); put(&i, 4); break; }
		case 1: { Real r = file.readReal(); put(&r, 4); break; }
		case 2: { Byte b = file.readByte(); put8((unsigned char)b); break; }
		case 3: { AsciiString s = file.readAsciiString(); putCStr(s.str()); break; }
		case 4: { UnicodeString s = file.readUnicodeString(); putWide(s.str()); break; }
		case 5: {
			int n = (unsigned char)file.readByte() & 0x3f;
			char buf[64];
			memset(buf, 0, sizeof(buf));
			file.readArrayOfBytes(buf, n);
			put8(n); put(buf, n);
			break;
		}
		case 6: { Bool r = file.parse(userData); put8('p'); put8(r ? 1 : 0); break; }
		case 7: {
			Dict d = file.readDict();
			std::vector<std::pair<std::string, const Dict::Value *> > v;
			for (std::map<NameKeyType, Dict::Value>::const_iterator it = d.pairs().begin(); it != d.pairs().end(); ++it)
				v.push_back(std::make_pair(std::string(TheNameKeyGenerator->keyToName(it->first).str()), &it->second));
			std::sort(v.begin(), v.end(), [](const std::pair<std::string, const Dict::Value *> &a, const std::pair<std::string, const Dict::Value *> &b) { return a.first < b.first; });
			putU32((unsigned)v.size());
			for (size_t i = 0; i < v.size(); ++i) {
				putCStr(v[i].first.c_str());
				const Dict::Value &x = *v[i].second;
				put8(x.type);
				switch (x.type) {
				case Dict::DICT_BOOL: put8(x.b ? 1 : 0); break;
				case Dict::DICT_INT: put(&x.i, 4); break;
				case Dict::DICT_REAL: put(&x.r, 4); break;
				case Dict::DICT_ASCIISTRING: putCStr(x.a.str()); break;
				case Dict::DICT_UNICODESTRING: putWide(x.u.str()); break;
				default: abort();
				}
			}
			break;
		}
		case 8: { NameKeyType k = file.readNameKey(); putCStr(TheNameKeyGenerator->keyToName(k).str()); break; }
		case 9: { put8('z'); putU32(file.getChunkDataSizeLeft()); put8(file.atEndOfFile() ? 1 : 0); break; }
		case 10: put8('F'); return FALSE;
		case 11: put8('S'); return TRUE;
		}
	}
	return TRUE;
}

Bool failParser(DataChunkInput &, DataChunkInfo *, void *) { put8('X'); return FALSE; }
Bool quietParser(DataChunkInput &file, DataChunkInfo *info, void *) { put8('Q'); putU32((unsigned)info->dataSize); return TRUE; }

} /* namespace */

extern "C" {

/* Build a DataChunkInput over the data, register the test parsers, log the
** table-of-contents lookups, parse, reset and parse again. Returns 0, or 3
** with the ErrorCode in *code if a read threw; the log is copied out. */
int ref_chunk_run(const char *data, int len, unsigned char *log, int cap, int *logLen, unsigned *code)
{
	g_log.clear();
	delete TheNameKeyGenerator;
	TheNameKeyGenerator = new NameKeyGenerator;
	TheNameKeyGenerator->init();
	TestStream stream;
	stream.setData(data, len);
	int status = 0;
	try {
		TestInput in(&stream);
		put8(in.isValidFileType() ? 1 : 0);
		const char *names[] = { "", "A", "B", "C", "D", "Fail", "Unknown", "a" };
		for (int i = 0; i < 8; ++i) putU32(in.idOf(names[i]));
		for (UnsignedInt id = 0; id < 12; ++id) putCStr(in.nameOf(id).str());
		putCStr(in.nameOf(0xFFFFFFFFu).str());
		in.registerParser("A", "", failParser);
		in.registerParser("A", "", interp);
		in.registerParser("B", "A", interp);
		in.registerParser("C", "", interp);
		in.registerParser("C", "B", interp);
		in.registerParser("A", "A", interp);
		in.registerParser("B", "B", interp);
		in.registerParser("Fail", "", failParser);
		in.registerParser("D", "", quietParser);
		in.registerParser("D", "A", quietParser);
		Bool r = in.parse(NULL);
		put8('R'); put8(r ? 1 : 0);
		in.reset();
		r = in.parse(NULL);
		put8('R'); put8(r ? 1 : 0);
	} catch (ErrorCode c) {
		status = 3;
		*code = (unsigned)c;
	}
	int n = (int)g_log.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(log, &g_log[0], n);
	*logLen = (int)g_log.size();
	return status;
}

} /* extern "C" */
