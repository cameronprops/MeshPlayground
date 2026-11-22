//! Sampled bidirectional point-to-surface measurements.
//! This is a lower-bound estimate, not a guaranteed maximum surface distance.
use crate::{Mesh,MeshError,Point3};
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum DeviationError { InvalidSource(MeshError), InvalidTarget(MeshError), EmptySurface, WorkLimitExceeded, NumericalFailure }
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct DirectionalDeviation { pub sample_count:usize,pub maximum:f64,pub rms:f64 }
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct DeviationReport { pub source_to_target:DirectionalDeviation,pub target_to_source:DirectionalDeviation }
fn lerp(a:Point3,b:Point3,t:f64)->Point3{a.add(b.sub(a).scale(t))}
fn segment_distance2(p:Point3,a:Point3,b:Point3)->f64 {
 let ab=b.sub(a);let denom=ab.magnitude2();if denom==0.0{return p.sub(a).magnitude2();}
 let t=(p.sub(a).dot(ab)/denom).clamp(0.0,1.0);p.sub(lerp(a,b,t)).magnitude2()
}
/// Closest point on a triangle, including edges and degenerate triangles.
fn triangle_distance2(p:Point3,a:Point3,b:Point3,c:Point3)->f64 {
 let ab=b.sub(a);let ac=c.sub(a);let n=ab.cross(ac);let n2=n.magnitude2();
 let segment_min=||segment_distance2(p,a,b).min(segment_distance2(p,b,c)).min(segment_distance2(p,c,a));
 if n2==0.0 || !n2.is_finite(){return segment_min();}
 let signed=p.sub(a).dot(n);let projected=p.sub(n.scale(signed/n2));
 let ap=projected.sub(a);let d00=ab.dot(ab);let d01=ab.dot(ac);let d11=ac.dot(ac);let d20=ap.dot(ab);let d21=ap.dot(ac);
 let denom=d00*d11-d01*d01;
 if denom<=0.0 || !denom.is_finite(){return segment_min();}
 let v=(d11*d20-d01*d21)/denom;let w=(d00*d21-d01*d20)/denom;
 if v>=0.0 && w>=0.0 && v+w<=1.0 {p.sub(projected).magnitude2()}else{segment_min()}
}
fn directional(source:&Mesh,target:&Mesh)->Result<DirectionalDeviation,DeviationError>{
 let mut count=0usize;let mut max_squared:f64=0.0;let mut sum_squared=0.0;
 for &[a,b,c] in &source.triangles {
  let (a,b,c)=(source.vertices[a],source.vertices[b],source.vertices[c]);
  // Uniform barycentric samples per face: vertices, edge midpoints, centroid.
  let centroid=a.scale(1.0/3.0).add(b.scale(1.0/3.0)).add(c.scale(1.0/3.0));
  for p in [a,b,c,lerp(a,b,0.5),lerp(b,c,0.5),lerp(c,a,0.5),centroid] {
   if !p.finite(){return Err(DeviationError::NumericalFailure);}
   let mut best=f64::INFINITY;
   for &[x,y,z] in &target.triangles {best=best.min(triangle_distance2(p,target.vertices[x],target.vertices[y],target.vertices[z]));}
   if !best.is_finite(){return Err(DeviationError::NumericalFailure);}
   max_squared=max_squared.max(best);sum_squared+=best;count+=1;
  }
 }
 if !sum_squared.is_finite(){return Err(DeviationError::NumericalFailure);}
 Ok(DirectionalDeviation{sample_count:count,maximum:max_squared.sqrt(),rms:(sum_squared/count as f64).sqrt()})
}
/// Compare both directions, using seven samples per face. `max_triangle_tests`
/// caps brute-force work across both directions. Distances are in mesh units.
/// This estimator can miss peaks between samples and is not a tolerance proof.
pub fn sampled_deviation(source:&Mesh,target:&Mesh,max_triangle_tests:usize)->Result<DeviationReport,DeviationError>{
 source.validate().map_err(DeviationError::InvalidSource)?;target.validate().map_err(DeviationError::InvalidTarget)?;
 if source.triangles.is_empty()||target.triangles.is_empty(){return Err(DeviationError::EmptySurface);}
 let work=source.triangles.len().checked_mul(target.triangles.len()).and_then(|x|x.checked_mul(14)).ok_or(DeviationError::WorkLimitExceeded)?;
 if work>max_triangle_tests{return Err(DeviationError::WorkLimitExceeded);}
 Ok(DeviationReport{source_to_target:directional(source,target)?,target_to_source:directional(target,source)?})
}
#[cfg(test)]
mod tests {
 use super::*;
 fn plane(z:f64)->Mesh{Mesh{vertices:vec![Point3{x:0.,y:0.,z},Point3{x:1.,y:0.,z},Point3{x:1.,y:1.,z},Point3{x:0.,y:1.,z}],triangles:vec![[0,1,2],[0,2,3]]}}
 #[test] fn same_surface_different_triangulation(){let a=plane(0.);let mut b=plane(0.);b.triangles=vec![[0,1,3],[1,2,3]];let r=sampled_deviation(&a,&b,100).unwrap();assert!(r.source_to_target.maximum<1e-12);assert!(r.target_to_source.maximum<1e-12);}
 #[test] fn translated_surface(){let r=sampled_deviation(&plane(0.),&plane(0.1),100).unwrap();assert!((r.source_to_target.maximum-0.1).abs()<1e-12);assert!((r.target_to_source.rms-0.1).abs()<1e-12);}
 #[test] fn preserves_inputs_and_limits_work(){let a=plane(0.);let b=plane(1.);let original=a.clone();assert_eq!(sampled_deviation(&a,&b,55),Err(DeviationError::WorkLimitExceeded));assert_eq!(a,original);}
 #[test] fn empty_mesh_rejected(){let m=Mesh{vertices:vec![],triangles:vec![]};assert_eq!(sampled_deviation(&m,&plane(0.),100),Err(DeviationError::EmptySurface));}
 #[test] fn degenerate_target_is_measured_as_segments(){let mut b=plane(0.);b.triangles=vec![[0,1,1]];let r=sampled_deviation(&b,&b,14).unwrap();assert_eq!(r.source_to_target.maximum,0.);}
}
