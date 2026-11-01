// Glass Tide: an advected liquid relief, not an ocean/horizon or a radial
// tunnel. Three moving Gaussian sheets merge and separate in a smooth curl
// field. Their analytic gradients produce material normals and fine contours.
// MilkDrop's persistent-coordinate-warp principle informs the separate flow
// below; no MilkDrop preset, screenshot or image is embedded in this shader.

// Each recipe changes the liquid's coordinate map, connected surface and
// capillary geometry. They interpolate before one surface evaluation, not
// between ten rendered pictures. Recipe zero is the original Pearl Currents.
struct TideLook {
    domain: vec4<f32>,  // x/y scale, rotation, shear
    shape: vec4<f32>,   // sheet separation, level, x/y squeeze
    topology: vec4<f32>, // extra folds, vortex, braided ribbons, cellular relief
    caustic: vec4<f32>, // density, marble scale, line weight, current strength
    special: vec4<f32>, // shell, fan, laminae, ripple tessellation
};

fn tide_look_recipe(id: f32) -> TideLook {
    switch u32(clamp(round(id), 0.0, 9.0)) {
        case 1u: { // Folded Membrane: long, pleated, diagonally stretched glass.
            return TideLook(vec4<f32>(0.86, 1.34, -0.19, 0.28),
                vec4<f32>(0.92, 0.48, 0.78, 1.44), vec4<f32>(1.0, 0.0, 0.0, 0.0),
                vec4<f32>(1.25, 0.95, 1.30, 0.55), vec4<f32>(0.0));
        }
        case 2u: { // Vortex Pool: a turning liquid rim around a hollow centre.
            return TideLook(vec4<f32>(1.08, 1.08, 0.0, 0.0),
                vec4<f32>(0.74, 0.43, 1.0, 1.0), vec4<f32>(0.0, 1.0, 0.0, 0.0),
                vec4<f32>(0.88, 1.25, 1.35, 1.20), vec4<f32>(0.0));
        }
        case 3u: { // Moire Laminae: offset, thin sheets with broad interference.
            return TideLook(vec4<f32>(0.84, 1.18, 0.33, 0.12),
                vec4<f32>(0.85, 0.45, 0.76, 1.25), vec4<f32>(0.25, 0.0, 0.0, 0.0),
                vec4<f32>(1.65, 0.52, 1.18, 0.25), vec4<f32>(0.0, 0.0, 1.0, 0.0));
        }
        case 4u: { // Splitting Droplets: separated lenses, not one large sheet.
            return TideLook(vec4<f32>(0.96, 0.98, -0.10, 0.0),
                vec4<f32>(1.56, 0.53, 1.50, 1.50), vec4<f32>(0.0),
                vec4<f32>(0.85, 0.82, 0.95, 0.45), vec4<f32>(0.0));
        }
        case 5u: { // Shell Fold: open logarithmic folds, with no angular seam.
            return TideLook(vec4<f32>(0.94, 1.04, 0.20, 0.0),
                vec4<f32>(0.80, 0.40, 0.90, 1.0), vec4<f32>(0.22, 0.0, 0.0, 0.0),
                vec4<f32>(0.78, 0.62, 1.35, 0.45), vec4<f32>(1.0, 0.0, 0.0, 0.0));
        }
        case 6u: { // Braided Currents: two crossing, moving liquid ribbons.
            return TideLook(vec4<f32>(0.86, 1.12, -0.28, 0.16),
                vec4<f32>(0.90, 0.45, 1.0, 1.0), vec4<f32>(0.0, 0.0, 1.0, 0.0),
                vec4<f32>(1.18, 1.12, 1.40, 0.80), vec4<f32>(0.0));
        }
        case 7u: { // Cellular Glass: connected foamy lenses and dark cavities.
            return TideLook(vec4<f32>(1.02, 0.98, 0.13, 0.09),
                vec4<f32>(1.0, 0.38, 1.0, 1.0), vec4<f32>(0.0, 0.0, 0.0, 1.0),
                vec4<f32>(1.12, 1.38, 1.28, 0.80), vec4<f32>(0.0));
        }
        case 8u: { // Prismatic Fan: three tapered, asymmetrical glass leaves.
            return TideLook(vec4<f32>(0.90, 1.05, -0.38, 0.18),
                vec4<f32>(1.0, 0.47, 0.78, 1.30), vec4<f32>(0.30, 0.0, 0.0, 0.0),
                vec4<f32>(1.80, 0.68, 1.28, 0.35), vec4<f32>(0.0, 1.0, 0.0, 0.0));
        }
        case 9u: { // Ripple Tessellation: interlocking saddle-shaped ripples.
            return TideLook(vec4<f32>(0.98, 1.08, 0.08, -0.10),
                vec4<f32>(1.0, 0.43, 1.0, 1.0), vec4<f32>(0.15, 0.0, 0.0, 0.0),
                vec4<f32>(1.45, 1.30, 1.30, 0.65), vec4<f32>(0.0, 0.0, 0.0, 1.0));
        }
        default: {
            return TideLook(vec4<f32>(1.0, 1.0, 0.0, 0.0),
                vec4<f32>(1.0, 0.43, 1.0, 1.0), vec4<f32>(0.0),
                vec4<f32>(1.0), vec4<f32>(0.0));
        }
    }
}

fn tide_look() -> TideLook {
    let state = u.looks[2];
    let amount = clamp(state.z, 0.0, 1.0);
    if amount >= 1.0 { return tide_look_recipe(state.y); }
    let before = tide_look_recipe(state.x);
    if amount <= 0.0 || state.x == state.y { return before; }
    let after = tide_look_recipe(state.y);
    return TideLook(mix(before.domain, after.domain, amount),
        mix(before.shape, after.shape, amount), mix(before.topology, after.topology, amount),
        mix(before.caustic, after.caustic, amount), mix(before.special, after.special, amount));
}

fn tide_current(p: vec2<f32>, time: f32) -> vec2<f32> {
    let a = p.x * 1.10 + time * 0.071;
    let b = p.y * 1.35 - time * 0.063;
    let cross = cos(p.x * 0.73 - p.y * 1.17 + time * 0.049);
    // Curl of a bounded scalar streamfunction: continuous, non-radial motion.
    return vec2<f32>(sin(a) * cos(b) * 1.35, -cos(a) * sin(b) * 1.10)
        + vec2<f32>(-1.17, -0.73) * cross * 0.19;
}

fn tide_look_current(p: vec2<f32>, time: f32, look: TideLook) -> vec2<f32> {
    var current = tide_current(p, time) * look.caustic.w;
    let radial = 1.0 + dot(p, p) * 1.6;
    current += vec2<f32>(-p.y, p.x) * (look.topology.y * 1.9 + look.special.x * 0.35) / radial;
    current += vec2<f32>(0.70, cos(p.x * 2.1 - time * 0.12) * 0.55) * look.topology.z;
    current += vec2<f32>(sin(p.y * 2.8 + time * 0.07), -sin(p.x * 2.6 - time * 0.09))
        * (look.topology.w * 0.18 + look.special.w * 0.12);
    current += vec2<f32>(0.38, p.y * 0.12) * look.special.y;
    return current;
}

fn tide_domain(p0: vec2<f32>, time: f32, look: TideLook) -> vec2<f32> {
    var p = rotate(p0 * look.domain.xy, look.domain.z);
    p.x += p.y * look.domain.w;
    p.y += sin(p.x * 2.7 + time * 0.09) * look.topology.x * 0.21;
    p = rotate(p, look.topology.y * 0.8 / (0.55 + dot(p, p)));
    return p;
}

fn tide_ribbon(p: vec2<f32>, phase: f32, frequency: f32, amplitude: f32,
    offset: f32, width: f32) -> vec3<f32> {
    let wave = p.x * frequency + phase;
    let centre = sin(wave) * amplitude + offset;
    let distance = p.y - centre;
    let value = exp(-distance * distance * width * width - p.x * p.x * 0.22);
    let slope = cos(wave) * amplitude * frequency;
    return vec3<f32>(value, value * (2.0 * distance * width * width * slope - p.x * 0.44),
        value * (-2.0 * distance * width * width));
}

fn tide_cells(p: vec2<f32>, time: f32, ripple: f32) -> vec3<f32> {
    let x = p.x * mix(4.1, 3.1, ripple) + time * 0.055;
    let y = p.y * mix(3.7, 4.0, ripple) - time * 0.043;
    let diagonal = (p.x + p.y) * 3.1 - time * 0.061;
    let weave = sin(x) * cos(y) + sin(diagonal) * mix(0.24, 0.65, ripple);
    let gradient = vec2<f32>(cos(x) * cos(y) * mix(4.1, 3.1, ripple),
        -sin(x) * sin(y) * mix(3.7, 4.0, ripple))
        + vec2<f32>(cos(diagonal) * 3.1 * mix(0.24, 0.65, ripple));
    let envelope = exp(-dot(p, p * vec2<f32>(0.20, 0.36)));
    let level = 0.51 + weave * mix(0.31, 0.25, ripple);
    return vec3<f32>(level * envelope,
        (gradient * mix(0.31, 0.25, ripple) - level * p * vec2<f32>(0.40, 0.72)) * envelope);
}

// Wide pleated membranes are a separate connected geometry, rather than
// another arrangement of the three Pearl Currents droplets. The measured
// low/mid envelopes bend the cloth; its continuous derivative shades the folds.
fn tide_membrane(p: vec2<f32>, time: f32, low: f32, middle: f32) -> vec3<f32> {
    let bend = p.x * 1.35 + time * 0.065;
    let phase = p.y * 6.5 + sin(bend) * (1.1 + middle * 0.50);
    let dphase = vec2<f32>(cos(bend) * 1.35 * (1.1 + middle * 0.50), 6.5);
    let weave = p.x * 2.3 - p.y * 0.7 - time * 0.085;
    let level = 0.53 + cos(phase) * (0.21 + low * 0.10) + sin(weave) * 0.14;
    let slope = -sin(phase) * dphase * (0.21 + low * 0.10)
        + cos(weave) * vec2<f32>(2.3, -0.7) * 0.14;
    let envelope = exp(-pow(abs(p.x) * 0.42, 4.0) - pow(abs(p.y) * 0.64, 4.0));
    let denvelope = -4.0 * p * p * p * vec2<f32>(0.03111696, 0.16777216);
    return vec3<f32>(level * envelope, (slope + level * denvelope) * envelope);
}

// A bounded Voronoi foam gives Cellular Glass polygonal cavities and moving
// junctions. Unlike a sine-grid it has no repeated horizontal/vertical rows.
fn tide_cellular_glass(p: vec2<f32>, time: f32) -> vec3<f32> {
    let frequency = 2.6;
    let q = p * frequency;
    let cell = floor(q);
    let local = fract(q);
    var first = 10.0;
    var second = 10.0;
    var first_gradient = vec2<f32>(0.0);
    var second_gradient = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let offset = vec2<f32>(f32(x), f32(y));
            let seed = hash(vec3<f32>(cell + offset, 7.13));
            let band = spectrum_at(seed);
            let node = offset + vec2<f32>(0.5)
                + sin(vec2<f32>(seed * 6.3, seed * 11.7) + time * 0.075)
                    * (0.13 + band * 0.12);
            let delta = local - node;
            let distance = max(length(delta), 0.0001);
            if distance < first {
                second = first;
                second_gradient = first_gradient;
                first = distance;
                first_gradient = delta / distance;
            } else if distance < second {
                second = distance;
                second_gradient = delta / distance;
            }
        }
    }
    let gap = second - first;
    let wall = exp(-gap * gap * 13.0);
    let slope = wall * (-26.0 * gap) * (second_gradient - first_gradient) * frequency;
    let envelope = exp(-dot(p, p * vec2<f32>(0.045, 0.075)));
    let level = 0.16 + wall * 0.69;
    return vec3<f32>(level * envelope,
        (slope * 0.69 - level * p * vec2<f32>(0.09, 0.15)) * envelope);
}

fn tide_shell(p: vec2<f32>, time: f32) -> vec3<f32> {
    // Sine/cosine of angle avoid atan2 seams; the soft centre has finite slope.
    let radius = sqrt(dot(p, p) + 0.04);
    let phase = log(radius + 0.22) * 4.2 - time * 0.065;
    let direction = p / radius;
    let dphase = p / radius / (radius + 0.22) * 4.2;
    let dx = vec2<f32>(1.0, 0.0) / radius - p.x * p / pow(radius, 3.0);
    let dy = vec2<f32>(0.0, 1.0) / radius - p.y * p / pow(radius, 3.0);
    let spiral = sin(phase) * direction.x - cos(phase) * direction.y;
    let slope = cos(phase) * dphase * direction.x + sin(phase) * dx
        + sin(phase) * dphase * direction.y - cos(phase) * dy;
    let envelope = exp(-dot(p, p) * 0.35);
    let level = 0.54 + spiral * 0.35;
    return vec3<f32>(level * envelope, (slope * 0.35 - level * p * 0.70) * envelope);
}

fn tide_sheet(p: vec2<f32>, centre: vec2<f32>, axes: vec2<f32>) -> vec3<f32> {
    let q = p - centre;
    let value = exp(-dot(q * axes, q * axes));
    return vec3<f32>(value, -2.0 * q * axes * axes * value);
}

fn tide_marble(p: vec2<f32>, time: f32) -> vec3<f32> {
    // Interlocked, differently advected folds. Analytic chain-rule slopes keep
    // the fine caustics in the same material instead of concentric blob rings.
    let bend0 = dot(p, vec2<f32>(-1.0, 1.6)) - time * 0.091;
    let bend1 = dot(p, vec2<f32>(1.8, 0.6)) + time * 0.073;
    let a = dot(p, vec2<f32>(2.7, 1.3)) + sin(bend0) * 0.85 - time * 0.11;
    let b = dot(p, vec2<f32>(-1.9, 3.4)) + sin(bend1) * 0.65 + time * 0.085;
    let da = vec2<f32>(2.7, 1.3) + vec2<f32>(-1.0, 1.6) * cos(bend0) * 0.85;
    let db = vec2<f32>(-1.9, 3.4) + vec2<f32>(1.8, 0.6) * cos(bend1) * 0.65;
    return vec3<f32>(sin(a) * 0.72 + cos(b) * 0.48,
        cos(a) * da * 0.72 - sin(b) * db * 0.48);
}

fn tide_flow(uv: vec2<f32>, delta: f32) -> vec2<f32> {
    let look = tide_look();
    let time = u.a.z;
    let advance = delta * u.c.z * u.i.y;
    let origin = uv * 0.92;
    let p = tide_domain(origin, time, look);
    var current = tide_look_current(p, time, look);
    // Pull the material-space current back through the same domain transform.
    // The small, bounded Jacobian avoids history sliding in a different frame
    // when a folded or anisotropic recipe morphs into another one.
    if any(look.domain != vec4<f32>(1.0, 1.0, 0.0, 0.0))
        || look.topology.x != 0.0 || look.topology.y != 0.0 {
        let epsilon = 0.003;
        let dx = (tide_domain(origin + vec2<f32>(epsilon, 0.0), time, look)
            - tide_domain(origin - vec2<f32>(epsilon, 0.0), time, look)) / (epsilon * 2.0);
        let dy = (tide_domain(origin + vec2<f32>(0.0, epsilon), time, look)
            - tide_domain(origin - vec2<f32>(0.0, epsilon), time, look)) / (epsilon * 2.0);
        let determinant = dx.x * dy.y - dy.x * dx.y;
        let safe_determinant = select(-max(abs(determinant), 0.20),
            max(abs(determinant), 0.20), determinant >= 0.0);
        current = vec2<f32>(dy.y * current.x - dy.x * current.y,
            dx.x * current.y - dx.y * current.x) / safe_determinant;
        current = current / max(1.0, length(current) / 2.4);
    }
    return uv - current * advance * (0.028 + u.d.z * 0.018);
}

fn tide_memory() -> f32 {
    // Enough memory for liquid streaks without an opaque, accumulated fog.
    let look = tide_look();
    return 0.48 + look.topology.y * 0.06 + look.topology.z * 0.07
        + look.special.z * 0.05;
}

fn tide_surface_domain(uv: vec2<f32>, look: TideLook) -> vec2<f32> {
    let time = u.a.z;
    let warp = u.d.z;
    var p = rotate(uv, sin(time * 0.043) * 0.085)
        * (0.94 + sin(time * 0.037) * 0.035);
    p += vec2<f32>(sin(time * 0.026) * 0.18, cos(time * 0.031) * 0.13);
    p = tide_domain(p, time, look);
    p += tide_look_current(p, time, look) * (0.10 + warp * 0.16);
    return p;
}

fn tide_surface_field(p: vec2<f32>, look: TideLook) -> vec3<f32> {
    let time = u.a.z;
    let warp = u.d.z;
    let look_time = time + u.looks[2].w * 0.12;
    // Mapped, measured frequency positions are distributed over the liquid,
    // rather than replacing the data with a scripted decorative waveform.
    let low = spectrum_at(clamp(0.085 + sin(p.y * 1.3) * 0.055, 0.0, 0.15));
    let middle = spectrum_at(clamp(0.32 + sin(p.x * 1.2 + p.y * 0.4) * 0.12, 0.18, 0.47));
    let passage = 0.5 + 0.5 * sin(time * 0.047 + u.h.z * 0.35);
    let c0 = vec2<f32>(-0.78 + sin(time * 0.073) * 0.36,
        -0.12 + cos(time * 0.055) * 0.22);
    let c1 = vec2<f32>(0.62 + cos(time * 0.057) * 0.34,
        0.24 + sin(time * 0.041) * 0.34);
    let c2 = vec2<f32>(-0.04 + sin(time * 0.037) * 0.52,
        -0.64 + cos(time * 0.069) * 0.29);
    let axes = look.shape.zw;
    let sheet0 = tide_sheet(p, c0 * look.shape.x, vec2<f32>(0.98, 1.18 + passage * 0.26) * axes);
    let sheet1 = tide_sheet(p, c1 * look.shape.x, vec2<f32>(1.11 + passage * 0.17, 0.94) * axes);
    let sheet2 = tide_sheet(p, c2 * look.shape.x, vec2<f32>(1.25, 1.04) * axes);
    var field = sheet0 * (0.92 + low * 0.045)
        + sheet1 * (0.81 + middle * 0.055) + sheet2 * 0.63;

    // These are genuinely different surfaces, not decorative overlays. The
    // associated analytic slopes feed the same material/caustic renderer.
    if look.topology.x > 0.0 {
        field = mix(field, tide_membrane(p, look_time, low, middle), look.topology.x);
    }
    if look.topology.y > 0.0 {
        let hollow = tide_sheet(p, vec2<f32>(0.10, -0.02), vec2<f32>(2.25, 2.10));
        field -= hollow * look.topology.y * (0.90 + low * 0.065);
    }
    if look.topology.z > 0.0 {
        let braid0 = tide_ribbon(p, -look_time * 0.073, 2.0, 0.38 + low * 0.018, -0.12, 3.8);
        let braid1 = tide_ribbon(p, 2.7 + look_time * 0.061, 1.75, 0.40 + middle * 0.025, 0.12, 4.2);
        field = mix(field, braid0 * 0.87 + braid1 * 0.79, look.topology.z);
    }
    if look.topology.w > 0.0 {
        field = mix(field, tide_cellular_glass(p, look_time), look.topology.w);
    }
    if look.special.x > 0.0 {
        field = mix(field, tide_shell(p, look_time) * (1.0 + low * 0.035), look.special.x);
    }
    if look.special.y > 0.0 {
        let leaf0 = tide_ribbon(p, 0.0 - look_time * 0.035, 0.72, 0.30, -0.54, 4.1);
        let leaf1 = tide_ribbon(p, 0.40 + look_time * 0.029, 0.85, 0.38, 0.04, 4.6);
        let leaf2 = tide_ribbon(p, 0.76 - look_time * 0.023, 1.03, 0.45, 0.56, 4.2);
        field = mix(field, leaf0 * 0.84 + leaf1 * (0.90 + middle * 0.025) + leaf2 * 0.78, look.special.y);
    }
    if look.special.z > 0.0 {
        let lamina0 = tide_ribbon(p, look_time * 0.035, 1.3, 0.18, -0.44, 5.0);
        let lamina1 = tide_ribbon(p, 0.8 - look_time * 0.027, 1.6, 0.17, 0.04, 5.4);
        let lamina2 = tide_ribbon(p, 1.9 + look_time * 0.023, 1.4, 0.22, 0.50, 5.0);
        field = mix(field, lamina0 * 0.82 + lamina1 * 0.88 + lamina2 * 0.77, look.special.z);
    }
    if look.special.w > 0.0 {
        field = mix(field, tide_cells(p, look_time, 1.0) * (1.0 + low * 0.03), look.special.w);
    }

    // A low-amplitude travelling fold changes actual surface level and slope.
    // Bass and mids reshape its relief; beats are reserved for illumination.
    let fold_frequency = vec2<f32>(2.3, -1.7);
    let fold_phase = dot(p, fold_frequency) + sin(p.y * 1.3 + time * 0.065) * 0.7
        - time * 0.12;
    let fold_strength = (0.045 + warp * 0.025 + low * 0.018 + middle * 0.020)
        * (1.0 + look.topology.x * 0.85);
    let fold_gradient = fold_frequency + vec2<f32>(0.0,
        cos(p.y * 1.3 + time * 0.065) * 0.91);
    field += vec3<f32>(sin(fold_phase), cos(fold_phase) * fold_gradient) * fold_strength;
    return field;
}

fn tide_render(p: vec2<f32>, field: vec3<f32>, look: TideLook) -> World {
    let time = u.a.z;
    let detail = detail_level();
    let warp = u.d.z;
    let low = spectrum_at(clamp(0.085 + sin(p.y * 1.3) * 0.055, 0.0, 0.15));
    let middle = spectrum_at(clamp(0.32 + sin(p.x * 1.2 + p.y * 0.4) * 0.12, 0.18, 0.47));
    let high = spectrum_at(clamp(0.73 + sin(p.x * 0.8 - p.y) * 0.18, 0.53, 0.98));
    let marble = tide_marble(p * look.caustic.y
        + tide_look_current(p * 1.7, time, look) * warp * 0.09, time);
    let gradient = field.yz;
    let gradient_length = max(length(gradient), 0.06);
    let edge_distance = (field.x - look.shape.y) / gradient_length;
    let body = smoothstep(-pixel() * 1.3, pixel() * 1.3, edge_distance);
    let height = sqrt(max(field.x - (look.shape.y - 0.04), 0.0));
    let normal = normalize(vec3<f32>(-gradient * 0.72 - marble.yz * detail * 0.045,
        0.30 + height * 0.62));
    let diffuse = max(dot(normal, normalize(vec3<f32>(-0.55, -0.35, 0.76))), 0.0);
    let fresnel = pow(1.0 - max(normal.z, 0.0), 2.4);
    let key = pow(max(dot(normal, normalize(vec3<f32>(-0.34, -0.46, 0.82))), 0.0), 24.0);
    let reflection = pow(max(dot(normal, normalize(vec3<f32>(0.53, 0.28, 0.80))), 0.0), 48.0);

    // Blue glass and muted violet interference in Cosmic become cream and
    // fine copper/ivory relief through the common Mocca material conversion.
    let interference = 0.5 + 0.5 * cos(height * 5.4 + normal.x * 2.0 - time * 0.075);
    let glass = mix(vec3<f32>(0.020, 0.145, 0.18),
        vec3<f32>(0.13, 0.055, 0.19), interference);
    let pearl = mix(vec3<f32>(0.35, 0.72, 0.72),
        vec3<f32>(0.69, 0.57, 0.88), interference * 0.65);
    let transmission = 0.48 + smoothstep(-0.9, 0.9, marble.x) * 0.52;
    let side_reflection = mix(pearl, vec3<f32>(0.68, 0.43, 0.24),
        smoothstep(0.20, 0.75, normal.x) * 0.85);
    var light = vec3<f32>(0.0025, 0.004, 0.009);
    light += glass * body * (0.15 + diffuse * 0.28 + height * 0.08)
        * (0.74 + mids() * 0.18 + low * 0.15) * transmission;
    light += pearl * body * key * (0.15 + air() * 0.10 + high * 0.25);
    var marks = (pearl * (key * 0.22 + fresnel * 0.16)
        + side_reflection * reflection * 0.17) * body
        * (0.62 + middle * 0.5 + high * 0.35 + beat() * 0.055);
    let edge = thread(edge_distance, pixel() * 0.68);
    marks += mix(pearl, vec3<f32>(0.80, 0.90, 0.91), 0.35) * edge
        * (0.22 + low * 0.20 + high * 0.23 + bass() * 0.10 + beat() * 0.035);

    // Capillary caustics sit in the moving material, not in a screen-space
    // grid. Normalized phase distance and a footprint gate keep fine lines
    // resolved as Detail increases; higher layers fade out before aliasing.
    for (var layer = 0; layer < 4; layer += 1) {
        let j = f32(layer);
        let frequency = (6.2 + j * (3.8 + detail * 6.0)) * look.caustic.x;
        let local_signal = spectrum_at(clamp((j + 0.5) * 0.25
            + sin(p.y * 0.9 + j) * 0.065, 0.0, 1.0));
        let bend_phase = p.y * 1.6 + time * 0.055 + j * 1.7;
        let topographic = 0.26 + j * 0.08 + sin(time * 0.031 + j) * 0.05;
        let caustic_field = field.x * topographic + marble.x * (0.46 - j * 0.045);
        let caustic_gradient = gradient * topographic + marble.yz * (0.46 - j * 0.045);
        let phase = caustic_field * frequency + p.x * (0.42 + j * 0.13)
            + sin(bend_phase) * (0.38 + local_signal * 0.30)
            - time * (0.13 + j * 0.025);
        let slope = caustic_gradient * frequency + vec2<f32>(0.42 + j * 0.13,
            cos(bend_phase) * (0.61 + local_signal * 0.48));
        let phase_gradient = max(length(slope), 0.45);
        let footprint = pixel() * phase_gradient * (1.0 + warp * 0.20);
        let resolved = 1.0 - smoothstep(0.65, 2.3, footprint);
        let contour_distance = sin(phase) / phase_gradient;
        let capillary = wire(contour_distance, pixel() * 0.50) * resolved;
        let detail_weight = select(0.10 + detail * 0.64,
            pow(detail, 1.0 + j * 0.35) * (0.62 - j * 0.065), layer > 0);
        let curvature_light = 0.28 + fresnel * 0.7 + diffuse * 0.24;
        marks += mix(glass * 1.8, pearl, 0.6) * capillary * body * look.caustic.z
            * detail_weight * curvature_light
            * (0.65 + local_signal * 1.2 + mids() * 0.15 + air() * 0.12);
    }
    // Relief has a quiet inner contour, not a bloom cloud or star overlay.
    let inner = wire((field.x - (look.shape.y + 0.19) - middle * 0.018) / gradient_length,
        pixel() * 0.65) * body;
    marks += pearl * inner * (0.08 + detail * 0.10 + middle * 0.16);
    return World(light, marks * (0.70 + u.d.y * 0.38));
}

fn tide(uv: vec2<f32>) -> World {
    let look = tide_look();
    let p = tide_surface_domain(uv, look);
    return tide_render(p, tide_surface_field(p, look), look);
}
