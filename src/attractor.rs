//! Static samples of three genuine autonomous three-dimensional ODE flows.
//! Integrate once when creating GPU resources, then retain the resulting vertex
//! buffer. No integration, allocation or random walk belongs in a draw callback.
//!
//! The integrator below is original Rust; the named differential equations are
//! mathematical definitions, not third-party source or downloaded geometry.

use bytemuck::{Pod, Zeroable};

pub const ATTRACTOR_POINT_COUNT: usize = 12_000;
const NORMALIZED_RADIUS: f64 = 1.2;
type State = [f64; 3];
type Flow = fn(State) -> State;

/// Three corresponding trajectory samples, suitable for one static vertex
/// buffer with Float32x3 attributes at byte offsets 0, 12 and 24 (stride 36).
/// Correspondence is sample order, not a claim that these different ODEs share
/// the same physical time or phase-space coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct AttractorPoint {
    pub lorenz: [f32; 3],
    pub rossler: [f32; 3],
    pub thomas: [f32; 3],
}

/// Generate deterministic, centred trajectories, each isotropically fitted to
/// radius 1.2. Independent fitting preserves each attractor's true proportions:
/// the flat Rössler scroll is not stretched into a spherical cloud.
pub fn attractor_points() -> Vec<AttractorPoint> {
    let lorenz = normalize(trajectory(lorenz_flow, [0.1, 0.0, 0.0], 0.005, 8_000, 3));
    let rossler = normalize(trajectory(rossler_flow, [0.1, 0.0, 0.0], 0.01, 20_000, 4));
    // An asymmetric seed avoids the invariant x=y=z line of the cyclic flow.
    let thomas = normalize(trajectory(
        thomas_flow,
        [0.17, 0.03, -0.11],
        0.025,
        12_000,
        3,
    ));
    lorenz
        .into_iter()
        .zip(rossler)
        .zip(thomas)
        .map(|((lorenz, rossler), thomas)| AttractorPoint {
            lorenz,
            rossler,
            thomas,
        })
        .collect()
}

fn lorenz_flow([x, y, z]: State) -> State {
    // Classical two-wing Lorenz flow: sigma=10, rho=28, beta=8/3.
    [10.0 * (y - x), x * (28.0 - z) - y, x * y - (8.0 / 3.0) * z]
}

fn rossler_flow([x, y, z]: State) -> State {
    // Rössler's single folded scroll, a=b=0.2, c=5.7.
    [-y - z, x + 0.2 * y, 0.2 + z * (x - 5.7)]
}

fn thomas_flow([x, y, z]: State) -> State {
    // Thomas' cyclic sine flow, in its dissipative chaotic regime.
    const DAMPING: f64 = 0.18;
    [
        y.sin() - DAMPING * x,
        z.sin() - DAMPING * y,
        x.sin() - DAMPING * z,
    ]
}

fn add_scaled(state: State, derivative: State, step: f64) -> State {
    std::array::from_fn(|axis| state[axis] + derivative[axis] * step)
}

fn rk4(flow: Flow, state: State, dt: f64) -> State {
    let k1 = flow(state);
    let k2 = flow(add_scaled(state, k1, dt * 0.5));
    let k3 = flow(add_scaled(state, k2, dt * 0.5));
    let k4 = flow(add_scaled(state, k3, dt));
    std::array::from_fn(|axis| {
        state[axis] + (k1[axis] + 2.0 * k2[axis] + 2.0 * k3[axis] + k4[axis]) * (dt / 6.0)
    })
}

fn trajectory(flow: Flow, initial: State, dt: f64, burn_in: usize, stride: usize) -> Vec<State> {
    let mut state = initial;
    for _ in 0..burn_in {
        state = rk4(flow, state, dt);
    }
    let mut points = Vec::with_capacity(ATTRACTOR_POINT_COUNT);
    for _ in 0..ATTRACTOR_POINT_COUNT {
        for _ in 0..stride {
            state = rk4(flow, state, dt);
        }
        points.push(state);
    }
    points
}

fn normalize(points: Vec<State>) -> Vec<[f32; 3]> {
    let mut centre = [0.0; 3];
    for point in &points {
        for axis in 0..3 {
            centre[axis] += point[axis] / points.len() as f64;
        }
    }
    let radius = points.iter().fold(0.0_f64, |radius, point| {
        let squared: f64 = (0..3)
            .map(|axis| (point[axis] - centre[axis]).powi(2))
            .sum();
        radius.max(squared.sqrt())
    });
    // A single uniform scale retains geometry, with a zero-extent guard rather
    // than per-axis division or clipping isolated points onto an artificial shell.
    let scale = NORMALIZED_RADIUS / radius.max(f64::EPSILON);
    points
        .into_iter()
        .map(|point| std::array::from_fn(|axis| ((point[axis] - centre[axis]) * scale) as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn systems(point: &AttractorPoint) -> [[f32; 3]; 3] {
        [point.lorenz, point.rossler, point.thomas]
    }

    #[test]
    fn all_12000_corresponding_samples_are_finite_and_gpu_packed() {
        let points = attractor_points();
        assert_eq!(points.len(), ATTRACTOR_POINT_COUNT);
        assert_eq!(std::mem::size_of::<AttractorPoint>(), 36);
        assert!(
            points
                .iter()
                .flat_map(systems)
                .flatten()
                .all(f32::is_finite)
        );
        assert_eq!(
            points,
            attractor_points(),
            "static trajectories must be deterministic"
        );
    }

    #[test]
    fn each_system_is_centred_and_isotropically_bounded() {
        let points = attractor_points();
        for system in 0..3 {
            let mut centre = [0.0_f64; 3];
            let mut max_radius = 0.0_f64;
            for point in &points {
                let value = systems(point)[system];
                for axis in 0..3 {
                    centre[axis] += value[axis] as f64 / points.len() as f64;
                }
                let radius = value
                    .iter()
                    .map(|v| (*v as f64).powi(2))
                    .sum::<f64>()
                    .sqrt();
                max_radius = max_radius.max(radius);
            }
            assert!(
                centre.iter().all(|v| v.abs() < 0.000_002),
                "system {system}: {centre:?}"
            );
            assert!(
                (1.199_99..=1.200_01).contains(&max_radius),
                "system {system}: {max_radius}"
            );
        }
    }

    // Rotation-invariant covariance signatures distinguish shape/volume, not
    // merely point ordering or a rotated copy of the same synthetic curve.
    fn geometry_signature(points: &[AttractorPoint], system: usize) -> [f64; 3] {
        let mut covariance = [[0.0_f64; 3]; 3];
        for point in points {
            let p = systems(point)[system];
            for row in 0..3 {
                for column in 0..3 {
                    covariance[row][column] +=
                        p[row] as f64 * p[column] as f64 / points.len() as f64;
                }
            }
        }
        let c = covariance;
        let trace = c[0][0] + c[1][1] + c[2][2];
        let anisotropy = c.iter().flatten().map(|v| v * v).sum::<f64>() / trace.powi(2);
        let determinant = c[0][0] * (c[1][1] * c[2][2] - c[1][2] * c[2][1])
            - c[0][1] * (c[1][0] * c[2][2] - c[1][2] * c[2][0])
            + c[0][2] * (c[1][0] * c[2][1] - c[1][1] * c[2][0]);
        [trace, anisotropy, determinant / trace.powi(3)]
    }

    #[test]
    fn the_three_ode_flows_produce_genuinely_different_geometry() {
        let points = attractor_points();
        let signatures: [_; 3] = std::array::from_fn(|system| geometry_signature(&points, system));
        for a in 0..3 {
            for b in a + 1..3 {
                let difference: f64 = signatures[a]
                    .iter()
                    .zip(signatures[b])
                    .map(|(a, b)| (a - b).abs())
                    .sum();
                assert!(
                    difference > 0.025,
                    "systems {a}/{b} have indistinct geometry: {:?} / {:?}",
                    signatures[a],
                    signatures[b]
                );
            }
        }
    }
}
