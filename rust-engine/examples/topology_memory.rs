//! Compare the previous per-edge Vec implementation with compact summaries.
//! Run each mode in a separate process under an OS peak-RSS measurement tool.
use std::{collections::{BTreeMap,HashSet},env,time::Instant};
use meshplayground_engine::{Mesh,Point3};
fn edge(a:usize,b:usize)->(usize,usize){if a<b{(a,b)}else{(b,a)}}
fn legacy(mesh:&Mesh)->(usize,usize,usize,usize){
 let mut edges:BTreeMap<(usize,usize),Vec<(usize,usize)>>=BTreeMap::new();let mut seen=HashSet::new();let mut dup=0;
 for &[a,b,c] in &mesh.triangles {let mut face=[a,b,c];face.sort_unstable();if !seen.insert(face){dup+=1;}for (u,v) in [(a,b),(b,c),(c,a)]{edges.entry(edge(u,v)).or_default().push((u,v));}}
 let (mut boundary,mut nonmanifold,mut inconsistent)=(0,0,0);
 for uses in edges.values(){match uses.len(){1=>boundary+=1,2=>if uses[0]==uses[1]{inconsistent+=1},n if n>2=>nonmanifold+=1,_=>{}}}
 (boundary,nonmanifold,inconsistent,dup)
}
fn main(){
 let args:Vec<_>=env::args().collect();assert!(args.len()==3,"Usage: topology_memory GRID_SIZE legacy|compact");let n:usize=args[1].parse().unwrap();assert!((1..=2000).contains(&n));let mode=&args[2];assert!(mode=="legacy"||mode=="compact");
 let mut vertices=Vec::with_capacity((n+1)*(n+1));for y in 0..=n{for x in 0..=n{vertices.push(Point3{x:x as f64,y:y as f64,z:0.});}}
 let mut triangles=Vec::with_capacity(n*n*2);for y in 0..n{for x in 0..n{let a=y*(n+1)+x;let b=a+1;let c=a+n+1;let d=c+1;triangles.extend([[a,b,d],[a,d,c]]);}}
 let mesh=Mesh{vertices,triangles};let start=Instant::now();
 let counts=if mode=="legacy"{legacy(&mesh)}else{let d=mesh.diagnose().unwrap();(d.boundary_edges,d.nonmanifold_edges,d.inconsistent_edges,d.duplicate_triangles)};
 assert_eq!(counts,(n*4,0,0,0));println!("mode={mode} grid={n} triangles={} boundary={} elapsed_ms={}",mesh.triangles.len(),counts.0,start.elapsed().as_millis());
}
