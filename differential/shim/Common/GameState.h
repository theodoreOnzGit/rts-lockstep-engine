/* Shim for GameEngine/Include/Common/GameState.h, differential tests only.
** Xfer::xferMapName (NOT translated, NOT tested) calls the two map-path
** functions; XferLoad::xferSnapshot calls addPostProcessSnapshot, which the
** wrapper counts so the XO_NO_POST_PROCESSING option can be compared. */
#ifndef SHIM_GAMESTATE_H
#define SHIM_GAMESTATE_H
#include "Common/AsciiString.h"
class Snapshot;
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
