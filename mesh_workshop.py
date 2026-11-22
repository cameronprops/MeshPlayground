#!/usr/bin/env python3
"""Standalone offline triangle mesh repair. Python standard library only."""
import math, struct, sys, copy
from collections import defaultdict, Counter
from pathlib import Path
import tkinter as tk
from tkinter import filedialog, messagebox, simpledialog, ttk

def sub(a,b):return tuple(x-y for x,y in zip(a,b))
def add(a,b):return tuple(x+y for x,y in zip(a,b))
def mul(a,s):return tuple(x*s for x in a)
def dot(a,b):return sum(x*y for x,y in zip(a,b))
def cross(a,b):return (a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def norm(a):return math.sqrt(dot(a,a))
def unit(a):
 n=norm(a);return mul(a,1/n) if n>1e-20 else (0.,0.,0.)

class Mesh:
 def __init__(self,vertices=None,faces=None):self.v=vertices or [];self.f=faces or []
 def clone(self):return Mesh(self.v[:],self.f[:])
 def validate(self):
  if not self.v or not self.f:raise ValueError('No triangle geometry found.')
  if any(not all(math.isfinite(x) for x in p) for p in self.v):raise ValueError('Non-finite vertex coordinate.')
  if any(len(f)!=3 or min(f)<0 or max(f)>=len(self.v) for f in self.f):raise ValueError('Invalid face index.')
 def edge_map(self):
  edges=defaultdict(list)
  for i,(a,b,c) in enumerate(self.f):
   for u,v in ((a,b),(b,c),(c,a)):edges[tuple(sorted((u,v)))].append((i,u,v))
  return edges
 def diagnostics(self):
  e=self.edge_map();return {'vertices':len(self.v),'triangles':len(self.f),'boundary_edges':sum(len(x)==1 for x in e.values()),'nonmanifold_edges':sum(len(x)>2 for x in e.values()),'degenerate':sum(len(set(f))<3 or norm(cross(sub(self.v[f[1]],self.v[f[0]]),sub(self.v[f[2]],self.v[f[0]])))<1e-15 for f in self.f)}
 def clean(self,selected=None):
  # Exact-coordinate weld, no rounding or positional changes. Delete duplicate and zero-area triangles.
  # Selected mode only removes affected triangles; vertex weld is global but only exact-coincident.
  old=len(self.f); lookup={}; mapping=[];verts=[]
  for p in self.v:
   if p not in lookup:lookup[p]=len(verts);verts.append(p)
   mapping.append(lookup[p])
  faces=[tuple(mapping[x] for x in f) for f in self.f];seen=set();new=[];removed=0
  for i,f in enumerate(faces):
   bad=len(set(f))<3 or norm(cross(sub(verts[f[1]],verts[f[0]]),sub(verts[f[2]],verts[f[0]])))<1e-15
   key=tuple(sorted(f));dup=key in seen
   if (bad or dup) and (selected is None or i in selected):removed+=1;continue
   seen.add(key);new.append(f)
  welded=len(self.v)-len(verts);self.v=verts;self.f=new;return {'removed_faces':removed,'welded_exact_vertices':welded}
 def boundary_loops(self):
  e=self.edge_map();boundary=[(u,v,refs[0][0]) for (u,v),refs in e.items() if len(refs)==1];adj=defaultdict(list)
  for u,v,fi in boundary:adj[u].append(v);adj[v].append(u)
  visited=set();loops=[];skipped=0
  for a,b,fi in boundary:
   edge=tuple(sorted((a,b)))
   if edge in visited:continue
   stack=[a];component=set();verts=set()
   while stack:
    x=stack.pop()
    if x in verts:continue
    verts.add(x)
    for y in adj[x]:component.add(tuple(sorted((x,y))));stack.append(y)
   visited.update(component)
   if len(verts)<3 or any(len(adj[x])!=2 for x in verts):skipped+=1;continue
   loop=[a];loop_seen={a};prev=None;cur=a
   while True:
    nxt=next(x for x in adj[cur] if x!=prev)
    if nxt==a:break
    if nxt in loop_seen:loop=[];break
    loop.append(nxt);loop_seen.add(nxt);prev,cur=cur,nxt
   if loop and len(loop)==len(verts):loops.append(loop)
   else:skipped+=1
  return loops,skipped
 def patch(self,selected=None,max_edges=100):
  em=self.edge_map();loops,skipped=self.boundary_loops();patched=0;newfaces=0;reasons=Counter()
  if skipped:reasons['branched boundary']+=skipped
  for loop in loops:
   if len(loop)>max_edges:skipped+=1;reasons['edge limit']+=1;continue
   refs=[em[tuple(sorted((loop[i],loop[(i+1)%len(loop)])))][0] for i in range(len(loop))]
   # All faces adjacent to the loop must be painted in selected mode.
   if selected is not None and not all(ref[0] in selected for ref in refs):continue
   # A boundary surrounding one existing triangle is an isolated face, not a
   # hole to cap. Filling it duplicates the face and hides an unresolved defect.
   if len(set(ref[0] for ref in refs))==1:
    skipped+=1;reasons['isolated face boundary']+=1;continue
   directions=[(u,v)==(loop[i],loop[(i+1)%len(loop)]) for i,(_,u,v) in enumerate(refs)]
   if not (all(directions) or not any(directions)):
    skipped+=1;reasons['inconsistent boundary winding']+=1;continue
   try: tris=triangulate(loop,self.v)
   except ValueError as e:skipped+=1;reasons[str(e)]+=1;continue
   # Make the patch share its first edge in opposite direction to the existing face.
   _,u,v=refs[0]
   if (u,v)==(loop[0],loop[1]):tris=[(a,c,b) for a,b,c in tris]
   self.f.extend(tris);patched+=1;newfaces+=len(tris)
  return {'patched_loops':patched,'added_triangles':newfaces,'skipped':skipped,'reasons':dict(reasons)}
 def shell(self,thickness):
  if not math.isfinite(thickness) or thickness<=0:raise ValueError('Thickness must be greater than zero.')
  self.validate();em=self.edge_map()
  if any(len(r)>2 for r in em.values()):raise ValueError('Shelling requires a mesh without non-manifold edges.')
  if any(len(r)==2 and r[0][1:]==r[1][1:] for r in em.values()):raise ValueError('Inconsistent face winding: orient the mesh before shelling.')
  normals=[(0.,0.,0.) for _ in self.v]
  for a,b,c in self.f:
   n=cross(sub(self.v[b],self.v[a]),sub(self.v[c],self.v[a]))
   for i in (a,b,c):normals[i]=add(normals[i],n)
  if any(norm(n)<1e-15 for n in normals):raise ValueError('Some vertices have no reliable normal. Clean the mesh first.')
  count=len(self.v);verts=self.v[:]+[sub(p,mul(unit(n),thickness)) for p,n in zip(self.v,normals)]
  faces=self.f[:]+[(a+count,c+count,b+count) for a,b,c in self.f]
  for refs in em.values():
   if len(refs)==1:
    _,a,b=refs[0];faces.extend([(b,a,a+count),(b,a+count,b+count)])
  return Mesh(verts,faces)

def triangulate(loop,vertices):
 # Project to dominant plane using Newell normal, then ear-clip concave boundaries.
 n=[0.,0.,0.]
 for i,a in enumerate(loop):
  p=vertices[a];q=vertices[loop[(i+1)%len(loop)]]
  n[0]+=(p[1]-q[1])*(p[2]+q[2]);n[1]+=(p[2]-q[2])*(p[0]+q[0]);n[2]+=(p[0]-q[0])*(p[1]+q[1])
 magnitude=norm(n)
 if magnitude<1e-15:raise ValueError('degenerate boundary')
 normal=mul(n,1/magnitude);origin=vertices[loop[0]]
 extent=max(norm(sub(vertices[i],origin)) for i in loop)
 if max(abs(dot(sub(vertices[i],origin),normal)) for i in loop)>max(1e-9,extent*0.01):
  raise ValueError('nonplanar boundary')
 axis=max(range(3),key=lambda x:abs(n[x]));axes=[j for j in range(3) if j!=axis];pts={i:(vertices[i][axes[0]],vertices[i][axes[1]]) for i in loop}
 def orient(a,b,c):p,q,r=pts[a],pts[b],pts[c];return (q[0]-p[0])*(r[1]-p[1])-(q[1]-p[1])*(r[0]-p[0])
 # Non-adjacent projected edges must not cross or touch; reject ambiguous loops.
 eps=max(extent*extent*1e-12,1e-15)
 def on_segment(a,b,c):
  p,q,r=pts[a],pts[b],pts[c]
  return abs(orient(a,b,c))<=eps and min(p[0],q[0])-eps<=r[0]<=max(p[0],q[0])+eps and min(p[1],q[1])-eps<=r[1]<=max(p[1],q[1])+eps
 for i,a in enumerate(loop):
  b=loop[(i+1)%len(loop)]
  for j in range(i+1,len(loop)):
   c,d=loop[j],loop[(j+1)%len(loop)]
   if len({a,b,c,d})<4:continue
   o1,o2,o3,o4=orient(a,b,c),orient(a,b,d),orient(c,d,a),orient(c,d,b)
   if ((o1>eps and o2< -eps or o1< -eps and o2>eps) and (o3>eps and o4< -eps or o3< -eps and o4>eps)) or on_segment(a,b,c) or on_segment(a,b,d) or on_segment(c,d,a) or on_segment(c,d,b):
    raise ValueError('self-intersecting boundary')
 area=sum(pts[loop[i]][0]*pts[loop[(i+1)%len(loop)]][1]-pts[loop[(i+1)%len(loop)]][0]*pts[loop[i]][1] for i in range(len(loop)))
 if abs(area)<1e-15:raise ValueError('Degenerate boundary')
 sign=1 if area>0 else -1;remain=loop[:];out=[]
 while len(remain)>3:
  found=False
  for j,b in enumerate(remain):
   a=remain[j-1];c=remain[(j+1)%len(remain)]
   if sign*orient(a,b,c)<=1e-14:continue
   if any(sign*orient(a,b,p)>=-1e-14 and sign*orient(b,c,p)>=-1e-14 and sign*orient(c,a,p)>=-1e-14 for p in remain if p not in (a,b,c)):continue
   out.append((a,b,c));remain.pop(j);found=True;break
  if not found:raise ValueError('Could not triangulate')
 out.append(tuple(remain));return out

def read_mesh(path):
 data=Path(path).read_bytes();v=[];f=[];ext=Path(path).suffix.lower()
 if ext=='.obj':
  for line in data.decode('utf8',errors='replace').splitlines():
   p=line.split()
   if not p:continue
   if p[0]=='v' and len(p)>=4:v.append(tuple(map(float,p[1:4])))
   if p[0]=='f' and len(p)>=4:
    ix=[]
    for token in p[1:]:
     n=int(token.split('/')[0]);ix.append(n-1 if n>0 else len(v)+n)
    if len(ix)!=3:raise ValueError('OBJ must contain triangles only. Triangulate polygon faces before import.')
    f.append(tuple(ix))
 elif ext=='.stl':
  if len(data)>=84 and 84+50*struct.unpack_from('<I',data,80)[0]==len(data):
   for k in range(struct.unpack_from('<I',data,80)[0]):
    rec=struct.unpack_from('<12fH',data,84+50*k);start=len(v);v.extend([tuple(rec[3:6]),tuple(rec[6:9]),tuple(rec[9:12])]);f.append((start,start+1,start+2))
  else:
   points=[]
   for line in data.decode('ascii',errors='strict').splitlines():
    p=line.strip().split()
    if p and p[0].lower()=='vertex' and len(p)==4:points.append(tuple(map(float,p[1:])))
   if len(points)%3:raise ValueError('Malformed STL')
   v=points;f=[(i,i+1,i+2) for i in range(0,len(v),3)]
 else:raise ValueError('Supported files: STL and triangle OBJ.')
 m=Mesh(v,f);m.validate();m.clean();return m

def write_stl(path,m):
 m.validate()
 with open(path,'wb') as out:
  out.write(b'Mesh Workshop - STL'.ljust(80,b' '));out.write(struct.pack('<I',len(m.f)))
  for a,b,c in m.f:
   p,q,r=m.v[a],m.v[b],m.v[c];n=unit(cross(sub(q,p),sub(r,p)));out.write(struct.pack('<12fH',*(n+p+q+r),0))

class App:
 def __init__(self,root):
  self.root=root;root.title('Mesh Workshop');root.geometry('1120x760');self.mesh=None;self.backup=None;self.selected=set();self.mode=tk.StringVar(value='rotate');self.scope=tk.StringVar(value='all');self.az=.55;self.el=.35;self.drag=None;self.projected=[];self.zoom=1.;self.path=''
  bar=ttk.Frame(root,padding=8);bar.pack(fill='x');
  for text,fn in [('Open STL / OBJ',self.open),('Save STL',self.save),('Undo operation',self.undo)]:ttk.Button(bar,text=text,command=fn).pack(side='left',padx=3)
  ttk.Radiobutton(bar,text='Rotate',variable=self.mode,value='rotate').pack(side='left',padx=(20,2));ttk.Radiobutton(bar,text='Paint faces',variable=self.mode,value='paint').pack(side='left',padx=2);ttk.Button(bar,text='Clear paint',command=self.clear).pack(side='left',padx=5)
  body=ttk.Frame(root,padding=8);body.pack(fill='both',expand=True);panel=ttk.Frame(body,width=290,padding=(12,0));panel.pack(side='right',fill='y');self.canvas=tk.Canvas(body,background='#17232c',highlightthickness=0);self.canvas.pack(side='left',fill='both',expand=True)
  ttk.Label(panel,text='REPAIR SCOPE',font=('Arial',10,'bold')).pack(anchor='w');ttk.Radiobutton(panel,text='Whole mesh',variable=self.scope,value='all').pack(anchor='w');ttk.Radiobutton(panel,text='Painted area only',variable=self.scope,value='selected').pack(anchor='w');ttk.Label(panel,text='Paint every face bordering a hole\nto patch it in painted-area mode.',justify='left').pack(anchor='w',pady=8)
  ttk.Label(panel,text='Maximum hole edges').pack(anchor='w');self.limit=tk.StringVar(value='100');ttk.Entry(panel,textvariable=self.limit,width=12).pack(anchor='w');ttk.Button(panel,text='Clean and patch',command=self.repair).pack(fill='x',pady=8)
  ttk.Separator(panel).pack(fill='x',pady=12);ttk.Label(panel,text='SHELLING',font=('Arial',10,'bold')).pack(anchor='w');ttk.Label(panel,text='Thickness (mesh units)').pack(anchor='w');self.thick=tk.StringVar(value='2');ttk.Entry(panel,textvariable=self.thick,width=12).pack(anchor='w');ttk.Button(panel,text='Create inward shell',command=self.shell).pack(fill='x',pady=8)
  self.info=tk.StringVar(value='Open an STL or triangle OBJ to begin.');ttk.Label(panel,textvariable=self.info,wraplength=260,justify='left').pack(anchor='w',pady=10);self.status=tk.StringVar(value='Offline · Original file is never overwritten automatically');ttk.Label(root,textvariable=self.status,padding=8).pack(fill='x')
  self.canvas.bind('<Configure>',lambda e:self.draw());self.canvas.bind('<Button-1>',self.press);self.canvas.bind('<B1-Motion>',self.move);self.canvas.bind('<MouseWheel>',self.wheel);self.canvas.bind('<Button-4>',lambda e:self.scale(1.1));self.canvas.bind('<Button-5>',lambda e:self.scale(.9))
 def open(self):
  p=filedialog.askopenfilename(filetypes=[('Mesh','*.stl *.obj'),('All files','*')]);
  if not p:return
  try:self.mesh=read_mesh(p);self.path=p;self.backup=None;self.selected=set();self.zoom=1.;self.status.set('Loaded '+Path(p).name+'; exact duplicate vertices welded on import.');self.draw()
  except Exception as e:messagebox.showerror('Could not open mesh',str(e))
 def save(self):
  if not self.mesh:return
  p=filedialog.asksaveasfilename(defaultextension='.stl',filetypes=[('Binary STL','*.stl')],initialfile=Path(self.path).stem+'_repaired.stl')
  if not p:return
  try:write_stl(p,self.mesh);self.status.set('Saved '+p+'. STL carries no unit metadata.')
  except Exception as e:messagebox.showerror('Could not save',str(e))
 def undo(self):
  if self.backup:self.mesh,self.backup=self.backup,None;self.selected.clear();self.status.set('Last operation undone.');self.draw()
 def clear(self):self.selected.clear();self.draw()
 def repair(self):
  if not self.mesh:return
  try:
   limit=int(self.limit.get())
   if not 3<=limit<=5000:raise ValueError('Maximum hole edges must be between 3 and 5000.')
   selection=None if self.scope.get()=='all' else set(self.selected)
   if selection is not None and not selection:raise ValueError('Paint some faces first.')
   temp=self.mesh.clone();patch=temp.patch(selection,limit);clean=temp.clean(selection);self.backup=self.mesh;self.mesh=temp;self.selected.clear();self.status.set(f"Patched {patch['patched_loops']} loops, added {patch['added_triangles']} triangles; removed {clean['removed_faces']} degenerate or duplicate faces. Skipped {patch['skipped']} unsuitable loops.");self.draw()
   if patch['reasons']:
    messagebox.showinfo('Repair report','Some boundaries were left untouched:\n\n'+'\n'.join(f'{reason}: {count}' for reason,count in patch['reasons'].items()))
  except Exception as e:messagebox.showerror('Repair failed',str(e))
 def shell(self):
  if not self.mesh:return
  try:
   t=float(self.thick.get());new=self.mesh.shell(t);self.backup=self.mesh;self.mesh=new;self.selected.clear();self.status.set('Shell created. Inspect for self-intersections and thickness: neither is verified.');self.draw()
  except Exception as e:messagebox.showerror('Shell failed',str(e))
 def scale(self,s):self.zoom=max(.1,min(10,self.zoom*s));self.draw()
 def wheel(self,e):self.scale(1.1 if e.delta>0 else .9)
 def press(self,e):
  self.drag=(e.x,e.y)
  if self.mode.get()=='paint':self.paint(e.x,e.y)
 def move(self,e):
  if self.mode.get()=='rotate' and self.drag:self.az+=(e.x-self.drag[0])*.01;self.el+=(e.y-self.drag[1])*.01;self.el=max(-1.55,min(1.55,self.el));self.draw()
  else:self.paint(e.x,e.y)
  self.drag=(e.x,e.y)
 def paint(self,x,y):
  # Hit the closest projected face beneath the brush point, not hidden faces.
  for z,fi,poly in reversed(self.projected):
   a,b,c=poly
   def ori(p,q,r):return (q[0]-p[0])*(r[1]-p[1])-(q[1]-p[1])*(r[0]-p[0])
   vals=[ori(a,b,(x,y)),ori(b,c,(x,y)),ori(c,a,(x,y))]
   if all(v>=0 for v in vals) or all(v<=0 for v in vals):self.selected.add(fi);self.draw();return
 def draw(self):
  self.canvas.delete('all');self.projected=[]
  if not self.mesh:self.canvas.create_text(20,20,anchor='nw',fill='#d4e4e9',text='Open an STL or triangle OBJ to begin.',font=('Arial',15));return
  m=self.mesh;w=max(self.canvas.winfo_width(),1);h=max(self.canvas.winfo_height(),1);lo=[min(p[i] for p in m.v) for i in range(3)];hi=[max(p[i] for p in m.v) for i in range(3)];center=[(lo[i]+hi[i])/2 for i in range(3)];span=max(hi[i]-lo[i] for i in range(3)) or 1;scale=min(w,h)*.72/span*self.zoom;ca,sa=math.cos(self.az),math.sin(self.az);ce,se=math.cos(self.el),math.sin(self.el);proj=[]
  for p in m.v:
   x,y,z=(p[i]-center[i] for i in range(3));xx=ca*x-sa*y;yy=sa*x+ca*y;zz=ce*z-se*yy;depth=se*z+ce*yy;proj.append((w/2+xx*scale,h/2-zz*scale,depth))
  # Canvas rendering is intended for modest meshes; large models need a GPU viewport.
  for fi,f in enumerate(m.f):
   pts=[proj[i] for i in f];self.projected.append((sum(p[2] for p in pts)/3,fi,[(p[0],p[1]) for p in pts]))
  self.projected.sort(key=lambda x:x[0])
  for z,fi,poly in self.projected:self.canvas.create_polygon(*[coord for p in poly for coord in p],fill='#efa34e' if fi in self.selected else '#5a9b9b',outline='#253e48',width=1)
  d=m.diagnostics();self.info.set(f"Vertices: {d['vertices']:,}\nTriangles: {d['triangles']:,}\nOpen edges: {d['boundary_edges']:,}\nNon-manifold edges: {d['nonmanifold_edges']:,}\nDegenerate triangles: {d['degenerate']:,}\nPainted faces: {len(self.selected):,}\n\nDrag to rotate or paint. Mouse wheel zooms. Patches do not move original points.")

def main():
 root=tk.Tk();App(root);root.mainloop()
if __name__=='__main__':main()
