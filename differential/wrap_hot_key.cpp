/* Differential-test wrapper for the control bar's hot keys (GitHub #131, upstream commit 0a05454d), cut verbatim by
** build.rs into OUT_DIR/hot_key_gen:
**   hot_key_class.inc    HotKey.h, `class HotKeyTranslator` to `extern HotKeyManager *TheHotKeyManager;` (the three
**                        classes);
**   hot_key.inc          HotKey.cpp, HotKeyTranslator::translateGameMessage to `HotKeyManager *TheHotKeyManager = NULL;`
**                        (the translator, HotKey(), the manager's constructor, destructor, init, reset, addHotKey,
**                        executeHotKey and both searchHotKey);
**   key_mod_state.inc    MetaEvent.h, `enum MappableKeyModState` to the `ModifierNames` after it (CTRL, ALT, SHIFT);
**   language_ids.inc     Language.h, `LANGUAGE_ID_US = 0,` to `} LanguageID;` (the wrapper opens the `typedef enum {`
**                        and closes it);
**   init_key_names.inc   Keyboard.cpp, Keyboard::initKeyNames, to its closing `}  // end initKeyNames` (the wrapper
**                        closes it);
**   printable_head.inc   Keyboard.cpp, Keyboard::getPrintableKey up to its `return L'';`, and
**   printable_tail.inc   from the `if(state == 0)` after it to the end of the file. **The one text not upstream's:** the
**                        wrapper writes `return 0;` between them, because g++ rejects the empty character constant
**                        `L''` (MSVC compiled it; its value is taken as 0, the "no name" of the other entries).
** They are compiled inside namespace hkx on stand-ins: AsciiString and UnicodeString with the real classes' meaning for
** what is used (`AsciiString::translate` narrows each character with `(char)` and `concat('\0')` appends nothing;
** `toLower` is `tolower` per char), the window (status bits, parent, id), the window manager (records
** `winSendSystemMsg`), the audio (records the event's name), the keyboard (the cut functions, on the stand-in's
** `m_keyNames`), the game text (unused by the tests). **Not upstream's:** `UnicodeString::set(const WideChar *)` copies
** one character: HotKeyTranslator calls it on a single `WideChar` (`uKey.set(&key)`), and the real `set` reads on to
** the next zero on the stack, which no test can reproduce; the game behaves as if that zero came right after it.
** The messages are the real GameMessage of the message stream build (wrap_message_stream.cpp), under its names.
**
** Entry point (extern "C", tests/differential_hot_key.rs): ref_hot_key_case(kind, in, nin, out, cap).
**   kind 0: in: OurLanguage, the keyboard layout (GetKeyboardLayout's value). out: m_shift2Key, then for each key 0..255
**     getPrintableKey(key, s) for s = -1, 0, 1, 2, 3.
**   kind 1: a sequence on a new manager and translator. in: language, layout, then steps [op, ...]:
**     0 reset                         -> out 1000
**     1 addHotKey(win, key): win, n, n bytes -> out 1001
**     2 a key message: down (1) / up (0), key, state -> out 1002, disposition
**     3 window win: hidden, enabled   -> out 1003
**     4 searchHotKey(UnicodeString): n, n characters -> out 1004, n, the key's bytes
**     5 executeHotKey(key): n, n bytes -> out 1005, the answer
**   every winSendSystemMsg adds (2000, parent, msg, window, id) and every addAudioEvent (3000, name bytes..., 0) as it
**   happens. Windows 0..7 have parent 100 + i and id 200 + i. */
#define GameMessage MsTestGameMessage
#define GameMessageArgument MsTestGameMessageArgument
#define GameMessageList MsTestGameMessageList
#define GameMessageTranslator MsTestGameMessageTranslator
#define MessageStream MsTestMessageStream
#define CommandList MsTestCommandList
#define TheMessageStream MsTestTheMessageStream
#define TheCommandList MsTestTheCommandList
#define buildRegion MsTestBuildRegion

#include <ctype.h>
#include <stdlib.h>
#include <string.h>
#include <map>
#include <string>
#include <vector>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/MessageStream.h"
#include "GameClient/KeyDefs.h"

namespace hkx {

std::vector<int> g_out;

/* ---- the strings, with the meaning of the real classes for what HotKey.cpp uses ---- */
class UnicodeString
{
public:
	static const UnicodeString TheEmptyString;
	std::basic_string<WideChar> m_s;
	Bool isEmpty() const { return m_s.empty(); }
	const WideChar *str() const { return m_s.c_str(); }
	Int getLength() const { return (Int)m_s.size(); }
	WideChar getCharAt(Int i) const { return m_s[i]; }
	/* the real concat(WideChar) appends the one-character string {c, 0}: nothing for a zero */
	void concat(WideChar c) { if (c) m_s += c; }
	/* NOT the real set (see the header): one character */
	void set(const WideChar *s) { m_s.clear(); if (s && *s) m_s += *s; }
};
const UnicodeString UnicodeString::TheEmptyString;
class AsciiString
{
public:
	static const AsciiString TheEmptyString;
	std::string m_s;
	void clear() { m_s.clear(); }
	void set(const AsciiString &a) { m_s = a.m_s; }
	void set(const char *s) { m_s = s ? s : ""; }
	const char *str() const { return m_s.c_str(); }
	Bool isEmpty() const { return m_s.empty(); }
	Bool isNotEmpty() const { return !m_s.empty(); }
	/* AsciiString.cpp:226 */
	void toLower() { for (size_t i = 0; i < m_s.size(); ++i) m_s[i] = tolower(m_s[i]); }
	/* AsciiString.cpp:178: concat((char)c), and concat of a zero char appends nothing */
	void translate(const UnicodeString &u) { clear(); for (Int i = 0; i < u.getLength(); ++i) { char c = (char)u.getCharAt(i); if (c) m_s += c; } }
};
const AsciiString AsciiString::TheEmptyString;
inline Bool operator<(const AsciiString &a, const AsciiString &b) { return strcmp(a.str(), b.str()) < 0; }

/* ---- the windows, the window manager, the audio, the game text ---- */
enum { WIN_STATUS_ENABLED = 0x00000002, WIN_STATUS_HIDDEN = 0x00000004 };
enum { GBM_SELECTED = 7 };
typedef void *WindowMsgData;
class GameWindow
{
public:
	Int m_index;
	UnsignedInt m_status;
	GameWindow *m_parent;
	UnsignedInt winGetStatus() { return m_status; }
	GameWindow *winGetParent() { return m_parent; }
	Int winGetWindowId() { return 200 + m_index; }
};
GameWindow g_wins[8], g_parents[8];
class GameWindowManager
{
public:
	void winSendSystemMsg(GameWindow *to, Int msg, WindowMsgData d1, Int d2)
	{
		Int w[5] = { 2000, 100 + to->m_index, msg, ((GameWindow *)d1)->m_index, d2 };
		g_out.insert(g_out.end(), w, w + 5);
	}
};
GameWindowManager g_wm;
GameWindowManager *TheWindowManager = &g_wm;
class AudioEventRTS
{
public:
	std::string m_name;
	AudioEventRTS(const char *name) : m_name(name) {}
};
class GameAudio
{
public:
	void addAudioEvent(AudioEventRTS *e)
	{
		g_out.push_back(3000);
		for (size_t i = 0; i < e->m_name.size(); ++i) g_out.push_back((unsigned char)e->m_name[i]);
		g_out.push_back(0);
	}
};
GameAudio g_audio;
GameAudio *TheAudio = &g_audio;
class GameText { public: UnicodeString fetch(const AsciiString &) { return UnicodeString(); } };
GameText g_text;
GameText *TheGameText = &g_text;
class SubsystemInterface
{
public:
	virtual ~SubsystemInterface() {}
	virtual void init() = 0;
	virtual void reset() = 0;
	virtual void update() = 0;
};

/* ---- the keyboard ---- */
typedef enum
{
#include "language_ids.inc"
} LanguageID;
LanguageID OurLanguage;
typedef UnsignedInt HKL;
UnsignedInt g_layout;
static HKL GetKeyboardLayout(Int) { return g_layout; }
class Keyboard
{
public:
	enum { MAX_KEY_STATES = 3 };
	enum { KEY_NAMES_COUNT = 256 };
	struct
	{
		WideChar stdKey;
		WideChar shifted;
		WideChar shifted2;
	} m_keyNames[ KEY_NAMES_COUNT ];
	UnsignedByte m_shift2Key;
	void initKeyNames( void );
	WideChar getPrintableKey( UnsignedByte key, Int state );
};
#include "init_key_names.inc"
}
#include "printable_head.inc"
		return 0;
#include "printable_tail.inc"
Keyboard g_keyboard;
Keyboard *TheKeyboard = &g_keyboard;

struct LookupListRec { const char *name; Int value; };
#include "key_mod_state.inc"
#include "hot_key_class.inc"
HotKeyManager *TheHotKeyManager = NULL;
#include "hot_key.inc"

} // namespace hkx

using namespace hkx;

static hkx::AsciiString bytes(const int *in, Int n)
{
	hkx::AsciiString a;
	for (Int i = 0; i < n; ++i) a.m_s += (char)in[i];
	return a;
}

extern "C" int ref_hot_key_case(int kind, const int *in, int nin, int *out, int cap)
{
	g_out.clear();
	if (kind == 0) {
		OurLanguage = (LanguageID)in[0];
		g_layout = (UnsignedInt)in[1];
		memset(&g_keyboard, 0, sizeof g_keyboard);
		g_keyboard.initKeyNames();
		g_out.push_back(g_keyboard.m_shift2Key);
		for (Int k = 0; k < 256; ++k)
			for (Int s = -1; s <= 3; ++s) g_out.push_back(g_keyboard.getPrintableKey((UnsignedByte)k, s));
	} else if (kind == 1) {
		OurLanguage = (LanguageID)in[0];
		g_layout = (UnsignedInt)in[1];
		memset(&g_keyboard, 0, sizeof g_keyboard);
		g_keyboard.initKeyNames();
		for (Int i = 0; i < 8; ++i) {
			g_wins[i].m_index = i; g_wins[i].m_status = WIN_STATUS_ENABLED; g_wins[i].m_parent = &g_parents[i];
			g_parents[i].m_index = i; g_parents[i].m_status = 0; g_parents[i].m_parent = NULL;
		}
		HotKeyManager manager;
		manager.init();
		TheHotKeyManager = &manager;
		HotKeyTranslator x;
		Int p = 2;
		while (p < nin) {
			Int op = in[p++];
			if (op == 0) {
				manager.reset();
				g_out.push_back(1000);
			} else if (op == 1) {
				Int win = in[p], n = in[p + 1];
				manager.addHotKey(&g_wins[win], bytes(&in[p + 2], n));
				p += 2 + n;
				g_out.push_back(1001);
			} else if (op == 2) {
				GameMessage *msg = newInstance(GameMessage)(in[p] ? GameMessage::MSG_RAW_KEY_DOWN : GameMessage::MSG_RAW_KEY_UP);
				msg->appendIntegerArgument(in[p + 1]);
				msg->appendIntegerArgument(in[p + 2]);
				p += 3;
				Int disp = (Int)x.translateGameMessage(msg);
				msg->deleteInstance();
				g_out.push_back(1002);
				g_out.push_back(disp);
			} else if (op == 3) {
				GameWindow &w = g_wins[in[p]];
				w.m_status = (in[p + 1] ? WIN_STATUS_HIDDEN : 0) | (in[p + 2] ? WIN_STATUS_ENABLED : 0);
				p += 3;
				g_out.push_back(1003);
			} else if (op == 4) {
				Int n = in[p];
				hkx::UnicodeString u;
				for (Int i = 0; i < n; ++i) u.m_s += (WideChar)in[p + 1 + i];
				p += 1 + n;
				hkx::AsciiString k = manager.searchHotKey(u);
				g_out.push_back(1004);
				g_out.push_back((Int)k.m_s.size());
				for (size_t i = 0; i < k.m_s.size(); ++i) g_out.push_back((unsigned char)k.m_s[i]);
			} else if (op == 5) {
				Int n = in[p];
				Bool r = manager.executeHotKey(bytes(&in[p + 1], n));
				p += 1 + n;
				g_out.push_back(1005);
				g_out.push_back((Int)r);
			} else {
				break;
			}
		}
		TheHotKeyManager = NULL;
	}
	int n = (int)g_out.size();
	for (int i = 0; i < n && i < cap; ++i) out[i] = g_out[i];
	return n;
}
