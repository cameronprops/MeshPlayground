//! Conservative subdivision of one explicitly chosen boundary edge.
//! An inserted floating-point midpoint can introduce numerical rounding.
use crate::{edge,Mesh,MeshError,Point3};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubdivisionError {
    InvalidMesh(MeshError), InvalidEdge(usize,usize), NotBoundary(usize,usize),
    DegenerateFace(usize), UnrepresentableMidpoint, SourceChanged,
}
/// Preview of a single edge split. No original vertex moves, but one original
/// face is replaced by two faces and a midpoint is inserted.
#[derive(Clone, Debug)]
pub struct BoundarySubdivisionProposal {
    source:Mesh,
    pub edge:(usize,usize),
    pub source_face:usize,
    pub midpoint:Point3,
    /// The largest axis deviation from the exact real-valued midpoint is not
    /// calculated. As with all f64 arithmetic, rounding remains possible.
    replacement:[[usize;3];2],
}
impl BoundarySubdivisionProposal {
    pub fn preview_faces(&self)->[[usize;3];2] {self.replacement}
    pub fn apply(&self,mesh:&Mesh)->Result<Mesh,SubdivisionError>{
        if mesh!=&self.source{return Err(SubdivisionError::SourceChanged);}
        let mut result=mesh.clone();result.vertices.push(self.midpoint);
        result.triangles[self.source_face]=self.replacement[0];result.triangles.push(self.replacement[1]);
        Ok(result)
    }
}
impl Mesh {
    /// Propose inserting one midpoint on an existing boundary edge.
    /// Only one incident face is split. No fairing or smoothing is applied.
    pub fn propose_boundary_edge_subdivision(&self,a:usize,b:usize)->Result<BoundarySubdivisionProposal,SubdivisionError>{
        self.validate().map_err(SubdivisionError::InvalidMesh)?;
        if a>=self.vertices.len() || b>=self.vertices.len() || a==b {return Err(SubdivisionError::InvalidEdge(a,b));}
        let wanted=edge(a,b);let mut incidence=Vec::new();
        for (i,&[x,y,z]) in self.triangles.iter().enumerate(){
            for (u,v,w) in [(x,y,z),(y,z,x),(z,x,y)] {
                if edge(u,v)==wanted {incidence.push((i,u,v,w));}
            }
        }
        if incidence.len()!=1{return Err(SubdivisionError::NotBoundary(wanted.0,wanted.1));}
        let (face,u,v,w)=incidence[0];
        if self.degenerate(self.triangles[face]){return Err(SubdivisionError::DegenerateFace(face));}
        let p=self.vertices[u];let q=self.vertices[v];
        let midpoint=Point3{x:p.x*0.5+q.x*0.5,y:p.y*0.5+q.y*0.5,z:p.z*0.5+q.z*0.5};
        if !midpoint.finite() || midpoint==p || midpoint==q {return Err(SubdivisionError::UnrepresentableMidpoint);}
        let n=self.vertices.len();let replacement=[[u,n,w],[n,v,w]];
        let area1=midpoint.sub(p).cross(self.vertices[w].sub(p)).magnitude2();
        let area2=q.sub(midpoint).cross(self.vertices[w].sub(midpoint)).magnitude2();
        if !area1.is_finite() || !area2.is_finite() || area1==0.0 || area2==0.0{return Err(SubdivisionError::UnrepresentableMidpoint);}
        Ok(BoundarySubdivisionProposal{source:self.clone(),edge:wanted,source_face:face,midpoint,replacement})
    }
}
#[cfg(test)]
mod tests {
 use super::*;
 fn mesh()->Mesh {Mesh{vertices:vec![Point3{x:0.,y:0.,z:0.},Point3{x:2.,y:0.,z:0.},Point3{x:0.,y:2.,z:0.},Point3{x:2.,y:2.,z:0.}],triangles:vec![[0,1,2],[1,3,2]]}}
 #[test] fn split_preserves_original_and_orientation(){let m=mesh();let before=m.clone();let p=m.propose_boundary_edge_subdivision(0,1).unwrap();let out=p.apply(&m).unwrap();assert_eq!(m,before);assert_eq!(&out.vertices[..m.vertices.len()],m.vertices.as_slice());assert_eq!(out.vertices[4],Point3{x:1.,y:0.,z:0.});assert_eq!(out.triangles,vec![[0,4,2],[1,3,2],[4,1,2]]);assert_eq!(out.diagnose().unwrap().inconsistent_edges,0);assert_eq!(out.diagnose().unwrap().boundary_edges,5);}
 #[test] fn reject_interior_and_missing_edges(){let m=mesh();assert!(matches!(m.propose_boundary_edge_subdivision(1,2),Err(SubdivisionError::NotBoundary(..))));assert!(matches!(m.propose_boundary_edge_subdivision(0,3),Err(SubdivisionError::NotBoundary(..))));}
 #[test] fn source_change_invalidates_preview(){let mut m=mesh();let p=m.propose_boundary_edge_subdivision(0,1).unwrap();m.vertices[0].z=0.1;assert_eq!(p.apply(&m),Err(SubdivisionError::SourceChanged));}
 #[test] fn too_small_to_represent_rejected(){let mut m=mesh();m.vertices[0].x=1.;m.vertices[1].x=f64::from_bits(1f64.to_bits()+1);assert!(matches!(m.propose_boundary_edge_subdivision(0,1),Err(SubdivisionError::UnrepresentableMidpoint)));}
}
