/* Stand-in for GameEngine/Include/Common/ObjectStatusTypes.h, TunnelTracker tests only: the enumerators
** TunnelTracker.cpp names, with the values of the real enum (counted in order from OBJECT_STATUS_NONE = 0;
** the Rust side checks the same numbers: object_status_types.rs). The real header's BitFlags are left out. */
#ifndef SHIM_TUNNEL_OBJECTSTATUSTYPES_H
#define SHIM_TUNNEL_OBJECTSTATUSTYPES_H
enum ObjectStatusTypes
{
	OBJECT_STATUS_NONE = 0,
	OBJECT_STATUS_STEALTHED = 16,
	OBJECT_STATUS_DETECTED = 17,
	OBJECT_STATUS_DISGUISED = 43,
};
#endif
