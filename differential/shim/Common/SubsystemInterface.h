/* Shim for GameEngine/Include/Common/SubsystemInterface.h, differential
** tests only. The real header pulls in INI.h; the archive classes need only
** the base class with its pure virtual init/reset/update and the default
** postProcessLoad (copied from the real header, timing members omitted). */
#ifndef SHIM_SUBSYSTEMINTERFACE_H
#define SHIM_SUBSYSTEMINTERFACE_H
#include "Common/STLTypedefs.h"
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
