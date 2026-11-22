//! User-directed flat patch preview for a single convex planar boundary.
//! This first implementation deliberately rejects ambiguous input and never
//! modifies the source. Intersection with other surfaces is not yet checked.
use std::collections::{BTreeMap, HashSet};
use crate::{edge, Mesh, Selection, SelectionError};

#[derive(Clone, Debug, PartialEq)]
pub enum PatchError {
    Selection(SelectionError), InvalidPlanarityTolerance, SelectExactlyOneClosedBoundary,
    SelectEntireBoundary, InconsistentBoundaryWinding, DegenerateBoundary,
    NonPlanarBoundary, NonConvexOrCollinearBoundary, SelfIntersectingBoundary, SourceChanged,
}
#[derive(Clone, Debug)]
pub struct PatchProposal {
    source: Mesh,
    /// Triangles reference original vertex IDs. Their coordinates are unchanged.
    pub triangles: Vec<[usize;3]>,
    /// Greatest distance of boundary vertices from the calculated plane, in mesh units.
    pub max_plane_deviation: f64,
}
impl PatchProposal {
    /// Return a separate mesh only when the reviewed source is unchanged.
    pub fn apply(&self, mesh:&Mesh)->Result<Mesh,PatchError> {
        if mesh!=&self.source {return Err(PatchError::SourceChanged);}
        let mut result=mesh.clone();result.triangles.extend_from_slice(&self.triangles);Ok(result)
    }
}
impl Mesh {
    /// Build a flat fan preview over an explicitly selected convex boundary.
    /// `planarity_tolerance` is caller supplied in mesh units. No vertices move.
    /// No patch-to-mesh collision test is performed; user inspection is essential.
    pub fn propose_flat_patch(&self,selection:&Selection,planarity_tolerance:f64)->Result<PatchProposal,PatchError> {
        if !planarity_tolerance.is_finite() || planarity_tolerance<0.0 {return Err(PatchError::InvalidPlanarityTolerance);}
        let mut seed=Selection::default();seed.edges=selection.edges.iter().map(|&(a,b)|edge(a,b)).collect();
        let boundaries=self.selected_boundaries(&seed).map_err(PatchError::Selection)?;
        if boundaries.len()!=1 || !boundaries[0].closed {return Err(PatchError::SelectExactlyOneClosedBoundary);}
        let boundary=&boundaries[0];let wanted:HashSet<_>=boundary.edges.iter().copied().collect();
        if seed.edges!=wanted {return Err(PatchError::SelectEntireBoundary);}
        let mut adjacency:BTreeMap<usize,Vec<usize>>=BTreeMap::new();
        for &(a,b) in &boundary.edges {adjacency.entry(a).or_default().push(b);adjacency.entry(b).or_default().push(a);}
        let start=boundary.vertices[0];let mut ordered=vec![start];let mut previous=start;let mut current=adjacency[&start][0];
        while current!=start {
            if ordered.len()>=boundary.vertices.len(){return Err(PatchError::SelectExactlyOneClosedBoundary);}
            ordered.push(current);let neighbors=&adjacency[&current];let next=if neighbors[0]==previous{neighbors[1]}else{neighbors[0]};previous=current;current=next;
        }
        if ordered.len()!=boundary.vertices.len(){return Err(PatchError::SelectExactlyOneClosedBoundary);}
        // All boundary faces must traverse the perimeter consistently. The patch
        // is oriented opposite the adjoining faces, without changing those faces.
        let mut orientation=None;
        for i in 0..ordered.len(){
            let (a,b)=(ordered[i],ordered[(i+1)%ordered.len()]);let mut found=None;
            for &face in &boundary.adjacent_faces {
                let [x,y,z]=self.triangles[face];
                for (u,v) in [(x,y),(y,z),(z,x)] {if edge(u,v)==edge(a,b) {found=Some((u,v)==(a,b));}}
            }
            let dir=found.ok_or(PatchError::InconsistentBoundaryWinding)?;
            if orientation.is_some_and(|prior|prior!=dir){return Err(PatchError::InconsistentBoundaryWinding);}orientation=Some(dir);
        }
        if orientation==Some(true){ordered.reverse();}
        let origin=self.vertices[ordered[0]];
        // Newell's method computed relative to origin for better large-coordinate stability.
        let mut n=crate::Point3{x:0.,y:0.,z:0.};
        for i in 0..ordered.len(){let a=self.vertices[ordered[i]].sub(origin);let b=self.vertices[ordered[(i+1)%ordered.len()]].sub(origin);n=n.add(a.cross(b));}
        let length=n.magnitude2().sqrt();if !length.is_finite() || length==0.0{return Err(PatchError::DegenerateBoundary);}
        let normal=n.scale(1.0/length);let mut deviation:f64=0.0;
        for &v in &ordered {deviation=deviation.max(self.vertices[v].sub(origin).dot(normal).abs());}
        if !deviation.is_finite() || deviation>planarity_tolerance{return Err(PatchError::NonPlanarBoundary);}
        let axis=if normal.x.abs()>=normal.y.abs() && normal.x.abs()>=normal.z.abs(){0}else if normal.y.abs()>=normal.z.abs(){1}else{2};
        let points:Vec<(f64,f64)>=ordered.iter().map(|&v|{let p=self.vertices[v].sub(origin);match axis{0=>(p.y,p.z),1=>(p.x,p.z),_=>(p.x,p.y)}}).collect();
        fn orient(a:(f64,f64),b:(f64,f64),c:(f64,f64))->f64 {(b.0-a.0)*(c.1-a.1)-(b.1-a.1)*(c.0-a.0)}
        // Reject crossings and touching of non-neighboring segments.
        fn intersects(a:(f64,f64),b:(f64,f64),c:(f64,f64),d:(f64,f64))->bool {
            let (o1,o2,o3,o4)=(orient(a,b,c),orient(a,b,d),orient(c,d,a),orient(c,d,b));
            fn on(a:(f64,f64),b:(f64,f64),p:(f64,f64))->bool {p.0>=a.0.min(b.0)&&p.0<=a.0.max(b.0)&&p.1>=a.1.min(b.1)&&p.1<=a.1.max(b.1)}
            (o1==0.0&&on(a,b,c))||(o2==0.0&&on(a,b,d))||(o3==0.0&&on(c,d,a))||(o4==0.0&&on(c,d,b))||((o1>0.)!=(o2>0.)&&(o3>0.)!=(o4>0.))
        }
        let count=points.len();for i in 0..count {for j in (i+1)..count {if j==i+1 || (i==0 && j==count-1){continue;}
            if intersects(points[i],points[(i+1)%count],points[j],points[(j+1)%count]){return Err(PatchError::SelfIntersectingBoundary);}
        }}
        let mut sign=0i8;
        for i in 0..count {let turn=orient(points[i],points[(i+1)%count],points[(i+2)%count]);
            if !turn.is_finite() || turn==0.0{return Err(PatchError::NonConvexOrCollinearBoundary);}
            let s=if turn>0.0{1}else{-1};if sign!=0 && sign!=s{return Err(PatchError::NonConvexOrCollinearBoundary);}sign=s;
        }
        let triangles=(1..ordered.len()-1).map(|i|[ordered[0],ordered[i],ordered[i+1]]).collect();
        Ok(PatchProposal{source:self.clone(),triangles,max_plane_deviation:deviation})
    }
}

#[cfg(test)]
mod tests {
 use super::*;use crate::Point3;
 fn cube()->(Mesh,Selection){
  let vertices=vec![(0.,0.,0.),(1.,0.,0.),(1.,1.,0.),(0.,1.,0.),(0.,0.,1.),(1.,0.,1.),(1.,1.,1.),(0.,1.,1.)].into_iter().map(|(x,y,z)|Point3{x,y,z}).collect();
  let triangles=vec![[0,2,1],[0,3,2],[0,1,5],[0,5,4],[1,2,6],[1,6,5],[2,3,7],[2,7,6],[3,0,4],[3,4,7]];
  let mut s=Selection::default();for (a,b) in [(4,5),(5,6),(6,7),(7,4)] {s.select_edge(a,b);} (Mesh{vertices,triangles},s)
 }
 #[test] fn patch_is_reviewable_and_preserves_vertices(){let (m,s)=cube();let before=m.clone();let p=m.propose_flat_patch(&s,0.).unwrap();assert_eq!(p.triangles.len(),2);assert_eq!(m,before);let out=p.apply(&m).unwrap();assert_eq!(out.vertices,m.vertices);assert_eq!(out.diagnose().unwrap().boundary_edges,0);assert_eq!(out.diagnose().unwrap().inconsistent_edges,0);assert_eq!(m.triangles.len(),10);}
 #[test] fn partial_boundary_is_rejected(){let (m,mut s)=cube();s.edges.remove(&edge(4,5));assert_eq!(m.propose_flat_patch(&s,0.).unwrap_err(),PatchError::SelectEntireBoundary);}
 #[test] fn nonplanar_and_stale_source_are_rejected(){let (mut m,s)=cube();let p=m.propose_flat_patch(&s,0.).unwrap();m.vertices[6].z+=0.1;assert_eq!(p.apply(&m).unwrap_err(),PatchError::SourceChanged);assert_eq!(m.propose_flat_patch(&s,0.001).unwrap_err(),PatchError::NonPlanarBoundary);}
 #[test] fn empty_and_bad_tolerance_fail(){let(m,_)=cube();assert_eq!(m.propose_flat_patch(&Selection::default(),0.).unwrap_err(),PatchError::SelectExactlyOneClosedBoundary);assert_eq!(m.propose_flat_patch(&Selection::default(),-1.).unwrap_err(),PatchError::InvalidPlanarityTolerance);}
}
