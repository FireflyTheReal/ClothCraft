"""Render recorded dress positions with a CPU z-buffer; never alter physics.

Requires numpy, numba, Pillow and ffmpeg. Uses the same orthographic camera,
topology, panel colours and physical frame index for every backend.
"""
import argparse
import json
import math
import os
from pathlib import Path
import subprocess

os.environ.setdefault('NUMBA_NUM_THREADS', '2')
import numpy as np
from numba import njit
from PIL import Image, ImageDraw, ImageFont

LABELS = {
    'vbd': ('VBD + OGC', 'Existing solver / CPU control'),
    'xpbd': ('XPBD + OGC', 'Original research hybrid'),
    'avbd-finite': ('AVBD finite', 'Original finite-ramp adaptation'),
    'xpbd-primal': ('Coupled primal', 'Full-residual research variant'),
    'avbd-target': ('AVBD target', 'Target stiffness + coupled polish'),
}
PALETTE = np.array([[.18, .69, .75], [.20, .61, .69], [.98, .70, .30],
                    [.96, .57, .29], [.91, .43, .27], [.67, .51, .85]])


@njit
def vertex_normals(x, triangles):
    out = np.zeros_like(x)
    for t in triangles:
        a, b, c = t
        normal = np.cross(x[b] - x[a], x[c] - x[a])
        for k in t:
            out[k] += normal
    for i in range(len(out)):
        length = np.sqrt(np.sum(out[i] ** 2))
        if length > 1e-20:
            out[i] /= length
    return out


@njit
def raster(image, depth, screen, normals, triangles, colours, light):
    h, w = depth.shape
    for ti in range(len(triangles)):
        ia, ib, ic = triangles[ti]
        a, b, c = screen[ia], screen[ib], screen[ic]
        xmin = max(0, int(math.floor(min(a[0], b[0], c[0]))))
        xmax = min(w - 1, int(math.ceil(max(a[0], b[0], c[0]))))
        ymin = max(0, int(math.floor(min(a[1], b[1], c[1]))))
        ymax = min(h - 1, int(math.ceil(max(a[1], b[1], c[1]))))
        den = (b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
        if abs(den) < 1e-12:
            continue
        for y in range(ymin, ymax + 1):
            for x in range(xmin, xmax + 1):
                u = ((b[1]-c[1])*(x+.5-c[0])+(c[0]-b[0])*(y+.5-c[1]))/den
                v = ((c[1]-a[1])*(x+.5-c[0])+(a[0]-c[0])*(y+.5-c[1]))/den
                z = 1-u-v
                if u < 0 or v < 0 or z < 0:
                    continue
                d = u*a[2]+v*b[2]+z*c[2]
                if d <= depth[y,x]:
                    continue
                depth[y,x] = d
                nx = u*normals[ia,0]+v*normals[ib,0]+z*normals[ic,0]
                ny = u*normals[ia,1]+v*normals[ib,1]+z*normals[ic,1]
                nz = u*normals[ia,2]+v*normals[ib,2]+z*normals[ic,2]
                length = math.sqrt(nx*nx+ny*ny+nz*nz)
                if length > 1e-20:
                    nx /= length
                    ny /= length
                    nz /= length
                if nz < 0:
                    nx,ny,nz = -nx,-ny,-nz
                shade = .34 + .60*max(0., nx*light[0]+ny*light[1]+nz*light[2]) + .06*abs(nx)
                for k in range(3):
                    image[y,x,k] = min(255., max(0., 255*colours[ti,k]*shade))


def font(size, bold=False):
    directory = Path(os.environ.get('WINDIR', 'C:/Windows')) / 'Fonts'
    return ImageFont.truetype(str(directory / ('segoeuib.ttf' if bold else 'segoeui.ttf')), size)


def load_run(path):
    geometry = json.loads((path / 'render-geometry.json').read_text())
    initial = json.loads((path / 'initial-state.json').read_text())
    protocol = json.loads((path / 'protocol.json').read_text())
    frames = [json.loads(line)['x'] for line in (path/'positions.jsonl').open()]
    rows = [json.loads(line) for line in (path/'steps.jsonl').open()]
    summary = json.loads((path/'summary.json').read_text()) if (path/'summary.json').exists() else None
    return {'geometry': geometry, 'initial': initial, 'protocol': protocol,
            'frames': np.asarray(frames), 'metrics': rows, 'summary': summary}


class Camera:
    def __init__(self, width, height, geometry, angle=12):
        self.width, self.height = width, height
        yaw = math.radians(angle)
        self.basis = np.array([[math.cos(yaw),0,-math.sin(yaw)], [0,1,0],
                               [math.sin(yaw),0,math.cos(yaw)]])
        self.centre = np.array([.035, .94, .12])
        self.scale = height/1.82
        self.light = np.array([-.45,.65,.85]); self.light /= np.linalg.norm(self.light)
        self.triangles = np.array(geometry['triangles'], dtype=np.int64)
        self.colours = np.zeros((len(self.triangles),3))
        for index, panel in enumerate(geometry['panels']):
            vr = panel['vertices']
            mask = (self.triangles[:,0] >= vr['start']) & (self.triangles[:,0] < vr['end'])
            self.colours[mask] = PALETTE[index % len(PALETTE)]
        gradient = np.linspace(0,1,height)[:,None,None]
        top = np.array([36,44,55.])[None,None,:]
        bottom = np.array([20,26,35.])[None,None,:]
        self.background = np.repeat((top*(1-gradient)+bottom*gradient).astype(np.uint8),width,axis=1)
        self.base_depth = np.full((height,width),-np.inf)
        av = np.array(geometry['avatar_vertices']); at = np.array(geometry['avatar_triangles'],dtype=np.int64)
        self.draw(self.background,self.base_depth,av,at,np.tile([.52,.56,.62],(len(at),1)))

    def project(self,x):
        p = (x-self.centre) @ self.basis.T
        p[:,0] = self.width/2 + self.scale*p[:,0]
        p[:,1] = self.height/2 - self.scale*p[:,1]
        return p

    def draw(self,image,depth,x,triangles,colours):
        normals = vertex_normals(x,triangles) @ self.basis.T
        raster(image,depth,self.project(x),normals,triangles,colours,self.light)

    def render(self,x,grab=None):
        image,depth = self.background.copy(),self.base_depth.copy()
        self.draw(image,depth,x,self.triangles,self.colours)
        result = Image.fromarray(image)
        if grab is not None:
            a,b = self.project(np.array(grab))[:,:2]
            d = ImageDraw.Draw(result)
            d.line([tuple(a),tuple(b)],fill=(255,245,190),width=3)
            d.ellipse((b[0]-6,b[1]-6,b[0]+6,b[1]+6),outline=(255,245,190),width=2)
        return result


def compose(runs,names,cameras,frame,steps,width,height,speed):
    canvas = Image.new('RGB',(width,height),(13,18,25)); d=ImageDraw.Draw(canvas)
    margin,gap=32,16; cw=(width-2*margin-(len(names)-1)*gap)//len(names)
    d.text((margin,24),'CLOTHCRAFT  /  SOLVER STUDY',font=font(40,True),fill=(238,242,247))
    d.text((margin,82),'Same saved dress  |  5,126 vertices  |  4 sweeps per step  |  CPU research comparison',font=font(23),fill=(149,164,181))
    first=runs[names[0]]; protocol=first['protocol']; dt=protocol['settings']['dt']
    t=frame*dt
    phase='SAVED STATE' if frame==0 else 'QUIET'
    step=frame-1
    start=protocol['pull_start']; stop=start+protocol['pull_steps']; release=stop+protocol['hold_steps']
    if frame>0:
        phase='PULL 40 mm' if start<=step<stop else 'HOLD' if stop<=step<release else 'RELEASE' if step>=release else 'QUIET'
    d.text((margin,122),f'{frame:03d} / {steps} steps     {t:.2f} s simulated     {phase}',font=font(24,True),fill=(100,219,225))
    right=f'{speed:g}x playback  /  fixed camera'
    d.text((width-margin-d.textlength(right,font=font(23)),122),right,font=font(23),fill=(177,190,205))
    for i,name in enumerate(names):
        run=runs[name]; x0=margin+i*(cw+gap)
        d.rounded_rectangle((x0,177,x0+cw,1292),radius=14,fill=(23,30,40),outline=(48,61,76),width=1)
        title,subtitle=LABELS[name]
        d.text((x0+18,195),title,font=font(30,True),fill=(238,242,247))
        d.text((x0+18,237),subtitle,font=font(19),fill=(150,169,187))
        available=len(run['frames']); idx=min(frame,available)-1
        x=np.array(run['initial']['x']) if idx<0 else run['frames'][idx]
        grab=None
        if start<=step<release and frame<=available:
            p=run['protocol']; current=x[p['grab_vertices']].mean(0)
            target=np.array(p['grab_anchor'])+np.array([0,0,p['pull_m']*min(1,(step+1-start)/p['pull_steps'])])
            grab=[current,target]
        picture=cameras[name].render(x,grab).resize((cw-2,900),Image.Resampling.LANCZOS)
        canvas.paste(picture,(x0+1,277))
        d=ImageDraw.Draw(canvas)
        if idx>=0:
            metric=run['metrics'][idx]; m=metric['metrics']
            d.text((x0+18,1193),f'Seam gap  {1000*m["max_seam_gap_m"]:.1f} mm',font=font(23,True),fill=(222,229,239))
            d.text((x0+18,1228),f'Stretch P95  {m["extension_p95_percent"]:.1f}%',font=font(21),fill=(168,185,201))
        summary=run['summary']
        if summary and summary.get('mean_ms') is not None:
            d.text((x0+18,1260),f'CPU mean  {summary["mean_ms"]:.0f} ms/step',font=font(19),fill=(142,160,179))
        if frame>available or (summary and summary.get('failure') and frame>=available):
            d.rectangle((x0+4,590,x0+cw-4,735),fill=(103,32,35))
            d.text((x0+18,604),'RUN STOPPED',font=font(29,True),fill='white')
            d.text((x0+18,651),f'Last state: step {available}',font=font(21),fill='white')
            reason='Separation check failed' if summary and 'intersection' in (summary.get('failure') or '').lower() else 'See recorded failure report'
            d.text((x0+18,693),reason,font=font(20),fill=(255,210,210))
    d.text((margin,1321),'Recorded physical states. Playback speed is NOT solver speed. Equal sweeps do not mean equal convergence.',font=font(23),fill=(177,190,205))
    d.text((margin,1362),'Teal: dress panels. Amber: ruffles. Detached panel remains simulated outside framing. No new contact oracle on this dress.',font=font(21),fill=(130,148,166))
    return canvas


def main():
    p=argparse.ArgumentParser(); p.add_argument('--input',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True); p.add_argument('--preview',type=int)
    p.add_argument('--backends',nargs='+',default=list(LABELS)); p.add_argument('--steps',type=int,default=350)
    p.add_argument('--fps',type=int,default=30); p.add_argument('--angle',type=float,default=12)
    args=p.parse_args(); runs={name:load_run(args.input/name) for name in args.backends}
    ref=runs[args.backends[0]]
    for run in runs.values():
        assert run['geometry']==ref['geometry'], 'Geometry mismatch'
        assert run['initial']==ref['initial'], 'Initial state mismatch'
        for key in ('settings','pull_start','pull_steps','hold_steps','pull_m','grab_vertices'):
            assert run['protocol'][key]==ref['protocol'][key], 'Protocol mismatch: '+key
    width,height=2560,1440; cw=(width-64-16*(len(args.backends)-1))//len(args.backends)
    # 1.25x supersampling of geometry, with identical fixed projection in all views.
    camera=Camera(round((cw-2)*1.25),1125,ref['geometry'],args.angle)
    cameras={name:camera for name in runs}
    speed=args.fps*ref['protocol']['settings']['dt']
    def frame(index): return compose(runs,args.backends,cameras,index,args.steps,width,height,speed)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    if args.preview is not None:
        frame(args.preview).save(args.output)
        return
    command=['ffmpeg','-y','-v','error','-f','rawvideo','-pix_fmt','rgb24','-s',f'{width}x{height}',
             '-r',str(args.fps),'-i','-','-an','-c:v','libx264','-threads','2','-preset','fast','-crf','19',
             '-pix_fmt','yuv420p','-movflags','+faststart',str(args.output)]
    proc=subprocess.Popen(command,stdin=subprocess.PIPE)
    try:
        first=frame(0)
        first.save(args.output.with_suffix('.png'))
        for _ in range(args.fps): proc.stdin.write(first.tobytes())
        for index in range(1,args.steps+1):
            img=frame(index);proc.stdin.write(img.tobytes())
            if index%50==0: print(f'Rendered {index}/{args.steps}',flush=True)
        for _ in range(args.fps):proc.stdin.write(img.tobytes())
    finally:
        proc.stdin.close()
    if proc.wait()!=0:raise RuntimeError('ffmpeg failed')
    args.output.with_suffix('.json').write_text(json.dumps({'steps':args.steps,'fps':args.fps,
        'playback_speed':speed,'initial_hold_seconds':1,'final_hold_seconds':1,
        'source':str(args.input.resolve()),'backends':args.backends,'width':width,'height':height,
        'camera_yaw_degrees':args.angle,'renderer':'CPU orthographic z-buffer; smooth normals; 1.25x supersampling'},indent=2))


if __name__=='__main__': main()
