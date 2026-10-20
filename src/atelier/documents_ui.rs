//! Document lifecycle. Dialog cancellation never changes the document or saved path.
use super::*;
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult};
use std::time::Instant;

pub(super) struct DocumentUi {
    path: Option<PathBuf>,
    pub dirty: bool,
    last_recovery: Instant,
    recovery: PathBuf,
    checked_recovery: bool,
    closing: bool,
}
impl DocumentUi {
    pub fn new(output: &std::path::Path, path: Option<PathBuf>, modified: bool) -> Self {
        Self {
            dirty: path.is_none() || modified,
            path,
            last_recovery: Instant::now(),
            recovery: output.join(format!("recovery-{}.json", std::process::id())),
            checked_recovery: false,
            closing: false,
        }
    }
}
impl Atelier {
    fn has_document_draft(&self) -> bool {
        !self.authoring.draft.is_empty()
            || !self.authoring.ends.is_empty()
            || self.authoring.sewing.sides.iter().any(|s| !s.is_empty())
            || self.authoring.sewing.start.is_some()
    }
    fn pause_document(&mut self, ctx: &egui::Context) {
        self.playing = false;
        self.generation += 1;
        self.pending = false;
        self.submit(ctx, false);
    }
    fn snapshot_document(&self) -> FittingProject {
        let mut project = self.project.clone();
        project.prearrange = false;
        project.simulation = Some(self.settings.clone());
        project.state = Some(self.scene.state.clone());
        project
    }
    fn save_document(&mut self, ctx: &egui::Context, save_as: bool) -> bool {
        self.pause_document(ctx);
        let path = if save_as || self.document.path.is_none() {
            FileDialog::new()
                .add_filter("ClothCraft project", &["json"])
                .set_file_name(
                    self.document
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                        .unwrap_or("garment.clothcraft.json"),
                )
                .save_file()
        } else {
            self.document.path.clone()
        };
        let Some(path) = path else {
            return false;
        };
        if !self.commit_placement() {
            return false;
        }
        match self.snapshot_document().save(&path) {
            Ok(()) => {
                self.document.path = Some(path.clone());
                self.document.dirty = false;
                self.remove_recovery();
                self.status = format!(
                    "Saved {}{}",
                    path.display(),
                    if self.has_document_draft() {
                        " · unfinished drawing/sewing is not included"
                    } else {
                        ""
                    }
                );
                true
            }
            Err(e) => {
                self.status = format!("Save failed: {e}");
                false
            }
        }
    }
    fn can_replace_document(&mut self, ctx: &egui::Context) -> bool {
        self.pause_document(ctx);
        if !self.document.dirty && self.authoring.preview.is_none() && !self.has_document_draft() {
            return true;
        }
        match MessageDialog::new().set_title("Unsaved garment")
            .set_description("Save committed changes before continuing?\nUnfinished drawing/sewing drafts are not saved.\nYes: save · No: discard · Cancel: stay here.")
            .set_buttons(MessageButtons::YesNoCancel).show() {
            MessageDialogResult::Yes => self.save_document(ctx, false),
            MessageDialogResult::No => true,
            _ => false,
        }
    }
    fn open_document_path(&mut self, path: PathBuf, recovered: bool) {
        let result = (|| {
            let mut project = FittingProject::load(&path)?;
            let mut relinked = false;
            if !project.avatar_path.is_file() {
                let Some(avatar) = FileDialog::new()
                    .set_title("Locate missing avatar OBJ")
                    .add_filter("Avatar mesh", &["obj"])
                    .pick_file()
                else {
                    return Err("Open cancelled: avatar was not located".to_owned());
                };
                project.avatar_path = avatar;
                relinked = true;
            }
            let scene = project.compile()?;
            Ok((project, scene, relinked))
        })();
        match result {
            Ok((project, scene, relinked)) => {
                self.remove_recovery();
                self.settings = project.simulation.clone().unwrap_or_else(|| VbdSettings {
                    dt: self.prefs.dt,
                    iterations: self.prefs.iterations,
                    ..Default::default()
                });
                if let Some(poly) = project
                    .template
                    .panels
                    .iter()
                    .find_map(|p| p.polygon.as_ref())
                {
                    let m = poly.meshing_settings();
                    self.mesh_mm = m.target_edge_m * 1000.;
                    self.anisotropy = m.anisotropy;
                    self.mesh_axis = m.axis_degrees;
                }
                self.project = project;
                self.scene = scene;
                self.authoring = AuthoringUi::default();
                self.authoring.board.snap_mm = self.prefs.snap_mm;
                self.generation += 1;
                self.pending = false;
                self.playing = false;
                self.metrics = None;
                self.simulation_error = None;
                self.grab = None;
                self.drag_plane = None;
                self.document.path = (!recovered).then_some(path);
                self.document.dirty = recovered || relinked;
                self.status = if recovered {
                    "Recovery opened · Save As to keep it"
                } else {
                    "Project opened"
                }
                .into();
            }
            Err(e) => self.status = format!("Open failed; current document retained: {e}"),
        }
    }
    fn open_document(&mut self, ctx: &egui::Context, recovery: bool) {
        if !self.can_replace_document(ctx) {
            return;
        }
        let picker = FileDialog::new().add_filter("ClothCraft project", &["json"]);
        let picker = if recovery {
            picker
                .set_directory(&self.output)
                .set_title("Open a recovery-*.json snapshot")
        } else {
            picker
        };
        if let Some(path) = picker.pick_file() {
            self.open_document_path(path, recovery);
        }
    }
    fn new_document(&mut self, ctx: &egui::Context) {
        if !self.can_replace_document(ctx) {
            return;
        }
        // Start with a single unsewn, unpinned rectangle; empty solver meshes are unsupported.
        let mut p = self.project.clone();
        let command = marvelous_cloth::authoring::Command::Primitive {
            kind: marvelous_cloth::garment::shapes::Primitive::Rectangle,
            size: glam::DVec2::new(0.4, 0.6),
            sweep_degrees: 180.,
            origin: glam::DVec2::ZERO,
            meshing: Default::default(),
            fabric: p.template.fabrics[0].id.clone(),
            replace: true,
        };
        match marvelous_cloth::authoring::apply(&p.template, command) {
            Ok(edit) => {
                p.template = edit.template;
                p.state = None;
                p.mesh_signature = None;
                p.prearrange = false;
                if !self.load(p) {
                    return;
                }
                self.remove_recovery();
                self.authoring.history = Default::default();
                self.document.path = None;
                self.document.dirty = true;
                self.status = "New project · one unsewn rectangle".into();
            }
            Err(e) => self.status = e,
        }
    }
    fn remove_recovery(&mut self) {
        if self.document.recovery.is_file() {
            let _ = std::fs::remove_file(&self.document.recovery);
        }
        self.document.last_recovery = Instant::now();
    }
    pub(super) fn document_menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        for (label, action) in [
            ("New project…", 0),
            ("Open…  Ctrl+O", 1),
            ("Save  Ctrl+S", 2),
            ("Save As…  Ctrl+Shift+S", 3),
            ("Recover snapshot…", 4),
        ] {
            if ui.button(label).clicked() {
                ui.close();
                match action {
                    0 => self.new_document(&ctx),
                    1 => self.open_document(&ctx, false),
                    2 => {
                        self.save_document(&ctx, false);
                    }
                    3 => {
                        self.save_document(&ctx, true);
                    }
                    _ => self.open_document(&ctx, true),
                }
            }
        }
        ui.separator();
        self.export_information(ui);
    }
    pub(super) fn document_tick(&mut self, ctx: &egui::Context) {
        if self.smoke {
            return;
        }
        if !self.document.checked_recovery {
            self.document.checked_recovery = true;
            let previous = std::fs::read_dir(&self.output)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.starts_with("recovery-") && s.ends_with(".json"))
                })
                .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok());
            if let Some(path) = previous
                && MessageDialog::new()
                    .set_title("Recovery available")
                    .set_description(format!(
                        "Open the last recovery snapshot?\n{}",
                        path.display()
                    ))
                    .set_buttons(MessageButtons::YesNo)
                    .show()
                    == MessageDialogResult::Yes
            {
                self.open_document_path(path, true);
            }
        }
        let shortcut = ctx.input_mut(|i| {
            if i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::S,
            ) {
                3
            } else if i.consume_key(egui::Modifiers::COMMAND, egui::Key::S) {
                2
            } else if i.consume_key(egui::Modifiers::COMMAND, egui::Key::O) {
                1
            } else {
                0
            }
        });
        match shortcut {
            1 => self.open_document(ctx, false),
            2 => {
                self.save_document(ctx, false);
            }
            3 => {
                self.save_document(ctx, true);
            }
            _ => {}
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.document.closing {
            if self.can_replace_document(ctx) {
                self.document.closing = true;
                self.remove_recovery();
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        if self.document.dirty
            && self.authoring.preview.is_none()
            && self.document.last_recovery.elapsed().as_secs() >= 60
        {
            let _ = std::fs::create_dir_all(&self.output);
            match self.snapshot_document().save(&self.document.recovery) {
                Ok(()) => {}
                Err(e) => self.status = format!("Recovery save failed: {e}"),
            }
            self.document.last_recovery = Instant::now();
        }
        let name = self
            .document
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} — ClothCraft",
            name,
            if self.document.dirty { " *" } else { "" }
        )));
        ctx.request_repaint_after(Duration::from_secs(5));
    }
}
