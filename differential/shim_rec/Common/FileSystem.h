/* Stand-in for Common/FileSystem.h, differential tests only (differential/wrap_recorder.cpp): createDirectory (logged). */
#ifndef SHIM_REC_FILESYSTEM_H
#define SHIM_REC_FILESYSTEM_H
#include "Common/AsciiString.h"
class FileSystem { public: Bool createDirectory(AsciiString path); };
extern FileSystem *TheFileSystem;
#endif
