//! Optional in-memory mesh grouping with explicit edit propagation.
//! This is a mesh-specific slice, not a complete multi-format CAD document.
use std::{collections::BTreeMap,num::NonZeroU32};
use crate::{Mesh,MeshError,CleanupProposal,ProposalError};
use crate::orient::{OrientationProposal,OrientationError};
use crate::patch::{PatchProposal,PatchError};
use crate::subdivide::{BoundarySubdivisionProposal,SubdivisionError};

/// A nonzero, document-local group ID. IDs are never reused in a document.
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct GroupId(NonZeroU32);
impl GroupId {pub fn get(self)->u32{self.0.get()}}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Group {pub name:String}
#[derive(Clone,Debug,PartialEq)]
pub enum DocumentError {
 InvalidMesh(MeshError), InvalidFace(usize), UnknownGroup(GroupId),
 EmptyName, GroupIdExhausted, GroupInUse(GroupId),
 Cleanup(ProposalError), Orientation(OrientationError), Patch(PatchError),
 Subdivision(SubdivisionError),
}
/// A single triangle mesh plus optional per-face group IDs.
/// Unassigned meshes allocate no face-group vector. The vector, when present,
/// uses Option<NonZeroU32>, 4 bytes per triangle on Rust's supported layouts.
/// Mesh editing is exposed only through methods that update groups together.
#[derive(Clone,Debug,PartialEq)]
pub struct MeshDocument {
 mesh:Mesh,groups:BTreeMap<GroupId,Group>,face_groups:Option<Vec<Option<NonZeroU32>>>,next_group:u32,
}
impl MeshDocument {
 pub fn new(mesh:Mesh)->Result<Self,DocumentError>{
  mesh.validate().map_err(DocumentError::InvalidMesh)?;
  Ok(Self{mesh,groups:BTreeMap::new(),face_groups:None,next_group:1})
 }
 pub fn mesh(&self)->&Mesh {&self.mesh}
 pub fn groups(&self)->&BTreeMap<GroupId,Group>{&self.groups}
 /// None means a valid, ungrouped face; invalid face IDs are errors.
 pub fn face_group(&self,face:usize)->Result<Option<GroupId>,DocumentError>{
  if face>=self.mesh.triangles.len(){return Err(DocumentError::InvalidFace(face));}
  Ok(self.face_groups.as_ref().and_then(|groups|groups[face]).map(GroupId))
 }
 pub fn create_group(&mut self,name:&str)->Result<GroupId,DocumentError>{
  let name=name.trim();if name.is_empty(){return Err(DocumentError::EmptyName);}
  let raw=NonZeroU32::new(self.next_group).ok_or(DocumentError::GroupIdExhausted)?;
  self.next_group=self.next_group.checked_add(1).unwrap_or(0);
  let id=GroupId(raw);self.groups.insert(id,Group{name:name.to_string()});Ok(id)
 }
 /// Assign or clear a primary group. All inputs are validated before mutation.
 pub fn assign_faces(&mut self,faces:&[usize],group:Option<GroupId>)->Result<(),DocumentError>{
  if let Some(id)=group {if !self.groups.contains_key(&id){return Err(DocumentError::UnknownGroup(id));}}
  if let Some(&invalid)=faces.iter().find(|&&i|i>=self.mesh.triangles.len()){return Err(DocumentError::InvalidFace(invalid));}
  if faces.is_empty() || (group.is_none()&&self.face_groups.is_none()){return Ok(());}
  let map=self.face_groups.get_or_insert_with(||vec![None;self.mesh.triangles.len()]);
  for &i in faces {map[i]=group.map(|id|id.0);}
  if group.is_none() && map.iter().all(Option::is_none){self.face_groups=None;}
  Ok(())
 }
 /// Deletion is rejected until the caller explicitly clears assigned faces.
 pub fn delete_group(&mut self,id:GroupId)->Result<(),DocumentError>{
  if !self.groups.contains_key(&id){return Err(DocumentError::UnknownGroup(id));}
  if self.face_groups.as_ref().is_some_and(|map|map.contains(&Some(id.0))){return Err(DocumentError::GroupInUse(id));}
  self.groups.remove(&id);Ok(())
 }
 pub fn apply_cleanup(&mut self,proposal:&CleanupProposal)->Result<(),DocumentError>{
  let result=proposal.apply(&self.mesh).map_err(DocumentError::Cleanup)?;
  let mut removed=proposal.removed_face_ids().peekable();
  if let Some(map)=&mut self.face_groups {
   let mut index=0;map.retain(|_|{let discard=removed.peek()==Some(&index);if discard{removed.next();}index+=1;!discard});
   if map.iter().all(Option::is_none){self.face_groups=None;}
  }
  self.mesh=result;Ok(())
 }
 pub fn apply_orientation(&mut self,proposal:&OrientationProposal)->Result<(),DocumentError>{
  let result=proposal.apply(&self.mesh).map_err(DocumentError::Orientation)?;self.mesh=result;Ok(())
 }
 /// The two child faces inherit the original face group.
 pub fn apply_boundary_subdivision(&mut self,proposal:&BoundarySubdivisionProposal)->Result<(),DocumentError>{
  let result=proposal.apply(&self.mesh).map_err(DocumentError::Subdivision)?;
  if let Some(map)=&mut self.face_groups {let id=map[proposal.source_face];map.push(id);}
  self.mesh=result;Ok(())
 }
 /// New patch faces are ungrouped unless an existing group is specified.
 pub fn apply_patch(&mut self,proposal:&PatchProposal,group:Option<GroupId>)->Result<(),DocumentError>{
  if let Some(id)=group {if !self.groups.contains_key(&id){return Err(DocumentError::UnknownGroup(id));}}
  let result=proposal.apply(&self.mesh).map_err(DocumentError::Patch)?;
  result.validate().map_err(DocumentError::InvalidMesh)?;
  let added=result.triangles.len()-self.mesh.triangles.len();
  if added>0 && (self.face_groups.is_some()||group.is_some()) {
   let old_len=self.mesh.triangles.len();let map=self.face_groups.get_or_insert_with(||vec![None;old_len]);
   map.extend(std::iter::repeat(group.map(|id|id.0)).take(added));
  }
  self.mesh=result;Ok(())
 }
}
#[cfg(test)]
mod tests {
 use super::*;use crate::{Point3,Selection};
 fn plane()->Mesh {Mesh{vertices:vec![Point3{x:0.,y:0.,z:0.},Point3{x:1.,y:0.,z:0.},Point3{x:1.,y:1.,z:0.},Point3{x:0.,y:1.,z:0.}],triangles:vec![[0,1,2],[0,2,3]]}}
 #[test] fn compact_optional_storage_and_atomic_assignment(){
  assert_eq!(std::mem::size_of::<Option<NonZeroU32>>(),4);
  let mut d=MeshDocument::new(plane()).unwrap();assert!(d.face_groups.is_none());let id=d.create_group("handle").unwrap();
  let before=d.clone();assert_eq!(d.assign_faces(&[0,99],Some(id)),Err(DocumentError::InvalidFace(99)));assert_eq!(d,before);
  d.assign_faces(&[0],Some(id)).unwrap();assert_eq!(d.face_group(0),Ok(Some(id)));assert_eq!(d.face_group(1),Ok(None));
  assert_eq!(d.delete_group(id),Err(DocumentError::GroupInUse(id)));d.assign_faces(&[0],None).unwrap();assert!(d.face_groups.is_none());d.delete_group(id).unwrap();assert_ne!(d.create_group("other").unwrap(),id);
 }
 #[test] fn groups_do_not_duplicate_or_move_shared_geometry(){let mut d=MeshDocument::new(plane()).unwrap();let before=d.mesh().clone();let a=d.create_group("body").unwrap();let b=d.create_group("handle").unwrap();d.assign_faces(&[0],Some(a)).unwrap();d.assign_faces(&[1],Some(b)).unwrap();assert_eq!(d.mesh(),&before);assert_eq!(d.mesh().diagnose().unwrap().boundary_edges,4);}
 #[test] fn subdivision_inherits_group(){let mut d=MeshDocument::new(plane()).unwrap();let id=d.create_group("rim").unwrap();d.assign_faces(&[0],Some(id)).unwrap();let p=d.mesh().propose_boundary_edge_subdivision(0,1).unwrap();d.apply_boundary_subdivision(&p).unwrap();assert_eq!(d.face_group(0),Ok(Some(id)));assert_eq!(d.face_group(2),Ok(Some(id)));assert_eq!(d.face_group(1),Ok(None));}
 #[test] fn cleanup_remaps_groups(){let mut mesh=plane();mesh.triangles.push([2,1,0]);let mut d=MeshDocument::new(mesh).unwrap();let a=d.create_group("first").unwrap();let b=d.create_group("second").unwrap();d.assign_faces(&[0],Some(a)).unwrap();d.assign_faces(&[1,2],Some(b)).unwrap();let p=d.mesh().propose_cleanup(None).unwrap();d.apply_cleanup(&p).unwrap();assert_eq!(d.mesh().triangles.len(),2);assert_eq!(d.face_group(0),Ok(Some(a)));assert_eq!(d.face_group(1),Ok(Some(b)));}
 #[test] fn orientation_preserves_groups(){let mut m=plane();m.triangles[1]=[0,3,2];let mut d=MeshDocument::new(m).unwrap();let id=d.create_group("body").unwrap();d.assign_faces(&[1],Some(id)).unwrap();let p=d.mesh().propose_consistent_orientation().unwrap();d.apply_orientation(&p).unwrap();assert_eq!(d.face_group(1),Ok(Some(id)));}
 #[test] fn stale_proposal_does_not_alter_groups(){let mut d=MeshDocument::new(plane()).unwrap();let p=d.mesh().propose_boundary_edge_subdivision(0,1).unwrap();d.apply_boundary_subdivision(&p).unwrap();let before=d.clone();assert!(matches!(d.apply_boundary_subdivision(&p),Err(DocumentError::Subdivision(SubdivisionError::SourceChanged))));assert_eq!(d,before);}
 #[test] fn patch_group_is_explicit(){let m=Mesh{vertices:vec![Point3{x:0.,y:0.,z:0.},Point3{x:1.,y:0.,z:0.},Point3{x:1.,y:1.,z:0.},Point3{x:0.,y:1.,z:0.},Point3{x:0.5,y:0.5,z:-1.}],triangles:vec![[0,4,1],[1,4,2],[2,4,3],[3,4,0]]};let mut d=MeshDocument::new(m).unwrap();let id=d.create_group("patch").unwrap();let mut s=Selection::default();for (a,b) in [(0,1),(1,2),(2,3),(3,0)]{s.select_edge(a,b);}let p=d.mesh().propose_flat_patch(&s,0.).unwrap();d.apply_patch(&p,Some(id)).unwrap();assert_eq!(d.face_group(4),Ok(Some(id)));assert_eq!(d.face_group(5),Ok(Some(id)));assert_eq!(d.face_group(0),Ok(None));}
}
