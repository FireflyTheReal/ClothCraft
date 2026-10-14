// Independent VBD implementation. Exact energy/gradient; SPD Gauss-Newton
// local Hessians. OGC candidates and conservative radii are supplied by f64 CPU.
struct Term { ids:vec4u, info:vec4u, a:vec4f, b:vec4f, c:vec4f, d:vec4f }
struct Config { offset:u32, count:u32, moving:u32, pad:u32 }
@group(0) @binding(0) var<storage,read_write> positions:array<vec4f>;
@group(0) @binding(1) var<storage,read> targets:array<vec4f>;
@group(0) @binding(2) var<storage,read> ranges:array<vec2u>;
@group(0) @binding(3) var<storage,read> terms:array<Term>;
@group(0) @binding(4) var<storage,read> incident:array<u32>;
struct Ball { center_radius:vec4f, guess:vec4f }
@group(0) @binding(5) var<storage,read> balls:array<Ball>;
@group(0) @binding(6) var<storage,read> order:array<u32>;
@group(0) @binding(7) var<uniform> config:Config;
@group(0) @binding(8) var<storage,read_write> probes:array<vec4f>;
const I=mat3x3f(vec3f(1,0,0),vec3f(0,1,0),vec3f(0,0,1));
struct Result { e:f32,g:vec3f,h:mat3x3f }
fn outer(a:vec3f,b:vec3f)->mat3x3f{return mat3x3f(a*b.x,a*b.y,a*b.z);}
fn skew(v:vec3f)->mat3x3f{return mat3x3f(vec3f(0,v.z,-v.y),vec3f(-v.z,0,v.x),vec3f(v.y,-v.x,0));}
fn point(id:u32,v:u32,p:vec3f)->vec3f {if id==v{return p;}return positions[id].xyz;}
// Axial value, first and second derivatives with the same smooth strain split.
fn axial(s:f32,k:f32,c:f32,delta:f32)->vec3f {
    let root=sqrt(s*s+delta*delta);let positive=0.5*(s+root-delta);
    let first=0.5*(1+s/root);let second=0.5*delta*delta/(root*root*root);
    return vec3f(0.5*k*(c*s*s+(1-c)*positive*positive),k*(c*s+(1-c)*positive*first),k*(c+(1-c)*(first*first+positive*second)));
}
fn barrier(d:f32,r:f32,k:f32)->vec3f {
    if d>=r{return vec3f(0);}
    if d<=1e-12{return vec3f(1e30,-1e25,1e30);}
    let tau=0.5*r;
    if d>=tau{return vec3f(0.5*k*(r-d)*(r-d),k*(d-r),k);}
    let b=k*tau*tau;return vec3f(0.5*b-b*log(d/tau),-b/d,b/(d*d));
}
fn evaluate(t:Term,v:u32,p:vec3f)->Result {
    var x:array<vec3f,4>;var local=4u;
    for(var j=0u;j<4u;j++){x[j]=point(t.ids[j],v,p);if j<t.info.y && t.ids[j]==v{local=j;}}
    var r=Result(0,vec3f(0),mat3x3f());
    if local==4u{return r;}
    if t.info.x==0u {
        let e1=x[1]-x[0];let e2=x[2]-x[0];
        let w=t.a.y*e1+t.a.z*e2;let f=t.b.y*e1+t.b.z*e2;
        let ew=0.5*(dot(w,w)-1);let ef=0.5*(dot(f,f)-1);let shear=dot(w,f);
        let aw=axial(ew,t.b.w,t.c.z,t.c.w);let af=axial(ef,t.c.x,t.c.z,t.c.w);
        let cw=select(t.a[local],-t.a.y-t.a.z,local==0u);let cf=select(t.b[local],-t.b.y-t.b.z,local==0u);
        let gw=cw*w;let gf=cf*f;let gs=cw*f+cf*w;
        r.e=t.a.w*(aw.x+af.x+0.5*t.c.y*shear*shear);
        r.g=t.a.w*(aw.y*gw+af.y*gf+t.c.y*shear*gs);
        r.h=t.a.w*(max(aw.z,0)*outer(gw,gw)+max(af.z,0)*outer(gf,gf)+t.c.y*outer(gs,gs)
            +I*(max(aw.y,0)*cw*cw+max(af.y,0)*cf*cf+max(2*t.c.y*shear*cw*cf,0)));
    } else if t.info.x==1u {
        let e=x[1]-x[0];let u=x[2]-x[0];let w=x[3]-x[0];
        let na=cross(e,u);let nb=cross(w,e);let la=length(na);let lb=length(nb);
        if min(la,lb)<1e-12 {return Result(1e30,vec3f(0),I);}
        let a=na/la;let b=nb/lb;var da=mat3x3f();var db=mat3x3f();
        if local==0u {da=skew(u-e);db=skew(e-w);}
        if local==1u {da=skew(-u);db=skew(w);}
        if local==2u {da=skew(e);}
        if local==3u {db=skew(-e);}
        let jac=(I-outer(a,a))*da*(1/la)-(I-outer(b,b))*db*(1/lb);let d=a-b;
        r.e=0.5*t.a.x*dot(d,d);r.g=t.a.x*transpose(jac)*d;r.h=t.a.x*transpose(jac)*jac;
    } else if t.info.x==2u {
        // c.w is the exact weight sum: zero for seams, one for a mouse grab.
        var d=t.c.w*x[0]-t.c.xyz;var first_weight=t.c.w;
        for(var j=1u;j<t.info.y;j++){d+=t.a[j]*(x[j]-x[0]);first_weight-=t.a[j];}
        let weight=select(t.a[local],first_weight,local==0u);
        r.e=0.5*t.b.x*dot(d,d);r.g=t.b.x*weight*d;r.h=I*(t.b.x*weight*weight);
    } else {
        var d=0.0;var gd=vec3f(0);var moll=1.0;var gm=vec3f(0);
        if t.info.x==10u {
            let q=x[0]-x[1];d=length(q);if d>=t.a.x{return r;}
            gd=select(-q,q,local==0u)/max(d,1e-20);
        } else if t.info.x==11u {
            let e=x[2]-x[1];let s=dot(x[0]-x[1],e)/max(dot(e,e),1e-20);let q=x[0]-x[1]-s*e;d=length(q);
            if d>=t.a.x{return r;}
            let weights=vec3f(1,s-1,-s);gd=weights[local]*q/max(d,1e-20);
        } else if t.info.x==12u {
            let e=x[2]-x[1];let f=x[3]-x[1];let n=normalize(cross(e,f));let h=dot(x[0]-x[1],n);d=abs(h);
            if d>=t.a.x{return r;}
            let w=x[0]-x[1];let ee=dot(e,e);let ff=dot(f,f);let ef=dot(e,f);let den=max(ee*ff-ef*ef,1e-25);
            let b=(dot(w,e)*ff-dot(w,f)*ef)/den;let c=(dot(w,f)*ee-dot(w,e)*ef)/den;
            let weights=vec4f(1,b+c-1,-b,-c);gd=weights[local]*sign(h)*n;
        } else {
            let e=x[1]-x[0];let f=x[3]-x[2];let n=cross(e,f);let n2=dot(n,n);
            if n2<1e-24{return r;}
            let w=x[0]-x[2];let h=dot(w,n)/sqrt(n2);d=abs(h);
            if d>=t.a.x{return r;}
            let ee=dot(e,e);let ff=dot(f,f);let ef=dot(e,f);let den=max(ee*ff-ef*ef,1e-25);
            let s=(ef*dot(f,w)-ff*dot(e,w))/den;let z=(ee*dot(f,w)-ef*dot(e,w))/den;
            let weights=vec4f(1-s,s,z-1,-z);gd=weights[local]*sign(h)*n/sqrt(n2);
            let q=n2/t.a.z;
            if q<1 {moll=q*(2-q);var derivative=vec3f(0);
                if local==0u {derivative=-2*cross(f,n);}if local==1u{derivative=2*cross(f,n);}
                if local==2u {derivative=-2*cross(n,e);}if local==3u{derivative=2*cross(n,e);}
                gm=2*(1-q)*derivative/t.a.z;
            }
        }
        let b=barrier(d,t.a.x,t.a.y);r.e=moll*b.x;r.g=moll*b.y*gd+b.x*gm;
        r.h=moll*b.z*outer(gd,gd); // SPD local approximation, exact gradient above.
    }
    return r;
}
fn objective(v:u32,p:vec3f)->f32 {
    let q=p-targets[v].xyz;var e=0.5*targets[v].w*dot(q,q);
    for(var i=ranges[v].x;i<ranges[v].y;i++){e+=evaluate(terms[incident[i]],v,p).e;}return e;
}
// Adaptive initialization from the previous accepted acceleration. The inertia
// target still contains full gravity. Keep the ORIGINAL OGC balls and fixed pins.
@compute @workgroup_size(64)
fn predict(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;
    if v>=config.moving {return;}
    if positions[v].w>0.5 {return;}
    let ball=balls[v].center_radius;
    let d=balls[v].guess.xyz-ball.xyz;
    let scale=min(1.0,ball.w/max(length(d),1e-20));
    positions[v]=vec4f(ball.xyz+scale*d,positions[v].w);
}

// One 32-lane workgroup per vertex, rather than one serial invocation.
// Coloring still prevents all inter-group energy dependencies. Reductions are
// explicitly synchronized, portable to devices without subgroup extensions.
var<workgroup> energies:array<f32,32>;
var<workgroup> gradients:array<vec3f,32>;
var<workgroup> hessians:array<mat3x3f,32>;
var<workgroup> direction:vec3f;
var<workgroup> candidate:vec3f;
var<workgroup> initial_energy:f32;
var<workgroup> finished:u32;
fn assemble(v:u32,lane:u32) {
    let p=positions[v].xyz;
    var e=0.0;var g=vec3f(0);var h=mat3x3f();
    for(var i=ranges[v].x+lane;i<ranges[v].y;i+=32u) {
        let a=evaluate(terms[incident[i]],v,p);e+=a.e;g+=a.g;h+=a.h;
    }
    if lane==0u {let q=p-targets[v].xyz;e+=0.5*targets[v].w*dot(q,q);g+=q*targets[v].w;h+=I*targets[v].w;}
    energies[lane]=e;gradients[lane]=g;hessians[lane]=h;
    workgroupBarrier();
    for(var stride=16u;stride>0u;stride/=2u) {
        if lane<stride {energies[lane]+=energies[lane+stride];gradients[lane]+=gradients[lane+stride];hessians[lane]+=hessians[lane+stride];}
        workgroupBarrier();
    }
}
@compute @workgroup_size(32)
fn probe(@builtin(workgroup_id) group:vec3u,@builtin(local_invocation_index) lane:u32){
    let v=group.x;if v>=config.moving{return;}
    assemble(v,lane);
    if lane==0u {probes[v]=vec4f(gradients[0],energies[0]);}
}
@compute @workgroup_size(32)
fn iterate(@builtin(workgroup_id) group:vec3u,@builtin(local_invocation_index) lane:u32){
    if group.x>=config.count{return;}let v=order[config.offset+group.x];let old=positions[v].xyz;
    assemble(v,lane);
    if lane==0u {
        let h=hessians[0];let g=gradients[0];
        let a=cross(h[1],h[2]);let b=cross(h[2],h[0]);let c=cross(h[0],h[1]);let det=dot(h[0],a);
        finished=select(0u,1u,det<=1e-20 || positions[v].w>0.5);
        direction=-vec3f(dot(a,g),dot(b,g),dot(c,g))/max(det,1e-20);
        initial_energy=energies[0];
    }
    if workgroupUniformLoad(&finished)!=0u {return;}
    var alpha=1.0;
    for(var attempt=0u;attempt<8u;attempt++) {
        if lane==0u {
            let ball=balls[v].center_radius;
            candidate=old+alpha*direction;let d=candidate-ball.xyz;let len=length(d);
            if len>ball.w {candidate=ball.xyz+d*(ball.w/max(len,1e-20));}
        }
        workgroupBarrier();
        var e=0.0;
        for(var i=ranges[v].x+lane;i<ranges[v].y;i+=32u){e+=evaluate(terms[incident[i]],v,candidate).e;}
        if lane==0u {let q=candidate-targets[v].xyz;e+=0.5*targets[v].w*dot(q,q);}
        energies[lane]=e;workgroupBarrier();
        for(var stride=16u;stride>0u;stride/=2u){
            if lane<stride {energies[lane]+=energies[lane+stride];}
            workgroupBarrier();
        }
        if lane==0u && energies[0]<=initial_energy+1e-9 && energies[0]<1e25 {
            positions[v]=vec4f(candidate,positions[v].w);finished=1u;
        }
        if workgroupUniformLoad(&finished)!=0u {break;}
        alpha*=0.5;
    }
}
