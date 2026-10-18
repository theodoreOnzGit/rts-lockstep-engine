/* Forced include (-include) for SidesList.cpp (the OUT_DIR copy with the one
** edit build.rs makes) and Scripts.cpp, differential tests only. The chunk
** build's prelude (differential/shim_chunk/chunk_prelude.h), DataChunk.h, and
** the declaration of the stand-in that replaces the skirmish scripts file
** open (see build.rs: SidesList::prepareForMP_or_Skirmish opens
** "data\Scripts\SkirmishScripts.scb" through the file system and
** CompressionManager, which the harness does not have). */
#ifndef SHIM_SIDES_PRELUDE_H
#define SHIM_SIDES_PRELUDE_H
#include "../shim_chunk/chunk_prelude.h"
#include "Common/DataChunk.h"
Bool sides_open_skirmish_stream(CachedFileInputStream &stream, const AsciiString &path);
#endif
