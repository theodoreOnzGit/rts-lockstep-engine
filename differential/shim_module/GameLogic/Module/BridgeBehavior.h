/* Stand-in for GameEngine/Include/GameLogic/Module/BridgeBehavior.h, Module build only (differential tests, p1w13-sonnet, batch P): ActiveBody mirrors its
** indestructible flag on the towers of a bridge. The interface is the six pure virtuals of the real header (lines 79-92, verbatim); BridgeTowerType is
** GameClient/TerrainRoads.h lines 47-55, verbatim apart from the comment; getBridgeBehaviorInterfaceFromObject is the real function (BridgeBehavior.cpp), cut by build.rs into
** mod_gen/BridgeBehaviorFromObject.inc and compiled by differential/l9_active_body_p.inc. */
#ifndef SHIM_MODULE_BRIDGEBEHAVIOR_H
#define SHIM_MODULE_BRIDGEBEHAVIOR_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"
class Object;
enum BridgeTowerType
{
	BRIDGE_TOWER_FROM_LEFT = 0,
	BRIDGE_TOWER_FROM_RIGHT,
	BRIDGE_TOWER_TO_LEFT,
	BRIDGE_TOWER_TO_RIGHT,

	BRIDGE_MAX_TOWERS				///< keep this last
};
class BridgeBehaviorInterface
{

public:

	virtual void setTower( BridgeTowerType towerType, Object *tower ) = 0;
	virtual ObjectID getTowerID( BridgeTowerType towerType ) = 0;
	virtual void createScaffolding( void ) = 0;
	virtual void removeScaffolding( void ) = 0;
	virtual Bool isScaffoldInMotion( void ) = 0;
	virtual Bool isScaffoldPresent( void ) = 0;

};
class BridgeBehavior
{
public:
	static BridgeBehaviorInterface *getBridgeBehaviorInterfaceFromObject( Object *obj );
};
#endif
