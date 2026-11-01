// Original Mandelbrot renderer: z starts at zero and c is the camera's point
// in the complex plane. Music never changes c's set equation into Julia.
// These are finite f32 excursions, not an arbitrary-precision infinite zoom.
struct MandelbrotView {
    focus: vec2<f32>,
    span: f32,
    angle: f32,
    pigment: f32,
};

// Physical visible exposure starts anew on each world visit. The valid flag
// leaves old diagnostic callers compatible with their explicit time samples.
fn mandelbrot_age() -> f32 {
    return select(max(u.a.z, 0.0), u.material.y, u.material.z > 0.5);
}

fn mandelbrot_formation() -> f32 {
    return smoothstep(0.0, 10.0, mandelbrot_age());
}

fn mandelbrot_formed_camera(view: MandelbrotView) -> MandelbrotView {
    let age = mandelbrot_age();
    // Begin close enough to this recipe's true boundary to see its structure,
    // rather than centring a huge cardioid silhouette on every pullback.
    // Only after the broad shape has formed do we dive closely. Approach
    // starts with zero velocity and is bounded to <=0.45 log-scale units/sec;
    // the tiniest recipes therefore cannot jump through several zoom decades.
    let aim = smoothstep(3.0, 7.0, age);
    let travel = max(age - 5.0, 0.0);
    let distance = 0.45 * (travel - 1.5 * (1.0 - exp(-travel / 1.5)));
    let approach = 1.0 - exp(-distance);
    let opening_span = clamp(view.span * 3.0, 0.24, 0.65);
    let span = mix(opening_span, view.span, approach);
    // The seahorse valley is narrow between two large interior lobes. Descend
    // along its coloured outer shoulder before settling on the exact target,
    // so the middle of a short visit is not swallowed by those black lobes.
    // This C1 window vanishes before the held detail view; the set and final
    // camera destination are unchanged, including during recipe blends.
    let look = u.looks[10];
    let seahorse = mix(select(0.0, 1.0, look.x < 0.5),
        select(0.0, 1.0, look.y < 0.5), look.z);
    let shoulder = smoothstep(6.0, 9.0, age) * (1.0 - smoothstep(12.0, 20.0, age));
    var focus = view.focus + vec2<f32>(-0.18, 1.10) * span * shoulder * seahorse;
    var angle = view.angle * mix(0.25, 1.0, aim);
    if u.material.w > 0.0 {
        // A bounded orbit keeps broad creation visibly travelling even at an
        // eased depth turn. Its integrated Pace clock never resets per region;
        // the CPU envelope makes Auto/manual switches continuous too.
        let orbit = u.a.z * 0.24;
        focus += vec2<f32>(cos(orbit), sin(orbit)) * span * 0.12 * u.material.w;
        angle += sin(u.a.z * 0.11) * 0.14 * u.material.w;
    }
    return MandelbrotView(focus, span, angle, view.pigment);
}

fn mandelbrot_view(id: f32, time: f32) -> MandelbrotView {
    // Complex-plane focal point, broad half-height, finite zoom depth in logs.
    let destinations = array<vec4<f32>, 10>(
        vec4<f32>(-0.7436439, 0.1318259, 0.028, 3.0),
        vec4<f32>(-0.5500, 0.0000, 1.15, 0.25),
        vec4<f32>(0.2820, 0.0080, 0.085, 1.8),
        vec4<f32>(-0.04172, 0.68871, 0.070, 1.8),
        vec4<f32>(-1.2500, 0.0000, 0.230, 1.0),
        vec4<f32>(-1.7690, 0.0040, 0.070, 1.4),
        vec4<f32>(-0.1600, 1.0360, 0.060, 1.3),
        vec4<f32>(-0.7450, 0.1860, 0.065, 1.4),
        // Upper shoulder of the period-two bulb, on its true boundary. The
        // former target was in empty exterior even at its deepest zoom.
        vec4<f32>(-1.143394, 0.204788, 0.045, 1.8),
        vec4<f32>(-1.25066, 0.03070, 0.016, 1.8)
    );
    let index = u32(clamp(round(id), 0.0, 9.0));
    let recipe = destinations[index];
    // The already-integrated travel clock follows Pace and musical phrasing.
    // A cosine turns the finite excursion around with zero radial velocity;
    // there is no modulo reset, respawn, beat-time multiplication or zoom cut.
    let excursion = 0.5 - 0.5 * cos(time * 0.075);
    let span = max(0.0008, recipe.z * exp(-recipe.w * excursion));
    var focus = recipe.xy;
    // Follow the outside shoulder as the view opens, so nearby large bulbs
    // cannot fill the whole canvas during these narrow-valley excursions.
    if index == 2u { focus.x += (1.0 - excursion) * 0.040; }
    if index == 9u { focus.y += (1.0 - excursion) * 0.016; }
    let orbit = time * 0.027 + f32(index) * 0.47;
    let offset = vec2<f32>(sin(orbit * 0.71), cos(orbit * 0.83)) * span * 0.10;
    return MandelbrotView(focus + offset, span,
        sin(time * 0.021) * 0.18 + f32(index) * 0.31, f32(index) * 0.087);
}

fn mandelbrot_camera() -> MandelbrotView {
    let look = u.looks[10];
    let time = u.a.z;
    let a = mandelbrot_view(look.x, time);
    if look.z <= 0.0 { return mandelbrot_formed_camera(a); }
    let b = mandelbrot_view(look.y, time);
    if look.z >= 1.0 || look.x == look.y { return mandelbrot_formed_camera(b); }
    // Pull back while navigating between distant boundaries, then descend.
    // This is one continuous complex-plane camera, never a set crossfade.
    let progress = smoothstep(0.0, 1.0, look.z);
    let bridge = sin(progress * 3.14159265);
    // Travel over the outside shoulder instead of cutting through the solid
    // cardioid and period-two bulb. Both endpoints have zero route velocity.
    let route = vec2<f32>(0.0, 0.48 * bridge);
    return mandelbrot_formed_camera(MandelbrotView(mix(a.focus, b.focus, progress) + route,
        exp(mix(log(a.span), log(b.span), progress))
            + length(a.focus - b.focus) * bridge * 0.65,
        mix(a.angle, b.angle, progress), mix(a.pigment, b.pigment, progress)));
}

fn mandelbrot_bands() -> vec3<f32> {
    var groups = vec3<f32>(0.0);
    // Uniforms already apply the independent user mapping to every FFT row.
    for (var i = 0u; i < 48u; i += 1u) {
        let sample = u.spectrum[i / 4u][i % 4u];
        if i < 8u { groups.x += sample / 8.0; }
        else if i < 24u { groups.y += sample / 16.0; }
        else { groups.z += sample / 24.0; }
    }
    // Keep FFT detail visible even under a strong scalar envelope; max()
    // would hide quieter spectral changes below that envelope's floor.
    return clamp(groups * 0.85 + vec3<f32>(bass(), mids(), air()) * 0.65,
        vec3<f32>(0.0), vec3<f32>(1.5));
}

fn mandelbrot(uv: vec2<f32>) -> World {
    let camera = mandelbrot_camera();
    let formed = mandelbrot_formation();
    // Unclassified pixels start as one quiet field. As the actual escape
    // iterations reveal the exterior, the unresolved interior recedes into
    // ink; this avoids presenting a completed black silhouette on frame one.
    let unclassified = mix(vec3<f32>(0.035, 0.030, 0.058),
        vec3<f32>(0.004, 0.002, 0.013), smoothstep(0.0, 1.0, formed));
    let c = camera.focus + rotate(uv, camera.angle) * camera.span;
    let cardioid_x = c.x - 0.25;
    let cardioid_q = cardioid_x * cardioid_x + c.y * c.y;
    if cardioid_q * (cardioid_q + cardioid_x) <= 0.25 * c.y * c.y
        || (c.x + 1.0) * (c.x + 1.0) + c.y * c.y <= 0.0625 {
        return World(unclassified, vec3<f32>(0.0));
    }
    var z = vec2<f32>(0.0);
    var derivative = vec2<f32>(0.0);
    var iteration = 0.0;
    var first_escape = 0.0;
    var previous_radius_squared = 0.0;
    let limit = u32(160.0 + detail_level() * 192.0);
    // Analytic derivative dz/dc supplies distance/relief without offset copies
    // of the fractal. The escape threshold is fixed and well outside radius 2.
    for (var i = 0u; i < 352u; i += 1u) {
        if i >= limit { break; }
        derivative = 2.0 * vec2<f32>(z.x * derivative.x - z.y * derivative.y,
            z.x * derivative.y + z.y * derivative.x) + vec2<f32>(1.0, 0.0);
        z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        iteration = f32(i) + 1.0;
        let radius_squared = dot(z, z);
        if first_escape == 0.0 && radius_squared > 4.0 {
            // The radius-two escape criterion gives the actual finite-step
            // construction. Interpolate its last crossing for a moving light
            // frontier rather than popping all pixels of an integer step.
            let previous_radius = sqrt(previous_radius_squared);
            first_escape = f32(i) + clamp((2.0 - previous_radius)
                / max(sqrt(radius_squared) - previous_radius, 0.000001), 0.0, 1.0);
        }
        previous_radius_squared = radius_squared;
        if radius_squared > 65536.0 { break; }
    }
    let music = mandelbrot_bands();
    if first_escape == 0.0 {
        // This point has not escaped within the finite budget. It may be
        // interior or simply need more iterations; do not claim certainty.
        return World(unclassified, vec3<f32>(0.0));
    }
    let radius = max(length(z), 1.001);
    let smooth_iteration = iteration + 1.0 - log2(log2(radius));
    let deriv_length = max(length(derivative), 0.000001);
    let distance = radius * log(radius) / deriv_length;
    let edge_pixels = distance / max(camera.span * pixel(), 0.00000001);
    let normal_complex = vec2<f32>(dot(z, derivative),
        z.y * derivative.x - z.x * derivative.y) / max(radius * deriv_length, 0.000001);
    let light_direction = rotate(vec2<f32>(0.7, -0.7), camera.angle);
    let relief = clamp(dot(normal_complex, light_direction) * 0.5 + 0.5, 0.0, 1.0);
    // Broad vinyl-like bands with fine etched contours: music lights the
    // mathematical relief; it does not displace the complex-plane boundary.
    let phase = smooth_iteration * (0.19 + music.y * 0.032)
        + camera.pigment + u.a.z * 0.006;
    let coloured = 0.48 + 0.48 * cos(6.2831853
        * (vec3<f32>(phase) + vec3<f32>(0.00, 0.31, 0.61)));
    // At subpixel escape distances the iteration colour oscillates too fast
    // to resolve. Converge on a stable ink colour instead of rainbow sparkle.
    let resolved = smoothstep(0.25, 2.5, edge_pixels);
    let pigment = mix(vec3<f32>(0.12, 0.44, 0.58), coloured, resolved);
    let contour_phase = smooth_iteration * 0.86;
    let contour = pow(0.5 + 0.5 * cos(contour_phase), 12.0) * resolved;
    let fine = pow(0.5 + 0.5 * cos(smooth_iteration * 2.6), 20.0)
        * smoothstep(0.7, 3.0, edge_pixels);
    let coverage = smoothstep(0.03, 0.8, edge_pixels);
    let edge = exp(-max(edge_pixels, 0.0) * 0.42) * coverage;
    let body = pigment * (0.10 + relief * (0.20 + music.x * 0.32)) * coverage;
    let grooves = pigment * contour * (0.10 + music.y * 0.25);
    let glint = mix(vec3<f32>(0.16, 0.54, 0.96), vec3<f32>(1.0, 0.47, 0.13), relief);
    let detail = glint * (fine * (0.025 + music.z * 0.21)
        + edge * (0.08 + music.z * 0.16));
    let budget = 1.3 + (f32(limit) - 0.3) * pow(formed, 1.8);
    let construction = smoothstep(first_escape - 0.65, first_escape + 0.65, budget);
    let frontier = exp(-pow((first_escape - budget) / (0.70 + budget * 0.018), 2.0))
        * (1.0 - smoothstep(0.90, 1.0, formed));
    // Escape-time generations illuminate where the calculation is reaching
    // next. Music colours and lights this real frontier; it never changes the
    // set equation, creates fake branches or moves the construction clock.
    let growth_light = mix(pigment, glint, 0.25)
        * frontier * (0.24 + music.x * 0.12 + music.y * 0.10 + music.z * 0.12);
    return World(mix(unclassified, body + grooves, construction),
        detail * construction + growth_light);
}
