// Feedback Reverie is original WGSL inspired by MilkDrop's documented workflow:
// draw musical marks, animate per-pixel history coordinates, composite, repeat.
// These curves use measured spectrum envelopes, not a claimed PCM waveform.

fn reverie_velocity(p: vec2<f32>, time: f32) -> vec2<f32> {
    let q = p * 1.45 + vec2<f32>(time * 0.23, -time * 0.19);
    let v = vec2<f32>(sin(q.x) * cos(q.y), -cos(q.x) * sin(q.y));
    let r = rotate(p, 0.73) * 2.85 + vec2<f32>(-time * 0.17, time * 0.13 + 2.0);
    let fine = vec2<f32>(sin(r.x) * cos(r.y), -cos(r.x) * sin(r.y));
    return v * 0.72 + rotate(fine, -0.73) * 0.28;
}

// Independent mathematical emitters, not ten palette variations. Values carry
// field and analytic gradient so all modes share filtered, sculpted engraving.
fn reverie_pattern(p: vec2<f32>, time: f32, id: f32) -> vec3<f32> {
    if id < 0.5 {
        let x = p.x * 2.25 + 0.28 * sin(time * 0.093);
        let y = p.y * 2.65 - time * 0.115;
        let a = x * 0.73 - y * 0.81 + time * 0.13;
        let b = x * 0.80 + y * 0.60 - time * 0.11;
        return vec3<f32>(sin(x) * cos(y) + cos(a) * 0.42 + sin(b) * 0.28,
            (cos(x) * cos(y) - sin(a) * 0.3066 + cos(b) * 0.224) * 2.25,
            (-sin(x) * sin(y) + sin(a) * 0.3402 + cos(b) * 0.168) * 2.65);
    }
    if id < 1.5 {
        // Chladni standing-wave modes; measured mids change their mixture.
        let q = p * 3.14159265;
        let a = cos(q.x * 2.0) * cos(q.y * 3.0) - cos(q.x * 3.0) * cos(q.y * 2.0);
        let b = cos(q.x * 4.0) * cos(q.y * 5.0) - cos(q.x * 5.0) * cos(q.y * 4.0);
        let da = vec2<f32>(-2.0 * sin(q.x * 2.0) * cos(q.y * 3.0) + 3.0 * sin(q.x * 3.0) * cos(q.y * 2.0),
            -3.0 * cos(q.x * 2.0) * sin(q.y * 3.0) + 2.0 * cos(q.x * 3.0) * sin(q.y * 2.0));
        let db = vec2<f32>(-4.0 * sin(q.x * 4.0) * cos(q.y * 5.0) + 5.0 * sin(q.x * 5.0) * cos(q.y * 4.0),
            -5.0 * cos(q.x * 4.0) * sin(q.y * 5.0) + 4.0 * cos(q.x * 5.0) * sin(q.y * 4.0));
        let mix_mode = 0.25 + sin(time * 0.17) * 0.15 + mids() * 0.15;
        return vec3<f32>(mix(a, b, mix_mode) * 0.65, mix(da, db, mix_mode) * 2.042035);
    }
    if id < 2.5 || (id >= 4.5 && id < 5.5) {
        let r2 = max(dot(p, p), 0.015);
        let r = sqrt(r2);
        let angle = atan2(p.y, p.x + 0.000001);
        let arms = select(5.0, 3.0, id >= 4.5);
        let phase = angle * arms + log(1.0 + r * 3.0) * 9.0 - time * 0.27;
        let grad_phase = vec2<f32>(-p.y, p.x) * arms / r2
            + p / r * (27.0 / (1.0 + r * 3.0));
        let secondary = r * 6.0 - time * 0.18;
        let amplitude = select(0.62, 0.82, id >= 4.5);
        return vec3<f32>(sin(phase) * amplitude + cos(secondary) * 0.28,
            grad_phase * cos(phase) * amplitude - p / r * sin(secondary) * 1.68);
    }
    if id < 3.5 {
        // Interference from oblique plane waves, not more outward rings.
        let a = p.x * 4.8 + p.y * 2.2 + sin(time * 0.09) * 0.8;
        let b = p.x * -2.7 + p.y * 5.1 - time * 0.14;
        let modulation = 0.35 + clamp(mids(), 0.0, 1.5) * 0.24;
        return vec3<f32>(sin(a) * cos(b) + sin(a + b) * modulation,
            cos(a) * cos(b) * vec2<f32>(4.8, 2.2)
                - sin(a) * sin(b) * vec2<f32>(-2.7, 5.1)
                + cos(a + b) * vec2<f32>(2.1, 7.3) * modulation);
    }
    if id < 4.5 {
        let a = (p.x * p.x - p.y * p.y) * 3.2 + time * 0.16;
        let b = p.x * p.y * 6.4 - time * 0.22;
        return vec3<f32>(sin(a) * 0.62 + cos(b) * 0.55,
            vec2<f32>(p.x, -p.y) * (cos(a) * 3.968)
                - vec2<f32>(p.y, p.x) * (sin(b) * 3.52));
    }
    if id < 6.5 {
        let q = p * 3.6; let z = time * 0.14;
        let f = sin(q.x) * cos(q.y) + sin(q.y) * cos(z) + sin(z) * cos(q.x);
        let g = vec2<f32>(cos(q.x) * cos(q.y) - sin(z) * sin(q.x),
            -sin(q.x) * sin(q.y) + cos(q.y) * cos(z));
        return vec3<f32>(f * 0.68, g * 2.448);
    }
    if id < 7.5 {
        let q = p * 2.2;
        let cell = floor(q); let local = fract(q);
        var first = 10.0; var second = 10.0;
        var ga = vec2<f32>(0.0); var gb = vec2<f32>(0.0);
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let offset = vec2<f32>(f32(x), f32(y));
                let h = hash(vec3<f32>(cell + offset, 3.7));
                let node = offset + vec2<f32>(0.5) + sin(vec2<f32>(h * 7.3, h * 9.1) + time * 0.17) * 0.3;
                let delta = local - node;
                let distance = max(length(delta), 0.0001);
                if distance < first {
                    second = first; gb = ga; first = distance; ga = delta / distance;
                } else if distance < second { second = distance; gb = delta / distance; }
            }
        }
        return vec3<f32>((second - first) * 2.5 - 0.65, (gb - ga) * 5.5);
    }
    if id < 8.5 {
        var field = 0.0; var gradient = vec2<f32>(0.0);
        // Five incommensurate directions create a quasicrystalline weave.
        // Different measured bins independently strengthen its five families.
        for (var index = 0; index < 5; index++) {
            let i = f32(index);
            let direction = vec2<f32>(cos(i * 1.256637), sin(i * 1.256637));
            let frequency = 6.2;
            let amplitude = 0.17 + spectrum_at(i * 0.23) * 0.19;
            let phase = dot(p, direction) * frequency
                + sin(time * (0.045 + i * 0.007) + i) * 0.70;
            field += cos(phase) * amplitude;
            gradient -= direction * sin(phase) * frequency * amplitude;
        }
        return vec3<f32>(field, gradient);
    }
    let a = p.x * 5.2 + time * 0.11;
    let b = (p.x * 0.5 + p.y * 0.8660254) * 5.2 - time * 0.13;
    let c = (-p.x * 0.5 + p.y * 0.8660254) * 5.2 + time * 0.17;
    return vec3<f32>((cos(a) + cos(b) + cos(c)) * 0.46,
        vec2<f32>(-sin(a) - sin(b) * 0.5 + sin(c) * 0.5,
            -(sin(b) + sin(c)) * 0.8660254) * 2.392);
}

fn reverie_recipe(id: f32) -> vec4<f32> {
    let recipes = array<vec4<f32>, 10>(
        vec4<f32>(1.0, 0.0, 1.0, 1.0), vec4<f32>(0.45, -0.01, 0.5, 0.22),
        vec4<f32>(1.65, -0.025, 0.8, 0.30), vec4<f32>(0.7, 0.015, 0.7, 0.45),
        vec4<f32>(0.35, -0.005, 1.1, 0.28), vec4<f32>(1.85, -0.03, 0.5, 0.20),
        vec4<f32>(0.8, 0.008, 1.3, 0.28), vec4<f32>(0.3, -0.008, 0.65, 0.20),
        vec4<f32>(0.5, 0.01, 0.6, 0.35), vec4<f32>(0.4, -0.005, 0.8, 0.22));
    return recipes[u32(clamp(round(id), 0.0, 9.0))];
}

fn reverie_look() -> vec4<f32> {
    let look = u.looks[3];
    return mix(reverie_recipe(look.x), reverie_recipe(look.y), clamp(look.z, 0.0, 1.0));
}

// Each field keeps its own visual grammar: standing nodes, solid gyroid
// sheets, foam walls or filigree. They no longer all wear identical contour
// stacks. Style coefficients interpolate with the scalar geometry itself.
fn reverie_style_recipe(id: f32) -> vec4<f32> {
    let styles = array<vec4<f32>, 10>(
        vec4<f32>(1.0, 1.0, 1.0, 1.0),
        vec4<f32>(0.12, 0.10, 0.24, 2.4),
        vec4<f32>(0.55, 1.0, 0.70, 1.15),
        vec4<f32>(0.25, 0.24, 0.80, 1.65),
        vec4<f32>(0.24, 0.60, 0.50, 1.70),
        vec4<f32>(0.48, 0.50, 1.20, 1.30),
        vec4<f32>(0.38, 0.16, 2.4, 1.40),
        vec4<f32>(0.18, 0.04, 1.45, 2.1),
        vec4<f32>(0.10, 0.12, 0.35, 2.2),
        vec4<f32>(0.14, 0.20, 1.5, 1.7));
    return styles[u32(clamp(round(id), 0.0, 9.0))];
}

fn reverie_style() -> vec4<f32> {
    let state = u.looks[3];
    return mix(reverie_style_recipe(state.x), reverie_style_recipe(state.y),
        clamp(state.z, 0.0, 1.0));
}

// Four smooth, non-linear curl maps carry their screen-space Jacobian with
// them. The resulting level sets can open, divide and rejoin as their saddle
// level moves; they are not a stack of rigidly rotating decorative curves.
// Keeping the derivative also lets the finest engraving fade before aliasing.
fn reverie_liquid(p: vec2<f32>, time: f32) -> vec3<f32> {
    var q = p;
    var dx = vec2<f32>(1.0, 0.0);
    var dy = vec2<f32>(0.0, 1.0);
    let tension = (0.25 + u.d.z * 0.08 + u.h.x * 0.06 + mids() * 0.035)
        * reverie_style().x;
    for (var octave = 0; octave < 4; octave += 1) {
        let i = f32(octave);
        let angle = 0.71 * i + 0.16 * sin(time * (0.077 + i * 0.011));
        let frequency = 1.30 + i * 0.83;
        let r = rotate(q, angle) * frequency
            + vec2<f32>(time * (0.12 + i * 0.017), -time * (0.095 + i * 0.013) + i * 1.73);
        let sx = sin(r.x);
        let sy = sin(r.y);
        let cx = cos(r.x);
        let cy = cos(r.y);
        let curl = vec2<f32>(sx * cy, -cx * sy);
        let xx = rotate(dx, angle) * frequency;
        let yy = rotate(dy, angle) * frequency;
        let curl_dx = vec2<f32>(cx * cy * xx.x - sx * sy * xx.y,
            sx * sy * xx.x - cx * cy * xx.y);
        let curl_dy = vec2<f32>(cx * cy * yy.x - sx * sy * yy.y,
            sx * sy * yy.x - cx * cy * yy.y);
        let amplitude = tension / (1.0 + i * 0.64);
        q += rotate(curl, -angle) * amplitude;
        dx += rotate(curl_dx, -angle) * amplitude;
        dy += rotate(curl_dy, -angle) * amplitude;
    }
    let look = u.looks[3];
    var pattern = reverie_pattern(q, time, look.y);
    if look.z < 1.0 && look.x != look.y {
        pattern = mix(reverie_pattern(q, time, look.x), pattern, look.z);
    }
    return vec3<f32>(pattern.x, dot(pattern.yz, dx), dot(pattern.yz, dy));
}

fn reverie_level() -> f32 {
    let time = u.a.z;
    return sin(time * 0.157) * 0.29 + sin(time * 0.233 + 1.1) * 0.18
        + (u.h.x - 0.35) * 0.13 + bass() * 0.055;
}

fn feedback_render(uv: vec2<f32>, liquid: vec3<f32>) -> World {
    let time = u.a.z;
    let detail = detail_level();
    var p = uv;
    // Composed curl maps continuously change the emitted ribbons themselves;
    // the old frame has a separate evolving coordinate map below.
    for (var fold = 0; fold < 3; fold += 1) {
        let i = f32(fold);
        p += reverie_velocity(p * (1.0 + i * 0.45), time + i * 3.7)
            * (0.15 + u.d.z * 0.055 + u.h.x * 0.045) / (1.0 + i * 0.55);
    }
    var light = vec3<f32>(0.002, 0.002, 0.009);
    var marks = vec3<f32>(0.0);
    for (var layer = 0; layer < 8; layer += 1) {
        let i = f32(layer);
        let phase = time * (0.22 + i * 0.009) + i * 0.71;
        let orientation = 0.28 * sin(time * 0.11) + i * 0.11
            + select(0.0, 1.32 + sin(time * 0.071) * 0.18, layer >= 4);
        let q = rotate(p, orientation);
        let spectrum_position = clamp(q.x * 0.24 + 0.5 + sin(i * 1.9) * 0.08, 0.0, 1.0);
        let signal = spectrum_at(spectrum_position);
        let bend = sin(q.x * (1.35 + sin(time * 0.083) * 0.16) + phase) * 0.29
            + sin(q.x * 3.1 - time * 0.17 + i * 0.9) * (0.10 + mids() * 0.025);
        let centre = (fract(i * 0.25) - 0.375) * 0.60;
        let distance = q.y - bend - centre - signal * (0.12 + bass() * 0.025);
        let width = 0.012 + detail * 0.044 + signal * 0.009;
        let sheet = exp(-distance * distance / (width * width));
        let fibre_frequency = 110.0 + detail * 70.0;
        let fibres = pow(0.5 + 0.5 * filtered_sine(
            distance * fibre_frequency + q.x * 3.0 + phase,
            pixel() * fibre_frequency * (1.2 + u.d.z * 0.6)), 2.0);
        let taper = exp(-pow(abs(q.x) * 0.38, 4.0));
        let colour = mix(vec3<f32>(0.055, 0.28, 0.66), vec3<f32>(0.48, 0.055, 0.58), i / 7.0);
        let presence = 0.18 + signal * 0.78 + mids() * 0.14 + bass() * 0.08;
        marks += colour * sheet * (0.28 + fibres * 0.72) * taper * presence;
        let edge = wire(distance - width * 0.65, pixel() * 0.55);
        marks += mix(colour, vec3<f32>(0.67, 0.85, 0.95), 0.65) * edge * taper
            * (0.11 + signal * 0.52 + air() * 0.13);
        let reverse_edge = wire(distance + width * 0.60, pixel() * 0.45);
        marks += colour * reverse_edge * taper * detail * (0.10 + signal * 0.30);
        // Independent moving highlights run along actual musical contours.
        let crest = pow(0.5 + 0.5 * sin(q.x * 6.0 - time * 0.29 + i), 18.0);
        marks += vec3<f32>(1.0, 0.58, 0.18) * edge * crest * taper
            * (0.025 + beat() * 0.12 + signal * 0.10);
        light += colour * exp(-abs(distance) * 28.0) * taper * 0.018;
    }
    let recipe = reverie_look();
    light *= recipe.w;
    marks *= recipe.w;
    // Living liquid engraving. Passing the level through the field's saddles
    // genuinely splits/merges pools; logarithmic contour spacing nests fine
    // strata around their lips while leaving the deeper cavities unfilled.
    let threshold = reverie_level();
    let style = reverie_style();
    let distance = liquid.x - threshold;
    let gradient = liquid.yz;
    let contour_scale = 9.0 + detail * 17.0;
    let phase = log(1.0 + abs(distance) * 4.5) * contour_scale - time * 0.14;
    let phase_gradient = gradient * sign(distance) * 4.5 * contour_scale
        / (1.0 + abs(distance) * 4.5);
    let footprint = length(phase_gradient) * pixel();
    let resolved = 1.0 - smoothstep(1.2, 3.0, footprint);
    let fine_contour = (1.0 - smoothstep(0.055, 0.10 + footprint * 0.65, abs(sin(phase))))
        * resolved;
    let terraces = 0.5 + 0.5 * cos(phase);
    let ridge = exp(-abs(distance) * 3.2);
    let height_gradient = gradient * (-sign(distance) * ridge * 1.3)
        + phase_gradient * sin(phase) * resolved * 0.018;
    let normal = normalize(vec3<f32>(height_gradient, 1.0));
    let relief = 0.24 + 0.76 * max(dot(normal, normalize(vec3<f32>(-0.45, 0.65, 0.75))), 0.0);
    let cavity = smoothstep(-0.74, -0.08, distance);
    let pools = 0.32 + 0.68 * smoothstep(-0.15, 0.65, liquid.x);
    let spectrum_detail = spectrum_at(clamp(uv.y * 0.30 + 0.5, 0.0, 1.0));
    let mineral = mix(vec3<f32>(0.035, 0.17, 0.40), vec3<f32>(0.34, 0.045, 0.40),
        smoothstep(-0.8, 0.95, liquid.x + sin(time * 0.073) * 0.20));
    light += mineral * (0.045 + ridge * 0.32 + terraces * 0.09 * resolved * style.y) * relief * cavity * style.z
        * (0.65 + spectrum_detail * 0.50 + mids() * 0.13);
    marks += mix(mineral, vec3<f32>(0.54, 0.80, 0.94), 0.58)
        * fine_contour * relief * pools * (0.16 + spectrum_detail * 0.50 + air() * 0.14) * style.y;
    let lip = wire(distance / max(length(gradient), 0.20), pixel() * (0.7 + detail * 0.45));
    marks += mix(vec3<f32>(0.62, 0.84, 1.0), vec3<f32>(1.0, 0.66, 0.26),
        pow(0.5 + 0.5 * sin(uv.x * 2.8 + time * 0.18), 5.0) * 0.42)
        * lip * (0.18 + spectrum_detail * 0.46 + beat() * 0.08) * style.w;
    return World(light, marks * (0.70 + u.d.y * 0.45));
}

fn feedback_reverie(uv: vec2<f32>) -> World {
    return feedback_render(uv, reverie_liquid(uv, u.a.z));
}

fn feedback_flow(uv: vec2<f32>) -> vec2<f32> {
    let time = u.a.z;
    let advance = clamp(u.g.y, 0.0, 0.1) * u.c.z * u.i.y;
    let centre = vec2<f32>(sin(time * 0.137) * 0.23, cos(time * 0.113) * 0.17);
    let p = uv - centre;
    let radius2 = dot(p, p);
    let recipe = reverie_look();
    // Radially varying rotation, breathing zoom, reciprocal stretch and curl
    // evolve at staggered frequencies. Stored ink keeps stretching and curling,
    // rather than merely showing a static ornament with a trailing duplicate.
    let angular = (0.075 * sin(time * 0.173) + 0.11 / (0.7 + radius2))
        * (0.75 + u.h.x * 0.45) * recipe.x;
    let zoom = exp(advance * (0.045 * sin(time * 0.191) - 0.016 + recipe.y));
    let stretch = exp(advance * 0.032 * sin(time * 0.127));
    var warped = rotate(p, advance * angular) * vec2<f32>(stretch, 1.0 / stretch) * zoom;
    warped -= reverie_velocity(p, time) * advance * (0.065 + u.d.z * 0.070) * recipe.z;
    return warped + centre;
}

fn feedback_memory() -> f32 {
    return 0.94;
}
