/* (#215, group wwlib4) stand-in for WWSaveLoad/twiddler.h, which definitionmgr.cpp includes to call ((TwiddlerClass *)definition)->Twiddle()
** on a definition of class CLASSID_TWIDDLERS. The real Twiddle() picks a random member of the twiddler's list (RandomClass seeded from
** TIMEGETTIME); here it returns the definition itself, which is what the draft's twiddle_definition does for a definition that is not a
** TwiddlerClass. The twiddler's own behaviour is not tested here (twiddler.rs). */
#ifndef __TWIDDLER_H
#define __TWIDDLER_H
#include "definition.h"
#include "definitionclassids.h"
class TwiddlerClass : public DefinitionClass
{
public:
	DefinitionClass *Twiddle(void) const { return (DefinitionClass *)this; }
};
#endif
