//! Indicative fabric library. Mass/thickness references are per specimen;
//! elasticity and bending are authored approximations, not calibrated samples.
//! See docs/fabrics.md for sources, units and the load-defined stretch control.
use crate::garment::template::FabricProperties;

pub const REFERENCE_TRACTION: f64 = 100.; // N/m = 5 N on a 50 mm strip.
pub const MIT_SOURCE: &str =
    "https://people.csail.mit.edu/klbouman/pw/projects/materialproperties/dataset.html";
pub const WANG_SOURCE: &str = "https://wanghmin.github.io/publication/wang-2011-dde/";
pub const LEATHER_SOURCE: &str = "https://purleder.com/en/our-leathers/autonappa";

pub struct FabricPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub gsm: f64,
    pub thickness_mm: f64,
    /// Authored extension percentages under REFERENCE_TRACTION, not breaking strain.
    pub extension: [f64; 2],
    pub shear: f64,
    pub bending: [f64; 2],
    pub source: &'static str,
    pub evidence: &'static str,
}
impl FabricPreset {
    pub fn properties(&self) -> FabricProperties {
        let mut p = FabricProperties {
            density: self.gsm / 1000.,
            thickness: self.thickness_mm / 1000.,
            shear: self.shear,
            bend_warp: self.bending[0],
            bend_weft: self.bending[1],
            ..Default::default()
        };
        p.warp = stiffness_for_extension(self.extension[0], &p).unwrap();
        p.weft = stiffness_for_extension(self.extension[1], &p).unwrap();
        p
    }
}

// MIT: mean of the XLSX specimens, oz/yd² -> g/m² and inches -> mm.
// MIT beam stiffness (lbf in²) is deliberately NOT used as shell rigidity (N m).
pub const PRESETS: &[FabricPreset] = &[
    FabricPreset {
        id: "cotton",
        name: "Light woven cotton",
        gsm: 99.648,
        thickness_mm: 0.1905,
        extension: [3., 5.],
        shear: 120.,
        bending: [0.00003, 0.00002],
        source: MIT_SOURCE,
        evidence: "MIT #5: measured mass and thickness; elasticity/bending estimated.",
    },
    FabricPreset {
        id: "jersey",
        name: "Cotton-blend jersey",
        gsm: 187.,
        thickness_mm: 0.6,
        extension: [15., 35.],
        shear: 25.,
        bending: [0.000015, 0.00001],
        source: WANG_SOURCE,
        evidence: "Wang 2011, Gray Interlock (60% cotton, 40% polyester): measured mass; other values estimated.",
    },
    FabricPreset {
        id: "wool",
        name: "Thick woven wool",
        gsm: 384.58,
        thickness_mm: 1.1201,
        extension: [6., 9.],
        shear: 100.,
        bending: [0.00018, 0.00012],
        source: MIT_SOURCE,
        evidence: "MIT #6: measured mass and thickness; elasticity/bending estimated.",
    },
    FabricPreset {
        id: "denim",
        name: "Denim",
        gsm: 296.675,
        thickness_mm: 0.6096,
        extension: [1.5, 3.],
        shear: 300.,
        bending: [0.0004, 0.00025],
        source: MIT_SOURCE,
        evidence: "MIT #13 (one specimen): measured mass and thickness; elasticity/bending estimated.",
    },
    FabricPreset {
        id: "leather",
        name: "Thick nappa leather",
        gsm: 750.,
        thickness_mm: 1.,
        extension: [4., 6.],
        shear: 500.,
        bending: [0.001, 0.0008],
        source: LEATHER_SOURCE,
        evidence: "PURLEDER Auto Nappa: stated mass, median thickness 0.9–1.1 mm; mechanics estimated. Leather-goods/furniture reference.",
    },
    FabricPreset {
        id: "silk",
        name: "Light silk",
        gsm: 66.968,
        thickness_mm: 0.127,
        extension: [3., 5.],
        shear: 50.,
        bending: [0.000008, 0.000005],
        source: MIT_SOURCE,
        evidence: "MIT #3: measured mass and thickness; elasticity/bending estimated.",
    },
    FabricPreset {
        id: "linen",
        name: "Linen",
        gsm: 151.787,
        thickness_mm: 0.2286,
        extension: [2., 3.],
        shear: 170.,
        bending: [0.00008, 0.00005],
        source: MIT_SOURCE,
        evidence: "MIT #8: measured mass and thickness; elasticity/bending estimated.",
    },
    FabricPreset {
        id: "nylon",
        name: "Nylon ripstop",
        gsm: 58.516,
        thickness_mm: 0.0762,
        extension: [4., 6.],
        shear: 80.,
        bending: [0.000012, 0.00001],
        source: MIT_SOURCE,
        evidence: "MIT #25: measured mass and thickness; elasticity/bending estimated.",
    },
    FabricPreset {
        id: "stretch",
        name: "Stretch knit / Lycra",
        gsm: 239.405,
        thickness_mm: 0.381,
        extension: [40., 60.],
        shear: 15.,
        bending: [0.00002, 0.000015],
        source: MIT_SOURCE,
        evidence: "MIT #1: measured mass and thickness; elasticity/bending estimated. Knit loops are not simulated.",
    },
];

/// Nominal axial traction dW/dlambda for unit modulus, using exactly the smooth
/// Green-strain law in energy::membrane. Orthogonal stretch stays one, shear zero.
fn unit_traction(extension_percent: f64, p: &FabricProperties) -> f64 {
    let lambda = 1. + extension_percent / 100.;
    let e = (lambda * lambda - 1.) * 0.5;
    let root = (e * e + p.compression_transition.powi(2)).sqrt();
    let positive = (e + root - p.compression_transition) * 0.5;
    let derivative = (1. + e / root) * 0.5;
    lambda * (p.compression_ratio * e + (1. - p.compression_ratio) * positive * derivative)
}
pub fn stiffness_for_extension(percent: f64, p: &FabricProperties) -> Result<f64, String> {
    if !percent.is_finite() || !(0.1..=100.).contains(&percent) {
        return Err("Extension expected between 0.1 and 100% under 5 N / 5 cm".into());
    }
    p.bind(0.).validate()?;
    Ok(REFERENCE_TRACTION / unit_traction(percent, p))
}
/// Inverse display mapping; does not clamp or silently change imported materials.
pub fn extension_percent(stiffness: f64, p: &FabricProperties) -> f64 {
    let mut lo = 0.;
    let mut hi = 100.;
    while stiffness * unit_traction(hi, p) < REFERENCE_TRACTION && hi < 1e6 {
        hi *= 2.;
    }
    for _ in 0..64 {
        let mid = (lo + hi) * 0.5;
        if stiffness * unit_traction(mid, p) < REFERENCE_TRACTION {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * 0.5
}
