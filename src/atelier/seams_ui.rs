//! Existing seam selection/deletion is available independently of sewing mode.
use super::{authoring_ui::Tool, board::same_edge, *};
use marvelous_cloth::{authoring::Command, garment::seam::BoundaryRef};

impl Atelier {
    pub(super) fn edge_seams(&self) -> Vec<String> {
        self.project
            .template
            .seams
            .iter()
            .filter(|s| {
                self.authoring
                    .edge
                    .as_ref()
                    .is_some_and(|e| same_edge(e, &s.a) || same_edge(e, &s.b))
            })
            .map(|s| s.id.clone())
            .collect()
    }
    pub(super) fn seam_edge_highlighted(&self, edge: &BoundaryRef) -> bool {
        self.authoring.edge.as_ref().is_some_and(|selected| {
            same_edge(selected, edge)
                || self.project.template.seams.iter().any(|s| {
                    (same_edge(selected, &s.a) && same_edge(edge, &s.b))
                        || (same_edge(selected, &s.b) && same_edge(edge, &s.a))
                })
        })
    }
    pub(super) fn remove_seam(&mut self, seam: String) {
        if self.edit(Command::RemoveSeam { seam }) {
            self.status = "Couture supprimée · drapé conservé · Ctrl+Z pour annuler".into();
        }
    }
    pub(super) fn existing_seams(&mut self, ui: &mut egui::Ui) {
        let selected = self.edge_seams();
        let mut remove = None;
        for id in &selected {
            ui.colored_label(Color32::GOLD, format!("Couture : {id}"));
            let button = ui.button("Supprimer la couture").on_hover_text("Libérer les deux bords sans supprimer le tissu. Raccourci : Suppr. Annuler : Ctrl+Z.");
            self.smoke_target("remove_seam", button.rect.center());
            if button.clicked() {
                remove = Some(id.clone());
            }
        }
        let mut select = None;
        egui::CollapsingHeader::new(format!(
            "Coutures existantes ({})",
            self.project.template.seams.len()
        ))
        .id_salt("existing_seams")
        .default_open(false)
        .show(ui, |ui| {
            if self.project.template.seams.is_empty() {
                ui.small("Aucune couture. Coudre relie deux bords.");
            }
            for seam in &self.project.template.seams {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(selected.contains(&seam.id), &seam.id)
                        .clicked()
                    {
                        select = Some(seam.a.clone());
                    }
                    if ui
                        .small_button("Supprimer")
                        .on_hover_text("Supprimer cette couture uniquement")
                        .clicked()
                    {
                        remove = Some(seam.id.clone());
                    }
                });
                ui.small(format!(
                    "{} / {} ↔ {} / {}",
                    seam.a.panel,
                    seam.a.edge.as_deref().unwrap_or("bord"),
                    seam.b.panel,
                    seam.b.edge.as_deref().unwrap_or("bord")
                ));
            }
        });
        if let Some(edge) = select {
            self.playing = false;
            self.authoring.tool = Tool::Edit;
            self.authoring.ends.clear();
            self.select_boundary(edge);
        }
        if let Some(seam) = remove {
            self.remove_seam(seam);
        }
        ui.separator();
    }
}
