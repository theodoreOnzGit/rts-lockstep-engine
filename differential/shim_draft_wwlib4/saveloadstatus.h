/* (#215, group wwlib4) stand-in for WWSaveLoad/saveloadstatus.h: the load-progress text and count are a display concern (saveloadstatus.rs
** is its own unit); the calls SaveLoadSystemClass::Load makes are no-ops here. */
#ifndef WWSAVELOADSTATUS_H
#define WWSAVELOADSTATUS_H
namespace SaveLoadStatus {
	inline void Set_Status_Text(const char *, int) {}
	inline void Reset_Status_Count(void) {}
	inline void Inc_Status_Count(void) {}
	inline int Get_Status_Count(void) { return 0; }
}
#define INIT_STATUS(t) SaveLoadStatus::Set_Status_Text(t,0)
#define INIT_SUB_STATUS(t) SaveLoadStatus::Set_Status_Text(t,1)
#endif
