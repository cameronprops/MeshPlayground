//! Strict, geometry-only OBJ subset. It preserves polygon faces and independent
//! position, texture-coordinate, and normal indices; unsupported statements are
//! rejected rather than dropped. This is not yet a full OBJ/MTL adapter.
use std::io::{self,BufRead,Write};
use crate::{Mesh,Point3};
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct TextureCoord {pub u:f64,pub v:Option<f64>,pub w:Option<f64>}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Corner {pub vertex:usize,pub texture:Option<usize>,pub normal:Option<usize>}
#[derive(Clone,Debug,PartialEq)]
pub struct ObjGeometry {pub vertices:Vec<Point3>,pub texcoords:Vec<TextureCoord>,pub normals:Vec<Point3>,pub faces:Vec<Vec<Corner>>}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ObjErrorKind {Io,UnsupportedStatement,Malformed,InvalidNumber,InvalidIndex,InvalidFace,NonTriangle,AttributesWouldBeLost}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ObjError {pub line:usize,pub kind:ObjErrorKind,pub detail:String}
impl std::fmt::Display for ObjError {fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"OBJ line {}: {}",self.line,self.detail)}}
impl std::error::Error for ObjError {}
fn error(line:usize,kind:ObjErrorKind,detail:&str)->ObjError {ObjError{line,kind,detail:detail.into()}}
fn number(s:&str,line:usize)->Result<f64,ObjError>{let x:f64=s.parse().map_err(|_|error(line,ObjErrorKind::InvalidNumber,"invalid number"))?;if !x.is_finite(){return Err(error(line,ObjErrorKind::InvalidNumber,"non-finite number"));}Ok(x)}
fn index(s:&str,size:usize,line:usize)->Result<usize,ObjError>{
 let raw:i64=s.parse().map_err(|_|error(line,ObjErrorKind::InvalidIndex,"invalid index"))?;
 let value=if raw>0 {raw.checked_sub(1)}else if raw<0 {i64::try_from(size).ok().and_then(|n|n.checked_add(raw))}else{None};
 match value.and_then(|n|usize::try_from(n).ok()){Some(i) if i<size=>Ok(i),_=>Err(error(line,ObjErrorKind::InvalidIndex,"index out of range or zero"))}
}
impl ObjGeometry {
 pub fn parse<R:BufRead>(reader:R)->Result<Self,ObjError>{
  let mut out=Self{vertices:vec![],texcoords:vec![],normals:vec![],faces:vec![]};
  for (i,line) in reader.lines().enumerate(){let ln=i+1;let text=line.map_err(|_|error(ln,ObjErrorKind::Io,"could not read UTF-8 line"))?;let trimmed=text.split('#').next().unwrap_or("").trim();let mut tokens=trimmed.split_whitespace();let Some(op)=tokens.next() else{continue};let args:Vec<_>=tokens.collect();
   match op {
    "v"=>{if args.len()!=3{return Err(error(ln,ObjErrorKind::UnsupportedStatement,"only 3-coordinate vertex records are supported; weights and vertex colors are not"));}out.vertices.push(Point3{x:number(args[0],ln)?,y:number(args[1],ln)?,z:number(args[2],ln)?});},
    "vn"=>{if args.len()!=3{return Err(error(ln,ObjErrorKind::Malformed,"normal requires three coordinates"));}out.normals.push(Point3{x:number(args[0],ln)?,y:number(args[1],ln)?,z:number(args[2],ln)?});},
    "vt"=>{if args.is_empty()||args.len()>3{return Err(error(ln,ObjErrorKind::Malformed,"texture coordinate requires one to three values"));}out.texcoords.push(TextureCoord{u:number(args[0],ln)?,v:if args.len()>1{Some(number(args[1],ln)?)}else{None},w:if args.len()>2{Some(number(args[2],ln)?)}else{None}});},
    "f"=>{
     if args.len()<3{return Err(error(ln,ObjErrorKind::InvalidFace,"face requires at least three corners"));}
     let mut face=Vec::with_capacity(args.len());
     for part in args {let fields:Vec<_>=part.split('/').collect();
      if fields.is_empty()||fields.len()>3||fields[0].is_empty()||(fields.len()==2&&fields[1].is_empty())||(fields.len()==3&&fields[2].is_empty()){return Err(error(ln,ObjErrorKind::Malformed,"invalid face corner"));}
      let vertex=index(fields[0],out.vertices.len(),ln)?;
      let texture=if fields.len()>1&&!fields[1].is_empty(){Some(index(fields[1],out.texcoords.len(),ln)?)}else{None};
      let normal=if fields.len()==3{Some(index(fields[2],out.normals.len(),ln)?)}else{None};
      face.push(Corner{vertex,texture,normal});
     }out.faces.push(face);
    },
    _=>return Err(error(ln,ObjErrorKind::UnsupportedStatement,&format!("unsupported statement {op:?}; no data has been silently discarded"))),
   }
  }Ok(out)
 }
 /// Write the supported subset; independent per-corner indices remain intact.
 /// Floating-point formatting round-trips the parsed f64 values, not original text.
 pub fn write<W:Write>(&self,mut w:W)->Result<(),ObjError>{
  for p in self.vertices.iter().chain(&self.normals){if !p.x.is_finite()||!p.y.is_finite()||!p.z.is_finite(){return Err(error(0,ObjErrorKind::InvalidNumber,"non-finite coordinate"));}}
  for t in &self.texcoords {if !t.u.is_finite()||t.v.is_some_and(|x|!x.is_finite())||t.w.is_some_and(|x|!x.is_finite())||t.w.is_some()&&t.v.is_none(){return Err(error(0,ObjErrorKind::InvalidNumber,"invalid texture coordinate"));}}
  for f in &self.faces{if f.len()<3{return Err(error(0,ObjErrorKind::InvalidFace,"face requires at least three corners"));}for c in f{if c.vertex>=self.vertices.len()||c.texture.is_some_and(|x|x>=self.texcoords.len())||c.normal.is_some_and(|x|x>=self.normals.len()){return Err(error(0,ObjErrorKind::InvalidIndex,"face index out of range"));}}}
  let emit=|r:io::Result<()>|r.map_err(|_|error(0,ObjErrorKind::Io,"write failed"));
  for p in &self.vertices {emit(writeln!(w,"v {} {} {}",p.x,p.y,p.z))?;}
  for p in &self.texcoords {emit(write!(w,"vt {}",p.u))?;if let Some(v)=p.v{emit(write!(w," {v}"))?;}if let Some(v)=p.w{emit(write!(w," {v}"))?;}emit(writeln!(w))?;}
  for p in &self.normals {emit(writeln!(w,"vn {} {} {}",p.x,p.y,p.z))?;}
  for f in &self.faces {emit(write!(w,"f"))?;for c in f {let v=c.vertex+1;match(c.texture,c.normal){(None,None)=>emit(write!(w," {v}"))?,(Some(t),None)=>emit(write!(w," {v}/{}",t+1))?,(None,Some(n))=>emit(write!(w," {v}//{}",n+1))?,(Some(t),Some(n))=>emit(write!(w," {v}/{}/{}",t+1,n+1))?}}emit(writeln!(w))?;}
  Ok(())
 }
 /// Convert only when no UV/normal information would be lost and every face is
 /// triangular. No coordinate-based welding is performed.
 pub fn try_triangle_mesh(&self)->Result<Mesh,ObjError>{
  if !self.texcoords.is_empty()||!self.normals.is_empty()||self.faces.iter().flatten().any(|c|c.texture.is_some()||c.normal.is_some()){return Err(error(0,ObjErrorKind::AttributesWouldBeLost,"triangle mesh cannot yet preserve OBJ UVs or normals"));}
  let mut triangles=Vec::with_capacity(self.faces.len());
  for face in &self.faces{if face.len()!=3{return Err(error(0,ObjErrorKind::NonTriangle,"explicit polygon triangulation is required"));}triangles.push([face[0].vertex,face[1].vertex,face[2].vertex]);}
  let mesh=Mesh{vertices:self.vertices.clone(),triangles};mesh.validate().map_err(|_|error(0,ObjErrorKind::InvalidIndex,"invalid mesh data"))?;Ok(mesh)
 }
}
#[cfg(test)]
mod tests {
 use super::*;
 const INPUT:&str="v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 1\nf -4/1/1 -3/2/1 -2/3/1 -1/4/1\n";
 #[test] fn preserves_polygon_and_independent_attributes(){let m=ObjGeometry::parse(INPUT.as_bytes()).unwrap();assert_eq!(m.faces[0].len(),4);assert_eq!(m.faces[0][0],Corner{vertex:0,texture:Some(0),normal:Some(0)});let mut b=Vec::new();m.write(&mut b).unwrap();assert_eq!(ObjGeometry::parse(b.as_slice()).unwrap(),m);}
 #[test] fn rejects_silent_loss(){let m=ObjGeometry::parse(INPUT.as_bytes()).unwrap();assert_eq!(m.try_triangle_mesh().unwrap_err().kind,ObjErrorKind::AttributesWouldBeLost);assert_eq!(ObjGeometry::parse(b"o body\n".as_slice()).unwrap_err().kind,ObjErrorKind::UnsupportedStatement);}
 #[test] fn basic_triangle_conversion(){let m=ObjGeometry::parse(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n".as_slice()).unwrap();assert_eq!(m.try_triangle_mesh().unwrap().triangles,vec![[0,1,2]]);}
 #[test] fn rejects_invalid_or_forward_indices(){for f in ["f 0 1 2","f -9 1 2","f 1 2 4","f 1/ 2 3"]{let s=format!("v 0 0 0\nv 1 0 0\nv 0 1 0\n{f}\n");assert!(ObjGeometry::parse(s.as_bytes()).is_err());}}
 #[test] fn nontriangle_requires_explicit_conversion(){let m=ObjGeometry::parse(b"v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n".as_slice()).unwrap();assert_eq!(m.try_triangle_mesh().unwrap_err().kind,ObjErrorKind::NonTriangle);}
 #[test] fn preflight_detects_bad_manual_model_before_writing(){let mut m=ObjGeometry::parse(INPUT.as_bytes()).unwrap();m.faces[0][0].vertex=99;let mut out=Vec::new();assert!(m.write(&mut out).is_err());assert!(out.is_empty());}
}
