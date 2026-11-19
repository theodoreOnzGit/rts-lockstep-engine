/* Differential-test wrapper for the game message parser draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/game_message_parser.rs (GameMessageParser, GameMessageParserArgumentType), cut verbatim by
** build.rs into OUT_DIR/draft_guilogic2_gen:
**   mp_enum.inc   Common/MessageStream.h, enum GameMessageArgumentDataType
**   mp_h.inc      GameNetwork/GameMessageParser.h, both classes and their inline members (to the end of the file)
**   mp_c.inc      GameNetwork/GameMessageParser.cpp, the constructors, destructors and addArgType (to the end of the file)
** build.rs renames GameMessageParser, GameMessageParserArgumentType, GameMessageArgumentDataType and GameMessage (-D).
** Stand-ins: GameMessage (the argument types in order; getArgumentCount is an UnsignedByte as upstream's m_argCount is, and
** getArgumentDataType answers UNKNOWN from index m_argCount on, as MessageStream.cpp:110 does).
** Entry points (extern "C", tests/differential_draft_msgparser.rs):
**   ref_mp_from_message(types, n, out, cap)   GameMessageParser(msg) over n argument types; writes [getNumTypes, then each run's
**                                              type and count]; the number of words (-1: too small)
**   ref_mp_manual(pairs, n, out, cap)         the default constructor, then addArgType(type, count) for each of n pairs;
**                                              writes [getNumTypes, then each run's type and count] */
#include "PreRTS.h"
#include <vector>
#include <string.h>
#include "Common/GameMemory.h"

#include "mp_enum.inc"

class GameMessage
{
public:
	std::vector<GameMessageArgumentDataType> m_types;
	UnsignedByte getArgumentCount( void ) const { return (UnsignedByte)m_types.size(); }
	GameMessageArgumentDataType getArgumentDataType( Int argIndex )
	{
		if( argIndex >= getArgumentCount() ) return ARGUMENTDATATYPE_UNKNOWN;
		return m_types[argIndex];
	}
};

#include "mp_h.inc"
#include "mp_c.inc"

static int dump( GameMessageParser &p, int *out, int cap )
{
	std::vector<int> o;
	o.push_back( p.getNumTypes() );
	for( GameMessageParserArgumentType *t = p.getFirstArgumentType(); t; t = t->getNext() )
	{
		o.push_back( (int)t->getType() );
		o.push_back( t->getArgCount() );
	}
	if( (int)o.size() > cap ) return -1;
	memcpy( out, o.data(), o.size() * sizeof( int ) );
	return (int)o.size();
}

extern "C" int ref_mp_from_message( const int *types, int n, int *out, int cap )
{
	GameMessage msg;
	for( int i = 0; i < n; ++i ) msg.m_types.push_back( (GameMessageArgumentDataType)types[i] );
	GameMessageParser p( &msg );
	return dump( p, out, cap );
}

extern "C" int ref_mp_manual( const int *pairs, int n, int *out, int cap )
{
	GameMessageParser p;
	for( int i = 0; i < n; ++i ) p.addArgType( (GameMessageArgumentDataType)pairs[2 * i], pairs[2 * i + 1] );
	return dump( p, out, cap );
}
