// Northern Lights: layered luminous curtains, seen across a deep polar sky.
// Continuous travel supplies advection; measured FFT bands supply relief and
// luminosity. No beat resets, randomly respawned rays or screen-wide flashes.
struct AuroraLook {
    fold: vec4<f32>, // horizontal frequency, fold depth, arch, tilt
    veil: vec4<f32>, // height, width, vertical fan, opposing curtain amount
    motion: vec4<f32>, // bounded drift, filament spacing, distant volume, twist
    foot: vec4<f32>, // lower emission RGB, base height
    tip: vec4<f32>, // upper emission RGB, ray contrast
};

fn aurora_recipe(id: f32) -> AuroraLook {
    switch u32(clamp(round(id), 0.0, 9.0)) {
        case 1u: { // Boreal Ribbons: wide, gently braided bands.
            return AuroraLook(vec4<f32>(3.8, 0.18, 0.10, -0.14), vec4<f32>(0.56, 1.04, 0.14, 0.22),
                vec4<f32>(0.37, 19.0, 0.15, 0.04), vec4<f32>(0.08, 0.90, 0.76, 0.32), vec4<f32>(0.29, 0.21, 0.90, 0.7));
        }
        case 2u: { // Violet Crown: an overhead vault of radiating veils.
            return AuroraLook(vec4<f32>(2.6, 0.15, -0.46, 0.02), vec4<f32>(0.78, 0.91, 0.82, 0.10),
                vec4<f32>(0.22, 25.0, 0.09, 0.07), vec4<f32>(0.33, 0.72, 1.0, 0.35), vec4<f32>(0.85, 0.13, 0.69, 1.0));
        }
        case 3u: { // Polar Cascade: tall, separated columns of silk.
            return AuroraLook(vec4<f32>(5.6, 0.11, 0.12, 0.08), vec4<f32>(1.12, 0.82, -0.18, 0.0),
                vec4<f32>(0.30, 13.0, 0.09, -0.04), vec4<f32>(0.12, 1.0, 0.61, 0.51), vec4<f32>(0.16, 0.40, 0.95, 1.25));
        }
        case 4u: { // Twin Horizons: opposing folds woven through each other.
            return AuroraLook(vec4<f32>(3.1, 0.22, -0.08, 0.25), vec4<f32>(0.52, 0.95, 0.28, 0.90),
                vec4<f32>(0.39, 22.0, 0.19, 0.0), vec4<f32>(0.14, 0.88, 0.75, 0.26), vec4<f32>(0.78, 0.19, 0.78, 0.80));
        }
        case 5u: { // Solar Silk: long amber-green luminous swells.
            return AuroraLook(vec4<f32>(1.8, 0.26, 0.18, -0.19), vec4<f32>(0.62, 1.10, 0.10, 0.15),
                vec4<f32>(0.29, 16.0, 0.12, 0.05), vec4<f32>(0.86, 0.72, 0.16, 0.22), vec4<f32>(0.10, 0.70, 0.57, 0.65));
        }
        case 6u: { // Glacial Wings: two curved wings with a lifted central return.
            return AuroraLook(vec4<f32>(3.4, 0.16, 0.12, 0.0), vec4<f32>(0.86, 0.94, -0.42, 0.54),
                vec4<f32>(0.25, 20.0, 0.12, -0.03), vec4<f32>(0.10, 0.76, 0.98, 0.02), vec4<f32>(0.46, 0.26, 0.91, 0.95));
        }
        case 7u: { // Amber Storm: dense copper folds with green upper light.
            return AuroraLook(vec4<f32>(5.4, 0.22, -0.12, 0.15), vec4<f32>(0.74, 0.88, 0.36, 0.62),
                vec4<f32>(0.44, 29.0, 0.13, 0.12), vec4<f32>(0.98, 0.38, 0.16, 0.31), vec4<f32>(0.18, 0.83, 0.47, 1.1));
        }
        case 8u: { // Midnight Spiral: oblique curtains curled through depth.
            return AuroraLook(vec4<f32>(3.7, 0.21, -0.25, -0.20), vec4<f32>(0.83, 0.88, 0.58, 0.38),
                vec4<f32>(0.32, 24.0, 0.15, 0.38), vec4<f32>(0.16, 0.90, 0.77, 0.33), vec4<f32>(0.60, 0.16, 0.92, 0.95));
        }
        case 9u: { // Prismatic Dawn: broad pink-edged turquoise curtains.
            return AuroraLook(vec4<f32>(2.4, 0.19, 0.04, 0.11), vec4<f32>(0.90, 1.0, -0.12, 0.34),
                vec4<f32>(0.27, 18.0, 0.14, -0.10), vec4<f32>(0.20, 0.92, 0.82, 0.36), vec4<f32>(0.98, 0.31, 0.58, 0.80));
        }
        default: { // Emerald Veil: the classic green folded polar curtain.
            return AuroraLook(vec4<f32>(2.8, 0.19, -0.18, -0.05), vec4<f32>(0.86, 0.98, 0.22, 0.05),
                vec4<f32>(0.28, 21.0, 0.12, 0.02), vec4<f32>(0.14, 1.0, 0.47, 0.35), vec4<f32>(0.34, 0.22, 0.88, 0.85));
        }
    }
}

fn aurora_look() -> AuroraLook {
    let state = u.looks[9];
    let a = aurora_recipe(state.x);
    let b = aurora_recipe(state.y);
    let blend = clamp(state.z, 0.0, 1.0);
    return AuroraLook(mix(a.fold, b.fold, blend), mix(a.veil, b.veil, blend),
        mix(a.motion, b.motion, blend), mix(a.foot, b.foot, blend), mix(a.tip, b.tip, blend));
}

fn aurora_noise(x: f32, seed: f32) -> f32 {
    let cell = floor(x);
    let f = fract(x);
    let ease = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    return mix(hash(vec3<f32>(cell, seed, 1.0)),
        hash(vec3<f32>(cell + 1.0, seed, 1.0)), ease);
}

// The density warp needs its derivative so finer, irregularly spaced fibres
// still fade correctly when their projection becomes smaller than a pixel.
fn aurora_noise_slope(x: f32, seed: f32) -> vec2<f32> {
    let cell = floor(x);
    let f = fract(x);
    let ease = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = hash(vec3<f32>(cell, seed, 1.0));
    let b = hash(vec3<f32>(cell + 1.0, seed, 1.0));
    return vec2<f32>(mix(a, b, ease), (b - a) * 30.0 * f * f * (1.0 - f) * (1.0 - f));
}

// RenderMotion already eases measured bands and FFT rows. Average overlapping
// evidence before this soft shoulder: adding it and using a steep exponential
// left ordinary loud music almost fully saturated, hiding subsequent accents.
// Zero stays exactly zero; valid mapped inputs span 0..1.5 without a hard clip.
fn aurora_response(measured: f32) -> f32 {
    let value = max(measured, 0.0);
    return value / (0.65 + value);
}

fn aurora_audio() -> vec3<f32> {
    let low_fft = (spectrum_at(0.01) + spectrum_at(0.05)
        + spectrum_at(0.09) + spectrum_at(0.135)) * 0.25;
    let middle_fft = (spectrum_at(0.19) + spectrum_at(0.28)
        + spectrum_at(0.38) + spectrum_at(0.48)) * 0.25;
    let high_fft = (spectrum_at(0.55) + spectrum_at(0.68)
        + spectrum_at(0.82) + spectrum_at(0.97)) * 0.25;
    let live = select(0.0, 1.0, u.raster.z > 0.0);
    return vec3<f32>(aurora_response(bass() * 0.60 + low_fft * 0.40),
        aurora_response(mids() * 0.55 + middle_fft * 0.45),
        aurora_response(air() * 0.45 + high_fft * 0.55)) * live;
}

// Fixed angular geology gives the eye a quiet horizon below the moving sky.
// Linear interpolation preserves craggy peaks instead of rounded noise hills.
fn aurora_rock(x: f32, seed: f32) -> f32 {
    let cell = floor(x);
    return mix(hash(vec3<f32>(cell, seed, 9.0)),
        hash(vec3<f32>(cell + 1.0, seed, 9.0)), fract(x));
}

// Folded luminous edges in three-dimensional space. A near curtain hangs
// overhead while a more distant one passes behind the ridge. Their filaments
// climb toward a common magnetic zenith rather than rising from the ground.
struct AuroraEdgeFrame {
    point: vec3<f32>,
    slope: vec3<f32>,
};

fn aurora_edge_frame(s: f32, lane: f32, phase: f32, look: AuroraLook, low: f32, middle: f32) -> AuroraEdgeFrame {
    // Several alternating returns are visible together. Keeping depth within
    // a narrow distant range avoids a single near panel growing into a huge
    // triangular fan, while horizontal reversals make true draped S folds.
    let sweep = s * (3.1 + look.fold.x * 1.4) + phase * 0.72 + lane * 2.35;
    let fold = 0.90 + look.fold.y * 4.6 + low * 0.90;
    let sweep_sin = sin(sweep);
    let sweep_cos = cos(sweep);
    let wave_x = s * 3.7 - phase * 0.31 + lane;
    let wave_bass = s * 2.1 + phase * 0.47 + lane;
    let wave_z = s * 4.1 + phase * 0.25;
    let x = s * (7.6 + look.veil.y * 1.6) + sweep_sin * fold
        + sin(wave_x) * (0.35 + middle * 0.42)
        + sin(wave_bass) * low * 0.65;
    let z = 11.5 + lane * (3.8 + look.veil.w * 1.1)
        + sweep_cos * (1.1 + fold * 0.50)
        + sin(wave_z) * 0.52;
    // Separate near/remote heights keep a hanging ribbon above a lower one
    // whose folds continue behind the mountain silhouette. Recipe arch and
    // fold count change the whole composition, not just its colour.
    let altitude = 1.25 - lane * 2.85
        + sin(phase * 0.43 + lane * 2.2) * 0.30;
    // An independent travelling height wave makes visibly suspended crests
    // and dips. Correlating height with the depth cosine flattened the whole
    // projected hem into a nearly horizontal wire.
    let wave_drape = s * 5.3 - phase * 0.38 + lane;
    let drape = s * (3.9 + look.fold.x * 0.32) + phase * 0.61 + lane * 1.23
        + sin(wave_drape) * middle * 0.36;
    let bass_drape = drape + 0.85 + lane * 0.4;
    let wave_lift = s * 2.5 + phase * 0.37 + lane;
    let wave_return = s * 7.1 - phase * 0.43 + lane;
    let wave_pleat = s * 11.3 - phase * 0.81 + lane * 1.3;
    // Bass opens and lifts whole folds; mids curl their smaller returns.
    // All audio changes bounded distances, never the accumulated clock.
    let y = altitude + sin(drape) * (1.40 + middle * 0.55)
        + sin(bass_drape) * low * 0.62
        + sin(wave_lift) * 0.32
        + sin(wave_return) * (0.23 + middle * 0.30)
        - look.fold.z * (1.0 - s * s) * 4.2
        + sin(wave_pleat) * (0.12 + middle * 0.24)
        + look.fold.w * s * 1.6;
    // Exact differentiation replaces two complete edge evaluations at s±.002
    // for each hit. Geometry is unchanged; the derivative serves filtering and
    // grazing opacity only. Panel endpoints use the position-only function.
    let sweep_slope = 3.1 + look.fold.x * 1.4;
    let drape_slope = 3.9 + look.fold.x * 0.32
        + cos(wave_drape) * 5.3 * middle * 0.36;
    let dx = 7.6 + look.veil.y * 1.6 + sweep_cos * sweep_slope * fold
        + cos(wave_x) * 3.7 * (0.35 + middle * 0.42)
        + cos(wave_bass) * 2.1 * low * 0.65;
    let dy = cos(drape) * drape_slope * (1.40 + middle * 0.55)
        + cos(bass_drape) * drape_slope * low * 0.62
        + cos(wave_lift) * 2.5 * 0.32
        + cos(wave_return) * 7.1 * (0.23 + middle * 0.30)
        + look.fold.z * s * 8.4
        + cos(wave_pleat) * 11.3 * (0.12 + middle * 0.24)
        + look.fold.w * 1.6;
    let dz = -sweep_sin * sweep_slope * (1.1 + fold * 0.50)
        + cos(wave_z) * 4.1 * 0.52;
    return AuroraEdgeFrame(vec3<f32>(x, y, z), vec3<f32>(dx, dy, dz));
}

fn aurora_edge(s: f32, lane: f32, phase: f32, look: AuroraLook, low: f32, middle: f32) -> vec3<f32> {
    // Keep the 48 panel endpoint evaluations explicitly position-only; their
    // cost must not depend on eliminating unused fields from the hit frame.
    // These positions match aurora_edge_frame; only active hits need a slope.
    let sweep = s * (3.1 + look.fold.x * 1.4) + phase * 0.72 + lane * 2.35;
    let fold = 0.90 + look.fold.y * 4.6 + low * 0.90;
    let x = s * (7.6 + look.veil.y * 1.6) + sin(sweep) * fold
        + sin(s * 3.7 - phase * 0.31 + lane) * (0.35 + middle * 0.42)
        + sin(s * 2.1 + phase * 0.47 + lane) * low * 0.65;
    let z = 11.5 + lane * (3.8 + look.veil.w * 1.1)
        + cos(sweep) * (1.1 + fold * 0.50)
        + sin(s * 4.1 + phase * 0.25) * 0.52;
    let altitude = 1.25 - lane * 2.85
        + sin(phase * 0.43 + lane * 2.2) * 0.30;
    let drape = s * (3.9 + look.fold.x * 0.32) + phase * 0.61 + lane * 1.23
        + sin(s * 5.3 - phase * 0.38 + lane) * middle * 0.36;
    let y = altitude + sin(drape) * (1.40 + middle * 0.55)
        + sin(drape + 0.85 + lane * 0.4) * low * 0.62
        + sin(s * 2.5 + phase * 0.37 + lane) * 0.32
        + sin(s * 7.1 - phase * 0.43 + lane) * (0.23 + middle * 0.30)
        - look.fold.z * (1.0 - s * s) * 4.2
        + sin(s * 11.3 - phase * 0.81 + lane * 1.3) * (0.12 + middle * 0.24)
        + look.fold.w * s * 1.6;
    return vec3<f32>(x, y, z);
}

fn aurora(uv: vec2<f32>) -> World {
    let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
    let terrain_x = uv.x / max(aspect, 0.8);
    let distant_ridge = 0.78 - aurora_rock(terrain_x * 3.4 + 0.7, 11.8) * 0.42
        - aurora_rock(terrain_x * 13.2, 37.0) * 0.055;
    let near_ridge = 0.93 - aurora_rock(terrain_x * 4.2 - 1.3, 62.0) * 0.40
        - aurora_rock(terrain_x * 19.0, 83.0) * 0.036;
    let edge_width = max(pixel() * 1.4, 0.0006);
    let distant_cover = smoothstep(distant_ridge - edge_width, distant_ridge + edge_width, uv.y);
    let near_cover = smoothstep(near_ridge - edge_width, near_ridge + edge_width, uv.y);
    let terrain_cover = max(distant_cover, near_cover);
    let mountain = mix(vec3<f32>(0.0018, 0.0032, 0.0060),
        vec3<f32>(0.00045, 0.0007, 0.0014), near_cover);
    // Opaque terrain entirely hides the sky and both curtains. Its exact mask
    // is unchanged; partially covered ridge pixels still run full shading.
    if terrain_cover == 1.0 {
        return World(mountain, vec3<f32>(0.0));
    }
    let look = aurora_look();
    let time = u.a.z;
    var p = vec2<f32>(uv.x / max(aspect, 0.8), uv.y);
    p = rotate(p, look.motion.w * (0.42 + 0.16 * sin(time * 0.071)));
    let live = select(0.0, 1.0, u.raster.z > 0.0);
    let audio = aurora_audio();
    let low = audio.x;
    let middle = audio.y;
    let high = audio.z;
    let detail = detail_level();
    let origin = vec3<f32>(sin(time * 0.047) * 0.72, 0.0,
        sin(time * 0.061) * 1.28);
    let ray = vec3<f32>(p.x * 1.34, 0.46 - p.y, 1.50);
    // Every coefficient of the unbounded clock is fixed. Recipe morphs only
    // deform bounded geometry, so changing a look never relocates its phase.
    let phase = time * 0.13 + sin(time * 0.047) * 0.36;
    var emission = vec3<f32>(0.0);
    var haze = vec3<f32>(0.0);
    // Intersect two actual folded sheets, not 28 fog samples. Their separated
    // lower edges and visible returns survive palette changes and bass gain.
    // Each sheet has its own perspective fan and far-to-near luminous travel.
    for (var curtain = 0u; curtain < 2u; curtain += 1u) {
        let lane = f32(curtain);
        let up = vec3<f32>(0.065 * sin(phase * 0.41 + lane * 2.0)
            + look.fold.w * 0.14, 1.0, 0.08 + look.veil.z * 0.12);
        var a = aurora_edge(-1.0, lane, phase, look, low, middle);
        for (var segment = 0u; segment < 24u; segment += 1u) {
            let end = -1.0 + f32(segment + 1u) / 12.0;
            let b = aurora_edge(end, lane, phase, look, low, middle);
            let edge = b - a;
            let normal = cross(edge, up);
            let denominator = dot(ray, normal);
            if abs(denominator) > 0.0001 {
                let travel = dot(a - origin, normal) / denominator;
                let hit = origin + ray * travel - a;
                let across_axis = cross(up, normal);
                let rise_axis = cross(normal, edge);
                let along = dot(hit, across_axis) / max(dot(edge, across_axis), 0.0001);
                let panel_rise = dot(hit, rise_axis) / max(dot(up, rise_axis), 0.0001);
                // Short, soft lower fringes preserve the suspended edge. The
                // farther sheet itself descends behind the terrain mask.
                let lower_reach = 1.60 + lane * 2.2;
                if travel > 0.4 && along >= 0.0 && along < 1.0 && panel_rise > -lower_reach - 0.5 {
                    let s = end - 1.0 / 12.0 + along / 12.0;
                    // Keep the inexpensive panel intersection, then restore
                    // the actual smooth curve's height at this hit. Its
                    // luminous hem no longer follows 24 straight panel chords.
                    let curve = aurora_edge_frame(s, lane, phase, look, low, middle);
                    let curve_error = curve.point - (a + edge * along);
                    let position = clamp(s * 0.5 + 0.5, 0.0, 1.0);
                    // Stay inside each mapping's exact FFT row boundaries.
                    // This is spectrum_at's interpolation with its derivative,
                    // so the small local pleat can also be footprint-filtered.
                    let mid_x = 8.0 + position * 15.0;
                    let mid_row = u32(floor(mid_x));
                    let mid_next = min(mid_row + 1u, 23u);
                    let mid_a = u.spectrum[mid_row / 4u][mid_row % 4u];
                    let mid_b = u.spectrum[mid_next / 4u][mid_next % 4u];
                    let mid_f = fract(mid_x);
                    let mid_measured = mix(mid_a, mid_b, mid_f * mid_f * (3.0 - 2.0 * mid_f));
                    let local_mid = aurora_response(mid_measured) * live;
                    let mid_slope = (mid_b - mid_a) * 6.0 * mid_f * (1.0 - mid_f) * 7.5
                        * 0.65 / pow(0.65 + mid_measured, 2.0) * live;
                    let local_air = aurora_response(spectrum_at((24.0 + position * 23.0) / 47.0)) * live;
                    let pleat_phase = s * 7.5 + phase * 0.55 + lane;
                    let rise = panel_rise
                        - dot(curve_error, rise_axis) / max(dot(up, rise_axis), 0.0001)
                        - sin(pleat_phase) * local_mid * 0.45;
                    // Advect the whole luminous fabric through gently bowed
                    // coordinates. Each curve is anchored at its hanging edge;
                    // neighbouring fibres bend together, not as straight combs
                    // or independently respawned sparks. Neighbouring bundles
                    // vary their bow, so they need not form identical S wires.
                    let bow_base = 0.025 + middle * 0.020;
                    let bow_phase = s * 7.7 + phase * 0.21 + lane;
                    let bow = bow_base * (0.85 + 0.15 * sin(bow_phase));
                    let curl_shift = s * 4.7 - phase * 0.19 + lane;
                    let curl_a = s * 3.4 + phase * 0.50 + lane + 0.22 * sin(curl_shift);
                    let curl_b = s * 5.8 - phase * 0.31 + lane * 1.7;
                    let fine_bow = 0.0018 + middle * 0.0004;
                    let curl_c = s * 61.0 + 0.80 * sin(s * 7.3 + phase * 0.33) + lane * 2.3;
                    let bend_a = rise * 0.65 + curl_a;
                    let bend_b = rise * 0.24 + curl_b;
                    let bend_c = rise * 0.91 + curl_c;
                    let bend_a_sin = sin(bend_a) - sin(curl_a);
                    let bend_a_cos = cos(bend_a);
                    let bend_b_cos = cos(bend_b);
                    let bend_c_cos = cos(bend_c);
                    let flow_s = s + bow * bend_a_sin
                        + 0.018 * (sin(bend_b) - sin(curl_b))
                        + fine_bow * (sin(bend_c) - sin(curl_c));
                    let broad = aurora_noise(flow_s * 5.5 - time * 0.035 + lane * 7.0, 17.4);
                    let bundle = aurora_noise(flow_s * 15.5 + time * 0.025 + lane * 9.0, 42.1);
                    let height = (2.2 + look.veil.x * 2.5)
                        * (0.70 + broad * 0.42 + low * 0.15 + local_mid * 0.35);
                    let level = max(rise, 0.0) / height;
                    let strand_scale = 104.0 + look.motion.y * 1.90;
                    let density = aurora_noise_slope(flow_s * 17.0 - time * 0.027 + lane * 7.0, 39.1);
                    let strand = flow_s * strand_scale + (density.x - 0.5) * 1.8;
                    let strand_derivative = strand_scale + density.y * 17.0 * 1.8;
                    let threads = aurora_noise(strand - time * 0.17, 62.8);
                    let fine = aurora_noise(strand * 2.4 + time * 0.13, 91.3);
                    // Differentiate the ray/panel hit analytically. A grazing
                    // fold increases strand density, so filter it rather than
                    // allowing subpixel fibres to become a shimmering comb.
                    let across_ray = dot(ray, across_axis);
                    let hit_gradient = travel * vec2<f32>(1.34, -1.0)
                        * (across_axis.xy - across_ray * normal.xy / denominator)
                        / max(dot(edge, across_axis), 0.0001) / 12.0;
                    let tangent = curve.slope;
                    let curve_slope = dot(tangent - edge * 12.0, rise_axis)
                        / max(dot(up, rise_axis), 0.0001);
                    let pleat_slope = (cos(pleat_phase) * 7.5 * local_mid
                        + sin(pleat_phase) * mid_slope) * 0.45;
                    let rise_gradient = travel * vec2<f32>(1.34, -1.0)
                        * (rise_axis.xy - dot(ray, rise_axis) * normal.xy / denominator)
                        / max(dot(up, rise_axis), 0.0001)
                        - hit_gradient * (curve_slope + pleat_slope);
                    let flow_across = 1.0
                        + bow_base * 0.15 * 7.7 * cos(bow_phase)
                            * bend_a_sin
                        + bow * (3.4 + 0.22 * 4.7 * cos(curl_shift))
                            * (bend_a_cos - cos(curl_a))
                        + 0.018 * 5.8 * (bend_b_cos - cos(curl_b))
                        + fine_bow * (61.0 + 0.80 * 7.3 * cos(s * 7.3 + phase * 0.33))
                            * (bend_c_cos - cos(curl_c));
                    let flow_up = bow * 0.65 * bend_a_cos
                        + 0.018 * 0.24 * bend_b_cos
                        + fine_bow * 0.91 * bend_c_cos;
                    // Account for both across-sheet compression and the new
                    // vertical bow when filtering unresolved fine filaments.
                    let footprint = (length(hit_gradient) * abs(flow_across)
                        + length(rise_gradient) * abs(flow_up)) * pixel() * abs(strand_derivative)
                        / clamp(aspect, 0.8, 1.0);
                    let resolved = 1.0 - smoothstep(0.65, 1.80, footprint);
                    let fine_resolved = 1.0 - smoothstep(0.65, 1.80, footprint * 2.4);
                    let fine_light = mix(0.12, pow(fine, 2.1 + bundle * 2.3)
                        * (0.30 + threads * 0.80), fine_resolved);
                    let fibre = mix(0.28, 0.07 + pow(threads, 1.05 + bundle * 2.0) * 0.66, resolved);
                    // High-frequency energy lights the thin resolved ridges
                    // between broader fibres, rather than washing the sky.
                    let filament_light = fibre * 1.32 + fine_light
                        * (0.35 + detail * 0.25 + high * 3.4 + local_air * 3.2);
                    // Light is attached to the folded sheet. Dense mid bands
                    // open its pleats; high bins illuminate individual fibres.
                    let resolved_threads = mix(0.55, threads, resolved);
                    let ray_height = height * (0.42 + resolved_threads * 0.48 + bundle * 0.62);
                    let upper_body = smoothstep(-0.12, 0.20, rise)
                        * exp(-max(rise, 0.0) / max(height, 0.1) * 0.82)
                        * (1.0 - smoothstep(ray_height * 0.40, ray_height, rise));
                    let lower_body = smoothstep(-lower_reach, -0.60, rise)
                        * (1.0 - smoothstep(-0.15, 0.20, rise)) * 0.22;
                    let body = upper_body + lower_body;
                    let hem = exp(-pow((rise - 0.10) / (0.085 + broad * 0.075), 2.0));
                    let gap = 0.06 + smoothstep(0.14, 0.72, broad) * 0.94;
                    let endpoint = smoothstep(0.0, 0.18, 1.0 - abs(s));
                    // At a projected fold return, a zero-thickness plane
                    // otherwise appears abruptly and reads as a plastic edge.
                    // Use the continuous curve tangent, not the faceted panel
                    // normal, to dissolve the grazing veil smoothly into gas.
                    let smooth_normal = normalize(cross(tangent, up));
                    let incidence = abs(dot(smooth_normal, normalize(ray)));
                    let translucent = smoothstep(0.035, 0.32, incidence);
                    // Soft local light joins the fibres into luminous fabric;
                    // it belongs to this curtain's shaped, fading envelope,
                    // never to the empty sky. Keep the rim below its body.
                    let local_fill = 0.15 + broad * 0.10 + bundle * 0.08;
                    let material = (body * (local_fill + filament_light) * gap
                        + hem * (0.44 + resolved_threads * 0.08) * (0.28 + gap * 0.72))
                        * endpoint * translucent;
                    let recipe_colour = mix(look.foot.rgb, look.tip.rgb,
                        smoothstep(0.12, 0.86, level));
                    // Colour travels along each material sheet and climbs its
                    // fibres. Adjacent folds occupy different spectral phases;
                    // the whole sky never just fades between flat colours.
                    let current = 0.5 + 0.5 * sin(flow_s * 5.4 - time * 0.14
                        + level * 2.2 + lane * 1.8);
                    let upper_current = 0.5 + 0.5 * sin(flow_s * 8.1 + time * 0.091
                        - level * 1.7 + lane * 2.6);
                    let green = vec3<f32>(0.045, 0.96, 0.32);
                    let turquoise = vec3<f32>(0.035, 0.68, 0.97);
                    let violet = vec3<f32>(0.59, 0.11, 0.93);
                    let pink = vec3<f32>(0.94, 0.12, 0.51);
                    var spectral_colour = mix(green, turquoise,
                        smoothstep(0.08, 0.58, current));
                    spectral_colour = mix(spectral_colour, violet,
                        smoothstep(0.52, 0.95, current) * (0.42 + level * 0.48));
                    spectral_colour = mix(spectral_colour, pink,
                        smoothstep(0.50, 0.94, upper_current)
                            * smoothstep(0.08, 0.72, level) * 0.65);
                    let pigment = mix(recipe_colour, spectral_colour, 0.64);
                    let depth = 1.0 / (1.0 + travel * travel * 0.018);
                    let side = mix(1.0, 0.60 + look.veil.w * 0.30, lane);
                    let intensity = 0.52 + low * 0.20 + middle * 0.32 + local_mid * 0.40;
                    emission += pigment * material * depth * side * intensity;
                    haze += pigment * body * gap * endpoint * translucent * depth * 0.005;
                }
            }
            a = b;
        }
    }
    let sky = mix(vec3<f32>(0.002, 0.004, 0.013), vec3<f32>(0.008, 0.014, 0.025),
        smoothstep(-0.85, 0.7, p.y))
        + vec3<f32>(0.004, 0.008, 0.017) * exp(-pow((uv.y - 0.55) * 2.5, 2.0));
    return World(mix(sky + haze, mountain, terrain_cover),
        emission * (0.80 + u.d.y * 0.20) * (1.0 - terrain_cover));
}
