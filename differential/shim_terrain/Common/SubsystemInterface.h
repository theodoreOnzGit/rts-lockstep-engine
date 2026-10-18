/* Shim for GameEngine/Include/Common/SubsystemInterface.h, for the Terrain,
** Multiplayer and Crate build only (differential tests). The same class as
** differential/shim/Common/SubsystemInterface.h, token for token (so the two
** builds agree on its layout and vtable), without that file's include of the
** shared STLTypedefs.h, which pulls in the UnicodeString stand-in (see
** shim_terrain/Common/STLTypedefs.h). */
#ifndef SHIM_SUBSYSTEMINTERFACE_H
#define SHIM_SUBSYSTEMINTERFACE_H
class SubsystemInterface
{
public:
	SubsystemInterface() {}
	virtual ~SubsystemInterface() {}
	virtual void init() = 0;
	virtual void postProcessLoad() { }
	virtual void reset() = 0;
	virtual void update() = 0;
};
#endif
