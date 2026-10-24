/* Stand-in for GameEngine/Include/GameClient/InGameUI.h, Upgrade/SpecialPower
** build only (differential tests, p1w2-upgrade): the one call
** UpgradeCenter::canAffordUpgrade makes (a message to the player), which the
** tests never reach. */
#ifndef SHIM_UPGRADE_INGAMEUI_H
#define SHIM_UPGRADE_INGAMEUI_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/GameType.h"
#include "GameClient/Color.h"
/* --- p1w12-sonnet --- (crate batch F: CrateCollide's execute animation; WorldAnimationOptions is GameClient/InGameUI.h lines 291-296, verbatim) */
class Anim2DTemplate;
enum WorldAnimationOptions
{
	WORLD_ANIM_NO_OPTIONS								= 0x00000000,
	WORLD_ANIM_FADE_ON_EXPIRE						= 0x00000001,
	WORLD_ANIM_PLAY_ONCE_AND_DESTROY		= 0x00000002,
};
/* --- end p1w12-sonnet --- */
extern void (*g_uiMessageHook)(const char *);   /* p1w13-sonnet, batch R24 (null but in differential/l9_overcharge_r24.inc) */
class InGameUI
{
public:
	void message(const char *label) { if (g_uiMessageHook) g_uiMessageHook(label); }   /* p1w13-sonnet, batch R24: a line for OverchargeBehavior's driver */
	void message(const class UnicodeString &text);   /* p1w13-sonnet, batch R27: the label of a fetched text (differential/l9_stealth_detector_r27.inc) */
	/* --- p1w12-sonnet --- */
	void addSuperweapon(Int playerIndex, const AsciiString &powerName, ObjectID id, const class SpecialPowerTemplate *powerTemplate);
	Bool removeSuperweapon(Int playerIndex, const AsciiString &powerName, ObjectID id, const class SpecialPowerTemplate *powerTemplate);
	void addWorldAnimation( Anim2DTemplate *animTemplate, const Coord3D *pos, WorldAnimationOptions options, Real durationInSeconds, Real zRisePerSecond );
	void addFloatingText(const class UnicodeString &text, const Coord3D *pos, Color color);   /* update batch L (differential/l9_updates_l.inc) */
	/* --- end p1w12-sonnet --- */
	/* --- p1w13-sonnet --- (batch P: ActiveBody's veterancy feedback; differential/l9_active_body_p.inc) */
	class Drawable *getFirstSelectedDrawable();
	Int getSelectCount();
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R5: RiderChangeContain moves the selection; differential/l9_open_contain_r.inc) */
	void selectDrawable( class Drawable *draw );
	void deselectDrawable( class Drawable *draw );
	void setDisplayedMaxWarning( Bool selected );
	/* --- end p1w13-sonnet --- */
};
extern InGameUI *TheInGameUI;
#endif
