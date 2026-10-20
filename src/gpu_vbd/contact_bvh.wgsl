struct Config {
    queries: u32, width: u32, radius: f32, padding: f32,
    vertices: u32, triangles: u32, edges: u32, moving: u32,
    roots: vec4<u32>, capacity: u32, pad0: u32, pad1: u32, pad2: u32,
}
struct Node { lo: vec4<f32>, hi: vec4<f32>, info: vec4<u32> }
// The high bit of the second ID distinguishes EE from VF. The host rejects
// topologies exceeding that range, and never consumes a truncated output.
struct Output { count: atomic<u32>, overflow: atomic<u32>, pad0: u32, pad1: u32, pairs: array<vec2<u32>> }
@group(0) @binding(0) var<storage, read> positions: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> primitives: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read> order: array<u32>;
@group(0) @binding(3) var<storage, read_write> nodes: array<Node>;
@group(0) @binding(4) var<uniform> config: Config;
@group(0) @binding(5) var<storage, read_write> output: Output;
@group(0) @binding(6) var<storage, read> exclusions: array<u32>;

fn outside(a: Box, b: Box) -> bool {
    let gap = max(max(a.lo-b.hi,b.lo-a.hi)-vec3<f32>(config.padding),vec3<f32>(0.0));
    return dot(gap,gap) > config.radius*config.radius;
}
var<workgroup> lower: array<vec3<f32>,64>;
var<workgroup> upper: array<vec3<f32>,64>;
// Each node independently unions its primitive interval. This O(n log n)
// refit avoids cross-workgroup dependencies and level-by-level dispatches.
@compute @workgroup_size(64)
fn refit_nodes(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let index=group.x+group.y*config.width;
    if index >= config.pad0+config.pad2 { return; }
    var node_index=index;
    if index >= config.pad0 { node_index=config.pad1+index-config.pad0; }
    let node=nodes[node_index];
    var lo=vec3<f32>(1e30); var hi=vec3<f32>(-1e30);
    for (var i=lane; i<node.info.y; i+=64u) {
        let b=bounds(order[node.info.x+i]);
        lo=min(lo,b.lo); hi=max(hi,b.hi);
    }
    lower[lane]=lo; upper[lane]=hi;
    workgroupBarrier();
    for (var stride=32u; stride>0u; stride/=2u) {
        if lane<stride {
            lower[lane]=min(lower[lane],lower[lane+stride]);
            upper[lane]=max(upper[lane],upper[lane+stride]);
        }
        workgroupBarrier();
    }
    if lane==0u { nodes[node_index].lo=vec4<f32>(lower[0],0.0); nodes[node_index].hi=vec4<f32>(upper[0],0.0); }
}
@compute @workgroup_size(64)
fn query_bvh(@builtin(global_invocation_id) id: vec3<u32>) {
    let query = id.x + id.y * config.width * 64u;
    if query >= config.queries { return; }
    let is_edge = query >= config.vertices;
    var ai = query;
    var roots = config.roots.xy;
    if is_edge { ai = query + config.triangles; roots = config.roots.zw; }
    let av = primitives[ai];
    let moving = any(av.xyz < vec3<u32>(config.moving));
    let a = bounds(ai);
    for (var root_index=0u; root_index<2u; root_index++) {
        let root=roots[root_index];
        if root==0xffffffffu { continue; }
        let end=nodes[root].info.z;
        var node_index=root;
        loop {
            if node_index>=end { break; }
            let node=nodes[node_index];
            if (!moving && (node.info.w & 2u)==0u) || outside(a,Box(node.lo.xyz,node.hi.xyz)) {
                node_index=node.info.z; continue;
            }
            if (node.info.w & 1u)==0u { node_index++; continue; }
            node_index=node.info.z;
        for (var j=0u; j<node.info.y; j++) {
            let bi = order[node.info.x+j];
            if is_edge && ai >= bi { continue; }
            let bv = primitives[bi];
            if outside(a,bounds(bi)) || adjacent(av,bv) { continue; }
            var axis: vec3<f32>;
            if is_edge { axis=segment_axis(positions[av.x].xyz,positions[av.y].xyz,positions[bv.x].xyz,positions[bv.y].xyz); }
            else { axis=triangle_axis(positions[av.x].xyz,positions[bv.x].xyz,positions[bv.y].xyz,positions[bv.z].xyz); }
            if separated(av,bv,axis) { continue; }
            let slot=atomicAdd(&output.count,1u);
            if slot < config.capacity {
                if is_edge { output.pairs[slot]=vec2<u32>(ai-config.vertices-config.triangles,(bi-config.vertices-config.triangles)|0x80000000u); }
                else { output.pairs[slot]=vec2<u32>(ai,bi-config.vertices); }
            } else { atomicOr(&output.overflow,2u); }
        }
    }
    }
}
