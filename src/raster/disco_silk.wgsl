// Disco Silk / Endless Silk Current. Three continuous, world-space satin
// sheets surround a centred travelling eye. A finite sampling window advances
// through their unbounded coordinates; no objects are born, wrapped or reset.
// Unchanged draw budget: 3 * 128 * 24 * 6 = 55,296 vertices, one draw,
// layer 4 / pipeline 5, alpha blend, depth Less/write and no culling.

struct SilkSection {
    shape: vec4<f32>, // bounded angle offset, radius, half-width, curved/flat mix
    folds: vec4<f32>, // relief, transverse frequency, fold phase, edge bank
    scale: vec2<f32>,
};

// These are spatial passages, not closed knot silhouettes. Every recipe keeps
// the same three sheet identities. Only bounded section values are morphed:
// changing a look never changes a coefficient multiplying the journey clock.
fn silk_section(id: u32, s: f32, sheet: u32) -> SilkSection {
    let phase = f32(sheet) * 2.0943951;
    let slow = sin(s * 0.113 + phase * 0.71);
    let wave = sin(s * 0.271 + phase);
    let braid = sin(s * 0.421 + phase);
    switch id {
        case 1u: { // Braided Passage: broad sails exchange depth and lean.
            return SilkSection(vec4<f32>(braid * 0.52,
                2.65 + cos(s * 0.421 + phase) * 0.48, 0.83 + slow * 0.12, 0.48),
                vec4<f32>(0.36, 4.8, wave * 0.75, slow * 0.20), vec2<f32>(1.0));
        }
        case 2u: { // Turning Sails: open, wide sheets billow across the view.
            return SilkSection(vec4<f32>(slow * 0.23, 2.75 + wave * 0.32,
                1.03 + sin(s * 0.187 + phase) * 0.25, 0.10),
                vec4<f32>(0.48, 2.8, sin(s * 0.239) * 0.90, braid * 0.28),
                vec2<f32>(1.0));
        }
        case 3u: { // Silk Canyon: two vertical sides over a rolling floor.
            let angles = array<f32, 3>(-0.5235988, 0.5235988, 0.0);
            return SilkSection(vec4<f32>(angles[sheet] + slow * 0.10,
                2.45 + wave * 0.22, select(1.32, 1.42, sheet == 2u), 0.03),
                vec4<f32>(0.39, 3.5, wave * 0.65, 0.0), vec2<f32>(1.0));
        }
        case 4u: { // Petal Vault: deeply scalloped shoulders open overhead.
            return SilkSection(vec4<f32>(slow * 0.18,
                2.65 + sin(s * 0.173) * 0.35, 1.07, 0.96),
                vec4<f32>(0.54, 5.8, sin(s * 0.193) * 0.40, 0.0),
                vec2<f32>(1.0, 1.12));
        }
        case 5u: { // Twin Currents: paired, separated S-shaped silk banks.
            let angles = array<f32, 3>(-0.34, 0.34, 0.0);
            return SilkSection(vec4<f32>(angles[sheet] + slow * 0.35,
                2.75 + braid * 0.40, select(1.18, 0.0, sheet == 2u), 0.24),
                vec4<f32>(0.46, 3.0, wave, slow * 0.24), vec2<f32>(1.0, 1.18));
        }
        case 6u: { // Folded Canopy: a wide, low roof above a deeper floor.
            let angles = array<f32, 3>(0.20, -0.20, 0.0);
            return SilkSection(vec4<f32>(angles[sheet] + wave * 0.10,
                select(2.50, 4.1, sheet == 2u) + slow * 0.25, 1.35, 0.08),
                vec4<f32>(0.55, 6.5, braid * 0.45, slow * 0.10),
                vec2<f32>(1.16, 0.78));
        }
        case 7u: { // Infinity Weave: narrow interlacing streams, open sky.
            return SilkSection(vec4<f32>(sin(s * 0.231 + phase) * 0.63,
                2.85 + cos(s * 0.231 + phase) * 0.50, 0.58 + slow * 0.13, 0.32),
                vec4<f32>(0.34, 4.0, wave * 0.65, braid * 0.16),
                vec2<f32>(1.15, 0.88));
        }
        case 8u: { // Opening Bloom: deep folds unfurl into broad chambers.
            let opening = sin(s * 0.109) * 0.62 + sin(s * 0.367 + phase) * 0.18;
            return SilkSection(vec4<f32>(slow * 0.17, 3.05 + opening,
                1.12 - opening * 0.20, 0.85),
                vec4<f32>(0.47, 4.4, wave * 0.60, slow * 0.08), vec2<f32>(1.04));
        }
        case 9u: { // Spiral Loom: pleated satin turns in several slow currents.
            return SilkSection(vec4<f32>(sin(s * 0.151) * 0.62
                + sin(s * 0.337 + phase) * 0.24, 2.75 + slow * 0.28, 1.04, 0.68),
                vec4<f32>(0.55, 7.0, braid * 0.50, wave * 0.18), vec2<f32>(1.0));
        }
        default: { // Helix Current: three broad, continuously unfurling walls.
            return SilkSection(vec4<f32>(sin(s * 0.193) * 0.66 + wave * 0.13,
                2.65 + slow * 0.24, 1.05, 0.92),
                vec4<f32>(0.30, 4.2, sin(s * 0.317) * 0.50, 0.0), vec2<f32>(1.0));
        }
    }
}

fn silk_morph_section(s: f32, sheet: u32) -> SilkSection {
    let state = u.looks[12];
    let a = u32(clamp(round(state.x), 0.0, 9.0));
    let b = u32(clamp(round(state.y), 0.0, 9.0));
    let first = silk_section(a, s, sheet);
    if state.z <= 0.0 || a == b { return first; }
    let next = silk_section(b, s, sheet);
    if state.z >= 1.0 { return next; }
    let blend = clamp(state.z, 0.0, 1.0);
    return SilkSection(mix(first.shape, next.shape, blend),
        mix(first.folds, next.folds, blend), mix(first.scale, next.scale, blend));
}

fn silk_unit(v: vec3<f32>) -> vec3<f32> {
    return v / max(length(v), 0.00001);
}

fn silk_filtered_at(position: f32) -> f32 {
    // The existing raster lane already contains two-pole filtered current
    // FFT values, mapped once, followed by three all-bin RMS groups. Reading
    // it adds no state, allocations or delay to the shared audio pipeline.
    // Smooth spatial interpolation avoids a crease at a frequency-bin edge.
    let band = clamp(position, 0.0, 1.0) * 47.0;
    let low = u32(floor(band));
    let high = min(low + 1u, 47u);
    return mix(dot_history[3072u + low], dot_history[3072u + high],
        smoothstep(0.0, 1.0, fract(band)));
}

fn silk_sound(s: f32, across: f32, sheet: u32) -> vec4<f32> {
    if u.raster.z <= 0.0 { return vec4<f32>(0.0); }
    let position = 0.5 + 0.5 * sin(s * 0.097 + across * 0.81 + f32(sheet) * 2.0943951);
    // Slow mass and shape use the same 55ms-attack/180ms-release two-pole
    // lane as the dots/Attractor. A little existing RenderMotion envelope
    // retains articulation. High-frequency light keeps its faster measured
    // 35/45ms envelope/FFT attacks; it does not shake the large geometry.
    let lower = silk_filtered_at(position * (7.0 / 47.0));
    let middle = silk_filtered_at(mix(8.0 / 47.0, 23.0 / 47.0, position));
    let upper = spectrum_at(mix(24.0 / 47.0, 1.0, position));
    let groups = vec3<f32>(dot_history[3120u], dot_history[3121u], dot_history[3122u]);
    let signal = clamp(vec3<f32>(bass(), mids(), air()) * vec3<f32>(0.30, 0.20, 0.30)
        + vec3<f32>(lower, middle, upper) * vec3<f32>(0.40, 0.55, 0.50)
        + groups * vec3<f32>(0.30, 0.25, 0.20), vec3<f32>(0.0), vec3<f32>(1.5));
    // Convex weights preserve the measured 0..1.5 mapped range. Unlike the
    // former additive signal/(signal+small threshold), this gentle curve
    // retains changes between ordinary musical levels instead of pinning
    // folds and light near their maximum. At maximum input it stays <0.981.
    let response = signal * 0.85 / (vec3<f32>(1.0) + signal * 0.20);
    // Immediate disconnect is explicit even on a repeated render pass;
    // mappings are gates here, never applied a second time as gains.
    let routed = select(vec3<f32>(0.0), response, u.e.xyz > vec3<f32>(0.0));
    return vec4<f32>(routed,
        clamp(beat(), 0.0, 1.5));
}

fn silk_surface(depth: f32, across: f32, sheet: u32) -> vec3<f32> {
    let time = u.a.z;
    let s = time * 1.18 + depth;
    let section = silk_morph_section(s, sheet);
    // Twin Currents' third sheet has zero width. Collapse its relief too, so
    // the inactive sheet cannot leave an accidental radial fin behind.
    let cloth = across * smoothstep(0.0, 0.20, section.shape.z);
    let audio = silk_sound(s, cloth, sheet);
    let phase = f32(sheet) * 2.0943951;
    let base_angles = array<f32, 3>(0.5235988, 2.6179939, 4.7123890);
    let angle = base_angles[sheet] + section.shape.x + sin(s * 0.527 + phase) * 0.11
        + sin(s * 0.463 + phase) * audio.x * 0.11;
    let outward = vec2<f32>(cos(angle), sin(angle));
    let sideways = vec2<f32>(-outward.y, outward.x);
    // The last part of each sheet curls back through a real rounded hem.
    // Value and first derivative join the inner fabric at |cloth|=0.70.
    // Extra transverse samples below resolve this turn without more vertices.
    let roll_strength = 7.5 + sin(s * 0.541 + phase) * audio.y * 0.90;
    let roll_angle = max(abs(cloth) - 0.70, 0.0) * roll_strength;
    let v = sign(cloth) * (min(abs(cloth), 0.70) + sin(roll_angle) / roll_strength);
    let rolled_hem = (1.0 - cos(roll_angle)) / roll_strength;
    // A moving pressure crest changes broad fabric curvature, not the eye or
    // its clock. Mids lift distinct folds; upper bins make smaller creases.
    let pressure = sin(s * 0.91 - time * 0.23 + v * 1.6 + phase);
    let crease = sin(v * section.folds.y + section.folds.z
        + sin(s * 0.573 + phase) * 0.62
        + sin(s * 0.277 + v * 2.1 + phase) * audio.y * 0.65);
    let middle_fold = sin(s * 1.31 + v * 3.3 + phase);
    let fine_fold = sin(s * 1.1 + cloth * 9.5 - time * 0.41 + phase);
    let billow = sin(s * 0.643 + phase + v * 0.8) * 0.22
        + cos(s * 0.937 - v * 2.4 + phase) * 0.12;
    let width = section.shape.z * (1.0 + audio.y * 0.085)
        * (0.95 + sin(s * 0.593 + phase) * 0.11 + cos(s * 0.347 - phase) * 0.05);
    let musical_displacement = section.shape.y * audio.x * 0.11
        + pressure * audio.x * 0.62
        + (crease * section.folds.x * 0.55 + billow * 0.55 + middle_fold * 0.42) * audio.y;
    // Bass visibly pushes the travelling folds; mids change their curvature
    // and rolled edges. Bound this added displacement smoothly below0.90,
    // preserving positive clearance at maximum mappings and every morph.
    // The camera, world-space phase and quiet geometry do not use its level.
    let bounded_displacement = musical_displacement / (1.0 + abs(musical_displacement) / 0.90);
    let radius = section.shape.y * (1.0 + audio.w * 0.015)
        + crease * section.folds.x + billow * 0.75 + bounded_displacement
        + fine_fold * audio.z * 0.018
        + rolled_hem * section.shape.y * width * (0.86 + sin(s * 0.413 + phase) * 0.14);
    let flat = outward * (radius + v * section.folds.w)
        + sideways * (v * section.shape.y * width);
    let curved = rotate(outward, v * width) * radius;
    let curvature = section.shape.w + audio.y * 0.22
        * (0.65 - section.shape.w + sin(s * 0.431 + phase) * 0.25);
    var point = mix(flat, curved, curvature) * section.scale;
    // Positive clearance is shared by all recipes and all their morphs.
    // The section's radial component stays positive (curled span < pi/2), so
    // no polar branch flip occurs. This smooth inner envelope never caps an
    // outer fold: sheets extend beyond the view instead of fitting a badge.
    let radius_squared = dot(point, point);
    point *= sqrt((radius_squared + 0.85 * 0.85) / max(radius_squared, 0.00001));
    return vec3<f32>(point, -depth);
}

fn silk_pigment(phase: f32) -> vec3<f32> {
    let blue = vec3<f32>(0.025, 0.24, 1.0);
    let violet = vec3<f32>(0.48, 0.025, 0.94);
    let pink = vec3<f32>(1.0, 0.035, 0.34);
    let copper = vec3<f32>(1.0, 0.35, 0.055);
    let weights = pow(0.5 + 0.5 * cos(vec4<f32>(phase)
        - vec4<f32>(0.0, 1.5707963, 3.14159265, 4.712389)), vec4<f32>(3.0));
    return (blue * weights.x + violet * weights.y + pink * weights.z + copper * weights.w)
        / max(dot(weights, vec4<f32>(1.0)), 0.01);
}

// Local interstage data only: the bind groups, uniform layout, texture layers
// and Rust pipeline remain unchanged. Real clip.w gives every material and
// normal perspective-correct interpolation along these deep surfaces.
struct SilkVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) local: vec2<f32>,
    @location(4) sound: vec4<f32>,
    @location(5) @interpolate(flat) sheet: u32,
};

@vertex
fn vs_disco_silk(@builtin(vertex_index) vertex: u32) -> SilkVertex {
    let cell = vertex / 6u;
    // Far-to-near depth slabs, interleaving all rows and all three sheets.
    // This also orders the small translucent distant/edge regions coherently.
    let sheet = cell % 3u;
    let row = (cell / 3u) % 24u;
    let along = 127u - cell / (3u * 24u);
    let corner = quad_corner(vertex) * 0.5 + 0.5;
    let lane = (f32(along) + corner.x) / 128.0;
    // Concentrate transverse vertices on the rolled hems. The middle still
    // carries enough samples for each broad pleat and musical deformation.
    let across = sin(((f32(row) + corner.y) / 24.0 * 2.0 - 1.0) * 1.5707963);
    let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
    let viewport_corner = length(vec2<f32>(aspect + abs(u.g.w), 1.0));
    // At the near cut, even the minimum0.85 clearance projects several times
    // outside the farthest screen corner, including the open sidebar. More
    // samples are placed near the eye; distant geometry ends after its fade.
    let near = 0.30 / viewport_corner;
    let depth = near + lane * lane * 32.0;
    let point = silk_surface(depth, across, sheet);
    let along_tangent = silk_surface(depth + 0.012, across, sheet) - point;
    let across_tangent = silk_surface(depth, across + 0.006, sheet) - point;
    let normal = silk_unit(cross(along_tangent, across_tangent));
    let lens = 1.32;
    var out: SilkVertex;
    out.position = vec4<f32>((point.x * lens + u.g.w * depth) / aspect,
        point.y * lens, depth - 0.01, depth);
    out.point = point;
    out.normal = normal;
    out.tangent = silk_unit(along_tangent);
    out.local = vec2<f32>(u.a.z * 1.18 + depth, across);
    out.sound = silk_sound(out.local.x, across, sheet);
    out.sheet = sheet;
    return out;
}

@fragment
fn fs_disco_silk(in: SilkVertex) -> @location(0) vec4<f32> {
    let footprint = max(fwidth(in.local), vec2<f32>(0.0001));
    let edge = 1.0 - smoothstep(1.0 - footprint.y * 1.1, 1.0, abs(in.local.y));
    let distance_fade = 1.0 - smoothstep(23.0, 31.5, -in.point.z);
    let coverage = edge * distance_fade;
    // An invisible edge must not write depth and hide a farther sheet.
    if coverage <= 0.001 { discard; }
    let view = silk_unit(-in.point);
    var normal = silk_unit(in.normal);
    normal *= select(-1.0, 1.0, dot(normal, view) >= 0.0);
    let tangent = silk_unit(in.tangent - normal * dot(in.tangent, normal));
    let cross_fibre = silk_unit(cross(normal, tangent));
    let light = silk_unit(vec3<f32>(-0.42, 0.58, 0.72));
    let fill = silk_unit(vec3<f32>(0.65, -0.42, 0.63));
    let diffuse = max(dot(normal, light), 0.0);
    let fill_light = max(dot(normal, fill), 0.0);
    let half_light = silk_unit(light + view);
    let hn = max(dot(half_light, normal), 0.0);
    let ht = dot(half_light, tangent);
    let hb = dot(half_light, cross_fibre);
    // A broad, directionally stretched satin reflection, coloured by the
    // fibre. This is a material highlight, not an additive white surface.
    let satin = exp(-(ht * ht * 18.0 + hb * hb * 3.2) / max(hn * hn, 0.04)) * hn;
    // A long softbox reflection follows the actual folded normal and viewing
    // angle. It exposes cloth curvature across broad spans, where a small
    // specular point alone made the material read as uniformly matte panels.
    let reflected_ray = reflect(-view, normal);
    let strip_distance = reflected_ray.x * 0.72 + reflected_ray.y * 0.28 - 0.10;
    let softbox = exp(-strip_distance * strip_distance * 32.0)
        * (0.42 + abs(reflected_ray.z) * 0.58);
    let rim = pow(1.0 - max(dot(normal, view), 0.0), 3.0);
    let phase = in.local.x * 0.31 + f32(in.sheet) * 2.0943951
        + in.local.y * 0.61 + sin(in.local.x * 0.071) * 0.80;
    let pigment = silk_pigment(phase);
    let reflected = silk_pigment(phase + 0.85);
    // Upper frequencies illuminate small travelling creases, not the whole
    // screen. Footprint filtering removes detail that cannot be resolved;
    // the measured high envelope changes amplitude, never a clock or phase.
    let detail_phase = in.local.x * 3.8 + in.local.y * 17.0
        + sin(in.local.y * 4.0 + in.local.x * 0.6) * 1.10 - u.a.z * 0.31;
    let detail_span = footprint.x * 4.46 + footprint.y * 21.4;
    let detail_visibility = 1.0 - smoothstep(0.65, 2.6, detail_span);
    let crease_light = pow(0.5 + 0.5 * sin(detail_phase), 8.0) * detail_visibility;
    let body = pigment * (0.13 + diffuse * 0.68 + fill_light * 0.16)
        * (0.88 + in.sound.x * 0.12 + in.sound.y * 0.16 + in.sound.w * 0.055);
    let highlight = mix(pigment, reflected, 0.42) * (satin * 1.05 + softbox * 0.60)
        * (0.85 + in.sound.z * 0.25)
        + reflected * crease_light * in.sound.z * 0.72
        + pigment * rim * (0.095 + in.sound.z * 0.045);
    let weave_visibility = (1.0 - smoothstep(0.7, 2.8, footprint.x * 38.0))
        * (1.0 - smoothstep(0.7, 2.8, footprint.y * 31.0));
    let weave = sin(in.local.x * 38.0) * cos(in.local.y * 31.0) * weave_visibility;
    return vec4<f32>((body + highlight) * (0.98 + weave * 0.02), coverage);
}
