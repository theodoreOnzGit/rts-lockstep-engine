/* Stand-in for GameEngine/Include/Common/GameState.h, ObjectTypes tests only
** (differential/wrap_object_types.cpp). Same class as differential/shim/Common/GameState.h
** (Xfer.cpp, ObjectTypes.cpp and the wrapper read the same members); plus SaveCode, copied
** verbatim from upstream (GameState.h lines 129-138), which ObjectTypes::xfer throws
** (SC_INVALID_DATA) for a load into a non-empty list. */
#ifndef SHIM_OBJECTTYPES_GAMESTATE_H
#define SHIM_OBJECTTYPES_GAMESTATE_H
#include "Common/AsciiString.h"
class Snapshot;
enum SaveCode
{
	SC_INVALID = -1,
	SC_OK,
	SC_NO_FILE_AVAILABLE,
	SC_FILE_NOT_FOUND,
	SC_UNABLE_TO_OPEN_FILE,
	SC_INVALID_XFER,
	SC_UNKNOWN_BLOCK,
	SC_INVALID_DATA,
	SC_ERROR,
};
class GameState
{
public:
	AsciiString realMapPathToPortableMapPath(const AsciiString &) const;
	AsciiString portableMapPathToRealMapPath(const AsciiString &) const;
	void addPostProcessSnapshot(Snapshot *);
	int m_postProcessCount;
};
extern GameState *TheGameState;
#endif
