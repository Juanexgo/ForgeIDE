use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{io, io::Write, path::PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub light: bool,
    pub font_size: f32,
    pub zoom: f32,
    pub compact: bool,
    pub sidebar_visible: bool,
    pub custom_theme: Option<crate::packages::Theme>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            light: false,
            font_size: 15.0,
            zoom: 1.0,
            compact: false,
            sidebar_visible: true,
            custom_theme: None,
        }
    }
}

impl Preferences {
    fn path() -> io::Result<PathBuf> {
        Ok(crate::recovery::directory()?
            .parent()
            .expect("data directory")
            .join("preferences.json"))
    }

    pub fn load() -> Self {
        let loaded = Self::path()
            .ok()
            .and_then(|path| std::fs::read(path).ok())
            .filter(|bytes| bytes.len() < 16384)
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok());
        let mut preferences = loaded.unwrap_or_default();
        if !preferences.font_size.is_finite() {
            preferences.font_size = 15.0;
        }
        if !preferences.zoom.is_finite() {
            preferences.zoom = 1.0;
        }
        preferences.font_size = preferences.font_size.clamp(10.0, 28.0);
        preferences.zoom = preferences.zoom.clamp(0.75, 1.75);
        preferences
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::path()?;
        let mut temp = tempfile::NamedTempFile::new_in(path.parent().expect("preferences parent"))?;
        temp.write_all(&serde_json::to_vec(self)?)?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|error| error.error)?;
        Ok(())
    }

    pub fn apply(&self, context: &egui::Context) {
        crate::chrome::set_theme(self.custom_theme.clone());
        crate::chrome::configure(context, self.light, self.font_size, self.compact, self.zoom);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = ui.checkbox(&mut self.light, "Light theme").changed();
        if changed {
            self.custom_theme = None;
        }
        if let Some(theme) = &self.custom_theme {
            ui.label(format!("Theme — {}", theme.name));
            if ui.button("Use built-in theme").clicked() {
                self.custom_theme = None;
                changed = true;
            }
        }
        changed |= ui.checkbox(&mut self.compact, "Compact spacing").changed();
        changed |= ui
            .checkbox(&mut self.sidebar_visible, "Show sidebar")
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.font_size, 10.0..=28.0).text("Editor font"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.zoom, 0.75..=1.75).text("Interface zoom"))
            .changed();
        changed
    }
}
