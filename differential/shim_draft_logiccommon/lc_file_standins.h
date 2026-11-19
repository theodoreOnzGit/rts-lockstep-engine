/* Stand-ins shared by the logic_common drafts' file wrappers (#215, build.rs drafts_logiccommon). Included INSIDE the wrapper's anonymous
** namespace, after the system headers:
**  - DEBUG_ASSERTCRASH / DEBUG_CRASH, empty (compiled out of the release build); NEW is new; MEMORY_POOL_GLUE_ABC is empty;
**  - MemoryPoolObject with deleteInstance() doing nothing (File::close calls it only for a file marked deleteOnClose, which no test marks);
**  - AsciiString on std::string, with concat(char) as AsciiString.h line 536 has it (a 2-char buffer through concat(const char*), so that a
**    NUL character appends nothing), str(), clear(), set();
**  - the class declaration of File (Include/Common/file.h), member for member, the members public. */
#define DEBUG_ASSERTCRASH(c, m)
#define DEBUG_CRASH(m)
#define NEW new
#define MEMORY_POOL_GLUE_ABC(x)

struct MemoryPoolObject
{
	virtual ~MemoryPoolObject() {}
	void deleteInstance() {}
};

class AsciiString
{
	std::string s;
public:
	void concat(const char *p) { s += p; }
	void concat(const char c) { char tmp[2] = { c, 0 }; concat(tmp); } /* AsciiString.h line 536 */
	const char *str() const { return s.c_str(); }
	void clear() { s.clear(); }
	void set(const char *p) { s = p; }
};

/* ---- file.h's class File, member for member (members public) ---- */
class File : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_ABC(File)
public:
	enum access { NONE = 0x0, READ = 0x1, WRITE = 0x2, APPEND = 0x4, CREATE = 0x8, TRUNCATE = 0x10, TEXT = 0x20, BINARY = 0x40, READWRITE = (READ | WRITE), ONLYNEW = 0x80, STREAMING = 0x100 };
	enum seekMode { START, CURRENT, END };
	AsciiString m_nameStr;
	Int m_access;
	Bool m_open;
	Bool m_deleteOnClose;
	File();
	virtual ~File();
	Bool eof();
	virtual Bool open(const Char *filename, Int access = 0);
	virtual void close(void);
	virtual Int read(void *buffer, Int bytes) = 0;
	virtual Int write(const void *buffer, Int bytes) = 0;
	virtual Int seek(Int bytes, seekMode mode = CURRENT) = 0;
	virtual void nextLine(Char *buf = NULL, Int bufSize = 0) = 0;
	virtual Bool scanInt(Int &newInt) = 0;
	virtual Bool scanReal(Real &newReal) = 0;
	virtual Bool scanString(AsciiString &newString) = 0;
	virtual Int size(void);
	virtual Int position(void);
	void setName(const char *name) { m_nameStr.set(name); }
	virtual char *readEntireAndClose() = 0;
	virtual File *convertToRAMFile() = 0;
};

