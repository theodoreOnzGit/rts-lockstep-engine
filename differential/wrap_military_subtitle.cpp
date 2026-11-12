/* Differential-test wrapper for the military subtitle (GitHub #151, upstream commit 0a05454d), cut verbatim by build.rs
** into OUT_DIR/military_subtitle_gen:
**   ms_struct.inc    GameEngine/Include/GameClient/InGameUI.h, `struct MilitarySubtitleData` (to the `ObjectList`
**                    typedef after it)
**   ms_create.inc    GameEngine/Source/GameClient/InGameUI.cpp, InGameUI::militarySubtitle and removeMilitarySubtitle
**                    (to `Bool InGameUI::areSelectedObjectsControllable() const`)
**   ms_update.inc    InGameUI.cpp, update()'s block from `// Update the Military Subtitle display` to `// update the
**                    player money window`
**   ms_draw.inc      InGameUI.cpp, postDraw()'s block from `if( m_militarySubtitle )` (with `ICoord2D pos;`) to `// draw
**                    superweapon timers`
**   ms_tooltips.inc  InGameUI.cpp, disableTooltipsUntil, clearTooltipsDisabled, areTooltipsDisabled (to
**                    `WindowMsgHandledType IdleWorkerSystem(`)
** No edit. Compiled inside namespace ms as the members of a stand-in InGameUI holding the members they use (the update's
** block in updateSubtitle(), with update()'s locals before it: currLogicFrame, r, g, b, a, amount; the draw's in
** drawSubtitle()). REAL_TO_INT is the real BaseType.h's, on wrap_base_type.cpp's transcription of its asm.
**
** Everything else is a STAND-IN, the same rules as the Rust test's host (tests/differential_military_subtitle.rs):
** - UnicodeString: the shim's, with `set(const UnicodeString &)` (UnicodeString.h's, a copy);
** - TheGameText->fetch(label): the label's bytes as units; TheGameLogic->getFrame(): the sequence's frame;
**   TheScriptEngine->isTimeFrozenScript(): the sequence's flag; TheDisplay: the sequence's size, drawFillRect recorded;
** - TheGlobalLanguageData: m_militaryCaptionDelayMS and m_militaryCaptionSpeed the sequence's, adjustFontSize(s) = s + 2;
** - TheFontLibrary->getFont(name, size, bold): one GameFont per triple, height = size;
** - DisplayString: its text and font; getSize: width = units * (pointSize / 2 + 1), height = pointSize + 1; draw
**   recorded; TheDisplayStringManager: new / delete;
** - TheAudio->addAudioEvent and UpdateDiplomacyBriefingText recorded.
**
** Entry point (extern "C"): ref_military_subtitle_seq(in, nin, out, cap). `in` is 16 words of configuration [display
** width, height, delay ms, language speed, InGameUI speed, caption x, y, colour r, g, b, a, title size, caption size,
** title bold, caption bold, 0], then steps of 4 words [kind, a, b, c]:
**   kind 0  militarySubtitle(LABELS[a], duration b)     kind 1  the frame a, frozen b
**   kind 2  the update's block                          kind 3  the draw's block
**   kind 4  removeMilitarySubtitle()
** The calls as they are made: [1] the typing sound, [2, label] UpdateDiplomacyBriefingText, [3, x, y, text FNV-1a, font
** FNV-1a, colour, drop colour] a string drawn, [4, x, y, width, height, colour] drawFillRect. After each step [100, the
** subtitle exists, then (if it does) index, currentDisplayString, lifetime, blockDrawn, blockBeginFrame, blockPos x, y,
** incrementOnFrame, colour, position x, y, and per line 0..currentDisplayString its text's and font's FNV-1a; then
** m_tooltipsDisabledUntil and areTooltipsDisabled()]. A step that leaves a fifth line (currentDisplayString 4, which the
** next draw or removal would read past displayStrings: undefined) ends the sequence with [-999]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <map>
#include <vector>
#include <string>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/Debug.h"

extern "C" { int ref_fast_float2long_round(float f); float ref_fast_float_trunc(float f); }
#ifndef REAL_TO_INT
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
#endif

namespace ms {

std::vector<int> g_out;
static void rec(const std::vector<Int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }

typedef Int Color;
inline Color GameMakeColor( UnsignedByte red, UnsignedByte green, UnsignedByte blue, UnsignedByte alpha )
{
	return (alpha << 24) | (red << 16) | (green << 8) | (blue);
}
/* GameGetColorComponents (Color.cpp:92), transcribed */
static void GameGetColorComponents( Color color, UnsignedByte *red, UnsignedByte *green, UnsignedByte *blue, UnsignedByte *alpha )
{
	*alpha	= (color & 0xFF000000) >> 24;
	*red		= (color & 0x00FF0000) >> 16;
	*green	= (color & 0x0000FF00) >> 8;
	*blue		= (color & 0x000000FF);
}
enum { LOGICFRAMES_PER_SECOND = 30 };
enum { MAX_SUBTITLE_LINES = 4 };
#define NEW new

class UnicodeString : public ::UnicodeString
{
public:
	UnicodeString() {}
	UnicodeString( const ::UnicodeString &o ) : ::UnicodeString(o) {}
	void set( const ::UnicodeString &o ) { ::UnicodeString::operator=(o); }
};

static Int fnv_units( const ::UnicodeString &t )
{
	UnsignedInt h = 2166136261u;
	for (Int i = 0; i < t.getLength(); ++i) {
		h ^= (UnsignedInt)(unsigned short)t.getCharAt(i);
		h *= 16777619u;
	}
	return (Int)h;
}

struct GameFont { AsciiString nameString; Int pointSize; Int height; Bool bold; };
static Int fnv_font( const GameFont *f )
{
	UnsignedInt h = 2166136261u;
	for (Int i = 0; i < f->nameString.getLength(); ++i) {
		h ^= (UnsignedInt)(unsigned char)f->nameString.str()[i];
		h *= 16777619u;
	}
	h ^= (UnsignedInt)f->pointSize; h *= 16777619u;
	h ^= (UnsignedInt)(f->bold ? 1 : 0); h *= 16777619u;
	return (Int)h;
}
struct FontLibraryStub
{
	std::map<std::string, GameFont *> m_fonts;
	GameFont *getFont( AsciiString name, Int pointSize, Bool bold )
	{
		char key[64];
		snprintf(key, sizeof key, "/%d/%d", pointSize, bold ? 1 : 0);
		std::string k = std::string(name.str()) + key;
		std::map<std::string, GameFont *>::iterator it = m_fonts.find(k);
		if (it != m_fonts.end()) return it->second;
		GameFont *f = new GameFont;
		f->nameString = name; f->pointSize = pointSize; f->height = pointSize; f->bold = bold;
		m_fonts[k] = f;
		return f;
	}
};
FontLibraryStub g_fonts;
FontLibraryStub *TheFontLibrary = &g_fonts;

class DisplayString
{
public:
	::UnicodeString m_text;
	GameFont *m_font;
	DisplayString() : m_font(NULL) {}
	void reset( void ) { m_text = ::UnicodeString(); m_font = NULL; }
	void setFont( GameFont *f ) { m_font = f; }
	GameFont *getFont( void ) { return m_font; }
	void appendChar( WideChar c ) { m_text.concat(c); }
	void getSize( Int *width, Int *height )
	{
		Int ps = m_font ? m_font->pointSize : 0;
		if (width) *width = m_text.getLength() * (ps / 2 + 1);
		if (height) *height = ps + 1;
	}
	void draw( Int x, Int y, Color color, Color dropColor ) { rec({3, x, y, fnv_units(m_text), m_font ? fnv_font(m_font) : 0, color, dropColor}); }
};
struct DisplayStringManagerStub
{
	DisplayString *newDisplayString( void ) { return new DisplayString; }
	void freeDisplayString( DisplayString *d ) { delete d; }
};
DisplayStringManagerStub g_dsm;
DisplayStringManagerStub *TheDisplayStringManager = &g_dsm;

struct Config { Int width, height, delay, langSpeed, iniSpeed, x, y, r, g, b, a, titleSize, captionSize, titleBold, captionBold; };
Config g_cfg;
UnsignedInt g_frame;
Bool g_frozen;

struct GlobalLanguageStub
{
	Int m_militaryCaptionDelayMS;
	Int m_militaryCaptionSpeed;
	Int adjustFontSize( Int s ) { return s + 2; }
};
GlobalLanguageStub g_lang;
GlobalLanguageStub *TheGlobalLanguageData = &g_lang;
struct GameLogicStub { UnsignedInt getFrame( void ) { return g_frame; } };
GameLogicStub g_logic;
GameLogicStub *TheGameLogic = &g_logic;
struct ScriptEngineStub { Bool isTimeFrozenScript( void ) { return g_frozen; } };
ScriptEngineStub g_se;
ScriptEngineStub *TheScriptEngine = &g_se;
struct DisplayStub
{
	UnsignedInt getWidth( void ) { return (UnsignedInt)g_cfg.width; }
	UnsignedInt getHeight( void ) { return (UnsignedInt)g_cfg.height; }
	void drawFillRect( Int x, Int y, Int w, Int h, Color c ) { rec({4, x, y, w, h, c}); }
};
DisplayStub g_display;
DisplayStub *TheDisplay = &g_display;
struct GameTextStub { ::UnicodeString fetch( AsciiString label ) { ::UnicodeString u; u.translate(label); return u; } };
GameTextStub g_text;
GameTextStub *TheGameText = &g_text;
class AudioEventRTS { public: AsciiString m_name; AudioEventRTS( const char *n ) : m_name(n) {} };
struct AudioStub { void addAudioEvent( const AudioEventRTS * ) { rec({1}); } };
AudioStub g_audio;
AudioStub *TheAudio = &g_audio;

static const char *const LABELS[] = { "Hi", "Two\nlines", "A\nB\nC\nD", "1\n2\n3\n4\n5", "", "\n\nX", "Long caption text typed out", "Ends\n" };
static const Int NLABELS = 8;
static void UpdateDiplomacyBriefingText( AsciiString label, Bool clear )
{
	Int li = -1;
	for (Int i = 0; i < NLABELS; ++i)
		if (label == AsciiString(LABELS[i]))
			li = i;
	rec({2, li});
}

class InGameUI
{
public:
#include "ms_struct.inc"
	MilitarySubtitleData *m_militarySubtitle;
	UnsignedInt m_tooltipsDisabledUntil;
	RGBAColorInt m_militaryCaptionColor;
	ICoord2D m_militaryCaptionPosition;
	AsciiString m_militaryCaptionTitleFont;
	Int m_militaryCaptionTitlePointSize;
	Bool m_militaryCaptionTitleBold;
	AsciiString m_militaryCaptionFont;
	Int m_militaryCaptionPointSize;
	Bool m_militaryCaptionBold;
	Int m_militaryCaptionSpeed;

	void militarySubtitle( const AsciiString& label, Int duration );
	void removeMilitarySubtitle( void );
	void disableTooltipsUntil( UnsignedInt frameNum );
	void clearTooltipsDisabled();
	Bool areTooltipsDisabled() const;
	void updateSubtitle( void );
	void drawSubtitle( void );
};
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;

#include "ms_create.inc"

#include "ms_tooltips.inc"

void InGameUI::updateSubtitle( void )
{
	UnsignedInt currLogicFrame = TheGameLogic->getFrame();
	UnsignedByte r, g, b, a;
	Int amount;
#include "ms_update.inc"
}

void InGameUI::drawSubtitle( void )
{
#include "ms_draw.inc"
}

} /* namespace ms */

extern "C" int ref_military_subtitle_seq(const int *in, int nin, int *out, int cap)
{
	using namespace ms;
	g_out.clear();
	const int *c = in;
	g_cfg.width = c[0]; g_cfg.height = c[1]; g_cfg.delay = c[2]; g_cfg.langSpeed = c[3]; g_cfg.iniSpeed = c[4];
	g_lang.m_militaryCaptionDelayMS = c[2];
	g_lang.m_militaryCaptionSpeed = c[3];
	InGameUI &u = g_ui;
	/* a new program's InGameUI (the constructor's NULL and 0), then the sequence's members */
	if (u.m_militarySubtitle) {
		u.m_militarySubtitle = NULL;
	}
	u.m_tooltipsDisabledUntil = 0;
	u.m_militaryCaptionSpeed = c[4];
	u.m_militaryCaptionPosition.x = c[5]; u.m_militaryCaptionPosition.y = c[6];
	u.m_militaryCaptionColor.red = c[7]; u.m_militaryCaptionColor.green = c[8]; u.m_militaryCaptionColor.blue = c[9]; u.m_militaryCaptionColor.alpha = c[10];
	u.m_militaryCaptionTitleFont = AsciiString("Courier"); u.m_militaryCaptionTitlePointSize = c[11]; u.m_militaryCaptionTitleBold = c[13] != 0;
	u.m_militaryCaptionFont = AsciiString("Arial"); u.m_militaryCaptionPointSize = c[12]; u.m_militaryCaptionBold = c[14] != 0;
	g_frame = 0;
	g_frozen = FALSE;
	for (int k = 16; k + 4 <= nin; k += 4) {
		const int *w = in + k + 1;
		switch (in[k]) {
		case 0: u.militarySubtitle(AsciiString(LABELS[w[0] % NLABELS]), w[1]); break;
		case 1: g_frame = (UnsignedInt)w[0]; g_frozen = w[1] != 0; break;
		case 2: u.updateSubtitle(); break;
		case 3: u.drawSubtitle(); break;
		case 4: u.removeMilitarySubtitle(); break;
		}
		InGameUI::MilitarySubtitleData *m = u.m_militarySubtitle;
		rec({100, m ? 1 : 0});
		if (m) {
			rec({(Int)m->index, (Int)m->currentDisplayString, (Int)m->lifetime, m->blockDrawn ? 1 : 0, (Int)m->blockBeginFrame, m->blockPos.x, m->blockPos.y, (Int)m->incrementOnFrame, m->color, m->position.x, m->position.y});
			for (UnsignedInt i = 0; i <= m->currentDisplayString && i < MAX_SUBTITLE_LINES; ++i)
				rec({fnv_units(m->displayStrings[i]->m_text), m->displayStrings[i]->m_font ? fnv_font(m->displayStrings[i]->m_font) : 0});
		}
		rec({(Int)u.m_tooltipsDisabledUntil, u.areTooltipsDisabled() ? 1 : 0});
		/* a fifth line: the next draw or removal reads past displayStrings (undefined). The Rust stops the sequence
		** before this update; a planted Rust may not, so the program stops here rather than crash. */
		if (m && m->currentDisplayString >= MAX_SUBTITLE_LINES) {
			rec({-999});
			break;
		}
	}
	/* (the program's end: the subtitle dropped) */
	u.m_militarySubtitle = NULL;
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n > 0)
		memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
