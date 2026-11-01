// Deep Flight: a camera travelling through a persistent mathematical space.
// Absolute world depth anchors a curved luminous passage, irregular broken
// ribbon chambers and parallax dust. No source asset or preset is embedded.

struct FlightLook {
    shape: vec4<f32>, // bore scale, closed angular harmonics 2 / 3 / 5
    weave: vec4<f32>, // harmonic 8, spatial twist, ribbon width, arch closure
    space: vec4<f32>, // rail radius, canyon opening, paired helix, inlay density
    relic: vec4<f32>, // chamber scale, fold depth, local tilt, neck modulation
};

fn flight_recipe(index: u32) -> FlightLook {
    switch index {
        // Harmonic Vaults: the original Deep Flight structure.
        case 0u: { return FlightLook(vec4<f32>(1.0, 1.0, 1.0, 1.0), vec4<f32>(0.0, 1.0, 1.0, 0.0),
            vec4<f32>(1.0, 0.0, 0.0, 1.0), vec4<f32>(1.0)); }
        // Braided Currents: sparse shells open around tight flowing rails.
        case 1u: { return FlightLook(vec4<f32>(0.90, 0.25, 0.35, 0.4), vec4<f32>(0.0, 2.35, 0.32, -0.35),
            vec4<f32>(1.15, 0.0, 0.0, 0.7), vec4<f32>(0.85, 0.8, 1.5, 0.6)); }
        // Trefoil Gardens: three-lobed chambers and broad knotted relics.
        case 2u: { return FlightLook(vec4<f32>(0.95, 0.12, 2.65, 0.3), vec4<f32>(0.5, 0.65, 0.70, 0.6),
            vec4<f32>(0.75, 0.0, 0.0, 0.8), vec4<f32>(1.3, 1.5, 1.35, 0.5)); }
        // Helical Ribs: fast spatial winding with nearly circular diaphragms.
        case 3u: { return FlightLook(vec4<f32>(1.05, 0.15, 0.3, 0.15), vec4<f32>(0.0, 3.2, 0.55, 0.45),
            vec4<f32>(0.96, 0.0, 0.0, 1.3), vec4<f32>(0.8, 0.6, 0.7, 0.6)); }
        // Folded Canyon: broad low floors and walls, open above the camera.
        case 4u: { return FlightLook(vec4<f32>(1.12, 2.4, -0.8, 0.3), vec4<f32>(0.0, 0.4, 1.45, 0.0),
            vec4<f32>(0.95, 1.0, 0.0, 0.8), vec4<f32>(0.85, 0.8, 1.0, 1.4)); }
        // Cathedral Lace: deeply scalloped, densely engraved archways.
        case 5u: { return FlightLook(vec4<f32>(1.1, 0.4, 2.15, 2.45), vec4<f32>(2.0, 0.65, 0.72, 0.8),
            vec4<f32>(1.07, 0.0, 0.0, 1.65), vec4<f32>(1.1, 1.2, 0.6, 0.65)); }
        // Orbital Petals: five-fold blooms, reversed handedness, inner rails.
        case 6u: { return FlightLook(vec4<f32>(1.02, 0.4, 1.45, 3.2), vec4<f32>(0.5, -0.75, 1.0, 0.15),
            vec4<f32>(0.77, 0.0, 0.0, 1.1), vec4<f32>(1.0, 1.35, 1.5, 1.4)); }
        // Gravity Wells: narrow breathing throats and almost closed shells.
        case 7u: { return FlightLook(vec4<f32>(0.7, 0.18, 0.2, 0.1), vec4<f32>(0.0, 0.25, 0.86, 1.4),
            vec4<f32>(1.06, 0.0, 0.0, 0.65), vec4<f32>(0.60, 1.1, 0.7, 4.5)); }
        // Crystal Wings: opposing folds sharpened by an eighth harmonic.
        case 8u: { return FlightLook(vec4<f32>(1.14, 1.9, -1.8, 2.7), vec4<f32>(2.5, 0.33, 0.5, -0.1),
            vec4<f32>(1.10, 0.0, 0.0, 1.4), vec4<f32>(1.15, 0.5, 1.7, 0.7)); }
        // Double Helix: two true winding rails replace the seven-strand bore.
        default: { return FlightLook(vec4<f32>(0.94, 0.3, 0.3, 0.2), vec4<f32>(0.0, 1.45, 0.36, -0.65),
            vec4<f32>(0.90, 0.0, 1.0, 0.95), vec4<f32>(0.8, 0.85, 1.3, 0.8)); }
    }
}

fn flight_look() -> FlightLook {
    let lane = u.looks[0];
    let before = flight_recipe(u32(clamp(lane.x, 0.0, 9.0)));
    if lane.z <= 0.0 || lane.x == lane.y { return before; }
    let after = flight_recipe(u32(clamp(lane.y, 0.0, 9.0)));
    let amount = clamp(lane.z, 0.0, 1.0);
    return FlightLook(mix(before.shape, after.shape, amount), mix(before.weave, after.weave, amount),
        mix(before.space, after.space, amount), mix(before.relic, after.relic, amount));
}

fn flight_route(z: f32) -> vec2<f32> {
    return vec2<f32>(sin(z * 0.041) * 4.8 + sin(z * 0.093 + 1.2) * 1.1,
        sin(z * 0.031 + 0.6) * 3.4 + cos(z * 0.073) * 0.75);
}

fn flight_relative_route(travel: f32, z: f32) -> vec2<f32> {
    let tangent = vec2<f32>(cos(travel * 0.041) * 0.1968
        + cos(travel * 0.093 + 1.2) * 0.1023,
        cos(travel * 0.031 + 0.6) * 0.1054 - sin(travel * 0.073) * 0.05475);
    let bend = flight_route(travel + z) - flight_route(travel) - tangent * z;
    // Follow the curve without letting a far tangent push the vanishing point
    // outside a narrow canvas. This soft bound is continuous at every depth.
    return bend * 0.55 / sqrt(1.0 + dot(bend, bend) * 0.018);
}

// Closed integer angular harmonics retain a seamless bore. Its proportions
// evolve along actual world depth, not a repeating screen-space ring stack.
fn flight_radius(angle: f32, world_z: f32, look: FlightLook) -> vec2<f32> {
    let a = angle * 2.0 + world_z * 0.071;
    let b = angle * 3.0 - world_z * 0.053;
    let c = angle * 5.0 + world_z * 0.097;
    let neck = 0.24 * sin(world_z * 0.037) * min(look.relic.w, 2.5);
    var radius = vec2<f32>((3.7 + neck) * look.shape.x
        + sin(a) * look.shape.y * 0.34 + sin(b) * look.shape.z * 0.22
        + sin(c) * look.shape.w * 0.10,
        cos(a) * look.shape.y * 0.68 + cos(b) * look.shape.z * 0.66
        + cos(c) * look.shape.w * 0.50);
    radius += vec2<f32>(sin(angle * 8.0 - world_z * 0.083),
        cos(angle * 8.0 - world_z * 0.083) * 8.0) * look.weave.x * 0.055;
    return radius;
}

fn flight_material(position: f32, depth: f32) -> vec3<f32> {
    let violet = vec3<f32>(0.53, 0.075, 0.47);
    let blue = vec3<f32>(0.055, 0.30, 0.70);
    let cyan = vec3<f32>(0.075, 0.61, 0.62);
    let gold = vec3<f32>(0.83, 0.42, 0.105);
    let colour = mix(blue, violet, 0.5 + 0.5 * sin(position * 2.0 + depth * 0.019));
    let cool = mix(colour, cyan, smoothstep(0.2, 0.9, sin(position * 3.0 - depth * 0.033)) * 0.72);
    return mix(cool, gold, pow(0.5 + 0.5 * sin(position * 2.0 + depth * 0.027 + 1.7), 7.0) * 0.70);
}

fn flight_flow(uv: vec2<f32>, delta: f32) -> vec2<f32> {
    let advance = clamp(delta, 0.0, 0.1) * u.c.z * u.i.y;
    let travel = u.a.z * 12.0;
    let roll = (cos(travel * 0.017) * 0.009945
        + cos(travel * 0.009 + 0.7) * 0.002835) * (0.5 + u.d.z * 0.3);
    return rotate(uv * exp(-advance * 0.66), advance * roll * (12.0 / 9.0));
}

fn flight_memory() -> f32 { return 0.055; }

fn flight(uv0: vec2<f32>) -> World {
    let look = flight_look();
    // Time is already velocity-integrated by the app. No instantaneous beat
    // or FFT value enters this camera's travel, heading or roll.
    let travel = u.a.z * 12.0;
    let roll = (sin(travel * 0.017) * 0.065 + sin(travel * 0.009 + 0.7) * 0.035)
        * (0.5 + u.d.z * 0.3);
    let uv = rotate(uv0, roll);
    let detail = detail_level();
    let px = pixel();
    let low = bass();
    let middle = mids();
    let high = air();
    let lens = 1.44;
    let screen_radius = max(length(uv), 0.055);
    let bore = 3.7 * look.shape.x * (0.85 + look.space.x * 0.15);
    var light = vec3<f32>(0.008, 0.017, 0.043);
    var marks = vec3<f32>(0.0);

    // A continuous curved passage skin supplies immersive near/far parallax.
    // Two bounded analytic perspective refinements, not a surface raymarch.
    var wall_z = clamp(bore * lens / screen_radius, 1.4, 82.0);
    for (var refinement = 0; refinement < 2; refinement += 1) {
        let centre = flight_relative_route(travel, wall_z);
        let p = uv * wall_z / lens - centre;
        let angle = atan2(p.y, p.x);
        let radius = max(flight_radius(angle, travel + wall_z, look).x + low * 0.09, 1.6);
        wall_z = clamp(wall_z * clamp(radius / max(length(p), 0.2), 0.65, 1.40), 1.4, 82.0);
    }
    let wall_centre = flight_relative_route(travel, wall_z);
    let wall = uv * wall_z / lens - wall_centre;
    let theta = atan2(wall.y, wall.x);
    let world_z = travel + wall_z;
    let signal = spectrum_at(0.5 + 0.5 * sin(theta));
    let scroll = theta + sin(world_z * 0.067 + theta * 3.0) * (0.13 + u.d.z * 0.055)
        + sin(world_z * 0.093 - theta * 5.0) * 0.09;
    // A recipe can change bounded winding, never the coefficient of unbounded
    // absolute depth: otherwise a late-session look morph could spin violently.
    let broad_phase = scroll * 6.0 + world_z * 0.20
        + sin(world_z * 0.083 + theta * 2.0) * 0.9
        + (look.weave.y - 1.0) * sin(world_z * 0.037) * 1.4 + signal * 0.22;
    let paired_phase = scroll * 2.0 + world_z * 0.17 + sin(world_z * 0.061) * 0.4;
    let fine_phase = theta * 19.0 + world_z * 0.71
        + sin(theta * 5.0 - world_z * 0.19) * (0.45 + look.relic.y * 0.45 + middle * 0.14);
    let micro_phase = theta * 41.0 - world_z * 1.31
        + sin(theta * 7.0 + world_z * 0.11) * 1.1;
    let angular_footprint = px / max(screen_radius, 0.10);
    let depth_footprint = px * wall_z / max(screen_radius, 0.10);
    let broad_fp = (angular_footprint * 11.0 + depth_footprint * 0.35) * 1.6;
    let fine_fp = (angular_footprint * 24.0 + depth_footprint * 0.95) * 1.6;
    let micro_fp = (angular_footprint * 49.0 + depth_footprint * 1.5) * 1.6;
    let resolved = 1.0 - smoothstep(1.0, 3.0, broad_fp);
    let crest = 0.5 + 0.5 * mix(filtered_sine(broad_phase, broad_fp),
        filtered_sine(paired_phase, broad_fp * 0.6), look.space.z);
    let stratum = pow(crest, 2.0);
    let engraved = pow(0.5 + 0.5 * filtered_sine(fine_phase, fine_fp), 13.0)
        * (1.0 - smoothstep(0.9, 2.6, fine_fp));
    let micro = pow(0.5 + 0.5 * filtered_sine(micro_phase, micro_fp), 18.0)
        * (1.0 - smoothstep(0.8, 2.4, micro_fp)) * detail;
    let normal = normalize(vec3<f32>(cos(theta) * (0.35 + stratum * 0.9),
        sin(theta) * (0.35 + stratum * 0.9), 0.65));
    let relief = 0.32 + max(dot(normal, normalize(vec3<f32>(-0.4, -0.55, 0.85))), 0.0) * 0.68;
    let shell = 1.0 - smoothstep(57.0, 82.0, wall_z);
    let open_canyon = 1.0 - look.space.y * smoothstep(0.2, 0.9, sin(theta)) * 0.62;
    let material = flight_material(scroll, world_z);
    light += material * (0.16 + stratum * 0.42 * resolved) * relief * shell * open_canyon
        * (0.72 + signal * 0.50 + middle * 0.10);
    marks += mix(material, vec3<f32>(0.69, 0.88, 0.98), 0.42) * shell * open_canyon
        * (engraved * (0.13 + signal * 0.42 + high * 0.12)
        + micro * (0.08 + signal * 0.22) + stratum * resolved * 0.055);

    // Longitudinal rails and fixed world-depth inlays make travel legible:
    // these stay attached to the wall as the camera approaches and passes.
    // Frequency sectors illuminate different rails instead of expanding the
    // whole image. No camera position is changed by a beat or spectrum bin.
    let rail_count = mix(7.0, 2.0, look.space.z);
    let rail_bend = sin(world_z * 0.085) * 1.6
        + sin(theta * 3.0 + world_z * 0.071) * (0.25 + look.weave.y * 0.19);
    let rail_gradient = angular_footprint * (rail_count + 2.6)
        + depth_footprint * 0.30;
    // Blend closed fields, never a fractional angular harmonic: seven rails
    // can become the double helix without an atan2 seam opening mid-morph.
    let rail_distance = mix(abs(sin((theta * 7.0 + rail_bend) * 0.5)),
        abs(sin((theta * 2.0 + rail_bend) * 0.5)), look.space.z);
    let rail_width = max(rail_gradient * (0.75 + signal * 0.75), 0.006);
    let rails = (1.0 - smoothstep(rail_width * 0.45, rail_width * 1.45, rail_distance))
        * (1.0 - smoothstep(0.70, 2.4, rail_gradient));
    let inlay_phase = world_z * 1.7 + sin(theta * 3.0) * 0.7;
    let inlay = 0.35 + 0.65 * pow(0.5 + 0.5 * filtered_sine(inlay_phase,
        depth_footprint * 1.7 + angular_footprint * 2.1), 4.0);
    marks += mix(material, vec3<f32>(0.78, 0.92, 1.0), 0.56)
        * rails * shell * open_canyon * inlay
        * (0.36 + signal * 1.45 + high * 0.18);

    // Irregular, broken ribbon chambers are anchored to absolute plane IDs.
    // Their last/first visible intervals are already fully faded at rollover;
    // no new hash or near-plane topology is switched into a visible plane.
    let spacing = 3.8;
    let origin = floor(travel / spacing);
    for (var layer = 0; layer < 18; layer += 1) {
        let id = origin + f32(layer);
        let seed = hash(vec3<f32>(id, 7.21, 3.17));
        let seed_b = hash(vec3<f32>(id, 1.93, 8.61));
        let absolute_z = (id + 0.10 + seed * 0.60) * spacing;
        let z = absolute_z - travel;
        let fade = smoothstep(1.0, 3.6, z) * (1.0 - smoothstep(45.0, 60.0, z))
            / (1.0 + z * z * 0.009);
        if fade <= 0.0 { continue; }
        let offset = rotate(vec2<f32>(cos(id * 1.37), sin(id * 1.91)),
            (look.relic.z - 1.0) * sin(id * 0.17) * 0.6) * (0.35 + seed * 0.55);
        let centre = flight_relative_route(travel, z) + offset + vec2<f32>(0.0, look.space.y * 0.75);
        let p = uv * z / lens - centre;
        let radial = length(p);
        if radial > 7.4 * max(look.shape.x, 1.0) { continue; }
        let angle = atan2(p.y, p.x);
        let harmonic = flight_radius(angle + seed * 6.283185, absolute_z, look);
        let radius = max(harmonic.x * (0.77 + seed_b * 0.48)
            * (0.9 + look.relic.x * 0.1) + low * 0.12, 1.6);
        let metric = sqrt(1.0 + pow(harmonic.y / max(radius, 1.0), 2.0));
        let thickness = (0.34 + seed * 0.48) * (0.65 + look.weave.z * 0.45);
        let cross = (radial - radius) / thickness;
        let body = 1.0 - smoothstep(0.80, 1.25, abs(cross));
        let lobes = 2.0 + floor(seed_b * 4.0);
        let aperture = sin(angle * lobes + seed * 6.283185 + absolute_z * 0.021)
            + sin(angle * 3.0 - absolute_z * 0.037) * 0.40;
        let arc = smoothstep(-0.10, 0.44, aperture + look.weave.w * 0.15);
        let projected_distance = (radial - radius) * lens / (z * metric);
        let rim = thread(projected_distance, px * (0.70 + detail * 0.22));
        let projected_thickness = thickness * lens / (z * metric);
        let side = wire(abs(projected_distance) - projected_thickness * 0.75, px * 0.55);
        let band_position = 0.5 + 0.5 * sin(angle * 2.0 + seed_b * 6.283185);
        let measured = spectrum_at(band_position);
        let plane_material = flight_material(angle + seed * 3.0, absolute_z);
        let surface_normal = normalize(vec3<f32>(p / max(radial, 0.01) * cross * 0.75, 0.8));
        let diffuse = 0.30 + max(dot(surface_normal, normalize(vec3<f32>(-0.35, -0.45, 0.84))), 0.0) * 0.70;
        let specular = pow(max(dot(surface_normal, normalize(vec3<f32>(0.25, 0.30, 0.92))), 0.0), 24.0);
        light += plane_material * body * arc * fade * diffuse
            * (0.19 + measured * 0.40 + middle * 0.12);
        marks += mix(plane_material, vec3<f32>(0.76, 0.91, 1.0), 0.45)
            * (rim + side * detail * 0.32) * arc * fade
            * (0.23 + measured * 0.66 + high * 0.12);
        let inlay_phase = cross * (12.0 + detail * 16.0) * look.space.w
            + angle * 5.0 + absolute_z * 0.17;
        let footprint = px * z / lens * (12.0 + detail * 16.0) * look.space.w / thickness;
        let inlay = pow(0.5 + 0.5 * filtered_sine(inlay_phase, footprint), 12.0)
            * (1.0 - smoothstep(0.8, 2.5, footprint));
        marks += mix(plane_material, vec3<f32>(0.82, 0.86, 1.0), 0.56)
            * body * arc * fade * detail * (inlay * (0.08 + measured * 0.25)
            + specular * (0.10 + high * 0.10 + beat() * 0.045));
    }

    // Persistent world-space dust supplies an independent near/far speed cue.
    // Tails depend on travel speed, not on percussion; margins keep each streak
    // inside its own world cell instead of clipping at a cell boundary.
    let dust_origin = floor(travel / 6.0);
    for (var layer = 1; layer < 9; layer += 1) {
        let id = dust_origin + f32(layer);
        let z = id * 6.0 - travel;
        let fade = smoothstep(1.0, 3.0, z) * (1.0 - smoothstep(34.0, 46.0, z));
        let p = uv * z / lens - flight_relative_route(travel, z);
        let cell = floor(p / 3.2);
        let seed = vec3<f32>(cell, id);
        let offset = vec2<f32>(hash(seed), hash(seed + 17.7)) * 2.0 + 0.6;
        let q = p - cell * 3.2 - offset;
        let direction = uv / max(length(uv), 0.03);
        let tail = direction * (0.16 + min(u.c.z * u.i.y, 5.0) * 0.055);
        let along = clamp(dot(q, tail) / max(dot(tail, tail), 0.0001), 0.0, 1.0);
        let distance = length(q - tail * along) * lens / max(z, 0.2);
        let selected = smoothstep(0.83 - detail * 0.07, 0.94 - detail * 0.04, hash(seed + 4.2));
        let hue = hash(seed + 9.1);
        let colour = mix(vec3<f32>(0.22, 0.69, 0.94), vec3<f32>(0.94, 0.46, 0.57), hue);
        marks += colour * thread(distance, px * (0.66 + detail * 0.20)) * fade * selected
            * (0.40 + high * 0.70 + beat() * 0.035);
    }
    return World(light, marks);
}
