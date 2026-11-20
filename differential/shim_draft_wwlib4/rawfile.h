/* (#215, group wwlib4) stand-in for WWLib/RAWFILE.H: RawFileClass over an in-memory file system (a map from name to byte vector),
** for the tagblock and mixfile references. RawFileClass is the Win32 file layer (CreateFile, ReadFile, WriteFile, SetFilePointer);
** what is modelled is exactly what those calls do for the callers here:
**   Open(READ): OPEN_EXISTING (fails if absent); Open(WRITE): CREATE_ALWAYS (truncates); Open(READ|WRITE): OPEN_ALWAYS (keeps bytes);
**   a write past the end zero-fills the gap; a read past the end returns the bytes there are; SetFilePointer(FILE_BEGIN/CURRENT/END).
** Seek, Size, Bias and the bias clipping of Read are TRANSCRIBED from rawfile.cpp (the Seek/Size/Bias/Read bodies minus the OS calls).
** Get_Date_Time is 0 (the file's time stamp is the operating system's). */
#ifndef RAWFILE_Hx
#define RAWFILE_Hx
#include <string>
#include <vector>
#include <map>
#include <string.h>
#include "wwfile.h"

typedef std::map<std::string, std::vector<unsigned char> > MemFs;
inline MemFs &mem_fs(void) { static MemFs m; return m; }

class RawFileClass : public FileClass
{
public:
	int Rights;
	int BiasStart;
	int BiasLength;
	RawFileClass(void) : Rights(0), BiasStart(0), BiasLength(-1), opened(false), data(0), pos(0) {}
	RawFileClass(char const *filename) : Rights(0), BiasStart(0), BiasLength(-1), opened(false), data(0), pos(0) { Filename = filename; }
	virtual ~RawFileClass(void) { Close(); }
	virtual char const *File_Name(void) const { return Filename.c_str(); }
	virtual char const *Set_Name(char const *filename) { Close(); Filename = filename; return Filename.c_str(); }
	virtual int Create(void) { Close(); mem_fs()[Filename] = std::vector<unsigned char>(); return 1; }
	virtual int Delete(void) { Close(); return mem_fs().erase(Filename) ? 1 : 0; }
	virtual bool Is_Available(int = false) { return opened || mem_fs().count(Filename) != 0; }
	virtual bool Is_Open(void) const { return opened; }
	virtual int Open(char const *filename, int rights = READ) { Set_Name(filename); return Open(rights); }
	virtual int Open(int rights = READ)
	{
		Close();
		Rights = rights;
		MemFs &fs = mem_fs();
		if (rights == READ) {
			MemFs::iterator it = fs.find(Filename);
			if (it == fs.end()) return false;
			data = &it->second;
		} else if (rights == WRITE) {
			fs[Filename] = std::vector<unsigned char>();
			data = &fs[Filename];
		} else if (rights == (READ | WRITE)) {
			data = &fs[Filename];
		} else {
			return false;
		}
		opened = true;
		pos = 0;
		if (BiasStart != 0 || BiasLength != -1) Seek(0, SEEK_SET);
		return true;
	}
	virtual int Read(void *buffer, int size)
	{
		bool was_opened = false;
		if (!Is_Open()) {
			if (!Open(READ)) return 0;
			was_opened = true;
		}
		if (BiasLength != -1) {
			int remainder = BiasLength - Seek(0);
			size = size < remainder ? size : remainder;
		}
		int total = 0;
		while (size > 0) {
			int avail = (int)data->size() - pos;
			if (avail < 0) avail = 0;
			int n = size < avail ? size : avail;
			if (n > 0) memcpy(buffer, &(*data)[pos], n);
			pos += n; size -= n; total += n;
			buffer = (char *)buffer + n;
			if (n == 0) break;
		}
		if (was_opened) Close();
		return total;
	}
	virtual int Seek(int p, int dir = SEEK_CUR)
	{
		if (BiasLength != -1) {
			switch (dir) {
				case SEEK_SET:
					if (p > BiasLength) p = BiasLength;
					p += BiasStart;
					break;
				case SEEK_CUR:
					break;
				case SEEK_END:
					dir = SEEK_SET;
					p += BiasStart + BiasLength;
					break;
			}
			long newpos = Raw_Seek(p, dir) - BiasStart;
			if (newpos < 0) newpos = Raw_Seek(BiasStart, SEEK_SET) - BiasStart;
			if (newpos > BiasLength) newpos = Raw_Seek(BiasStart + BiasLength, SEEK_SET) - BiasStart;
			return (int)newpos;
		}
		return Raw_Seek(p, dir);
	}
	virtual int Size(void)
	{
		int size = 0;
		if (BiasLength != -1) return BiasLength;
		if (Is_Open()) {
			size = (int)data->size();
		} else {
			if (Open()) { size = Size(); Close(); }
		}
		BiasLength = size - BiasStart;
		return BiasLength;
	}
	virtual int Write(void const *buffer, int size)
	{
		bool was_opened = false;
		if (!Is_Open()) {
			if (!Open(WRITE)) return 0;
			was_opened = true;
		}
		if (size > 0) {
			if (pos + size > (int)data->size()) data->resize(pos + size, 0);
			memcpy(&(*data)[pos], buffer, size);
			pos += size;
		}
		if (was_opened) Close();
		return size > 0 ? size : 0;
	}
	virtual void Close(void) { opened = false; data = 0; }
	virtual unsigned long Get_Date_Time(void) { return 0; }
	virtual void Bias(int start, int length = -1)
	{
		if (start == 0) {
			BiasStart = 0;
			BiasLength = -1;
			return;
		}
		BiasLength = RawFileClass::Size();
		BiasStart += start;
		if (length != -1) {
			BiasLength = BiasLength < length ? BiasLength : length;
		}
		BiasLength = BiasLength > 0 ? BiasLength : 0;
		if (Is_Open()) {
			RawFileClass::Seek(0, SEEK_SET);
		}
	}
protected:
	int Raw_Seek(int p, int dir = SEEK_CUR)
	{
		long np = p;
		if (dir == SEEK_CUR) np = pos + p;
		else if (dir == SEEK_END) np = (long)data->size() + p;
		if (np < 0) np = 0;
		pos = (int)np;
		return pos;
	}
private:
	std::string Filename;
	bool opened;
	std::vector<unsigned char> *data;
	int pos;
};
#endif
