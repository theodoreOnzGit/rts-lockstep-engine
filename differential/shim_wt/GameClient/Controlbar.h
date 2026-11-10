/* Shim for GameEngine/Include/GameClient/ControlBar.h (included as "GameClient/Controlbar.h"), differential/wrap_wt.cpp
** only (#77): the arrow image ControlBarArrowTransition asks for (getArrowImage, as the real header's inline), set by
** the test. */
#ifndef SHIM_WT_CONTROLBAR_H
#define SHIM_WT_CONTROLBAR_H
class Image;
class ControlBar
{
public:
	const Image *getArrowImage( void ){ return m_genArrow;	}
	const Image *m_genArrow;
};
extern ControlBar *TheControlBar;
#endif
