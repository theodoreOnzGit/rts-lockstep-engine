/* Differential-test wrapper for Player.cpp (upstream commit 0a05454d), p1w10-sonnet, slice 1:
**   GeneralsMD/Code/GameEngine/Include/Common/Player.h           (unchanged: the whole class)
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp      (the functions of slice 1, each taken whole and unchanged by
**   build.rs into OUT_DIR/player_gen/player_fns.inc, with the statics they use; PlayerRelationMap's five functions)
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp        (as in wrap_team.cpp: one mechanical edit of the include block)
**   Energy.cpp, Money.cpp, Handicap.cpp, MissionStats.cpp        (the functions slice 1 calls, whole and unchanged)
** Every other class of the world that these reach (objects, the AI, the script engine, the client, the score keeper, the
** academy statistics, the rank and science stores, ...) is a stand-in (shim_player/player_standins_decl.h, player_driver.h)
** that answers from the program's configuration and writes a line for every call that has an effect; all of them, and the
** real classes of the files above, are renamed by the #define lines below so that no symbol of this archive collides with
** another fixture's.
**
** Entry point (extern "C", tests/differential_player.rs): ref_player_run. */
/* the real headers, the stand-ins of the world, the unchanged functions (the renames and the readable members are the
** forced include shim_player/player_prelude.h's) */
#include "Common/NameKeyGenerator.h"
#include "Common/INI.h"
#include "Common/Xfer.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/DisabledTypes.h"
#include "Common/WellKnownKeys.h"
#include "Common/GameCommon.h"
#include "Common/Snapshot.h"
#include "Common/XferCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/Team.h"
/* the Xfer family, renamed (player_prelude.h) and compiled here, unchanged: Player::xfer reaches xferScienceVec and xferUpgradeMask, which
** look the sciences and the upgrades up in the world of this fixture (TheScienceStore, TheUpgradeCenter) */
#include "matrix3d.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Xfer.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/XferCRC.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/XferSave.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/XferLoad.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Snapshot.cpp"
#define DEFINE_SCIENCE_AVAILABILITY_NAMES
#include "Common/Player.h"
#include "Team_cut.cpp"
#include "player_fns.inc"
#include "player_driver.h"
