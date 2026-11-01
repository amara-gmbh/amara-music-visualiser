@group(0) @binding(1) var<storage, read> sound_history: array<f32>;

struct RasterVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec3<f32>,
    @location(1) local: vec2<f32>,
    @location(2) @interpolate(flat) kind: u32,
};

fn quad_corner(index: u32) -> vec2<f32> {
    let corners = array<vec2<f32>, 6>(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0));
    return corners[index % 6u];
}

fn camera_point(point: vec3<f32>, time: f32, tilt: f32) -> vec3<f32> {
    let yaw = rotate(point.xz, time * 0.095);
    let pitch = rotate(vec2<f32>(point.y, yaw.y), tilt);
    return vec3<f32>(yaw.x, pitch.x, pitch.y);
}

fn project_raster(point: vec3<f32>, lens: f32) -> vec4<f32> {
    let depth = max(6.0 - point.z, 0.25);
    let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
    let xy = point.xy * lens / depth;
    return vec4<f32>((xy.x + u.g.w) / aspect, xy.y, 1.0 - 0.1 / depth, 1.0);
}

fn fitted_raster_lens(radius: f32, original: f32) -> f32 {
    let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
    // u.g.w is the usable canvas centre relative to the whole window. Removing
    // its absolute offset gives the space beside an open sidebar, not behind it.
    let available = max(aspect - abs(u.g.w), 0.05);
    let aperture = min(available * 0.90, 0.90);
    // Exact projected silhouette of a centred bounding sphere at camera z=6.
    // Keep a small pixel gutter for the point sprites beyond their centres.
    let bounded_radius = clamp(radius, 0.05, 5.85);
    let silhouette = bounded_radius / sqrt(36.0 - bounded_radius * bounded_radius);
    let gutter = 6.0 / max(u.a.y, 1.0);
    return min(original, max(aperture - gutter, 0.02) / silhouette);
}

@vertex
fn vs_constellation(@builtin(vertex_index) vertex: u32, @builtin(instance_index) index: u32) -> RasterVertex {
    let corner = quad_corner(vertex);
    let point = constellation_point(index);
    let camera = camera_point(point.xyz, u.a.z, 0.22 + sin(u.a.z * 0.07) * 0.16);
    var clip = project_raster(camera, constellation_fit_lens());
    let radius = 1.05 + point.w * 0.80 + detail_level() * 0.30;
    // Give the filtered Gaussian a padded support. Cutting a tiny circular
    // sprite at a pixel-centre boundary makes moving dots twinkle/shimmer.
    clip.x += corner.x * radius * 4.0 / max(u.a.x, 1.0);
    clip.y += corner.y * radius * 4.0 / max(u.a.y, 1.0);
    let depth = clamp(0.62 + camera.z * 0.17, 0.22, 1.0);
    var out: RasterVertex;
    out.position = clip;
    out.colour = constellation_colour(index, point) * depth * 1.65;
    out.local = corner * 2.0;
    out.kind = 0u;
    return out;
}

@vertex
fn vs_attractor(@builtin(vertex_index) vertex: u32, @builtin(instance_index) index: u32,
    @location(0) a: vec3<f32>, @location(1) b: vec3<f32>, @location(2) c: vec3<f32>,
    @location(3) na: vec3<f32>, @location(4) nb: vec3<f32>, @location(5) nc: vec3<f32>) -> RasterVertex {
    let corner = quad_corner(vertex);
    let audio = attractor_audio();
    let point = attractor_point(a, b, c, index, audio);
    let next = attractor_point(na, nb, nc, index + 1u, audio);
    let camera = attractor_camera(point.xyz);
    let next_camera = attractor_camera(next.xyz);
    // Radius1.2 paths, at most1.107 morph compensation and1.24 bass scale,
    // plus <=sqrt(3)*0.04 flex. Music twists preserve radius; the complete
    // envelope stays below1.72 at every recipe and gain.
    // A fixed envelope avoids beat-driven camera pumping while filling the view.
    let lens = fitted_raster_lens(1.73, 4.8);
    let pa = project_raster(camera, lens); let pb = project_raster(next_camera, lens);
    let pixel_span = (pb.xy - pa.xy) * u.a.xy;
    let tangent = pixel_span / max(length(pixel_span), 0.0001);
    let normal = vec2<f32>(-tangent.y, tangent.x);
    var clip = mix(pa, pb, corner.x * 0.5 + 0.5);
    let along = f32(index) / 11999.0;
    let stream = attractor_stream(along, audio);
    // Padded luminous threads: a narrow filtered core and low local halo,
    // not thousands of equally bright hairlines over the same path.
    let width = 2.1 + stream * 1.1 + audio.z * 0.35;
    clip.x += normal.x * corner.y * width * 2.0 / max(u.a.x, 1.0);
    clip.y += normal.y * corner.y * width * 2.0 / max(u.a.y, 1.0);
    var out: RasterVertex;
    out.position = clip;
    out.colour = attractor_colour(index, point, audio) * (0.48 + stream * 1.25)
        * clamp(0.72 + camera.z * 0.22, 0.35, 1.0);
    out.local = vec2<f32>(along, corner.y);
    out.kind = 2u;
    return out;
}

fn history_sample(band: u32, row: u32) -> f32 {
    if u.raster.z <= 0.0 { return 0.0; }
    if row == 0u { return spectrum_at(f32(band) / 47.0); }
    let head = u32(u.raster.x);
    return sound_history[((head + 64u - (min(row, 63u) - 1u)) % 64u) * 48u + min(band, 47u)];
}

fn history_age(row: u32) -> f32 {
    // Retained ridges move through space without repeatedly changing height.
    // At insertion an existing row advances one index while phase wraps, so
    // its position is continuous. Only the live leading strip changes shape.
    if row == 0u { return 0.0; }
    return f32(row - 1u) + u.raster.y;
}

fn landscape_height(level: f32, style: f32) -> f32 {
    return pow(clamp(level, 0.0, 1.5), 0.72) * (1.45 + sin(style * 0.7) * 0.25);
}

@vertex
fn vs_landscape(@builtin(vertex_index) vertex: u32) -> RasterVertex {
    let triangle = vertex / 6u;
    let corner = quad_corner(vertex) * 0.5 + 0.5;
    let x = triangle % 47u + u32(corner.x);
    let z = triangle / 47u + u32(corner.y);
    let band = f32(x) / 47.0;
    let age = history_age(z);
    let level = history_sample(x, z);
    let state = u.looks[6];
    let style = mix(state.x, state.y, state.z);
    let height = landscape_height(level, style);
    let bend = sin(style * 0.8) * 0.18;
    var point = vec3<f32>((band - 0.5) * 5.7, height - 0.8, (0.5 - age / 63.0) * 5.0);
    point.x += sin(point.z * 0.65) * bend;
    let yaw = sin(u.a.z * 0.048) * 0.35;
    let xz = rotate(point.xz, yaw);
    let yz = rotate(vec2<f32>(point.y, xz.y), 0.46);
    let camera = vec3<f32>(xz.x, yz.x + 0.15, yz.y);
    let left = select(0u, x - 1u, x > 0u); let right = min(x + 1u, 47u);
    let near = select(0u, z - 1u, z > 0u); let far = min(z + 1u, 63u);
    let dx = (landscape_height(history_sample(right, z), style)
        - landscape_height(history_sample(left, z), style)) / (f32(right - left) * 5.7 / 47.0);
    var dz = (landscape_height(history_sample(x, far), style)
        - landscape_height(history_sample(x, near), style))
        / max(history_age(far) - history_age(near), 0.35) * (-63.0 / 5.0);
    if z == 0u {
        // A full-row lookback gives the leading edge the same normal before
        // and after insertion, even when its newly born strip has zero width.
        let behind = mix(landscape_height(history_sample(x, 1u), style),
            landscape_height(history_sample(x, 2u), style), 1.0 - u.raster.y);
        dz = (behind - height) * (-63.0 / 5.0);
    }
    let normal = normalize(vec3<f32>(-dx, 1.0, -dz));
    let light = 0.26 + max(dot(normal, normalize(vec3<f32>(-0.5, 0.8, 0.6))), 0.0) * 0.74;
    let low = mix(vec3<f32>(0.09, 0.18, 0.26), vec3<f32>(0.19, 0.39, 0.62), level);
    let high = mix(vec3<f32>(0.23, 0.11, 0.34), vec3<f32>(0.77, 0.50, 0.24), band);
    let material = mix(low, high, smoothstep(0.15, 0.85, height));
    var out: RasterVertex;
    // The old fixed 2.15 lens cropped the nearest ridges, especially beside
    // the sidebar. Bound all looks/heights and the full +/-0.35 yaw range,
    // not this frame's sound, so the camera never pumps on a musical peak.
    // Per-unit lens extrema are <=0.98 horizontally and <=0.624 vertically.
    let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
    let available = max(aspect - abs(u.g.w), 0.05);
    let lens = min(1.35, available * 0.86 / 0.98);
    out.position = project_raster(camera, lens);
    out.colour = material * light * (0.40 + level * 1.4) * (1.0 - age / 63.0 * 0.45)
        * (1.0 - smoothstep(60.0, 62.0, age));
    // Grid lines travel with each measured row; an integer offset at insertion
    // leaves their fractional coordinates unchanged instead of flashing a line.
    out.local = vec2<f32>(band * 47.0, age + 1.0 - u.raster.y);
    out.kind = 1u;
    return out;
}

@fragment
fn fs_raster(in: RasterVertex) -> @location(0) vec4<f32> {
    let footprint = max(fwidth(in.local), vec2<f32>(0.001));
    if in.kind == 1u {
        // Discrete frequency/history coordinates remain legible on the solid mesh.
        let cell = abs(fract(in.local) - 0.5);
        let edges = smoothstep(vec2<f32>(0.5) - footprint * 0.85, vec2<f32>(0.5), cell);
        let grid = max(edges.x, edges.y);
        return vec4<f32>(in.colour * (0.8 + grid * 0.65), 1.0);
    }
    var coverage = 1.0 - smoothstep(0.25, 1.0, abs(in.local.y));
    if in.kind == 2u {
        let variance = 0.052 + footprint.y * footprint.y / 12.0;
        let core = sqrt(0.052 / variance) * exp(-0.5 * in.local.y * in.local.y / variance);
        let halo = exp(-in.local.y * in.local.y * 3.0)
            * (1.0 - smoothstep(0.72, 1.0, abs(in.local.y)));
        coverage = core * 0.74 + halo * 0.035;
    }
    if in.kind == 0u {
        // Pixel-footprint filtering: approximate the box footprint by its
        // variance and preserve the Gaussian's integrated light. This avoids
        // hard circular discards and softens fractional-pixel motion without
        // retaining stale dot positions in a long framebuffer trail.
        let variance = vec2<f32>(0.20) + footprint * footprint / 12.0;
        let exponent = -0.5 * dot(in.local * in.local, 1.0 / variance);
        coverage = 0.20 / sqrt(variance.x * variance.y) * exp(exponent);
    }
    return vec4<f32>(in.colour * coverage, coverage);
}
