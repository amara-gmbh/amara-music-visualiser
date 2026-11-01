// Liquid Stained Glass: travel through three persistent sheets of coloured
// optical cells. The cells are world-space Voronoi lenses, not a radial overlay
// or a kaleidoscope. Domain advection keeps their glass boundaries flowing;
// mapped bass deforms the sheets, mids shape the channels inside each lens,
// and local upper FFT bins illuminate their fine, anti-aliased edges.
//
// Three sheets x nine neighbouring cells: fixed work, no raymarch or history
// dependency. Plane births/deaths are exactly invisible at the depth wrap.

struct LiquidGlassLook {
    domain: vec4<f32>, // x/y cell scale, orientation, shear
    cells: vec4<f32>, // site disorder, stagger, angular metric, lens curvature
    current: vec4<f32>, // broad fold, secondary curl, channel frequency, twist
    optics: vec4<f32>, // bevel width, transmission, chromatic relief, colour phase
};

fn liquid_glass_recipe(id: f32) -> LiquidGlassLook {
    switch u32(clamp(round(id), 0.0, 9.0)) {
        case 1u: { // Honey Current: ordered, rounded, staggered honeycomb lenses.
            return LiquidGlassLook(vec4<f32>(1.30, 1.10, 0.12, 0.0),
                vec4<f32>(0.16, 1.0, 0.0, 1.10), vec4<f32>(0.15, 0.16, 5.4, 0.08),
                vec4<f32>(0.030, 0.60, 0.85, 0.14));
        }
        case 2u: { // Molten Petals: large curled cells with deep liquid shoulders.
            return LiquidGlassLook(vec4<f32>(0.72, 0.85, -0.28, 0.24),
                vec4<f32>(0.92, 0.18, 0.0, 1.55), vec4<f32>(0.60, 0.62, 3.5, 0.44),
                vec4<f32>(0.070, 0.72, 1.30, 0.31));
        }
        case 3u: { // Ribbon Prism: long diagonal strips of softly bending glass.
            return LiquidGlassLook(vec4<f32>(0.58, 2.30, -0.52, 0.32),
                vec4<f32>(0.60, 0.12, 0.10, 0.72), vec4<f32>(0.40, 0.23, 8.0, 0.17),
                vec4<f32>(0.037, 0.58, 1.05, 0.57));
        }
        case 4u: { // Deep Mosaic: small faceted panes over a deep coloured vault.
            return LiquidGlassLook(vec4<f32>(1.75, 1.52, 0.30, -0.13),
                vec4<f32>(0.72, 0.46, 0.42, 0.60), vec4<f32>(0.23, 0.25, 7.4, -0.18),
                vec4<f32>(0.028, 0.50, 0.90, 0.75));
        }
        case 5u: { // Copper Lagoon: broad, reclining pools with thick warm rims.
            return LiquidGlassLook(vec4<f32>(1.00, 0.56, 0.22, 0.17),
                vec4<f32>(0.84, 0.10, 0.0, 1.32), vec4<f32>(0.48, 0.32, 3.1, -0.22),
                vec4<f32>(0.085, 0.77, 0.70, 0.92));
        }
        case 6u: { // Crystal Channels: narrow bright tributaries through clear panes.
            return LiquidGlassLook(vec4<f32>(1.30, 0.96, -0.13, 0.52),
                vec4<f32>(0.92, 0.0, 0.20, 0.92), vec4<f32>(0.35, 0.58, 11.0, 0.28),
                vec4<f32>(0.022, 0.46, 1.40, 0.44));
        }
        case 7u: { // Velvet Lenses: plush, bulging optical cells and broad reflection.
            return LiquidGlassLook(vec4<f32>(0.88, 0.83, 0.40, -0.14),
                vec4<f32>(0.40, 0.55, 0.0, 1.85), vec4<f32>(0.24, 0.19, 4.2, 0.12),
                vec4<f32>(0.105, 0.66, 1.60, 0.64));
        }
        case 8u: { // Broken Spectrum: angular elongated facets, without hard cuts.
            return LiquidGlassLook(vec4<f32>(1.65, 0.90, -0.46, -0.42),
                vec4<f32>(1.0, 0.08, 0.85, 0.50), vec4<f32>(0.17, 0.20, 6.2, -0.30),
                vec4<f32>(0.036, 0.68, 1.15, 0.08));
        }
        case 9u: { // Opal Drift: overlapping large clear lenses with luminous seams.
            return LiquidGlassLook(vec4<f32>(0.63, 0.67, 0.07, 0.10),
                vec4<f32>(0.58, 0.30, 0.0, 1.30), vec4<f32>(0.43, 0.45, 4.7, 0.35),
                vec4<f32>(0.045, 0.40, 1.75, 0.25));
        }
        default: { // Prismatic Passage: saturated, irregular glass cells in depth.
            return LiquidGlassLook(vec4<f32>(1.04, 1.0, 0.0, 0.06),
                vec4<f32>(0.75, 0.25, 0.0, 1.05), vec4<f32>(0.32, 0.38, 5.8, 0.20),
                vec4<f32>(0.052, 0.64, 1.15, 0.0));
        }
    }
}

fn liquid_glass_look() -> LiquidGlassLook {
    let state = u.looks[11];
    let a = liquid_glass_recipe(state.x);
    let b = liquid_glass_recipe(state.y);
    let blend = clamp(state.z, 0.0, 1.0);
    return LiquidGlassLook(mix(a.domain, b.domain, blend),
        mix(a.cells, b.cells, blend), mix(a.current, b.current, blend),
        mix(a.optics, b.optics, blend));
}

fn liquid_glass_colour(phase: f32) -> vec3<f32> {
    // A closed jewel-colour path. Adjacent cells retain different pigments;
    // gradual travel changes that material even in a manually held look.
    let colours = array<vec3<f32>, 6>(vec3<f32>(0.025, 0.56, 0.90),
        vec3<f32>(0.28, 0.035, 0.80), vec3<f32>(0.91, 0.055, 0.43),
        vec3<f32>(0.94, 0.30, 0.055), vec3<f32>(0.83, 0.67, 0.095),
        vec3<f32>(0.035, 0.72, 0.48));
    let position = fract(phase) * 6.0;
    let index = u32(floor(position)) % 6u;
    return mix(colours[index], colours[(index + 1u) % 6u],
        smoothstep(0.0, 1.0, fract(position)));
}

struct LiquidGlassCell {
    delta: vec2<f32>,
    edge: f32,
    seed: f32,
};

fn liquid_glass_cell(p: vec2<f32>, layer: f32, look: LiquidGlassLook) -> LiquidGlassCell {
    let tile = floor(p);
    var first = 100.0;
    var second = 100.0;
    var nearest = vec2<f32>(0.0);
    var colour_seed = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let cell = tile + vec2<f32>(f32(x), f32(y));
            let seeds = vec2<f32>(hash(vec3<f32>(cell, layer * 7.31 + 3.7)),
                hash(vec3<f32>(cell, layer * 9.17 + 19.4)));
            let stagger = select(-0.15, 0.15, (i32(cell.y) & 1) == 0) * look.cells.y;
            let site = cell + vec2<f32>(0.5 + stagger, 0.5)
                + (seeds - 0.5) * (0.65 * look.cells.x);
            let delta = p - site;
            let taxi = abs(delta.x) + abs(delta.y);
            let distance = mix(dot(delta, delta), taxi * taxi * 0.62, look.cells.z);
            if distance < first {
                second = first;
                first = distance;
                nearest = delta;
                colour_seed = seeds.x * 0.73 + seeds.y * 0.27;
            } else {
                second = min(second, distance);
            }
        }
    }
    return LiquidGlassCell(nearest,
        max(sqrt(second) - sqrt(first), 0.0) * 0.5, colour_seed);
}

fn liquid_stained_glass(uv: vec2<f32>) -> World {
    let look = liquid_glass_look();
    let time = u.a.z;
    let live = select(0.0, 1.0, u.raster.z > 0.0);
    let low_fft = (spectrum_at(0.02) + spectrum_at(0.075) + spectrum_at(0.13)) / 3.0;
    let low = (1.0 - exp(-max(bass() * 0.70 + low_fft, 0.0) * 1.8)) * live;
    let middle = (1.0 - exp(-max(mids(), 0.0) * 1.9)) * live;
    let high = (1.0 - exp(-max(air(), 0.0) * 1.8)) * live;
    let pulse = clamp(beat(), 0.0, 1.5) * live;
    let heading = sin(time * 0.063) * 0.25 + sin(time * 0.097 + 0.8) * 0.13;
    let turn = rotate(uv, heading);
    let travel = time * 0.82;
    let spacing = 3.8;
    let first_layer = floor(travel / spacing);
    let camera = vec2<f32>(sin(time * 0.093) * 0.50, cos(time * 0.071) * 0.45);
    let distant_colour = liquid_glass_colour(time * 0.011 + uv.y * 0.09);
    var body = vec3<f32>(0.0);
    var seams = vec3<f32>(0.0);
    var transmission = 1.0;
    var refraction = vec2<f32>(0.0);
    // Near-to-far optical panes. Each lens bends the ray used to evaluate the
    // next sheet, so farther seams really distort through the nearer glass.
    // This reuses the same three cell searches; no second field evaluation.
    // Both opacity AND refraction vanish at a sheet's entry/exit boundary.
    for (var pane = 0; pane < 3; pane += 1) {
        let identity = first_layer + 1.0 + f32(pane);
        let depth = identity * spacing - travel;
        let visible = smoothstep(0.35, 1.05, depth)
            * (1.0 - smoothstep(9.5, 11.4, depth));
        let near = 1.0 - smoothstep(1.2, 9.0, depth);
        let radius = dot(turn, turn);
        let curved = rotate(turn, look.current.w * radius / (1.0 + radius));
        let domain_angle = look.domain.z + sin(identity * 2.17) * 0.30;
        var p = rotate((curved + refraction) * (depth * 0.36) + camera, domain_angle);
        p = vec2<f32>(p.x + look.domain.w * p.y, p.y) * look.domain.xy;
        let phase = time * 0.19 + identity * 1.79;
        let flow = vec2<f32>(sin(p.y * 1.47 + phase), cos(p.x * 1.39 - phase * 0.73));
        let twist = vec2<f32>(sin(p.x * 0.91 + p.y * 1.12 - phase * 0.57),
            cos(p.x * 1.23 - p.y * 0.86 + phase * 0.61));
        p += flow * (look.current.x + low * 0.34)
            + twist * (look.current.y * 0.46 + middle * 0.12);
        let cell = liquid_glass_cell(p, identity, look);
        let middle_bin = spectrum_at(0.18 + cell.seed * 0.31) * live;
        let upper_bin = spectrum_at(0.55 + cell.seed * 0.44) * live;
        let local_mid = 1.0 - exp(-max(middle_bin, 0.0) * 2.2);
        let local_high = 1.0 - exp(-max(upper_bin, 0.0) * 2.2);
        let footprint = pixel() * depth * 0.36 * max(look.domain.x, look.domain.y);
        let edge_width = max(look.optics.x, footprint * 0.85);
        let bevel = exp(-cell.edge / max(edge_width * 2.4, 0.001));
        let rim = 1.0 - smoothstep(edge_width * 0.20, edge_width * 0.85, cell.edge);
        let curvature = look.cells.w * (0.75 + low * 0.40);
        let normal = normalize(vec3<f32>(-cell.delta * (curvature + bevel * 0.85),
            0.52 + cell.edge * 1.7));
        let light = normalize(vec3<f32>(-0.38 + sin(time * 0.12) * 0.18,
            0.47 + cos(time * 0.093) * 0.16, 0.76));
        let diffuse = max(dot(normal, light), 0.0);
        let sheen = pow(max(dot(normal, light), 0.0), 38.0);
        let edge_facing = max(dot(normal.xy, normalize(light.xy)), 0.0);
        let channel_phase = cell.delta.y * look.current.z
            + sin(cell.delta.x * 5.1 + phase * 0.67) * (0.55 + local_mid * 1.6)
            - phase * 0.53;
        let channel_wave = filtered_sine(channel_phase, footprint * (look.current.z + 5.1));
        let channel = pow(max(channel_wave, 0.0), 12.0);
        let hue = cell.seed * 0.84 + identity * 0.19 + look.optics.w + time * 0.011;
        let raw_pigment = liquid_glass_colour(hue);
        let pigment = raw_pigment / max(max(raw_pigment.x, raw_pigment.y), raw_pigment.z);
        let refracted = liquid_glass_colour(hue + normal.x * 0.08
            + channel_wave * 0.025 * look.optics.z);
        let glass = mix(pigment, refracted, 0.28)
            * (0.09 + diffuse * diffuse * 0.92 + channel * local_mid * 0.20);
        // Dark lead between coloured panes supplies contrast. The bright rim
        // stays coloured; fine-frequency detail never becomes a white screen.
        let lead = mix(1.0, 0.025, rim);
        let face_opacity = 0.29 + look.optics.y * 0.35 + near * 0.10;
        let opacity = visible * mix(face_opacity, 0.96, rim);
        body += glass * lead * opacity * transmission;
        let glint_colour = mix(pigment, vec3<f32>(0.86, 0.96, 1.0), 0.28);
        let edge_light = liquid_glass_colour(hue + 0.08)
            * (bevel * edge_facing * (0.10 + high * 0.16 + local_high * 0.30)
                + channel * (0.015 + local_mid * 0.09)
                + rim * edge_facing * (0.045 + local_high * 0.16 + pulse * 0.030))
            + glint_colour * sheen * (0.24 + near * 0.45 + local_high * 0.22)
                * (1.0 - rim);
        seams += edge_light * visible * transmission * (0.72 + near * 0.28);
        // No ray-direction jump when the nearest Voronoi identity changes:
        // the opaque seam brings displacement smoothly back to zero there.
        let lens_interior = smoothstep(edge_width * 0.25, edge_width * 1.5, cell.edge);
        let scaled_normal = normal.xy * look.domain.xy;
        let ray_normal = rotate(vec2<f32>(scaled_normal.x,
            scaled_normal.y + look.domain.w * scaled_normal.x), -domain_angle);
        refraction += ray_normal / max(length(ray_normal), 1.0) * visible * lens_interior
            * (0.035 + look.optics.z * 0.085) * (0.60 + near * 0.40);
        transmission *= 1.0 - opacity;
    }
    body += distant_colour * transmission * 0.12;
    // Most pigment is material, so independently tinting the background does
    // not turn every pane into one colour. Common hue/Mocca transforms remain.
    return World(body * 0.16, body * 0.84 + seams);
}
