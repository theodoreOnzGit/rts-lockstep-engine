/* Stand-in for GameEngine/Include/Common/Xfer.h, SubsystemInterface build
** only (differential tests): SubsystemInterface.cpp names the class only to
** pass its pointer on to INI::load, which this build never calls. */
#ifndef SHIM_SUBSYS_XFER_H
#define SHIM_SUBSYS_XFER_H
class Xfer;
#endif
