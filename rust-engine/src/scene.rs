//! In-memory scene foundations shared by future mesh and 2D path adapters.
//! This module does not parse or export formats or implement CAD B-reps.
use std::sync::Arc;
use crate::{document::MeshDocument,Point3};
use crate::tolerance::LengthUnit;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum UnitError { UnknownUnit,NonFiniteValue,OutOfRange }
impl LengthUnit {
 /// Convert one value explicitly. Unspecified units are never guessed; a zero
 /// result from a nonzero input is treated as underflow and rejected.
 pub fn convert(self,value:f64,to:LengthUnit)->Result<f64,UnitError>{
  if !value.is_finite(){return Err(UnitError::NonFiniteValue);}
  fn meters(unit:LengthUnit)->Option<f64>{match unit{
   LengthUnit::Microns=>Some(1e-6),LengthUnit::Millimeters=>Some(1e-3),
   LengthUnit::Centimeters=>Some(1e-2),LengthUnit::Meters=>Some(1.),
   LengthUnit::Inches=>Some(0.0254),LengthUnit::Feet=>Some(0.3048),
   LengthUnit::Unspecified=>None,
  }}
  let result=value*(meters(self).ok_or(UnitError::UnknownUnit)?/meters(to).ok_or(UnitError::UnknownUnit)?);
  if !result.is_finite() || (result==0.0&&value!=0.0){return Err(UnitError::OutOfRange);}
  Ok(result)
 }
}
/// Axis is unsigned; direction specifies positive or negative separately.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Axis { X,Y,Z }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct SignedAxis {pub axis:Axis,pub negative:bool}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Handedness { Left,Right }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct CoordinateFrame {pub up:SignedAxis,pub forward:SignedAxis,pub handedness:Handedness}
impl CoordinateFrame {
 pub fn new(up:SignedAxis,forward:SignedAxis,handedness:Handedness)->Result<Self,SceneError>{
  if up.axis==forward.axis{return Err(SceneError::ParallelAxes);}
  Ok(Self{up,forward,handedness})
 }
}
/// Row-major affine matrix [linear | translation]. Does not bake transforms.
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Affine3 { rows:[[f64;4];3] }
impl Affine3 {
 pub fn identity()->Self {Self{rows:[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.]]}}
 pub fn new(rows:[[f64;4];3])->Result<Self,SceneError>{
  if rows.iter().flatten().any(|x|!x.is_finite()){return Err(SceneError::NonFiniteTransform);}Ok(Self{rows})
 }
 pub fn rows(&self)->[[f64;4];3]{self.rows}
 pub fn transform_point(&self,p:Point3)->Result<Point3,SceneError>{
  let v=[p.x,p.y,p.z,1.];if v.iter().any(|x|!x.is_finite()){return Err(SceneError::NonFiniteGeometry);}
  let mut out=[0.;3];for (i,row) in self.rows.iter().enumerate(){out[i]=row.iter().zip(v).map(|(a,b)|a*b).sum();}
  if out.iter().any(|x|!x.is_finite()){return Err(SceneError::NonFiniteTransform);}
  Ok(Point3{x:out[0],y:out[1],z:out[2]})
 }
 /// Compose parent * child. Child coordinates are transformed first.
 pub fn compose(&self,child:&Self)->Result<Self,SceneError>{
  let mut rows=[[0.;4];3];
  for i in 0..3 {for j in 0..3 {rows[i][j]=(0..3).map(|k|self.rows[i][k]*child.rows[k][j]).sum();}
   rows[i][3]=self.rows[i][3]+(0..3).map(|k|self.rows[i][k]*child.rows[k][3]).sum::<f64>();}
  Self::new(rows)
 }
}
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Point2 {pub x:f64,pub y:f64}
impl Point2 {fn finite(self)->bool{self.x.is_finite()&&self.y.is_finite()}}
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum PathSegment { Line{end:Point2}, Cubic{control1:Point2,control2:Point2,end:Point2} }
/// A 2D path is separate from a mesh; it is never extruded automatically.
#[derive(Clone,Debug,PartialEq)]
pub struct Path2 {start:Point2,segments:Vec<PathSegment>,closed:bool}
impl Path2 {
 pub fn new(start:Point2,segments:Vec<PathSegment>,closed:bool)->Result<Self,SceneError>{
  if !start.finite()||segments.iter().any(|s|match *s{PathSegment::Line{end}=>!end.finite(),PathSegment::Cubic{control1,control2,end}=>!control1.finite()||!control2.finite()||!end.finite()}) {return Err(SceneError::NonFiniteGeometry);}
  Ok(Self{start,segments,closed})
 }
 pub fn start(&self)->Point2{self.start}
 pub fn segments(&self)->&[PathSegment]{&self.segments}
 pub fn closed(&self)->bool{self.closed}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub struct ResourceId(usize);
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub struct NodeId(usize);
#[derive(Clone,Debug,PartialEq)]
pub enum Geometry {Mesh(Arc<MeshDocument>),Path2(Arc<Path2>)}
#[derive(Clone,Debug,PartialEq)]
pub struct Node {pub name:String,pub parent:Option<NodeId>,pub geometry:Option<ResourceId>,pub local_transform:Affine3}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum SceneError { ParallelAxes,NonFiniteTransform,NonFiniteGeometry,UnknownParent,UnknownResource }
/// Resource definitions can be instanced by multiple nodes without copying.
/// New nodes only refer to existing parents, so the hierarchy cannot cycle.
#[derive(Clone,Debug)]
pub struct Scene {unit:LengthUnit,frame:Option<CoordinateFrame>,resources:Vec<Geometry>,nodes:Vec<Node>}
impl Scene {
 pub fn new(unit:LengthUnit,frame:Option<CoordinateFrame>)->Result<Self,SceneError>{
  if let Some(f)=frame {if f.up.axis==f.forward.axis{return Err(SceneError::ParallelAxes);}}
  Ok(Self{unit,frame,resources:Vec::new(),nodes:Vec::new()})
 }
 pub fn add_geometry(&mut self,geometry:Geometry)->ResourceId {let id=ResourceId(self.resources.len());self.resources.push(geometry);id}
 pub fn unit(&self)->LengthUnit{self.unit}
 pub fn frame(&self)->Option<CoordinateFrame>{self.frame}
 pub fn geometry(&self,id:ResourceId)->Option<&Geometry>{self.resources.get(id.0)}
 pub fn node(&self,id:NodeId)->Option<&Node>{self.nodes.get(id.0)}
 pub fn add_node(&mut self,node:Node)->Result<NodeId,SceneError>{
  if node.parent.is_some_and(|id|id.0>=self.nodes.len()){return Err(SceneError::UnknownParent);}
  if node.geometry.is_some_and(|id|id.0>=self.resources.len()){return Err(SceneError::UnknownResource);}
  let id=NodeId(self.nodes.len());self.nodes.push(node);Ok(id)
 }
 pub fn node_count(&self)->usize{self.nodes.len()}
 /// Evaluate parent transforms without applying them to stored vertices.
 pub fn world_transform(&self,id:NodeId)->Option<Affine3>{
  let mut chain=Vec::new();let mut next=Some(id);
  while let Some(n)=next{let node=self.node(n)?;chain.push(node.local_transform);next=node.parent;}
  let mut out=Affine3::identity();for local in chain.iter().rev(){out=out.compose(local).ok()?;}Some(out)
 }
}
#[cfg(test)]
mod tests {
 use super::*;use crate::Mesh;
 fn pt(x:f64,y:f64)->Point2{Point2{x,y}}
 #[test] fn units_are_explicit_and_unknown_is_not_assumed(){assert!((LengthUnit::Inches.convert(1.,LengthUnit::Millimeters).unwrap()-25.4).abs()<1e-12);assert_eq!(LengthUnit::Unspecified.convert(1.,LengthUnit::Millimeters),Err(UnitError::UnknownUnit));assert_eq!(LengthUnit::Meters.convert(f64::NAN,LengthUnit::Inches),Err(UnitError::NonFiniteValue));}
 #[test] fn units_reject_extreme_numeric_range(){assert_eq!(LengthUnit::Feet.convert(f64::MAX,LengthUnit::Microns),Err(UnitError::OutOfRange));assert_eq!(LengthUnit::Microns.convert(f64::from_bits(1),LengthUnit::Feet),Err(UnitError::OutOfRange));}
 #[test] fn paths_preserve_curves_and_validate(){let p=Path2::new(pt(0.,0.),vec![PathSegment::Cubic{control1:pt(1.,0.),control2:pt(1.,1.),end:pt(2.,1.)}],false).unwrap();assert_eq!(p.segments().len(),1);assert!(Path2::new(pt(f64::NAN,0.),vec![],false).is_err());}
 #[test] fn hierarchy_instances_share_geometry_without_transforming_source(){
  let mut scene=Scene::new(LengthUnit::Millimeters,None).unwrap();let mesh=MeshDocument::new(Mesh{vertices:vec![Point3{x:1.,y:2.,z:3.}],triangles:vec![]}).unwrap();let handle=Arc::new(mesh);let id=scene.add_geometry(Geometry::Mesh(handle.clone()));
  let parent=scene.add_node(Node{name:"root".into(),parent:None,geometry:None,local_transform:Affine3::new([[1.,0.,0.,10.],[0.,1.,0.,0.],[0.,0.,1.,0.]]).unwrap()}).unwrap();
  let child=scene.add_node(Node{name:"part".into(),parent:Some(parent),geometry:Some(id),local_transform:Affine3::new([[1.,0.,0.,0.],[0.,1.,0.,5.],[0.,0.,1.,0.]]).unwrap()}).unwrap();
  scene.add_node(Node{name:"second instance".into(),parent:None,geometry:Some(id),local_transform:Affine3::identity()}).unwrap();
  assert_eq!(scene.world_transform(child).unwrap().transform_point(handle.mesh().vertices[0]).unwrap(),Point3{x:11.,y:7.,z:3.});
  assert_eq!(handle.mesh().vertices[0],Point3{x:1.,y:2.,z:3.});
  if let Some(Geometry::Mesh(stored))=scene.geometry(id){assert!(Arc::ptr_eq(stored,&handle))}else{panic!("mesh missing")}
 }
 #[test] fn invalid_references_and_parallel_axes_are_rejected(){let mut s=Scene::new(LengthUnit::Unspecified,None).unwrap();let n=Node{name:"bad".into(),parent:Some(NodeId(0)),geometry:None,local_transform:Affine3::identity()};assert_eq!(s.add_node(n),Err(SceneError::UnknownParent));assert_eq!(s.node_count(),0);let x=SignedAxis{axis:Axis::X,negative:false};assert_eq!(CoordinateFrame::new(x,x,Handedness::Right),Err(SceneError::ParallelAxes));}
 #[test] fn invalid_transform_cannot_enter_scene(){assert_eq!(Affine3::new([[f64::NAN,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.]]),Err(SceneError::NonFiniteTransform));}
}
