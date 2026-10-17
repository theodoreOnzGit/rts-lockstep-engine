/* Differential-test wrapper for the CSF string loader (upstream commit
** 0a05454d): GameEngine/Source/GameClient/GameText.cpp compiled unchanged
** as its own translation unit, with differential/shim_csf/csf_prelude.h
** force-included and the stand-ins of differential/shim_csf.
**
** GameTextManager::init opens "data\Generals.str" (preferred) and then
** "data\<language>\Generals.csf" through TheFileSystem->openFile, whose
** stand-in (differential/wrap_big.cpp) forwards to the hook installed here.
** The hook serves the CSF under the name g_csfFile gives for the language
** "english", as a File with RAMFile::read's semantics (RAMFile.cpp lines
** 263-285: clamp to what is left, copy a positive count, move the position
** by the count, also when it is negative), because the game reads the CSF
** from a .big archive; the .str file does not exist. The manager is the
** real one, made by CreateGameTextInterface and used through
** GameTextInterface.
**
** Copied from upstream (labelled): g_strFile and g_csfFile, from
** Main/WinMain.cpp lines 84-85. */
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include "Common/File.h"
#include "Common/FileSystem.h"
#include "Common/Registry.h"
#include "GameClient/GameText.h"
#include "GameClient/LanguageFilter.h"

/* Main/WinMain.cpp lines 84-85 (copied) */
const Char *g_strFile = "data\\Generals.str";
const Char *g_csfFile = "data\\%s\\Generals.csf";

/* ---- stand-ins (never reached: the window handle is NULL and the map
** string-file parser is not called; they abort if reached) */
HWND ApplicationHWnd = 0;
int SetWindowText(HWND, const char *) { abort(); }
int SetWindowTextW(HWND, const WideChar *) { abort(); }
LanguageFilter *TheLanguageFilter = 0;
void LanguageFilter::filterLine(UnicodeString &) { abort(); }
/* Common/Registry.h: the language the install was set up with */
AsciiString GetRegistryLanguage(void) { return AsciiString("english"); }

namespace {

std::vector<char> g_csf;
std::string g_csfName;
GameTextInterface *g_text = 0;

class RamLikeFile : public File
{
public:
	RamLikeFile(const std::vector<char> &data) : m_bytes(data), m_pos(0) {}
	virtual ~RamLikeFile() {}
	/* RAMFile::read */
	virtual Int read(void *buffer, Int bytes)
	{
		Int bytesLeft = (Int)m_bytes.size() - m_pos;
		if (bytes > bytesLeft) bytes = bytesLeft;
		if (bytes > 0 && buffer != NULL) {
			if (m_pos < 0) abort(); /* undefined upstream: the Rust excludes these files */
			memcpy(buffer, &m_bytes[m_pos], bytes);
		}
		m_pos += bytes;
		return bytes;
	}
	virtual Int write(const void *, Int) { return -1; }
	virtual Int seek(Int, seekMode) { abort(); }
	virtual void nextLine(Char *, Int) { abort(); }
	virtual Bool scanInt(Int &) { abort(); }
	virtual Bool scanReal(Real &) { abort(); }
	virtual Bool scanString(AsciiString &) { abort(); }
	virtual char *readEntireAndClose() { abort(); }
	virtual File *convertToRAMFile() { return this; }
private:
	std::vector<char> m_bytes;
	Int m_pos;
};

File *openHook(const Char *name, Int access)
{
	if (g_csfName != name) return NULL;
	RamLikeFile *f = new RamLikeFile(g_csf);
	f->open(name, access);
	f->deleteOnClose();
	return f;
}

/* storage for TheFileSystem: only its non-virtual openFile is called, which
** does not touch the object */
alignas(16) unsigned char g_fsStorage[sizeof(FileSystem)];

} /* namespace */

extern File *(*shim_file_system_open_hook)(const Char *, Int);

extern "C" {

/* a fresh GameTextManager, init() with the CSF data as the language file;
** returns 0, or 1 if init threw (new StringInfo[n] for a huge declared label
** count throws std::bad_alloc), leaving a manager that never initialised */
int ref_csf_load(const char *data, int len)
{
	shim_file_system_open_hook = openHook;
	TheFileSystem = (FileSystem *)g_fsStorage;
	g_csf.assign(data, data + len);
	AsciiString name;
	name.format(g_csfFile, GetRegistryLanguage().str());
	g_csfName = name.str();
	delete g_text;
	g_text = CreateGameTextInterface();
	try { g_text->init(); } catch (...) { return 1; }
	return 0;
}

/* fetch(label, &exists): the text's units into out (up to cap), its length
** returned, exists in *exists */
int ref_csf_fetch(const char *label, unsigned short *out, int cap, int *exists)
{
	Bool e = FALSE;
	UnicodeString s = g_text->fetch(label, &e);
	*exists = e ? 1 : 0;
	int n = s.getLength();
	for (int i = 0; i < n && i < cap; ++i) out[i] = s.str()[i];
	return n;
}

/* getStringsWithLabelPrefix(prefix): the labels, each NUL-terminated, in
** order; returns the total bytes */
int ref_csf_prefix(const char *prefix, char *out, int cap)
{
	AsciiStringVec &v = g_text->getStringsWithLabelPrefix(AsciiString(prefix));
	int n = 0;
	for (size_t i = 0; i < v.size(); ++i) {
		int len = v[i].getLength();
		for (int k = 0; k <= len; ++k) { if (n < cap) out[n] = v[i].str()[k]; ++n; }
	}
	return n;
}

} /* extern "C" */
