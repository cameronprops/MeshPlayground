import tempfile, pathlib
from mesh_workshop import Mesh,write_stl,read_mesh
v=[(0.,0.,0.),(1.,0.,0.),(1.,1.,0.),(0.,1.,0.),(0.,0.,1.),(1.,0.,1.),(1.,1.,1.),(0.,1.,1.)]
f=[(0,2,1),(0,3,2),(0,1,5),(0,5,4),(1,2,6),(1,6,5),(2,3,7),(2,7,6),(3,0,4),(3,4,7)]
m=Mesh(v[:],f[:]);assert m.diagnostics()['boundary_edges']==4
p=m.patch();assert p['patched_loops']==1,p
assert m.v==v and m.diagnostics()['boundary_edges']==0
assert all(len(x)==2 for x in m.edge_map().values())
x=Mesh(v[:],f[:]);assert x.patch({4})['patched_loops']==0
p=x.patch({3,5,7,9});assert p['patched_loops']==1,p
plate=Mesh(v[:4],[(0,1,2),(0,2,3)]);shell=plate.shell(.1);assert shell.v[:4]==v[:4] and shell.diagnostics()['boundary_edges']==0
with tempfile.TemporaryDirectory() as d:
 path=pathlib.Path(d)/'test.stl';write_stl(path,m);n=read_mesh(path);assert n.diagnostics()['boundary_edges']==0 and len(n.f)==len(m.f)
print('PASS: hole patch, original coordinates, painted scope, shell boundary, STL round trip')
from mesh_workshop import triangulate

def rejected(vertices):
 try:triangulate(list(range(len(vertices))),vertices)
 except ValueError as e:return str(e)
 raise AssertionError('Unsafe loop was accepted')

assert rejected([(0,0,0),(2,0,0),(2,2,1),(0,2,0)])=='nonplanar boundary'
assert rejected([(0,0,0),(3,3,0),(0,2,0),(2,0,0)]) in ('self-intersecting boundary','degenerate boundary')
# A flipped adjacent face must not receive a patch with inconsistent normals.
bad=Mesh(v[:],f[:]);bad.f[3]=tuple(reversed(bad.f[3]));result=bad.patch()
assert result['patched_loops']==0 and result['reasons']['inconsistent boundary winding']==1
assert bad.v==v and len(bad.f)==len(f)
print('PASS: nonplanar and crossing loops rejected; inconsistent winding left untouched')
