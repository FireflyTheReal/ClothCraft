//! Read-only net cloth quantities from a freshly compiled material-space mesh.
//!
//! These are not cutting/marker consumption: no seam allowance, nesting loss,
//! shrinkage, purchased length or trim weight is inferred.
use super::GarmentTemplate;
use glam::DVec2;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MaterialBounds {
    pub min_m: [f64; 2],
    pub max_m: [f64; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PanelMetrics {
    pub panel_id: String,
    pub panel_name: String,
    pub fabric_id: String,
    pub net_area_m2: f64,
    pub mass_kg: f64,
    /// Axis-aligned bounds in panel-local material coordinates, not roll space.
    pub material_bounds: MaterialBounds,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FabricMetrics {
    pub fabric_id: String,
    pub fabric_name: String,
    pub density_kg_m2: f64,
    pub panel_count: usize,
    pub net_area_m2: f64,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProductionMetrics {
    pub format: &'static str,
    pub version: u32,
    pub geometry_basis: &'static str,
    pub panel_count: usize,
    pub net_area_m2: f64,
    pub mass_kg: f64,
    /// Template order; every authored panel is counted once, including mirrors.
    pub panels: Vec<PanelMetrics>,
    /// Used fabrics only, sorted by stable fabric ID (not display name).
    pub fabrics: Vec<FabricMetrics>,
}

fn positive(value: f64, label: &str) -> Result<f64, String> {
    if value.is_finite() && value > 0. {
        Ok(value)
    } else {
        Err(format!(
            "Production metric {label} must be finite and positive"
        ))
    }
}

/// Compile and report net pattern area and corresponding physical cloth mass.
///
/// Compilation deliberately happens here: callers cannot combine a changed
/// template with stale rest/mass caches. Curves use the compiler's sampled
/// contour; holes are absent from the triangulation. No draped state is read.
pub fn report(template: &GarmentTemplate) -> Result<ProductionMetrics, String> {
    let garment = template.compile()?;
    let fabric_by_id: BTreeMap<_, _> = template
        .fabrics
        .iter()
        .map(|fabric| (fabric.id.as_str(), fabric))
        .collect();
    let mut fabrics: BTreeMap<String, FabricMetrics> = BTreeMap::new();
    let mut panels = Vec::with_capacity(template.panels.len());
    for (panel, range) in template.panels.iter().zip(&garment.panels) {
        let area = positive(
            garment.mesh.triangles[range.triangles.clone()]
                .iter()
                .map(|triangle| triangle.area)
                .sum(),
            "panel net area",
        )?;
        let mass = positive(
            garment.mesh.masses[range.vertices.clone()].iter().sum(),
            "panel mass",
        )?;
        let mut min = DVec2::splat(f64::INFINITY);
        let mut max = DVec2::splat(f64::NEG_INFINITY);
        for &uv in &garment.mesh.uv[range.vertices.clone()] {
            if !uv.is_finite() {
                return Err("Production metric material bounds must be finite".into());
            }
            min = min.min(uv);
            max = max.max(uv);
        }
        if !min.is_finite() || !max.is_finite() {
            return Err("Production metric material bounds require a nonempty mesh".into());
        }
        let fabric = fabric_by_id[panel.fabric.as_str()];
        let total = fabrics
            .entry(panel.fabric.clone())
            .or_insert_with(|| FabricMetrics {
                fabric_id: fabric.id.clone(),
                fabric_name: fabric.name.clone(),
                density_kg_m2: fabric.properties.density,
                panel_count: 0,
                net_area_m2: 0.,
                mass_kg: 0.,
            });
        total.panel_count += 1;
        total.net_area_m2 = positive(total.net_area_m2 + area, "fabric net area")?;
        total.mass_kg = positive(total.mass_kg + mass, "fabric mass")?;
        panels.push(PanelMetrics {
            panel_id: panel.id.clone(),
            panel_name: panel.name.clone(),
            fabric_id: fabric.id.clone(),
            net_area_m2: area,
            mass_kg: mass,
            material_bounds: MaterialBounds {
                min_m: min.to_array(),
                max_m: max.to_array(),
            },
        });
    }
    Ok(ProductionMetrics {
        format: "clothcraft-net-material-metrics",
        version: 1,
        geometry_basis: "compiled_material_rest_mesh",
        panel_count: panels.len(),
        net_area_m2: positive(panels.iter().map(|p| p.net_area_m2).sum(), "total net area")?,
        mass_kg: positive(panels.iter().map(|p| p.mass_kg).sum(), "total mass")?,
        panels,
        fabrics: fabrics.into_values().collect(),
    })
}
