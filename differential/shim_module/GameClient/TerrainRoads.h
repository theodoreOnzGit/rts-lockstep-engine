/* Stand-in for GameEngine/Include/GameClient/TerrainRoads.h, Module build only (differential tests, p1w13-sonnet, batch R77): what BridgeBehavior reads
** of a bridge template. BridgeTowerType is the module build's (GameLogic/Module/BridgeBehavior.h, the same lines); MAX_BRIDGE_BODY_FX and the
** getters BridgeBehavior calls are TerrainRoads.h's (lines 57, 92-104 and the members they read), verbatim; the collection's findBridge is the
** test's (differential/l9_bridge_r77.inc). */
#ifndef SHIM_MODULE_TERRAINROADS_H
#define SHIM_MODULE_TERRAINROADS_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "GameLogic/Module/BodyModule.h"
#include "GameLogic/Module/BridgeBehavior.h"
enum { MAX_BRIDGE_BODY_FX = 3 };
class TerrainRoadType
{
public:
	inline AsciiString getScaffoldObjectName( void ) { return m_scaffoldObjectName; }
	inline AsciiString getScaffoldSupportObjectName( void ) { return m_scaffoldSupportObjectName; }
	inline AsciiString getDamageToSoundString( BodyDamageType state ) { return m_damageToSoundString[ state ]; }
	inline AsciiString getDamageToOCLString( BodyDamageType state, Int index ) { return m_damageToOCLString[ state ][ index ]; }
	inline AsciiString getDamageToFXString( BodyDamageType state, Int index ) { return m_damageToFXString[ state ][ index ]; }
	inline AsciiString getRepairedToSoundString( BodyDamageType state ) { return m_repairedToSoundString[ state ]; }
	inline AsciiString getRepairedToOCLString( BodyDamageType state, Int index ) { return m_repairedToOCLString[ state ][ index ]; }
	inline AsciiString getRepairedToFXString( BodyDamageType state, Int index ) { return m_repairedToFXString[ state ][ index ]; }
	inline Real getTransitionEffectsHeight( void ) { return m_transitionEffectsHeight; }
	inline Int getNumFXPerType( void ) { return m_numFXPerType; }
	AsciiString m_name;
	AsciiString m_scaffoldObjectName;
	AsciiString m_scaffoldSupportObjectName;
	AsciiString m_damageToSoundString[ BODYDAMAGETYPE_COUNT ];
	AsciiString m_damageToOCLString[ BODYDAMAGETYPE_COUNT ][ MAX_BRIDGE_BODY_FX ];
	AsciiString m_damageToFXString[ BODYDAMAGETYPE_COUNT ][ MAX_BRIDGE_BODY_FX ];
	AsciiString m_repairedToSoundString[ BODYDAMAGETYPE_COUNT ];
	AsciiString m_repairedToOCLString[ BODYDAMAGETYPE_COUNT ][ MAX_BRIDGE_BODY_FX ];
	AsciiString m_repairedToFXString[ BODYDAMAGETYPE_COUNT ][ MAX_BRIDGE_BODY_FX ];
	Real m_transitionEffectsHeight;
	Int m_numFXPerType;
};
class TerrainRoadCollection
{
public:
	TerrainRoadType *findBridge( AsciiString name );
};
extern TerrainRoadCollection *TheTerrainRoads;
#endif
