// Mocha Atelier: a flowing, full-frame landscape of cream, coffee and bronze.
// Its material colours are taken from the native Mocca & Vanilla palette in
// shader.wgsl. The geometry is an advected family of folded marble/satin fields
// rather than a radial wave, a particle overlay, or the stained-glass world.
// Low frequencies bend the broad folds, mids sculpt their seams and local high
// bins illuminate their fine capillary detail. All recipe parameters crossfade
// continuously, so changing a look does not restart the moving surface.

struct MochaLook {
    domain: vec4<f32>, // horizontal/vertical scale, angle, shear
    flow: vec4<f32>,   // broad curl, secondary curl, fold frequency, fold angle
    form: vec4<f32>,   // layered grain, flowing braid, satin sweep, veining
    finish: vec4<f32>, // coffee depth, bronze warmth, highlight, fine detail
};

fn mocha_recipe(id: f32) -> MochaLook {
    switch u32(clamp(round(id), 0.0, 9.0)) {
        case 1u: { // Espresso Ribbons
            return MochaLook(vec4<f32>(0.78, 1.30, -0.30, 0.22),
                vec4<f32>(0.48, 0.25, 4.1, 0.18), vec4<f32>(0.25, 0.90, 0.10, 0.40),
                vec4<f32>(0.88, 0.58, 0.75, 0.72));
        }
        case 2u: { // Vanilla Drift
            return MochaLook(vec4<f32>(0.74, 0.77, 0.08, 0.04),
                vec4<f32>(0.44, 0.52, 2.0, -0.24), vec4<f32>(0.25, 0.12, 0.72, 0.30),
                vec4<f32>(0.42, 0.24, 0.88, 0.48));
        }
        case 3u: { // Copper Creases
            return MochaLook(vec4<f32>(1.12, 0.92, 0.48, -0.26),
                vec4<f32>(0.27, 0.30, 5.7, 0.26), vec4<f32>(0.34, 0.70, 0.30, 0.80),
                vec4<f32>(0.74, 0.94, 0.95, 0.96));
        }
        case 4u: { // Crema Canopy
            return MochaLook(vec4<f32>(0.78, 1.02, -0.16, 0.38),
                vec4<f32>(0.74, 0.27, 2.8, -0.22), vec4<f32>(0.42, 0.22, 0.82, 0.38),
                vec4<f32>(0.55, 0.30, 0.92, 0.58));
        }
        case 5u: { // Mocha Marble
            return MochaLook(vec4<f32>(1.08, 1.07, 0.12, 0.10),
                vec4<f32>(0.68, 0.52, 3.5, 0.18), vec4<f32>(0.92, 0.28, 0.12, 1.00),
                vec4<f32>(0.78, 0.42, 0.62, 0.98));
        }
        case 6u: { // Silk Espresso
            return MochaLook(vec4<f32>(0.68, 1.58, -0.58, 0.12),
                vec4<f32>(0.35, 0.16, 5.1, -0.35), vec4<f32>(0.08, 0.72, 0.78, 0.30),
                vec4<f32>(0.65, 0.46, 1.00, 0.60));
        }
        case 7u: { // Roasted Whorls
            return MochaLook(vec4<f32>(1.05, 0.95, 0.18, -0.08),
                vec4<f32>(0.88, 0.65, 2.7, 0.70), vec4<f32>(0.52, 0.26, 0.25, 0.68),
                vec4<f32>(0.90, 0.40, 0.72, 0.68));
        }
        case 8u: { // Pearl Weave
            return MochaLook(vec4<f32>(1.45, 0.86, 0.66, -0.25),
                vec4<f32>(0.31, 0.36, 4.8, 0.12), vec4<f32>(0.46, 0.94, 0.18, 0.54),
                vec4<f32>(0.56, 0.35, 1.00, 0.95));
        }
        case 9u: { // Honeyed Passage
            return MochaLook(vec4<f32>(0.86, 0.74, -0.38, 0.16),
                vec4<f32>(0.62, 0.42, 2.6, 0.46), vec4<f32>(0.50, 0.30, 0.55, 0.42),
                vec4<f32>(0.50, 0.80, 0.86, 0.70));
        }
        default: { // Crema Current
            return MochaLook(vec4<f32>(1.0, 0.92, 0.0, 0.08),
                vec4<f32>(0.56, 0.36, 3.2, 0.12), vec4<f32>(0.56, 0.50, 0.42, 0.62),
                vec4<f32>(0.66, 0.42, 0.80, 0.76));
        }
    }
}

fn mocha_world(uv: vec2<f32>, look_state: vec4<f32>) -> World {
    let before = mocha_recipe(look_state.x);
    let after = mocha_recipe(look_state.y);
    let amount = clamp(look_state.z, 0.0, 1.0);
    let domain = mix(before.domain, after.domain, amount);
    let flow = mix(before.flow, after.flow, amount);
    let form = mix(before.form, after.form, amount);
    let finish = mix(before.finish, after.finish, amount);
    let time = u.a.z;
    let live = select(0.0, 1.0, u.raster.z > 0.0);
    let low = (1.0 - exp(-max(0.0, bass()) * 1.5)) * live;
    let mid = (1.0 - exp(-max(0.0, mids()) * 1.5)) * live;
    let high = (1.0 - exp(-max(0.0, air()) * 1.5)) * live;
    let beat_envelope = clamp(beat(), 0.0, 1.0) * live;

    // A continuously turning camera and two independently advected currents
    // provide a sense of travelling through a material, without a tunnel.
    let camera_angle = domain.z + 0.12 * sin(time * 0.043)
        + 0.06 * sin(time * 0.071 + 1.3);
    var p = rotate(uv, camera_angle) * domain.xy;
    p.x += p.y * domain.w;
    p += vec2<f32>(sin(time * 0.071) * 0.26, cos(time * 0.053) * 0.23);
    let radius = dot(p, p);
    p = rotate(p, flow.w / (1.4 + radius) + sin(time * 0.035) * 0.14);
    let local_mid = spectrum_at(clamp(0.31 + 0.15 * sin(p.x * 0.8 + p.y * 0.5), 0.16, 0.49)) * live;
    let local_high = spectrum_at(clamp(0.75 + 0.18 * sin(p.y * 0.9 - p.x * 0.4), 0.52, 0.97)) * live;
    let drift = time * 0.19;
    let current1 = vec2<f32>(sin(p.y * 1.8 + drift), cos(p.x * 1.55 - drift * 0.78));
    let current2 = vec2<f32>(sin(p.x * 1.17 + p.y * 1.31 - drift * 0.67),
        cos(p.y * 1.11 - p.x * 1.37 + drift * 0.61));
    p += current1 * (flow.x * 0.42 + low * 0.22)
        + current2 * (flow.y * 0.32 + mid * 0.08);

    // Three mathematical structures blend as material, rather than cutting
    // abruptly between effect programs on a recipe change.
    let broad_phase = p.y * flow.z + sin(p.x * 1.8 + drift * 0.52) * (1.05 + low * 0.55)
        + sin(p.x * 3.2 - drift * 0.33) * (0.30 + local_mid * 0.30) - drift;
    let braid_phase = p.x * (2.0 + form.y * 2.3) - p.y * 1.8
        + sin(p.y * 2.0 + drift * 0.55) * (1.0 + mid * 0.45) + drift * 0.62;
    let marble_phase = p.x * 2.1 + p.y * 1.5
        + sin(p.y * 3.15 - drift * 0.43 + sin(p.x * 2.05 + drift * 0.28))
            * (1.0 + form.x * 1.7) - drift * 0.22;
    let fold = sin(broad_phase);
    let braid = sin(braid_phase);
    let marble = sin(marble_phase);
    let satin = cos(broad_phase * 0.52 + sin(braid_phase * 0.50) * 0.8);
    let field = clamp(fold * (0.48 + form.z * 0.18)
        + braid * form.y * 0.24 + marble * form.x * 0.25 + satin * form.z * 0.16,
        -1.45, 1.45);
    // Keep broad areas of both cream and espresso visible after the shared
    // exposure/tonemapping: a narrow, middle-heavy mix washes out to taupe.
    let pigment = smoothstep(-0.48, 0.52, field);
    let cream = vec3<f32>(0.96, 0.91, 0.79);
    let bronze = vec3<f32>(0.62, 0.44, 0.23);
    let coffee = vec3<f32>(0.109804, 0.094118, 0.074510);
    let dark = mix(coffee * 1.20, bronze * 0.65, 0.10 + finish.y * 0.17);
    var material = mix(cream, dark, pigment * (0.88 + finish.x * 0.12));
    let illuminated_fold = pow(max(0.0, 0.5 + 0.5 * cos(broad_phase + 0.55)), 5.0);
    material += cream * illuminated_fold * (0.14 + finish.z * 0.20 + low * 0.08);

    // A finite projected line width prevents high-frequency detail aliasing.
    let vein_phase = marble_phase * (2.0 + form.w * 3.4)
        + sin(braid_phase * 0.7) * (1.1 + local_mid * 0.8);
    let frequency = (2.0 + form.w * 3.4) * (3.0 + flow.z) + 3.0;
    let resolved = 1.0 - smoothstep(0.45, 1.5, pixel() * frequency);
    let line_width = max(0.04, pixel() * frequency * 1.15);
    let vein = (1.0 - smoothstep(line_width, line_width * 2.8, abs(sin(vein_phase)))) * resolved;
    let lace_phase = braid_phase * 3.1 + sin(marble_phase * 1.6) * 1.5;
    let fine = (1.0 - smoothstep(line_width * 0.65, line_width * 2.1, abs(sin(lace_phase))))
        * resolved * (0.20 + detail_level() * 0.80);
    let mid_light = 0.26 + mid * 0.24 + local_mid * 0.22;
    let high_light = 0.12 + high * 0.24 + local_high * 0.28 + beat_envelope * 0.025;
    let warm_trace = mix(bronze, cream, 0.45 + finish.z * 0.28);
    let marks = warm_trace * (vein * form.w * finish.w * mid_light
        + fine * finish.w * high_light);
    return World(material * 0.64, material * 0.16 + marks);
}
