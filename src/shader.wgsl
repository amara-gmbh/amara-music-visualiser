@group(0) @binding(1) var previous_frame: texture_2d<f32>;
@group(0) @binding(2) var frame_sampler: sampler;
@group(0) @binding(3) var raster_worlds: texture_2d_array<f32>;

// Compile only the procedural worlds in the current pair (bit = world index).
// Raster sampling remains available in every variant. The general fallback
// includes all twelve procedural worlds; production prepares 79 unique masks.
const included_worlds: u32 = 126735u;
const implicit_bridges: bool = true;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct World {
    light: vec3<f32>,
    marks: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var out: VertexOut;
    out.position = vec4<f32>(positions[index], 0.0, 1.0);
    out.uv = vec2<f32>(positions[index].x * 0.5 + 0.5, 0.5 - positions[index].y * 0.5);
    return out;
}

// Fine material lines have no halo. Width is in projected screen units, so
// structure that becomes subpixel softens instead of flickering into dots.
fn wire(distance: f32, width: f32) -> f32 {
    let w = max(width, pixel() * 0.5);
    return 1.0 - smoothstep(w * 0.35, w * 1.35, abs(distance));
}

fn filtered_sine(phase: f32, footprint: f32) -> f32 {
    return sin(phase) * (1.0 - smoothstep(0.7, 2.7, abs(footprint)));
}

fn segment_distance(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let span = b - a;
    let along = clamp(dot(p - a, span) / max(dot(span, span), 0.000001), 0.0, 1.0);
    return length(p - a - span * along);
}

// Thin anti-aliased light with a bounded local halo, not a volumetric fog fill.
fn thread(distance: f32, width: f32) -> f32 {
    let d = abs(distance);
    let w = max(width, pixel() * 0.55);
    return 1.0 - smoothstep(w * 0.45, w * 1.4, d) + 0.10 * exp(-d / (w * 4.0));
}

fn rotate_hue(colour: vec3<f32>, turns: f32) -> vec3<f32> {
    let angle = turns * 6.283185;
    let y = dot(colour, vec3<f32>(0.299, 0.587, 0.114));
    let iq = rotate(vec2<f32>(dot(colour, vec3<f32>(0.596, -0.274, -0.322)),
        dot(colour, vec3<f32>(0.211, -0.523, 0.312))), angle);
    return max(vec3<f32>(y + 0.956 * iq.x + 0.621 * iq.y,
        y - 0.272 * iq.x - 0.647 * iq.y,
        y - 1.106 * iq.x + 1.703 * iq.y), vec3<f32>(0.0));
}

// The user's hue remains the base for every world and its environment.
fn tint(colour: vec3<f32>) -> vec3<f32> {
    return rotate_hue(colour, u.d.x - 0.72);
}

// A material palette, applied only after every world's geometry and shading.
// Cosmic takes the original path exactly. Vanilla carries the same relief and
// fine ink. Aquarelle uses a soft pigment mix while preserving luminance.
fn palette_radiance(colour: vec3<f32>) -> vec3<f32> {
    let mode = clamp(u.d.w, 0.0, 2.0);
    if mode <= 0.0 { return colour; }
    let weights = vec3<f32>(0.2126, 0.7152, 0.0722);
    let luminance = dot(colour, weights);
    let peak = max(colour.x, max(colour.y, colour.z));
    let energy = mix(luminance, peak, 0.18);
    let warmth = clamp((colour.x - colour.z) / max(peak, 0.00001), 0.0, 1.0);
    let ivory = vec3<f32>(0.96, 0.91, 0.79);
    let bronze = vec3<f32>(0.62, 0.44, 0.23);
    let material = mix(ivory, bronze, smoothstep(0.2, 0.9, warmth) * 0.22);
    let vanilla = material * energy / max(dot(material, weights), 0.00001);
    let pigment = vec3<f32>(
        colour.x * 0.72 + colour.z * 0.28,
        colour.y * 0.68 + colour.x * 0.18 + colour.z * 0.14,
        colour.z * 0.66 + colour.y * 0.24 + colour.x * 0.10);
    let wash = mix(vec3<f32>(luminance), pigment, 0.72);
    let aquarelle = wash * luminance / max(dot(wash, weights), 0.00001);
    let mocca_amount = min(mode, 1.0) * (1.0 - max(mode - 1.0, 0.0));
    return mix(mix(colour, vanilla, mocca_amount), aquarelle, max(mode - 1.0, 0.0));
}

// Independent backdrop colour, eased on the CPU in physical time. Preserve
// the world's light/shadow relief and musical luminance; never recolour its
// foreground marks or change geometry. Zero mix takes the original path.
fn background_radiance(colour: vec3<f32>) -> vec3<f32> {
    let amount = clamp(u.background.w, 0.0, 1.0);
    if amount <= 0.0 { return colour; }
    let weights = vec3<f32>(0.2126, 0.7152, 0.0722);
    let luminance = dot(colour, weights);
    let tinted = u.background.rgb * luminance
        / max(dot(u.background.rgb, weights), 0.03);
    return mix(colour, tinted, amount);
}


// A stable frequency-to-space map gives the music a visible physical job.
// Bass opens the whole structure; actual local bins lift/bend different parts.
// Nothing here oscillates by itself or advances the camera on a drum hit.
fn musical_domain(uv: vec2<f32>) -> vec2<f32> {
    let low = clamp(bass(), 0.0, 1.5);
    let middle = clamp(mids(), 0.0, 1.5);
    let p = uv / (1.0 + low * 0.22);
    let horizontal = spectrum_at(clamp(p.x * 0.25 + 0.5, 0.0, 1.0));
    let diagonal = spectrum_at(clamp(p.y * 0.21 + p.x * 0.11 + 0.5, 0.0, 1.0));
    let fold = vec2<f32>(sin(p.y * 1.8) * middle * 0.11,
        (cos(p.x * 1.3) - 1.0) * horizontal * 0.24);
    return p + fold + vec2<f32>(sin(p.y * 1.2) * diagonal * 0.10, 0.0);
}

fn world(uv: vec2<f32>, scene: f32) -> World {
    if (included_worlds & 65536u) != 0u && scene > 15.5 { return aquarelle_world(uv / (0.96 + u.h.y * 0.08), u.looks[16]); }
    if (included_worlds & 32768u) != 0u && scene > 14.5 && scene < 15.5 { return mocha_world(uv / (0.96 + u.h.y * 0.08), u.looks[15]); }
    if (included_worlds & 16384u) != 0u && scene > 13.5 && scene < 14.5 { return neon_coast(uv / (0.96 + u.h.y * 0.08)); }
    if (included_worlds & 8192u) != 0u && scene > 12.5 && scene < 13.5 { return neon_memory(uv); }
    if (included_worlds & 2048u) != 0u && scene > 10.5 && scene < 11.5 {
        return liquid_stained_glass(uv / (0.96 + u.h.y * 0.08));
    }
    if (included_worlds & 1024u) != 0u && scene > 9.5 && scene < 10.5 {
        return mandelbrot(uv / (0.96 + u.h.y * 0.08));
    }
    if (included_worlds & 512u) != 0u && scene > 8.5 && scene < 9.5 {
        return aurora(uv / (0.96 + u.h.y * 0.08));
    }
    if (included_worlds & 256u) != 0u && scene > 7.5 && scene < 8.5 {
        // Fixed overhead view: no shared beat zoom/warping or horizon.
        return water(uv / (0.96 + u.h.y * 0.08));
    }
    if scene >= 3.5 {
        // Actual instanced/mesh renderers, not another contour-field recipe.
        // Undo shared view scaling: raster projection already uses this canvas.
        let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
        let canvas = uv / (0.96 + u.h.y * 0.08);
        let coordinate = vec2<f32>((canvas.x + u.g.w) / aspect, canvas.y) * 0.5 + 0.5;
        // A transported geometry layer has finite canvas coverage. Fade its
        // border to transparent instead of stretching the last texel forever.
        let edge = vec2<f32>(3.0) / max(u.a.xy, vec2<f32>(1.0));
        let coverage = smoothstep(vec2<f32>(0.0), edge, coordinate)
            * smoothstep(vec2<f32>(0.0), edge, 1.0 - coordinate);
        let layer = select(i32(clamp(round(scene) - 4.0, 0.0, 3.0)), 4, scene > 11.5);
        let sample = textureSampleLevel(raster_worlds, frame_sampler, coordinate, layer, 0.0).rgb;
        let bounded = sample * coverage.x * coverage.y;
        return World(bounded * 0.08, bounded);
    }
    var result: World;
    let surface = musical_domain(uv);
    if (included_worlds & 1u) != 0u && scene < 0.5 { result = flight(surface); }
    else if (included_worlds & 2u) != 0u && scene >= 0.5 && scene < 1.5 { result = living_mathematics(surface); }
    else if (included_worlds & 4u) != 0u && scene >= 1.5 && scene < 2.5 { result = tide(surface); }
    else if (included_worlds & 8u) != 0u && scene >= 2.5 { result = feedback_reverie(surface); }
    let presence = clamp(bass() * 0.34 + mids() * 0.40 + air() * 0.26, 0.0, 1.5);
    // Modest overall exposure, strong fine-structure contrast: the background
    // remains colourful while active musical detail clearly wakes and relaxes.
    result.marks *= 0.58 + presence * 1.15;
    return result;
}

// Glass Tide and Feedback Reverie share a real implicit-surface bridge.
// Interpolate signed levels AND their analytic slopes before either material
// is shaded: cavities open, join and become the incoming field. This is not
// a wipe between two pictures. It costs one evaluation of each existing
// surface, just like the usual two-world transition, and preserves endpoints.
fn liquid_world_morph(uv: vec2<f32>, reverie_amount: f32) -> World {
    // Keep endpoint instructions exact. Subtracting a surface level and then
    // adding it back can round differently at a fine contour, even when the
    // blend is mathematically zero or one.
    if reverie_amount <= 0.0 { return world(uv, 2.0); }
    if reverie_amount >= 1.0 { return world(uv, 3.0); }
    let surface = musical_domain(uv);
    let look = tide_look();
    let p = tide_surface_domain(surface, look);
    let glass = tide_surface_field(p, look);
    let engraving = reverie_liquid(surface, u.a.z);
    let glass_level = look.shape.y;
    let engraving_level = reverie_level();
    let field = mix(vec3<f32>(glass.x - glass_level, glass.yz),
        vec3<f32>(engraving.x - engraving_level, engraving.yz), reverie_amount);
    let before = tide_render(p, vec3<f32>(field.x + glass_level, field.yz), look);
    let after = feedback_render(surface,
        vec3<f32>(field.x + engraving_level, field.yz));
    let presence = clamp(bass() * 0.34 + mids() * 0.40 + air() * 0.26, 0.0, 1.5);
    return World(mix(before.light, after.light, reverie_amount),
        mix(before.marks, after.marks, reverie_amount) * (0.58 + presence * 1.15));
}

// Water, glass relief and engraved feedback share a continuous scalar surface
// during these passages. The outgoing cavities flatten into actual incoming
// ripple crests, with BOTH materials shaded from that same intermediate field.
// This is a geometry/normal interpolation, not two finished images dissolving.
// The gain only reconciles the water's metre-like height with implicit levels.
fn water_world_morph(uv: vec2<f32>, liquid_scene: f32, water_amount: f32) -> World {
    if water_amount <= 0.0 { return world(uv, liquid_scene); }
    if water_amount >= 1.0 { return world(uv, 8.0); }
    let water_uv = uv / (0.96 + u.h.y * 0.08);
    let water_appearance = water_look();
    let water_field = water_surface(water_domain(water_uv, water_appearance), water_appearance);
    let water_height = vec3<f32>(water_field.x, water_field.yz) * 8.0;
    let liquid_uv = musical_domain(uv);
    let glass_appearance = tide_look();
    var liquid_position = liquid_uv;
    var level = reverie_level();
    var liquid_field: vec3<f32>;
    if (included_worlds & 4u) != 0u && liquid_scene < 2.5 {
        liquid_position = tide_surface_domain(liquid_uv, glass_appearance);
        liquid_field = tide_surface_field(liquid_position, glass_appearance);
        level = glass_appearance.shape.y;
    } else if (included_worlds & 8u) != 0u {
        liquid_field = reverie_liquid(liquid_uv, u.a.z);
    }
    let geometry = mix(vec3<f32>(liquid_field.x - level, liquid_field.yz),
        water_height, water_amount);
    var outgoing: World;
    if (included_worlds & 4u) != 0u && liquid_scene < 2.5 {
        outgoing = tide_render(liquid_position,
            vec3<f32>(geometry.x + level, geometry.yz), glass_appearance);
    } else if (included_worlds & 8u) != 0u {
        outgoing = feedback_render(liquid_position,
            vec3<f32>(geometry.x + level, geometry.yz));
    }
    let presence = clamp(bass() * 0.34 + mids() * 0.40 + air() * 0.26, 0.0, 1.5);
    outgoing.marks *= 0.58 + presence * 1.15;
    // Only the water has an explicit curvature channel. Its caustic focus
    // grows continuously while the shared height and normal already transform.
    let incoming = water_render(water_uv, water_appearance,
        vec4<f32>(geometry / 8.0, water_field.w * water_amount));
    return World(mix(outgoing.light, incoming.light, water_amount),
        mix(outgoing.marks, incoming.marks, water_amount));
}

fn math_flow(uv: vec2<f32>, family: f32) -> vec2<f32> {
    let time = u.a.z;
    let delta = clamp(u.g.y, 0.0, 0.1);
    if family < 0.5 {
        // Reproject the slow Julia-domain rotation/zoom instead of dragging a
        // sharp fractal through the former fabric's strong image-space curl.
        let advance = delta * u.c.z * u.i.y;
        let zoom = 0.80 + 0.12 * sin(time * 0.16);
        let scale = 1.0 + advance * 0.0192 * cos(time * 0.16) / zoom;
        let radial = log(1.0 + dot(uv, uv) * 0.5);
        let angle = advance * (0.025 + 0.00568 * cos(time * 0.071)
            + 0.0242 * cos(time * 0.22) * radial);
        let curl = vec2<f32>(sin(uv.y * 2.3 + time * 0.12), -sin(uv.x * 1.9 - time * 0.09));
        return rotate(uv * scale, angle) + curl * delta * u.d.z * 0.003;
    }
    let advance = delta * u.c.z * u.i.y;
    if family < 1.5 {
        return rotate(uv, advance * 0.035) * (1.0 - advance * 0.008);
    }
    if family < 2.5 {
        let sculpture_time = time * 2.40 + u.j.w * 0.18;
        return rotate(uv, advance * (0.0672 + 0.06336 * cos(sculpture_time * 0.12)));
    }
    return uv - curl_velocity(uv, time) * advance * (0.025 + u.d.z * 0.030);
}

fn math_memory(family: f32) -> f32 {
    if family < 0.5 { return 0.62; }
    if family < 1.5 { return 0.42; }
    if family < 2.5 { return 0.46; }
    return 0.88;
}

fn flow(uv: vec2<f32>, scene: f32) -> vec2<f32> {
    if scene > 13.5 { return uv; }
    if (included_worlds & 8192u) != 0u && scene > 12.5 { return neon_memory_flow(uv); }
    let delta = clamp(u.g.y, 0.0, 0.1);
    if (included_worlds & 1u) != 0u && scene < 0.5 { return flight_flow(uv, delta); }
    if (included_worlds & 2u) != 0u && scene >= 0.5 && scene < 1.5 {
        let amount = clamp(u.j.z, 0.0, 1.0);
        if amount >= 1.0 { return math_flow(uv, u.j.y); }
        let before = math_flow(uv, u.j.x);
        if amount <= 0.0 || u.j.x == u.j.y { return before; }
        return mix(before, math_flow(uv, u.j.y), math_interlace_weight(uv, amount));
    }
    if (included_worlds & 4u) != 0u && scene >= 1.5 && scene < 2.5 { return tide_flow(uv, delta); }
    if (included_worlds & 8u) != 0u && scene >= 2.5 && scene < 3.5 { return feedback_flow(uv); }
    return uv;
}

fn memory_scale(uv: vec2<f32>, scene: f32) -> f32 {
    if scene > 13.5 { return 0.0; }
    if (included_worlds & 8192u) != 0u && scene > 12.5 { return neon_memory_memory(); }
    if scene > 11.5 { return 0.04; }
    // Re-evaluate the exact set at the current camera; stale image history
    // would smear its fine boundary under zoom and suggest false geometry.
    if scene > 9.5 { return 0.0; }
    if scene > 8.5 { return 0.12; }
    if scene > 7.5 { return 0.07; }
    if scene < 0.5 { return flight_memory(); }
    if scene < 1.5 { return mix(math_memory(u.j.x), math_memory(u.j.y), math_interlace_weight(uv, clamp(u.j.z, 0.0, 1.0))); }
    if scene < 2.5 { return tide_memory(); }
    if scene >= 3.5 { return 0.10; }
    return feedback_memory();
}

// Two complete worlds are woven by a smooth spatial field during Auto morphs.
// Both endpoint images stay exact, and flow/memory follow the identical local
// weight. Randomized passages choose a seed; continuous time and mapped music
// evolve the seam, rather than a fixed sequence or an abrupt overlay toggle.
fn interlace_weight(uv: vec2<f32>, amount: f32) -> f32 {
    if amount <= 0.0 || amount >= 1.0 || u.i.z <= 0.0 { return amount; }
    let seed = u.i.w;
    let time = u.a.z;
    let anchor = vec2<f32>(sin(seed * 1.73), cos(seed * 0.91)) * 0.42;
    let p = rotate(uv - anchor, sin(seed * 1.17) * 3.0);
    let q = p + vec2<f32>(sin(p.y * 2.4 - time * 0.19 + seed),
        cos(p.x * 2.1 + time * 0.16 - seed)) * (0.24 + u.h.x * 0.12);
    // Continuously seeded coefficients, not a choice from four recognizable
    // transition masks. Different currents deform one another at unequal rates.
    let first = sin(q.x * (1.85 + sin(seed * 0.73) * 0.48) + q.y * 0.7
        + sin(q.y * 2.1 + time * 0.113 + seed));
    let second = cos(q.y * (2.1 + cos(seed * 1.13) * 0.52) - q.x * 0.9
        - time * 0.097 + first * 0.65);
    let third = sin((q.x + q.y) * (0.9 + sin(seed * 0.41) * 0.32)
        + time * 0.061 + first * 1.2 - second * 0.4);
    let current = first * 0.90 + second * 0.63 + third * 0.41;
    let envelope = 16.0 * amount * amount * (1.0 - amount) * (1.0 - amount);
    let spectrum = spectrum_at(clamp(q.x * 0.18 + 0.5, 0.0, 1.0));
    let field = amount * 2.0 - 1.0 + current * envelope * (0.60 + spectrum * 0.16);
    let width = 0.48 + 0.10 * sin(seed * 0.53);
    let woven = smoothstep(-width, width, field);
    return mix(amount, woven, clamp(u.i.z, 0.0, 1.0));
}

// Analytic currents for fluid materials only. Projected dots, rigid sculpture,
// landscapes and native cameras never pass through this display-space field.
fn melt_current(p: vec2<f32>, time: f32, seed: f32) -> vec2<f32> {
    let low = clamp(bass(), 0.0, 1.5);
    let mid = clamp(mids(), 0.0, 1.5);
    let high = clamp(air(), 0.0, 1.5);
    let a = p * 1.12 + vec2<f32>(time * 0.17 + seed, -time * 0.131 + seed * 0.7);
    let b = rotate(p, 0.81) * 1.63 + vec2<f32>(-time * 0.113, time * 0.157 + seed * 1.3);
    let broad = vec2<f32>(sin(a.x) * cos(a.y), -cos(a.x) * sin(a.y));
    let fine = rotate(vec2<f32>(sin(b.x) * cos(b.y), -cos(b.x) * sin(b.y)), -0.81);
    let orbit = vec2<f32>(-p.y, p.x) / (1.0 + dot(p, p));
    return broad * (0.70 + low * 0.16) + fine * (0.25 + mid * 0.14)
        + orbit * (sin(time * 0.091 + seed * 0.39) * 0.26 + high * 0.08);
}

fn morphic_transport(uv: vec2<f32>, amount: f32) -> vec2<f32> {
    if amount <= 0.0 || amount >= 1.0 { return vec2<f32>(0.0); }
    let seed = u.i.w;
    let time = u.a.z;
    let angle = sin(seed * 0.47) * 2.8;
    let p = rotate(uv, angle);
    let envelope = 16.0 * amount * amount * (1.0 - amount) * (1.0 - amount);
    let strength = (0.30 + clamp(u.d.z, 0.0, 1.5) * 0.08) * envelope
        * (0.45 + 0.55 * clamp(u.i.z, 0.0, 1.0));
    // Subtract the current at the origin in both integration steps. The
    // material may flow around the subject, but the stage focus stays fixed.
    let origin_current = melt_current(vec2<f32>(0.0), time, seed);
    let half_step = strength * 0.5;
    let first_midpoint = p + (melt_current(p, time, seed) - origin_current) * half_step * 0.5;
    let midway = p + (melt_current(first_midpoint, time, seed) - origin_current) * half_step;
    let second_midpoint = midway + (melt_current(midway, time, seed) - origin_current) * half_step * 0.5;
    let arrived = midway + (melt_current(second_midpoint, time, seed) - origin_current) * half_step;
    return rotate(arrived - p, -angle);
}

fn fluid_transport_scale(scene: f32) -> f32 {
    // Glass Tide and Feedback Reverie can melt. Their direct pairing uses
    // the real implicit-surface bridge above. Every other world's projected
    // structure is evaluated at its own unchanged, centred coordinates.
    return select(0.0, 1.0, scene > 1.5 && scene < 3.5);
}

// Musical changes reveal the world's OWN material, never add a common ring,
// waveform, or expanding outline. This runs after both sides of a world melt,
// so dots keep their positions and each world keeps its distinct silhouette.
fn musical_material(material: World, uv: vec2<f32>) -> World {
    if u.raster.z <= 0.0 { return material; }
    var result = material;
    let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
    let position = clamp(uv.x / (aspect * 2.0) + 0.5, 0.0, 1.0);
    let local = spectrum_at(position);
    let texture = smoothstep(0.015, 0.65, local);
    // These controls are slow physical-time envelopes of measured onsets.
    // More articulation illuminates local details; no whole-screen flash.
    let rhythm = u.music.x * 0.30 + u.music.y * (0.45 + texture * 0.65);
    let bass_body = u.music.z * 0.15;
    let air_detail = u.music.w * texture * 0.24;
    result.marks *= 1.0 + rhythm + bass_body + air_detail;
    // A voice-like entry changes the pigment within existing surfaces and
    // particles. Black/empty space stays empty: there is no overlaid shape.
    let voice = clamp(u.voice.x * 0.28 + u.voice.z * 0.85, 0.0, 1.5)
        * select(0.0, 1.0, u.e.y > 0.0);
    let mid_detail = spectrum_at(0.18 + position * 0.32);
    let region = 0.38 + 0.62 * smoothstep(-0.6, 0.6,
        uv.y * 0.35 + uv.x * 0.22 + mid_detail * 0.7);
    let marks = max(material.marks.x, max(material.marks.y, material.marks.z));
    let surface = max(material.light.x, max(material.light.y, material.light.z));
    let pigment = mix(vec3<f32>(0.22, 0.70, 0.95),
        vec3<f32>(0.91, 0.37, 0.70), smoothstep(0.15, 0.85, position));
    result.marks += pigment * marks * voice * region * 0.75;
    result.light += pigment * surface * voice * region * 0.28;
    return result;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
    let uv = (in.uv * 2.0 - 1.0) * vec2<f32>(aspect, 1.0) - vec2<f32>(u.g.w, 0.0);
    let amount = clamp(u.f.w, 0.0, 1.0);
    let view = uv * (0.96 + u.h.y * 0.08);
    let transitioning = amount < 1.0 && u.f.z != u.a.w;
    // The paired dot renderer already moves the same 50,000 vertices from one
    // 3-D sculpture to the other in its destination layer. Sampling/warping a
    // second image here would destroy that correspondence.
    let dot_pair = transitioning && ((u.f.z == 4.0 && u.a.w == 7.0)
        || (u.f.z == 7.0 && u.a.w == 4.0));
    // Exact endpoints take the identical ordinary world branch, avoiding a
    // driver-dependent floating-point rounding difference in a nested call.
    let liquid_pair = implicit_bridges && (included_worlds & 12u) == 12u && transitioning && amount > 0.0 && ((u.f.z == 2.0 && u.a.w == 3.0)
        || (u.f.z == 3.0 && u.a.w == 2.0));
    let water_pair = implicit_bridges && (included_worlds & 256u) != 0u && transitioning && amount > 0.0
        && ((u.a.w == 8.0 && (u.f.z == 2.0 || u.f.z == 3.0))
            || (u.f.z == 8.0 && (u.a.w == 2.0 || u.a.w == 3.0)));
    let field_pair = liquid_pair || water_pair;
    var woven = 1.0;
    if transitioning {
        woven = amount;
        if !dot_pair && !field_pair { woven = interlace_weight(uv, amount); }
    }
    var scene = u.a.w;
    var outgoing_view = view;
    var incoming_view = view;
    if transitioning && !dot_pair && !field_pair
        && (fluid_transport_scale(u.f.z) > 0.0 || fluid_transport_scale(u.a.w) > 0.0) {
        let transport = morphic_transport(view, amount);
        // An in-place, relief-guided interlace grows through intact objects.
        // Only liquid material rides this current; no screen-space shear is
        // allowed to bend equaliser bars, dots, terrain, or native cameras.
        outgoing_view += transport * fluid_transport_scale(u.f.z);
        incoming_view += transport * fluid_transport_scale(u.a.w);
    }
    var sample_view = incoming_view;
    if woven <= 0.0 && !dot_pair { scene = u.f.z; sample_view = outgoing_view; }
    var radiance: World;
    if liquid_pair {
        radiance = liquid_world_morph(view, select(1.0 - amount, amount, u.a.w == 3.0));
    } else if water_pair {
        radiance = water_world_morph(view, select(u.a.w, u.f.z, u.a.w == 8.0),
            select(1.0 - amount, amount, u.a.w == 8.0));
    } else {
        radiance = world(sample_view, scene);
    }
    var drift = flow(uv, scene);
    var memory = memory_scale(uv, scene);
    let coast_pair = transitioning && (u.f.z == 14.0 || u.a.w == 14.0);
    let mocha_pair = transitioning && (u.f.z == 15.0 || u.a.w == 15.0);
    var material_applied = false;
    if transitioning && woven > 0.0 && woven < 1.0 && !dot_pair {
        if !field_pair {
            var before = world(outgoing_view, u.f.z);
            // Let actual ridges and points guide the growth front, using only
            // the geometry already rendered on each side of the transition.
            let before_density = max(before.marks.x, max(before.marks.y, before.marks.z))
                + max(before.light.x, max(before.light.y, before.light.z)) * 0.45;
            let after_density = max(radiance.marks.x, max(radiance.marks.y, radiance.marks.z))
                + max(radiance.light.x, max(radiance.light.y, radiance.light.z)) * 0.45;
            let relief = after_density / (1.0 + after_density) - before_density / (1.0 + before_density);
            woven = clamp(woven + relief * woven * (1.0 - woven) * 0.72, 0.0, 1.0);
            // Coast assigns each band to a physical subject. Keep generic
            // emphasis on the other world's contribution only; relief above
            // still uses the identical unmodified geometry/material density.
            if coast_pair || mocha_pair {
                if u.f.z != 14.0 && u.f.z != 15.0 { before = musical_material(before, uv); }
                if u.a.w != 14.0 && u.a.w != 15.0 { radiance = musical_material(radiance, uv); }
                material_applied = true;
            }
            radiance.light = mix(before.light, radiance.light, woven);
            radiance.marks = mix(before.marks, radiance.marks, woven);
        }
        drift = mix(flow(uv, u.f.z), drift, woven);
        memory = mix(memory_scale(uv, u.f.z), memory, woven);
    }
    if transitioning {
        // Shorten old exposure while the visibility seam moves through intact
        // structures, avoiding bent-looking ghosts from a departing world.
        let melting = 16.0 * amount * amount * (1.0 - amount) * (1.0 - amount);
        memory = mix(memory, min(memory, 0.22), melting);
        if !field_pair {
            // A liquid's feedback flow must not drag the previous exposure
            // of a rigid subject into a bent after-image. Ease this protection
            // at either endpoint so the world's ordinary history stays exact.
            let protection = smoothstep(0.0, 0.12, amount)
                * (1.0 - smoothstep(0.88, 1.0, amount));
            drift = mix(drift, uv, protection);
            memory = mix(memory, min(memory, 0.10), protection);
        }
    }
    // Musical emphasis stays attached to existing geometry/material. No
    // universal spectrum shells, onset rings or outward vocal ripples.
    if !material_applied && scene != 14.0 && scene != 15.0 {
        radiance = musical_material(radiance, uv);
    }
    // A continuously travelling chromatic environment occupies the negative
    // space too. Evaluate once per frame pixel, not once per blended world.
    let foreground = max(radiance.light.x, max(radiance.light.y, radiance.light.z)) * 3.5
        + max(radiance.marks.x, max(radiance.marks.y, radiance.marks.z)) * 1.5;
    // Auto's independent colour journey rotates the world's own materials,
    // including dots and surfaces. Apply it before adding the environment so
    // background choice stays independent. Mocca's final conversion still
    // controls the palette. Zero follows the original manual path exactly.
    let mocha_weight = mix(select(0.0, 1.0, u.f.z == 15.0),
        select(0.0, 1.0, u.a.w == 15.0), woven);
    if u.material.x != 0.0 {
        if mocha_weight > 0.0 {
            let material_rotation = u.material.x * (1.0 - mocha_weight);
            if abs(material_rotation) > 0.00001 {
                radiance.light = rotate_hue(radiance.light, material_rotation);
                radiance.marks = rotate_hue(radiance.marks, material_rotation);
            }
        } else {
            radiance.light = rotate_hue(radiance.light, u.material.x);
            radiance.marks = rotate_hue(radiance.marks, u.material.x);
        }
    }
    // The dot equaliser needs contrast behind its translucent volume. Dim
    // only its backdrop, smoothly through the existing world melt; every
    // other world's colour environment retains its previous exposure.
    let equaliser_focus = mix(select(0.0, 1.0, u.f.z > 6.5 && u.f.z < 7.5),
        select(0.0, 1.0, u.a.w > 6.5 && u.a.w < 7.5), woven);
    let attractor_focus = mix(select(0.0, 1.0, u.f.z > 4.5 && u.f.z < 5.5),
        select(0.0, 1.0, u.a.w > 4.5 && u.a.w < 5.5), woven);
    // General world rule: every new non-flight scene with a separate
    // foreground inherits travel through morphing background colours. Native
    // camera flight and full-canvas colour surfaces opt out here. Water and
    // Aurora already supply their own coloured space; never cover it in stars
    // or evaluate an expensive environment only to multiply it by zero.
    let source_environment = select(1.0, 0.0, u.f.z < 0.5 || (u.f.z > 7.5 && u.f.z < 11.5) || u.f.z > 13.5);
    let destination_environment = select(1.0, 0.0, u.a.w < 0.5 || (u.a.w > 7.5 && u.a.w < 11.5) || u.a.w > 13.5);
    let environment_gain = mix(1.0, 0.28, equaliser_focus)
        * mix(1.0, 0.35, attractor_focus)
        * mix(source_environment, destination_environment, woven);
    var environment = vec3<f32>(0.0);
    if environment_gain > 0.0001 {
        environment = chromatic_space(uv);
        radiance.light += environment * environment_gain / (1.0 + foreground);
    }
    let exposure = 0.8 + u.c.y * 1.35;
    let coast_weight = mix(select(0.0, 1.0, u.f.z == 14.0),
        select(0.0, 1.0, u.a.w == 14.0), woven);
    let phrase_light = 0.82 + u.h.x * 0.12 + u.h.z * 0.12 + u.h.w * 0.15;
    let vignette = 1.0 - smoothstep(0.6, 2.1, length(uv)) * 0.35;
    var base = palette_radiance(background_radiance(tint(radiance.light)))
        * exposure * phrase_light * vignette;
    var ink = palette_radiance(tint(radiance.marks)) * exposure * phrase_light * vignette;
    if coast_weight > 0.0 {
        let coast_phrase = mix(phrase_light, 1.0, coast_weight);
        base = palette_radiance(background_radiance(tint(radiance.light)))
            * exposure * coast_phrase * vignette;
        ink = palette_radiance(tint(radiance.marks)) * exposure * coast_phrase * vignette;
    }
    if mocha_weight > 0.0 {
        let tinted_light = mix(tint(radiance.light), radiance.light, mocha_weight);
        let tinted_marks = mix(tint(radiance.marks), radiance.marks, mocha_weight);
        let local_phrase = mix(phrase_light, 1.0, coast_weight);
        let protected_light = mix(background_radiance(tinted_light), tinted_light, mocha_weight);
        base = palette_radiance(protected_light) * exposure * local_phrase * vignette;
        ink = palette_radiance(tinted_marks) * exposure * local_phrase * vignette;
    }
    if environment_gain > 0.0001 && u.i.z > 0.0 {
        // A little reflected colour connects the foreground to its travelling
        // space in Auto. Use the displayed background's palette, not a second
        // environment sample or raw pigment in a different colour space.
        let reflection = palette_radiance(background_radiance(tint(environment)));
        let weights = vec3<f32>(0.2126, 0.7152, 0.0722);
        let illumination = dot(reflection, weights);
        let peak = max(ink.x, max(ink.y, ink.z));
        let saturation = 1.0 - min(ink.x, min(ink.y, ink.z)) / max(peak, 0.00001);
        let link = clamp(u.i.z, 0.0, 1.0) * 0.08
            * mix(source_environment, destination_environment, woven)
            * smoothstep(0.02, 0.25, illumination)
            * smoothstep(0.04, 0.50, peak) * (1.0 - saturation * 0.55);
        // Preserve foreground luminance and empty space. Fully saturated dots
        // retain at least 96.4% of their own colour; manual mode stays exact.
        let reflected_ink = reflection * dot(ink, weights) / max(illumination, 0.00001);
        ink = mix(ink, reflected_ink, link);
    }
    var colour = 1.0 - exp(-(base + ink));
    if u.d.w > 0.0 {
        let coffee = vec3<f32>(0.109804, 0.094118, 0.074510);
        let paper = coffee + colour * (1.0 - coffee);
        colour = mix(colour, paper, clamp(u.d.w, 0.0, 1.0));
    }
    if u.g.x > 0.001 && u.g.z > 0.5 && memory > 0.0 {
        let delta = clamp(u.g.y, 0.0, 0.1);
        let echo = clamp(u.g.x * memory, 0.0, 0.92);
        let lifetime = 0.35 * echo / max(1.0 - echo, 0.02);
        let retention = exp(-delta / max(lifetime, 0.005));
        let history_uv = (drift + vec2<f32>(u.g.w, 0.0)) / vec2<f32>(aspect, 1.0) * 0.5 + 0.5;
        let history = textureSample(previous_frame, frame_sampler,
            clamp(history_uv, vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
        let inside = step(0.0, history_uv.x) * step(history_uv.x, 1.0)
            * step(0.0, history_uv.y) * step(history_uv.y, 1.0);
        let neon_weight = mix(select(0.0, 1.0, u.f.z > 12.5 && u.f.z < 13.5),
            select(0.0, 1.0, u.a.w > 12.5 && u.a.w < 13.5), woven);
        let attenuation = mix(0.998, exp(-0.12 * delta), neon_weight);
        colour = mix(colour, history * inside * attenuation, retention);
        // Retain new stroke definition without unbounded additive exposure.
        colour = max(colour, 1.0 - exp(-ink));
    }
    return vec4<f32>(clamp(colour, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

@fragment
fn fs_present(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(previous_frame, frame_sampler, in.uv);
}
