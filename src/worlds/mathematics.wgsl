// Curated structural recipes, not ten hue variations. Offsets interpolate
// continuously inside a single mathematical evaluation; no second fractal is
// needed for look changes. All-zero Original leaves the established form intact.
struct MathLook {
    julia: vec4<f32>,     // bounded c offset, domain pinch, orbit angularity
    domain: vec4<f32>,    // anisotropy, nonlinear twist, curl scale, modest zoom
    lace: vec4<f32>,      // box proportions, inversion radius, recursive turn
    orbit: vec4<f32>,     // trap radius, cross share, hyperbola share, trap offset
    sculpture: vec4<f32>, // closed 3/5/7 harmonics, superformula exponent
    curl: vec4<f32>,      // x/y frequencies, diagonal interference, second harmonic
};

fn math_look_recipe(look: f32) -> MathLook {
    switch u32(clamp(round(look), 0.0, 9.0)) {
        case 1u: { // Cathedral Lace: architectural symmetry without buildings.
            return MathLook(vec4<f32>(0.0, 0.0, 0.08, 0.45),
                vec4<f32>(0.16, 0.05, 0.10, 0.0), vec4<f32>(0.14, -0.14, 0.08, -0.09),
                vec4<f32>(0.05, 0.55, 0.08, -0.02), vec4<f32>(0.045, -0.025, 0.025, 1.2),
                vec4<f32>(0.30, -0.25, 0.22, -0.12));
        }
        case 2u: { // Pearl Cells: round lenses, spacious recursive cavities.
            return MathLook(vec4<f32>(0.0, 0.0, -0.06, -0.18),
                vec4<f32>(-0.08, -0.08, -0.10, 0.04), vec4<f32>(-0.17, 0.12, -0.12, 0.10),
                vec4<f32>(0.14, 0.0, 0.0, 0.025), vec4<f32>(-0.065, -0.035, 0.0, -1.1),
                vec4<f32>(-0.50, -0.35, -0.16, 0.20));
        }
        case 3u: { // Tidal Ribbons: long folded channels, interlaced strata.
            return MathLook(vec4<f32>(0.0, 0.0, 0.10, 0.18),
                vec4<f32>(-0.25, 0.14, -0.05, -0.02), vec4<f32>(0.04, -0.16, 0.16, 0.15),
                vec4<f32>(-0.04, 0.15, 0.50, 0.04), vec4<f32>(-0.08, 0.045, -0.025, -0.4),
                vec4<f32>(-0.70, 0.40, -0.22, 0.12));
        }
        case 4u: { // Dragon Veins: branching Julia/domain and hyperbolic traps.
            return MathLook(vec4<f32>(-0.009, 0.004, 0.18, 0.62),
                vec4<f32>(0.10, 0.20, 0.22, 0.02), vec4<f32>(-0.06, 0.08, 0.18, -0.12),
                vec4<f32>(-0.04, 0.15, 0.65, 0.035), vec4<f32>(0.075, -0.035, 0.045, 0.6),
                vec4<f32>(0.45, 0.15, 0.30, -0.24));
        }
        case 5u: { // Obsidian Petals: deep lobed vessels and tight orbit pearls.
            return MathLook(vec4<f32>(0.007, -0.008, -0.12, -0.32),
                vec4<f32>(-0.08, -0.10, 0.05, -0.03), vec4<f32>(0.13, 0.04, -0.08, 0.06),
                vec4<f32>(-0.12, 0.12, 0.0, -0.025), vec4<f32>(0.04, 0.12, 0.04, 1.5),
                vec4<f32>(0.15, -0.45, 0.10, 0.30));
        }
        case 6u: { // Helix Garden: spiralling limits and braided closed curves.
            return MathLook(vec4<f32>(0.004, 0.011, 0.16, 0.28),
                vec4<f32>(0.18, 0.20, 0.20, 0.0), vec4<f32>(-0.12, -0.03, 0.20, 0.23),
                vec4<f32>(0.07, 0.30, 0.20, 0.05), vec4<f32>(-0.06, 0.04, 0.08, -1.0),
                vec4<f32>(-0.20, 0.35, 0.26, 0.18));
        }
        case 7u: { // Crystal Estuary: angular waterways and faceted nested folds.
            return MathLook(vec4<f32>(-0.012, -0.003, 0.22, 0.78),
                vec4<f32>(-0.14, -0.14, 0.35, -0.015), vec4<f32>(0.20, 0.13, -0.10, -0.16),
                vec4<f32>(-0.03, 0.65, 0.20, -0.035), vec4<f32>(0.03, -0.05, 0.05, 1.8),
                vec4<f32>(0.40, -0.30, 0.25, -0.18));
        }
        case 8u: { // Woven Nebula: dense recursive intersections and interference.
            return MathLook(vec4<f32>(0.012, 0.003, 0.25, 0.40),
                vec4<f32>(0.26, 0.32, 0.50, 0.05), vec4<f32>(-0.15, 0.18, 0.12, 0.14),
                vec4<f32>(0.05, 0.15, 0.55, 0.055), vec4<f32>(-0.05, 0.10, -0.05, -0.6),
                vec4<f32>(0.60, 0.50, -0.25, 0.25));
        }
        case 9u: { // Ember Labyrinth: narrow recursive passages and etched mazes.
            return MathLook(vec4<f32>(-0.003, -0.011, -0.16, 0.68),
                vec4<f32>(-0.22, 0.16, -0.15, 0.02), vec4<f32>(0.08, -0.20, -0.18, 0.20),
                vec4<f32>(0.11, 0.60, 0.25, 0.015), vec4<f32>(0.08, 0.02, 0.07, 0.7),
                vec4<f32>(-0.40, 0.60, 0.35, -0.30));
        }
        default: {
            let zero = vec4<f32>(0.0);
            return MathLook(zero, zero, zero, zero, zero, zero);
        }
    }
}

fn math_look() -> MathLook {
    let state = u.looks[1];
    let amount = clamp(state.z, 0.0, 1.0);
    if amount >= 1.0 { return math_look_recipe(state.y); }
    let before = math_look_recipe(state.x);
    if amount <= 0.0 || state.x == state.y { return before; }
    let after = math_look_recipe(state.y);
    return MathLook(mix(before.julia, after.julia, amount), mix(before.domain, after.domain, amount),
        mix(before.lace, after.lace, amount), mix(before.orbit, after.orbit, amount),
        mix(before.sculpture, after.sculpture, amount), mix(before.curl, after.curl, amount));
}

fn math_domain(uv: vec2<f32>, look: MathLook) -> vec2<f32> {
    if all(look.domain == vec4<f32>(0.0)) && look.julia.z == 0.0 { return uv; }
    let phase = u.a.z + u.looks[1].w * 0.11;
    let aspect = 1.0 + look.domain.x;
    let radial = log(1.0 + dot(uv, uv) * 0.55);
    var p = rotate(uv * vec2<f32>(aspect, 1.0 / aspect),
        look.domain.y * radial * sin(phase * 0.13 + 1.1));
    p += sin(p.yx * (1.8 + look.domain.z) + vec2<f32>(phase * 0.14, -phase * 0.11))
        * look.julia.z / (1.0 + dot(p, p) * 0.3);
    return p * (1.0 + look.domain.w);
}

fn complex_multiply(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn fractal_bloom(uv0: vec2<f32>) -> World {
    let time = u.a.z;
    let detail = detail_level();
    let look = math_look();
    // A genuine quadratic Julia orbit, not a sinusoidal approximation. Domain
    // rotation and bounded parameter loops are continuous; percussion
    // lights the orbit traps rather than teleporting the fractal/camera.
    let zoom = 0.80 + 0.12 * sin(time * 0.16);
    let domain = math_domain(uv0, look);
    let radial_twist = 0.11 * sin(time * 0.22) * log(1.0 + dot(domain, domain) * 0.5);
    let p = rotate(domain * zoom, time * 0.025 + 0.08 * sin(time * 0.071) + radial_twist)
        + vec2<f32>(sin(time * 0.021), cos(time * 0.017)) * 0.035;
    let morph_extent = (0.014 + u.d.z * 0.010) * (0.8 + u.h.x * 0.25);
    let c = vec2<f32>(-0.74543, 0.11301)
        + vec2<f32>(sin(time * 0.31), cos(time * 0.27) * 0.78) * morph_extent
        + vec2<f32>((u.h.x - 0.35) * 0.012, (u.h.z - 0.2) * 0.008)
        + vec2<f32>(bass() * 0.00045, mids() * 0.00035) + look.julia.xy;
    var z = p;
    var derivative = vec2<f32>(1.0, 0.0);
    var trap = vec2<f32>(10.0);
    var trap_gradient = vec2<f32>(0.0);
    var orbit_signal = 0.0;
    var orbit_weight = 0.0;
    var iterations = 0.0;
    var escaped = false;
    let limit = 16 + i32(detail * 20.0);
    for (var index = 0; index < 36; index += 1) {
        if index >= limit { break; }
        let r2 = dot(z, z);
        if r2 > 256.0 { escaped = true; break; }
        let signal = spectrum_at(f32(index) / 35.0);
        let orbit_radius = max(sqrt(r2), 0.0001);
        let trap_radius = 0.62 + signal * 0.08 + look.orbit.x;
        var ring_trap = abs(sqrt(r2) - trap_radius);
        let cross_trap = min(abs(z.x + 0.12 + look.orbit.w), abs(z.y - 0.05 - look.orbit.w));
        var local_gradient = z / orbit_radius;
        if any(look.orbit != vec4<f32>(0.0)) || look.julia.w != 0.0 {
            let angular = max(look.julia.w, 0.0) * 0.55;
            let diamond = (abs(z.x) + abs(z.y)) * 0.78 - trap_radius;
            local_gradient = mix(z / orbit_radius * sign(orbit_radius - trap_radius),
                sign(z) * 0.78 * sign(diamond), angular);
            ring_trap = mix(ring_trap, abs(diamond), angular);
            let cross_gradient = select(vec2<f32>(0.0, sign(z.y - 0.05 - look.orbit.w)),
                vec2<f32>(sign(z.x + 0.12 + look.orbit.w), 0.0),
                abs(z.x + 0.12 + look.orbit.w) < abs(z.y - 0.05 - look.orbit.w));
            local_gradient = mix(local_gradient, cross_gradient, look.orbit.y);
            ring_trap = mix(ring_trap, cross_trap, look.orbit.y);
            let product = z.x * z.y - 0.11 + look.orbit.w;
            let hyperbola = abs(product) / (1.0 + orbit_radius);
            let hyper_gradient = sign(product) * z.yx / (1.0 + orbit_radius)
                - abs(product) * z / (orbit_radius * pow(1.0 + orbit_radius, 2.0));
            ring_trap = mix(ring_trap, hyperbola, look.orbit.z);
            local_gradient = mix(local_gradient, hyper_gradient, look.orbit.z);
        }
        if ring_trap < trap.x {
            if all(look.orbit == vec4<f32>(0.0)) && look.julia.w == 0.0 {
                trap_gradient = vec2<f32>(dot(z, derivative), z.y * derivative.x - z.x * derivative.y)
                    / max(sqrt(r2), 0.0001);
            } else {
                trap_gradient = vec2<f32>(dot(local_gradient, derivative),
                    local_gradient.y * derivative.x - local_gradient.x * derivative.y);
            }
        }
        trap = min(trap, vec2<f32>(ring_trap, cross_trap));
        let weight = exp(-ring_trap * 7.0) / (1.0 + f32(index) * 0.09);
        orbit_signal += signal * weight;
        orbit_weight += weight;
        derivative = complex_multiply(z, derivative) * 2.0;
        derivative *= min(1.0, 1.0e15 / max(length(derivative), 0.000001));
        z = complex_multiply(z, z) + c;
        iterations += 1.0;
    }
    let radius = max(length(z), 1.00001);
    let derivative_size = max(length(derivative), 0.000001);
    let escape_distance = select(0.0, 0.5 * log(radius) * radius / derivative_size, escaped);
    let smooth_iterations = iterations + 1.0 - log2(max(log2(radius), 0.0001));
    let resolved_distance = max(escape_distance, pixel() * 0.20);
    let boundary = exp(-sqrt(resolved_distance / (0.010 + detail * 0.009)));
    let contour_phase = log(resolved_distance + pixel() * 0.35) * 4.5;
    let contours = pow(0.5 + 0.5 * cos(contour_phase), 7.0)
        * smoothstep(pixel() * 0.6, pixel() * 3.0, escape_distance);
    let ring_ink = exp(-trap.x * (20.0 + detail * 18.0));
    let cross_ink = exp(-trap.y * 34.0);
    let orbit_phase = log(trap.x + 0.007) * (4.5 + detail * 4.0);
    let orbit_footprint = length(trap_gradient) * pixel() * zoom * 1.25
        * (4.5 + detail * 4.0) / (trap.x + 0.007);
    let orbit_filter = 1.0 - smoothstep(0.8, 2.8, orbit_footprint);
    let orbit_contours = pow(0.5 + 0.5 * cos(orbit_phase) * orbit_filter, 9.0);
    let signal = orbit_signal / max(orbit_weight, 0.0001);
    let depth = clamp(smooth_iterations / 32.0, 0.0, 1.0);
    let blue = vec3<f32>(0.018, 0.085, 0.43);
    let violet = vec3<f32>(0.29, 0.025, 0.63);
    let gold = vec3<f32>(1.0, 0.54, 0.13);
    let mineral = mix(blue, violet, smoothstep(0.14, 0.72, depth) * 0.72);
    // The analytic complex derivative gives a relief orientation without two
    // more fractal evaluations or a 3D raymarch.
    let gradient = trap_gradient;
    let direction = gradient / max(length(gradient), 0.00001);
    let normal = normalize(vec3<f32>(direction * (0.4 + boundary * 2.8), 1.0));
    let sculpture = 0.20 + 0.80 * max(dot(normal, normalize(vec3<f32>(-0.45, 0.55, 0.8))), 0.0);
    let exterior_edge = boundary * select(0.12, 1.0, escaped);
    var light = vec3<f32>(0.002, 0.002, 0.012)
        + mineral * (0.09 + depth * 0.13 + exterior_edge * 0.16) * sculpture;
    light += mix(blue, violet, ring_ink) * contours * (0.07 + detail * 0.13);
    var marks = mineral * exterior_edge * (0.18 + signal * 0.60 + mids() * 0.12);
    marks += mix(vec3<f32>(0.42, 0.66, 0.98), gold, pow(ring_ink, 3.0) * 0.62)
        * orbit_contours * (0.30 + signal * 0.78 + bass() * 0.12) * sculpture;
    marks += mix(blue, violet, 0.55) * cross_ink * (0.03 + air() * 0.12 + signal * 0.12);
    marks += gold * orbit_contours * pow(ring_ink, 5.0) * (beat() * 0.20 + u.h.w * 0.18);
    return World(light, marks * (0.65 + u.d.y * 0.42));
}

// Recursive plane folds and bounded circle inversion: a distinct Apollonian-
// like limit-set construction, not a second quadratic Julia colour treatment.
fn inversion_lace(uv: vec2<f32>) -> World {
    let time = u.a.z + u.j.w * 0.25;
    let detail = detail_level();
    let look = math_look();
    var z = rotate(math_domain(uv, look) * (0.88 + sin(time * 0.13) * 0.07), time * 0.035);
    var derivative = 1.0;
    var light = vec3<f32>(0.002, 0.004, 0.012);
    var marks = vec3<f32>(0.0);
    let fold = vec2<f32>(0.72 + sin(time * 0.23) * 0.15 + u.h.z * 0.07,
        0.62 + cos(time * 0.19 + 0.7) * 0.13 + u.h.x * 0.04) + look.lace.xy;
    let recursion_turn = 0.13 + sin(time * 0.17) * 0.13 + look.lace.w;
    let recursion_offset = vec2<f32>(-0.48 + sin(time * 0.21) * 0.10,
        -0.36 + cos(time * 0.16 + 1.1) * 0.09);
    let inversion_radius = 1.30 + sin(time * 0.15 + 0.6) * 0.16 + u.h.z * 0.06 + look.lace.z;
    for (var layer = 0; layer < 10; layer += 1) {
        let i = f32(layer);
        let signal = spectrum_at(i / 9.0);
        z = 2.0 * clamp(z, -fold, fold) - z;
        // Off-centre moving orbit traps and changing inversion geometry make
        // entire recursive branches grow through one another, not just spin.
        let trap_centre = vec2<f32>(sin(time * 0.25 + i * 0.37),
            cos(time * 0.22 - i * 0.29)) * (0.055 + u.d.z * 0.025 + look.orbit.w);
        let trap_radius = 0.46 + sin(time * 0.29 + i * 0.43) * 0.065
            + signal * 0.055 + look.orbit.x * 0.65;
        let orbit = z - trap_centre;
        let orbit_radius = max(length(orbit), 0.0001);
        let r2 = max(dot(z, z), 0.0001);
        var trap_distance = orbit_radius - trap_radius;
        if any(look.orbit != vec4<f32>(0.0)) || look.julia.w != 0.0 {
            let diamond = (abs(orbit.x) + abs(orbit.y)) * 0.78 - trap_radius;
            trap_distance = mix(trap_distance, diamond, max(look.julia.w, 0.0) * 0.55);
            let cross = min(abs(orbit.x), abs(orbit.y)) - trap_radius * 0.24;
            trap_distance = mix(trap_distance, cross, look.orbit.y);
            let hyperbola = abs(orbit.x * orbit.y) / (1.0 + orbit_radius) - trap_radius * 0.18;
            trap_distance = mix(trap_distance, hyperbola, look.orbit.z);
        }
        let distance = trap_distance / derivative;
        let visible = (1.0 - smoothstep(0.24, 0.90, pixel() * derivative))
            * (1.0 - smoothstep(4.5 + detail * 5.0, 5.5 + detail * 5.0, i));
        let edge = wire(distance, pixel() * 0.60) * visible;
        let engraving = wire((orbit_radius - trap_radius - 0.026) / derivative,
            pixel() * 0.45) * visible * detail;
        let lens = exp(-abs(distance) * (10.0 + i * 2.0)) * visible / (1.0 + i * 0.24);
        let normal = normalize(vec3<f32>(orbit / orbit_radius * 0.8, 0.65));
        let relief = 0.22 + 0.78 * max(dot(normal, normalize(vec3<f32>(-0.4, 0.6, 0.9))), 0.0);
        let material = mix(vec3<f32>(0.035, 0.16, 0.48), vec3<f32>(0.54, 0.08, 0.62), i / 9.0);
        light += material * lens * relief * 0.065;
        marks += mix(material, vec3<f32>(0.52, 0.78, 0.95), 0.55)
            * (edge + engraving * 0.28) * relief
            * (0.20 + signal * 0.75 + mids() * 0.11 + air() * 0.07);
        marks += vec3<f32>(1.0, 0.58, 0.19) * edge * pow(relief, 5.0)
            * (0.018 + bass() * 0.06 + beat() * 0.055);
        let inversion = inversion_radius / clamp(r2, 0.18, inversion_radius);
        z = rotate(z * inversion, recursion_turn)
            + recursion_offset + vec2<f32>(mids(), bass()) * 0.018;
        derivative = min(derivative * inversion, 100000.0);
    }
    return World(light, marks * (0.70 + u.d.y * 0.40));
}

// Projected, out-of-plane superformula contours form a folded vessel. The
// height and view tilt differ across strata, so this is not a flat petal decal.
fn harmonic_sculpture(uv: vec2<f32>) -> World {
    // The journey clock is already pace/rhythm integrated. A stronger local
    // rate unfolds these strata visibly without assigning phase to raw beats.
    let time = u.a.z * 2.40 + u.j.w * 0.18;
    let detail = detail_level();
    let look = math_look();
    // CPU-smoothed, user-mapped frequency rows give the connected strata
    // different jobs. Never multiply the clock by a current audio envelope:
    // music changes the folds, not their accumulated position or camera.
    let low_level = (spectrum_at(0.045) + spectrum_at(0.10) + spectrum_at(0.15)) / 3.0;
    let mid_level = (spectrum_at(0.23) + spectrum_at(0.34) + spectrum_at(0.46)) / 3.0;
    let high_level = (spectrum_at(0.61) + spectrum_at(0.76) + spectrum_at(0.92)) / 3.0;
    let low_drive = low_level / (0.45 + low_level);
    let mid_drive = mid_level / (0.45 + mid_level);
    let high_drive = high_level / (0.45 + high_level);
    let turn = 0.22 * sin(time * 0.12) + time * 0.028;
    let p = rotate(math_domain(uv, look), turn);
    let tilt = 0.58 + 0.24 * sin(time * 0.16);
    let ct = cos(tilt);
    let st = sin(tilt);
    var light = vec3<f32>(0.003, 0.003, 0.012);
    var marks = vec3<f32>(0.0);
    for (var layer = 0; layer < 28; layer += 1) {
        let i = f32(layer);
        let radius = 0.16 + i * 0.039;
        let band = i / 27.0;
        let measured = spectrum_at(band) * 0.60
            + (spectrum_at(band - 0.025) + spectrum_at(band + 0.025)) * 0.20;
        let signal = measured / (0.40 + measured);
        // Each stratum follows its own slow bend through the sculptural body.
        // Closed 2/3/5 harmonics exchange prominence without an angular seam.
        let centre = vec2<f32>(sin(time * 0.17 + radius * 2.1) * (0.27 + mid_drive * 0.14),
            cos(time * 0.14 - radius * 1.7) * (0.19 + signal * 0.10)) * radius;
        let projected = p - centre;
        var q = vec2<f32>(projected.x, projected.y / ct);
        var theta = atan2(q.y, q.x);
        var height = 0.0;
        for (var projection = 0; projection < 2; projection += 1) {
            height = sin(theta * 3.0 + time * 0.31 + radius * 1.3) * radius
                    * (0.25 + sin(time * 0.18 + radius) * 0.075 + signal * 0.16)
                + sin(theta * 2.0 - time * 0.25) * radius * (0.15 + signal * 0.09);
            if look.sculpture.z != 0.0 {
                height += sin(theta * 5.0 - time * 0.19 + radius * 1.8)
                    * radius * look.sculpture.z * 0.7;
            }
            let perspective = 1.0 + height * 0.28;
            q = vec2<f32>(projected.x * perspective, (projected.y * perspective - height * st) / ct);
            theta = atan2(q.y, q.x);
        }
        let exponent = 3.8 + sin(time * 0.23 + radius * 0.7) * 1.3 + u.h.z * 0.6 + look.sculpture.w;
        let folded_angle = theta + sin(theta * 2.0 + time * 0.21 + radius * 1.6)
            * (0.14 + mid_drive * 0.12 + signal * 0.07);
        let superformula = pow(pow(abs(cos(folded_angle * 0.75)), exponent)
            + pow(abs(sin(folded_angle * 0.75)), exponent), -1.0 / (2.3 + u.h.x * 0.3));
        // Closed integer harmonics braid through the tilted strata. Their
        // changing phase, not a discontinuous angular frequency, makes folds
        // open into lobes and then interleave without a seam at +/- pi.
        let third = theta * 3.0 + time * 0.29 + radius * 2.3;
        let fifth = theta * 5.0 - time * 0.24 - radius * 1.7;
        var seventh_height = 0.0;
        var seventh_slope = 0.0;
        if look.sculpture.z != 0.0 {
            let seventh = theta * 7.0 + time * 0.18 + radius * 2.6;
            seventh_height = sin(seventh) * look.sculpture.z;
            seventh_slope = cos(seventh) * look.sculpture.z * 7.0;
        }
        let third_strength = 0.105 + sin(time * 0.19 + radius * 0.7) * 0.085
            + u.h.z * 0.045 + look.sculpture.x + signal * 0.13;
        let fifth_strength = 0.075 + cos(time * 0.16 - radius * 0.8) * 0.055
            + u.d.z * 0.025 + look.sculpture.y + mid_drive * 0.04 + high_drive * 0.04;
        let lobes = 1.0 + sin(third) * third_strength + cos(fifth) * fifth_strength
            + seventh_height;
        let shape = radius * (0.65 + superformula * 0.35) * lobes
            * (1.0 + signal * 0.10 + low_drive * 0.16);
        let distance = (length(q) - shape) * ct;
        let visible = 1.0 - smoothstep(12.0 + detail * 14.0, 13.0 + detail * 14.0, i);
        let line = wire(distance, pixel() * 0.65) * visible;
        let fine = wire(distance - 0.012 * ct, pixel() * 0.45) * visible * detail;
        let slope = cos(theta * 3.0 + time * 0.31 + radius * 1.3)
                * (0.75 + sin(time * 0.18 + radius) * 0.225 + signal * 0.48)
            + cos(theta * 2.0 - time * 0.25) * (0.30 + signal * 0.18)
            + cos(third) * third_strength * 3.0 - sin(fifth) * fifth_strength * 5.0
            + seventh_slope;
        let normal = normalize(vec3<f32>(-cos(theta) * slope * 0.4,
            -sin(theta) * slope * ct * 0.4 - st, ct));
        let relief = 0.24 + 0.76 * max(dot(normal, normalize(vec3<f32>(-0.4, 0.5, 1.0))), 0.0);
        let material = mix(vec3<f32>(0.13, 0.09, 0.46), vec3<f32>(0.24, 0.55, 0.83), i / 27.0);
        light += material * exp(-abs(distance) * 52.0) * relief * visible * 0.035;
        marks += mix(material, vec3<f32>(0.80, 0.83, 0.95), 0.48) * (line + fine * 0.30)
            * relief * (0.28 + signal * 0.67 + mids() * 0.10 + air() * 0.08);
        marks += vec3<f32>(1.0, 0.61, 0.22) * line * pow(relief, 5.0)
            * (0.025 + beat() * 0.065);
    }
    return World(light, marks * (0.75 + u.d.y * 0.40));
}

fn curl_velocity(p: vec2<f32>, time: f32) -> vec2<f32> {
    let q = p * 1.7 + vec2<f32>(time * 0.19, -time * 0.16);
    return vec2<f32>(sin(q.x) * cos(q.y), -cos(q.x) * sin(q.y));
}

// Six composed curl-advection maps, carrying their Jacobian for filtered
// contour shading. This is procedural flow, not a Navier-Stokes simulation.
fn vortex_weave(uv: vec2<f32>) -> World {
    let time = u.a.z + u.j.w * 0.2;
    let detail = detail_level();
    let look = math_look();
    let stretch = vec2<f32>(1.0 + look.domain.x, 1.0 / (1.0 + look.domain.x));
    let scale = 1.05 * (1.0 + look.domain.w);
    var p = uv * scale * stretch;
    var dx = vec2<f32>(scale * stretch.x, 0.0);
    var dy = vec2<f32>(0.0, scale * stretch.y);
    var frequency = 1.2 + look.domain.z;
    var signal_sum = 0.0;
    for (var layer = 0; layer < 6; layer += 1) {
        let i = f32(layer);
        let signal = spectrum_at(i / 5.0);
        signal_sum += signal / 6.0;
        let angle = i * 0.71 + sin(time * 0.18 + i * 0.37) * (0.23 + look.domain.y * 0.5);
        let q = rotate(p, angle) * frequency
            + vec2<f32>(time * (0.24 + i * 0.017), -time * 0.19 + i);
        let sx = sin(q.x); let sy = sin(q.y);
        let cx = cos(q.x); let cy = cos(q.y);
        let amplitude = (0.32 + u.d.z * 0.12 + u.h.x * 0.09 + signal * 0.04)
            * (0.70 + detail * 0.30) * (0.90 + sin(time * 0.22 + i * 0.83) * 0.18);
        let local_dx = rotate(dx, angle);
        let local_dy = rotate(dy, angle);
        dx += rotate(vec2<f32>(cx * cy * local_dx.x - sx * sy * local_dx.y,
            sx * sy * local_dx.x - cx * cy * local_dx.y), -angle) * amplitude;
        dy += rotate(vec2<f32>(cx * cy * local_dy.x - sx * sy * local_dy.y,
            sx * sy * local_dy.x - cx * cy * local_dy.y), -angle) * amplitude;
        p += rotate(vec2<f32>(sx * cy, -cx * sy), -angle) * amplitude / frequency;
        frequency *= 1.65;
    }
    let x_frequency = 3.1 + look.curl.x;
    let y_frequency = 2.7 + look.curl.y;
    let x = p.x * x_frequency + time * 0.27;
    let y = p.y * y_frequency - time * 0.21;
    let diagonal = p.x * 1.8 + p.y * 1.6 - time * 0.19;
    let cross = p.x * 1.1 - p.y * 0.8 + time * 0.15;
    // Moving levels cross saddle points of this interference surface. The
    // channels open into islands and rejoin while their nested curl persists.
    let level = sin(time * 0.16) * 0.34 + cos(time * 0.21 + 0.7) * 0.16;
    let diagonal_weight = 0.55 + look.curl.z;
    var second_harmonic = 0.0;
    var second_gradient = vec2<f32>(0.0);
    if look.curl.w != 0.0 {
        second_harmonic = sin(x * 2.0) * cos(y * 2.0) * look.curl.w;
        second_gradient = vec2<f32>(cos(x * 2.0) * cos(y * 2.0) * (x_frequency * 2.0),
            -sin(x * 2.0) * sin(y * 2.0) * (y_frequency * 2.0)) * look.curl.w;
    }
    let field = sin(x) * cos(y) + sin(diagonal) * diagonal_weight + cos(cross) * 0.30
        + second_harmonic - level;
    let gradient = vec2<f32>(cos(x) * cos(y) * x_frequency
            + cos(diagonal) * (0.99 + look.curl.z * 1.8) - sin(cross) * 0.33
            + second_gradient.x,
        -sin(x) * sin(y) * y_frequency + cos(diagonal) * (0.88 + look.curl.z * 1.6)
            + sin(cross) * 0.24 + second_gradient.y);
    let world_gradient = vec2<f32>(dot(gradient, dx), dot(gradient, dy));
    let contour_scale = 5.0 + detail * 13.0;
    let footprint = length(world_gradient) * pixel() * contour_scale;
    let contour = pow(0.5 + 0.5 * cos(field * contour_scale + time * 0.17)
        * (1.0 - smoothstep(0.8, 2.8, footprint)), 9.0);
    let normal = normalize(vec3<f32>(world_gradient * sin(field * 1.3) * 0.28, 1.0));
    let relief = 0.18 + 0.82 * max(dot(normal, normalize(vec3<f32>(-0.5, 0.65, 0.7))), 0.0);
    let ridge = pow(0.5 + 0.5 * cos(field * 1.3), 3.0);
    let material = mix(vec3<f32>(0.018, 0.11, 0.30), vec3<f32>(0.28, 0.035, 0.48), smoothstep(-1.0, 1.5, field));
    let light = vec3<f32>(0.002, 0.003, 0.009) + material * ridge * relief * (0.10 + mids() * 0.035);
    var marks = mix(material, vec3<f32>(0.43, 0.73, 0.88), 0.55) * contour * relief
        * (0.23 + signal_sum * 0.95 + air() * 0.12);
    marks += material * ridge * relief * (0.025 + bass() * 0.07 + signal_sum * 0.12);
    marks += vec3<f32>(1.0, 0.56, 0.14) * contour * pow(relief, 7.0) * (0.02 + beat() * 0.08);
    return World(light, marks * (0.75 + u.d.y * 0.4));
}

fn math_family(uv: vec2<f32>, family: f32) -> World {
    if family < 0.5 { return fractal_bloom(uv); }
    if family < 1.5 { return inversion_lace(uv); }
    if family < 2.5 { return harmonic_sculpture(uv); }
    return vortex_weave(uv);
}

// Original spatial transition field: a travelling, curled growth front rather
// than a whole-canvas opacity fade. uv is the centred height-normalized canvas
// coordinate BEFORE shared openness scaling, so light, flow and memory can all
// use exactly the same weight. Family IDs seed a stable route; continuous
// journey phase and mapped phrase energy shape its growth without frame noise.
fn math_interlace_weight(uv: vec2<f32>, amount: f32) -> f32 {
    let progress = clamp(amount, 0.0, 1.0);
    if progress <= 0.0 { return 0.0; }
    if progress >= 1.0 { return 1.0; }
    if u.j.x == u.j.y { return progress; }
    let phase = u.j.w;
    let seed = u.j.x * 1.73 + u.j.y * 0.91;
    let p = rotate(uv, seed + sin(phase * 0.17 + seed) * 0.14);
    let centre = vec2<f32>(sin(seed + phase * 0.13) * 0.16,
        cos(seed * 1.3 - phase * 0.11) * 0.12);
    let radial = length(p - centre);
    let curl = sin(p.x * 2.7 + p.y * 0.8 + phase * 0.21 + seed) * 0.31
        + sin(p.y * 3.2 - p.x * 0.7 - phase * 0.17 - seed) * 0.20
        + sin((p.x + p.y) * 5.1 + phase * 0.09) * 0.075;
    let field = p.x * 0.42 + radial * 0.18 + curl;
    let position = 0.5 + field / (1.0 + abs(field)) * 0.42;
    let softness = 0.17 - clamp(u.h.x, 0.0, 1.0) * 0.035;
    let front = progress * (1.0 + softness * 2.0) - softness;
    return smoothstep(position - softness, position + softness, front);
}

fn living_mathematics(uv: vec2<f32>) -> World {
    let amount = clamp(u.j.z, 0.0, 1.0);
    if amount >= 1.0 { return math_family(uv, u.j.y); }
    if amount <= 0.0 || u.j.x == u.j.y { return math_family(uv, u.j.x); }
    let canvas_uv = uv / (0.96 + u.h.y * 0.08);
    let woven = math_interlace_weight(canvas_uv, amount);
    // Only the moving front needs both families. Exact zero/one spatial
    // regions do not pay for a second fractal, and introduce no cutoff seam.
    if woven >= 1.0 { return math_family(uv, u.j.y); }
    let before = math_family(uv, u.j.x);
    if woven <= 0.0 { return before; }
    let after = math_family(uv, u.j.y);
    return World(mix(before.light, after.light, woven), mix(before.marks, after.marks, woven));
}
