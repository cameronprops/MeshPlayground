#!/usr/bin/env python3
"""Measure preservation and topology against the supplied test mesh pairs.
Run: python benchmarks/run.py /path/to/unzipped/Mesh_Repair_Test_Models [--blender]
The Blender baseline is optional and is never required by the offline product.
"""
import argparse,collections,importlib.util,json,math,os,struct,subprocess,sys,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('prototype',ROOT/'mesh_workshop.py');prototype=importlib.util.module_from_spec(spec);spec.loader.exec_module(prototype)

def load(path):
 v=[];f=[]
 for line in path.read_text().splitlines():
  a=line.split()
  if a and a[0]=='v':v.append(tuple(map(float,a[1:4])))
  elif a and a[0]=='f':f.append(tuple(int(x.split('/')[0])-1 for x in a[1:]))
 return prototype.Mesh(v,f)
def key(m,f,quantize=False):
 def point(p):return tuple(struct.unpack('<f',struct.pack('<f',x))[0] for x in p) if quantize else p
 return tuple(sorted(point(m.v[i]) for i in f))
def compare(reference,input_mesh,result,quantize=False):
 ref=collections.Counter(key(reference,f,quantize) for f in reference.f);inp=collections.Counter(key(input_mesh,f,quantize) for f in input_mesh.f);out=collections.Counter(key(result,f,quantize) for f in result.f)
 same_count=len(result.v)>=len(input_mesh.v)
 max_delta=max((math.dist(a,b) for a,b in zip(input_mesh.v,result.v)),default=0.0) if same_count else None
 return {'comparison_precision':'float32' if quantize else 'original','reference_faces_missing':sum((ref-out).values()),'output_faces_absent_from_reference':sum((out-ref).values()),'input_faces_removed':sum((inp-out).values()),'new_faces_added':sum((out-inp).values()),'original_vertices_same_index_and_coordinate':same_count and result.v[:len(input_mesh.v)]==input_mesh.v,'max_index_matched_vertex_displacement_mesh_units':max_delta,'topology':result.diagnostics()}

def main():
 p=argparse.ArgumentParser();p.add_argument('fixtures',type=Path);p.add_argument('--blender',action='store_true');p.add_argument('--output',type=Path);args=p.parse_args();results={}
 for name in ['sphere','cube','teapot']:
  reference=load(args.fixtures/f'{name}_watertight.obj');source=load(args.fixtures/f'{name}_damaged.obj');work=source.clone()
  t=time.perf_counter();report=work.patch();elapsed=time.perf_counter()-t
  result={'input':{'triangles':len(source.f),'topology':source.diagnostics()},'python_flat_patch':{'elapsed_seconds':round(elapsed,6),'report':report,**compare(reference,source,work)}}
  if args.blender:
   with tempfile.TemporaryDirectory() as d:
    out=Path(d)/'out.obj';log=Path(d)/'report.json';cmd=['blender','-b','--python',str(ROOT/'benchmarks'/'blender_fill.py'),'--',str(args.fixtures/f'{name}_damaged.obj'),str(out),str(log)]
    try:
     run=subprocess.run(cmd,capture_output=True,text=True,timeout=180,check=False)
     if run.returncode or not log.exists():result['blender_fill']={'ok':False,'error':run.stderr[-2000:] or run.stdout[-2000:]}
     else:
      data=json.loads(log.read_text());result['blender_fill']=data
      if data['ok']:data.update(compare(reference,source,load(out),quantize=True))
    except (OSError,subprocess.TimeoutExpired) as e:result['blender_fill']={'ok':False,'error':str(e)}
  results[name]=result
 output=json.dumps({'notes':['Triangle comparison ignores orientation, uses the stated precision, and is not a surface-distance measurement.','Intersection counts are only available for the Blender baseline and exclude pairs that share a vertex.','Blender triangle comparisons normalize both sides to float32; vertex displacement compares original values by index and may include float rounding or reordering.','These benchmarks do not prove printability.'],'models':results},indent=2)
 if args.output:args.output.write_text(output+'\n')
 print(output)
if __name__=='__main__':main()
