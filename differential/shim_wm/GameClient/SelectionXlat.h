/* Shim for GameEngine/Include/GameClient/SelectionXlat.h, differential/wrap_wm.cpp only (#70). */
#ifndef SHIM_WM_SELECTIONXLAT_H
#define SHIM_WM_SELECTIONXLAT_H
#include "Lib/BaseType.h"
class SelectionTranslator
{
public:
	void setDragSelecting(Bool);
	void setLeftMouseButton(Bool);
};
extern SelectionTranslator *TheSelectionTranslator;
class View
{
public:
	void setMouseLock(Bool);
};
extern View *TheTacticalView;
#endif
