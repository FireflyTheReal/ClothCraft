//! Phase 1 only: discrete vertex/sphere penalty. No CCD or self-contact guarantee.
use crate::{
    energy::Element,
    math::{Jet, Scalar, dot},
};
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Static collider interface. Implementations state their own geometric limits.
pub trait Collider: Sync {
    fn validate(&self) -> Result<(), String>;
    fn valid_position(&self, x: DVec3) -> bool {
        x.is_finite()
    }
    fn penetration(&self, x: DVec3, thickness: f64) -> f64;
    fn value(&self, x: DVec3, thickness: f64, area: f64) -> f64;
    fn element(&self, vertex: usize, x: DVec3, thickness: f64, area: f64) -> Option<Element>;
}
impl Collider for Sphere {
    fn validate(&self) -> Result<(), String> {
        Sphere::validate(self)
    }
    fn valid_position(&self, x: DVec3) -> bool {
        x.is_finite() && x.distance_squared(self.center) > 1e-20
    }
    fn penetration(&self, x: DVec3, thickness: f64) -> f64 {
        Sphere::penetration(self, x, thickness)
    }
    fn value(&self, x: DVec3, thickness: f64, area: f64) -> f64 {
        Sphere::energy(self, x.to_array(), thickness, area)
    }
    fn element(&self, vertex: usize, x: DVec3, thickness: f64, area: f64) -> Option<Element> {
        Sphere::element(self, vertex, x, thickness, area)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sphere {
    pub center: DVec3,
    pub radius: f64,
    /// Pressure per penetration depth: N/m^3; integrated over vertex rest area.
    pub stiffness: f64,
}
impl Default for Sphere {
    fn default() -> Self {
        Self {
            center: DVec3::ZERO,
            radius: 0.35,
            stiffness: 100_000.0,
        }
    }
}
impl Sphere {
    pub fn validate(&self) -> Result<(), String> {
        if !self.center.is_finite()
            || !self.radius.is_finite()
            || self.radius <= 0.0
            || !self.stiffness.is_finite()
            || self.stiffness <= 0.0
        {
            return Err("Invalid sphere parameters".into());
        }
        Ok(())
    }
    pub fn penetration(&self, x: DVec3, thickness: f64) -> f64 {
        (self.radius + thickness * 0.5 - x.distance(self.center)).max(0.0)
    }
    pub fn energy<S: Scalar>(&self, x: [S; 3], thickness: f64, area: f64) -> S {
        let p = std::array::from_fn(|i| x[i] - S::c(self.center[i]));
        let d = dot(p, p).sqrt();
        let gap = d - S::c(self.radius + thickness * 0.5);
        if gap.value() >= 0.0 {
            S::c(0.0)
        } else {
            S::c(0.5 * self.stiffness * area) * gap * gap
        }
    }
    pub fn element(&self, vertex: usize, x: DVec3, thickness: f64, area: f64) -> Option<Element> {
        if self.penetration(x, thickness) == 0.0 {
            return None;
        }
        let p = std::array::from_fn(|i| Jet::<3>::variable(x[i], i));
        Some(Element::from_jet(
            &[vertex],
            self.energy(p, thickness, area),
        ))
    }
}
