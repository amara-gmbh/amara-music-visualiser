// Shared chromatic space behind worlds with a separate foreground. Travel is
// through the existing morphing colour fields themselves: no stars, streaks,
// circular waves or additional foreground objects. The integrated Pace clock
// supplies motion; changing a world/look never resets this environment.

fn chromatic_dye(p: vec2<f32>, phase: f32) -> vec3<f32> {
    let sweep = vec2<f32>(sin(p.y * 1.71 - phase * 0.71),
        cos(p.x * 1.31 + phase * 0.53));
    let q = p + sweep * 0.43;
    let blue_field = 0.5 + 0.5 * sin(q.x * 1.43 + q.y * 0.81
        + sin(q.y * 1.67 - phase * 0.47) * 0.93 - phase * 0.81);
    let violet_field = 0.5 + 0.5 * cos(q.y * 1.47 - q.x * 0.93
        + cos(q.x * 1.83 + phase * 0.41) * 0.79);
    let ember_field = 0.5 + 0.5 * sin(q.x * 0.87 - q.y * 1.63
        + sin(q.x * 1.13 + q.y * 0.91 + phase * 0.59) * 1.17 - phase * 0.63);
    let cyan = vec3<f32>(0.025, 0.70, 0.94);
    let violet = vec3<f32>(0.51, 0.035, 0.84);
    let coral = vec3<f32>(0.98, 0.105, 0.23);
    let gold = vec3<f32>(0.96, 0.50, 0.045);
    let fields = vec3<f32>(blue_field, violet_field, ember_field);
    let squared = fields * fields;
    let weights = squared * squared;
    let warm_material = mix(coral, gold, blue_field * 0.53);
    return (cyan * weights.x + violet * weights.y + warm_material * weights.z)
        / (0.075 + weights.x + weights.y + weights.z);
}

fn chromatic_space(uv: vec2<f32>) -> vec3<f32> {
    let time = u.a.z;
    // Turns belong to the colour volume, never the foreground camera. Their
    // analytic phases continue through world changes and use the integrated
    // Pace clock. Angular velocity eases through zero before reversing.
    let bank = sin(time * 0.137) * 1.60 + sin(time * 0.061 + 0.7) * 0.52;
    let curl = sin(time * 0.097 + 0.4) * 2.20 + sin(time * 0.043) * 0.72;
    let turn = vec2<f32>(sin(time * 0.11 + 0.8), sin(time * 0.073 - 1.1));
    // The origin remains the stage's stable centre while surrounding colour
    // banks and curls around it. No wandering vanishing point or scene shear.
    let view = uv;
    let radius_squared = dot(view, view);
    let outer = radius_squared / (0.65 + radius_squared);
    let fresh = clamp(u.raster.z, 0.0, 1.0);
    let low = clamp(bass(), 0.0, 1.5) * fresh;
    let middle = clamp(mids(), 0.0, 1.5) * fresh;
    let high = clamp(air(), 0.0, 1.5) * fresh;
    // Bass illuminates pigment/relief rather than changing the travelled
    // coordinate. The existing slow bass envelope reinforces sustained body;
    // the mapped DSP band supplies the same route in isolated/audio previews.
    let bass_light = clamp(low * 0.68 + u.music.z * fresh * 0.32, 0.0, 1.5)
        * select(0.0, 1.0, u.e.x > 0.0);
    var colour = vec3<f32>(0.012, 0.017, 0.047)
        + chromatic_dye(rotate(view, bank * 0.28 + curl * outer * 0.35) * 0.40,
            time * 0.025) * (0.105 + bass_light * 0.036);

    // Persistent folds emerge near the centre at a small projected scale,
    // visibly broaden on approach, then pass beyond the screen edges. Their
    // soft colour boundaries supply the depth cues themselves: no stars,
    // streaks, rings or extra foreground geometry. Low ambient illumination
    // keeps those near/far silhouettes legible through the original dyes.
    let travel = time * 2.3;
    let spacing = 4.8;
    let first = floor(travel / spacing);
    for (var layer = 0; layer < 6; layer += 1) {
        let identity = first + 6.0 - f32(layer);
        let world_depth = identity * spacing;
        let depth = world_depth - travel;
        // The entering and departing identities are both exactly invisible
        // at rollover; every other colour stratum keeps its world position.
        let visible = (1.0 - smoothstep(20.0, 24.0, depth))
            * smoothstep(0.25, 1.2, depth);
        let seed = vec3<f32>(hash(vec3<f32>(identity, 23.4, 5.1)),
            hash(vec3<f32>(identity, 8.7, 61.3)), hash(vec3<f32>(identity, 47.2, 3.8)));
        let approach = 1.0 - smoothstep(2.0, 20.0, depth);
        // Distant folds turn gently; nearby folds sweep past with greater
        // angular travel. Radius-dependent curl exposes the turn as a curved
        // passage rather than rotating a flat wallpaper. The broad outward
        // sweep changes horizontal and vertical direction continuously too.
        let curved_view = rotate(view,
            bank * (0.28 + approach * 0.72)
                + curl * log(1.0 + radius_squared * 2.4) * (0.26 + approach * 0.54))
            + turn * outer * (0.08 + approach * 0.28);
        // Neighbouring depths share a winding current. Bounded variation
        // avoids six unrelated flat washes; no circular wall or ring is drawn.
        let stream_turn = world_depth * 0.145 + (seed.z - 0.5) * 0.95;
        let space = rotate(curved_view * depth * 0.30, stream_turn);
        let phase = world_depth * 0.11 + time * 0.035;
        let bend_a = space.x * (0.67 + seed.z * 0.16) + seed.x * 6.283185;
        let bend_b = space.x * 1.37 + phase * 0.35;
        // A nonzero mean offset lets a fold travel past the viewer instead
        // of remaining a stationary wash over the central subject.
        let anchor = 0.92 + seed.y * 0.70;
        let bend = anchor + sin(bend_a) * 0.55 + sin(bend_b) * 0.20;
        // Music breathes the broad colour relief. Camera velocity/position
        // remain independent of fast energy changes, avoiding forward jolts.
        let width = 0.33 + seed.x * 0.16 + low * 0.035;
        let cross = (space.y - bend) / width;
        // A defined fold inside a softer veil remains traceable as it grows
        // towards the viewer. Uniformly broad blur hid that optical expansion.
        let coverage = (exp(-cross * cross * 0.85) * 0.72
            + exp(-cross * cross * 0.16) * 0.18) * visible;
        let slope = cos(bend_a) * (0.67 + seed.z * 0.16) * 0.55
            + cos(bend_b) * 0.274;
        let normal = normalize(vec3<f32>(-slope * 0.35, cross * 0.55, 1.0));
        let relief = 0.48 + 0.52 * max(dot(normal, vec3<f32>(-0.39, -0.48, 0.786)), 0.0);
        let dye_space = space * 0.73 + vec2<f32>(seed.x * 2.7, seed.y * 3.3);
        let dye = chromatic_dye(dye_space, phase);
        let body = dye * (0.31 + approach * 0.40 + bass_light * 0.27
            + middle * 0.035 + high * 0.018) * relief;
        colour = mix(colour, body, coverage);
        // Broad translucent sheen preserves the shape of colour folds. No
        // narrow vein/point survives as a separate line or sparkle layer.
        let shoulder_distance = cross + 0.32;
        let shoulder = exp(-shoulder_distance * shoulder_distance * 1.7);
        colour += dye * shoulder * visible * (0.040 + approach * 0.050
            + bass_light * 0.081 + middle * 0.014 + high * 0.018);
    }
    // Keep sapphire, violet and ember distinct instead of adding them into
    // white fog. Common background tint and Mocca conversion stay downstream.
    let grey_floor = min(colour.x, min(colour.y, colour.z));
    let saturated = (colour - vec3<f32>(grey_floor * 0.78)) * 1.28;
    return clamp(saturated, vec3<f32>(0.009, 0.014, 0.038), vec3<f32>(0.88));
}
