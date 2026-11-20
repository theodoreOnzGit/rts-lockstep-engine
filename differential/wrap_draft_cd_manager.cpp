/* Differential-test wrapper for the CD manager draft (#215, group lc2, upstream commit 0a05454d), src/drafts/logic_common/cd_manager.rs. Cut verbatim by build.rs:
**   cd_list_h.inc  Include/Common/List.h                from `class LListNode` to the include guard's end (the linked list the drives live in);
**   cd_list_cpp.inc Source/Common/System/List.cpp       from `LList::LList( )` to the end;
**   cd_h.inc       Include/Common/CDManager.h           from `namespace CD` to the inlining banner (CDDrive, CDManager);
**   cd_cpp.inc     Source/Common/System/CDManager.cpp   from `CDDrive::CDDrive()` to the end;
**   cd_w32_h.inc   GameEngineDevice/.../Win32CDManager.h from `class Win32CDDrive` to its inlining banner;
**   cd_w32_cpp.inc GameEngineDevice/.../Win32CDManager.cpp from `Win32CDDrive::Win32CDDrive()` to the end (not `CreateCDManager`).
**
** Stand-ins (outside the cuts): AsciiString is a std::string with clear(), str(), assignment from char*, format() (the one use is "%c:\\") and
** length; `SubsystemInterface` declares init/update/reset; GetDriveType, GetVolumeInformation, `TheFileSystem->unloadMusicFilesFromCD()` and
** `TheGameLogic->getFrame()` are the test's fakes (a CD-ROM mask over the letters, a volume name or failure per drive, a counter, a frame number);
** `NEW` is `new`. THE ONE THING TO KNOW: `LList::LList` leaves `m_addToEndOfGroup` UNINITIALISED, and `LList::add` reads it. The shipped game makes
** every manager with `NEW Win32CDManager`, i.e. the global `operator new` -> `DynamicMemoryAllocator::allocateBytes`, which zero-fills (GameMemory.cpp
** `allocateBytesImplementation`'s memset 0); so the flag is FALSE there. This wrapper builds each manager in zeroed storage for the same reason.
**
** Entry point: ref_lc2_cd_run (tests/differential_draft_cd_manager.rs). */
#include <string>
#include <string.h>
#include <stdarg.h>
#include <stdio.h>
#include <new>

typedef int Int;
typedef bool Bool;
typedef char Char;
#define TRUE true
#define FALSE false
#define NULL 0
#define NEW new

namespace {

class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	AsciiString &operator=(const char *c) { s = c; return *this; }
	void clear() { s.clear(); }
	const char *str() const { return s.c_str(); }
	void format(const char *f, ...) { char b[256]; va_list a; va_start(a, f); vsnprintf(b, sizeof b, f, a); va_end(a); s = b; }
};

class SubsystemInterface
{
public:
	virtual ~SubsystemInterface() {}
	virtual void init(void) = 0;
	virtual void update(void) = 0;
	virtual void reset(void) = 0;
};

// the test's fakes
unsigned g_cdromMask = 0;
int g_vol[26];
int g_unloads = 0;
unsigned g_frame = 0;
const int DRIVE_CDROM = 5;
int GetDriveType(const char *path) { int l = path[0] - 'a'; return (g_cdromMask >> l) & 1 ? DRIVE_CDROM : 1; }
int GetVolumeInformation(const char *path, char *vol, unsigned long, void *, void *, void *, void *, unsigned long)
{
	int l = path[0] - 'a';
	if (l < 0 || l >= 26 || g_vol[l] == 0) return 0;
	snprintf(vol, 1000, "VOL%d", g_vol[l]);
	return 1;
}
struct FileSystemStandIn { void unloadMusicFilesFromCD() { ++g_unloads; } } fsStandIn;
FileSystemStandIn *TheFileSystem = &fsStandIn;
struct GameLogicStandIn { unsigned getFrame() { return g_frame; } } glStandIn;
GameLogicStandIn *TheGameLogic = &glStandIn;

#include "cd_list_h.inc"
#include "cd_list_cpp.inc"
#include "cd_h.inc"
#include "cd_cpp.inc"
#include "cd_w32_h.inc"
#include "cd_w32_cpp.inc"

}

extern "C" {

/* The base CDDrive alone (CDDrive::CDDrive, getPath, getDiskName, getDisk, setPath, and the base refreshInfo that the Win32 drive overrides):
** out: disk before, path length, disk-name length, then after setPath("q:\\") and refreshInfo(): disk, path's first letter, path length. */
int ref_lc2_cd_base(int *out)
{
	CDDrive d;
	out[0] = (int)d.getDisk();
	out[1] = (int)d.getPath().s.size();
	out[2] = (int)d.getDiskName().s.size();
	d.setPath("q:\\");
	d.refreshInfo();
	out[3] = (int)d.getDisk();
	out[4] = d.getPath().str()[0];
	out[5] = (int)d.getPath().s.size();
	return 6;
}

/* prog: [cdromMask, then 26 volume ids (0 = GetVolumeInformation fails, k = "VOLk")], ops of 3 words [kind, a, b]:
**   0 init()   1 update() with the frame number a   2 refreshDrives()   3 volume of letter a becomes b   4 getDrive(a)   5 newDrive("<letter a>:\\")
**   6 destroyAllDrives()   7 reset()
** After each op: driveCount, the count of unloadMusicFilesFromCD calls, for the first 6 drives (path's letter, disk name's number, disk), and
** for op 4 the answer (found, letter, name number, disk). Returns the words written. */
int ref_lc2_cd_run(const int *prog, int nprog, int *out)
{
	static char storage[sizeof(Win32CDManager)];
	memset(storage, 0, sizeof storage);
	Win32CDManager *m = new (storage) Win32CDManager; /* zeroed storage: the zero-filled allocateBytes of the shipped game, see the header */
	int p = 0;
	g_cdromMask = (unsigned)prog[p++];
	for (int i = 0; i < 26; ++i) g_vol[i] = prog[p++];
	g_unloads = 0;
	g_frame = 0;
	int w = 0;
	while (p + 3 <= nprog)
	{
		int kind = prog[p], a = prog[p + 1], b = prog[p + 2];
		p += 3;
		int ans[4] = { 0, 0, 0, 0 };
		bool answered = false;
		switch (kind)
		{
		case 0: m->init(); break;
		case 1: g_frame = (unsigned)a; m->update(); break;
		case 2: m->refreshDrives(); break;
		case 3: g_vol[a] = b; break;
		case 4:
		{
			CDDriveInterface *d = m->getDrive(a);
			answered = true;
			if (d) { ans[0] = 1; ans[1] = d->getPath().str()[0]; ans[2] = d->getDiskName().s.empty() ? 0 : d->getDiskName().str()[3] - '0'; ans[3] = (int)d->getDisk(); }
			break;
		}
		case 5: { char path[4] = { (char)('a' + a), ':', '\\', 0 }; m->newDrive(path); break; }
		case 6: m->destroyAllDrives(); break;
		case 7: m->reset(); break;
		}
		out[w++] = m->driveCount();
		out[w++] = g_unloads;
		for (int i = 0; i < 6; ++i)
		{
			CDDriveInterface *d = m->getDrive(i);
			if (d) { out[w++] = d->getPath().str()[0]; out[w++] = d->getDiskName().s.empty() ? 0 : d->getDiskName().str()[3] - '0'; out[w++] = (int)d->getDisk(); }
			else { out[w++] = 0; out[w++] = 0; out[w++] = 0; }
		}
		if (answered) for (int i = 0; i < 4; ++i) out[w++] = ans[i];
	}
	m->~Win32CDManager();
	return w;
}

}
