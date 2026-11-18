/* Differential-test wrapper for the keyboard state draft (#215, upstream commit 0a05454d), src/drafts/sweep_common_client/keyboard.rs
** (KeyboardState: get_first_key, get_key_status_data, get_key_state_bit, get_key_sequence_data, set_key_status_data,
** set_key_state_data, is_shift, is_ctrl, is_alt), cut verbatim by build.rs into OUT_DIR/draft_guilogic_gen:
**   kb_io.inc      GameClient/Keyboard.h, `struct KeyboardIO` to its end comment;   kb_arrays.inc  Keyboard.h, `enum { NUM_KEYS  = 256 };`
**                  to `enum { KEY_NAMES_COUNT = 256 };` (m_keys, m_keyStatus);
**   kb_state.inc   GameClient/KeyDefs.h, the KEY_STATE_* enum;
**   kb_fns1.inc    GameClient/Input/Keyboard.cpp, getFirstKey .. setKeyStateData;   kb_fns2.inc  isShift, isCtrl, isAlt.
** Stand-ins: class Keyboard holds only the members those cuts use (the real class needs the subsystem machinery, the key name
** tables and a constructor outside the cut); `m_modifiers` is declared as Keyboard.h line 137 does (UnsignedShort).
**
** Entry point (extern "C", tests/differential_draft_keyboard.rs): ref_kb_run(in, nin, out, cap) runs a list of ops, each appending
** its answer to out: 0 key data: setKeyStatusData; 1 key data: setKeyStateData; 2 key: getKeyStatusData -> [v]; 3 key bit:
** getKeyStateBit -> [v]; 4 key: getKeySequenceData -> [v]; 5 v: m_modifiers = v; 6: [isShift, isCtrl, isAlt]; 7 key state sequence status:
** m_keyStatus[key] set raw; 8: getFirstKey -> [key status state sequence]; 9 key status state sequence: m_keys[0] set raw.
** Returns the number of words written (-1: cap too small). */
#include "PreRTS.h"
#include <vector>
#include <string.h>

#include "kb_io.inc"
#include "kb_state.inc"

class Keyboard
{
public:
	Keyboard() { memset(m_keys, 0, sizeof m_keys); memset(m_keyStatus, 0, sizeof m_keyStatus); m_modifiers = 0; }
	KeyboardIO *getFirstKey( void );
	UnsignedByte getKeyStatusData( UnsignedByte key );
	Bool getKeyStateBit( UnsignedByte key, Int bit );
	UnsignedInt getKeySequenceData( UnsignedByte key );
	void setKeyStatusData( UnsignedByte key, KeyboardIO::StatusType data );
	void setKeyStateData( UnsignedByte key, UnsignedByte data );
	Bool isShift();
	Bool isCtrl();
	Bool isAlt();

	UnsignedShort m_modifiers;
#include "kb_arrays.inc"
};

#include "kb_fns1.inc"
#include "kb_fns2.inc"

extern "C" int ref_kb_run(const int *in, int nin, int *out, int cap)
{
	Keyboard k;
	std::vector<int> o;
	int at = 0;
	while (at < nin)
	{
		int kind = in[at++];
		switch (kind)
		{
			case 0: k.setKeyStatusData((UnsignedByte)in[at], (KeyboardIO::StatusType)in[at + 1]); at += 2; break;
			case 1: k.setKeyStateData((UnsignedByte)in[at], (UnsignedByte)in[at + 1]); at += 2; break;
			case 2: o.push_back(k.getKeyStatusData((UnsignedByte)in[at])); at += 1; break;
			case 3: o.push_back(k.getKeyStateBit((UnsignedByte)in[at], in[at + 1])); at += 2; break;
			case 4: o.push_back((int)k.getKeySequenceData((UnsignedByte)in[at])); at += 1; break;
			case 5: k.m_modifiers = (UnsignedShort)in[at]; at += 1; break;
			case 6: o.push_back(k.isShift()); o.push_back(k.isCtrl()); o.push_back(k.isAlt()); break;
			case 7:
			{
				KeyboardIO &s = k.m_keyStatus[(UnsignedByte)in[at]];
				s.state = (UnsignedShort)in[at + 1]; s.sequence = (UnsignedInt)in[at + 2]; s.status = (UnsignedByte)in[at + 3];
				at += 4;
				break;
			}
			case 8: { KeyboardIO *f = k.getFirstKey(); o.push_back(f->key); o.push_back(f->status); o.push_back(f->state); o.push_back((int)f->sequence); break; }
			default:
			{
				k.m_keys[0].key = (UnsignedByte)in[at]; k.m_keys[0].status = (UnsignedByte)in[at + 1];
				k.m_keys[0].state = (UnsignedShort)in[at + 2]; k.m_keys[0].sequence = (UnsignedInt)in[at + 3];
				at += 4;
				break;
			}
		}
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); ++i) out[i] = o[i];
	return (int)o.size();
}
