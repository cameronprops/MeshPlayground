//! Explicit project-level accuracy target with optional operation override.
//! Tolerance is expressed in mesh coordinate units; unit metadata is separate.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum LengthUnit { Millimeters, Inches, Meters, Unspecified }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ToleranceSource { ProjectDefault, OperationOverride }
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct EffectiveTolerance { pub value:f64, pub unit:LengthUnit, pub source:ToleranceSource }
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct TolerancePolicy { project_default:f64, unit:LengthUnit }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ToleranceError { InvalidProjectDefault, InvalidOperationOverride }
impl TolerancePolicy {
 pub fn new(project_default:f64,unit:LengthUnit)->Result<Self,ToleranceError>{
  if !project_default.is_finite() || project_default<0.0{return Err(ToleranceError::InvalidProjectDefault);}
  Ok(Self{project_default,unit})
 }
 /// Override, when supplied, replaces the project value for this operation.
 /// Neither setting changes the mesh or certifies an operation meets it.
 pub fn resolve(&self,operation_override:Option<f64>)->Result<EffectiveTolerance,ToleranceError>{
  match operation_override {
   Some(value) if !value.is_finite() || value<0.0=>Err(ToleranceError::InvalidOperationOverride),
   Some(value)=>Ok(EffectiveTolerance{value,unit:self.unit,source:ToleranceSource::OperationOverride}),
   None=>Ok(EffectiveTolerance{value:self.project_default,unit:self.unit,source:ToleranceSource::ProjectDefault}),
  }
 }
}
/// A sampled maximum can establish that a tolerance was exceeded, but cannot
/// establish that the entire surface meets it between the sampled points.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SampledToleranceAssessment { Exceeded, Inconclusive }
pub fn assess_sampled_maximum(maximum:f64,tolerance:EffectiveTolerance)->Result<SampledToleranceAssessment,ToleranceError>{
 if !maximum.is_finite() || maximum<0.0 || !tolerance.value.is_finite() || tolerance.value<0.0 {return Err(ToleranceError::InvalidOperationOverride);}
 Ok(if maximum>tolerance.value{SampledToleranceAssessment::Exceeded}else{SampledToleranceAssessment::Inconclusive})
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn override_and_default_are_explicit(){let p=TolerancePolicy::new(0.01,LengthUnit::Millimeters).unwrap();let x=p.resolve(None).unwrap();assert_eq!(x.value,0.01);assert_eq!(x.source,ToleranceSource::ProjectDefault);let x=p.resolve(Some(0.002)).unwrap();assert_eq!(x.value,0.002);assert_eq!(x.unit,LengthUnit::Millimeters);assert_eq!(x.source,ToleranceSource::OperationOverride);assert_eq!(p.resolve(None).unwrap().value,0.01);}
 #[test] fn exact_zero_allowed_invalid_values_rejected(){assert!(TolerancePolicy::new(-1.,LengthUnit::Unspecified).is_err());assert!(TolerancePolicy::new(f64::NAN,LengthUnit::Unspecified).is_err());let p=TolerancePolicy::new(0.,LengthUnit::Unspecified).unwrap();assert!(p.resolve(Some(f64::INFINITY)).is_err());assert_eq!(p.resolve(None).unwrap().value,0.);}
 #[test] fn sample_below_limit_is_not_a_certification(){let p=TolerancePolicy::new(0.01,LengthUnit::Millimeters).unwrap().resolve(None).unwrap();assert_eq!(assess_sampled_maximum(0.009,p).unwrap(),SampledToleranceAssessment::Inconclusive);assert_eq!(assess_sampled_maximum(0.011,p).unwrap(),SampledToleranceAssessment::Exceeded);}
}
