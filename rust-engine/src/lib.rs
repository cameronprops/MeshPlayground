//! Offline, indexed triangle mesh primitives.
//! Repair functions preserve existing vertex coordinates. STL import can weld
//! bit-identical coordinates only. No floating-point tolerance welding is done.
pub mod patch;
pub mod orient;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::io::{self, Read, Write};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point3 { pub x: f64, pub y: f64, pub z: f64 }
impl Point3 {
    fn finite(self) -> bool { self.x.is_finite() && self.y.is_finite() && self.z.is_finite() }
    fn sub(self, other: Self) -> Self { Self { x:self.x-other.x, y:self.y-other.y, z:self.z-other.z } }
    fn cross(self, other: Self) -> Self { Self { x:self.y*other.z-self.z*other.y, y:self.z*other.x-self.x*other.z, z:self.x*other.y-self.y*other.x } }
    fn magnitude2(self) -> f64 { self.x*self.x+self.y*self.y+self.z*self.z }
    fn dot(self, other: Self) -> f64 { self.x*other.x+self.y*other.y+self.z*other.z }
    fn add(self, other: Self) -> Self { Self{x:self.x+other.x,y:self.y+other.y,z:self.z+other.z} }
    fn scale(self, s:f64) -> Self { Self{x:self.x*s,y:self.y*s,z:self.z*s} }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh { pub vertices: Vec<Point3>, pub triangles: Vec<[usize; 3]> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshError { InvalidVertex(usize), InvalidIndex(usize), MaskLength, TooManyTriangles }
impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}", self) }
}
impl std::error::Error for MeshError {}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub vertices: usize, pub triangles: usize, pub boundary_edges: usize,
    pub nonmanifold_edges: usize, pub inconsistent_edges: usize,
    pub degenerate_triangles: usize, pub duplicate_triangles: usize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CleanupReport { pub removed_degenerate: usize, pub removed_duplicate: usize }

/// Explicit indexed element selection. IDs refer to the current mesh revision.
/// Edge endpoints are canonicalized by `select_edge`; selections do not mutate geometry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub vertices: HashSet<usize>,
    pub edges: HashSet<(usize,usize)>,
    pub faces: HashSet<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element { Vertex(usize), Edge(usize,usize), Face(usize) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionAction { Replace, Add, Remove, Toggle }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickMode { Vertex, Edge, Face }
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray { pub origin: Point3, pub direction: Point3 }
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickHit { pub element: Element, pub position: Point3, pub ray_parameter: f64 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickError { InvalidMesh(MeshError), InvalidRay, InvalidTolerance }
impl Selection {
    pub fn select_edge(&mut self, a:usize, b:usize) { self.edges.insert(edge(a,b)); }
    /// Modify selection atomically: invalid hits leave the prior selection intact.
    pub fn apply(&mut self, mesh:&Mesh, element:Element, action:SelectionAction) -> Result<(),SelectionError> {
        mesh.validate().map_err(SelectionError::InvalidMesh)?;
        match element {
            Element::Vertex(v) if v>=mesh.vertices.len()=>return Err(SelectionError::InvalidVertex(v)),
            Element::Face(f) if f>=mesh.triangles.len()=>return Err(SelectionError::InvalidFace(f)),
            Element::Edge(a,b) if !mesh.triangles.iter().any(|f| [(f[0],f[1]),(f[1],f[2]),(f[2],f[0])].iter().any(|&(u,v)|edge(u,v)==edge(a,b)))=>return Err(SelectionError::InvalidEdge(a,b)),
            _=>{}
        }
        if action==SelectionAction::Replace {*self=Self::default();}
        fn update<T:std::hash::Hash+Eq>(set:&mut HashSet<T>, value:T, action:SelectionAction) {
            match action {
                SelectionAction::Replace | SelectionAction::Add=>{set.insert(value);},
                SelectionAction::Remove=>{set.remove(&value);},
                SelectionAction::Toggle=>{if !set.remove(&value){set.insert(value);}},
            }
        }
        match element {Element::Vertex(v)=>update(&mut self.vertices,v,action),Element::Edge(a,b)=>update(&mut self.edges,edge(a,b),action),Element::Face(f)=>update(&mut self.faces,f,action)}
        Ok(())
    }
}
/// A connected open boundary. A closed loop has every vertex of degree two.
/// Branched and open chains are reported but never represented as valid loops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    pub edges: Vec<(usize,usize)>,
    pub vertices: Vec<usize>,
    pub adjacent_faces: Vec<usize>,
    pub closed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionError { InvalidMesh(MeshError), InvalidVertex(usize), InvalidEdge(usize,usize), InvalidFace(usize) }
/// Result of explicitly expanding selected edges to closed boundary components.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoundaryExpansion { pub closed_boundaries: usize, pub added_edges: usize, pub skipped_non_closed: usize }

/// A reviewable cleanup proposal. The source snapshot prevents applying a
/// proposal to a different mesh or a mesh that changed after inspection.
/// Face IDs refer to the source mesh, not the reindexed result.
#[derive(Clone, Debug)]
pub struct CleanupProposal {
    source: Mesh,
    remove_face: Vec<bool>,
    pub report: CleanupReport,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalError { InvalidMesh(MeshError), SourceChanged }
impl From<MeshError> for ProposalError {
    fn from(value: MeshError) -> Self { Self::InvalidMesh(value) }
}
impl CleanupProposal {
    /// Face IDs marked for removal. Unlisted faces remain in their original order.
    pub fn removed_face_ids(&self) -> impl Iterator<Item=usize> + '_ {
        self.remove_face.iter().enumerate().filter_map(|(i,&remove)|remove.then_some(i))
    }
    /// Apply only to the exact reviewed snapshot and return a new mesh. The
    /// caller's original mesh remains available for comparison or undo.
    pub fn apply(&self, source: &Mesh) -> Result<Mesh, ProposalError> {
        source.validate()?;
        if source != &self.source { return Err(ProposalError::SourceChanged); }
        let triangles=source.triangles.iter().enumerate()
            .filter_map(|(i,&f)|(!self.remove_face[i]).then_some(f)).collect();
        Ok(Mesh { vertices: source.vertices.clone(), triangles })
    }
}
#[derive(Clone, Copy, Debug)]
struct EdgeUse { from: usize, to: usize }
fn edge(a: usize, b: usize) -> (usize, usize) { if a < b {(a,b)} else {(b,a)} }
fn canonical_face(mut f:[usize;3])->[usize;3] { f.sort_unstable(); f }
impl Mesh {
    pub fn validate(&self) -> Result<(), MeshError> {
        for (i,p) in self.vertices.iter().enumerate() { if !p.finite() { return Err(MeshError::InvalidVertex(i)); } }
        for (i,f) in self.triangles.iter().enumerate() { if f.iter().any(|&v|v>=self.vertices.len()) { return Err(MeshError::InvalidIndex(i)); } }
        Ok(())
    }
    /// Hit-test the nearest frontmost triangle along a ray, without backface culling.
    /// For vertex/edge mode, choose the closest element of that triangle if it is
    /// within `tolerance` mesh units of the hit point. This is a geometric baseline,
    /// not a pixel-space hit test; the viewport must convert its screen tolerance.
    /// Linear scan: not yet suitable for dense scans without a spatial index.
    pub fn pick(&self, ray:Ray, mode:PickMode, tolerance:f64) -> Result<Option<PickHit>,PickError> {
        self.validate().map_err(PickError::InvalidMesh)?;
        if !ray.origin.finite() || !ray.direction.finite() || ray.direction.magnitude2()==0.0 || !ray.direction.magnitude2().is_finite() {return Err(PickError::InvalidRay);}
        if !tolerance.is_finite() || tolerance<0.0 {return Err(PickError::InvalidTolerance);}
        let mut nearest:Option<(usize,f64,Point3)>=None;
        for (face,&[ia,ib,ic]) in self.triangles.iter().enumerate() {
            let (a,b,c)=(self.vertices[ia],self.vertices[ib],self.vertices[ic]);
            let (e1,e2)=(b.sub(a),c.sub(a));let p=ray.direction.cross(e2);let det=e1.dot(p);
            // Relative parallel test avoids treating a tiny but valid triangle as degenerate.
            let scale=e1.magnitude2().sqrt()*e2.magnitude2().sqrt()*ray.direction.magnitude2().sqrt();
            if !scale.is_finite() || scale==0.0 || det.abs()<=scale*1e-14 {continue;}
            let inv=1.0/det;let s=ray.origin.sub(a);let u=s.dot(p)*inv;
            if !(0.0..=1.0).contains(&u){continue;}
            let q=s.cross(e1);let v=ray.direction.dot(q)*inv;
            if v<0.0 || u+v>1.0 {continue;}
            let t=e2.dot(q)*inv;
            if !t.is_finite() || t<0.0 || nearest.as_ref().is_some_and(|(_,old,_)|t>=*old){continue;}
            let point=ray.origin.add(ray.direction.scale(t));if !point.finite(){continue;}
            nearest=Some((face,t,point));
        }
        let Some((face,t,position))=nearest else{return Ok(None)};
        let f=self.triangles[face];
        let element=match mode {
            PickMode::Face=>Element::Face(face),
            PickMode::Vertex=>{
                let (id,distance)=f.into_iter().map(|i|(i,self.vertices[i].sub(position).magnitude2())).min_by(|a,b|a.1.total_cmp(&b.1)).unwrap();
                if distance.sqrt()>tolerance{return Ok(None);} Element::Vertex(id)
            },
            PickMode::Edge=>{
                let (mut best,mut distance)=((0,0),f64::INFINITY);
                for (a,b) in [(f[0],f[1]),(f[1],f[2]),(f[2],f[0])] {
                    let start=self.vertices[a];let line=self.vertices[b].sub(start);let length=line.magnitude2();
                    if length==0.0 || !length.is_finite(){continue;}
                    let along=(position.sub(start).dot(line)/length).clamp(0.0,1.0);
                    let d=position.sub(start.add(line.scale(along))).magnitude2();
                    if d<distance {distance=d;best=edge(a,b);}
                }
                if distance.sqrt()>tolerance{return Ok(None);}Element::Edge(best.0,best.1)
            }
        };
        Ok(Some(PickHit {element,position,ray_parameter:t}))
    }
    fn degenerate(&self, f: [usize;3]) -> bool {
        if f[0]==f[1] || f[1]==f[2] || f[2]==f[0] { return true; }
        let [a,b,c]=f.map(|i|self.vertices[i]);
        b.sub(a).cross(c.sub(a)).magnitude2() == 0.0
    }
    fn edge_uses(&self) -> BTreeMap<(usize,usize),Vec<EdgeUse>> {
        let mut edges:BTreeMap<_,Vec<EdgeUse>>=BTreeMap::new();
        for &[a,b,c] in &self.triangles {
            for (from,to) in [(a,b),(b,c),(c,a)] {
                edges.entry(edge(from,to)).or_default().push(EdgeUse{from,to});
            }
        }
        edges
    }
    /// Inspect open-boundary components touching selected vertices, edges, or faces.
    /// Selecting any element discovers the whole connected boundary for review;
    /// it does not authorize or apply a repair. A face matches only its own edges.
    pub fn selected_boundaries(&self, selection:&Selection) -> Result<Vec<Boundary>,SelectionError> {
        self.validate().map_err(SelectionError::InvalidMesh)?;
        let edges=self.edge_uses();
        for &v in &selection.vertices {if v>=self.vertices.len(){return Err(SelectionError::InvalidVertex(v));}}
        for &(a,b) in &selection.edges {if !edges.contains_key(&edge(a,b)){return Err(SelectionError::InvalidEdge(a,b));}}
        for &f in &selection.faces {if f>=self.triangles.len(){return Err(SelectionError::InvalidFace(f));}}
        let mut adjacency:BTreeMap<usize,Vec<usize>>=BTreeMap::new();
        let mut boundary_face:BTreeMap<(usize,usize),usize>=BTreeMap::new();
        // Keep stable face IDs alongside boundary edges without depending on face winding.
        for (face,&[a,b,c]) in self.triangles.iter().enumerate() {
            for (u,v) in [(a,b),(b,c),(c,a)] {
                let e=edge(u,v);
                if edges.get(&e).is_some_and(|uses|uses.len()==1) {
                    boundary_face.insert(e,face);
                }
            }
        }
        for &(a,b) in boundary_face.keys() {
            adjacency.entry(a).or_default().push(b);adjacency.entry(b).or_default().push(a);
        }
        let mut visited=HashSet::new();let mut result=Vec::new();
        for &start in adjacency.keys() {
            if !visited.insert(start) {continue;}
            let mut stack=vec![start];let mut vertices=Vec::new();let mut component_edges=HashSet::new();
            while let Some(v)=stack.pop() {
                vertices.push(v);
                for &next in &adjacency[&v] {
                    component_edges.insert(edge(v,next));
                    if visited.insert(next) {stack.push(next);}
                }
            }
            vertices.sort_unstable();let mut component_edges:Vec<_>=component_edges.into_iter().collect();component_edges.sort_unstable();
            let mut faces:Vec<_>=component_edges.iter().map(|e|boundary_face[e]).collect();faces.sort_unstable();faces.dedup();
            let matches=vertices.iter().any(|v|selection.vertices.contains(v)) || component_edges.iter().any(|&(a,b)|selection.edges.contains(&(a,b)) || selection.edges.contains(&(b,a))) || faces.iter().any(|f|selection.faces.contains(f));
            if matches {
                let closed=vertices.len()>=3 && vertices.iter().all(|v|adjacency[v].len()==2);
                result.push(Boundary {vertices,edges:component_edges,adjacent_faces:faces,closed});
            }
        }
        Ok(result)
    }
    /// Expand only the selected boundary edges to their complete closed loops.
    /// This is an explicit selection command, never a geometry operation.
    /// Interior edges stay selected; branched components are reported and left
    /// unchanged. Invalid selection fails before any selection is mutated.
    pub fn expand_selected_boundary_edges(&self, selection:&mut Selection) -> Result<BoundaryExpansion,SelectionError> {
        let mut seed=Selection::default();
        seed.edges=selection.edges.clone();
        let boundaries=self.selected_boundaries(&seed)?;
        let mut result=BoundaryExpansion::default();
        for boundary in boundaries {
            if !boundary.closed {result.skipped_non_closed+=1;continue;}
            result.closed_boundaries+=1;
            for (a,b) in boundary.edges {
                if selection.edges.insert(edge(a,b)) {result.added_edges+=1;}
            }
        }
        Ok(result)
    }
    /// Read-only topology report. No epsilon or model-size assumptions.
    pub fn diagnose(&self) -> Result<Diagnostics,MeshError> {
        self.validate()?;
        let mut result=Diagnostics {vertices:self.vertices.len(),triangles:self.triangles.len(),..Default::default()};
        let mut seen=HashSet::new();
        for &f in &self.triangles {
            if self.degenerate(f) { result.degenerate_triangles+=1; }
            if !seen.insert(canonical_face(f)) { result.duplicate_triangles+=1; }
        }
        for uses in self.edge_uses().values() {
            match uses.len() { 1=>result.boundary_edges+=1, 2=>{
                if uses[0].from==uses[1].from && uses[0].to==uses[1].to {result.inconsistent_edges+=1;}
            }, n if n>2=>result.nonmanifold_edges+=1, _=>{} }
        }
        Ok(result)
    }
    /// Analyze a cleanup without mutating the mesh. Only exact duplicates and
    /// exactly zero-area triangles are candidates; no holes are filled.
    pub fn propose_cleanup(&self, mask: Option<&[bool]>) -> Result<CleanupProposal,MeshError> {
        self.validate()?;
        if mask.is_some_and(|m|m.len()!=self.triangles.len()) {return Err(MeshError::MaskLength);}
        let mut seen=HashSet::new();
        if let Some(m)=mask { for (i,&f) in self.triangles.iter().enumerate() {if !m[i] {seen.insert(canonical_face(f));}} }
        let mut remove_face=vec![false;self.triangles.len()];
        let mut report=CleanupReport::default();
        for (i,&f) in self.triangles.iter().enumerate() {
            if mask.is_some_and(|m|!m[i]) {continue;}
            if self.degenerate(f) {remove_face[i]=true;report.removed_degenerate+=1;continue;}
            if !seen.insert(canonical_face(f)) {remove_face[i]=true;report.removed_duplicate+=1;}
        }
        Ok(CleanupProposal {source:self.clone(),remove_face,report})
    }
    /// Remove exact zero-area and duplicate triangles within the selected face mask.
    /// Face IDs in the mask refer to the original mesh. Vertices are not moved or removed.
    /// Unselected duplicates are preserved. If a selected face duplicates an unselected
    /// face, the unselected face is retained regardless of ordering.
    pub fn cleanup(&mut self, mask: Option<&[bool]>) -> Result<CleanupReport,MeshError> {
        let proposal=self.propose_cleanup(mask)?;
        self.triangles=self.triangles.iter().enumerate()
            .filter_map(|(i,&f)|(!proposal.remove_face[i]).then_some(f)).collect();
        Ok(proposal.report)
    }
}

/// Import binary STL, welding only exactly equal f32 coordinates. ASCII STL is not yet supported.
/// STL has no unit metadata; caller must preserve or specify its intended units.
pub fn read_binary_stl<R: Read>(mut reader:R) -> io::Result<Mesh> {
    let mut header=[0u8;84];reader.read_exact(&mut header)?;
    let count=u32::from_le_bytes(header[80..84].try_into().unwrap()) as usize;
    // Avoid allowing a malicious header to trigger an enormous upfront allocation.
    let mut vertices=Vec::new();let mut triangles=Vec::new();let mut unique:HashMap<[u32;3],usize>=HashMap::new();
    let mut record=[0u8;50];
    for _ in 0..count {
        reader.read_exact(&mut record)?;let mut face=[0;3];
        for j in 0..3 {
            let mut key=[0;3];let mut xyz=[0.;3];
            for k in 0..3 {
                let o=12+j*12+k*4;let value=f32::from_le_bytes(record[o..o+4].try_into().unwrap());
                if !value.is_finite(){return Err(io::Error::new(io::ErrorKind::InvalidData,"non-finite coordinate"));}
                // Normalize signed zero because it represents the same exact coordinate.
                key[k]=if value==0.0 {0} else {value.to_bits()};xyz[k]=value as f64;
            }
            face[j]=*unique.entry(key).or_insert_with(|| {let i=vertices.len();vertices.push(Point3{x:xyz[0],y:xyz[1],z:xyz[2]});i});
        }
        triangles.push(face);
    }
    Ok(Mesh{vertices,triangles})
}
/// Binary STL stores coordinates as f32, so f64 input can round on export.
pub fn write_binary_stl<W:Write>(mesh:&Mesh,mut writer:W)->io::Result<()> {
    mesh.validate().map_err(|e|io::Error::new(io::ErrorKind::InvalidData,e))?;
    let count=u32::try_from(mesh.triangles.len()).map_err(|_|io::Error::new(io::ErrorKind::InvalidData,MeshError::TooManyTriangles))?;
    // Check all conversions before writing any bytes.
    if mesh.vertices.iter().any(|p|[p.x,p.y,p.z].into_iter().any(|v|!(v as f32).is_finite())) {return Err(io::Error::new(io::ErrorKind::InvalidData,"coordinate out of STL range"));}
    writer.write_all(&[0u8;80])?;writer.write_all(&count.to_le_bytes())?;
    for &f in &mesh.triangles {
        let [a,b,c]=f.map(|i|mesh.vertices[i]);let normal=b.sub(a).cross(c.sub(a));let length=normal.magnitude2().sqrt();
        let n=if length.is_finite() && length>0. {Point3{x:normal.x/length,y:normal.y/length,z:normal.z/length}}else{Point3{x:0.,y:0.,z:0.}};
        for v in [n.x,n.y,n.z,a.x,a.y,a.z,b.x,b.y,b.z,c.x,c.y,c.z] {writer.write_all(&(v as f32).to_le_bytes())?;}
        writer.write_all(&0u16.to_le_bytes())?;
    } Ok(())
}

#[cfg(test)]
mod tests {
 use super::*;
 fn mesh()->Mesh {Mesh{vertices:vec![Point3{x:0.,y:0.,z:0.},Point3{x:1.,y:0.,z:0.},Point3{x:1.,y:1.,z:0.},Point3{x:0.,y:1.,z:0.}],triangles:vec![[0,1,2],[0,2,3]]}}
 #[test] fn open_plate_diagnostics(){let d=mesh().diagnose().unwrap();assert_eq!(d.boundary_edges,4);assert_eq!(d.inconsistent_edges,0);}
 #[test] fn selected_cleanup_preserves_unselected_face_and_positions(){let mut m=mesh();m.triangles.push([2,1,0]);let vertices=m.vertices.clone();let r=m.cleanup(Some(&[true,false,false])).unwrap();assert_eq!(r.removed_duplicate,1);assert_eq!(m.triangles,vec![[0,2,3],[2,1,0]]);assert_eq!(vertices,m.vertices);}
 #[test] fn invalid_mask_does_not_modify(){let mut m=mesh();let before=m.clone();assert_eq!(m.cleanup(Some(&[true])),Err(MeshError::MaskLength));assert_eq!(m,before);}
 #[test] fn stl_round_trip(){let original=mesh();let mut bytes=Vec::new();write_binary_stl(&original,&mut bytes).unwrap();let restored=read_binary_stl(bytes.as_slice()).unwrap();assert_eq!(restored.diagnose().unwrap().boundary_edges,4);assert_eq!(restored.vertices.len(),4);}
 #[test] fn truncated_stl_is_rejected(){let m=mesh();let mut b=Vec::new();write_binary_stl(&m,&mut b).unwrap();b.pop();assert!(read_binary_stl(b.as_slice()).is_err());}
 #[test] fn inconsistent_edge_is_reported(){let mut m=mesh();m.triangles[1]=[0,3,2];assert_eq!(m.diagnose().unwrap().inconsistent_edges,1);}
 #[test] fn invalid_indices_reported(){let mut m=mesh();m.triangles.push([0,1,99]);assert_eq!(m.diagnose(),Err(MeshError::InvalidIndex(2)));}
 #[test] fn proposal_leaves_source_untouched_and_lists_face_ids(){
  let mut original=mesh();original.triangles.push([2,1,0]);
  let before=original.clone();let proposal=original.propose_cleanup(None).unwrap();
  assert_eq!(original,before);assert_eq!(proposal.removed_face_ids().collect::<Vec<_>>(),vec![2]);
  let result=proposal.apply(&original).unwrap();assert_eq!(result.vertices,original.vertices);
  assert_eq!(result.triangles.len(),2);assert_eq!(original.triangles.len(),3);
 }
 #[test] fn stale_proposals_rejected(){
  let original=mesh();let proposal=original.propose_cleanup(None).unwrap();let mut changed=original.clone();
  changed.vertices[0].x=0.000001;
  assert_eq!(proposal.apply(&changed),Err(ProposalError::SourceChanged));
 }
 #[test] fn selection_never_removes_unselected_duplicate(){
  let mut original=mesh();original.triangles.push([2,1,0]);
  let proposal=original.propose_cleanup(Some(&[true,false,false])).unwrap();
  assert_eq!(proposal.removed_face_ids().collect::<Vec<_>>(),vec![0]);
  let result=proposal.apply(&original).unwrap();assert_eq!(result.triangles,vec![[0,2,3],[2,1,0]]);
 }
 #[test] fn element_selection_finds_same_boundary_without_changing_mesh(){
  let original=mesh();let before=original.clone();
  let mut by_vertex=Selection::default();by_vertex.vertices.insert(0);
  let mut by_edge=Selection::default();by_edge.select_edge(1,0);
  let mut by_face=Selection::default();by_face.faces.insert(0);
  let a=original.selected_boundaries(&by_vertex).unwrap();
  assert_eq!(a,original.selected_boundaries(&by_edge).unwrap());
  assert_eq!(a,original.selected_boundaries(&by_face).unwrap());
  assert_eq!(a.len(),1);assert!(a[0].closed);assert_eq!(a[0].edges.len(),4);
  assert_eq!(original,before);
 }
 #[test] fn interior_edge_selection_does_not_select_boundary(){
  let original=mesh();let mut selection=Selection::default();selection.select_edge(0,2);
  assert!(original.selected_boundaries(&selection).unwrap().is_empty());
 }
 #[test] fn invalid_selection_is_rejected(){
  let original=mesh();let mut selection=Selection::default();selection.faces.insert(99);
  assert_eq!(original.selected_boundaries(&selection),Err(SelectionError::InvalidFace(99)));
 }
 #[test] fn branched_boundary_is_not_called_a_loop(){
  let mut original=mesh();original.vertices.extend([Point3{x:2.,y:0.,z:0.},Point3{x:2.,y:1.,z:0.}]);
  original.triangles.push([2,4,5]);let mut selection=Selection::default();selection.vertices.insert(2);
  let boundaries=original.selected_boundaries(&selection).unwrap();assert_eq!(boundaries.len(),1);assert!(!boundaries[0].closed);
 }
 #[test] fn explicit_boundary_expansion_keeps_other_selections_and_geometry(){
  let original=mesh();let before=original.clone();let mut s=Selection::default();
  s.select_edge(1,0);s.vertices.insert(2);s.faces.insert(0);
  let result=original.expand_selected_boundary_edges(&mut s).unwrap();
  assert_eq!(result,BoundaryExpansion{closed_boundaries:1,added_edges:3,skipped_non_closed:0});
  assert_eq!(s.edges.len(),4);assert!(s.vertices.contains(&2));assert!(s.faces.contains(&0));
  assert_eq!(original,before);
 }
 #[test] fn expansion_does_not_guess_from_interior_edge_or_vertex(){
  let original=mesh();let mut s=Selection::default();s.select_edge(0,2);s.vertices.insert(0);
  let before=s.clone();assert_eq!(original.expand_selected_boundary_edges(&mut s).unwrap(),BoundaryExpansion::default());assert_eq!(s,before);
 }
 #[test] fn expansion_skips_branched_boundary(){
  let mut m=mesh();m.vertices.extend([Point3{x:2.,y:0.,z:0.},Point3{x:2.,y:1.,z:0.}]);m.triangles.push([2,4,5]);
  let mut s=Selection::default();s.select_edge(4,5);let before=s.clone();
  let report=m.expand_selected_boundary_edges(&mut s).unwrap();
  assert_eq!(report.skipped_non_closed,1);assert_eq!(s,before);
 }
 #[test] fn expansion_with_invalid_edge_is_atomic(){
  let original=mesh();let mut s=Selection::default();s.select_edge(0,1);s.select_edge(0,99);let before=s.clone();
  assert!(original.expand_selected_boundary_edges(&mut s).is_err());assert_eq!(s,before);
 }
 fn ray(x:f64,y:f64)->Ray {Ray{origin:Point3{x,y,z:2.},direction:Point3{x:0.,y:0.,z:-1.}}}
 #[test] fn pointer_picks_face_edge_and_vertex(){
  let m=mesh();
  assert_eq!(m.pick(ray(0.75,0.25),PickMode::Face,0.).unwrap().unwrap().element,Element::Face(0));
  assert_eq!(m.pick(ray(0.5,0.01),PickMode::Edge,0.02).unwrap().unwrap().element,Element::Edge(0,1));
  assert_eq!(m.pick(ray(0.01,0.01),PickMode::Vertex,0.02).unwrap().unwrap().element,Element::Vertex(0));
  assert!(m.pick(ray(0.5,0.2),PickMode::Vertex,0.02).unwrap().is_none());
  assert!(m.pick(ray(2.,2.),PickMode::Face,0.).unwrap().is_none());
 }
 #[test] fn pointer_picks_front_surface(){
  let mut m=mesh();m.vertices.extend(m.vertices.clone().iter().map(|p|Point3{x:p.x,y:p.y,z:1.}));
  m.triangles.extend([[4,5,6],[4,6,7]]);
  let hit=m.pick(ray(0.75,0.25),PickMode::Face,0.).unwrap().unwrap();
  assert_eq!(hit.element,Element::Face(2));assert_eq!(hit.position.z,1.);
 }
 #[test] fn selection_actions_and_invalid_hit_are_atomic(){
  let m=mesh();let mut s=Selection::default();
  s.apply(&m,Element::Vertex(0),SelectionAction::Replace).unwrap();
  s.apply(&m,Element::Vertex(1),SelectionAction::Add).unwrap();
  s.apply(&m,Element::Vertex(0),SelectionAction::Toggle).unwrap();
  assert_eq!(s.vertices,HashSet::from([1]));
  let before=s.clone();assert!(s.apply(&m,Element::Face(99),SelectionAction::Replace).is_err());assert_eq!(s,before);
  s.apply(&m,Element::Edge(1,0),SelectionAction::Replace).unwrap();assert!(s.vertices.is_empty());assert!(s.edges.contains(&(0,1)));
  s.apply(&m,Element::Edge(0,1),SelectionAction::Remove).unwrap();assert!(s.edges.is_empty());
 }
 #[test] fn invalid_rays_and_tolerances_are_rejected(){
  let m=mesh();assert_eq!(m.pick(Ray{origin:ray(0.,0.).origin,direction:Point3{x:0.,y:0.,z:0.}},PickMode::Face,0.),Err(PickError::InvalidRay));
  assert_eq!(m.pick(ray(0.,0.),PickMode::Face,f64::NAN),Err(PickError::InvalidTolerance));
 }
}
