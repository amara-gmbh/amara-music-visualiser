// 50 x 50 x 20 = 50,000 stable, GPU-instanced dots. Horizontal position
// interpolates the 48 measured log-frequency bands; depth is 2.7 seconds of
// measured history. Neither camera time nor a procedural oscillator invents
// column heights. Reuse the filtered Gaussian of the original dot renderer.

@group(0) @binding(2) var<storage, read> dot_history: array<f32>;

fn equaliser_sample(band: u32, row: u32) -> f32 {
    if u.raster.z <= 0.0 { return 0.0; }
    if row == 0u { return dot_history[3072u + min(band, 47u)]; }
    let head = u32(u.raster.x);
    return dot_history[((head + 64u - (min(row, 63u) - 1u)) % 64u) * 48u + min(band, 47u)];
}

fn equaliser_history_band(band: u32, age: f32) -> f32 {
    if u.raster.z <= 0.0 { return 0.0; }
    if age <= 0.0 { return equaliser_sample(band, 0u); }
    // Fixed physical ages, not fixed ring-buffer rows. The two retained rows
    // move past this age continuously when a new history row is inserted.
    let past = max(age - u.raster.y, 0.0);
    let before = u32(floor(past));
    let after = min(before + 1u, 61u);
    let retained = mix(equaliser_sample(band, before + 1u),
        equaliser_sample(band, after + 1u), fract(past));
    if age < u.raster.y {
        return mix(equaliser_sample(band, 0u), equaliser_sample(band, 1u),
            age / max(u.raster.y, 0.0001));
    }
    return retained;
}

fn equaliser_level(band: f32, age: f32) -> f32 {
    let bin = clamp(band, 0.0, 1.0) * 47.0;
    let lo = u32(floor(bin));
    return mix(equaliser_history_band(lo, age),
        equaliser_history_band(min(lo + 1u, 47u), age), fract(bin));
}

fn equaliser_recipe(id: u32) -> vec4<f32> {
    // Width, depth, gentle plan curvature, height. Every variation remains a
    // readable upright spectrum, never abruptly swaps to an unrelated shape.
    let recipes = array<vec4<f32>, 10>(
        vec4<f32>(5.4, 3.0, 0.00, 2.8), // Crystal Columns
        vec4<f32>(5.8, 2.4, 0.14, 2.5), // Velvet Steps
        vec4<f32>(5.2, 3.5, -0.24, 2.7), // Spectrum Arcade
        vec4<f32>(5.7, 3.2, 0.28, 2.4), // Twin Terraces
        vec4<f32>(4.9, 2.5, 0.08, 3.1), // Glass Organ
        vec4<f32>(5.6, 3.8, -0.12, 2.6), // Frequency Orchard
        vec4<f32>(5.0, 3.2, 0.42, 2.8), // Harmonic Fold
        vec4<f32>(5.9, 2.9, -0.32, 2.8), // Aurora Pillars
        vec4<f32>(5.1, 3.3, 0.18, 3.2), // Bass Cathedral
        vec4<f32>(5.8, 3.6, 0.00, 2.7)  // Prism Skyline
    );
    return recipes[min(id, 9u)];
}

fn equaliser_camera(point: vec3<f32>, yaw: f32, pitch: f32) -> vec3<f32> {
    let xz = rotate(point.xz, yaw);
    let yz = rotate(vec2<f32>(point.y, xz.y), pitch);
    return vec3<f32>(xz.x, yz.x, yz.y - 2.5);
}

fn equaliser_lens(recipe: vec4<f32>, yaw: f32, pitch: f32) -> f32 {
    // Fit the stable maximum sculpture bounds, NOT each frame's audio peaks.
    // This fills the usable canvas without pumping the zoom with every kick.
    let half_height = (0.025 + 1.5 / 1.78 * recipe.w * 1.30) * 0.5;
    let extent = vec3<f32>(recipe.x * 0.5 + 0.02, half_height,
        recipe.y + abs(recipe.z) + 0.04);
    var projected = vec2<f32>(0.001);
    for (var corner = 0u; corner < 8u; corner += 1u) {
        let signs = vec3<f32>(select(-1.0, 1.0, (corner & 1u) != 0u),
            select(-1.0, 1.0, (corner & 2u) != 0u),
            select(-1.0, 1.0, (corner & 4u) != 0u));
        let camera = equaliser_camera(extent * signs, yaw, pitch);
        projected = max(projected, abs(camera.xy) / max(6.0 - camera.z, 0.25));
    }
    let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
    let available = max(aspect - abs(u.g.w), aspect * 0.5);
    return min(available * 0.90 / projected.x, 0.88 / projected.y);
}

// Shared material/geometry endpoint for the real 3-D dot-world passage.
// The settled renderer below still uses exactly these original calculations.
struct DotMaterialPoint {
    camera: vec3<f32>,
    lens: f32,
    radius: f32,
    colour: vec3<f32>,
};

fn equaliser_material_point(index: u32) -> DotMaterialPoint {
    let column = index % 50u;
    let height_index = (index / 50u) % 50u;
    let depth_index = index / 2500u;
    let band = f32(column) / 49.0;
    let vertical = f32(height_index) / 49.0;
    let depth = f32(depth_index) / 19.0;
    let lane = u.looks[7];
    let recipe = mix(equaliser_recipe(u32(clamp(round(lane.x), 0.0, 9.0))),
        equaliser_recipe(u32(clamp(round(lane.y), 0.0, 9.0))), clamp(lane.z, 0.0, 1.0));
    let measured = clamp(equaliser_level(band, depth * 27.0), 0.0, 1.5);
    // Soft dynamic range preserves visible quiet detail without normalizing
    // silence into activity. Top and every interior dot use the same FFT bin.
    let amplitude = measured / (0.28 + measured);
    let height = 0.025 + amplitude * recipe.w * 1.30;
    let half_height = (0.025 + 1.5 / 1.78 * recipe.w * 1.30) * 0.5;
    let seed = hash(vec3<f32>(f32(column), f32(height_index), f32(depth_index) + 7.3));
    var point = vec3<f32>((band - 0.5) * recipe.x,
        vertical * height - half_height, (0.5 - depth) * recipe.y * 2.0);
    // Tiny fixed grain creates a point cloud without frame-to-frame jitter.
    point.x += (seed - 0.5) * 0.028;
    point.z += (hash(vec3<f32>(f32(index), 19.2, 8.1)) - 0.5) * 0.075;
    point.z += (band - 0.5) * (band - 0.5) * recipe.z * 4.0;
    // Fixed oblique side view with bounded slow drift: never flips to an
    // end-on or top-down view, and drums never shake the camera.
    let yaw = 0.48 + sin(u.a.z * 0.015) * 0.015;
    let pitch = 0.18 + sin(u.a.z * 0.012) * 0.005;
    let camera = equaliser_camera(point, yaw, pitch);
    let lens = equaliser_lens(recipe, yaw, pitch);
    let crown = smoothstep(0.93, 1.0, vertical);
    let perspective = clamp(6.0 / max(6.0 - camera.z, 0.25), 0.48, 1.45);
    let radius = (0.78 + detail_level() * 0.18 + crown * 0.22) * perspective;
    let cyan = vec3<f32>(0.10, 0.64, 0.85);
    let violet = vec3<f32>(0.59, 0.29, 0.88);
    let amber = vec3<f32>(1.0, 0.56, 0.23);
    let spectral_colour = mix(mix(cyan, violet, smoothstep(0.10, 0.65, band)),
        amber, smoothstep(0.63, 1.0, band));
    // Keep each frequency's pigment saturated even at the crown. Broad white
    // highlights made a dense cloud look pale and translucent; the small
    // highlight now sits inside the same cyan/violet/amber material.
    let peak = max(spectral_colour.x, max(spectral_colour.y, spectral_colour.z));
    let pigment = spectral_colour / max(peak, 0.001);
    let crown_colour = mix(pigment, vec3<f32>(1.0), crown * 0.08);
    // The measured interior carries enough light to read as a solid volume
    // of separate dots. Depth still recedes, but no longer erases its back
    // half. Silence and disconnected mappings retain the original quiet floor.
    let audible = smoothstep(0.003, 0.035, measured);
    let interior = (0.42 + vertical * 0.42 + crown * 0.55) * audible;
    let floor_light = select(0.0, 0.18, height_index == 0u);
    let depth_light = 0.75 + perspective * 0.45;
    let body_light = interior * (0.65 + amplitude * 1.10)
        * (1.0 - depth * 0.36) * depth_light;
    let floor_glow = floor_light * (1.0 - depth * 0.55) * depth_light;
    return DotMaterialPoint(camera, lens, radius,
        crown_colour * body_light + spectral_colour * floor_glow);
}

fn dot_material_vertex(point: DotMaterialPoint, vertex: u32) -> RasterVertex {
    var clip = project_raster(point.camera, point.lens);
    let corner = quad_corner(vertex);
    clip.x += corner.x * point.radius * 4.0 / max(u.a.x, 1.0);
    clip.y += corner.y * point.radius * 4.0 / max(u.a.y, 1.0);
    var out: RasterVertex;
    out.position = clip;
    out.colour = point.colour;
    out.local = corner * 2.0;
    out.kind = 0u;
    return out;
}

@vertex
fn vs_dot_equaliser(@builtin(vertex_index) vertex: u32,
    @builtin(instance_index) index: u32) -> RasterVertex {
    return dot_material_vertex(equaliser_material_point(index), vertex);
}
