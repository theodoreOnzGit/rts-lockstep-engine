//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/ode.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/ode.h
//!
//! `IntegrationSystem`: Euler, midpoint, 4th-order Runge-Kutta and the 5th-order Cash-Karp-style Runge-Kutta
//! integrators over an `ODESystemClass`. Upstream keeps the state and work vectors as file-scope `static
//! StateVectorClass`es (`Y0`, `Y1`, `_WorkVector0..7`); here they are locals of each call, which is the same arithmetic
//! (every vector is `Resize`d before use; `Y0` is `Reset` then filled by `Get_State`).
//! `ODESystemClass` (a pure virtual class) is the [`OdeSystem`] trait.

/// `StateVectorClass` (ode.h:59): a `DynamicVectorClass<float>`.
pub type StateVector = Vec<f32>;

/// `ODESystemClass` (ode.h:72). `Compute_Derivatives(t, test_state, dydt, start_index)` is called here with the
/// default `start_index = 0`; `test_state` is `None` for upstream's `NULL` ("the current state").
pub trait OdeSystem {
    /// `Get_State(StateVectorClass & set_state)`: appends the state to `set_state`.
    fn get_state(&mut self, set_state: &mut StateVector);
    /// `Set_State(const StateVectorClass & new_state, int start_index = 0)`.
    fn set_state(&mut self, new_state: &StateVector, start_index: i32) -> i32;
    /// `Compute_Derivatives(float t, StateVectorClass * test_state, StateVectorClass * dydt, int start_index = 0)`.
    fn compute_derivatives(&mut self, t: f32, test_state: Option<&StateVector>, dydt: &mut StateVector, start_index: i32) -> i32;
}

/// `IntegrationSystem::Euler_Integrate(sys, dt)` (ode.cpp:72): "evaluate the derivative, multiply by the time-step and
/// add to the current state vector".
pub fn euler_integrate<S: OdeSystem>(sys: &mut S, dt: f32) {
    let mut y0: StateVector = Vec::new(); // Y0.Reset()
    sys.get_state(&mut y0);
    let n = y0.len();
    let mut dydt: StateVector = vec![0.0; n];
    sys.compute_derivatives(0.0, None, &mut dydt, 0);

    let mut y1: StateVector = vec![0.0; n];
    for i in 0..n {
        y1[i] = y0[i] + dydt[i] * dt;
    }
    sys.set_state(&y1, 0);
}

/// `IntegrationSystem::Midpoint_Integrate(sys, dt)` (ode.cpp:119).
pub fn midpoint_integrate<S: OdeSystem>(sys: &mut S, dt: f32) {
    let mut y0: StateVector = Vec::new();
    sys.get_state(&mut y0);
    let n = y0.len();
    let mut dydt: StateVector = vec![0.0; n];
    let mut ymid: StateVector = vec![0.0; n];

    // MidPoint method, first evaluate the derivatives of the state vector just like the Euler method.
    sys.compute_derivatives(0.0, None, &mut dydt, 0);

    // Compute the midpoint between the Euler solution and the input values.
    for i in 0..n {
        ymid[i] = y0[i] + dt * dydt[i] / 2.0;
    }

    // Re-compute derivatives at this point.
    sys.compute_derivatives(dt / 2.0, Some(&ymid), &mut dydt, 0);

    // Use these derivatives to compute the solution.
    let mut y1: StateVector = vec![0.0; n];
    for i in 0..n {
        y1[i] = y0[i] + dt * dydt[i];
    }
    sys.set_state(&y1, 0);
}

/// `IntegrationSystem::Runge_Kutta_Integrate(sys, dt)` (ode.cpp:183): classical fourth-order Runge-Kutta.
pub fn runge_kutta_integrate<S: OdeSystem>(sys: &mut S, dt: f32) {
    let dt2 = dt / 2.0;
    let dt6 = dt / 6.0;

    let mut y0: StateVector = Vec::new();
    sys.get_state(&mut y0);
    let n = y0.len();
    let mut y1: StateVector = vec![0.0; n];
    let mut dydt: StateVector = vec![0.0; n];
    let mut dym: StateVector = vec![0.0; n];
    let mut dyt: StateVector = vec![0.0; n];
    let mut yt: StateVector = vec![0.0; n];

    // First Step
    sys.compute_derivatives(0.0, None, &mut dydt, 0);
    for i in 0..n {
        yt[i] = y0[i] + dt2 * dydt[i];
    }

    // Second Step
    sys.compute_derivatives(dt2, Some(&yt), &mut dyt, 0);
    for i in 0..n {
        yt[i] = y0[i] + dt2 * dyt[i];
    }

    // Third Step
    sys.compute_derivatives(dt2, Some(&yt), &mut dym, 0);
    for i in 0..n {
        yt[i] = y0[i] + dt * dym[i];
        dym[i] += dyt[i];
    }

    // Fourth Step
    sys.compute_derivatives(dt, Some(&yt), &mut dyt, 0);
    for i in 0..n {
        y1[i] = y0[i] + dt6 * (dydt[i] + dyt[i] + 2.0 * dym[i]);
    }

    sys.set_state(&y1, 0);
}

/// `IntegrationSystem::Runge_Kutta5_Integrate(odesys, dt)` (ode.cpp:260): fifth-order Runge-Kutta with the error
/// estimate `yerr` computed and, as upstream, never used.
pub fn runge_kutta5_integrate<S: OdeSystem>(odesys: &mut S, dt: f32) {
    let a2: f32 = 0.2;
    let a3: f32 = 0.3;
    let a4: f32 = 0.6;
    let a5: f32 = 1.0;
    let a6: f32 = 0.875;
    let b21: f32 = 0.2;
    let b31: f32 = 3.0 / 40.0;
    let b32: f32 = 9.0 / 40.0;
    let b41: f32 = 0.3;
    let b42: f32 = -0.9;
    let b43: f32 = 1.2;
    let b51: f32 = -11.0 / 54.0;
    let b52: f32 = 2.5;
    let b53: f32 = -70.0 / 27.0;
    let b54: f32 = 35.0 / 27.0;
    let b61: f32 = 1631.0 / 55296.0;
    let b62: f32 = 175.0 / 512.0;
    let b63: f32 = 575.0 / 13824.0;
    let b64: f32 = 44275.0 / 110592.0;
    let b65: f32 = 253.0 / 4096.0;
    let c1: f32 = 37.0 / 378.0;
    let c3: f32 = 250.0 / 621.0;
    let c4: f32 = 125.0 / 594.0;
    let c6: f32 = 512.0 / 1771.0;
    let dc5: f32 = -277.0 / 14336.0;
    let dc1: f32 = c1 - 2825.0 / 27648.0;
    let dc3: f32 = c3 - 18575.0 / 48384.0;
    let dc4: f32 = c4 - 13525.0 / 55296.0;
    let dc6: f32 = c6 - 0.25;

    let mut y0: StateVector = Vec::new();
    odesys.get_state(&mut y0);
    let veclen = y0.len();
    let mut y1: StateVector = vec![0.0; veclen];

    let mut dydt: StateVector = vec![0.0; veclen];
    let mut ak2: StateVector = vec![0.0; veclen];
    let mut ak3: StateVector = vec![0.0; veclen];
    let mut ak4: StateVector = vec![0.0; veclen];
    let mut ak5: StateVector = vec![0.0; veclen];
    let mut ak6: StateVector = vec![0.0; veclen];
    let mut ytmp: StateVector = vec![0.0; veclen];
    let mut yerr: StateVector = vec![0.0; veclen];

    // First step
    odesys.compute_derivatives(0.0, None, &mut dydt, 0);
    for i in 0..veclen {
        ytmp[i] = y0[i] + b21 * dt * dydt[i];
    }

    // Second step
    odesys.compute_derivatives(a2 * dt, Some(&ytmp), &mut ak2, 0);
    for i in 0..veclen {
        ytmp[i] = y0[i] + dt * (b31 * dydt[i] + b32 * ak2[i]);
    }

    // Third step
    odesys.compute_derivatives(a3 * dt, Some(&ytmp), &mut ak3, 0);
    for i in 0..veclen {
        ytmp[i] = y0[i] + dt * (b41 * dydt[i] + b42 * ak2[i] + b43 * ak3[i]);
    }

    // Fourth step
    odesys.compute_derivatives(a4 * dt, Some(&ytmp), &mut ak4, 0);
    for i in 0..veclen {
        ytmp[i] = y0[i] + dt * (b51 * dydt[i] + b52 * ak2[i] + b53 * ak3[i] + b54 * ak4[i]);
    }

    // Fifth step
    odesys.compute_derivatives(a5 * dt, Some(&ytmp), &mut ak5, 0);
    for i in 0..veclen {
        ytmp[i] = y0[i] + dt * (b61 * dydt[i] + b62 * ak2[i] + b63 * ak3[i] + b64 * ak4[i] + b65 * ak5[i]);
    }

    // Sixth step
    odesys.compute_derivatives(a6 * dt, Some(&ytmp), &mut ak6, 0);
    for i in 0..veclen {
        y1[i] = y0[i] + dt * (c1 * dydt[i] + c3 * ak3[i] + c4 * ak4[i] + c6 * ak6[i]);
    }

    // Error approximation (upstream: "maybe I should use this someday?")
    for i in 0..veclen {
        yerr[i] = dt * (dc1 * dydt[i] + dc3 * ak3[i] + dc4 * ak4[i] + dc5 * ak5[i] + dc6 * ak6[i]);
    }
    let _ = &yerr;

    odesys.set_state(&y1, 0);
}
