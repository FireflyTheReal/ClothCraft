// Only enabled when the adapter guarantees subgroup min=max=32.
// Shuffle the same partners in the same order as the portable reduction.
// The entry points use subgroup_invocation_id for lane; no assumption about
// its correspondence to local_invocation_index is needed.
// Shared-memory publication and all cross-invocation control still synchronize.
fn reduce_system(e0:f32,g0:vec3f,h0:mat3x3f,lane:u32) {
    var e=e0;var g=g0;var h=h0;
    for(var stride=16u;stride>0u;stride/=2u) {
        let de=subgroupShuffleXor(e,stride);let dg=subgroupShuffleXor(g,stride);
        let da=subgroupShuffleXor(h[0],stride);let db=subgroupShuffleXor(h[1],stride);let dc=subgroupShuffleXor(h[2],stride);
        if lane<stride {e+=de;g+=dg;h+=mat3x3f(da,db,dc);}
    }
    if lane==0u {energies[0]=e;gradients[0]=g;hessians[0]=h;}
    workgroupBarrier();
}
fn reduce_energy(e0:f32,lane:u32) {
    var e=e0;
    for(var stride=16u;stride>0u;stride/=2u) {
        let de=subgroupShuffleXor(e,stride);
        if lane<stride {e+=de;}
    }
    if lane==0u {energies[0]=e;}
    workgroupBarrier();
}
