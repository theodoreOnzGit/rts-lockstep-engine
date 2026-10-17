/* Differential-test wrapper (upstream commit 0a05454d): the RAMFile that
** W3D data is read through in the game (from a .big archive, behind a
** GameFileClass), exposed as C functions for differential/wrap_w3d.cpp,
** whose WWLib headers cannot share a translation unit with the GameEngine
** ones. Compiled in the .big archive build (RAMFile.cpp itself is compiled
** there unchanged). The RAMFile's buffer is set directly (TestRAMFile), as
** openFromArchive leaves it after copying the file out of the archive. */
#include <string.h>
#include "Common/RAMFile.h"

namespace {

class TestRAMFile : public RAMFile
{
public:
	void setData(const char *data, int len)
	{
		m_data = new char[len > 0 ? len : 1];
		if (len > 0) memcpy(m_data, data, len);
		m_size = len;
		m_pos = 0;
	}
};

} /* namespace */

extern "C" {

void *ref_w3d_ramfile_open(const char *data, int len)
{
	TestRAMFile *f = newInstance(TestRAMFile);
	f->setData(data, len);
	return f;
}

int ref_w3d_ramfile_read(void *f, void *buffer, int bytes)
{
	return ((RAMFile *)f)->read(buffer, bytes);
}

/* mode: 0 START, 1 CURRENT, 2 END (File::seekMode) */
int ref_w3d_ramfile_seek(void *f, int pos, int mode)
{
	File::seekMode m = mode == 0 ? File::START : mode == 2 ? File::END : File::CURRENT;
	return ((RAMFile *)f)->seek(pos, m);
}

int ref_w3d_ramfile_size(void *f)
{
	return ((RAMFile *)f)->size();
}

void ref_w3d_ramfile_close(void *f)
{
	((RAMFile *)f)->close();
	((RAMFile *)f)->deleteInstance();
}

} /* extern "C" */
