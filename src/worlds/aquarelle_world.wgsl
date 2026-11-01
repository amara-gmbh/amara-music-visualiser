// Aquarelle: translucent, travelling pigment on luminous paper. Three
// overlapping washes have independent depth and colour. Bass bends the liquid
// flow, mid frequencies shape the pigment's branching edges, and high bands
// bring out fine wet-on-paper detail. There are no sampled images, no hard
// radial rings, and no random jumps at sheet wrap: each sheet fades to zero
// before its depth cycles.
//
// The caller owns the look lane: x = previous recipe, y = next recipe,
// z = smooth blend. Recipes interpolate BEFORE the field is evaluated.
struct AquarelleLook {
    domain: vec4<f32>, // scale x/y, angle, shear
    movement: vec4<f32>, // curl, elongation, flow character, fine flow
    pigment: vec4<f32>, // hue, hue separation, density, paper warmth
    form: vec4<f32>, // ribbons, blooms, marbling, edge definition
};

fn aquarelle_recipe(id: f32) -> AquarelleLook {
    switch u32(clamp(round(id), 0.0, 9.0)) {
        case 1u: { // Rose Tides: long coral and violet brush currents.
            return AquarelleLook(vec4<f32>(0.72, 1.45, -0.30, 0.28),
                vec4<f32>(0.42, 1.00, 1.08, 0.46), vec4<f32>(0.13, 0.22, 0.88, 0.45),
                vec4<f32>(0.86, 0.20, 0.40, 0.72));
        }
        case 2u: { // Lagoon Petals: blue-green blooms over golden paper.
            return AquarelleLook(vec4<f32>(1.18, 0.92, 0.15, -0.08),
                vec4<f32>(0.68, 0.22, 0.72, 0.62), vec4<f32>(0.48, 0.16, 0.83, 0.77),
                vec4<f32>(0.10, 0.92, 0.18, 0.78));
        }
        case 3u: { // Neon Ink: fine branching pigment over saturated cool wash.
            return AquarelleLook(vec4<f32>(1.52, 1.05, -0.20, 0.18),
                vec4<f32>(0.54, 0.48, 0.92, 0.82), vec4<f32>(0.70, 0.28, 0.93, 0.18),
                vec4<f32>(0.38, 0.18, 0.96, 1.16));
        }
        case 4u: { // Sunset Silk: broad terracotta and lilac strokes.
            return AquarelleLook(vec4<f32>(0.64, 1.72, 0.48, -0.24),
                vec4<f32>(0.31, 1.14, 0.78, 0.37), vec4<f32>(0.01, 0.19, 0.94, 0.88),
                vec4<f32>(1.00, 0.12, 0.30, 0.52));
        }
        case 5u: { // Floating Gardens: soft, overlapping irregular blossoms.
            return AquarelleLook(vec4<f32>(0.88, 0.88, 0.12, 0.12),
                vec4<f32>(0.82, 0.12, 0.64, 0.54), vec4<f32>(0.30, 0.25, 0.78, 0.58),
                vec4<f32>(0.08, 1.00, 0.28, 0.87));
        }
        case 6u: { // Disco Marbling: twisted threads of warm and cool pigment.
            return AquarelleLook(vec4<f32>(1.18, 1.30, -0.55, 0.38),
                vec4<f32>(0.91, 0.84, 1.18, 0.95), vec4<f32>(0.86, 0.32, 0.92, 0.32),
                vec4<f32>(0.66, 0.22, 1.00, 1.05));
        }
        case 7u: { // Misty Irises: pale translucent violet and turquoise veils.
            return AquarelleLook(vec4<f32>(0.76, 1.12, 0.36, 0.14),
                vec4<f32>(0.52, 0.45, 0.66, 0.42), vec4<f32>(0.62, 0.21, 0.66, 0.08),
                vec4<f32>(0.28, 0.66, 0.35, 0.60));
        }
        case 8u: { // Electric Saffron: granular, diagonal red-gold splashes.
            return AquarelleLook(vec4<f32>(1.46, 0.72, -0.42, -0.31),
                vec4<f32>(0.59, 0.72, 1.12, 0.88), vec4<f32>(0.95, 0.26, 1.00, 0.85),
                vec4<f32>(0.62, 0.54, 0.74, 1.20));
        }
        case 9u: { // Prismatic Rain: crossing washes and crisp fine capillaries.
            return AquarelleLook(vec4<f32>(1.15, 1.35, 0.60, 0.36),
                vec4<f32>(0.47, 1.02, 0.96, 0.91), vec4<f32>(0.37, 0.34, 0.91, 0.24),
                vec4<f32>(0.82, 0.34, 0.78, 1.27));
        }
        default: { // Chromatic Wash: generous multi-colour watercolour layers.
            return AquarelleLook(vec4<f32>(1.00, 1.00, 0.0, 0.08),
                vec4<f32>(0.56, 0.46, 0.86, 0.60), vec4<f32>(0.0, 0.25, 0.86, 0.42),
                vec4<f32>(0.36, 0.54, 0.58, 0.82));
        }
    }
}

fn aquarelle_look(state: vec4<f32>) -> AquarelleLook {
    let a = aquarelle_recipe(state.x);
    let b = aquarelle_recipe(state.y);
    let blend = clamp(state.z, 0.0, 1.0);
    return AquarelleLook(mix(a.domain, b.domain, blend),
        mix(a.movement, b.movement, blend), mix(a.pigment, b.pigment, blend),
        mix(a.form, b.form, blend));
}

fn aquarelle_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let part = fract(p);
    let f = part * part * (3.0 - 2.0 * part);
    let a = hash(vec3<f32>(cell, 2.81));
    let b = hash(vec3<f32>(cell + vec2<f32>(1.0, 0.0), 2.81));
    let c = hash(vec3<f32>(cell + vec2<f32>(0.0, 1.0), 2.81));
    let d = hash(vec3<f32>(cell + vec2<f32>(1.0, 1.0), 2.81));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

fn aquarelle_cloud(p: vec2<f32>) -> f32 {
    let a = aquarelle_noise(p);
    let b = aquarelle_noise(rotate(p * 2.08, 0.42) + vec2<f32>(12.1, 7.3));
    let c = aquarelle_noise(rotate(p * 4.16, -0.31) + vec2<f32>(4.7, 18.2));
    return a * 0.57 + b * 0.29 + c * 0.14;
}

fn aquarelle_pigment(phase: f32) -> vec3<f32> {
    let paints = array<vec3<f32>, 7>(
        vec3<f32>(0.94, 0.17, 0.31), vec3<f32>(0.95, 0.43, 0.12),
        vec3<f32>(0.89, 0.69, 0.16), vec3<f32>(0.10, 0.70, 0.56),
        vec3<f32>(0.04, 0.57, 0.84), vec3<f32>(0.32, 0.27, 0.83),
        vec3<f32>(0.79, 0.19, 0.67));
    let position = fract(phase) * 7.0;
    let index = u32(floor(position)) % 7u;
    return mix(paints[index], paints[(index + 1u) % 7u],
        smoothstep(0.0, 1.0, fract(position)));
}

fn aquarelle_world(uv: vec2<f32>, look_state: vec4<f32>) -> World {
    let look = aquarelle_look(look_state);
    let time = u.a.z;
    let live = select(0.0, 1.0, u.raster.z > 0.0);
    let low = (1.0 - exp(-max(bass(), 0.0) * 1.65)) * live;
    let middle = (1.0 - exp(-max(mids(), 0.0) * 1.70)) * live;
    let high = (1.0 - exp(-max(air(), 0.0) * 1.70)) * live;
    // Never multiply the accumulated clock by an interpolated look parameter:
    // changing looks would then rewind or accelerate every phase at once.
    let t = time;
    let scale = 1.0 + 0.12 * sin(t * 0.075);
    var view = rotate(uv * look.domain.xy * scale, look.domain.z
        + sin(t * 0.045) * 0.12);
    view.x += view.y * look.domain.w;
    // Smooth change of heading makes the camera swim through pigment instead
    // of zooming rigidly through a repeating tunnel.
    let drift = vec2<f32>(t * 0.055 + sin(t * 0.042) * 0.40,
        -t * 0.037 + cos(t * 0.051) * 0.36);
    let paper = mix(vec3<f32>(0.54, 0.63, 0.73), vec3<f32>(0.78, 0.63, 0.48),
        look.pigment.w);
    var colour = paper * 0.64;
    var detail = vec3<f32>(0.0);
    for (var layer = 0u; layer < 3u; layer += 1u) {
        let layer_id = f32(layer);
        let depth = fract(t * 0.025 + layer_id / 3.0);
        let visible = smoothstep(0.0, 0.20, depth)
            * (1.0 - smoothstep(0.76, 1.0, depth));
        let magnification = mix(1.80, 0.68, depth);
        var p = rotate(view * magnification + drift * (0.38 + layer_id * 0.13),
            layer_id * 0.83 + sin(t * 0.033 + layer_id) * 0.13);
        let curl = vec2<f32>(sin(p.y * 1.45 + t * 0.12 + layer_id),
            cos(p.x * 1.27 - t * 0.095 - layer_id));
        p += curl * (0.23 + look.movement.x * 0.30
            + (look.movement.z - 0.86) * 0.08 + low * 0.18);
        let turbulence = aquarelle_cloud(p * 1.35 + vec2<f32>(layer_id * 7.9, t * 0.015));
        let branch = aquarelle_cloud(p * 2.15 + vec2<f32>(turbulence * 1.4, layer_id * 6.3));
        let ribbon = 0.5 + 0.5 * sin(p.y * (2.1 + look.movement.y * 2.4)
            + (turbulence - 0.5) * (4.2 + middle * 2.5) - t * 0.12);
        let blossom = 1.0 - abs(turbulence * 2.0 - 1.0);
        var field = turbulence * 0.60 + branch * 0.40;
        field = mix(field, field * 0.58 + ribbon * 0.42, look.form.x * 0.55);
        field = mix(field, field * 0.62 + blossom * 0.38, look.form.y * 0.58);
        field += (branch - 0.5) * look.form.z * (0.18 + middle * 0.22);
        // The noise naturally clusters close to 0.5; spreading that range
        // reveals islands of pigment instead of a screen-wide blurred wash.
        field = (field - 0.5) * 1.75 + 0.5;
        let feather = 0.035 + (1.0 - look.pigment.z) * 0.035;
        let body = smoothstep(0.48 - feather, 0.48 + feather, field);
        let edge_distance = abs(field - 0.48);
        let edge_width = max(0.014 + (1.0 - look.form.w * 0.5) * 0.008,
            pixel() * magnification * 1.6);
        let wet_edge = 1.0 - smoothstep(edge_width, edge_width * 3.0, edge_distance);
        // A second soft tide mark gives the watercolour the irregular pooling
        // of pigment at its drying edge, without introducing hard geometry.
        let tide_edge = 1.0 - smoothstep(edge_width * 0.8, edge_width * 2.7,
            abs(field - 0.61));
        let local_mid = spectrum_at(0.20 + layer_id * 0.13) * live;
        let local_high = spectrum_at(0.63 + layer_id * 0.14) * live;
        let paint = aquarelle_pigment(look.pigment.x + layer_id * look.pigment.y
            + t * 0.008 + turbulence * 0.13);
        let strength = visible * look.pigment.z * (0.09 + body * 0.77);
        let translucent_paint = mix(colour * (0.48 + body * 0.12),
            paint * (0.84 + body * 0.22), 0.70);
        colour = mix(colour, translucent_paint, strength);
        // Pigment pooling outlines, not white electric wires. Its fine
        // stippling is damped at low detail or small pixel footprint.
        let fine = aquarelle_noise(p * (6.0 + look.movement.w * 3.0) + layer_id * 9.1);
        let resolved = 1.0 - smoothstep(0.03, 0.11, pixel() * magnification * 8.0);
        let capillary = pow(max(0.0, 1.0 - abs(branch - 0.5) * 8.0), 2.0)
            * look.form.z * resolved;
        let pooling = visible * (wet_edge * 0.32 + tide_edge * body * 0.14)
            * (0.74 + fine * 0.26);
        colour *= 1.0 - clamp(pooling, 0.0, 0.42);
        detail += paint * visible * (wet_edge * (0.12 + local_mid * 0.15)
            + tide_edge * body * 0.09
            + capillary * (0.035 + high * 0.10 + local_high * 0.12))
            * (0.48 + fine * 0.52);
    }
    // Fine, stable paper tooth. It is fixed in screen space and never flashes
    // as the music changes; the coloured liquid itself provides motion.
    let grain = aquarelle_noise(uv * 140.0 + vec2<f32>(31.7, 8.2));
    colour *= 0.97 + grain * 0.06;
    let bright = max(colour, vec3<f32>(0.0));
    return World(bright * 0.20, bright * 0.80 + detail);
}
