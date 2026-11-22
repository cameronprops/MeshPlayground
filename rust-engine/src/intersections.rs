//! Preliminary, read-only triangle intersection triage.
//! This is floating-point detection, not an exact predicate or a repair.
use crate::{Mesh,MeshError,Point3};
#[derive(Clone,Debug,PartialEq)]
pub enum IntersectionError { InvalidMesh(MeshError), InvalidTolerance, DegenerateTriangle(usize), NumericalFailure, CandidateLimitExceeded }
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct IntersectionReport {
 pub pairs:Vec<(usize,usize)>,
 /// Subset of reported pairs with distinct vertex IDs whose coordinates are
 /// within the caller's tolerance. These may be contacts or real overlaps.
 pub near_coincident_vertex_pairs:Vec<(usize,usize)>,
 pub candidate_pairs_tested:usize,pub shared_vertex_pairs_skipped:usize,
}
#[derive(Clone,Copy)]
struct Bounds { min:[f64;3],max:[f64;3],face:usize }
fn coord(p:Point3)->[f64;3] {[p.x,p.y,p.z]}
fn bounds(m:&Mesh,face:usize)->Bounds {
 let f=m.triangles[face];let a=coord(m.vertices[f[0]]);let b=coord(m.vertices[f[1]]);let c=coord(m.vertices[f[2]]);
 Bounds{min:std::array::from_fn(|i|a[i].min(b[i]).min(c[i])),max:std::array::from_fn(|i|a[i].max(b[i]).max(c[i])),face}
}
fn segment_triangle(a:Point3,b:Point3,t:[Point3;3],eps:f64)->bool{
 let dir=b.sub(a);let e1=t[1].sub(t[0]);let e2=t[2].sub(t[0]);let h=dir.cross(e2);let det=e1.dot(h);
 let scale=(e1.magnitude2()*e2.magnitude2()*dir.magnitude2()).sqrt();
 if scale==0.0 || !scale.is_finite() || det.abs()<=1e-14*scale{return false;}
 let inv=1.0/det;let s=a.sub(t[0]);let u=inv*s.dot(h);let q=s.cross(e1);let v=inv*dir.dot(q);let position=inv*e2.dot(q);
 let length=dir.magnitude2().sqrt();let param_tolerance=if length>0.0 {eps/length}else{0.0};
 // Barycentric slack is dimensionless and intentionally only a numeric guard.
 u>=-1e-12 && v>=-1e-12 && u+v<=1.0+1e-12 && position>=-param_tolerance && position<=1.0+param_tolerance
}
fn orient(a:(f64,f64),b:(f64,f64),c:(f64,f64))->f64{(b.0-a.0)*(c.1-a.1)-(b.1-a.1)*(c.0-a.0)}
fn on_segment(a:(f64,f64),b:(f64,f64),p:(f64,f64),tol:f64)->bool{
 let dx=b.0-a.0;let dy=b.1-a.1;let len=(dx*dx+dy*dy).sqrt();
 orient(a,b,p).abs()<=tol*len && p.0>=a.0.min(b.0)-tol && p.0<=a.0.max(b.0)+tol && p.1>=a.1.min(b.1)-tol && p.1<=a.1.max(b.1)+tol
}
fn segments_cross(a:(f64,f64),b:(f64,f64),c:(f64,f64),d:(f64,f64),tol:f64)->bool{
 let(o1,o2,o3,o4)=(orient(a,b,c),orient(a,b,d),orient(c,d,a),orient(c,d,b));
 (o1.signum()!=o2.signum()&&o3.signum()!=o4.signum())||on_segment(a,b,c,tol)||on_segment(a,b,d,tol)||on_segment(c,d,a,tol)||on_segment(c,d,b,tol)
}
fn in_triangle(p:(f64,f64),t:[(f64,f64);3])->bool{
 let a=orient(t[0],t[1],p);let b=orient(t[1],t[2],p);let c=orient(t[2],t[0],p);
 (a>=0.0&&b>=0.0&&c>=0.0)||(a<=0.0&&b<=0.0&&c<=0.0)
}
fn coplanar_overlap(a:[Point3;3],b:[Point3;3],normal:Point3,tol:f64)->bool{
 let n=coord(normal);let axis=if n[0].abs()>=n[1].abs()&&n[0].abs()>=n[2].abs(){0}else if n[1].abs()>=n[2].abs(){1}else{2};
 let proj=|p:Point3|{let q=coord(p);match axis{0=>(q[1],q[2]),1=>(q[0],q[2]),_=>(q[0],q[1])}};
 let x=a.map(proj);let y=b.map(proj);
 for i in 0..3 {for j in 0..3 {if segments_cross(x[i],x[(i+1)%3],y[j],y[(j+1)%3],tol){return true;}}}
 in_triangle(x[0],y)||in_triangle(y[0],x)
}
fn intersect(a:[Point3;3],b:[Point3;3],tolerance:f64)->bool{
 let n=a[1].sub(a[0]).cross(a[2].sub(a[0]));let norm=n.magnitude2().sqrt();
 let bnormal=b[1].sub(b[0]).cross(b[2].sub(b[0]));let parallel=n.cross(bnormal).magnitude2().sqrt()<=1e-12*norm*bnormal.magnitude2().sqrt();
 if parallel && b.iter().all(|p|p.sub(a[0]).dot(n).abs()<=tolerance*norm){return coplanar_overlap(a,b,n,tolerance);}
 for i in 0..3 {if segment_triangle(a[i],a[(i+1)%3],b,tolerance)||segment_triangle(b[i],b[(i+1)%3],a,tolerance){return true;}}
 false
}
impl Mesh {
 /// Find potentially intersecting pairs with an X-axis sweep-and-prune broad
 /// phase. Shared-vertex pairs are skipped, including adjacent faces that might
 /// overlap. A hit is evidence for review, not a complete printable-mesh check.
 /// Exceeding the candidate cap returns an error, never a partial clean result.
 pub fn detect_intersections(&self,tolerance:f64,max_candidates:usize)->Result<IntersectionReport,IntersectionError>{
  self.validate().map_err(IntersectionError::InvalidMesh)?;
  if !tolerance.is_finite()||tolerance<0.0{return Err(IntersectionError::InvalidTolerance);}
  for (i,&f) in self.triangles.iter().enumerate(){
   let a=self.vertices[f[0]];let b=self.vertices[f[1]];let c=self.vertices[f[2]];let n=b.sub(a).cross(c.sub(a)).magnitude2();
   if !n.is_finite(){return Err(IntersectionError::NumericalFailure);}if n==0.0{return Err(IntersectionError::DegenerateTriangle(i));}
  }
  let mut boxes:Vec<_>=(0..self.triangles.len()).map(|i|bounds(self,i)).collect();boxes.sort_by(|a,b|a.min[0].total_cmp(&b.min[0]));
  let mut report=IntersectionReport{pairs:Vec::new(),near_coincident_vertex_pairs:Vec::new(),candidate_pairs_tested:0,shared_vertex_pairs_skipped:0};
  for i in 0..boxes.len(){let x=boxes[i];for y in boxes.iter().skip(i+1){
   if y.min[0]>x.max[0]+tolerance{break;}
   if (1..3).any(|axis|y.min[axis]>x.max[axis]+tolerance||x.min[axis]>y.max[axis]+tolerance){continue;}
   let (a,b)=(self.triangles[x.face],self.triangles[y.face]);
   if a.iter().any(|p|b.contains(p)){report.shared_vertex_pairs_skipped+=1;continue;}
   if report.candidate_pairs_tested>=max_candidates{return Err(IntersectionError::CandidateLimitExceeded);}
   report.candidate_pairs_tested+=1;
   if intersect(a.map(|v|self.vertices[v]),b.map(|v|self.vertices[v]),tolerance){
    let pair=(x.face.min(y.face),x.face.max(y.face));report.pairs.push(pair);
    // Classify, never suppress or weld: proximity alone does not prove contact.
    if a.iter().any(|&v|b.iter().any(|&w|self.vertices[v].sub(self.vertices[w]).magnitude2().sqrt()<=tolerance)){
     report.near_coincident_vertex_pairs.push(pair);
    }
   }
  }}
  report.pairs.sort_unstable();report.near_coincident_vertex_pairs.sort_unstable();Ok(report)
 }
}
#[cfg(test)]
mod tests {
 use super::*;
 fn p(x:f64,y:f64,z:f64)->Point3{Point3{x,y,z}}
 #[test] fn finds_crossing_triangles_without_mutation(){let m=Mesh{vertices:vec![p(0.,0.,0.),p(2.,0.,0.),p(0.,2.,0.),p(0.5,0.5,-1.),p(0.5,0.5,1.),p(1.,0.5,1.)],triangles:vec![[0,1,2],[3,4,5]]};let before=m.clone();let r=m.detect_intersections(1e-9,100).unwrap();assert_eq!(r.pairs,vec![(0,1)]);assert_eq!(m,before);}
 #[test] fn finds_coplanar_overlap(){let m=Mesh{vertices:vec![p(0.,0.,0.),p(2.,0.,0.),p(0.,2.,0.),p(0.25,0.25,0.),p(0.5,0.25,0.),p(0.25,0.5,0.)],triangles:vec![[0,1,2],[3,4,5]]};assert_eq!(m.detect_intersections(1e-9,100).unwrap().pairs,vec![(0,1)]);}
 #[test] fn distinct_and_adjacent_are_not_reported(){let m=Mesh{vertices:vec![p(0.,0.,0.),p(2.,0.,0.),p(0.,2.,0.),p(2.,2.,0.),p(0.,0.,5.),p(1.,0.,5.),p(0.,1.,5.)],triangles:vec![[0,1,2],[1,3,2],[4,5,6]]};let r=m.detect_intersections(1e-9,100).unwrap();assert!(r.pairs.is_empty());assert_eq!(r.shared_vertex_pairs_skipped,1);}
 #[test] fn candidate_limit_must_not_return_partial_results(){let m=Mesh{vertices:vec![p(0.,0.,0.),p(2.,0.,0.),p(0.,2.,0.),p(0.5,0.5,-1.),p(0.5,0.5,1.),p(1.,0.5,1.)],triangles:vec![[0,1,2],[3,4,5]]};assert_eq!(m.detect_intersections(0.,0),Err(IntersectionError::CandidateLimitExceeded));}
 #[test] fn rejects_degenerate_triangles(){let m=Mesh{vertices:vec![p(0.,0.,0.),p(1.,0.,0.),p(2.,0.,0.)],triangles:vec![[0,1,2]]};assert_eq!(m.detect_intersections(0.,100),Err(IntersectionError::DegenerateTriangle(0)));}
 #[test] fn reports_near_coincident_vertex_contacts_without_suppressing(){
  let m=Mesh{vertices:vec![p(0.,0.,0.),p(1.,0.,0.),p(0.,1.,0.),p(0.,0.,1e-12),p(0.,0.,1.),p(-1.,0.,0.)],triangles:vec![[0,1,2],[3,4,5]]};
  let r=m.detect_intersections(1e-9,100).unwrap();assert_eq!(r.pairs,vec![(0,1)]);assert_eq!(r.near_coincident_vertex_pairs,vec![(0,1)]);
 }
}
