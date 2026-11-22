use std::{env,fs::{File,OpenOptions},io::{BufReader,BufWriter,Write},path::Path,process::ExitCode};
use meshplayground_engine::{read_binary_stl,write_binary_stl,Mesh};
fn show(label:&str,mesh:&Mesh)->Result<(),Box<dyn std::error::Error>> {
 let d=mesh.diagnose()?;
 println!("{label}:\n  vertices: {}\n  triangles: {}\n  boundary edges: {}\n  non-manifold edges: {}\n  inconsistent shared edges: {}\n  degenerate triangles: {}\n  duplicate triangles: {}",d.vertices,d.triangles,d.boundary_edges,d.nonmanifold_edges,d.inconsistent_edges,d.degenerate_triangles,d.duplicate_triangles);
 Ok(())
}
fn run()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<String>=env::args().collect();
 if args.get(1).is_some_and(|v|v=="intersections") {
  if args.len()!=5 {return Err("Usage: meshplayground intersections INPUT.stl TOLERANCE MAX_CANDIDATES".into());}
  let mesh=read_binary_stl(BufReader::new(File::open(&args[2])?))?;
  let tolerance:f64=args[3].parse()?;let max_candidates:usize=args[4].parse()?;
  let report=mesh.detect_intersections(tolerance,max_candidates).map_err(|e|format!("Intersection scan did not complete: {e:?}"))?;
  println!("Potential intersections: {}\nCandidate pairs tested: {}\nShared-vertex pairs skipped: {}",report.pairs.len(),report.candidate_pairs_tested,report.shared_vertex_pairs_skipped);
  for (a,b) in report.pairs {println!("Triangles {a} {b}");}
  println!("Experimental floating-point check; adjacent-face intersections and other numeric cases can be missed. No repair performed.");
  return Ok(());
 }
 if args.len()<3 || !matches!(args[1].as_str(),"inspect"|"clean"|"repair-safe") || (args[1]!="inspect" && args.len()!=4) || (args[1]=="inspect" && args.len()!=3) {
  return Err("Usage: meshplayground inspect INPUT.stl | meshplayground clean INPUT.stl OUTPUT.stl | meshplayground repair-safe INPUT.stl OUTPUT.stl | meshplayground intersections INPUT.stl TOLERANCE MAX_CANDIDATES".into());
 }
 let mut mesh=read_binary_stl(BufReader::new(File::open(&args[2])?))?;show("Before",&mesh)?;
 if args[1]!="inspect" {
  let output=Path::new(&args[3]);
  if output.exists(){return Err("Output already exists; choose a new path".into());}
  let cleanup=mesh.cleanup(None)?;println!("Proposed cleanup: {} degenerate, {} duplicate triangles removed.",cleanup.removed_degenerate,cleanup.removed_duplicate);
  if args[1]=="repair-safe" {
   let proposal=mesh.propose_consistent_orientation().map_err(|e|format!("Cannot safely orient mesh: {e:?}"))?;
   let flipped=proposal.flipped_face_ids().count();mesh=proposal.apply(&mesh).map_err(|e|format!("Orientation proposal failed: {e:?}"))?;
   println!("Orientation: {flipped} faces reversed across {} components. Outward-facing orientation is not verified.",proposal.components);
  }
  show("After",&mesh)?;
  // create_new prevents accidental overwrites, including input paths.
  let output_file=OpenOptions::new().write(true).create_new(true).open(output)?;
  let write_result=(||->std::io::Result<()>{let mut w=BufWriter::new(output_file);write_binary_stl(&mesh,&mut w)?;w.flush()})();
  if let Err(e)=write_result {let _=std::fs::remove_file(output);return Err(e.into());}
  println!("Saved {}. Existing vertex positions were not moved; STL stores float32. Holes and intersections are not repaired or validated.",output.display());
 }
 Ok(())
}
fn main()->ExitCode{match run(){Ok(())=>ExitCode::SUCCESS,Err(e)=>{eprintln!("Error: {e}");ExitCode::FAILURE}}}
