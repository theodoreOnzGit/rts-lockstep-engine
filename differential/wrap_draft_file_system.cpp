/* Differential-test wrapper for the draft src/drafts/sweep_common_client/file_system.rs (#215, group ui2, upstream
** commit 0a05454d), cut verbatim by build.rs (fn drafts_ui2) into OUT_DIR/draft_ui2_gen:
**   u2_fs_a.inc   GameEngine/Source/Common/System/FileSystem.cpp, FileSystem::loadMusicFilesFromCD (to the banner of
**                 unloadMusicFilesFromCD);
**   u2_fs_b.inc   GameEngine/Source/Common/System/RAMFile.cpp, RAMFile::copyDataToFile (to convertToRAMFile).
** No edit. Stand-ins: AsciiString (std::string holder, `=`), the CD manager (TheCDManager: a pointer, NULL or a
** manager with driveCount and getDrive; a drive is NULL or a CDDriveInterface whose getPath is "p<id>"),
** TheArchiveFileSystem (loadBigFilesFromDirectory(AsciiString, AsciiString) logs the directory and archive name, and
** answers from the script), MUSIC_BIG as ArchiveFileSystem.h defines it, File (write(data, size) logs the bytes and
** answers from the script) and RAMFile (m_data, m_size). Everything is in a namespace.
**
** Entry points (extern "C", tests/differential_draft_file_system.rs):
**   ref_u2_fs_music(in, n, out, cap): `in` is [has manager, ndrives, (null drive, path id, answer) x ndrives]; the
**   output is the number of loadBigFilesFromDirectory calls, then per call: the path id, the archive name's length and
**   its bytes.
**   ref_u2_fs_copy(in, n, out, cap): `in` is [has file, size, write's answer, the bytes x size]; the output is the
**   result (0/1), the number of bytes written, the bytes (a missing file writes nothing). */
#include <string>
#include <vector>
#include <stdlib.h>
#include <string.h>
#include "Lib/BaseType.h"

namespace u2fs {
#define MUSIC_BIG "Music.big"
struct AsciiString { std::string s; AsciiString() {} AsciiString(const char *c) : s(c) {} };

struct CDDriveInterface { AsciiString path; AsciiString getPath() { return path; } };
struct CDManager
{
	std::vector<CDDriveInterface *> drives;
	Int driveCount() { return (Int)drives.size(); }
	CDDriveInterface *getDrive(Int i) { return drives[i]; }
};
static CDManager *TheCDManager = NULL;

struct ArchiveFileSystem
{
	std::vector<int> calls;     /* path id, name length, name bytes ... */
	std::vector<int> answers; size_t next;
	Bool loadBigFilesFromDirectory(AsciiString dir, AsciiString fileMask)
	{
		calls.push_back(atoi(dir.s.c_str() + 1));
		calls.push_back((int)fileMask.s.size());
		for (size_t i = 0; i < fileMask.s.size(); i++) calls.push_back((unsigned char)fileMask.s[i]);
		return answers[next++] != 0;
	}
};
static ArchiveFileSystem *TheArchiveFileSystem = NULL;

class FileSystem { public: void loadMusicFilesFromCD(); };
#include "u2_fs_a.inc"

struct File
{
	std::vector<unsigned char> written; Int answer;
	Int write(const void *data, Int size) { written.assign((const unsigned char *)data, (const unsigned char *)data + size); return answer; }
};
class RAMFile { public: Bool copyDataToFile(File *localFile); Char *m_data; Int m_size; };
#include "u2_fs_b.inc"
}

extern "C" int ref_u2_fs_music(const int *in, int n, int *out, int cap)
{
	using namespace u2fs;
	int i = 0;
	int has = in[i++], nd = in[i++];
	CDManager mgr; ArchiveFileSystem afs; afs.next = 0;
	for (int d = 0; d < nd; d++) {
		int isnull = in[i++], pid = in[i++], ans = in[i++];
		if (isnull) mgr.drives.push_back(NULL);
		else { CDDriveInterface *c = new CDDriveInterface; c->path = AsciiString(("p" + std::to_string(pid)).c_str()); mgr.drives.push_back(c); }
		afs.answers.push_back(ans);
	}
	/* the answers are consumed per call, in call order; a NULL drive makes no call, so drop its answer */
	std::vector<int> a2;
	for (int d = 0; d < nd; d++) if (mgr.drives[d]) a2.push_back(afs.answers[d]);
	afs.answers = a2; afs.answers.push_back(0);
	TheCDManager = has ? &mgr : NULL;
	TheArchiveFileSystem = &afs;
	FileSystem fs; fs.loadMusicFilesFromCD();
	int o = 0, ncalls = 0;
	for (size_t k = 0; k < afs.calls.size(); ) { ncalls++; k += 2 + (size_t)afs.calls[k + 1]; }
	if (o < cap) out[o] = ncalls;
	o++;
	for (size_t k = 0; k < afs.calls.size(); k++) { if (o < cap) out[o] = afs.calls[k]; o++; }
	(void)n;
	return o;
}

extern "C" int ref_u2_fs_copy(const int *in, int n, int *out, int cap)
{
	using namespace u2fs;
	int has = in[0], size = in[1], ans = in[2];
	std::vector<char> data(size > 0 ? size : 1);
	for (int k = 0; k < size; k++) data[k] = (char)in[3 + k];
	RAMFile r; r.m_data = data.data(); r.m_size = size;
	File f; f.answer = ans;
	Bool res = r.copyDataToFile(has ? &f : NULL);
	int o = 0;
	if (o < cap) out[o] = res ? 1 : 0; o++;
	if (o < cap) out[o] = (int)f.written.size(); o++;
	for (size_t k = 0; k < f.written.size(); k++) { if (o < cap) out[o] = f.written[k]; o++; }
	(void)n;
	return o;
}
