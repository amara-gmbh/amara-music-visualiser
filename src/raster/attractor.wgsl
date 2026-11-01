// Vertices come from the actual RK4 trajectories in attractor.rs. This stage
// morphs corresponding samples; it does not invent a sine-wave point cloud or
// run an ODE solver in the frame loop. Root handles camera/depth/billboards.

fn attractor_cycle(phase: f32, sharpness: f32) -> vec3<f32> {
    let lobes = vec3<f32>(cos(phase), cos(phase - 2.0943951), cos(phase - 4.1887902));
    let weights = pow(clamp(lobes * 0.5 + 0.5, vec3<f32>(0.0), vec3<f32>(1.0)),
        vec3<f32>(sharpness));
    return weights / max(weights.x + weights.y + weights.z, 0.00001);
}

// xyz are Lorenz/Rossler/Thomas weights; w is a restrained local torsion.
// Pure-system looks remain available alongside continuously changing journeys.
fn attractor_recipe(look: f32, time: f32) -> vec4<f32> {
    let passage = smoothstep(0.08, 0.92, 0.5 + 0.5 * sin(time * 0.12));
    switch u32(clamp(round(look), 0.0, 9.0)) {
        case 1u: { return vec4<f32>(1.0, 0.0, 0.0, 0.0); } // Lorenz Wings
        case 2u: { return vec4<f32>(0.0, 1.0, 0.0, 0.0); } // Rossler Scroll
        case 3u: { return vec4<f32>(0.0, 0.0, 1.0, 0.0); } // Thomas Labyrinth
        case 4u: { return vec4<f32>(1.0 - passage, passage, 0.0, 0.0); } // Wing to Scroll
        case 5u: { return vec4<f32>(1.0 - passage, 0.0, passage, 0.10); } // Labyrinth Wings
        case 6u: { return vec4<f32>(0.0, 1.0 - passage, passage, -0.13); } // Scroll Garden
        case 7u: { return vec4<f32>(attractor_cycle(time * 0.16 + 1.4, 3.0), 0.12); } // Braided Chaos
        case 8u: { // The Lorenz body remains recognizable as its layers unfurl.
            let amount = 0.10 + passage * 0.22;
            return vec4<f32>(1.0 - amount, 0.0, amount, 0.25);
        }
        case 9u: { // Thomas' volumetric lattice opens into the Rossler scroll.
            let amount = 0.08 + passage * 0.32;
            return vec4<f32>(0.0, amount, 1.0 - amount, -0.22);
        }
        default: { return vec4<f32>(attractor_cycle(time * 0.13, 8.0), 0.0); } // Living Attractor
    }
}

fn attractor_audio() -> vec3<f32> {
    // All 48 measured bins contribute to these three RMS groups. The CPU
    // reduces the same mapped two-pole lane once per frame; every vertex only
    // reads its three results. Retained history/current FFT offsets stay fixed.
    if u.raster.z <= 0.0 { return vec3<f32>(0.0); }
    let level = vec3<f32>(dot_history[3120u], dot_history[3121u], dot_history[3122u]);
    // Quiet musical detail remains visible, with no normalization of silence
    // and a strict bound at any input gain. The source's smooth attack/release
    // controls these envelopes; the camera and travel phase never use them.
    return level / (vec3<f32>(0.16, 0.13, 0.08) + level);
}

fn attractor_point(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, index: u32,
    audio: vec3<f32>) -> vec4<f32> {
    let state = u.looks[5];
    let time = u.a.z * 1.40 + state.w * 0.17;
    let blend = clamp(state.z, 0.0, 1.0);
    var recipe = attractor_recipe(state.x, time);
    if blend >= 1.0 {
        recipe = attractor_recipe(state.y, time);
    } else if blend > 0.0 && state.x != state.y {
        recipe = mix(recipe, attractor_recipe(state.y, time), blend);
    }
    // Lorenz's height is its z axis; presenting x/y made the butterfly look
    // like a flattened scribble. Rössler's x/y scroll already faces the view.
    // This is a rigid reorientation of the genuine source paths.
    let wings = vec3<f32>(a.x, a.z, -a.y);
    var p = wings * recipe.x + b * recipe.y + c * recipe.z;
    p *= 1.0 + (1.0 - dot(recipe.xyz, recipe.xyz)) * 0.16;
    // This small, spatially varying torsion is a display deformation of the
    // precomputed path, not a claim to have changed the differential equation.
    if recipe.w != 0.0 {
        let twist = recipe.w * p.y * (0.8 + sin(time * 0.11) * 0.2);
        let folded = rotate(p.xz, twist);
        p = vec3<f32>(folded.x, p.y, folded.y);
    }
    let along = clamp(f32(index) / 11999.0, 0.0, 1.0);
    // Reuse the measured two-pole FFT lane to make frequency-driven bending
    // clearly visible without twitchy line vertices. It is already mapped:
    // never apply the user's band mapping a second time.
    let contour = clamp(equaliser_level(along, 0.0), 0.0, 1.5);
    let spectral_drive = contour / (0.12 + contour);
    let signal = clamp(spectral_drive * 0.48 + audio.x * 0.20
        + audio.y * 0.20 + audio.z * 0.12, 0.0, 1.0);
    // Bass visibly breathes the entire connected envelope. Midrange bends
    // the face as well as its depth: a depth-only twist was almost invisible
    // on the shallow Rossler scroll. Both rotations preserve the radius, so
    // the genuine RK4 path remains identifiable inside a fixed safe camera.
    p *= 1.0 + audio.x * 0.24;
    let face_turn = sin(p.y * 1.85 + p.z * 0.7 - time * 0.23)
        * audio.y * 0.68;
    p = vec3<f32>(rotate(p.xy, face_turn), p.z);
    let folded = rotate(p.xz, sin(p.y * 1.65 + along * 6.283185 - time * 0.34)
        * (audio.y * 0.46 + spectral_drive * 0.09));
    p = vec3<f32>(folded.x, p.y, folded.y);
    // High frequencies add a finer travelling flex, while their luminous
    // packets below follow the same connected curve. All phases use the
    // integrated travel clock, never time multiplied by a changing envelope.
    p += sin(p.zxy * (2.0 + audio.z * 1.8)
        + vec3<f32>(along * 18.849556 - time * 0.43, time * 0.31, -time * 0.37))
        * (audio.y * 0.018 + audio.z * 0.022);
    return vec4<f32>(p, signal);
}

// Keep the broad face of the scroll and wings visible. A full turn repeatedly
// reduced Rössler's flat orbit to an edge-on hairline. Slow parallax and roll
// reveal depth while the light streams carry the faster musical motion.
fn attractor_camera(point: vec3<f32>) -> vec3<f32> {
    let time = u.a.z;
    let yaw = rotate(point.xz, 0.20 + sin(time * 0.095) * 0.40);
    let pitch = rotate(vec2<f32>(point.y, yaw.y), 0.22 + sin(time * 0.071) * 0.15);
    let roll = rotate(vec2<f32>(yaw.x, pitch.x), sin(time * 0.053) * 0.12);
    return vec3<f32>(roll, pitch.y);
}

fn attractor_stream(along: f32, audio: vec3<f32>) -> f32 {
    // Mids open the broad flowing wake; highs reveal fine fast filaments.
    // Their travel phases remain continuous through every musical attack.
    let phase = along * 18.849556 - u.a.z * 0.92;
    let head = pow(0.5 + 0.5 * cos(phase), 28.0);
    let wake = pow(0.5 + 0.5 * cos(phase + 0.42), 5.0);
    let fine = pow(0.5 + 0.5 * cos(along * 94.24778 - u.a.z * 2.7), 20.0);
    return (head * 0.70 + wake * (0.18 + audio.y * 0.48))
        * (0.32 + audio.x * 0.22 + audio.y * 0.36) + fine * audio.z * 1.15;
}

fn attractor_colour(index: u32, point: vec4<f32>, audio: vec3<f32>) -> vec3<f32> {
    let along = clamp(f32(index) / 11999.0, 0.0, 1.0);
    let material = 0.5 + 0.5 * sin(along * 7.4 + point.z * 1.4);
    let cool = mix(vec3<f32>(0.045, 0.66, 0.91), vec3<f32>(0.60, 0.12, 0.83), material);
    let warm = vec3<f32>(1.0, 0.48, 0.12);
    let stream = attractor_stream(along, audio);
    let luminous = mix(cool, warm, clamp(stream * 0.42, 0.0, 0.72));
    // Preserve saturated filament colours. Broad white cores and halos hid
    // the motion of overlapping Lorenz loops in the earlier additive pass.
    return luminous * (0.56 + point.w * 0.75);
}
