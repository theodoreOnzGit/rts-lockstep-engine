/* Differential-test wrapper for the ParabolicEase draft (#215, upstream commit 0a05454d), src/drafts/logic_common/parabolic_ease.rs:
**   GeneralsMD/Code/GameEngine/Include/GameClient/ParabolicEase.h (read unchanged) and
**   GameEngine/Source/GameClient/ParabolicEase.cpp from `namespace {` to the end of the file (clamp, setEaseTimes, operator()),
**   cut verbatim by build.rs into parabolic_ease.inc.
**
** Stand-ins: PreRTS.h (shim: Real, DEBUG_CRASH as the release build's no-op) for the file's first include; `private` is
** defined as `public` while the header is read (m_in and m_out are private).
**
** Entry points (extern "C", tests/differential_draft_parabolic_ease.rs):
**   ref_ease_set(in, out, easeIn, easeOut)   setEaseTimes on a fresh ParabolicEase; writes m_in, m_out
**   ref_ease_ctor(with_args, in, out, o)    the explicit constructor with its arguments, or with its defaults (0, 0); writes m_in, m_out
**   ref_ease_eval(m_in, m_out, param)        operator()(param) on an ease whose members are set directly (as bits-exact floats) */
#include "PreRTS.h"
#define private public /* stand-in: m_in and m_out are private; the test sets them directly */
#include "GameClient/ParabolicEase.h"
#undef private
#include "parabolic_ease.inc"

extern "C" {

void ref_ease_set(float ease_in, float ease_out, float *out)
{
	ParabolicEase e;
	e.setEaseTimes(ease_in, ease_out);
	out[0] = e.m_in;
	out[1] = e.m_out;
}

void ref_ease_ctor(int with_args, float ease_in, float ease_out, float *out)
{
	ParabolicEase e = with_args ? ParabolicEase(ease_in, ease_out) : ParabolicEase();
	out[0] = e.m_in;
	out[1] = e.m_out;
}

float ref_ease_eval(float m_in, float m_out, float param)
{
	ParabolicEase e;
	e.m_in = m_in;
	e.m_out = m_out;
	return e(param);
}

}
