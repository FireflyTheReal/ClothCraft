struct Box { lo: vec3<f32>, hi: vec3<f32> }
fn bounds(id: u32) -> Box {
    let p = primitives[id];
    let a = positions[p.x].xyz;
    let b = positions[p.y].xyz;
    let c = positions[p.z].xyz;
    return Box(min(a, min(b, c)), max(a, max(b, c)));
}
// The approximate closest point chooses an axis only. Rejection uses the
// support intervals of ALL primitive vertices on that axis, with an error band;
// even a poor/degenerate closest-point estimate cannot omit a nearby pair.
fn triangle_axis(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, c: vec3<f32>) -> vec3<f32> {
    let ab = b - a; let ac = c - a; let ap = p - a;
    let d1 = dot(ab, ap); let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 { return p - a; }
    let bp = p - b; let d3 = dot(ab, bp); let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 { return p - b; }
    let vc = d1*d4 - d3*d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 { if d1-d3 > 1e-30 { return p - (a + ab * (d1 / (d1-d3))); } return ap; }
    let cp = p - c; let d5 = dot(ab, cp); let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 { return p - c; }
    let vb = d5*d2 - d1*d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 { if d2-d6 > 1e-30 { return p - (a + ac * (d2 / (d2-d6))); } return ap; }
    let va = d3*d6 - d5*d4;
    if va <= 0.0 && d4-d3 >= 0.0 && d5-d6 >= 0.0 { let den = (d4-d3)+(d5-d6); if den > 1e-30 { return p - (b + (c-b) * ((d4-d3) / den)); } return ap; }
    let den = va+vb+vc; if den <= 1e-30 { return ap; }
    return p - (a + ab*clamp(vb/den,0.0,1.0) + ac*clamp(vc/den,0.0,1.0));
}
fn segment_axis(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let u = b-a; let v = d-c; let w = a-c;
    let aa = dot(u,u); let ee = dot(v,v);
    if aa < 1e-24 || ee < 1e-24 { return w; }
    let bb = dot(u,v); let cc = dot(u,w); let ff = dot(v,w);
    let den = aa*ee - bb*bb;
    var s = 0.0;
    if den > 1e-6*aa*ee { s = clamp((bb*ff-cc*ee)/den, 0.0, 1.0); }
    var t = (bb*s+ff)/ee;
    if t < 0.0 { s = clamp(-cc/aa, 0.0, 1.0); t = 0.0; }
    else if t > 1.0 { s = clamp((bb-cc)/aa, 0.0, 1.0); t = 1.0; }
    return (a+u*s) - (c+v*t);
}
fn separated(a: vec4<u32>, b: vec4<u32>, axis: vec3<f32>) -> bool {
    let n2 = dot(axis,axis);
    // Includes zero/overflow/NaN estimates: uncertain pairs survive.
    if !(n2 > 1e-24 && n2 < 1e30) { return false; }
    let pa = vec3<f32>(dot(positions[a.x].xyz,axis), dot(positions[a.y].xyz,axis), dot(positions[a.z].xyz,axis));
    let pb = vec3<f32>(dot(positions[b.x].xyz,axis), dot(positions[b.y].xyz,axis), dot(positions[b.z].xyz,axis));
    let gap = max(min(pa.x,min(pa.y,pa.z))-max(pb.x,max(pb.y,pb.z)), min(pb.x,min(pb.y,pb.z))-max(pa.x,max(pa.y,pa.z)));
    let error = 4.0 * config.padding * (abs(axis.x)+abs(axis.y)+abs(axis.z));
    return gap > config.radius * sqrt(n2) + error;
}
fn excluded(a: u32, b: u32) -> bool {
    if a == b { return true; }
    var lo = exclusions[a]; var hi = exclusions[a+1u];
    loop {
        if lo >= hi { return false; }
        let mid = lo + (hi-lo)/2u;
        let v = exclusions[mid];
        if v == b { return true; }
        if v < b { lo=mid+1u; } else { hi=mid; }
    }
    return false;
}
fn adjacent(a: vec4<u32>, b: vec4<u32>) -> bool {
    for (var i=0u; i<a.w; i++) {
        for (var j=0u; j<b.w; j++) {
            if excluded(a[i],b[j]) { return true; }
        }
    }
    return false;
}
