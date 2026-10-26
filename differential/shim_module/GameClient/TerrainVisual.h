/* Stand-in for GameEngine/Include/GameClient/TerrainVisual.h, Module build only (differential tests, p1w12-sonnet, die batch D): DamDie.cpp includes it and
** uses nothing from it. */
#ifndef R54_SHIM_TERRAIN_VISUAL_H   /* (batch R54: a second includer, so a guard) */
#define R54_SHIM_TERRAIN_VISUAL_H
/* --- p1w13-sonnet --- (batch R54: BunkerBusterBehavior.cpp makes a static DomeStyleSeismicFilter, used only under DO_SEISMIC_SIMULATIONS, which the
** shipped build does not define: the class, with nothing of it used) */
class DomeStyleSeismicFilter {};
/* --- end p1w13-sonnet --- */
/* --- p1w13-sonnet --- (batch R70: WaveGuideUpdate pushes the water; a line, differential/l9_wave_guide_r70.inc) renamed by module_prelude.h */
#include "Lib/BaseType.h"
class TerrainVisual { public: void addWaterVelocity( Real worldX, Real worldY, Real velocity, Real preferredHeight ); };
extern TerrainVisual *TheTerrainVisual;
/* --- end p1w13-sonnet --- */
#endif
