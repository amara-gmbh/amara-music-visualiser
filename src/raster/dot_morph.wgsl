// A genuinely geometric passage between the two dot worlds. Stable identities
// travel through camera-space XYZ, not a screen-space dissolve of two images.
// The 50,000 equaliser points share the 12,000 original constellation origins.
// Splitting each origin's light by its exact multiplicity conserves energy.

fn constellation_material_point(index: u32) -> DotMaterialPoint {
    let original = index % 12000u;
    let point = constellation_point(original);
    let camera = camera_point(point.xyz, u.a.z, 0.22 + sin(u.a.z * 0.07) * 0.16);
    let radius = 1.05 + point.w * 0.80 + detail_level() * 0.30;
    let depth = clamp(0.62 + camera.z * 0.17, 0.22, 1.0);
    let multiplicity = 1u + (49999u - original) / 12000u;
    let colour = constellation_colour(original, point) * depth * 1.65 / f32(multiplicity);
    return DotMaterialPoint(camera, constellation_fit_lens(), radius, colour);
}

fn dot_world_amount(index: u32, progress: f32) -> f32 {
    if progress <= 0.0 { return 0.0; }
    if progress >= 1.0 { return 1.0; }
    // A coherent front crosses frequency/depth coordinates. Adjacent columns
    // depart together, instead of jittery per-frame RNG or a uniform collapse.
    let band = f32(index % 50u) / 49.0;
    let depth = f32(index / 2500u) / 19.0;
    let delay = 0.09 + 0.06 * sin(band * 6.283185307 + depth * 3.141592654);
    let t = clamp((progress - delay) / 0.82, 0.0, 1.0);
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
}

@vertex
fn vs_dot_morph(@builtin(vertex_index) vertex: u32,
    @builtin(instance_index) index: u32) -> RasterVertex {
    let progress = select(1.0 - u.f.w, u.f.w, u.a.w == 7.0);
    let amount = dot_world_amount(index, clamp(progress, 0.0, 1.0));
    if amount <= 0.0 {
        return dot_material_vertex(constellation_material_point(index), vertex);
    }
    if amount >= 1.0 {
        return dot_material_vertex(equaliser_material_point(index), vertex);
    }
    let before = constellation_material_point(index);
    let after = equaliser_material_point(index);
    // Use the same stable point coordinates in both directions. Reversing a
    // transition retraces the 3-D path, with zero drift at either endpoint.
    let seed = vec3<f32>(f32(index % 50u) / 49.0,
        f32(index / 2500u) / 19.0, hash(vec3<f32>(f32(index), 17.3, 43.7)));
    let camera = constellation_morph(before.camera, after.camera, seed, amount);
    let material = DotMaterialPoint(camera, mix(before.lens, after.lens, amount),
        mix(before.radius, after.radius, amount), mix(before.colour, after.colour, amount));
    return dot_material_vertex(material, vertex);
}
