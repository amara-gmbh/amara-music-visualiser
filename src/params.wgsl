struct Params {
    a: vec4<f32>, // resolution, integrated travel time, destination world
    b: vec4<f32>, // measured band envelopes
    c: vec4<f32>, // beat, intensity, motion, detail
    d: vec4<f32>, // colour, glow, warp, palette
    e: vec4<f32>, // user-controlled audio mappings
    f: vec4<f32>, // DNA metadata, source world, transition
    g: vec4<f32>, // echo, dt, history valid, canvas centre
    h: vec4<f32>, // mapped slow phrase
    i: vec4<f32>, // phase, speed, interlace amount, seed
    j: vec4<f32>, // mathematical family blend
    spectrum: array<vec4<f32>, 12>,
    looks: array<vec4<f32>, 17>,
    raster: vec4<f32>, // spectral history head/fraction, freshness, source epoch
    voice: vec4<f32>, // heuristic presence, entry progress/envelope, continuous phase
    music: vec4<f32>, // smoothed rhythm density, pickup, low pulse, air detail
    ripples: array<vec4<f32>, 24>, // 12 measured impacts + 12 smoothly retiring tails
    background: vec4<f32>, // independent linear RGB background tint and mix amount
    water_bands: array<vec4<f32>, 2>, // water-only full-coverage smoothed frequency groups
    material: vec4<f32>, // independent world hue offset; remaining lanes reserved
};
@group(0) @binding(0) var<uniform> u: Params;

fn rotate(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle); let s = sin(angle);
    return vec2<f32>(c * p.x - s * p.y, s * p.x + c * p.y);
}
fn hash(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + vec3<f32>(33.33));
    return fract((q.x + q.y) * q.z);
}
fn spectrum_at(position: f32) -> f32 {
    let x = clamp(position, 0.0, 1.0) * 47.0;
    let lo = u32(floor(x)); let hi = min(lo + 1u, 47u);
    return mix(u.spectrum[lo / 4u][lo % 4u], u.spectrum[hi / 4u][hi % 4u],
        smoothstep(0.0, 1.0, fract(x)));
}
fn pixel() -> f32 { return 2.0 / max(u.a.y, 1.0); }
fn bass() -> f32 { return u.b.x * u.e.x; }
fn mids() -> f32 { return u.b.y * u.e.y; }
fn air() -> f32 { return u.b.z * u.e.z; }
fn beat() -> f32 { return u.c.x * u.e.w; }
fn detail_level() -> f32 { return clamp((u.c.w - 0.15) / 1.35, 0.0, 1.0); }
