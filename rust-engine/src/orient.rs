//! Reviewable face-orientation proposals, independent of vertex coordinates.
//! This ensures shared-edge consistency only; it does not infer material inside/outside.
use std::collections::{HashMap, VecDeque};
use crate::{edge, Mesh, MeshError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrientationError {
    InvalidMesh(MeshError),
    DegenerateFace(usize),
    NonManifoldEdge(usize, usize),
    ContradictoryWinding,
    SourceChanged,
}
/// A proposed set of face reversals. Face IDs refer to the unchanged source.
#[derive(Clone, Debug)]
pub struct OrientationProposal {
    source: Mesh,
    reverse: Vec<bool>,
    /// Number of disconnected edge-connected face components considered.
    pub components: usize,
}
impl OrientationProposal {
    pub fn flipped_face_ids(&self)->impl Iterator<Item=usize>+'_ {
        self.reverse.iter().enumerate().filter_map(|(i,&yes)|yes.then_some(i))
    }
    /// Return a separate mesh; source positions and face ordering stay unchanged.
    pub fn apply(&self, mesh:&Mesh)->Result<Mesh,OrientationError> {
        if mesh!=&self.source{return Err(OrientationError::SourceChanged);}
        let mut output=mesh.clone();
        for (face,&reverse) in output.triangles.iter_mut().zip(&self.reverse) {
            if reverse {face.swap(1,2);}
        }
        Ok(output)
    }
}
impl Mesh {
    /// Propose locally consistent winding without choosing an outward side.
    /// Reject non-manifold edges and contradictory constraints rather than guess.
    /// Within each component, choose the equivalent orientation with fewer flips.
    pub fn propose_consistent_orientation(&self)->Result<OrientationProposal,OrientationError> {
        self.validate().map_err(OrientationError::InvalidMesh)?;
        let mut edges:HashMap<(usize,usize),Vec<(usize,bool)>>=HashMap::new();
        for (i,&face) in self.triangles.iter().enumerate() {
            if self.degenerate(face){return Err(OrientationError::DegenerateFace(i));}
            let [a,b,c]=face;
            for (from,to) in [(a,b),(b,c),(c,a)] {
                let e=edge(from,to);let uses=edges.entry(e).or_default();
                uses.push((i,(from,to)==e));
                if uses.len()>2{return Err(OrientationError::NonManifoldEdge(e.0,e.1));}
            }
        }
        let mut adj=vec![Vec::<(usize,bool)>::new();self.triangles.len()];
        for uses in edges.values() {
            if let [(a,ad),(b,bd)]=uses.as_slice(){
                // If both original faces run along the shared edge in the same
                // direction, exactly one must be reversed.
                let different=ad==bd;adj[*a].push((*b,different));adj[*b].push((*a,different));
            }
        }
        let mut states:Vec<Option<bool>>=vec![None;self.triangles.len()];let mut components=0;
        for root in 0..self.triangles.len() {
            if states[root].is_some(){continue;}components+=1;states[root]=Some(false);
            let mut queue=VecDeque::from([root]);let mut members=Vec::new();
            while let Some(face)=queue.pop_front(){
                members.push(face);let state=states[face].unwrap();
                for &(other,different) in &adj[face] {
                    let expected=state^different;
                    match states[other] {
                        Some(existing) if existing!=expected=>return Err(OrientationError::ContradictoryWinding),
                        Some(_)=>{},None=>{states[other]=Some(expected);queue.push_back(other);}
                    }
                }
            }
            // Either component orientation is consistent. Choose fewer reversals
            // but make no assumption that the result points outward.
            let flips=members.iter().filter(|&&i|states[i]==Some(true)).count();
            if flips>members.len()-flips {for i in members{states[i]=Some(!states[i].unwrap());}}
        }
        Ok(OrientationProposal{source:self.clone(),reverse:states.into_iter().map(|x|x.unwrap()).collect(),components})
    }
}
#[cfg(test)]
mod tests {
    use super::*;use crate::Point3;
    fn plate()->Mesh {Mesh{vertices:vec![Point3{x:0.,y:0.,z:0.},Point3{x:1.,y:0.,z:0.},Point3{x:1.,y:1.,z:0.},Point3{x:0.,y:1.,z:0.}],triangles:vec![[0,1,2],[0,3,2]]}}
    #[test] fn propose_and_apply_preserves_positions(){let m=plate();let copy=m.clone();let p=m.propose_consistent_orientation().unwrap();assert_eq!(p.flipped_face_ids().count(),1);assert_eq!(m,copy);let out=p.apply(&m).unwrap();assert_eq!(out.vertices,m.vertices);assert_eq!(out.diagnose().unwrap().inconsistent_edges,0);assert_eq!(out.diagnose().unwrap().boundary_edges,4);}
    #[test] fn stale_source_cannot_be_applied(){let mut m=plate();let p=m.propose_consistent_orientation().unwrap();m.vertices[0].z=0.1;assert_eq!(p.apply(&m),Err(OrientationError::SourceChanged));}
    #[test] fn reject_nonmanifold_edge(){let mut m=plate();m.triangles.push([0,2,1]);assert!(matches!(m.propose_consistent_orientation(),Err(OrientationError::NonManifoldEdge(0,2))));}
    #[test] fn reject_degenerate_face(){let mut m=plate();m.triangles.push([0,0,1]);assert_eq!(m.propose_consistent_orientation().unwrap_err(),OrientationError::DegenerateFace(2));}
    #[test] fn disconnected_components_not_forced_to_match(){let mut m=plate();m.vertices.extend([Point3{x:3.,y:0.,z:0.},Point3{x:4.,y:0.,z:0.},Point3{x:3.,y:1.,z:0.}]);m.triangles.push([4,5,6]);let p=m.propose_consistent_orientation().unwrap();assert_eq!(p.components,2);let out=p.apply(&m).unwrap();assert_eq!(out.triangles[2],m.triangles[2]);}
}
