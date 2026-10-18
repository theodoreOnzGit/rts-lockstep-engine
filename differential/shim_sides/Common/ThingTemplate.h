/* Stand-in for Common/ThingTemplate.h in the sides/scripts build: BuildableStatus
** and its name table (ThingTemplate.h lines 217-238), which Parameter::getUiText names. */
#ifndef SHIM_SIDES_THINGTEMPLATE_H
#define SHIM_SIDES_THINGTEMPLATE_H
enum BuildableStatus
{
	BSTATUS_YES = 0,
	BSTATUS_IGNORE_PREREQUISITES,
	BSTATUS_NO,
	BSTATUS_ONLY_BY_AI,
	BSTATUS_NUM_TYPES
};
#ifdef DEFINE_BUILDABLE_STATUS_NAMES
static const char *BuildableStatusNames[] =
{
	"Yes",
	"Ignore_Prerequisites",
	"No",
	"Only_By_AI",
	NULL
};
#endif
#endif
