//! Benchmarks known injected intersections from the downloadable STL test set.
//! Run with MESH_FIXTURES set to a directory containing the decompressed files.
use std::{fs::File,io::BufReader,path::Path,time::Instant};
use meshplayground_engine::read_binary_stl;
#[test]
fn test_known_intersections_on_real_fixtures(){
 let root=match std::env::var_os("MESH_FIXTURES"){Some(p)=>p,None=>{eprintln!("fixture benchmark skipped: MESH_FIXTURES not set");return;}};
 for name in ["sphere","cube","teapot"] {
  let path=Path::new(&root).join(format!("{name}_damaged.stl"));
  let mut mesh=read_binary_stl(BufReader::new(File::open(&path).expect("missing test fixture"))).unwrap();
  let before=mesh.diagnose().unwrap();let clean=mesh.cleanup(None).unwrap();
  // The generated fixtures append two crossing triangles after all other faces.
  // Cleanup can renumber faces, so identify the surviving final two after cleanup.
  let marker=[mesh.triangles.len()-2,mesh.triangles.len()-1];let t=Instant::now();
  let report=mesh.detect_intersections(1e-6,20_000_000).expect("intersection scan must complete");
  let elapsed=t.elapsed().as_secs_f64();
  let marker_hits=marker.map(|id|report.pairs.iter().filter(|&&(a,b)|a==id||b==id).count());
  let other=report.pairs.iter().filter(|&&(a,b)|!marker.contains(&a)&&!marker.contains(&b)).count();
  println!("FIXTURE {name} triangles={} boundary_edges={} degenerate_removed={} duplicate_removed={} candidates={} pairs={} near_coincident={} injected_hits={marker_hits:?} other_pairs={other} shared_vertex_skipped={} seconds={elapsed:.6}",before.triangles,before.boundary_edges,clean.removed_degenerate,clean.removed_duplicate,report.candidate_pairs_tested,report.pairs.len(),report.near_coincident_vertex_pairs.len(),report.shared_vertex_pairs_skipped);
  assert!(marker_hits[0]>0 && marker_hits[1]>0,"missed injected crossing for {name}: {marker_hits:?}");
  if name=="teapot" {
   for tolerance in [0.0,1e-9] {
    let scan=mesh.detect_intersections(tolerance,20_000_000).unwrap();
    let extras:Vec<_>=scan.pairs.iter().filter(|&&(a,b)|!marker.contains(&a)&&!marker.contains(&b)).copied().collect();
    println!("FIXTURE teapot tolerance={tolerance} pair_count={} extras={extras:?}",scan.pairs.len());
   }
  }
 }
}
