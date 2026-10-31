/* Stand-in for GameNetwork/FileTransfer.h, differential tests only (differential/wrap_game_info.cpp): the map-derived paths (FileTransfer.cpp lines 138-239, cut verbatim by build.rs into OUT_DIR/ginfo_gen/file_transfer_paths.inc). */
#ifndef SHIM_GINFO_FILETRANSFER_H
#define SHIM_GINFO_FILETRANSFER_H
#include "Common/AsciiString.h"
AsciiString GetBasePathFromPath( AsciiString path );
AsciiString GetFileFromPath( AsciiString path );
AsciiString GetExtensionFromFile( AsciiString fname );
AsciiString GetBaseFileFromFile( AsciiString fname );
AsciiString GetPreviewFromMap( AsciiString path );
AsciiString GetINIFromMap( AsciiString path );
AsciiString GetStrFileFromMap( AsciiString path );
AsciiString GetSoloINIFromMap( AsciiString path );
AsciiString GetAssetUsageFromMap( AsciiString path );
AsciiString GetReadmeFromMap( AsciiString path );
#endif
