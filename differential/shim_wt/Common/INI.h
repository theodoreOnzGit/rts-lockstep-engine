/* Shim for GameEngine/Include/Common/INI.h, differential/wrap_wt.cpp only (#77). The declarations
** GameWindowTransitions.cpp uses, copied from the real header (LookupListRec, FieldParse, INIFieldParseProc,
** INILoadType, and the INI members it calls); their bodies are wrap_wt.cpp's stand-ins, transcribed from INI.cpp for
** the subset a WindowTransitions.ini uses (the real INI.cpp needs every block handler of the game: theTypeTable). */
#ifndef SHIM_WT_INI_H
#define SHIM_WT_INI_H
#include <stddef.h>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
class INI;
enum INILoadType
{
	INI_LOAD_INVALID,
	INI_LOAD_OVERWRITE,
	INI_LOAD_CREATE_OVERRIDES,
	INI_LOAD_MULTIFILE
};
class Xfer;
typedef void (*INIFieldParseProc)( INI *ini, void *instance, void *store, const void* userData );
struct LookupListRec
{
	const char* name;
	Int value;
};
typedef const LookupListRec *ConstLookupListRecArray;
struct FieldParse
{
	const char*					token;
	INIFieldParseProc		parse;
	const void*					userData;
	Int									offset;
};
class INI
{
public:
	INI();
	~INI();
	void load( AsciiString filename, INILoadType loadType, Xfer *pXfer );
	void initFromINI( void *what, const FieldParse* parseTable );
	const char* getNextToken(const char* seps = NULL);
	static void parseWindowTransitions( INI* ini );
	static void parseBool( INI *ini, void *instance, void *store, const void* userData );
	static void parseInt( INI *ini, void *instance, void *store, const void* userData );
	static void parseAsciiString( INI *ini, void *instance, void *store, const void* userData );
	static void parseLookupList( INI *ini, void *instance, void *store, const void* userData );
	/* the stand-in's own state: the file's lines, the current line's tokens */
	const char *m_text;
	size_t m_pos;
	char m_line[1024];
	char *m_tok;
	Bool readLine();
};
#endif
