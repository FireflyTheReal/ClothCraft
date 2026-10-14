@group(0) @binding(0) var<uniform> view_projection: mat4x4f;
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) barycentric: vec3<f32>,
};

@vertex
fn vs_main(@location(0) clip: vec4<f32>, @location(1) normal: vec3<f32>,
           @location(2) color: vec4<f32>, @location(3) bary: vec3<f32>) -> VertexOut {
    var out: VertexOut;
    out.position = view_projection * clip;
    out.normal = normal;
    out.color = color;
    out.barycentric = bary;
    return out;
}

@fragment
fn fs_main(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let n = normalize(in.normal) * select(-1.0, 1.0, front);
    let lighting = 0.35 + 0.65 * abs(dot(n, normalize(vec3<f32>(0.4, 0.8, 0.6))));
    let derivative = max(fwidth(in.barycentric), vec3<f32>(0.00001));
    let edges = smoothstep(vec3<f32>(0.0), derivative * 1.2, in.barycentric);
    let wire = min(edges.x, min(edges.y, edges.z));
    let rgb = in.color.rgb * lighting * mix(1.0, 0.28 + 0.72 * wire, in.color.a);
    return vec4<f32>(rgb, 1.0);
}
