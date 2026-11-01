// Actual billboard-instance positions: no fullscreen particle loop, stored
// physics or image asset. Stable point identities morph between bounded 3-D
// sculptures. The caller draws at most 12,000 instances and owns projection.

fn constellation_seed(index: u32) -> vec3<f32> {
    let i = f32(index % 12000u);
    return vec3<f32>(hash(vec3<f32>(i, 31.7, 2.3)),
        hash(vec3<f32>(i, 8.13, 71.9)), hash(vec3<f32>(i, 53.6, 19.2)));
}

// Stable geometric envelopes, evaluated at the FULL configured audio range
// rather than today's peaks. Camera framing therefore never pumps with music.
// Rigid rotations preserve these radii. Each recipe retains its actual shape.
fn constellation_shape_radius(look: u32) -> f32 {
    let low = clamp(u.e.x, 0.0, 1.5);
    let middle = clamp(u.e.y, 0.0, 1.5);
    if look == 0u { return 1.595 + low * 0.055; }
    if look == 1u { return 1.682; }
    let minor = 0.36 + middle * 0.025;
    let tube = 0.125 + low * 0.018;
    if look == 2u { return 1.18 + minor + tube; }
    if look == 3u {
        return length(vec2<f32>(1.18 + minor, minor * 1.65)) + tube * 1.65;
    }
    if look == 4u { return 1.923; }
    if look == 5u { return length(vec3<f32>(1.40, 0.78, 0.8725)) + 0.30 + middle * 0.035; }
    if look == 6u { return 1.05 + 3.464102 * (0.305 + middle * 0.025); }
    if look == 7u { return length(vec3<f32>(1.23, 1.40, 0.65)); }
    if look == 8u { return length(vec2<f32>(1.78, 0.245)); }
    return 1.706;
}

fn constellation_fit_lens() -> f32 {
    let lane = u.looks[4];
    let before = u32(clamp(round(lane.x), 0.0, 9.0));
    let after = u32(clamp(round(lane.y), 0.0, 9.0));
    let amount = clamp(lane.z, 0.0, 1.0);
    var radius = constellation_shape_radius(after);
    if before != after && amount < 1.0 {
        // Every local shape morph lies within this interval. Its curved paths
        // get headroom too, with no sudden framing change at either endpoint.
        let low_t = clamp((amount - 0.16) / 0.84, 0.0, 1.0);
        let high_t = clamp(amount / 0.84, 0.0, 1.0);
        let low_blend = low_t * low_t * low_t * (low_t * (low_t * 6.0 - 15.0) + 10.0);
        let high_blend = high_t * high_t * high_t * (high_t * (high_t * 6.0 - 15.0) + 10.0);
        let source_radius = constellation_shape_radius(before);
        radius = max(mix(source_radius, radius, low_blend), mix(source_radius, radius, high_blend));
        let peak = clamp(0.5, low_blend, high_blend);
        radius += 0.88 * 16.0 * peak * peak * (1.0 - peak) * (1.0 - peak);
    }
    let low = clamp(u.e.x, 0.0, 1.5);
    let middle = clamp(u.e.y, 0.0, 1.5);
    let high = clamp(u.e.z, 0.0, 1.5);
    let spectral = max(low, max(middle, high));
    let low_drive = low / (0.45 + low);
    let middle_drive = middle / (0.60 + middle);
    let high_drive = high / (0.55 + high);
    let spectral_drive = spectral / (0.35 + spectral);
    radius *= 1.0 + low_drive * 0.30;
    radius += spectral_drive * 0.68;
    radius += 3.464102 * middle_drive * (0.075 + spectral_drive * 0.095);
    radius += 3.464102 * high_drive * spectral_drive * 0.025;
    return fitted_raster_lens(radius, 3.15);
}

fn constellation_sphere(seed: vec3<f32>) -> vec3<f32> {
    let height = seed.x * 2.0 - 1.0;
    let angle = seed.y * 6.283185307;
    let radial = sqrt(max(1.0 - height * height, 0.0));
    return vec3<f32>(cos(angle) * radial, height, sin(angle) * radial);
}

// Curl of A=(sin(y)cos(z),sin(z)cos(x),sin(x)cos(y)). This is a
// continuous divergence-free deformation field, not claimed fluid simulation.
fn constellation_curl(p: vec3<f32>) -> vec3<f32> {
    let s = sin(p);
    let c = cos(p);
    return vec3<f32>(-s.x * s.y - c.z * c.x,
        -s.y * s.z - c.x * c.y, -s.z * s.x - c.y * c.z);
}

fn constellation_knot(seed: vec3<f32>, winding: f32, time: f32) -> vec3<f32> {
    let t = seed.x * 6.283185307;
    let p = vec2<f32>(cos(t * 2.0), sin(t * 2.0));
    let q = vec2<f32>(cos(t * winding), sin(t * winding));
    let major = 1.18;
    let minor = 0.36 + mids() * 0.025;
    let ring = major + minor * q.x;
    let centre = vec3<f32>(ring * p.x, ring * p.y, minor * q.y);
    let tangent = normalize(vec3<f32>(-minor * winding * q.y * p.x - ring * 2.0 * p.y,
        -minor * winding * q.y * p.y + ring * 2.0 * p.x, minor * winding * q.x));
    let outward = vec3<f32>(p, 0.0);
    let normal = normalize(outward - tangent * dot(outward, tangent));
    let binormal = cross(tangent, normal);
    let tube_angle = seed.y * 6.283185307 + time * 0.11;
    let tube = (0.045 + seed.z * 0.080 + bass() * 0.018);
    return centre + (normal * cos(tube_angle) + binormal * sin(tube_angle)) * tube;
}

fn constellation_shape(index: u32, seed: vec3<f32>, look: u32, time: f32) -> vec3<f32> {
    let sphere = constellation_sphere(seed);
    if look == 0u {
        // Luminous Sphere: a fine, slightly embossed spherical shell.
        let emboss = sin(seed.y * 31.41592654 + sphere.y * 6.0 + time * 0.17);
        return sphere * (1.55 + emboss * 0.045 + bass() * 0.055);
    }
    if look == 1u {
        // Breathing Shells: three nested surfaces, not a solid ball of fog.
        let shell = floor(seed.z * 3.0);
        let radius = 0.70 + shell * 0.46 + sin(time * 0.23 + shell * 1.9) * 0.045;
        let p = sphere * radius;
        let turn = rotate(p.xz, shell * 0.42 + sin(time * 0.13) * 0.18);
        return vec3<f32>(turn.x, p.y * (0.92 + shell * 0.045), turn.y);
    }
    if look == 2u {
        // Trefoil Orbit: a real (2,3) torus knot with a fine tube cross-section.
        return constellation_knot(seed, 3.0, time);
    }
    if look == 3u {
        // Cinquefoil Ribbon: five interwoven crossings and a wider axial fold.
        let knot = constellation_knot(seed, 5.0, -time);
        return vec3<f32>(knot.x, knot.y * 0.82, knot.z * 1.65);
    }
    if look == 4u {
        // Lissajous Lattice: seven closed (3,4,5) spatial strands.
        let t = seed.x * 6.283185307;
        let lane = floor(seed.z * 7.0) * 0.897597901;
        let thickness = (seed.y - 0.5) * 0.035;
        return vec3<f32>(sin(t * 3.0 + lane), sin(t * 4.0 + lane * 0.8),
            sin(t * 5.0 + lane * 1.7)) * 1.10 + sphere * thickness;
    }
    if look == 5u {
        // Harmonic Ribbon: a wide, closed 3-D ribbon with three lateral twists.
        let t = seed.x * 6.283185307;
        let transverse = (seed.y * 2.0 - 1.0) * (0.30 + mids() * 0.035);
        return vec3<f32>(sin(t) * 1.40 + transverse * cos(t * 3.0),
            sin(t * 2.0) * 0.78 + transverse * sin(t * 3.0),
            cos(t * 3.0) * 0.86 + (seed.z - 0.5) * 0.025);
    }
    if look == 6u {
        // Curl Swarm: a coherently deforming volume; no frame RNG/CPU physics.
        let base = sphere * pow(0.08 + seed.z * 0.92, 0.333333333) * 1.05;
        let flow = constellation_curl(base * 1.45 + vec3<f32>(time * 0.11, -time * 0.09, time * 0.13));
        let fine = constellation_curl(base * 2.25 + flow * 0.40
            + vec3<f32>(-time * 0.07, time * 0.10, time * 0.06));
        return base + flow * (0.23 + mids() * 0.025) + fine * 0.075;
    }
    if look == 7u {
        // Twin Vortices: two separated helices, each carrying a deep point cloud.
        let side = select(-1.0, 1.0, index % 2u == 0u);
        let height = (seed.x * 2.0 - 1.0) * 1.40;
        let angle = seed.y * 6.283185307 + height * 2.1 + time * 0.13 * side;
        let radius = 0.18 + sqrt(seed.z) * 0.31;
        return vec3<f32>(side * 0.74 + cos(angle) * radius,
            height, sin(angle) * radius + sin(height * 2.0 + time * 0.20) * 0.16);
    }
    if look == 8u {
        // Spiral Observatory: five discrete spiral arms and a small core halo.
        if index % 7u == 0u { return sphere * (0.20 + seed.z * 0.48); }
        let radius = sqrt(seed.x) * 1.78;
        let arm = f32(index % 5u) * 1.256637061;
        let angle = arm + radius * 2.4 + time * 0.08 + (seed.y - 0.5) * 0.24;
        let height = (seed.z * 2.0 - 1.0) * (0.065 + (1.0 - radius / 1.78) * 0.18);
        return vec3<f32>(cos(angle) * radius, height, sin(angle) * radius);
    }
    // Crystal Lanterns: four nested superellipsoid shells, genuinely box-like.
    let shell = floor(seed.z * 4.0);
    let exponent = 0.42;
    let crystal = sign(sphere) * pow(abs(sphere), vec3<f32>(exponent));
    let ripple = 1.0 + sin(sphere.y * 8.0 + time * 0.14) * 0.025;
    return crystal * (0.64 + shell * 0.19) * ripple;
}

fn constellation_morph_amount(seed: vec3<f32>, amount: f32) -> f32 {
    if amount <= 0.0 { return 0.0; }
    if amount >= 1.0 { return 1.0; }
    // A smooth wave across the sculpture's stable surface coordinates lets
    // neighbouring regions depart and arrive together, not all through the
    // same midpoint. Every local window lies inside the global [0,1] passage.
    let delay = 0.08 + 0.08 * sin(seed.x * 6.283185307 + seed.y * 3.141592654);
    let t = clamp((amount - delay) / 0.84, 0.0, 1.0);
    return clamp(t * t * t * (t * (t * 6.0 - 15.0) + 10.0), 0.0, 1.0);
}

fn constellation_morph(before: vec3<f32>, after: vec3<f32>, seed: vec3<f32>, amount: f32) -> vec3<f32> {
    let straight = mix(before, after, amount);
    // These analytic orthogonal axes have no zero-vector normalization. Their
    // spatially varying orientation gives the passage real depth: each stable
    // point takes a curled orbital arc instead of collapsing along a chord.
    let radial = constellation_sphere(seed);
    let azimuth = seed.y * 6.283185307;
    let sideways = vec3<f32>(-sin(azimuth), 0.0, cos(azimuth));
    let normal = cross(radial, sideways);
    let angle = seed.z * 6.283185307 + amount * 3.141592654;
    let arc = radial * cos(angle) + sideways * sin(angle)
        + normal * sin(angle * 2.0 + seed.x * 6.283185307) * 0.22;
    let radius = 0.30 + min(length(after - before), 3.0) * 0.10;
    let curl = constellation_curl(straight * 0.85 + radial * 0.35
        + vec3<f32>(amount * 0.8, -amount * 0.6, amount * 0.5));
    // The bounded bulge and its derivative are exactly zero at both ends.
    // No frame RNG, accumulated particle physics, or extra shape evaluations.
    let envelope = 16.0 * amount * amount * (1.0 - amount) * (1.0 - amount);
    return straight + (arc * radius + curl * 0.075) * envelope;
}

fn constellation_point(index: u32) -> vec4<f32> {
    let seed = constellation_seed(index);
    let lane = u.looks[4];
    let before_id = u32(clamp(round(lane.x), 0.0, 9.0));
    let after_id = u32(clamp(round(lane.y), 0.0, 9.0));
    let amount = clamp(lane.z, 0.0, 1.0);
    let local_amount = constellation_morph_amount(seed, amount);
    let time = u.a.z + lane.w * 0.035;
    var position: vec3<f32>;
    if local_amount >= 1.0 || before_id == after_id {
        position = constellation_shape(index, seed, after_id, time);
    } else {
        position = constellation_shape(index, seed, before_id, time);
        if local_amount > 0.0 {
            position = constellation_morph(position,
                constellation_shape(index, seed, after_id, time), seed, local_amount);
        }
    }
    // Each endpoint keeps its surface/path frequency coordinate. Blend the
    // measured values with the same local-quintic shape amount: switching
    // the destination ID must not jump a dot to an unrelated band at amount 0,
    // and interpolating the coordinate would sweep through unrelated bands.
    let before_band = select(seed.y, seed.x, before_id >= 2u);
    let after_band = select(seed.y, seed.x, after_id >= 2u);
    var contour = spectrum_at(before_band);
    if local_amount >= 1.0 || before_id == after_id {
        contour = spectrum_at(after_band);
    } else if local_amount > 0.0 {
        contour = mix(contour, spectrum_at(after_band), local_amount);
    }
    // A soft, bounded response makes ordinary measured levels legible as
    // geometry, not merely a tiny brightness tint. Every response is exactly
    // zero with a disconnected mapping or silent signal. No beat enters the
    // camera and no procedural oscillator invents an audio envelope.
    let low = clamp(bass(), 0.0, 2.0);
    let middle = clamp(mids(), 0.0, 2.0);
    let high = clamp(air(), 0.0, 2.0);
    let measured = clamp(contour, 0.0, 2.0);
    let low_drive = low / (0.45 + low);
    let middle_drive = middle / (0.60 + middle);
    let high_drive = high / (0.55 + high);
    let spectral_drive = measured / (0.35 + measured);

    // Bass changes the whole topology's scale; neighbouring frequency bands
    // sculpt different regions. Work before rigid rotation, so the deformed
    // material turns with its sculpture instead of drifting across it.
    position *= 1.0 + low_drive * 0.30;
    // Morphing points can pass through the origin. A soft radius avoids a
    // near-zero unit-vector flip amplifying that small motion into a twitch.
    let direction = position / sqrt(dot(position, position) + 0.25 * 0.25);
    position += direction * spectral_drive * 0.68;
    let surface = constellation_sphere(seed);
    let curl = constellation_curl(position * 1.15 + surface * 0.27);
    position += curl * middle_drive * (0.075 + spectral_drive * 0.095);
    // Treble adds a fine local ripple rather than shaking the entire cloud.
    let filigree = constellation_curl(position * 3.1 + surface * 1.3);
    position += filigree * high_drive * spectral_drive * 0.025;

    // The shared camera still follows its continuous, user-paced orbit.
    let horizontal = rotate(position.xz, time * 0.075 + sin(time * 0.041) * 0.10);
    position = vec3<f32>(horizontal.x, position.y, horizontal.y);
    let vertical = rotate(position.yz, 0.35 + sin(time * 0.047) * 0.18);
    position = vec3<f32>(position.x, vertical.x, vertical.y);
    let signal = clamp(spectral_drive * 0.92 + low_drive * 0.12
        + middle_drive * 0.08 + high_drive * 0.10, 0.0, 1.0);
    return vec4<f32>(position, signal);
}

fn constellation_colour(index: u32, point: vec4<f32>) -> vec3<f32> {
    let grain = hash(vec3<f32>(f32(index % 12000u), 4.7, 89.2));
    let cyan = vec3<f32>(0.12, 0.56, 0.78);
    let ivory = vec3<f32>(0.88, 0.91, 0.83);
    let coral = vec3<f32>(0.94, 0.41, 0.28);
    let pearl = mix(cyan, ivory, smoothstep(0.52, 0.95, grain) * 0.80);
    let material = mix(pearl, coral, smoothstep(0.91, 0.995, grain) * 0.72);
    let high = clamp(air(), 0.0, 2.0);
    let fine_light = high / (0.55 + high) * smoothstep(0.58, 0.97, grain);
    return material * (0.30 + clamp(point.w, 0.0, 1.0) * 1.65 + fine_light * 0.38);
}
