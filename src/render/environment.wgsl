// Visual environment only. The ground is not a simulation collider.
struct Environment {
    inverse_view: mat4x4<f32>,
    view: mat4x4<f32>,
    top: vec4<f32>,
    bottom: vec4<f32>,
    ground: vec4<f32>,
    options: vec4<f32>, // ground height, ground enabled, grid enabled, shadow opacity
    shadow: vec4<f32>, // center x/z, radii x/z (contact-style preview, not a shadow map)
}
@group(0) @binding(0) var<uniform> env: Environment;
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let points = array<vec2<f32>, 3>(vec2(-1., -1.), vec2(3., -1.), vec2(-1., 3.));
    var out: VertexOutput;
    out.ndc = points[index];
    out.position = vec4(out.ndc, 0., 1.);
    return out;
}
struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}
@fragment fn fs_main(input: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    let backdrop = mix(env.top.rgb, env.bottom.rgb, clamp(0.5 - 0.5 * input.ndc.y, 0., 1.));
    out.color = vec4(backdrop, 1.);
    out.depth = 1.;
    let near_h = env.inverse_view * vec4(input.ndc, 0., 1.);
    let far_h = env.inverse_view * vec4(input.ndc, 1., 1.);
    let origin = near_h.xyz / near_h.w;
    let direction = normalize(far_h.xyz / far_h.w - origin);
    let dy = select(-max(abs(direction.y), 0.00001), max(abs(direction.y), 0.00001), direction.y >= 0.);
    let distance = (env.options.x - origin.y) / dy;
    let point = origin + direction * distance;
    let clip = env.view * vec4(point, 1.);
    let depth = clip.z / max(clip.w, 0.00001);
    let cell = point.xz / 0.2;
    // Derivatives must run for every fragment, including background pixels.
    let footprint = max(fwidth(cell), vec2(0.0001));
    let grid_distance = abs(fract(cell - 0.5) - 0.5) / footprint;
    let line = 1. - clamp(min(grid_distance.x, grid_distance.y), 0., 1.);
    let fade = 1. - smoothstep(3., 12., length(point.xz));
    let q = (point.xz - env.shadow.xy) / max(env.shadow.zw, vec2(0.01));
    let shadow = exp(-3. * dot(q, q)) * env.options.w;
    let grid = line * env.options.z * 0.16 * fade;
    let floor_color = env.ground.rgb * (1. - grid) * (1. - shadow);
    if env.options.y > 0.5 && distance > 0. && abs(direction.y) > 0.00001 && clip.w > 0. && depth >= 0. && depth < 1. {
        out.color = vec4(mix(backdrop, floor_color, fade), 1.);
        out.depth = depth;
    }
    return out;
}
