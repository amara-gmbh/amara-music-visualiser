// Neon Memory draws new measured-spectrum ink, then the production ping-pong
// history carries that ink through a continuous velocity field. The image's
// history is real: this file does not substitute an analytic trail animation.
// The existing shared colour flight remains its environment.

// Names, in order: Afterglow River, Velvet Infinity, Split Current,
// Electric Weave, Magnetic Script, Neon Cadence, Helix Parade,
// Spectral Delta, Fresnel Garden, Prismatic Fugue.

fn neon_memory_flow_recipe(id: f32) -> vec4<f32> {
    let recipes = array<vec4<f32>, 10>(
        vec4<f32>(0.20, 0.42, 0.18, 0.65),
        vec4<f32>(-0.72, 0.92, 0.08, 0.22),
        vec4<f32>(0.12, 0.34, 0.76, -0.35),
        vec4<f32>(0.42, 0.54, -0.72, 0.15),
        vec4<f32>(-0.58, 0.80, 0.42, -0.22),
        vec4<f32>(0.08, 0.26, 0.90, 0.55),
        vec4<f32>(0.78, 0.38, -0.46, -0.18),
        vec4<f32>(-0.20, 0.94, 0.62, 0.32),
        vec4<f32>(0.52, 0.68, 0.38, -0.58),
        vec4<f32>(-0.68, 0.58, -0.66, 0.42));
    return recipes[u32(clamp(round(id), 0.0, 9.0))];
}

fn neon_memory_recipe() -> vec4<f32> {
    let look = u.looks[13];
    return mix(neon_memory_flow_recipe(look.x), neon_memory_flow_recipe(look.y),
        clamp(look.z, 0.0, 1.0));
}

// Every emitter has an analytic screen-space gradient. The distance estimate
// filters fine lines before they become subpixel, including during topology
// morphs. These are single ink strokes, never stacked contour terraces.
fn neon_memory_field(p: vec2<f32>, time: f32, id: f32, strand: f32) -> vec3<f32> {
    let lane = strand - 1.5;
    if id < 0.5 {
        let a = p.x * 1.70 + time * 0.34 + lane * 0.74;
        let b = p.x * 3.20 - time * 0.23 + lane * 1.13;
        return vec3<f32>(p.y - sin(a) * 0.34 - sin(b) * 0.12 - lane * 0.25,
            -cos(a) * 0.578 - cos(b) * 0.384, 1.0);
    }
    if id < 1.5 {
        let scale = 0.73 + lane * 0.09;
        let r2 = dot(p, p);
        let aa = scale * scale * 1.70;
        return vec3<f32>(r2 * r2 - aa * (p.x * p.x - p.y * p.y) - 0.075,
            p.x * (4.0 * r2 - 2.0 * aa), p.y * (4.0 * r2 + 2.0 * aa));
    }
    if id < 2.5 {
        let a = p.x * 2.15 - time * 0.26 + lane;
        return vec3<f32>(p.y * p.y - p.x * 0.43 - sin(a) * 0.12 - lane * 0.16,
            -0.43 - cos(a) * 0.258, p.y * 2.0);
    }
    if id < 3.5 {
        let a = p.x * 1.65 + time * 0.19;
        let b = p.y * 2.05 - time * 0.23;
        return vec3<f32>(sin(a) * 0.56 + sin(b) * 0.56 - lane * 0.26,
            cos(a) * 0.924, cos(b) * 1.148);
    }
    if id < 4.5 {
        let a = p.x * 2.4 + time * 0.25 + lane * 0.57;
        return vec3<f32>(p.x * p.y + sin(a) * 0.17 - lane * 0.18,
            p.y + cos(a) * 0.408, p.x);
    }
    if id < 5.5 {
        let a = p.x * 1.20 - time * 0.21 + lane * 0.38;
        let b = p.x * 4.3 + time * 0.27 + lane * 0.81;
        let s = sin(a);
        return vec3<f32>(p.y - s * s * s * 0.42 - sin(b) * 0.095 - lane * 0.28,
            -s * s * cos(a) * 1.512 - cos(b) * 0.4085, 1.0);
    }
    if id < 6.5 {
        let a = p.y * 2.10 + time * 0.31 + strand * 1.5707963;
        return vec3<f32>(p.x - sin(a) * 0.52 - lane * 0.15,
            1.0, -cos(a) * 1.092);
    }
    if id < 7.5 {
        let a = p.x * 2.8 - time * 0.24 + lane * 0.55;
        return vec3<f32>(p.y * p.y * p.y - p.y * 0.58 - p.x * 0.32
                - sin(a) * 0.12 - lane * 0.12,
            -0.32 - cos(a) * 0.336, p.y * p.y * 3.0 - 0.58);
    }
    if id < 8.5 {
        let a = p.x * p.x * 1.75 - time * 0.32 + lane * 0.91;
        return vec3<f32>(p.y - sin(a) * 0.38 - lane * 0.25,
            -cos(a) * p.x * 1.33, 1.0);
    }
    let a = p.x * 1.8 + p.y * 0.8 + time * 0.20;
    let b = -p.x * 0.7 + p.y * 2.2 - time * 0.17;
    let c = p.x * 3.1 - p.y * 1.2 + time * 0.23 + lane * 0.6;
    return vec3<f32>(sin(a) * 0.48 + sin(b) * 0.48 + sin(c) * 0.16 - lane * 0.28,
        cos(a) * 0.864 - cos(b) * 0.336 + cos(c) * 0.496,
        cos(a) * 0.384 + cos(b) * 1.056 - cos(c) * 0.192);
}

fn neon_memory_pigment(position: f32, time: f32) -> vec3<f32> {
    let phase = position * 0.78 + time * 0.011;
    let wave = vec3<f32>(0.5) + 0.5 * cos(vec3<f32>(phase, phase + 0.34, phase + 0.67) * 6.2831853);
    let minimum = min(wave.x, min(wave.y, wave.z));
    let pigment = max(wave - vec3<f32>(minimum * 0.84), vec3<f32>(0.0));
    return vec3<f32>(0.015) + pigment * 0.95;
}

fn neon_memory(uv: vec2<f32>) -> World {
    let time = u.a.z;
    let look = u.looks[13];
    // Every visible stroke shares the measured low-bin body. A local spectral
    // window may miss those bins when its curve leaves the view (Split Current
    // in particular), so use the already-mapped eight-bin mean as a floor.
    // Max avoids counting the scalar bass envelope and FFT twice.
    let low_spectrum = dot(u.spectrum[0] + u.spectrum[1], vec4<f32>(0.125));
    let low = clamp(max(bass(), low_spectrum), 0.0, 1.5);
    let middle = clamp(mids(), 0.0, 1.5);
    let high = clamp(air(), 0.0, 1.5);
    let pulse = clamp(beat(), 0.0, 1.5);
    let detail = detail_level();
    // Broad music changes ink shape, never the accumulated camera clock.
    // The field's bank is continuous through every look/world transition.
    let angle = sin(time * 0.093) * 0.28;
    let q = rotate(uv, angle) / (0.90 + low * 0.12);
    var ink = vec3<f32>(0.0);
    var soft = vec3<f32>(0.0);
    for (var strand = 0; strand < 4; strand += 1) {
        let i = f32(strand);
        let along = 0.5 + 0.48 * sin(q.x * 0.64 + q.y * 0.27 + i * 0.51);
        // Each stroke reads a different spectral window, while all four
        // together expose low, middle and high-frequency changes spatially.
        let band_position = mix(i * 0.23, min(i * 0.23 + 0.30, 1.0), along);
        let signal = clamp(spectrum_at(band_position), 0.0, 1.5);
        let bend = sin(q.x * 2.0 + i * 0.8 - time * 0.18);
        let p = q + vec2<f32>(0.0, bend * middle * 0.10 + signal * 0.13);
        var field = neon_memory_field(p, time, look.y, i);
        if look.z < 1.0 && look.x != look.y {
            field = mix(neon_memory_field(p, time, look.x, i), field,
                clamp(look.z, 0.0, 1.0));
        }
        let gradient = max(length(field.yz), 0.25);
        let distance = abs(field.x) / gradient;
        let width = max(pixel() * 0.95, 0.010 + signal * 0.010 + low * 0.004 + pulse * 0.005);
        let core = 1.0 - smoothstep(width * 0.32, width * 1.60, distance);
        let ribbon = exp(-distance * distance / max(width * width * 13.0, 0.00001));
        let edge_distance = abs(distance - width * 1.7);
        let edge = 1.0 - smoothstep(pixel() * 0.65, pixel() * 1.9 + 0.002, edge_distance);
        let audible = smoothstep(0.004, 0.045, signal + low * 0.08 + middle * 0.12 + high * 0.10 + pulse * 0.10);
        let strength = audible * (0.13 + signal * 0.95
            + low * 0.07 + middle * 0.10 + high * 0.07 + pulse * 0.20);
        let grain_phase = (p.x * 0.68 + p.y * 0.32) * (28.0 + detail * 54.0)
            - time * 1.2 + i * 1.73;
        let grain = 0.65 + 0.35 * filtered_sine(grain_phase,
            pixel() * (28.0 + detail * 54.0));
        let pigment = neon_memory_pigment(band_position + i * 0.12, time);
        // Keep broad body and crisp high-frequency threads coloured. No white
        // crown or additive full-screen flash is used when many strokes cross.
        ink = max(ink, pigment * strength * (core * 0.85
            + edge * high * 0.34 * grain));
        soft += pigment * ribbon * strength * 0.12;
    }
    // Quiet input stops injection; previous musical ink remains free to decay.
    // The normal shared colour environment supplies quiet visual continuity.
    let fresh = clamp(u.raster.z, 0.0, 1.0);
    return World(soft * fresh, ink * fresh * (0.78 + u.d.y * 0.35));
}

fn neon_memory_velocity(p: vec2<f32>, time: f32, recipe: vec4<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    let forward = 0.12 + sin(time * 0.081) * 0.027;
    let bank = sin(time * 0.109) * 0.19 + recipe.x * 0.20;
    var velocity = p * forward + vec2<f32>(-p.y, p.x) * bank / (0.8 + r2 * 0.36);
    let a = p - vec2<f32>(-0.53 + sin(time * 0.067) * 0.20, 0.16);
    let b = p - vec2<f32>(0.58, -0.20 + cos(time * 0.079) * 0.18);
    let eddies = vec2<f32>(-a.y, a.x) * exp(-dot(a, a) * 0.90)
        - vec2<f32>(-b.y, b.x) * exp(-dot(b, b) * 0.90);
    velocity += eddies * recipe.y * 0.46;
    velocity += vec2<f32>(sin(p.y * 1.6 + time * 0.13),
        sin(p.x * 1.5 - time * 0.11)) * recipe.z * 0.11;
    velocity += vec2<f32>(cos(time * 0.077), sin(time * 0.063)) * recipe.w * 0.06;
    return velocity;
}

fn neon_memory_flow(uv: vec2<f32>) -> vec2<f32> {
    let time = u.a.z;
    let advance = clamp(u.g.y, 0.0, 0.1) * clamp(u.c.z * u.i.y, 0.0, 8.0);
    let recipe = neon_memory_recipe();
    // Midpoint back-tracing transports the actual stored image. Positive
    // radial velocity moves the retained marks toward the viewer; two unequal
    // eddies curl them in opposite directions instead of spinning a flat layer.
    let velocity = neon_memory_velocity(uv, time, recipe);
    let midpoint = uv - velocity * advance * 0.5;
    return uv - neon_memory_velocity(midpoint, time, recipe) * advance;
}

fn neon_memory_memory() -> f32 {
    let echo = clamp(u.g.x, 0.0, 0.92);
    let connected = max(max(u.e.x, u.e.y), max(u.e.z, u.e.w));
    if echo <= 0.001 || connected <= 0.0 || u.raster.z < 0.5 { return 0.0; }
    // The ordinary 0.22 Echo setting now retains a phrase (~2.1 s nominal
    // lifetime), not merely a few frames. Echo0 still follows the exact off
    // path; the common compositor retains bounded, non-additive exposure.
    let effective_echo = 0.90 * (1.0 - exp(-echo * 14.0));
    return effective_echo / echo;
}
