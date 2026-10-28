use crate::prefs;
pub use nyx_theme::*;
pub fn install() {
    nyx_theme::use_design_2();
    nyx_theme::install(Style {
        window_class: "store-window",
        base_skip: &[],
        css: include_str!("style.css"),
        no_glow_css: "",
        extra_colors: Some(swatch_colors),
        prefs: || {
            let p = prefs::get();
            ThemePrefs { mode: p.mode, theme: p.theme, glow: p.glow, reduce_motion: p.reduce_motion }
        },
        set_theme: Some(|mode, theme| {
            prefs::update(|p| {
                p.mode = mode;
                p.theme = theme;
            })
        }),
    });
}

/// The swatches every theme names (blue, violet, …) as `@store_<name>`, for the
/// colours of categories, collections and the spotlight.
fn swatch_colors(_: &Palette) -> String {
    nyx_theme::swatches().into_iter().map(|(name, colour)| format!("@define-color store_{name} {colour};\n")).collect()
}

/// The swatch a category is shown in.
pub fn tone(category: &str) -> &'static str {
    match category {
        "Internet" => "blue",
        "Productivity" => "amber",
        "Graphics" => "pink",
        "Audio & video" => "coral",
        "Development" => "teal",
        "Games" => "violet",
        "Education" => "green",
        "Security" => "red",
        _ => "graphite",
    }
}
