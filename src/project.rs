use crate::{
    contact::Sphere,
    model::{Material, Mesh, Pattern, Scene, State},
    solver::SolverSettings,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Write as _, fs, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub version: u32,
    pub pattern: Pattern,
    pub material: Material,
    pub solver: SolverSettings,
    pub scene: Scene,
    pub sphere: Option<Sphere>,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            version: 1,
            pattern: Pattern::default(),
            material: Material::default(),
            solver: SolverSettings::default(),
            scene: Scene::Sphere,
            sphere: Some(Sphere::default()),
        }
    }
}
impl Project {
    pub fn build(&self) -> Result<(Mesh, State), String> {
        if self.version != 1 {
            return Err(format!("Unsupported project version {}", self.version));
        }
        self.solver.validate()?;
        if let Some(s) = &self.sphere {
            s.validate()?;
        }
        self.pattern.build(&self.material, self.scene)
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let project: Self = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        project.build()?;
        Ok(project)
    }
    /// Stores the editable rest model and settings, not a transient simulation cache.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.build()?;
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, data).map_err(|e| e.to_string())
    }
}

pub fn obj(mesh: &Mesh, state: &State) -> String {
    let mut out = String::from(
        "# MARVELOUS CLOTH; positions in metres; vt = pattern coordinates in metres\no cloth\n",
    );
    for p in &state.x {
        writeln!(out, "v {:.12} {:.12} {:.12}", p.x, p.y, p.z).unwrap();
    }
    for p in &mesh.uv {
        writeln!(out, "vt {:.12} {:.12}", p.x, p.y).unwrap();
    }
    for t in &mesh.triangles {
        let [a, b, c] = t.vertices.map(|i| i + 1);
        writeln!(out, "f {a}/{a} {b}/{b} {c}/{c}").unwrap();
    }
    out
}
