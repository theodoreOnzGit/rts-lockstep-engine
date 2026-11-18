/* Differential-test wrapper for the ODE integrator draft (GitHub #215, upstream commit 0a05454d), group "ww3dgeo":
** src/drafts/sweep_ww3d/ode.rs.
**
** NO CUT: WWMath/ode.cpp is compiled whole and unchanged (build.rs, `drafts_ww3dgeo`), with ode.h, vector.h (WWLib's
** DynamicVectorClass) and the release-build always.h / wwdebug.h stand-ins. Stand-ins in this wrapper: only the
** ODESystemClass the integrators drive (it is a pure virtual class), here DSystem: n state variables with the derivative
**   dydt[i] = c0[i]*s[(i+1)%n] - c1[i]*s[i]*s[i] + t*c2[i] + c3[i]*s[i]
** (s = the test state, or the current state for a NULL test_state). Every call the integrator makes is logged, so the
** comparison covers the whole call sequence and every argument, not only the result.
**
** Entry point (extern "C", tests/differential_draft_ode.rs):
**   ref_dode_run(op, dt_bits, n, coef, state, out, cap): op 0 Euler, 1 Midpoint, 2 Runge_Kutta, 3 Runge_Kutta5; coef is
**   4*n floats (c0 block, c1 block, c2 block, c3 block) as bits; state n floats as bits. `out`: the new state (n words),
**   then the log: per Compute_Derivatives [1, t bits, has_test_state, start_index, n test-state words if present], per
**   Set_State [2, start_index], per Get_State [3]. Returns the number of words written, or -1 over `cap`. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "ode.h"

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }

class DSystem : public ODESystemClass
{
public:
	int n;
	std::vector<float> c0, c1, c2, c3, s;
	std::vector<uint32_t> log;

	virtual void Get_State(StateVectorClass & set_state)
	{
		log.push_back(3);
		for (int i = 0; i < n; i++) set_state.Add(s[i]);
	}
	virtual int Set_State(const StateVectorClass & new_state, int start_index = 0)
	{
		log.push_back(2);
		log.push_back((uint32_t)start_index);
		for (int i = 0; i < n; i++) s[i] = new_state[start_index + i];
		return start_index + n;
	}
	virtual int Compute_Derivatives(float t, StateVectorClass * test_state, StateVectorClass * dydt, int start_index = 0)
	{
		log.push_back(1);
		log.push_back(w_of(t));
		log.push_back(test_state != NULL ? 1u : 0u);
		log.push_back((uint32_t)start_index);
		std::vector<float> in(n);
		for (int i = 0; i < n; i++) in[i] = test_state ? (*test_state)[start_index + i] : s[i];
		if (test_state) for (int i = 0; i < n; i++) log.push_back(w_of(in[i]));
		for (int i = 0; i < n; i++) {
			(*dydt)[start_index + i] = c0[i] * in[(i + 1) % n] - c1[i] * in[i] * in[i] + t * c2[i] + c3[i] * in[i];
		}
		return start_index + n;
	}
};

}

extern "C" int ref_dode_run(int op, uint32_t dt_bits, int n, const uint32_t *coef, const uint32_t *state, uint32_t *out, int cap)
{
	DSystem sys;
	sys.n = n;
	for (int i = 0; i < n; i++) {
		sys.c0.push_back(f_of(coef[i]));
		sys.c1.push_back(f_of(coef[n + i]));
		sys.c2.push_back(f_of(coef[2 * n + i]));
		sys.c3.push_back(f_of(coef[3 * n + i]));
		sys.s.push_back(f_of(state[i]));
	}
	float dt = f_of(dt_bits);
	switch (op) {
	case 0: IntegrationSystem::Euler_Integrate(&sys, dt); break;
	case 1: IntegrationSystem::Midpoint_Integrate(&sys, dt); break;
	case 2: IntegrationSystem::Runge_Kutta_Integrate(&sys, dt); break;
	case 3: IntegrationSystem::Runge_Kutta5_Integrate(&sys, dt); break;
	default: return -1;
	}
	int total = n + (int)sys.log.size();
	if (total > cap) return -1;
	for (int i = 0; i < n; i++) out[i] = w_of(sys.s[i]);
	for (size_t i = 0; i < sys.log.size(); i++) out[n + i] = sys.log[i];
	return total;
}
