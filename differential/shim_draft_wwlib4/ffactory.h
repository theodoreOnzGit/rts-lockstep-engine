/* (#215, group wwlib4) stand-in for WWLib/ffactory.h: FileFactoryClass as declared upstream, and SimpleFileFactoryClass reduced to what
** mixfile.cpp's MixFileCreator needs (its _SimpleFileFactory.Get_File / Return_File). The real one searches a sub-directory list and
** returns a BufferedFileClass; here Get_File is a RawFileClass of the given name and Return_File deletes it (as RawFileFactoryClass does). */
#ifndef FFACTORY_H
#define FFACTORY_H
#include "always.h"
#include "vector.h"
#include "wwstring.h"
#include "rawfile.h"
class FileClass;
class FileFactoryClass {
public:
	virtual ~FileFactoryClass(void) {}
	virtual FileClass *Get_File(char const *filename) = 0;
	virtual void Return_File(FileClass *file) = 0;
};
class SimpleFileFactoryClass : public FileFactoryClass {
public:
	virtual FileClass *Get_File(char const *filename) { return new RawFileClass(filename); }
	virtual void Return_File(FileClass *file) { delete file; }
};
#endif
