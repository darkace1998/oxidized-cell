//! UI themes

use eframe::egui;

/// Available themes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    /// Apply the theme to the egui context
    pub fn apply(&self, ctx: &egui::Context) {
        match self {
            Theme::Light => {
                ctx.set_visuals(egui::Visuals::light());
            }
            Theme::Dark => {
                ctx.set_visuals(egui::Visuals::dark());
            }
        }
    }

    /// Get theme name for display
    pub fn name(&self) -> &'static str {
        match self {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }

    /// Get all available themes
    pub fn all() -> &'static [Theme] {
        &[Theme::Light, Theme::Dark]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}
