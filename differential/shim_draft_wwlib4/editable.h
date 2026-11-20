/* (#215, group wwlib4) stand-in for WWSaveLoad/editable.h: the LevelEdit property-sheet base. The saved bytes do not depend on it (no
** PARAM_EDITING_ON in the shipped build: DECLARE_EDITABLE expands to nothing); only the base class and the macro are provided. */
#ifndef __EDITABLE_H
#define __EDITABLE_H
#include "always.h"
#include "persist.h"
class EditableClass { public: virtual ~EditableClass(void) {} };
#define DECLARE_EDITABLE(_class, _parent)
#endif
