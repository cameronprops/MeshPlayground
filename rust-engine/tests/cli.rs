use std::{fs,process::Command,time::{SystemTime,UNIX_EPOCH}};
use meshplayground_engine::{Mesh,Point3,write_binary_stl,read_binary_stl};
#[test]
fn conservative_repair_orients_and_never_overwrites(){
 let dir=std::env::temp_dir().join(format!("meshplayground-test-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir(&dir).unwrap();
 let input=dir.join("input.stl");let output=dir.join("output.stl");
 let m=Mesh{vertices:vec![Point3{x:0.,y:0.,z:0.},Point3{x:1.,y:0.,z:0.},Point3{x:1.,y:1.,z:0.},Point3{x:0.,y:1.,z:0.}],triangles:vec![[0,1,2],[0,3,2]]};
 write_binary_stl(&m,fs::File::create(&input).unwrap()).unwrap();let before=fs::read(&input).unwrap();
 let run=Command::new(env!("CARGO_BIN_EXE_meshplayground")).arg("repair-safe").arg(&input).arg(&output).output().unwrap();assert!(run.status.success(),"{}",String::from_utf8_lossy(&run.stderr));
 let result=read_binary_stl(fs::File::open(&output).unwrap()).unwrap();assert_eq!(result.diagnose().unwrap().inconsistent_edges,0);assert_eq!(result.diagnose().unwrap().boundary_edges,4);assert_eq!(fs::read(&input).unwrap(),before);
 let output_before=fs::read(&output).unwrap();let second=Command::new(env!("CARGO_BIN_EXE_meshplayground")).arg("repair-safe").arg(&input).arg(&output).output().unwrap();assert!(!second.status.success());assert_eq!(fs::read(&output).unwrap(),output_before);
 fs::remove_dir_all(dir).unwrap();
}
