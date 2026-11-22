"""Benchmark-only baseline. Run under Blender's Python; not a product dependency."""
import bpy,bmesh,sys,time,json
from mathutils import Vector
from mathutils.bvhtree import BVHTree
args=sys.argv[sys.argv.index('--')+1:];source,output,report=args
v=[];f=[]
with open(source,encoding='utf8') as stream:
 for line in stream:
  p=line.split()
  if p and p[0]=='v':v.append(tuple(map(float,p[1:4])))
  elif p and p[0]=='f':f.append(tuple(int(x.split('/')[0])-1 for x in p[1:]))
mesh=bpy.data.meshes.new('benchmark');mesh.from_pydata(v,[],f);mesh.update()
bm=bmesh.new();bm.from_mesh(mesh);start=time.perf_counter()
try:
 edges=[e for e in bm.edges if e.is_boundary];new=bmesh.ops.holes_fill(bm,edges=edges,sides=0)['faces'];elapsed=time.perf_counter()-start
 bmesh.ops.triangulate(bm,faces=list(bm.faces));bm.to_mesh(mesh);mesh.update();status={'ok':True,'elapsed_seconds':elapsed,'new_polygons_before_triangulation':len(new)}
except Exception as e:status={'ok':False,'error':str(e),'elapsed_seconds':time.perf_counter()-start}
bm.free()
if status['ok']:
 coords=[tuple(p.co) for p in mesh.vertices];faces=[tuple(p.vertices) for p in mesh.polygons]
 tree=BVHTree.FromPolygons([Vector(x) for x in coords],faces,all_triangles=True,epsilon=0)
 overlaps={(min(a,b),max(a,b)) for a,b in tree.overlap(tree) if a!=b and not(set(faces[a])&set(faces[b]))}
 status['intersection_pairs_excluding_shared_vertex']=len(overlaps)
 with open(output,'w',encoding='utf8') as out:
  for p in coords:out.write('v %.17g %.17g %.17g\n'%p)
  for face in faces:out.write('f %s\n'%(' '.join(str(x+1) for x in face)))
with open(report,'w',encoding='utf8') as out:json.dump(status,out)
