//! Workspace presentation; no physical state or file-format promises live here.
use super::*;

/// Workspace accent: a saturated blue that reads on both the light chrome and
/// the white pattern board.
pub(super) const ACCENT: Color32 = Color32::from_rgb(30, 136, 229);
pub(super) fn style(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.spacing.item_spacing = Vec2::new(8., 6.);
    style.spacing.button_padding = Vec2::new(10., 6.);
    style.spacing.interact_size.y = 28.;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.));
    style.visuals.panel_fill = Color32::from_rgb(0xEC, 0xEC, 0xEC);
    style.visuals.window_fill = Color32::from_rgb(0xF7, 0xF7, 0xF7);
    style.visuals.faint_bg_color = Color32::from_rgb(0xDD, 0xDD, 0xDD);
    style.visuals.extreme_bg_color = Color32::WHITE;
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.selection.stroke = Stroke::new(1., Color32::from_rgb(21, 101, 192));
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(0xDD, 0xDD, 0xDD);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0xCB, 0xDD, 0xF0);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(0xAE, 0xCC, 0xEC);
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
}
impl Atelier {
    pub(super) fn panel_browser(&mut self, ui: &mut egui::Ui) {
        let mut select = None;
        egui::CollapsingHeader::new(format!("Pièces ({})", self.project.template.panels.len()))
            .id_salt("panel_browser")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("panel_list")
                    .max_height(115.)
                    .show(ui, |ui| {
                        for panel in &self.project.template.panels {
                            if ui
                                .selectable_label(
                                    self.authoring.selected.as_ref() == Some(&panel.id),
                                    &panel.name,
                                )
                                .on_hover_text(format!(
                                    "{} · sélectionner dans les deux vues",
                                    panel.id
                                ))
                                .clicked()
                            {
                                select = Some(panel.id.clone());
                            }
                        }
                    });
            });
        if let Some(id) = select {
            self.set_authoring_tool(Tool::Edit, false);
            self.cancel_preview();
            self.authoring.selected = Some(id);
            self.authoring.edge = None;
        }
        ui.separator();
    }
    pub(super) fn export_information(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Fichiers & formats", |ui| {
            ui.label("Export disponible");
            ui.small("Projet JSON · patrons JSON · OBJ · aperçu SVG · guides de fibres");
            ui.label("Entrée disponible");
            ui.small("Projet/patrons JSON ; avatar OBJ via la ligne de commande.");
            ui.small("DXF, GLB, USD, Alembic, FBX et PDF : prévus, pas encore disponibles.");
            ui.separator();
            ui.small("Dossier d’export (fichiers remplacés à chaque export) :");
            ui.label(egui::RichText::new(self.output.display().to_string()).small());
        });
    }
}
