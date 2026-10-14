//! Fabric controls convert user units to the existing SI material model.
use super::*;
use marvelous_cloth::{
    authoring::Command,
    fabrics::{self, PRESETS},
    garment::template::{FabricProperties, Panel},
};

pub(super) struct FabricDraft {
    panel: String,
    name: String,
    properties: FabricProperties,
    extension: [f64; 2],
    preset: Option<usize>,
    all_panels: bool,
}
impl Atelier {
    pub(super) fn fabric_properties(&mut self, ui: &mut egui::Ui, panel: &Panel) {
        let Some(fabric) = self
            .project
            .template
            .fabrics
            .iter()
            .find(|f| f.id == panel.fabric)
        else {
            return;
        };
        if self
            .authoring
            .fabric_draft
            .as_ref()
            .is_none_or(|d| d.panel != panel.id)
        {
            self.authoring.fabric_draft = Some(FabricDraft {
                panel: panel.id.clone(),
                name: fabric.name.clone(),
                extension: [
                    fabrics::extension_percent(fabric.properties.warp, &fabric.properties),
                    fabrics::extension_percent(fabric.properties.weft, &fabric.properties),
                ],
                properties: fabric.properties.clone(),
                preset: PRESETS
                    .iter()
                    .position(|p| p.properties() == fabric.properties),
                all_panels: false,
            });
        }
        let panel_mass: f64 = self
            .scene
            .garment
            .panels
            .iter()
            .find(|p| p.id == panel.id)
            .map(|p| {
                self.scene.garment.mesh.masses[p.vertices.clone()]
                    .iter()
                    .sum()
            })
            .unwrap_or(0.);
        ui.small(format!(
            "{} · {:.0} g/m²",
            fabric.name,
            fabric.properties.density * 1000.
        ));
        ui.small(format!(
            "Masse : panneau {:.1} g · vêtement {:.1} g",
            panel_mass * 1000.,
            self.scene.garment.mesh.masses.iter().sum::<f64>() * 1000.
        ));
        let mut apply = false;
        let mut discard = false;
        let mut targets = vec![];
        let header = egui::CollapsingHeader::new("Tissu · poids et stretch")
            .id_salt("fabric_controls").show(ui, |ui| {
                let d = self.authoring.fabric_draft.as_mut().unwrap();
                let old = d.preset;
                let combo = egui::ComboBox::from_id_salt("fabric_preset")
                    .selected_text(d.preset.map(|i| PRESETS[i].name).unwrap_or("Choisir un type de tissu"))
                    .show_ui(ui, |ui| {
                        for (i, p) in PRESETS.iter().enumerate() {
                            let response = ui.selectable_value(&mut d.preset, Some(i), p.name);
                            targets.push((format!("fabric_preset_{}", p.id), response.rect.center()));
                        }
                    });
                targets.push(("fabric_presets".into(), combo.response.rect.center()));
                if d.preset != old && let Some(i) = d.preset {
                    let preset = &PRESETS[i];
                    d.name = format!("{} (approx.)", preset.name);
                    d.properties = preset.properties();
                    d.extension = preset.extension;
                }
                ui.small("Presets indicatifs. Élasticité et flexion à calibrer sur ton tissu.");
                if let Some(i) = d.preset {
                    ui.small(PRESETS[i].evidence);
                    ui.hyperlink_to("Source des mesures", PRESETS[i].source);
                }
                ui.text_edit_singleline(&mut d.name);
                let p = &mut d.properties;
                let before_properties = p.clone();
                let mut gsm = p.density * 1000.;
                if value(ui, "Masse surfacique", &mut gsm, 1., 10., 2000., " g/m²") { p.density = gsm / 1000.; }
                let mut thickness = p.thickness * 1000.;
                if value(ui, "Épaisseur", &mut thickness, 0.01, 0.02, 5., " mm") { p.thickness = thickness / 1000.; }
                ui.label("Stretch sous 5 N sur une bande de 5 cm");
                ui.small("Plus le % est grand, plus le tissu est extensible. Ce n'est pas l'allongement à rupture.");
                for (axis, label) in ["Chaîne", "Trame"].iter().enumerate() {
                    if value(ui, label, &mut d.extension[axis], 0.1, 0.1, 100., " %")
                        && let Ok(k) = fabrics::stiffness_for_extension(d.extension[axis], p) {
                            if axis == 0 { p.warp = k; } else { p.weft = k; }
                        }
                }
                ui.collapsing("Rigidités avancées", |ui| {
                    value(ui, "Cisaillement", &mut p.shear, 1., 0.1, 10000., " N/m");
                    for (label, b) in [("Flexion chaîne", &mut p.bend_warp), ("Flexion trame", &mut p.bend_weft)] {
                        let mut micro = *b * 1e6;
                        if value(ui, label, &mut micro, 1., 0., 100000., " µN·m") { *b = micro * 1e-6; }
                    }
                    ui.small(format!("Membrane chaîne/trame : {:.0} / {:.0} N/m", p.warp, p.weft));
                    ui.small("Le rayon de collision se règle séparément dans Simulation.");
                });
                if d.properties != before_properties { d.preset = None; }
                ui.checkbox(&mut d.all_panels, "Appliquer à tous les panneaux");
                let button = ui.button("Appliquer le tissu");
                targets.push(("fabric_apply".into(), button.rect.center()));
                apply = button.clicked();
                discard = ui.button("Annuler les réglages").clicked();
            });
        self.smoke_target("fabric_panel", header.header_response.rect.center());
        for (name, p) in targets {
            self.smoke_target(&name, p);
        }
        if apply {
            let d = self.authoring.fabric_draft.as_ref().unwrap();
            self.edit(Command::Fabric {
                panel: d.panel.clone(),
                name: d.name.clone(),
                properties: d.properties.clone(),
                all_panels: d.all_panels,
            });
        } else if discard {
            self.authoring.fabric_draft = None;
        }
    }
}
fn value(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut f64,
    speed: f64,
    min: f64,
    max: f64,
    suffix: &str,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(v)
                .speed(speed)
                .range(min..=max)
                .suffix(suffix),
        )
        .changed()
    })
    .inner
}
