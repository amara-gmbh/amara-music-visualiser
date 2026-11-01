// Neon Coast: an original procedural coastal night drive. No sampled artwork.
// 24 analytic building boxes, 10 bounded palm crowns, one ground intersection.
// The integrated travel clock never changes phase with a look or music envelope.
struct CoastLook {
    city: vec4<f32>, // building height, left density, coast width, palm height
    mood: vec4<f32>, // night, water, neon intensity, warm sky
    colour: vec4<f32>, // neon hue, sky hue, road width, architectural steps
}

struct CoastSurface {
    colour: vec3<f32>,
    depth: f32,
    coverage: f32,
}

struct CoastSky {
    colour: vec3<f32>,
    ambient: vec3<f32>,
}

struct CoastCache {
    base: vec4<f32>,
    bends: vec4<f32>,
    broad: vec4<f32>,
    district: vec4<f32>,
    width: vec4<f32>,
}

fn coast_recipe(index: i32) -> CoastLook {
    switch clamp(index, 0, 9) {
        case 0: { return CoastLook(vec4<f32>(0.75, 0.20, 6.0, 1.05), vec4<f32>(0.10, 0.80, 0.62, 1.0), vec4<f32>(0.0, 0.0, 3.1, 0.50)); }
        case 1: { return CoastLook(vec4<f32>(0.85, 0.75, 9.0, 1.0), vec4<f32>(0.76, 0.35, 1.20, 0.3), vec4<f32>(0.03, 0.04, 3.8, 1.0)); }
        case 2: { return CoastLook(vec4<f32>(0.48, 0.08, 4.9, 1.20), vec4<f32>(0.36, 1.0, 0.85, 0.7), vec4<f32>(0.20, 0.02, 2.8, 0.25)); }
        case 3: { return CoastLook(vec4<f32>(1.15, 0.28, 6.4, 0.80), vec4<f32>(1.0, 1.0, 1.15, 0.1), vec4<f32>(0.16, 0.10, 2.7, 0.30)); }
        case 4: { return CoastLook(vec4<f32>(0.67, 1.0, 12.0, 0.85), vec4<f32>(0.60, 0.15, 1.25, 0.5), vec4<f32>(-0.08, 0.0, 3.6, 1.0)); }
        case 5: { return CoastLook(vec4<f32>(1.65, 0.62, 7.5, 0.78), vec4<f32>(0.82, 0.65, 1.10, 0.2), vec4<f32>(0.08, 0.13, 3.0, 0.50)); }
        case 6: { return CoastLook(vec4<f32>(0.42, 0.0, 4.4, 1.30), vec4<f32>(0.24, 1.0, 0.65, 0.4), vec4<f32>(0.24, 0.06, 2.5, 0.10)); }
        case 7: { return CoastLook(vec4<f32>(1.12, 0.85, 10.0, 0.85), vec4<f32>(0.94, 0.60, 1.35, 0.15), vec4<f32>(0.18, 0.09, 3.7, 0.75)); }
        case 8: { return CoastLook(vec4<f32>(0.35, 0.04, 4.8, 1.12), vec4<f32>(0.0, 1.0, 0.50, 1.0), vec4<f32>(-0.10, -0.03, 2.6, 0.10)); }
        default: { return CoastLook(vec4<f32>(1.10, 0.95, 8.8, 1.05), vec4<f32>(0.90, 0.50, 1.35, 0.40), vec4<f32>(-0.02, 0.08, 3.3, 0.85)); }
    }
}

fn coast_look() -> CoastLook {
    let a = coast_recipe(i32(u.looks[14].x));
    let b = coast_recipe(i32(u.looks[14].y));
    let blend = clamp(u.looks[14].z, 0.0, 1.0);
    return CoastLook(mix(a.city, b.city, blend), mix(a.mood, b.mood, blend), mix(a.colour, b.colour, blend));
}

fn coast_response(value: f32) -> f32 {
    let x = max(value, 0.0);
    return x / (0.70 + x);
}

fn coast_audio() -> vec3<f32> {
    let low = (spectrum_at(0.02) + spectrum_at(0.06) + spectrum_at(0.11)) / 3.0;
    let middle = (spectrum_at(0.24) + spectrum_at(0.33) + spectrum_at(0.43)) / 3.0;
    let high = (spectrum_at(0.64) + spectrum_at(0.78) + spectrum_at(0.93)) / 3.0;
    let fresh = select(0.0, 1.0, u.raster.z > 0.0);
    return vec3<f32>(coast_response(bass() * 0.58 + low * 0.42),
        coast_response(mids() * 0.58 + middle * 0.42),
        coast_response(air() * 0.58 + high * 0.42)) * fresh;
}

fn coast_colour(hue: f32) -> vec3<f32> {
    // Saturated violet, coral and turquoise light, with a continuous hue orbit.
    return rotate_hue(vec3<f32>(1.0, 0.075, 0.36), hue);
}

fn coast_landscape(position: f32, salt: f32) -> f32 {
    let cell = floor(position);
    let t = fract(position);
    // Quintic interpolation gives a continuous slope and curvature at every
    // persistent district boundary. There is no short repeating neighbourhood.
    let blend = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    return mix(hash(vec3<f32>(cell, salt, 73.4)), hash(vec3<f32>(cell + 1.0, salt, 73.4)), blend);
}

fn coast_route(z: f32) -> f32 {
    return (coast_landscape(z / 104.0, 9.0) - 0.5) * 45.0
        + (coast_landscape(z / 231.0, 23.0) - 0.5) * 22.0;
}

fn coast_heading(travel: f32) -> f32 {
    return (coast_route(travel + 1.0) - coast_route(travel - 1.0)) * 0.5;
}

fn coast_road_width(z: f32, base: f32) -> f32 {
    return base * (0.92 + coast_landscape(z / 127.0, 42.0) * 0.18);
}

fn coast_nodes(base: f32, salt: f32) -> vec4<f32> {
    return vec4<f32>(hash(vec3<f32>(base, salt, 73.4)),
        hash(vec3<f32>(base + 1.0, salt, 73.4)),
        hash(vec3<f32>(base + 2.0, salt, 73.4)),
        hash(vec3<f32>(base + 3.0, salt, 73.4)));
}

fn coast_cache(travel: f32) -> CoastCache {
    let base = floor(vec4<f32>(travel) / vec4<f32>(104.0, 231.0, 136.0, 127.0));
    return CoastCache(base, coast_nodes(base.x, 9.0), coast_nodes(base.y, 23.0),
        coast_nodes(base.z, 91.0), coast_nodes(base.w, 42.0));
}

fn coast_cached_landscape(position: f32, base: f32, nodes: vec4<f32>, salt: f32) -> f32 {
    let index = floor(position) - base;
    // All visible buildings/palms fit in these three cells. Far ground and the
    // one-metre backward heading sample retain the original exact formula.
    if index < 0.0 || index >= 3.0 { return coast_landscape(position, salt); }
    let t = fract(position);
    let blend = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let i = u32(index);
    return mix(nodes[i], nodes[i + 1u], blend);
}

fn coast_cached_route(z: f32, cache: CoastCache) -> f32 {
    return (coast_cached_landscape(z / 104.0, cache.base.x, cache.bends, 9.0) - 0.5) * 45.0
        + (coast_cached_landscape(z / 231.0, cache.base.y, cache.broad, 23.0) - 0.5) * 22.0;
}

fn coast_cached_heading(travel: f32, cache: CoastCache) -> f32 {
    return (coast_cached_route(travel + 1.0, cache) - coast_cached_route(travel - 1.0, cache)) * 0.5;
}

fn coast_cached_width(z: f32, base: f32, cache: CoastCache) -> f32 {
    return base * (0.92 + coast_cached_landscape(z / 127.0, cache.base.w, cache.width, 42.0) * 0.18);
}

fn coast_box(origin: vec3<f32>, direction: vec3<f32>, centre: vec3<f32>, size: vec3<f32>) -> vec2<f32> {
    // Guard axis-parallel rays explicitly; no zero division or raymarch.
    let safe = select(vec3<f32>(-0.00001), vec3<f32>(0.00001), direction >= vec3<f32>(0.0));
    let ray = select(direction, safe, abs(direction) < vec3<f32>(0.00001));
    let a = (centre - size - origin) / ray;
    let b = (centre + size - origin) / ray;
    let near = min(a, b);
    let far = max(a, b);
    return vec2<f32>(max(max(near.x, near.y), near.z), min(min(far.x, far.y), far.z));
}

fn coast_buildings_cached(base: vec3<f32>, ambient: vec3<f32>, direction: vec3<f32>, eye: vec3<f32>, travel: f32,
    appearance: CoastLook, audio: vec3<f32>, hue: f32, cache: CoastCache) -> CoastSurface {
    var colour = base;
    var closest = 10000.0;
    var coverage = 0.0;
    let first = floor(travel / 11.0);
    let camera_x = coast_cached_route(travel, cache);
    // Retain the existing heuristic vocal-entry emphasis inside the mid-owned
    // facade only, after opting out of the generic whole-world material accent.
    let vocal = clamp(u.voice.x * 0.28 + u.voice.z * 0.85, 0.0, 1.0)
        * select(0.0, 1.0, u.raster.z > 0.0 && u.e.y > 0.0);
    // Far to near painter order; each row has its own persistent world identity.
    for (var row = 11; row >= 0; row -= 1) {
        let identity = first + f32(row) + 1.0;
        let district = coast_cached_landscape(identity * 11.0 / 136.0, cache.base.z, cache.district, 91.0);
        for (var side = 0; side < 2; side += 1) {
            let sign_side = select(-1.0, 1.0, side == 1);
            let seed = hash(vec3<f32>(identity, f32(side) * 7.0, 83.4));
            let z = identity * 11.0 + (hash(vec3<f32>(identity, f32(side), 98.0)) - 0.5) * 2.8;
            let depth = z - travel;
            // Spacing jitter is smaller than the zero-opacity margins at both
            // cell boundaries, so adding/removing a row is still invisible.
            let visible = smoothstep(3.0, 8.0, depth) * (1.0 - smoothstep(105.0, 129.0, depth));
            let look_a = coast_recipe(i32(u.looks[14].x));
            let look_b = coast_recipe(i32(u.looks[14].y));
            let left_a = select(0.0, 1.0, seed < look_a.city.y);
            let left_b = select(0.0, 1.0, seed < look_b.city.y);
            let left = mix(left_a, left_b, clamp(u.looks[14].z, 0.0, 1.0));
            let gap = select(1.0, 0.0, hash(vec3<f32>(identity, f32(side), 512.0)) > 0.80 + district * 0.18);
            let density = select(left, 1.0, side == 1) * gap;
            // Stable low shops, broad hotels and slender towers change the
            // actual skyline; music never resizes these opaque solid volumes.
            let character = hash(vec3<f32>(identity, f32(side), 24.0));
            let tower = smoothstep(0.52, 0.90, character * 0.62 + district * 0.62);
            let height = (2.5 + character * 3.2 + tower * (9.0 + seed * 10.0)) * appearance.city.x;
            let half_width = mix(3.2 + seed * 0.65, 1.45 + seed * 0.65, tower);
            let x = coast_cached_route(z, cache) - camera_x + sign_side * (8.0 + half_width + seed * 3.5 + district * 1.5);
            let centre = vec3<f32>(x, height * 0.5, depth);
            let size = vec3<f32>(half_width, height * 0.5, 3.0);
            let hit = coast_box(eye, direction, centre, size);
            if hit.x > 0.0 && hit.y >= hit.x && density * visible > 0.001 {
                let point = eye + direction * hit.x;
                let relative = point - centre;
                let side_face = abs(relative.x) > half_width - 0.015;
                let roof = abs(relative.y) > height * 0.5 - 0.015;
                let face_x = select(relative.x + half_width, relative.z + 3.0, side_face);
                let front = select(1.0, 0.53, side_face);
                let fog = exp(-depth * 0.012);
                let pigment = coast_colour(hue + seed * 0.22);
                let pale = mix(vec3<f32>(0.055, 0.042, 0.095), vec3<f32>(0.20, 0.095, 0.145), 1.0 - appearance.mood.x);
                let elevation = clamp(point.y / max(height, 0.1), 0.0, 1.0);
                var face = pale * front * (0.58 + elevation * 0.42);
                face += vec3<f32>(0.12, 0.055, 0.080) * pow(elevation, 5.0) * (1.0 - appearance.mood.x);
                let window_coord = vec2<f32>(face_x * mix(0.90, 1.6, tower), point.y * mix(0.80, 1.35, tower));
                let cell = floor(window_coord);
                let window = abs(fract(window_coord) - 0.5);
                let footprint = max(pixel() * hit.x * 1.3, 0.012);
                let opening = (1.0 - smoothstep(0.20 - footprint, 0.20 + footprint, window.x))
                    * (1.0 - smoothstep(0.26 - footprint, 0.26 + footprint, window.y));
                let window_seed = hash(vec3<f32>(cell, identity + f32(side) * 39.0));
                let occupied = smoothstep(0.38, 0.55, window_seed);
                let band_position = (8.0 + fract(cell.y * 0.103 + seed * 0.71) * 15.0) / 47.0;
                let local_mid = coast_response(spectrum_at(band_position)) * select(0.0, 1.0, u.raster.z > 0.0);
                let room_colour = mix(vec3<f32>(1.0, 0.54, 0.21), vec3<f32>(0.23, 0.70, 0.92),
                    smoothstep(0.62, 0.86, window_seed));
                let lit = mix(room_colour, pigment, 0.23 + appearance.mood.x * 0.27);
                face += lit * opening * occupied * (0.08 + audio.y * 0.55 + local_mid * 0.95 + vocal * 0.20) * front * select(1.0, 0.0, roof);
                // Deco cornices and continuous vertical strips describe actual faces.
                let storey = abs(fract(point.y / 1.85) - 0.5);
                let cornice = 1.0 - smoothstep(0.018, 0.018 + footprint * 0.45, storey);
                let edge = min(face_x, select(half_width * 2.0, 6.0, side_face) - face_x);
                let rim = 1.0 - smoothstep(0.035, 0.035 + footprint, edge);
                let crown = 1.0 - smoothstep(0.06, 0.06 + footprint, height - point.y);
                face += pigment * (cornice * appearance.colour.w * 0.24 + rim * 0.22 + crown * 0.45)
                    * appearance.mood.z * (0.52 + audio.y * 1.60);
                face += vec3<f32>(0.84, 0.42, 0.23) * (cornice * 0.07 + crown * 0.12) * vocal;
                // A broad, slowly shaped musical facade wash is readable at distance.
                let wash = 0.5 + 0.5 * sin(point.y * 0.65 - u.a.z * 0.70 + seed * 7.0
                    + sin(face_x * 0.7) * audio.y * 1.8);
                face += pigment * wash * audio.y * 0.18;
                let shop_band = (1.0 - smoothstep(0.12, 0.12 + footprint, abs(point.y - min(height * 0.60, 2.0)))) * (1.0 - tower);
                face += coast_colour(hue + seed * 0.35 + 0.16) * shop_band
                    * (0.16 + audio.y * 0.40 + local_mid * 0.42);
                face = mix(face, pale * 0.40, select(0.0, 0.88, roof));
                // Fog is atmospheric radiance only. Reusing `base` here leaks
                // the sun/moon AND bass-lit roadway straight through houses.
                colour = mix(colour, mix(ambient, face, fog), visible * density);
                if hit.x < closest {
                    closest = hit.x;
                    coverage = visible * density;
                }
            }
        }
    }
    return CoastSurface(colour, closest, coverage);
}

fn coast_buildings(base: vec3<f32>, ambient: vec3<f32>, direction: vec3<f32>, eye: vec3<f32>, travel: f32,
    appearance: CoastLook, audio: vec3<f32>, hue: f32) -> CoastSurface {
    return coast_buildings_cached(base, ambient, direction, eye, travel, appearance, audio, hue, coast_cache(travel));
}

fn coast_project(point: vec3<f32>) -> vec2<f32> {
    return vec2<f32>(point.x, 2.7 - point.y) * 1.20 / max(point.z, 0.25) + vec2<f32>(0.0, -0.11);
}

fn coast_palms_cached(surface: CoastSurface, uv: vec2<f32>, travel: f32, appearance: CoastLook, audio: vec3<f32>, hue: f32,
    cache: CoastCache) -> vec3<f32> {
    var colour = surface.colour;
    let first = floor(travel / 18.0);
    let camera_x = coast_cached_route(travel, cache);
    let heading = coast_cached_heading(travel, cache);
    // Frond motion uses the slower, measured treble envelope as well as the
    // render-smoothed FFT. Fast high detail brightens edges without jolting leaves.
    let fresh = select(0.0, 1.0, u.raster.z > 0.0 && u.e.z > 0.0);
    let flutter = audio.z * 0.45 + coast_response(u.music.w) * fresh * 0.55;
    for (var row = 4; row >= 0; row -= 1) {
        let identity = first + f32(row) + 1.0;
        for (var side = 0; side < 2; side += 1) {
            let seed = hash(vec3<f32>(identity, f32(side), 414.0));
            let z = identity * 18.0 + (seed - 0.5) * 3.0;
            let depth = z - travel;
            let group = select(1.0, 0.0, hash(vec3<f32>(identity, f32(side), 919.0)) > 0.87);
            let visible = smoothstep(2.0, 6.0, depth) * (1.0 - smoothstep(70.0, 87.0, depth)) * group;
            let sign_side = select(-1.0, 1.0, side == 1);
            let height = (5.0 + seed * 2.3) * appearance.city.w;
            let x = coast_cached_route(z, cache) - camera_x + sign_side * (coast_cached_width(z, appearance.colour.z, cache) + 1.4 + seed * 0.65);
            let scale = 1.20 / max(depth, 0.5);
            // Same off-axis heading as the hit ray; depth remains forward Z.
            let foot = coast_project(vec3<f32>(x - heading * depth, 0.0, depth));
            let p = (uv - foot) / scale;
            if abs(p.x) < 5.4 && p.y < 0.3 && p.y > -height - 3.0 {
                let breeze = sin(u.a.z * 0.38 + seed * 7.0);
                let sway = breeze * (0.14 + flutter * 0.60);
                let crown = vec2<f32>(sign_side * 0.42 + sway, -height);
                var trunk = 10.0;
                var previous = vec2<f32>(0.0);
                for (var piece = 1; piece <= 4; piece += 1) {
                    let t = f32(piece) * 0.25;
                    let next = vec2<f32>(crown.x * t * t, -height * t);
                    trunk = min(trunk, segment_distance(p, previous, next) - mix(0.12, 0.055, t));
                    previous = next;
                }
                var leaf = 10.0;
                var leaf_tip = 0.0;
                // Every frond uses the same two evolving phases plus a fixed
                // 0.71-radian offset. Share sin/cos and rotate that fixed offset
                // instead of repeating the evolving trigonometry 45 times.
                let opening_phase = u.a.z * 0.85 + seed * 7.0;
                let bending_phase = u.a.z * 1.10 + seed * 7.0;
                let opening_basis = vec2<f32>(sin(opening_phase), cos(opening_phase));
                let bending_basis = vec2<f32>(sin(bending_phase), cos(bending_phase));
                var frond_rotation = vec2<f32>(0.0, 1.0);
                let frond_step = vec2<f32>(0.6518337727, 0.7583618760);
                // sin/cos(t * 1.9), t = 1/4, 2/4, 3/4, 4/4. These retain
                // the existing four curve subdivisions and their exact angles.
                let section_sine = vec4<f32>(0.4573384523, 0.8134155273, 0.9893904924, 0.9463000894);
                let section_cosine = vec4<f32>(0.8892927170, 0.5816830993, 0.1452804059, -0.3232895434);
                for (var frond = 0; frond < 9; frond += 1) {
                    let opening = dot(opening_basis, frond_rotation.yx) * flutter * 0.24;
                    let bending = vec2<f32>(dot(bending_basis, frond_rotation.yx),
                        bending_basis.y * frond_rotation.y - bending_basis.x * frond_rotation.x);
                    let angle = -2.70 + f32(frond) * 0.675 + seed * 0.2 + opening;
                    let reach = vec2<f32>(cos(angle) * (2.7 + seed * 0.45), sin(angle) * 1.9 - 0.35);
                    var start = crown;
                    for (var section = 1; section <= 4; section += 1) {
                        let t = f32(section) * 0.25;
                        let bend = (bending.x * section_cosine[u32(section - 1)]
                            + bending.y * section_sine[u32(section - 1)]) * flutter;
                        let next = crown + reach * t + vec2<f32>(sway * t + bend * t * t * 0.65,
                            (1.45 - flutter * 0.65) * t * t + bend * t * (1.0 - t) * 1.6);
                        let spine = next - start;
                        let along = clamp(dot(p - start, spine) / max(dot(spine, spine), 0.001), 0.0, 1.0);
                        let along_frond = (f32(section - 1) + along) * 0.25;
                        let distance_to_spine = length(p - mix(start, next, along));
                        // Dense, tapered leaflets are cut into a continuous ribbon.
                        // Width is evaluated at the closest point, avoiding rounded
                        // capsule ends at every subdivision of the curved frond.
                        let taper = pow(max(sin(along_frond * 3.14159), 0.0), 0.72);
                        let serration = 0.70 + 0.30 * smoothstep(-0.65, 0.65,
                            sin(along_frond * 94.0 + distance_to_spine * 24.0));
                        let width = 0.22 * taper * serration * (0.91 + flutter * 0.15);
                        let frond_distance = distance_to_spine - width;
                        if frond_distance < leaf {
                            leaf = frond_distance;
                            leaf_tip = smoothstep(0.52, 0.98, along_frond);
                        }
                        start = next;
                    }
                    frond_rotation = vec2<f32>(dot(frond_rotation, frond_step.yx),
                        frond_rotation.y * frond_step.y - frond_rotation.x * frond_step.x);
                }
                let distance = min(trunk, leaf);
                let aa = max(pixel() / scale, 0.008);
                let silhouette = 1.0 - smoothstep(-aa, aa, distance);
                let rim = exp(-abs(distance) / max(aa * 1.7, 0.025));
                let palm_colour = vec3<f32>(0.010, 0.025, 0.033)
                    + coast_colour(hue + 0.24) * rim * (0.020 + audio.z * 0.18)
                    + coast_colour(hue + 0.12) * leaf_tip * audio.z * 0.13 * select(0.0, 1.0, leaf < trunk);
                let occlusion = select(1.0, 1.0 - surface.coverage, depth > surface.depth);
                colour = mix(colour, palm_colour, silhouette * visible * occlusion);
            }
        }
    }
    return colour;
}

fn coast_palms(surface: CoastSurface, uv: vec2<f32>, travel: f32, appearance: CoastLook, audio: vec3<f32>, hue: f32) -> vec3<f32> {
    return coast_palms_cached(surface, uv, travel, appearance, audio, hue, coast_cache(travel));
}

fn coast_orb_position(time: f32) -> vec2<f32> {
    return vec2<f32>(-0.33 + sin(time * 0.004) * 0.10, -0.27);
}

fn coast_orb(uv: vec2<f32>, night: f32, low: f32, time: f32) -> vec3<f32> {
    let moon = smoothstep(0.62, 0.90, night);
    let radius = mix(0.085, 0.10, moon);
    let q = (uv - coast_orb_position(time)) / radius;
    let radial = length(q);
    let aa = max(pixel() / radius, 0.003);
    let disc = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, radial);
    var colour = vec3<f32>(0.0);
    if radial < 1.0 + aa {
        let normal = vec3<f32>(q.x, -q.y, sqrt(max(0.0, 1.0 - dot(q, q))));
        let fine = filtered_sine(q.x * 31.0 + sin(q.y * 17.0 + time * 0.035) * 2.3, aa * 66.0)
            * filtered_sine(q.y * 29.0 - time * 0.027, aa * 29.0);
        let convection = 0.5 + 0.5 * sin(q.x * 8.0 + sin(q.y * 5.7 + time * 0.041) * 2.4);
        let solar = mix(vec3<f32>(1.0, 0.30, 0.075), vec3<f32>(1.0, 0.72, 0.30),
            clamp(normal.z * 0.58 + convection * 0.22 + fine * 0.08, 0.0, 1.0));
        let maria = sin(q.x * 6.8 + sin(q.y * 5.4)) * sin(q.y * 7.3 - q.x * 2.4);
        var lunar = vec3<f32>(0.62, 0.73, 0.89) * (0.72 + maria * 0.13 + fine * 0.07);
        let craters = array<vec3<f32>, 6>(vec3<f32>(-0.42, 0.28, 0.22), vec3<f32>(0.27, -0.24, 0.26),
            vec3<f32>(0.36, 0.43, 0.13), vec3<f32>(-0.12, -0.60, 0.12),
            vec3<f32>(-0.51, -0.31, 0.10), vec3<f32>(0.01, 0.13, 0.085));
        for (var crater = 0; crater < 6; crater += 1) {
            let c = craters[crater];
            let delta = q - c.xy;
            let distance = length(delta);
            let bowl = 1.0 - smoothstep(c.z * 0.62, c.z, distance);
            let rim = exp(-abs(distance - c.z) / (0.010 + aa * 0.65));
            lunar *= 1.0 - bowl * 0.17;
            lunar += vec3<f32>(0.12, 0.14, 0.18) * rim * clamp(0.6 - delta.x / c.z, 0.0, 1.0);
        }
        let illumination = 0.10 + 0.90 * max(dot(normal, normalize(vec3<f32>(-0.62, 0.26, 0.75))), 0.0);
        colour = mix(solar * (0.62 + normal.z * 0.12), lunar * illumination * 0.92, moon) * disc;
    }
    // Radius/position are audio-independent. Bass breathes broad halo light.
    let glow = exp(-radial * 2.1) * (0.11 + low * 0.17);
    return colour + mix(vec3<f32>(1.0, 0.30, 0.11), vec3<f32>(0.32, 0.48, 0.85), moon) * glow;
}

fn coast_sky(uv: vec2<f32>, appearance: CoastLook, audio: vec3<f32>, hue: f32, time: f32) -> CoastSky {
    let night = appearance.mood.x;
    let horizon = -0.11;
    let sky_height = clamp((horizon - uv.y) * 0.9, 0.0, 1.0);
    let dusk = mix(vec3<f32>(0.78, 0.21, 0.20), vec3<f32>(0.29, 0.045, 0.30), night);
    let zenith = mix(vec3<f32>(0.095, 0.075, 0.25), vec3<f32>(0.017, 0.013, 0.075), night);
    var sky = mix(dusk, zenith, smoothstep(0.0, 1.0, sky_height));
    let cloud_domain = vec2<f32>(uv.x * 1.5 + time * 0.008, uv.y * 7.0);
    let cloud = sin(cloud_domain.x + sin(cloud_domain.y * 0.6))
        * sin(cloud_domain.y + sin(cloud_domain.x * 0.7));
    sky += vec3<f32>(0.13, 0.07, 0.13) * smoothstep(0.12, 0.75, cloud) * sky_height;
    let sun_position = coast_orb_position(time);
    let warm_haze = exp(-length((uv - sun_position) * vec2<f32>(1.0, 2.4)) * 3.7);
    let wisps = 0.5 + 0.5 * sin(uv.y * 34.0 + sin(uv.x * 2.7 + time * 0.007) * 2.3
        + sin(uv.y * 11.0 + uv.x * 1.7) * 1.2);
    let haze_layer = smoothstep(0.40, 0.88, wisps) * exp(-abs(uv.y - horizon + 0.10) * 6.0);
    sky += vec3<f32>(0.38, 0.17, 0.16) * warm_haze * 0.65 * (1.0 - night * 0.68);
    sky = mix(sky, sky * vec3<f32>(0.84, 0.74, 0.88) + vec3<f32>(0.055, 0.025, 0.05), haze_layer * 0.30);
    // Opaque objects fog toward this disc-free, music-independent atmosphere.
    // Thus bass cannot create false windows or transmit road light through walls.
    let ambient = rotate_hue(sky, appearance.colour.y) * 0.64;
    let fresh = select(0.0, 1.0, u.raster.z > 0.0 && u.e.x > 0.0);
    let low = audio.x * 0.65 + coast_response(u.music.z) * fresh * 0.35;
    let sky_bloom = exp(-abs(uv.y - horizon + 0.12) * 1.45);
    sky += coast_colour(hue + 0.02) * low * 0.32 * sky_bloom;
    sky = rotate_hue(sky, low * 0.045);
    sky += coast_orb(uv, night, low, time);
    sky = rotate_hue(sky, appearance.colour.y);
    return CoastSky(sky, ambient);
}

fn neon_coast(uv: vec2<f32>) -> World {
    let appearance = coast_look();
    let audio = coast_audio();
    let time = u.a.z;
    let travel = time * 5.4;
    let cache = coast_cache(travel);
    let hue = appearance.colour.x + sin(time * 0.011) * 0.075
        + (coast_landscape(travel / 167.0, 313.0) - 0.5) * 0.10;
    let night = appearance.mood.x;
    let horizon = -0.11;
    let sun_position = coast_orb_position(time);
    let atmosphere = coast_sky(uv, appearance, audio, hue, time);
    let sky = atmosphere.colour;
    let eye = vec3<f32>(0.0, 2.7, 0.0);
    let direction = vec3<f32>(uv.x / 1.2 + coast_cached_heading(travel, cache), (horizon - uv.y) / 1.2, 1.0);
    var colour = sky;
    if direction.y < -0.00001 {
        let depth = min(2.7 / -direction.y, 600.0);
        let world_z = travel + depth;
        let x = direction.x * depth + coast_cached_route(travel, cache) - coast_cached_route(world_z, cache);
        let road_width = coast_cached_width(world_z, appearance.colour.z, cache);
        let water = smoothstep(appearance.city.z - 0.8, appearance.city.z + 0.8, abs(x)) * appearance.mood.y;
        let distance_fog = 1.0 - exp(-depth * 0.018);
        let footprint = max(pixel() * depth, 0.012);
        let meander = sin(x * 0.31 + world_z * 0.13 + time * 0.21) * 2.8
            + sin(x * 0.73 - world_z * 0.071 + time * 0.13) * 1.3;
        let ripple_phase = world_z * 1.8 + meander * 2.3;
        let ripple = filtered_sine(ripple_phase, footprint * 2.7);
        let fine = filtered_sine(world_z * 5.9 + x * 0.7 + meander * 8.0 - time * 0.60, footprint * 11.0);
        let reflection_hue = hue + sin(x * 0.27 + time * 0.025) * 0.15;
        let reflection = coast_colour(reflection_hue);
        let rippled_x = x + ripple * (0.035 + water * 0.30);
        let reflected_lights = pow(max(0.0, 0.5 + 0.5 * sin(rippled_x * 1.5 + meander * water * 0.25)), 6.0);
        let water_colour = mix(vec3<f32>(0.026, 0.10, 0.15), vec3<f32>(0.008, 0.024, 0.068), night)
            + sky * 0.12 + reflection * (0.035 + reflected_lights * (0.12 + audio.x * 0.40 + audio.y * 0.18)) * (0.73 + ripple * 0.18)
            + vec3<f32>(0.21, 0.66, 0.76) * pow(max(fine, 0.0), 8.0) * (0.006 + audio.z * 0.12)
                * smoothstep(-0.10, 0.75, ripple);
        var ground = vec3<f32>(0.023, 0.020, 0.042) + reflection * reflected_lights * (0.07 + audio.x * 0.40 + audio.y * 0.16);
        let road = 1.0 - smoothstep(road_width - 0.10, road_width + 0.10, abs(x));
        ground *= 0.66 + road * 0.34;
        let kerb = exp(-abs(abs(x) - road_width) / max(footprint, 0.055));
        ground += coast_colour(hue + 0.18) * kerb * (0.30 + audio.y * 0.55 + audio.z * 0.45) * appearance.mood.z;
        // Perspective lane paint moves with world position, not a repeating screen pulse.
        let lane = 1.0 - smoothstep(0.035, 0.035 + footprint, abs(abs(x) - road_width * 0.42));
        let dash = smoothstep(0.25, 0.38, sin(world_z * 0.52));
        ground += vec3<f32>(0.65, 0.40, 0.23) * lane * dash * 0.20 * road;
        let wave = 0.5 + 0.5 * filtered_sine(world_z * 0.25 + time * 0.5 + x * 0.3, footprint * 0.55);
        ground += reflection * reflected_lights * wave * audio.y * 0.27;
        ground += coast_colour(hue + 0.25) * reflected_lights * pow(max(fine, 0.0), 5.0) * audio.z * 0.20;
        ground = mix(ground, water_colour, water);
        // Sun shimmer extends along the water into the viewer, without radial rings.
        let sun_lane = exp(-pow((uv.x - sun_position.x) / (0.055 + (uv.y - horizon) * 0.27), 2.0));
        let moon = smoothstep(0.62, 0.90, night);
        ground += mix(vec3<f32>(0.95, 0.33, 0.12), vec3<f32>(0.26, 0.45, 0.75), moon)
            * sun_lane * (0.12 + max(ripple, 0.0) * 0.21) * water * (0.8 + audio.x * 0.45 + audio.z * 0.3);
        colour = mix(ground, sky * 0.64, distance_fog);
    }
    let city = coast_buildings_cached(colour, atmosphere.ambient, direction, eye, travel, appearance, audio, hue, cache);
    colour = coast_palms_cached(city, uv, travel, appearance, audio, hue, cache);
    // Keep architecture in the material channel; shared palette treatment remains.
    return World(colour * 0.18, colour * 0.82);
}
