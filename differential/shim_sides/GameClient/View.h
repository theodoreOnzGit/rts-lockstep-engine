/* Stand-in for GameClient/View.h in the sides/scripts build: View::CameraShakeType
** (View.h lines 75-84), which Parameter::getUiText names. */
#ifndef SHIM_SIDES_VIEW_H
#define SHIM_SIDES_VIEW_H
class View
{
public:
	enum CameraShakeType
	{
		SHAKE_SUBTLE = 0,
		SHAKE_NORMAL,
		SHAKE_STRONG,
		SHAKE_SEVERE,
		SHAKE_CINE_EXTREME,
		SHAKE_CINE_INSANE,
		SHAKE_COUNT
	};
};
#endif
