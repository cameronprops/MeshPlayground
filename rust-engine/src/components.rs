//! Read-only surface-component discovery and reviewable separation.
//! Edge connectivity alone does not determine whether surfaces intersect or
//! whether nested shells are parts of one solid.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use crate::{edge, Mesh, MeshError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceComponent {
    /// IDs in the original mesh; sorted for reproducible results.
    pub face_ids: Vec<usize>,
    pub vertex_ids: Vec<usize>,
    pub boundary_edges: usize,
    pub nonmanifold_edges: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeparationError { InvalidMesh(MeshError), InvalidComponent(usize), SourceChanged }
/// Surface-only split proposal. Intersection cutting and solid classification
/// are separate operations and are not performed by this proposal.
#[derive(Clone, Debug)]
pub struct SeparationProposal { source: Mesh, pub components: Vec<SurfaceComponent> }
#[derive(Clone, Debug, PartialEq)]
pub struct ExtractedSurface {
    pub mesh: Mesh,
    /// Maps each output vertex back to its original vertex ID.
    pub source_vertex_ids: Vec<usize>,
    /// Maps each output face back to its original face ID.
    pub source_face_ids: Vec<usize>,
}
impl SeparationProposal {
    /// Extract one component to a separate mesh, leaving the original untouched.
    /// Orphan source vertices that are not used by any face are not extracted.
    pub fn extract(&self, mesh:&Mesh, component:usize)->Result<ExtractedSurface,SeparationError> {
        if mesh!=&self.source{return Err(SeparationError::SourceChanged);}
        let c=self.components.get(component).ok_or(SeparationError::InvalidComponent(component))?;
        let vertex_map:HashMap<usize,usize>=c.vertex_ids.iter().enumerate().map(|(new,&old)|(old,new)).collect();
        let vertices=c.vertex_ids.iter().map(|&i|mesh.vertices[i]).collect();
        let triangles=c.face_ids.iter().map(|&i|mesh.triangles[i].map(|v|vertex_map[&v])).collect();
        Ok(ExtractedSurface{mesh:Mesh{vertices,triangles},source_vertex_ids:c.vertex_ids.clone(),source_face_ids:c.face_ids.clone()})
    }
}
struct UnionFind { parents:Vec<usize>, ranks:Vec<u8> }
impl UnionFind {
    fn new(size:usize)->Self {Self{parents:(0..size).collect(),ranks:vec![0;size]}}
    fn root(&mut self,mut x:usize)->usize {while self.parents[x]!=x {self.parents[x]=self.parents[self.parents[x]];x=self.parents[x];}x}
    fn join(&mut self,a:usize,b:usize){let(mut a,mut b)=(self.root(a),self.root(b));if a==b{return;}if self.ranks[a]<self.ranks[b]{std::mem::swap(&mut a,&mut b);}self.parents[b]=a;if self.ranks[a]==self.ranks[b]{self.ranks[a]+=1;}}
}
impl Mesh {
    /// Identify edge-connected face components; no surface welding or movement.
    /// Faces sharing a vertex alone remain separate. Faces that intersect in space
    /// but don't share an indexed edge also remain separate. A nonmanifold edge
    /// connects all incident faces and is reported, not silently repaired.
    pub fn propose_separation(&self)->Result<SeparationProposal,SeparationError>{
        self.validate().map_err(SeparationError::InvalidMesh)?;
        let mut uses:HashMap<(usize,usize),Vec<usize>>=HashMap::new();
        for (i,&[a,b,c]) in self.triangles.iter().enumerate(){
            if a==b || b==c || c==a || self.degenerate([a,b,c]) {return Err(SeparationError::InvalidMesh(MeshError::DegenerateFace(i)));}
            for (u,v) in [(a,b),(b,c),(c,a)]{uses.entry(edge(u,v)).or_default().push(i);}
        }
        let mut groups=UnionFind::new(self.triangles.len());
        for faces in uses.values(){for &f in faces.iter().skip(1){groups.join(faces[0],f);}}
        let mut by_root:BTreeMap<usize,Vec<usize>>=BTreeMap::new();
        for f in 0..self.triangles.len(){by_root.entry(groups.root(f)).or_default().push(f);}
        let mut components=Vec::new();
        for face_ids in by_root.into_values(){
            let mut vertices=BTreeSet::new();let mut component_edges=BTreeSet::new();
            for &f in &face_ids{let [a,b,c]=self.triangles[f];vertices.extend([a,b,c]);for (u,v) in [(a,b),(b,c),(c,a)]{component_edges.insert(edge(u,v));}}
            let boundary_edges=component_edges.iter().filter(|e|uses[*e].len()==1).count();
            let nonmanifold_edges=component_edges.iter().filter(|e|uses[*e].len()>2).count();
            components.push(SurfaceComponent{face_ids,vertex_ids:vertices.into_iter().collect(),boundary_edges,nonmanifold_edges});
        }
        components.sort_by_key(|c|c.face_ids[0]);
        Ok(SeparationProposal{source:self.clone(),components})
    }
}
#[cfg(test)]
mod tests {
 use super::*;use crate::Point3;
 fn scene()->Mesh {Mesh{vertices:vec![Point3{x:0.,y:0.,z:0.},Point3{x:1.,y:0.,z:0.},Point3{x:0.,y:1.,z:0.},Point3{x:1.,y:1.,z:0.},Point3{x:2.,y:0.,z:0.},Point3{x:2.,y:1.,z:0.}],triangles:vec![[0,1,2],[1,3,2],[1,4,5]]}}
 #[test] fn separates_edge_connected_and_preserves_coordinates(){let m=scene();let before=m.clone();let p=m.propose_separation().unwrap();assert_eq!(p.components.len(),2);assert_eq!(p.components[0].face_ids,vec![0,1]);let a=p.extract(&m,0).unwrap();assert_eq!(a.source_face_ids,vec![0,1]);for (i,&old) in a.source_vertex_ids.iter().enumerate(){assert_eq!(a.mesh.vertices[i],m.vertices[old]);}assert_eq!(m,before);}
 #[test] fn shared_vertex_only_is_not_welded(){let m=scene();let p=m.propose_separation().unwrap();assert!(p.components[0].vertex_ids.contains(&1));assert!(p.components[1].vertex_ids.contains(&1));}
 #[test] fn stale_and_bad_component_rejected(){let mut m=scene();let p=m.propose_separation().unwrap();assert_eq!(p.extract(&m,2),Err(SeparationError::InvalidComponent(2)));m.vertices[0].x=0.1;assert_eq!(p.extract(&m,0),Err(SeparationError::SourceChanged));}
 #[test] fn nonmanifold_component_is_reported(){let mut m=scene();m.triangles.push([1,2,4]);let p=m.propose_separation().unwrap();assert_eq!(p.components[0].nonmanifold_edges,1);}
 #[test] fn degenerate_face_fails_without_change(){let mut m=scene();m.triangles.push([0,0,1]);let copy=m.clone();assert!(matches!(m.propose_separation(),Err(SeparationError::InvalidMesh(MeshError::DegenerateFace(3)))));assert_eq!(m,copy);}
}
