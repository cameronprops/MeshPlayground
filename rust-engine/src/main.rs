use std::{env,fs::File,io::{BufReader,BufWriter},process::ExitCode};
use meshplayground_engine::{read_binary_stl,write_binary_stl};
fn run()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<String>=env::args().collect();
 if args.len()<3 || !matches!(args[1].as_str(),"inspect"|"clean") || (args[1]=="clean" && args.len()!=4) || (args[1]=="inspect" && args.len()!=3) {
  return Err("Usage: meshplayground inspect INPUT.stl | meshplayground clean INPUT.stl OUTPUT.stl".into());
 }
 if args[1]=="clean" && std::path::Path::new(&args[2]).canonicalize().ok()==std::path::Path::new(&args[3]).canonicalize().ok() && std::path::Path::new(&args[3]).exists() {return Err("Input and output must be different files".into());}
 let mut mesh=read_binary_stl(BufReader::new(File::open(&args[2])?))?;let before=mesh.diagnose()?;
 println!("Vertices: {}\nTriangles: {}\nBoundary edges: {}\nNon-manifold edges: {}\nInconsistent edges: {}\nDegenerate triangles: {}\nDuplicate triangles: {}",before.vertices,before.triangles,before.boundary_edges,before.nonmanifold_edges,before.inconsistent_edges,before.degenerate_triangles,before.duplicate_triangles);
 if args[1]=="clean" {let report=mesh.cleanup(None)?;write_binary_stl(&mesh,BufWriter::new(File::create(&args[3])?))?;println!("Removed {} degenerate and {} duplicate triangles. Coordinates were not moved. STL uses float32.",report.removed_degenerate,report.removed_duplicate);}
 Ok(())
}
fn main()->ExitCode{match run(){Ok(())=>ExitCode::SUCCESS,Err(e)=>{eprintln!("Error: {e}");ExitCode::FAILURE}}}
