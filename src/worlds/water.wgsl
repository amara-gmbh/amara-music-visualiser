// Water Garden: an orthographic view directly down onto a continuous liquid
// surface. Every travelling packet begins with a real audio event recorded by
// Rust; time alone cannot manufacture a splash. The quiet surface stays flat.
// The layered spectrum is light refracting through glass, not an Apple mark,
// borrowed animation, texture, or a set of coloured contour lines.
//
// u.ripples[24] = centre.xy (centred world units), integrated age, strength.
// Slots 0..11 are current impacts; 12..23 are their short retiring tails.
// Analytic height derivatives shade each crest as a volume. Fixed loop bounds
// keep the cost independent of the number of beats or session duration.

struct WaterLook {
    domain: vec4<f32>, // x/y scale, angle, shear
    packet: vec4<f32>, // propagation speed, packet width, frequency, decay
    sources: vec4<f32>, // centre spread, second source, plane mixture, fine relief
    glass: vec4<f32>, // refraction, reflection spread, spectrum tilt, caustic gain
};

fn water_recipe(id: f32) -> WaterLook {
    switch u32(clamp(round(id), 0.0, 9.0)) {
        case 1u: { // Twin Springs: two overlapping circular wavefront families.
            return WaterLook(vec4<f32>(1.0, 1.0, 0.0, 0.0),
                vec4<f32>(0.62, 0.24, 20.0, 0.48),
                vec4<f32>(0.90, 0.82, 0.0, 0.45), vec4<f32>(1.0, 1.1, -0.25, 0.85));
        }
        case 2u: { // Silk Canal: long elliptical crests crossing the pool.
            return WaterLook(vec4<f32>(0.67, 1.35, 0.24, 0.18),
                vec4<f32>(0.72, 0.32, 13.5, 0.40),
                vec4<f32>(0.65, 0.12, 0.55, 0.30), vec4<f32>(1.15, 0.8, 0.45, 0.75));
        }
        case 3u: { // Rain Glass: tighter packets from dispersed musical impacts.
            return WaterLook(vec4<f32>(1.16, 1.16, -0.16, 0.0),
                vec4<f32>(0.75, 0.17, 29.0, 0.56),
                vec4<f32>(1.80, 0.0, 0.0, 0.90), vec4<f32>(0.90, 1.25, 0.20, 1.0));
        }
        case 4u: { // Tidal Silk: broad swells, very little fine engraving.
            return WaterLook(vec4<f32>(0.80, 0.88, -0.36, -0.10),
                vec4<f32>(0.56, 0.43, 10.5, 0.36),
                vec4<f32>(0.45, 0.16, 0.85, 0.18), vec4<f32>(1.45, 0.7, -0.40, 0.55));
        }
        case 5u: { // Prism Lagoon: connected circular lenses in layered colour.
            return WaterLook(vec4<f32>(0.97, 1.04, 0.32, 0.08),
                vec4<f32>(0.65, 0.29, 16.0, 0.43),
                vec4<f32>(1.20, 0.42, 0.20, 0.55), vec4<f32>(1.65, 1.0, 0.70, 1.05));
        }
        case 6u: { // Cross Current: two oblique, overlapping ripple systems.
            return WaterLook(vec4<f32>(1.05, 0.85, -0.48, 0.36),
                vec4<f32>(0.82, 0.24, 18.0, 0.46),
                vec4<f32>(1.25, 0.68, 0.65, 0.65), vec4<f32>(1.05, 0.85, -0.55, 0.90));
        }
        case 7u: { // Mercury Pool: compact embossed crests, broad reflection.
            return WaterLook(vec4<f32>(1.08, 1.08, 0.12, 0.0),
                vec4<f32>(0.60, 0.21, 24.0, 0.50),
                vec4<f32>(0.35, 0.10, 0.05, 0.38), vec4<f32>(0.75, 1.6, 0.05, 0.80));
        }
        case 8u: { // Longshore: open stretched ripples with travelling folds.
            return WaterLook(vec4<f32>(1.25, 0.66, 0.38, -0.28),
                vec4<f32>(0.69, 0.35, 14.0, 0.40),
                vec4<f32>(1.45, 0.24, 0.75, 0.40), vec4<f32>(1.20, 0.9, 0.40, 0.70));
        }
        case 9u: { // Crystal Rain: fine capillary interference around soft waves.
            return WaterLook(vec4<f32>(1.04, 0.98, -0.22, 0.12),
                vec4<f32>(0.73, 0.21, 26.0, 0.48),
                vec4<f32>(1.55, 0.40, 0.25, 1.0), vec4<f32>(1.30, 1.1, -0.30, 1.15));
        }
        default: { // Listening Pool: clean, broad, unmistakable water ripples.
            return WaterLook(vec4<f32>(1.0, 1.0, 0.0, 0.0),
                vec4<f32>(0.67, 0.29, 17.0, 0.43),
                vec4<f32>(0.70, 0.0, 0.15, 0.50), vec4<f32>(1.20, 1.0, 0.25, 0.90));
        }
    }
}

fn water_look() -> WaterLook {
    let state = u.looks[8];
    let a = water_recipe(state.x);
    let b = water_recipe(state.y);
    let t = clamp(state.z, 0.0, 1.0);
    return WaterLook(mix(a.domain, b.domain, t), mix(a.packet, b.packet, t),
        mix(a.sources, b.sources, t), mix(a.glass, b.glass, t));
}

// Smooth, non-cycling violet -> blue -> green -> yellow -> orange -> red.
// Soft boundaries remain coloured rather than turning into white stripes.
fn water_prism(position: f32) -> vec3<f32> {
    let stops = array<vec3<f32>, 6>(vec3<f32>(0.45, 0.10, 0.78),
        vec3<f32>(0.045, 0.37, 0.95), vec3<f32>(0.08, 0.69, 0.49),
        vec3<f32>(0.95, 0.76, 0.20), vec3<f32>(0.96, 0.34, 0.10),
        vec3<f32>(0.76, 0.075, 0.24));
    let x = clamp(position, 0.0, 1.0) * 5.0;
    let i = u32(floor(x));
    return mix(stops[i], stops[min(i + 1u, 5u)], smoothstep(0.0, 1.0, fract(x)));
}

// Returns height, its two spatial derivatives, and curvature. The Gaussian
// packet moves outwards; amplitude does not respawn a rigid ring every frame.
fn water_packet(p: vec2<f32>, centre: vec2<f32>, age: f32,
    strength: f32, recipe: vec4<f32>) -> vec4<f32> {
    let delta = p - centre;
    let radius = sqrt(dot(delta, delta) + 0.0025);
    let front = recipe.x * age;
    let width = recipe.y + age * 0.018;
    let distance = radius - front;
    let inverse_width = 1.0 / (width * width);
    let frequency = recipe.z;
    let phase = frequency * distance;
    let fade = exp(-age * recipe.w) * smoothstep(0.0, 0.16, age)
        * (1.0 - smoothstep(5.2, 6.0, age))
        / sqrt(1.0 + front * 2.1);
    let amplitude = strength * 0.043 * fade * exp(-distance * distance * inverse_width);
    let cosine = cos(phase);
    let sine = sin(phase);
    let envelope_slope = -2.0 * distance * inverse_width;
    let derivative = amplitude * (envelope_slope * cosine - frequency * sine);
    let second = amplitude * ((envelope_slope * envelope_slope
        - 2.0 * inverse_width - frequency * frequency) * cosine
        - 2.0 * envelope_slope * frequency * sine);
    let radial_ratio = 0.0025 / (radius * radius);
    let curvature = second * (1.0 - radial_ratio)
        + derivative * (1.0 + radial_ratio) / radius;
    return vec4<f32>(amplitude * cosine, delta / radius * derivative, curvature);
}

fn water_plane(p: vec2<f32>, direction: vec2<f32>, frequency: f32,
    phase: f32, amplitude: f32) -> vec4<f32> {
    let wave = dot(p, direction) * frequency + phase;
    let value = amplitude * sin(wave);
    return vec4<f32>(value, direction * (frequency * amplitude * cos(wave)),
        -value * frequency * frequency);
}

// Bounded soft knee makes quiet musical detail legible without hard gates,
// clipping or a fake minimum. Silence is exactly zero. Input was smoothed in
// Rust; only height changes with the music, never a wave's travelling phase.
fn water_signal(index: u32) -> f32 {
    let measured = max(u.water_bands[index / 4u][index % 4u], 0.0);
    // The broad bass swell has fewer spatial cycles than the upper bands,
    // hence much gentler reflection slopes. Bring its quiet detail into the
    // same visible range without raising its maximum height or adding crests.
    let sensitivity = select(4.2, 7.2, index == 0u);
    return 1.0 - exp(-measured * sensitivity);
}

// Sustained musical texture gathers along a broad, gently wandering current.
// This shapes actual water height, not the colour/material coverage: the sky
// reflections continue across the entire screen, including the calm areas.
// Return envelope, gradient and Laplacian for the exact product rule below.
fn water_current(p: vec2<f32>, time: f32) -> vec4<f32> {
    let normal = vec2<f32>(0.34, 0.9404254);
    let tangent = vec2<f32>(normal.y, -normal.x);
    let phase = dot(p, tangent) * 1.35 - time * 0.11;
    let distance = dot(p, normal) - 0.28 * sin(phase) - 0.14 * sin(time * 0.07);
    let gradient = normal - tangent * (0.28 * 1.35 * cos(phase));
    let laplacian = 0.28 * 1.35 * 1.35 * sin(phase);
    let inverse_width = 1.0 / (0.48 * 0.48);
    let envelope = exp(-distance * distance * inverse_width);
    let first = -2.0 * distance * inverse_width;
    let second = first * first - 2.0 * inverse_width;
    return vec4<f32>(envelope, envelope * first * gradient,
        envelope * (second * dot(gradient, gradient) + first * laplacian));
}

fn water_localise(surface: vec4<f32>, envelope: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(surface.x * envelope.x,
        surface.yz * envelope.x + surface.x * envelope.yz,
        surface.w * envelope.x + 2.0 * dot(surface.yz, envelope.yz)
            + surface.x * envelope.w);
}

fn water_surface(p: vec2<f32>, look: WaterLook) -> vec4<f32> {
    if u.raster.z <= 0.0 { return vec4<f32>(0.0); }
    var surface = vec4<f32>(0.0);
    for (var index = 0u; index < 24u; index += 1u) {
        let event = u.ripples[index];
        if event.w > 0.0001 && event.z >= 0.0 {
            // CPU keeps three separate four-event pools. Their spatial scales
            // make a low drum, a melodic onset and a hi-hat visibly different.
            // User mappings remain authoritative even for retained packets.
            var packet = look.packet;
            // Fewer distinct crests per impact, not a dimmer mass of fine
            // rings. Event age, position and its retiring tail stay intact.
            packet.z *= 0.72;
            packet.y *= 0.88;
            var strength = event.w;
            let band_slot = index % 12u;
            if band_slot < 4u {
                packet.y *= 1.25;
                packet.z *= 0.76;
                strength *= u.e.x;
            } else if band_slot < 8u {
                packet.y *= 0.86;
                packet.z *= 1.25;
                strength *= u.e.y * 0.95;
            } else {
                packet.y *= 0.66;
                packet.z *= 1.80;
                strength *= u.e.z * 0.60;
            }
            let centre = event.xy * look.sources.x;
            surface += water_packet(p, centre, event.z, strength, packet);
            if look.sources.y > 0.001 {
                // A secondary source is coupled to the SAME measured impact,
                // never a second synthetic beat. Sources merge continuously
                // as a look transforms, including between one/two-source pools.
                let opposite = -centre + vec2<f32>(0.42, -0.27);
                surface += water_packet(p, opposite, max(event.z - 0.12, 0.0),
                    strength * look.sources.y * 0.65, packet);
            }
        }
    }

    // Sustained energy has a physical job even between detected impacts.
    // All 48 measured rows contribute to eight compact frequency groups.
    // Mids make long folds; highs supply finer travelling capillaries. Fewer
    // spatial cycles leave readable waves instead of an all-over interference
    // texture. The broad current leaves calm colour around these live details.
    // Continuous phases never restart when a band gains or loses energy.
    let time = u.a.z;
    let low = water_signal(0u);
    var sustained = water_plane(p, vec2<f32>(0.9363, 0.3511), 3.4,
        -time * 1.4, low * (0.017 + look.sources.z * 0.0238));
    for (var index = 0u; index < 3u; index += 1u) {
        let i = f32(index);
        let frequency = 5.4 + i * 2.5;
        let angle = 2.18 + i * 1.381;
        let direction = vec2<f32>(cos(angle), sin(angle));
        // Mercury keeps its compact reflected crests, but no longer has
        // almost half the mid response simply because its plane mix is low.
        let amplitude = water_signal(index + 1u) * (0.036 + look.sources.z * 0.012);
        sustained += water_plane(p, direction, frequency,
            -time * (1.7 + i * 0.31) + i * 1.1, amplitude);
    }
    for (var index = 0u; index < 4u; index += 1u) {
        let i = f32(index);
        let frequency = 15.5 + i * (4.0 + detail_level() * 1.35);
        let angle = i * 1.381 + 0.27;
        let direction = vec2<f32>(cos(angle), sin(angle));
        let resolved = 1.0 - smoothstep(0.55, 1.8, pixel() * frequency);
        let amplitude = water_signal(index + 4u)
            * (0.0054 + look.sources.w * 0.0030) * resolved;
        sustained += water_plane(p, direction, frequency, -time * (1.9 + i * 0.47) + i,
            amplitude);
    }
    return surface + water_localise(sustained, water_current(p, time));
}

fn water_domain(uv: vec2<f32>, look: WaterLook) -> vec2<f32> {
    var p = rotate(uv * look.domain.xy, look.domain.z);
    p.x += p.y * look.domain.w;
    return p;
}

// Approximate a reflected strip over its pixel footprint. Strong upper-band
// slopes must not turn a thinner-than-a-pixel highlight into glittering noise.
// Preserve its light integral as the footprint widens, instead of clipping it.
fn water_strip(axis: f32, centre: f32, sharpness: f32) -> f32 {
    let footprint = fwidth(axis) * sharpness;
    let variance = 1.0 + 0.5 * footprint * footprint;
    let distance = (axis - centre) * sharpness;
    return exp(-distance * distance / variance) / sqrt(variance);
}

fn water_render(uv: vec2<f32>, look: WaterLook, surface: vec4<f32>) -> World {
    // Pull the normal through the SAME domain transform as the surface.
    let unsheared = vec2<f32>(surface.y, surface.z + surface.y * look.domain.w);
    let slope = clamp(rotate(unsheared, -look.domain.z) * look.domain.xy,
        vec2<f32>(-4.0), vec2<f32>(4.0));
    let normal = normalize(vec3<f32>(-slope * (0.93 + u.d.z * 0.13), 1.0));
    let view = vec3<f32>(0.0, 0.0, 1.0); // fixed top-down camera: no horizon or flight
    let reflection = reflect(-view, normal);
    let fresnel = pow(1.0 - normal.z, 2.0);
    let refraction = normal.xy * look.glass.x * 0.62 + surface.x * vec2<f32>(0.8, -0.5);
    let beneath = uv + refraction;

    // Two wide, overlapping refracted light sheets furnish the whole pond.
    // Both stand still when the surface is silent: no autonomous hue cycling.
    let spectrum_coordinate = 0.5 + beneath.x * 0.30
        + beneath.y * (0.15 + look.glass.z * 0.12);
    let transmitted = water_prism(spectrum_coordinate);
    let refracted = water_prism(0.60 - beneath.y * 0.25 + beneath.x * 0.10
        + normal.y * 0.48);
    let layer = smoothstep(-0.9, 1.3, beneath.x * 0.55 - beneath.y);
    let glass = mix(transmitted, refracted, layer * 0.38);
    let depth = 0.57 + 0.16 * tanh(surface.x * 8.0);
    let absorption = mix(vec3<f32>(0.018, 0.045, 0.095), glass * 0.34, depth);

    // Broad rectangular studio lights, reflected by the analytic normal.
    // Several oblique facets can light simultaneously; not a white ring mask.
    let key_axis = normal.x * 0.74 + normal.y * 0.67;
    let key = exp(-pow((key_axis - 0.21) * (8.0 / look.glass.y), 2.0));
    let softbox = exp(-pow((normal.x * -0.55 + normal.y * 0.83 + 0.28) * 7.0, 2.0));
    // Narrow strip lights reveal polished crests within the broader softbox.
    // These are reflected directions on the physical normal, not painted
    // topographic contours. Slightly separated coloured strips act as glass
    // dispersion while a crisp pearl core keeps the surface from looking matte.
    let sharp_key = water_strip(key_axis, 0.16, 36.0);
    let cross_key = water_strip(normal.x * -0.58 + normal.y * 0.81, -0.12, 44.0);
    let blue_edge = water_strip(key_axis, 0.19, 45.0);
    let warm_edge = water_strip(key_axis, 0.13, 45.0);
    let rim = pow(max(dot(reflection, normalize(vec3<f32>(-0.65, 0.35, 0.82))), 0.0), 28.0);
    let pearl = mix(vec3<f32>(0.82, 0.93, 1.0), refracted, 0.26);
    let shadow = 0.70 + 0.30 * max(dot(normal, normalize(vec3<f32>(-0.35, -0.45, 0.82))), 0.0);

    // Curvature focuses light below the waves. Bounded and smoothly filtered;
    // unlike a contour test this produces broad lens-like pools of light.
    let curvature = clamp(surface.w * look.glass.w, -64.0, 64.0);
    let caustic = smoothstep(0.3, 4.5, -curvature)
        * (0.55 + 0.45 * smoothstep(-0.6, 0.7, normal.x - normal.y));
    let trough = smoothstep(0.5, 7.0, curvature);
    var light = absorption * shadow + glass * 0.13;
    light *= 1.0 - trough * 0.46;
    light += transmitted * caustic * 0.19;
    var marks = pearl * key * 0.10 + refracted * softbox * 0.07
        + vec3<f32>(0.82, 0.91, 0.96) * rim * 0.22;
    marks += mix(water_prism(spectrum_coordinate + normal.y * 0.80),
        vec3<f32>(0.88, 0.97, 1.0), 0.50) * sharp_key * 0.95;
    marks += mix(refracted, vec3<f32>(0.83, 0.96, 1.0), 0.30) * cross_key * 0.56;
    marks += vec3<f32>(0.08, 0.32, 1.0) * blue_edge * 0.22
        + vec3<f32>(1.0, 0.18, 0.055) * warm_edge * 0.18;
    marks += water_prism(spectrum_coordinate + normal.x * 0.30) * fresnel * 0.65;
    marks += mix(glass, pearl, 0.32) * caustic * (0.055 + detail_level() * 0.065);
    return World(light, marks * (0.72 + u.d.y * 0.24));
}

fn water(uv: vec2<f32>) -> World {
    let look = water_look();
    let p = water_domain(uv, look);
    return water_render(uv, look, water_surface(p, look));
}
