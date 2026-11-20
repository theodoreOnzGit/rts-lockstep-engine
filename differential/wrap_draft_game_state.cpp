/* Differential-test wrapper for the game state draft (#215, group lc3, upstream commit 0a05454d), src/drafts/sweep_common_client/game_state.rs. Cut verbatim by build.rs:
**   gs_dir.inc     GameState.cpp  getSaveDirectory and getFilePathInSaveDirectory (to isInSaveDirectory);
**   gs_exists.inc  GameState.cpp  doesSaveGameExist (to getSaveGameInfoFromFile's banner);
**   xfer_load.inc  XferLoad.cpp   open and close (the file's constructor and destructor are above and below them: the cut runs from `XferLoad::open` to beginBlock's banner).
**
** Stand-ins (outside the cuts): AsciiString (std::string with concat, str, assignment); TheGlobalData->getPath_UserData() (the case's string); class GameState (only the
** two functions); class Xfer (m_identifier, open, and the XferStatus numbers) and class XferLoad (m_fileFP, the constructor's `m_fileFP = NULL`, the destructor's close);
** `fopen` / `fclose` are the test's fakes (a file opens iff its path is in the case's set); DEBUG_CRASH is nothing.
** Entry point: ref_lc3_gs_exists(blob) with blob = user path NUL leaf NUL (count) existing paths each NUL-ended ... returns 1 / 0; blob layout in the test. */
#include <string>
#include <set>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

typedef bool Bool;
typedef int Int;
#define TRUE true
#define FALSE false
#define NULL 0
#define DEBUG_CRASH(a)

namespace {

class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	const char *str() const { return s.c_str(); }
	void concat(const char *c) { s += c; }
	void concat(const AsciiString &a) { s += a.s; }
	void clear() { s.clear(); }
};

std::set<std::string> g_files;
std::string g_user;
FILE *ref_fopen(const char *path, const char *) { return g_files.count(path) ? (FILE *)&g_files : NULL; }
int ref_fclose(FILE *) { return 0; }
#define fopen ref_fopen
#define fclose ref_fclose

struct GlobalDataStandIn { AsciiString getPath_UserData() const { return AsciiString(g_user.c_str()); } } g_gd;
GlobalDataStandIn *TheGlobalData = &g_gd;

enum XferStatus { XFER_STATUS_INVALID = 0, XFER_OK, XFER_EOF, XFER_FILE_NOT_FOUND, XFER_FILE_NOT_OPEN, XFER_FILE_ALREADY_OPEN };
enum XferMode { XFER_LOAD = 1 };

class Xfer
{
public:
	AsciiString m_identifier;
	XferMode m_xferMode;
	virtual void open(AsciiString identifier) { m_identifier = identifier; }
	virtual void close(void) {}
	virtual ~Xfer() {}
};

class XferLoad : public Xfer
{
public:
	FILE *m_fileFP;
	XferLoad() { m_xferMode = XFER_LOAD; m_fileFP = NULL; }
	~XferLoad() { if (m_fileFP != NULL) close(); }
	void open(AsciiString identifier);
	void close(void);
};

class GameState
{
public:
	AsciiString getSaveDirectory() const;
	AsciiString getFilePathInSaveDirectory(const AsciiString &leaf) const;
	AsciiString getFilePathInSaveDirectory2(AsciiString filename) { return getFilePathInSaveDirectory(filename); }
	Bool doesSaveGameExist(AsciiString filename);
};

#include "xfer_load.inc"
#include "gs_dir.inc"
#include "gs_exists.inc"

}

extern "C" int ref_lc3_gs_exists(const char *blob, int n)
{
	g_files.clear();
	const char *p = blob;
	g_user = p; p += strlen(p) + 1;
	std::string leaf = p; p += strlen(p) + 1;
	int count = atoi(p); p += strlen(p) + 1;
	for (int i = 0; i < count; ++i) { g_files.insert(p); p += strlen(p) + 1; }
	GameState gs;
	return gs.doesSaveGameExist(AsciiString(leaf.c_str())) ? 1 : 0;
}
