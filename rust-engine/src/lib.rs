//! Offline, indexed triangle mesh primitives.
//! Repair functions preserve existing vertex coordinates. STL import can weld
//! bit-identical coordinates only. No floating-point tolerance welding is done.
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
    /// Remove exact zero-area and duplicate triangles within the selected face mask.
    /// Face IDs in the mask refer to the original mesh. Vertices are not moved or removed.
    /// Unselected duplicates are preserved. If a selected face duplicates an unselected
    /// face, the unselected face is retained regardless of ordering.
    pub fn cleanup(&mut self, mask: Option<&[bool]>) -> Result<CleanupReport,MeshError> {
        self.validate()?;
        if mask.is_some_and(|m|m.len()!=self.triangles.len()) {return Err(MeshError::MaskLength);}
        let mut seen=HashSet::new();
        if let Some(m)=mask { for (i,&f) in self.triangles.iter().enumerate() {if !m[i] {seen.insert(canonical_face(f));}} }
        let mut kept=Vec::with_capacity(self.triangles.len());let mut result=CleanupReport::default();
        for (i,&f) in self.triangles.iter().enumerate() {
            if mask.is_some_and(|m|!m[i]) {kept.push(f);continue;}
            if self.degenerate(f) {result.removed_degenerate+=1;continue;}
            if !seen.insert(canonical_face(f)) {result.removed_duplicate+=1;continue;}
            kept.push(f);
        }
        self.triangles=kept;Ok(result)
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
}
