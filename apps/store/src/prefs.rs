use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub mode: nyx_theme::ThemeMode,
    pub theme: String,
    pub glow: bool,
    pub reduce_motion: bool,
    pub sidebar_collapsed: bool,
    pub last_section: String,
}
impl Default for Prefs {
    fn default() -> Self {
        Self {
            mode: nyx_theme::ThemeMode::Omarchy,
            theme: "tokyo-night".into(),
            glow: true,
            reduce_motion: false,
            sidebar_collapsed: false,
            last_section: "home".into(),
        }
    }
}
nyx_core::prefs_store!(Prefs);
